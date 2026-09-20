// Relay — Tauri entry point.
//
// Boots the window with the Svelte frontend and opens the local database.
// Real pipeline commands (audio, STT, detection, routing, channels) get wired
// here as each module below is built out — don't front-load functionality.

mod audio;
mod channels;
mod db;
mod detection;
mod diagnostics;
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
mod latency;
mod models;
mod pipeline;
mod prodiscover;
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
mod search;
mod servicelock;
mod songs;
mod stt;
mod sysprobe;
mod telemetry;
mod timers;
mod updates;
mod wake;

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
        .manage(Detecting(AtomicBool::new(true)))
        .manage(channels::Rehearsal::default())
        .manage(channels::CountdownWarnDefault::default())
        .manage(channels::MediaTransport::default())
        .manage(channels::WallState::default())
        .manage(channels::LiveContent::default())
        .manage(timers::TimerRegistry::default())
        .manage(channels::OutputHealth::default())
        .manage(channels::ScreensDown::default())
        .manage(servicelock::ServiceLock::default())
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
            let kiosk_default_tpl = kiosk.default_template_handle();
            let kiosk_roles = kiosk.channel_roles_handle();
            let kiosk_kind_looks = kiosk.channel_looks_handle();
            let kiosk_screen_tpls = kiosk.channel_templates_handle();
            let kiosk_shows = kiosk.channel_shows_handle();
            let kiosk_last = kiosk.last_screen_handle();
            let kiosk_last_by_ch = kiosk.last_screen_by_channel_handle();
            let kiosk_last_x = kiosk.last_transition_handle();
            let kiosk_last_t = kiosk.last_timers_handle();
            let kiosk_last_bg = kiosk.last_background_handle();
            let kiosk_last_stage_media = kiosk.last_stage_media_handle();
            let kiosk_last_transport = kiosk.last_media_transport_handle();
            let kiosk_down = kiosk.screens_down_handle();
            let kiosk_looks = kiosk.look_ids_handle();
            // The configured default, warmed before any client can connect — a
            // screen that joins during launch must not be told the default is
            // `null` and then corrected.
            {
                let db = app.state::<Db>();
                let dj =
                    db.0.lock()
                        .ok()
                        .and_then(|conn| {
                            db::get_setting(&conn, "default_template_id")
                                .ok()
                                .flatten()
                                .and_then(|s| s.parse::<i64>().ok())
                                .and_then(|id| db::get_template(&conn, id).ok().flatten())
                        })
                        .and_then(|t| serde_json::to_string(&t).ok())
                        .unwrap_or_else(|| "null".into());
                kiosk.cache_default_template(&dj);
            }
            // …and what each screen is FOR, on the same argument: a page that
            // connects during launch must not be told it has no role and then
            // corrected, because between the two it would refuse a stage message
            // meant for it.
            {
                let db = app.state::<Db>();
                let rj =
                    db.0.lock()
                        .ok()
                        .and_then(|conn| db::channel_roles_json(&conn).ok())
                        .unwrap_or_else(|| "{}".into());
                kiosk.cache_channel_roles(&rj);
            }
            // …AND WHAT EACH SCREEN WEARS FOR EACH KIND (DECISIONS §97), on the
            // identical argument. A browser source that connects during launch
            // must not be told it has no per-kind look and then corrected: between
            // the two it would resolve the next fire against its blanket template,
            // and the next fire during launch is the countdown a congregation is
            // already watching.
            //
            // An unreadable database leaves `{}`, which is "no screen has a
            // per-kind look" — every kind falls through to the screen's own
            // template, which is the behaviour before this existed and the right
            // answer when nothing can be read.
            {
                let db = app.state::<Db>();
                let lj =
                    db.0.lock()
                        .ok()
                        .and_then(|conn| db::channel_looks_json(&conn).ok())
                        .unwrap_or_else(|| "{}".into());
                kiosk.cache_channel_looks(&lj);
            }
            // …AND WHICH KINDS EACH SCREEN SHOWS AT ALL (DECISIONS §98), on the
            // identical argument once more. An unreadable database leaves `{}` —
            // no screen has an opinion, so every screen follows its template,
            // which is where this decision lived before the column existed. The
            // other direction, an empty LIST arrived at by accident, is a
            // congregation screen that paints nothing.
            {
                let db = app.state::<Db>();
                let sj =
                    db.0.lock()
                        .ok()
                        .and_then(|conn| db::channel_shows_json(&conn).ok())
                        .unwrap_or_else(|| "{}".into());
                kiosk.cache_channel_shows(&sj);
            }
            // …AND WHAT EACH SCREEN WEARS FOR EVERYTHING ELSE — its own template.
            //
            // The most consequential of the four warms, and the one that was
            // missing entirely. A browser source opened from `Copy URL` is
            // CHANNEL-keyed and sends `template_id: null`, so until this existed
            // the hub had nothing to answer it with and every such screen resolved
            // against a null template: the content look, or the configured
            // default, on every screen in the building, whatever the operator had
            // assigned. It looked correct in testing because a screen that happens
            // to be connected when the operator reassigns a template does receive
            // the broadcast; it was wrong on every reconnect and every cold start.
            //
            // `null` is stored for a screen that follows the content look, rather
            // than the row being skipped: that is an ANSWER, and a page that cannot
            // tell it from silence keeps whatever its URL gave it.
            {
                let db = app.state::<Db>();
                let rows = db
                    .0
                    .lock()
                    .ok()
                    .and_then(|conn| {
                        db::list_output_channels(&conn).ok().map(|cs| {
                            cs.into_iter()
                                .map(|c| {
                                    let j = c
                                        .template_id
                                        .and_then(|id| db::get_template(&conn, id).ok().flatten())
                                        .and_then(|t| serde_json::to_string(&t).ok())
                                        .unwrap_or_else(|| "null".into());
                                    (c.id, j)
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .unwrap_or_default();
                for (id, j) in rows {
                    kiosk.cache_channel_template(id, &j);
                }
            }
            // WARM THE CONFIGURED COUNTDOWN WARNING WINDOW, for the same reason
            // the default template is warmed one block up: the first screen to
            // connect must not be told the shipped minute by a machine that has
            // been set to something else for a year (RG-149(c)). An unreadable or
            // absent row leaves the mirror at zero, which reads as ABSENT and puts
            // every screen on the shipped minute — the behaviour before this
            // existed, which is the right answer when nobody has chosen one.
            {
                let db = app.state::<Db>();
                let warn =
                    db.0.lock()
                        .ok()
                        .and_then(|conn| db::get_setting(&conn, "countdown.warn_ms").ok().flatten())
                        .and_then(|s| s.trim().parse::<i64>().ok());
                app.state::<channels::CountdownWarnDefault>().set(warn);
            }
            // …AND WHICH TEMPLATES THE CONTENT LOOKS NAME, on the same argument
            // once more. A content look reaches a screen as an ID and no JSON
            // (`cue_or_content_tpl` records why, in megabytes), so a browser
            // source can only resolve it if the hub hands it the bytes on
            // connect — and it has to hand them to the FIRST client too, not
            // only to one that reconnects after the operator happens to touch
            // the map.
            {
                let db = app.state::<Db>();
                let ids =
                    db.0.lock()
                        .ok()
                        .map(|conn| resolvable_look_ids(&conn))
                        .unwrap_or_default();
                kiosk.cache_look_ids(&ids);
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
                kiosk_default_tpl,
                kiosk_roles,
                kiosk_kind_looks,
                kiosk_screen_tpls,
                kiosk_shows,
                kiosk_last,
                kiosk_last_by_ch,
                kiosk_last_x,
                kiosk_last_t,
                kiosk_last_bg,
                kiosk_last_stage_media,
                kiosk_last_transport,
                kiosk_down,
                kiosk_looks,
                app.state::<channels::OutputHealth>().inner().clone(),
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
            latency_report,
            latency_mark,
            latency_reset,
            latency_set_enabled,
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
            set_plan_timer,
            set_plan_channels,
            set_plan_template,
            list_songs,
            search_songs,
            get_song,
            save_song,
            delete_song,
            start_countdown,
            adjust_countdown,
            start_timer,
            adjust_timer,
            reset_timer,
            list_stage_layouts,
            set_channel_stage_layout,
            upsert_stage_layout,
            delete_stage_layout,
            stop_timer,
            list_timers,
            show_timer,
            list_arrangements,
            save_arrangement,
            delete_arrangement,
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
            demo_status,
            load_demo_content,
            remove_demo_content,
            fire_content,
            fire_media,
            show_background,
            send_stage_media,
            set_media_transport,
            find_propresenter,
            get_content_templates,
            set_content_template,
            get_setting,
            set_setting,
            set_live_transition,
            live_transition,
            live_background,
            data_health,
            list_books,
            chapter_verses,
            system_hardware,
            probe_integrations,
            migration_status,
            list_audio_devices,
            local_ip,
            network_addresses,
            start_capture,
            stop_capture,
            stt_status,
            confirm_detection,
            dismiss_detection,
            get_thresholds,
            get_sensitivity,
            set_sensitivity,
            get_rehearsal,
            set_rehearsal,
            get_crash_reporting,
            set_crash_reporting,
            list_models,
            download_model,
            find_model_files,
            install_model_file,
            cancel_model_download,
            load_stt_model,
            select_stt_model,
            manual_fire,
            list_output_channels,
            channel_status,
            close_channel_output,
            output_beat,
            service_timeline,
            service_perf,
            perf_history,
            export_diagnostics,
            language_report,
            list_environments,
            save_environment,
            use_environment,
            delete_environment,
            update_preflight,
            update_begin,
            update_verify,
            update_accept,
            update_restore,
            service_lock,
            set_service_lock,
            set_channel_template,
            list_channel_looks,
            set_channel_look,
            set_channel_shows,
            rename_channel,
            clear_screen,
            blackout_screen,
            restore_screen,
            set_default_template,
            send_stage_alert,
            list_monitors,
            open_channel_output,
            auto_open_outputs,
            set_channel_display,
            set_channel_role,
            add_channel,
            delete_channel,
            clear_screens,
            blackout,
            set_stage_next,
            set_detection_enabled,
            get_detection_enabled,
            nav,
            start_service,
            end_service,
            list_services,
            delete_service,
            service_detail,
            export_service,
            list_templates,
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
    // WHICH SCREENS (RG-161). `None` is every screen, which every path but a
    // planned cue passes: a detected verse and a manual fire have nowhere
    // anybody could have said otherwise.
    channels: Option<Vec<i64>>,
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
        channels,
        stage_note,
        next_reference: None,
        next_text: None,
        template_id,
        template_json,
        template_pinned,
        matched_text,
        // RG-135. Filled in by the caller, for the same reason `trace_id` is: this
        // builder has the verse and not the WINDOW, and the question is about what
        // was said around the reference rather than about the reference itself.
        named_translation_missing: None,
        // Filled in by the caller when a decode pass is behind this fire.
        trace_id: None,
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
/// THE ONE DOOR CONTENT LEAVES BY — and now the one place it is checked first.
///
/// `channels::broadcast_content` has exactly one caller, which is this, so a
/// pre-air check here covers every path: the AI's, the operator's manual box, a
/// spoken next/back, a plan cue, a media slide, the emergency announcement and
/// the countdown. That is deliberate. A validator added at five call sites is a
/// validator that will be missing from the sixth — this repository has produced
/// four separate bugs of exactly that shape.
///
/// Returns `Err` when the payload would put something broken in front of a
/// congregation. It refuses only the two things that are unambiguously broken and
/// silently so; everything else it lets through and reports elsewhere. See
/// `pipeline::preflight` for what it deliberately does NOT check.
fn broadcast_with_clock<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    mut content: channels::OutputContent,
) -> error::Result<()> {
    // A NEW THING ON THE SCREENS IS A CLIP AT ITS BEGINNING, PLAYING.
    //
    // The hub empties the retained transport frame on any content, clear or black
    // (`media_transport_retention`); this is the same decision for the state the
    // commands read back from, in the same place the content leaves by (rule 36).
    // Without it the next video a church put up would arrive already held, because
    // somebody paused a different one twenty minutes earlier, and nothing in the
    // product would say why.
    //
    // The epoch is deliberately NOT wound back — it is a monotonic instruction
    // counter, and a replay number a screen has already seen is a replay that does
    // nothing.
    handle.state::<channels::MediaTransport>().reset();
    if let Err(bad) = pipeline::preflight(&content) {
        // The screens are left exactly as they were. Doing nothing quietly is the
        // failure being fixed, so this is said in three places: stdout for a
        // developer, the panic banner for the operator watching the console, and
        // the returned error for whichever caller can put it in front of them.
        eprintln!("preflight refused a broadcast: {bad:?}");
        let _ = handle.emit("output://panic_failed", bad.message());
        return Err(error::Error::refused(bad.message()));
    }
    // R2-D · A PASSAGE MUST NOT OUTLIVE THE CONTENT THAT REPLACED IT.
    //
    // `Context` was written only by scripture fires and cleared by nothing, so a
    // song, a notice, a picture or a countdown left the previous reading armed for
    // the rest of the service. `nav` would then walk a passage the congregation
    // stopped looking at twenty minutes earlier and report `Fired` — true of the
    // wall, false of the sermon — and the operator reaches that state by an ordinary
    // route: blackout clears `planOnAir` while leaving `$live` set, which flips the
    // transport from SLIDE to VERSE without them asking.
    //
    // Here, at the one door content leaves by, so every path is covered at once
    // (rule 36) and a new content kind added tomorrow is disarmed by construction.
    // The lock is taken and RELEASED before the broadcast below — never held across
    // an emit (rule 2).
    //
    // AND AN ABSENT KIND IS NOT SCRIPTURE. This read `is_some_and(|k| k !=
    // "scripture")`, which is **false for `None`**, so a payload built the way
    // `..Default::default()` invites — every field the caller cared about, `kind`
    // left unset — walked past the one place a passage is disarmed. Every caller in
    // this file sets it and nothing said so, and the failure is reached by
    // forgetting a field rather than by adding a content kind, which is the one
    // shape the choke point did not cover.
    //
    // It disarms rather than refusing, deliberately. A passage wrongly disarmed
    // makes `nav` answer `NoPassage`, a correct boundary the operator is told about
    // (rule 38b); a passage wrongly left armed walks a reading the congregation
    // stopped looking at and answers `Fired`. Refusing instead would blank a screen
    // over content that renders perfectly well, and `preflight` above refuses only
    // what is broken AND silent (rule 36). Pinned by
    // `e2e::r2_a_payload_that_forgot_its_kind_still_disarms_the_passage`.
    if content.kind.as_deref() != Some("scripture") {
        if let Some(ctx) = handle.try_state::<Context>() {
            if let Ok(mut c) = ctx.0.lock() {
                c.forget();
            }
        }
    }

    // THE CONFIGURED WARNING WINDOW, STAMPED AT THE ONE DOOR CONTENT LEAVES BY.
    //
    // `Settings → General → Countdown warning` is console state and the screens
    // that need it cannot read it: a browser source in OBS and a kiosk page on a
    // Pi have no Tauri bridge, so the setting moved the console and left every
    // congregation screen on the shipped minute (RG-149(c)). It is DELIVERED
    // instead, and delivered here rather than at `countdown_content`'s three
    // callers, for rule 36's reason: a content path added next year carries it by
    // construction, and there is no sixth call site to forget.
    //
    // Unconditional, like the service clock below it. Asking "is this a countdown?"
    // here would be a fourth reading of that question, and the one reading of it
    // lives in `pipeline::preflight`.
    //
    // It is never resolved against `countdown_warn_ms`. Ranking the chosen figure
    // against the configured one is `layers.js::countdownWarning`'s job, once — two
    // authorities on when a screen turns red is how they come to disagree.
    content.countdown_warn_default_ms = channels::countdown_warn_default(handle);

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
    Ok(())
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
    // WHICH SCREENS (RG-161). Only a planned cue has an answer; every other
    // operator path passes `None`, which is every screen.
    cue_channels: Option<Vec<i64>>,
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
            cue_channels,
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

    // A refused payload must not be followed by a `detection://match` saying it
    // went out — that is the console reporting a success it did not achieve, in a
    // new place (DECISIONS §20). Bail before the event.
    if broadcast_with_clock(handle, fire.output()).is_err() {
        return false;
    }
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
/// Order one window's candidates so the strongest is first, and drop the readings
/// that are just a less complete parse of another one in the same window.
///
/// **The chapter-only rule.** A `whole_chapter` candidate is what "chapter 9" alone
/// yields, and it resolves to verse 1. When the same window also names a specific
/// verse in that same book and chapter, the chapter-only reading is not a second
/// reference the preacher made — it is the first half of the one they did make.
/// Firing it puts verse 1 on the wall next to the verse they asked for. Removed
/// entirely rather than demoted: offering the operator "Genesis 12:1?" while they
/// are reading Genesis 12:5 is noise, not a decision.
///
/// A chapter-only reading with NO specific verse beside it survives untouched —
/// "turn to Psalm 23" is a real thing to say and verse 1 is the right answer.
///
/// Ordering is `pipeline::better`, the same comparison the per-reference dedup
/// uses, so "strongest" means one thing in this file rather than two.
fn rank_for_wall(mut cands: Vec<(String, Cand)>) -> Vec<(String, Cand)> {
    let specific: Vec<(String, i64)> = cands
        .iter()
        .filter(|(_, c)| !c.whole_chapter)
        .map(|(_, c)| (c.r.book.clone(), c.r.chapter))
        .collect();
    cands.retain(|(_, c)| {
        !c.whole_chapter
            || !specific
                .iter()
                .any(|(b, ch)| *b == c.r.book && *ch == c.r.chapter)
    });
    // Strongest first, and TIES MUST COMPARE EQUAL.
    //
    // R4-07, and the HashMap was only half of it. This used to ask
    // `pipeline::better` in both directions — but `better` is `>=`, "a is at least
    // as good as b", which is the right question for the dedup that keeps the
    // strongest evidence per verse and the WRONG one for a sort. On a tie it
    // answered yes both ways, so the comparator claimed `a < b` **and** `b < a`.
    // That violates the strict weak ordering `sort_by` requires, and a violated
    // comparator makes "the sort is stable, so equal candidates keep their input
    // order" a sentence with no meaning behind it: the result was simply
    // unspecified.
    //
    // Two `Direct` candidates at the same score is the ordinary case — "turn to
    // John 3:16 and Romans 8:28" — and rank 0 is the only one that may reach a
    // wall (DECISIONS §37). So this decided what a congregation saw.
    //
    // Ordered explicitly, descending, ties Equal, which is what makes the stable
    // sort keep the order the preacher spoke in.
    cands.sort_by(|(_, a), (_, b)| {
        (b.method.may_auto_fire(), b.conf)
            .partial_cmp(&(a.method.may_auto_fire(), a.conf))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    cands
}

#[cfg(test)]
mod rank_for_wall_tests {
    use super::*;
    use detection::{DetectionMethod, VerseRef};

    fn cand(book: &str, ch: i64, v: i64, conf: f32) -> (String, Cand) {
        let r = VerseRef {
            book: book.into(),
            chapter: ch,
            verse: v,
        };
        (
            Fire::key_for(&r),
            Cand::single(r, conf, DetectionMethod::Direct, None),
        )
    }

    /// R4-07 · WHICH VERSE THE CONGREGATION SEES MUST NOT DEPEND ON A HASH.
    ///
    /// A window may put at most one verse on a wall (DECISIONS §37): rank 0 fires,
    /// everything else is offered. So when two candidates tie under
    /// `pipeline::better` — the ordinary case for "turn to John 3:16 and Romans
    /// 8:28", two `Direct` matches at the same score — the tie IS the decision.
    ///
    /// The sort is stable, so the answer is whatever order the caller passed in.
    /// That used to be `HashMap::into_iter`, seeded per map instance, and two runs
    /// of the same sentence could put different verses on the screen.
    #[test]
    fn a_tie_keeps_the_order_the_preacher_spoke_in() {
        let spoken = vec![cand("John", 3, 16, 0.90), cand("Romans", 8, 28, 0.90)];
        let ranked = rank_for_wall(spoken);
        assert_eq!(
            ranked.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            vec!["John 3:16", "Romans 8:28"],
            "a tie must fall to what was said first"
        );

        // …and the same two the other way round come out the other way round.
        // If this passed regardless, the assertion above would be meaningless.
        let other = vec![cand("Romans", 8, 28, 0.90), cand("John", 3, 16, 0.90)];
        assert_eq!(
            rank_for_wall(other)
                .iter()
                .map(|(k, _)| k.as_str())
                .collect::<Vec<_>>(),
            vec!["Romans 8:28", "John 3:16"]
        );
    }

    /// Strength still beats order — the tie-break is only for ties.
    #[test]
    fn a_stronger_candidate_still_outranks_an_earlier_weaker_one() {
        let spoken = vec![cand("John", 3, 16, 0.55), cand("Romans", 8, 28, 0.92)];
        assert_eq!(rank_for_wall(spoken)[0].0, "Romans 8:28");
    }
}

/// RG-135 — the translation the speaker named, when Relay does not have it.
///
/// Returns the named abbreviation only when ALL of these hold, because each one is
/// a case where saying something would be noise:
///
///   * the window names a translation at all (`detection::named_translation`);
///   * it is not the translation this fire is actually showing — if the preacher
///     said "King James" and the wall says KJV, there is nothing to report;
///   * Relay does not have it installed, so it could not have shown it anyway.
///     A church that has added the named translation and is simply not using it for
///     this fire is a different situation, and one an operator can see and fix.
///
/// A failed read of `translations` answers `None`. That is the safe direction: this
/// is a caveat on an otherwise correct fire, and inventing one from a database error
/// would put a warning on a verse that is right.
fn named_translation_gap(
    conn: &rusqlite::Connection,
    window: &str,
    fire: &pipeline::Fire,
) -> Option<String> {
    let named = detection::named_translation(window)?;
    if fire.translation.as_deref() == Some(named.as_str()) {
        return None; // the wall already says what the preacher said
    }
    let installed = db::list_translations(conn).ok()?;
    if installed
        .iter()
        .any(|t| t.abbreviation.eq_ignore_ascii_case(&named))
    {
        return None; // Relay has it; this is not the gap this row is about
    }
    Some(named)
}

fn emit_detections<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    text: &str,
    now_ms: u64,
    is_final: bool,
    trace: Option<u64>,
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
    // Latency stamps sampled under the locks and applied after they are released.
    // The block yields them so `detected_at` is INITIALISED by the sample rather
    // than pre-seeded with a value no path ever reads.
    let (detected_at, authorised_at) = {
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
        // A reference named in THIS window outranks the one in memory. FIELD F-1:
        // "…going through in Luke 10. If you read from verse 32, 37" put
        // **Proverbs 3:32** on a congregation's wall, because Proverbs 3:6 had been
        // fired by hand five minutes earlier and the bare 32 was resolved against
        // it — with Luke 10 sitting in the same sentence.
        //
        // Memory is what Relay has when the words do not say. When the words do
        // say, the words win.
        //
        // FIELD F-8 added the second half of that rule: a window can STATE a
        // chapter without any reference parsing out of it ("4th Peter chapter 5
        // verse 10" — there is no 4th Peter), and memory used to win there too.
        // `resolve_bare_verse_for_window` owns the whole decision so it is one
        // pure function with a test, rather than an `or_else` chain here that no
        // test could reach.
        let anchor = detection::anchor_for_bare_verses(text);
        for n in detection::detect_bare_verses(text) {
            let from_memory = context.resolve_bare_verse(n);
            let resolved = detection::resolve_bare_verse_for_window(
                text,
                n,
                anchor.as_ref(),
                from_memory.as_ref(),
            );
            if let Some(r) = resolved {
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
        // A reference exists in this transcript. Sampled, not stamped: the locks
        // above are still held, and `latency` takes a mutex of its own. It is a
        // leaf lock that calls nothing, so there is no cycle to deadlock on — but
        // rule 2 in CLAUDE.md is about not reaching outward while holding a lock,
        // and the discipline is worth more than the two lines it costs. The stamps
        // are applied below, after every lock is released.
        let detected_at = crate::latency::now_us();
        let mut authorised_at: Option<u64> = None;

        // Dedup by reference, keeping the strongest evidence per verse — see
        // pipeline::better for why this is NOT simply the highest confidence.
        //
        // ── R4-07 · A VEC, NOT A HASHMAP, AND THAT IS THE WHOLE FIX ─────────
        //
        // `detect_direct` returns matches left to right — the order the preacher
        // said them. A `HashMap` threw that away and replaced it with SipHash
        // order, seeded per map instance. `rank_for_wall`'s sort is stable, so
        // whenever two candidates tie under `pipeline::better` — same method, same
        // confidence, which is the ordinary case for "turn to John 3:16 and
        // Romans 8:28", both `Direct` at the same score — **which one was left on
        // the wall was decided by a hash, and could differ between two runs of the
        // same sentence.**
        //
        // A window may put at most one verse on a wall (DECISIONS §37), so this is
        // not a cosmetic ordering question: it chooses what the congregation sees.
        // Ties now break on what was said FIRST, which is the only defensible
        // answer available and the one an operator would predict.
        //
        // The linear scan is deliberate: a window yields a handful of candidates,
        // never a corpus, and the same reasoning as `detection.rs`'s 31k-verse
        // linear scan applies — measure before optimising.
        let mut best: Vec<(String, Cand)> = Vec::new();
        for c in candidates {
            let key = Fire::key_for(&c.r);
            match best.iter_mut().find(|(k, _)| *k == key) {
                Some((_, existing)) => {
                    if !pipeline::better(existing, &c) {
                        *existing = c;
                    }
                }
                None => best.push((key, c)),
            }
        }

        // ── ONE WINDOW, ONE WALL ────────────────────────────────────────────────
        //
        // Measured in a real service, 2026-08-23: 58 broadcasts reached the
        // congregation's screens in 45 minutes, and the wall visibly flickered.
        // Two distinct causes, both here, both invisible to every existing gate
        // because the debounce is keyed per REFERENCE and these are different
        // references.
        //
        // 1. A chapter-only reading rides along with the verse. "1 Corinthians
        //    chapter 9 and verse 24" parses as BOTH `9:1` (chapter-only defaults to
        //    verse 1) and `9:24`, each `Direct` at 0.88. Live, the wall showed
        //    9:24 -> 9:1 -> 9:24 over six seconds. Same shape produced 2 Chronicles
        //    15:1, 26:1, Proverbs 3:1, Isaiah 61:1, Hebrews 6:1, Genesis 12:1 and
        //    Psalms 23:1 in one service.
        //
        // 2. Several verses fire from ONE window, at the same instant. Two fires
        //    share a timestamp to the tenth of a second (Matthew 13:10 and
        //    2 Chronicles 15:1 at 1194.2s). A wall can only show one thing, so the
        //    second is not information — it is the first one being erased before
        //    anybody read it.
        //
        // A window is one hearing of one moment of speech. It may inform the
        // operator about several verses; it may put at most ONE on a wall.
        let ranked = rank_for_wall(best);

        for (rank, (key, c)) in ranked.into_iter().enumerate() {
            // Everything after the strongest candidate in this window is offered,
            // never fired. It still reaches the operator instantly.
            let may_fire = rank == 0;
            // `decide_live`, not `decide`: the live path is the one place a
            // candidate is read out of a PARTIAL window that will be decoded again a
            // step later, so it is the one place corroboration is both possible and
            // necessary. See `Router::decide_live` for the measured misreads it
            // exists to catch.
            let status = match router.decide_live(&key, c.conf, c.method, now_ms, is_final) {
                RouteDecision::AutoFire if may_fire => FireStatus::Auto,
                RouteDecision::AutoFire => FireStatus::Suggested,
                RouteDecision::Suggest => FireStatus::Suggested,
                RouteDecision::Drop => continue,
            };
            let end = passage_end(&conn, &c);
            // Auto-detection has no plan cue behind it — content-type default.
            let mut fire = resolve_fire(
                &conn, c.r, c.conf, c.method, status, None, c.matched, None, None,
            );
            // Which decode pass put this verse here. Rides to the console and to
            // every output so the last leg — pixels on a projector — can be timed
            // rather than assumed.
            fire.trace_id = trace;
            // RG-135. Did the speaker NAME a translation, and is it one Relay does
            // not have? The detector is pure and lives in `detection`; whether the
            // named one is installed is a database question, so it is asked here,
            // once, under the connection this loop already holds.
            //
            // Set only when Relay can tell the operator something they do not
            // already know. If the named translation IS what went on the wall,
            // there is nothing to say, and a caveat on a correct fire is how an
            // operator learns to stop reading the line.
            fire.named_translation_missing = named_translation_gap(&conn, text, &fire);

            // Parsed, but the verse doesn't exist (garbled speech readily yields
            // "Psalms 23:99"). Demote to a suggestion rather than broadcasting a
            // verse with no text, which would blank the projector. See
            // `Fire::may_broadcast`.
            if fire.status.goes_to_screen() && fire.verse_id.is_none() {
                fire.status = FireStatus::Suggested;
            }

            if fire.may_broadcast() {
                // The gate said yes and the verse exists: this is the instant a
                // fire became authorised. Everything after it is delivery.
                if authorised_at.is_none() {
                    authorised_at = Some(crate::latency::now_us());
                }
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
        (detected_at, authorised_at)
    }; // locks released here

    if let Some(id) = trace {
        crate::latency::stamp_at(id, crate::latency::Stage::ReferenceDetected, detected_at);
        if let Some(at) = authorised_at {
            crate::latency::stamp_at(id, crate::latency::Stage::FireAuthorised, at);
        }
    }

    let sent_anything = !broadcasts.is_empty();
    for content in broadcasts {
        // The detect thread has nobody to return an error to, and `preflight`
        // has already raised the banner and printed the reason. Swallowed here
        // and nowhere else, deliberately: the alternative is killing the
        // detection thread over one unshowable payload.
        let _ = broadcast_with_clock(handle, content);
    }
    for ev in events {
        let _ = handle.emit("detection://match", ev);
    }
    // Stamped AFTER the broadcast returns, not before it: the kiosk fan-out and
    // the Tauri emit are on this path and are exactly the kind of cost a
    // "reference detected → fired" number is supposed to expose. Only when
    // something actually left for a wall — a window that produced suggestions
    // only has no fire to time, and counting it as an instant one would flatter
    // the metric with the passes that did the least work.
    if let (Some(id), true) = (trace, sent_anything) {
        crate::latency::stamp(id, crate::latency::Stage::FireSent);
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
    if fire_manual(handle, r, 1.0, PassageUpdate::Advance, None, None, None) {
        Ok(NavResult::Fired { reference })
    } else {
        Ok(NavResult::NotInLibrary { reference })
    }
}

/// Tell the operator what a SPOKEN navigation did, when it did not move the wall.
///
/// The preacher is talking; there is no caller to return a result to and nobody is
/// looking at a return value. `Fired` needs no announcement — the wall changed, and
/// that IS the announcement. Everything else is pushed.
///
/// One function, because there is more than one spoken door and the last time a
/// rule was written per-door it held on three of four.
fn announce_nav<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    outcome: error::Result<NavResult>,
) {
    match outcome {
        Ok(NavResult::Fired { .. }) => {}
        Ok(blocked) => {
            let _ = handle.emit("nav://blocked", blocked);
        }
        Err(e) => {
            eprintln!("spoken nav failed: {e}");
            let _ = handle.emit("output://panic_failed", e.to_string());
        }
    }
}

/// Tell the operating system whether Relay still needs the display up.
///
/// ONE caller decides, from the three facts as they are right now — the same
/// choke-point reasoning as rule 36. A `wake::apply` sprinkled at six call sites
/// is a `wake::apply` that will be missing from the seventh, and the symptom
/// would be a projector going black in the one situation nobody added it to.
///
/// Reads each fact under its own lock and releases before calling out, because
/// `wake::apply` talks to IOKit and nothing may hold a lock across a call into
/// the platform (rule 2, in a different coat).
fn refresh_wake<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let capturing = app
        .try_state::<Audio>()
        .and_then(|a| a.0.lock().ok().map(|g| g.is_some()))
        .unwrap_or(false);
    let service_recording = app
        .try_state::<Session>()
        .and_then(|s| s.0.lock().ok().map(|g| g.is_some()))
        .unwrap_or(false);
    let outputs_open = !channels::list_open(app).is_empty();
    wake::apply(wake::Need {
        capturing,
        service_recording,
        outputs_open,
    });
}

/// Spoken in-passage jump ("chapter 5 verse 1", "verse 4"): resolve the BOOK from
/// the current context and fire book chapter:verse, keeping the operator inside
/// the same passage. Chapter-only defaults to verse 1; verse-only keeps the
/// current chapter.
///
/// ## R2-C · IT USED TO RETURN `bool`, AND THAT WAS THE BUG
///
/// This is the FOURTH door into the same failure `NavResult` was built to close.
/// `handle_nav` distinguishes four outcomes and pushes `nav://blocked` for every
/// one that does not move the wall, because the preacher is speaking and there is
/// nobody to return a result to. This function collapsed all of them into `false` —
/// so "verse ninety nine" in a six-verse psalm, or "verse four" before anything had
/// been fired, left the wall unmoved with **no toast, no banner and no log line**.
/// That is the original bug verbatim, on a door nobody had listed.
///
/// It announces its own outcome rather than returning it, because — unlike
/// `handle_nav`, which is also a command with an operator waiting on it — this has
/// exactly one caller and it is the transcript thread. Reporting at the call site
/// would put the guarantee on the door instead of in the room, and the next caller
/// added would silently not have it.
///
/// `None` means the text was not a jump at all, so the caller falls through to
/// detection. `Some(())` means it WAS a jump and has been dealt with — and a jump
/// phrase can never also be a reference, because `detect_passage_nav` returns
/// `None` the moment a book is named.
fn handle_passage_nav<R: tauri::Runtime>(handle: &tauri::AppHandle<R>, text: &str) -> Option<()> {
    let outcome = passage_nav_outcome(handle, text)?;
    announce_nav(handle, outcome);
    Some(())
}

fn passage_nav_outcome<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    text: &str,
) -> Option<error::Result<NavResult>> {
    let nav = detection::detect_passage_nav(text)?;
    let target = {
        let ctx = handle.state::<Context>();
        let context = match ctx.0.lock() {
            Ok(c) => c,
            Err(_) => {
                return Some(Err("Relay lost track of the passage it was reading."
                    .to_string()
                    .into()))
            }
        };
        // No current passage → there is no book to resolve the jump against. Said
        // out loud rather than swallowed: from the operator's seat "nothing is
        // staged" and "Relay did not hear you" look identical.
        let Some(cur) = context.current() else {
            return Some(Ok(NavResult::NoPassage));
        };
        VerseRef {
            book: cur.book.clone(),
            chapter: nav.chapter.unwrap_or(cur.chapter),
            verse: nav.verse.unwrap_or(1),
        }
    };
    let reference = Fire::key_for(&target);
    Some(Ok(
        if fire_manual(handle, target, 1.0, PassageUpdate::Jump, None, None, None) {
            NavResult::Fired { reference }
        } else {
            // The verse parsed and is not in the corpus. Firing it would blank the
            // wall (`Fire::may_broadcast`), so the screen is left alone — and the
            // operator is told which verse it was.
            NavResult::NotInLibrary { reference }
        },
    ))
}

/// Persist a finalized transcript line into the current service (if recording),
/// updating the session's last-transcript id for detection linkage. Locks its
/// own db handle — call OUTSIDE any held db lock.
fn persist_transcript<R: tauri::Runtime>(handle: &tauri::AppHandle<R>, text: &str, language: &str) {
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
    // ── FIELD F-2 · the record must say what this verse actually came from ──
    //
    // Only FINAL transcripts are persisted, and a detection born in a PARTIAL
    // window was attached to whatever final happened to be last. In a real service
    // that put `Proverbs 3:32` next to a sentence containing no book, no number and
    // no keyword — 72 finals in that service contained the words "verse", "chapter"
    // or "bible" exactly zero times, while the detections' own `heard_text` values
    // contained all three.
    //
    // Every history and replay surface is built on `detections → transcripts`, so
    // all of them were reporting a sentence that did not produce the verse beside
    // it, and anything that ever scores accuracy from that join scores the wrong
    // text.
    //
    // So the row this detection points at is a row that really does hold the words
    // the detector read. When the last final IS that text, it is reused and nothing
    // extra is written; when it is not, the window is persisted in its own right.
    // Six rows in a fifty-minute service, and the join stops lying.
    let matches_last = st
        .last_transcript
        .and_then(|t| db::transcript_text(conn, t).ok().flatten())
        .is_some_and(|prev| prev == window_text);
    let tid = match st.last_transcript {
        Some(t) if matches_last || window_text.is_empty() => t,
        _ => match db::insert_transcript(conn, st.id, ts, window_text, "en", None) {
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

/// Append one row to the service timeline.
///
/// Mirrors `persist_cue` in shape and in tolerance: **best-effort, and silent when
/// there is no service.** A history that could take a live service down would be a
/// worse trade than a history with a gap in it, and most of what this records
/// happens at exactly the moments things are already going wrong.
///
/// Lock order `Db` before `Session`, like every other writer here (rule 6).
fn log_event<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    kind: db::EventKind,
    detail: Option<&str>,
) {
    let db = handle.state::<Db>();
    let session = handle.state::<Session>();
    let (Ok(conn), Ok(sess)) = (db.0.lock(), session.0.lock()) else {
        return;
    };
    if let Some(st) = sess.as_ref() {
        let at = st.started.elapsed().as_secs_f64() * 1000.0;
        let _ = db::log_event(&conn, st.id, at, kind, detail);
    }
}

/// Write the latency instrument's current percentiles into the service's history.
///
/// The numbers `latency.rs` holds are in memory only, so the evidence from the run
/// that mattered — the one that ended badly — died when the app closed. Called once
/// a minute while a service records, and once more when it ends.
///
/// **A stage never reached is stored as NULL, not as zero.** Writing 0 would make
/// every service look instantaneous on the stages it never performed, which is the
/// same mistake `latency.rs` fixed inside the histogram.
fn snapshot_latency<R: tauri::Runtime>(handle: &tauri::AppHandle<R>) {
    let report = latency::report(0);
    let db = handle.state::<Db>();
    let session = handle.state::<Session>();
    let (Ok(conn), Ok(sess)) = (db.0.lock(), session.0.lock()) else {
        return;
    };
    let Some(st) = sess.as_ref() else { return };
    let at = st.started.elapsed().as_secs_f64() * 1000.0;
    for m in &report.metrics {
        if m.samples == 0 {
            continue;
        }
        let _ = db::log_perf_sample(
            &conn,
            st.id,
            at,
            &db::PerfSample {
                metric: m.metric,
                samples: m.samples as i64,
                // The per-minute line, which the live report has always carried and
                // nothing ever wrote down (FIELD F-3). The LAST complete bucket, not
                // the one still filling — a partial minute reads as a dip and a dip
                // is exactly the shape somebody would mistake for good news.
                last_minute_ms: if m.per_minute_mean_ms.len() >= 2 {
                    m.per_minute_mean_ms
                        .get(m.per_minute_mean_ms.len() - 2)
                        .copied()
                } else {
                    None
                },
                p50_ms: m.p50_ms,
                p95_ms: m.p95_ms,
                p99_ms: m.p99_ms,
                worst_ms: m.worst_ms,
            },
        );
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

/// THE LATENCY REPORT — where the time went, this session.
///
/// `recent` is how many complete traces to include for reading a single spoken
/// reference end to end; the percentiles cover the whole session regardless.
///
/// This is a diagnostic and it is deliberately available on a packaged build. A
/// church's laptop in a church's room is the only place the numbers are real, and
/// a measurement that needs a developer build is a measurement nobody in a church
/// will ever take.
#[tauri::command]
fn latency_report(recent: Option<usize>) -> latency::Report {
    latency::report(recent.unwrap_or(20).min(latency_recent_cap()))
}

/// Upper bound on `latency_report`'s detail list, so a frontend asking for
/// `usize::MAX` cannot make the bridge serialise the whole ring on every poll.
fn latency_recent_cap() -> usize {
    64
}

/// The console or an output page reporting that it has PAINTED something.
///
/// `at_epoch_ms` is the surface's own `Date.now()`; Rust also stamps the arrival,
/// and the gap between the two is the IPC bridge — reported, not hidden, because
/// "the transcript is late" has a completely different fix depending on which
/// side of that gap the time went.
///
/// Unknown stage names are ignored rather than erroring: this is telemetry on a
/// hot path and a rejected mark must never become an exception in a render.
#[tauri::command]
fn latency_mark(trace_id: u64, stage: String, at_epoch_ms: u64) {
    if let Some(st) = latency::Stage::from_wire(&stage) {
        latency::frontend_mark(trace_id, st, at_epoch_ms);
    }
}

/// Start a clean measurement run — for a field test that wants the numbers for
/// THIS service and not for the hour the app spent idle before it.
#[tauri::command]
fn latency_reset() {
    latency::reset();
}

/// Turn measurement off (or back on). On by default; the cost is a few integer
/// stamps against a decode measured in hundreds of milliseconds.
#[tauri::command]
fn latency_set_enabled(on: bool) -> bool {
    latency::set_enabled(on);
    latency::is_enabled()
}

/// One search result: the verse, and WHY it is here.
///
/// The verse is `#[serde(flatten)]`ed, so every surface that already reads a
/// `VerseRow` off this command — the Library, the Planner, the Live rail, the
/// preacher's remote — keeps reading exactly the fields it read before, and the
/// explanation is additive. That mattered: DECISIONS §72 deferred "why it
/// matched" precisely because it changes the shape three surfaces read.
///
/// `method` and `why` are the same pairing as `DetectionEvent`'s `method` +
/// `matched_text` (CLAUDE.md rule 18): the machine fact the surface colours by,
/// and the human evidence it renders. There is **no percentage** on a
/// paraphrase, here as there.
#[derive(Debug, Clone, Serialize)]
struct SearchHit {
    #[serde(flatten)]
    verse: db::VerseRow,
    /// `reference` · `prefix` · `phrase` · `words` · `paraphrase`.
    method: &'static str,
    /// True when Relay guessed rather than read. Cyan on the rail, never amber.
    guess: bool,
    /// One line saying why this verse is in the list.
    why: String,
    /// The query words that landed. Empty for a reference.
    matched: Vec<String>,
}

impl SearchHit {
    fn new(verse: db::VerseRow, why: search::Why) -> Self {
        SearchHit {
            verse,
            method: why.kind.wire(),
            guess: why.kind.is_guess(),
            why: why.sentence,
            matched: why.matched,
        }
    }
}

/// Scripture search — the Planner's box, the Library, the Live rail and the
/// preacher's remote all come here, so there is one answer to "what did they
/// mean". Two questions in one box (`docs/REBRAND.md` §9): which verse is this
/// REFERENCE, and which verse says these WORDS. Offline, corpus-only.
///
/// **Never a fire.** Every row this returns is an offer; only an operator
/// choosing one reaches a screen (DECISIONS §72, and rule 10 — nothing on this
/// path can reach `AutoFire` because nothing on it touches the router at all).
#[tauri::command]
fn search_scripture(
    db: tauri::State<'_, Db>,
    sem: tauri::State<'_, Semantic>,
    query: String,
) -> error::Result<Vec<SearchHit>> {
    let conn = db.0.lock()?;
    Ok(search_verses(&conn, &sem.0, query.trim()))
}

/// The scripture search itself, over a connection + semantic index — shared by
/// the `search_scripture` command and the preacher-remote HTTP endpoint.
///
/// Five passes, in band order (`search::MatchKind::band`), first-wins per verse:
///
///   1. **Reference** — the query parsed, through the SAME parser the live
///      pipeline uses. A second parser would be a second thing that could
///      disagree with the router about what a reference is.
///   2. **Book prefix** — the query parsed only after a ≥2-letter book prefix was
///      expanded ("philipp 4 13"). Search-only, and deliberately absent from
///      `detection.rs`; see the boundary note at the top of `search.rs`.
///   3. **Phrase** — the whole query, verbatim.
///   4. **Paraphrase** — the semantic index. Marked a guess, with no percentage.
///   5. **Words** — FTS5, floored at `search::MIN_COVERAGE`.
///
/// Every hit carries its `Why`. Nothing here decides, routes or fires: the
/// scores order a LIST and never cross the router (CLAUDE.md rule 10).
fn search_verses(conn: &rusqlite::Connection, sem: &SemanticIndex, query: &str) -> Vec<SearchHit> {
    use search::{MatchKind, Why};

    let q = query.trim();
    if q.is_empty() {
        return vec![];
    }

    let mut scored: Vec<(f32, SearchHit)> = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
    fn take(
        score: f32,
        v: db::VerseRow,
        why: search::Why,
        seen: &mut std::collections::HashSet<i64>,
        scored: &mut Vec<(f32, SearchHit)>,
    ) {
        if seen.insert(v.id) {
            scored.push((score, SearchHit::new(v, why)));
        }
    }

    // 1) Explicit references ("john 3:16", "ps 23", "ps23:1").
    let refs = search::references_in(q);
    let parsed_a_reference = !refs.is_empty();
    for m in refs {
        let r = &m.reference;
        if let Ok(Some(v)) = db::lookup_verse(conn, &r.book, r.chapter, r.verse) {
            take(
                MatchKind::Reference.band(),
                v,
                Why::reference(q),
                &mut seen,
                &mut scored,
            );
        }
    }

    // 2) A book PREFIX, expanded and handed back to the same parser. Only when
    //    nothing parsed as typed — an exact alias (`ps`, `mt`, `jn`, `php`)
    //    already won above, and must never be second-guessed by a prefix.
    if !parsed_a_reference {
        for (prefix, book, rewritten) in search::prefix_expansions(q) {
            for m in search::references_in(&rewritten) {
                let r = &m.reference;
                if let Ok(Some(v)) = db::lookup_verse(conn, &r.book, r.chapter, r.verse) {
                    take(
                        MatchKind::BookPrefix.band(),
                        v,
                        Why::book_prefix(&prefix, book),
                        &mut seen,
                        &mut scored,
                    );
                }
            }
        }
    }

    // 3) Exact phrase (the whole query appears verbatim).
    if q.split_whitespace().count() >= 2 {
        if let Ok(hits) = db::search_verses_text(conn, q, 12) {
            for v in hits {
                take(
                    MatchKind::Phrase.band(),
                    v,
                    Why::phrase(),
                    &mut seen,
                    &mut scored,
                );
            }
        }
    }

    // 4) Semantic paraphrase — top matches by meaning, highest first. NO
    //    coverage floor here: a paraphrase is supposed to find a verse whose
    //    words are different, so a word floor would break the feature it was
    //    meant to protect (DECISIONS §72).
    for (r, score) in sem.top_k(q, 12) {
        if score < 0.08 {
            continue;
        }
        if let Ok(Some(v)) = db::lookup_verse(conn, &r.book, r.chapter, r.verse) {
            // 0.5..0.9, inside the Paraphrase band's own room.
            take(
                MatchKind::Paraphrase.band() + score * 0.4,
                v,
                Why::paraphrase(),
                &mut seen,
                &mut scored,
            );
        }
    }

    // 5) Full-text word/phrase recall (FTS5, bm25-ranked). Catches loose,
    //    non-contiguous word queries ("lord shepherd") a substring LIKE misses,
    //    and ranks the best-matching verse first.
    for (i, v) in db::search_verses_fts(conn, q, 15)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
    {
        // COVERAGE, not just a hit. FTS returns a verse that matched ANY term, so
        // "quantum shepherd tractor engine banana" came back with nineteen verses
        // and Ezekiel 26:9 at the top. A confident wrong answer is worse than an
        // empty list: the operator acts on it.
        let (share, matched) = search::coverage(q, &v.text);
        if share < search::MIN_COVERAGE {
            continue;
        }
        take(
            MatchKind::Words.band() - (i as f32) * 0.008,
            v,
            Why::words(matched),
            &mut seen,
            &mut scored,
        );
    }

    // 5b) Last-ditch substring scan if FTS returned nothing (index still building).
    if scored.is_empty() {
        if let Ok(hits) = db::search_verses_text(conn, q, 15) {
            for v in hits {
                let (share, matched) = search::coverage(q, &v.text);
                if share < search::MIN_COVERAGE {
                    continue;
                }
                take(
                    MatchKind::Words.band() - 0.15,
                    v,
                    Why::words(matched),
                    &mut seen,
                    &mut scored,
                );
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
        // WHICH LAYOUT EACH STAGE SCREEN WEARS — `{"2":{"reading":true,…}}`.
        //
        // READ over HTTP rather than replayed on the WebSocket hello, and that
        // is a deliberate trade with a cost worth naming. Every other
        // configuration map (roles, looks, shows) is a retained hub slot
        // replayed on hello, because the pages that need those have no other
        // way to ask. `stage.html` is the ONLY consumer of this one and it
        // already has this HTTP control plane, so the alternative was an
        // eighteenth parameter on `run_kiosk_server` and twenty-five test call
        // sites for a fact one page reads.
        //
        // The cost: initial state and live updates arrive by two different
        // paths — this route on connect, and a `stage_zones` broadcast when an
        // operator changes an assignment. They are the same two paths this page
        // already uses for its control panel (search over HTTP, content over
        // the socket), and a failed read falls back to the device's own zones
        // rather than to a blank screen.
        "stage_zones" => {
            let db = app.state::<Db>();
            let blob =
                db.0.lock()
                    .ok()
                    .and_then(|conn| db::stage_zones_json(&conn).ok())
                    .unwrap_or_else(|| "{}".to_string());
            // THE WHOLE OBJECT, not a fragment. Every arm of this match builds
            // its own complete reply — `ok` sets the body verbatim and wraps
            // nothing. This returned `"zones":{…}` with no braces, which is not
            // JSON at all: `Stage.svelte`'s `api()` calls `r.json()`, that
            // throws, `loadStageZones` swallows it by design, and the page
            // falls back to the device's own zones. An assigned stage layout
            // would have silently never applied on a real device while every
            // test passed, because the tests mock `fetch`. Found by running the
            // packaged app and curling the route.
            format!(r#"{{"ok":true,"zones":{blob}}}"#)
        }
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
            // `why` and `method` ride to the preacher's phone too. The rule they
            // serve — the operator must see WHICH KIND of claim this is
            // (CLAUDE.md rule 18) — does not stop at the console, and a surface
            // that had to compose its own sentence would compose a different one.
            let items: Vec<String> = rows
                .into_iter()
                .take(20)
                .map(|h| {
                    format!(
                        "{{\"reference\":{},\"text\":{},\"method\":{},\"why\":{},\"guess\":{}}}",
                        json_str(&format!(
                            "{} {}:{}",
                            h.verse.book, h.verse.chapter, h.verse.verse
                        )),
                        json_str(&h.verse.text),
                        json_str(h.method),
                        json_str(&h.why),
                        h.guess
                    )
                })
                .collect();
            format!("{{\"ok\":true,\"results\":[{}]}}", items.join(","))
        }
        "fire" => match param("ref") {
            None => "{\"ok\":false,\"error\":\"no reference\"}".to_string(),
            Some(reference) => {
                // The preacher's phone fires at every screen: it is a person asking
                // for a verse, not a plan cue that named screens.
                match manual_fire(app.clone(), app.state::<Db>(), reference, None, None, None) {
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
fn delete_plan(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    lock.guard("delete_plan")?;
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
fn remove_plan_item(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    // DECISIONS §85. The lock protected the PLAN and not the cues inside it, so a
    // running order could be emptied one row at a time during a service while
    // deleting the whole plan was refused. Skipping a cue is the reversible way to
    // do what a volunteer actually wants mid-service.
    lock.guard("remove_plan_item")?;
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

/// Planner: bind a cue to a programme timer of `minutes`, or clear the binding.
///
/// It STORES and it starts nothing. The Planner may not reach an output or a
/// preacher's rail (`plannerbuildonly.test.js`), so the binding is a fact about
/// the plan and Live is what acts on it when the cue goes on air.
#[tauri::command]
fn set_plan_timer(db: tauri::State<'_, Db>, id: i64, minutes: Option<i64>) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::set_plan_timer(&conn, id, minutes).map_err(Into::into)
}

/// Planner: which screens this cue is for, or every screen (RG-161).
///
/// `None` clears the targeting and means EVERY screen — what every cue written
/// before this existed has, and what a cue goes back to. An empty list reaches
/// NO screen and is deliberately not folded into `None`: those are opposite
/// instructions, and collapsing them would make the emptier one unsayable.
///
/// It STORES and it fires nothing, like `set_plan_timer` beside it: the Planner
/// may not reach an output (`plannerbuildonly.test.js`), so this is a fact
/// about the plan and Live is what acts on it when the cue goes on air.
#[tauri::command]
fn set_plan_channels(
    db: tauri::State<'_, Db>,
    id: i64,
    channels: Option<Vec<i64>>,
) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::set_plan_channels(&conn, id, channels).map_err(Into::into)
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
fn delete_song(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    lock.guard("delete_song")?;
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
fn delete_arrangement(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    lock.guard("delete_arrangement")?;
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
fn delete_saved_scripture(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    lock.guard("delete_saved_scripture")?;
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
fn delete_announcement(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    lock.guard("delete_announcement")?;
    let conn = db.0.lock()?;
    db::delete_announcement(&conn, id).map_err(Into::into)
}

/// Media (Library): all imported media/document assets.
#[tauri::command]
fn list_media(db: tauri::State<'_, Db>) -> error::Result<Vec<db::MediaAsset>> {
    let conn = db.0.lock()?;
    db::list_media(&conn).map_err(Into::into)
}

/// The largest file the Library will accept across the import bridge.
///
/// An imported file does not arrive as a path. The webview's `<input type=file>`
/// yields bytes, so `capture.js::fileToBase64` builds the whole file as a base64
/// string, Tauri serialises that string across the IPC bridge, and this side
/// decodes it into another complete copy before writing it to disk. Several
/// simultaneous copies of a 1.5 GB service video on a church laptop is not a slow
/// import — it is the operating system killing Relay with no error, no message and
/// nothing in any log, on a Saturday, while somebody sets up for Sunday.
///
/// 256 MiB sits comfortably above every background loop, still and lyric file a
/// church actually imports. Above it the answer is a sentence, not a crash.
///
/// The webview holds the SAME limit (`capture.js::MAX_IMPORT_BYTES`) and that copy
/// is the one that actually prevents the allocation — by the time bytes reach here
/// the large string already exists. This one is the door that cannot be walked
/// past: a command is invokable from the webview whatever the UI does.
const MAX_IMPORT_BYTES: usize = 256 * 1024 * 1024;

/// Decode an import payload, refusing an oversized one BEFORE allocating the
/// decoded copy.
///
/// The length check is on the base64 text, which is 4 bytes per 3 decoded, so the
/// estimate is exact enough to be a guard and never rejects a file that would have
/// fit. Shared by both import commands so the limit cannot come to mean two things.
fn decode_import(filename: &str, data: &str) -> error::Result<Vec<u8>> {
    use base64::Engine as _;
    let approx = data.len() / 4 * 3;
    if approx > MAX_IMPORT_BYTES {
        return Err(error::Error::refused(format!(
            "{filename} is about {} MB, and Relay imports files up to {} MB. \
             Shorten or compress it and try again.",
            approx / (1024 * 1024),
            MAX_IMPORT_BYTES / (1024 * 1024)
        )));
    }
    base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| error::Error::refused(format!("could not read {filename}: {e}")))
}

/// Write an imported file to disk, and UNDO the row if it cannot be written.
///
/// The row has to be inserted first, because its id is half the on-disk name. That
/// ordering is what made an orphan possible: when the write failed the row stayed,
/// and its `path` stayed at the schema's empty-string default — a Library entry
/// that exists, lists, and plays nothing. The realistic way to fail here is a full
/// disk, which is also exactly when a church is importing the last thing before a
/// service.
///
/// Split out from `import_media` so the failure branch can actually be executed by
/// a test: pass a directory that is not there and the write fails the same way a
/// full disk does. A cleanup path that has never run is a cleanup path that does
/// not work.
fn write_media_file(
    conn: &rusqlite::Connection,
    dir: &std::path::Path,
    id: i64,
    filename: &str,
    bytes: &[u8],
) -> error::Result<String> {
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
    if let Err(e) = std::fs::write(&path, bytes) {
        let _ = db::delete_media(conn, id);
        return Err(e.into());
    }
    Ok(path.to_string_lossy().to_string())
}

/// Media (Library): import a file (image / video / document). The webview hands
/// us the picked file's bytes (base64); we write it beside the DB and store a
/// pointer — offline-first, nothing uploads.
#[tauri::command]
fn import_media(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    kind: String,
    filename: String,
    data: String,
    date: String,
) -> error::Result<db::MediaAsset> {
    lock.guard("import_media")?;
    let bytes = decode_import(&filename, &data)?;
    let dir = db::media_dir();
    std::fs::create_dir_all(&dir)?;

    let conn = db.0.lock()?;
    let id = db::insert_media(&conn, &kind, &filename, &date)?;
    let path_str = write_media_file(&conn, &dir, id, &filename, &bytes)?;
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
fn delete_media(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    lock.guard("delete_media")?;
    let path = {
        let conn = db.0.lock()?;
        db::delete_media(&conn, id)?
    };
    if let Some(p) = path {
        // A bundled picture's path is a marker, not a location — there is no file
        // to unlink, and asking the filesystem for one would be a no-op dressed
        // as an attempt.
        if media_file_is_on_disk(&p) {
            let _ = std::fs::remove_file(p); // best-effort
        }
    }
    Ok(())
}

/// Demo content: what is loaded right now.
///
/// A read. It guards nothing and changes nothing — a panel that cannot say what is
/// loaded is worse than no panel, and a service lock on a read would be a refusal
/// with nothing behind it.
#[tauri::command]
fn demo_status(db: tauri::State<'_, Db>) -> error::Result<db::demo::DemoStatus> {
    let conn = db.0.lock()?;
    db::demo::status(&conn).map_err(Into::into)
}

/// Demo content: load the sample service.
///
/// **This is the ONLY thing that ever writes demo content** — not a migration, not
/// a first run, not an empty database (`db/demo.rs`). It refuses when a set is
/// already loaded rather than doubling it: two copies of the same plan is exactly
/// the mess an operator would then have to clear by hand.
///
/// Held back during a recorded service. It is a bulk write of the same class as
/// `save_reviewed_songs`, and the Library and Planner it fills are one click from
/// the transport at 10:31.
#[tauri::command]
fn load_demo_content(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    date: String,
) -> error::Result<db::demo::DemoStatus> {
    lock.guard("load_demo_content")?;
    let conn = db.0.lock()?;
    if db::demo::is_loaded(&conn)? {
        return Err(error::Error::refused(
            "Relay's demo content is already loaded. Remove it first if you want a fresh copy.",
        ));
    }
    db::demo::load(&conn, &date, &db::media_dir()).map_err(Into::into)
}

/// Demo content: take it back out.
///
/// Removes exactly the rows the ledger recorded and nothing else. A demo item the
/// operator has since edited is KEPT and released from the ledger; the count comes
/// back so the console can say so rather than leaving them to find out.
///
/// Held back during a recorded service for the plainest of the two reasons the lock
/// exists: it deletes, and there is no undo.
#[tauri::command]
fn remove_demo_content(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
) -> error::Result<db::demo::DemoRemoval> {
    lock.guard("remove_demo_content")?;
    let gone = {
        let conn = db.0.lock()?;
        db::demo::remove(&conn)?
    };
    // Files last, outside the lock, best-effort — the same shape as `delete_media`.
    for p in &gone.files {
        let _ = std::fs::remove_file(p);
    }
    Ok(gone)
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
    let bytes = decode_import(&filename, &data)?;
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
    lock: tauri::State<'_, servicelock::ServiceLock>,
    songs: Vec<SaveSong>,
    date: String,
) -> error::Result<ImportResult> {
    lock.guard("save_reviewed_songs")?;
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

/// Normalize an operator stage note: trim, and treat blank as absent.
fn clean_note(note: Option<String>) -> Option<String> {
    note.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Start a pre-service countdown on every output. Broadcasts the target epoch
/// (now + `minutes`), then each output ticks the MM:SS locally — no per-second
/// network traffic. `label` shows above the timer; `done_msg` replaces it at 0.
///
/// This is the one place a countdown is CREATED. Re-aiming and holding one is
/// [`adjust_countdown`], which can never create one.
// GENERIC OVER THE RUNTIME (rule 24). It puts content on a wall, so it is fire-path
// code, and welded to the concrete desktop handle it could not be driven from
// `e2e.rs` — which is why the countdown was the one fire path with no end-to-end
// test while every other take had one.
// EIGHT ARGUMENTS, AND A STRUCT WOULD BE WORSE HERE. A Tauri command's
// parameters are the named fields of the IPC payload, so grouping them nests
// what the frontend sends and what `ipc.test.js` reads — a shape change to
// every caller in exchange for a lint. Same precedent as `save_song`.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn start_countdown<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    minutes: f64,
    label: String,
    done_msg: String,
    template_id: Option<i64>,
    warn_ms: Option<i64>,
    until_ms: Option<i64>,
    // WHICH SCREENS (RG-161). `None` is every screen. It is stamped onto the
    // timer rather than used here, because this command is one of three that
    // broadcast the same countdown — see `Timer::channels`.
    channels: Option<Vec<i64>>,
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
    // AN APPOINTMENT WINS OVER A LENGTH. `until_ms` is an absolute instant the
    // caller worked out from a clock time, because turning "10:30" into an
    // instant needs the machine's timezone and DST rules and `std` has neither.
    // A time already gone is kept as it is rather than rolled to tomorrow: the
    // countdown starts over, which is what an operator who typed a time that has
    // passed needs to see. 23:55:00 on a lobby screen would hide it.
    let until_ms = until_ms.filter(|at| *at > 0);
    let target = match until_ms {
        Some(at) => at,
        None => now_ms + (mins * 60_000.0) as i64,
    };

    // THE REGISTRY IS WHERE THE COUNTDOWN NOW LIVES, and the four wire fields below
    // are its projection rather than a second copy of it. A second `Both` timer over
    // the first is always a mistake, so starting one takes the one before it — the
    // same rule the old single slot kept by construction, stated out loud now that
    // the slot is a map.
    let timer = {
        let reg = app.state::<timers::TimerRegistry>();
        reg.stop_scope(timers::Scope::Both);
        let id = reg.start(timers::Timer {
            id: 0, // assigned by the registry
            label: label.trim().to_string(),
            done_msg: clean_note(Some(done_msg)).unwrap_or_default(),
            target_ms: target,
            // The instant it is aimed FROM, so `to - from` is the length it was
            // aimed for and the warning rule has a span to work from. This field
            // had a reader and no writer, so §7's short-countdown rule had never
            // fired in the product (see `OutputContent::countdown_from`).
            from_ms: now_ms,
            // A countdown that has just been STARTED is running, always. Pausing is
            // `adjust_countdown`, which is about a countdown already on a screen.
            paused_ms: None,
            // The threshold chosen for THIS countdown, if the caller chose one.
            // It was hard-coded to `None` here, so the transport's own Start was
            // the one door into a `Both` timer that could not express a threshold
            // at all (RG-149(b)). `start_timer` has taken one since wave 3; this
            // is the same field on the same registry, reached from the other door.
            // None is absent, never zero — see `BothProjection::countdown_warn_ms`.
            warn_ms: warn_ms.filter(|n| *n > 0),
            scope: timers::Scope::Both,
            // What Reset would go back to. A congregation countdown has no
            // Reset control today — the dock's Reset is the TOOL's, and puts
            // the length field back rather than the running clock — but the
            // registry row is the same shape either way, and a field filled by
            // one creator and left at zero by the other is how the two come to
            // disagree about the same timer.
            configured_ms: (mins * 60_000.0) as i64,
            until_ms,
            plan_item_id: None,
            // The mode in force at this instant, stamped once and never rewritten
            // (RG-150). Leaving a rehearsal happens to clear the screens, which
            // takes every `Both` timer with it — but that is DECISIONS §27's
            // guarantee, not this one, and a rule that holds only where something
            // else already holds it is not a rule.
            started_in_rehearsal: channels::rehearsing(&app),
            // The cue's screen set, stamped once. Every later broadcast of this
            // countdown reads it back off the timer, so a Pause cannot widen it.
            channels,
        });
        // Cloned out and the lock released before the broadcast below (rule 2).
        reg.get(id).ok_or_else(|| {
            error::Error::refused("The countdown could not be started. Try again.")
        })?
    };

    let (tid, tjson, tpinned) = {
        let conn = db.0.lock()?;
        cue_or_content_tpl(&conn, template_id, "countdown")
    };
    broadcast_with_clock(&app, countdown_content(&timer, tid, tjson, tpinned))?;
    persist_cue(&app, "countdown", None);
    Ok(())
}

/// RE-AIM OR HOLD THE COUNTDOWN THAT IS ALREADY ON THE SCREENS — Reset, ±1, Pause,
/// Resume. It can never create one.
///
/// `remaining_ms` is how long should be left; `paused` whether it should be held.
/// `None` for either means "leave that alone", so `+1` moves the time without
/// touching the hold, and Pause holds it without moving the time.
///
/// ## Why this exists rather than a second call to `start_countdown`
///
/// The console used to assemble a re-aim out of its mirror of the live content —
/// label, done message and template read back off the event and handed to
/// `start_countdown` again. It worked, and it only worked while every caller
/// remembered every field. `countdown_paused_ms` is one more thing to forget, and
/// forgetting THAT one restarts a held timer in front of a congregation: the operator
/// presses `+1` on a paused countdown and it starts running. So the engine keeps the
/// countdown (`timers::TimerRegistry`) and this changes one thing about it.
///
/// ## Two things it must not do
///
/// **It must not put content on a wall by itself.** With no countdown in front of the
/// operator it refuses, in words, rather than starting one: Start is the control that
/// puts a countdown in front of people and there must be exactly one of those.
///
/// **It must not re-skin the screens.** The content is carried over verbatim, which
/// includes `template_pinned` — a countdown fired from the dock resolves through the
/// content LOOK, which defers to each screen's own template (DECISIONS §29). Rebuilding
/// the fire and handing the resolved id back as a cue template would take that
/// deference away, and a press of `+1` would silently re-skin every screen in the
/// building.
#[tauri::command]
fn adjust_countdown<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    remaining_ms: Option<i64>,
    paused: Option<bool>,
) -> error::Result<()> {
    let Some(timer) = newest_congregation_timer(&app) else {
        // Still exactly the sentence an operator reads, and it is still true in the
        // only case that can now produce it: there is no congregation timer at all.
        // A cleared or blacked wall stops one, so the transport still cannot bring
        // back what a panic control took.
        return Err(error::Error::refused("Nothing is counting down."));
    };
    let now_ms = cd_now_ms();
    let adjusted = app
        .state::<timers::TimerRegistry>()
        .adjust(timer.id, remaining_ms, paused, now_ms)
        // `newest_congregation_timer` is the only source of `timer` here, so the
        // scope is `Both` by construction rather than by a lookup that can be wrong.
        .map_err(|e| timer_refusal(e, timers::Scope::Both))?;

    // A RE-AIM MAY NOT TAKE A CONGREGATION SCREEN BACK FROM A SERMON.
    //
    // Before the registry, this could not arise: the countdown WAS the live content,
    // so it was always what the screens were showing or it did not exist. Now it can
    // outlive a verse, and a `+1` that repainted itself over the reading would be the
    // same class of failure this command's own doc comment forbids for templates —
    // a transport press with a consequence nobody asked it for.
    //
    // So the registry changes and nothing is published. The way back onto a wall is
    // `show_timer`, one action that says what it does. "What is on the screens right
    // now" is read from the one slot that already answers it (`channels::LiveContent`)
    // rather than guessed at a second time.
    let Some(mut content) = channels::live_content(&app).filter(is_countdown_content) else {
        return Ok(());
    };
    let shown = timers::project_both(&adjusted);
    // `countdown_to` stays set even while held: it is where the countdown would land
    // if it were resumed now, and it is what keeps the content reading as a countdown
    // to `preflight`, to the retained screen frame and to the slide key.
    content.countdown_to = Some(shown.countdown_to);
    content.countdown_paused_ms = shown.countdown_paused_ms;
    // `countdown_from` is NOT re-stamped. It is the instant the countdown was first
    // aimed from, so the warning span stays the countdown's own length rather than
    // shrinking to whatever is left each time somebody presses a button.
    //
    // Everything else — the label, the done message and above all the template
    // triple — is carried over VERBATIM, which is what keeps a press of `+1` from
    // silently re-skinning every screen in the building (DECISIONS §29).
    //
    // `trace_id` is cleared: an operator's press has no decode pass behind it, and
    // inventing one would put a human action into the AI's latency percentile.
    content.trace_id = None;
    broadcast_with_clock(&app, content)?;
    Ok(())
}

/// Epoch milliseconds. The clock every timer command reads, so they cannot disagree
/// about "now" within one press.
fn cd_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Is this live content a countdown — the same three-way question `pipeline`'s
/// pre-air validator asks, so the two cannot disagree about what a countdown is.
fn is_countdown_content(c: &channels::OutputContent) -> bool {
    c.countdown_to.is_some()
        || c.countdown_paused_ms.is_some()
        || c.kind.as_deref() == Some("countdown")
}

/// The congregation timer the transport is about: the newest `Both` timer, or None.
///
/// Newest rather than oldest because `start_countdown` takes the previous one, so
/// there is at most one — and if a later track ever allows two, the one an operator
/// just started is the one the transport means.
fn newest_congregation_timer<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<timers::Timer> {
    app.try_state::<timers::TimerRegistry>()
        .and_then(|reg| reg.snapshot_scope(timers::Scope::Both).pop())
}

/// A registry refusal in words an operator can act on. Both are `Refused`, not
/// faults: nothing is broken in either case.
///
/// ONE REFUSAL, TWO INSTRUMENTS, AND THE SENTENCE HAS TO KNOW WHICH (DECISIONS §99).
///
/// `TooShort` used to read *"A countdown needs a second or more left. Clear the
/// screens to take it down."* over BOTH scopes. On a `Stage` timer both halves are
/// false: it is not a countdown, it is on no screen, and `Clear screens` takes
/// congregation timers only (DECISIONS §27) — so the one instruction in the sentence
/// is an instruction that will not work, handed to an operator mid-service. Rule 35
/// in its smallest form: one reassuring sentence over two different situations.
///
/// The scope is passed in rather than read here, because this function has no
/// registry and a refusal that had to look one up could fail to.
fn timer_refusal(e: timers::TimerError, scope: timers::Scope) -> error::Error {
    match e {
        timers::TimerError::NoSuchTimer => error::Error::not_found("That timer is not running."),
        timers::TimerError::TooShort => error::Error::refused(match scope {
            timers::Scope::Both => {
                "A countdown needs a second or more left. Clear the screens to take it down."
            }
            timers::Scope::Stage => {
                "A Stage Timer needs a second or more left. Press Stop to take it off the preacher's monitor."
            }
        }),
    }
}

/// One timer as the console reads it: the timer's own fields plus how long is left.
///
/// `remaining_ms` is computed by `timers::remaining_ms`, the ONE Rust statement of
/// that rule, so a list the console renders and a wall a congregation reads cannot
/// disagree about the same timer. The frontend still ticks through
/// `countdown.js::countdownRemainingMs` and that stays the only arithmetic on its
/// side — one rule, stated once on each side of the bridge and never twice on one.
#[derive(serde::Serialize)]
struct TimerView {
    #[serde(flatten)]
    timer: timers::Timer,
    remaining_ms: i64,
}

/// START A TIMER WITHOUT PUTTING IT IN FRONT OF ANYBODY.
///
/// It creates the timer and hands back its identity, and it publishes nothing. That
/// is deliberate: `start_countdown` and `show_timer` are the only two things that
/// may put a timer on a congregation screen, and a third door into that would be
/// the shape of bug this repository keeps finding — a guarantee kept on the doors
/// somebody remembered.
///
/// `scope` is `"both"` or `"stage"`. An unknown scope is refused rather than
/// guessed at: guessing `Both` would put a programme timer in front of a
/// congregation, which is the one mistake that cannot be taken back quietly.
// GENERIC OVER THE RUNTIME (rule 24) — it is timer-path code and `e2e.rs` drives it.
// EIGHT ARGUMENTS, AND A STRUCT WOULD BE WORSE HERE. A Tauri command's
// parameters are the named fields of the IPC payload, so grouping them nests
// what the frontend sends and what `ipc.test.js` reads — a shape change to
// every caller in exchange for a lint. Same precedent as `save_song`.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn start_timer<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    minutes: f64,
    label: String,
    done_msg: String,
    scope: String,
    warn_ms: Option<i64>,
    plan_item_id: Option<i64>,
    until_ms: Option<i64>,
) -> error::Result<i64> {
    let scope = match scope.trim().to_ascii_lowercase().as_str() {
        "both" => timers::Scope::Both,
        "stage" => timers::Scope::Stage,
        other => {
            return Err(error::Error::refused(format!(
                "A timer is for \"both\" screens or the \"stage\" monitor, not \"{other}\"."
            )))
        }
    };
    let mins = if minutes.is_finite() && minutes > 0.0 {
        minutes
    } else {
        5.0
    };
    let now_ms = cd_now_ms();
    // ONE CLOCK PER CUE. A cue that is put on air again — the operator steps back
    // and forward, or re-takes a slide — asks for its timer to start again, not for
    // a second one beside it. Without this, walking a plan backwards and forwards
    // stacks a clock on the preacher's rail per press, and rule 35's floor on that
    // rail (Track A) would be dividing the width between clocks nobody asked for.
    // `for_plan_item` is the reader that makes it answerable; an unbound start
    // (`plan_item_id: None`) is untouched and still makes a new timer every time.
    if let Some(cue) = plan_item_id {
        let reg = app.state::<timers::TimerRegistry>();
        if let Some(previous) = reg.for_plan_item(cue) {
            reg.stop(previous.id);
        }
    }
    let id = app.state::<timers::TimerRegistry>().start(timers::Timer {
        id: 0, // assigned by the registry
        label: label.trim().to_string(),
        done_msg: clean_note(Some(done_msg)).unwrap_or_default(),
        target_ms: match until_ms.filter(|at| *at > 0) {
            Some(at) => at,
            None => now_ms + (mins * 60_000.0) as i64,
        },
        from_ms: now_ms,
        paused_ms: None,
        warn_ms,
        scope,
        // WHAT RESET GOES BACK TO. Stated here rather than derived later: a
        // re-aim moves `target_ms` and leaves `from_ms`, so the span stops
        // being the length anybody chose the first time `+5` is pressed.
        configured_ms: (mins * 60_000.0) as i64,
        // See `start_countdown`: an appointment is an instant the caller worked
        // out where local time is known, and Reset goes back to it rather than
        // to a length.
        until_ms: until_ms.filter(|at| *at > 0),
        plan_item_id,
        // The mode in force at this instant — see `start_countdown`, and
        // `timers::Timer::started_in_rehearsal` for why it is a property of the
        // timer rather than a question asked at the exit.
        started_in_rehearsal: channels::rehearsing(&app),
        // NO SCREEN SET FROM THIS DOOR. `start_timer` makes the preacher's
        // programme clocks, and a stage frame is addressed to the tablet by being
        // the stage frame. A `Both` timer started here carries none either, which
        // is every screen — the behaviour it has always had.
        channels: None,
    });
    // THE STAGE TABLET IS TOLD, UNCONDITIONALLY — not "if this one was a stage
    // timer". `publish_timers` sends the whole stage-visible SET, so it is
    // idempotent and asks no question; a publisher that had to decide whether it
    // was needed is a publisher that can decide wrongly, which is the shape of the
    // four "guarantee kept on one door" bugs this repository has already had.
    channels::publish_timers(&app);
    Ok(id)
}

/// RE-AIM OR HOLD ONE TIMER BY ITS IDENTITY — the transport, addressed.
///
/// `adjust_countdown` is the same action aimed at "whichever congregation timer is
/// running", which is what the dock's transport means. This one names the timer, so
/// a console showing several can move the one under the operator's finger.
///
/// The stage layouts an operator can choose between. Global, by name.
#[tauri::command]
fn list_stage_layouts(db: tauri::State<'_, Db>) -> error::Result<Vec<db::StageLayout>> {
    let conn = db.0.lock().map_err(|_| error::Error::Busy {
        message: "The database is busy. Try again.".into(),
    })?;
    Ok(db::list_stage_layouts(&conn)?)
}

/// Turn a layout refusal into a sentence a volunteer can act on.
///
/// Every arm names WHAT to do next, because a refusal an operator cannot act on
/// is a dead end in the middle of setting a service up.
fn layout_refusal(r: db::LayoutRefusal) -> error::Error {
    match r {
        db::LayoutRefusal::NoName => {
            error::Error::refused("A stage layout needs a name.".to_string())
        }
        db::LayoutRefusal::NameTaken => error::Error::refused(
            "There is already a stage layout with that name. Choose another.".to_string(),
        ),
        db::LayoutRefusal::BadZones => error::Error::refused(
            "That layout does not name any zones, so nothing would change.".to_string(),
        ),
        db::LayoutRefusal::Seeded => error::Error::refused(
            "This is one of the layouts Relay ships with, so it cannot be removed — \
             it would come back the next time Relay starts. Rename it and change \
             what it shows instead."
                .to_string(),
        ),
        db::LayoutRefusal::InUse(names) => error::Error::refused(format!(
            "{} {} using this layout. Give {} a different one first.",
            names.join(", "),
            if names.len() == 1 { "is" } else { "are" },
            if names.len() == 1 { "it" } else { "them" },
        )),
        db::LayoutRefusal::NotFound => {
            error::Error::not_found("That stage layout is no longer there.".to_string())
        }
    }
}

/// Create a stage layout, or rename and re-zone one that exists.
#[tauri::command]
fn upsert_stage_layout<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    id: Option<i64>,
    name: String,
    zones: serde_json::Value,
) -> error::Result<i64> {
    let saved = {
        let conn = db.0.lock().map_err(|_| error::Error::Busy {
            message: "The database is busy. Try again.".into(),
        })?;
        db::upsert_stage_layout(&conn, id, &name, &zones)?
    };
    let id = saved.map_err(layout_refusal)?;
    // An EDIT changes what screens already wearing it show, so the screens are
    // told. A create changes nothing until it is assigned, and publishing then
    // is a no-op — one call either way rather than a branch that can be wrong.
    publish_stage_zones(&app, &db);
    Ok(id)
}

/// Remove a stage layout, unless doing so would be silently wrong.
#[tauri::command]
fn delete_stage_layout<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    id: i64,
) -> error::Result<()> {
    {
        let conn = db.0.lock().map_err(|_| error::Error::Busy {
            message: "The database is busy. Try again.".into(),
        })?;
        db::delete_stage_layout(&conn, id)?.map_err(layout_refusal)?;
    }
    publish_stage_zones(&app, &db);
    Ok(())
}

/// Point one stage screen at one layout, or at none.
///
/// `None` is the way back, and it is a real answer rather than a reset: the
/// screen returns to whatever zones the DEVICE has in its own `localStorage`,
/// which is the arrangement a church may already be using. That is what stops
/// this feature silently erasing one.
///
/// It publishes the whole map, the way every other configuration map is
/// published — a delta would leave a screen that missed one frame wrong about
/// itself for the rest of a service with no way to find out.
#[tauri::command]
fn set_channel_stage_layout<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    channel_id: i64,
    layout_id: Option<i64>,
) -> error::Result<()> {
    {
        let conn = db.0.lock().map_err(|_| error::Error::Busy {
            message: "The database is busy. Try again.".into(),
        })?;
        db::set_channel_stage_layout(&conn, channel_id, layout_id)?;
    }
    publish_stage_zones(&app, &db);
    Ok(())
}

/// Tell every stage screen which layout it wears now.
///
/// Broadcast only — there is no retained slot and no hello replay for this one.
/// `stage.html` reads its initial state from `GET /api/stage_zones` on connect,
/// because it is the only consumer and the only page with that HTTP plane; see
/// the route for the trade and its cost.
fn publish_stage_zones<R: tauri::Runtime>(app: &tauri::AppHandle<R>, db: &tauri::State<'_, Db>) {
    let blob =
        db.0.lock()
            .ok()
            .and_then(|conn| db::stage_zones_json(&conn).ok())
            .unwrap_or_else(|| "{}".to_string());
    if let Some(hub) = app.try_state::<channels::KioskHub>() {
        hub.publish(channels::stage_zones_frame(&blob));
    }
}

/// PUT A TIMER BACK TO THE LENGTH IT WAS STARTED AT.
///
/// The third transport verb. `+5` adds to what is there and Stop takes the timer
/// away; neither is "start that again", and doing it by hand — Stop then Start —
/// loses the label, the chosen warning threshold and the cue binding along with
/// the figure.
///
/// It answers HOW LONG, never running-or-not: a held timer is reset where it
/// stands and stays held. Resuming as a side effect would start a clock nobody
/// asked to start, which on a stage is a figure moving under somebody
/// mid-sentence.
///
/// Same publication rule as `adjust_timer`: it puts nothing on a screen, and a
/// `Both` timer that IS on the screens has the wall brought into line rather
/// than re-fired — the registry and the wall may never disagree about the same
/// countdown.
#[tauri::command]
fn reset_timer<R: tauri::Runtime>(app: tauri::AppHandle<R>, timer_id: i64) -> error::Result<()> {
    let scope = app
        .state::<timers::TimerRegistry>()
        .get(timer_id)
        .map(|t| t.scope)
        .unwrap_or(timers::Scope::Both);
    let back = app
        .state::<timers::TimerRegistry>()
        .reset(timer_id, cd_now_ms())
        .map_err(|e| timer_refusal(e, scope))?;

    if back.scope == timers::Scope::Both {
        if let Some(mut content) = channels::live_content(&app).filter(is_countdown_content) {
            let shown = timers::project_both(&back);
            content.countdown_to = Some(shown.countdown_to);
            content.countdown_from = Some(shown.countdown_from);
            content.countdown_paused_ms = shown.countdown_paused_ms;
            content.trace_id = None;
            broadcast_with_clock(&app, content)?;
        }
    }
    channels::publish_timers(&app);
    Ok(())
}

/// Like `adjust_countdown`, it publishes nothing: changing a number on a timer that
/// is not on the screens must not put it on them.
#[tauri::command]
fn adjust_timer<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    timer_id: i64,
    remaining_ms: Option<i64>,
    paused: Option<bool>,
) -> error::Result<()> {
    // WHICH INSTRUMENT IS BEING REFUSED. Read before the adjustment and not after,
    // because a refused adjustment returns no timer to ask; `None` can only mean
    // `NoSuchTimer`, whose sentence is the same either way, so the fallback is never
    // the one an operator reads.
    let scope = app
        .state::<timers::TimerRegistry>()
        .get(timer_id)
        .map(|t| t.scope)
        .unwrap_or(timers::Scope::Both);
    let adjusted = app
        .state::<timers::TimerRegistry>()
        .adjust(timer_id, remaining_ms, paused, cd_now_ms())
        .map_err(|e| timer_refusal(e, scope))?;

    // …unless it IS on the screens, in which case the wall must agree with the
    // registry. Same rule, same reading of the same slot, as `adjust_countdown`.
    if adjusted.scope == timers::Scope::Both {
        if let Some(mut content) = channels::live_content(&app).filter(is_countdown_content) {
            let shown = timers::project_both(&adjusted);
            content.countdown_to = Some(shown.countdown_to);
            content.countdown_paused_ms = shown.countdown_paused_ms;
            content.trace_id = None;
            broadcast_with_clock(&app, content)?;
        }
    }
    // And the stage tablet, whichever scope this was — see `start_timer`.
    channels::publish_timers(&app);
    Ok(())
}

/// TAKE ONE TIMER OFF THE REGISTRY.
///
/// It does not touch a screen. Stopping a timer that is currently painted leaves the
/// countdown on the wall until something replaces it or a panic control takes it —
/// which is the right way round: `Clear screens` is how a wall is taken back, and it
/// is one key away at every moment (rule 15).
///
/// Stopping a timer that is not there is not an error. The operator asked for it to
/// be gone and it is gone; refusing would be a control that fails at doing nothing.
#[tauri::command]
fn stop_timer<R: tauri::Runtime>(app: tauri::AppHandle<R>, timer_id: i64) -> error::Result<()> {
    app.state::<timers::TimerRegistry>().stop(timer_id);
    // Stopping the LAST programme timer publishes an empty set, which is how a clock
    // comes off a preacher's screen. Not publishing would leave it there, counting,
    // for the rest of the service — an absent frame cannot say "there are none now".
    channels::publish_timers(&app);
    Ok(())
}

/// EVERY TIMER, OLDEST FIRST, WITH HOW LONG IS LEFT ON EACH.
#[tauri::command]
fn list_timers<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> error::Result<Vec<TimerView>> {
    let now_ms = cd_now_ms();
    Ok(app
        .state::<timers::TimerRegistry>()
        .snapshot()
        .into_iter()
        .map(|timer| TimerView {
            remaining_ms: timers::remaining_ms(&timer, now_ms),
            timer,
        })
        .collect())
}

/// PUT A CONGREGATION TIMER BACK IN FRONT OF PEOPLE — **the explicit way back.**
///
/// A timer now outlives the content that replaced it, so after a reading there is
/// something to return to. Returning to it is this, an action an operator takes on
/// purpose; it is never a side effect of `+1`, because a transport press that
/// repainted a countdown over a sermon would be a control doing something other
/// than what it says.
///
/// It carries whatever the timer says NOW — the adjusted figure, and the hold if it
/// is held — so what goes back up is what the operator has been looking at in the
/// list, not the five minutes it started as.
///
/// A `Stage`-scoped timer is refused, in words: it has no congregation wire form,
/// and projecting one into the four `countdown_*` fields would put the preacher's
/// private clock on the wall.
// GENERIC OVER THE RUNTIME (rule 24) — it puts content on a wall, so it is fire-path
// code and `e2e.rs` has to be able to drive it.
#[tauri::command]
fn show_timer<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    timer_id: i64,
    template_id: Option<i64>,
) -> error::Result<()> {
    // Cloned out of the registry, with the lock released, before anything is
    // broadcast (rule 2).
    let timer = app
        .state::<timers::TimerRegistry>()
        .get(timer_id)
        .ok_or_else(|| error::Error::not_found("That timer is not running."))?;
    if timer.scope != timers::Scope::Both {
        return Err(error::Error::refused(
            "That timer is for the stage monitor, so it cannot be put on the screens.",
        ));
    }
    let (tid, tjson, tpinned) = {
        let conn = db.0.lock()?;
        cue_or_content_tpl(&conn, template_id, "countdown")
    };
    broadcast_with_clock(&app, countdown_content(&timer, tid, tjson, tpinned))?;
    // ── SITE 11 OF THE CONTENT-KIND SWEEP. NOTHING CHANGED, AND WHY ───────────
    //
    // `cues.type` is free-form TEXT with no CHECK (docs/data/schema.sql), so a new
    // value would need no migration — and none is written. Putting a timer back is
    // recorded as `"countdown"`, the same value `start_countdown` writes, because
    // it is the same thing appearing on the same screens; a service history that
    // called the two different things would be making a distinction a reader of the
    // history cannot act on. Nothing writes a sixth `plan_items.cue_type` either,
    // so the enumerating comment at `schema.sql`'s `cue_type` column is still
    // accurate and is deliberately left alone.
    persist_cue(&app, "countdown", None);
    Ok(())
}

/// Build the wire form of a `Both` timer. **The one place a timer becomes content**,
/// so `start_countdown` and `show_timer` cannot put different things on a wall.
fn countdown_content(
    timer: &timers::Timer,
    template_id: Option<i64>,
    template_json: Option<String>,
    template_pinned: bool,
) -> channels::OutputContent {
    let shown = timers::project_both(timer);
    channels::OutputContent {
        kind: Some("countdown".into()),
        // OFF THE TIMER, NOT OFF THE CALL. This function has three callers —
        // `start_countdown`, `adjust_countdown` and `show_timer` — and a screen
        // set that lived on the argument would be correct at the first and lost
        // at the other two, which is a countdown that leaks onto every screen in
        // the building the moment somebody holds it.
        channels: timer.channels.clone(),
        reference: shown.reference,
        countdown_to: Some(shown.countdown_to),
        countdown_from: Some(shown.countdown_from),
        countdown_paused_ms: shown.countdown_paused_ms,
        countdown_done: Some(shown.countdown_done).filter(|s| !s.is_empty()),
        // The threshold chosen for THIS timer, straight off the one projection.
        // The configured default is not resolved against it here: that ranking is
        // `layers.js::countdownWarning`'s, once, and it is stamped at the one
        // content door (`broadcast_with_clock`) rather than at the three callers of
        // this function.
        countdown_warn_ms: shown.countdown_warn_ms,
        template_id,
        template_json,
        template_pinned,
        ..Default::default()
    }
}

/// Fire arbitrary content straight to the output screens — the generic take for
/// non-scripture cues (a song section, an announcement). Same broadcast path as
/// a scripture manual fire; operator override, always. `label` is the on-screen
/// citation, `text` the body. `stage_note` is the operator's confidence-monitor
/// note for this cue, if any.
// GENERIC OVER THE RUNTIME, deliberately (CLAUDE.md §24). Welded to the
// concrete desktop handle, this path could not be driven from `e2e.rs` — and
// the one code that decides what a congregation reads would have no test.
// EIGHT ARGUMENTS, and a struct would be worse — see `start_countdown`. A
// Tauri command's parameters are the named fields of the IPC payload.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn fire_content<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    label: String,
    text: String,
    kind: String,
    stage_note: Option<String>,
    template_id: Option<i64>,
    // WHICH SCREENS (RG-161). `None` is every screen.
    channels: Option<Vec<i64>>,
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
            channels,
            reference: projected,
            text: Some(text),
            translation: None,
            template_id: tid,
            template_json: tjson,
            template_pinned: tpinned,
            stage_note: clean_note(stage_note),
            ..Default::default()
        },
    )?;
    persist_cue(&app, "manual_override", Some(&label));
    Ok(())
}

/// Fire a media asset (image/video) to the output screens as a full-screen
/// background. The file is served by the embedded HTTP server at
/// `http://<lan-ip>:8032/media/<id>` so native windows AND kiosk/OBS clients
/// load the same URL. Documents (pdf/pptx) aren't renderable as output yet.
#[tauri::command]
fn fire_media<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    id: i64,
    template_id: Option<i64>,
    // WHICH SCREENS (RG-161). `None` is every screen, which is what every media
    // cue written before targeting existed carries. This argument was missing
    // while `fire_content`'s twin three functions up already had it, so the
    // Planner's `Screens` row ticked, said "Other screens keep what they are
    // showing", and reached all of them.
    channels: Option<Vec<i64>>,
) -> error::Result<()> {
    #[allow(clippy::type_complexity)]
    let (kind, filename, path, tid, tjson, tpinned): (
        String,
        String,
        String,
        Option<i64>,
        Option<String>,
        bool,
    ) = {
        let conn = db.0.lock()?;
        let (k, f, p) = conn
            .query_row(
                "SELECT kind, filename, path FROM media_assets WHERE id = ?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
            .map_err(|_| "media not found".to_string())?;
        let (tid, tjson, tpinned) = cue_or_content_tpl(&conn, template_id, "media");
        (k, f, p, tid, tjson, tpinned)
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
            channels,
            media_url: Some(media_url(&ip, id, &path)),
            media_kind: Some(media_kind.to_string()),
            template_id: tid,
            template_json: tjson,
            template_pinned: tpinned,
            ..Default::default()
        },
    )?;
    persist_cue(&app, "media", Some(&filename));
    Ok(())
}

/// THE ONE DOOR A BACKGROUND LEAVES BY — `broadcast_with_clock` for the second
/// payload kind.
///
/// It exists for exactly the reason that one does: the pre-air check goes at the
/// choke point and not at the call sites (rule 36). There is one caller today and
/// that is the point at which to build the door — a validator added to the second
/// caller, next year, is a validator the first one never had. Four separate bugs
/// in this repository have that shape, and `pipeline::preflight` was written
/// after the fourth.
///
/// `None` takes the background down and is NOT validated, deliberately: a check
/// that could refuse a removal is a removal that can fail, and a backdrop nobody
/// can take off a congregation screen is the failure `clear` exists to prevent
/// (DECISIONS §20). The rehearsal gate, both doors and the retained slot are all
/// `channels::set_background`'s; this function owns the check and nothing else.
///
/// It does NOT touch the passage, `LiveContent` or `WallState`. See
/// `channels::set_background` for why each of those is the wrong question to ask
/// about furniture.
// GENERIC OVER THE RUNTIME (rule 24) — `e2e.rs` has to be able to drive it.
fn publish_background<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    bg: Option<channels::Background>,
) -> error::Result<()> {
    if let Some(b) = bg.as_ref() {
        if let Err(bad) = pipeline::preflight_background(b) {
            // Said in the same three places a refused broadcast is said in, and for
            // the same reason: doing nothing quietly is the failure being fixed.
            eprintln!("preflight refused a background: {bad:?}");
            let _ = handle.emit("output://panic_failed", bad.message());
            return Err(error::Error::refused(bad.message()));
        }
    }
    channels::set_background(handle, bg);
    Ok(())
}

/// PUT A PICTURE BEHIND THE WORDS — or take it away (`id: None`).
///
/// The control that closes the largest gap between Relay and the software
/// churches compare it with: until this existed a verse and a picture were
/// mutually exclusive payloads, because the whole layer stack renders inside
/// `{#if content}` and `media_url` is a field ON the content. Firing the church's
/// backdrop REPLACED the reading; firing the reading replaced the backdrop.
///
/// **One command for both directions, on purpose.** A separate `clear_background`
/// would be a second door onto one piece of state, and this repository's own
/// register of that mistake runs to four entries. `None` is an answer here, not a
/// missing argument.
///
/// Documents are refused with the same sentence `fire_media` uses, because it is
/// the same fact about the same table: a PDF has no frame to paint.
///
/// **Nothing is persisted.** A background is service state, like the transition
/// override and unlike a template: it belongs to the morning it was put up in,
/// and a church that reopened Relay on Tuesday to a Sunday backdrop would have to
/// find the control that took it off. The retained hub slot is what carries it
/// across a screen reconnecting, which is the case that actually happens.
/// IS THERE A PROPRESENTER LIBRARY ON THIS COMPUTER ALREADY?
///
/// Requirement 14: *"allow the app to be able to find any propresenter files
/// automatically on the computer if its ever available"*.
///
/// **It finds and counts. It imports nothing**, opens no song, and changes nothing
/// on disk. The operator is offered what was found and decides; `Import folder`
/// is what actually reads it.
///
/// `home_dir()` rather than `$HOME`, for rule 9's reason one directory up: a
/// hand-rolled `$HOME` is why packaged Windows once ran with speech recognition
/// silently dead. Windows has no `HOME`.
///
/// **On macOS this is rule 17 ground.** `~/Documents` is TCC-gated, and a build
/// without `NSDocumentsFolderUsageDescription` does not get a polite refusal — the
/// directory reads as empty, which is indistinguishable from a church that has no
/// library. `scan_root` treats an unreadable directory as zero and says nothing
/// was found rather than claiming there is nothing there, and the string is in
/// `Info.plist`. Neither is visible in `tauri dev`; `scripts/sign-local.sh` is how
/// it gets checked without a certificate.
#[tauri::command]
fn find_propresenter<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> error::Result<Vec<prodiscover::FoundLibrary>> {
    use tauri::Manager;
    let home = app
        .path()
        .home_dir()
        .map_err(|_| "this computer has no home directory Relay can read".to_string())?;
    Ok(prodiscover::find(&home))
}

/// HOLD THE CLIP, LOOP IT, OR START IT AGAIN.
///
/// Requirement 11's transport. Each argument is what `adjust_countdown` calls a
/// re-aim: `None` means "leave that alone", so Pause cannot un-loop and Loop
/// cannot un-pause. An operator presses one control at a time and the others must
/// survive it.
///
/// `replay` is not a state, so it is not a boolean on the wire either — it bumps a
/// counter. An operator pressing Replay twice on a clip already at its start would
/// otherwise publish a frame identical to the retained one, and a screen that had
/// acted on the first would do nothing. See `channels::media_transport_frame_json`.
///
/// **It says nothing about whether a screen obeyed**, and must not: the screens
/// report where their clip actually is on the beat (`channels::MediaBeat`), and
/// Live reads the transport's effect from THAT rather than from the fact that a
/// command returned `Ok`. A control that reported its own instruction back as an
/// outcome is rule 35 with extra steps.
#[tauri::command]
fn set_media_transport<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    transport: tauri::State<'_, channels::MediaTransport>,
    paused: Option<bool>,
    // `looping`, not `loop`: the wire says `loop` and Rust cannot.
    looping: Option<bool>,
    replay: Option<bool>,
) -> error::Result<()> {
    let (paused, looping, epoch) = transport.apply(paused, looping, replay.unwrap_or(false));
    channels::media_transport(&app, paused, looping, epoch);
    Ok(())
}

/// PUT SOMETHING ON THE PREACHER'S OWN SCREEN, or take it off (`None`).
///
/// An announcement slide, or the preacher's own deck, on the stage display and
/// nowhere else. **Not a background**: `show_background` puts the church's
/// picture behind the words on every screen, and this puts one person's
/// reference material on one screen.
///
/// **Scripture overrides it, and that rule lives on the device.** The stage page
/// paints a reading over the media while it has one and paints the media again
/// when the reading is cleared. It is deliberately NOT taken down when a verse
/// arrives: an operator who had to push the slide again after every reading would
/// not call that "overrides", and it is not what was asked for.
///
/// Documents are refused here for the same reason `fire_media` refuses them —
/// nothing in the product renders a PDF to a screen, so a cue built from one
/// would look fine in the Library and die on a Sunday.
#[tauri::command]
fn send_stage_media<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    id: Option<i64>,
) -> error::Result<()> {
    let Some(id) = id else {
        // TAKE IT DOWN. No lookup, no database — the way off a screen may never
        // depend on a row still being there. The same rule as `show_background`.
        channels::stage_media(&app, None);
        return Ok(());
    };
    let (kind, path) = {
        let conn = db.0.lock()?;
        conn.query_row(
            "SELECT kind, path FROM media_assets WHERE id = ?1",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(|_| "media not found".to_string())?
    };
    let media_kind = match kind.as_str() {
        "image" => "image",
        "video" => "video",
        _ => {
            return Err(error::Error::refused(
                "documents can't be put on the stage screen yet",
            ))
        }
    };
    let ip = local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
    // The one URL builder, for the reason its own doc comment records: a picture
    // Relay ships has no file under `/media/<id>`, so a second rule here would
    // hand the stage screen a URL that 404s.
    channels::stage_media(
        &app,
        Some((media_url(&ip, id, &path), media_kind.to_string())),
    );
    Ok(())
}

#[tauri::command]
fn show_background<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    id: Option<i64>,
) -> error::Result<()> {
    let Some(id) = id else {
        // TAKE IT DOWN. No lookup, no validation, no database — the way off a
        // congregation screen may never depend on a row still being there.
        return publish_background(&app, None);
    };
    let (kind, path) = {
        let conn = db.0.lock()?;
        conn.query_row(
            "SELECT kind, path FROM media_assets WHERE id = ?1",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .map_err(|_| "media not found".to_string())?
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
    publish_background(
        &app,
        Some(channels::Background {
            // THE SAME BUILDER THE FIRED PICTURE USES. A picture Relay ships has no
            // file under `/media/<id>` at all (DECISIONS §90), so a second rule here
            // would hand every screen a URL that 404s — a black wall with nothing in
            // any log, which is precisely the failure `media_url`'s own doc comment
            // records.
            media_url: media_url(&ip, id, &path),
            media_kind: media_kind.to_string(),
        }),
    )
}

/// Where an output page loads a media asset from.
///
/// Two kinds of row live in `media_assets` and they are served from two
/// different places. A file the operator imported sits in the media directory
/// under `{id}_{name}` and is streamed by id. A picture Relay ships has no file
/// of its own at all: its bytes are in the embedded bundle, at the stable path
/// its `bundled:` marker names, which the same server already serves
/// (DECISIONS §90). Building `…/media/<id>` for one of those hands every screen
/// a URL that 404s, which paints a black wall and logs nothing.
fn media_url(ip: &str, id: i64, path: &str) -> String {
    match path.strip_prefix(db::BUNDLED_PREFIX) {
        Some(rest) => format!("http://{ip}:8032/{rest}"),
        None => format!("http://{ip}:8032/media/{id}"),
    }
}

/// Is this stored path a real file somewhere, or a marker for bundled bytes?
/// Deleting a bundled row removes the Library entry; there is nothing to unlink.
fn media_file_is_on_disk(path: &str) -> bool {
    !path.starts_with(db::BUNDLED_PREFIX)
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
///
/// ── SITE 3 OF THE CONTENT-KIND SWEEP. NOTHING CHANGED HERE, AND WHY ──────────
///
/// `kind` here is a lookup key into the content-look register, so a kind with no
/// row simply falls through to the configured default — it is never silently
/// unstyled. The timer registry adds no key: `start_countdown` and `show_timer`
/// both ask for `"countdown"`, which is the row that already exists, because a
/// congregation timer's content kind did not change. A `Stage`-scoped timer asks
/// nothing of this function: it renders on the stage page, which has no
/// congregation template to resolve, and `show_timer` refuses to project one.
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
    let id = db::content_template_id(conn, kind)
        .ok()
        .flatten()
        .or_else(|| {
            // NOTHING BOUND THIS KIND, so the screens following the content look
            // wear the configured default. The id travels so the console readout
            // names what the wall will actually paint; the JSON still does not.
            db::get_setting(conn, "default_template_id")
                .ok()
                .flatten()
                .and_then(|s| s.parse::<i64>().ok())
        });
    (id, None, false)
}

/// THE CONTENT KINDS A LOOK CAN BE SET FOR.
///
/// The same five names exist in two shapes, and this is the only one that can be
/// iterated: `ContentTemplates` names them as struct FIELDS, because that is the
/// map an operator edits and the IPC shape the console reads. Neither can be
/// derived from the other, so `the_content_look_kinds_agree_with_the_map` asserts
/// that they still say the same thing — a kind added to the matrix and not to
/// this array is a look an operator can set and no screen is ever sent.
///
/// There used to be a THIRD shape: a bound in `channels` equal to this list's
/// length, expressed as a limit on what a hello reply may carry. Per-kind looks
/// (DECISIONS §97) put a second source of the same kind of id on that wire, so
/// the bound is now `channels::MAX_LOOK_IDS` and is a judgement rather than this
/// list's length. The assertion that survives is the useful half: these five must
/// still fit.
///
/// It is also what `set_channel_look` validates against, so a kind missing from
/// here cannot be written into `channel_looks` either.
const CONTENT_LOOK_KINDS: [&str; 5] = ["scripture", "song", "media", "announce", "countdown"];

/// The distinct template ids this install's content looks name, in kind order.
///
/// This is the whole of what a screen with no look of its own can be asked to
/// wear, and it is small by construction — one template per kind, five kinds. It
/// deliberately does NOT include `default_template_id`: the configured default
/// already reaches every client in its own `default_template` frame carrying its
/// own JSON, and the output page's resolver ends there anyway, so adding it here
/// would put the same bytes on the wire twice for no change in what is painted.
fn content_look_ids(conn: &rusqlite::Connection) -> Vec<i64> {
    CONTENT_LOOK_KINDS
        .iter()
        .filter_map(|k| db::content_template_id(conn, k).ok().flatten())
        .collect()
}

/// EVERY TEMPLATE ID A SCREEN CAN BE ASKED TO WEAR WITHOUT SHIPPING ITS BYTES.
///
/// Two sources, one list, and it has to be one list because the hub holds one
/// bound (`channels::MAX_LOOK_IDS`) and a client gets one hello reply:
///
///   * the FIVE global content looks (`content_look_ids`), §70;
///   * every per-kind look a screen carries (`db::channel_look_ids`), §97.
///
/// Both cross the wire as an ID and nothing else, for the same recorded reason
/// (`cue_or_content_tpl`, in megabytes), which means the bytes have to be at the
/// receiver before the id arrives — and a browser source has no database to look
/// them up in. This is what the hub is told to send.
///
/// The configured default is deliberately still absent, exactly as
/// `content_look_ids` records: it reaches every client in its own
/// `default_template` frame carrying its own JSON, so naming it here would put
/// the same bytes on the wire twice for no change in what is painted.
///
/// A failed read of the per-kind looks yields the content looks alone rather than
/// nothing. That is the honest degradation: the screens that follow the global
/// map still resolve, and a screen with a per-kind look falls back to its own
/// template — which is where it was before this feature existed.
fn resolvable_look_ids(conn: &rusqlite::Connection) -> Vec<i64> {
    let mut ids = content_look_ids(conn);
    for id in db::channel_look_ids(conn).unwrap_or_default() {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

#[cfg(test)]
mod media_url_tests {
    use super::*;

    /// A FILE THE OPERATOR IMPORTED IS SERVED BY ID; A PICTURE RELAY SHIPS IS
    /// SERVED OUT OF THE BUNDLE.
    ///
    /// The bundled rows have no file in the media directory at all — the bytes
    /// are inside the binary, in `dist/`, where the embedded server already
    /// serves them. Building `…/media/<id>` for one of those gives every screen
    /// a URL that 404s, and an image element that fails is a black wall with
    /// nothing in any log.
    #[test]
    fn a_bundled_picture_is_served_from_the_bundle_and_an_imported_one_by_id() {
        assert_eq!(
            media_url("10.0.0.5", 7, "/Users/x/media/7_photo.jpg"),
            "http://10.0.0.5:8032/media/7"
        );
        assert_eq!(
            media_url("10.0.0.5", 7, ""),
            "http://10.0.0.5:8032/media/7",
            "a row whose file has not been written yet is still served by id"
        );
        assert_eq!(
            media_url("10.0.0.5", 42, "bundled:backgrounds/01-2.jpg"),
            "http://10.0.0.5:8032/backgrounds/01-2.jpg"
        );
    }

    /// Nothing tries to unlink a picture that was never a file. `delete_media`'s
    /// caller passes the stored path straight to `remove_file`, and a bundled
    /// row's path is a marker, not a location.
    #[test]
    fn a_bundled_picture_has_no_file_to_delete() {
        assert!(!media_file_is_on_disk("bundled:backgrounds/01-2.jpg"));
        assert!(media_file_is_on_disk("/Users/x/media/7_photo.jpg"));
    }
}

#[cfg(test)]
mod content_look_kinds_tests {
    use super::*;

    /// THE THREE SHAPES OF ONE LIST MUST STILL AGREE.
    ///
    /// `CONTENT_LOOK_KINDS` is what `content_look_ids` iterates to tell the hub
    /// which templates a following screen may be asked to wear.
    /// `ContentTemplates` is the map an operator edits. A sixth kind added
    /// to the map alone is a look an operator can set, save, and never see: the
    /// fire path would resolve its id and the hub would never send the bytes, so
    /// the screen falls back to the configured default in silence — which is the
    /// defect this whole path was built to close, reintroduced one kind at a time.
    #[test]
    fn the_content_look_kinds_agree_with_the_map() {
        let map = serde_json::to_value(ContentTemplates {
            scripture: None,
            song: None,
            media: None,
            announce: None,
            countdown: None,
        })
        .expect("the content-look map serialises");
        let fields: Vec<&String> = map
            .as_object()
            .expect("an object")
            .keys()
            .collect::<Vec<_>>();
        assert_eq!(
            fields.len(),
            CONTENT_LOOK_KINDS.len(),
            "the map an operator edits and the list the hub is told about have              different lengths: {fields:?} vs {CONTENT_LOOK_KINDS:?}"
        );
        for kind in CONTENT_LOOK_KINDS {
            assert!(
                fields.iter().any(|f| f.as_str() == kind),
                "`{kind}` is iterated but is not a field of the map an operator edits"
            );
        }
        assert!(
            CONTENT_LOOK_KINDS.len() < channels::MAX_LOOK_IDS,
            "the five global content looks alone would fill a hello reply, so a \
             per-kind look could never be sent at all"
        );
    }
}

#[cfg(test)]
mod cue_or_content_tpl_tests {
    use super::*;

    #[test]
    fn a_kind_with_no_content_look_answers_with_the_configured_default() {
        // The console readout names the template a fire will wear. With no content
        // look set for this kind it said "none" — while the wall, since the default
        // now reaches it, wears the operator's default. Two surfaces, one fire, two
        // answers. The ID travels; the JSON deliberately does not (a default
        // carrying an embedded image has been 13 MB, and it used to be serialized
        // and broadcast on every single fire).
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        db::migrate(&conn, true).unwrap();
        db::set_setting(&conn, "default_template_id", "3").unwrap();
        let (id, json, pinned) = cue_or_content_tpl(&conn, None, "scripture");
        assert_eq!(id, Some(3));
        assert!(json.is_none(), "the default must never ship its JSON");
        assert!(!pinned, "a default is not a deliberate per-cue choice");
    }

    #[test]
    fn a_content_look_still_beats_the_configured_default() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        db::migrate(&conn, true).unwrap();
        db::set_setting(&conn, "default_template_id", "3").unwrap();
        db::set_content_template(&conn, "scripture", Some(5)).unwrap();
        let (id, _, _) = cue_or_content_tpl(&conn, None, "scripture");
        assert_eq!(id, Some(5));
    }
}

/// The default template ids mapped to each content type.
///
/// ── SITE 4 OF THE CONTENT-KIND SWEEP. NOTHING CHANGED HERE, AND WHY ──────────
///
/// Five hard-coded fields, and a kind missing from them gets no row in the
/// content-look UI — an operator can never choose its look, silently. The timer
/// registry adds no field: a congregation timer is still `countdown`, which is
/// already the fifth row, and a `Stage` timer has no congregation look to set.
/// The list here must stay in step with `CONTENT_KINDS` in `src/lib/layers.js`,
/// which is the canonical vocabulary; the two are mirrored by hand and no test
/// links them, so a sixth kind has to be written in both places.
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
fn set_content_template<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    kiosk: tauri::State<'_, channels::KioskHub>,
    kind: String,
    template_id: Option<i64>,
) -> error::Result<()> {
    // WRITE, THEN TELL THE SCREENS — and release the lock in between, because
    // nothing may hold a `Mutex` across a publish or an emit (rule 2).
    let (ids, fresh) = {
        let conn = db.0.lock()?;
        db::set_content_template(&conn, &kind, template_id)?;
        let fresh = template_id
            .and_then(|id| db::get_template(&conn, id).ok().flatten())
            .and_then(|t| serde_json::to_string(&t).ok().map(|j| (t.id, j)));
        (resolvable_look_ids(&conn), fresh)
    };
    // A LOOK CHANGED MID-SESSION IS NEWS, AND A SCREEN ALREADY OPEN HAS TO GET IT.
    //
    // A content look reaches an output as an id alone, so a screen can only wear
    // one it holds the bytes for. Warming the hub's list is what makes the NEXT
    // client resolve it; a screen that is already connected — the projector, the
    // OBS source, the lobby TV — would otherwise go on resolving the new id
    // against a cache that has never heard of it and silently paint the
    // configured default until something reloaded it. That is DECISIONS §70's own
    // finding ("staying silent leaves it wearing the look it was given") on the
    // other half of the pair.
    //
    // Both doors, and neither of them a new message: `KioskHub::set_template` is
    // the frame a browser source already applies, and `template://updated` is the
    // event a native output window already answers by re-reading that id. A screen
    // that does not care drops both, which is what they already do for every
    // template edit the operator makes.
    kiosk.cache_look_ids(&ids);
    if let Some((id, j)) = fresh {
        kiosk.set_template(id, &j);
        let _ = app.emit("template://updated", id);
    }
    Ok(())
}

/// Read a raw app setting by key (the generic KV store). Used by the frontend
/// for small, whole-set config blobs — the planned service length, the chosen
/// STT model, and so on. Returns None when the key was never set. This is a
/// general primitive on purpose: it is the offline-first, local-SQLite home for
/// frontend-owned config that does not warrant its own table.
#[tauri::command]
fn get_setting(db: tauri::State<'_, Db>, key: String) -> error::Result<Option<String>> {
    let conn = db.0.lock()?;
    db::get_setting(&conn, &key).map_err(Into::into)
}

/// Write a raw app setting (upsert). Counterpart to `get_setting`.
///
/// ## Why this one generic command knows about one key
///
/// `countdown.warn_ms` is not only a preference the console reads back: it is a
/// figure two publishers stamp onto frames bound for screens that cannot read a
/// setting at all (RG-149(c)). The mirror those publishers read
/// (`channels::CountdownWarnDefault`) has to be kept in step with the row, and this
/// is the ONE writer of the row — which is where the check goes, per rule 36. A
/// second, dedicated command would be a second door, and the door somebody forgets
/// is the shape of four separate bugs in this repository.
///
/// The write happens FIRST and the mirror follows, so a failed write cannot leave
/// the screens warning at a figure the next launch has never heard of.
// GENERIC OVER THE RUNTIME (rule 24) — it now reaches state that reaches a screen.
#[tauri::command]
fn set_setting<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    key: String,
    value: String,
) -> error::Result<()> {
    {
        let conn = db.0.lock()?;
        db::set_setting(&conn, &key, &value)?;
    }
    if key == "countdown.warn_ms" {
        if let Some(s) = app.try_state::<channels::CountdownWarnDefault>() {
            s.set(value.trim().parse::<i64>().ok());
        }
    }
    Ok(())
}

/// THE OPERATOR'S TRANSITION OVERRIDE — how the next thing appears, on every
/// screen (docs/REBRAND.md §8, DECISIONS §84).
///
/// `mode: None` clears it and every screen goes back to following its own
/// template, which is §71 untouched.
///
/// It is a plain `()` rather than a `Result` on purpose: there is nothing here
/// that can fail and nothing a congregation can be misled about. It reaches the
/// native windows through a Tauri emit and the kiosk/OBS sources through the hub,
/// and it changes no screen until the NEXT thing is put on one.
/// GENERIC OVER `tauri::Runtime`, like every other command that reaches a screen
/// (rule 24). A concrete `AppHandle` here would weld this control to the desktop
/// runtime, and the e2e test below — the one that checks a panic control is not
/// delayed by a transition — could not be written at all.
#[tauri::command]
fn set_live_transition<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    mode: Option<String>,
    ms: Option<u32>,
) {
    channels::transition(&app, mode, ms);
}

/// What override is in force right now, for the console to read back on mount.
///
/// The console can reload mid-service (a crash recovery, a devtools refresh) and
/// the override lives in the backend. Without this read the picker would come back
/// saying "Follow template" while every screen in the building was crossfading —
/// a control that reads the same when it is in force as when it is not (rule 35).
#[tauri::command]
fn live_transition(kiosk: tauri::State<'_, channels::KioskHub>) -> channels::TransitionOverride {
    kiosk.current_transition()
}

/// What picture is behind everything right now, for a screen that just opened.
///
/// The `live_transition` argument, on the second payload kind: the hub replays the
/// retained background on `hello`, and a native output window has the bridge and
/// no socket. Without this read, a projector opened mid-service is the one screen
/// in the building painting the words on black.
///
/// `[url, kind]` or null.
#[tauri::command]
fn live_background(kiosk: tauri::State<'_, channels::KioskHub>) -> Option<(String, String)> {
    kiosk.current_background()
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

// ===== THE DIAGNOSTIC BUNDLE (RG-12) =====

/// Write everything a support request needs to one file, and nothing else.
///
/// Composed as an ALLOW-LIST — every field named on the way in, never a list of
/// things to strip. `telemetry.rs` learned that the expensive way: its comment
/// promised an allow-list and its implementation was a blocklist that shipped every
/// field nobody had thought of. This file is the one artefact in Relay that is
/// *expected* to leave the building, so it gets the stricter of the two.
///
/// Present: versions, the machine, the model, ports, the current state, latency
/// percentiles, the screens by NAME and status, and the migration report. Absent:
/// every transcript, verse, lyric, announcement, service title, plan name, song
/// name, template name and media filename — and the home directory, which names a
/// person.
#[tauri::command]
fn export_diagnostics(app: tauri::AppHandle) -> error::Result<String> {
    use diagnostics::Fact;
    // Eight pieces of state, reached through the handle rather than taken as eight
    // parameters. A report ABOUT the whole app legitimately needs to see most of it,
    // and a signature that long is a signature nobody reads.
    let db = app.state::<Db>();
    let stt = app.state::<Stt>();
    let kiosk = app.state::<channels::KioskHub>();
    let health = app.state::<channels::OutputHealth>();
    let lock = app.state::<servicelock::ServiceLock>();
    let rehearsal = app.state::<channels::Rehearsal>();
    let detecting = app.state::<Detecting>();
    let hw = sysprobe::read(&db::app_data_dir());
    let report = latency::report(0);

    let mut relay = vec![
        Fact::new("Version", app.package_info().version.to_string()),
        Fact::new(
            "Build",
            if cfg!(debug_assertions) {
                "development"
            } else {
                "release"
            },
        ),
        Fact::new(
            "Ports",
            "5032 console (dev only) · 8031 websocket · 8032 http",
        ),
    ];
    {
        let conn = db.0.lock()?;
        let (version, expected, rows) = db::schema_report(&conn)?;
        let missing: Vec<&str> = rows
            .iter()
            .filter(|(_, _, present)| !present)
            .map(|(_, t, _)| *t)
            .collect();
        relay.push(Fact::new(
            "Database",
            format!("v{version} (this build expects v{expected})"),
        ));
        relay.push(Fact::new(
            "Schema objects",
            if missing.is_empty() {
                "all present".into()
            } else {
                format!("MISSING: {}", missing.join(", "))
            },
        ));
        // Where the gate sits, and what it is decaying TOWARD.
        //
        // Two different numbers and a church report needs both: "Relay stopped
        // firing this morning" reads completely differently depending on whether
        // the operator moved the dial or the self-calibration walked the bar up
        // from a run of dismissals. The baseline is the anchor (DECISIONS §26).
        if let Some(routing) = app.try_state::<Routing>() {
            if let Ok(r) = routing.0.lock() {
                let t = r.thresholds();
                let b = r.baseline();
                relay.push(Fact::new(
                    "Gate",
                    format!(
                        "auto-fire {:.2}, suggest {:.2} (baseline {:.2}, dial {})",
                        t.auto_fire,
                        t.suggest,
                        b.auto_fire,
                        b.to_sensitivity()
                    ),
                ));
            }
        }
        // WHICH MICROPHONE (RG-122). A bundle arrives with a sentence like "it did
        // not hear the preacher", and the first question is which input it was
        // listening to. Both candidates in the field on 2026-09-06 ran at 48 kHz,
        // so the rate line could not answer it and neither could anything else in
        // the bundle. Absent until capture has actually opened something, because
        // naming the default Relay never opened would be a confident lie.
        relay.push(Fact::new(
            "Microphone",
            match audio::last_input() {
                Some(input) => audio::describe_input(&input),
                None => "not opened yet this run".into(),
            },
        ));
        // Whether the display was being held awake. A church reporting "the
        // projector went black in the middle of the sermon" needs this line: it
        // separates a screen Relay let sleep from a screen that failed for some
        // other reason, and it distinguishes both from a machine that REFUSED the
        // assertion — which is a third thing and looks identical from the room.
        relay.push(Fact::new(
            "Display kept awake",
            if wake::failed() {
                "NO — this machine refused the request".into()
            } else if wake::is_held() {
                "yes".to_string()
            } else {
                "not needed right now".into()
            },
        ));
        // Whether an update is mid-flight, which is the first question when a
        // machine started misbehaving after one. The version is a version; the
        // snapshot PATH is not included, because it is a path in someone's home.
        relay.push(Fact::new(
            "Pending update",
            match updates::pending(&conn).filter(|p| !p.from_version.is_empty()) {
                Some(p) => format!("started from {}", p.from_version),
                None => "none".into(),
            },
        ));
    }

    let machine = vec![
        Fact::new("Operating system", format!("{} · {}", hw.os, hw.arch)),
        Fact::new(
            "Processor",
            match hw.cores {
                Some(c) => format!("{c} threads"),
                None => "the OS would not report a thread count".into(),
            },
        ),
        Fact::new(
            "Memory",
            format!(
                "{:.1} GB free of {:.1} GB",
                hw.available_memory_bytes as f64 / 1e9,
                hw.total_memory_bytes as f64 / 1e9
            ),
        ),
        Fact::new(
            "Disk",
            format!("{:.1} GB free", hw.free_disk_bytes as f64 / 1e9),
        ),
        // A BUILD fact, not a hardware one. Naming the GPU in this machine next to
        // a CPU-only build would be the most convincing lie in the file.
        Fact::new(
            "GPU acceleration compiled in",
            if hw.gpu_backends.is_empty() {
                "none — CPU only".into()
            } else {
                hw.gpu_backends.join(", ")
            },
        ),
    ];

    let s = stt.0.lock()?;
    let speech = vec![
        Fact::new(
            "Speech model",
            match s.as_ref() {
                // The model's FILENAME, not its path: the path is in a home folder.
                Some(e) => e
                    .model_path()
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| "loaded".into()),
                None => "none loaded — Relay is not listening for verses".into(),
            },
        ),
        Fact::new(
            "Recognition language",
            s.as_ref()
                .and_then(|e| e.language())
                .unwrap_or_else(|| "auto-detect".into()),
        ),
        Fact::new(
            "Detection",
            if detecting.0.load(Ordering::Relaxed) {
                "armed"
            } else {
                "off"
            },
        ),
        Fact::new(
            "Rehearsal",
            if rehearsal.on() {
                "ON — nothing reaches a screen"
            } else {
                "off"
            },
        ),
        Fact::new(
            "Service lock",
            if lock.engaged() {
                "engaged"
            } else {
                "not engaged"
            },
        ),
    ];
    drop(s);

    let mut speed = vec![
        Fact::new("Measuring", if report.enabled { "on" } else { "off" }),
        Fact::new(
            "Transcript updates skipped",
            report.dropped_partials.to_string(),
        ),
        // RG-120. Without these two, an end-to-end stage with no samples is
        // unreadable in a bundle: nothing distinguishes "the AI never fired" from
        // "nothing was attached to paint what it fired". A real service reported
        // zero samples against three auto-fires for the second reason.
        Fact::new(
            "Verses no screen reported painting",
            report.fires_never_painted.to_string(),
        ),
        Fact::new(
            "Render reports that arrived too late",
            report.marks_after_close.to_string(),
        ),
    ];
    for m in &report.metrics {
        if m.samples == 0 {
            continue; // a stage never reached is an absence, not a zero
        }
        speed.push(Fact::new(
            // Leaked as a &'static str from the metric's own wire name, which is
            // already static in `latency.rs`.
            m.metric,
            format!(
                "n={} · p50 {} · p95 {} · worst {}",
                m.samples,
                ms(m.p50_ms),
                ms(m.p95_ms),
                ms(m.worst_ms)
            ),
        ));
    }

    // Screens by the operator's own NAME for them plus their state. A name the
    // operator chose is their configuration, not the church's material.
    let screens = {
        let conn = db.0.lock()?;
        let open = channels::open_channel_ids(&app);
        let clients = kiosk.clients_handle();
        db::list_output_channels(&conn)?
            .into_iter()
            .map(|c| {
                let painting = health.painting(c.id);
                let attached = match c.render_target.as_str() {
                    "native_window" => open.contains(&c.id),
                    "network_client" => true,
                    _ => false,
                };
                let viewers = c.template_id.map(|t| clients.count(t)).unwrap_or(0);
                Fact::new(
                    c.name,
                    format!(
                        "{} · {} · {}",
                        c.render_target,
                        if attached { "attached" } else { "not attached" },
                        if painting {
                            "responding".to_string()
                        } else {
                            format!("NOT responding ({viewers} connected)")
                        }
                    ),
                )
            })
            .collect::<Vec<_>>()
    };

    let body = diagnostics::compose(&[
        ("Relay", relay),
        ("This machine", machine),
        ("Speech and detection", speech),
        ("Speed", speed),
        ("Screens", screens),
    ]);
    let path = diagnostics::write_bundle(&body, &now_epoch_ms().to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod diagnostic_bundle_tests {
    use super::*;
    use crate::diagnostics::{compose, Fact};

    /// THE BUNDLE MAY NOT CARRY ANYTHING THAT BELONGS TO THE CHURCH.
    ///
    /// The one artefact in Relay that is EXPECTED to leave the building, so this is
    /// the test that matters most about it. It is written against the composed
    /// document rather than the field list, because the question is what a stranger
    /// reading the file can learn — not what somebody intended to put in it.
    #[test]
    fn nothing_of_the_churchs_reaches_the_file() {
        // A worst case: every field is fed something it must not repeat.
        let md = compose(&[
            (
                "Relay",
                vec![
                    Fact::new("Version", "0.1.0-4"),
                    Fact::new("Speech model", "ggml-base.bin"),
                ],
            ),
            (
                "Screens",
                vec![Fact::new(
                    "Main screen",
                    "native_window · attached · responding",
                )],
            ),
        ]);

        // The composer only ever emits what it is given, so what this really pins is
        // that the SHAPE cannot smuggle anything: no free-form tail, no dump of a
        // struct, no "and everything else".
        for forbidden in [
            "For God so loved",
            "Sunday Service",
            "Amazing Grace",
            "the car park is closed",
        ] {
            assert!(!md.contains(forbidden), "{forbidden:?} must never appear");
        }
        assert!(
            md.contains("ggml-base.bin"),
            "the model filename is diagnostic"
        );
        assert!(
            md.contains("Main screen"),
            "a screen's own name is the operator's configuration"
        );
    }

    /// A STAGE NEVER REACHED IS AN ABSENCE IN THE FILE TOO.
    ///
    /// `ms(None)` is the last hop of the rule `latency.rs` enforces in its histogram
    /// and `perf_samples` enforces in the schema. A "0ms" here would tell whoever
    /// reads this file that the fastest part of the pipeline was the part that never
    /// ran.
    #[test]
    fn an_unreached_stage_prints_a_dash_not_a_zero() {
        assert_eq!(ms(None), "—");
        assert_eq!(ms(Some(139.4)), "139ms");
        assert_eq!(
            ms(Some(0.0)),
            "0ms",
            "a measured zero is still a measurement"
        );
    }
}

/// Milliseconds, or an em dash. **A stage never reached is an absence, not a zero.**
fn ms(v: Option<f64>) -> String {
    v.map(|v| format!("{}ms", v.round() as i64))
        .unwrap_or_else(|| "—".into())
}

/// The state of Relay's African-language support, measured rather than asserted.
///
/// Derived from the data the binary actually ships, so the report cannot flatter
/// the product: the only way to improve a number here is to improve the table the
/// detector uses. `wer` is always null and `native_reviewed` always false, because
/// neither has ever happened — and reporting either as a score would be the single
/// most misleading thing in this product, since it is the moat.
#[tauri::command]
fn language_report() -> Vec<detection::LanguageReport> {
    detection::language_report()
}

// ===== ROOM PROFILES (RG-10) =====

/// Every room this church has set up.
#[tauri::command]
fn list_environments(db: tauri::State<'_, Db>) -> error::Result<Vec<db::Environment>> {
    let conn = db.0.lock()?;
    db::list_environments(&conn).map_err(Into::into)
}

/// Remember this room, or update the one already called that.
///
/// The settings blob is composed by the CONSOLE, not here: it is a snapshot of
/// choices the operator has already made through commands that each have their own
/// validation, and re-validating them in a second place is how the two get to
/// disagree about what is legal.
#[tauri::command]
fn save_environment(
    db: tauri::State<'_, Db>,
    name: String,
    settings_json: String,
    notes: String,
) -> error::Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(error::Error::refused("A room needs a name."));
    }
    // The blob is stored verbatim and handed back verbatim, so it must at least be
    // JSON — otherwise a corrupt row would silently fail to apply later, at the one
    // moment the operator is relying on it.
    if serde_json::from_str::<serde_json::Value>(&settings_json).is_err() {
        return Err(error::Error::refused("Those settings could not be saved."));
    }
    let conn = db.0.lock()?;
    let now = chrono_now();
    db::save_environment(&conn, name, &settings_json, notes.trim(), &now).map_err(Into::into)
}

/// Switch to a room. Returns its settings so the console can apply them.
///
/// **It applies nothing itself, deliberately.** Every setting in the blob already
/// has a command with its own contract — `set_stt_language`, `set_channel_display`,
/// `select_voice_profile` — and applying them here would be a second implementation
/// of each, with its own idea of what a failure means. The console drives them one
/// at a time and reports which ones did not take, so a room that half-applied says
/// so rather than reporting a success it did not achieve.
#[tauri::command]
fn use_environment(db: tauri::State<'_, Db>, id: i64) -> error::Result<db::Environment> {
    let conn = db.0.lock()?;
    db::set_active_environment(&conn, id)?;
    // Read back the ACTIVE one rather than the one asked for: if the id no longer
    // exists, `set_active_environment` marks nothing and this returns the refusal
    // instead of a row that would claim a room was applied when none was.
    db::active_environment(&conn)?
        .filter(|e| e.id == id)
        .ok_or_else(|| error::Error::refused("That room is no longer saved."))
}

#[tauri::command]
fn delete_environment(db: tauri::State<'_, Db>, id: i64) -> error::Result<()> {
    let conn = db.0.lock()?;
    db::delete_environment(&conn, id).map_err(Into::into)
}

/// An ISO-ish timestamp, without pulling in a date crate for one field.
fn chrono_now() -> String {
    let ms = now_epoch_ms();
    format!("{ms}")
}

// ===== UPDATE SAFETY (RG-06) =====

/// Is it safe to start an update right now?
///
/// The service lock's answer comes first and is separate: "not during a service" is
/// a different sentence from "not onto this database", and an operator needs to know
/// which one they are looking at.
#[tauri::command]
fn update_preflight(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
) -> error::Result<UpdateReadiness> {
    let free = sysprobe::read(&db::app_data_dir()).free_disk_bytes;
    let conn = db.0.lock()?;
    let p = updates::preflight(&conn, free);
    Ok(UpdateReadiness {
        ok: p.ok && !lock.engaged(),
        during_service: lock.engaged(),
        checks: p.checks,
    })
}

#[derive(serde::Serialize)]
struct UpdateReadiness {
    ok: bool,
    /// Reported separately from the checks: a service in progress is not a fault in
    /// the database, and telling an operator their database is unhealthy when the
    /// real answer is "wait twenty minutes" would send them debugging the wrong thing.
    during_service: bool,
    checks: Vec<updates::Check>,
}

/// Take the pre-update snapshot and record what we are updating from.
///
/// Called immediately before the download starts. Returns the snapshot's path so the
/// operator can be told, in the moment, that their history has been copied — which is
/// the difference between an update they will press and one they will not.
#[tauri::command]
fn update_begin(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    from_version: String,
) -> error::Result<String> {
    if lock.engaged() {
        return Err(error::Error::refused(
            "A service is being recorded. Relay will not update until it ends — an update restarts the app.",
        ));
    }
    let conn = db.0.lock()?;
    let p = updates::preflight(&conn, u64::MAX);
    if let Some(bad) = p.checks.iter().find(|c| c.state == "fail") {
        return Err(error::Error::refused(format!(
            "Relay will not update on top of this database yet: {}",
            bad.note
        )));
    }
    let path = updates::begin(&conn, &from_version)?;
    Ok(path.to_string_lossy().to_string())
}

/// Did the last update actually work? Asked once, on the launch after one.
#[tauri::command]
fn update_verify(
    db: tauri::State<'_, Db>,
    current_version: String,
) -> error::Result<updates::Verdict> {
    let conn = db.0.lock()?;
    Ok(updates::verify(&conn, &current_version))
}

/// The operator accepts the update — stop asking.
#[tauri::command]
fn update_accept(db: tauri::State<'_, Db>) -> error::Result<()> {
    let conn = db.0.lock()?;
    updates::clear(&conn).map_err(Into::into)
}

/// The operator wants their history back. Takes effect on the next launch.
///
/// A request, not an action, and the app says so — an operator who thinks it has
/// already happened will not restart, and will conclude Relay ignored them.
#[tauri::command]
fn update_restore(db: tauri::State<'_, Db>, snapshot: String) -> error::Result<()> {
    updates::request_restore(std::path::Path::new(&snapshot))
        .map_err(|e| error::Error::refused(e.to_string()))?;
    // Clear the pending record too: whatever happens next, this update has been
    // answered, and asking again after a restore would be asking about a database
    // that no longer exists.
    let conn = db.0.lock()?;
    updates::clear(&conn).map_err(Into::into)
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

/// Local interface addresses for sharing links. No network requests or settings changes.
#[tauri::command]
async fn network_addresses() -> Vec<sysprobe::NetworkAddress> {
    sysprobe::network_addresses()
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
                // Bounded since RG-84. FULL means the whisper worker has stopped
                // consuming — stuck in a decode, or on a model this machine cannot
                // run — and the queue would otherwise grow for as long as the
                // preacher kept talking. Shed, and count it: audio Relay never
                // heard is a worse thing than a shed partial, and both have to be
                // visible. DISCONNECTED is an engine that has been unloaded and is
                // not a gap in anything.
                if let Err(std::sync::mpsc::TrySendError::Full(_)) = tx.try_send(chunk.clone()) {
                    latency::note_dropped_audio();
                }
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
    // The lock is dropped before asking the OS to keep the display up, and the
    // decision is `refresh_wake`'s, not this function's — a microphone starting is
    // one of three reasons and none of them owns the answer.
    drop(slot);
    refresh_wake(&app);
    Ok(())
}

/// Stop the running capture, if any. Idempotent. Leaves the STT worker loaded.
#[tauri::command]
async fn stop_capture(app: tauri::AppHandle, audio: tauri::State<'_, Audio>) -> error::Result<()> {
    let mut slot = audio.0.lock()?;
    if let Some(engine) = slot.take() {
        engine.stop();
    }
    drop(slot);
    // Releasing matters as much as taking: a laptop that never sleeps again
    // because Relay was opened once is the reason this is tied to the three facts
    // and not to the process being alive.
    refresh_wake(&app);
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
fn stt_status(stt: tauri::State<'_, Stt>, db: tauri::State<'_, Db>) -> error::Result<StatusStt> {
    // Read the engine under its own lock and DROP it before touching the database.
    // Every other path here takes the database first (`set_stt_language`,
    // `load_stt_model`), so holding the engine lock across a database lock is the
    // one ordering that could meet them head-on.
    let loaded = {
        let slot = stt.0.lock()?;
        slot.as_ref()
            .map(|e| (e.model_path().display().to_string(), e.language()))
    };
    Ok(match loaded {
        Some((model, language)) => StatusStt {
            loaded: true,
            model: Some(model),
            language,
            install_dir: None,
        },
        // NO ENGINE IS NOT "NO LANGUAGE". Before a model is downloaded there is
        // nothing to ask, and the recognition language is still a real stored fact
        // — it lives on the active voice profile and is applied the moment an
        // engine exists. Reporting `None` here printed "Auto-detect" over a profile
        // that said English, on a fresh install, which is the whole shape of rule 35.
        None => StatusStt {
            loaded: false,
            model: None,
            language: {
                let conn = db.0.lock()?;
                db::active_voice_profile(&conn)?.and_then(|p| p.language)
            },
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
fn set_active_translation(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    lock.guard("set_active_translation")?;
    let conn = db.0.lock()?;
    db::set_setting(&conn, "active_translation", &id.to_string()).map_err(Into::into)
}

/// Set the STT language: a code ("yo"/"sw"/"ha"/"en"/…) or null for auto-detect
/// (code-switching). Tier-1 targets: Yoruba, Swahili, Hausa (CLAUDE.md).
///
/// IT WRITES TO THE ACTIVE VOICE PROFILE, and that is the whole point (RG-138).
/// For as long as this command existed it took no `Db` at all: it set a field on
/// the live engine and nothing else, so an operator who chose English lost it at
/// the next launch, silently — while `stt_status` read the engine back and made it
/// look sticky for the rest of the run. `docs/qa/RELAY_GAP.md` RG-116 names this
/// control as the mitigation for a real field failure (whisper's language election
/// wandered off English and cost a service on `ggml-small`), so the register named
/// a fix that did not survive a relaunch.
///
/// The language is already stored durably, once, on `voice_profiles.language` —
/// applied in `setup`, on a profile switch, and after a model reload. So this
/// writes there rather than adding a second key: two stores for one fact would
/// race at startup, and nothing would say which won.
///
/// NOT on `servicelock::PROTECTED`, deliberately. Its two nearest neighbours are —
/// `select_stt_model` unloads whisper and takes the ears away mid-sermon, and
/// `set_active_translation` changes the words on the wall. This does neither: it
/// sets a hint that the next decode window picks up, unloads nothing, and is undone
/// by choosing again. More to the point it is the REMEDY for a live failure rather
/// than the hazard — when auto-detect wanders mid-sermon (one real service went
/// en·yo·pt·sw·sv·ms; `stt.rs`) pinning the language is the operator's only lever,
/// and holding it back behind an unlock would be withholding the fix at the exact
/// moment it is needed.
#[tauri::command]
fn set_stt_language(
    stt: tauri::State<'_, Stt>,
    db: tauri::State<'_, Db>,
    language: Option<String>,
) -> error::Result<db::VoiceProfile> {
    // Persist first, then apply. A write that fails must not leave the engine
    // decoding in a language nothing remembers.
    let profile = {
        let conn = db.0.lock()?;
        db::set_active_profile_language(&conn, language.as_deref())?
            .ok_or_else(|| "no voice profile to store the recognition language on".to_string())?
    };
    // The FULL profile, not just the language: `apply_profile_to_stt` re-derives the
    // decoder-bias prompt for the language now chosen. Setting the language alone
    // would leave English book names biasing a Yorùbá sermon, which is the exact
    // thing that function's comment says pushes whisper away from the words we need.
    if let Some(e) = stt.0.lock()?.as_ref() {
        apply_profile_to_stt(e, &profile);
    }
    Ok(profile)
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
    confidence: Option<f32>,
    method: Option<String>,
) -> error::Result<Thresholds> {
    // The confidence of the SUGGESTION the operator accepted, and how it was
    // found. Both were known to the console and thrown away at the call site.
    //
    // R4-09: without them this re-parsed the reference STRING and fed that
    // parse's score to `record_feedback` as "the score the operator agreed
    // with". A canonical "Book C:V" always re-parses through the colon-pair
    // branch at the same number, for all 66 books — so the confirm arm of the
    // self-calibrating gate always learned the same constant, and because
    // `record_feedback` only corrects when `c < auto_fire` (0.50 at the default
    // dial, 0.90 at the most cautious) the correction never fired at all. Every
    // confirm was pure decay toward baseline. `router.rs`'s own unit test passed
    // because it calls `record_feedback` directly.
    //
    // Optional so the LAN remote and older callers still work; when absent the
    // re-parse is used and the old behaviour stands, which is honest rather than
    // silently better.

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
    // What the operator actually agreed with, when the console told us.
    //
    // Clamped, because this crosses the bridge: a value outside 0..1 is not a
    // confidence and must not be allowed to drag the gate anywhere.
    //
    // And ONLY when the suggestion could have auto-fired. A paraphrase's
    // "confidence" is a raw cosine — a distance in an arbitrary vector space, not
    // a probability (rule 10) — so feeding it into the auto-fire bar would be a
    // category error dressed as calibration. Confirming one still counts as a
    // confirmation; it just carries no number, which `record_feedback` already
    // handles.
    let accepted_method = method
        .as_deref()
        .map(detection::DetectionMethod::from_wire)
        .unwrap_or(detection::DetectionMethod::Direct);
    let confirmed_conf = match confidence {
        Some(c) if accepted_method.may_auto_fire() => Some(c.clamp(0.0, 1.0)),
        Some(_) => None,
        // No confidence supplied: fall back to the re-parse, which is what this
        // always did. Not better, but not a lie either.
        None => Some(m.confidence),
    };
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
            // …and for the same reason it names no screens. Nobody has said
            // which screens an AI suggestion belongs on; every screen is the
            // only honest answer.
            None,
        ) {
            // Same wording as `manual_fire`'s, deliberately: it is the same
            // failure, and a volunteer should not have to learn two sentences for
            // one problem depending on which control they pressed.
            return Err(error::Error::not_found(format!(
                "{key} isn't in the Bible text — check the reference"
            )));
        }
        // ── THE OPERATOR'S DECISION, RECORDED ───────────────────────────────
        //
        // Accepting a suggestion fires through `fire_manual`, which records the
        // detection as `'manual'` — correct, and indistinguishable from a verse
        // typed into the box by hand. So the service record could say how many
        // verses a human put up and could NOT say how many of them were Relay's
        // idea, which is the one number that says whether the AI is earning its
        // place.
        //
        // A cue, not a `service_event`: `service_events` explicitly does not
        // duplicate what `cues` already holds, and `cues` is what the operator
        // pressed.
        //
        // Not during a rehearsal, for the same reason `record_feedback` is not —
        // a rehearsal is not evidence, and an acceptance rate inflated by
        // practice is worse than none.
        if !rehearsal.on() {
            persist_cue(&app, "suggestion_accepted", Some(&key));
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
        // AND SAY SO. This is the writer nobody presses: the gate moves on every
        // confirm and dismiss, so a console that read it once at launch drifted
        // stale on its own, with the operator touching nothing.
        thresholds_changed(&app, t);
    }
    Ok(t)
}

/// Operator rejected an auto-fired detection (undo). Tightens the gate,
/// persists the nudge onto the active profile, and — since the rejection is the
/// half of the acceptance rate that was invisible — records that it happened.
///
/// ## Why a rejection had no record at all
///
/// `detections.status` permits `'suggested'` and `'dismissed'`, `db/services.rs`
/// documents all four, and `service_timeline` reads them — but **the only
/// production insert runs inside `persist_fire`, which is called only for a fire
/// that reaches a screen.** So in a real service the column can only ever hold
/// `'auto'` or `'manual'`, and two of its four documented values were structurally
/// unreachable. The Sunday report counted them anyway and reported **0**, which
/// reads as *"Relay never offered you anything"* rather than *"nothing records
/// that"* — the exact inversion DECISIONS §44 exists to forbid.
///
/// **Persisting every suggestion is not the fix.** Suggestions are deliberately
/// not debounced (CLAUDE.md rule 28), so one spoken paraphrase yields a suggestion
/// on every decode pass — hundreds of rows a minute, none of which a person saw as
/// separate. What is bounded, meaningful and exactly what the metric needs is what
/// the OPERATOR did, so that is what is recorded, here and in `confirm_detection`.
///
/// `reference` is optional so the LAN remote and any older caller keep working —
/// the same precedent as `confidence`/`method` above. Absent, the rejection is
/// still counted; only the *which verse* is lost.
///
/// ## The reference is CANONICALISED, never stored as given
///
/// It arrives as a string across the bridge, and `cues.payload_json` is read back
/// by `service_timeline` — the part of the history most likely to be sent to
/// somebody for support. `db/services.rs` is explicit that what goes in there is
/// *"a short phrase Relay composes … never verse text, never a transcript"*, and
/// every other writer satisfies that by construction: `manual_fire` builds its key
/// from an already-parsed `VerseRef`, so it cannot be a sentence.
///
/// A raw string from the webview would be the first cue payload whose shape was
/// *trusted* rather than *guaranteed* — and "a future column could quietly widen
/// this" is the exact risk the two-sided privacy tests exist for. So the reference
/// is parsed and the canonical `Book C:V` is stored; anything that does not parse
/// stores nothing at all, and the rejection is still counted.
#[tauri::command]
fn dismiss_detection<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    routing: tauri::State<'_, Routing>,
    db: tauri::State<'_, Db>,
    rehearsal: tauri::State<'_, channels::Rehearsal>,
    reference: Option<String>,
) -> error::Result<Thresholds> {
    if !rehearsal.on() {
        let canonical = reference.as_deref().and_then(|r| {
            detection::detect_direct(r)
                .into_iter()
                .next()
                .map(|m| pipeline::Fire::key_for(&m.reference))
        });
        persist_cue(&app, "suggestion_dismissed", canonical.as_deref());
    }
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
        // AND SAY SO. This is the writer nobody presses: the gate moves on every
        // confirm and dismiss, so a console that read it once at launch drifted
        // stale on its own, with the operator touching nothing.
        thresholds_changed(&app, t);
    }
    Ok(t)
}

/// ENDING A REHEARSAL ENDS THE TIMERS IT STARTED, AND THEN TELLS THE TABLET —
/// RG-150, the operator decision of 2026-09-17.
///
/// A rehearsal is a sandbox in every other respect: nothing it publishes reaches a
/// screen. A clock it started is not an exception. The alternative considered and
/// rejected was to republish the set on the way out on the grounds the timers were
/// real all along — which means an operator who practises a twenty-minute sermon
/// clock at ten o'clock finds it on the preacher's tablet when the service starts,
/// counting toward a moment that has passed.
///
/// **STOP, THEN PUBLISH, IN THAT ORDER.** The tablet is never shown a set that is
/// about to change. Publishing first would put the rehearsal's clock on the
/// preacher's screen for exactly as long as it takes to take it off again, which is
/// a flicker nobody would ever reproduce on purpose.
///
/// The registry decides by the timer's own stamp and is asked nothing else
/// (`timers::TimerRegistry::stop_started_in_rehearsal`) — a control that has to ask
/// a question can fail to answer it, which is why the panic controls split by
/// `Scope` rather than by what a screen is showing. **A timer started BEFORE the
/// rehearsal began survives**, deliberately and with its own test: it was never a
/// rehearsal's timer.
///
/// `publish_timers` runs unconditionally, not "if anything was taken". It sends the
/// whole stage-visible SET, so it is idempotent and asks no question — and the
/// measured defect was precisely an exit that published `clear` and `stage_next`
/// and no `timer` frame at all, leaving the tablet's set and the registry to
/// disagree in silence until something unrelated republished
/// (`audits/DESIGN-2026-09-16-WAVE3.md` §6).
///
/// Called only with the rehearsal flag already flipped OFF, so the publish is a real
/// one rather than a suppression.
// GENERIC OVER THE RUNTIME (rule 24) — it reaches the screens, and `e2e.rs` drives it.
fn end_the_rehearsals_timers<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(reg) = app.try_state::<timers::TimerRegistry>() {
        // The registry takes and releases its own lock and returns an owned count,
        // so nothing is held across the publish below (rule 2).
        let stopped = reg.stop_started_in_rehearsal();
        if stopped > 0 {
            // Content-free: a count, never a label. An operator's timer name is
            // service data and this line goes to disk.
            println!("rehearsal: {stopped} timer(s) started in the rehearsal stopped");
        }
    }
    channels::publish_timers(app);
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
        if !on {
            end_the_rehearsals_timers(&app);
        }
        log_event(
            &app,
            if on {
                db::EventKind::RehearsalOn
            } else {
                db::EventKind::RehearsalOff
            },
            None,
        );
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

/// ── THE GATE MOVED. SAY SO. ─────────────────────────────────────────────────
///
/// `detection://thresholds` carries the whole of what every surface showing the
/// gate needs: the two thresholds and the dial position they map back to, through
/// `to_sensitivity` — the one inverse mapping, so a listener never re-derives it
/// and the two directions cannot drift.
///
/// WHY THIS EXISTS AT ALL. Until 2026-09-17 nothing in Rust announced a threshold
/// change, and the frontend had nothing to subscribe to. Live's dial was the one
/// setting in the shell held in a component-local `let`, read once at `onMount`,
/// and the dock is mounted OUTSIDE the workspace router — so unlike every view it
/// is never rebuilt and never re-read. Three things followed, all of them
/// measured rather than argued:
///
///   * Settings moved the gate and Live went on showing the old number, for the
///     rest of the session.
///   * Live moved the gate and Settings, holding the number it loaded when the tab
///     opened, SILENTLY REVERTED IT on the next profile save — `update_voice_profile`
///     compares the stale figure against an already-updated row, concludes the dial
///     moved, and re-derives from it.
///   * nobody had to touch anything at all: the router self-calibrates on every
///     confirm and dismiss, so the dock drifted stale on its own.
///
/// That is rule 35 on the one control governing what the AI may put on a wall
/// unasked — a reading that cannot tell "the engine says 50" from "nobody has
/// asked the engine since launch".
///
/// FIVE DOORS, ONE ANNOUNCEMENT. `Router::thresholds` is moved by
/// `apply_thresholds` (the dial and the two sliders), by `apply_profile` (a profile
/// saved, a profile selected, a room applied) and by `record_feedback` (the
/// learning, on every confirm and dismiss). A guarantee kept on four of five doors
/// is not a mitigation, it is the bug — this repository has shipped that shape four
/// separate times — so `thresholds_changed` is called from all five and
/// `hardrules.test.js` fails on a sixth writer that does not call it.
///
/// RULE 2. The router lock is released before this runs, at every call site. It is
/// never held across the emit.
fn thresholds_changed<R: tauri::Runtime>(app: &tauri::AppHandle<R>, t: Thresholds) {
    // An emit that fails is a console that will be one reading behind until the
    // next change. It is not worth failing an operator's action over, and there is
    // no second channel to report it down, so this is deliberately not a Result.
    let _ = app.emit(
        "detection://thresholds",
        serde_json::json!({
            "auto_fire": t.auto_fire,
            "suggest": t.suggest,
            "sensitivity": t.to_sensitivity(),
            // ON THE CURVE, OR MERELY NEAREST TO IT. `sensitivity` alone cannot
            // say which, and three of the five doors that move the gate move it to
            // somewhere the dial cannot reach — see `Thresholds::follows_dial`.
            // Without this field a surface showing the dial at 40 has no way to
            // tell the operator's own setting from the position the learning has
            // wandered nearest to, which is the defect the event itself exists to
            // fix, one level down.
            "on_dial": t.follows_dial(),
        }),
    );
}

/// Everything a surface needs to show the gate honestly, in one read.
///
/// The same four facts `detection://thresholds` carries, and deliberately the
/// same shape: the event is how a surface hears about a change, this is how it
/// starts out, and a surface that learned two different things from the two would
/// be the drift this pair exists to prevent.
///
/// `sensitivity` and `on_dial` are both computed in Rust because both are
/// questions about the one mapping — `to_sensitivity` and `from_sensitivity` live
/// here and nowhere else, and the frontend never holds a copy of the curve.
///
/// WHY THIS IS READ AT LAUNCH AND NOT ONLY LISTENED FOR. `setup` applies the
/// active profile's LEARNED gate before the window exists, so the one emit that
/// would have announced it has nobody to reach (the named exception in
/// `hardrules.test.js`). A console that only listened would therefore open with
/// the learned gate on screen, drawn at whatever dial position is nearest it, and
/// no caveat anywhere — which is the exact state this work was opened to fix.
#[derive(Serialize)]
struct GateReadout {
    auto_fire: f32,
    suggest: f32,
    sensitivity: u8,
    on_dial: bool,
}

/// The live gate: the two thresholds, the dial position they map back to, and
/// whether that dial position actually explains them.
#[tauri::command]
fn get_thresholds(routing: tauri::State<'_, Routing>) -> error::Result<GateReadout> {
    let t = routing.0.lock()?.thresholds();
    Ok(GateReadout {
        auto_fire: t.auto_fire,
        suggest: t.suggest,
        sensitivity: t.to_sensitivity(),
        on_dial: t.follows_dial(),
    })
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

/// Move the gate, the baseline, and the stored profile — together, always.
///
/// ── Why this is one function and one caller, and not three lines copied ─────
///
/// `sensitivity` is defined as the anchor the self-calibration decays back toward
/// (DECISIONS §26). Setting the gate without setting the anchor means every later
/// operator decision drags the gate back to the position they just left; writing
/// the thresholds to the profile without the dial leaves a row saying
/// `sensitivity = 50` beside an `auto_fire` that 50 could never produce — a state
/// the router was never in — and `apply_profile` re-anchors from that stale dial
/// at the next launch.
///
/// `set_sensitivity` got all of this right after it was caught in a live service.
/// `set_thresholds`, doing the same job from the other control, got none of it.
/// **A rule kept on one of two doors is this repository's most repeated bug**, so
/// the rule moved into the doorway both used.
///
/// THERE IS NOW ONE DOOR. `set_thresholds` was deleted with the two Settings
/// sliders it served: two controls over one fact cannot be reconciled by syncing,
/// because every sync makes one of them lie, and the dial is the control
/// DECISIONS §26 names and the one the self-calibration decays toward. This stays
/// a separate function from `set_sensitivity` regardless — `apply_profile` and a
/// room application reach the same three facts, and the next control to want them
/// must find the rule in a doorway rather than reconstruct it.
///
/// Returns the dial position that actually landed, recovered through
/// `to_sensitivity` — the one inverse mapping, so the two directions cannot drift.
fn apply_thresholds<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    routing: &tauri::State<'_, Routing>,
    db: &tauri::State<'_, Db>,
    t: Thresholds,
) -> error::Result<u8> {
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
    // The gate moved. Every surface that shows it hears about it here, not from
    // whichever control happened to move it — see `thresholds_changed`.
    thresholds_changed(app, t);
    Ok(landed)
}

/// The single operator "sensitivity" dial (0..=100), and the ONLY control that
/// sets the gate by hand. It maps through `from_sensitivity` — the one forward
/// mapping — so there is exactly one baseline. Returns the resulting dial
/// position so the caller can reflect what actually landed.
///
/// It is reached from two places, the dock's card on Live and Settings → AI &
/// Detection, and that is two doors onto one control rather than two controls:
/// one value, one store, one command, and `detection://thresholds` moves both the
/// moment either moves. The pair of Settings sliders that used to sit here were a
/// second control, pointing the opposite way, over a range the dial could not
/// express — see DECISIONS §96.
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
fn set_sensitivity<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    routing: tauri::State<'_, Routing>,
    db: tauri::State<'_, Db>,
    sensitivity: u8,
) -> error::Result<u8> {
    let t = Thresholds::from_sensitivity(sensitivity.min(100));
    apply_thresholds(&app, &routing, &db, t)
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
fn apply_profile<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    stt: &Stt,
    routing: &Routing,
    p: &db::VoiceProfile,
) -> error::Result<()> {
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
    let t = router.thresholds();
    drop(router); // rule 2 — never across an emit
                  // A profile switch and a room change move the gate as surely as the dial does,
                  // and until this line no surface displaying it was told. Switching preacher
                  // changed what may auto-fire unattended, in silence.
    thresholds_changed(app, t);
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
/// How many transcripts may be waiting for detection before back-pressure bites.
///
/// Small on purpose. Detection costs single-digit milliseconds against a decode
/// costing hundreds, so this queue should never hold more than one entry in
/// practice; its whole job is to be a bounded place for the exception rather than
/// an unbounded one. A queue that can grow without limit does not prevent
/// falling behind — it hides it, and then hides it for the rest of the service.
const DETECT_QUEUE: usize = 8;

/// Everything that happens BECAUSE of a transcript: the language-stability watch,
/// persistence, spoken commands, and reference detection.
///
/// ── Why this is not in the STT callback any more ────────────────────────────
///
/// It used to be, and that made the decoder wait for it. The callback ran on the
/// whisper worker thread, between one decode and the worker's next `recv()`, so
/// the semantic scan, three lock acquisitions, a DB write, the verse lookup, the
/// Tauri emit and the kiosk fan-out all sat directly on the cadence. Every
/// millisecond spent deciding what the LAST window said was a millisecond the
/// next window was not being decoded in — and on a fire it is not milliseconds:
/// `persist_fire` writes to SQLite and `broadcast_content` walks every connected
/// output, all before the worker may look at the microphone again.
///
/// That is a serial dependency between two things that have no reason to be
/// serial. The decoder's only job is to turn audio into text as fast as the
/// machine allows; deciding what the text MEANS can happen alongside the next
/// decode, and the answer is the same either way.
///
/// Order is still exact: one consumer thread, one queue, so a final can never
/// overtake the partial before it and a spoken "next" cannot be applied out of
/// sequence.
fn handle_transcript(
    handle: &tauri::AppHandle,
    lang_stability: &Mutex<stt::LanguageStability>,
    update: stt::TranscriptUpdate,
) {
    if update.is_final {
        // CONTENT-FREE. `stt.rs` states the rule a hundred lines away in this same
        // pipeline — "The transcript is sermon data and must never be logged" — and
        // this line printed the sermon, in full, once per final window. Every field
        // service so far was run from a terminal, so in each of them a real
        // congregation's preaching went to a console verbatim.
        //
        // The length is kept because it is the diagnostic anyone actually wanted
        // here (is the decoder returning anything?) and it says nothing about what
        // was said. The words go behind `RELAY_STT_TIMING`, the existing debug
        // switch `stt.rs` uses for exactly this purpose, so a developer chasing a
        // transcript bug can still have them by asking.
        //
        // This also protects the boot heartbeat: `greet` prints one line per launch
        // and its whole value is that the line is countable (rule 26). A stream
        // flooded with the sermon is one nobody can count.
        if std::env::var_os("RELAY_STT_TIMING").is_some() {
            println!("stt[{}]: {}", update.language, update.text);
        } else {
            println!(
                "stt[{}]: {} chars (set RELAY_STT_TIMING=1 for the text)",
                update.language,
                update.text.chars().count()
            );
        }
        // Compute under the lock, release, THEN emit — CLAUDE.md rule #2.
        let unstable = lang_stability
            .lock()
            .ok()
            .and_then(|mut s| s.observe(&update.language));
        if let Some(langs) = unstable {
            println!("stt: language auto-detect is unstable ({langs:?})");
            let _ = handle.emit("stt://language_unstable", langs);
        }
        persist_transcript(handle, &update.text, &update.language);
        // Spoken "next"/"back" navigates from the current verse.
        //
        // This runs off the operator's thread, with nobody to return a result to —
        // exactly like the spoken "clear the screen" below. So a nav that did
        // nothing is PUSHED to the operator rather than swallowed: the preacher
        // says "next", the wall does not move, and the console says why.
        if let Some(cmd) = detection::detect_command(&update.text) {
            announce_nav(handle, handle_nav(handle, cmd));
            latency::close(update.trace_id);
            return;
        }
        // Spoken "clear the screen" / "blackout".
        if detection::detect_clear(&update.text) {
            clear_or_report(handle);
            latency::close(update.trace_id);
            return;
        }
        // Spoken in-passage jump — "chapter 5 verse 1", "verse 4". Handled exactly
        // like the spoken next/back above it, because it fails in exactly the same
        // ways and the preacher has no console to look at either.
        if handle_passage_nav(handle, &update.text).is_some() {
            latency::close(update.trace_id);
            return;
        }
    }
    // Detect references, then route each through the confidence gate.
    //
    // The gate's clock is WALL TIME, not `update.timestamp_ms`. The audio
    // position advances in backlog-sized jumps and silently defeated the
    // repeat cooldown — see `router_clock_ms`.
    emit_detections(
        handle,
        &update.text,
        router_clock_ms(),
        update.is_final,
        Some(update.trace_id),
    );
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

    // KEEPING THE LATENCY EVIDENCE PAST THE END OF THE APP.
    //
    // `latency.rs` holds everything in memory, so the numbers from the run that
    // matters most — the one that ended badly — died when the church closed Relay.
    // A snapshot a minute, plus one at `end_service`, is enough to answer "did it
    // get worse over the service" from history rather than from a screen somebody
    // had to be looking at.
    //
    // Its OWN thread, deliberately not the detect thread (rule 33: the decoder
    // decodes, and the thread behind it decides what was said — neither is a place
    // to put a periodic chore) and not a timer on the frontend, which only ticks
    // while somebody has the Diagnostics tab open. It does nothing at all when no
    // service is recording, which is most of the time.
    let historian = handle.clone();
    if let Err(e) = std::thread::Builder::new()
        .name("relay-history".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            snapshot_latency(&historian);
        })
    {
        // Non-fatal, and said out loud: the service still runs, the live Diagnostics
        // screen still works, and only the after-the-fact record is missing.
        eprintln!("history: could not start the latency recorder ({e}) — live diagnostics still work, but this service will keep no latency history");
    }

    // The detection thread. See `handle_transcript` for why it is not the STT
    // thread. Bounded, so a stall here can never become unbounded memory growth
    // in the middle of a service.
    let (tx, rx) = std::sync::mpsc::sync_channel::<stt::TranscriptUpdate>(DETECT_QUEUE);
    let consumer = handle.clone();
    if let Err(e) = std::thread::Builder::new()
        .name("relay-detect".into())
        .spawn(move || {
            // Auto-detect re-elects a language every window and, on accented speech,
            // does not settle — which degrades the decode and looks exactly like the
            // AI being bad. Say so once, out loud, because the operator has the
            // control that fixes it and no reason to suspect they should touch it.
            // See `LanguageStability`.
            let lang_stability = Mutex::new(stt::LanguageStability::default());
            for update in rx {
                handle_transcript(&consumer, &lang_stability, update);
            }
        })
    {
        // A thread that will not spawn is not a reason to run deaf, but it IS a
        // reason to say so: without this consumer nothing is ever detected, and
        // silence here would look exactly like an AI that never hears anything.
        eprintln!("stt: could not start the detection thread ({e}) — no detection this session");
    }

    match SttEngine::try_load(path, move |update| {
        // The operator's eyes first. This is the cheapest thing on the path and
        // the only one they are waiting on, so it happens before the hand-off and
        // before anything decides what the words MEAN.
        let _ = handle.emit("stt://transcript", &update);
        let is_final = update.is_final;
        let trace = update.trace_id;
        match tx.try_send(update) {
            Ok(()) => {}
            Err(std::sync::mpsc::TrySendError::Full(update)) => {
                if is_final {
                    // A final carries persistence and the spoken commands, so it is
                    // never dropped — this blocks the decoder, which is the correct
                    // trade at the one point where dropping would lose something a
                    // partial cannot re-supply.
                    let _ = tx.send(update);
                } else {
                    // A partial is one revision of a window that will be decoded
                    // again in a moment, so dropping it loses nothing permanent —
                    // and dropping it is far better than stalling the decoder, which
                    // would make the very backlog that caused the drop worse.
                    // Counted, because silent shedding is how a pipeline gets to
                    // "fine" while missing half its work.
                    latency::note_dropped_partial();
                    latency::close(trace);
                }
            }
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                latency::close(trace);
            }
        }
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
    app.state::<servicelock::ServiceLock>()
        .guard("select_stt_model")?;
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
    // Rebuilding the engine takes the ears away for as long as whisper takes to
    // load, which on a big model is most of a paragraph.
    app.state::<servicelock::ServiceLock>()
        .guard("load_stt_model")?;
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
    // A 1.6 GB download over a church's broadband, started by a mis-click during a
    // sermon, competes with nothing else — but it does compete, and it cannot be
    // undone quickly.
    app.state::<servicelock::ServiceLock>()
        .guard("download_model")?;
    models::download(app, id).await.map_err(Into::into)
}

/// Install a speech model from a file this machine already has.
///
/// The half of offline installation that was missing: everything else a church
/// needs already works without a network — the app is an installer, the KJV is
/// compiled in — and the 148 MB model could only ever arrive over a connection they
/// do not have.
///
/// Held back during a service like every other model change (§40): copying 148 MB
/// and reloading whisper is exactly as disruptive from a USB stick as from the
/// internet.
#[tauri::command]
fn install_model_file(
    lock: tauri::State<'_, servicelock::ServiceLock>,
    path: String,
) -> error::Result<String> {
    lock.guard("install_model_file")?;
    models::install_from_file(std::path::Path::new(&path)).map_err(error::Error::refused)
}

/// Model files already sitting on this machine, waiting to be installed.
///
/// Read-only and cheap: three folders, no recursion, size as a pre-filter before
/// anything is hashed.
#[tauri::command]
fn find_model_files() -> Vec<models::FoundModel> {
    models::scan_for_models()
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
fn update_voice_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
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
        apply_profile(&app, &stt, &routing, &profile)?;
    }
    // `is_active` comes back from the DATABASE, so say so in what is returned. The
    // field arrived on the payload as whatever the frontend happened to be holding,
    // and the console now decides from this answer whether the recognition language
    // it shows on another tab has just changed underneath it (RG-138). Echoing the
    // caller's own guess back at it would be a second register for one fact.
    profile.is_active = is_active;
    Ok(profile)
}

/// Switch the active profile — applies its language + bias prompt + thresholds
/// immediately, before the next transcript window.
#[tauri::command]
fn select_voice_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
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
    apply_profile(&app, &stt, &routing, &profile)?;
    Ok(profile)
}

/// Delete a profile. If it was active, the next remaining profile becomes active
/// (a Default is re-seeded if it was the last) and is applied live.
#[tauri::command]
fn delete_voice_profile<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    stt: tauri::State<'_, Stt>,
    routing: tauri::State<'_, Routing>,
    db: tauri::State<'_, Db>,
    id: i64,
) -> error::Result<db::VoiceProfile> {
    lock.guard("delete_voice_profile")?;
    let profile = {
        let conn = db.0.lock()?;
        db::delete_voice_profile(&conn, id)?;
        db::active_voice_profile(&conn)?
            .ok_or_else(|| "no active profile after delete".to_string())?
    };
    apply_profile(&app, &stt, &routing, &profile)?;
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
    // WHICH SCREENS (RG-161), when a plan cue said so. `None` from the
    // operator's own reference box, which is every screen.
    channels: Option<Vec<i64>>,
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
        channels,
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
    // The screen's own answer, Option and all — see `channels::output_url`.
    let template_id = channel.template_id;
    // A REMEMBERED DISPLAY THAT IS GONE IS A REFUSAL, NOT A GUESS.
    //
    // This used to be `.and_then(parse_display)`, and a stale index simply fell
    // through the placement block inside `open_native_window`: the window was
    // built at its default position and then fullscreened, so the OS put it on
    // the primary display. Unplug the dock, press Open, and a borderless
    // undecorated fullscreen output covers the console the operator is running
    // the service from, with nothing reported. `auto_open_outputs` has always
    // skipped that case; this is the manual path agreeing with it, out loud.
    let monitor_index = match resolve_display(
        channel.display_target.as_deref(),
        &channels::list_monitors(&app),
    ) {
        DisplayChoice::On(idx) => Some(idx),
        DisplayChoice::Anywhere => None,
        DisplayChoice::Missing(n) => {
            return Err(error::Error::refused(format!(
                "{} is set to open on Display {n}, which is not connected. \
                 Plug it in, or choose a different display for this screen.",
                channel.name
            )))
        }
    };
    // Deterministic, so the window can be traced back to this channel — that is
    // what makes the channel's "online" light real. It also makes
    // `open_native_window`'s already-open check a duplicate guard: the counter
    // used to mint a fresh label each time, so opening one channel twice put two
    // fullscreen windows on the same projector.
    let label = channels::channel_label(channel_id);
    channels::open_native_window(&app, &label, template_id, &channel.name, monitor_index)?;
    refresh_wake(&app);
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
        // ONE RESOLVER, shared with `open_channel_output` (rule 36 in miniature:
        // the two paths that decide which physical screen an output lands on must
        // not be able to disagree). This half was already safe and is unchanged in
        // behaviour — `Anywhere` and `Missing` both skip here, because an
        // automatic open has no operator to refuse to.
        let idx = match resolve_display(c.display_target.as_deref(), &monitors) {
            DisplayChoice::On(idx) => idx,
            // No display assigned → not a fixed physical screen, and an unreadable
            // one is the same. Nothing auto-opens for either.
            DisplayChoice::Anywhere => continue,
            // That display isn't connected right now.
            DisplayChoice::Missing(_) => continue,
        };
        let Some(m) = monitors.iter().find(|m| m.index == idx) else {
            continue;
        };
        if m.primary {
            continue; // never cover the operator's console
        }
        let tid = c.template_id;
        let label = channels::channel_label(c.id);
        if channels::open_native_window(&app, &label, tid, &c.name, Some(idx)).is_ok() {
            opened.push(label);
        }
    }
    // THIS is the path that runs at every launch — `App.svelte` calls it on mount —
    // so it is the one that mattered most and the one that was missed. Outputs came
    // back by themselves after a restart and nothing told the OS to keep the display
    // up, which is the exact failure `wake.rs` exists to prevent.
    refresh_wake(&app);
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
    /// The screen's NAME, as the operator typed it.
    ///
    /// It is here because the shell's degraded banner had only the id and said
    /// "3 is not responding" — a number a volunteer cannot map to a screen while a
    /// congregation waits. `degraded.js` documented these as names for months; the
    /// producer sent ids, and no test could see the difference.
    name: String,
    online: bool,
    clients: usize,
    detail: String,
    /// False for a target Relay cannot drive at all (NDI is parked), so the UI can
    /// say "unavailable" rather than "offline" — a different claim.
    supported: bool,
    /// WHERE THIS SCREEN SAYS ITS CLIP IS — `None` when it said nothing.
    ///
    /// The console must never time a clip off its own preview: its programme pane
    /// renders through the same component, so it has a second player of the same
    /// file that buffers differently and carries on happily if the wall's copy
    /// stalls. An operator reading "0:12 left" while the congregation's screen is
    /// frozen at 2:30 is rule 35 exactly. So the figure comes from the screen that
    /// is painting, and `None` means the operator is told nobody said.
    media: Option<channels::MediaBeat>,
    /// The screen answered for itself within `channels::BEAT_STALE_MS`.
    ///
    /// This is the only field here that can tell a working screen from a frozen
    /// one. `online` says Relay is holding a window or serving a URL, and both stay
    /// true of a projector showing a dead renderer. `painting` is the screen's own
    /// claim, and it goes false by itself when the screen stops.
    painting: bool,
    /// Age of the last beat, in milliseconds. **`None` means the screen has never
    /// answered — an absence, not a zero** (`latency.rs` learned this the hard
    /// way), and the UI must say so rather than render it as a fresh beat.
    last_beat_ms: Option<u64>,
    /// What that beat said the screen was showing — `content` / `clear` / `black`.
    /// Parsed against a closed enum at the door; never free text off the LAN.
    paint_state: Option<&'static str>,
    /// THE OPERATOR TOOK THIS SCREEN OUT OF THE WALL — `clear` or `black`, and
    /// `None` when it is following the wall like every other screen.
    ///
    /// It is here rather than in a command of its own because every surface that
    /// describes a screen has to know it, and there is exactly one helper allowed
    /// to turn a row into words (`outputHealth.js::describeScreen`, rule 35).
    /// Without it that helper would compare Relay's belief — content is on the
    /// wall — against the screen's own beat, which says `clear`, and report
    /// `Not confirmed` for the rest of the service: a standing alarm about a
    /// screen doing exactly what it was told.
    down: Option<&'static str>,
}

/// WHICH PHYSICAL DISPLAY A SCREEN SHOULD OPEN ON — and whether it can at all.
///
/// Three answers, and the middle one did not exist.
///
/// `display_target` is an INDEX into the OS monitor list (see `parse_display`),
/// which is the honest shape of what Tauri exposes and is the reason this function
/// has to be careful. **Tauri 2.11 hands out `Monitor { name, size, position,
/// work_area, scale_factor }` and nothing else** — no native display id. Under it,
/// `tao` names a Windows monitor `\\.\DISPLAY1` (the `MONITORINFOEX.szDevice`
/// path, which the OS renumbers when displays are attached or detached) and a
/// macOS monitor `Monitor #<EDID model number>` (per MODEL, so two identical
/// projectors are indistinguishable). So there is no stable per-display identity
/// to store instead of the index — not on both platforms, and a scheme that worked
/// on one of them would make the control that decides which physical screen a
/// congregation sees behave differently on Windows and macOS. That is recorded in
/// full in `docs/DECISIONS.md`.
///
/// What CAN be fixed is the fallback, and it was the dangerous half.
/// `auto_open_outputs` has always skipped a channel whose index is not connected
/// — and skipped the primary display too, because auto-opening a borderless
/// fullscreen output over the console covers the UI the operator is running the
/// service from. `open_channel_output`, the **Open** button, did neither: a stale
/// index fell through the placement block, the window was built at its default
/// position and fullscreened, and the OS put it on the primary. The projector is
/// unplugged, the operator presses Open, and the congregation's output covers the
/// console.
///
/// `Missing` is that case and only that case: the operator named a screen, and
/// that screen is not here. An unreadable target and an empty monitor list are
/// BOTH `Anywhere` — the first is not a claim about a screen at all, and the
/// second is ambiguous (`list_monitors` returns an empty vector rather than
/// erroring, so a failed probe looks exactly like a machine with no displays).
/// Refusing on an ambiguity would turn a transient probe failure into an output
/// that cannot be opened, mid-service, with a sentence the operator cannot act on.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum DisplayChoice {
    /// Place it on this monitor index.
    On(usize),
    /// No preference recorded, or nothing that could be checked. Let the OS place
    /// it, which is exactly what has always happened when no display is assigned.
    Anywhere,
    /// The operator named a display (1-based, as a human reads it) and it is not
    /// connected. Refuse, and say which.
    Missing(usize),
}

fn resolve_display(target: Option<&str>, monitors: &[channels::MonitorInfo]) -> DisplayChoice {
    let Some(idx) = target.and_then(parse_display) else {
        return DisplayChoice::Anywhere;
    };
    if monitors.is_empty() {
        return DisplayChoice::Anywhere;
    }
    if monitors.iter().any(|m| m.index == idx) {
        return DisplayChoice::On(idx);
    }
    // 1-based, because that is how the picker and every OS display panel name it.
    DisplayChoice::Missing(idx + 1)
}

/// Whether the operator has taken this screen out of the wall, flattened for
/// `ChannelLiveness`. `None` is a screen following the wall — the ordinary case,
/// and the one that must be an absence rather than the word "live", so nothing
/// downstream can read "down" off a row that says it is up.
fn down_of(down: &channels::ScreensDown, id: i64) -> Option<&'static str> {
    down.get(id).map(|s| s.as_str())
}

/// The beat for one channel, flattened for `ChannelLiveness`.
fn beat_of(health: &channels::OutputHealth, id: i64) -> (Option<u64>, Option<&'static str>) {
    match health.read(id) {
        Some((age, state, _)) => (Some(age), Some(state.as_str())),
        None => (None, None),
    }
}

/// Record output loss and recovery edges into the service's timeline.
///
/// **An edge is only detected while a service is RECORDING, because that is the
/// only time it can be written down.** Two individually reasonable things are
/// wrong together (RG-136, field service 2026-09-13): `OutputHealth::transition`
/// CONSUMES the edge it reports — `reported` advances whether or not anything
/// records it — and `log_event` is a silent no-op with no service running. So a
/// screen already dead before the operator pressed record had its `output_lost`
/// computed, thrown away and marked as reported, and the matching recovery landed
/// in the service record with no partner. A report and a replay built on that
/// table then understate the outage count, and the one they lose is the one that
/// began before anybody was watching.
///
/// Outside a service every attached channel is FORGOTTEN rather than advanced, so
/// recording starts from a clean baseline: a screen that is already dead is
/// reported lost inside the service once the grace window has passed, and its
/// recovery has a partner. This deliberately does not try to back-date the lost
/// event into a service that had not started — an event cannot belong to a
/// service that did not exist, and inventing a time for it would be worse than
/// the gap it fills.
///
/// Split out of `channel_status` so it can be driven by a test: the command needs
/// a `KioskHub` and a webview to answer at all, and neither has anything to do
/// with the question this decides.
fn record_output_edges<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    health: &channels::OutputHealth,
    list: &[db::OutputChannel],
    open: &[i64],
    recording: bool,
) {
    for c in list {
        let attached = match c.render_target.as_str() {
            "native_window" => open.contains(&c.id),
            "network_client" => true,
            _ => false,
        };
        if !attached || !recording {
            // Not attached: neither "lost" nor "recovered" says anything true about
            // it, and a window the operator closed on purpose must not read as a
            // fault (RG-01's grace rule, one layer down).
            //
            // Not recording: the edge could be computed, but nothing could write it
            // down, and `transition` would consume it on the way to being dropped
            // (RG-136). Forgetting is what keeps the next service's first poll a
            // real first sighting rather than a continuation of a state no record
            // ever saw.
            health.forget_transition(c.id);
            continue;
        }
        if let Some(now_painting) = health.transition(c.id) {
            // A RECOVERY CARRIES THE SCREEN'S OWN ACCOUNT OF ITS SILENCE (RG-119).
            //
            // Relay cannot tell a screen that stopped painting from a heartbeat it
            // failed to keep: both look like no beat arriving. The page can, and
            // the beat that ends the outage is the only moment it can say so, so
            // that phrase is written into the timeline entry where an audit will
            // find it next to the times. Two durations and a channel name, no text
            // from the page (`BeatGap::describe`), so this stays inside the rule
            // that nothing a preacher said reaches the service record.
            let detail = match (
                now_painting,
                health.last_gap(c.id).and_then(|g| g.describe()),
            ) {
                (true, Some(said)) => format!("{} · {said}", c.name),
                _ => c.name.clone(),
            };
            log_event(
                app,
                if now_painting {
                    db::EventKind::OutputRecovered
                } else {
                    db::EventKind::OutputLost
                },
                Some(&detail),
            );
        }
    }
}

/// Live status for every channel. Polled by the Channels screen.
#[tauri::command]
fn channel_status(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    kiosk: tauri::State<'_, channels::KioskHub>,
    health: tauri::State<'_, channels::OutputHealth>,
    down: tauri::State<'_, channels::ScreensDown>,
) -> error::Result<Vec<ChannelLiveness>> {
    let list = {
        let conn = db.0.lock()?;
        db::list_output_channels(&conn)?
    };
    let open = channels::open_channel_ids(&app);
    let clients = kiosk.clients_handle();

    // A screen going quiet, and coming back, belong in the service's record — they
    // are exactly what an operator is trying to reconstruct afterwards ("the
    // projector was blank for a bit, when?"). This poll is the only regular tick on
    // this path, so it is the edge detector.
    //
    // Whether a service is RECORDING is read here and passed in, so the lock is
    // taken and released before `record_output_edges` runs: `log_event` locks
    // `Session` itself, and holding it across that call would deadlock the poll
    // against the service it is trying to write to. `Db` is already released
    // above, which keeps the global order of rule 6 (Db before Session).
    let recording = app
        .state::<Session>()
        .0
        .lock()
        .map(|s| s.is_some())
        .unwrap_or(false);
    record_output_edges(&app, &health, &list, &open, recording);

    Ok(list
        .into_iter()
        .map(|c| match c.render_target.as_str() {
            "native_window" => {
                // "The app is holding a window object" and "the projector is
                // showing something" are different claims, and only the first was
                // ever checked. A window whose webview has died or hung, or that
                // sits on a display which went to sleep, keeps `online` true
                // forever. So the window's own report decides the wording, and
                // where the two disagree that is stated rather than smoothed over.
                let online = open.contains(&c.id);
                let (age, state) = beat_of(&health, c.id);
                let painting = online && health.painting(c.id);
                ChannelLiveness {
                    id: c.id,
                    name: c.name.clone(),
                    online,
                    clients: 0,
                    detail: match (online, painting, age) {
                        (false, _, _) => "No output window open".into(),
                        (true, true, _) => "Output window open · screen responding".into(),
                        (true, false, None) => {
                            "Output window open · waiting for the screen to answer".into()
                        }
                        (true, false, Some(a)) => {
                            format!("Output window open · NOT responding for {}s", a / 1000)
                        }
                    },
                    supported: true,
                    media: health.media_of(c.id),
                    painting,
                    last_beat_ms: age,
                    paint_state: state,
                    down: down_of(&down, c.id),
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
                let (age, state) = beat_of(&health, c.id);
                let painting = health.painting(c.id);
                ChannelLiveness {
                    id: c.id,
                    name: c.name.clone(),
                    online: true,
                    clients: n,
                    // The viewer count answers "did a browser connect". The beat
                    // answers "is that browser still drawing", which is the actual
                    // question — and a connected-but-frozen source is precisely the
                    // case a count cannot see, because the socket stays open long
                    // after the page stops.
                    detail: match (painting, n, age) {
                        (true, 0, _) => "Serving · screen responding".into(),
                        (true, 1, _) => "Serving · 1 viewer · responding".into(),
                        (true, n, _) => format!("Serving · {n} viewers · responding"),
                        (false, 0, None) => "Serving · no viewer connected yet".into(),
                        (false, n, None) => {
                            format!("Serving · {n} connected · has never reported painting")
                        }
                        (false, 0, Some(a)) => {
                            format!("Serving · NOT responding for {}s", a / 1000)
                        }
                        (false, n, Some(a)) => {
                            format!("Serving · {n} connected · NOT responding for {}s", a / 1000)
                        }
                    },
                    supported: true,
                    media: health.media_of(c.id),
                    painting,
                    last_beat_ms: age,
                    paint_state: state,
                    down: down_of(&down, c.id),
                }
            }
            // NDI is parked, not broken — `open_ndi_output` says so too.
            "ndi_encode" => ChannelLiveness {
                id: c.id,
                name: c.name.clone(),
                online: false,
                clients: 0,
                detail: "NDI output is not available in this build".into(),
                supported: false,
                // A target Relay cannot drive reports nothing about a clip either.
                media: None,
                painting: false,
                last_beat_ms: None,
                paint_state: None,
                down: down_of(&down, c.id),
            },
            other => ChannelLiveness {
                id: c.id,
                name: c.name.clone(),
                online: false,
                clients: 0,
                detail: format!("Unknown render target '{other}'"),
                supported: false,
                // A target Relay cannot drive reports nothing about a clip either.
                media: None,
                painting: false,
                last_beat_ms: None,
                paint_state: None,
                down: down_of(&down, c.id),
            },
        })
        .collect())
}

/// A screen reporting that it is still painting.
///
/// The native output window's half of `channels::OutputHealth` — the kiosk half
/// arrives over the WebSocket. It is the same claim over a different transport, so
/// it deliberately carries the same closed `state` enum and nothing else: no
/// caption, no content, no identity.
///
/// Silent by design. It runs several times a minute for the length of a service,
/// and a print here would bury every other line in stdout (rule 4's lesson, one
/// layer up). Unlike `greet`, whose entire value is that it appears exactly once,
/// this one's value is that it never appears at all.
// EIGHT FLAT ARGUMENTS, AND FLAT ON PURPOSE.
//
// The WebSocket beat carries `media_pos_ms`, `media_dur_ms` and `media_paused` as
// three fields on one object, because that is what a JSON frame is. Bundling them
// into a struct here would make the native window's beat a different shape from
// the browser source's for no gain, and the whole point of `MediaBeat::clamped`
// beside `MediaBeat::from_json` is that one rule reads both transports. A window
// and a browser source must not be able to reach different conclusions about the
// same clip.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn output_beat(
    health: tauri::State<'_, channels::OutputHealth>,
    channel_id: i64,
    state: String,
    // What the page's own clock says about the gap before this beat. Optional on
    // purpose: a page that does not send them is silent about its silence, and
    // absent is the honest reading of that. See `channels::BeatGap` and RG-119.
    since_ms: Option<u64>,
    hidden_ms: Option<u64>,
    // WHERE THE CLIP IS, if this screen is playing one. Absent for every screen
    // showing a verse, and absent is the honest reading — see `channels::MediaBeat`
    // for why the console must never time a clip off its own preview instead.
    media_pos_ms: Option<u64>,
    media_dur_ms: Option<u64>,
    media_paused: Option<bool>,
) -> error::Result<()> {
    // An unparseable state is dropped, not defaulted. Defaulting would let a
    // malformed beat keep a dead screen looking alive, which is the exact failure
    // this whole mechanism exists to end.
    if let Some(st) = channels::PaintState::parse(&state) {
        health.beat(
            channel_id,
            st,
            "window",
            channels::BeatGap::clamped(since_ms, hidden_ms),
            channels::MediaBeat::clamped(media_pos_ms, media_dur_ms, media_paused),
        );
    }
    Ok(())
}

/// Close a channel's native output window, if it has one open.
#[tauri::command]
fn close_channel_output(
    app: tauri::AppHandle,
    health: tauri::State<'_, channels::OutputHealth>,
    channel_id: i64,
) -> error::Result<()> {
    // Forget the beat as well as the window. A channel reopened later must start
    // from "waiting for the screen to answer", not inherit the last beat of the
    // window that was deliberately closed — which would read as a screen that just
    // went silent, i.e. as a fault, immediately after the operator did something
    // completely normal.
    health.forget(channel_id);
    let r = channels::close_window(&app, &channels::channel_label(channel_id));
    // After, not before: the answer depends on whether any window is left, and
    // the window this closed has to be gone before that can be asked.
    refresh_wake(&app);
    r.map_err(Into::into)
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

/// SET (or clear) WHAT A SCREEN IS FOR, and tell every screen at once.
///
/// `role` is `main`, `stage`, or null for a screen with no special job. The
/// one-main rule is enforced in `db::set_channel_role` — one place, so the
/// console, the LAN remote and any future caller cannot disagree about it — and
/// what comes back is turned into a sentence here rather than relayed as a
/// constraint violation (`error.rs`, and `Channels.svelte`'s five monospace Rust
/// strings, are why).
///
/// BOTH DOORS, like every other piece of screen configuration in this file. A
/// native output window hears `output://channel_roles`; a kiosk/OBS browser
/// source gets the hub frame and picks out its own channel. A control wired to one
/// of the two is the mistake this repository has now made four times — and here it
/// would mean a projector and a browser source disagreeing about which of them may
/// be shown a word meant for the preacher.
///
/// Rule 2: the write and the read happen under the lock, which is released before
/// anything is emitted or published.
#[tauri::command]
fn set_channel_role<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    id: i64,
    role: Option<String>,
) -> error::Result<()> {
    let roles = {
        let conn = db.0.lock()?;
        match db::set_channel_role(&conn, id, role.as_deref())? {
            db::RoleOutcome::Set => {}
            db::RoleOutcome::MainTaken(name) => {
                return Err(error::Error::refused(format!(
                    "{name} is already the main screen. Clear its role first, or \
                     choose a different role for this one."
                )))
            }
            db::RoleOutcome::NotARole(r) => {
                return Err(error::Error::refused(format!(
                    "Relay has no screen role called \"{r}\"."
                )))
            }
        }
        db::channel_roles_json(&conn)?
    };
    publish_channel_roles(&app, &roles);
    Ok(())
}

/// The two doors, once. Called by every command that can change the role map.
///
/// The hub is reached through `try_state`, not taken as a `State` parameter: a
/// headless Relay manages no hub — that is the "no LAN" case `qa::bare_app`
/// deliberately reproduces — and a `State` argument panics there instead of
/// quietly doing nothing, which is what `channels::publish_kiosk` and
/// `channels::transition` already do for the same reason.
fn publish_channel_roles<R: tauri::Runtime>(app: &tauri::AppHandle<R>, roles_json: &str) {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(roles_json) {
        let _ = app.emit("output://channel_roles", serde_json::json!({ "roles": v }));
    }
    if let Some(hub) = app.try_state::<channels::KioskHub>() {
        hub.set_channel_roles(roles_json);
    }
}

/// Add an output channel. Returns its id.
#[tauri::command]
fn add_channel<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
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
    // WRITE AND READ UNDER ONE LOCK, RELEASE, THEN TELL THE HUB (rule 2).
    let (id, tjson) = {
        let conn = db.0.lock()?;
        let id = db::add_channel(&conn, name.trim(), &target, template_id.unwrap_or(1))?;
        let j = db::get_template(&conn, template_id.unwrap_or(1))?
            .and_then(|t| serde_json::to_string(&t).ok())
            .unwrap_or_else(|| "null".into());
        (id, j)
    };
    // THE NEW SCREEN'S LOOK, RETAINED BEFORE ANYTHING CAN OPEN IT. A screen added
    // during a service is opened seconds later, and the hub answers a hello out of
    // this map — so a screen created and never reassigned would have been the one
    // shape with no entry at all, which is the defect this map exists to close,
    // reintroduced through the create path. There is no publish: nothing is showing
    // this channel yet, and a broadcast about a screen nobody has opened is a frame
    // every other screen drops.
    //
    // `try_state`, not a `State` parameter, for the reason `publish_channel_roles`
    // records: a headless Relay manages no hub — the "no LAN" case `qa::bare_app`
    // deliberately reproduces — and a `State` argument panics there instead of
    // quietly doing nothing. This command IS driven headless, by
    // `qa::cold_start`, which is how that was found rather than reasoned.
    if let Some(hub) = app.try_state::<channels::KioskHub>() {
        hub.cache_channel_template(id, &tjson);
    }
    Ok(id)
}

/// RENAME A SCREEN. There was no way to do this at all.
///
/// The name is the only handle anybody in the building has on a screen. It is what
/// the Outputs cards are keyed by, what the degraded banner says when a screen
/// stops answering ("3 is not responding" was the defect that put the name on
/// `ChannelLiveness` in the first place), and what an operator says out loud to
/// somebody standing at the back. A church that inherits a Relay seeded with
/// `Lobby screen` and hangs it in the crèche instead had no way to say so.
///
/// **Not held by the service lock, and that is a decision rather than an
/// oversight.** `servicelock.rs` protects two things: the irreversible, and
/// anything that takes the engine away mid-sermon. A rename is neither — it is
/// reversible by doing it again, it moves no pixels, and the moment an operator
/// most wants it is the moment a screen's name turns out to be wrong, which is
/// during a service. Over-blocking is the more dangerous failure there.
///
/// The validation is here and only here, the same discipline `save_environment`
/// states: two layers that both validate are two layers that can disagree about
/// what is legal.
#[tauri::command]
fn rename_channel(db: tauri::State<'_, Db>, id: i64, name: String) -> error::Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(error::Error::refused("A screen needs a name."));
    }
    // A cap, because this string is rendered on a card, in a badge, in the shell's
    // degraded banner and in a service's own timeline — four places sized for a
    // name. It is generous enough that no real screen name reaches it, and the
    // refusal says the figure rather than silently truncating: a name quietly cut
    // in half is a name that stops matching what the operator typed.
    const MAX: usize = 60;
    if name.chars().count() > MAX {
        return Err(error::Error::refused(format!(
            "That name is too long for a screen — keep it under {MAX} characters."
        )));
    }
    let conn = db.0.lock()?;
    if !db::rename_channel(&conn, id, name)? {
        // Deleted on another surface between the card rendering and the rename
        // landing. Saying so beats showing the operator a name on a screen that is
        // not there any more.
        return Err(error::Error::refused(
            "That screen is no longer there — it may have been deleted.",
        ));
    }
    Ok(())
}

/// Delete an output channel.
#[tauri::command]
fn delete_channel<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    lock.guard("delete_channel")?;
    // Deleting the screen that held a role changes the role map, and a page that
    // is still open would otherwise keep the role of a channel that no longer
    // exists — which on a stage display means it keeps accepting stage messages
    // after the operator has deleted it.
    let (roles, looks, shows, ids) = {
        let conn = db.0.lock()?;
        db::delete_channel(&conn, id)?;
        (
            db::channel_roles_json(&conn)?,
            db::channel_looks_json(&conn)?,
            db::channel_shows_json(&conn)?,
            resolvable_look_ids(&conn),
        )
    };
    publish_channel_roles(&app, &roles);
    // …AND THE PER-KIND LOOKS THE DELETED SCREEN HELD. `channel_looks` cascades
    // on the foreign key, so the rows are already gone from the database — but a
    // page that is still open would go on holding a map naming a channel nobody
    // can be, and the console would go on listing per-kind looks for a screen that
    // is not there. The same argument as the role map one line up, on the map that
    // decides what a screen paints rather than what it accepts.
    publish_channel_looks(&app, &looks);
    publish_channel_shows(&app, &shows);
    if let Some(hub) = app.try_state::<channels::KioskHub>() {
        hub.cache_look_ids(&ids);
        // …AND WHAT THE DELETED SCREEN WORE. Nothing can be that channel any more,
        // so an entry left here is the hub answering for a screen that is not
        // there — harmless on its own, and exactly the stale row that makes a later
        // reader trust the map further than it should.
        hub.forget_channel_template(id);
    }
    Ok(())
}

/// All output templates (Templates tab, Channels tab).
#[tauri::command]
fn list_templates(db: tauri::State<'_, Db>) -> error::Result<Vec<db::Template>> {
    let conn = db.0.lock()?;
    db::list_templates(&conn).map_err(Into::into)
}

/// Delete a template (unassigns it from any channel first).
#[tauri::command]
fn delete_template(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    lock.guard("delete_template")?;
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
fn set_channel_template<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    kiosk: tauri::State<'_, channels::KioskHub>,
    id: i64,
    template_id: Option<i64>,
) -> error::Result<()> {
    // `None` means THIS SCREEN HAS NO LOOK OF ITS OWN and follows the content look
    // (DECISIONS §70). Until this was possible every screen always had a template,
    // and since a screen's own template wins over a content-type default (§29), the
    // content-look map could be filled in, saved, and do nothing on every screen in
    // the building.
    //
    // DB write + resolve the new template JSON under one lock, then release before
    // emitting (never hold a lock across emit — CLAUDE.md rule #2).
    let tjson = {
        let conn = db.0.lock()?;
        db::set_channel_template(&conn, id, template_id)?;
        match template_id {
            Some(tid) => db::get_template(&conn, tid)?.and_then(|t| serde_json::to_string(&t).ok()),
            None => None,
        }
    };
    match (template_id, tjson) {
        (Some(tid), Some(j)) => {
            if let Ok(tpl) = serde_json::from_str::<serde_json::Value>(&j) {
                let _ = app.emit(
                    "channel://retemplate",
                    serde_json::json!({ "channel": id, "template": tpl }),
                );
            }
            // THROUGH THE HUB, WHICH RETAINS IT. This used to be a bare
            // `kiosk.publish`, and that IS the defect the retained slot exists to
            // close: the frame reached whoever was connected at that instant and
            // nothing answered for the screen that connected a minute later, so a
            // reassignment survived exactly as long as nothing reloaded.
            kiosk.set_channel_template(id, &j);
            // Keep the hub's per-template cache current so a fresh kiosk connect on
            // this template id renders the up-to-date template too.
            kiosk.cache_template(tid, &j);
        }
        // CLEARING IS ALSO NEWS. A screen that is already open has to be told it is
        // now following the content look; staying silent leaves it wearing the look
        // it was given until something happens to reload it.
        (None, _) => {
            let _ = app.emit(
                "channel://retemplate",
                serde_json::json!({ "channel": id, "template": serde_json::Value::Null }),
            );
            // Retained as an explicit `null`, on the same argument: a screen that
            // reconnects has to be able to learn it is a FOLLOWER, and silence
            // cannot say that.
            kiosk.set_channel_template(id, "null");
        }
        // A template id that resolves to nothing: the row is written, and no screen
        // is told to paint something that could not be read.
        (Some(_), None) => {}
    }
    Ok(())
}

/// WHAT EVERY SCREEN WEARS FOR EVERY KIND — `{"1":{"scripture":9,"song":12}}`.
///
/// The console reads this once at boot and keeps it in a store; the output pages
/// get it from the hub (a browser source) or from this same command (a native
/// window, which has the bridge and no socket). One command, both readers, so
/// there is no second notion of what a screen is wearing.
#[tauri::command]
fn list_channel_looks(db: tauri::State<'_, Db>) -> error::Result<serde_json::Value> {
    let conn = db.0.lock()?;
    let raw = db::channel_looks_json(&conn)?;
    serde_json::from_str(&raw).map_err(|_| {
        // The map is built by `serde_json` two lines earlier, so this branch is
        // unreachable in a working build — and it is a REFUSAL rather than a
        // silent `{}` because the alternative is the console showing "no screen
        // has a per-kind look" over four screens that do, which is rule 35 on the
        // one surface an operator opens to check the setup.
        error::Error::refused("Relay could not read what the screens are wearing.")
    })
}

/// SET (or CLEAR, with `None`) WHAT ONE SCREEN WEARS FOR ONE KIND OF CONTENT.
///
/// `None` deletes the row, because no row is the only spelling of "this kind
/// inherits" (`db::ensure_channel_looks`). There is no second spelling, and a
/// look of NONE is NOT "this screen skips this kind" — that question is
/// `layout.shows`, it lives on the template, and answering it here would be
/// RG-161's per-cue targeting arriving through the back door with none of its
/// pieces. **A per-kind look changes what a screen WEARS, never whether it
/// PAINTS.**
///
/// **NOT HELD BY THE SERVICE LOCK, and that is a decision rather than an
/// oversight** — the same one `rename_channel` states, for the same two reasons.
/// `servicelock.rs` protects the irreversible and anything that takes the engine
/// away mid-sermon; this is neither. It is reversible by doing it again, and the
/// moment an operator most wants it is the moment a look turns out to be wrong,
/// which is during a service. Over-blocking is the more dangerous failure there.
///
/// BOTH DOORS, like every other piece of screen configuration in this file. A
/// native output window hears `output://channel_looks`; a kiosk/OBS browser
/// source gets the hub frame and picks out its own channel. A control wired to
/// one of the two is the mistake this repository has now made four times — and
/// here it would mean a projector on HDMI and the OBS source beside it wearing
/// different templates for the same verse, which is the whole failure this
/// feature exists to make impossible.
///
/// AND THE BYTES GO WITH THE ID. A look reaches a screen as a template id and no
/// JSON, so a screen can only wear one it already holds: `cache_look_ids` warms
/// what the NEXT client will be sent, and `set_template` / `template://updated`
/// hand the bytes to the ones already connected — neither of them a new message,
/// both of them things every open screen already answers. Without this, an
/// operator setting a look mid-service would watch the id arrive at a screen that
/// has never heard of it and silently paint the blanket template.
///
/// Rule 2: the writes and the reads happen under the lock, which is released
/// before anything is emitted or published.
#[tauri::command]
fn set_channel_look<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    kiosk: tauri::State<'_, channels::KioskHub>,
    channel_id: i64,
    kind: String,
    template_id: Option<i64>,
) -> error::Result<()> {
    if !CONTENT_LOOK_KINDS.contains(&kind.as_str()) {
        return Err(error::Error::refused(format!(
            "Relay has no kind of content called \"{kind}\"."
        )));
    }
    let (looks, ids, fresh) = {
        let conn = db.0.lock()?;
        db::set_channel_look(&conn, channel_id, &kind, template_id)?;
        let fresh = template_id
            .and_then(|id| db::get_template(&conn, id).ok().flatten())
            .and_then(|t| serde_json::to_string(&t).ok().map(|j| (t.id, j)));
        (
            db::channel_looks_json(&conn)?,
            resolvable_look_ids(&conn),
            fresh,
        )
    };
    kiosk.cache_look_ids(&ids);
    if let Some((id, j)) = fresh {
        kiosk.set_template(id, &j);
        let _ = app.emit("template://updated", id);
    }
    publish_channel_looks(&app, &looks);
    Ok(())
}

/// SET (or CLEAR, with `None`) WHICH KINDS A SCREEN SHOWS AT ALL (§98).
///
/// The operator's report was "timers still show on all screens, even the live
/// screen". Measured in a live install: of 44 templates, 40 list `countdown` in
/// `layout.shows` and the other four declare no `shows` key at all, which
/// `templateShows` reads as showing every kind. So every template in that install
/// painted the congregation countdown, and `shows` was not editable from any
/// surface — it existed in seed data and in `TemplateRender` and nowhere an
/// operator could reach.
///
/// **THIS IS A SECOND FACT ON A SECOND COLUMN, NOT A MEANING STRETCHED ONTO THE
/// FIRST.** A template's `shows` is the DESIGNER'S statement about what that
/// template can render — a lower third has no regions for a countdown and never
/// will. This is the OPERATOR'S statement about what this screen is for. They are
/// ANDed at the receiver, so this can only ever NARROW and can never force a
/// template to paint a kind it has no regions for, which would be a second
/// authority on a fact the template already owns.
///
/// `None` clears the column to SQL NULL and means "no opinion, follow the
/// template" — the behaviour every install has today, which is why there is no
/// back-fill and why a newly added channel is NULL rather than an explicit set.
/// An EMPTY list is a different thing and is stored as one: an operator who
/// unticks every kind has said this screen shows nothing.
///
/// **IT NARROWS CONTENT AND NOTHING ELSE.** `clear_screens` and `blackout` do not
/// pass through any of this, are never published from it, and never will be — a
/// screen an operator can accidentally configure out of a panic control is rule
/// 15's exact failure, and this is the precise shape it would take.
///
/// Not service-lock protected, for the same reason as `set_channel_look` and
/// `rename_channel`: reversible by doing it again, and the moment an operator most
/// wants it is when something is on a screen it should not be on, which is during
/// a service.
///
/// BOTH DOORS, in the same arms including the CLEARING arm — which is the half
/// `set_channel_template` already gets right and the half this kind of command
/// most often gets wrong, because clearing reads as "nothing to tell anybody".
#[tauri::command]
fn set_channel_shows<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    id: i64,
    kinds: Option<Vec<String>>,
) -> error::Result<()> {
    if let Some(list) = &kinds {
        for k in list {
            if !CONTENT_LOOK_KINDS.contains(&k.as_str()) {
                return Err(error::Error::refused(format!(
                    "Relay has no kind of content called \"{k}\"."
                )));
            }
        }
    }
    let shows = {
        let conn = db.0.lock()?;
        db::set_channel_shows(&conn, id, kinds.as_deref())?;
        db::channel_shows_json(&conn)?
    };
    publish_channel_shows(&app, &shows);
    Ok(())
}

/// The two doors, once. Called by every command that can change the `shows` map.
///
/// `try_state` rather than a `State` parameter, for the reason
/// `publish_channel_roles` records: a headless Relay manages no hub.
fn publish_channel_shows<R: tauri::Runtime>(app: &tauri::AppHandle<R>, shows_json: &str) {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(shows_json) {
        let _ = app.emit("output://channel_shows", serde_json::json!({ "shows": v }));
    }
    if let Some(hub) = app.try_state::<channels::KioskHub>() {
        hub.set_channel_shows(shows_json);
    }
}

/// The two doors, once. Called by every command that can change the look map.
///
/// The hub is reached through `try_state`, not taken as a `State` parameter, for
/// the reason `publish_channel_roles` records: a headless Relay manages no hub —
/// the "no LAN" case `qa::bare_app` deliberately reproduces — and a `State`
/// argument panics there instead of quietly doing nothing.
fn publish_channel_looks<R: tauri::Runtime>(app: &tauri::AppHandle<R>, looks_json: &str) {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(looks_json) {
        let _ = app.emit("output://channel_looks", serde_json::json!({ "looks": v }));
    }
    if let Some(hub) = app.try_state::<channels::KioskHub>() {
        hub.set_channel_looks(looks_json);
    }
}

/// Set (or clear, with `None`) the DEFAULT template — the last link in every
/// screen's resolution chain — and push the change live.
///
/// Changing the default used to be a bare `set_setting` from the frontend: it
/// was read at channel creation and by two console panes, and nothing else in
/// the building was told. A screen already following the content look kept the
/// old look until it was reopened, which is why the default "did not activate on
/// all screens". Native windows get `output://default_template`; kiosk/OBS
/// clients get the hub frame.
#[tauri::command]
fn set_default_template<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    kiosk: tauri::State<'_, channels::KioskHub>,
    template_id: Option<i64>,
) -> error::Result<()> {
    // DB write + resolve the JSON under one lock, release BEFORE emitting
    // (rule 2: never hold a Mutex across emit).
    let tjson = {
        let conn = db.0.lock()?;
        match template_id {
            Some(id) => {
                db::set_setting(&conn, "default_template_id", &id.to_string())?;
                db::get_template(&conn, id)?.and_then(|t| serde_json::to_string(&t).ok())
            }
            None => {
                db::set_setting(&conn, "default_template_id", "")?;
                None
            }
        }
    };
    let blob = tjson.unwrap_or_else(|| "null".into());
    kiosk.set_default_template(&blob);
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&blob) {
        let _ = app.emit(
            "output://default_template",
            serde_json::json!({ "template": v }),
        );
    }
    Ok(())
}

/// A WORD TO THE PREACHER: take over the stage monitor with one line of text.
///
/// Whitespace is not a message — a blank send CLEARS, which is also what the
/// Clear button does, so an operator who empties the box and presses Send gets
/// the obvious result rather than a red screen with nothing on it.
///
/// The line is capped. A stage monitor renders this at 8.5cqw across the whole
/// screen; a pasted paragraph is not a word to the preacher, it is a wall of type
/// nobody can read from a platform.
#[tauri::command]
fn send_stage_alert<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    text: Option<String>,
) -> error::Result<()> {
    const MAX: usize = 140;
    let line = text.unwrap_or_default();
    let line = line.trim();
    let msg = if line.is_empty() {
        None
    } else {
        Some(line.chars().take(MAX).collect::<String>())
    };
    channels::stage_alert(&app, msg);
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

/// ── ONE SCREEN, NOT THE WALL ───────────────────────────────────────────────
///
/// "Take the lobby TV down but leave the wall live" is an ordinary request. The
/// three commands below are the whole of it, and every one of them is a thin call
/// into `channels::set_screen_state` — the choke point (rule 36), so a fourth way
/// of taking a screen down cannot arrive with its own idea of what that means.
///
/// **`clear_screens` and `blackout` above are untouched.** They are the panic
/// controls: first, largest, reachable in one action, addressing every screen and
/// asking nothing (rule 15, DECISIONS §20). The split is in the CALL and never
/// inside them, because a panic control that has to work out which screen it is
/// addressing is a panic control that can fail to answer. Nothing here is bound
/// to `Esc` or to `B`, and `pipeline::preflight` gains no new power: these publish
/// no content, so there is nothing for a validator to refuse.
///
/// **They do not touch the wall's own state.** Not `LiveContent`, not the
/// debounce, not the congregation timers, not `WallState`. The verse is still in
/// front of the congregation on every other screen, and a control that forgot it
/// would make the next spoken "next verse" answer `NoPassage` — which is the
/// class of bug rule 40 and `NavResult` exist to prevent, arriving through a
/// side door.
#[tauri::command]
fn clear_screen<R: tauri::Runtime>(app: tauri::AppHandle<R>, channel_id: i64) -> error::Result<()> {
    channels::set_screen_state(&app, channel_id, channels::ScreenState::Clear)
        .map_err(error::Error::refused)
}

/// Blackout ONE screen (opaque black), leaving every other screen as it is.
#[tauri::command]
fn blackout_screen<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    channel_id: i64,
) -> error::Result<()> {
    channels::set_screen_state(&app, channel_id, channels::ScreenState::Black)
        .map_err(error::Error::refused)
}

/// Put one screen back into the wall: it shows whatever the wall is showing.
///
/// THE WAY BACK IS A CONTROL, not a side effect of the next fire. A screen taken
/// down stays down across every fire in between — a one-shot would be undone
/// within a minute of being used, which is to say useless for the thing it is for
/// — so there has to be something that undoes it, and it has to be as easy to
/// find as the control that did it.
#[tauri::command]
fn restore_screen<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    channel_id: i64,
) -> error::Result<()> {
    channels::set_screen_state(&app, channel_id, channels::ScreenState::Live)
        .map_err(error::Error::refused)
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
            // The one row in this history that somebody will go looking for. A
            // panic control that did not reach the screens is the worst thing
            // Relay can do quietly, and until now the only record of it was a
            // banner the operator dismissed.
            log_event(app, db::EventKind::PanicFailed, Some("clear"));
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
fn start_service<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session: tauri::State<'_, Session>,
    db: tauri::State<'_, Db>,
    rehearsal: tauri::State<'_, channels::Rehearsal>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
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
    // From here on the console is a live control surface, not an editing one.
    // Re-armed on EVERY start, so an override the operator made last Sunday does
    // not silently carry into this one.
    lock.arm();
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
    // First row of the timeline, at 0 ms, written while both locks are already
    // held rather than through `log_event` — which would deadlock on them.
    let _ = db::log_event(&conn, id, 0.0, db::EventKind::ServiceStarted, Some(&title));
    // Both locks go before the OS call. A service is recording: the display stays
    // up for its whole length, whether or not anyone touches the trackpad.
    drop(sess);
    drop(conn);
    refresh_wake(&app);
    Ok(id)
}

