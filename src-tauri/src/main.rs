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
/// End-to-end tests for the fire → nav → clear path. Test-only. `main.rs` had zero
/// tests, so the one path that actually puts scripture on a wall was verified only
/// by hand — see the module doc.
#[cfg(test)]
mod e2e;
mod error;
/// Detection benchmark. Test-only — it exists to FAIL THE BUILD when detection
/// regresses, not to ship. `cargo test eval -- --nocapture` prints the scorecard.
#[cfg(test)]
mod eval;
mod models;
mod pipeline;
mod proimport;
/// The shared QA harness: a first-launch fixture plus the two doors (Tauri events
/// and the kiosk hub) a guarantee has to be checked on. Test-only. See `qa.rs`.
#[cfg(test)]
mod qa;
/// R5 audit evidence: the LAN remote's route surface, the telemetry scrub's
/// blocklist shape, and the nav choose-then-commit baseline. Test-only, and two
/// of its tests are RED on purpose — see the module doc.
#[cfg(test)]
mod qa_r5;
/// R6 audit evidence, written independently of R1–R5: the LAN remote's answer to
/// "what is live", checked against what actually reached the wall. Test-only, and
/// two of its tests are RED on purpose — see the module doc.
#[cfg(test)]
mod r6;
mod router;
mod songs;
mod stt;
mod sysprobe;
mod telemetry;

use audio::AudioEngine;
use channels::OutputContent;
use detection::{ContextMemory, DetectionMethod, SemanticIndex, VerseRef};
use pipeline::{Cand, DetectionEvent, Fire, FireStatus};
use router::{RouteDecision, Router, Thresholds};
use rusqlite::Connection;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
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
    /// Wall-clock epoch (ms) the service started — the reference for a monitor's
    /// elapsed timer. `Instant` can't be turned into a wall-clock time, so the
    /// epoch is captured separately at start.
    started_at_ms: i64,
    /// Planned service length in ms, for a monitor's REMAINING timer. 0 = no
    /// target set (the remaining line simply shows nothing). Captured once at
    /// start from the `service.target_minutes` setting, so changing the setting
    /// mid-service does not retro-move the current service's target.
    target_ms: i64,
    last_transcript: Option<i64>,
}

/// Current wall-clock time in epoch milliseconds. `0` before the UNIX epoch
/// (never happens in practice), so callers never handle an error.
fn now_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
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
        // Auto-update. Without it there is no way to deliver a fix to a church
        // that already installed Relay — and this is software that fails LIVE.
        // Update checks are driven from the frontend and are NEVER run during a
        // service (see src/lib/updater.js).
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(Db(Mutex::new(conn)))
        .manage(Audio::default())
        .manage(Routing::default())
        .manage(Outputs::default())
        .manage(Detecting(AtomicBool::new(true)))
        .manage(channels::Rehearsal::default())
        .manage(channels::WallState::default())
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
                // A debug-build-only `RELAY_SENTRY_DSN` stands in for the Settings
                // toggle, so reporting can be tested without re-entering a DSN into
                // every fresh dev DB. `telemetry::dev_dsn()` is `None` by
                // construction in a release build — see its doc comment.
                let dev = telemetry::dev_dsn();
                if dev.is_some() {
                    println!("telemetry: DSN taken from RELAY_SENTRY_DSN (debug build)");
                }
                if let Some(dsn) = dev.or(consent) {
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
            let kiosk_clients = kiosk.clients_handle();
            let kiosk_themes = kiosk.themes_handle();
            // Warm the custom-themes blob so a kiosk connecting before any theme is
            // saved this session still gets the operator's themes on `hello`.
            {
                let db = app.state::<Db>();
                if let Some(blob) =
                    db.0.lock()
                        .ok()
                        .and_then(|conn| db::get_setting(&conn, "themes.custom").ok().flatten())
                {
                    kiosk.cache_themes(&blob);
                }
            }
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
                kiosk_clients,
                kiosk_themes,
                8031,
            ));
            // Serve the output/stage pages over LAN HTTP so other devices load
            // them in a packaged app (not only in `tauri dev`). See channels.rs.
            // The `api` closure is the preacher's-remote control plane: search,
            // next/prev and fire, performed against this app. LAN-only, no auth —
            // a recorded expansion of the broadcast-only exposure (DECISIONS §35).
            let api_handle = app.handle().clone();
            let api: channels::ApiSink = std::sync::Arc::new(move |method: &str, rest: &str| {
                Some(remote_api(&api_handle, method, rest))
            });
            tauri::async_runtime::spawn(channels::run_output_http_server(
                channels::report_to(app.handle()),
                api,
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
            ping,
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
            set_plan_section,
            set_plan_duration,
            set_plan_template,
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
            get_setting,
            set_setting,
            sync_kiosk_themes,
            data_health,
            list_books,
            chapter_verses,
            system_hardware,
            probe_integrations,
            migration_status,
            list_audio_devices,
            local_ip,
            start_capture,
            stop_capture,
            stt_status,
            confirm_detection,
            dismiss_detection,
            get_thresholds,
            set_thresholds,
            get_sensitivity,
            set_sensitivity,
            get_rehearsal,
            set_rehearsal,
            get_crash_reporting,
            set_crash_reporting,
            list_models,
            download_model,
            cancel_model_download,
            load_stt_model,
            select_stt_model,
            manual_fire,
            open_output_window,
            list_output_windows,
            list_output_channels,
            channel_status,
            close_channel_output,
            set_channel_template,
            list_monitors,
            open_channel_output,
            auto_open_outputs,
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
            list_services,
            service_detail,
            export_service,
            list_templates,
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
///
/// NOTE: this floor, not the length of the suggestion list, is what currently
/// limits paraphrase recall. `eval::suggestion_policy_scorecard` shows the right
/// passage sits in the top 5 for 98% of retellings but only 84% survive this
/// cut. Lowering it would trade that back for noise — and the corpus has no
/// negative cases yet (transcript that mentions no scripture at all), so the
/// noise it would cost is currently UNMEASURED. Do not lower it on a hunch.
const SEMANTIC_FLOOR: f32 = 0.30;

/// Most paraphrase alternatives to offer for one transcript chunk.
const SEMANTIC_SUGGESTIONS_MAX: usize = 3;

/// Keep an alternative only if it scores within this fraction of the best hit.
/// At 1.0 only ties survive (the old single-suggestion behaviour); lower widens
/// the list when scores are close. 0.60 measured +12 points of reachable recall
/// on modern-wording retellings for about one extra row.
const SEMANTIC_RELATIVE_FLOOR: f32 = 0.60;

/// Which paraphrase hits are worth an operator's attention.
///
/// Absolute floor removes noise; relative floor keeps the list at one when a
/// verse wins outright and widens it only when Relay is genuinely torn. Input is
/// assumed ordered best-first, as `top_k_explained` returns it.
fn worth_suggesting(
    hits: Vec<(detection::VerseRef, f32, Vec<String>)>,
) -> Vec<(detection::VerseRef, f32, Vec<String>)> {
    let best = hits.first().map(|(_, s, _)| *s).unwrap_or(0.0);
    hits.into_iter()
        .filter(|(_, s, _)| *s >= SEMANTIC_FLOOR && *s >= best * SEMANTIC_RELATIVE_FLOOR)
        .collect()
}

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
    matched_text: Option<String>,
    cue_template_id: Option<i64>,
) -> Fire {
    let looked = db::lookup_verse(conn, &r.book, r.chapter, r.verse)
        .ok()
        .flatten();
    // A plan scripture cue's own template wins; the AI/auto path passes None and
    // gets the scripture content-type default.
    let (template_id, template_json, template_pinned) =
        cue_or_content_tpl(conn, cue_template_id, "scripture");
    // `next_*` are filled in LATER by `attach_next_verse`, after the passage
    // context has been updated — the bounded "up next" verse depends on the
    // current passage span, which is only known after this fire is staged.
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
        next_reference: None,
        next_text: None,
        template_id,
        template_json,
        template_pinned,
        matched_text,
    }
}

