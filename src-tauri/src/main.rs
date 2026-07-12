// Relay — Tauri entry point.
//
// Boots the window with the Svelte frontend and opens the local database.
// Real pipeline commands (audio, STT, detection, routing, channels) get wired
// here as each module below is built out — don't front-load functionality.

mod audio;
mod channels;
mod db;
mod detection;
mod dsp;
mod models;
mod pipeline;
mod proimport;
mod router;
mod songs;
mod stt;
mod telemetry;

use audio::AudioEngine;
use channels::OutputContent;
use detection::{ContextMemory, DetectionMethod, SemanticIndex, VerseRef};
use pipeline::{Cand, DetectionEvent, Fire, FireStatus};
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
        .manage(models::DownloadState::default())
        .setup(|app| {
            // Crash reporting: OFF unless the operator previously opted in. This
            // runs before anything else can panic, but deliberately after the DB
            // is open, because the consent lives in the DB. No consent → no DSN,
            // no client, no network stack at all.
            {
                let consent = {
                    let db = app.state::<Db>();
                    let conn = db.0.lock().expect("db lock");
                    let on = db::get_setting(&conn, telemetry::ENABLED_KEY)
                        .ok()
                        .flatten()
                        .as_deref()
                        == Some("1");
                    let dsn = db::get_setting(&conn, telemetry::DSN_KEY)
                        .ok()
                        .flatten()
                        .unwrap_or_default();
                    on.then_some(dsn)
                };
                if let Some(dsn) = consent {
                    telemetry::enable(&dsn, env!("CARGO_PKG_VERSION"));
                }
            }
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
            let kiosk_templates = kiosk.templates_handle();
            // Warm the template cache so a browser client (OBS/kiosk) gets the
            // REAL saved template immediately on connect (matches the editor).
            {
                let db = app.state::<Db>();
                let tpls =
                    db.0.lock()
                        .ok()
                        .and_then(|conn| db::list_templates(&conn).ok())
                        .unwrap_or_default();
                for t in &tpls {
                    if let Ok(j) = serde_json::to_string(t) {
                        kiosk.cache_template(t.id, &j);
                    }
                }
            }
            app.manage(kiosk);
            tauri::async_runtime::spawn(channels::run_kiosk_server(
                channels::report_to(app.handle()),
                kiosk_tx,
                kiosk_templates,
                8031,
            ));
            // Serve the output/stage pages over LAN HTTP so other devices load
            // them in a packaged app (not only in `tauri dev`). See channels.rs.
            tauri::async_runtime::spawn(channels::run_output_http_server(
                channels::report_to(app.handle()),
                8032,
            ));

            // Load STT here (not before .run) because the worker needs an
            // AppHandle to emit transcript events. Missing model → audio-only,
            // logged but non-fatal: capture and manual override still work.
            let engine = build_stt(app.handle());
            // Phase B: apply the active voice profile at startup — language +
            // decoder-bias prompt to STT, calibrated thresholds to the router —
            // so accent calibration is live from the first word, before any UI.
            {
                let profile = {
                    let db = app.state::<Db>();
                    let conn = db.0.lock().expect("db lock");
                    db::active_voice_profile(&conn).ok().flatten()
                };
                if let Some(p) = profile {
                    if let Some(e) = engine.as_ref() {
                        apply_profile_to_stt(e, &p);
                    }
                    let routing = app.state::<Routing>();
                    if let Ok(mut r) = routing.0.lock() {
                        // Learned thresholds, then re-anchor the decay baseline to
                        // the dial (see apply_profile — same two-step, same reason).
                        r.set_thresholds(Thresholds {
                            auto_fire: p.auto_fire as f32,
                            suggest: p.suggest as f32,
                        });
                        r.set_baseline(Thresholds::from_sensitivity(
                            p.sensitivity.clamp(0, 100) as u8
                        ));
                    }
                    println!(
                        "profile: active '{}' · lang {:?} · sensitivity {}",
                        p.name, p.language, p.sensitivity
                    );
                }
            }
            app.manage(Stt(Mutex::new(engine)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            lookup_verse,
            search_scripture,
            list_plans,
            create_plan,
            delete_plan,
            duplicate_plan,
            plan_items,
            add_plan_item,
            remove_plan_item,
            move_plan_item,
            set_plan_note,
            reorder_plan,
            list_songs,
            search_songs,
            get_song,
            import_song,
            save_song,
            delete_song,
            start_countdown,
            list_arrangements,
            save_arrangement,
            delete_arrangement,
            import_pro,
            parse_import,
            save_reviewed_songs,
            list_saved_scripture,
            save_scripture,
            delete_saved_scripture,
            list_announcements,
            save_announcement,
            delete_announcement,
            list_media,
            import_media,
            delete_media,
            fire_content,
            fire_media,
            get_content_templates,
            set_content_template,
            data_health,
            list_audio_devices,
            local_ip,
            start_capture,
            stop_capture,
            stt_status,
            confirm_detection,
            dismiss_detection,
            get_thresholds,
            set_thresholds,
            get_crash_reporting,
            set_crash_reporting,
            list_models,
            download_model,
            cancel_model_download,
            load_stt_model,
            manual_fire,
            open_output_window,
            close_output_window,
            list_output_windows,
            list_output_channels,
            set_channel_template,
            list_monitors,
            open_channel_output,
            set_channel_display,
            add_channel,
            delete_channel,
            clear_screens,
            blackout,
            set_stage_next,
            push_announcement,
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
            list_active_templates,
            set_template_active,
            create_template,
            delete_template,
            get_template,
            save_template,
            set_stt_language,
            list_translations,
            get_active_translation,
            set_active_translation,
            list_voice_profiles,
            active_voice_profile,
            create_voice_profile,
            update_voice_profile,
            select_voice_profile,
            delete_voice_profile,
            related_scripture,
            verse_repeat_count,
            open_ndi_output
        ])
        .run(tauri::generate_context!())
        .expect("error while running Relay");
}

/// Minimum semantic cosine to even consider a paraphrase candidate. Below this
/// it's noise; above, the router's suggest/auto thresholds still apply.
const SEMANTIC_FLOOR: f32 = 0.30;

/// Resolve the inclusive last verse to stage for a candidate: the explicit range
/// end, or the chapter's last verse for a whole-chapter reference, or None for a
/// single verse (the walk then just steps until the chapter runs out).
fn passage_end(conn: &Connection, c: &Cand) -> Option<i64> {
    if c.whole_chapter {
        db::chapter_last_verse(conn, &c.r.book, c.r.chapter)
            .ok()
            .flatten()
    } else {
        c.verse_end
    }
}

/// Look up a verse and its scripture template, and assemble the `Fire` that
/// describes what the screens will show.
///
/// THE single place a verse becomes screen content. Every fire path goes through
/// here, which is what guarantees they all carry the scripture template — the nav
/// paths used to build their broadcast by hand and forget it, so a verse reached
/// by saying "next" rendered differently from the same verse reached by saying
/// its reference. Caller holds the Db lock; this does no locking of its own.
#[allow(clippy::too_many_arguments)]
fn resolve_fire(
    conn: &Connection,
    r: VerseRef,
    confidence: f32,
    method: DetectionMethod,
    status: FireStatus,
    stage_note: Option<String>,
) -> Fire {
    let looked = db::lookup_verse(conn, &r.book, r.chapter, r.verse)
        .ok()
        .flatten();
    let (template_id, template_json) = content_tpl(conn, "scripture");
    Fire {
        key: Fire::key_for(&r),
        reference: r,
        verse_id: looked.as_ref().map(|v| v.id),
        text: looked.as_ref().map(|v| v.text.clone()),
        translation: looked.as_ref().map(|v| v.translation.clone()),
        confidence,
        method,
        status,
        stage_note,
        template_id,
        template_json,
    }
}

/// How a fire updates the passage context (what "next" will walk to).
enum PassageUpdate {
    /// A fresh reference — stage its passage span ("Psalm 23" → the whole chapter).
    Note(Option<i64>),
    /// A step within the passage already staged — keep the span, move the cursor.
    Advance,
    /// A jump inside the current book — new position, no new span.
    Jump,
}

/// Put a verse on the screens because a HUMAN said so.
///
/// Shared by every operator-driven path — the manual reference box, a spoken
/// "next"/"back", and a spoken in-passage jump. Those three were three separate
/// ~70-line functions that did the same six things in the same order; two of them
/// (`handle_nav` / `handle_passage_nav`) were near-identical twins that had
/// already drifted apart from the third.
///
/// Bypasses the gate entirely: operator override is a first-class control and
/// must always win (CLAUDE.md). Follows the lock rules — all DB work under the
/// lock, then RELEASE, then broadcast/emit. Never hold a lock across `emit`.
fn fire_manual(
    handle: &tauri::AppHandle,
    r: VerseRef,
    confidence: f32,
    update: PassageUpdate,
    stage_note: Option<String>,
) -> bool {
    let db = handle.state::<Db>();
    let ctx = handle.state::<Context>();

    let fire = {
        let Ok(conn) = db.0.lock() else { return false };
        let f = resolve_fire(
            &conn,
            r,
            confidence,
            DetectionMethod::Direct,
            FireStatus::Manual,
            stage_note,
        );
        // Not in the corpus → leave the screen exactly as it is. Better to show
        // the previous verse than to blank the wall mid-sentence. Same rule the
        // AI path uses (`Fire::may_broadcast`).
        if !f.may_broadcast() {
            return false;
        }
        if let Ok(mut context) = ctx.0.lock() {
            match update {
                PassageUpdate::Note(end) => context.note_passage(&f.reference, end),
                PassageUpdate::Advance => context.advance(&f.reference),
                PassageUpdate::Jump => context.note(&f.reference),
            }
        }
        if let Ok(mut router) = handle.state::<Routing>().0.lock() {
            router.manual_fire(&f.key, 0);
        }
        persist_fire(
            &conn,
            handle.state::<Session>(),
            f.verse_id,
            f.method.db_method(),
            f.confidence,
            f.status.as_str(),
            &f.key,
        );
        f
    }; // locks released BEFORE the emit below — CLAUDE.md rule #2.

    channels::broadcast_content(handle, fire.output());
    let _ = handle.emit("detection://match", fire.event());
    true
}

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

        // Gather candidates.
        let mut candidates: Vec<Cand> = Vec::new();

        let directs = detection::detect_direct(text);
        let direct_empty = directs.is_empty();
        for m in directs {
            candidates.push(Cand {
                r: m.reference,
                conf: m.confidence,
                method: DetectionMethod::Direct,
                verse_end: m.verse_end,
                whole_chapter: m.whole_chapter,
            });
        }
        for n in detection::detect_bare_verses(text) {
            if let Some(r) = context.resolve_bare_verse(n) {
                candidates.push(Cand::single(r, 0.88, DetectionMethod::Direct));
            }
        }
        if let Some((r, score)) = sem.0.top_k(text, 1).into_iter().next() {
            if score >= SEMANTIC_FLOOR {
                candidates.push(Cand::single(r, score.min(0.95), DetectionMethod::Semantic));
            }
        }
        if direct_empty {
            for r in detection::detect_ambiguous(text) {
                candidates.push(Cand::single(r, 0.70, DetectionMethod::Ambiguous));
            }
        }
        if candidates.is_empty() {
            return;
        }

        // Dedup by reference, keeping the strongest evidence per verse — see
        // pipeline::better for why this is NOT simply the highest confidence.
        let mut best: std::collections::HashMap<String, Cand> = std::collections::HashMap::new();
        for c in candidates {
            let key = Fire::key_for(&c.r);
            match best.get(&key) {
                Some(existing) if pipeline::better(existing, &c) => {}
                _ => {
                    best.insert(key, c);
                }
            }
        }

        for (key, c) in best {
            let status = match router.decide(&key, c.conf, c.method, now_ms) {
                RouteDecision::AutoFire => FireStatus::Auto,
                RouteDecision::Suggest => FireStatus::Suggested,
                RouteDecision::Drop => continue,
            };
            let end = passage_end(&conn, &c);
            let mut fire = resolve_fire(&conn, c.r, c.conf, c.method, status, None);

            // Parsed, but the verse doesn't exist (garbled speech readily yields
            // "Psalms 23:99"). Demote to a suggestion rather than broadcasting a
            // verse with no text, which would blank the projector. See
            // `Fire::may_broadcast`.
            if fire.status.goes_to_screen() && fire.verse_id.is_none() {
                fire.status = FireStatus::Suggested;
            }

            if fire.may_broadcast() {
                // Stage the passage so "next" walks a range / whole chapter.
                context.note_passage(&fire.reference, end);
                persist_fire(
                    &conn,
                    handle.state::<Session>(),
                    fire.verse_id,
                    fire.method.db_method(),
                    fire.confidence,
                    fire.status.as_str(),
                    text,
                );
                broadcasts.push(fire.output());
            }
            events.push(fire.event());
        }
    } // locks released here

    for content in broadcasts {
        channels::broadcast_content(handle, content);
    }
    for ev in events {
        let _ = handle.emit("detection://match", ev);
    }
}

