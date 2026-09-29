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
mod mediaprobe;
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
/// **How many suggestions a real service produces, measured.** Test-only. The
/// schema decision in RG-309 rests on it, so it is a module rather than a script.
#[cfg(test)]
mod suggestions;
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
struct Semantic(std::sync::RwLock<SemanticIndex>);

/// The contiguous-phrase index. Built from the same corpus as `Semantic` and
/// answering the opposite question: not "which verse means this" but "which
/// verse did the preacher just READ ALOUD". See `detection::PhraseIndex`.
struct Phrases(std::sync::RwLock<detection::PhraseIndex>);

/// "Current passage" state for resolving bare verse references ("verse 4").
#[derive(Default)]
struct Context(Mutex<ContextMemory>);

/// Whether automatic detection is armed. Off = the pipeline still transcribes,
/// but no auto-fire/suggest reaches the console; manual override is unaffected
/// (it bypasses this entirely — a first-class control, CLAUDE.md).
struct Detecting(AtomicBool);

/// **MUST A PARAPHRASE ECHO THE VERSE BEFORE RELAY OFFERS IT?**
///
/// The church's switch over the paraphrase path: a contiguous run of
/// `detection::PARAPHRASE_RUN_WORDS` words shared with the verse, in the verse's
/// own order (DECISIONS §125, RG-311). **Off by default**, so nothing changes for a
/// church that never opens Settings.
///
/// ## Why it lives here and not on the `Router`
///
/// `follow_the_reader` is on the router because the router is what DECIDES it, and
/// `get_follow_the_reader` reads from the router for exactly that reason. This is
/// decided in `candidates_for_window`, before the gate, so putting it on the router
/// would mean a surface asking a thing that does not decide — and it would invite
/// the next reader to consume it inside `Router::decide`, where it would be a
/// fourth kind of cap over a method rule 10 already caps absolutely. `Detecting` is
/// the precedent: a persisted-or-not switch over what the detection path surfaces,
/// read in `emit_detections` and nowhere else.
///
/// An `AtomicBool` and not a `Mutex`: it is read once per window on the live path
/// and written by one command, so it must never be able to contend a lock with
/// anything the decoder is waiting on (rule 2).
struct ParaphraseRun(AtomicBool);

/// ONE SPEECH MODEL LOAD AT A TIME — RG-299.
///
/// `load_stt_model` costs **1,097 ms warm and about 3.6 s on a cold 1.6 GB read**
/// (`models::tests::what_the_model_flow_costs`), and until it carried this it ran
/// on the macOS window's run loop — which is a freeze an operator feels, and, per
/// rule 2, a run loop the STT worker's own `stt://transcript` emit can end up
/// waiting on.
///
/// **What the main thread was silently providing was mutual exclusion**, and that
/// is the whole reason this had to exist before `#[tauri::command(async)]` could:
/// two concurrent calls would each build a whisper context — **3.2 GB of resident
/// memory for two copies of `large-v3-turbo`** — and both would then write the same
/// `Stt` slot, so the engine the church ends up listening through is whichever race
/// finished second while the other 1.6 GB is dropped. The same objection settled
/// `install_model_file`'s shared `.part` path, and it is not an argument for leaving
/// it on the run loop; it is a thing to pay for first.
///
/// A second caller is REFUSED with a sentence, not queued: queueing would hide a
/// double-click behind a second full load, and the honest answer to "load it again
/// while it is loading" is that it is already loading.
///
/// `AtomicBool` with an RAII guard, following `models::RunningGuard` and the lesson
/// written above it: a bare `store(false)` on the way out leaves the flag stuck
/// `true` for the life of the process if the path is ever left early, and a stuck
/// flag turns one bad load into a feature that is dead until Relay is restarted.
#[derive(Default)]
struct ModelLoad(AtomicBool);

/// Clears `ModelLoad` however the load is left, including a panic.
struct ModelLoadGuard<'a>(&'a AtomicBool);

impl Drop for ModelLoadGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl ModelLoad {
    /// Claim the slot, or `None` if a load is already in flight.
    fn begin(&self) -> Option<ModelLoadGuard<'_>> {
        self.0
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| ModelLoadGuard(&self.0))
    }
}

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
    /// **THE LANGUAGE THE DECODER REPORTED FOR THE WINDOW BEING HANDLED — RG-113(5).**
    ///
    /// `persist_fire` writes an evidence transcript row when the detection came out
    /// of a window the last FINAL does not contain (the FIELD F-2 fix), and that
    /// insert carried a hardcoded `"en"` — on the product whose Tier-1 languages are
    /// Yorùbá, Swahili and Hausa and where code-switching is the normal case. Every
    /// one of those rows asserted English about speech nobody had established was
    /// English, and `service_transcripts` hands them to the replay.
    ///
    /// It lives beside `last_transcript` because it is the same kind of fact about
    /// the same row, and because a PARTIAL window never reaches
    /// `persist_transcript` at all — so the Session is the only thing that can carry
    /// its language to the one place that needs it. `handle_transcript` is the ONE
    /// writer, on `relay-detect`, in order, and it sets this from the same
    /// `TranscriptUpdate` it then hands to `emit_detections`.
    ///
    /// `"und"` (ISO 639-3, *undetermined*) until a decoder has said otherwise, and
    /// that is not a placeholder: a manual fire's evidence row is a reference the
    /// operator typed, and no language was spoken for it. Whisper never returns
    /// `und`, so a real answer and an absent one stay separable — which is the whole
    /// complaint against the `"en"` this replaces.
    last_language: String,
}

