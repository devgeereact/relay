// Relay — Tauri entry point.
//
// Boots the window with the Svelte frontend and opens the local database.
// Real pipeline commands (audio, STT, detection, routing, channels) get wired
// here as each module below is built out — don't front-load functionality.

mod audio;
mod channels;
mod db;
mod detection;
mod router;
mod stt;

use audio::AudioEngine;
use channels::OutputContent;
use detection::{ContextMemory, DetectionMethod, SemanticIndex, VerseRef};
use router::{RouteDecision, Router, Thresholds};
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Mutex;
use stt::SttEngine;
use tauri::{Emitter, Manager};

/// The open SQLite connection, guarded for shared access across commands.
/// rusqlite's Connection is not Sync, so a Mutex is required in Tauri state.
struct Db(Mutex<Connection>);

/// The currently-running audio capture engine, if any.
#[derive(Default)]
struct Audio(Mutex<Option<AudioEngine>>);

/// The loaded STT engine, if a model was found at startup. None = audio-only.
struct Stt(Mutex<Option<SttEngine>>);

/// The content router — confidence gating, debounce, self-calibrating
/// thresholds. Stateful, so guarded.
#[derive(Default)]
struct Routing(Mutex<Router>);

/// Monotonic counter for output-window labels (output-1, output-2, …).
#[derive(Default)]
struct Outputs(Mutex<u32>);

/// The semantic (paraphrase) index, built once from the corpus at startup.
struct Semantic(SemanticIndex);

/// "Current passage" state for resolving bare verse references ("verse 4").
#[derive(Default)]
struct Context(Mutex<ContextMemory>);

/// Per-chunk metadata pushed to the frontend on `audio://chunk`. Deliberately
/// does NOT carry the raw samples — the console only needs level + voicing to
/// drive the meter; STT (Phase 4) consumes the samples through a separate path.
#[derive(Clone, Serialize)]
struct ChunkEvent {
    timestamp_ms: u64,
    sample_rate: u32,
    rms: f32,
    is_voice: bool,
    samples: usize,
}