/// Spoken "next" / "back": step to the adjacent verse in the staged passage.
///
/// Operator intent, so it bypasses the gate — see `fire_manual`, which owns the
/// whole sequence. This and `handle_passage_nav` were previously two ~70-line
/// near-identical functions; all that actually differs between them is how the
/// target verse is chosen, which is the four lines below.
fn handle_nav(handle: &tauri::AppHandle, dir: detection::NavCommand) {
    let target = {
        let ctx = handle.state::<Context>();
        let Ok(context) = ctx.0.lock() else { return };
        match dir {
            detection::NavCommand::Next => context.next_verse(),
            detection::NavCommand::Previous => context.prev_verse(),
        }
    };
    // None = we stepped off the end of the chapter. Nothing to do.
    let Some(r) = target else { return };
    // Advance keeps the staged passage span, so a range/chapter walk stays bounded.
    fire_manual(handle, r, 1.0, PassageUpdate::Advance, None);
}

/// Spoken in-passage jump ("chapter 5 verse 1", "verse 4"): resolve the BOOK from
/// the current context and fire book chapter:verse, keeping the operator inside
/// the same passage. Chapter-only defaults to verse 1; verse-only keeps the
/// current chapter. Returns true if it fired.
fn handle_passage_nav(handle: &tauri::AppHandle, text: &str) -> bool {
    let Some(nav) = detection::detect_passage_nav(text) else {
        return false;
    };
    let target = {
        let ctx = handle.state::<Context>();
        let Ok(context) = ctx.0.lock() else {
            return false;
        };
        // No current passage → there is no book to resolve the jump against.
        let Some(cur) = context.current() else {
            return false;
        };
        VerseRef {
            book: cur.book.clone(),
            chapter: nav.chapter.unwrap_or(cur.chapter),
            verse: nav.verse.unwrap_or(1),
        }
    };
    fire_manual(handle, target, 1.0, PassageUpdate::Jump, None)
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
/// `status` is what ACTUALLY happened — `"auto"` (the AI fired it unprompted),
/// `"suggested"` (offered to the operator), or `"dismissed"`. It used to be
/// hardcoded to `"auto"` at the insert, so an operator's manual override was
/// recorded in `detections` as if the AI had decided it.
///
/// That is not a cosmetic bug: the self-calibrating threshold loop
/// (`router::record_feedback`, docs/DECISIONS.md) learns from precisely this
/// confirm/reject signal. Logging every human decision as a machine decision
/// means the router is being trained on a record that cannot tell the two apart.
#[allow(clippy::too_many_arguments)]
fn persist_fire(
    conn: &Connection,
    session: tauri::State<'_, Session>,
    verse_id: Option<i64>,
    method: &str,
    confidence: f32,
    status: &str,
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
    let _ = db::insert_detection(conn, tid, verse_id, method, confidence, status, Some(ts));
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
    // Called once from App.svelte's onMount. It is the console's boot heartbeat:
    // this line appearing in the log is the proof that the webview loaded, ran its
    // JavaScript, and reached the Tauri bridge. This machine cannot screenshot the
    // GUI (see CLAUDE.md), so this is how a rendering/CSP regression is caught —
    // a blank webview prints nothing here.
    println!("console: webview up ({name})");
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

/// Scripture search for the Planner — resolve a query to verses to add as cues.
/// First tries to parse explicit references ("john 3:16", "ps 23", "rom 8 1")
/// via the same detector the live pipeline uses; if none parse, falls back to a
/// full-text corpus search ("shepherd"). Offline, corpus-only.
#[tauri::command]
fn search_scripture(
    db: tauri::State<'_, Db>,
    sem: tauri::State<'_, Semantic>,
    query: String,
) -> Result<Vec<db::VerseRow>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Score candidates and rank: exact reference > exact phrase > semantic
    // paraphrase > loose text. Semantic is what turns a paraphrase ("there is
    // therefore no condemnation in christ") into the real verse (Romans 8:1)
    // plus suggestions — the same engine that drives live detection.
    let mut scored: Vec<(f32, db::VerseRow)> = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();

    // 1) Explicit references ("john 3:16", "ps 23").
    for m in detection::detect_direct(q) {
        let r = &m.reference;
        if let Ok(Some(v)) = db::lookup_verse(&conn, &r.book, r.chapter, r.verse) {
            if seen.insert(v.id) {
                scored.push((1.0, v));
            }
        }
    }
    // 2) Exact phrase (the whole query appears verbatim).
    if q.split_whitespace().count() >= 2 {
        if let Ok(hits) = db::search_verses_text(&conn, q, 12) {
            for v in hits {
                if seen.insert(v.id) {
                    scored.push((0.95, v));
                }
            }
        }
    }
    // 3) Semantic paraphrase — top matches by meaning, highest first.
    for (r, score) in sem.0.top_k(q, 12) {
        if score < 0.08 {
            continue;
        }
        if let Ok(Some(v)) = db::lookup_verse(&conn, &r.book, r.chapter, r.verse) {
            if seen.insert(v.id) {
                scored.push((0.5 + score * 0.4, v)); // 0.5..0.9 band
            }
        }
    }
    // 4) Full-text word/phrase recall (FTS5, bm25-ranked). Catches loose,
    //    non-contiguous word queries ("lord shepherd") a substring LIKE misses,
    //    and ranks the best-matching verse first.
    for (i, v) in db::search_verses_fts(&conn, q, 15)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
    {
        if seen.insert(v.id) {
            scored.push((0.45 - (i as f32) * 0.008, v)); // 0.45..~0.33 band
        }
    }
    // 4b) Last-ditch substring scan if FTS returned nothing (index still building).
    if scored.is_empty() {
        if let Ok(hits) = db::search_verses_text(&conn, q, 15) {
            for v in hits {
                if seen.insert(v.id) {
                    scored.push((0.3, v));
                }
            }
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(scored.into_iter().take(25).map(|(_, v)| v).collect())
}

/// Planner: all service plans (newest first) with cue counts.
#[tauri::command]
fn list_plans(db: tauri::State<'_, Db>) -> Result<Vec<db::PlanSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_plans(&conn).map_err(|e| e.to_string())
}

/// Planner: create a plan.
#[tauri::command]
fn create_plan(db: tauri::State<'_, Db>, title: String, date: String) -> Result<i64, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("plan needs a title".into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::create_plan(&conn, title, &date).map_err(|e| e.to_string())
}

/// Planner: delete a plan and its cues.
#[tauri::command]
fn delete_plan(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::delete_plan(&conn, id).map_err(|e| e.to_string())
}

/// Planner: duplicate a plan (with all its cues). Returns the new plan id.
#[tauri::command]
fn duplicate_plan(
    db: tauri::State<'_, Db>,
    id: i64,
    title: String,
    date: String,
) -> Result<i64, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("the copy needs a title".into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::duplicate_plan(&conn, id, title, &date).map_err(|e| e.to_string())
}

/// Planner: ordered cues of a plan.
#[tauri::command]
fn plan_items(db: tauri::State<'_, Db>, plan_id: i64) -> Result<Vec<db::PlanItem>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::plan_items(&conn, plan_id).map_err(|e| e.to_string())
}

/// Planner: append a cue of any type to a plan.
#[tauri::command]
fn add_plan_item(
    db: tauri::State<'_, Db>,
    plan_id: i64,
    cue_type: String,
    label: String,
    payload_json: String,
    template_id: Option<i64>,
) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::add_plan_item(
        &conn,
        plan_id,
        &cue_type,
        &label,
        &payload_json,
        template_id,
    )
    .map_err(|e| e.to_string())
}

/// Planner: remove a cue.
#[tauri::command]
fn remove_plan_item(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::remove_plan_item(&conn, id).map_err(|e| e.to_string())
}

/// Planner: reorder a cue up (-1) or down (+1).
#[tauri::command]
fn move_plan_item(db: tauri::State<'_, Db>, id: i64, direction: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::move_plan_item(&conn, id, direction).map_err(|e| e.to_string())
}

/// Planner: set/clear a cue's operator stage note (confidence-monitor only).
#[tauri::command]
fn set_plan_note(db: tauri::State<'_, Db>, id: i64, note: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::set_plan_note(&conn, id, &note).map_err(|e| e.to_string())
}

/// Planner: apply a drag-reorder — the new ordered list of cue ids.
#[tauri::command]
fn reorder_plan(db: tauri::State<'_, Db>, plan_id: i64, ids: Vec<i64>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::reorder_plan_items(&conn, plan_id, &ids).map_err(|e| e.to_string())
}

/// Lyrics: all songs (with section counts).
#[tauri::command]
fn list_songs(db: tauri::State<'_, Db>) -> Result<Vec<db::SongSummary>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_songs(&conn).map_err(|e| e.to_string())
}

/// Lyrics: search songs by title or author (Planner add + Library browse).
#[tauri::command]
fn search_songs(db: tauri::State<'_, Db>, query: String) -> Result<Vec<db::SongSummary>, String> {
    let q = query.trim();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    if q.is_empty() {
        db::list_songs(&conn).map_err(|e| e.to_string())
    } else {
        db::search_songs(&conn, q).map_err(|e| e.to_string())
    }
}

/// Lyrics: a full song with ordered sections.
#[tauri::command]
fn get_song(db: tauri::State<'_, Db>, id: i64) -> Result<Option<db::Song>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::get_song(&conn, id).map_err(|e| e.to_string())
}

/// Lyrics: import a song from pasted text. The pure `songs` parser splits the
/// lyrics into sections; nothing leaves the device.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn import_song(
    db: tauri::State<'_, Db>,
    title: String,
    author: String,
    ccli: String,
    song_key: String,
    bpm: Option<i64>,
    lyrics: String,
    date: String,
) -> Result<i64, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("song needs a title".into());
    }
    let sections = songs::parse_song(&lyrics);
    if sections.is_empty() {
        return Err("no lyrics found to import".into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    // Dedupe by title: replace an existing song rather than duplicate it.
    if let Some(id) = db::song_id_by_title(&conn, title).map_err(|e| e.to_string())? {
        db::update_song(
            &conn,
            id,
            title,
            author.trim(),
            ccli.trim(),
            song_key.trim(),
            bpm,
            &sections,
        )
        .map_err(|e| e.to_string())?;
        Ok(id)
    } else {
        db::import_song(
            &conn,
            title,
            author.trim(),
            ccli.trim(),
            song_key.trim(),
            bpm,
            &date,
            &sections,
        )
        .map_err(|e| e.to_string())
    }
}

/// Lyrics: save edits to a song — metadata + the full ordered section list.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn save_song(
    db: tauri::State<'_, Db>,
    id: i64,
    title: String,
    author: String,
    ccli: String,
    song_key: String,
    bpm: Option<i64>,
    sections: Vec<songs::ParsedSection>,
) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("song needs a title".into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::update_song(
        &conn,
        id,
        title,
        author.trim(),
        ccli.trim(),
        song_key.trim(),
        bpm,
        &sections,
    )
    .map_err(|e| e.to_string())?;
    // Propagate the edit to every plan that cues this song (real-time everywhere).
    db::sync_song_in_plans(&conn, id, title, &sections).map_err(|e| e.to_string())?;
    Ok(())
}