/// Stop recording the current service (history is kept).
#[tauri::command]
fn end_service<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session: tauri::State<'_, Session>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
) -> error::Result<()> {
    // ORDER MATTERS. Both of these read the session to find out which service they
    // belong to, so they run BEFORE it is cleared — the reverse silently wrote
    // nothing and the last minute of every service was the one minute never kept.
    snapshot_latency(&app);
    log_event(&app, db::EventKind::ServiceEnded, None);
    *session.0.lock()? = None;
    lock.release();
    // THE PROGRAMME IS OVER, SO THE PROGRAMME CLOCKS ARE.
    //
    // A `Stage` timer lives until something stops it, and until this landed nothing
    // ever did: a service's cue clocks stayed on the preacher's rail, counting past
    // zero, for as long as Relay was open (RG-163). `Live::retireCueTimer` ends each
    // cue's clock as the plan walks past it, which is the half that matters during a
    // service; this is the sweep behind it, at the one moment the whole programme is
    // finished. It is the choke point rather than the two controls that call
    // `end_service` (the dock, and the History list) — a rule kept at call sites is
    // the shape of four separate bugs in this repository.
    //
    // `Both` is untouched. A congregation countdown is on a wall and comes off it
    // through a panic control or through the operator; emptying it from here would
    // be a second door onto that screen, which `start_timer` already refuses to be.
    if let Some(reg) = app.try_state::<timers::TimerRegistry>() {
        reg.stop_scope(timers::Scope::Stage);
    }
    // The registry is read and dropped before this, per rule 2 — `stop_scope` takes
    // the lock, finishes and returns a count. Publishing is how a rail learns there
    // are none now: an absent frame cannot say that.
    channels::publish_timers(&app);
    refresh_wake(&app);
    Ok(())
}