/// Fill in the "up next" fields on a fire from the CURRENT passage context.
///
/// Called AFTER the passage has been staged/advanced, so `context.next_verse()`
/// reflects where the walk now is and — critically — is BOUNDED by the passage's
/// range end: reading John 3:16–17 shows no "next" once 3:17 is up, rather than
/// spilling into 3:18. A standalone verse (no range) still previews the following
/// verse in the chapter. `None` when there is no next (end of range/chapter) or
/// the next verse is not in the corpus. Only a monitor template with a `next`
/// layer renders these, so this can never change what the congregation sees.
fn attach_next_verse(conn: &Connection, context: &detection::ContextMemory, fire: &mut Fire) {
    if let Some(nr) = context.next_verse() {
        if let Some(v) = db::lookup_verse(conn, &nr.book, nr.chapter, nr.verse)
            .ok()
            .flatten()
        {
            fire.next_reference = Some(Fire::key_for(&nr));
            fire.next_text = Some(v.text);
        }
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

/// Broadcast content, first stamping the elapsed-service clock onto it when a
/// service is being recorded. ONE place, so every fire path's output carries the
/// timer for a stage/confidence monitor without each caller remembering to — the
/// same reason the pipeline builds the payload once. The Session lock is taken
/// and RELEASED (mapped to a value) before the broadcast emits — never held
/// across an emit (CLAUDE.md rule #2). `try_state` so a context without a managed
/// Session simply stamps nothing rather than panicking.
fn broadcast_with_clock<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    mut content: channels::OutputContent,
) {
    if let Some(session) = handle.try_state::<Session>() {
        if let Ok(g) = session.0.lock() {
            if let Some(st) = g.as_ref() {
                content.service_started_at = Some(st.started_at_ms);
                // Only advertise a target when one is set (>0), so a monitor's
                // remaining line stays blank rather than reading a bogus 0:00.
                content.service_target_ms = (st.target_ms > 0).then_some(st.target_ms);
            }
        }
    }
    channels::broadcast_content(handle, content);
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
fn fire_manual<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    r: VerseRef,
    confidence: f32,
    update: PassageUpdate,
    stage_note: Option<String>,
    cue_template_id: Option<i64>,
) -> bool {
    let db = handle.state::<Db>();
    let ctx = handle.state::<Context>();

    let fire = {
        let Ok(conn) = db.0.lock() else { return false };
        let mut f = resolve_fire(
            &conn,
            r,
            confidence,
            DetectionMethod::Direct,
            FireStatus::Manual,
            stage_note,
            // No evidence line for a human's own decision. "Why is this on screen?"
            // — because you put it there. Explaining that back to the operator would
            // be noise, and worse, would dilute the badge that matters: the one on
            // the AI's guesses.
            None,
            cue_template_id,
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
            // The passage now reflects this fire, so "up next" is the bounded
            // next verse (None at a range end). Computed here, under both locks.
            attach_next_verse(&conn, &context, &mut f);
        }
        if let Ok(mut router) = handle.state::<Routing>().0.lock() {
            // The same wall clock the AI path uses. This was a literal `0`, which
            // on any clock means "long ago" — so a verse the operator had just put
            // on the wall themselves was never protected from the AI immediately
            // re-firing it off the still-rolling STT window.
            router.manual_fire(&f.key, router_clock_ms());
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

    broadcast_with_clock(handle, fire.output());
    let _ = handle.emit("detection://match", fire.event());
    true
}

/// Monotonic milliseconds since process start — THE ROUTER'S CLOCK.
///
/// ── Why this is not the audio timestamp ─────────────────────────────────────
///
/// The router's repeat cooldown asks one question: "has this verse been on the
/// wall long enough that saying it again means the preacher said it again?"
/// That is a question about a room, so it is measured in wall time.
///
/// It used to be handed `TranscriptUpdate::timestamp_ms` — a position in the
/// audio — and that silently breaks the debounce under load. The STT worker
/// drains its entire backlog per decode (stt.rs: "the deeper the backlog, the
/// more audio each decode consumes"), so `last_ts_ms` advances in JUMPS. One
/// decode can move the audio clock 10+ seconds while one second of real time
/// passed, putting every partial past the cooldown. Live, at one-second
/// intervals: `Romans 8:28 · Romans 8:28 · Romans 8:28` — the same verse
/// re-broadcast three times because the clock, not the gate, had moved.
///
/// It fails hardest exactly when whisper is running behind, which is when the
/// transcript is worst and the gate matters most.
///
/// `Router::decide` still takes `now_ms` as a parameter and stays clock-free, so
/// the gate remains deterministic and unit-testable. Only the source changed.
fn router_clock_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Detect references in `text` — direct, context-resolved bare verses, and
/// semantic paraphrase — dedup them, gate each through the router, resolve
/// against the corpus, and emit one `detection://match` per survivor. Dropped
/// (debounced / low-confidence) detections are silent.
///
/// `now_ms` is a WALL-CLOCK monotonic stamp (`router_clock_ms`), never an audio
/// position — see that function for why the difference is load-bearing.
///
/// `is_final` says whether `text` is a CLOSED utterance or a partial that is still
/// growing. Detection deliberately runs on partials (DECISIONS.md) — waiting for a
/// pause would put the verse on the wall long after the preacher moved on — but a
/// partial is a sentence caught mid-word, and one shape of reference is created by
/// that truncation rather than described by it. See the whole-chapter guard below.
fn emit_detections<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    text: &str,
    now_ms: u64,
    is_final: bool,
) {
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

        // Gather candidates. Each one carries the EVIDENCE for itself — the words
        // that produced it — so the console can show the operator why, and not just
        // a number (see pipeline::DetectionEvent).
        let mut candidates: Vec<Cand> = Vec::new();

        let directs = detection::detect_direct(text);
        let direct_empty = directs.is_empty();
        for m in directs {
            // A reading that exists only because the transcript was cut mid-sentence
            // describes the window boundary, not the sermon. See
            // `RefMatch::is_provisional`, which owns the rule so this path and the
            // bench that scores it cannot disagree.
            if m.is_provisional(is_final) {
                continue;
            }
            candidates.push(Cand {
                r: m.reference,
                conf: m.confidence,
                // `m.method`, NOT a hardcoded `Direct`. This line threw away the
                // parser's own verdict about how good the evidence was, and it is
                // the THIRD place in this codebase found doing it on 2026-08-14 —
                // `eval.rs`'s scorer and `detection.rs`'s harness were the other
                // two. Between them they meant the `UncertainBook` cap existed,
                // was unit-tested, passed at the router, and did nothing whatever
                // in the product: "hymn number three sixteen" still reached the
                // wall, because by the time the router saw the candidate it had
                // been relabelled as something Relay heard.
                //
                // Caught by `e2e::ordinary_church_announcements_reach_nobody`,
                // which is the first test in this repo to drive the AI's own path
                // end to end. A router that is told the answer is not a gate.
                method: m.method,
                verse_end: m.verse_end,
                whole_chapter: m.whole_chapter,
                matched: Some(m.matched_text),
            });
        }
        for n in detection::detect_bare_verses(text) {
            if let Some(r) = context.resolve_bare_verse(n) {
                // "…and verse eighteen", resolved against the passage already on
                // screen. The operator needs to see that this came from CONTEXT, not
                // from a book name they never heard the preacher say.
                candidates.push(Cand::single(
                    r,
                    0.88,
                    DetectionMethod::Direct,
                    Some(format!("verse {n}")),
                ));
            }
        }
        // Paraphrase alternatives. Only ONE was ever offered, which threw away
        // most of what the index had already found: measured on the paraphrase
        // corpus, the right passage is in the top 5 for 98% of retellings but is
        // ranked first for only 81% — and for a retelling in modern words, only
        // 53%. The operator was never shown the difference.
        //
        // Two limits, because a longer list is not free — every row costs a
        // volunteer attention in a dark booth mid-service:
        //   * a RELATIVE floor, so the list widens only when Relay is genuinely
        //     torn between similar scores, and stays at one when a verse wins
        //     outright,
        //   * a hard CAP, because a well-quoted verse matches many verses
        //     strongly and would otherwise pad the list exactly when the first
        //     answer was already correct.
        // Both are configuration (§ thresholds are config, not constants).
        for (r, score, terms) in
            worth_suggesting(sem.0.top_k_explained(text, SEMANTIC_SUGGESTIONS_MAX))
        {
            candidates.push(Cand::single(
                r,
                score.min(0.95),
                DetectionMethod::Semantic,
                Some(terms.join(" · ")),
            ));
        }
        if direct_empty {
            for r in detection::detect_ambiguous(text) {
                candidates.push(Cand::single(r, 0.70, DetectionMethod::Ambiguous, None));
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
            // Auto-detection has no plan cue behind it — content-type default.
            let mut fire =
                resolve_fire(&conn, c.r, c.conf, c.method, status, None, c.matched, None);

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
                // Fill "up next" from the now-staged passage (bounded by its end).
                attach_next_verse(&conn, &context, &mut fire);
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
        broadcast_with_clock(handle, content);
    }
    for ev in events {
        let _ = handle.emit("detection://match", ev);
    }
}

/// What a next/back actually did. The operator is told, every time.
///
/// `nav` used to return `()` and `handle_nav` used to return `()` — and inside it
/// were THREE separate silent bail-outs: a poisoned lock, stepping off the end of
/// the passage, and `fire_manual`'s `bool` being discarded outright. So the operator
/// pressed **Next** mid-sermon, the wall did not change, and there was no error, no
/// toast and no log. Nothing anywhere said why.
///
/// It is the same silent-no-op class as the "Screens cleared" lie (DECISIONS §20),
/// living on the key an operator presses more than any other.
///
/// These are NOT all failures, and flattening them into a bool is what hid them.
/// Reaching the end of a passage is a normal, correct boundary; the operator simply
/// needs to know that is why nothing moved. A verse that is missing from the corpus
/// is a real fault. They deserve different sentences.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum NavResult {
    /// It moved. This is the only outcome that changes the screens.
    Fired { reference: String },
    /// The passage has an end and we are standing on it.
    EndOfPassage,
    /// Nothing is staged, so there is nothing to step through.
    NoPassage,
    /// The next verse parsed but is not in the corpus — firing it would blank the
    /// wall (`Fire::may_broadcast`), so we left the screen alone and say so.
    NotInLibrary { reference: String },
}

/// Spoken "next" / "back": step to the adjacent verse in the staged passage.
///
/// Operator intent, so it bypasses the gate — see `fire_manual`, which owns the
/// whole sequence. This and `handle_passage_nav` were previously two ~70-line
/// near-identical functions; all that actually differs between them is how the
/// target verse is chosen, which is the four lines below.
fn handle_nav<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    dir: detection::NavCommand,
) -> error::Result<NavResult> {
    let (target, staged) = {
        let ctx = handle.state::<Context>();
        let context = ctx
            .0
            .lock()
            .map_err(|_| "Relay lost track of the passage it was reading.".to_string())?;
        // Distinguish "there is a passage and we are at its end" from "there is no
        // passage at all" — from the operator's seat those look identical (the screen
        // does not change) and mean completely different things.
        let staged = context.current().is_some();
        let t = match dir {
            detection::NavCommand::Next => context.next_verse(),
            detection::NavCommand::Previous => context.prev_verse(),
        };
        (t, staged)
    };

    let Some(r) = target else {
        return Ok(if staged {
            NavResult::EndOfPassage
        } else {
            NavResult::NoPassage
        });
    };

    let reference = Fire::key_for(&r);
    // Advance keeps the staged passage span, so a range/chapter walk stays bounded.
    // A nav step walks a passage, not a plan cue — content-type default.
    if fire_manual(handle, r, 1.0, PassageUpdate::Advance, None, None) {
        Ok(NavResult::Fired { reference })
    } else {
        Ok(NavResult::NotInLibrary { reference })
    }
}

/// Spoken in-passage jump ("chapter 5 verse 1", "verse 4"): resolve the BOOK from
/// the current context and fire book chapter:verse, keeping the operator inside
/// the same passage. Chapter-only defaults to verse 1; verse-only keeps the
/// current chapter. Returns true if it fired.
fn handle_passage_nav<R: tauri::Runtime>(handle: &tauri::AppHandle<R>, text: &str) -> bool {
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
    fire_manual(handle, target, 1.0, PassageUpdate::Jump, None, None)
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
///
/// ── `window_text` IS THE EVIDENCE, and it is now STORED ─────────────────────
///
/// It used to be a fallback only — used to seed a transcript row when none
/// existed yet, and otherwise thrown away. The row was then attached to
/// `last_transcript`, the most recent FINAL transcript.
///
/// But detection runs on every partial STT hypothesis, and only finals are
/// persisted (`build_stt`). So in a real service the two routinely have nothing
/// to do with each other: nine auto-fires were logged against a final from three
/// minutes earlier which, replayed through the detector, produces no matches at
/// all. `transcript_id` said where the service was; it could not say what was
/// heard. Now `heard_text` does, so a wrong verse on a wall can be explained
/// after the fact instead of guessed at.
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
    let _ = db::insert_detection(
        conn,
        tid,
        verse_id,
        method,
        confidence,
        status,
        Some(ts),
        Some(window_text),
    );
}

/// Record an operator cue (manual_override / clear_screens) into the current
/// service. Locks its own db handle — call outside a held db lock.
fn persist_cue<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    cue_type: &str,
    payload: Option<&str>,
) {
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
//
// EXACTLY ONE CALLER, FOREVER. `greet` is not a health check; it is a COUNTER of
// console mounts that happens to return a string. Its whole diagnostic value is
// that one line means one webview came up — so a second caller does not add
// information, it destroys it. The boot sequence and the Dashboard both used to
// call this to ask "is the engine attached?", which printed the heartbeat three
// times per launch and made it impossible to tell a healthy boot from a webview
// reloading twice. Liveness probes call `ping`, which is silent. Pinned by
// `ipc.test.js`.
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

/// Is the Rust core attached? The SILENT counterpart to `greet`.
///
/// Anything that repeatedly asks "is the bridge up?" — the launch sequence, the
/// Dashboard health panel, anything polled — belongs here. It prints nothing, so
/// it cannot drown the one line that tells you the console actually booted.
#[tauri::command]
fn ping() -> bool {
    true
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
) -> error::Result<Vec<db::VerseRow>> {
    let conn = db.0.lock()?;
    Ok(search_verses(&conn, &sem.0, query.trim()))
}

/// The scripture search itself, over a connection + semantic index — shared by
/// the `search_scripture` command and the preacher-remote HTTP endpoint.
fn search_verses(
    conn: &rusqlite::Connection,
    sem: &SemanticIndex,
    query: &str,
) -> Vec<db::VerseRow> {
    let q = query.trim();
    if q.is_empty() {
        return vec![];
    }

    // Score candidates and rank: exact reference > exact phrase > semantic
    // paraphrase > loose text. Semantic is what turns a paraphrase ("there is
    // therefore no condemnation in christ") into the real verse (Romans 8:1)
    // plus suggestions — the same engine that drives live detection.
    let mut scored: Vec<(f32, db::VerseRow)> = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();

    // 1) Explicit references ("john 3:16", "ps 23").
    for m in detection::detect_direct(q) {
        let r = &m.reference;
        if let Ok(Some(v)) = db::lookup_verse(conn, &r.book, r.chapter, r.verse) {
            if seen.insert(v.id) {
                scored.push((1.0, v));
            }
        }
    }
    // 2) Exact phrase (the whole query appears verbatim).
    if q.split_whitespace().count() >= 2 {
        if let Ok(hits) = db::search_verses_text(conn, q, 12) {
            for v in hits {
                if seen.insert(v.id) {
                    scored.push((0.95, v));
                }
            }
        }
    }
    // 3) Semantic paraphrase — top matches by meaning, highest first.
    for (r, score) in sem.top_k(q, 12) {
        if score < 0.08 {
            continue;
        }
        if let Ok(Some(v)) = db::lookup_verse(conn, &r.book, r.chapter, r.verse) {
            if seen.insert(v.id) {
                scored.push((0.5 + score * 0.4, v)); // 0.5..0.9 band
            }
        }
    }
    // 4) Full-text word/phrase recall (FTS5, bm25-ranked). Catches loose,
    //    non-contiguous word queries ("lord shepherd") a substring LIKE misses,
    //    and ranks the best-matching verse first.
    for (i, v) in db::search_verses_fts(conn, q, 15)
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
        if let Ok(hits) = db::search_verses_text(conn, q, 15) {
            for v in hits {
                if seen.insert(v.id) {
                    scored.push((0.3, v));
                }
            }
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(25).map(|(_, v)| v).collect()
}

/// The preacher's-remote HTTP control plane. `rest` is the path after `/api/`
/// (e.g. `search?q=john`, `next`, `prev`, `fire?ref=John%203:16`, `live`). Returns
/// a JSON body. Runs on the HTTP server task with the real AppHandle, so it drives
/// the SAME fire/nav path the console does — one verse engine, one source of truth.
/// Routes that CHANGE what a congregation is looking at.
///
/// Kept as one list because it is the thing the method gate and the CORS decision
/// must agree about, and two copies of it would be the next place they disagree.
fn remote_mutates(route: &str) -> bool {
    matches!(route, "fire" | "next" | "prev" | "clear" | "black")
}

/// The verb a route requires, derived from `remote_mutates` rather than restated.
///
/// Test-only, and it shares the list on purpose: a test that hard-coded its own
/// verbs could keep passing while the gate it exercises drifted underneath it.
#[cfg(test)]
fn remote_verb(rest: &str) -> &'static str {
    let route = rest.split('?').next().unwrap_or("").trim_end_matches('/');
    if remote_mutates(route) {
        "POST"
    } else {
        "GET"
    }
}

/// The preacher's remote, and the answer to the drive-by in DECISIONS §35.
///
/// Every action used to be a side-effecting `GET` answered with
/// `Access-Control-Allow-Origin: *`, so `<img src="http://<relay>:8032/api/black">`
/// on any page — opened by anyone on the church network, browsing anything — blacked
/// out the wall. No preflight, no foothold beyond a victim's browser.
///
/// A mutating route now requires `POST`. An `<img>`, a `<script>`, a stylesheet, a
/// prefetch and a plain link can only issue `GET`, so the entire class is gone, and
/// the wildcard is withheld from those routes as well so nothing cross-origin can
/// read what happened. `search` and `live` are unchanged: they mutate nothing, and a
/// kiosk fetching them cross-origin is a real use.
///
/// **This is not authentication and does not pretend to be.** The LAN control plane
/// is deliberately unauthenticated (DECISIONS §35) — the preacher driving their own
/// reading from a phone is the feature. This closes the drive-by, which is a
/// different and much wider audience than "someone on the church wifi".
fn remote_api<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    method: &str,
    rest: &str,
) -> channels::ApiReply {
    let (route, query) = rest.split_once('?').unwrap_or((rest, ""));
    let route_name = route.trim_end_matches('/');

    if remote_mutates(route_name) && !method.eq_ignore_ascii_case("POST") {
        return channels::ApiReply {
            status: 405,
            body: format!(
                "{{\"ok\":false,\"error\":{}}}",
                json_str(&format!(
                    "{route_name} changes what the congregation sees, so it needs POST, not {}. \
                     See docs/DECISIONS.md §35.",
                    method.to_uppercase()
                ))
            ),
            cors: false,
        };
    }
    let ok = |body: String| channels::ApiReply {
        status: 200,
        body,
        // Withheld from the mutating routes even when they succeed: a cross-origin
        // caller must not be able to read what it just did to the wall.
        cors: !remote_mutates(route_name),
    };
    let param = |key: &str| -> Option<String> {
        query.split('&').find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            (k == key).then(|| channels::urldecode(v))
        })
    };

    ok(match route_name {
        "search" => {
            let q = param("q").unwrap_or_default();
            let rows = {
                let db = app.state::<Db>();
                let sem = app.state::<Semantic>();
                let guard = db.0.lock();
                match guard {
                    Ok(conn) => search_verses(&conn, &sem.0, &q),
                    Err(_) => vec![],
                }
            };
            let items: Vec<String> = rows
                .into_iter()
                .take(20)
                .map(|v| {
                    format!(
                        "{{\"reference\":{},\"text\":{}}}",
                        json_str(&format!("{} {}:{}", v.book, v.chapter, v.verse)),
                        json_str(&v.text)
                    )
                })
                .collect();
            format!("{{\"ok\":true,\"results\":[{}]}}", items.join(","))
        }
        "fire" => match param("ref") {
            None => "{\"ok\":false,\"error\":\"no reference\"}".to_string(),
            Some(reference) => {
                match manual_fire(app.clone(), app.state::<Db>(), reference, None, None) {
                    Ok(()) => format!("{{\"ok\":true,{}}}", live_json(app)),
                    Err(e) => format!("{{\"ok\":false,\"error\":{}}}", json_str(&e.to_string())),
                }
            }
        },
        "next" | "prev" => {
            let dir = if route_name == "next" {
                detection::NavCommand::Next
            } else {
                detection::NavCommand::Previous
            };
            // The OUTCOME rides, not just "ok". `NavResult` exists because a nav
            // that returned `()` let the operator press Next mid-sermon, watch the
            // wall not change, and get no error, no toast and nothing in any log.
            // That was repaired for the console and left standing here: the remote
            // discarded the outcome with `Ok(_)`, so the preacher's own phone
            // answered `{"ok":true}` at the end of a reading and moved nothing —
            // the same silent no-op, one surface along.
            match handle_nav(app, dir) {
                Ok(outcome) => format!(
                    "{{\"ok\":true,\"nav\":{},{}}}",
                    serde_json::to_string(&outcome).unwrap_or_else(|_| "null".into()),
                    live_json(app)
                ),
                Err(e) => format!("{{\"ok\":false,\"error\":{}}}", json_str(&e.to_string())),
            }
        }
        // Panic from the LAN (the preacher's phone, a remote operator): clear or
        // black out every screen. Same threat model as `fire`/`next` — anyone on
        // the church network can already drive the wall — and the same engine the
        // console panic keys use, so the outputs behave identically.
        "clear" => match clear_screens(app.clone()) {
            Ok(()) => "{\"ok\":true}".to_string(),
            Err(e) => format!("{{\"ok\":false,\"error\":{}}}", json_str(&e.to_string())),
        },
        "black" => match blackout(app.clone()) {
            Ok(()) => "{\"ok\":true}".to_string(),
            Err(e) => format!("{{\"ok\":false,\"error\":{}}}", json_str(&e.to_string())),
        },
        "live" => format!("{{\"ok\":true,{}}}", live_json(app)),
        _ => "{\"ok\":false,\"error\":\"unknown\"}".to_string(),
    })
}