/// Lyrics: delete a song and its sections.
#[tauri::command]
fn delete_song(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::delete_song(&conn, id).map_err(|e| e.to_string())
}

/// Arrangements: named play-orders of a song's sections.
#[tauri::command]
fn list_arrangements(
    db: tauri::State<'_, Db>,
    song_id: i64,
) -> Result<Vec<db::Arrangement>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_arrangements(&conn, song_id).map_err(|e| e.to_string())
}

/// Arrangements: create (id None) or update one. Returns its id.
#[tauri::command]
fn save_arrangement(
    db: tauri::State<'_, Db>,
    song_id: i64,
    id: Option<i64>,
    name: String,
    sequence: Vec<i64>,
) -> Result<i64, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("arrangement needs a name".into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::save_arrangement(&conn, song_id, id, name, &sequence).map_err(|e| e.to_string())
}

/// Arrangements: delete one.
#[tauri::command]
fn delete_arrangement(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::delete_arrangement(&conn, id).map_err(|e| e.to_string())
}

/// Scripture (Library): verses the operator saved.
#[tauri::command]
fn list_saved_scripture(db: tauri::State<'_, Db>) -> Result<Vec<db::SavedScripture>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_saved_scripture(&conn).map_err(|e| e.to_string())
}

/// Scripture (Library): resolve a reference and save it to the library.
#[tauri::command]
fn save_scripture(
    db: tauri::State<'_, Db>,
    book: String,
    chapter: i64,
    verse: i64,
    date: String,
) -> Result<db::SavedScripture, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let v = db::lookup_verse(&conn, &book, chapter, verse)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("{book} {chapter}:{verse} not found"))?;
    let id = db::save_scripture(&conn, &v, &date).map_err(|e| e.to_string())?;
    Ok(db::SavedScripture {
        id,
        reference: v.reference,
        book: v.book,
        chapter: v.chapter,
        verse: v.verse,
        text: v.text,
        translation: v.translation,
    })
}

