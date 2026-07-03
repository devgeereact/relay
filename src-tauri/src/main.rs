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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;
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

/// Whether automatic detection is armed. Off = the pipeline still transcribes,
/// but no auto-fire/suggest reaches the console; manual override is unaffected
/// (it bypasses this entirely — a first-class control, CLAUDE.md).
struct Detecting(AtomicBool);

/// The in-progress service being recorded to local history, if any.
struct SessionState {
    id: i64,
    started: Instant,
    last_transcript: Option<i64>,
}

/// Current service-session state (None = not recording).
#[derive(Default)]
struct Session(Mutex<Option<SessionState>>);

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
        .manage(Detecting(AtomicBool::new(true)))
        .manage(Session::default())
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

            // Start the kiosk WebSocket server (network_client render target) on
            // the reserved api port. Kiosks on the LAN connect here for state.
            let kiosk = channels::KioskHub::default();
            let kiosk_tx = kiosk.sender();
            app.manage(kiosk);
            tauri::async_runtime::spawn(channels::run_kiosk_server(kiosk_tx, 8031));

            // Load STT here (not before .run) because the worker needs an
            // AppHandle to emit transcript events. Missing model → audio-only,
            // logged but non-fatal: capture and manual override still work.
            let handle = app.handle().clone();
            let engine = match stt::default_model_path() {
                Some(path) => match SttEngine::try_load(path, move |update| {
                    let _ = handle.emit("stt://transcript", &update);
                    if update.is_final {
                        println!("stt[{}]: {}", update.language, update.text);
                        persist_transcript(&handle, &update.text, &update.language);
                        // Spoken "next"/"back" navigates from the current verse.
                        if let Some(cmd) = detection::detect_command(&update.text) {
                            handle_nav(&handle, cmd);
                            return;
                        }
                    }
                    // Detect references, then route each through the confidence
                    // gate + debounce before surfacing it.
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
            list_output_channels,
            set_channel_template,
            clear_screens,
            set_detection_enabled,
            get_detection_enabled,
            nav,
            start_service,
            end_service,
            current_service,
            list_services,
            service_detail,
            export_service,
            list_templates,
            get_template,
            save_template,
            set_stt_language,
            open_ndi_output
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
    // Detection disarmed → transcribe but surface nothing. Manual override is a
    // separate path and stays live.
    if !handle.state::<Detecting>().0.load(Ordering::Relaxed) {
        return;
    }
    let db = handle.state::<Db>();
    let routing = handle.state::<Routing>();
    let ctx = handle.state::<Context>();
    let sem = handle.state::<Semantic>();

    // Compute everything UNDER the locks, but collect the emits/broadcasts and
    // fire them AFTER releasing — never hold a lock across handle.emit /
    // broadcast_content, which can otherwise deadlock the main run loop with a
    // command contending the same lock (this was the freeze on Start listening).
    let mut events: Vec<DetectionEvent> = Vec::new();
    let mut broadcasts: Vec<OutputContent> = Vec::new();
    {
        let (Ok(conn), Ok(mut router), Ok(mut context)) =
            (db.0.lock(), routing.0.lock(), ctx.0.lock())
        else {
            return;
        };

        // Gather candidates: (reference, confidence, method, explicit).
        let mut candidates: Vec<(VerseRef, f32, DetectionMethod, bool)> = Vec::new();

        let directs = detection::detect_direct(text);
        let direct_empty = directs.is_empty();
        for m in directs {
            let explicit = m.confidence >= 0.95;
            candidates.push((m.reference, m.confidence, DetectionMethod::Direct, explicit));
        }
        for n in detection::detect_bare_verses(text) {
            if let Some(r) = context.resolve_bare_verse(n) {
                candidates.push((r, 0.88, DetectionMethod::Direct, false));
            }
        }
        if let Some((r, score)) = sem.0.top_k(text, 1).into_iter().next() {
            if score >= SEMANTIC_FLOOR {
                candidates.push((r, score.min(0.95), DetectionMethod::Semantic, false));
            }
        }
        if direct_empty {
            for r in detection::detect_ambiguous(text) {
                candidates.push((r, 0.70, DetectionMethod::Direct, false));
            }
        }
        if candidates.is_empty() {
            return;
        }

        // Dedup by reference, keeping the highest-confidence candidate.
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

            if status == "auto" {
                context.note(&r);
                let method_str = match method {
                    DetectionMethod::Direct => "direct",
                    DetectionMethod::Semantic => "semantic",
                };
                persist_fire(
                    &conn,
                    handle.state::<Session>(),
                    looked.as_ref().map(|v| v.id),
                    method_str,
                    confidence,
                    text,
                );
                broadcasts.push(OutputContent {
                    reference: key.clone(),
                    text: vtext.clone(),
                    translation: translation.clone(),
                });
            }
            events.push(DetectionEvent {
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
            });
        }
    } // locks released here

    for content in broadcasts {
        channels::broadcast_content(handle, content);
    }
    for ev in events {
        let _ = handle.emit("detection://match", ev);
    }
}

/// Handle a spoken navigation command ("next" / "back"): fire the next/previous
/// verse relative to the current on-screen verse. Bypasses the gate (operator
/// intent), like a manual override. Locks db before ctx/router (global order).
fn handle_nav(handle: &tauri::AppHandle, dir: detection::NavCommand) {
    let db = handle.state::<Db>();
    let ctx = handle.state::<Context>();

    // All lock work in one scope; broadcast/emit happen AFTER releasing (no
    // lock held across emit — see emit_detections).
    let fired: Option<(String, VerseRef, String, String)> = {
        let Ok(conn) = db.0.lock() else {
            return;
        };
        let target = {
            let Ok(context) = ctx.0.lock() else {
                return;
            };
            match dir {
                detection::NavCommand::Next => context.next_verse(),
                detection::NavCommand::Previous => context.prev_verse(),
            }
        };
        let Some(r) = target else { return };
        let Some(v) = db::lookup_verse(&conn, &r.book, r.chapter, r.verse)
            .ok()
            .flatten()
        else {
            return; // stepped off the end of the chapter
        };
        let key = format!("{} {}:{}", r.book, r.chapter, r.verse);
        if let Ok(mut context) = ctx.0.lock() {
            context.note(&r);
        }
        if let Ok(mut router) = handle.state::<Routing>().0.lock() {
            router.manual_fire(&key, 0);
        }
        persist_fire(
            &conn,
            handle.state::<Session>(),
            Some(v.id),
            "direct",
            1.0,
            &key,
        );
        Some((key, r, v.text, v.translation))
    };

    if let Some((key, r, text, translation)) = fired {
        channels::broadcast_content(
            handle,
            OutputContent {
                reference: key.clone(),
                text: Some(text.clone()),
                translation: Some(translation.clone()),
            },
        );
        let _ = handle.emit(
            "detection://match",
            DetectionEvent {
                reference: key,
                book: r.book,
                chapter: r.chapter,
                verse: r.verse,
                confidence: 1.0,
                method: DetectionMethod::Direct,
                status: "auto",
                in_library: true,
                text: Some(text),
                translation: Some(translation),
            },
        );
    }
}

/// Persist a finalized transcript line into the current service (if recording),
/// updating the session's last-transcript id for detection linkage. Locks its
/// own db handle — call OUTSIDE any held db lock.
fn persist_transcript(handle: &tauri::AppHandle, text: &str, language: &str) {
    let db = handle.state::<Db>();
    let session = handle.state::<Session>();
    // Consistent lock order everywhere: db before session (see persist_fire,
    // which is called while db is already held) — avoids a lock-ordering deadlock.
    let (Ok(conn), Ok(mut sess)) = (db.0.lock(), session.0.lock()) else {
        return;
    };
    if let Some(st) = sess.as_mut() {
        let ts = st.started.elapsed().as_secs_f64();
        if let Ok(tid) = db::insert_transcript(&conn, st.id, ts, text, language, None) {
            st.last_transcript = Some(tid);
        }
    }
}

/// Persist a fired detection into the current service, using an already-held db
/// connection (avoids re-locking). Creates a transcript row if none exists yet.
fn persist_fire(
    conn: &Connection,
    session: tauri::State<'_, Session>,
    verse_id: Option<i64>,
    method: &str,
    confidence: f32,
    window_text: &str,
) {
    let Ok(mut sess) = session.0.lock() else {
        return;
    };
    let Some(st) = sess.as_mut() else {
        return; // not recording
    };
    let ts = st.started.elapsed().as_secs_f64();
    let tid = match st.last_transcript {
        Some(t) => t,
        None => match db::insert_transcript(conn, st.id, ts, window_text, "en", None) {
            Ok(t) => {
                st.last_transcript = Some(t);
                t
            }
            Err(_) => return,
        },
    };
    let _ = db::insert_detection(conn, tid, verse_id, method, confidence, "auto", Some(ts));
}

/// Record an operator cue (manual_override / clear_screens) into the current
/// service. Locks its own db handle — call outside a held db lock.
fn persist_cue(handle: &tauri::AppHandle, cue_type: &str, payload: Option<&str>) {
    let db = handle.state::<Db>();
    let session = handle.state::<Session>();
    let (Ok(conn), Ok(sess)) = (db.0.lock(), session.0.lock()) else {
        return;
    };
    if let Some(st) = sess.as_ref() {
        let ts = st.started.elapsed().as_secs_f64();
        let _ = db::insert_cue(&conn, st.id, cue_type, payload, ts);
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
async fn start_capture(
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
    let err_emitter = app.clone();
    // Throttle the level-meter event: chunks arrive ~5/sec but the UI only needs
    // a couple updates/sec. Flooding the webview with events is a real freeze
    // risk. STT still gets EVERY chunk.
    let chunk_n = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    // Non-blocking: returns instantly, so the UI thread never stalls on device
    // init. Stream failures surface as `audio://error`.
    let engine = AudioEngine::start(
        device,
        move |chunk| {
            let n = chunk_n.fetch_add(1, Ordering::Relaxed);
            if n.is_multiple_of(3) {
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
            }
            if let Some(tx) = &stt_tx {
                let _ = tx.send(chunk.clone());
            }
        },
        move |err| {
            eprintln!("audio: {err}");
            let _ = err_emitter.emit("audio://error", err);
        },
    );
    *slot = Some(engine);
    Ok(())
}

/// Stop the running capture, if any. Idempotent. Leaves the STT worker loaded.
#[tauri::command]
async fn stop_capture(audio: tauri::State<'_, Audio>) -> Result<(), String> {
    let mut slot = audio.0.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = slot.take() {
        engine.stop();
    }
    Ok(())
}

/// Whether a local STT model is loaded, its path, and the current language
/// setting (None = auto-detect / code-switching) — surfaced in Settings.
#[tauri::command]
fn stt_status(stt: tauri::State<'_, Stt>) -> Result<StatusStt, String> {
    let slot = stt.0.lock().map_err(|e| e.to_string())?;
    Ok(match slot.as_ref() {
        Some(e) => StatusStt {
            loaded: true,
            model: Some(e.model_path().display().to_string()),
            language: e.language(),
        },
        None => StatusStt {
            loaded: false,
            model: None,
            language: None,
        },
    })
}

/// Set the STT language: a code ("yo"/"sw"/"ha"/"en"/…) or null for auto-detect
/// (code-switching). Tier-1 targets: Yoruba, Swahili, Hausa (CLAUDE.md).
#[tauri::command]
fn set_stt_language(stt: tauri::State<'_, Stt>, language: Option<String>) -> Result<(), String> {
    let slot = stt.0.lock().map_err(|e| e.to_string())?;
    if let Some(e) = slot.as_ref() {
        e.set_language(language);
    }
    Ok(())
}

#[derive(Clone, Serialize)]
struct StatusStt {
    loaded: bool,
    model: Option<String>,
    language: Option<String>,
}

/// NDI render target — not yet available. Honest seam: NDI needs the
/// proprietary NDI SDK (native lib + FFI, no pure-Rust crate), which isn't
/// bundled. Returns a clear error rather than pretending. Integration path:
/// install the NDI SDK, add FFI bindings, render each channel's template to an
/// off-screen surface, and publish it as an NDI source. See docs/SPEC.md §9.
#[tauri::command]
fn open_ndi_output(_template_id: i64) -> Result<String, String> {
    Err(
        "NDI output is not yet available — it requires the NDI SDK (Phase 10, \
         parked). Use a native output window, or point OBS/vMix at a kiosk \
         (network) channel for now."
            .into(),
    )
}

/// Operator confirmed a suggestion — fire it to the output channels and feed the
/// self-calibrating gate. Returns updated thresholds so Settings reflects the nudge.
#[tauri::command]
fn confirm_detection(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    routing: tauri::State<'_, Routing>,
    ctx: tauri::State<'_, Context>,
    session: tauri::State<'_, Session>,
    reference: String,
) -> Result<Thresholds, String> {
    if let Some(m) = detection::detect_direct(&reference).into_iter().next() {
        let r = &m.reference;
        let key = format!("{} {}:{}", r.book, r.chapter, r.verse);
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let looked = db::lookup_verse(&conn, &r.book, r.chapter, r.verse)
            .ok()
            .flatten();
        channels::broadcast_content(
            &app,
            OutputContent {
                reference: key.clone(),
                text: looked.as_ref().map(|v| v.text.clone()),
                translation: looked.as_ref().map(|v| v.translation.clone()),
            },
        );
        if let Ok(mut context) = ctx.0.lock() {
            context.note(r);
        }
        persist_fire(
            &conn,
            session,
            looked.as_ref().map(|v| v.id),
            "direct",
            m.confidence,
            &key,
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
    session: tauri::State<'_, Session>,
    ctx: tauri::State<'_, Context>,
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
    if let Ok(mut context) = ctx.0.lock() {
        context.note(r);
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
            reference: key.clone(),
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

    // Record to the service: the fired verse (as a detection) + the override cue.
    let method_str = match m.method {
        DetectionMethod::Semantic => "semantic",
        DetectionMethod::Direct => "direct",
    };
    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        persist_fire(
            &conn,
            session,
            looked.as_ref().map(|v| v.id),
            method_str,
            1.0,
            &key,
        );
    }
    persist_cue(&app, "manual_override", Some(&key));
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

/// All configured output channels (Channels tab).
#[tauri::command]
fn list_output_channels(db: tauri::State<'_, Db>) -> Result<Vec<db::OutputChannel>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_output_channels(&conn).map_err(|e| e.to_string())
}

/// Assign a template to a channel — outputs are freely assignable.
#[tauri::command]
fn set_channel_template(db: tauri::State<'_, Db>, id: i64, template_id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::set_channel_template(&conn, id, template_id).map_err(|e| e.to_string())
}

/// Operator "Clear all screens" — blank every output channel.
#[tauri::command]
fn clear_screens(app: tauri::AppHandle) {
    channels::clear(&app);
    persist_cue(&app, "clear_screens", None);
}

/// Manual next/previous verse (console buttons) — same path as the spoken
/// "next"/"back" command.
#[tauri::command]
fn nav(app: tauri::AppHandle, direction: String) {
    let dir = if direction == "previous" || direction == "back" {
        detection::NavCommand::Previous
    } else {
        detection::NavCommand::Next
    };
    handle_nav(&app, dir);
}

/// Start (or resume) recording a service. If one is already active it's reused
/// so pause/resume of capture doesn't fragment history. Returns the service id.
#[tauri::command]
fn start_service(
    session: tauri::State<'_, Session>,
    db: tauri::State<'_, Db>,
    title: String,
    date: String,
) -> Result<i64, String> {
    // db before session (consistent global lock order — see persist_transcript).
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut sess = session.0.lock().map_err(|e| e.to_string())?;
    if let Some(st) = sess.as_ref() {
        return Ok(st.id);
    }
    let id = db::create_service(&conn, &date, &title).map_err(|e| e.to_string())?;
    *sess = Some(SessionState {
        id,
        started: Instant::now(),
        last_transcript: None,
    });
    Ok(id)
}

/// Stop recording the current service (history is kept).
#[tauri::command]
fn end_service(session: tauri::State<'_, Session>) -> Result<(), String> {
    *session.0.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}

/// Id of the service currently being recorded, if any.
#[tauri::command]
fn current_service(session: tauri::State<'_, Session>) -> Result<Option<i64>, String> {
    Ok(session
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(|s| s.id))
}

/// All services for the Library list, newest first.
#[tauri::command]
fn list_services(db: tauri::State<'_, Db>) -> Result<Vec<db::ServiceSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_services(&conn).map_err(|e| e.to_string())
}

/// Full transcript + fired detections for one service (Library detail view).
#[tauri::command]
fn service_detail(db: tauri::State<'_, Db>, id: i64) -> Result<ServiceDetail, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(ServiceDetail {
        transcripts: db::service_transcripts(&conn, id).map_err(|e| e.to_string())?,
        detections: db::service_detections(&conn, id).map_err(|e| e.to_string())?,
    })
}

/// Export a service as a Markdown file (transcript + detected verses) to the
/// user's Downloads folder. Returns the written path. Uses std::fs — no fs
/// plugin needed; nothing leaves the device.
#[tauri::command]
fn export_service(db: tauri::State<'_, Db>, id: i64) -> Result<String, String> {
    let (summary, transcripts, detections) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let summary = db::list_services(&conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("service {id} not found"))?;
        let transcripts = db::service_transcripts(&conn, id).map_err(|e| e.to_string())?;
        let detections = db::service_detections(&conn, id).map_err(|e| e.to_string())?;
        (summary, transcripts, detections)
    };

    let mut md = String::new();
    md.push_str(&format!("# {}\n\n", summary.title));
    md.push_str(&format!(
        "{} · {} · {} verses · {} overrides\n\n",
        summary.date,
        fmt_secs(summary.duration_secs),
        summary.verses,
        summary.overrides
    ));
    md.push_str("## Detected verses\n\n");
    if detections.is_empty() {
        md.push_str("_None._\n\n");
    } else {
        for d in &detections {
            md.push_str(&format!(
                "- **{}** — {} {:.2} @ {}\n",
                d.reference.as_deref().unwrap_or("unresolved"),
                d.method,
                d.confidence,
                fmt_secs(d.fired_at)
            ));
        }
        md.push('\n');
    }
    md.push_str("## Transcript\n\n");
    if transcripts.is_empty() {
        md.push_str("_No transcript recorded._\n");
    } else {
        for t in &transcripts {
            md.push_str(&format!(
                "`{}` ({}) {}\n\n",
                fmt_secs(t.timestamp),
                t.language,
                t.text
            ));
        }
    }

    // Sanitize a filename and write to Downloads (fallback: app-data/exports).
    let safe: String = summary
        .title
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let filename = format!("relay-{}-{}.md", safe, summary.date);
    let home = std::env::var_os("HOME").ok_or("no HOME")?;
    let downloads = std::path::PathBuf::from(&home).join("Downloads");
    let dir = if downloads.is_dir() {
        downloads
    } else {
        let d = std::path::PathBuf::from(&home)
            .join("Library/Application Support/com.relay.app/exports");
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        d
    };
    let path = dir.join(filename);
    std::fs::write(&path, md).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// Format seconds as m:ss.
fn fmt_secs(secs: f64) -> String {
    let s = secs.max(0.0) as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

#[derive(Clone, Serialize)]
struct ServiceDetail {
    transcripts: Vec<db::TranscriptRow>,
    detections: Vec<db::ServiceDetection>,
}

/// Arm/disarm automatic detection. Returns the new state.
#[tauri::command]
fn set_detection_enabled(detecting: tauri::State<'_, Detecting>, enabled: bool) -> bool {
    detecting.0.store(enabled, Ordering::Relaxed);
    enabled
}

/// Whether automatic detection is currently armed.
#[tauri::command]
fn get_detection_enabled(detecting: tauri::State<'_, Detecting>) -> bool {
    detecting.0.load(Ordering::Relaxed)
}