/// The current live verse (reference + text) as JSON fields, for the remote to
/// show what is on the wall. Reads the context's current passage anchor.
fn live_json<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> String {
    // WHAT THE CONGREGATION CAN SEE — not where the playhead is.
    //
    // This read the Context passage ANCHOR and published it under the key `live`.
    // The anchor deliberately survives a clear (it is what makes `→` resume rather
    // than restart), so the preacher's phone was told "John 3:16 is live" over
    // cleared screens and over blacked-out ones. Cued ≠ On Air, violated on the one
    // surface whose holder cannot look up and check.
    //
    // It also answered a REHEARSAL fire byte-identically to a real one, so a
    // preacher practising on a Thursday was told the congregation's wall had their
    // verse on it. Containment held — nothing reached the wall or the kiosk — but
    // the HTTP control plane is a fifth door and it is a *reporter*, not a
    // publisher, so nobody enumerated it. Same quiet shape as the `stage_next` leak.
    let rehearsing = app
        .try_state::<channels::Rehearsal>()
        .map(|r| r.on())
        .unwrap_or(false);
    let wall = app.try_state::<channels::WallState>();
    let on_air = wall.as_ref().map(|w| w.on_air()).unwrap_or(false);
    let blacked = wall.as_ref().map(|w| w.blacked()).unwrap_or(false);

    // The anchor still rides, under a name that says what it is: where the
    // transport would resume. It is genuinely useful to the remote — it is what
    // Next/Prev will step — and it is not a claim about any screen.
    let ctx = app.state::<Context>();
    let cur = ctx.0.lock().ok().and_then(|c| c.current().cloned());
    let cued = match &cur {
        Some(r) => json_str(&format!("{} {}:{}", r.book, r.chapter, r.verse)),
        None => "null".to_string(),
    };

    let live = if on_air && !rehearsing {
        match &cur {
            Some(r) => {
                let text = {
                    let db = app.state::<Db>();
                    db.0.lock()
                        .ok()
                        .and_then(|conn| {
                            db::lookup_verse(&conn, &r.book, r.chapter, r.verse)
                                .ok()
                                .flatten()
                        })
                        .map(|v| v.text)
                        .unwrap_or_default()
                };
                format!(
                    "{{\"reference\":{},\"text\":{}}}",
                    json_str(&format!("{} {}:{}", r.book, r.chapter, r.verse)),
                    json_str(&text)
                )
            }
            None => "null".to_string(),
        }
    } else {
        "null".to_string()
    };

    format!("\"live\":{live},\"cued\":{cued},\"rehearsing\":{rehearsing},\"blacked\":{blacked}")
}

/// Minimal JSON string escaper (quotes, backslashes, control chars).
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Planner: all service plans (newest first) with cue counts.
#[tauri::command]
fn list_plans(db: tauri::State<'_, Db>) -> error::Result<Vec<db::PlanSummary>> {
    let conn = db.0.lock()?;
    db::list_plans(&conn).map_err(Into::into)
}

/// Planner: create a plan.
#[tauri::command]
fn create_plan(db: tauri::State<'_, Db>, title: String, date: String) -> error::Result<i64> {
    let title = title.trim();
    if title.is_empty() {
        return Err(error::Error::refused("plan needs a title"));
    }
    let conn = db.0.lock()?;
    db::create_plan(&conn, title, &date).map_err(Into::into)
}

/// Planner: delete a plan and its cues.
#[tauri::command]
fn delete_plan(db: tauri::State<'_, Db>, id: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::delete_plan(&conn, id).map_err(Into::into)
}

/// Planner: duplicate a plan (with all its cues). Returns the new plan id.
#[tauri::command]
fn duplicate_plan(
    db: tauri::State<'_, Db>,
    id: i64,
    title: String,
    date: String,
) -> error::Result<i64> {
    let title = title.trim();
    if title.is_empty() {
        return Err(error::Error::refused("the copy needs a title"));
    }
    let conn = db.0.lock()?;
    db::duplicate_plan(&conn, id, title, &date).map_err(Into::into)
}

/// Planner: ordered cues of a plan.
#[tauri::command]
fn plan_items(db: tauri::State<'_, Db>, plan_id: i64) -> error::Result<Vec<db::PlanItem>> {
    let conn = db.0.lock()?;
    db::plan_items(&conn, plan_id).map_err(Into::into)
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
) -> error::Result<i64> {
    let conn = db.0.lock()?;
    db::add_plan_item(
        &conn,
        plan_id,
        &cue_type,
        &label,
        &payload_json,
        template_id,
    )
    .map_err(Into::into)
}

/// Planner: remove a cue.
#[tauri::command]
fn remove_plan_item(db: tauri::State<'_, Db>, id: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::remove_plan_item(&conn, id).map_err(Into::into)
}

/// Planner: reorder a cue up (-1) or down (+1).
#[tauri::command]
fn move_plan_item(db: tauri::State<'_, Db>, id: i64, direction: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::move_plan_item(&conn, id, direction).map_err(Into::into)
}

/// Planner: set/clear a cue's operator stage note (confidence-monitor only).
#[tauri::command]
fn set_plan_note(db: tauri::State<'_, Db>, id: i64, note: String) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::set_plan_note(&conn, id, &note).map_err(Into::into)
}

/// Planner: apply a drag-reorder — the new ordered list of cue ids.
#[tauri::command]
fn reorder_plan(db: tauri::State<'_, Db>, plan_id: i64, ids: Vec<i64>) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::reorder_plan_items(&conn, plan_id, &ids).map_err(Into::into)
}

/// Planner: begin a section at this cue (blank title merges it into the one above).
#[tauri::command]
fn set_plan_section(db: tauri::State<'_, Db>, id: i64, title: String) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::set_plan_section(&conn, id, &title).map_err(Into::into)
}

/// Planner: set a cue's planned length in seconds (0 = untimed).
#[tauri::command]
fn set_plan_duration(db: tauri::State<'_, Db>, id: i64, seconds: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::set_plan_duration(&conn, id, seconds).map_err(Into::into)
}

/// Planner: point a cue at a specific template, or back at the channel default.
#[tauri::command]
fn set_plan_template(
    db: tauri::State<'_, Db>,
    id: i64,
    template_id: Option<i64>,
) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::set_plan_template(&conn, id, template_id).map_err(Into::into)
}

/// Lyrics: all songs (with section counts).
#[tauri::command]
fn list_songs(db: tauri::State<'_, Db>) -> error::Result<Vec<db::SongSummary>> {
    let conn = db.0.lock()?;
    db::list_songs(&conn).map_err(Into::into)
}