/// Scripture (Library): remove a saved verse.
#[tauri::command]
fn delete_saved_scripture(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::delete_saved_scripture(&conn, id).map_err(|e| e.to_string())
}

/// Announcements (Library): all saved notices, newest first.
#[tauri::command]
fn list_announcements(db: tauri::State<'_, Db>) -> Result<Vec<db::Announcement>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_announcements(&conn).map_err(|e| e.to_string())
}

/// Announcements: create (id None) or update one. Returns its id.
#[tauri::command]
fn save_announcement(
    db: tauri::State<'_, Db>,
    id: Option<i64>,
    title: String,
    body: String,
    date: String,
) -> Result<i64, String> {
    let title = title.trim();
    let body = body.trim();
    if title.is_empty() && body.is_empty() {
        return Err("an announcement needs a title or body".into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let saved = db::save_announcement(&conn, id, title, body, &date).map_err(|e| e.to_string())?;
    // Editing an existing announcement propagates to any plan that cues it.
    if id.is_some() {
        let _ = db::sync_announcement_in_plans(&conn, saved, title, body);
    }
    Ok(saved)
}

/// Announcements: delete one.
#[tauri::command]
fn delete_announcement(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::delete_announcement(&conn, id).map_err(|e| e.to_string())
}

/// Media (Library): all imported media/document assets.
#[tauri::command]
fn list_media(db: tauri::State<'_, Db>) -> Result<Vec<db::MediaAsset>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_media(&conn).map_err(|e| e.to_string())
}

/// Media (Library): import a file (image / video / document). The webview hands
/// us the picked file's bytes (base64); we write it beside the DB and store a
/// pointer — offline-first, nothing uploads.
#[tauri::command]
fn import_media(
    db: tauri::State<'_, Db>,
    kind: String,
    filename: String,
    data: String,
    date: String,
) -> Result<db::MediaAsset, String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("could not read file data: {e}"))?;
    let dir = db::media_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let id = db::insert_media(&conn, &kind, &filename, &date).map_err(|e| e.to_string())?;
    // Prefix with the row id to guarantee a unique on-disk name.
    let safe: String = filename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = dir.join(format!("{id}_{safe}"));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let path_str = path.to_string_lossy().to_string();
    db::set_media_path(&conn, id, &path_str).map_err(|e| e.to_string())?;
    Ok(db::MediaAsset {
        id,
        kind,
        filename,
        path: path_str,
        created_at: date,
    })
}

/// Media (Library): delete an asset (row + file).
#[tauri::command]
fn delete_media(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let path = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::delete_media(&conn, id).map_err(|e| e.to_string())?
    };
    if let Some(p) = path {
        let _ = std::fs::remove_file(p); // best-effort
    }
    Ok(())
}

/// Lyrics: import songs from a ProPresenter file. The webview reads the picked
/// file and hands us its bytes (base64) — a `.proplaylist` yields many songs,
/// a single `.pro` yields one. Each slide becomes a section. Fully offline;
/// nothing leaves the device. Returns the imported song titles.
/// Result of a ProPresenter import — which songs were added new vs replaced
/// (deduped by title).
#[derive(serde::Serialize)]
struct ImportResult {
    added: Vec<String>,
    replaced: Vec<String>,
}

/// A song parsed for the pre-save review step (not yet in the DB).
#[derive(serde::Serialize, serde::Deserialize)]
struct ReviewSong {
    title: String,
    sections: Vec<songs::ParsedSection>,
}

/// Parse a lyric file (ProPresenter / playlist / text) into songs WITHOUT
/// saving — the operator reviews and edits before committing (avoids the
/// import-then-fix-then-replace cycle). Offline.
#[tauri::command]
fn parse_import(filename: String, data: String) -> Result<Vec<ReviewSong>, String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("could not read file data: {e}"))?;
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    let mut out = Vec::new();
    if ["txt", "text", "md", "lyric", "lyrics"].contains(&ext.as_str()) {
        let text = String::from_utf8_lossy(&bytes).to_string();
        let sections = songs::parse_song(&text);
        if !sections.is_empty() {
            let title = filename
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&filename)
                .rsplit_once('.')
                .map(|(a, _)| a)
                .unwrap_or(&filename)
                .trim()
                .to_string();
            out.push(ReviewSong { title, sections });
        }
    } else {
        for s in proimport::import_bytes(&filename, &bytes)? {
            let sections = s
                .slides
                .iter()
                .enumerate()
                .map(|(i, t)| songs::ParsedSection {
                    tag: format!("{}", i + 1),
                    label: format!("Slide {}", i + 1),
                    lyrics: t.clone(),
                })
                .collect();
            out.push(ReviewSong {
                title: s.title,
                sections,
            });
        }
    }
    if out.is_empty() {
        return Err("no lyrics found in this file".into());
    }
    Ok(out)
}

/// One reviewed song ready to save (edited by the operator).
#[derive(serde::Deserialize)]
struct SaveSong {
    title: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    ccli: String,
    #[serde(default)]
    song_key: String,
    #[serde(default)]
    bpm: Option<i64>,
    sections: Vec<songs::ParsedSection>,
}

/// Commit reviewed songs to the library (dedupe by title; propagate edits to
/// any plans that already cue a replaced song).
#[tauri::command]
fn save_reviewed_songs(
    db: tauri::State<'_, Db>,
    songs: Vec<SaveSong>,
    date: String,
) -> Result<ImportResult, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut added = Vec::new();
    let mut replaced = Vec::new();
    for s in songs {
        let title = s.title.trim();
        if title.is_empty() || s.sections.is_empty() {
            continue;
        }
        if let Some(id) = db::song_id_by_title(&conn, title).map_err(|e| e.to_string())? {
            db::update_song(
                &conn,
                id,
                title,
                s.author.trim(),
                s.ccli.trim(),
                s.song_key.trim(),
                s.bpm,
                &s.sections,
            )
            .map_err(|e| e.to_string())?;
            db::sync_song_in_plans(&conn, id, title, &s.sections).map_err(|e| e.to_string())?;
            replaced.push(title.to_string());
        } else {
            db::import_song(
                &conn,
                title,
                s.author.trim(),
                s.ccli.trim(),
                s.song_key.trim(),
                s.bpm,
                &date,
                &s.sections,
            )
            .map_err(|e| e.to_string())?;
            added.push(title.to_string());
        }
    }
    Ok(ImportResult { added, replaced })
}