fn main() {
    // Open the on-device DB at startup. Failing here is intentional and loud:
    // a broken data layer must surface before a service, never mid-sermon.
    let conn = db::open().expect("failed to open Relay database");

    tauri::Builder::default()
        .manage(Db(Mutex::new(conn)))
        .manage(Audio::default())
        .manage(Routing::default())
        .manage(Outputs::default())
        .setup(|app| {
            // Build the semantic index from the corpus once at startup, and set
            // up context-memory state (Phase 9).
            let corpus: Vec<(VerseRef, String)> = {
                let db = app.state::<Db>();
                let conn = db.0.lock().expect("db lock");
                db::all_verses(&conn)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|v| {
                        (
                            VerseRef {
                                book: v.book,
                                chapter: v.chapter,
                                verse: v.verse,
                            },
                            v.text,
                        )
                    })
                    .collect()
            };
            app.manage(Semantic(SemanticIndex::build(&corpus)));
            app.manage(Context(Mutex::new(ContextMemory::default())));

            // Load STT here (not before .run) because the worker needs an
            // AppHandle to emit transcript events. Missing model → audio-only,
            // logged but non-fatal: capture and manual override still work.
            let handle = app.handle().clone();
            let engine = match stt::default_model_path() {
                Some(path) => match SttEngine::try_load(path, move |update| {
                    let _ = handle.emit("stt://transcript", &update);
                    // Phase 5/6: detect references, then route each through the
                    // confidence gate + debounce before surfacing it.
                    emit_detections(&handle, &update.text, update.timestamp_ms);
                }) {
                    Ok(e) => {
                        println!("stt: model loaded from {}", e.model_path().display());
                        Some(e)
                    }
                    Err(e) => {
                        eprintln!("stt: {e} — running audio-only");
                        None
                    }
                },
                None => {
                    eprintln!("stt: no model found — running audio-only");
                    None
                }
            };
            app.manage(Stt(Mutex::new(engine)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            lookup_verse,
            data_health,
            list_audio_devices,
            start_capture,
            stop_capture,
            stt_status,
            confirm_detection,
            dismiss_detection,
            get_thresholds,
            set_thresholds,
            manual_fire,
            open_output_window,
            close_output_window,
            list_output_windows,
            clear_screens,
            list_templates,
            get_template,
            save_template
        ])
        .run(tauri::generate_context!())
        .expect("error while running Relay");
}

/// A routed detection pushed to the console. `status` is the router's decision
/// (auto / suggested / manual). `in_library` is false when the reference parsed
/// cleanly but isn't in the seeded corpus yet.
#[derive(Clone, Serialize)]
struct DetectionEvent {
    reference: String,
    book: String,
    chapter: i64,
    verse: i64,
    confidence: f32,
    method: DetectionMethod,
    status: &'static str,
    in_library: bool,
    text: Option<String>,
    translation: Option<String>,
}

/// Minimum semantic cosine to even consider a paraphrase candidate. Below this
/// it's noise; above, the router's suggest/auto thresholds still apply.
const SEMANTIC_FLOOR: f32 = 0.30;

/// Detect references in `text` — direct, context-resolved bare verses, and
/// semantic paraphrase — dedup them, gate each through the router, resolve
/// against the corpus, and emit one `detection://match` per survivor. Dropped
/// (debounced / low-confidence) detections are silent.
fn emit_detections(handle: &tauri::AppHandle, text: &str, now_ms: u64) {
    let db = handle.state::<Db>();
    let routing = handle.state::<Routing>();
    let ctx = handle.state::<Context>();
    let sem = handle.state::<Semantic>();
    let (Ok(conn), Ok(mut router), Ok(mut context)) = (db.0.lock(), routing.0.lock(), ctx.0.lock())
    else {
        return;
    };

    // Gather candidates: (reference, confidence, method, explicit).
    let mut candidates: Vec<(VerseRef, f32, DetectionMethod, bool)> = Vec::new();

    // 1. Direct matches — also update the current passage (context memory).
    for m in detection::detect_direct(text) {
        context.note(&m.reference);
        let explicit = m.confidence >= 0.95;
        candidates.push((m.reference, m.confidence, DetectionMethod::Direct, explicit));
    }
    // 2. Bare verses resolved against the current passage.
    for n in detection::detect_bare_verses(text) {
        if let Some(r) = context.resolve_bare_verse(n) {
            candidates.push((r, 0.88, DetectionMethod::Direct, false));
        }
    }
    // 3. Best semantic (paraphrase) candidate.
    if let Some((r, score)) = sem.0.top_k(text, 1).into_iter().next() {
        if score >= SEMANTIC_FLOOR {
            candidates.push((r, score.min(0.95), DetectionMethod::Semantic, false));
        }
    }
    if candidates.is_empty() {
        return;
    }

    // Dedup by reference, keeping the highest-confidence candidate (a direct
    // match beats a weaker semantic hit for the same verse).
    let mut best: std::collections::HashMap<String, (VerseRef, f32, DetectionMethod, bool)> =
        std::collections::HashMap::new();
    for c in candidates {
        let key = format!("{} {}:{}", c.0.book, c.0.chapter, c.0.verse);
        match best.get(&key) {
            Some(existing) if existing.1 >= c.1 => {}
            _ => {
                best.insert(key, c);
            }
        }
    }

    for (key, (r, confidence, method, explicit)) in best {
        let status = match router.decide(&key, confidence, explicit, now_ms) {
            RouteDecision::AutoFire => "auto",
            RouteDecision::Suggest => "suggested",
            RouteDecision::Drop => continue,
        };
        let looked = db::lookup_verse(&conn, &r.book, r.chapter, r.verse)
            .ok()
            .flatten();
        let vtext = looked.as_ref().map(|v| v.text.clone());
        let translation = looked.as_ref().map(|v| v.translation.clone());

        // Auto-fired content goes straight to output; suggestions wait for the
        // operator to confirm.
        if status == "auto" {
            channels::broadcast_content(
                handle,
                OutputContent {
                    reference: key.clone(),
                    text: vtext.clone(),
                    translation: translation.clone(),
                },
            );
        }
        let _ = handle.emit(
            "detection://match",
            DetectionEvent {
                reference: key,
                book: r.book.clone(),
                chapter: r.chapter,
                verse: r.verse,
                confidence,
                method,
                status,
                in_library: looked.is_some(),
                text: vtext,
                translation,
            },
        );
    }
}

// Bridge liveness probe — the frontend calls this on mount to tell whether the
// Rust core is attached (see App.svelte). Cheap, no side effects.
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Relay is running. Hello, {name}.")
}