/// Lyrics: search songs by title or author (Planner add + Library browse).
#[tauri::command]
fn search_songs(db: tauri::State<'_, Db>, query: String) -> error::Result<Vec<db::SongSummary>> {
    let q = query.trim();
    let conn = db.0.lock()?;
    if q.is_empty() {
        db::list_songs(&conn).map_err(Into::into)
    } else {
        db::search_songs(&conn, q).map_err(Into::into)
    }
}

/// Lyrics: a full song with ordered sections.
#[tauri::command]
fn get_song(db: tauri::State<'_, Db>, id: i64) -> error::Result<Option<db::Song>> {
    let conn = db.0.lock()?;
    db::get_song(&conn, id).map_err(Into::into)
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
) -> error::Result<i64> {
    let title = title.trim();
    if title.is_empty() {
        return Err(error::Error::refused("song needs a title"));
    }
    let sections = songs::parse_song(&lyrics);
    if sections.is_empty() {
        return Err(error::Error::refused("no lyrics found to import"));
    }
    let conn = db.0.lock()?;
    // Dedupe by title: replace an existing song rather than duplicate it.
    if let Some(id) = db::song_id_by_title(&conn, title)? {
        db::update_song(
            &conn,
            id,
            title,
            author.trim(),
            ccli.trim(),
            song_key.trim(),
            bpm,
            &sections,
        )?;
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
        .map_err(Into::into)
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
) -> error::Result<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err(error::Error::refused("song needs a title"));
    }
    let conn = db.0.lock()?;
    db::update_song(
        &conn,
        id,
        title,
        author.trim(),
        ccli.trim(),
        song_key.trim(),
        bpm,
        &sections,
    )?;
    // Propagate the edit to every plan that cues this song (real-time everywhere).
    db::sync_song_in_plans(&conn, id, title, &sections)?;
    Ok(())
}

/// Lyrics: delete a song and its sections.
#[tauri::command]
fn delete_song(db: tauri::State<'_, Db>, id: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::delete_song(&conn, id).map_err(Into::into)
}

/// Arrangements: named play-orders of a song's sections.
#[tauri::command]
fn list_arrangements(
    db: tauri::State<'_, Db>,
    song_id: i64,
) -> error::Result<Vec<db::Arrangement>> {
    let conn = db.0.lock()?;
    db::list_arrangements(&conn, song_id).map_err(Into::into)
}

/// Arrangements: create (id None) or update one. Returns its id.
#[tauri::command]
fn save_arrangement(
    db: tauri::State<'_, Db>,
    song_id: i64,
    id: Option<i64>,
    name: String,
    sequence: Vec<i64>,
) -> error::Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(error::Error::refused("arrangement needs a name"));
    }
    let conn = db.0.lock()?;
    db::save_arrangement(&conn, song_id, id, name, &sequence).map_err(Into::into)
}

/// Arrangements: delete one.
#[tauri::command]
fn delete_arrangement(db: tauri::State<'_, Db>, id: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::delete_arrangement(&conn, id).map_err(Into::into)
}

/// Scripture (Library): verses the operator saved.
#[tauri::command]
fn list_saved_scripture(db: tauri::State<'_, Db>) -> error::Result<Vec<db::SavedScripture>> {
    let conn = db.0.lock()?;
    db::list_saved_scripture(&conn).map_err(Into::into)
}