#[tauri::command]
fn import_pro(
    db: tauri::State<'_, Db>,
    filename: String,
    data: String,
    date: String,
) -> Result<ImportResult, String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("could not read file data: {e}"))?;
    let songs = proimport::import_bytes(&filename, &bytes)?;
    if songs.is_empty() {
        return Err("no lyrics found in this file".into());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut added = Vec::new();
    let mut replaced = Vec::new();
    for song in songs {
        let sections: Vec<songs::ParsedSection> = song
            .slides
            .iter()
            .enumerate()
            .map(|(i, t)| songs::ParsedSection {
                // Sequential slide numbers — ProPresenter group names (Verse/
                // Chorus) aren't reliably in the file, so keep tags clean and
                // let the operator relabel in the slide-flow editor.
                tag: format!("{}", i + 1),
                label: format!("Slide {}", i + 1),
                lyrics: t.clone(),
            })
            .collect();
        if sections.is_empty() {
            continue;
        }
        // Dedupe by title: replace an existing song's slides (keeping any
        // metadata the operator set), otherwise add it fresh.
        if let Some(id) = db::song_id_by_title(&conn, &song.title).map_err(|e| e.to_string())? {
            db::replace_song_sections(&conn, id, &sections).map_err(|e| e.to_string())?;
            replaced.push(song.title);
        } else {
            db::import_song(&conn, &song.title, "", "", "", None, &date, &sections)
                .map_err(|e| e.to_string())?;
            added.push(song.title);
        }
    }
    Ok(ImportResult { added, replaced })
}

/// Normalize an operator stage note: trim, and treat blank as absent.
fn clean_note(note: Option<String>) -> Option<String> {
    note.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Start a pre-service countdown on every output. Broadcasts the target epoch
/// (now + `minutes`), then each output ticks the MM:SS locally — no per-second
/// network traffic. `label` shows above the timer; `done_msg` replaces it at 0.
#[tauri::command]
fn start_countdown(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    minutes: f64,
    label: String,
    done_msg: String,
) -> Result<(), String> {
    let mins = if minutes.is_finite() && minutes > 0.0 {
        minutes
    } else {
        5.0
    };
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let target = now_ms + (mins * 60_000.0) as i64;
    let (tid, tjson) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        content_tpl(&conn, "countdown")
    };
    channels::broadcast_content(
        &app,
        OutputContent {
            reference: label.trim().to_string(),
            countdown_to: Some(target),
            countdown_done: clean_note(Some(done_msg)),
            template_id: tid,
            template_json: tjson,
            ..Default::default()
        },
    );
    persist_cue(&app, "countdown", None);
    Ok(())
}

/// Fire arbitrary content straight to the output screens — the generic take for
/// non-scripture cues (a song section, an announcement). Same broadcast path as
/// a scripture manual fire; operator override, always. `label` is the on-screen
/// citation, `text` the body. `stage_note` is the operator's confidence-monitor
/// note for this cue, if any.
#[tauri::command]
fn fire_content(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    label: String,
    text: String,
    kind: String,
    stage_note: Option<String>,
) -> Result<(), String> {
    let label = label.trim().to_string();
    let (tid, tjson) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        content_tpl(&conn, &kind)
    };
    channels::broadcast_content(
        &app,
        OutputContent {
            reference: label.clone(),
            text: Some(text),
            translation: None,
            template_id: tid,
            template_json: tjson,
            stage_note: clean_note(stage_note),
            ..Default::default()
        },
    );
    persist_cue(&app, "manual_override", Some(&label));
    Ok(())
}