/// Is the console currently protecting a service, and what is being held back?
///
/// The list rides with the flag so the UI can say what is unavailable without
/// keeping its own copy — a second list in the frontend is a second answer to one
/// question, and the two would drift.
#[derive(serde::Serialize)]
struct ServiceLockState {
    engaged: bool,
    held_back: Vec<&'static str>,
    /// Is a service row OPEN right now — the fact `end_service` acts on.
    ///
    /// This is deliberately NOT `engaged`. The lock is armed by `start_service`
    /// and released by `end_service`, so the two usually agree — but the operator
    /// can lift the lock in one action (`set_service_lock`, "operator override is
    /// a first-class control"), and after that `engaged` is false over a service
    /// that is still recording. A control that ended a service off `engaged`
    /// would read "nothing to end" at exactly that moment: a status control that
    /// cannot detect its own failure (CLAUDE.md rule 35).
    ///
    /// It reads the session directly, which is the same state `end_service`
    /// clears — so the button and the command can never disagree about whether
    /// there is a service. `current_service` was deleted as a dead command and
    /// this does NOT bring it back: no id, no title, no times cross the bridge,
    /// only whether one is open.
    recording: bool,
}

#[tauri::command]
fn service_lock(
    session: tauri::State<'_, Session>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
) -> ServiceLockState {
    ServiceLockState {
        engaged: lock.engaged(),
        held_back: servicelock::PROTECTED
            .iter()
            .map(|(_, what)| *what)
            .collect(),
        // A poisoned session lock is not a service. It is also not a reason to
        // fail a status read the whole shell polls — `is_ok_and` answers false
        // and the operator sees "no service" rather than a console that cannot
        // draw its own dock.
        recording: session.0.lock().is_ok_and(|s| s.is_some()),
    }
}