/// Look up a verse by canonical reference for the operator console / manual
/// override. Errors are returned as strings for the frontend to surface —
/// no panics on a live path.
#[tauri::command]
fn lookup_verse(
    db: tauri::State<'_, Db>,
    book: String,
    chapter: i64,
    verse: i64,
) -> Result<Option<db::VerseRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::lookup_verse(&conn, &book, chapter, verse).map_err(|e| e.to_string())
}

/// Number of verses currently seeded — surfaced in Settings as a data-layer
/// health indicator.
#[tauri::command]
fn data_health(db: tauri::State<'_, Db>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::verse_count(&conn).map_err(|e| e.to_string())
}

/// List available audio input devices for the Settings picker.
#[tauri::command]
fn list_audio_devices() -> Vec<audio::DeviceInfo> {
    audio::list_input_devices()
}

/// Start capturing from `device` (default input when None). Each produced chunk
/// is emitted to the frontend as `audio://chunk` (metadata only). Replaces any
/// capture already running.
#[tauri::command]
fn start_capture(
    app: tauri::AppHandle,
    audio: tauri::State<'_, Audio>,
    stt: tauri::State<'_, Stt>,
    device: Option<String>,
) -> Result<(), String> {
    let mut slot = audio.0.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = slot.take() {
        engine.stop();
    }
    // Feed the same chunks to STT when a model is loaded. The sender is a clone,
    // so the persistent STT worker outlives individual capture start/stop.
    let stt_tx = stt
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(|e| e.sender());
    let emitter = app.clone();
    let engine = AudioEngine::start(device, move |chunk| {
        let _ = emitter.emit(
            "audio://chunk",
            ChunkEvent {
                timestamp_ms: chunk.timestamp_ms,
                sample_rate: chunk.sample_rate,
                rms: chunk.rms,
                is_voice: chunk.is_voice,
                samples: chunk.samples.len(),
            },
        );
        if let Some(tx) = &stt_tx {
            let _ = tx.send(chunk.clone());
        }
    })?;
    *slot = Some(engine);
    Ok(())
}

/// Stop the running capture, if any. Idempotent. Leaves the STT worker loaded.
#[tauri::command]
fn stop_capture(audio: tauri::State<'_, Audio>) -> Result<(), String> {
    let mut slot = audio.0.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = slot.take() {
        engine.stop();
    }
    Ok(())
}

/// Whether a local STT model is loaded, and its path — surfaced in Settings.
#[tauri::command]
fn stt_status(stt: tauri::State<'_, Stt>) -> Result<StatusStt, String> {
    let slot = stt.0.lock().map_err(|e| e.to_string())?;
    Ok(match slot.as_ref() {
        Some(e) => StatusStt {
            loaded: true,
            model: Some(e.model_path().display().to_string()),
        },
        None => StatusStt {
            loaded: false,
            model: None,
        },
    })
}

#[derive(Clone, Serialize)]
struct StatusStt {
    loaded: bool,
    model: Option<String>,
}

/// Operator confirmed a suggestion — fire it to the output channels and feed the
/// self-calibrating gate. Returns updated thresholds so Settings reflects the nudge.
#[tauri::command]
fn confirm_detection(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    routing: tauri::State<'_, Routing>,
    reference: String,
) -> Result<Thresholds, String> {
    if let Some(m) = detection::detect_direct(&reference).into_iter().next() {
        let r = &m.reference;
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let looked = db::lookup_verse(&conn, &r.book, r.chapter, r.verse)
            .ok()
            .flatten();
        channels::broadcast_content(
            &app,
            OutputContent {
                reference: format!("{} {}:{}", r.book, r.chapter, r.verse),
                text: looked.as_ref().map(|v| v.text.clone()),
                translation: looked.as_ref().map(|v| v.translation.clone()),
            },
        );
    }
    let mut router = routing.0.lock().map_err(|e| e.to_string())?;
    router.record_feedback(true);
    Ok(router.thresholds())
}

/// Operator rejected an auto-fired detection (undo). Tightens the gate.
#[tauri::command]
fn dismiss_detection(routing: tauri::State<'_, Routing>) -> Result<Thresholds, String> {
    let mut router = routing.0.lock().map_err(|e| e.to_string())?;
    router.record_feedback(false);
    Ok(router.thresholds())
}