/// Fire a media asset (image/video) to the output screens as a full-screen
/// background. The file is served by the embedded HTTP server at
/// `http://<lan-ip>:8032/media/<id>` so native windows AND kiosk/OBS clients
/// load the same URL. Documents (pdf/pptx) aren't renderable as output yet.
#[tauri::command]
fn fire_media(app: tauri::AppHandle, db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let (kind, filename, tid, tjson): (String, String, Option<i64>, Option<String>) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let (k, f) = conn
            .query_row(
                "SELECT kind, filename FROM media_assets WHERE id = ?1",
                [id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(|_| "media not found".to_string())?;
        let (tid, tjson) = content_tpl(&conn, "media");
        (k, f, tid, tjson)
    };
    let media_kind = match kind.as_str() {
        "image" => "image",
        "video" => "video",
        _ => return Err("documents can't be shown as an output background yet".into()),
    };
    let ip = local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
    channels::broadcast_content(
        &app,
        OutputContent {
            media_url: Some(format!("http://{ip}:8032/media/{id}")),
            media_kind: Some(media_kind.to_string()),
            template_id: tid,
            template_json: tjson,
            ..Default::default()
        },
    );
    persist_cue(&app, "media", Some(&filename));
    Ok(())
}

/// Resolve a content type's override template to (id, json) for the broadcast.
/// A missing/unmapped type yields (None, None) → the channel template is used.
fn content_tpl(conn: &rusqlite::Connection, kind: &str) -> (Option<i64>, Option<String>) {
    match db::content_template(conn, kind) {
        Ok(Some((id, j))) => (Some(id), Some(j)),
        _ => (None, None),
    }
}

/// The default template ids mapped to each content type.
#[derive(serde::Serialize)]
struct ContentTemplates {
    scripture: Option<i64>,
    song: Option<i64>,
    media: Option<i64>,
    announce: Option<i64>,
}

/// Read the content-type → template mapping (Templates screen defaults).
#[tauri::command]
fn get_content_templates(db: tauri::State<'_, Db>) -> Result<ContentTemplates, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let id = |k: &str| db::content_template_id(&conn, k).ok().flatten();
    Ok(ContentTemplates {
        scripture: id("scripture"),
        song: id("song"),
        media: id("media"),
        announce: id("announce"),
    })
}

/// Map a content type to a template (None clears it → channel default).
#[tauri::command]
fn set_content_template(
    db: tauri::State<'_, Db>,
    kind: String,
    template_id: Option<i64>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::set_content_template(&conn, &kind, template_id).map_err(|e| e.to_string())
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

/// This machine's LAN IPv4, so output URLs point at a real address other devices
/// can reach (not `localhost`). Uses the connect-a-UDP-socket trick — no packet
/// is actually sent; the OS just picks the outbound interface. None if offline.
#[tauri::command]
fn local_ip() -> Option<String> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    let ip = sock.local_addr().ok()?.ip();
    if ip.is_loopback() {
        None
    } else {
        Some(ip.to_string())
    }
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
    let quality_emitter = app.clone();
    let err_emitter = app.clone();
    // Throttle the level-meter event: chunks arrive ~5/sec but the UI only needs
    // a couple updates/sec. Flooding the webview with events is a real freeze
    // risk. STT still gets EVERY chunk.
    let chunk_n = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    // Quality snapshots arrive per processed block (~many/sec) — throttle to a
    // couple/sec on their own additive channel. Existing UI ignores it.
    let quality_n = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
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
        move |quality| {
            let n = quality_n.fetch_add(1, Ordering::Relaxed);
            if n.is_multiple_of(10) {
                let _ = quality_emitter.emit("audio://quality", quality);
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
///
/// When no model loads, Relay must degrade to a fully working MANUAL tool, never
/// to a dead one — so this reports the failure loudly enough for the UI to put a
/// banner up. It used to fail silently, which on Windows (where the model lookup
/// was broken outright) meant the operator had no idea the AI was never running.
#[tauri::command]
fn stt_status(stt: tauri::State<'_, Stt>) -> Result<StatusStt, String> {
    let slot = stt.0.lock().map_err(|e| e.to_string())?;
    Ok(match slot.as_ref() {
        Some(e) => StatusStt {
            loaded: true,
            model: Some(e.model_path().display().to_string()),
            language: e.language(),
            install_dir: None,
        },
        None => StatusStt {
            loaded: false,
            model: None,
            language: None,
            install_dir: Some(stt::model_install_dir().display().to_string()),
        },
    })
}

/// Bible translations available in the corpus (Settings → Bible translations).
#[tauri::command]
fn list_translations(db: tauri::State<'_, Db>) -> Result<Vec<db::Translation>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_translations(&conn).map_err(|e| e.to_string())
}

/// The active translation id used for verse lookups + output. Falls back to the
/// first (KJV) when unset.
#[tauri::command]
fn get_active_translation(db: tauri::State<'_, Db>) -> Result<Option<i64>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let set = db::get_setting(&conn, "active_translation")
        .map_err(|e| e.to_string())?
        .and_then(|v| v.parse::<i64>().ok());
    match set {
        Some(id) => Ok(Some(id)),
        None => Ok(db::list_translations(&conn)
            .map_err(|e| e.to_string())?
            .first()
            .map(|t| t.id)),
    }
}

/// Choose which translation to read from. Every verse lookup (detection, nav,
/// manual, output) then prefers it, falling back to any that has the verse.
#[tauri::command]
fn set_active_translation(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::set_setting(&conn, "active_translation", &id.to_string()).map_err(|e| e.to_string())
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
    /// Where the operator should put a model file when none was found. Resolved
    /// per-OS, so the message shows the real path on *their* machine.
    install_dir: Option<String>,
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
    reference: String,
) -> Result<Thresholds, String> {
    // The confidence of the suggestion the operator just accepted — this is the
    // evidence the self-calibrating gate learns from, so it has to outlive the
    // `if let` that parses the reference.
    let mut confirmed_conf: Option<f32> = None;
    if let Some(m) = detection::detect_direct(&reference).into_iter().next() {
        confirmed_conf = Some(m.confidence);
        // Stage the passage span, then fire through the one shared manual path —
        // the operator accepting a suggestion IS a human decision, so it records
        // as "manual" and carries the scripture template like every other fire.
        let end = {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            if m.whole_chapter {
                db::chapter_last_verse(&conn, &m.reference.book, m.reference.chapter)
                    .ok()
                    .flatten()
            } else {
                m.verse_end
            }
        };
        fire_manual(
            &app,
            m.reference,
            m.confidence,
            PassageUpdate::Note(end),
            None,
        );
    }
    let t = {
        let mut router = routing.0.lock().map_err(|e| e.to_string())?;
        router.record_feedback(true, confirmed_conf);
        router.thresholds()
    };
    // Persist the nudge onto the active profile (calibration survives restart).
    if let Ok(conn) = db.0.lock() {
        persist_active_thresholds(&conn, t);
    }
    Ok(t)
}

/// Operator rejected an auto-fired detection (undo). Tightens the gate and
/// persists the nudge onto the active profile.
#[tauri::command]
fn dismiss_detection(
    routing: tauri::State<'_, Routing>,
    db: tauri::State<'_, Db>,
) -> Result<Thresholds, String> {
    let t = {
        let mut router = routing.0.lock().map_err(|e| e.to_string())?;
        // No argument: the router remembers what it last auto-fired, so the
        // correction is proportional to what was actually wrong.
        router.record_feedback(false, None);
        router.thresholds()
    };
    if let Ok(conn) = db.0.lock() {
        persist_active_thresholds(&conn, t);
    }
    Ok(t)
}

/// Crash-reporting status for the Settings toggle.
#[derive(Serialize)]
struct CrashReportingStatus {
    enabled: bool,
    dsn: String,
}

#[tauri::command]
fn get_crash_reporting(db: tauri::State<'_, Db>) -> Result<CrashReportingStatus, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let dsn = db::get_setting(&conn, telemetry::DSN_KEY)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    Ok(CrashReportingStatus {
        enabled: telemetry::is_enabled(),
        dsn,
    })
}

/// Turn crash reporting on/off. OFF is the default and requires no consent;
/// turning it ON is an explicit, visible operator action (CLAUDE.md: nothing
/// leaves the device without one).
#[tauri::command]
fn set_crash_reporting(
    db: tauri::State<'_, Db>,
    enabled: bool,
    dsn: String,
) -> Result<CrashReportingStatus, String> {
    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::set_setting(
            &conn,
            telemetry::ENABLED_KEY,
            if enabled { "1" } else { "0" },
        )
        .map_err(|e| e.to_string())?;
        db::set_setting(&conn, telemetry::DSN_KEY, dsn.trim()).map_err(|e| e.to_string())?;
    }
    if enabled {
        telemetry::enable(dsn.trim(), env!("CARGO_PKG_VERSION"));
    } else {
        telemetry::disable();
    }
    Ok(CrashReportingStatus {
        enabled: telemetry::is_enabled(),
        dsn: dsn.trim().to_string(),
    })
}

/// Current gate thresholds — for the Settings sliders.
#[tauri::command]
fn get_thresholds(routing: tauri::State<'_, Routing>) -> Result<Thresholds, String> {
    let router = routing.0.lock().map_err(|e| e.to_string())?;
    Ok(router.thresholds())
}

// ===== Related scripture & series tracker (Phase A: A3/A4/A6) ===============

/// One related-scripture suggestion, resolved to verse text.
#[derive(Serialize)]
struct RelatedVerse {
    reference: String,
    book: String,
    chapter: i64,
    verse: i64,
    verse_end: Option<i64>,
    text: Option<String>,
    translation: Option<String>,
}

/// A themed set of related references for a transcript window.
#[derive(Serialize)]
struct RelatedPayload {
    theme: String,
    refs: Vec<RelatedVerse>,
}

/// A3/A4: topical cross-references for a transcript window, each resolved to
/// verse text. `exclude` drops the currently-shown verse. Pull-based, additive —
/// the console can poll this to offer "related scripture" chips. Returns None
/// when no theme is clearly indicated.
#[tauri::command]
fn related_scripture(
    db: tauri::State<'_, Db>,
    text: String,
    exclude: Option<String>,
) -> Result<Option<RelatedPayload>, String> {
    let ex = exclude
        .and_then(|s| detection::detect_direct(&s).into_iter().next())
        .map(|m| m.reference);
    let Some(sug) = detection::suggest_related(&text, ex.as_ref(), 4) else {
        return Ok(None);
    };
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let refs = sug
        .refs
        .iter()
        .map(|m| {
            let r = &m.reference;
            let looked = db::lookup_verse(&conn, &r.book, r.chapter, r.verse)
                .ok()
                .flatten();
            let reference = match m.verse_end {
                Some(e) => format!("{} {}:{}-{}", r.book, r.chapter, r.verse, e),
                None => format!("{} {}:{}", r.book, r.chapter, r.verse),
            };
            RelatedVerse {
                reference,
                book: r.book.clone(),
                chapter: r.chapter,
                verse: r.verse,
                verse_end: m.verse_end,
                text: looked.as_ref().map(|v| v.text.clone()),
                translation: looked.as_ref().map(|v| v.translation.clone()),
            }
        })
        .collect();
    Ok(Some(RelatedPayload {
        theme: sug.theme,
        refs,
    }))
}

/// A6: how many times a verse has already fired in the current service — lets the
/// console flag repeats ("shown earlier today"). 0 when not recording or unseen.
#[tauri::command]
fn verse_repeat_count(
    db: tauri::State<'_, Db>,
    session: tauri::State<'_, Session>,
    reference: String,
) -> Result<i64, String> {
    let Some(m) = detection::detect_direct(&reference).into_iter().next() else {
        return Ok(0);
    };
    let service_id = session
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(|s| s.id);
    let Some(sid) = service_id else {
        return Ok(0);
    };
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let r = &m.reference;
    let Some(v) = db::lookup_verse(&conn, &r.book, r.chapter, r.verse)
        .ok()
        .flatten()
    else {
        return Ok(0);
    };
    db::count_verse_in_service(&conn, sid, v.id).map_err(|e| e.to_string())
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

// ===== Voice profiles (Phase B — accent & speaker calibration) ==============

/// Apply a profile's STT settings: language hint (code-switch when None) + the
/// scripture decoder-bias prompt (book names + the profile's extra vocabulary).
fn apply_profile_to_stt(engine: &SttEngine, p: &db::VoiceProfile) {
    engine.set_language(p.language.clone());
    engine.set_prompt(Some(stt::scripture_bias_prompt(&p.bias_terms)));
}

/// Apply a full profile live: STT language + bias prompt, and the profile's
/// calibrated thresholds to the router.
fn apply_profile(stt: &Stt, routing: &Routing, p: &db::VoiceProfile) -> Result<(), String> {
    if let Some(e) = stt.0.lock().map_err(|e| e.to_string())?.as_ref() {
        apply_profile_to_stt(e, p);
    }
    let mut router = routing.0.lock().map_err(|e| e.to_string())?;
    // Two DIFFERENT things, and conflating them is what made calibration a
    // one-way ratchet:
    //   • the profile's stored thresholds are what the router has LEARNED so far,
    //   • the sensitivity dial is the baseline that learning decays back toward.
    // Restore the learned gate, then re-anchor the baseline to the dial.
    router.set_thresholds(Thresholds {
        auto_fire: p.auto_fire as f32,
        suggest: p.suggest as f32,
    });
    router.set_baseline(Thresholds::from_sensitivity(
        p.sensitivity.clamp(0, 100) as u8
    ));
    Ok(())
}

/// Build the STT engine and wire its transcript callback into the pipeline.
///
/// Extracted from `setup` so it can be run AGAIN, at runtime, the moment the
/// operator finishes downloading a model. Without this, a 148 MB download would
/// end with "now quit and reopen Relay" — a miserable last step for the very
/// first thing a new user does.
///
/// Returns None (audio-only) when no model is installed. That is a supported
/// state, not a failure: manual fire and plan playback still work.
fn build_stt(handle: &tauri::AppHandle) -> Option<SttEngine> {
    let path = stt::default_model_path()?;
    let handle = handle.clone();
    match SttEngine::try_load(path, move |update| {
        let _ = handle.emit("stt://transcript", &update);
        if update.is_final {
            println!("stt[{}]: {}", update.language, update.text);
            persist_transcript(&handle, &update.text, &update.language);
            // Spoken "next"/"back" navigates from the current verse.
            if let Some(cmd) = detection::detect_command(&update.text) {
                handle_nav(&handle, cmd);
                return;
            }
            // Spoken "clear the screen" / "blackout".
            if detection::detect_clear(&update.text) {
                channels::clear(&handle);
                forget_debounce(&handle);
                persist_cue(&handle, "clear_screens", None);
                return;
            }
            // Spoken in-passage jump — "chapter 5 verse 1", "verse 4".
            if handle_passage_nav(&handle, &update.text) {
                return;
            }
        }
        // Detect references, then route each through the confidence gate.
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
    }
}

/// Bring speech recognition up after a model has just been installed, without a
/// restart. Re-applies the active voice profile so language + decoder bias are
/// live from the first word.
#[tauri::command]
fn load_stt_model(app: tauri::AppHandle) -> Result<bool, String> {
    let engine = build_stt(&app);
    let loaded = engine.is_some();
    {
        let stt_state = app.state::<Stt>();
        let mut slot = stt_state.0.lock().map_err(|e| e.to_string())?;
        *slot = engine;
    }
    if loaded {
        let profile = {
            let db = app.state::<Db>();
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            db::active_voice_profile(&conn).ok().flatten()
        };
        if let Some(p) = profile {
            let stt_state = app.state::<Stt>();
            let slot = stt_state.0.lock().map_err(|e| e.to_string())?;
            if let Some(e) = slot.as_ref() {
                apply_profile_to_stt(e, &p);
            }
        }
    }
    Ok(loaded)
}

/// The speech models Relay can install, and whether each is already on this
/// machine.
#[tauri::command]
fn list_models() -> Vec<models::ModelInfo> {
    models::catalog()
}

/// Download a speech model. Resumable, checksummed, atomic — see models.rs.
/// Progress arrives as `model://progress`; completion as `model://done`.
#[tauri::command]
async fn download_model(app: tauri::AppHandle, id: String) -> Result<(), String> {
    models::download(app, id).await
}

/// Cancel an in-flight model download.
#[tauri::command]
fn cancel_model_download(state: tauri::State<'_, models::DownloadState>) {
    state
        .cancel
        .store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Persist the router's freshly-adapted thresholds onto the active profile so
/// per-speaker calibration survives a restart (the self-calibrating loop).
fn persist_active_thresholds(conn: &Connection, t: Thresholds) {
    if let Ok(Some(p)) = db::active_voice_profile(conn) {
        let _ = db::save_profile_thresholds(conn, p.id, t.auto_fire as f64, t.suggest as f64);
    }
}

/// All voice profiles (Settings → Voice profiles).
#[tauri::command]
fn list_voice_profiles(db: tauri::State<'_, Db>) -> Result<Vec<db::VoiceProfile>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_voice_profiles(&conn).map_err(|e| e.to_string())
}

/// The currently active profile.
#[tauri::command]
fn active_voice_profile(db: tauri::State<'_, Db>) -> Result<Option<db::VoiceProfile>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::active_voice_profile(&conn).map_err(|e| e.to_string())
}

/// Create a new profile (default calibration); returns its id.
#[tauri::command]
fn create_voice_profile(
    db: tauri::State<'_, Db>,
    name: String,
    language: Option<String>,
) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::create_voice_profile(&conn, &name, language.as_deref()).map_err(|e| e.to_string())
}

/// Save editable profile fields (name, language, bias terms, sensitivity).
///
/// Thresholds are re-derived from the sensitivity dial ONLY when the operator
/// actually moved that dial. Every other edit — renaming the profile, switching
/// language, adding a bias term — leaves the live thresholds untouched.
///
/// This used to reset them unconditionally, which meant that renaming a profile
/// silently discarded every confirm/reject nudge the self-calibrating router had
/// accumulated (docs/DECISIONS.md) and snapped `auto_fire` back to the baseline
/// mid-preparation. The operator saw the AI "just stop working", with no error.
#[tauri::command]
fn update_voice_profile(
    stt: tauri::State<'_, Stt>,
    routing: tauri::State<'_, Routing>,
    db: tauri::State<'_, Db>,
    mut profile: db::VoiceProfile,
) -> Result<db::VoiceProfile, String> {
    let is_active = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;

        // Did the sensitivity dial actually move? Compare against what's stored.
        let stored = db::list_voice_profiles(&conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|p| p.id == profile.id);
        let sensitivity_changed = stored
            .as_ref()
            .map(|s| s.sensitivity != profile.sensitivity)
            .unwrap_or(true);

        let current = stored
            .as_ref()
            .map(|s| Thresholds {
                auto_fire: s.auto_fire as f32,
                suggest: s.suggest as f32,
            })
            .unwrap_or_default();
        let next = router::thresholds_on_profile_save(
            sensitivity_changed,
            profile.sensitivity.clamp(0, 100) as u8,
            current,
        );
        profile.auto_fire = next.auto_fire as f64;
        profile.suggest = next.suggest as f64;

        db::update_voice_profile(&conn, &profile).map_err(|e| e.to_string())?;
        db::save_profile_thresholds(&conn, profile.id, profile.auto_fire, profile.suggest)
            .map_err(|e| e.to_string())?;
        db::active_voice_profile(&conn).ok().flatten().map(|a| a.id) == Some(profile.id)
    };
    if is_active {
        apply_profile(&stt, &routing, &profile)?;
    }
    Ok(profile)
}

/// Switch the active profile — applies its language + bias prompt + thresholds
/// immediately, before the next transcript window.
#[tauri::command]
fn select_voice_profile(
    stt: tauri::State<'_, Stt>,
    routing: tauri::State<'_, Routing>,
    db: tauri::State<'_, Db>,
    id: i64,
) -> Result<db::VoiceProfile, String> {
    let profile = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::set_active_profile(&conn, id).map_err(|e| e.to_string())?;
        db::active_voice_profile(&conn)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "no active profile after select".to_string())?
    };
    apply_profile(&stt, &routing, &profile)?;
    Ok(profile)
}