/// ISO 639-3 for *undetermined*. See `SessionState::last_language`.
const LANGUAGE_UNDETERMINED: &str = "und";

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
    /// THE SHAPE OF THE SOUND, not just how much of it there was.
    ///
    /// `audio::CHUNK_PEAKS` readings across this chunk's 400 ms, so the console
    /// can draw a real envelope instead of joining one level per delivery with a
    /// straight line. Peaks of the CLEANED stream, which is what the voice gate
    /// and whisper are given, so a peak at 1.0 here is the signal Relay is
    /// actually working from being clipped, after gain, and is worth a colour.
    peaks: Vec<f32>,
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
        // OFF until the row says otherwise. Overwritten in `setup` from
        // `app_settings`, below, before anything can be heard.
        .manage(ParaphraseRun(AtomicBool::new(false)))
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
        .manage(ModelLoad::default())
        .setup(|app| {
            // THE CLOCKS COME BACK BEFORE ANYTHING ELSE CAN TOUCH THEM (F28,
            // DECISIONS §112). Restored, not re-aired: nothing here reaches a
            // congregation screen. Then every later change is written on its own
            // thread, so the registry never holds the database lock.
            // ONE LINE NAMING THE BUILD, before anything else prints. The
            // heartbeat below stays exactly one line per launch (rule 26);
            // this is a different line with a different word.
            println!("relay: build {} (v{})", diagnostics::BUILD, env!("CARGO_PKG_VERSION"));
            restore_timers(&app.handle().clone(), cd_now_ms());
            {
                let h = app.handle().clone();
                let (tx, rx) = std::sync::mpsc::sync_channel::<(i64, Vec<timers::Timer>)>(64);
                let spawned = std::thread::Builder::new()
                    .name("relay-timers".into())
                    .spawn(move || {
                        for (next_id, set) in rx {
                            let db = h.state::<Db>();
                            let conn = match db.0.lock() {
                                Ok(c) => c,
                                Err(e) => e.into_inner(),
                            };
                            if let Err(e) = db::save_timers(&conn, next_id, &set) {
                                eprintln!("timers: could not save ({e}) — a relaunch will not have this change");
                            }
                        }
                    });
                match spawned {
                    Ok(_) => app.state::<timers::TimerRegistry>().set_sink(Box::new(
                        move |next_id, set| {
                            // A full queue drops THIS snapshot; the next change carries
                            // the whole registry again, so nothing is lost for long.
                            let _ = tx.try_send((next_id, set));
                        },
                    )),
                    Err(e) => eprintln!("timers: no persistence thread ({e}) — clocks will not survive a relaunch"),
                }
            }
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
                // ONE TRANSLATION (RG-50): the active one. Two in the index would
                // offer every quotation twice.
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
            app.manage(Phrases(std::sync::RwLock::new(
                detection::PhraseIndex::build(&corpus),
            )));
            app.manage(Semantic(std::sync::RwLock::new(SemanticIndex::build(
                &corpus,
            ))));
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
                let (profile, follow, needs_a_run) = {
                    let db = app.state::<Db>();
                    let conn = db.0.lock().expect("db lock");
                    (
                        db::active_voice_profile(&conn).ok().flatten(),
                        db::follow_the_reader(&conn),
                        db::paraphrase_needs_a_run(&conn),
                    )
                };
                // THE PARAPHRASE BAR, BEFORE THE FIRST WORD, for the same reason as
                // the reader switch below: a rule the operator set weeks ago must be
                // live from the first window, not from whenever a settings page
                // happens to be opened. Off unless the row says otherwise, so this
                // line is a no-op on every install that has never touched it.
                app.state::<ParaphraseRun>()
                    .0
                    .store(needs_a_run, Ordering::Relaxed);
                // THE CHURCH'S SWITCH, BEFORE THE FIRST WORD IS HEARD. It governs
                // what may reach a wall unattended, so it has to be on the router
                // by the time anything can be decided — not applied when a settings
                // page happens to be opened. Same reasoning as the learned gate two
                // lines down, and the same named exception to rule 35: there is no
                // webview yet to announce it to.
                if let Ok(mut r) = app.state::<Routing>().0.lock() {
                    r.set_follow_the_reader(follow);
                }
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
            start_timer,
            adjust_timer,
            reset_timer,
            list_stage_layouts,
            set_channel_stage_layout,
            upsert_stage_layout,
            delete_stage_layout,
            stop_timer,
            list_timers,
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
            build_marker,
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
            set_follow_the_reader,
            get_follow_the_reader,
            set_paraphrase_needs_a_run,
            get_paraphrase_needs_a_run,
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
            import_translation,
            delete_translation,
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
        .build(tauri::generate_context!())
        .expect("error while running Relay")
        .run(|app, event| {
            // THE ONE THING THAT HAPPENS ON THE WAY OUT (RG-269). `RunEvent::Exit`
            // is a CLEAN exit and nothing else: a crash, a force-quit or a power
            // cut never reaches here, which is exactly the line this rule wants
            // drawn. DECISIONS §112 promises the clocks survive a crash, and a
            // crash still keeps them.
            if let tauri::RunEvent::Exit = event {
                stop_clocks_a_relaunch_would_paint(app);
            }
        });
}

/// STOP THE CLOCKS A RELAUNCH WOULD PUT ON A SCREEN BY ITSELF — RG-269.
///
/// The operator: *"When Application close clear all active timer running or if not
/// its running it should display on the right output...stage"*.
///
/// `restore_timers` publishes to the stage unconditionally, because a programme
/// clock is the one thing that reaches a preacher's screen without a content frame.
/// So a running stage clock left in the registry at quitting time came back on that
/// screen at the next launch, counting from a moment that had passed, with nobody
/// having asked for it. `db::restorable` drops anything over six hours old, which
/// covers last Sunday and does nothing at all for this afternoon.
///
/// ## The rule, and the two things it deliberately leaves
///
/// **It takes a clock only if a relaunch would PAINT it and it would still be
/// counting.** That is one sentence and it decides both exemptions:
///
/// - **A HELD timer stays.** It was not running, so nothing about it goes stale: it
///   comes back at the figure somebody parked it at, which is the figure they
///   parked. Taking it would be taking a decision the operator made.
/// - **A congregation countdown (`Scope::Both`) stays.** §112 restores it to the
///   DESK and never to a wall — Live's Screen Countdown band offers it as
///   *counting, off the screens* behind **Put back on screens** — so it cannot
///   paint itself unasked, which is the whole harm here. It is also the clock a
///   church most wants back after a mid-service relaunch.
///
/// ## What this costs, stated rather than discovered
///
/// An UPDATE restart is a clean exit, so a church that updates mid-service loses the
/// sermon clock and has to press it again. That is the price, and it is one press of
/// a control the operator is already looking at, against a preacher's screen showing
/// a clock nobody started. Of §112's three motivating cases — a crash, an update, a
/// laptop closed and opened — only the middle one is a clean exit, and the other two
/// keep every clock exactly as they did.
///
/// Generic over the runtime (rule 24) so `e2e.rs` can drive the real thing.
fn stop_clocks_a_relaunch_would_paint<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> usize {
    let n = {
        let reg = app.state::<timers::TimerRegistry>();
        reg.stop_running(timers::Scope::Stage)
    };
    if n == 0 {
        return 0;
    }
    println!("timers: {n} running stage clock(s) stopped on exit — a relaunch will not paint them");
    // WRITTEN ON THIS THREAD, not through the sink. The sink hands its snapshot to
    // `relay-timers` and a process that is quitting does not wait for that thread
    // to drain, so the registry change would be lost and the next launch would
    // restore exactly the clock this just stopped.
    let (next_id, set) = {
        let reg = app.state::<timers::TimerRegistry>();
        reg.saveable()
    };
    {
        let db = app.state::<Db>();
        let conn = match db.0.lock() {
            Ok(c) => c,
            Err(e) => e.into_inner(),
        };
        if let Err(e) = db::save_timers(&conn, next_id, &set) {
            eprintln!("timers: could not write the stopped clocks ({e}) — the next launch may still have them");
        }
    }
    // AND TELL THE STAGE. A kiosk browser source on another machine outlives
    // Relay's own window and keeps rendering the last frame it was sent, so
    // without this the clock stays on the preacher's screen after Relay has gone.
    // No lock is held here (rule 2); both blocks above released theirs.
    channels::publish_timers(app);
    n
}

/// Minimum semantic cosine to even consider a paraphrase candidate. Below this
/// it's noise; above, the router's suggest/auto thresholds still apply.
///
/// This floor, not the length of the suggestion list, is what limits paraphrase
/// recall. Measured rather than remembered — this comment carried **98% and 84%**
/// from an older, smaller corpus and both were stale: against the 43 cases in
/// `data/paraphrase_corpus.json` today, `eval::paraphrase_scorecard` puts the right
/// passage in the top 5 for **100%** of retellings and
/// `eval::suggestion_policy_scorecard` shows **77%** surviving this cut (41% of the
/// modern-wording ones). Reproduce both rather than trusting this sentence:
/// `cargo test --release print_paraphrase_scorecard print_suggestion_policy -- --nocapture`.
///
/// ── THE NOISE IT COSTS IS NO LONGER UNMEASURED, AND THE ANSWER WAS NOT A NUMBER ──
///
/// This comment said the negative cases did not exist — *"transcript that mentions
/// no scripture at all"* — and asked that the floor not be lowered on a hunch. The
/// cases exist now: 14,158 final transcript lines across eleven of the author's own
/// services, 35.3 hours, of which **85.3% name no reference and hold no verbatim
/// run**. `suggestions::bar::paraphrase_bar` replays them through this path and the
/// real `Router`; 150 of those windows were then read by hand and judged.
///
/// **The false-positive rate at this floor is 74%** — 111 of 150 offers answer
/// speech that is not about the verse named, and 28 are plainly right. And the
/// measurement that decides this constant is not the rate but the ORDERING:
///
/// | policy                | offers removed | precision | recall | ALL | MODERN |
/// |-----------------------|---------------:|----------:|-------:|----:|-------:|
/// | **0.30 (shipped)**    |         **0%** | **18.7%** | **100%** | **77%** | **41%** |
/// | 0.40                  |          70.9% |     38.0% |  67.9% | 58% |     6% |
/// | 0.45                  |          84.8% |     56.7% |  60.7% | 51% |     0% |
/// | a 3-word shared run   |          73.5% |     50.0% |  78.6% | 70% |    24% |
///
/// *precision and recall are over the 150 hand-read windows; ALL and MODERN are
/// `para_cases()` recall, the retellings this path exists for.*
///
/// **RAISING IT IS REFUSED ON THE EVIDENCE, NOT ON CAUTION.** The two populations
/// have the same distribution — p50 cosine **0.356** in windows that name no
/// scripture against **0.361** in windows that do — so there is nothing for an
/// absolute to cut between, which is rule 12's shape one door along. Worse, the
/// ordering is inverted at both tails: the top-scoring false positives in the whole
/// sample are stock liturgical formulae (*"Hallelujah. Hallelujah. Praise the
/// Lord."* → `Psalms 146:1` at **0.557**; *"In the name of Jesus Christ"* four
/// times → `1 Corinthians 5:4` at **0.580**) and they outscore twenty-six of the
/// twenty-eight correct offers, whose bottom end is real citation (*"ten times
/// better than their colleagues"* → `Daniel 1:20` at **0.327**). A floor high
/// enough to silence the boilerplate silences the citations first. Pinned by
/// `suggestions::why_the_floor_holds`, which is the one part of that measurement
/// reproducible from the bundled KJV alone.
///
/// **What the data supports is not a bar on this number at all**: a contiguous run of
/// three words shared with the verse named beats every value of this constant on all
/// four columns above. **That rule now EXISTS, and it is deliberately not this
/// constant** — `detection::PARAPHRASE_RUN_WORDS`, applied in
/// `candidates_for_window`, behind the church's own switch
/// (`detection.paraphrase_needs_a_run`) and OFF by default.
///
/// It is a setting rather than a value of this number for two reasons that both still
/// hold. It is a different INSTRUMENT — a question about word order, where this is a
/// bar on a score — so folding it in here would give one gate two owners, which is
/// §96's whole subject. And it costs recall on exactly the case the product's claim
/// rests on: three of the 43 labelled retellings, every one `vocab: modern`
/// (`suggestions::what_the_bar_silences`, which names them and asserts the count).
/// Spending those is an operator's trade, so the operator makes it.
///
/// **This constant did not move and may not.** A church that has never opened that
/// switch is running the recall it ran before it existed — 77% ALL, 41% MODERN — and
/// that is what the switch defaulting off is for. So: do not raise it, and do not
/// lower it either. The number is not the lever.
const SEMANTIC_FLOOR: f32 = 0.30;

/// Most paraphrase alternatives to offer for one transcript chunk.
const SEMANTIC_SUGGESTIONS_MAX: usize = 3;

/// Keep an alternative only if it scores within this fraction of the best hit.
/// At 1.0 only ties survive (the old single-suggestion behaviour); lower widens
/// the list when scores are close. 0.60 measured +12 points of reachable recall
/// on modern-wording retellings for about one extra row.
const SEMANTIC_RELATIVE_FLOOR: f32 = 0.60;

/// How many quoted verses one window may offer. A preacher reading a passage
/// aloud quotes several verses in one breath, and the run surface is read in a
/// dark booth by a volunteer — so the list is capped where it stays readable.
const QUOTED_SUGGESTIONS_MAX: usize = 3;

/// An ordering number for a quoted run. NOT a probability, and never rendered as
/// a percentage (rule 18): it exists so `pipeline::better` can put a twelve-word
/// quotation above a five-word one, and for nothing else. The evidence a person
/// judges this by is the PHRASE, which is carried beside it.
fn quoted_confidence(run: usize) -> f32 {
    (0.60 + 0.03 * run.saturating_sub(detection::MIN_RUN_WORDS) as f32).min(0.95)
}

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

/// **WHAT COUNTS AS A PASSAGE WORTH TELLING THE OPERATOR ABOUT — RG-302.**
///
/// The ONE place that decision is made, called by every path that has a passage
/// end in its hand. Service 40 at 945 s: *"Proverbs 7, 1 to 5."* auto-fired
/// **Proverbs 7:1** and the console showed one verse with nothing to say that four
/// more had been asked for. `→` would have walked them, if anybody had known to
/// press it — which is the whole defect: an absence, not a wrong verse.
///
/// A span is reported only when it is LONGER than the anchor. `passage_end`
/// already answers `Some(end)` for a whole chapter and for an explicit range, and
/// both can legitimately land on the anchor itself — "Psalm 117" is a two-verse
/// chapter fired at verse 1, but "verse 5 to 5" and a one-verse chapter are not
/// passages and must not be announced as though the operator had something to
/// walk. Saying "there is more" when there is not is the same class of lie as a
/// status badge that cannot fail (rule 35), in the other direction.
fn span_to_report(anchor_verse: i64, end: Option<i64>) -> Option<i64> {
    end.filter(|e| *e > anchor_verse)
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
        // RG-302. Filled in by the caller, like `trace_id` and
        // `named_translation_missing` and for the same reason: how much was asked
        // for is a fact about the WINDOW (or about the operator's own typed
        // reference), not about the anchor verse this builder is handed.
        passage_end: None,
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
///
/// ## `gate_clock_ms` — ONE clock, named by the caller, never read twice
///
/// RG-321's dwell floor compares "when did the wall last change" with the `now_ms`
/// the router was handed, and **the two have to be the same clock or the difference
/// is not a duration.** The first version read `router_clock_ms()` here instead, and
/// in production that is the very same function the gate is given — so it was
/// correct, and correct for a reason no test could see. Anywhere the two readings
/// differed, the subtraction saturated to zero and the floor concluded *"the wall
/// changed this instant"*, which is the strongest possible hold, over a pair of
/// numbers that cannot be compared at all.
///
/// So the clock arrives as an argument. Every operator-driven caller passes a fresh
/// `router_clock_ms()`; the detect loop passes the reading its own gate decision was
/// made on. A new caller cannot forget, because it will not compile.
fn broadcast_with_clock<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    mut content: channels::OutputContent,
    gate_clock_ms: u64,
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
    // …AND THE INSTANT IT STARTED, so every screen can agree about where it is
    // (RG-220). Stamped here, at the one door content leaves by, for rule 36's
    // reason: a media path added next year carries it by construction and there
    // is no second call site to forget. Only for content that actually has a
    // clip — `media_started_at` on a verse would be a fact about nothing, and
    // `syncSeek` corrects nothing without a duration in any case.
    //
    // It is NOT overwritten if a caller has already set one: a replay or a
    // re-send of the same clip is the caller's decision to make, and this is the
    // default rather than an authority.
    if content.media_url.is_some() && content.media_started_at.is_none() {
        content.media_started_at = Some(now_epoch_ms());
    }
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
    // …AND WHAT IS ON THE SCREENS IS DECIDED HERE TOO, for the same reason and at
    // the same door. `Router::last_wall` is what the passage guard compares a
    // reading against (the passage guard, 2026-09-25), and it must record what
    // actually LEFT: rule
    // 29 lets one window auto-fire only its rank-0 candidate, so recording it inside
    // `Router::decide` — the first attempt — put verses on the record that were
    // demoted to suggestions and never shown. `Router::note_wall` carries the
    // measurement that found it.
    if content.kind.as_deref() != Some("scripture") {
        if let Some(ctx) = handle.try_state::<Context>() {
            if let Ok(mut c) = ctx.0.lock() {
                c.forget();
            }
        }
        // AND NO VERSE IS ON THE SCREENS ANY MORE, which is a different fact from
        // the one above and is why it needs its own line. `ContextMemory` is the
        // passage `→` resumes from; `Router::last_wall` is what the screens are
        // showing, and the passage guard refuses to re-fire a verse that is already
        // up. Leaving it set here would mean a song, then the preacher reading that
        // same verse again, and Relay declining to put it back — a screen held blank
        // by the guard, which is the failure `forget_last_fire` was written for.
        //
        // A SEPARATE `if let`, after the one above has dropped its guard: two locks
        // held at once on a path that also emits is how the Start-listening freeze
        // happened (rule 2, rule 6). Neither lock is needed while the other is.
        //
        // …AND THE WALL JUST CHANGED, WHICH IS A THIRD FACT (RG-321). Both branches
        // below stamp `gate_clock_ms` — the caller's reading, never a second one taken
        // here — because "how long has a congregation had this screen to itself" is a
        // question about a room and does not care what kind of content is on it. It is
        // stamped HERE, at the one door, for rule 36's reason and for one more: the
        // clock must start when the content actually left, not when the gate decided,
        // or a verse demoted by rule 29 would hold a wall it never reached.
        if let Some(routing) = handle.try_state::<Routing>() {
            if let Ok(mut r) = routing.0.lock() {
                r.forget_wall(gate_clock_ms);
            }
        }
    } else if let Some(routing) = handle.try_state::<Routing>() {
        if let Ok(mut r) = routing.0.lock() {
            r.note_wall(&content.reference, gate_clock_ms);
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
    // ONE reading for this whole fire — the repeat cooldown below and the wall's
    // dwell stamp at the door are then the same instant rather than two readings a
    // few microseconds apart (RG-321, `broadcast_with_clock`'s `gate_clock_ms`).
    let at_ms = router_clock_ms();

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
            // RG-302, and it is taken from the CONTEXT here rather than from the
            // `PassageUpdate` on purpose. `Note(end)` is only one of three arrivals:
            // a nav step is `Advance`, which keeps the span it is walking inside, so
            // an operator three verses into a five-verse reading must still be told
            // what they are inside of. Reading the staged span answers all three
            // with one line, and `Jump` correctly answers `None` because `note`
            // clears the span.
            f.passage_end = span_to_report(f.reference.verse, context.span_end());
        }
        if let Ok(mut router) = handle.state::<Routing>().0.lock() {
            // The same wall clock the AI path uses. This was a literal `0`, which
            // on any clock means "long ago" — so a verse the operator had just put
            // on the wall themselves was never protected from the AI immediately
            // re-firing it off the still-rolling STT window.
            router.manual_fire(&f.key, at_ms);
        }
        persist_fire(
            &conn,
            handle.state::<Session>(),
            f.verse_id,
            f.method.db_method(),
            f.confidence,
            f.status.as_str(),
            &f.key,
            // A manual fire reached a screen — it returned `false` above otherwise.
            Provenance::Fired,
        );
        f
    }; // locks released BEFORE the emit below — CLAUDE.md rule #2.

    // A refused payload must not be followed by a `detection://match` saying it
    // went out — that is the console reporting a success it did not achieve, in a
    // new place (DECISIONS §20). Bail before the event.
    if broadcast_with_clock(handle, fire.output(), at_ms).is_err() {
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
        // THREE TIERS, NOT TWO — see `pipeline::better`, which this must agree
        // with exactly or the dedup and the sort would disagree about which
        // evidence is stronger.
        (b.method.unattended_rank(), b.conf)
            .partial_cmp(&(a.method.unattended_rank(), a.conf))
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

/// What this window found and did NOT offer, because the preacher is reading the
/// passage already on the screen. Carried to the console so a held candidate is
/// visible somewhere (rule 35) rather than simply absent.
#[derive(Serialize, Clone)]
struct HeldCandidate {
    reference: String,
    method: DetectionMethod,
    /// The words that produced it — the phrase for a quotation, the terms for a
    /// paraphrase. The same field `DetectionEvent::matched_text` carries, for the
    /// same reason: a person judges this by the words, not by a number.
    matched_text: Option<String>,
    /// WHICH RULE HELD IT. Two rules do very different things and an operator
    /// reading "3 held" cannot act on it without knowing which — "the verse is
    /// already up" needs nothing from them, "this one is outside the reading" might.
    reason: detection::HeldReason,
}

/// `detection://held` — the whole of what the passage guard did in one window.
///
/// Its own event rather than a field on `detection://match`, because the thing it
/// has to report happens in exactly the windows where a match may not be emitted
/// at all: the in-passage reading that armed the guard is usually a repeat inside
/// the router's cooldown and is Dropped, so a field on a `match` would go missing
/// precisely when the operator needed it. One door, beside the decision.
/// One candidate the citation-doubt rule demoted, and why. RG-305.
///
/// Rule 35: a demotion an operator cannot see is indistinguishable from a detector
/// that missed. The reference is still offered and the run beside it is still
/// offered; what the operator has to be told is that Relay heard TWO things in one
/// breath that disagree, and which words say so.
#[derive(Serialize, Clone)]
struct DoubtedClaim {
    /// The candidate that was demoted — the spoken reference, or the run that
    /// contradicted it.
    reference: String,
    /// What it is now. `uncertain_number` / `uncertain_book` for a doubted citation,
    /// `quoted` for the run, and every one of those is capped at Suggest.
    method: DetectionMethod,
    /// The words behind it. Never a number (rule 18).
    matched_text: Option<String>,
    /// WHICH RULE, and for the run, that it is the accuser rather than the accused.
    doubt: detection::Doubt,
}

#[derive(Serialize, Clone)]
struct PassageHold {
    /// The book and chapter Relay believes is being read — "Psalms 107". `None`
    /// when only rule B fired, which needs no passage: the wall says it itself.
    passage: Option<String>,
    /// The phrase in this window, verbatim in that passage, that says the preacher
    /// is reading it. The evidence, not a number.
    reading: Option<String>,
    held: Vec<HeldCandidate>,
    /// **What the citation-doubt rule demoted in this window** (RG-305).
    ///
    /// On this event rather than on one of its own, and the reasoning is the same
    /// reasoning the event already carries: the thing to report happens in a window
    /// where a `detection://match` may not be emitted at all, and a second event
    /// name is a second thing to keep listened-for in both directions
    /// (`ipc.test.js`). It is a window-level decision about a candidate SET, which
    /// is what this event is for; `held` and `doubted` are the two such decisions
    /// there are.
    ///
    /// Empty in the overwhelmingly common case, and `skip_serializing_if` so an
    /// ordinary hold's payload is byte-for-byte what it was.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    doubted: Vec<DoubtedClaim>,
    trace_id: Option<u64>,
}

/// Everything one window of transcript has to say about scripture.
///
/// **Why this is a function and not sixty lines inside `emit_detections`.** The
/// passage guard below is a decision about a window's WHOLE candidate set — it
/// cannot live inside any one gatherer, and a set-level rule applied at four
/// gathering sites is rule 36's four-separate-bugs shape. It also could not be
/// measured: the assembly sat inside a closure holding three locks in a function
/// that needs a Tauri window, so no test and no bench could reach it, and the
/// only way to score a real service was to write a second copy of it. There is
/// now one copy, `emit_detections` calls it under the locks, and
/// `passage_guard_bench` scores the same code a congregation gets.
struct WindowCandidates {
    /// What reaches the gate.
    kept: Vec<Cand>,
    /// What the passage guard held, each with the rule that held it.
    ///
    /// The whole `Cand`, not the report shape: the report is built where it is
    /// reported, and a bench that wants to replay the shipped behaviour has to be
    /// able to put back exactly what was taken — a reconstruction from a narrower
    /// struct is a different candidate wearing the same reference.
    held: Vec<(Cand, detection::HeldReason)>,
    /// The book and chapter on screen that this window was heard reading, and the
    /// phrase that said so. `None` when the guard did not apply.
    reading_in: Option<(String, String)>,
    /// What the citation-doubt rule demoted, and why (RG-305). These candidates are
    /// in `kept` — nothing is held by that rule and nothing is dropped; only what
    /// may reach a wall unattended changed.
    doubted: Vec<Doubted>,
}

/// One candidate the citation-doubt rule demoted, as the window reports it.
struct Doubted {
    reference: String,
    /// What it is now — all three outcomes capped at Suggest by `Router::decide`.
    method: DetectionMethod,
    /// What it WAS. Carried so a bench can put back exactly the shipped behaviour of
    /// 2026-09-25 rather than a near miss at it, which is the same reasoning
    /// `passage_guard_bench` records for a held candidate.
    ///
    /// Read by `citation_doubt_bench` alone, so a release build has no reader —
    /// allowed rather than `#[cfg(test)]`, because a field the SHIPPED path fills and
    /// a test reads is not the same thing as a test-only field, and hiding it behind
    /// `cfg` would let the two builds disagree about what a window reported.
    #[allow(dead_code)]
    was: DetectionMethod,
    matched_text: Option<String>,
    doubt: detection::Doubt,
}

/// Gather, and apply the passage guard.
///
/// Pure over its inputs — no database, no clock, no Tauri handle — so the bench
/// and the live path cannot disagree about what a window produces.
fn candidates_for_window(
    text: &str,
    is_final: bool,
    sem: &Semantic,
    phrases: &Phrases,
    context: &ContextMemory,
    on_the_wall: Option<&str>,
    // **THE CHURCH'S BAR ON A PARAPHRASE** — must the words echo a run of the
    // verse's own words before Relay offers it? A PARAMETER rather than a read of
    // the state it comes from, so this function stays pure over its inputs and
    // `suggestions::bar::paraphrase_bar` can score both settings against the same
    // corpus through the same code a congregation gets (rule 13).
    paraphrase_needs_a_run: bool,
) -> WindowCandidates {
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
        let resolved = detection::resolve_bare_verse_with_source(
            text,
            n,
            anchor.as_ref(),
            // RG-318. A book and chapter said one window ago, with the sentence
            // plainly unfinished — `chapter_in_flight`. It beats memory and loses to
            // this window's own anchor, which is rule 40's ordering rather than a
            // new one, and it is labelled `UncertainBook` so it can never fire.
            context.in_flight(),
            from_memory.as_ref(),
        );
        if let Some((r, source)) = resolved {
            // "…and verse eighteen", resolved against the passage already on
            // screen. The operator needs to see that this came from CONTEXT, not
            // from a book name they never heard the preacher say.
            //
            // AND THE LABEL SAYS WHICH (FIELD 2026-09-20, RG-179). This was a
            // hardcoded `Direct` for both sources, and rule 40 recorded the lie
            // on purpose while one service was the evidence. The second service
            // put **Psalms 55:1** on a wall for a preacher quoting Hosea 6:1:
            // the book was misheard into a word no alias knows, memory answered
            // with the psalm already up, and `Direct` at 0.88 auto-fired it.
            // A book Relay assumed is `UncertainBook` — the router offers it and
            // fires nothing. A book named in this breath is still heard.
            let mut c = Cand::single(
                r,
                0.88,
                DetectionMethod::for_bare_verse(source),
                Some(format!("verse {n}")),
            );
            // HOW FAR HE SAID HE WAS GOING, when the chunker cut the announcement in
            // two and this bare verse is all the second window has (RG-328). *"verse 6
            // all the way to 8"* carried the 6 and dropped the 8, so the operator was
            // shown one verse of a three-verse reading with nothing saying there were
            // two more — `detect_bare_verses` answers numbers, not spans, and nothing
            // else on this path was looking.
            if let Some((v, end)) = detection::bare_verse_span(text) {
                if v == n {
                    c.verse_end = Some(end);
                }
            }
            candidates.push(c);
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
    // READ GUARD (RG-300). Readers do not block readers, so the detection path
    // costs what the bare field cost; the one writer is a translation switch,
    // which the service lock refuses while a service is recording.
    //
    // A POISONED INDEX SUGGESTS NOTHING rather than panicking. The only writer
    // runs off the live path, so this can practically only follow a panic
    // elsewhere — and an empty suggestion list is the same answer an empty
    // corpus gives, on a path that must never bring the service down.
    let semantic_hits = match sem.0.read() {
        Ok(idx) => worth_suggesting(idx.top_k_explained(text, SEMANTIC_SUGGESTIONS_MAX)),
        Err(_) => Vec::new(),
    };
    for (r, score, terms) in semantic_hits {
        candidates.push(Cand::single(
            r,
            score.min(0.95),
            DetectionMethod::Semantic,
            Some(terms.join(" · ")),
        ));
    }
    // ── QUOTED SCRIPTURE ──────────────────────────────────────────────
    //
    // A contiguous run of the preacher's own words that is verbatim in one
    // verse. This is the operator's instruction of 2026-09-20 — *"it has to
    // be three words together as in the scripture"* — and it exists because
    // the paraphrase row above renders `terms.join(" · ")` inside quotation
    // marks, so a verse justified by `lord` and `shepherd`, in neither order,
    // reached the run surface dressed as a quotation.
    //
    // THE ANCHOR IS RULE 40, AND IT IS APPLIED IN TWO STRENGTHS, because the
    // two kinds of evidence are not equal:
    //
    //   * A BOOK THIS WINDOW NAMED restricts. The words said it, so nothing
    //     outside it is a candidate.
    //   * THE PASSAGE ON SCREEN only re-ranks. Memory is what Relay has when
    //     the words do not say, and a quotation IS the words saying — a
    //     preacher reading Proverbs who quotes Isaiah is quoting Isaiah.
    //     Restricting on memory would hide it; preferring merely puts the
    //     likelier reading first.
    //
    // Measured on the service of 2026-09-20: "Verse 7 says, Be not wise in
    // your own eyes" names no book at all, and that phrase is verbatim in
    // Romans 12:16 as well as in the Proverbs 3 the preacher was reading.
    let quoted_in = anchor.as_ref().map(|r| r.book.as_str());
    let mut quoted = match phrases.0.read() {
        Ok(g) => g.quoted(text, quoted_in, QUOTED_SUGGESTIONS_MAX),
        Err(_) => Vec::new(),
    };
    if quoted_in.is_none() {
        if let Some(on_screen) = context.current().map(|r| r.book.clone()) {
            quoted.sort_by_key(|h| h.r.book != on_screen);
        }
    }
    // ── THE ONE CROSS-BOOK EXCEPTION TO THE RESTRICTION ABOVE (RG-305) ────────
    //
    // FIELD, service 40, 2026-09-25 at 9831 s: *"Acts 8, 12, I wisdom dwell with
    // prudence and find out the knowledge of witty inventions."* Those fifteen words
    // are **Proverbs 8:12**; whisper heard `Proverbs` as `Acts`. Relay fired
    // **Acts 8:12**. The words that named the right verse were in the same window,
    // long enough to be unambiguous — and the restriction above had already thrown
    // them away, because the window "named" Acts.
    //
    // **This does not relax the restriction; it carves out the single shape that is
    // evidence ABOUT the restriction.** A hit is admitted only when it is in another
    // book at the EXACT chapter and verse a spoken reference in this window named.
    // Chapter-and-verse pairs collide across sixty-six books, so on its own that is
    // a coincidence — which is why the hit cannot fire (`doubt_from_a_quotation`
    // demotes it) and is only ever offered beside the reference it contradicts. Every
    // other verse outside the named book stays hidden exactly as before, so the
    // measured reason the restriction exists is untouched.
    //
    // It costs one more `quoted` call, in windows that name a book AND a verse.
    // Measured in `citation_doubt_bench`.
    let said_pairs: Vec<(i64, i64)> = candidates
        .iter()
        .filter(|c| c.method == DetectionMethod::Direct && !c.whole_chapter)
        .map(|c| (c.r.chapter, c.r.verse))
        .collect();
    if let (Some(named), false) = (quoted_in, said_pairs.is_empty()) {
        if let Ok(g) = phrases.0.read() {
            quoted.extend(
                g.quoted(text, None, QUOTED_SUGGESTIONS_MAX)
                    .into_iter()
                    .filter(|h| {
                        !h.r.book.eq_ignore_ascii_case(named)
                            && said_pairs.contains(&(h.r.chapter, h.r.verse))
                    }),
            );
        }
    }
    // Where the quotation candidates start, and the two facts about each run that
    // `DetectionMethod` cannot carry. `Reading` means *eight words and sole*, but
    // `Quoted` means *shorter OR shared*, and the citation-doubt rule may only be
    // armed by a run one verse holds alone. Recorded by INDEX rather than pushed in
    // lockstep with every `candidates.push` in this function, because a parallel push
    // is a thing a future gatherer forgets; `the_run_facts_land_on_the_quotation`
    // holds the mapping.
    let quoted_at = candidates.len();
    let run_facts: Vec<(usize, bool)> = quoted.iter().map(|h| (h.run, h.sole)).collect();
    for h in quoted {
        candidates.push(Cand::single(
            h.r,
            quoted_confidence(h.run),
            // IS THIS THE PREACHER READING, OR MERELY QUOTING? The operator's
            // instruction of 2026-09-23 is that a verse being READ should go
            // up without being asked for (DECISIONS §118). `for_quotation` is
            // the one place that is decided and it decides from the evidence
            // alone — the run length, whether one verse holds it, and since
            // RG-307 whether any word in it is rare enough to name a verse at
            // all. The church's switch over it is in `Router::decide`, the door
            // every candidate passes through, so it cannot be skipped here.
            DetectionMethod::for_quotation(h.run, h.sole, h.rare_for_a_wall),
            // THE PHRASE, not a word list. The whole point.
            Some(h.phrase),
        ));
    }
    if direct_empty {
        for r in detection::detect_ambiguous(text) {
            candidates.push(Cand::single(r, 0.70, DetectionMethod::Ambiguous, None));
        }
    }
    // ── THE CITATION-DOUBT RULE, 2026-09-25 (RG-305) ──────────────────────
    //
    // Eight wrong verses reached congregations in one day, and they are one failure:
    // the decoder loses or alters a digit or an ordinal in a spoken reference, and
    // the result is a SMALLER, VALID, WRONG reference. *eighty*-seven → 7 ·
    // *eigh*-teen → 8 · *sixty*-one → 1 · *twelve* → 2 · `Proverbs` → `Acts`. The
    // chapter exists, the verse exists, the parse confidence is real, and it fires at
    // 0.95 `Direct` — so nothing that looks at one piece of evidence can see it.
    //
    // `detection::doubt_from_a_quotation` is the whole rule and it is pure. Applied
    // HERE, once, over the finished set, for the reason the passage guard is
    // (rule 36): it is a decision about the SET — which of these candidates is the
    // reference the preacher meant — and a set-level rule added at the gathering
    // sites is the shape four separate bugs in this repository have.
    //
    // **Applied BEFORE the passage guard, and the order does not matter to the
    // guard**: both `Reading` and `Quoted` are `came_from_the_verse_text` and
    // `is_a_verbatim_run`, so a demotion between them changes nothing the guard asks.
    // It is first because evidence should be weighed before anything is withheld.
    //
    // **Nothing is held and nothing gains a wall.** A doubted citation drops to
    // `UncertainNumber`/`UncertainBook` and the run that accused it drops to
    // `Quoted` — all three already capped at Suggest by `Router::decide` at any
    // score, so a disagreement fires nothing at all and the operator picks between
    // two things Relay genuinely heard. Rule 10's cap is applied to one more case and
    // relaxed for none.
    let doubts = {
        let view: Vec<detection::Claim> = candidates
            .iter()
            .enumerate()
            .map(|(i, c)| detection::Claim {
                r: &c.r,
                method: c.method,
                verse_end: c.verse_end,
                whole_chapter: c.whole_chapter,
                run: i
                    .checked_sub(quoted_at)
                    .and_then(|k| run_facts.get(k).copied()),
            })
            .collect();
        detection::doubt_from_a_quotation(&view)
    };
    let mut doubted: Vec<Doubted> = Vec::new();
    for (c, doubt) in candidates.iter_mut().zip(&doubts) {
        let Some(doubt) = *doubt else { continue };
        let was = c.method;
        c.method = match doubt {
            // The NUMBER is in doubt and `UncertainNumber` is what that already
            // means in this codebase — the variant `uncertain_number` stamps on the
            // five parse sites that infer a number rather than hear one.
            detection::Doubt::SpokenChapter => DetectionMethod::UncertainNumber,
            // The BOOK is in doubt: "chapter and verse heard, the book not", which is
            // `UncertainBook`'s own sentence (DECISIONS §106).
            detection::Doubt::SpokenBook => DetectionMethod::UncertainBook,
            // The accuser is demoted too, so a disagreement puts nothing on a wall.
            // A `Reading` may auto-fire under DECISIONS §118 and this is the one
            // place that is taken back — never widened.
            detection::Doubt::TheQuotation => DetectionMethod::Quoted,
        };
        doubted.push(Doubted {
            reference: Fire::key_for(&c.r),
            method: c.method,
            was,
            matched_text: c.matched.clone(),
            doubt,
        });
    }

    // ── A DOUBT IS A FACT ABOUT A REFERENCE, NOT ABOUT ONE CANDIDATE ─────
    //
    // `doubt_from_a_quotation` returns one verdict per candidate INDEX, and the loop
    // above applied each verdict to that index alone. A window can name the same
    // reference twice — service 40, 2026-09-25 at 4691.7 s produced `Psalms 27:1`
    // both from the range parse (`Direct` 0.95) and from the bare *"verse 1"*
    // anchored on it (`Direct` 0.88) — and the demotion reached the first while the
    // duplicate went to the wall still firable. The rule had already concluded the
    // reference was in doubt and the conclusion was being thrown away.
    //
    // Found by printing a candidate set rather than reasoning about one, which is
    // also how the anchor fix's apparent regressions turned out to be this.
    {
        let doubted_keys: std::collections::HashMap<String, detection::Doubt> = doubted
            .iter()
            .map(|d| (d.reference.clone(), d.doubt))
            .collect();
        for c in candidates.iter_mut() {
            // Only a candidate that could still reach a wall needs taking down; the
            // rest are already capped and re-labelling them would say nothing.
            if c.method.unattended_rank() == 0 {
                continue;
            }
            let Some(doubt) = doubted_keys.get(&Fire::key_for(&c.r)) else {
                continue;
            };
            c.method = match doubt {
                detection::Doubt::SpokenChapter => DetectionMethod::UncertainNumber,
                detection::Doubt::SpokenBook => DetectionMethod::UncertainBook,
                detection::Doubt::TheQuotation => DetectionMethod::Quoted,
            };
        }
    }

    // ── A SPAN IS A FACT ABOUT THE REFERENCE, NOT ABOUT ONE CANDIDATE (RG-328) ──
    //
    // Found by `e2e::a_conversationally_announced_range_stages_and_walks_to_the_end_the_preacher_gave`,
    // which is the first test to drive the operator's whole workflow rather than the
    // parse: announce *"Romans 1 and we will be reading from verse 6 all the way to
    // 8"* and the verse reaches the wall — but at `direct` 0.88 with **no
    // `passage_end`**, so *"all the way to 8"* is thrown away.
    //
    // Two paths answer that window. The full parse yields `Romans 1:6-8`, and
    // `detect_bare_verses` independently resolves the bare *"verse 6"* against the
    // window's own anchor (`resolve_bare_verse`, the hardcoded 0.88) and yields
    // `Romans 1:6` with no span at all. The anchor candidate is the one that can
    // fire, so the span vanished behind the stronger claim.
    //
    // The same shape as the doubt fix above and the same lesson: the span belongs to
    // the REFERENCE, so every candidate naming it carries it. Copied rather than
    // ranked, because whichever candidate survives should stage the passage the
    // preacher announced — and copied ONLY onto a candidate that has no span of its
    // own, so an explicit range can never be widened by another one.
    {
        let spans: std::collections::HashMap<String, i64> = candidates
            .iter()
            .filter_map(|c| c.verse_end.map(|e| (Fire::key_for(&c.r), e)))
            .collect();
        for c in candidates.iter_mut() {
            if c.verse_end.is_some() || c.whole_chapter {
                continue;
            }
            if let Some(end) = spans.get(&Fire::key_for(&c.r)) {
                c.verse_end = Some(*end);
            }
        }
    }

    // ── FOLLOWING THE PREACHER THROUGH A PASSAGE HE ANNOUNCED (2026-09-28) ──
    //
    // The operator's report: *"even if the preacher paraphrases some section of the
    // scripture, follow the preacher to know when to move to the next verse."*
    //
    // Reading verse 7 of a staged passage aloud already works — a verbatim run is
    // `Reading` and `Reading` may fire (40 of service 42's 118 auto-fires were
    // exactly that). What did not work is the preacher RETELLING verse 7, which is
    // `Semantic`, and rule 10 caps a paraphrase at `Suggest` at any score.
    //
    // **That cap is not weakened here, and this is the argument for why this is a
    // different act.** Rule 10 exists so the AI cannot put an ARBITRARY verse on a
    // wall from a bag of words — a cosine is not a probability, and the harm is a
    // verse nobody asked for. Inside an EXPLICIT span the set of possible outcomes is
    // not arbitrary: it is the verses the preacher named out loud, bounded at both
    // ends, and Relay is choosing WHERE IN THAT PASSAGE he is rather than which verse
    // he means. `router::corroboration_never_promotes_a_paraphrase` still holds,
    // because corroboration still promotes nothing; the bound is the announcement.
    //
    // Four conditions, all necessary:
    //   * an EXPLICIT range is staged (`span_end`) — a whole chapter is not an
    //     announcement of how far he intends to read, and 150 verses of Psalms is
    //     back to arbitrary;
    //   * the candidate is the SAME book and chapter as what is on the screen;
    //   * it is FORWARD of the current verse and no further than the range end —
    //     forward-only, so a retelling that brushes an earlier verse cannot walk the
    //     wall backwards or oscillate;
    //   * it came from the VERSE'S OWN WORDS. `Semantic` and `Quoted` qualify;
    //     `UncertainBook`/`UncertainNumber` never do, because those are doubts about
    //     which reference this is and that is the question a span cannot answer.
    //
    // The dwell floor (rule 45) still applies, so following cannot flash past a
    // congregation, and the whole thing is inert until a passage is staged.
    if let (Some(cur), Some(end)) = (context.current(), context.span_end()) {
        for c in candidates.iter_mut() {
            if c.method.unattended_rank() != 0 {
                continue; // already able to reach the wall on its own evidence
            }
            let words_only = matches!(
                c.method,
                DetectionMethod::Semantic | DetectionMethod::Quoted
            );
            let in_passage = c.r.book == cur.book
                && c.r.chapter == cur.chapter
                && c.r.verse > cur.verse
                && c.r.verse <= end;
            if words_only && in_passage {
                c.method = DetectionMethod::Reading;
            }
        }
    }

    // ── THE SHORT RUN A NAMED CHAPTER MAKES ADMISSIBLE, RG-313 ───────────
    //
    // The rule above sources its accusing run from `PhraseIndex::quoted`, which
    // needs `MIN_RUN_WORDS` (5) because it answers *which verse do these words
    // belong to* and below five that question has too many answers. Two of the six
    // wrong references it could not reach fail on that floor and nothing else —
    // `Isaiah 61:3` at 4 words, `Romans 12:3` at 3 — and both had already named
    // their book and their chapter out loud. That leaves a narrower question, which
    // a shorter run can answer: *did these words touch the verse one digit away
    // from the one he said*. `PhraseIndex::shared_run_with` measures exactly that,
    // against ONE named verse, and `PARAPHRASE_RUN_WORDS` (3) is the floor already
    // in use for corroborating a reference that exists rather than offering one
    // standing alone.
    //
    // **A PROBE, NOT A SCAN.** The set is the inverse of the same slip test the
    // rule above applies — nine chapters for a one-digit chapter, a handful of
    // substitutions above that — so this asks the index a bounded number of
    // questions about a single book and never walks the corpus.
    //
    // **It reaches for a verse and never for a book**: only `SpokenChapter` can
    // come out of it, so the cross-book carve-out above, which is the one that
    // protects `John 15:14` and `Hebrews 13:7`, is untouched.
    //
    // Runs SECOND and only on what the first rule left alone, so a candidate the
    // stronger evidence already doubted keeps the doubt that evidence gave it.
    {
        // Psalms has 150 and nothing has more. A chapter this book does not have
        // resolves to no verse and `shared_run_with` answers 0, so the bound is a
        // cost ceiling rather than a rule about scripture.
        const MAX_CHAPTER: i64 = 150;
        // KEYED BY REFERENCE, and it grows as the probe reports. Two candidates in
        // one window can carry the same reference — a bare verse resolved against an
        // anchor beside the same reference parsed outright — and the first draft
        // reported both, so `detection://doubted` printed `Isaiah 1:3` twice in a row
        // to an operator who saw one claim.
        let mut already: std::collections::HashSet<String> =
            doubted.iter().map(|d| d.reference.clone()).collect();
        let idx = phrases.0.read();
        let probes: Vec<(usize, i64)> = match idx.as_ref() {
            Ok(idx) => candidates
                .iter()
                .enumerate()
                .filter(|(_, c)| !already.contains(&Fire::key_for(&c.r)))
                .filter_map(|(i, c)| {
                    let claim = detection::Claim {
                        r: &c.r,
                        method: c.method,
                        verse_end: c.verse_end,
                        whole_chapter: c.whole_chapter,
                        run: None,
                    };
                    detection::chapter_the_words_point_at(&claim, MAX_CHAPTER, |probe| {
                        idx.shared_run_with(text, probe)
                    })
                    .map(|ch| (i, ch))
                })
                .collect(),
            Err(_) => Vec::new(),
        };
        drop(idx);
        for (i, _chapter) in probes {
            let c = &mut candidates[i];
            let key = Fire::key_for(&c.r);
            if !already.insert(key.clone()) {
                // Demote it all the same — the cap is what protects the wall — and
                // report it once.
                c.method = DetectionMethod::UncertainNumber;
                continue;
            }
            let was = c.method;
            c.method = DetectionMethod::UncertainNumber;
            doubted.push(Doubted {
                reference: key,
                method: c.method,
                was,
                matched_text: c.matched.clone(),
                doubt: detection::Doubt::SpokenChapter,
            });
        }
    }

    // ── THE RUN BAR ON A PARAPHRASE, off unless the church asked for it ───
    //
    // The operator, 2026-09-25: *"The preacher paraphrases a lot so I want you to
    // catch that and use the style to work on how the app respond."* Measured, that
    // turned out to be two findings and only one of them is about catching more:
    // 74% of what the paraphrase path offers on speech naming no scripture is noise
    // (DECISIONS §125, RG-311), and no value of `SEMANTIC_FLOOR` can cut it because
    // the two populations share a distribution and invert at the tails.
    //
    // What does cut it is a different question about the same evidence: a contiguous
    // run of `detection::PARAPHRASE_RUN_WORDS` words shared with the verse named. A
    // cosine is a bag of words in no order (rule 18); this asks whether any of them
    // were said in the verse's order. It removes 73.5% of offers and takes precision
    // from 18.7% to 50.0%, dominating every threshold on all four measures at once.
    //
    // **IT IS A SETTING AND IT DEFAULTS OFF, and that is the whole design.** It
    // silences three of the 43 labelled retellings and every one is `vocab: modern` —
    // the four friends tearing open a roof, Paul and Silas at midnight, Jonah
    // overboard — which is the narrative case the product's claim rests on. It also
    // removes the one paraphrase offer anybody can prove the operator wanted, and to
    // a MISHEARD word rather than to modern wording. Spending those is an operator's
    // trade, so a church that never opens Settings is offered exactly what it was
    // offered yesterday. `suggestions::what_the_bar_silences` holds both figures.
    //
    // **HERE, over the finished set, and never at the gathering site**, for rule
    // 36's reason: this is the third window-level rule and it belongs with the other
    // two, so the answer to *what did Relay decide not to offer, and why* has one
    // place. It is also why a held paraphrase is REPORTED rather than dropped — a
    // switch that quietly stops offering things is rule 35 exactly, and
    // `HeldReason::NoSharedRun` rides the event that already exists for that.
    //
    // **ORDER DOES NOT MATTER against the passage guard** and the masks are merged
    // rather than chained: a `Semantic` candidate arms neither of the guard's rules
    // (rule A needs `is_a_verbatim_run`, rule B needs the wall), so holding it first
    // or last cannot change what the guard decides about anything else.
    //
    // A POISONED PHRASE INDEX HOLDS, rather than waving everything through. The
    // operator asked for a bar; a bar that silently stops being applied is the
    // failure rule 35 is about, and because every hold is announced the operator can
    // see that this is what happened. Practically unreachable — the only writer is a
    // translation switch the service lock refuses mid-service.
    let run_mask: Vec<Option<detection::HeldReason>> = if paraphrase_needs_a_run {
        let index = phrases.0.read();
        candidates
            .iter()
            .map(|c| {
                if c.method != DetectionMethod::Semantic {
                    return None;
                }
                let echoes = index.as_ref().is_ok_and(|g| {
                    g.shared_run_with(text, &c.r) >= detection::PARAPHRASE_RUN_WORDS
                });
                (!echoes).then_some(detection::HeldReason::NoSharedRun)
            })
            .collect()
    } else {
        // NOT `Vec::new()`. Every mask below is indexed in lockstep with
        // `candidates`, and a short one would silently stop holding at the first
        // index it ran out at.
        vec![None; candidates.len()]
    };

    // ── THE PASSAGE GUARD, 2026-09-25 ─────────────────────────────────────
    //
    // The operator, 2026-09-25: *"I dont want suggestion to be changing when a
    // bible verse is reading because it heard a phrase which is in another bible
    // verse… verses needs to be guarded so when a preacher is reading a verse it
    // stays within the verse/chapter until the preacher calls another verse…
    // suggesting too many verses whilst the preacher is reading a verse will
    // cause confusion"*.
    //
    // `detection::hold_for_the_passage` is the whole rule and it is pure. Applied
    // HERE, once, over the finished set: it is a decision about the SET (which
    // verse is the one being read, and which is merely also holding those words),
    // so no gatherer can hold it, and a set-level rule added at four gathering
    // sites is the shape rule 36 records four separate bugs for.
    let on_screen = context.current();
    let view: Vec<(&VerseRef, DetectionMethod)> =
        candidates.iter().map(|c| (&c.r, c.method)).collect();
    // ONE MASK, TWO RULE SETS. `or` keeps the run bar's answer when both fire,
    // which is the more actionable of the two sentences: "you turned this on" is
    // something the operator can undo, and "the preacher is reading elsewhere" is
    // not.
    let passage_mask = detection::hold_for_the_passage(
        on_screen,
        on_the_wall,
        detection::window_states_a_reference(text, anchor.as_ref()),
        &view,
    );
    // `zip` TRUNCATES, silently, which is the one way this merge could go wrong: a
    // mask one short would stop holding at the last index and nothing would say so.
    // Both are built from `candidates.len()`, so a mismatch is a programming error
    // rather than a state a church can reach — hence a debug assertion and not a
    // runtime branch on the fire path.
    debug_assert_eq!(passage_mask.len(), run_mask.len());
    let mask: Vec<Option<detection::HeldReason>> = passage_mask
        .into_iter()
        .zip(&run_mask)
        .map(|(passage, run)| run.or(passage))
        .collect();
    if mask.iter().all(Option::is_none) {
        return WindowCandidates {
            kept: candidates,
            held: Vec::new(),
            reading_in: None,
            doubted,
        };
    }
    // Rule A armed only if an in-passage verbatim run is present, and such a run is
    // never itself held — which is what makes the announcement reachable. A guard
    // that could hold EVERYTHING would be indistinguishable from Relay having gone
    // quiet, which is rule 35 exactly. Rule B needs no passage: it fires on the
    // verse the screens are already showing, and the screens are the evidence.
    let reading_in = on_screen
        .filter(|_| mask.contains(&Some(detection::HeldReason::OutsideTheReading)))
        .map(|p| {
            let phrase = candidates
                .iter()
                .zip(&mask)
                .find(|(c, held)| {
                    held.is_none()
                        && c.method.is_a_verbatim_run()
                        && c.r.book == p.book
                        && c.r.chapter == p.chapter
                })
                .and_then(|(c, _)| c.matched.clone())
                .unwrap_or_default();
            (format!("{} {}", p.book, p.chapter), phrase)
        });
    let mut kept: Vec<Cand> = Vec::with_capacity(candidates.len());
    let mut held: Vec<(Cand, detection::HeldReason)> = Vec::new();
    for (c, why) in candidates.into_iter().zip(mask) {
        match why {
            Some(reason) => held.push((c, reason)),
            None => kept.push(c),
        }
    }
    WindowCandidates {
        kept,
        held,
        reading_in,
        doubted,
    }
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
    let phrases = handle.state::<Phrases>();
    // THE CHURCH'S PARAPHRASE BAR, read before any lock is taken. See
    // `ParaphraseRun`; off unless an operator turned it on.
    let needs_a_run = handle.state::<ParaphraseRun>().0.load(Ordering::Relaxed);

    // Compute everything UNDER the locks, but collect the emits/broadcasts and
    // fire them AFTER releasing — never hold a lock across handle.emit /
    // broadcast_content, which can otherwise deadlock the main run loop with a
    // command contending the same lock (this was the freeze on Start listening).
    let mut events: Vec<DetectionEvent> = Vec::new();
    let mut broadcasts: Vec<OutputContent> = Vec::new();
    // What the passage guard held, and the passage it held it for. Collected here
    // and announced after the locks go, for the same reason as `events`.
    // Declared uninitialised deliberately: every path that reaches the emit below
    // assigns them, and seeding them with an empty vec would let a future early
    // return announce "nothing was held" when the truth is that nobody looked.
    let held_by_the_passage: Vec<(Cand, detection::HeldReason)>;
    let reading_inside: Option<(String, String)>;
    // What the citation-doubt rule demoted (RG-305). Assigned on the same path as
    // the two above and for the same reason: an early return that left this empty
    // would say "nothing was doubted" when the truth is that nobody looked.
    let doubted_citations: Vec<Doubted>;
    // Latency stamps sampled under the locks and applied after they are released.
    // The block yields them so `detected_at` is INITIALISED by the sample rather
    // than pre-seeded with a value no path ever reads.
    // A LABELLED BLOCK, and `Option`, because one path through it must skip the gate
    // and still reach the announcement below. See the empty-candidates arm.
    let stamps: Option<(u64, Option<u64>)> = 'gate: {
        let (Ok(conn), Ok(mut router), Ok(mut context)) =
            (db.0.lock(), routing.0.lock(), ctx.0.lock())
        else {
            return;
        };
        let WindowCandidates {
            kept: candidates,
            held,
            reading_in,
            doubted,
        } = candidates_for_window(
            text,
            is_final,
            &sem,
            &phrases,
            &context,
            // What the screens are actually showing, from the one module that knows
            // — and NOT `context.current()`, which survives a blackout on purpose.
            router.wall(),
            // READ OUTSIDE THE LOCKS ABOVE, on purpose: an `AtomicBool` read cannot
            // block and cannot participate in a lock order (rule 6).
            needs_a_run,
        );
        // ASSIGNED BEFORE THE EARLY EXIT, and that order is the whole point.
        //
        // This read `return` under an assertion that the guard could not have held
        // anything in a window with nothing left to say. **The assertion was false
        // and the first e2e test written against the fire path caught it on its
        // first run.** Rule B holds the verse the screens are already showing, and a
        // window whose only candidate is that verse — a preacher reading on through
        // the verse Relay already put up, which is the ordinary case the rule exists
        // for — leaves the set empty. The `return` then took the report with it, so
        // Relay held something back and said nothing: rule 35's failure, introduced
        // by the code that exists to prevent it.
        held_by_the_passage = held;
        reading_inside = reading_in;
        doubted_citations = doubted;
        // ── WHAT THIS WINDOW LEFT UNFINISHED (RG-318) ──────────────────────────
        //
        // AFTER `candidates_for_window` has read the last window's entry, and BEFORE
        // the early exit below, for the same reason `held_by_the_passage` is: a
        // window with nothing to say is exactly the window that carries half a
        // citation — *"…before we give Genesis chapter 8 and verse"* produces no
        // candidate at all — and a `break` above this line would drop the half that
        // matters. `note_in_flight` owns the expiry because this is the one place on
        // the path with a clock.
        context.note_in_flight(detection::chapter_in_flight(text), now_ms);
        if candidates.is_empty() {
            break 'gate None;
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
            // RG-302. Set HERE, before the `may_broadcast` branch below, so a
            // passage that is only OFFERED carries its span too — the operator
            // deciding whether to accept "Proverbs 7:1" is exactly the person who
            // needs to know five verses were asked for. It comes from the candidate
            // rather than from `ContextMemory`, because nothing is staged for a
            // suggestion and the staged span would then be the PREVIOUS reading's.
            fire.passage_end = span_to_report(fire.reference.verse, end);
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

            // ── PARSED, BUT THERE IS NO VERSE BEHIND IT (RG-322) ────────────────
            //
            // Garbled speech readily yields "Psalms 23:99", and service 42 yielded
            // `Jude 29:4` — *"Job 29 verse 4 to 17"* heard as *"Jude 29"*, and Jude
            // has one chapter. This used to DEMOTE to a suggestion, which was the
            // right half of the answer: `Fire::may_broadcast` is false for a
            // suggestion, so nothing blanks a projector.
            //
            // It is not offered at all any more. A row that names a verse Relay
            // cannot show says LESS than no row — the operator's list is what a
            // volunteer reads in a dark booth mid-service, and three of these
            // rendered blank on it, with no reference in them, on the morning this
            // was filed. There is nothing to act on and nothing to learn from, and
            // `persist_fire` below would write a detection whose `verse_id` is NULL.
            //
            // **It is not silent** in the sense rule 35 means: the operator is not
            // being told a screen is fine when it is not, and no guarantee hangs on
            // this row's absence. What is worth saying about the window is said by
            // the cause — an impossible number now refutes the `fuzzy_book` repair
            // that made it outright (`detection::detect_direct`), so the commoner
            // route to this state no longer produces a candidate to begin with.
            if fire.verse_id.is_none() {
                continue;
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
                broadcasts.push(fire.output());
            }
            // ── WHAT RELAY OFFERED IS PART OF WHAT HAPPENED (RG-309) ────────────
            //
            // This call sat INSIDE the `if` above, and the consequence was structural
            // rather than a bug anybody could see: rule 10 caps a paraphrase at
            // `Suggest` at any score, `Fire::may_broadcast` is false for a
            // suggestion, and so **a paraphrase never reached the database at all.**
            // Measured on this machine, across every service it has ever recorded:
            // `SELECT COUNT(*) FROM detections WHERE status='suggested'` → 0.
            //
            // Three things followed. The paraphrase detector was unobservable, on
            // the product whose operator asked for it by name. Every accuracy claim
            // about it rested on nothing, which `report.js` was honest about and the
            // register was not. And `record_feedback` learns its bar from a column
            // that only ever saw what worked.
            //
            // Rule 14 is untouched and this is the reason it can be: the status
            // written is the one the gate reached. `'auto'` is still Relay's own
            // initiative and `'manual'` is still a human — a suggestion is neither,
            // and `'suggested'` is the value the CHECK constraint has permitted, and
            // nothing has ever written, since the schema was first drawn.
            //
            // **It is not a new write on the fire path.** It is the same one write,
            // reached on more windows: inside the connection this loop already holds,
            // on `relay-detect` behind the bounded queue rule 33 put it behind, and
            // measured at 6 µs against a 139 ms cadence (`suggestions::the_write_a_
            // suggestion_costs`).
            persist_fire(
                &conn,
                handle.state::<Session>(),
                fire.verse_id,
                fire.method.db_method(),
                fire.confidence,
                fire.status.as_str(),
                text,
                if fire.may_broadcast() {
                    Provenance::Fired
                } else {
                    Provenance::Offered
                },
            );
            events.push(fire.event());
        }
        Some((detected_at, authorised_at))
    }; // locks released here

    // A window that produced no candidate to gate timed no reference, so there is
    // nothing to stamp — and a stage never reached is an ABSENCE, not a zero
    // (rule 31).
    if let (Some(id), Some((detected_at, authorised_at))) = (trace, stamps) {
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
        let _ = broadcast_with_clock(handle, content, now_ms);
    }
    for ev in events {
        let _ = handle.emit("detection://match", ev);
    }
    // ── RELAY IS HOLDING SOMETHING BACK, AND SAYS SO ──────────────────────────
    //
    // Rule 35. A guard that quietly stops offering verses is indistinguishable
    // from a detector that has gone deaf, and the operator would have no way to
    // tell which. One emit, at the one place the decision is made, carrying the
    // passage it is following, the phrase that says so, and every candidate it
    // did not offer — so nothing is discarded silently (RG-178's precedent), it
    // is simply moved off the list the operator asked to stop churning.
    if !held_by_the_passage.is_empty() || !doubted_citations.is_empty() {
        let (passage, reading) = match reading_inside {
            Some((p, r)) => (Some(p), Some(r)),
            None => (None, None),
        };
        let _ = handle.emit(
            "detection://held",
            PassageHold {
                passage,
                reading,
                held: held_by_the_passage
                    .into_iter()
                    .map(|(c, reason)| HeldCandidate {
                        reference: Fire::key_for(&c.r),
                        method: c.method,
                        matched_text: c.matched,
                        reason,
                    })
                    .collect(),
                doubted: doubted_citations
                    .into_iter()
                    .map(|d| DoubtedClaim {
                        reference: d.reference,
                        method: d.method,
                        matched_text: d.matched_text,
                        doubt: d.doubt,
                    })
                    .collect(),
                trace_id: trace,
            },
        );
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
/// ── RG-309 · WHETHER THIS ROW MAY CREATE A TRANSCRIPT ROW OF ITS OWN ─────────
///
/// A fire may. F-2 is the whole argument: a verse that reached a congregation must
/// hang off a transcript row that really holds the words the detector read, and six
/// extra rows in a fifty-minute service is what that costs.
///
/// **A suggestion may not**, and the difference is not a judgement about
/// importance. Only FINAL transcripts are persisted, on purpose, and the live path
/// detects on every PARTIAL — roughly one a second. A suggestion that inserted its
/// own row would write thousands of rows of mid-word text per service into the one
/// table every history and replay surface renders, and `transcripts` is already the
/// largest content table Relay keeps. The window still travels, in `heard_text`,
/// which is exactly the column F-2 added for it; what a suggestion loses is a
/// transcript row of its own, not its evidence.
///
/// A suggestion with no final yet to hang off is not recorded. That is the first
/// seconds of a service and it is an absence, not a claim.
#[derive(Clone, Copy, PartialEq)]
enum Provenance {
    /// This reached a screen. It may persist the window as its own transcript row.
    Fired,
    /// Relay offered this and nothing went anywhere. It hangs off the last final.
    Offered,
}

#[allow(clippy::too_many_arguments)]
fn persist_fire(
    conn: &Connection,
    session: tauri::State<'_, Session>,
    verse_id: Option<i64>,
    method: &str,
    confidence: f32,
    status: &str,
    window_text: &str,
    provenance: Provenance,
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
        // An offer hangs off whatever final is there and never writes one — see
        // `Provenance`. No final yet means no row, which is honest.
        Some(t) if provenance == Provenance::Offered => t,
        None if provenance == Provenance::Offered => return,
        // RG-113(5). The language the decoder reported for THIS window, not a
        // hardcoded `"en"`. See `SessionState::last_language`: a bare clone rather
        // than a borrow because `st` is borrowed mutably on the success arm.
        _ => match db::insert_transcript(
            conn,
            st.id,
            ts,
            window_text,
            &st.last_language.clone(),
            None,
        ) {
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
    let idx = sem
        .0
        .read()
        .map_err(|_| error::Error::refused("the scripture index is unavailable — restart Relay"))?;
    Ok(search_verses(&conn, &idx, query.trim()))
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
                    Ok(conn) => match sem.0.read() {
                        Ok(idx) => search_verses(&conn, &idx, &q),
                        Err(_) => vec![],
                    },
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
///
/// ── THE MESSAGE USED TO CONTRADICT ITSELF AT ITS OWN BOUNDARY ────────────────
///
/// Two separate arithmetic faults, both only visible within a hair of the cap, and
/// the reason the sentence read *"…is about 256 MB, and Relay imports files up to
/// 256 MB"*:
///
/// **PADDING IS NOT PAYLOAD.** `len / 4 * 3` counts the trailing `=` as decoded
/// bytes. A file of EXACTLY 256 MiB encodes to 357,913,944 characters, of which two
/// are padding, and the old estimate answered 268,435,458 — two bytes over a
/// 268,435,456-byte cap. So the one file that is precisely at the documented limit
/// was refused, by two bytes, with a message saying it was the size of the limit.
/// The padding is discounted, which makes the estimate exact rather than merely
/// close, and `saturating_sub` keeps a malformed short string from underflowing.
///
/// **A FLOORED FIGURE CANNOT REPORT AN OVERAGE.** Even with the estimate right,
/// anything from one byte to a megabyte over the cap floors to 256, so the sentence
/// would still have equated the two numbers for the whole first mebibyte past the
/// limit. The reported size is rounded UP: a refusal now always names a number
/// strictly greater than the limit it cites. Overstating by under a megabyte, under
/// the word "about", is the right direction — a refusal that reads as though the
/// file fitted is the failure being fixed.
///
/// The estimate is split out so the boundary can be tested for nothing. Proving
/// what happens to a file of exactly 256 MiB through `decode_import` itself means
/// allocating the payload AND its base64 form — about 600 MB, which is why the only
/// test that did it is `#[ignore]`d and why the off-by-two lived at the one size
/// nothing could afford to check.
fn decoded_size_estimate(data: &str) -> usize {
    let padding = data
        .as_bytes()
        .iter()
        .rev()
        .take_while(|&&b| b == b'=')
        .count()
        .min(2);
    (data.len() / 4 * 3).saturating_sub(padding)
}

fn decode_import(filename: &str, data: &str) -> error::Result<Vec<u8>> {
    use base64::Engine as _;
    let approx = decoded_size_estimate(data);
    if approx > MAX_IMPORT_BYTES {
        return Err(error::Error::refused(format!(
            "{filename} is about {} MB, and Relay imports files up to {} MB. \
             Shorten or compress it and try again.",
            approx.div_ceil(1024 * 1024),
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
    // WHAT CODEC A CLIP CARRIES, while the bytes are already here (F5, 2026-09-21).
    // An iPhone `.mov` is HEVC, which the projector's own window decodes and an
    // OBS browser source or a Windows screen may not. Relay does not transcode;
    // it records the answer so the Library tile and the Planner's preview can
    // warn where the clip is chosen and where the cue is built.
    let codec = if kind == "video" {
        mediaprobe::codec_hint(&bytes).map(str::to_string)
    } else {
        None
    };
    let dir = db::media_dir();
    std::fs::create_dir_all(&dir)?;

    let conn = db.0.lock()?;
    let id = db::insert_media(&conn, &kind, &filename, &date, codec.as_deref())?;
    let path_str = write_media_file(&conn, &dir, id, &filename, &bytes)?;
    db::set_media_path(&conn, id, &path_str)?;
    Ok(db::MediaAsset {
        id,
        kind,
        filename,
        path: path_str,
        created_at: date,
        codec,
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
///
/// **OFF THE MAIN RUN LOOP — RG-299, and for the cap rather than for Tuesday.**
/// `import_guard_tests::what_parsing_a_lyric_import_costs`: a realistic 2 MiB
/// playlist parses in **27 ms**, which is nothing; a text file at
/// `MAX_IMPORT_BYTES` parses in **2,285 ms**, which is a freeze. The guard permits
/// the second, and no church has such a file, so this is not a fix for anything
/// anybody has hit — it is the same trade `import_media` (169 ms at its cap)
/// already took, at ten times the cost, and there is nothing to pay for it with:
/// this command takes no `State` and no handle, so it is a pure function of two
/// strings and two of them running at once share nothing. Also unlike
/// `load_stt_model`, there was no mutual exclusion here for the main thread to
/// have been quietly providing.
#[tauri::command(async)]
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
/// This is the one place a countdown is created, and since 2026-09-21 it is the
/// only place one is touched at all: `adjust_countdown` re-aimed and held one and
/// was deleted with the Screen Countdown's transport (DECISIONS §115).
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
            // A countdown that has just been STARTED is running, always. Nothing
            // holds one any more: `adjust_countdown` was the only door and it went
            // with the transport (DECISIONS §115). A plan cue can still carry a
            // held figure, which is why the field stays.
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
    broadcast_with_clock(
        &app,
        countdown_content(&timer, tid, tjson, tpinned),
        router_clock_ms(),
    )?;
    persist_cue(&app, "countdown", None);
    Ok(())
}

/// Epoch milliseconds. The clock every timer command reads, so they cannot disagree
/// about "now" within one press.
/// BRING THE SAVED CLOCKS BACK INTO THE REGISTRY, at launch — and tell the stage,
/// which is the one screen a timer reaches without a content frame.
///
/// Generic over the runtime (rule 24) so `e2e.rs` can drive it. It publishes NO
/// content frame: a congregation countdown that was on a wall when Relay quit
/// comes back into the registry, where Live's Screen Countdown band offers it
/// (`cdBack`, "counting, off the screens") and **Put back** returns it to the
/// screens — the same rule crash recovery keeps for a verse (position restored,
/// on-air-ness deliberately not). What is brought back is `db::restorable`'s
/// answer, and the number is printed so the boot log says what a relaunch did.
fn restore_timers<R: tauri::Runtime>(app: &tauri::AppHandle<R>, now_ms: i64) -> usize {
    let saved = {
        let db = app.state::<Db>();
        let conn = match db.0.lock() {
            Ok(c) => c,
            Err(e) => e.into_inner(),
        };
        db::load_timers(&conn)
    };
    let (next_id, rows) = match saved {
        Ok(v) => v,
        Err(e) => {
            eprintln!("timers: could not read the saved clocks ({e})");
            return 0;
        }
    };
    let keep = db::restorable(rows, now_ms);
    let n = keep.len();
    if n > 0 {
        let wall = keep
            .iter()
            .filter(|t| t.scope == timers::Scope::Both)
            .count();
        println!(
            "timers: restored {n} clock(s) from the last run ({wall} for the screens, held for Put back)"
        );
    }
    app.state::<timers::TimerRegistry>().restore(next_id, keep);
    channels::publish_timers(app);
    n
}

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
/// is deliberate: `start_countdown` is now the ONLY thing that may put a timer on a
/// congregation screen — `show_timer` was the second until §115 — and another door
/// into that would be
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
/// `adjust_countdown` was the same action aimed at "whichever congregation timer is
/// running" and is gone (§115); this one names the timer, so
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
            broadcast_with_clock(&app, content, router_clock_ms())?;
        }
    }
    channels::publish_timers(&app);
    Ok(())
}

/// It publishes nothing: changing a number on a timer that is not on the screens
/// must not put it on them. (`adjust_countdown` kept the same rule until §115.)
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
    // registry — the rule `adjust_countdown` kept too, before §115 deleted it.
    if adjusted.scope == timers::Scope::Both {
        if let Some(mut content) = channels::live_content(&app).filter(is_countdown_content) {
            let shown = timers::project_both(&adjusted);
            content.countdown_to = Some(shown.countdown_to);
            content.countdown_paused_ms = shown.countdown_paused_ms;
            content.trace_id = None;
            broadcast_with_clock(&app, content, router_clock_ms())?;
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

/// Build the wire form of a `Both` timer. **The one place a timer becomes content.**
/// It had three callers and has one; the guarantee it was written for — that two
/// doors onto a wall cannot disagree about the same countdown — is now kept by
/// there being one door (§115).
fn countdown_content(
    timer: &timers::Timer,
    template_id: Option<i64>,
    template_json: Option<String>,
    template_pinned: bool,
) -> channels::OutputContent {
    let shown = timers::project_both(timer);
    channels::OutputContent {
        kind: Some("countdown".into()),
        // OFF THE TIMER, NOT OFF THE CALL. This function had three callers —
        // `start_countdown`, `adjust_countdown` and `show_timer`, the last two
        // deleted in §115 — and a screen set that lived on the argument would have
        // been correct at the first and lost
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
        router_clock_ms(),
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
        router_clock_ms(),
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
/// Requirement 11's transport. Each argument is a re-aim, the shape
/// `adjust_countdown` used before §115 removed it: `None` means "leave that alone",
/// so Pause cannot un-loop and Loop
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
///
/// **It DOES hand back the frame it published (RG-260), and that is not the same
/// claim.** A frame is the instruction; the beat is the outcome. This returned
/// `()` and the console rebuilt its own copy from the arguments it had passed
/// in, which carried no `replay_epoch` and no `seek_epoch` — so the console's
/// own preview could act on neither a replay nor a scrub, and diverged from
/// every screen in the building the moment either was pressed. One instruction
/// now has one shape.
#[tauri::command]
fn set_media_transport<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    transport: tauri::State<'_, channels::MediaTransport>,
    paused: Option<bool>,
    // `looping`, not `loop`: the wire says `loop` and Rust cannot.
    looping: Option<bool>,
    replay: Option<bool>,
    // WHERE THE HANDLE WAS DROPPED, in milliseconds (RG-221). `None` leaves the
    // clip where it is; `Some` is an instruction and bumps its own counter, so a
    // screen joining later cannot be dragged back by a retained frame.
    seek_ms: Option<i64>,
    // The room's level, 0.0-1.0. `None` leaves it alone — a Pause that also
    // reset the sound to full is the shape of every "one control moved another"
    // bug this transport exists to avoid.
    volume: Option<f64>,
) -> error::Result<channels::TransportFrame> {
    let frame = transport.apply(paused, looping, replay.unwrap_or(false), seek_ms, volume);
    channels::media_transport(&app, frame);
    Ok(frame)
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
/// unstyled. The timer registry adds no key: `start_countdown` asks for
/// `"countdown"`, which is the row that already exists, because a
/// congregation timer's content kind did not change. A `Stage`-scoped timer asks
/// nothing of this function: it renders on the stage page, which has no
/// congregation template to resolve.
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
/// Which build this is — `<short sha>[+dirty] <date>` (S13). Read-only.
#[tauri::command]
fn build_marker() -> &'static str {
    diagnostics::BUILD
}

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
///
/// ## Generic over the runtime, for rule 24's own reason
///
/// It was welded to `AppHandle<Wry>`, so the one artefact in Relay that is expected
/// to leave the building could not be driven by a test without a window — which is
/// exactly the argument rule 24 makes about the fire path. `channels::open_channel_ids`
/// came with it, because it is the only concrete thing on this path.
/// `diagnostic_bundle_tests::what_the_diagnostic_bundle_costs` is what that bought
/// (RG-299): the command timed whole rather than its pieces added up.
#[tauri::command]
fn export_diagnostics<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> error::Result<String> {
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
            format!(
                "{} · {}",
                if cfg!(debug_assertions) {
                    "development"
                } else {
                    "release"
                },
                diagnostics::BUILD
            ),
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

    /// WHAT THE DIAGNOSTIC BUNDLE COSTS THE MAIN RUN LOOP — RG-299.
    ///
    /// One of the two commands on that row's own list that had never been timed.
    /// It was on the list because it does real work — `sysprobe::read` refreshes
    /// the disk list, `latency::report(0)` walks the histograms, four database
    /// reads, and a file write — and the row will not say a command is fine until
    /// somebody has put a number on it. The whole command, not its pieces: a bench
    /// of the parts is how a slow whole goes unnoticed.
    ///
    /// `Stt(None)` is what `build_stt` returns on a machine with no model
    /// downloaded, which is the one the bundle is most often exported from — a
    /// church that cannot get Relay listening. There is no whisper in this harness
    /// to make it anything else.
    ///
    /// Reports, asserts nothing about a clock.
    #[test]
    #[ignore = "a bench: measures this machine"]
    fn what_the_diagnostic_bundle_costs() {
        let app = qa::bare_app();
        app.handle().manage(Stt(Mutex::new(None)));
        // And a hub. `bare_app` manages none on purpose — that is its "no LAN" case
        // — and the bundle reports the kiosk client count, so without one this
        // panics rather than measuring anything.
        app.handle().manage(channels::KioskHub::default());

        // THE COMMAND FIRST, and twice. The first export in a process pays for the
        // OS disk enumeration `sysprobe::read` does, and that is the one an operator
        // actually waits through — measuring the pieces first would warm it and
        // report the second export as if it were the first.
        for pass in ["cold", "warm"] {
            let t = std::time::Instant::now();
            let out = export_diagnostics(app.handle().clone());
            println!(
                "  export_diagnostics ({pass:<4})           {:>6} ms   {}",
                t.elapsed().as_millis(),
                match &out {
                    Ok(p) => format!(
                        "wrote {} bytes",
                        std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
                    ),
                    Err(e) => format!("refused: {}", e.message()),
                }
            );
            if let Ok(p) = out {
                let _ = std::fs::remove_file(p);
            }
        }
        // Then the two pieces with any reason to be slow, so a number that moves
        // can be attributed without re-deriving this. Warm by now, deliberately:
        // the cold cost is in the line above, where it belongs.
        let t = std::time::Instant::now();
        let _hw = sysprobe::read(&db::app_data_dir());
        println!(
            "  sysprobe::read (memory + disk list) {:>6} ms   (warm)",
            t.elapsed().as_millis()
        );
        let t = std::time::Instant::now();
        let _r = latency::report(0);
        println!(
            "  latency::report(0)                  {:>6} ms",
            t.elapsed().as_millis()
        );
        println!();
    }

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
    // WHAT A LOST MICROPHONE IS DOING ABOUT ITSELF (RG-291, DECISIONS §119).
    // Separate from `err_emitter` because the two say different things: an error
    // is the end of an attempt, and this is the attempt after it.
    let recovery_emitter = app.clone();
    // DISCONNECTED USED TO BE THE ONE SILENT ANSWER ON THIS PATH, and it is the
    // worst of the three. FULL is a backlog and is counted; OK is the normal case;
    // DISCONNECTED means the whisper worker is gone — it failed to create its
    // state, or the engine it belonged to was replaced underneath a running
    // capture — and every chunk from here to the end of the service falls on the
    // floor. The level meter still moves, the console still says Listening, and
    // not one word is ever transcribed again. Reported ONCE, on the same channel
    // as a dead microphone, because to an operator it is the same news.
    let stt_gone = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stt_dead_emitter = app.clone();
    // Throttle the level-meter event: chunks arrive ~5/sec but the UI only needs
    // a couple updates/sec. Flooding the webview with events is a real freeze
    // risk. STT still gets EVERY chunk.
    let chunk_n = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    // Quality snapshots arrive per processed block (~many/sec) — throttle to a
    // couple/sec on their own additive channel. Existing UI ignores it.
    let quality_n = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    // Non-blocking: returns instantly, so the UI thread never stalls on device
    // init. Stream failures surface as `audio://error`.
    let engine = AudioEngine::start_with_recovery(
        device,
        move |chunk| {
            let n = chunk_n.fetch_add(1, Ordering::Relaxed);
            // EVERY SECOND CHUNK, not every third. Chunks are 400 ms on a 200 ms
            // hop, so every second one covers the timeline exactly once with no
            // overlap and no gap; every third left two thirds of the audio out of
            // the console's picture entirely, which is why no drawing could make
            // that picture a waveform. The event RATE is unchanged in the way that
            // matters — ~2.5/s against ~1.7/s — while the readings it carries go
            // from one per 600 ms to one per 25 ms.
            if n.is_multiple_of(2) {
                let _ = emitter.emit(
                    "audio://chunk",
                    ChunkEvent {
                        timestamp_ms: chunk.timestamp_ms,
                        sample_rate: chunk.sample_rate,
                        rms: chunk.rms,
                        is_voice: chunk.is_voice,
                        samples: chunk.samples.len(),
                        peaks: audio::envelope(&chunk.samples, audio::CHUNK_PEAKS),
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
                match tx.try_send(chunk.clone()) {
                    Err(std::sync::mpsc::TrySendError::Full(_)) => latency::note_dropped_audio(),
                    Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                        if !stt_gone.swap(true, Ordering::Relaxed) {
                            let msg = "speech recognition has stopped — the transcript \
                                       and automatic scripture detection are not running. \
                                       Stop and start the microphone to bring them back."
                                .to_string();
                            eprintln!("audio: {msg}");
                            let _ = stt_dead_emitter.emit("audio://error", msg);
                        }
                    }
                    Ok(()) => {}
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
        // ── THE THREE STATES OF A MICROPHONE THAT WENT AWAY ────────────────────
        //
        // `lost`, `listening`, `gave_up` — and they must stay apart, because rule
        // 35 is the whole point of this event: a console that says the same thing
        // while Relay is retrying, while Relay is listening again, and while Relay
        // has given up is a console that says nothing. `Recovery` carries the
        // reason, the attempt number and how long until the next one, so the shell
        // can say which of the three it is without re-deriving anything.
        //
        // The line is printed as well as emitted, for the same reason every other
        // audio outcome is: on a machine that cannot screenshot the app, stdout is
        // the record of what happened during a service.
        move |r: audio::Recovery| {
            eprintln!("audio: {}", r.describe());
            let _ = recovery_emitter.emit("audio://recovery", r);
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
/// IMPORT A BIBLE from a JSON file in the KJV's shape (RG-50 option two, DECISIONS
/// §113). Held back during a service: a rebuild of the FTS index and a new
/// translation row under a running detector is not a Sunday job.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // one command, one file, six things about it — a struct would only rename the eight
fn import_translation(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    name: String,
    abbreviation: String,
    language: String,
    license_type: String,
    filename: String,
    data: String,
) -> error::Result<db::ImportedTranslation> {
    lock.guard("import_translation")?;
    let bytes = decode_import(&filename, &data)?;
    let json = String::from_utf8(bytes).map_err(|_| {
        error::Error::refused(format!(
            "{filename} is not a text file. Relay reads a Bible as UTF-8 JSON."
        ))
    })?;
    let conn = db.0.lock()?;
    db::import_translation(&conn, &name, &abbreviation, &language, &license_type, &json)
        .map_err(error::Error::refused)
}

/// DELETE AN IMPORTED BIBLE. The two Relay ships and the active one are refused
/// in `db::delete_translation`, with the reason.
#[tauri::command]
fn delete_translation(
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    lock.guard("delete_translation")?;
    let conn = db.0.lock()?;
    db::delete_translation(&conn, id).map_err(error::Error::refused)
}

#[tauri::command]
fn set_active_translation<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    lock: tauri::State<'_, servicelock::ServiceLock>,
    id: i64,
) -> error::Result<()> {
    lock.guard("set_active_translation")?;
    {
        let conn = db.0.lock()?;
        db::set_setting(&conn, "active_translation", &id.to_string())?;
    }
    // AND THE INDEXES THAT READ THAT SETTING ARE REBUILT — RG-300.
    //
    // `Semantic` and `Phrases` were built ONCE, in `setup`, from
    // `db::all_verses`, which scopes itself to the active translation. Writing
    // the setting and stopping there left both detectors scanning the PREVIOUS
    // translation's corpus until the app was relaunched — while every verse READ
    // was correctly scoped to the new one. So the console would show BSB words
    // for a reference the paraphrase detector found in KJV vocabulary, and
    // nothing on any surface would say the two disagreed: a wrong-verse risk
    // wearing a settings bug's clothes.
    //
    // The lock is DROPPED above before this runs, because `rebuild_corpus_indexes`
    // takes it again — the ordinary rule, stated here because the write and the
    // rebuild read as one action and are not.
    rebuild_corpus_indexes(&app)
}

/// Rebuild the two corpus indexes from whatever translation is active now.
///
/// **The one door**, per rule 36: anything that changes which verses Relay should
/// be scanning calls this, rather than each caller remembering two `build`s. It
/// is ~305 ms of work over 31,102 verses, and it is deliberately synchronous —
/// the alternative is a window that says the translation changed while the
/// detectors have not caught up, which is the defect being fixed wearing a
/// progress bar.
///
/// **It cannot run during a service.** Every caller is behind
/// `ServiceLock::guard`, so the live path never meets a write lock here. That is
/// what makes a plain `RwLock` the right shape: readers never block each other,
/// and the only writer is an operator at a settings screen between services.
fn rebuild_corpus_indexes<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> error::Result<()> {
    let corpus: Vec<(VerseRef, String)> = {
        let db = app.state::<Db>();
        let conn = db.0.lock()?;
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
    // BOTH, OR THE PAIR DISAGREES. A quotation index over one translation and a
    // paraphrase index over another is worse than either being stale.
    let phrases = detection::PhraseIndex::build(&corpus);
    let semantic = SemanticIndex::build(&corpus);
    if let Ok(mut g) = app.state::<Phrases>().0.write() {
        *g = phrases;
    }
    if let Ok(mut g) = app.state::<Semantic>().0.write() {
        *g = semantic;
    }
    Ok(())
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
    // ONE SHAPE FOR THE GATE, EVERYWHERE. This returned a bare `Thresholds`, and
    // the console wrote that straight into its mirror — so a confirm updated the
    // two numbers and left `on_dial` alone, on the ONE path that can move the gate
    // off the dial's curve. `record_feedback` is precisely what §96 says makes the
    // dial position stop explaining the gate, and the belt-and-braces writer for
    // it carried three of the four facts. Same struct as `get_thresholds` and the
    // same fields `detection://thresholds` carries, so no surface can learn a
    // different thing from a different door.
) -> error::Result<GateReadout> {
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
        // `confidence_is_calibrated`, not `may_auto_fire`. The two were one
        // question while `Direct` answered both; DECISIONS §118 split them. A
        // `Reading` MAY reach a wall and its number is a word count, so it must
        // not be handed to a bar that means a parse probability.
        Some(c) if accepted_method.confidence_is_calibrated() => Some(c.clamp(0.0, 1.0)),
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
    Ok(t.into())
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
    // `GateReadout`, not `Thresholds` — see `confirm_detection`.
) -> error::Result<GateReadout> {
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
    Ok(t.into())
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
/// (`audits/DESIGN.md` §6).
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
            // THE TWO FIGURES AS AN OPERATOR READS THEM, derived in Rust beside
            // the curve they are a question about (DECISIONS §117). The printed
            // pair used to be `auto_fire`/`suggest` straight off this struct,
            // which runs the opposite way to the dial they are printed under.
            // A second copy of `100 - x` in the frontend would be a second
            // opinion about one gate, which is what §96 deleted a control for.
            "readiness": t.readiness(),
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
#[derive(Serialize, Debug, Clone, Copy)]
struct GateReadout {
    auto_fire: f32,
    suggest: f32,
    /// The same pair on the 0-100 scale that rises with the dial. See
    /// `Thresholds::readiness`.
    readiness: router::GateReadiness,
    sensitivity: u8,
    on_dial: bool,
}

impl From<Thresholds> for GateReadout {
    fn from(t: Thresholds) -> Self {
        GateReadout {
            auto_fire: t.auto_fire,
            suggest: t.suggest,
            readiness: t.readiness(),
            sensitivity: t.to_sensitivity(),
            on_dial: t.follows_dial(),
        }
    }
}

/// The live gate: the two thresholds, the dial position they map back to, and
/// whether that dial position actually explains them.
#[tauri::command]
fn get_thresholds(routing: tauri::State<'_, Routing>) -> error::Result<GateReadout> {
    Ok(routing.0.lock()?.thresholds().into())
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
/// **Generic over the runtime, per rule 24.** Everything it calls already is —
/// `persist_transcript`, `handle_nav`, `handle_passage_nav`, `clear_or_report`,
/// `emit_detections` — and it was the one concrete link in that chain, which is
/// what made the whole window-handling step undrivable from `e2e.rs`: a fact this
/// function records about a window could only be checked by reading it. RG-113(5)
/// is exactly such a fact.
fn handle_transcript<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    lang_stability: &Mutex<stt::LanguageStability>,
    update: stt::TranscriptUpdate,
) {
    // **RG-113(5) · THE ONE WRITER, AND IT RUNS FOR A PARTIAL TOO.** Everything
    // below this line — including `emit_detections` and the `persist_fire` inside it
    // — is about THIS window, so the language is recorded before any of it. It has
    // to be here rather than in `persist_transcript`, which only ever sees finals:
    // the evidence row `persist_fire` writes exists precisely because the window was
    // a partial the last final does not contain. Taken and released on its own, as
    // rule 2 requires, and it holds no other lock.
    if let Ok(mut sess) = handle.state::<Session>().0.lock() {
        if let Some(st) = sess.as_mut() {
            if st.last_language != update.language {
                st.last_language = update.language.clone();
            }
        }
    }
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
        // **A FORCED CLOSE IS NOT AN UTTERANCE END — RG-262, and this is the
        // line that keeps rule 34.** A window that filled up is closed so its
        // text is kept, but the preacher is still speaking, so the next pass IS
        // coming. Rule 28's corroboration exemption rests on exactly the opposite
        // ("a FINAL window is exempt — no next pass is coming"), and handing it
        // `true` here would let an eight-second window auto-fire a reference the
        // decoder has not yet had a chance to revise. Four fifths of a sermon was
        // being lost; recovering it may not cost a single one of rule 10's,
        // 28's or 30's guarantees.
        update.is_final && !update.continued,
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
///
/// **AND THEREFORE OFF THE MAIN RUN LOOP AS WELL — RG-299.** This is the door an
/// operator actually presses in `Settings → Speech`; `load_stt_model` is the one
/// the model-installation flow calls. They cost the same 1,097 ms-to-3.6 s, because
/// the second is the last line of the first. A guarantee kept on one of two doors
/// is this repository's most-repeated bug, and here it would mean the window
/// freezing on the path a person uses and not on the path a wizard does.
#[tauri::command(async)]
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

/// FOLLOW THE READER, or do not. The church's one switch over what a quotation
/// may do (DECISIONS §118).
///
/// ## Why this is a command and not `set_setting`
///
/// Writing the row alone would change nothing until the next launch, while the
/// switch on screen showed the new position — so the operator would be told they
/// had turned it off and Relay would go on firing readings for the rest of the
/// service. Choosing and applying are one action or the promise is false, which
/// is rule 15 and the same reason `select_stt_model` is its own command.
///
/// ## Order, and why this one is the way round it is
///
/// The ROW is written first and the router second. A write that fails must not
/// leave the engine following a reader that nothing remembers — the next launch
/// would silently put it back. Same reasoning as `set_stt_language` (RG-138).
///
/// Behind the service lock: this decides what the AI may put on a congregation's
/// screen unasked, and changing that under a running service is exactly the class
/// of thing `servicelock.rs` exists for.
#[tauri::command]
fn set_follow_the_reader<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    routing: tauri::State<'_, Routing>,
    on: bool,
) -> error::Result<bool> {
    app.state::<servicelock::ServiceLock>()
        .guard("set_follow_the_reader")?;
    {
        let conn = db.0.lock()?;
        db::set_follow_the_reader(&conn, on)?;
    }
    {
        let mut r = routing.0.lock()?;
        r.set_follow_the_reader(on);
    }
    Ok(on)
}

/// Is Relay following a reader right now? Read from the ROUTER, not the row — the
/// router is what decides, and a surface asking the database would be asking a
/// different question that usually has the same answer.
#[tauri::command]
fn get_follow_the_reader(routing: tauri::State<'_, Routing>) -> error::Result<bool> {
    Ok(routing.0.lock()?.follows_the_reader())
}

/// **MUST A PARAPHRASE ECHO THE VERSE?** The church's one switch over what the
/// paraphrase detector is allowed to put in front of an operator (DECISIONS §125,
/// RG-311).
///
/// ## Why this is a command and not `set_setting`
///
/// The same reason as `set_follow_the_reader`: writing the row alone would change
/// nothing until the next launch while the switch on screen showed the new
/// position, so the operator would be told the noise had stopped and it would go on
/// for the rest of the service. Choosing and applying are one action or the promise
/// is false (rule 15).
///
/// ## Order — the ROW first, the engine second
///
/// `set_follow_the_reader`'s order, for `set_stt_language`'s reason (RG-138): a
/// write that fails must not leave the detector running under a rule nothing
/// remembers, because the next launch would silently put it back.
///
/// ## NOT behind the service lock, and this is a deliberate divergence
///
/// Its structural twin `set_follow_the_reader` IS locked, because turning that on
/// mid-service changes what reaches a CONGREGATION with nobody pressing anything.
/// This one cannot: `Semantic` is capped at `Suggest` by rule 10 at any score and
/// any setting, so the switch only ever removes rows from the operator's own list.
/// `servicelock.rs` protects two things — the irreversible, and anything that takes
/// the engine away mid-sermon — and this is neither: it is one click each way and it
/// stops nothing. The nearest precedent is therefore `set_sensitivity`, which the
/// lock's own module note names as explicitly unprotected, and for the same reason:
/// **the operator who most needs this is the one drowning in suggestions at 10:31**,
/// and over-blocking is the more dangerous failure there.
#[tauri::command]
fn set_paraphrase_needs_a_run<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    on: bool,
) -> error::Result<bool> {
    {
        let conn = db.0.lock()?;
        db::set_paraphrase_needs_a_run(&conn, on)?;
    }
    app.state::<ParaphraseRun>().0.store(on, Ordering::Relaxed);
    Ok(on)
}

/// Is the paraphrase bar on right now? Read from the STATE the detection path
/// reads, not from the row — the same rule `get_follow_the_reader` follows, so a
/// surface cannot show a preference that the detector is not applying.
#[tauri::command]
fn get_paraphrase_needs_a_run<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> error::Result<bool> {
    Ok(app.state::<ParaphraseRun>().0.load(Ordering::Relaxed))
}

/// Bring speech recognition up after a model has just been installed, without a
/// restart. Re-applies the active voice profile so language + decoder bias are
/// live from the first word.
///
/// **OFF THE MAIN RUN LOOP, AND ONLY BECAUSE OF THE GUARD — RG-299.** 1,097 ms warm
/// and about 3.6 s on a cold 1.6 GB read is a freeze on the window's thread, and a
/// run loop the decoder's own emit can wait on (rule 2). What the main thread was
/// providing was mutual exclusion, so `ModelLoad` has to hold that before
/// `#[tauri::command(async)]` may take it away — see `ModelLoad` for the 3.2 GB
/// two concurrent builds would hold while racing to write one slot. Faster by less
/// safe is rule 34; this is faster, having paid for it.
#[tauri::command(async)]
fn load_stt_model(app: tauri::AppHandle) -> error::Result<bool> {
    // Rebuilding the engine takes the ears away for as long as whisper takes to
    // load, which on a big model is most of a paragraph.
    app.state::<servicelock::ServiceLock>()
        .guard("load_stt_model")?;
    // Held for the whole load, released however this returns. The refusal is a
    // sentence an operator can act on, not a fault: nothing is broken, the thing
    // they asked for is already happening.
    // Bound, not chained: `State` is a temporary and the guard borrows from it.
    let one_at_a_time = app.state::<ModelLoad>();
    let Some(_in_flight) = one_at_a_time.begin() else {
        return Err(error::Error::refused(
            "A speech model is already loading. Wait for it to finish.",
        ));
    };
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
///
/// **OFF THE MAIN RUN LOOP — RG-299.** `#[tauri::command(async)]` on a synchronous
/// function is Tauri's own way of saying "do not run this on the window's thread",
/// and it keeps `State<'_, T>` working where an `async fn` would not. Measured on
/// this machine (`models::tests::what_the_model_flow_costs`): **7,086 ms** for
/// `ggml-large-v3-turbo` — hash the source, copy it, hash the copy — and the copy
/// itself was free only because APFS cloned it on the same volume. From the USB
/// stick this feature exists for it is minutes. Every one of those milliseconds was
/// a frozen window with no spinner, no progress and nothing in any log.
///
/// What the main thread was silently providing was MUTUAL EXCLUSION, and that is
/// paid for properly now: `models::install_from_file` copies to a scratch name
/// unique per attempt, so two installs cannot interleave into one file.
#[tauri::command(async)]
fn install_model_file(
    lock: tauri::State<'_, servicelock::ServiceLock>,
    path: String,
) -> error::Result<String> {
    lock.guard("install_model_file")?;
    models::install_from_file(std::path::Path::new(&path)).map_err(error::Error::refused)
}

/// Model files already sitting on this machine, waiting to be installed.
///
/// Three folders, no recursion, size as a pre-filter before anything is hashed.
///
/// **IT IS NOT CHEAP, AND THIS DOC COMMENT SAID IT WAS** (RG-299). The size filter
/// decides what gets hashed, not whether anything does: a file whose length matches a
/// catalogue entry is SHA-256'd in full, and the catalogue's largest entry is 1.6 GB.
/// Measured on this machine by `models::tests::what_the_model_flow_costs`: **3,543 ms**
/// for that one file at 437 MB/s, and **5,025 ms** with all three installed models
/// present. As a plain `#[tauri::command]` every one of those milliseconds was the
/// macOS window's run loop, held by a screen whose whole job is to look for a file —
/// an app that appears to have hung, five seconds after a click, under a doc comment
/// asserting the opposite.
///
/// `#[tauri::command(async)]` moves a SYNCHRONOUS command off that thread, which is
/// the small lever this needed: no `async fn`, so `State<'_, T>` keeps working. It is
/// safe off the main thread because it is a read — it opens files, hashes them and
/// returns; two concurrent scans agree, and neither writes anything.
#[tauri::command(async)]
fn find_model_files() -> Vec<models::FoundModel> {
    models::scan_for_models()
}

/// THE COMMANDS THAT MAY NOT GO BACK ON THE MACOS RUN LOOP — RG-299.
///
/// Tauri v2 runs a command that is neither `async fn` nor `#[tauri::command(async)]`
/// on the main thread, and on macOS that thread IS the window's run loop. Most of
/// Relay's 169 commands are a SQLite read and belong there: sync commands are
/// serialised by that thread, which is a real guarantee, and spending it buys
/// nothing on a one-millisecond query.
///
/// These two are the ones that were measured and found to matter. Nothing about the
/// attribute looks load-bearing, and `cargo fmt` will not restore it — so a tidy-up
/// that deletes `(async)` puts a **five-second frozen app** back, silently, on the
/// screen a church uses to install speech recognition from a USB stick.
///
/// Deliberately a list of two and not a rule about all of them. A scanner that
/// guessed which command is "slow" would fail on legitimate code, be weakened, and
/// take these with it. The measurement lives in
/// `models::tests::what_the_model_flow_costs`; the survey of what the rest cost is
/// in RELAY_GAP's RG-299 row.
#[cfg(test)]
mod main_loop_tests {
    /// Every command named here has been MEASURED at hundreds of milliseconds or
    /// more, and must stay off the main thread. A list, not a rule about the 160
    /// that stay on it: nothing about `(async)` looks load-bearing, `cargo fmt` will
    /// not restore it, and a one-millisecond SQLite read gains nothing from a thread
    /// hop.
    ///
    /// The measurements, all RG-299, each with the bench that produced it:
    ///
    /// * `find_model_files` — 3,543 ms for one 1.6 GB file, 5,025 ms with three
    ///   installed. The only one of these a service lock does not hold back.
    /// * `install_model_file` — 7,086 ms for `large-v3-turbo`.
    ///   (`models::tests::what_the_model_flow_costs`)
    /// * `load_stt_model` — 1,097 ms warm, ~3.6 s on a cold 1.6 GB read. It needed
    ///   `ModelLoad` first: the main thread was providing mutual exclusion, and two
    ///   builds at once would hold 3.2 GB while racing to write one `Stt` slot.
    /// * `select_stt_model` — the same cost by definition; it ends by calling
    ///   `load_stt_model`. Its twin, and a guarantee kept on one of two doors is
    ///   this repository's most-repeated bug.
    /// * `parse_import` — 2,285 ms at `MAX_IMPORT_BYTES`, 27 ms for a realistic
    ///   2 MiB playlist. (`import_guard_tests::what_parsing_a_lyric_import_costs`)
    ///
    /// **`export_diagnostics` is deliberately NOT here.** It was the other command
    /// this row named as never timed, and it is **16 ms** end to end
    /// (`diagnostic_bundle_tests::what_the_diagnostic_bundle_costs`). Measuring
    /// returned a no, which is a result.
    const OFF_THE_RUN_LOOP: [&str; 5] = [
        "find_model_files",
        "install_model_file",
        "load_stt_model",
        "select_stt_model",
        "parse_import",
    ];

    fn source() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
            .expect("read main.rs")
    }

    /// The attribute line directly above a command's DEFINITION.
    ///
    /// Line-by-line, and anchored at the start of a line, because the name also
    /// appears in this module as a string literal and in the doc comments above the
    /// commands themselves — a `str::find` for `fn <name>(` matched one of those and
    /// returned `.find("`, which is a scanner reading its own source.
    fn attribute_above(src: &str, name: &str) -> Option<String> {
        let signature = format!("fn {name}(");
        let lines: Vec<&str> = src.lines().collect();
        lines.iter().enumerate().find_map(|(i, l)| {
            (l.starts_with(&signature) && i > 0).then(|| lines[i - 1].trim().to_string())
        })
    }

    #[test]
    fn the_commands_measured_to_freeze_the_window_stay_off_the_main_thread() {
        let src = source();
        for name in OFF_THE_RUN_LOOP {
            let attr = attribute_above(&src, name)
                .unwrap_or_else(|| panic!("{name} is gone — update this list or remove it"));
            assert_eq!(
                attr, "#[tauri::command(async)]",
                "{name} is `{attr}` — it was measured at seconds of file I/O, and a plain \
                 `#[tauri::command]` runs it on the macOS window's run loop (RG-299). Put \
                 `#[tauri::command(async)]` back, or take it out of OFF_THE_RUN_LOOP with a \
                 measurement that says why."
            );
        }
    }

    /// **THE GUARD THAT PAID FOR `load_stt_model` GOING ASYNC — RG-299.**
    ///
    /// Two concurrent loads would each build a whisper context (3.2 GB for two
    /// `large-v3-turbo`) and both write the same `Stt` slot, and the reason that
    /// could not happen before is that the main run loop was serialising them.
    /// Taking the serialisation away and not replacing it is rule 34.
    ///
    /// Put the defect back by returning `Some(ModelLoadGuard(&self.0))`
    /// unconditionally from `begin` and this fails.
    #[test]
    fn only_one_model_load_may_be_in_flight() {
        let load = super::ModelLoad::default();
        let first = load.begin().expect("the first caller gets the slot");
        assert!(
            load.begin().is_none(),
            "a second load started while the first was still building — two whisper \
             contexts, 3.2 GB, both writing one slot"
        );
        drop(first);
        assert!(
            load.begin().is_some(),
            "the slot was not released, so one bad load kills the feature until \
             Relay is restarted — `models::RunningGuard` learned this the hard way"
        );
    }

    // ── THE TRIPWIRE FOR THE NEXT ONE — RG-299 ──────────────────────────────
    //
    // `OFF_THE_RUN_LOOP` above is a list of five commands that were measured and
    // moved. It says nothing about the 164 that stayed, and RG-299's last open
    // clause was exactly that: **nothing stops the next slow command being added to
    // the run loop without anybody measuring it.** The five were not found by
    // reasoning about what looks slow; they were found by a survey, and the survey
    // was written down as reproducible — *"walk each command body for `fs::`,
    // hashing, `decode_import`, a bundle or a model load"*. A survey that is
    // reproducible by hand is a test that nobody has written yet.
    //
    // So this is that survey, run on every build. It is deliberately NOT a rule
    // about which commands are slow — that is the scanner the note above this module
    // refuses, and rightly, because it would guess, fail on legitimate code, be
    // weakened and take the real ones with it. This asks a syntactic question with a
    // syntactic answer: **does this command's own body do bulk work with a file, a
    // hash, an import or a model?** If it does, it is `(async)`, or it is named
    // below with the measurement that says it may stay.
    //
    // **What it cannot see, stated rather than implied.** It reads the command's own
    // body and nothing a call deep, which is the same boundary the hand survey had:
    // `select_stt_model` does seconds of work and contains none of these markers,
    // because it calls `load_stt_model`. That is why `OFF_THE_RUN_LOOP` exists as a
    // list of names beside this — the two tests answer different questions, and
    // neither subsumes the other. It also cannot see cost that is not file-shaped;
    // `export_diagnostics` is 16 ms and the whole of it is `sysprobe::read`, which
    // this would never have found.

    /// Bulk work, and what each marker is a marker FOR. A body naming one of these
    /// is doing something whose cost is set by how big the thing is, which is the
    /// property that makes a command able to freeze a window.
    ///
    /// `fs::remove_file` is deliberately absent: unlinking a file is one syscall
    /// whatever is in it, and `delete_media` and `remove_demo_content` do exactly
    /// that and nothing else. A marker that catches them would be a marker that
    /// catches everything, and then this list gets weakened.
    const BULK_WORK: [(&str, &str); 15] = [
        ("fs::read", "reads a whole file (or a directory)"),
        ("read_to_string", "reads a whole file"),
        ("File::open", "opens a file to read it"),
        ("File::create", "opens a file to write it"),
        ("fs::write", "writes a whole file"),
        ("fs::copy", "copies a file"),
        ("io::copy", "streams a file"),
        ("read_dir", "enumerates a directory"),
        ("Sha256", "hashes bytes"),
        ("decode_import", "decodes an upload"),
        ("WhisperContext", "loads a speech model"),
        ("build_stt", "loads a speech model"),
        (
            "scan_for_models",
            "enumerates and hashes every installed model",
        ),
        ("install_from_file", "copies and hashes a model"),
        ("write_bundle", "writes the diagnostic bundle"),
    ];

    /// Commands whose own body does bulk work, which are MEASURED and staying on the
    /// run loop. Every entry carries its number and the bench that produced it, and
    /// an entry with neither does not belong here — that is the whole mechanism.
    ///
    /// * `export_diagnostics` — **16 ms** end to end, and `sysprobe::read` is the
    ///   whole of it (`diagnostic_bundle_tests::what_the_diagnostic_bundle_costs`).
    ///   Measuring returned a no, which is a result.
    /// * `export_service` — **~6 ms** for the largest real service, 7,163 transcript
    ///   rows and 549 KB. This is the command RG-299 called out for reading every
    ///   transcript row with no `LIMIT`.
    /// * `import_media` — **169 ms** at the 256 MiB cap, 81 decode and 88 write
    ///   (`import_guard_tests::what_an_import_at_the_cap_costs`). Service locked.
    /// * `import_translation` — **241 ms** for the whole KJV, 31,102 verses out of
    ///   4.3 MB of JSON (`db::verses::imported_translation::what_importing_a_whole_
    ///   bible_costs`). Service locked.
    ///
    /// The four that are NOT here are the four the same survey found and moved; they
    /// are in `OFF_THE_RUN_LOOP` and this test would fail on any of them that came
    /// back, from the other direction.
    const MEASURED_ON_THE_RUN_LOOP: [&str; 4] = [
        "export_diagnostics",
        "export_service",
        "import_media",
        "import_translation",
    ];

    /// One Tauri command as this file declares it: its name, whether its attribute
    /// took it off the run loop, and its own body with comments removed.
    ///
    /// **Comments removed, and that is not tidiness.** The first version of this scan
    /// reported `stt_status` — a lock read and a settings lookup — as loading a
    /// model, because the comment above its first statement mentions
    /// `load_stt_model` by name while explaining lock order. A scanner reading prose
    /// about code instead of code is the same failure `attribute_above` already
    /// records against itself one screen up.
    fn command_bodies(src: &str) -> Vec<(String, bool, String)> {
        let lines: Vec<&str> = src.lines().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            if !lines[i].starts_with("#[tauri::command") {
                i += 1;
                continue;
            }
            let is_async = lines[i].trim() == "#[tauri::command(async)]";
            let mut j = i + 1;
            while j < lines.len()
                && (lines[j].starts_with("#[") || lines[j].trim_start().starts_with("//"))
            {
                j += 1;
            }
            let Some(name) = lines.get(j).and_then(|l| {
                l.strip_prefix("pub ")
                    .unwrap_or(l)
                    .strip_prefix("async ")
                    .unwrap_or(l)
                    .strip_prefix("fn ")
                    .and_then(|r| r.split(['(', '<']).next())
            }) else {
                i = j + 1;
                continue;
            };
            // A command's body ends at the first line that is exactly a closing
            // brace, which is what `rustfmt` guarantees for a top-level item.
            let mut k = j;
            while k < lines.len() && lines[k] != "}" {
                k += 1;
            }
            let body = lines[j..k.min(lines.len())]
                .iter()
                .filter(|l| !l.trim_start().starts_with("//"))
                .map(|l| match l.find("//") {
                    // A trailing comment, unless there is a string before it — a URL
                    // holds a `//` and cutting there would hide real code.
                    Some(c) if !l[..c].contains('"') => &l[..c],
                    _ => *l,
                })
                .collect::<Vec<_>>()
                .join("\n");
            out.push((name.to_string(), is_async, body));
            i = k;
        }
        out
    }

    /// **A NEW COMMAND MAY NOT DO BULK WORK ON THE macOS RUN LOOP UNMEASURED.**
    ///
    /// RG-299's residual, and the reason it could not just be closed: five commands
    /// were measured and moved, 164 stayed, and the next one was going to be added
    /// the same way the first five were — by somebody writing an ordinary
    /// `#[tauri::command]` over an ordinary-looking body. Rule 2 is why it matters
    /// and it is not about a spinning beachball: the STT worker emits
    /// `stt://transcript` from its own thread, and that emit waits on the run loop,
    /// so a command hashing a 1.6 GB file during a service stops the decoder for as
    /// long as it hashes.
    ///
    /// Two directions, because a scanner that quietly narrows passes everything:
    /// every body doing bulk work is accounted for, AND the scanner can still see
    /// every instance we already know about.
    #[test]
    fn a_command_that_does_bulk_work_on_the_run_loop_has_been_measured() {
        let src = source();
        let cmds = command_bodies(&src);
        assert!(
            cmds.len() > 150,
            "the scanner found {} commands in a file that has around 169 — it has \
             stopped parsing this file and would pass whatever is in it",
            cmds.len()
        );

        let mut unaccounted: Vec<String> = Vec::new();
        for (name, is_async, body) in &cmds {
            let Some((_, what)) = BULK_WORK.iter().find(|(m, _)| body.contains(m)) else {
                continue;
            };
            if *is_async || MEASURED_ON_THE_RUN_LOOP.contains(&name.as_str()) {
                continue;
            }
            unaccounted.push(format!("{name} ({what})"));
        }
        assert!(
            unaccounted.is_empty(),
            "these commands do bulk work on the macOS window's run loop and nothing \
             says what it costs: {unaccounted:?}.\nMeasure it. Then either give it \
             `#[tauri::command(async)]` (and pay for the mutual exclusion the main \
             thread was silently providing — see `ModelLoad`), or add it to \
             MEASURED_ON_THE_RUN_LOOP with the number and the bench that produced it. \
             RG-299, rule 2."
        );

        // AND THE SCANNER CAN STILL SEE. Every name below is a body we know does
        // bulk work; if the markers or the parser drift, this fails here rather than
        // silently reporting a clean sweep of nothing.
        let seen: std::collections::HashMap<&str, &String> =
            cmds.iter().map(|(n, _, b)| (n.as_str(), b)).collect();
        for name in MEASURED_ON_THE_RUN_LOOP
            .iter()
            .chain(["find_model_files", "install_model_file", "parse_import"].iter())
        {
            let body = seen
                .get(name)
                .unwrap_or_else(|| panic!("{name} is gone — update these lists"));
            assert!(
                BULK_WORK.iter().any(|(m, _)| body.contains(m)),
                "the scanner can no longer see the bulk work in `{name}`, so it is \
                 passing on a pattern that has stopped matching"
            );
        }
        // …and it does NOT see it where there is none. `stt_status` is a lock read
        // and a settings lookup whose COMMENT names `load_stt_model` while explaining
        // lock order — the false positive the comment-stripping exists for.
        let status = seen.get("stt_status").expect("stt_status");
        assert!(
            !BULK_WORK.iter().any(|(m, _)| status.contains(m)),
            "the scanner is reading comments again: `stt_status` does no bulk work and \
             only its prose mentions any"
        );
    }

    /// And the scanner can still see a plain command, so it is checking something.
    /// A source scanner that quietly stops matching passes everything.
    #[test]
    fn the_scanner_can_tell_the_two_attributes_apart() {
        let src = source();
        assert_eq!(
            attribute_above(&src, "cancel_model_download").as_deref(),
            Some("#[tauri::command]"),
            "the scanner no longer finds a plain command above its signature, so the test \
             above is passing on a pattern that has stopped matching"
        );
    }
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
    let monitors = channels::list_monitors(&app);
    let monitor_index = match resolve_display(channel.display_target.as_deref(), &monitors) {
        DisplayChoice::Missing(n) => {
            return Err(error::Error::refused(format!(
                "{} is set to open on Display {n}, which is not connected. \
                 Plug it in, or choose a different display for this screen.",
                channel.name
            )))
        }
        // NEVER OVER THE CONSOLE (RG-188). `Anywhere` used to fall through to the
        // OS default — the primary display, the one the operator is running the
        // service from — and a borderless fullscreen output covered the console
        // with nothing reported. `auto_open_outputs` had always refused that;
        // the manual path now agrees with it, in a sentence.
        choice => manual_open_target(choice, &monitors)
            .map_err(|why| error::Error::refused(format!("{}: {why}", channel.name)))?,
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
    /// A picture or clip this screen said it could not load (O-4). `None` is the
    /// ordinary case. Read by `describeScreen`, which will not call a screen On Air
    /// over a frame it has said is blank.
    media_error: Option<String>,
    /// How many times this screen fell behind the hub and was re-synced (RG-195,
    /// rule 33). Zero is the ordinary answer; a rising number is a screen or a
    /// network that cannot keep up, and the desk should say so.
    resyncs: u32,
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

/// Where a MANUAL "Turn on" may put a fullscreen output (RG-188). `Ok(Some(idx))`
/// is a chosen, connected, non-primary display; `Ok(None)` is "wherever the OS
/// puts it", allowed only when there is nowhere else to go; `Err` is a sentence
/// for the operator. The primary monitor is the console, and covering it is the
/// one thing a manual open must never do without being told which display.
fn manual_open_target(
    choice: DisplayChoice,
    monitors: &[channels::MonitorInfo],
) -> Result<Option<usize>, String> {
    let primary = monitors.iter().find(|m| m.primary).map(|m| m.index);
    match choice {
        DisplayChoice::On(idx) if Some(idx) == primary && monitors.len() > 1 => Err(format!(
            "Display {} is this console's own screen. Choose the projector's display \
             for this screen in Outputs, or it would cover the console you are running \
             the service from.",
            idx + 1
        )),
        DisplayChoice::On(idx) => Ok(Some(idx)),
        DisplayChoice::Anywhere if monitors.len() > 1 => Err(
            "Choose a display for this screen in Outputs first. With none chosen it \
             would open over this console."
                .to_string(),
        ),
        DisplayChoice::Anywhere => Ok(None),
        DisplayChoice::Missing(n) => Err(format!("Display {n} is not connected.")),
    }
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
                    media_error: health.media_error_of(c.id),
                    resyncs: health.resyncs_of(c.id),
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
                    media_error: health.media_error_of(c.id),
                    resyncs: health.resyncs_of(c.id),
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
                media_error: None,
                resyncs: 0,
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
                media_error: None,
                resyncs: 0,
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
// NINE FLAT ARGUMENTS, AND FLAT ON PURPOSE.
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
    // HOW MANY BEATS THIS BRIDGE REFUSED BEFORE THIS ONE (RG-119). `Some(0)` on an
    // ordinary beat and a statement rather than an absence — the page reached the
    // bridge to say it, so it knows. A refused beat cannot report itself, which is
    // why this is a running count carried by the one that gets through.
    refused: Option<u32>,
    // WHERE THE CLIP IS, if this screen is playing one. Absent for every screen
    // showing a verse, and absent is the honest reading — see `channels::MediaBeat`
    // for why the console must never time a clip off its own preview instead.
    media_pos_ms: Option<u64>,
    media_dur_ms: Option<u64>,
    media_paused: Option<bool>,
    // A PICTURE OR CLIP THIS SCREEN COULD NOT LOAD, in the page's words (O-4).
    // Absent is "nothing failed", and it clears the last report.
    media_error: Option<String>,
) -> error::Result<()> {
    // An unparseable state is dropped, not defaulted. Defaulting would let a
    // malformed beat keep a dead screen looking alive, which is the exact failure
    // this whole mechanism exists to end.
    if let Some(st) = channels::PaintState::parse(&state) {
        health.beat(
            channel_id,
            st,
            "window",
            channels::BeatGap::clamped(since_ms, hidden_ms, refused),
            channels::MediaBeat::clamped(media_pos_ms, media_dur_ms, media_paused),
        );
        health.note_media_error(
            channel_id,
            media_error
                .map(|e| e.trim().chars().take(300).collect::<String>())
                .filter(|e| !e.is_empty()),
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
    urgent: Option<bool>,
) -> error::Result<()> {
    const MAX: usize = 140;
    let line = text.unwrap_or_default();
    let line = line.trim();
    let msg = if line.is_empty() {
        None
    } else {
        Some(line.chars().take(MAX).collect::<String>())
    };
    // TWO VERBS, ONE FIELD (operator, 2026-09-21; DECISIONS §116). `urgent`
    // absent is the quiet send, because a caller that does not ask for an alarm
    // must not get one — the safe default of a two-state control is the state
    // that interrupts nobody.
    channels::stage_alert(&app, msg, urgent.unwrap_or(false));
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
        last_language: LANGUAGE_UNDETERMINED.to_string(),
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
    use super::{channels, manual_open_target, resolve_display, DisplayChoice};

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

    /// 2026-09-21 · OU-1 (RG-188). The MANUAL open honoured `Anywhere`, so a screen
    /// with no display chosen fullscreened onto the primary monitor — the console
    /// the operator is running the service from — with nothing reported. The
    /// automatic open had refused that case since the dock bug; the manual path
    /// now agrees with it, out loud: no display chosen, or the chosen display is
    /// the operator's, is a refusal with a sentence.
    #[test]
    fn a_manual_open_never_lands_on_the_operators_display() {
        let two = [mon(0, true), mon(1, false)];
        assert_eq!(manual_open_target(DisplayChoice::On(1), &two), Ok(Some(1)));
        let err = manual_open_target(DisplayChoice::Anywhere, &two).unwrap_err();
        assert!(err.contains("Choose a display"), "{err}");
        let err = manual_open_target(DisplayChoice::On(0), &two).unwrap_err();
        assert!(err.contains("console"), "{err}");
        // Missing is still Missing's own sentence, decided by the caller.
        // One monitor only: there is nowhere else to go, and covering the console
        // is exactly what the operator asked for by pressing the button.
        assert_eq!(
            manual_open_target(DisplayChoice::Anywhere, &[mon(0, true)]),
            Ok(None)
        );
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

    /// WHAT AN IMPORT AT THE CAP COSTS THE MAIN RUN LOOP — RG-299.
    ///
    /// `import_media` and `import_translation` are `#[tauri::command]`s, so on macOS
    /// they ran on the window's run loop: the decode and the write happened with the
    /// app frozen, and no browser harness can see that. They are
    /// `#[tauri::command(async)]` now, and this is what was being paid for.
    ///
    /// Measures, asserts nothing about the clock. A bench that failed on a busy
    /// machine would be deleted within a month.
    #[test]
    #[ignore = "allocates ~600 MB and measures this machine"]
    fn what_an_import_at_the_cap_costs() {
        // Just under the cap: the guard estimates the decoded size from the base64
        // length, and `len/4*3` rounds a payload of EXACTLY the cap over it.
        let payload = vec![0x5Au8; MAX_IMPORT_BYTES - 4096];
        let t = std::time::Instant::now();
        let encoded = b64(&payload);
        println!(
            "  base64 encode (the webview's half)  {:>6} ms",
            t.elapsed().as_millis()
        );
        let t = std::time::Instant::now();
        let decoded = decode_import("clip.mov", &encoded).expect("at the cap");
        println!(
            "  decode_import                       {:>6} ms",
            t.elapsed().as_millis()
        );
        let path = std::env::temp_dir().join(format!("relay-import-cost-{}", std::process::id()));
        let t = std::time::Instant::now();
        std::fs::write(&path, &decoded).expect("write");
        println!(
            "  fs::write 256 MiB                   {:>6} ms",
            t.elapsed().as_millis()
        );
        let _ = std::fs::remove_file(&path);
    }

    /// WHAT PARSING A LYRIC IMPORT COSTS THE MAIN RUN LOOP — RG-299.
    ///
    /// The other command on that row's list with no measurement. `parse_import` does
    /// no file I/O — the webview reads the file and hands over base64 — so its cost
    /// is `decode_import` plus a parse, and the parse is the half nothing had
    /// priced: `MAX_IMPORT_BYTES` lets a **256 MiB text file** through to
    /// `songs::parse_song`, which is a great deal more work than the 81 ms decode
    /// `what_an_import_at_the_cap_costs` already measured.
    ///
    /// Both ends: the cap, because that is what the guard permits, and a realistic
    /// playlist, because that is what a church actually imports. A number for the
    /// worst case alone would say nothing about Tuesday.
    ///
    /// Reports, asserts nothing about a clock.
    #[test]
    #[ignore = "allocates ~1 GB and measures this machine"]
    fn what_parsing_a_lyric_import_costs() {
        // Realistic lyric text rather than one repeated byte: `parse_song` splits on
        // blank lines and reads section tags, so a file with no structure in it would
        // measure the wrong thing.
        let verse = "Verse 1\nAmazing grace how sweet the sound\nThat saved a wretch \
                     like me\n\nChorus\nI once was lost but now am found\nWas blind but \
                     now I see\n\n";
        for (label, target) in [
            ("a real playlist (2 MiB)", 2 * 1024 * 1024usize),
            ("at the cap (256 MiB)", MAX_IMPORT_BYTES - 8192),
        ] {
            let mut text = String::with_capacity(target + verse.len());
            while text.len() < target {
                text.push_str(verse);
            }
            let encoded = b64(text.as_bytes());
            let t = std::time::Instant::now();
            let out = parse_import("songs.txt".into(), encoded);
            println!(
                "  parse_import {label:<24} {:>6} ms   {}",
                t.elapsed().as_millis(),
                match &out {
                    Ok(v) => format!(
                        "{} song(s), {} section(s)",
                        v.len(),
                        v.iter().map(|s| s.sections.len()).sum::<usize>()
                    ),
                    Err(e) => format!("refused: {}", e.message()),
                }
            );
        }
        println!();
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

    /// **THE MESSAGE THAT CONTRADICTED ITSELF AT ITS OWN BOUNDARY.**
    ///
    /// A file of exactly 256 MiB was refused with *"…is about 256 MB, and Relay
    /// imports files up to 256 MB"* — a sentence that gives an operator no action,
    /// because the number it complains about and the number it permits are the same.
    /// Two faults, checked separately here because they are independent and either
    /// one alone still produces that sentence.
    ///
    /// The estimate is exercised directly rather than through `decode_import`:
    /// proving the boundary through the real function means allocating the payload
    /// and its base64 form, about 600 MB, which is exactly why this size was never
    /// checked. The base64 LENGTH of an N-byte file is arithmetic, so it can be
    /// stated instead of built.
    #[test]
    fn the_import_cap_is_exact_at_its_own_boundary() {
        /// Characters of padded standard base64 for `n` bytes, and how many of
        /// them are `=`. This is the encoder's definition, not an approximation.
        fn encoded(n: usize) -> (usize, usize) {
            let groups = n.div_ceil(3);
            let pad = (3 - (n % 3)) % 3;
            (groups * 4, pad)
        }

        // Sanity: the shape of the helper, on sizes small enough to check by eye.
        assert_eq!(encoded(3), (4, 0));
        assert_eq!(encoded(4), (8, 2));
        assert_eq!(encoded(5), (8, 1));

        // The estimate is EXACT for every size and every padding residue, which is
        // the property the old `len / 4 * 3` did not have. Checked on sizes that
        // cost nothing, because the fault is in the padding term and the padding
        // term does not grow with the file.
        for n in [0, 1, 2, 3, 4, 5, 6, 7, 8, 100, 1023, 1024, 1025] {
            let (len, pad) = encoded(n);
            let s = format!("{}{}", "A".repeat(len - pad), "=".repeat(pad));
            assert_eq!(decoded_size_estimate(&s), n, "estimate for {n} bytes");
        }

        // FAULT ONE — PADDING COUNTED AS PAYLOAD, AT THE ONE SIZE IT MATTERS.
        // A file of exactly 256 MiB encodes to 357,913,944 characters of which two
        // are `=`, so `len / 4 * 3` answered 268,435,458 against a 268,435,456-byte
        // cap and refused it by two bytes. The string is built and dropped without
        // being decoded: the guard reads its length, and decoding it as well would
        // put ~600 MB in a test that runs on every commit.
        let (len, pad) = encoded(MAX_IMPORT_BYTES);
        assert_eq!((len, pad), (357_913_944, 2), "the encoded form of 256 MiB");
        {
            let at_the_cap = format!("{}{}", "A".repeat(len - pad), "=".repeat(pad));
            assert_eq!(
                decoded_size_estimate(&at_the_cap),
                MAX_IMPORT_BYTES,
                "a file of EXACTLY the cap must estimate to exactly the cap, so the \
                 guard's `approx > MAX_IMPORT_BYTES` lets it through — the old \
                 arithmetic answered {} and refused the one file that is precisely at \
                 the documented limit",
                MAX_IMPORT_BYTES + 2
            );
        }

        // FAULT TWO — A FLOORED FIGURE CANNOT REPORT AN OVERAGE, and it is
        // independent of fault one: with the padding discounted, every size from one
        // byte to one mebibyte past the cap still floored to 256, so the sentence
        // went on equating its two numbers across that whole range. One byte over is
        // both still refused and now reported as larger than the limit.
        let (len, pad) = encoded(MAX_IMPORT_BYTES + 1);
        let over = format!("{}{}", "A".repeat(len - pad), "=".repeat(pad));
        assert!(
            decoded_size_estimate(&over) > MAX_IMPORT_BYTES,
            "one byte past the cap must still be over it — the fix must not have \
             simply loosened the guard"
        );
        let err = decode_import("service.mp4", &over).expect_err("must refuse");
        let msg = err.message();
        let reported: usize = msg
            .split(" is about ")
            .nth(1)
            .and_then(|rest| rest.split(' ').next())
            .and_then(|w| w.parse().ok())
            .unwrap_or_else(|| panic!("could not read the reported size out of {msg:?}"));
        assert!(
            reported > MAX_IMPORT_BYTES / (1024 * 1024),
            "a refusal must name a size STRICTLY GREATER than the limit it cites. It \
             read {reported} against a limit of {}, which tells an operator their \
             file was within the limit and refused anyway: {msg}",
            MAX_IMPORT_BYTES / (1024 * 1024)
        );
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
        let id = db::insert_media(&conn, "video", "loop.mp4", "2026-09-02", None).expect("insert");
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
        let id = db::insert_media(&conn, "image", "a b/c.png", "2026-09-02", None).expect("insert");
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
            passage_end: None,
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

/// **WHAT THE PASSAGE GUARD COSTS AND WHAT IT BUYS, ON REAL PREACHING.**
///
/// `RELAY_SERVICE_CORPUS=<file> cargo test --release passage_guard_bench -- --ignored --nocapture`
///
/// One line per line of the file: `<seconds>\t<transcript text>`, in service order.
/// The author's own database produces it, read-only:
///
/// ```text
/// sqlite3 -readonly relay.db -noheader -separator $'\t' \
///   "select round(timestamp,1), replace(text, char(10),' ')
///      from transcripts where service_id = 39 and trim(text) <> '' order by timestamp;"
/// ```
///
/// **Why it drives `candidates_for_window` and the real `Router`.** Rule 13: the only
/// question is which verse Relay would put on a screen or offer, and neither half of
/// that is answerable by reading a transcript. The timestamps are used as the
/// router's clock, so the per-reference cooldown behaves as it did on the morning.
///
/// **What it cannot tell you.** These lines are FINALS out of the database, and the
/// live path also runs detection on every partial — roughly one a second — so every
/// count here is a floor on the churn, not a measurement of it. Suggestions are
/// never persisted (`persist_fire` is inside `if fire.may_broadcast()`), so the
/// database cannot corroborate the suggestion half at all; only the fires can be
/// checked against it. And there is no audio, so nothing here is about accuracy.
#[cfg(test)]
mod passage_guard_bench {
    use super::*;
    use detection::VerseRef;

    pub(super) fn kjv_corpus() -> Vec<(VerseRef, String)> {
        let kjv: serde_json::Value =
            serde_json::from_str(include_str!("../data/kjv.json").trim_start_matches('\u{feff}'))
                .expect("kjv");
        let mut corpus: Vec<(VerseRef, String)> = Vec::new();
        for (bi, book) in kjv.as_array().expect("books").iter().enumerate() {
            // CANONICAL names, not the file's `abbrev`: the guard compares a
            // candidate's book with the book on the screen, and an index built on
            // abbreviations would make every comparison false and the bench would
            // measure nothing while printing numbers.
            let name = detection::CANONICAL_BOOKS[bi].to_string();
            for (ci, chapter) in book["chapters"].as_array().expect("ch").iter().enumerate() {
                for (vi, verse) in chapter.as_array().expect("vs").iter().enumerate() {
                    corpus.push((
                        VerseRef {
                            book: name.clone(),
                            chapter: ci as i64 + 1,
                            verse: vi as i64 + 1,
                        },
                        // THROUGH `db::clean_verse` (RG-324): the raw file keeps the
                        // KJV's editorial apparatus, braces and all, and a bench
                        // that indexes it is measuring a corpus no install has.
                        crate::db::clean_verse(verse.as_str().unwrap_or("")),
                    ));
                }
            }
        }
        corpus
    }

    /// One replay of a service. With `guard` and `doubt` both off it reproduces the
    /// shipped behaviour of 2026-09-24 exactly, which is what makes the columns
    /// comparable. RG-305 added the second switch rather than a second bench: two
    /// replays of one service that disagreed about the router's clock would be worse
    /// than no measurement at all.
    struct Run {
        offered: usize,
        fired: Vec<(f32, String, String)>,
        held: usize,
        held_refs: Vec<String>,
        windows_with_a_hold: usize,
        /// Every citation-doubt demotion, with the window that produced it.
        doubts: Vec<(f32, String, String)>,
    }

    fn replay(lines: &[(f32, String)], guard: bool, doubt: bool) -> Run {
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        let mut context = ContextMemory::default();
        let mut router = Router::default();
        let mut out = Run {
            offered: 0,
            fired: Vec::new(),
            held: 0,
            held_refs: Vec::new(),
            windows_with_a_hold: 0,
            doubts: Vec::new(),
        };
        for (at, text) in lines {
            let now_ms = (at * 1000.0) as u64;
            let WindowCandidates {
                mut kept,
                held,
                reading_in: _,
                doubted,
            } = candidates_for_window(
                text,
                true,
                &sem,
                &phrases,
                &context,
                router.wall(),
                // FALSE: this bench measures the passage guard against the SHIPPED
                // default. What the paraphrase bar costs is measured where it can be
                // swept on and off — `suggestions::bar::paraphrase_bar`.
                false,
            );
            // RG-318, and it must be the same call `emit_detections` makes, in the
            // same place: a bench that did not carry an unfinished citation would
            // measure a product nobody ships, which is the mistake this module's own
            // `doubt` switch was built to avoid.
            context.note_in_flight(detection::chapter_in_flight(text), now_ms);
            for d in &doubted {
                out.doubts.push((
                    *at,
                    format!("{:?}  {} → {:?}", d.doubt, d.reference, d.method),
                    text.chars().take(90).collect(),
                ));
            }
            if !doubt {
                // The DOUBT RULE OFF: put every demoted method back to what it was,
                // so the "before" column is the path that ran on 2026-09-25 and not a
                // near miss at it. `was` is carried on `Doubted` for exactly this.
                let mut restored = 0;
                for d in &doubted {
                    for c in kept.iter_mut() {
                        if Fire::key_for(&c.r) == d.reference && c.method == d.method {
                            c.method = d.was;
                            restored += 1;
                            break;
                        }
                    }
                }
                assert_eq!(
                    restored,
                    doubted.len(),
                    "a demotion could not be put back, so the before column would be \
                     measuring something nobody shipped: {text}"
                );
            }
            let (candidates, held) = if guard {
                (kept, held)
            } else {
                // The guard OFF: put back exactly what it held — the same `Cand`,
                // same confidence, same method — so the "before" column IS the
                // shipped path of 2026-09-24 and not a near miss at it.
                let mut all = kept;
                for (c, _) in held {
                    all.push(c);
                }
                (all, Vec::new())
            };
            if !held.is_empty() {
                out.windows_with_a_hold += 1;
                out.held += held.len();
                for (c, reason) in &held {
                    out.held_refs
                        .push(format!("{:?}  {}", reason, Fire::key_for(&c.r)));
                }
            }
            if candidates.is_empty() {
                continue;
            }
            let mut best: Vec<(String, Cand)> = Vec::new();
            for c in candidates {
                let key = Fire::key_for(&c.r);
                match best.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, e)) => {
                        if !pipeline::better(e, &c) {
                            *e = c;
                        }
                    }
                    None => best.push((key, c)),
                }
            }
            for (rank, (key, c)) in rank_for_wall(best).into_iter().enumerate() {
                match router.decide_live(&key, c.conf, c.method, now_ms, true) {
                    RouteDecision::AutoFire if rank == 0 => {
                        out.fired
                            .push((*at, key.clone(), text.chars().take(70).collect()));
                        context.note_passage(&c.r, None);
                        // What `broadcast_with_clock` does, because rule 29 means only
                        // this one actually reaches a screen. A bench that told the
                        // router about rank 1 would measure a product nobody ships —
                        // and that mistake is exactly what this bench caught in the
                        // first design of the guard.
                        router.note_wall(&key, now_ms);
                    }
                    RouteDecision::AutoFire | RouteDecision::Suggest => out.offered += 1,
                    RouteDecision::Drop => {}
                }
            }
        }
        out
    }

    #[test]
    #[ignore]
    fn what_the_guard_costs_a_real_service() {
        let Ok(path) = std::env::var("RELAY_SERVICE_CORPUS") else {
            println!("set RELAY_SERVICE_CORPUS to `<seconds>\\t<text>` lines, in order");
            return;
        };
        let body = std::fs::read_to_string(&path).expect("corpus unreadable");
        let lines: Vec<(f32, String)> = body
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .filter(|(_, t)| !t.trim().is_empty())
            .map(|(a, t)| (a.trim().parse().unwrap_or(0.0), t.to_string()))
            .collect();
        println!("\n{} transcript lines\n", lines.len());

        let before = replay(&lines, false, false);
        let after = replay(&lines, true, false);

        println!(
            "  suggestions offered   before {:>4}   after {:>4}   ({} fewer)",
            before.offered,
            after.offered,
            before.offered as i64 - after.offered as i64
        );
        println!(
            "  auto-fires            before {:>4}   after {:>4}",
            before.fired.len(),
            after.fired.len()
        );
        println!(
            "  held by the guard     {} candidates across {} windows",
            after.held, after.windows_with_a_hold
        );

        let b: Vec<&String> = before.fired.iter().map(|(_, k, _)| k).collect();
        let a: Vec<&String> = after.fired.iter().map(|(_, k, _)| k).collect();
        // **BROADCASTS REMOVED, AND WHETHER ANY OF THEM WAS A VERSE.** The question
        // rule 13 asks is which verse a congregation would see, so a removed
        // broadcast of a verse that was ALREADY on the screens costs nothing and a
        // removed broadcast of a verse that was not is a real loss. Printing them in
        // one list without the distinction is how a number gets quoted wrongly.
        println!("\n  BROADCASTS REMOVED:");
        let mut duplicates = 0;
        let mut real_losses = 0;
        for (at, key, heard) in &before.fired {
            if after.fired.iter().any(|(t, k, _)| k == key && t == at) {
                continue;
            }
            // What was on the wall, in the AFTER run, at the moment this fired?
            let on_wall = after
                .fired
                .iter()
                .rev()
                .find(|(t, _, _)| t <= at)
                .map(|(_, k, _)| k.clone());
            if on_wall.as_deref() == Some(key.as_str()) {
                duplicates += 1;
                println!("    {at:>7.1}s  DUPLICATE of the wall   {key:<22} “{heard}”");
            } else {
                real_losses += 1;
                println!(
                    "    {at:>7.1}s  A VERSE IS LOST         {key:<22} (wall held {on_wall:?}) “{heard}”"
                );
            }
        }
        if duplicates + real_losses == 0 {
            println!("    none");
        }
        println!(
            "\n  {duplicates} duplicate broadcasts removed · {real_losses} verses actually lost"
        );
        println!("\n  FIRES GAINED:");
        let mut gained = 0;
        for (at, key, heard) in &after.fired {
            if !before.fired.iter().any(|(t, k, _)| k == key && t == at) {
                gained += 1;
                println!("    {at:>7.1}s  {key:<22} “{heard}”");
            }
        }
        if gained == 0 {
            println!("    none");
        }
        println!("\n  HELD REFERENCES (first 40):");
        for r in after.held_refs.iter().take(40) {
            println!("    {r}");
        }
        println!(
            "\n  fire order identical: {}\n",
            b == a && before.fired.len() == after.fired.len()
        );
    }

    /// **HOW FAST A READING MOVES THE WALL** — the churn the operator is counting,
    /// measured rather than argued. Prints every pair of consecutive auto-fires and
    /// **WHAT THE CITATION-DOUBT RULE COSTS A REAL SERVICE** (RG-305).
    ///
    /// `RELAY_SERVICE_CORPUS=<file> cargo test --release what_the_citation_doubt_rule_costs
    /// -- --ignored --nocapture`
    ///
    /// The passage guard is ON in both columns, because it is shipped: the question
    /// is what this rule changes on top of it. Rule 13 — the only question is which
    /// verse Relay would put on a screen, so the fires are compared and the
    /// suggestions counted, and neither is answerable by reading a transcript.
    ///
    /// **Every fire this rule removes is printed with the window that produced it**,
    /// because a count of demotions says nothing about whether they were right and
    /// this rule demotes 0.95 `Direct`, the strongest claim Relay can make. The eight
    /// field instances and the two correct fires of 2026-09-25 are the reference set;
    /// anything else in the list has to be read by a person.
    #[test]
    #[ignore]
    fn what_the_citation_doubt_rule_costs() {
        let Ok(path) = std::env::var("RELAY_SERVICE_CORPUS") else {
            println!("set RELAY_SERVICE_CORPUS to `<seconds>\t<text>` lines, in order");
            return;
        };
        let body = std::fs::read_to_string(&path).expect("corpus unreadable");
        let lines: Vec<(f32, String)> = body
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .filter(|(_, t)| !t.trim().is_empty())
            .map(|(a, t)| (a.trim().parse().unwrap_or(0.0), t.to_string()))
            .collect();
        println!("\n{} transcript lines\n", lines.len());

        let before = replay(&lines, true, false);
        let after = replay(&lines, true, true);

        println!(
            "  suggestions offered   before {:>4}   after {:>4}",
            before.offered, after.offered
        );
        println!(
            "  auto-fires            before {:>4}   after {:>4}",
            before.fired.len(),
            after.fired.len()
        );
        println!("  citations doubted     {}", after.doubts.len());
        println!("\n  EVERY DEMOTION, IN ORDER:");
        for (at, what, heard) in &after.doubts {
            println!("    {at:>8.1}s  {what:<52} “{heard}”");
        }
        if after.doubts.is_empty() {
            println!("    none");
        }
        println!("\n  FIRES REMOVED (each one has to be read, not counted):");
        let mut removed = 0;
        for (at, key, heard) in &before.fired {
            if after.fired.iter().any(|(t, k, _)| k == key && t == at) {
                continue;
            }
            removed += 1;
            println!("    {at:>8.1}s  {key:<22} “{heard}”");
        }
        if removed == 0 {
            println!("    none");
        }
        println!("\n  FIRES GAINED (a run that now ranks first — must be zero):");
        let mut gained = 0;
        for (at, key, heard) in &after.fired {
            if before.fired.iter().any(|(t, k, _)| k == key && t == at) {
                continue;
            }
            gained += 1;
            println!("    {at:>8.1}s  {key:<22} “{heard}”");
        }
        if gained == 0 {
            println!("    none");
        }
        println!("\n  {removed} fires removed · {gained} fires gained\n");
    }

    /// **WHICH OF THE DAY'S WRONG VERSES THIS RULE CAN EVEN REACH** (RG-305).
    ///
    /// `cargo test --release which_field_instances_this_rule_can_reach -- --ignored
    /// --nocapture`
    ///
    /// Nine wrong verses reached congregations on 2026-09-25 through a misheard
    /// number or book — RG-301's `Jude 28` and RG-305's eight. Each window below is
    /// the `heard_text` off the operator's own database, verbatim, with the
    /// `detections.id` beside it, run through the real `PhraseIndex` and the real
    /// rule.
    ///
    /// **It reaches five.** It reached THREE until 2026-09-26, and the two it gained
    /// are the two that failed on a word count and nothing else: `Isaiah 61:3` at a
    /// four-word run and `Romans 12:3` at three, both below `MIN_RUN_WORDS`, both in
    /// windows where the preacher had already named the book and the chapter out
    /// loud. `detection::chapter_the_words_point_at` asks the narrower question that
    /// leaves (RG-313, DECISIONS §127).
    ///
    /// **The remaining four carry no verbatim run that can be evidence in the window
    /// that fired**: three windows are the reference and nothing else, and one —
    /// `Jude 1:7` — carries a run pointing at a verse he was referring BACK to rather
    /// than at a slip of the reference.
    ///
    /// **In four of the six originally out of reach the quotation arrived 6 to 16
    /// seconds LATER**, in a separate window, and corrected the record after the wrong
    /// verse was already on the wall. No window-local rule can reach those, and
    /// reversing a fire already on a congregation's screen is a different decision
    /// with a different cost.
    ///
    /// This exists so the claim cannot drift. A later reader who widens the rule
    /// should see the ceiling first: the limit is the EVIDENCE, not the predicate —
    /// **and widening it is not free.** The first draft of §127's rule took this
    /// number to five and cost two auto-fires of references the preacher said
    /// correctly, which `what_the_citation_doubt_rule_costs` is what caught. Raise
    /// this assertion only beside that bench's output.
    #[test]
    #[ignore]
    fn which_field_instances_this_rule_can_reach() {
        // (detections.id, what fired, what it should have been, the heard_text)
        const FIELD: &[(u32, &str, &str, &str)] = &[
            (588, "Jude 1:7", "Romans 11:33", "We have tried to look at that from Jude 28 and verse 7 to 28."),
            (603, "Psalms 7:1", "Psalms 87:7", "This one was born there, the other one was born there, all my springs are in thee. Psalm 7 verse 1 to 7."),
            (645, "Mark 6:12", "Mark 6:2", "Mark 6, 12. Mark 6, 12."),
            (668, "Acts 8:12", "Proverbs 8:12", "Acts 8, 12, I wisdom dwell with prudence and find out the knowledge of witty inventions. Now, what"),
            (786, "Luke 8:8", "Luke 18:8", "And then we shall find faith on the earth. Luke chapter 8, chapter 8, verse 8. So faith is the truth."),
            (805, "1 Timothy 1:7", "2 Timothy 1:7", "1 Timothy, chapter 1, verse 7. 1 Timothy, chapter 1, verse 7."),
            (818, "Isaiah 1:3", "Isaiah 61:3", "Verse 5 and verse 7 and 8, the oil of gladness. Now, Isaiah 1 verse 3, it calls it the oil of joy."),
            (861, "Romans 2:3", "Romans 12:3", "We have common faith, measure of faith, Romans, 2, 3 We have little faith, Matthew, 2"),
            (930, "Psalms 35:5", "Psalms 34:5", "helped me in the journey. Then life broke out from Psalm 35 verse 5, which I later defined as the law"),
        ];
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        let context = ContextMemory::default();
        let mut reached = 0usize;
        let mut still_firable = 0usize;
        println!();
        for (id, fired, should_be, text) in FIELD {
            let w = candidates_for_window(text, true, &sem, &phrases, &context, None, false);
            let doubt = w.doubted.iter().find(|d| d.reference == *fired);
            // ── AND THE QUESTION RULE 13 ACTUALLY ASKS (added 2026-09-28) ────────
            //
            // *Which verse would Relay put on a screen.* This test only ever asked
            // whether THIS rule reaches a window, which made "out of reach" read as
            // "still fires" — and after the RG-301 anchor fix that is no longer true
            // of `Jude 1:7`, whose wrong verse is now refused by a different rule
            // entirely. A ceiling stated for one rule is not a ceiling on the
            // product.
            let firable = w
                .kept
                .iter()
                .any(|c| Fire::key_for(&c.r) == *fired && c.method.unattended_rank() > 0);
            if firable {
                still_firable += 1;
            }
            let wall = if firable { "CAN STILL FIRE" } else { "refused" };
            match doubt {
                Some(d) => {
                    reached += 1;
                    println!(
                        "  id {id:<4} REACHED   {fired:<15} → {:?}  [{wall}] (should be {should_be})",
                        d.doubt
                    );
                }
                None => println!(
                    "  id {id:<4} out of reach  {fired:<15}  [{wall}] (should be {should_be})"
                ),
            }
        }
        println!(
            "\n  {reached} of {} reached by THIS rule · {still_firable} of {} can still reach a wall by ANY route\n",
            FIELD.len(),
            FIELD.len()
        );
        assert_eq!(
            reached, 5,
            "the reachable set changed — if a rule was widened, say so and measure \
             what it costs in correct fires before quoting this number"
        );
    }

    /// **WHAT THE CROSS-BOOK CARVE-OUT COSTS PER WINDOW** (RG-305).
    ///
    /// The doubt rule itself is a double loop over a handful of candidates and is
    /// free. The one thing that is not free is the extra `PhraseIndex::quoted` call
    /// the cross-book case needs, in windows that name both a book and a verse — this
    /// runs on `relay-detect`, once per decode pass, and rule 31's whole lesson is
    /// that this path is measured rather than reasoned about.
    #[test]
    #[ignore]
    fn what_the_extra_index_lookup_costs() {
        let Ok(path) = std::env::var("RELAY_SERVICE_CORPUS") else {
            println!("set RELAY_SERVICE_CORPUS");
            return;
        };
        let body = std::fs::read_to_string(&path).expect("corpus unreadable");
        let corpus = kjv_corpus();
        let idx = detection::PhraseIndex::build(&corpus);
        let mut windows = 0usize;
        let mut extra = 0usize;
        let mut restricted = std::time::Duration::ZERO;
        let mut unrestricted = std::time::Duration::ZERO;
        for line in body
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(_, t)| t)
        {
            windows += 1;
            let anchor = detection::anchor_for_bare_verses(line);
            let named = anchor.as_ref().map(|r| r.book.clone());
            let says_a_verse = detection::detect_direct(line)
                .iter()
                .any(|m| m.method == DetectionMethod::Direct && !m.whole_chapter);
            let t = std::time::Instant::now();
            let _ = idx.quoted(line, named.as_deref(), QUOTED_SUGGESTIONS_MAX);
            restricted += t.elapsed();
            if named.is_some() && says_a_verse {
                extra += 1;
                let t = std::time::Instant::now();
                let _ = idx.quoted(line, None, QUOTED_SUGGESTIONS_MAX);
                unrestricted += t.elapsed();
            }
        }
        println!(
            "\n  {windows} windows · {extra} take the extra lookup ({:.1}%)",
            100.0 * extra as f32 / windows.max(1) as f32
        );
        println!(
            "  the lookup Relay already did:  {:?} total · {:?}/window",
            restricted,
            restricted / windows.max(1) as u32
        );
        println!(
            "  the extra one:                 {:?} total · {:?} per window that takes it\n",
            unrestricted,
            unrestricted / extra.max(1) as u32
        );
    }

    /// **WHY THE CITATION-DOUBT RULE DID NOT REACH A DROPPED ORDINAL** — service 41,
    /// 2026-09-26, `detections.id = 1678`. **A wrong verse reached a congregation.**
    ///
    /// `cargo test --release why_the_carve_out_missed_1_john -- --ignored --nocapture`
    ///
    /// *"John 5, 3 This is the love of God that will keep His commandments."* fired
    /// **John 5:3** — *"In these lay a great multitude of impotent folk"* — at 0.55
    /// `Direct`. He said **1 John 5:3**, *"For this is the love of God, that we keep
    /// his commandments"*, and whisper dropped the ordinal. The next window settles it
    /// beyond argument: *"The commandments are not grievous"* is that verse's second
    /// half.
    ///
    /// **§123's cross-book carve-out is written for exactly this shape and did not
    /// fire.** It admits a quotation hit from another book when it sits at the EXACT
    /// chapter and verse a spoken reference in the window named — citation `John 5:3`,
    /// quotation `1 John 5:3`, chapter 5 and verse 3 both matching, different book.
    /// Measured against the bundled KJV, the runs are not close:
    ///
    /// | verse | contiguous run with what he said |
    /// |---|---|
    /// | `1 John 5:3` | **7 words** — *"this is the love of god that"* |
    /// | `John 5:3` (fired) | **1 word** — *"the"* |
    ///
    /// Seven clears `MIN_RUN_WORDS`, so `PhraseIndex::quoted` should have returned it.
    /// Relay plainly FOUND the verse — `1 John 5:3` was offered three times as
    /// `Semantic` at 0.60 — but semantic offers are not what the carve-out inspects.
    ///
    /// This prints each stage separately so the missing link is named rather than
    /// guessed: the restricted `quoted`, the unrestricted `quoted`, what the carve-out
    /// admits, and what the whole window rule concludes.
    /// One-off: what does the index actually see for the two windows RG-309's rule
    /// could not reach? Prints the run `PhraseIndex` itself measures, not a run
    /// computed some other way — the discrepancy between those two is the open
    /// question.
    #[test]
    #[ignore]
    fn what_the_index_sees_for_the_two_missed_windows() {
        let corpus = kjv_corpus();
        let idx = detection::PhraseIndex::build(&corpus);
        let cases = [
            ("John 5, 3 This is the love of God that will keep His commandments.", "1 John", 5, 3),
            ("We heard these words here, Romans 1, 8 My son, hear the instruction of thy father.", "Proverbs", 1, 8),
        ];
        for (heard, book, ch, vs) in cases {
            let r = detection::VerseRef {
                book: book.into(),
                chapter: ch,
                verse: vs,
            };
            println!("\n  “{heard}”");
            println!(
                "     shared_run_with({book} {ch}:{vs}) = {}",
                idx.shared_run_with(heard, &r)
            );
            println!(
                "     MIN_RUN_WORDS={} SELF_EVIDENT_RUN(7) PARAPHRASE_RUN_WORDS={}",
                detection::MIN_RUN_WORDS,
                detection::PARAPHRASE_RUN_WORDS
            );
            let hits = idx.quoted(heard, None, 8);
            if hits.is_empty() {
                println!("     quoted(None, 8): NOTHING");
            }
            for h in hits {
                println!(
                    "     quoted: {} {}:{} run={} sole={} “{}”",
                    h.r.book, h.r.chapter, h.r.verse, h.run, h.sole, h.phrase
                );
            }
        }
        println!();
    }

    /// **STATUS 2026-09-27: BOTH CAUSES FOUND, and they are different. Neither is
    /// the carve-out's conditions, and my first two theories were both wrong.**
    ///
    /// `what_the_index_sees_for_the_two_missed_windows` and `explain_one_window` gave
    /// the answers that reading the code three times did not.
    ///
    /// **`Romans 1:8` — the rule works and it races the decode.** Given the WHOLE
    /// sentence the carve-out fires exactly as designed:
    ///
    /// ```text
    ///   DOUBT  Romans 1:8    SpokenBook    Direct -> UncertainBook
    ///   DOUBT  Proverbs 1:8  TheQuotation  Reading -> Quoted
    /// ```
    ///
    /// Live, the citation and the quotation were in DIFFERENT windows — *"…Romans 1,
    /// 8, My son."* at 9613.5 s fired, and *"…My son, hear the instruction of thy
    /// father."* arrived at 9617.1 s, 3.6 s later. There was nothing to contradict it
    /// yet, and by the time there was, the wrong verse was on the wall and debounced.
    /// Same shape as RG-315: **the reference fires before its evidence arrives.**
    /// Rule 28 holds a PARTIAL window for a second pass and exempts a FINAL one
    /// because *no next pass is coming* — which is not true of a rolling window that
    /// keeps producing finals.
    ///
    /// **`John 5:3` — the run is undiscoverable, whatever its length.** A direct scan
    /// measures 7 words against `1 John 5:3`, and `PhraseIndex::quoted` finds nothing.
    /// `quoted` can only discover a run that starts from an INDEXED 3-gram, and
    /// `verses_with` returns an empty bucket for any gram in more than
    /// `MAX_GRAM_VERSES` (8) verses as *too common to be evidence*. Every 3-gram in
    /// *"this is the love of god that"* is far commoner than that, so a 7-word
    /// verbatim run is invisible. **The pruning is per-gram; the evidence is
    /// per-run.** Raising `MAX_GRAM_VERSES` is not the fix — it would widen every
    /// lookup — and seeding from the rarest gram in the window, or admitting one
    /// common seed when the extended run is long, are the candidates. Both need
    /// measuring against `print_every_auto_fire` before either ships.
    ///
    /// Running it answered the question and killed my first two theories. For
    /// *"John 5, 3 This is the love of God that will keep His commandments."*:
    ///
    /// ```text
    ///   -- quoted RESTRICTED to the named book:   (nothing)
    ///   -- quoted UNRESTRICTED:                   (nothing)
    /// ```
    ///
    /// `PhraseIndex::quoted` returns **nothing at all**, even unrestricted, so the
    /// cross-book carve-out never had an input to weigh and cannot be at fault.
    /// `1 John 5:3` reached the set only as `Semantic` 0.53.
    ///
    /// **What does not add up yet.** The contiguous run between that window and
    /// `1 John 5:3` is **7 words** — *"this is the love of god that"* — measured
    /// against the bundled KJV. `quoted`'s gate is
    /// `n >= MIN_RUN_WORDS && (n >= SELF_EVIDENT_RUN || has_a_rare_word(..))`, and
    /// `SELF_EVIDENT_RUN` is 7, so a 7-word run should qualify on the first clause
    /// alone. Either the run `quoted` computes is shorter than the one a plain
    /// longest-common-run gives, or something after the gate drops it.
    ///
    /// The next step is to print `n` and `sole` from inside `quoted` for this window
    /// rather than to reason about it further — twice now reasoning gave a confident
    /// wrong answer here.
    #[test]
    #[ignore]
    fn why_the_carve_out_missed_1_john() {
        // TWO INSTANCES, same service, different books and different evidence —
        // which is what rules out the first explanation I reached for.
        //
        //   * `John 5, 3 …` → fired John 5:3; right verse `1 John 5:3` appeared as
        //     `Semantic` at 0.60. I guessed the carve-out missed it because the
        //     evidence was not a `Quoted` run.
        //   * `Romans 1, 8, My son, hear the instruction of thy father.` → fired
        //     **Romans 1:8** (`detections.id` at 9613.5 s); the right verse
        //     `Proverbs 1:8` appeared as **`Quoted` at 0.69**. Same chapter, same
        //     verse, different book — the carve-out's exact condition — and the doubt
        //     still did not fire. So the guess above is WRONG and the fault is in the
        //     wiring or the conditions, not in how the evidence is classified.
        for heard in [
            "John 5, 3 This is the love of God that will keep His commandments.",
            "We heard these words here, Romans 1, 8 My son, hear the instruction of thy father.",
        ] {
            probe_the_carve_out(heard);
        }
    }

    /// One window, every stage printed. Shared by both instances above.
    #[cfg(test)]
    fn probe_the_carve_out(heard: &str) {
        let corpus = kjv_corpus();
        let idx = detection::PhraseIndex::build(&corpus);
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        let context = ContextMemory::default();
        println!("\n  “{heard}”");
        let anchor = detection::anchor_for_bare_verses(heard);
        println!("  anchor: {anchor:?}");
        let named = anchor.as_ref().map(|r| r.book.clone());
        println!("  quoted_in (the restriction): {named:?}");
        println!("  -- quoted RESTRICTED to the named book:");
        for h in idx.quoted(heard, named.as_deref(), QUOTED_SUGGESTIONS_MAX) {
            println!(
                "     {} {}:{}  run={} sole={}",
                h.r.book, h.r.chapter, h.r.verse, h.run, h.sole
            );
        }
        println!("  -- quoted UNRESTRICTED (what the carve-out draws from):");
        for h in idx.quoted(heard, None, QUOTED_SUGGESTIONS_MAX) {
            println!(
                "     {} {}:{}  run={} sole={}",
                h.r.book, h.r.chapter, h.r.verse, h.run, h.sole
            );
        }
        println!("  -- the shipped window rule:");
        let w = candidates_for_window(heard, true, &sem, &phrases, &context, None, false);
        for c in &w.kept {
            println!(
                "     KEPT  {} {}:{}  {:?}  {:.2}",
                c.r.book, c.r.chapter, c.r.verse, c.method, c.conf
            );
        }
        for d in &w.doubted {
            println!(
                "     DOUBT {}  {:?}  {:?} -> {:?}",
                d.reference, d.doubt, d.was, d.method
            );
        }
        // THE CLAIM: the verse he actually cited must not reach the wall unattended
        // while a seven-word run in the same window names a different one.
        let firable: Vec<_> = w
            .kept
            .iter()
            .filter(|c| c.method == DetectionMethod::Direct)
            .map(|c| (detection::reference_key(&c.r), c.conf))
            .collect();
        assert!(
            firable.is_empty(),
            "John 5:3 still reaches the wall against a 7-word run for 1 John 5:3: {firable:?}"
        );
    }

    /// **WHY A VERSE READ ALOUD, VERBATIM, DID NOT REACH THE WALL** — service 41,
    /// 2026-09-26, watched live.
    ///
    /// `cargo test --release why_psalm_121_6_never_fired -- --ignored --nocapture`
    ///
    /// The preacher read Psalms 121 straight through, calling the verse numbers out
    /// loud. Verses 2, 4 and 5 auto-fired as `Reading`. **Verses 3 and 6 did not, and
    /// the operator fired 7 and 8 by hand.** Verse 6 is the interesting one: its
    /// window holds the verse almost word for word, and the only thing Relay produced
    /// was an `UncertainBook` answer from memory, which rule 40's third half caps at
    /// Suggest for ever.
    ///
    /// This prints, for each real window off the operator's own database: what
    /// `PhraseIndex::quoted` returns on its own, and what the whole window rule
    /// produces. It is a diagnosis, not an assertion — it exists to find out which
    /// stage dropped the verse, because reading the code three ways gave three
    /// answers.
    #[test]
    #[ignore]
    fn why_psalm_121_6_never_fired() {
        // Verbatim from `transcripts`, service 41, in order.
        const WINDOWS: &[(f32, &str)] = &[
            (93.1, "my food to be moved will not slumber"),
            (98.9, "Behold, he that keepeth Israel shall neither slumber nor sleep."),
            (100.1, "Behold, he that keepeth Israel shall neither slumber nor sleep. Verse 5."),
            (104.8, "The Lord is thy keeper. The Lord is thy keeper. The Lord is thy keeper."),
            (107.2, "The Lord is thy keeper. The Lord is thy shield upon thy right arm."),
            (115.0, "Luke, verse 6 The sun shall not smite thee by the day, nor the moon by the night."),
            // ── THE SECOND SERVICE, same psalm, 2026-09-26 at 6359.5 s ──────────
            //
            // He read Psalms 121 again in the second service and verse 6 failed AGAIN
            // — for a DIFFERENT reason, which is why it is not one bug: this time he
            // said "by day" correctly and whisper heard "smile" for "smite", cutting
            // the run to 4. Only the morning failure (run 6, "by the day") is
            // reachable by anything but a better decoder.
            //
            // **Verse 7 is the case worth chasing.** The same window carries "The Lord
            // shall preserve thee from all evil" — 8 words verbatim, EXACTLY
            // `READING_RUN_WORDS` — and four other verses sat at exactly 8 that day
            // and every one of them fired. This one did not. Rule 29 is the suspect:
            // one window may put at most one verse on a wall, and this window also
            // holds the mangled verse 6.
            (6359.5, "The sun shall not smile thee by day or the moon by night. The Lord shall preserve thee from all evil."),
            (6372.2, "He shall preserve thy soul. The loudest now verse 8 want to go. The Lord shall preserve"),
        ];
        let corpus = kjv_corpus();
        let idx = detection::PhraseIndex::build(&corpus);
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        let mut context = ContextMemory::default();
        // THE WALL AS IT WAS: Psalms 121:5 had been put up by the reading path.
        context.note(&detection::VerseRef {
            book: "ps".into(),
            chapter: 121,
            verse: 5,
        });
        println!();
        for (at, text) in WINDOWS {
            println!("── {at}s  “{text}”");
            println!("   anchor: {:?}", detection::anchor_for_bare_verses(text));
            let raw = idx.quoted(text, None, QUOTED_SUGGESTIONS_MAX);
            if raw.is_empty() {
                println!("   quoted(unrestricted): NOTHING");
            }
            for h in &raw {
                println!(
                    "   quoted(unrestricted): {} {}:{}  run={} sole={}",
                    h.r.book, h.r.chapter, h.r.verse, h.run, h.sole
                );
            }
            // And what the shipped window rule makes of it.
            let w = candidates_for_window(text, true, &sem, &phrases, &context, None, false);
            for c in &w.kept {
                println!(
                    "   KEPT  {} {}:{}  {:?}  {:.2}",
                    c.r.book, c.r.chapter, c.r.verse, c.method, c.conf
                );
            }
            for (c, why) in &w.held {
                println!(
                    "   HELD  {} {}:{}  {:?}",
                    c.r.book, c.r.chapter, c.r.verse, why
                );
            }
            for d in &w.doubted {
                println!("   DOUBT {}  {:?}", d.reference, d.doubt);
            }
        }
        println!();
    }

    /// **A READING MAY NOT WALK OFF THE PASSAGE IT WAS CITED IN ONTO A SYNOPTIC
    /// PARALLEL — RG-308.**
    ///
    /// Twice in one day, both watched live in the operator's own services,
    /// 2026-09-25:
    ///
    ///  * 16389 s / 16414 s — he cited **Micah 4** and read verse 2 aloud.
    ///    *"out of Zion shall go forth the law"* is in **Micah 4:2 AND Isaiah 2:3**,
    ///    and Relay fired Isaiah.
    ///  * 44725 s / 44741 s — he cited **Mark 4:11** and Relay put it up; sixteen
    ///    seconds later the reading of those same words fired **Luke 8:10**, because
    ///    *"unto you it is given to know the mysteries of the kingdom of God"* is in
    ///    both almost verbatim.
    ///
    /// In each case the wall LEFT the passage the preacher had NAMED while he was
    /// still reading it, and the cost is not a nonsense verse — it is the right words
    /// under the wrong reference, which is harder for an operator to spot.
    ///
    /// **The `sole` rule cannot help**, and that is the finding rather than a gap: it
    /// asks whether the exact token run belongs to one verse, and between parallels
    /// the wording differs by a word or two, so each run genuinely IS unique to its
    /// own verse. Uniqueness is satisfied and the choice is still wrong.
    ///
    /// **Nor could the passage guard, and the reason is its ARMING condition.** Rule A
    /// holds anything that came from the verse text and sits outside the passage on
    /// screen — Luke 8 is outside Mark 4, so the hold itself was always right. But it
    /// arms only on an in-passage verbatim CANDIDATE, and `PhraseIndex::quoted`
    /// returns the parallel rather than the passage precisely because the parallel's
    /// run is the longer one. The evidence that the preacher is reading the passage on
    /// screen existed and was not being asked for.
    ///
    /// This prints what the index measures against BOTH verses, so the arming fact is
    /// a number rather than a belief.
    #[test]
    #[ignore]
    fn what_the_index_measures_against_the_passage_on_screen() {
        let corpus = kjv_corpus();
        let idx = detection::PhraseIndex::build(&corpus);
        let cases = [
            (
                "And he said unto them, Unto you it is given to know the mysteries of \
                 the kingdom of God, but unto them that are without",
                ("Mark", 4, 11),
                ("Luke", 8, 10),
            ),
            (
                "he will teach us of his ways, and we will walk in his paths, for out \
                 of Zion shall go forth the law, and the word of the Lord from Jerusalem",
                ("Micah", 4, 2),
                ("Isaiah", 2, 3),
            ),
        ];
        println!();
        for (heard, on_screen, parallel) in cases {
            let cited = detection::VerseRef {
                book: on_screen.0.into(),
                chapter: on_screen.1,
                verse: on_screen.2,
            };
            let other = detection::VerseRef {
                book: parallel.0.into(),
                chapter: parallel.1,
                verse: parallel.2,
            };
            println!("  “{heard}”");
            println!(
                "     shared_run_with(ON SCREEN {} {}:{}) = {}",
                cited.book,
                cited.chapter,
                cited.verse,
                idx.shared_run_with(heard, &cited)
            );
            println!(
                "     shared_run_with(PARALLEL {} {}:{}) = {}",
                other.book,
                other.chapter,
                other.verse,
                idx.shared_run_with(heard, &other)
            );
            println!("     -- what quoted() returns:");
            for h in idx.quoted(heard, None, QUOTED_SUGGESTIONS_MAX) {
                println!(
                    "        {} {}:{}  run={} sole={} rare={}",
                    h.r.book, h.r.chapter, h.r.verse, h.run, h.sole, h.rare_for_a_wall
                );
            }
        }
        println!();
    }

    /// **THE PASSAGE ON SCREEN HOLDS ITS OWN SYNOPTIC PARALLEL — RG-308, and the
    /// register's account of WHY it did not was wrong.**
    ///
    /// The row says *"RG-306's guard does not apply by construction — a parallel is a
    /// different book and chapter, so neither the wall rule nor the chapter rule
    /// reaches it."* That has rule A backwards. Rule A holds every candidate that
    /// `came_from_the_verse_text` and is NOT inside the passage on screen, so being a
    /// different book and chapter is precisely the condition it holds on. What it
    /// needs is ARMING: an in-passage verbatim run in the same window.
    ///
    /// `what_the_index_measures_against_the_passage_on_screen` shows the arming
    /// evidence is there in both field cases — `quoted` returns the cited verse
    /// alongside the parallel, at a shorter run (`Mark 4:11` at 10 against
    /// `Luke 8:10` at 13; `Micah 4:2` at 15 against `Isaiah 2:3` at 31). So the guard
    /// arms, and the parallel is held.
    ///
    /// Both field instances predate the guard: it landed on 2026-09-25, the same day,
    /// and this test is what settles which side of it those services were on. The
    /// assertion is on the wall, not on the offer list — the parallel is still
    /// OFFERED, which is right, because a preacher who genuinely moved to the parallel
    /// is one click away.
    #[test]
    fn a_reading_may_not_walk_off_the_cited_passage_onto_its_parallel() {
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        let cases = [
            (
                "And he said unto them, Unto you it is given to know the mysteries of \
                 the kingdom of God, but unto them that are without",
                ("Mark", 4, 11),
                ("Luke", 8, 10),
            ),
            (
                "he will teach us of his ways, and we will walk in his paths, for out \
                 of Zion shall go forth the law, and the word of the Lord from Jerusalem",
                ("Micah", 4, 2),
                ("Isaiah", 2, 3),
            ),
        ];
        for (heard, cited, parallel) in cases {
            // THE WALL AS IT WAS: the verse he cited, put up sixteen seconds earlier.
            let mut context = ContextMemory::default();
            let on_screen = detection::VerseRef {
                book: cited.0.into(),
                chapter: cited.1,
                verse: cited.2,
            };
            context.note(&on_screen);
            let wall = Fire::key_for(&on_screen);
            let w = candidates_for_window(
                heard,
                true,
                &sem,
                &phrases,
                &context,
                Some(wall.as_str()),
                false,
            );
            for c in &w.kept {
                println!(
                    "   KEPT  {} {}:{}  {:?}  {:.2}",
                    c.r.book, c.r.chapter, c.r.verse, c.method, c.conf
                );
            }
            for (c, why) in &w.held {
                println!(
                    "   HELD  {} {}:{}  {:?}",
                    c.r.book, c.r.chapter, c.r.verse, why
                );
            }
            // PRECONDITION: the parallel really is found, so the assertion is not
            // vacuous. Its absence would mean the index changed, not that the guard
            // worked.
            let found_anywhere = w
                .kept
                .iter()
                .map(|c| &c.r)
                .chain(w.held.iter().map(|(c, _)| &c.r))
                .any(|r| r.book == parallel.0 && r.chapter == parallel.1);
            assert!(
                found_anywhere,
                "precondition: {} {}:{} must still be found for this to be a test",
                parallel.0, parallel.1, parallel.2
            );
            // THE CLAIM: the parallel may not reach the wall while the cited passage
            // is on it and the preacher is still reading that passage.
            let firable: Vec<_> = w
                .kept
                .iter()
                .filter(|c| c.r.book == parallel.0 && c.method.unattended_rank() > 0)
                .map(|c| (Fire::key_for(&c.r), c.method, c.conf))
                .collect();
            assert!(
                firable.is_empty(),
                "the wall left {} {}:{} for its parallel while he was still reading \
                 it: {firable:?}",
                cited.0,
                cited.1,
                cited.2
            );
        }
    }

    /// **A DOUBT MUST REACH EVERY CANDIDATE CARRYING THAT REFERENCE** — found
    /// 2026-09-26 while measuring the anchor fix, by printing a window's candidate
    /// set instead of reasoning about it.
    ///
    /// Service 40, 2026-09-25 at 4691.7 s: *"…all my springs are in thee. Psalm 27
    /// verse 1 to 7."* Those words are **Psalms 87:7**; he said Psalm 27. The window
    /// produces the reference TWICE —
    ///
    /// | candidate | from |
    /// |---|---|
    /// | `Psalms 27:1` `Direct` 0.95 | the range parse, `detect_direct` |
    /// | `Psalms 27:1` `Direct` 0.88 | the bare *"verse 1"*, anchored on that parse |
    ///
    /// — and `doubt_from_a_quotation` demotes by INDEX, so it caught the first and
    /// the duplicate went to the wall still `Direct`. The doubt rule was working and
    /// its conclusion was being discarded.
    ///
    /// This is the shape CLAUDE.md records four times over: *a guarantee is only kept
    /// Scratch probe: which wording of Romans 1:8 is a `Semantic` candidate without
    /// already being a `Reading`? Used to build the test below honestly rather than
    /// guessing at a paraphrase.
    #[test]
    #[ignore]
    fn print_candidate_methods_for_retellings() {
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        let ctx = ContextMemory::default();
        for t in [
            "your faith is spoken of throughout the whole world",
            "I thank my God through Jesus Christ for you all",
            "he thanks God for them because their faith is spoken of everywhere",
            "he gives thanks to God for them all because their faith is known everywhere",
            "first he thanks his God for all of them because their faith is talked about",
        ] {
            let w = candidates_for_window(t, true, &sem, &phrases, &ctx, None, false);
            let rows: Vec<String> = w
                .kept
                .iter()
                .map(|c| format!("{} {:?} {:.2}", Fire::key_for(&c.r), c.method, c.conf))
                .collect();
            println!("  {t:?}\n      {rows:?}\n");
        }
    }

    /// **FOLLOWING THE PREACHER THROUGH A PASSAGE HE ANNOUNCED**, and the four
    /// boundaries that keep it from becoming rule 10's own failure.
    ///
    /// The operator's report: a preacher announces *"Romans 1 … verse 6 all the way to
    /// 8"* and then RETELLS the verses rather than reading them word for word. A
    /// retelling is `Semantic`, rule 10 caps it at `Suggest` at any score, so the wall
    /// stayed on the verse he started from. Inside an explicit span the answer is
    /// bounded to verses he named out loud, which is what makes this a different act
    /// from choosing a verse out of 31,102.
    ///
    /// Not `#[ignore]`d: it builds the real 31k index once, like its neighbours, and
    /// the four refusals are the whole value.
    #[test]
    #[ignore]
    fn a_retelling_follows_the_preacher_only_inside_a_passage_he_announced() {
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));

        // Romans 1:6-8 announced, wall on verse 6. A retelling of verse 8.
        // A genuine RETELLING of Romans 1:8, not a reading of it: no run of the
        // verse's own words long enough to be a `Reading` on its own merits. The
        // first draft of this test used wording close to the KJV and was a `Reading`
        // with no span at all, which would have made boundary 2 vacuous.
        // A genuine RETELLING of Romans 1:8 and not a reading of it: `Semantic` 0.47
        // with NO `Reading` beside it. Chosen with
        // `print_candidate_methods_for_retellings` rather than guessed — the first
        // draft used wording close to the KJV, which came out `Reading 0.72` on its
        // own merits and would have made boundary 2 vacuous, and the second draft was
        // so loose it was not a candidate at all and made boundary 1 vacuous.
        const RETELL: &str = "he thanks God for them because their faith is spoken of everywhere";
        let staged = |verse: i64, end: Option<i64>| {
            let mut c = ContextMemory::default();
            c.note_passage(
                &detection::VerseRef {
                    book: "Romans".into(),
                    chapter: 1,
                    verse,
                },
                end,
            );
            c
        };
        let firable = |ctx: &ContextMemory| -> Vec<String> {
            candidates_for_window(RETELL, true, &sem, &phrases, ctx, None, false)
                .kept
                .iter()
                .filter(|c| c.method.unattended_rank() > 0)
                .map(|c| format!("{} {:?}", Fire::key_for(&c.r), c.method))
                .collect()
        };

        // 1. With the span announced, a retelling of a LATER verse may follow.
        let inside = firable(&staged(6, Some(8)));
        println!("  inside an announced span: {inside:?}");
        assert!(
            inside.iter().any(|r| r.starts_with("Romans 1:8")),
            "a retelling of verse 8 did not follow the preacher inside Romans 1:6-8: {inside:?}"
        );

        // 2. NO span — the same words, the same wall, nothing may fire. This is the
        //    boundary that keeps rule 10 intact: a whole chapter is not an
        //    announcement of how far he means to read.
        assert!(
            firable(&staged(6, None)).is_empty(),
            "a retelling reached the wall with no announced range: {:?}",
            firable(&staged(6, None))
        );

        // 3. BACKWARDS is refused — forward-only, so a retelling that brushes an
        //    earlier verse cannot walk the wall back or oscillate.
        assert!(
            firable(&staged(8, Some(8))).is_empty(),
            "a retelling moved the wall backwards inside a passage"
        );

        // 4. PAST THE END is refused: verse 9 is outside what he announced.
        let past = candidates_for_window(
            RETELL,
            true,
            &sem,
            &phrases,
            &staged(6, Some(7)),
            None,
            false,
        );
        assert!(
            past.kept
                .iter()
                .all(|c| c.method.unattended_rank() == 0 || c.r.verse <= 7),
            "a verse past the announced end became firable: {:?}",
            past.kept
                .iter()
                .map(|c| (Fire::key_for(&c.r), c.method))
                .collect::<Vec<_>>()
        );
    }

    /// on the doors you checked*. The doubt is a fact about a REFERENCE, not about one
    /// candidate that happens to name it.
    #[test]
    #[ignore]
    fn a_doubt_reaches_every_candidate_naming_that_reference() {
        const HEARD: &str =
            "And this one was born there, that one was born there, all my springs are in thee. Psalm 27 verse 1 to 7.";
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        let context = ContextMemory::default();
        let w = candidates_for_window(HEARD, true, &sem, &phrases, &context, None, false);
        for c in &w.kept {
            println!(
                "   {} {}:{}  {:?}  {:.2}  rank={}",
                c.r.book,
                c.r.chapter,
                c.r.verse,
                c.method,
                c.conf,
                c.method.unattended_rank()
            );
        }
        let doubted: std::collections::HashSet<String> =
            w.doubted.iter().map(|d| d.reference.clone()).collect();
        let escaped: Vec<_> = w
            .kept
            .iter()
            .filter(|c| doubted.contains(&Fire::key_for(&c.r)))
            .filter(|c| c.method.unattended_rank() > 0)
            .map(|c| (Fire::key_for(&c.r), c.method, c.conf))
            .collect();
        assert!(
            escaped.is_empty(),
            "a doubted reference still has a firable candidate: {escaped:?}"
        );
    }

    /// **A MISHEARD "VERSE" INVENTS A BOOK AND HIJACKS THE QUOTATION RESTRICTION**
    /// — service 42, 2026-09-27 at 3795.5 s, watched live. **A correct verse, read
    /// word for word, was withheld.**
    ///
    /// `cargo test --release a_spurious_trailing_book_hijacks_the_restriction --
    /// --ignored --nocapture`
    ///
    /// He said *"From Psalm 19 VERSE 7, the word says, the law of the Lord is perfect,
    /// converting the soul"* and whisper heard *"Psalm 19, NUMBERS 7"*. That single
    /// mishearing did two things:
    ///
    /// ```text
    ///   detect_direct:  Psalms 19:1  UncertainNumber 0.45  (whole chapter)
    ///                   Numbers 7:1  UncertainNumber 0.45  (whole chapter)
    ///   anchor:         Numbers 7:1   ← the LAST parse wins
    ///   quoted_in:      "Numbers"
    /// ```
    ///
    /// `PhraseIndex::quoted` was restricted to **Numbers** while he was reading
    /// **Psalms 19:7**, so the quotation could not be found at all; the verse surfaced
    /// only through the semantic path at 0.49 and stayed a suggestion. *"The law of the
    /// Lord is perfect, converting the soul"* is that verse verbatim.
    ///
    /// **This is RG-305's restriction in mirror image.** There it discarded the right
    /// verse because the window "named" `Acts` where the preacher said `Proverbs`; here
    /// it discards the right verse because a misheard *"verse"* appended a spurious book
    /// AFTER the real one, and the anchor takes the last parse rather than the best one.
    /// RG-305's cross-book carve-out exists for exactly this and cannot run: it requires
    /// a `Direct` chapter-and-verse in `said_pairs`, and both parses here are
    /// `UncertainNumber` whole-chapters.
    ///
    /// Not fixed. Two candidate shapes, neither measured yet: prefer the FIRST book a
    /// window names over the last when both are whole-chapter guesses, or let the
    /// restriction admit any book the window named rather than only the final one. The
    /// second is closer to what RG-305 actually argues — the restriction exists because
    /// a quotation from a book nobody named is a coincidence, and `Psalms` WAS named.
    #[test]
    #[ignore]
    fn a_spurious_trailing_book_hijacks_the_restriction() {
        const HEARD: &str =
            "From Psalm 19, Numbers 7 The word says, The law of the Lord is perfect, converting the soul,";
        let corpus = kjv_corpus();
        let idx = detection::PhraseIndex::build(&corpus);
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        let context = ContextMemory::default();
        println!("\n  “{HEARD}”");
        println!("  anchor: {:?}", detection::anchor_for_bare_verses(HEARD));
        let r = detection::VerseRef {
            book: "Psalms".into(),
            chapter: 19,
            verse: 7,
        };
        println!(
            "  shared_run_with(Psalms 19:7) = {}",
            idx.shared_run_with(HEARD, &r)
        );
        println!("  -- quoted restricted to Numbers:");
        for h in idx.quoted(HEARD, Some("Numbers"), QUOTED_SUGGESTIONS_MAX) {
            println!(
                "     {} {}:{} run={} sole={}",
                h.r.book, h.r.chapter, h.r.verse, h.run, h.sole
            );
        }
        println!("  -- quoted restricted to Psalms (the book he NAMED FIRST):");
        for h in idx.quoted(HEARD, Some("Psalms"), QUOTED_SUGGESTIONS_MAX) {
            println!(
                "     {} {}:{} run={} sole={}",
                h.r.book, h.r.chapter, h.r.verse, h.run, h.sole
            );
        }
        let w = candidates_for_window(HEARD, true, &sem, &phrases, &context, None, false);
        for c in &w.kept {
            println!(
                "  KEPT  {} {}:{}  {:?}  {:.2}",
                c.r.book, c.r.chapter, c.r.verse, c.method, c.conf
            );
        }
        // THE CLAIM: a verse cited by chapter and verse AND read verbatim should be
        // able to reach the wall. Today it cannot, because the restriction followed a
        // mishearing.
        let firable = w.kept.iter().any(|c| {
            c.r.book == "Psalms"
                && c.r.chapter == 19
                && c.r.verse == 7
                && c.method.unattended_rank() > 0
        });
        assert!(
            firable,
            "Psalms 19:7 was read word for word and cannot reach the wall; the \
             quotation search was locked to Numbers by a misheard \"verse\""
        );
    }

    /// **A CITATION SPLIT ACROSS TWO WINDOWS LOSES ITS BOOK — service 42,
    /// 2026-09-27 at 2072.8 s, watched live. The constructive half of RG-315.**
    ///
    /// `cargo test --release a_citation_split_across_windows -- --ignored --nocapture`
    ///
    /// The offering reading, verbatim from `transcripts`:
    ///
    /// ```text
    ///   2072.8  "Let's read from God's Word before we give Genesis chapter 8 and verse"
    ///   2081.2  "Verse 22. This is God's commandment. It says, While the earth remained,"
    ///   2098.5  → the operator fired Genesis 8:22 BY HAND
    /// ```
    ///
    /// **Every existing guard worked and the operator still did the work.** The first
    /// window ends on a dangling verse MARKER, which `parse_reference` refuses
    /// outright, so no `Genesis 8:1` went up — correct. The second window holds a bare
    /// *"Verse 22"* with no book, so `resolve_bare_verse_with_source` fell through to
    /// memory, which still held `Ephesians 5:20` from **eighteen minutes earlier**, and
    /// offered `Ephesians 5:22` at 0.88 `UncertainBook` — capped, never fired, also
    /// correct (rule 40's third half, RG-179).
    ///
    /// **`Genesis 8:22` was never offered at all.** Relay held both halves of the
    /// reference eight seconds apart and cannot join them, because the anchor is
    /// window-local: memory reached back eighteen minutes to the wrong book while the
    /// right chapter sat in the previous window, unremembered.
    ///
    /// RG-315 stops a truncated citation putting verse 1 on a wall. This is the same
    /// mechanism in the other direction: **a chapter stated at a window's edge should
    /// survive one window, so the next window's bare verse can complete it.** Rule 40's
    /// ordering already says a book named in this breath beats memory; a book named one
    /// breath ago should also beat memory from eighteen minutes ago.
    ///
    /// **CLOSED 2026-09-28.** `detection::chapter_in_flight` names the two shapes a
    /// window can end on with the sentence still open, `ContextMemory` carries one
    /// for `IN_FLIGHT_MS`, and `resolve_bare_verse_with_source` consults it between
    /// the anchor and memory. It is `UncertainBook`, so `Genesis 8:22` is OFFERED and
    /// can never fire — which is all three field instances needed, because all three
    /// were already refused correctly and what they cost was the right verse being
    /// absent from the list.
    ///
    /// Not `#[ignore]`d any more, and it drives BOTH windows in order through the
    /// same two calls `emit_detections` makes, so the carry is exercised rather than
    /// assumed.
    #[test]
    fn a_citation_split_across_windows_loses_its_book() {
        const W1: &str = "Let's read from God's Word before we give Genesis chapter 8 and verse";
        const W2: &str = "Verse 22. This is God's commandment. It says, While the earth remained,";
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        // MEMORY AS IT WAS: Ephesians 5:20 fired at 1003.1 s and was still the passage.
        let mut context = ContextMemory::default();
        context.note(&detection::VerseRef {
            book: "Ephesians".into(),
            chapter: 5,
            verse: 20,
        });
        // THE REAL TIMESTAMPS, so the 8.4 s gap is the one the budget has to cover.
        for (label, at_ms, text) in [("window 1", 2_072_800u64, W1), ("window 2", 2_081_200, W2)] {
            println!("\n  {label}: “{text}”");
            println!("     anchor: {:?}", detection::anchor_for_bare_verses(text));
            println!("     in flight: {:?}", detection::chapter_in_flight(text));
            println!(
                "     bare verses: {:?}",
                detection::detect_bare_verses(text)
            );
            let w = candidates_for_window(text, true, &sem, &phrases, &context, None, false);
            for c in &w.kept {
                println!(
                    "     KEPT  {} {}:{}  {:?}  {:.2}",
                    c.r.book, c.r.chapter, c.r.verse, c.method, c.conf
                );
            }
            if text == W2 {
                // THE CLAIM: the verse he actually read must be reachable from the
                // SECOND window, because the FIRST window named its book and chapter.
                // Asserted on the set that window actually produced, not on a third
                // re-read of it after the loop — a re-read is a different call with a
                // different `in_flight` and would be asserting about something the
                // product never computed.
                let genesis = w
                    .kept
                    .iter()
                    .find(|c| c.r.book == "Genesis" && c.r.chapter == 8 && c.r.verse == 22);
                let g = genesis.unwrap_or_else(|| {
                    panic!(
                        "Genesis 8:22 is not reachable from the second window; the \
                         operator fired it by hand while memory offered Ephesians 5:22 \
                         from eighteen minutes before. Got: {:?}",
                        w.kept
                            .iter()
                            .map(|c| (Fire::key_for(&c.r), c.method))
                            .collect::<Vec<_>>()
                    )
                });
                // AND IT MAY NOT FIRE. A book heard one window ago is not a book heard
                // in this breath, and rule 10 only lets the second reach a wall.
                assert_eq!(
                    g.method,
                    DetectionMethod::UncertainBook,
                    "a chapter carried across a window boundary reached a firable method"
                );
                // The wrong answer this replaces must still be offered beside it —
                // `Ephesians 5:22` from eighteen minutes earlier is what memory says,
                // and a rule that silently deleted the rival would be hiding the
                // disagreement rather than resolving it.
                assert!(
                    w.kept
                        .iter()
                        .any(|c| c.r.book == "Ephesians" && c.r.chapter == 5 && c.r.verse == 22)
                        || w.kept.iter().all(|c| c.r.book != "Ephesians"),
                    "the memory answer changed shape unexpectedly: {:?}",
                    w.kept
                        .iter()
                        .map(|c| (Fire::key_for(&c.r), c.method))
                        .collect::<Vec<_>>()
                );
            }
            // The same order `emit_detections` uses: read the window, THEN record
            // what it left unfinished.
            context.note_in_flight(detection::chapter_in_flight(text), at_ms);
        }
    }

    /// **THE OFFER LIST'S TOP ROW WHEN THE CHAPTER WAS SAID ONE WINDOW AGO** —
    /// RG-320's second route, service 42 at 20079.8 s → 20088.0 s, watched live.
    ///
    /// *"Knowing how to do it is wisdom. Ecclesiastes 10."* then *"And verse 15. It
    /// said, the labor of the foolish…"* — those words are **Ecclesiastes 10:15**. On
    /// the morning, the bare *"verse 15"* had no anchor, memory answered with the
    /// passage on the wall (`Matthew 7:24`, fired at 19797.6 s) at the hardcoded 0.88,
    /// and the right verse reached the list only as a `Semantic` 0.38 — BELOW it,
    /// because both are `unattended_rank() == 0` and `pipeline::better` then falls to
    /// confidence, comparing a constant against a cosine.
    ///
    /// **The fix is not a reordering.** RG-318's carry makes the right answer
    /// available at all: the chapter was said one window ago, so `verse 15` hangs on
    /// `Ecclesiastes 10` and the wrong row is not produced in the first place. Nothing
    /// in `pipeline::better` changed, and the hardcoded 0.88 rule 40 calls *"still a
    /// lie"* is still there — it just no longer answers this window.
    ///
    /// RG-320's FIRST route is not reached by this and is not reached by any ordering:
    /// see the row.
    #[test]
    fn a_chapter_from_the_previous_window_answers_the_bare_verse_memory_would_have() {
        const W1: &str =
            "Knowing what to do is knowledge. Knowing how to do it is wisdom. Ecclesiastes 10.";
        const W2: &str =
            "And verse 15. It said, the labor of the foolish willis every one of them because";
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        // THE WALL AS IT WAS: Matthew 7:24 fired at 19797.6 s and was still the
        // passage, which is what memory answered with.
        let mut context = ContextMemory::default();
        context.note(&detection::VerseRef {
            book: "Matthew".into(),
            chapter: 7,
            verse: 24,
        });
        let mut got: Vec<(String, DetectionMethod)> = Vec::new();
        for (at_ms, text) in [(20_079_800u64, W1), (20_088_000, W2)] {
            let w = candidates_for_window(text, true, &sem, &phrases, &context, None, false);
            if text == W2 {
                got = w
                    .kept
                    .iter()
                    .map(|c| (Fire::key_for(&c.r), c.method))
                    .collect();
            }
            context.note_in_flight(detection::chapter_in_flight(text), at_ms);
        }
        println!("  second window: {got:?}");
        assert!(
            got.iter().any(|(k, _)| k == "Ecclesiastes 10:15"),
            "the verse he was reading is still not on the list: {got:?}"
        );
        assert!(
            !got.iter().any(|(k, _)| k == "Matthew 7:15"),
            "memory still answered over a chapter said one window ago: {got:?}"
        );
        // AND IT STILL MAY NOT FIRE — the book was heard in a window this one cannot
        // see, so rule 10's cap holds exactly as it does for a memory answer.
        //
        // **Asserted on FIRABILITY, not on a method name** (corrected 2026-09-28).
        // This read `*m == DetectionMethod::UncertainBook` and broke the moment the
        // bench corpus was routed through `db::clean_verse` (RG-324): on the corpus
        // the product actually ships, this window offers `Ecclesiastes 10:15` TWICE —
        // once as the carried chapter and once as a `Semantic` from the words he was
        // reading — and the second one is a candidate this test never saw because the
        // editorial apparatus had been changing the scores. Both have
        // `unattended_rank() == 0`, so the guarantee this test exists for held the
        // whole time; the assertion was narrower than its own comment. A test that
        // names one permitted method fails on a new candidate that is equally
        // incapable of reaching a wall, and the next person weakens it.
        for (k, m) in &got {
            if k == "Ecclesiastes 10:15" {
                assert_eq!(
                    m.unattended_rank(),
                    0,
                    "a carried chapter reached a firable method: {got:?}"
                );
            }
        }
    }

    /// **THE TWO SHAPES `chapter_in_flight` ANSWERS, AND THE ONES IT MUST NOT** —
    /// RG-318. Pure, so it needs no corpus and no index.
    #[test]
    fn only_a_sentence_that_plainly_stopped_mid_citation_is_in_flight() {
        let f =
            |t: &str| detection::chapter_in_flight(t).map(|r| format!("{} {}", r.book, r.chapter));
        // THE FIELD SHAPE: a dangling verse marker.
        assert_eq!(
            f("Let's read from God's Word before we give Genesis chapter 8 and verse"),
            Some("Genesis 8".into())
        );
        // The CHAPTER NUMBER as the last thing said — RG-315's own window, from the
        // other side.
        assert_eq!(
            f("on the sheet of faith. In John chapter 12,"),
            Some("John 12".into())
        );
        // WITHOUT the chapter keyword, which is RG-320's second instance: "Knowing
        // how to do it is wisdom. Ecclesiastes 10." then "And verse 15" in the next
        // window. This shape was excluded for one commit and the measurement put it
        // back — see `chapter_in_flight`.
        assert_eq!(
            f("Knowing what to do is knowledge. Knowing how to do it is wisdom. Ecclesiastes 10."),
            Some("Ecclesiastes 10".into())
        );
        assert_eq!(f("and then we went to Romans 8"), Some("Romans 8".into()));
        // ── AND THE ONES THAT MUST STAY None ──────────────────────────────────
        // A finished citation.
        assert_eq!(f("Genesis chapter 8 and verse 22 says"), None);
        // A finished citation at the very edge.
        assert_eq!(f("Genesis chapter 8 verse 22"), None);
        // The sentence moved on, so nothing is in flight.
        assert_eq!(f("Genesis chapter 8 tells us what happened"), None);
        // A chapter Genesis cannot have is a misparse, not a citation in flight
        // (RG-322's reasoning, one door along).
        assert_eq!(f("Genesis chapter 80 and verse"), None);
        // No book at all.
        assert_eq!(f("and verse"), None);
        // ── THE OPERATOR'S PAUSE, AND THE LINE IT SITS ON (RG-328) ────────────
        // A run of ordinary words after the chapter carries the citation ONLY when
        // the window ends on a word that cannot end a sentence. Both of these are
        // ordinary words running out the window; only one was cut off mid-phrase.
        assert_eq!(
            f("we are in Romans 1 and we will be reading from"),
            Some("Romans 1".into()),
            "the operator's own phrasing, cut off by the chunker mid-announcement"
        );
        assert_eq!(
            f("let us turn to Psalm 23 and we will be reading from"),
            Some("Psalms 23".into())
        );
        // …and a finished clause is still the sentence moving on. This is the case a
        // generic filler run got wrong on its first draft, caught by this test.
        assert_eq!(f("Genesis chapter 8 tells us what happened"), None);
        assert_eq!(f("Romans 1 explains the gospel to everyone"), None);
    }

    /// **EXPLAIN ONE WINDOW: every stage, every candidate, every doubt.**
    ///
    /// `RELAY_EXPLAIN_WINDOW="<text>" cargo test --release explain_one_window --
    /// --ignored --nocapture`
    ///
    /// Built 2026-09-26 after the anchor fix moved fires in ways reading the code
    /// could not account for, and after two rounds of guessing at why. Printing the
    /// set is faster than reasoning about it and it cannot be wrong.
    ///
    /// **TWO LIMITS, both learned by being misled by them.** It starts from an EMPTY
    /// `ContextMemory` and an empty wall, so it cannot reproduce a fire that depended
    /// on the passage already on screen — a replay difference this cannot explain may
    /// simply be memory. And `print_every_auto_fire` replays a corpus through the real
    /// `Router`, which self-calibrates on feedback, so the run is globally coupled:
    /// one changed candidate set shifts thresholds for everything after it. **Aggregate
    /// counts and presence checks from that instrument are sound; per-verse attribution
    /// is not.**
    #[test]
    #[ignore]
    fn explain_one_window() {
        let Ok(text) = std::env::var("RELAY_EXPLAIN_WINDOW") else {
            println!("set RELAY_EXPLAIN_WINDOW");
            return;
        };
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        let context = ContextMemory::default();
        println!("\n  “{text}”");
        println!(
            "  anchor (firable only): {:?}",
            detection::anchor_for_bare_verses(&text)
        );
        println!("  bare verses: {:?}", detection::detect_bare_verses(&text));
        // THE RAW INDEX ANSWER, before anything in this crate has an opinion about
        // it. Added while chasing RG-320: two verses that share a run are one
        // `quoted` call away from being visibly two rows or visibly one, and the
        // candidate list above cannot tell those apart. `run` and `sole` are the two
        // facts `DetectionMethod` throws away on its way to `for_quotation`.
        println!("  -- quoted (unrestricted, run/sole as the index measures them):");
        if let Ok(g) = phrases.0.read() {
            let hits = g.quoted(&text, None, QUOTED_SUGGESTIONS_MAX);
            if hits.is_empty() {
                println!("     NOTHING");
            }
            for h in hits {
                println!(
                    "     {} {}:{}  run={} sole={}  “{}”",
                    h.r.book, h.r.chapter, h.r.verse, h.run, h.sole, h.phrase
                );
            }
        }
        println!("  -- detect_direct:");
        for m in detection::detect_direct(&text) {
            println!(
                "     {} {}:{}  {:?}  {:.2}  whole={}  end={:?}",
                m.reference.book,
                m.reference.chapter,
                m.reference.verse,
                m.method,
                m.confidence,
                m.whole_chapter,
                m.verse_end
            );
        }
        let w = candidates_for_window(&text, true, &sem, &phrases, &context, None, false);
        println!("  -- KEPT:");
        for c in &w.kept {
            println!(
                "     {} {}:{}  {:?}  {:.2}",
                c.r.book, c.r.chapter, c.r.verse, c.method, c.conf
            );
        }
        for (c, why) in &w.held {
            println!(
                "  HELD  {} {}:{}  {:?}",
                c.r.book, c.r.chapter, c.r.verse, why
            );
        }
        for d in &w.doubted {
            println!(
                "  DOUBT {}  {:?}  {:?} -> {:?}",
                d.reference, d.doubt, d.was, d.method
            );
        }
        println!("  -- rank_for_wall order:");
        let mut ranked: Vec<&Cand> = w.kept.iter().collect();
        ranked.sort_by(|a, b| {
            if pipeline::better(a, b) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });
        for c in ranked {
            println!(
                "     {} {}:{}  {:?}  {:.2}",
                c.r.book, c.r.chapter, c.r.verse, c.method, c.conf
            );
        }
        println!();
    }

    /// **EVERY AUTO-FIRE A CORPUS PRODUCES, ONE PER LINE** — the instrument for
    /// comparing two revisions of the detection path against real speech.
    ///
    /// `RELAY_SERVICE_CORPUS=<file> cargo test --release print_every_auto_fire
    /// -- --ignored --nocapture > after.txt`, then the same on the other revision,
    /// then `diff`. The citation-doubt bench only ever compares ITS rule on against
    /// off inside one binary, so it cannot answer *what did this change cost*, which
    /// is the only question rule 13 accepts. Built 2026-09-26, when the anchor fix
    /// (RG-301) moved 11 auto-fires to the suggestion list and nothing existed to say
    /// which 11.
    ///
    /// Output is `<seconds>\t<reference>` and nothing else, so a diff is readable.
    ///
    /// **The replay is globally coupled and a line-by-line diff will lie to you.** The
    /// `Router` self-calibrates on feedback, so a single changed candidate set moves
    /// thresholds for every later window: verses appear and disappear far from the
    /// change that caused them. Use this for TOTALS and for "does reference X fire at
    /// all", and use `explain_one_window` plus a unit test for anything causal. Two
    /// apparent regressions on 2026-09-26 were chased this way before the real cause
    /// turned out to be a duplicate candidate escaping a doubt.
    #[test]
    #[ignore]
    fn print_every_auto_fire() {
        let Ok(path) = std::env::var("RELAY_SERVICE_CORPUS") else {
            println!("set RELAY_SERVICE_CORPUS");
            return;
        };
        let body = std::fs::read_to_string(&path).expect("corpus unreadable");
        let lines: Vec<(f32, String)> = body
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .filter_map(|(t, x)| t.parse::<f32>().ok().map(|t| (t, x.to_string())))
            .collect();
        let run = replay(&lines, true, true);
        for (at, key, _) in &run.fired {
            println!("{at:.1}\t{key}");
        }
        // ON STDERR, so the stdout diff stays `<seconds>\t<reference>` and nothing
        // else. The in-flight count is here because RG-318's carry can only ever add
        // a CAPPED candidate — the auto-fire diff above is guaranteed to be empty for
        // it, so a reader looking only at that diff would conclude the rule does
        // nothing. This is the number that says how often it can speak at all.
        let in_flight = lines
            .iter()
            .filter(|(_, t)| detection::chapter_in_flight(t).is_some())
            .count();
        // **AND HOW MANY OF THEM A LIVE INSTALL WOULD NEVER SHOW — RG-323.** This
        // replay has no database, so it cannot ask the one question `emit_detections`
        // asks before anything is broadcast: did the reference resolve to a verse?
        // `if fire.verse_id.is_none() { continue; }` drops an impossible reference
        // before an operator sees a row, let alone a congregation a screen.
        //
        // Service 42 produces two — `Psalms 1:97` at 3893.2 s and `Psalms 14:14` at
        // 17789.3 s, Psalm 1 having six verses and Psalm 14 seven — so the totals this
        // bench prints are two higher than what a church would have watched. **Stated
        // here because a diff is the whole purpose of this instrument**: a change that
        // removes one of those reads as a lost auto-fire and is not one, and the
        // opposite mistake was already made once in this register, where the count of
        // references in a confidence BAND was read as the count of references the field
        // never fired.
        let real: std::collections::BTreeSet<String> =
            kjv_corpus().iter().map(|(r, _)| Fire::key_for(r)).collect();
        let unshowable: Vec<&String> = run
            .fired
            .iter()
            .map(|(_, key, _)| key)
            .filter(|key| !real.contains(*key))
            .collect();
        eprintln!(
            "{} auto-fires from {} windows · {} windows end mid-citation (RG-318) · \
             {} name a verse the bundled Bible does not have and a live install drops \
             them at emit_detections' verse-exists check: {:?}",
            run.fired.len(),
            lines.len(),
            in_flight,
            unshowable.len(),
            unshowable
        );
    }

    /// **EVERY WINDOW A CROSS-WINDOW CONTINUITY RULE WOULD REORDER — RG-320,
    /// measured 2026-09-28.**
    ///
    /// `RELAY_SERVICE_CORPUS=<file> cargo test --release
    /// what_a_continuity_tie_break_would_reorder -- --ignored --nocapture`
    ///
    /// RG-320's route 1 is `Jeremiah 6:16` `Quoted` 0.60 offered above
    /// `Matthew 11:29` `Semantic` 0.48 at 19178.0 s. The index is right — *"find rest
    /// for your souls"* is `sole` in Jeremiah 6:16, because Matthew reads *"rest UNTO
    /// your souls"* — so the only verbatim evidence in that window genuinely names
    /// Jeremiah. The evidence that names Matthew is in the window BEFORE it, where
    /// *"Come unto me. I'll give you rest"* offered `Matthew 11:28` `Semantic` 0.36 as
    /// its top row, and nothing in Relay joins the two.
    ///
    /// **The rule this bench measures is the narrowest thing that would join them**:
    /// among the candidates that may only be OFFERED (`unattended_rank() == 0`), prefer
    /// one whose book and chapter a candidate of the previous window already named. It
    /// cannot touch a congregation's screen — every candidate it compares is already
    /// capped at `Suggest` by rule 10, and `rank_for_wall` puts the firable tiers ahead
    /// of all of them — so the whole of its effect is which row an operator reads
    /// first.
    ///
    /// ── WHY THIS IS A BENCH AND NOT THE RULE ─────────────────────────────────────
    ///
    /// **14 windows of 3,161 reorder, and reading them is a human's job, not this
    /// machine's.** Rule 13 is explicit that a detection change is scored by *which
    /// verse would Relay put on a screen* and never by reading the transcript, and
    /// there is no labelled corpus anywhere in this repository for *which offered row
    /// should be first*. So the 14 are printed rather than counted, and the reading
    /// below is recorded as a reading.
    ///
    /// On 2026-09-28 the author's own reading of the printed 14 was **12 better, 2
    /// worse**. The two it gets wrong are the same shape as each other — a first row
    /// that is verbatim and correct, losing to a chapter carried from a window the
    /// preacher has left:
    ///
    /// ```text
    ///   3349.5  "You shall serve the Lord your God and He shall bless"
    ///           Exodus 23:25 is right; Joshua 24:15 would rise
    ///   5653.4  "The Lord here in the day of trouble, the name of the God of Jacob"
    ///           Psalms 20:1 is right; Deuteronomy 29:15 would rise
    /// ```
    ///
    /// **And two of the twelve are the exact case RG-320's hard constraint is about.**
    /// `Psalms 111:4`/`Psalms 112:4` at 7736.0 s and `Proverbs 6:10`/`Proverbs 24:33`
    /// at 11933.4 s are word-for-word identical and tie on confidence, so today the
    /// order between them is arbitrary. Continuity separates both correctly and **both
    /// rows are still offered** — the constraint is about not DROPPING a row, and
    /// nothing here drops one. That is the strongest single argument for the rule and
    /// it is why this measurement is worth keeping rather than discarding with the
    /// experiment.
    ///
    /// Nothing is changed by running this. The next reader needs the 14 windows and a
    /// person who knows what the preacher meant, and that person is the operator.
    #[test]
    #[ignore]
    fn what_a_continuity_tie_break_would_reorder() {
        let Ok(path) = std::env::var("RELAY_SERVICE_CORPUS") else {
            println!("set RELAY_SERVICE_CORPUS");
            return;
        };
        let body = std::fs::read_to_string(&path).expect("corpus unreadable");
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        // EMPTY AND LEFT EMPTY, like `explain_one_window`: the reorder this measures is
        // between two offered rows, and nothing on the wall is allowed to decide that
        // (rule 40 — memory only ever re-ranks a quotation, and this bench must not
        // quietly reproduce that rule and call it this one).
        let context = ContextMemory::default();
        let lines: Vec<(f32, String)> = body
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .filter_map(|(t, x)| t.parse::<f32>().ok().map(|t| (t, x.to_string())))
            .collect();
        // The book and chapter of every offered candidate of the PREVIOUS window. One
        // window back and no further: `ContextMemory::in_flight` is the precedent for a
        // carry on this path and it is a single slot with a ceiling, because a carry
        // that lingers is a carry that starts answering for a passage the preacher has
        // left — which is exactly the failure the two regressions above are.
        let mut prev: Vec<(String, i64)> = Vec::new();
        let mut reorders = 0usize;
        println!();
        for (at, text) in &lines {
            let w = candidates_for_window(text, true, &sem, &phrases, &context, None, false);
            let offered_now: Vec<&Cand> = w
                .kept
                .iter()
                .filter(|c| c.method.unattended_rank() == 0)
                .collect();
            let mut offered = offered_now.clone();
            // The order an operator reads today: `pipeline::better` inside one tier is
            // confidence alone, which is RG-320's diagnosis — a run length and a cosine
            // compared as though they were one scale.
            offered.sort_by(|a, b| {
                b.conf
                    .partial_cmp(&a.conf)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let continues = |c: &Cand| {
                prev.iter()
                    .any(|(b, ch)| *b == c.r.book && *ch == c.r.chapter)
            };
            if offered.len() >= 2 && !continues(offered[0]) {
                if let Some(c) = offered.iter().skip(1).find(|c| continues(c)) {
                    reorders += 1;
                    println!(
                        "  {at:.1}\n     reads first  {} {}:{}  {:?} {:.2}\n     would rise   \
                         {} {}:{}  {:?} {:.2}\n     “{}”",
                        offered[0].r.book,
                        offered[0].r.chapter,
                        offered[0].r.verse,
                        offered[0].method,
                        offered[0].conf,
                        c.r.book,
                        c.r.chapter,
                        c.r.verse,
                        c.method,
                        c.conf,
                        text.chars().take(90).collect::<String>()
                    );
                }
            }
            prev = offered_now
                .iter()
                .map(|c| (c.r.book.clone(), c.r.chapter))
                .collect();
        }
        println!(
            "\n  {reorders} of {} windows reorder. Read them; do not count them.\n",
            lines.len()
        );
    }

    /// **WHY THE DOUBT RULE NEVER HAD A DOUBT TO CARRY — RG-319, measured
    /// 2026-09-28. A WRONG VERSE REACHED A CONGREGATION and the register's account of
    /// the mechanism does not reproduce.**
    ///
    /// `cargo test --release why_no_doubt_was_available_for_psalms_119_39 -- --ignored
    /// --nocapture`
    ///
    /// Service 42, verbatim from `transcripts`:
    ///
    /// ```text
    ///   19730.1  "What is this? … For my covenant will I not break."   → offered Psalms 89:34
    ///   19737.1  "… the comfort of my lips. Psalm 119, verse 39. …"    → FIRED Psalms 119:39
    /// ```
    ///
    /// The row says a doubt *"was computable one pass earlier"* and that the first
    /// window held *"everything `chapter_the_words_point_at` needs"*. It did not.
    /// Given the two windows JOINED — which is what a rolling window would have
    /// produced, and the most generous reading of the claim — the rule produces **no
    /// doubt at all**, and `the_run_contradicts` refuses it twice over:
    ///
    ///  * the run's verse is **34** and the spoken reference says **39**, so
    ///    `verse_inside_what_was_said` is false. That is the rule's most deliberate
    ///    line — *a verse-only difference is never a disagreement, at any distance* —
    ///    and it is what protects `John 15:14` cited while 15:15 is read.
    ///  * **89 is not a decode slip of 119.** `chapter_is_a_decode_slip` needs either
    ///    a lost leading digit (119 does not end in 89) or one substitution at equal
    ///    length (two digits against three).
    ///
    /// So neither a doubt carried forward nor a RUN carried forward reaches this
    /// window. Reaching it means admitting a disagreement where both coordinates
    /// differ, and `doubt_from_a_quotation` records what that costs.
    ///
    /// This is kept as a bench rather than an assertion **because there is nothing
    /// here to assert yet.** It prints the set so the next reader starts from the
    /// measurement instead of from the row.
    #[test]
    #[ignore]
    fn why_no_doubt_was_available_for_psalms_119_39() {
        const W1: &str =
            "What is this? It becomes finding on God to affirm. For my covenant will I not break.";
        const W2: &str = "The author does send the comfort of my lips. Psalm 119, verse 39. If you cannot bring my covenant of the";
        const JOINED: &str = "For my covenant will I not break. The author does send the comfort of my lips. Psalm 119, verse 39.";
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        let context = ContextMemory::default();
        println!();
        for (label, text) in [
            ("19730.1 (offered only)", W1),
            ("19737.1 (FIRED)", W2),
            ("both windows JOINED", JOINED),
        ] {
            println!("  ── {label}: “{text}”");
            let w = candidates_for_window(text, true, &sem, &phrases, &context, None, false);
            for c in &w.kept {
                println!(
                    "     KEPT  {} {}:{}  {:?}  {:.2}",
                    c.r.book, c.r.chapter, c.r.verse, c.method, c.conf
                );
            }
            for d in &w.doubted {
                println!(
                    "     DOUBT {}  {:?}  {:?} -> {:?}",
                    d.reference, d.doubt, d.was, d.method
                );
            }
            if w.doubted.is_empty() {
                println!("     DOUBT none");
            }
        }
        println!(
            "\n  the two refusals, stated: verse 34 is not inside “verse 39”, and \
             chapter 89 is not a decode slip of 119\n"
        );
    }

    /// **WHY A RUN CARRIED ACROSS A WINDOW CANNOT REACH `Psalms 119:39` EITHER —
    /// RG-319, measured 2026-09-28 by building the rule and replaying it.**
    ///
    /// `why_no_doubt_was_available_for_psalms_119_39` shows the window-local rule had
    /// nothing to carry. The obvious next move is the one the row nominates: carry the
    /// RUN forward a window, and admit a disagreement where BOTH coordinates differ —
    /// same book, a different chapter, whatever the verse. That reaches the wall.
    /// **It was built, gated, and replayed over service 42's 3,161 windows: auto-fires
    /// 153 → 148.** `Psalms 119:39` went, and so did `Psalms 41:1`,
    /// `1 Corinthians 12:7` and `Matthew 6:33`, all three correct, plus a duplicate
    /// `Isaiah 11:1` and a 2.8 s delay to `Luke 11:49`.
    ///
    /// **The register refused this rule on evidence that does not apply to it, and
    /// that correction is half the point of this test.** RG-319 cites `John 15:15`
    /// read while *"John 15, 14"* was cited, and `Hebrews 13:17` quoted while 13:7 was
    /// insisted on. Both are ONE book, ONE chapter, a different verse — the
    /// same-chapter escape in `the_run_contradicts` refuses them on its own, and a
    /// rule about differing CHAPTERS never touches either. The refusal was right; its
    /// stated reason was not.
    ///
    /// **The reason that holds is in this test, and it is four windows of one
    /// service.** Three correct auto-fires have the identical shape to the wrong one:
    ///
    /// ```text
    ///   Psalms 20:3          6 words, sole  →  "Now Psalm 41 verse 1."        RIGHT
    ///   1 Corinthians 2:16   7 words, sole  →  "1 Corinthians 12, verse 7"    RIGHT
    ///   Matthew 13:44        5 words, sole  →  "hidden in Matthew 6, 33."     RIGHT
    ///   Psalms 89:34         6 words, sole  →  "Psalm 119, verse 39."         WRONG
    /// ```
    ///
    /// A preacher quoting one chapter and then citing another chapter of the same book
    /// is the ordinary thing, and a decoder that drops a digit leaves exactly the same
    /// trace. **No length bar separates them**: the wrong one's run is neither the
    /// longest nor the shortest of the four, which the second assertion holds.
    ///
    /// **And waiting a window for the cited chapter to be confirmed does not separate
    /// them either.** Only `1 Corinthians 12:7` is read aloud verbatim in the window
    /// after its citation; `Psalms 41:1` and `Matthew 6:33` are not, so a
    /// hold-until-agreed rule withholds the wall AND two correct fires. The third
    /// assertion holds that, so the idea cannot be re-proposed without this test
    /// failing.
    ///
    /// Not `#[ignore]`d: it is four windows against the bundled index, and it is the
    /// only thing standing between the next reader and re-measuring all of this.
    #[test]
    fn a_carried_run_cannot_tell_a_misheard_chapter_from_the_next_one_he_cites() {
        /// The longest run in this window that exactly one verse holds — the only
        /// evidence `doubt_from_a_quotation` accepts, and so the only thing a carry
        /// could carry.
        fn sole_run(idx: &detection::PhraseIndex, text: &str) -> Option<(VerseRef, usize)> {
            idx.quoted(text, None, QUOTED_SUGGESTIONS_MAX)
                .into_iter()
                .filter(|h| h.sole && h.run >= detection::MIN_RUN_WORDS)
                .max_by_key(|h| h.run)
                .map(|h| (h.r, h.run))
        }
        let idx = detection::PhraseIndex::build(&kjv_corpus());

        // (the window that carries the run, the window that cites, what it cites,
        //  the window AFTER the citation, whether the citation was CORRECT)
        let field: [(&str, &str, &str, &str, bool); 4] = [
            (
                "And, as I said, I am strengthening thee out of Zion. Remember all thy \
                 offerings and accept their bond sacrifices.",
                "That's what your offering does among others. It delivers in the day of \
                 trouble. Now Psalm 41 verse 1.",
                "Psalms 41:1",
                "Blessed is the man that considerate the poor",
                true,
            ),
            (
                "He said, but we have the mind of Christ and because this is endowed",
                "1 Corinthians 12, verse 7 The Bible tells us,",
                "1 Corinthians 12:7",
                "us this, he says that the manifestation of the spirit, as he spoke about \
                 the gifts of the spirit, he said he's giving",
                true,
            ),
            (
                "The kingdom of heaven is like unto treasure, eat in the field, which any \
                 man has found.",
                "You cannot discover the treasure hidden in Matthew 6, 33.",
                "Matthew 6:33",
                "And not by, in to you, with utmost delight, with utmost delight.",
                true,
            ),
            (
                "What is this? It becomes finding on God to affirm. For my covenant will I \
                 not break.",
                "The author does send the comfort of my lips. Psalm 119, verse 39. If you \
                 cannot bring my covenant of the",
                "Psalms 119:39",
                "The end of the night. Then don't try it. You can't break my covenant with \
                 my servant David.",
                false,
            ),
        ];

        let mut right: Vec<usize> = Vec::new();
        let mut wrong: Vec<usize> = Vec::new();
        let mut confirmed_next_window = 0;
        for (carries, cites, reference, after, correct) in field {
            let (run_at, words) =
                sole_run(&idx, carries).unwrap_or_else(|| panic!("no sole run in “{carries}”"));
            let cited = detection::detect_direct(cites)
                .into_iter()
                .find(|m| m.method == DetectionMethod::Direct)
                .unwrap_or_else(|| panic!("nothing Direct in “{cites}”"));
            assert_eq!(
                Fire::key_for(&cited.reference),
                reference,
                "the citation this window makes has changed"
            );
            // THE WHOLE POINT: the four are indistinguishable to the widened rule.
            assert_eq!(
                run_at.book, cited.reference.book,
                "{reference}: the carried run left the book, so the rule under test \
                 would not have reached this window at all"
            );
            assert_ne!(
                run_at.chapter, cited.reference.chapter,
                "{reference}: same chapter is already escaped by `the_run_contradicts`"
            );
            if sole_run(&idx, after).is_some_and(|(r, _)| {
                r.book == cited.reference.book && r.chapter == cited.reference.chapter
            }) {
                confirmed_next_window += 1;
            }
            if correct {
                right.push(words);
            } else {
                wrong.push(words);
            }
        }

        // NO LENGTH BAR SEPARATES THEM. The one wrong citation's run sits inside the
        // spread of the three correct ones, so every bar either keeps all four or
        // takes the wall away along with a correct fire.
        let (lo, hi) = (
            *right.iter().min().expect("right"),
            *right.iter().max().expect("right"),
        );
        for w in &wrong {
            assert!(
                (lo..=hi).contains(w),
                "the wrong citation's run is {w} words and the correct ones span \
                 {lo}..={hi} — if that has changed, a length bar may now separate them \
                 and RG-319 is worth reopening"
            );
        }

        // AND NEITHER DOES WAITING A WINDOW. Exactly one of the four is read aloud
        // verbatim in the window after its citation, and it is a CORRECT one — so a
        // hold-until-agreed rule costs two correct fires to withhold one wall.
        assert_eq!(
            confirmed_next_window, 1,
            "a hold-until-the-next-window-agrees rule is only worth proposing if more \
             than one of these citations is confirmed by the window after it"
        );
    }

    /// **WHICH SPOKEN REFERENCES A MOVING BAR WITHHOLDS** — RG-323.
    ///
    /// `RELAY_SERVICE_CORPUS=<file> cargo test --release what_the_bar_withholds --
    /// --ignored --nocapture`
    ///
    /// RG-323 says 34 references were parsed as `Direct` and never fired, and blames
    /// the corroboration rule: *"corroboration accepts agreement only from a LATER
    /// window"*. **That cannot be the mechanism for its own example, and the proof is
    /// in the code rather than in a replay.** `Router::decide_live` exempts a FINAL
    /// window from corroboration — there is no next pass coming — and
    /// `persist_transcript` is called inside `if update.is_final`, so a row in
    /// `transcripts` at 21608.4 s IS a final window carrying those words. Corroboration
    /// was not consulted.
    ///
    /// What is left is the numeric gate. *"…hidden in Matthew 6, 33."* parses at
    /// **0.55**, which is `parse_reference`'s `bare_digits` value: a pair of bare
    /// digits with no chapter or verse keyword, deliberately scored just above the
    /// DEFAULT `auto_fire` of 0.50 and left dial-controllable. `record_feedback` moves
    /// that bar on every confirm and dismiss, so **one dismissal of anything scoring
    /// 0.55 or more puts the bar above every bare-digit citation for the rest of the
    /// service.**
    ///
    /// **THIS BENCH CANNOT CLASSIFY THE 34, AND THAT IS WHY THE ROW ASKS FOR THE
    /// WAV.** It feeds every line as a final, so corroboration never holds anything;
    /// and its `Router` never receives feedback, so the bar never moves. It has
    /// neither of the two mechanisms that can withhold a `Direct` — `Matthew 6:33`
    /// fires here at 21608.4 s, which is the opposite of the field outcome.
    ///
    /// What it CAN do is bound the class: print every distinct `Direct` reference by
    /// the best confidence the service ever gave it, so a reader can see how many sit
    /// in the band a moved bar sweeps. **No threshold is changed by any of this**
    /// (rule 10): the point is to know which references are one dismissal away from
    /// silence, not to lower the gate that protects a congregation.
    #[test]
    #[ignore]
    fn what_the_bar_withholds() {
        let Ok(path) = std::env::var("RELAY_SERVICE_CORPUS") else {
            println!("set RELAY_SERVICE_CORPUS");
            return;
        };
        let body = std::fs::read_to_string(&path).expect("corpus unreadable");
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        let context = ContextMemory::default();
        // reference → the BEST confidence any window ever gave it as a firable
        // candidate. Best, not last: the question is whether the service ever had
        // evidence strong enough for the gate.
        let mut best: std::collections::BTreeMap<String, f32> = std::collections::BTreeMap::new();
        let mut windows = 0usize;
        for line in body
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(_, t)| t)
            .filter(|t| !t.trim().is_empty())
        {
            windows += 1;
            let w = candidates_for_window(line, true, &sem, &phrases, &context, None, false);
            for c in w
                .kept
                .iter()
                .filter(|c| c.method == DetectionMethod::Direct)
            {
                let e = best.entry(Fire::key_for(&c.r)).or_insert(0.0);
                if c.conf > *e {
                    *e = c.conf;
                }
            }
        }
        // The bands that matter, read off `Thresholds::from_sensitivity`: 0.50 is the
        // default bar, 0.90 the most cautious dial and 0.30 the most eager.
        const BARS: &[f32] = &[0.30, 0.50, 0.55, 0.60, 0.70, 0.90];
        println!(
            "\n  {windows} windows · {} distinct Direct references\n",
            best.len()
        );
        for bar in BARS {
            let withheld = best.values().filter(|c| **c < *bar).count();
            println!(
                "  auto_fire {bar:.2} → {withheld:>3} of {} withheld",
                best.len()
            );
        }
        // ── WHICH OF THEM NEVER REACHED A WALL, AND WHETHER THEY COULD HAVE ──────
        //
        // RG-323's owed classification, as far as it can be taken without the WAV.
        // The row asks which of the never-fired references are *refusals working
        // correctly*, and one class of that answer is a fact about scripture rather
        // than about the audio: **a reference to a verse the bundled Bible does not
        // have was never going to be right.** `Psalms 1:97` is the row's own example,
        // and Psalm 1 has six verses.
        //
        // The distinction matters because the only lever the row's diagnosis offers is
        // a LOWER auto-fire bar (rule 10 in its plainest form), and a reader has to
        // know that the band such a bar would sweep contains references that must stay
        // refused. An impossible one cannot be rescued by any number: it resolves to no
        // text, so `emit_detections`' verse-exists check drops it before an operator
        // ever sees it, and it is refused for the right reason already.
        //
        // The fired set comes from the SAME `replay` every other bench here uses, not
        // from a second scan — two scans of one service that disagreed about the
        // router's clock would be worse than no measurement (the reason this module
        // has one `replay` and switches rather than a bench per rule).
        let lines: Vec<(f32, String)> = body
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .filter_map(|(t, x)| t.parse::<f32>().ok().map(|t| (t, x.to_string())))
            .collect();
        let fired: std::collections::BTreeSet<String> = replay(&lines, true, true)
            .fired
            .into_iter()
            .map(|(_, key, _)| key)
            .collect();
        let real: std::collections::BTreeSet<String> =
            corpus.iter().map(|(r, _)| Fire::key_for(r)).collect();
        let never: Vec<(&String, &f32)> =
            best.iter().filter(|(k, _)| !fired.contains(*k)).collect();
        println!(
            "\n  {} distinct references reached a wall in this replay · {} parsed as \
             Direct and NEVER did:",
            fired.len(),
            never.len()
        );
        for (key, conf) in &never {
            println!("    {conf:.2}  {key}");
        }
        // **THE REPLAY'S NEVER-FIRED SET IS NOT THE FIELD'S, AND THE COUNTS MUST NOT BE
        // READ AS THOUGH IT WERE.** The row counts 34 references the OPERATOR'S service
        // parsed as `Direct` and never put on a wall; this replay withholds far fewer,
        // for the two structural reasons above. They are different sets and the
        // agreement of any two totals between them is a coincidence.
        //
        // What IS transferable is impossibility. A reference to a verse the bundled
        // Bible does not have was never going to be right, whatever the bar — it
        // resolves to no text, so `emit_detections`' verse-exists check drops it before
        // an operator sees it. Those are the row's *"refusals working correctly"*, and
        // they are a fact about scripture rather than about the audio, so this machine
        // can name them without the WAV.
        let cannot_be_right: Vec<(&String, &f32)> =
            best.iter().filter(|(k, _)| !real.contains(*k)).collect();
        println!(
            "\n  {} of the {} parsed references NAME A VERSE THE BUNDLED BIBLE DOES NOT \
             HAVE — these must stay refused at any bar:",
            cannot_be_right.len(),
            best.len()
        );
        for (key, conf) in &cannot_be_right {
            println!("    {conf:.2}  {key}");
        }

        println!("\n  EVERY DISTINCT REFERENCE, WITH ITS BEST CONFIDENCE:");
        let mut rows: Vec<(&String, &f32)> = best.iter().collect();
        rows.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));
        for (key, conf) in rows {
            println!("    {conf:.2}  {key}");
        }
        println!();
    }

    /// **WHAT THE SHORT-RUN PROBE COSTS PER WINDOW** (RG-313).
    ///
    /// `RELAY_SERVICE_CORPUS=<file> cargo test --release what_the_short_run_probe_costs
    /// -- --ignored --nocapture`
    ///
    /// The probe is bounded by the slip test rather than by the corpus, but bounded
    /// is not free: it is one `shared_run_with` for the bar plus one per candidate
    /// chapter, on `relay-detect`, once per decode pass. Rule 31's whole lesson is
    /// that this path is measured and not reasoned about, and the reasoning here
    /// would be especially easy to get wrong — `shared_run_with` walks the window
    /// against a verse, so its cost grows with how much the preacher said.
    #[test]
    #[ignore]
    fn what_the_short_run_probe_costs() {
        let Ok(path) = std::env::var("RELAY_SERVICE_CORPUS") else {
            println!("set RELAY_SERVICE_CORPUS");
            return;
        };
        let body = std::fs::read_to_string(&path).expect("corpus unreadable");
        let corpus = kjv_corpus();
        let idx = detection::PhraseIndex::build(&corpus);
        let mut windows = 0usize;
        let mut probed = 0usize;
        let mut lookups = 0usize;
        let mut spent = std::time::Duration::ZERO;
        for line in body
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(_, t)| t)
        {
            windows += 1;
            for m in detection::detect_direct(line) {
                if m.method != DetectionMethod::Direct || m.whole_chapter {
                    continue;
                }
                probed += 1;
                let claim = detection::Claim {
                    r: &m.reference,
                    method: m.method,
                    verse_end: m.verse_end,
                    whole_chapter: m.whole_chapter,
                    run: None,
                };
                let t = std::time::Instant::now();
                let _ = detection::chapter_the_words_point_at(&claim, 150, |probe| {
                    lookups += 1;
                    idx.shared_run_with(line, probe)
                });
                spent += t.elapsed();
            }
        }
        println!(
            "\n  {windows} windows · {probed} probed candidates · {lookups} index \
             lookups\n  {spent:?} total · {:?} per probed candidate · {:?} per window\n",
            spent / probed.max(1) as u32,
            spent / windows.max(1) as u32
        );
    }

    /// the gap between them, so a wall change every few seconds is visible as a
    /// number instead of as a complaint.
    #[test]
    #[ignore]
    fn how_fast_the_wall_moves() {
        let Ok(path) = std::env::var("RELAY_SERVICE_CORPUS") else {
            println!("set RELAY_SERVICE_CORPUS");
            return;
        };
        let body = std::fs::read_to_string(&path).expect("corpus unreadable");
        let lines: Vec<(f32, String)> = body
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .filter(|(_, t)| !t.trim().is_empty())
            .map(|(a, t)| (a.trim().parse().unwrap_or(0.0), t.to_string()))
            .collect();
        let run = replay(&lines, true, true);
        println!("\n  {} auto-fires\n", run.fired.len());
        let mut same_passage_within_30s = 0;
        for w in run.fired.windows(2) {
            let (t0, k0, _) = &w[0];
            let (t1, k1, heard) = &w[1];
            let gap = t1 - t0;
            let p = |k: &String| {
                k.rsplit_once(':')
                    .map(|(a, _)| a.to_string())
                    .unwrap_or_default()
            };
            let same = p(k0) == p(k1);
            if same && gap <= 30.0 {
                same_passage_within_30s += 1;
            }
            println!(
                "    {t1:>7.1}s  +{gap:>6.1}s  {}{k1:<22} “{heard}”",
                if same {
                    "SAME PASSAGE "
                } else {
                    "             "
                }
            );
        }
        println!(
            "\n  consecutive fires inside one passage within 30s: {same_passage_within_30s}\n"
        );
    }
}

/// **THE PASSAGE GUARD, THROUGH THE ASSEMBLY IT IS APPLIED IN**, 2026-09-25.
///
/// `detection::passage_guard` holds the rule. These hold the WIRING: that the two
/// modules spell a reference the same way, that a held candidate is partitioned out
/// of what reaches the gate, and that the announcement is reachable.
#[cfg(test)]
mod passage_guard_wiring {
    use super::*;
    use detection::VerseRef;

    fn vr(book: &str, chapter: i64, verse: i64) -> VerseRef {
        VerseRef {
            book: book.into(),
            chapter,
            verse,
        }
    }

    /// **THE JOIN, AND IT IS THE ONE THING THAT CAN SILENTLY UNDO THE WHOLE GUARD.**
    ///
    /// Rule B compares a candidate against the string the router says is on the wall.
    /// The router's string comes from `pipeline::Fire::key_for`; the guard's comes
    /// from `detection::reference_key`, because `detection` is DB- and IO-free on
    /// purpose and cannot see `pipeline`. Two spellings of one key would make every
    /// comparison false, hold nothing, break no test and print no error.
    #[test]
    fn the_two_reference_keys_agree() {
        for r in [
            vr("John", 3, 16),
            vr("Psalms", 119, 105),
            // A numbered book — the space inside the name is exactly where a
            // hand-rolled split has gone wrong here before (RG-178's `rsplit_once`).
            vr("1 Corinthians", 13, 4),
            vr("Song of Solomon", 2, 1),
            vr("3 John", 1, 4),
        ] {
            assert_eq!(
                detection::reference_key(&r),
                Fire::key_for(&r),
                "the guard and the router must spell {r:?} the same way"
            );
        }
    }

    /// A held candidate is REMOVED from what reaches the gate — that is what makes
    /// the list stop churning — and it is carried out beside it with its reason, so
    /// nothing is discarded silently.
    #[test]
    fn a_held_candidate_leaves_the_gate_and_arrives_in_the_report() {
        let on = vr("Psalms", 107, 8);
        let elsewhere = vr("Ephesians", 5, 20);
        let candidates = [
            Cand::single(
                on.clone(),
                0.80,
                DetectionMethod::Reading,
                Some("oh that men would praise the lord".into()),
            ),
            Cand::single(
                elsewhere.clone(),
                0.65,
                DetectionMethod::Quoted,
                Some("giving thanks always for all things".into()),
            ),
        ];
        let view: Vec<(&VerseRef, DetectionMethod)> =
            candidates.iter().map(|c| (&c.r, c.method)).collect();
        let mask = detection::hold_for_the_passage(Some(&on), None, false, &view);
        assert_eq!(
            mask,
            vec![None, Some(detection::HeldReason::OutsideTheReading)]
        );
        // The partition `candidates_for_window` performs, asserted on the shapes it
        // produces — `HeldCandidate` carries the reference, the method, the WORDS and
        // the reason, which is everything the operator needs to act on one.
        let held = HeldCandidate {
            reference: Fire::key_for(&elsewhere),
            method: DetectionMethod::Quoted,
            matched_text: Some("giving thanks always for all things".into()),
            reason: detection::HeldReason::OutsideTheReading,
        };
        assert_eq!(held.reference, "Ephesians 5:20");
        let json = serde_json::to_string(&PassageHold {
            passage: Some("Psalms 107".into()),
            reading: Some("oh that men would praise the lord".into()),
            held: vec![held],
            doubted: Vec::new(),
            trace_id: None,
        })
        .expect("the report must serialise");
        // The wire names the console reads. Changing one is changing a contract.
        assert!(json.contains("\"passage\":\"Psalms 107\""), "{json}");
        assert!(
            json.contains("\"reason\":\"outside_the_reading\""),
            "{json}"
        );
        assert!(json.contains("\"method\":\"quoted\""), "{json}");
        // AN ORDINARY HOLD'S PAYLOAD IS BYTE-FOR-BYTE WHAT IT WAS. RG-305 added a
        // field to this event rather than a second event; a field that serialised
        // when it was empty would change every hold report the console already reads.
        assert!(!json.contains("doubted"), "{json}");
    }

    /// **THE CITATION-DOUBT RULE, THROUGH THE ASSEMBLY** (RG-305).
    ///
    /// `detection::citation_doubt` holds the rule. This holds the WIRING: that a run's
    /// two facts land on the run's own candidate and not on a neighbour, that the
    /// demotion is the one the router caps, and that the report reaches the wire.
    #[test]
    fn the_run_facts_land_on_the_quotation() {
        // The mapping `candidates_for_window` builds: the quotation candidates start
        // at `quoted_at` and `run_facts` is indexed from there. Everything before it
        // has no run. Asserted on the arithmetic rather than on a mounted app, because
        // an off-by-one here would attribute a fifteen-word sole run to the reference
        // beside it and every test in the pure module would still pass.
        let quoted_at = 3usize;
        let run_facts = [(7usize, true), (5usize, false)];
        let at = |i: usize| -> Option<(usize, bool)> {
            i.checked_sub(quoted_at)
                .and_then(|k| run_facts.get(k).copied())
        };
        assert_eq!(at(0), None, "a direct reference has no run");
        assert_eq!(
            at(2),
            None,
            "the candidate just before the quotations has none"
        );
        assert_eq!(at(3), Some((7, true)));
        assert_eq!(at(4), Some((5, false)));
        assert_eq!(at(5), None, "past the end of the quotations");
    }

    /// **THE THREE DEMOTIONS ARE ALL METHODS THE ROUTER CAPS**, which is the whole
    /// safety claim. A demotion to something that may auto-fire would be a rule that
    /// looks like a gate and is not one — rule 10's own failure mode, and the reason
    /// `Router::decide` is the door rather than this assembly.
    #[test]
    fn every_doubt_outcome_is_capped_at_suggest() {
        for m in [
            DetectionMethod::UncertainNumber,
            DetectionMethod::UncertainBook,
            DetectionMethod::Quoted,
        ] {
            assert!(
                !m.may_auto_fire(),
                "{m:?} was chosen as a doubt outcome and may reach a wall unattended"
            );
        }
    }

    /// The doubt report on the wire, with the names the console reads.
    #[test]
    fn the_doubt_report_names_itself_on_the_wire() {
        let json = serde_json::to_string(&PassageHold {
            passage: None,
            reading: None,
            held: Vec::new(),
            doubted: vec![
                DoubtedClaim {
                    reference: "Acts 8:12".into(),
                    method: DetectionMethod::UncertainBook,
                    matched_text: Some("acts 8 12".into()),
                    doubt: detection::Doubt::SpokenBook,
                },
                DoubtedClaim {
                    reference: "Proverbs 8:12".into(),
                    method: DetectionMethod::Quoted,
                    matched_text: Some("i wisdom dwell with prudence and find out".into()),
                    doubt: detection::Doubt::TheQuotation,
                },
            ],
            trace_id: None,
        })
        .expect("the report must serialise");
        assert!(json.contains("\"doubt\":\"spoken_book\""), "{json}");
        assert!(json.contains("\"doubt\":\"the_quotation\""), "{json}");
        assert!(json.contains("\"method\":\"uncertain_book\""), "{json}");
        // THE WORDS, never a number (rule 18).
        assert!(
            json.contains("i wisdom dwell with prudence and find out"),
            "{json}"
        );
        assert!(!json.contains("confidence"), "{json}");
    }

    /// `already_on_screen` is the other wire name, and the console tells the two
    /// apart to decide whether the operator has anything to do about it.
    #[test]
    fn the_wall_rule_names_itself_on_the_wire() {
        let json = serde_json::to_string(&detection::HeldReason::AlreadyOnScreen).unwrap();
        assert_eq!(json, "\"already_on_screen\"");
    }
}

/// **THE CHURCH'S PARAPHRASE BAR, THROUGH THE ASSEMBLY THAT APPLIES IT** — the
/// setting `detection.paraphrase_needs_a_run` and `detection::PARAPHRASE_RUN_WORDS`
/// (DECISIONS §125, RG-311).
///
/// `detection::paraphrase_run_bar` holds the run test itself, purely. These hold the
/// WIRING, and the first thing they hold is the one that matters most: **that the
/// switch, OFF, reaches nothing at all.**
///
/// The corpus is invented on purpose. A cosine is a bag of words in no order, so the
/// case this bar exists for is a window that shares a verse's whole vocabulary and
/// none of its order — and with made-up tokens that can be built exactly, at a
/// cosine of 1.0, with no argument about whether the index "should" have scored it.
/// Two verses, no database, no service recording, runs in CI.
#[cfg(test)]
mod paraphrase_bar_wiring {
    use super::*;
    use detection::{HeldReason, PhraseIndex, VerseRef};

    fn vr(book: &str, chapter: i64, verse: i64) -> VerseRef {
        VerseRef {
            book: book.into(),
            chapter,
            verse,
        }
    }

    /// Six invented words per verse. Nothing here parses as a reference, so the only
    /// candidate a window can produce is a paraphrase — which is the whole surface
    /// under test.
    fn corpus() -> Vec<(VerseRef, String)> {
        vec![
            (
                vr("Psalms", 23, 1),
                "alpha bravo charlie delta echo foxtrot".into(),
            ),
            (
                vr("Romans", 8, 28),
                "golf hotel india juliett kilo lima".into(),
            ),
        ]
    }

    struct Fixture {
        sem: Semantic,
        phrases: Phrases,
        context: ContextMemory,
    }

    fn fixture() -> Fixture {
        let c = corpus();
        Fixture {
            sem: Semantic(std::sync::RwLock::new(SemanticIndex::build(&c))),
            phrases: Phrases(std::sync::RwLock::new(PhraseIndex::build(&c))),
            context: ContextMemory::default(),
        }
    }

    fn window(f: &Fixture, text: &str, needs_a_run: bool) -> WindowCandidates {
        candidates_for_window(
            text,
            true,
            &f.sem,
            &f.phrases,
            &f.context,
            None,
            needs_a_run,
        )
    }

    /// The same words, out of order — the shape 111 of the 150 hand-read offers had.
    const SCATTERED: &str = "charlie alpha echo bravo foxtrot delta";
    /// The same words, three of them in the verse's own order.
    const ECHOED: &str = "delta alpha bravo charlie foxtrot echo";

    /// **THE FIXTURE IS NOT VACUOUS.** Both windows must reach the gate as
    /// paraphrases with the switch off, or every assertion below passes over an empty
    /// list — the failure mode `qa.rs` calls a fixture that is not a first launch.
    #[test]
    fn both_windows_are_offered_as_paraphrases_before_anything_is_switched_on() {
        let f = fixture();
        for text in [SCATTERED, ECHOED] {
            let w = window(&f, text, false);
            assert!(
                w.kept
                    .iter()
                    .any(|c| c.method == DetectionMethod::Semantic && c.r == vr("Psalms", 23, 1)),
                "{text:?} produced no paraphrase for Psalms 23:1: {:?}",
                w.kept.iter().map(|c| (&c.r, c.method)).collect::<Vec<_>>()
            );
            assert!(
                w.held.is_empty(),
                "{text:?} held something with the switch off"
            );
        }
    }

    /// **OFF IS A NO-OP, AND THIS IS THE TEST THAT SAYS SO.**
    ///
    /// A church that never opens Settings is offered exactly what it was offered
    /// yesterday: the bar holds nothing, at any run length, including the window it
    /// was built to remove.
    #[test]
    fn a_church_that_never_opens_the_setting_is_offered_what_it_was_offered_yesterday() {
        let f = fixture();
        let w = window(&f, SCATTERED, false);
        assert!(
            w.held
                .iter()
                .all(|(_, why)| *why != HeldReason::NoSharedRun),
            "the bar held a candidate with the switch OFF"
        );
        assert!(w.kept.iter().any(|c| c.method == DetectionMethod::Semantic));
    }

    /// And ON it removes exactly the window with no run, reports it with its reason,
    /// and leaves the one that echoes the verse alone.
    #[test]
    fn on_it_holds_the_paraphrase_that_echoes_nothing_and_keeps_the_one_that_does() {
        let f = fixture();
        let scattered = window(&f, SCATTERED, true);
        assert!(
            !scattered
                .kept
                .iter()
                .any(|c| c.method == DetectionMethod::Semantic),
            "a paraphrase with no shared run reached the gate with the bar ON"
        );
        // REPORTED, NOT DROPPED (rule 35). A switch that quietly stops offering
        // things is indistinguishable from a detector that has gone deaf.
        assert!(
            scattered
                .held
                .iter()
                .any(|(c, why)| *why == HeldReason::NoSharedRun
                    && c.method == DetectionMethod::Semantic),
            "the hold was not reported: {:?}",
            scattered
                .held
                .iter()
                .map(|(c, w)| (&c.r, *w))
                .collect::<Vec<_>>()
        );
        let echoed = window(&f, ECHOED, true);
        assert!(
            echoed
                .kept
                .iter()
                .any(|c| c.method == DetectionMethod::Semantic),
            "a paraphrase sharing {} words in order was held",
            detection::PARAPHRASE_RUN_WORDS
        );
    }

    /// **THE SWITCH CAN REACH NOTHING BUT A PARAPHRASE**, which is the structural
    /// half of "off is a no-op": whatever the setting, the two runs differ only in
    /// `Semantic` candidates, so no reference, quotation, reading or bare verse can
    /// change under it in either direction.
    ///
    /// Asserted over a table rather than one window, because the failure this guards
    /// is a mask applied at the wrong index — which shows up on the SECOND candidate
    /// and not the first.
    #[test]
    fn nothing_but_a_paraphrase_changes_when_the_switch_moves() {
        let f = fixture();
        for text in [
            SCATTERED,
            ECHOED,
            // A real spoken reference beside the scattered paraphrase. Nothing
            // reference-shaped may move.
            "turn with me to Romans chapter eight verse twenty eight charlie alpha echo bravo",
            "psalm twenty three verse one",
            "good morning everybody and welcome",
            "",
        ] {
            let off = window(&f, text, false);
            let on = window(&f, text, true);
            let moved: Vec<(&VerseRef, DetectionMethod)> = off
                .kept
                .iter()
                .filter(|c| !on.kept.iter().any(|k| k.r == c.r && k.method == c.method))
                .map(|c| (&c.r, c.method))
                .collect();
            assert!(
                moved.iter().all(|(_, m)| *m == DetectionMethod::Semantic),
                "{text:?}: the switch moved something that is not a paraphrase: {moved:?}"
            );
            // And it only ever REMOVES. Nothing gains anything from the bar.
            assert!(
                on.kept.len() <= off.kept.len(),
                "{text:?}: the bar added a candidate"
            );
            assert!(
                on.kept
                    .iter()
                    .all(|c| off.kept.iter().any(|k| k.r == c.r && k.method == c.method)),
                "{text:?}: the bar produced a candidate the shipped path did not"
            );
        }
    }
}

#[cfg(test)]
mod a_pause_between_the_chapter_and_the_verse {
    use super::passage_guard_bench::kjv_corpus;
    use super::*;

    /// **THE OTHER HALF OF THE OPERATOR'S REPORT**: *"there might be a pause before
    /// the chapter and verse is called and this leads to failing or not rendering the
    /// verse at all."*
    ///
    /// A pause is not filler — it is the CHUNKER cutting the announcement in two, so
    /// no single window ever holds both the chapter and the verse. RG-315 and RG-318
    /// built the carry for exactly this (`chapter_in_flight` / `note_in_flight`, 15 s
    /// ceiling). This asks whether it reaches the operator's own phrasing, which is
    /// longer than the citations those rows were written from.
    /// The operator's report, as assertions: **a pause between the chapter and the
    /// verse must not lose the reference.** Before this, *"we are in Romans 1 and we
    /// will be reading from"* | *"verse 6 all the way to 8"* produced **nothing at
    /// all** in the second window — literally *"not rendering the verse at all"* — and
    /// where the carry did work the span was dropped, leaving one verse of a
    /// three-verse reading with nothing saying there were two more.
    ///
    /// Still `UncertainBook`, and that is right: the book came from a window this one
    /// cannot see, which is exactly what that method means (rule 40, RG-318). The
    /// operator gets the whole reference, one action away.
    #[test]
    fn a_pause_mid_announcement_keeps_the_reference_and_its_span() {
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        for (w1, w2, want) in [
            (
                "we are in Romans 1 and we will be reading from",
                "verse 6 all the way to 8",
                ("Romans 1:6", Some(8)),
            ),
            (
                "let us turn to Psalm 23 and we will be reading from",
                "verse 1 through to number 6",
                ("Psalms 23:1", Some(6)),
            ),
            (
                "turn with me to Job 22",
                "verse 21 to 25",
                ("Job 22:21", Some(25)),
            ),
        ] {
            let mut context = ContextMemory::default();
            let _ = candidates_for_window(w1, true, &sem, &phrases, &context, None, false);
            context.note_in_flight(detection::chapter_in_flight(w1), 0);
            let after = candidates_for_window(w2, true, &sem, &phrases, &context, None, false);
            let got = after
                .kept
                .iter()
                .find(|c| Fire::key_for(&c.r) == want.0)
                .unwrap_or_else(|| {
                    panic!(
                        "{w1:?} then {w2:?}: the verse he announced reached nobody: {:?}",
                        after
                            .kept
                            .iter()
                            .map(|c| Fire::key_for(&c.r))
                            .collect::<Vec<_>>()
                    )
                });
            assert_eq!(
                got.verse_end, want.1,
                "{w2:?}: the operator was not told how far the reading goes"
            );
            // Never unattended: the book came from a window this one cannot see.
            assert_eq!(got.method.unattended_rank(), 0, "{w2:?} became firable");
        }
    }

    #[test]
    #[ignore]
    fn print_what_survives_a_pause() {
        let corpus = kjv_corpus();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
            &corpus,
        )));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
        for (w1, w2) in [
            (
                "let us turn to Psalm 23 and we will be reading from",
                "verse 1 through to number 6",
            ),
            (
                "we are in Romans 1 and we will be reading from",
                "verse 6 all the way to 8",
            ),
            (
                "open your Bibles to Romans 1",
                "and we will be reading from verse 6 all the way to 8",
            ),
            ("turn with me to Job 22", "verse 21 to 25"),
        ] {
            let mut context = ContextMemory::default();
            // Window one, then the chunker's cut.
            let a = candidates_for_window(w1, true, &sem, &phrases, &context, None, false);
            context.note_in_flight(detection::chapter_in_flight(w1), 0);
            // Window two, 1.2 s later — what the operator sees for the verse called.
            let b = candidates_for_window(w2, true, &sem, &phrases, &context, None, false);
            let show = |w: &WindowCandidates| -> Vec<String> {
                w.kept
                    .iter()
                    .map(|c| {
                        format!(
                            "{}{} {:?}",
                            Fire::key_for(&c.r),
                            c.verse_end.map(|e| format!("-{e}")).unwrap_or_default(),
                            c.method
                        )
                    })
                    .collect()
            };
            println!("  {w1:?}\n      {:?}", show(&a));
            println!("  …{w2:?}\n      {:?}\n", show(&b));
        }
    }
}