/// Current gate thresholds — for the Settings sliders.
#[tauri::command]
fn get_thresholds(routing: tauri::State<'_, Routing>) -> Result<Thresholds, String> {
    let router = routing.0.lock().map_err(|e| e.to_string())?;
    Ok(router.thresholds())
}

/// Manual override of the thresholds (the always-available slider, DECISIONS.md).
#[tauri::command]
fn set_thresholds(
    routing: tauri::State<'_, Routing>,
    thresholds: Thresholds,
) -> Result<Thresholds, String> {
    let mut router = routing.0.lock().map_err(|e| e.to_string())?;
    router.set_thresholds(thresholds);
    Ok(router.thresholds())
}

/// Operator manual override: fire a free-text reference now, bypassing the gate.
/// First-class control (CLAUDE.md) — parses the reference, resolves it, and
/// emits a `detection://match` with status "manual".
#[tauri::command]
fn manual_fire(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    routing: tauri::State<'_, Routing>,
    reference: String,
) -> Result<(), String> {
    let m = detection::detect_direct(&reference)
        .into_iter()
        .next()
        .ok_or_else(|| format!("could not parse a reference from \"{reference}\""))?;
    let r = &m.reference;
    let key = format!("{} {}:{}", r.book, r.chapter, r.verse);

    let looked = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::lookup_verse(&conn, &r.book, r.chapter, r.verse)
            .ok()
            .flatten()
    };
    {
        let mut router = routing.0.lock().map_err(|e| e.to_string())?;
        router.manual_fire(&key, 0);
    }
    let text = looked.as_ref().map(|v| v.text.clone());
    let translation = looked.as_ref().map(|v| v.translation.clone());
    // Manual override fires straight to output.
    channels::broadcast_content(
        &app,
        OutputContent {
            reference: key.clone(),
            text: text.clone(),
            translation: translation.clone(),
        },
    );
    let _ = app.emit(
        "detection://match",
        DetectionEvent {
            reference: key,
            book: r.book.clone(),
            chapter: r.chapter,
            verse: r.verse,
            confidence: 1.0,
            method: m.method.clone(),
            status: "manual",
            in_library: looked.is_some(),
            text,
            translation,
        },
    );
    Ok(())
}

/// Open a native fullscreen output window rendering template `template_id`.
/// Returns the window's label. Multiple channels can be open at once.
#[tauri::command]
fn open_output_window(
    app: tauri::AppHandle,
    outputs: tauri::State<'_, Outputs>,
    template_id: i64,
    name: Option<String>,
) -> Result<String, String> {
    let name = name.unwrap_or_else(|| "Output".into());
    let label = {
        let mut n = outputs.0.lock().map_err(|e| e.to_string())?;
        *n += 1;
        format!("output-{n}")
    };
    channels::open_native_window(&app, &label, template_id, &name)?;
    Ok(label)
}

/// All output templates (Templates tab, Channels tab).
#[tauri::command]
fn list_templates(db: tauri::State<'_, Db>) -> Result<Vec<db::Template>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_templates(&conn).map_err(|e| e.to_string())
}

/// A single template by id (fetched by each output window on load).
#[tauri::command]
fn get_template(db: tauri::State<'_, Db>, id: i64) -> Result<Option<db::Template>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::get_template(&conn, id).map_err(|e| e.to_string())
}

/// Save a template (insert or update). Broadcasts `template://updated` so any
/// open output window on that template re-renders live. Returns the id.
#[tauri::command]
fn save_template(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    template: db::Template,
) -> Result<i64, String> {
    let id = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::upsert_template(&conn, &template).map_err(|e| e.to_string())?
    };
    let _ = app.emit("template://updated", id);
    Ok(id)
}

/// Close an output window by label.
#[tauri::command]
fn close_output_window(app: tauri::AppHandle, label: String) -> Result<(), String> {
    channels::close_window(&app, &label)
}

/// Labels of currently-open output windows.
#[tauri::command]
fn list_output_windows(app: tauri::AppHandle) -> Vec<String> {
    channels::list_open(&app)
}

/// Operator "Clear all screens" — blank every output channel.
#[tauri::command]
fn clear_screens(app: tauri::AppHandle) {
    channels::clear(&app);
}