/// Delete a profile. If it was active, the next remaining profile becomes active
/// (a Default is re-seeded if it was the last) and is applied live.
#[tauri::command]
fn delete_voice_profile(
    stt: tauri::State<'_, Stt>,
    routing: tauri::State<'_, Routing>,
    db: tauri::State<'_, Db>,
    id: i64,
) -> Result<db::VoiceProfile, String> {
    let profile = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::delete_voice_profile(&conn, id).map_err(|e| e.to_string())?;
        db::active_voice_profile(&conn)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "no active profile after delete".to_string())?
    };
    apply_profile(&stt, &routing, &profile)?;
    Ok(profile)
}

/// Operator manual override: fire a free-text reference now, bypassing the gate.
/// First-class control (CLAUDE.md) — parses the reference, resolves it, and
/// emits a `detection://match` with status "manual".
#[tauri::command]
fn manual_fire(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    reference: String,
    stage_note: Option<String>,
) -> Result<(), String> {
    let m = detection::detect_direct(&reference)
        .into_iter()
        .next()
        .ok_or_else(|| format!("could not parse a reference from \"{reference}\""))?;

    // Stage the passage span so a later "next" walks "Psalm 23" / "John 3:16-18"
    // rather than stopping dead after the anchor verse. Short lock, released
    // before fire_manual takes its own — sequential, never nested.
    let end = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if m.whole_chapter {
            db::chapter_last_verse(&conn, &m.reference.book, m.reference.chapter)
                .ok()
                .flatten()
        } else {
            m.verse_end
        }
    };

    let key = pipeline::Fire::key_for(&m.reference);
    if !fire_manual(
        &app,
        m.reference,
        1.0,
        PassageUpdate::Note(end),
        clean_note(stage_note),
    ) {
        // Parsed fine, but that verse doesn't exist (e.g. "John 3:99"). Say so.
        // This used to broadcast an EMPTY verse instead — blanking the wall
        // mid-service and leaving the operator with no idea why.
        return Err(format!(
            "{key} isn't in the Bible text — check the reference"
        ));
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
    monitor_index: Option<usize>,
) -> Result<String, String> {
    let name = name.unwrap_or_else(|| "Output".into());
    let label = {
        let mut n = outputs.0.lock().map_err(|e| e.to_string())?;
        *n += 1;
        format!("output-{n}")
    };
    channels::open_native_window(&app, &label, template_id, &name, monitor_index)?;
    Ok(label)
}