/// The operator lifts (or re-applies) the lock.
///
/// "Operator override is a first-class control, never a fallback UI" (CLAUDE.md).
/// The lock exists to catch an accident, not to overrule the person standing in the
/// room, so this takes no confirmation from Rust and gives no argument back. It is
/// scoped to the service it was made in: `start_service` re-arms.
#[tauri::command]
fn set_service_lock<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    on: bool,
) -> bool {
    let was = lock.engaged();
    lock.set(on);
    if was != on {
        // Recorded because it changes what the rest of the service was protected
        // from, and a replay that cannot see the override cannot explain what
        // happened after it.
        log_event(
            &app,
            if on {
                db::EventKind::LockRestored
            } else {
                db::EventKind::LockLifted
            },
            None,
        );
    }
    lock.engaged()
}

/// All services for the Library list, newest first.
#[tauri::command]
fn list_services(db: tauri::State<'_, Db>) -> error::Result<Vec<db::ServiceSummary>> {
    let conn = db.0.lock()?;
    db::list_services(&conn).map_err(Into::into)
}

/// Erase one recorded service — its transcript, its detections, its operator
/// actions, its timeline and its latency samples.
///
/// THE ONE THING RELAY COULD NOT DO WITH THE MOST SENSITIVE DATA IT HOLDS.
/// `transcripts.text` is verbatim text of what a preacher said to a congregation.
/// Every document here promises it never leaves the device, and none of them could
/// say how to remove it: `PRIVACY.md` answered with *"delete that folder"*, which
/// means quitting Relay and deleting every service ever recorded, from the Finder,
/// or keeping all of them. A church that wanted one sermon gone had no path.
///
/// Held back while a service is recording, like every other `delete_*` — including,
/// necessarily, the service being recorded right now (`servicelock::PROTECTED`).
#[tauri::command]
fn delete_service(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<i64> {
    lock.guard("delete_service")?;
    let conn = db.0.lock()?;
    db::delete_service(&conn, id).map_err(Into::into)
}

/// Everything that happened in one service, in order — the replay's spine.
///
/// Merged from three tables rather than kept in a fourth: `detections` is what the
/// AI claimed, `cues` is what the operator pressed, `service_events` is what Relay
/// observed about itself, and each row says which it is. Flattening that away is
/// how a replay starts to lie.
#[tauri::command]
fn service_timeline(db: tauri::State<'_, Db>, id: i64) -> error::Result<Vec<db::TimelineRow>> {
    let conn = db.0.lock()?;
    db::service_timeline(&conn, id).map_err(Into::into)
}

/// The latency snapshots kept for one service.
#[tauri::command]
fn service_perf(db: tauri::State<'_, Db>, id: i64) -> error::Result<Vec<db::PerfRow>> {
    let conn = db.0.lock()?;
    db::service_perf(&conn, id).map_err(Into::into)
}

/// One row per SERVICE for a metric, newest first — is it getting slower week by
/// week?
///
/// The question a single service cannot answer. A church that adds a bigger model,
/// or whose laptop fills up over a winter, degrades gradually and every individual
/// Sunday looks fine.
#[tauri::command]
fn perf_history(
    db: tauri::State<'_, Db>,
    metric: String,
    limit: Option<i64>,
) -> error::Result<Vec<db::PerfTrend>> {
    let conn = db.0.lock()?;
    // Capped: an unbounded limit from the frontend is a query nobody sized.
    db::perf_history(&conn, &metric, limit.unwrap_or(12).clamp(1, 52)).map_err(Into::into)
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

    // ── A REMEMBERED DISPLAY THAT IS GONE ──────────────────────────────────
    //
    // `display_target` is an INDEX into the OS monitor list, so unplugging a dock
    // renumbers it. `auto_open_outputs` has always been safe about that — it skips
    // a channel whose index is not connected, and skips the primary display too,
    // because auto-opening a fullscreen borderless window over the console covers
    // the very UI the operator needs.
    //
    // `open_channel_output` — the **Open** button, the path an operator presses
    // deliberately — was not. A stale index simply fell through the placement
    // block, and the window was built at its default position and then
    // fullscreened, which lands it on whatever monitor the OS chooses: normally
    // the primary. The projector is unplugged, the operator presses Open, and a
    // borderless undecorated fullscreen window covers the console they are running
    // the service from. Nothing reported anything.
    //
    // These hold the decision, as a pure function, so the refusal can be tested
    // without a window server.
    use super::{channels, resolve_display, DisplayChoice};

    fn mon(index: usize, primary: bool) -> channels::MonitorInfo {
        channels::MonitorInfo {
            index,
            name: format!("Display {}", index + 1),
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            scale: 1.0,
            primary,
        }
    }

    #[test]
    fn no_assigned_display_means_wherever_the_os_puts_it() {
        // An explicit "no preference". The operator never chose a screen, so
        // there is nothing to be stale and nothing to refuse.
        assert_eq!(
            resolve_display(None, &[mon(0, true)]),
            DisplayChoice::Anywhere
        );
    }

    #[test]
    fn an_assigned_display_that_is_connected_is_used() {
        assert_eq!(
            resolve_display(Some("1"), &[mon(0, true), mon(1, false)]),
            DisplayChoice::On(1)
        );
    }

    #[test]
    fn an_assigned_display_that_is_gone_is_refused_rather_than_guessed() {
        // THE WHOLE POINT. Falling through to "wherever" here is what puts a
        // fullscreen output over the operator's console when a dock is unplugged.
        // Refusing is the answer the automatic path already gives; this makes the
        // manual one agree with it, and say so.
        // BOTH FORMS, and the base conversion is the subtlety. A bare `2` is the
        // 0-BASED index `set_channel_display` writes, so the screen the operator
        // is looking for is `Display 3`; `Display 3` is the 1-BASED human label
        // the seed writes. Both name the same missing screen and both must report
        // it by the number an operator would read off their own OS.
        assert_eq!(
            resolve_display(Some("2"), &[mon(0, true), mon(1, false)]),
            DisplayChoice::Missing(3)
        );
        assert_eq!(
            resolve_display(Some("Display 3"), &[mon(0, true), mon(1, false)]),
            DisplayChoice::Missing(3)
        );
    }

    #[test]
    fn an_unreadable_target_is_no_preference_rather_than_a_missing_screen() {
        // A value nothing in Relay writes — a hand-edited row, or a form from an
        // older build. It is not a claim about a screen, so it must not produce a
        // refusal that names one; it means the same as nothing.
        assert_eq!(
            resolve_display(Some("HDMI-A-1"), &[mon(0, true)]),
            DisplayChoice::Anywhere
        );
    }

    #[test]
    fn a_display_list_that_could_not_be_read_at_all_does_not_refuse() {
        // `list_monitors` returns an empty vector rather than erroring, so "no
        // monitors" is ambiguous: a machine with none, or an enumeration that
        // failed. Refusing on an empty list would turn a transient probe failure
        // into an output that cannot be opened at all, mid-service, and the
        // operator has no way to act on that sentence.
        assert_eq!(resolve_display(Some("1"), &[]), DisplayChoice::Anywhere);
    }
}

#[cfg(test)]
mod import_guard_tests {
    use super::*;
    use base64::Engine as _;

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    /// An ordinary file still imports. The guard must not be a limit nobody can
    /// reach *downwards* either — a cap that rejects a 40 KB logo is a cap that
    /// gets deleted by the next person.
    #[test]
    fn a_normal_file_decodes_unchanged() {
        let payload = vec![7u8; 64 * 1024];
        let got = decode_import("logo.png", &b64(&payload)).expect("normal import");
        assert_eq!(got, payload);
    }

    /// THE BUG. There was no limit at all: the webview built the whole file as a
    /// base64 string, Tauri serialised it across the bridge, and this side decoded
    /// a second complete copy — so a service video killed the process rather than
    /// producing an error. Refusing has to happen BEFORE the decode allocation,
    /// which is why the check reads the base64 length.
    #[test]
    fn an_oversized_file_is_refused_and_never_decoded() {
        // One base64 character per byte is a 25% under-estimate of the decoded
        // size, so this is comfortably over the cap without allocating 256 MiB of
        // real payload in the test.
        let huge = "A".repeat(MAX_IMPORT_BYTES + (MAX_IMPORT_BYTES / 2));
        let err = decode_import("service.mp4", &huge).expect_err("must refuse");
        assert!(
            matches!(err, error::Error::Refused { .. }),
            "an oversized import is a refusal, not a fault: {err:?}"
        );
        let msg = err.message();
        assert!(msg.contains("service.mp4"), "names the file: {msg}");
        assert!(msg.contains("256"), "names the limit: {msg}");
    }

    /// Both import commands share one limit. Two copies of this number is how it
    /// comes to mean two different things.
    #[test]
    fn the_lyric_importer_is_behind_the_same_guard() {
        let huge = "A".repeat(MAX_IMPORT_BYTES * 2);
        assert!(decode_import("set.pro", &huge).is_err());
    }

    /// THE ORPHAN. The row is inserted before the file is written, because its id
    /// is half the on-disk name. When the write failed the row survived with an
    /// empty `path` — an asset that lists in the Library and plays nothing.
    ///
    /// The write is made to fail the way a full disk fails, by naming a directory
    /// that is not there.
    #[test]
    fn a_failed_write_leaves_no_media_row_behind() {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        conn.execute_batch(include_str!("../../docs/data/schema.sql"))
            .expect("schema");
        let id = db::insert_media(&conn, "video", "loop.mp4", "2026-09-02").expect("insert");
        assert_eq!(
            db::list_media(&conn).expect("list").len(),
            1,
            "the row exists before the write is attempted"
        );

        let nowhere = std::path::Path::new("/relay-no-such-directory-9f3a/media");
        let err = write_media_file(&conn, nowhere, id, "loop.mp4", b"x").expect_err("must fail");
        assert!(
            matches!(err, error::Error::Io { .. } | error::Error::Internal { .. }),
            "a write failure is a fault, not a refusal: {err:?}"
        );
        assert!(
            db::list_media(&conn).expect("list").is_empty(),
            "a file that never landed must not leave a Library entry"
        );
    }

    /// The happy path still writes, and still returns the path the row records.
    #[test]
    fn a_successful_write_returns_the_path_and_keeps_the_row() {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        conn.execute_batch(include_str!("../../docs/data/schema.sql"))
            .expect("schema");
        let id = db::insert_media(&conn, "image", "a b/c.png", "2026-09-02").expect("insert");
        let dir = std::env::temp_dir().join("relay-import-guard-test");
        std::fs::create_dir_all(&dir).expect("tmp dir");
        let path = write_media_file(&conn, &dir, id, "a b/c.png", b"hello").expect("write");
        assert!(
            path.ends_with(&format!("{id}_a_b_c.png")),
            "id-prefixed, sanitised name: {path}"
        );
        assert_eq!(std::fs::read(&path).expect("read back"), b"hello");
        assert_eq!(db::list_media(&conn).expect("list").len(), 1);
        let _ = std::fs::remove_file(&path);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RG-135 · THE TRANSLATION THE PREACHER NAMED
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod named_translation_gap_tests {
    use super::*;

    /// A fire carrying a translation, shaped the way `resolve_fire` leaves one.
    fn fire_showing(translation: Option<&str>) -> pipeline::Fire {
        pipeline::Fire {
            // A fixture names no screens: every screen.
            channels: None,
            key: "Hebrews 11:19".into(),
            reference: detection::VerseRef {
                book: "Hebrews".into(),
                chapter: 11,
                verse: 19,
            },
            verse_id: Some(1),
            text: Some("By faith Abraham, when he was tried".into()),
            translation: translation.map(|s| s.to_string()),
            named_translation_missing: None,
            confidence: 0.88,
            method: detection::DetectionMethod::Direct,
            status: pipeline::FireStatus::Auto,
            stage_note: None,
            next_reference: None,
            next_text: None,
            template_id: None,
            template_json: None,
            template_pinned: false,
            matched_text: None,
            trace_id: None,
        }
    }

    #[test]
    fn the_field_case_reports_the_translation_relay_does_not_have() {
        // FIELD-2026-09-13 §2. A fresh install has one translation, so the wall
        // showed KJV while the preacher read the Passion Translation aloud, and
        // the fire wore the same badge as the seven correct ones around it.
        let app = qa::bare_app();
        let db = app.state::<Db>();
        let conn = db.0.lock().expect("db");
        let fire = fire_showing(Some("KJV"));
        assert_eq!(
            named_translation_gap(
                &conn,
                "it says here in the passion translation hebrews 11 verse 19",
                &fire,
            ),
            Some("TPT".into()),
        );
    }

    #[test]
    fn it_says_nothing_when_the_wall_already_shows_what_was_named() {
        // The preacher says "King James" over a KJV wall. There is nothing to
        // report, and a caveat on a correct fire is how an operator learns to stop
        // reading the line this exists for.
        let app = qa::bare_app();
        let db = app.state::<Db>();
        let conn = db.0.lock().expect("db");
        let fire = fire_showing(Some("KJV"));
        assert_eq!(
            named_translation_gap(&conn, "turn with me, king james, hebrews 11", &fire),
            None,
        );
    }

    #[test]
    fn it_says_nothing_when_relay_actually_has_the_named_translation() {
        // A church that HAS added a translation and is simply not using it for this
        // fire is a different situation, and one an operator can see and change.
        let app = qa::bare_app();
        let db = app.state::<Db>();
        let conn = db.0.lock().expect("db");
        conn.execute(
            "INSERT INTO translations (name, abbreviation, language) VALUES (?1, ?2, ?3)",
            ("The Passion Translation", "TPT", "en"),
        )
        .expect("seed a second translation");
        let fire = fire_showing(Some("KJV"));
        assert_eq!(
            named_translation_gap(&conn, "in the passion translation, hebrews 11", &fire),
            None,
        );
    }

    #[test]
    fn an_ordinary_window_reports_nothing() {
        let app = qa::bare_app();
        let db = app.state::<Db>();
        let conn = db.0.lock().expect("db");
        let fire = fire_showing(Some("KJV"));
        assert_eq!(
            named_translation_gap(&conn, "turn with me to hebrews chapter eleven", &fire),
            None,
        );
    }
}