/// Scripture (Library): resolve a reference and save it to the library.
#[tauri::command]
fn save_scripture(
    db: tauri::State<'_, Db>,
    book: String,
    chapter: i64,
    verse: i64,
    date: String,
) -> error::Result<db::SavedScripture> {
    let conn = db.0.lock()?;
    let v = db::lookup_verse(&conn, &book, chapter, verse)?
        .ok_or_else(|| format!("{book} {chapter}:{verse} not found"))?;
    let id = db::save_scripture(&conn, &v, &date)?;
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
fn delete_saved_scripture(db: tauri::State<'_, Db>, id: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::delete_saved_scripture(&conn, id).map_err(Into::into)
}

/// Announcements (Library): all saved notices, newest first.
#[tauri::command]
fn list_announcements(db: tauri::State<'_, Db>) -> error::Result<Vec<db::Announcement>> {
    let conn = db.0.lock()?;
    db::list_announcements(&conn).map_err(Into::into)
}

/// Announcements: create (id None) or update one. Returns its id.
#[tauri::command]
fn save_announcement(
    db: tauri::State<'_, Db>,
    id: Option<i64>,
    title: String,
    body: String,
    date: String,
) -> error::Result<i64> {
    let title = title.trim();
    let body = body.trim();
    if title.is_empty() && body.is_empty() {
        return Err(error::Error::refused(
            "an announcement needs a title or body",
        ));
    }
    let conn = db.0.lock()?;
    let saved = db::save_announcement(&conn, id, title, body, &date)?;
    // Editing an existing announcement propagates to any plan that cues it.
    if id.is_some() {
        let _ = db::sync_announcement_in_plans(&conn, saved, title, body);
    }
    Ok(saved)
}

/// Announcements: delete one.
#[tauri::command]
fn delete_announcement(db: tauri::State<'_, Db>, id: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::delete_announcement(&conn, id).map_err(Into::into)
}

/// Media (Library): all imported media/document assets.
#[tauri::command]
fn list_media(db: tauri::State<'_, Db>) -> error::Result<Vec<db::MediaAsset>> {
    let conn = db.0.lock()?;
    db::list_media(&conn).map_err(Into::into)
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
) -> error::Result<db::MediaAsset> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("could not read file data: {e}"))?;
    let dir = db::media_dir();
    std::fs::create_dir_all(&dir)?;

    let conn = db.0.lock()?;
    let id = db::insert_media(&conn, &kind, &filename, &date)?;
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
    std::fs::write(&path, &bytes)?;
    let path_str = path.to_string_lossy().to_string();
    db::set_media_path(&conn, id, &path_str)?;
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
fn delete_media(db: tauri::State<'_, Db>, id: i64) -> error::Result<()> {
    let path = {
        let conn = db.0.lock()?;
        db::delete_media(&conn, id)?
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
fn parse_import(filename: String, data: String) -> error::Result<Vec<ReviewSong>> {
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
        return Err(error::Error::refused("no lyrics found in this file"));
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
) -> error::Result<ImportResult> {
    let conn = db.0.lock()?;
    let mut added = Vec::new();
    let mut replaced = Vec::new();
    for s in songs {
        let title = s.title.trim();
        if title.is_empty() || s.sections.is_empty() {
            continue;
        }
        if let Some(id) = db::song_id_by_title(&conn, title)? {
            db::update_song(
                &conn,
                id,
                title,
                s.author.trim(),
                s.ccli.trim(),
                s.song_key.trim(),
                s.bpm,
                &s.sections,
            )?;
            db::sync_song_in_plans(&conn, id, title, &s.sections)?;
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
            )?;
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
) -> error::Result<ImportResult> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("could not read file data: {e}"))?;
    let songs = proimport::import_bytes(&filename, &bytes)?;
    if songs.is_empty() {
        return Err(error::Error::refused("no lyrics found in this file"));
    }
    let conn = db.0.lock()?;
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
        if let Some(id) = db::song_id_by_title(&conn, &song.title)? {
            db::replace_song_sections(&conn, id, &sections)?;
            replaced.push(song.title);
        } else {
            db::import_song(&conn, &song.title, "", "", "", None, &date, &sections)?;
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
    template_id: Option<i64>,
) -> error::Result<()> {
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
    let (tid, tjson, tpinned) = {
        let conn = db.0.lock()?;
        cue_or_content_tpl(&conn, template_id, "countdown")
    };
    broadcast_with_clock(
        &app,
        OutputContent {
            kind: Some("countdown".into()),
            reference: label.trim().to_string(),
            countdown_to: Some(target),
            countdown_done: clean_note(Some(done_msg)),
            template_id: tid,
            template_json: tjson,
            template_pinned: tpinned,
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
// GENERIC OVER THE RUNTIME, deliberately (CLAUDE.md §24). Welded to the
// concrete desktop handle, this path could not be driven from `e2e.rs` — and
// the one code that decides what a congregation reads would have no test.
#[tauri::command]
fn fire_content<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    label: String,
    text: String,
    kind: String,
    stage_note: Option<String>,
    template_id: Option<i64>,
) -> error::Result<()> {
    let label = label.trim().to_string();
    let (tid, tjson, tpinned) = {
        let conn = db.0.lock()?;
        cue_or_content_tpl(&conn, template_id, &kind)
    };
    // A LYRIC SLIDE PROJECTS THE LYRIC. The congregation is not singing the
    // song title, and "Blessed Assurance · Slide 1" across the top of the wall
    // is the operator's bookkeeping leaking onto a screen full of people. The
    // label still names the cue in history and in the plan — it just does not
    // go out. Scripture is the opposite case: the reference IS part of what is
    // being shown, so it is projected.
    let projected = if kind == "song" {
        String::new()
    } else {
        label.clone()
    };
    broadcast_with_clock(
        &app,
        OutputContent {
            kind: Some(kind.clone()),
            reference: projected,
            text: Some(text),
            translation: None,
            template_id: tid,
            template_json: tjson,
            template_pinned: tpinned,
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
fn fire_media(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    id: i64,
    template_id: Option<i64>,
) -> error::Result<()> {
    let (kind, filename, tid, tjson, tpinned): (String, String, Option<i64>, Option<String>, bool) = {
        let conn = db.0.lock()?;
        let (k, f) = conn
            .query_row(
                "SELECT kind, filename FROM media_assets WHERE id = ?1",
                [id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(|_| "media not found".to_string())?;
        let (tid, tjson, tpinned) = cue_or_content_tpl(&conn, template_id, "media");
        (k, f, tid, tjson, tpinned)
    };
    let media_kind = match kind.as_str() {
        "image" => "image",
        "video" => "video",
        _ => {
            return Err(error::Error::refused(
                "documents can't be shown as an output background yet",
            ))
        }
    };
    let ip = local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
    broadcast_with_clock(
        &app,
        OutputContent {
            kind: Some("media".into()),
            media_url: Some(format!("http://{ip}:8032/media/{id}")),
            media_kind: Some(media_kind.to_string()),
            template_id: tid,
            template_json: tjson,
            template_pinned: tpinned,
            ..Default::default()
        },
    );
    persist_cue(&app, "media", Some(&filename));
    Ok(())
}

/// The template a fire should render with: the CUE's own choice when it set one,
/// otherwise the content-type default.
///
/// A Planner cue can carry a `template_id` (the operator picked a specific look
/// for that item), but every fire path used to resolve the template purely from
/// the content TYPE — so a scripture cue always rendered with the one scripture
/// default and the per-cue choice was dead data. This is the seam that honours
/// it: "always use the template that is set for a planner item when pushing it".
///
/// A cue pointing at a since-deleted template falls back to the content default
/// rather than the channel's, so the intent (a deliberate, non-default look)
/// degrades to the next best thing instead of to whatever the channel happens to
/// be set to.
fn cue_or_content_tpl(
    conn: &rusqlite::Connection,
    cue_template_id: Option<i64>,
    kind: &str,
) -> (Option<i64>, Option<String>, bool) {
    if let Some(id) = cue_template_id {
        if let Ok(Some(t)) = db::get_template(conn, id) {
            if let Ok(j) = serde_json::to_string(&t) {
                // PINNED: a cue's deliberate choice overrides the screen's template.
                return (Some(id), Some(j), true);
            }
        }
    }
    // A content-type default DEFERS to the screen's own template, so it does NOT
    // ship the template JSON — only the id, for the console readout. Each output
    // resolves its own template locally.
    //
    // This is also a hard PERFORMANCE fix: `content_tpl` used to fetch AND
    // serialize the whole default template on every fire and broadcast it to every
    // output. A default template carrying an embedded image (a `data:` URL) is
    // MEGABYTES — one was 13 MB — so every verse took seconds to serialize, send
    // and re-parse on each screen. Reading only the id (a settings lookup) makes a
    // fire instant regardless of how heavy the default template is.
    let id = db::content_template_id(conn, kind).ok().flatten();
    (id, None, false)
}

/// The default template ids mapped to each content type.
#[derive(serde::Serialize)]
struct ContentTemplates {
    scripture: Option<i64>,
    song: Option<i64>,
    media: Option<i64>,
    announce: Option<i64>,
    countdown: Option<i64>,
}

/// Read the content-type → template mapping (Templates screen defaults).
#[tauri::command]
fn get_content_templates(db: tauri::State<'_, Db>) -> error::Result<ContentTemplates> {
    let conn = db.0.lock()?;
    let id = |k: &str| db::content_template_id(&conn, k).ok().flatten();
    Ok(ContentTemplates {
        scripture: id("scripture"),
        song: id("song"),
        media: id("media"),
        announce: id("announce"),
        countdown: id("countdown"),
    })
}

/// Map a content type to a template (None clears it → channel default).
#[tauri::command]
fn set_content_template(
    db: tauri::State<'_, Db>,
    kind: String,
    template_id: Option<i64>,
) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::set_content_template(&conn, &kind, template_id).map_err(Into::into)
}

/// Read a raw app setting by key (the generic KV store). Used by the frontend
/// for small, whole-set config blobs — currently the operator's custom THEMES
/// (`themes.custom`), which are read and written as one JSON array. Returns None
/// when the key was never set. This is a general primitive on purpose: it is the
/// offline-first, local-SQLite home for future frontend-owned config that does
/// not warrant its own table.
#[tauri::command]
fn get_setting(db: tauri::State<'_, Db>, key: String) -> error::Result<Option<String>> {
    let conn = db.0.lock()?;
    db::get_setting(&conn, &key).map_err(Into::into)
}

/// Write a raw app setting (upsert). Counterpart to `get_setting`.
#[tauri::command]
fn set_setting(db: tauri::State<'_, Db>, key: String, value: String) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::set_setting(&conn, &key, &value).map_err(Into::into)
}

/// Push the operator's custom themes to every connected kiosk/OBS client so a
/// browser source (no DB) can resolve a template that pins a custom theme. The
/// frontend calls this after persisting the `themes.custom` blob; builtin themes
/// need no sync (the kiosk page bundles them). A no-op when nothing is connected.
#[tauri::command]
fn sync_kiosk_themes(kiosk: tauri::State<'_, channels::KioskHub>, themes_json: String) {
    kiosk.set_themes(&themes_json);
}

/// Books available to browse, in canonical order — Library (§7).
#[tauri::command]
fn list_books(db: tauri::State<'_, Db>) -> error::Result<Vec<db::BookSummary>> {
    let conn = db.0.lock()?;
    let tid = db::active_translation_id(&conn)?;
    db::list_books(&conn, tid).map_err(Into::into)
}

/// One chapter's verses, in order — the Library's reading pane.
#[tauri::command]
fn chapter_verses(
    db: tauri::State<'_, Db>,
    book: String,
    chapter: i64,
) -> error::Result<Vec<db::VerseRow>> {
    let conn = db.0.lock()?;
    let tid = db::active_translation_id(&conn)?;
    db::chapter_verses(&conn, tid, &book, chapter).map_err(Into::into)
}

/// Number of verses currently seeded — surfaced in Settings as a data-layer
/// health indicator.
#[tauri::command]
fn data_health(db: tauri::State<'_, Db>) -> error::Result<i64> {
    let conn = db.0.lock()?;
    db::verse_count(&conn).map_err(Into::into)
}

/// List available audio input devices for the Settings picker.
#[tauri::command]
fn list_audio_devices() -> Vec<audio::DeviceInfo> {
    audio::list_input_devices()
}

/// What this machine and this build can do — Hardware Check (Launch & Startup).
///
/// Measures the volume holding APP-DATA, not the boot volume: models, media and
/// the database all land there, and it is the one that fills up.
#[tauri::command]
fn system_hardware() -> sysprobe::Hardware {
    sysprobe::read(&db::app_data_dir())
}

/// Is something listening on the default OBS / ATEM ports — Plugin Loading.
///
/// A TCP connect and nothing more. Relay implements neither control protocol, so
/// it may not claim the app is running; the screen words it as "something is
/// listening on the port a default install would use".
#[tauri::command]
async fn probe_integrations() -> Vec<sysprobe::PortProbe> {
    // Two 300 ms connects worst case, off the main thread — a boot screen must
    // never be held behind a firewall prompt.
    tauri::async_runtime::spawn_blocking(sysprobe::probe_integrations)
        .await
        .unwrap_or_default()
}

/// One row of the Database Migration screen.
#[derive(serde::Serialize)]
struct MigrationRow {
    label: String,
    table: String,
    present: bool,
}

/// What the schema actually looks like — Database Migration (Launch & Startup).
#[derive(serde::Serialize)]
struct MigrationStatus {
    version: i64,
    expected: i64,
    tables: Vec<MigrationRow>,
    /// Did the `detections.status` rebuild land? (CLAUDE.md §25)
    manual_status: bool,
    /// A leftover `detections_new` — the fingerprint of the §25 failure.
    scratch_table: bool,
}

/// Report the schema by ASKING THE DATABASE.
///
/// The migration runner finishes before the webview exists, so there is nothing
/// to stream — but "already applied" was previously asserted from a hard-coded
/// list and would have drawn six green ticks over a database missing every one
/// of those tables. This queries `sqlite_master`.
#[tauri::command]
fn migration_status(db: tauri::State<'_, Db>) -> error::Result<MigrationStatus> {
    let conn = db.0.lock()?;
    let (version, expected, rows) = db::schema_report(&conn)?;
    let (manual_status, scratch_table) = db::manual_status_report(&conn)?;
    Ok(MigrationStatus {
        version,
        expected,
        tables: rows
            .into_iter()
            .map(|(label, table, present)| MigrationRow {
                label: label.to_string(),
                table: table.to_string(),
                present,
            })
            .collect(),
        manual_status,
        scratch_table,
    })
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
) -> error::Result<()> {
    let mut slot = audio.0.lock()?;
    if let Some(engine) = slot.take() {
        engine.stop();
    }
    // Feed the same chunks to STT when a model is loaded. The sender is a clone,
    // so the persistent STT worker outlives individual capture start/stop.
    let stt_tx = stt.0.lock()?.as_ref().map(|e| e.sender());
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
async fn stop_capture(audio: tauri::State<'_, Audio>) -> error::Result<()> {
    let mut slot = audio.0.lock()?;
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
fn stt_status(stt: tauri::State<'_, Stt>) -> error::Result<StatusStt> {
    let slot = stt.0.lock()?;
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
fn list_translations(db: tauri::State<'_, Db>) -> error::Result<Vec<db::Translation>> {
    let conn = db.0.lock()?;
    db::list_translations(&conn).map_err(Into::into)
}

/// The active translation id used for verse lookups + output. Falls back to the
/// first (KJV) when unset.
#[tauri::command]
fn get_active_translation(db: tauri::State<'_, Db>) -> error::Result<Option<i64>> {
    let conn = db.0.lock()?;
    let set = db::get_setting(&conn, "active_translation")?.and_then(|v| v.parse::<i64>().ok());
    match set {
        Some(id) => Ok(Some(id)),
        None => Ok(db::list_translations(&conn)?.first().map(|t| t.id)),
    }
}

/// Choose which translation to read from. Every verse lookup (detection, nav,
/// manual, output) then prefers it, falling back to any that has the verse.
#[tauri::command]
fn set_active_translation(db: tauri::State<'_, Db>, id: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::set_setting(&conn, "active_translation", &id.to_string()).map_err(Into::into)
}

/// Set the STT language: a code ("yo"/"sw"/"ha"/"en"/…) or null for auto-detect
/// (code-switching). Tier-1 targets: Yoruba, Swahili, Hausa (CLAUDE.md).
#[tauri::command]
fn set_stt_language(stt: tauri::State<'_, Stt>, language: Option<String>) -> error::Result<()> {
    let slot = stt.0.lock()?;
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
fn open_ndi_output(_template_id: i64) -> error::Result<String> {
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
fn confirm_detection<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    routing: tauri::State<'_, Routing>,
    rehearsal: tauri::State<'_, channels::Rehearsal>,
    reference: String,
) -> error::Result<Thresholds> {
    // The confidence of the suggestion the operator just accepted — this is the
    // evidence the self-calibrating gate learns from, so it has to outlive the
    // `if let` that parses the reference.
    // ── BOTH FAILURE PATHS REPORT. Neither used to. ─────────────────────────
    //
    // This returned `Ok(thresholds)` in two situations where nothing reached any
    // screen: `detect_direct` finding nothing (the `if let` simply fell through),
    // and `fire_manual` returning `false` — whose bool was DISCARDED, with no
    // binding and no `if`. Its twin `manual_fire` reports both, one function along,
    // with the same engine underneath. `NavResult`'s `Ok(_)` all over again.
    //
    // The reachable case is not hypothetical. `emit_detections` deliberately
    // demotes a parsed-but-absent verse to a suggestion and emits it with
    // `in_library: false` — "heard-but-unresolvable must degrade to a suggestion,
    // never to silence" — and NO frontend file reads `in_library`. So a garbled
    // "Psalms 23:99" renders as an ordinary card with Accept enabled; the backend
    // answered Ok; `capture.js` ran `leavePlan()` and removed the card; and
    // `Live.svelte` flashed **"Now live: Psalms 23:99"** while the previous verse
    // was still on the wall. That is the exact bug the comment above `acceptTop`
    // says was fixed — the caller was hardened and the callee was not.
    //
    // It also fed the calibrator: `record_feedback(true, …)` ran on the Ok path
    // whether or not anything had fired.
    let m = detection::detect_direct(&reference)
        .into_iter()
        .next()
        .ok_or_else(|| {
            error::Error::not_found(format!(
                "could not read a reference from \"{reference}\" — nothing was put on the screens"
            ))
        })?;
    // Always Some now: the parse is a hard precondition rather than an `if let`
    // that could quietly do nothing. `record_feedback` still takes an Option
    // because the router's own tests exercise the None arm.
    let confirmed_conf = Some(m.confidence);
    {
        // Stage the passage span, then fire through the one shared manual path —
        // the operator accepting a suggestion IS a human decision, so it records
        // as "manual" and carries the scripture template like every other fire.
        let end = {
            let conn = db.0.lock()?;
            if m.whole_chapter {
                db::chapter_last_verse(&conn, &m.reference.book, m.reference.chapter)
                    .ok()
                    .flatten()
            } else {
                m.verse_end
            }
        };
        let key = format!(
            "{} {}:{}",
            m.reference.book, m.reference.chapter, m.reference.verse
        );
        if !fire_manual(
            &app,
            m.reference,
            m.confidence,
            PassageUpdate::Note(end),
            None,
            // Confirming an AI suggestion is not a plan cue — scripture default.
            None,
        ) {
            // Same wording as `manual_fire`'s, deliberately: it is the same
            // failure, and a volunteer should not have to learn two sentences for
            // one problem depending on which control they pressed.
            return Err(error::Error::not_found(format!(
                "{key} isn't in the Bible text — check the reference"
            )));
        }
    }
    let t = {
        let mut router = routing.0.lock()?;
        // A rehearsal is not evidence. The volunteer is practising — clicking
        // accept on a verse they picked themselves, against speech that may be
        // them reading aloud from a phone. Feeding that to the self-calibrating
        // gate trains it on a fiction, and the fiction persists onto the profile
        // and into the real service on Sunday.
        if !rehearsal.on() {
            router.record_feedback(true, confirmed_conf);
        }
        router.thresholds()
    };
    // Persist the nudge onto the active profile (calibration survives restart).
    if !rehearsal.on() {
        if let Ok(conn) = db.0.lock() {
            persist_active_thresholds(&conn, t);
        }
    }
    Ok(t)
}

/// Operator rejected an auto-fired detection (undo). Tightens the gate and
/// persists the nudge onto the active profile.
#[tauri::command]
fn dismiss_detection(
    routing: tauri::State<'_, Routing>,
    db: tauri::State<'_, Db>,
    rehearsal: tauri::State<'_, channels::Rehearsal>,
) -> error::Result<Thresholds> {
    let t = {
        let mut router = routing.0.lock()?;
        // No argument: the router remembers what it last auto-fired, so the
        // correction is proportional to what was actually wrong.
        // Not in rehearsal — see confirm_detection.
        if !rehearsal.on() {
            router.record_feedback(false, None);
        }
        router.thresholds()
    };
    if !rehearsal.on() {
        if let Ok(conn) = db.0.lock() {
            persist_active_thresholds(&conn, t);
        }
    }
    Ok(t)
}

/// Is rehearsal mode on?
#[tauri::command]
fn get_rehearsal(rehearsal: tauri::State<'_, channels::Rehearsal>) -> bool {
    rehearsal.on()
}

/// Turn rehearsal mode on or off.
///
/// Leaving rehearsal CLEARS the screens. The outputs have been showing whatever
/// they were showing before the rehearsal began — a countdown, the last verse of
/// the previous service, nothing at all — while the operator has spent twenty
/// minutes watching a console preview that says something else entirely. Handing
/// them back a live wall whose contents they have not looked at in twenty minutes,
/// silently, is how the wrong thing ends up in front of a congregation.
///
/// So the wall is cleared, and the operator puts the next thing up deliberately.
#[tauri::command]
fn set_rehearsal<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session: tauri::State<'_, Session>,
    rehearsal: tauri::State<'_, channels::Rehearsal>,
    on: bool,
) -> error::Result<()> {
    // The other half of the same rule. Mid-service is not when you practise, and
    // an operator who flips this by accident during the sermon would silently cut
    // every screen off from the console with no visible cause on the wall.
    if on {
        let recording = session.0.lock().map(|s| s.is_some())?;
        if recording {
            return Err(error::Error::refused(
                "A service is being recorded. End it before rehearsing.",
            ));
        }
    }
    let was = rehearsal.on();
    rehearsal.set(on);
    if was != on {
        println!(
            "rehearsal: {} — outputs are {}",
            if on { "ON" } else { "OFF" },
            if on {
                "SANDBOXED (console preview only)"
            } else {
                "LIVE"
            }
        );
        // Clear AFTER flipping the flag, so it lands on the right side: entering
        // rehearsal clears the console preview only (the wall is untouched, as it
        // must be — the service may be running); leaving it clears the real wall.
        //
        // Reported, not propagated: the flag has already flipped, so returning Err
        // here would leave the frontend's rehearsal store disagreeing with the
        // backend's actual mode — a worse lie than the one being fixed. The operator
        // is told the clear failed via the panic banner instead.
        clear_or_report(&app);
        let _ = app.emit("rehearsal://changed", on);
    }
    Ok(())
}

/// Crash-reporting status for the Settings toggle.
#[derive(Serialize)]
struct CrashReportingStatus {
    enabled: bool,
    dsn: String,
}

#[tauri::command]
fn get_crash_reporting(db: tauri::State<'_, Db>) -> error::Result<CrashReportingStatus> {
    let conn = db.0.lock()?;
    let dsn = db::get_setting(&conn, telemetry::DSN_KEY)?.unwrap_or_default();
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
) -> error::Result<CrashReportingStatus> {
    {
        let conn = db.0.lock()?;
        db::set_setting(
            &conn,
            telemetry::ENABLED_KEY,
            if enabled { "1" } else { "0" },
        )?;
        db::set_setting(&conn, telemetry::DSN_KEY, dsn.trim())?;
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
fn get_thresholds(routing: tauri::State<'_, Routing>) -> error::Result<Thresholds> {
    let router = routing.0.lock()?;
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
) -> error::Result<Option<RelatedPayload>> {
    let ex = exclude
        .and_then(|s| detection::detect_direct(&s).into_iter().next())
        .map(|m| m.reference);
    let Some(sug) = detection::suggest_related(&text, ex.as_ref(), 4) else {
        return Ok(None);
    };
    let conn = db.0.lock()?;
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
) -> error::Result<i64> {
    let Some(m) = detection::detect_direct(&reference).into_iter().next() else {
        return Ok(0);
    };
    let service_id = session.0.lock()?.as_ref().map(|s| s.id);
    let Some(sid) = service_id else {
        return Ok(0);
    };
    let conn = db.0.lock()?;
    let r = &m.reference;
    let Some(v) = db::lookup_verse(&conn, &r.book, r.chapter, r.verse)
        .ok()
        .flatten()
    else {
        return Ok(0);
    };
    db::count_verse_in_service(&conn, sid, v.id).map_err(Into::into)
}

/// Manual override of the thresholds (the always-available slider, DECISIONS.md).
#[tauri::command]
fn set_thresholds(
    routing: tauri::State<'_, Routing>,
    thresholds: Thresholds,
) -> error::Result<Thresholds> {
    let mut router = routing.0.lock()?;
    router.set_thresholds(thresholds);
    Ok(router.thresholds())
}

/// The single operator "sensitivity" dial (0..=100). Applies the SAME thresholds
/// the two-slider Settings control would (`from_sensitivity` — the one forward
/// mapping), so there is exactly one baseline. Returns the resulting dial
/// position so the caller can reflect what actually landed.
/// ── Moving the dial must MOVE THE BASELINE, and must SURVIVE ────────────────
///
/// This used to call `set_thresholds` alone. Two things followed from that, both
/// invisible, and both were caught in a live service:
///
/// 1. **The baseline never moved.** `sensitivity` is defined as the anchor the
///    self-calibration decays back toward (`apply_profile`, DECISIONS §26). Set
///    the gate without setting the anchor and every subsequent operator decision
///    drags the gate back toward the dial position they just left. The dial did
///    not stick even within the session.
///
/// 2. **Nothing was written down.** The learned thresholds are persisted on every
///    confirm/dismiss, but a deliberate dial move was not — so the DB kept a
///    stale learned value and reloaded it at next launch, silently undoing the
///    operator's change. The live evidence was a profile reading
///    `auto_fire = 0.832` beside `sensitivity = 50`, whose mapping is 0.50: a
///    state the router was never in.
///
/// The dial is the operator overruling the machine. It is the one input here
/// that must outlast both the learning and the restart.
#[tauri::command]
fn set_sensitivity(
    routing: tauri::State<'_, Routing>,
    db: tauri::State<'_, Db>,
    sensitivity: u8,
) -> error::Result<u8> {
    let t = Thresholds::from_sensitivity(sensitivity.min(100));
    let landed = {
        let mut router = routing.0.lock()?;
        router.set_thresholds(t);
        router.set_baseline(t); // the dial IS the baseline
        router.thresholds().to_sensitivity()
    }; // lock released before touching the db — Db before Session, never nested here
    if let Ok(conn) = db.0.lock() {
        if let Ok(Some(p)) = db::active_voice_profile(&conn) {
            let _ = db::save_profile_sensitivity(
                &conn,
                p.id,
                landed as i64,
                t.auto_fire as f64,
                t.suggest as f64,
            );
        }
    }
    Ok(landed)
}

/// The current dial position, recovered from the live thresholds.
#[tauri::command]
fn get_sensitivity(routing: tauri::State<'_, Routing>) -> error::Result<u8> {
    let router = routing.0.lock()?;
    Ok(router.thresholds().to_sensitivity())
}

// ===== Voice profiles (Phase B — accent & speaker calibration) ==============

/// Apply a profile's STT settings: language hint (code-switch when None) + the
/// scripture decoder-bias prompt (book names + the profile's extra vocabulary).
fn apply_profile_to_stt(engine: &SttEngine, p: &db::VoiceProfile) {
    engine.set_language(p.language.clone());
    // Bias the decoder in the language actually being preached — feeding it
    // English book names during a Yorùbá sermon pushes whisper AWAY from the
    // words we need it to hear.
    engine.set_prompt(Some(stt::scripture_bias_prompt(
        p.language.as_deref(),
        &p.bias_terms,
    )));
}

/// Apply a full profile live: STT language + bias prompt, and the profile's
/// calibrated thresholds to the router.
fn apply_profile(stt: &Stt, routing: &Routing, p: &db::VoiceProfile) -> error::Result<()> {
    if let Some(e) = stt.0.lock()?.as_ref() {
        apply_profile_to_stt(e, p);
    }
    let mut router = routing.0.lock()?;
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
    // Which model the operator picked, if any. Read and RELEASE the lock before
    // constructing the engine — rule 2, and `try_load` reads a ~1.6 GB file.
    let chosen: Option<String> = handle
        .try_state::<Db>()
        .and_then(|db| db.0.lock().ok().and_then(|c| stt_model_setting(&c)));
    let path = stt::model_path_for(chosen.as_deref())?;
    let handle = handle.clone();
    // Auto-detect re-elects a language every window and, on accented speech, does
    // not settle — which degrades the decode and looks exactly like the AI being
    // bad. Say so once, out loud, because the operator has the control that fixes
    // it and no reason to suspect they should touch it. See `LanguageStability`.
    let lang_stability = Mutex::new(stt::LanguageStability::default());
    match SttEngine::try_load(path, move |update| {
        let _ = handle.emit("stt://transcript", &update);
        if update.is_final {
            println!("stt[{}]: {}", update.language, update.text);
            // Compute under the lock, release, THEN emit — CLAUDE.md rule #2.
            let unstable = lang_stability
                .lock()
                .ok()
                .and_then(|mut s| s.observe(&update.language));
            if let Some(langs) = unstable {
                println!("stt: language auto-detect is unstable ({langs:?})");
                let _ = handle.emit("stt://language_unstable", langs);
            }
            persist_transcript(&handle, &update.text, &update.language);
            // Spoken "next"/"back" navigates from the current verse.
            //
            // This runs on the STT thread, which has nobody to return a result to —
            // exactly like the spoken "clear the screen" below. So a nav that did
            // nothing is PUSHED to the operator rather than swallowed: the preacher
            // says "next", the wall does not move, and the console says why.
            if let Some(cmd) = detection::detect_command(&update.text) {
                match handle_nav(&handle, cmd) {
                    Ok(NavResult::Fired { .. }) => {}
                    Ok(blocked) => {
                        let _ = handle.emit("nav://blocked", blocked);
                    }
                    Err(e) => {
                        eprintln!("nav failed: {e}");
                        let _ = handle.emit("output://panic_failed", e.to_string());
                    }
                }
                return;
            }
            // Spoken "clear the screen" / "blackout".
            if detection::detect_clear(&update.text) {
                clear_or_report(&handle);
                return;
            }
            // Spoken in-passage jump — "chapter 5 verse 1", "verse 4".
            if handle_passage_nav(&handle, &update.text) {
                return;
            }
        }
        // Detect references, then route each through the confidence gate.
        //
        // The gate's clock is WALL TIME, not `update.timestamp_ms`. The audio
        // position advances in backlog-sized jumps and silently defeated the
        // repeat cooldown — see `router_clock_ms`.
        emit_detections(&handle, &update.text, router_clock_ms(), update.is_final);
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

/// The app-settings key holding the model filename the operator chose.
///
/// A filename, not a path or a catalogue id: the catalogue can be re-edited and
/// ids can be renamed, but the file on disk is the thing that has to be found, and
/// `stt::model_path_for` reduces whatever is stored here to a bare filename anyway.
const STT_MODEL_KEY: &str = "stt.model";

fn stt_model_setting(conn: &rusqlite::Connection) -> Option<String> {
    db::get_setting(conn, STT_MODEL_KEY)
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
}

/// Choose which installed speech model to run, and switch to it now.
///
/// `filename` of `None` clears the choice and returns to the default order.
///
/// This is a separate command from `set_setting` on purpose. Writing the setting
/// alone would change nothing until the next launch, while the model list showed
/// the new model as selected — so the operator would be told they had switched,
/// and be running the old model for the rest of the service. Choosing a model and
/// loading it are one action or the promise is false (see rule 15).
#[tauri::command]
fn select_stt_model(app: tauri::AppHandle, filename: Option<String>) -> error::Result<bool> {
    {
        let db = app.state::<Db>();
        let conn = db.0.lock()?;
        match filename.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(name) => db::set_setting(&conn, STT_MODEL_KEY, name)?,
            None => db::set_setting(&conn, STT_MODEL_KEY, "")?,
        }
    }
    load_stt_model(app)
}

/// Bring speech recognition up after a model has just been installed, without a
/// restart. Re-applies the active voice profile so language + decoder bias are
/// live from the first word.
#[tauri::command]
fn load_stt_model(app: tauri::AppHandle) -> error::Result<bool> {
    let engine = build_stt(&app);
    let loaded = engine.is_some();
    {
        let stt_state = app.state::<Stt>();
        let mut slot = stt_state.0.lock()?;
        *slot = engine;
    }
    if loaded {
        let profile = {
            let db = app.state::<Db>();
            let conn = db.0.lock()?;
            db::active_voice_profile(&conn).ok().flatten()
        };
        if let Some(p) = profile {
            let stt_state = app.state::<Stt>();
            let slot = stt_state.0.lock()?;
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
async fn download_model(app: tauri::AppHandle, id: String) -> error::Result<()> {
    models::download(app, id).await.map_err(Into::into)
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
fn list_voice_profiles(db: tauri::State<'_, Db>) -> error::Result<Vec<db::VoiceProfile>> {
    let conn = db.0.lock()?;
    db::list_voice_profiles(&conn).map_err(Into::into)
}

/// The currently active profile.
#[tauri::command]
fn active_voice_profile(db: tauri::State<'_, Db>) -> error::Result<Option<db::VoiceProfile>> {
    let conn = db.0.lock()?;
    db::active_voice_profile(&conn).map_err(Into::into)
}

/// Create a new profile (default calibration); returns its id.
#[tauri::command]
fn create_voice_profile(
    db: tauri::State<'_, Db>,
    name: String,
    language: Option<String>,
) -> error::Result<i64> {
    let conn = db.0.lock()?;
    db::create_voice_profile(&conn, &name, language.as_deref()).map_err(Into::into)
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
) -> error::Result<db::VoiceProfile> {
    let is_active = {
        let conn = db.0.lock()?;

        // Did the sensitivity dial actually move? Compare against what's stored.
        let stored = db::list_voice_profiles(&conn)?
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

        db::update_voice_profile(&conn, &profile)?;
        db::save_profile_thresholds(&conn, profile.id, profile.auto_fire, profile.suggest)?;
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
) -> error::Result<db::VoiceProfile> {
    let profile = {
        let conn = db.0.lock()?;
        db::set_active_profile(&conn, id)?;
        db::active_voice_profile(&conn)?
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
) -> error::Result<db::VoiceProfile> {
    let profile = {
        let conn = db.0.lock()?;
        db::delete_voice_profile(&conn, id)?;
        db::active_voice_profile(&conn)?
            .ok_or_else(|| "no active profile after delete".to_string())?
    };
    apply_profile(&stt, &routing, &profile)?;
    Ok(profile)
}

/// Operator manual override: fire a free-text reference now, bypassing the gate.
/// First-class control (CLAUDE.md) — parses the reference, resolves it, and
/// emits a `detection://match` with status "manual".
#[tauri::command]
fn manual_fire<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    reference: String,
    stage_note: Option<String>,
    template_id: Option<i64>,
) -> error::Result<()> {
    let m = detection::detect_direct(&reference)
        .into_iter()
        .next()
        .ok_or_else(|| format!("could not parse a reference from \"{reference}\""))?;

    // Stage the passage span so a later "next" walks "Psalm 23" / "John 3:16-18"
    // rather than stopping dead after the anchor verse. Short lock, released
    // before fire_manual takes its own — sequential, never nested.
    let end = {
        let conn = db.0.lock()?;
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
        template_id,
    ) {
        // Parsed fine, but that verse doesn't exist (e.g. "John 3:99"). Say so.
        // This used to broadcast an EMPTY verse instead — blanking the wall
        // mid-service and leaving the operator with no idea why.
        return Err(error::Error::not_found(format!(
            "{key} isn't in the Bible text — check the reference"
        )));
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
) -> error::Result<String> {
    let name = name.unwrap_or_else(|| "Output".into());
    let label = {
        let mut n = outputs.0.lock()?;
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
) -> error::Result<String> {
    let channel = {
        let conn = db.0.lock()?;
        db::list_output_channels(&conn)?
            .into_iter()
            .find(|c| c.id == channel_id)
            .ok_or_else(|| format!("channel {channel_id} not found"))?
    };
    let template_id = channel.template_id.unwrap_or(1);
    let monitor_index = channel.display_target.as_deref().and_then(parse_display);
    // Deterministic, so the window can be traced back to this channel — that is
    // what makes the channel's "online" light real. It also makes
    // `open_native_window`'s already-open check a duplicate guard: the counter
    // used to mint a fresh label each time, so opening one channel twice put two
    // fullscreen windows on the same projector.
    let label = channels::channel_label(channel_id);
    channels::open_native_window(&app, &label, template_id, &channel.name, monitor_index)?;
    let _ = outputs; // labels no longer come from the counter
    Ok(label)
}

/// Auto-open the physical output windows on launch, so HDMI/projector screens
/// come back BY THEMSELVES after a restart, an update or a rebuild — the operator
/// never re-opens them or re-assigns displays. The channel config (template +
/// `display_target`) lives in SQLite and survives every rebuild, so this just
/// re-materialises the windows from it.
///
/// SAFE BY CONSTRUCTION: a window is opened ONLY onto a display that is actually
/// connected AND is NOT the primary (operator) monitor — auto-opening a fullscreen
/// output on the console's own screen would cover the very UI the operator needs.
/// On a single-monitor desk nothing auto-opens; plug in the projector and its
/// screen restores itself. Already-open windows are skipped (duplicate guard in
/// `open_native_window`). Best-effort: one screen failing never blocks the others.
#[tauri::command]
fn auto_open_outputs(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
) -> error::Result<Vec<String>> {
    let monitors = channels::list_monitors(&app);
    let list = {
        let conn = db.0.lock()?;
        db::list_output_channels(&conn)?
    };
    let mut opened = Vec::new();
    for c in list {
        if c.render_target != "native_window" {
            continue; // OBS/kiosk auto-reconnect over the WS; nothing to open here
        }
        let Some(idx) = c.display_target.as_deref().and_then(parse_display) else {
            continue; // no display assigned → not a fixed physical screen
        };
        let Some(m) = monitors.iter().find(|m| m.index == idx) else {
            continue; // that display isn't connected right now
        };
        if m.primary {
            continue; // never cover the operator's console
        }
        let tid = c.template_id.unwrap_or(1);
        let label = channels::channel_label(c.id);
        if channels::open_native_window(&app, &label, tid, &c.name, Some(idx)).is_ok() {
            opened.push(label);
        }
    }
    Ok(opened)
}

/// A channel's `display_target` as a monitor index.
///
/// Accepts a bare index ("1") and the "Display 1" form the seed writes. The seed
/// has always written `display_target = "Display 1"` for the Main screen while
/// this parsed with a plain `parse::<usize>()`, so it silently returned `None`
/// and the channel opened on the PRIMARY display — ignoring the display it was
/// configured with, with nothing reported. On a two-screen setup that means the
/// congregation's verse appears on the operator's monitor.
///
/// "Display 1" is 1-BASED (it is a human label); a bare index is 0-based, matching
/// `MonitorInfo.index` and what `set_channel_display` writes.
fn parse_display(s: &str) -> Option<usize> {
    let s = s.trim();
    if let Ok(n) = s.parse::<usize>() {
        return Some(n);
    }
    let rest = s
        .strip_prefix("Display ")
        .or_else(|| s.strip_prefix("display "))?;
    rest.trim()
        .parse::<usize>()
        .ok()
        .map(|n| n.saturating_sub(1))
}

/// What is actually live on each output channel, right now.
///
/// Computed from the running app, never read from `output_channels.status` — that
/// column is written once at insert and never updated, so it has always said
/// `offline` for every channel, including one filling a projector.
///
/// `clients` is only meaningful for a networked channel, and is a COUNT, not a
/// list: Relay records no address, identity, or connect time for a kiosk client,
/// so the count is the most that can honestly be reported. `detail` is the one
/// line the UI shows; it never claims more than the two facts above.
#[derive(serde::Serialize)]
struct ChannelLiveness {
    id: i64,
    online: bool,
    clients: usize,
    detail: String,
    /// False for a target Relay cannot drive at all (NDI is parked), so the UI can
    /// say "unavailable" rather than "offline" — a different claim.
    supported: bool,
}

/// Live status for every channel. Polled by the Channels screen.
#[tauri::command]
fn channel_status(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    kiosk: tauri::State<'_, channels::KioskHub>,
) -> error::Result<Vec<ChannelLiveness>> {
    let list = {
        let conn = db.0.lock()?;
        db::list_output_channels(&conn)?
    };
    let open = channels::open_channel_ids(&app);
    let clients = kiosk.clients_handle();

    Ok(list
        .into_iter()
        .map(|c| match c.render_target.as_str() {
            "native_window" => {
                let online = open.contains(&c.id);
                ChannelLiveness {
                    id: c.id,
                    online,
                    clients: 0,
                    detail: if online {
                        "Output window open".into()
                    } else {
                        "No output window open".into()
                    },
                    supported: true,
                }
            }
            "network_client" => {
                // A networked output is SERVED CONTINUOUSLY: its URL responds and
                // receives the live program the whole time the app runs, whether or
                // not a browser is pulling it right now. So its liveness is "is it
                // serving" (always true here), and the viewer count is reported
                // SEPARATELY in the detail — not folded into the live/idle badge.
                //
                // The old rule (`online = clients > 0`) read IDLE for a perfectly
                // live output the instant OBS momentarily dropped or hid its source,
                // which is exactly the "some screens say not-live but OBS shows them
                // all live" confusion. A viewer count of 0 means "nobody watching
                // yet", not "the output is off".
                let n = c.template_id.map(|t| clients.count(t)).unwrap_or(0);
                ChannelLiveness {
                    id: c.id,
                    online: true,
                    clients: n,
                    detail: match n {
                        0 => "Serving · no viewer connected yet".into(),
                        1 => "Serving · 1 viewer".into(),
                        n => format!("Serving · {n} viewers"),
                    },
                    supported: true,
                }
            }
            // NDI is parked, not broken — `open_ndi_output` says so too.
            "ndi_encode" => ChannelLiveness {
                id: c.id,
                online: false,
                clients: 0,
                detail: "NDI output is not available in this build".into(),
                supported: false,
            },
            other => ChannelLiveness {
                id: c.id,
                online: false,
                clients: 0,
                detail: format!("Unknown render target '{other}'"),
                supported: false,
            },
        })
        .collect())
}

/// Close a channel's native output window, if it has one open.
#[tauri::command]
fn close_channel_output(app: tauri::AppHandle, channel_id: i64) -> error::Result<()> {
    channels::close_window(&app, &channels::channel_label(channel_id)).map_err(Into::into)
}

/// Assign a physical display to a channel (HDMI). `display` is the monitor index
/// as a string, or null to use the primary display.
#[tauri::command]
fn set_channel_display(
    db: tauri::State<'_, Db>,
    id: i64,
    display: Option<String>,
) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::set_channel_display(&conn, id, display.as_deref()).map_err(Into::into)
}

/// Add an output channel. Returns its id.
#[tauri::command]
fn add_channel(
    db: tauri::State<'_, Db>,
    name: String,
    render_target: Option<String>,
    template_id: Option<i64>,
) -> error::Result<i64> {
    let target = render_target.unwrap_or_else(|| "native_window".into());
    if !matches!(
        target.as_str(),
        "native_window" | "ndi_encode" | "network_client"
    ) {
        return Err(error::Error::refused(format!(
            "invalid render target: {target}"
        )));
    }
    let conn = db.0.lock()?;
    db::add_channel(&conn, name.trim(), &target, template_id.unwrap_or(1)).map_err(Into::into)
}

/// Delete an output channel.
#[tauri::command]
fn delete_channel(db: tauri::State<'_, Db>, id: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::delete_channel(&conn, id).map_err(Into::into)
}

/// All output templates (Templates tab, Channels tab).
#[tauri::command]
fn list_templates(db: tauri::State<'_, Db>) -> error::Result<Vec<db::Template>> {
    let conn = db.0.lock()?;
    db::list_templates(&conn).map_err(Into::into)
}

/// Create a new (blank-styled) template. Returns its id.
#[tauri::command]
fn create_template(db: tauri::State<'_, Db>, name: Option<String>) -> error::Result<i64> {
    let conn = db.0.lock()?;
    let name = name.unwrap_or_else(|| "New template".into());
    db::create_template(&conn, name.trim()).map_err(Into::into)
}

/// Delete a template (unassigns it from any channel first).
#[tauri::command]
fn delete_template(db: tauri::State<'_, Db>, id: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::delete_template(&conn, id).map_err(Into::into)
}

/// A single template by id (fetched by each output window on load).
#[tauri::command]
fn get_template(db: tauri::State<'_, Db>, id: i64) -> error::Result<Option<db::Template>> {
    let conn = db.0.lock()?;
    db::get_template(&conn, id).map_err(Into::into)
}

/// Save a template (insert or update). Broadcasts `template://updated` so any
/// open output window on that template re-renders live. Returns the id.
#[tauri::command]
fn save_template(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    template: db::Template,
) -> error::Result<i64> {
    let id = {
        let conn = db.0.lock()?;
        db::upsert_template(&conn, &template)?
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

/// Labels of currently-open output windows.
#[tauri::command]
fn list_output_windows(app: tauri::AppHandle) -> Vec<String> {
    channels::list_open(&app)
}

/// All configured output channels (Channels tab).
#[tauri::command]
fn list_output_channels(db: tauri::State<'_, Db>) -> error::Result<Vec<db::OutputChannel>> {
    let conn = db.0.lock()?;
    db::list_output_channels(&conn).map_err(Into::into)
}

/// Assign a template to a channel — outputs are freely assignable — and push the
/// change LIVE to that channel's outputs so switching a screen's template needs no
/// reload and no URL change. Native windows get a `channel://retemplate` event; kiosk
/// / OBS clients get a `channel_template` WS message they filter by their own channel.
#[tauri::command]
fn set_channel_template(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    kiosk: tauri::State<'_, channels::KioskHub>,
    id: i64,
    template_id: i64,
) -> error::Result<()> {
    // DB write + resolve the new template JSON under one lock, then release before
    // emitting (never hold a lock across emit — CLAUDE.md rule #2).
    let tjson = {
        let conn = db.0.lock()?;
        db::set_channel_template(&conn, id, template_id)?;
        db::get_template(&conn, template_id)?.and_then(|t| serde_json::to_string(&t).ok())
    };
    if let Some(j) = tjson {
        if let Ok(tpl) = serde_json::from_str::<serde_json::Value>(&j) {
            let _ = app.emit(
                "channel://retemplate",
                serde_json::json!({ "channel": id, "template": tpl }),
            );
        }
        kiosk.publish(format!(
            r#"{{"kind":"channel_template","channel":{id},"template":{j}}}"#
        ));
        // Keep the hub's per-template cache current so a fresh kiosk connect on this
        // template id renders the up-to-date template too.
        kiosk.cache_template(template_id, &j);
    }
    Ok(())
}

/// Operator "Clear all screens" / blackout — blank every output channel (D4).
/// Instant, always available. Same effect the spoken "clear"/"blackout" reaches.
///
/// This RETURNS A RESULT, and the console must not claim the screens are clear
/// unless it resolves Ok. It used to return `()`, which made a failed clear look
/// exactly like a successful one — and the operator was shown "Screens cleared"
/// while the verse was still in front of the congregation.
///
/// The debounce is forgotten and the cue recorded ONLY on success: if the screens
/// did not actually clear, then the verse IS still showing, and "forget what is on
/// screen" would be a lie told to the router as well as to the operator.
#[tauri::command]
fn clear_screens<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> error::Result<()> {
    channels::clear(&app)?;
    forget_debounce(&app);
    persist_cue(&app, "clear_screens", None);
    Ok(())
}

/// Blackout every output (opaque black). The next fire/clear cancels it.
/// Returns a Result for the same reason `clear_screens` does — see above.
#[tauri::command]
fn blackout<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> error::Result<()> {
    channels::black(&app)?;
    forget_debounce(&app);
    persist_cue(&app, "blackout", None);
    Ok(())
}

/// Clear the wall from a path that has nobody to return an error to — the STT
/// thread acting on a spoken "clear the screen", and the exit from rehearsal.
///
/// Those are panic controls too, and they used to `let _ =` the clear. A spoken
/// clear that failed was as silent as a keyed one that failed. There is no caller
/// to hand a Result to here, so the failure is pushed to the operator instead:
/// `output://panic_failed` raises the same banner the buttons and keys raise.
fn clear_or_report<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    match channels::clear(app) {
        Ok(()) => {
            forget_debounce(app);
            persist_cue(app, "clear_screens", None);
        }
        Err(e) => {
            eprintln!("clear failed: {e}");
            let _ = app.emit(
                "output://panic_failed",
                format!("Clear screens failed: {e}"),
            );
        }
    }
}

/// The screens are empty, so nothing is "already showing" any more — drop the
/// repeat-cooldown memory. Otherwise, clearing the screen and having the preacher
/// immediately re-reference the same verse would leave it blank for the rest of
/// the cooldown: the debounce would suppress the one fire the operator wants.
fn forget_debounce<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Ok(mut r) = app.state::<Routing>().0.lock() {
        r.forget_last_fire();
    }
}

/// Push the "up next" preview to the stage/confidence monitor. None clears it.
#[tauri::command]
fn set_stage_next<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    label: Option<String>,
    text: Option<String>,
) {
    channels::stage_next(&app, label, text);
}

/// D5: push an emergency announcement over whatever is currently shown, on every
/// output channel. Reuses the shared content broadcast (no per-channel special-
/// casing) so it renders through the same template engine as any slide.
#[tauri::command]
fn push_announcement(app: tauri::AppHandle, message: String) -> error::Result<()> {
    let message = message.trim().to_string();
    if message.is_empty() {
        return Err(error::Error::refused("empty announcement"));
    }
    broadcast_with_clock(
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

/// Manual next/previous verse (console buttons, and the `→`/`←` transport keys) —
/// same path as the spoken "next"/"back" command.
///
/// Returns what it DID (see `NavResult`). It used to return `()`, so the single most
/// pressed key in a live service had no way to tell the operator that it had done
/// nothing, or why.
#[tauri::command]
fn nav<R: tauri::Runtime>(app: tauri::AppHandle<R>, direction: String) -> error::Result<NavResult> {
    let dir = if direction == "previous" || direction == "back" {
        detection::NavCommand::Previous
    } else {
        detection::NavCommand::Next
    };
    handle_nav(&app, dir)
}

/// Start (or resume) recording a service. If one is already active it's reused
/// so pause/resume of capture doesn't fragment history. Returns the service id.
#[tauri::command]
fn start_service(
    session: tauri::State<'_, Session>,
    db: tauri::State<'_, Db>,
    rehearsal: tauri::State<'_, channels::Rehearsal>,
    title: String,
    date: String,
) -> error::Result<i64> {
    // A rehearsal is not a service and must never be written into the church's
    // history as one. They are mutually exclusive, and this is refused loudly
    // rather than quietly recorded — a practice run filed under last Sunday is a
    // record nobody can trust afterwards.
    if rehearsal.on() {
        return Err(
            "Relay is in rehearsal mode. Turn rehearsal off to record a real service.".into(),
        );
    }
    // db before session (consistent global lock order — see persist_transcript).
    let conn = db.0.lock()?;
    let mut sess = session.0.lock()?;
    if let Some(st) = sess.as_ref() {
        return Ok(st.id);
    }
    let id = db::create_service(&conn, &date, &title)?;
    // Planned length (minutes) → ms, captured once so a later settings change does
    // not retro-move this service's target. Absent/unparseable = no target.
    let target_ms = db::get_setting(&conn, "service.target_minutes")
        .ok()
        .flatten()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .filter(|m| *m > 0)
        .map(|m| m * 60_000)
        .unwrap_or(0);
    *sess = Some(SessionState {
        id,
        started: Instant::now(),
        started_at_ms: now_epoch_ms(),
        target_ms,
        last_transcript: None,
    });
    Ok(id)
}

/// Stop recording the current service (history is kept).
#[tauri::command]
fn end_service(session: tauri::State<'_, Session>) -> error::Result<()> {
    *session.0.lock()? = None;
    Ok(())
}

/// All services for the Library list, newest first.
#[tauri::command]
fn list_services(db: tauri::State<'_, Db>) -> error::Result<Vec<db::ServiceSummary>> {
    let conn = db.0.lock()?;
    db::list_services(&conn).map_err(Into::into)
}

/// Full transcript + fired detections for one service (Library detail view).
#[tauri::command]
fn service_detail(db: tauri::State<'_, Db>, id: i64) -> error::Result<ServiceDetail> {
    let conn = db.0.lock()?;
    Ok(ServiceDetail {
        transcripts: db::service_transcripts(&conn, id)?,
        detections: db::service_detections(&conn, id)?,
    })
}

/// Export a service as a Markdown file (transcript + detected verses) to the
/// user's Downloads folder. Returns the written path. Uses std::fs — no fs
/// plugin needed; nothing leaves the device.
#[tauri::command]
fn export_service(db: tauri::State<'_, Db>, id: i64) -> error::Result<String> {
    let (summary, transcripts, detections) = {
        let conn = db.0.lock()?;
        let summary = db::list_services(&conn)?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("service {id} not found"))?;
        let transcripts = db::service_transcripts(&conn, id)?;
        let detections = db::service_detections(&conn, id)?;
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
    // resolved per-OS by db::, which is the ONLY module allowed to read HOME/APPDATA —
    // exporting a service used to demand $HOME and hardcode a macOS path, so it failed
    // outright on Windows with "no HOME".
    let dir = match db::downloads_dir() {
        Some(d) => d,
        None => {
            let d = db::app_data_dir().join("exports");
            std::fs::create_dir_all(&d)?;
            d
        }
    };
    let path = dir.join(filename);
    std::fs::write(&path, md)?;
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

#[cfg(test)]
mod suggestion_tests {
    use super::*;
    use detection::VerseRef;

    fn hit(book: &str, verse: i64, score: f32) -> (VerseRef, f32, Vec<String>) {
        (
            VerseRef {
                book: book.into(),
                chapter: 1,
                verse,
            },
            score,
            vec!["why".into()],
        )
    }

    /// A clear winner stays a list of ONE. Widening it every time would spend a
    /// volunteer's attention on alternatives Relay is not actually unsure about.
    #[test]
    fn a_runaway_best_hit_is_offered_alone() {
        let kept = worth_suggesting(vec![
            hit("Mark", 1, 0.90),
            hit("Luke", 2, 0.40),
            hit("John", 3, 0.35),
        ]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].0.book, "Mark");
    }

    /// Scores this close mean Relay cannot tell them apart — so the operator,
    /// who can, is shown all of them rather than one picked by a hair.
    #[test]
    fn near_ties_are_all_offered() {
        let kept = worth_suggesting(vec![
            hit("Mark", 1, 0.62),
            hit("Matthew", 2, 0.60),
            hit("Luke", 3, 0.58),
        ]);
        assert_eq!(kept.len(), 3);
    }

    /// The absolute floor still rules: noise never reaches the operator, however
    /// close it sits to an equally weak best hit.
    #[test]
    fn nothing_below_the_absolute_floor_is_ever_offered() {
        let kept = worth_suggesting(vec![hit("Mark", 1, 0.20), hit("Luke", 2, 0.19)]);
        assert!(kept.is_empty());
    }

    #[test]
    fn no_hits_is_not_a_panic() {
        assert!(worth_suggesting(vec![]).is_empty());
    }
}

#[cfg(test)]
mod display_target_tests {
    use super::parse_display;

    /// A channel's assigned display must actually be honoured.
    ///
    /// `seed_channels` wrote `display_target = "Display 1"` while this parsed with
    /// a plain `parse::<usize>()`, which returned `None` — so the seeded main
    /// screen silently opened on the PRIMARY display instead of the one it was
    /// configured with, reporting nothing. On a two-screen booth that puts the
    /// congregation's verse on the operator's monitor.
    #[test]
    fn a_human_readable_display_target_is_not_silently_ignored() {
        assert_eq!(
            parse_display("Display 1"),
            Some(0),
            "1-based label → 0-based index"
        );
        assert_eq!(parse_display("Display 2"), Some(1));
        assert_eq!(parse_display("display 3"), Some(2));
    }

    #[test]
    fn a_bare_index_is_still_a_zero_based_index() {
        // What `set_channel_display` writes, and what MonitorInfo.index means.
        assert_eq!(parse_display("0"), Some(0));
        assert_eq!(parse_display("1"), Some(1));
        assert_eq!(parse_display(" 2 "), Some(2));
    }

    #[test]
    fn an_unreadable_target_falls_back_rather_than_guessing() {
        // None → primary display, which is the safe default.
        assert_eq!(parse_display(""), None);
        assert_eq!(parse_display("HDMI-A-1"), None);
        assert_eq!(parse_display("Display"), None);
    }

    #[test]
    fn display_zero_does_not_underflow_to_a_huge_index() {
        // "Display 0" is not a form anything writes, but saturating_sub must not
        // turn it into usize::MAX and index past the monitor list.
        assert_eq!(parse_display("Display 0"), Some(0));
    }
}