/// Connected displays for HDMI screen assignment (Channels tab).
#[tauri::command]
fn list_monitors(app: tauri::AppHandle) -> Vec<channels::MonitorInfo> {
    channels::list_monitors(&app)
}

/// Open a channel's native fullscreen output on its assigned display (HDMI). Uses
/// the channel's template and `display_target` monitor index; falls back to the
/// primary display when unassigned or the index is stale. Returns the label.
#[tauri::command]
fn open_channel_output(
    app: tauri::AppHandle,
    outputs: tauri::State<'_, Outputs>,
    db: tauri::State<'_, Db>,
    channel_id: i64,
) -> Result<String, String> {
    let channel = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        db::list_output_channels(&conn)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|c| c.id == channel_id)
            .ok_or_else(|| format!("channel {channel_id} not found"))?
    };
    let template_id = channel.template_id.unwrap_or(1);
    let monitor_index = channel
        .display_target
        .as_deref()
        .and_then(|s| s.parse::<usize>().ok());
    let label = {
        let mut n = outputs.0.lock().map_err(|e| e.to_string())?;
        *n += 1;
        format!("output-{n}")
    };
    channels::open_native_window(&app, &label, template_id, &channel.name, monitor_index)?;
    Ok(label)
}

/// Assign a physical display to a channel (HDMI). `display` is the monitor index
/// as a string, or null to use the primary display.
#[tauri::command]
fn set_channel_display(
    db: tauri::State<'_, Db>,
    id: i64,
    display: Option<String>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::set_channel_display(&conn, id, display.as_deref()).map_err(|e| e.to_string())
}

/// Add an output channel. Returns its id.
#[tauri::command]
fn add_channel(
    db: tauri::State<'_, Db>,
    name: String,
    render_target: Option<String>,
    template_id: Option<i64>,
) -> Result<i64, String> {
    let target = render_target.unwrap_or_else(|| "native_window".into());
    if !matches!(
        target.as_str(),
        "native_window" | "ndi_encode" | "network_client"
    ) {
        return Err(format!("invalid render target: {target}"));
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::add_channel(&conn, name.trim(), &target, template_id.unwrap_or(1))
        .map_err(|e| e.to_string())
}

/// Delete an output channel.
#[tauri::command]
fn delete_channel(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::delete_channel(&conn, id).map_err(|e| e.to_string())
}

/// All output templates (Templates tab, Channels tab).
#[tauri::command]
fn list_templates(db: tauri::State<'_, Db>) -> Result<Vec<db::Template>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_templates(&conn).map_err(|e| e.to_string())
}

/// The active templates (max 4) previewed on the console Output grid.
#[tauri::command]
fn list_active_templates(db: tauri::State<'_, Db>) -> Result<Vec<db::Template>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::list_active_templates(&conn).map_err(|e| e.to_string())
}

/// Activate/deactivate a template on the console Output grid. Enforces the
/// max-4 rule with a clear error the UI can show.
#[tauri::command]
fn set_template_active(db: tauri::State<'_, Db>, id: i64, active: bool) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    if active {
        let others = db::active_template_count(&conn, id).map_err(|e| e.to_string())?;
        if others >= 4 {
            return Err(
                "Only 4 templates can be active on the console at once — deactivate one first."
                    .into(),
            );
        }
    }
    db::set_template_active(&conn, id, active).map_err(|e| e.to_string())
}

/// Create a new (blank-styled) template. Returns its id.
#[tauri::command]
fn create_template(db: tauri::State<'_, Db>, name: Option<String>) -> Result<i64, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let name = name.unwrap_or_else(|| "New template".into());
    db::create_template(&conn, name.trim()).map_err(|e| e.to_string())
}

/// Delete a template (unassigns it from any channel first).
#[tauri::command]
fn delete_template(db: tauri::State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    db::delete_template(&conn, id).map_err(|e| e.to_string())
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
    // Push the fresh template live to any OBS/kiosk client showing it (WYSIWYG),
    // and to native output windows via the event.
    if let Ok(conn) = db.0.lock() {
        if let Ok(Some(fresh)) = db::get_template(&conn, id) {
            if let Ok(j) = serde_json::to_string(&fresh) {
                app.state::<channels::KioskHub>().set_template(id, &j);
            }
        }
    }
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

/// Operator "Clear all screens" / blackout — blank every output channel (D4).
/// Instant, always available. Same effect the spoken "clear"/"blackout" reaches.
#[tauri::command]
fn clear_screens(app: tauri::AppHandle) {
    channels::clear(&app);
    forget_debounce(&app);
    persist_cue(&app, "clear_screens", None);
}

/// Blackout every output (opaque black). The next fire/clear cancels it.
#[tauri::command]
fn blackout(app: tauri::AppHandle) {
    channels::black(&app);
    forget_debounce(&app);
    persist_cue(&app, "blackout", None);
}

/// The screens are empty, so nothing is "already showing" any more — drop the
/// repeat-cooldown memory. Otherwise, clearing the screen and having the preacher
/// immediately re-reference the same verse would leave it blank for the rest of
/// the cooldown: the debounce would suppress the one fire the operator wants.
fn forget_debounce(app: &tauri::AppHandle) {
    if let Ok(mut r) = app.state::<Routing>().0.lock() {
        r.forget_last_fire();
    }
}

/// Push the "up next" preview to the stage/confidence monitor. None clears it.
#[tauri::command]
fn set_stage_next(app: tauri::AppHandle, label: Option<String>, text: Option<String>) {
    channels::stage_next(&app, label, text);
}

/// D5: push an emergency announcement over whatever is currently shown, on every
/// output channel. Reuses the shared content broadcast (no per-channel special-
/// casing) so it renders through the same template engine as any slide.
#[tauri::command]
fn push_announcement(app: tauri::AppHandle, message: String) -> Result<(), String> {
    let message = message.trim().to_string();
    if message.is_empty() {
        return Err("empty announcement".into());
    }
    channels::broadcast_content(
        &app,
        OutputContent {
            reference: "Announcement".into(),
            text: Some(message.clone()),
            translation: None,
            ..Default::default()
        },
    );
    persist_cue(&app, "announcement", Some(&message));
    Ok(())
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
    // Prefer the user's Downloads folder; fall back to app-data/exports. Both
    // resolved per-OS — this used to demand $HOME and hardcode a macOS path, so
    // exporting a service failed outright on Windows with "no HOME".
    let downloads = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|h| std::path::PathBuf::from(h).join("Downloads"))
        .filter(|d| d.is_dir());
    let dir = match downloads {
        Some(d) => d,
        None => {
            let d = db::app_data_dir().join("exports");
            std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
            d
        }
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

/// Master AI switch (D1). ON = the AI drives output: high-confidence detections
/// auto-fire, mid-confidence surface as one-tap suggestions. OFF = fully manual —
/// the pipeline still transcribes, but nothing auto-reaches the screens. Operator
/// override (manual push / next / clear) is a separate path and works in BOTH
/// modes (CLAUDE.md: override is first-class, never gated by this). Returns the
/// new state.
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
