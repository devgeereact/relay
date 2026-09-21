//! The shared QA harness: one fixture, one assertion surface, no drift.
//!
//! ## Why this exists separately from `e2e.rs`
//!
//! `e2e.rs` proves the fire path works. This module exists so that *every other*
//! test — and every audit agent — starts from the same place, and from a place
//! that is honestly a **fresh install**.
//!
//! That distinction is load-bearing. `e2e::app()` does one thing a real first launch
//! does not: it assigns a per-content-type template override, because without one
//! its "every fire carries its template" assertion would be vacuous. That is correct
//! *there* and disqualifying anywhere the question is "can a new operator actually do
//! this?" — a workflow that only completes because of a convenience the installer
//! never performs is exactly the class of defect a cold-start audit is hired to find.
//!
//! So: [`bare_app`] is `db::init_fresh` and nothing else, forever. If you find
//! yourself wanting to add a convenience to it, add it in your own test and say why,
//! the way `e2e::app()` does.
//!
//! ## The assertion surfaces
//!
//! There are **two doors** out of this machine, and a guarantee is only kept on the
//! doors you checked:
//!
//! - [`Wall`] — Tauri events (`output://content`, `output://clear`, `output://black`).
//!   The native output window and the console see these.
//! - [`Kiosk`] — the WebSocket hub. OBS browser sources and the preacher's stage
//!   tablet see this, and **nothing here emits a Tauri event**.
//!
//! Watching only the first is how the rehearsal guarantee was tested, passing, and
//! false for the stage monitor for as long as it existed. `channels::stage_next`
//! publishes to the hub and emits nothing at all. Anything new that publishes to the
//! kiosk needs a [`Kiosk`] in its test.

use super::*;
use std::sync::Arc;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::{Listener, Manager};

/// A headless Relay, exactly as a church receives it on first launch.
///
/// Real in-memory database, real schema, real seed (the full KJV, the built-in
/// templates, the default channels, one active voice profile), real router, real
/// pipeline. Nothing pre-selected, nothing pre-configured, no operator has ever
/// touched it.
///
/// The kiosk hub and the audio engine are deliberately NOT managed: `channels`
/// reaches for them with `try_state`, so their absence is the "no LAN, no mic" case,
/// which is what a headless test is. Attach the hub with [`Kiosk::attach`] when the
/// thing under test publishes to it.
pub(crate) fn bare_app() -> tauri::App<tauri::test::MockRuntime> {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    db::init_fresh(&conn).expect("seed schema + KJV + templates + channels");

    let corpus: Vec<(VerseRef, String)> = db::all_verses(&conn)
        .expect("corpus")
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
        .collect();

    mock_builder()
        .manage(Db(Mutex::new(conn)))
        .manage(Routing::default())
        .manage(Detecting(AtomicBool::new(true)))
        .manage(channels::Rehearsal::default())
        // What the congregation can actually see. `/api/live` reads it, so a test
        // that drives the remote needs it managed or the remote answers "clear".
        .manage(channels::WallState::default())
        // What the operator is looking at, and the timers that outlive it. Both are
        // managed by the real app at startup for the same reason the two below are:
        // without them `adjust_countdown` would answer "nothing is counting down" on
        // a fixture where one demonstrably is, and every timer command would panic
        // rather than fail a test with a readable message.
        .manage(channels::LiveContent::default())
        .manage(crate::timers::TimerRegistry::default())
        .manage(Session::default())
        // Whether the screens are answering, and whether a recorded service is
        // being protected. Both are managed by the real app at startup, so a
        // fixture without them is not a fresh install — it is an app in a state no
        // church could ever be in, and a command that reads either would panic
        // rather than fail a test with a readable message.
        .manage(channels::OutputHealth::default())
        // AND WHICH SCREENS THE OPERATOR HAS TAKEN OUT OF THE WALL. Same argument
        // as the two above and the same shape of failure: a fresh install manages
        // it, so a fixture without it is an app in a state no church could be in,
        // and `clear_screen` would refuse on a machine where it must work.
        // WHAT THE CLIP ON THE SCREENS IS DOING. A fresh install has no clip and
        // no transport, and a fixture missing this is not a fresh install — it is
        // an app in a state no church could be in, where the one content door
        // panics instead of firing.
        .manage(channels::MediaTransport::default())
        .manage(channels::ScreensDown::default())
        .manage(servicelock::ServiceLock::default())
        // AND THE PHRASE INDEX. A fresh install builds both from the same
        // corpus in `setup`, so a fixture with one and not the other is an app in
        // a state no church could be in — and `emit_detections` takes
        // `state::<Phrases>()` on every window, which PANICS rather than failing
        // with a readable message. Ten e2e tests found that within a minute of
        // the index landing, which is the fixture doing its job.
        .manage(Phrases(detection::PhraseIndex::build(&corpus)))
        .manage(Semantic(SemanticIndex::build(&corpus)))
        .manage(Context(Mutex::new(ContextMemory::default())))
        // mock_context, NOT generate_context!(): the real macro embeds Info.plist as a
        // link symbol, and expanding it a second time fails with
        // "symbol `_EMBED_INFO_PLIST` is already defined".
        .build(mock_context(noop_assets()))
        .expect("mock app")
}

/// Records everything that leaves the machine through the **Tauri event** door.
///
/// The assertion surface that matters: not "did the function return Ok", but "what
/// did the congregation actually see".
#[derive(Default)]
pub(crate) struct Wall {
    content: Arc<Mutex<Vec<serde_json::Value>>>,
    cleared: Arc<AtomicBool>,
    blacked: Arc<AtomicBool>,
}

impl Wall {
    pub(crate) fn watch(app: &tauri::AppHandle<tauri::test::MockRuntime>) -> Self {
        let w = Wall::default();
        let c = w.content.clone();
        // OutputContent is Serialize-only (it is never read back in production), so
        // the wall records the JSON that actually went out — which is, if anything,
        // the more honest thing to assert on: it is the bytes the outputs receive.
        app.listen("output://content", move |e| {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(e.payload()) {
                c.lock().unwrap().push(v);
            }
        });
        let cl = w.cleared.clone();
        app.listen("output://clear", move |_| cl.store(true, Ordering::SeqCst));
        let bl = w.blacked.clone();
        app.listen("output://black", move |_| bl.store(true, Ordering::SeqCst));
        w
    }

    pub(crate) fn last(&self) -> Option<serde_json::Value> {
        self.content.lock().unwrap().last().cloned()
    }

    pub(crate) fn count(&self) -> usize {
        self.content.lock().unwrap().len()
    }

    /// Every reference that reached an output, in order. `last()` answers "what is
    /// up now"; this answers "what did the congregation actually watch happen",
    /// which is the question a flickering wall raises.
    pub(crate) fn references(&self) -> Vec<String> {
        self.content
            .lock()
            .unwrap()
            .iter()
            .filter_map(|v| v.get("reference")?.as_str().map(str::to_string))
            .collect()
    }

    pub(crate) fn cleared(&self) -> bool {
        self.cleared.load(Ordering::SeqCst)
    }

    pub(crate) fn blacked(&self) -> bool {
        self.blacked.load(Ordering::SeqCst)
    }
}

/// Records everything that leaves through the **kiosk WebSocket** door.
///
/// OBS browser sources, LAN displays and the preacher's stage tablet all live behind
/// this one, and it emits no Tauri event — so a [`Wall`] is blind to it. Attaching
/// the hub is part of watching it: a headless app manages no hub, and a publisher
/// with no hub is a silent no-op that will make a test pass for the wrong reason.
pub(crate) struct Kiosk {
    rx: tokio::sync::broadcast::Receiver<String>,
}

impl Kiosk {
    /// Manage a hub on this app (if one is not already there) and subscribe to it.
    pub(crate) fn attach(app: &tauri::AppHandle<tauri::test::MockRuntime>) -> Self {
        app.manage(channels::KioskHub::default());
        Kiosk {
            rx: app.state::<channels::KioskHub>().sender().subscribe(),
        }
    }

    /// The next message the kiosk received, or `None` if it received nothing.
    pub(crate) fn next(&mut self) -> Option<String> {
        self.rx.try_recv().ok()
    }

    /// True when nothing at all reached the kiosk. The rehearsal assertion.
    pub(crate) fn silent(&mut self) -> bool {
        self.rx.try_recv().is_err()
    }

    /// EVERYTHING waiting, in order, leaving the receiver empty.
    ///
    /// `next()` answers "what came next", which is the right question when one
    /// frame is the claim. A configuration change publishes several frames and a
    /// fire publishes several more, so a test that wants the LAST frame of a kind
    /// — or wants to assert that no frame of a kind is present — has to look at
    /// all of them, and hand-rolling that loop per test is how two tests come to
    /// disagree about what "drained" means.
    pub(crate) fn drain(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(m) = self.rx.try_recv() {
            out.push(m);
        }
        out
    }
}

/// Tauri delivers events to listeners asynchronously on the mock runtime; give the
/// loop a moment to drain before asserting on what the wall received.
pub(crate) fn settle() {
    std::thread::sleep(std::time::Duration::from_millis(60));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture's whole promise, asserted rather than commented.
    ///
    /// If a convenience is ever added to `bare_app` — a selected template, a created
    /// plan, an opened output — every cold-start claim built on it becomes a claim
    /// about a system somebody had already set up. This is the tripwire.
    #[test]
    fn the_bare_fixture_is_a_first_launch_and_nothing_more() {
        let app = bare_app();
        let db = app.state::<Db>();
        let conn = db.0.lock().unwrap();

        // Seeded, because a church receives these: the verses, the built-in looks,
        // the default screens.
        assert!(
            db::verse_count(&conn).unwrap() > 31_000,
            "a fresh install ships the full KJV"
        );
        // THE SHELF, BY IDENTITY RATHER THAN BY NOT BEING EMPTY. The forty are
        // eight roles of five, and a fresh install that lost a whole role would
        // still have passed a non-emptiness check — which is what this line was.
        let templates = db::list_templates(&conn).unwrap();
        assert_eq!(templates.len(), 40, "a fresh install ships the shelf");
        for t in &templates {
            assert!(
                t.layout["layers"].is_array(),
                "{}: a fresh install is seeding a region-model row again (RG-140, RG-141)",
                t.name
            );
        }

        // Song is the ONE content-look a fresh install ships with a default, and it
        // is deliberate: it is the one role whose words have no reference at all, so
        // a lyric rendered through a scripture look put the song title where the
        // words should be. `templates.rs::ensure_lyrics_template` chooses the row
        // and writes `tpl_song`; it used to CREATE a region-model row, and the seed
        // it named here no longer does either.
        assert!(
            db::content_template_id(&conn, "song").unwrap().is_some(),
            "the lyrics content-look is seeded on purpose and has gone missing"
        );

        // Every other kind is the OPERATOR'S choice and is unset until they make it.
        // `e2e::app()` sets `scripture` on purpose and says why; that is exactly the
        // convenience this fixture must never acquire.
        for kind in ["scripture", "media", "announcement", "countdown"] {
            assert_eq!(
                db::content_template_id(&conn, kind).unwrap(),
                None,
                "a fresh install has no content-look chosen for {kind} — \
                 something added a convenience to the bare fixture"
            );
        }

        // AND THE CONTENT IN IT IS THE STARTER SET, EXACTLY.
        //
        // This block used to assert ZERO for all five tables. That was the right
        // assertion while `init_fresh` was forbidden to seed content, and it
        // caught the failure it was written for: wiring `db::demo::load` into
        // `init_fresh` was watched to leave the earlier version of this test
        // green, because everything above asserts what a first launch CONTAINS
        // and nothing asserted what it must not.
        //
        // DECISIONS §90 reverses that rule, and this test keeps its name and its
        // job by changing what it asserts rather than by being softened. The
        // value here was never the zero; it was that a seed which drifts fails
        // loudly. So the starter set is written out BY IDENTITY — these titles,
        // this many pictures, this plan with this shape — and the literals below
        // are the frozen copy. A seed that adds a sixth notice, renames one,
        // drops the countdown's words or grows a song list turns this red, which
        // is the whole point of it. It is NOT weakened to a range, and it must
        // never be.
        //
        // The demo ledger is asserted separately and still says "nothing". It is
        // the one fact that means "something seeded content it intends to own and
        // be able to take back", and starter content deliberately is not that.
        assert!(
            !db::demo::is_loaded(&conn).unwrap(),
            "a fresh install has no demo content — something taught Relay to seed itself"
        );

        // The tables the starter set does not touch. Songs and saved verses are
        // still zero: a church's songs are its own, and a saved verse is a thing
        // an operator chose to keep.
        for (what, sql) in [
            ("a song", "SELECT COUNT(*) FROM songs"),
            ("a saved verse", "SELECT COUNT(*) FROM saved_scripture"),
        ] {
            let n: i64 = conn.query_row(sql, [], |r| r.get(0)).unwrap();
            assert_eq!(
                n, 0,
                "the starter set does not write these, and this install has {what}"
            );
        }

        // Five announcements, by title. The operator's title and the room's
        // words are separate fields (REBRAND §10) and both are asserted, because
        // a starter notice with an empty body is a slide with a heading and
        // nothing under it.
        let notices = db::list_announcements(&conn).unwrap();
        let mut titles: Vec<&str> = notices.iter().map(|a| a.title.as_str()).collect();
        titles.sort_unstable();
        assert_eq!(
            titles,
            vec![
                "After the service",
                "Children's groups",
                "Giving",
                "Prayer meeting",
                "Welcome",
            ],
            "the starter announcements drifted"
        );
        for a in &notices {
            assert!(
                !a.body.trim().is_empty(),
                "{}: a starter notice with no words for the room",
                a.title
            );
        }

        // One row per picture Relay ships, each pointing into the bundle rather
        // than at a file on disk. The count is read from the bundle rather than
        // written down here on purpose: the identity being asserted is "every
        // bundled picture, and nothing else", and a literal would go stale the
        // first time somebody curated the folder while this test stayed green
        // over a library missing one.
        let pictures = db::list_media(&conn).unwrap();
        let bundled = channels::bundled_backgrounds();
        assert!(
            !bundled.is_empty(),
            "the bundle holds no pictures — was `npm run build` run before this \
             crate was compiled? (RG-127)"
        );
        assert_eq!(
            pictures.len(),
            bundled.len(),
            "a picture in the bundle with no library row, or a row with no picture"
        );
        for m in &pictures {
            assert!(
                m.path.starts_with(db::BUNDLED_PREFIX),
                "{}: a fresh install has a media row that is not a bundled picture",
                m.filename
            );
        }

        // ONE example plan, with the countdown that carries its own words. That
        // cue is the reason the plan is here at all: it is the first thing that
        // exercises the Planner fields wave 5 gave a countdown, and a plan whose
        // countdown lost them would still have passed a count.
        let plans = db::list_plans(&conn).unwrap();
        assert_eq!(plans.len(), 1, "one starter plan and no more");
        assert_eq!(plans[0].title, "Sunday Morning (example)");
        let items = db::plan_items(&conn, plans[0].id).unwrap();
        let kinds: Vec<&str> = items.iter().map(|i| i.cue_type.as_str()).collect();
        assert_eq!(kinds, vec!["countdown", "announce", "scripture"]);
        let countdown: serde_json::Value = serde_json::from_str(&items[0].payload_json).unwrap();
        assert_eq!(countdown["minutes"], 10);
        assert_eq!(countdown["label"], "Service begins in");
        assert_eq!(countdown["done"], "Welcome");
        assert_eq!(items[0].duration_sec, 600);
        assert!(
            items[0].template_id.is_some(),
            "the countdown cue lost the Timer look that can draw its words"
        );
    }

    /// The stage-monitor door exists and is watchable. Guards the harness itself:
    /// if `Kiosk::attach` ever stopped receiving, every rehearsal-containment test
    /// built on it would pass by seeing nothing, which is the assertion.
    #[test]
    fn the_kiosk_door_is_watchable_and_is_not_the_wall() {
        let app = bare_app();
        let h = app.handle().clone();
        let wall = Wall::watch(&h);
        let mut kiosk = Kiosk::attach(&h);

        set_stage_next(h.clone(), Some("Up next".into()), Some("John 3:16".into()));
        settle();

        let msg = kiosk
            .next()
            .expect("the stage monitor is a real door and it just received something");
        assert!(msg.contains("stage_next") && msg.contains("John 3:16"));
        assert_eq!(
            wall.count(),
            0,
            "stage_next emits NO Tauri event — a Wall is blind to it, which is why \
             Kiosk exists. If this ever fails, the two doors have merged and the \
             harness comment above is now a lie."
        );
    }
}

/// ── R1 · COLD START ──────────────────────────────────────────────────────────
///
/// The audit question these answer is not "does the code work" but "can a church
/// that has just installed Relay, and has done nothing else, actually GET here?"
///
/// Every test in this module starts from [`bare_app`] — `db::init_fresh` and
/// nothing else. None of them may add a convenience without saying, in the test,
/// why (the way `e2e::app()` does). Where a test drives a `db::` function instead
/// of the `#[tauri::command]`, it says so, because that is a weaker claim: it
/// proves the row can be written, not that a control can write it.
///
/// Several tests here are **characterisation tests**: they assert a gap that
/// exists today so that closing it is a loud, deliberate event rather than a
/// silent one. Each is labelled `GAP:` and says what to do when it fails.
#[cfg(test)]
mod cold_start {
    use super::*;
    use rusqlite::Connection;

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap_or_else(|e| panic!("counting {table}: {e}"))
    }

    // ── 1. The one workflow the whole product is for ─────────────────────────

    /// A brand-new install, nothing configured, puts a verse on the wall.
    ///
    /// This is `e2e::a_verse_the_operator_fires_reaches_the_congregation_with_its_text`
    /// with the convenience removed. It is also the honest version of the claim:
    /// on a genuine first launch `template_id` is **null**, because no
    /// content-look has been chosen for `scripture` and one is not seeded. The
    /// verse still renders — each screen resolves its own template (DECISIONS
    /// §29) and the seeded channels each carry one — but any assertion of the
    /// form "every fire carries its template" is only true once an operator has
    /// been to the Templates screen.
    #[test]
    fn a_first_launch_puts_a_verse_on_the_wall_with_no_setup_at_all() {
        let app = bare_app();
        let h = app.handle().clone();
        let wall = Wall::watch(&h);

        manual_fire(
            h.clone(),
            h.state::<Db>(),
            "John 3:16".into(),
            None,
            None,
            None,
        )
        .expect("a fresh install must be able to fire a verse");
        settle();

        let shown = wall.last().expect("nothing reached the outputs");
        assert_eq!(shown["reference"], "John 3:16");
        assert!(shown["text"]
            .as_str()
            .unwrap_or("")
            .contains("God so loved"));

        assert!(
            shown["template_id"].is_null(),
            "a fresh install has no scripture content-look, so a fire carries no \
             template id — if this now fails, something SEEDS one, and \
             `the_bare_fixture_is_a_first_launch_and_nothing_more` should have \
             caught it first"
        );

        // ...and the screens it lands on do each have a template of their own,
        // which is the reason the null above is safe rather than a blank wall.
        let db = h.state::<Db>();
        let conn = db.0.lock().unwrap();
        let chans = db::list_output_channels(&conn).unwrap();
        assert!(!chans.is_empty(), "a fresh install seeds output channels");
        assert!(
            chans.iter().all(|c| c.template_id.is_some()),
            "a seeded screen with no template would render nothing on first launch"
        );
    }

    // ── 2. The create-path matrix, executed ──────────────────────────────────

    /// Every table a rendered control can fill, filled by driving the real
    /// commands from an empty system.
    ///
    /// One test rather than fifteen on purpose: the claim is about the *system*
    /// reaching a fully-populated state from nothing, and a per-table test would
    /// let a later change break the ORDER (a plan cue needs a plan; a song
    /// section needs a song) without anything noticing.
    #[test]
    fn every_table_a_control_can_reach_is_filled_by_driving_the_real_commands() {
        let app = bare_app();
        let h = app.handle().clone();
        let db = h.state::<Db>();

        let seeded_templates = {
            let conn = db.0.lock().unwrap();
            db::list_templates(&conn).unwrap().len() as i64
        };

        // --- output_channels: Channels → Add screen
        let ch = add_channel(
            h.clone(),
            h.state::<Db>(),
            "Crèche TV".into(),
            Some("network_client".into()),
            None,
        )
        .expect("add_channel");

        // --- service_plans + plan_items: Planner → New plan → add cue
        let plan = create_plan(
            h.state::<Db>(),
            "Sunday Morning".into(),
            "2026-08-16".into(),
        )
        .expect("create_plan");
        let cue = add_plan_item(
            h.state::<Db>(),
            plan,
            "scripture".into(),
            "John 3:16".into(),
            r#"{"reference":"John 3:16"}"#.into(),
            None,
        )
        .expect("add_plan_item");

        // --- songs + song_sections: Library → Import → review → Save
        //     NOTE: this is the ONLY create path a rendered control reaches, and
        //     it requires a FILE. See `the_paste_or_draft_song_menu_item_is_dead`.
        let saved = save_reviewed_songs(
            h.state::<Db>(),
            h.state::<servicelock::ServiceLock>(),
            vec![SaveSong {
                title: "Great Are You Lord".into(),
                author: "All Sons & Daughters".into(),
                ccli: "".into(),
                song_key: "G".into(),
                bpm: None,
                sections: vec![
                    songs::ParsedSection {
                        tag: "V1".into(),
                        label: "Verse 1".into(),
                        lyrics: "You give life, You are love".into(),
                    },
                    songs::ParsedSection {
                        tag: "C".into(),
                        label: "Chorus".into(),
                        lyrics: "It's Your breath in our lungs".into(),
                    },
                ],
            }],
            "2026-08-16".into(),
        )
        .expect("save_reviewed_songs");
        assert_eq!(saved.added.len(), 1);

        // --- saved_scripture: Library → Browse/Scripture → Save
        save_scripture(h.state::<Db>(), "Psalms".into(), 23, 1, "2026-08-16".into())
            .expect("save_scripture");

        // --- announcements: Library → New Item → Draft announcement
        save_announcement(
            h.state::<Db>(),
            None,
            "Midweek".into(),
            "Wednesday 7pm".into(),
            "2026-08-16".into(),
        )
        .expect("save_announcement");

        // --- voice_profiles: Settings → Voice profiles → New
        create_voice_profile(h.state::<Db>(), "Pastor Ade".into(), Some("yo".into()))
            .expect("create_voice_profile");

        // --- app_settings: Settings → Bible translations (and every other pref)
        set_active_translation(h.state::<Db>(), h.state::<servicelock::ServiceLock>(), 1)
            .expect("set_active_translation");

        // --- services: starting to listen starts recording (capture.js
        //     `startCapture` calls `start_service` before `start_capture`).
        let svc = start_service(
            h.clone(),
            h.state::<Session>(),
            h.state::<Db>(),
            h.state::<channels::Rehearsal>(),
            h.state::<servicelock::ServiceLock>(),
            "Sunday Service".into(),
            "2026-08-16".into(),
        )
        .expect("start_service");
        assert!(svc > 0);

        // --- cues + transcripts + detections: firing during a live service.
        //     `fire_content` writes a cue; `manual_fire` writes a detection, and
        //     `persist_fire` synthesises the transcript row it hangs off.
        fire_content(
            h.clone(),
            h.state::<Db>(),
            "Notices".into(),
            "Bring a friend".into(),
            "announcement".into(),
            None,
            None,
            None,
        )
        .expect("fire_content");
        manual_fire(
            h.clone(),
            h.state::<Db>(),
            "Psalm 23".into(),
            None,
            None,
            None,
        )
        .expect("manual_fire during a service");
        settle();

        // --- templates: the Templates editor saves through `save_template`,
        //     which takes a CONCRETE `tauri::AppHandle` and so cannot be driven
        //     on the mock runtime. Driven one layer down; the wiring from the
        //     editor is layer B/C, not this test's claim.
        {
            let conn = db.0.lock().unwrap();
            db::upsert_template(
                &conn,
                &db::Template {
                    id: 0,
                    name: "Our church look".into(),
                    layout: serde_json::json!({"regions":["verse_text"]}),
                    style: serde_json::json!({"font":"var(--f-serif)"}),
                    active: false,
                },
            )
            .expect("upsert_template");
        }

        let conn = db.0.lock().unwrap();
        for (table, at_least) in [
            ("templates", seeded_templates + 1),
            ("output_channels", 5), // 4 seeded + the one just added
            ("service_plans", 1),
            ("plan_items", 1),
            ("songs", 1),
            ("song_sections", 2),
            ("saved_scripture", 1),
            ("announcements", 1),
            ("services", 1),
            ("transcripts", 1),
            ("detections", 1),
            ("cues", 1),
            ("app_settings", 1),
            ("voice_profiles", 1),
        ] {
            assert!(
                count(&conn, table) >= at_least,
                "{table}: a control-driven create path did not produce a row \
                 (expected at least {at_least}, found {})",
                count(&conn, table)
            );
        }

        // The human's fire must be logged as the human's — the router learns from
        // this column, and a manual fire recorded as 'auto' poisons calibration.
        let manual: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM detections WHERE status = 'manual'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            manual, 1,
            "the operator's own fire was not recorded as theirs"
        );

        // Sanity: the ids handed back are real rows, not zeros.
        assert!(ch > 0 && plan > 0 && cue > 0);
    }

    /// GAP, now one table rather than two: **`translations`**.
    ///
    /// This is the cold-start mandate asserted rather than argued — what a fresh
    /// install cannot be brought to contain by any control an operator can reach.
    ///
    /// `song_arrangements` **left this test on 2026-08-30**: the arrangement editor
    /// shipped (RG-21), and `a_component_can_create_a_song_arrangement` now pins the
    /// create path from both ends. What remains here about it is the fresh-install
    /// claim — a new install still seeds none, which is correct and is not a gap.
    ///
    /// `translations` is still real: there is no `add_translation` command at all,
    /// and Settings says so plainly. A church wanting a version other than KJV
    /// cannot get one, and no amount of UI would help — the corpus has to arrive.
    ///
    /// When the translation half fails, an importer shipped: move the matrix row.
    #[test]
    fn a_fresh_install_still_cannot_be_given_a_second_translation() {
        let app = bare_app();
        let h = app.handle().clone();
        let db = h.state::<Db>();
        let conn = db.0.lock().unwrap();

        assert_eq!(
            count(&conn, "song_arrangements"),
            0,
            "a fresh install seeds no arrangements"
        );
        assert_eq!(
            count(&conn, "translations"),
            1,
            "a fresh install ships exactly one translation (KJV)"
        );

        // The TABLE was never the problem, for either of them. For arrangements the
        // break used to be one level up — a wrapper no component imported — and
        // that is closed. For translations the break is at the command layer: there
        // is no `add_translation` at all, and behind it the verse corpus for a
        // second version does not exist to import.
        drop(conn);
        let conn = db.0.lock().unwrap();
        conn.execute_batch(
            "INSERT INTO songs (title) VALUES ('x');
             INSERT INTO song_arrangements (song_id, name, sequence)
                 VALUES (1, 'Live', '[0,1]');",
        )
        .expect("the TABLE is fine; the path to it is what is missing");
        assert_eq!(count(&conn, "song_arrangements"), 1);

        let translation_commands = std::fs::read_to_string("src/main.rs").unwrap();
        assert!(
            !translation_commands.contains("fn add_translation")
                && !translation_commands.contains("fn import_translation"),
            "a translation importer now exists — the Settings note 'Additional \
             versions need their verse data added to the corpus' is no longer the \
             end of the road, so update the matrix"
        );
    }

    // ── 3. Static contract: the link that breaks is in the frontend ──────────

    /// Read every `.svelte` under `src/`, once.
    fn components() -> Vec<(String, String)> {
        fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "svelte") {
                    if let Ok(s) = std::fs::read_to_string(&p) {
                        out.push((p.display().to_string(), s));
                    }
                }
            }
        }
        let mut out = Vec::new();
        walk(std::path::Path::new("../src"), &mut out);
        assert!(
            out.len() > 40,
            "the component scan found only {} files — the path is wrong and every \
             assertion built on it is vacuous",
            out.len()
        );
        out
    }

    /// CLOSED (was a GAP: F3, independently verified). `save_arrangement` was
    /// registered, `saveArrangement` existed in the store — and **no component
    /// imported it**, so the one thing the whole chain was for could not be done.
    ///
    /// The read half was wired the whole time: `ServicePlanner.svelte` calls
    /// `listArrangements` and opens a picker when the list is non-empty. Since the
    /// list could never be non-empty, that picker was unreachable code and the
    /// branch that ran was always "Standard".
    ///
    /// That is why "every registered command has a frontend caller" is true and
    /// not sufficient: the caller was a wrapper nothing called. This test now pins
    /// the closure from BOTH ends, so the editor cannot be deleted or orphaned
    /// without something going red.
    #[test]
    fn a_component_can_create_a_song_arrangement() {
        let comps = components();
        let writers: Vec<&str> = comps
            .iter()
            .filter(|(_, s)| s.contains("saveArrangement"))
            .map(|(p, _)| p.as_str())
            .collect();
        assert!(
            !writers.is_empty(),
            "nothing can write an arrangement again — the picker in the Planner is \
             unreachable code and `save_arrangement` is a dead command"
        );

        // A component that writes but that nothing renders is the same gap one
        // level along — the exact failure `qa-inventory.mjs` exists to catch.
        let rendered: Vec<&str> = writers
            .iter()
            .filter(|w| {
                let name = std::path::Path::new(w)
                    .file_stem()
                    .unwrap()
                    .to_string_lossy();
                comps
                    .iter()
                    .any(|(p, s)| p != *w && s.contains(&format!("<{name}")))
            })
            .copied()
            .collect();
        assert!(
            !rendered.is_empty(),
            "an arrangement writer exists in {writers:?} but no component renders \
             it — a control nobody can reach is not a create path"
        );

        // The reader too, or the picker below is gone and this needs rewriting.
        assert!(comps.iter().any(|(_, s)| s.contains("listArrangements")));

        let store = std::fs::read_to_string("../src/lib/stores/capture.js").unwrap();
        assert!(store.contains("export async function saveArrangement"));
        assert!(std::fs::read_to_string("src/main.rs")
            .unwrap()
            .contains("fn save_arrangement"));
    }

    /// CLOSED — all three entries in Library → **New Item** now do something.
    ///
    /// `newPasteSong()` set `lyricAction` and `newSaveScripture()` set
    /// `scriptureAction`, and neither variable was ever passed to the pane it was
    /// meant to drive — `<LyricsPane>` and `<Scripture>` declared no such prop. The
    /// third, "Draft announcement", IS wired, which is exactly what made the other
    /// two look correct at a glance.
    ///
    /// It mattered most for songs: **"Paste / draft song" is the only create path
    /// for `songs` that does not require a FILE**, so a church whose lyrics live on
    /// a website had no way in at all.
    ///
    /// Both are wired to the thing that actually does the work, rather than to a
    /// flag: pasting opens a sheet and hands the text to the SAME `parse_import`
    /// review a file goes through, and "Save scripture" puts the cursor in the
    /// search box on the Saved tab, because saving a verse happens by starring a
    /// result and inventing a second editor would be a second create path for a
    /// table that already has a good one.
    #[test]
    fn all_three_new_item_menu_entries_do_something() {
        let lib = std::fs::read_to_string("../src/lib/views/Library.svelte").unwrap();
        let lyrics = std::fs::read_to_string("../src/lib/views/library/LyricsPane.svelte").unwrap();
        let scripture =
            std::fs::read_to_string("../src/lib/views/library/Scripture.svelte").unwrap();
        let announcements =
            std::fs::read_to_string("../src/lib/views/library/Announcements.svelte").unwrap();

        // The menu items exist and are rendered.
        for label in ["Paste / draft song", "Save scripture", "Draft announcement"] {
            assert!(lib.contains(label), "the New Item menu lost {label:?}");
        }

        // The one that works: the flag reaches the pane, and the pane declares it.
        assert!(lib.contains("startDraft={announceAction}"));
        assert!(announcements.contains("export let startDraft"));

        // The dead flags are GONE — not merely unread, deleted. A variable that is
        // assigned and never read is the shape this defect had, and leaving one
        // behind would let it come back looking wired.
        //
        // Match the ASSIGNMENT, not the word. The file explains in prose why these
        // flags are gone, and a bare-word grep fails on the explanation itself —
        // the same trap the Announcements assertion below already records.
        assert!(
            !lib.contains("lyricAction =") && !lib.contains("scriptureAction ="),
            "the dead flags are back — they were assigned and read by nothing, \
             which is exactly how two menu entries came to do nothing"
        );

        // Pasting opens a sheet and goes through the SAME review a file import
        // does. A second, shorter path to `songs` is how two ways of creating one
        // start disagreeing about what a section is.
        assert!(lib.contains("function commitPaste"), "no paste handler");
        assert!(
            lib.contains("parseImport(") && lib.contains("reviewing = true"),
            "pasting must land in the same review as a file import"
        );
        assert!(
            lib.contains("aria-label=\"Paste a song\"") && lib.contains("use:trapFocus"),
            "the paste sheet is a real modal and must trap focus like every other"
        );

        // …and "Save scripture" puts the cursor where the work happens.
        assert!(
            lib.contains("searchEl?.focus()") && lib.contains("bind:this={searchEl}"),
            "Save scripture must land the operator in the search box"
        );

        // Neither pane grew a prop for this: the wiring is in the Library, which is
        // where the menu lives.
        let _ = (&lyrics, &scripture);
    }

    /// CLOSED — there is no command a rendered component cannot reach.
    ///
    /// Five were addressed only by a `capture.js` wrapper that nothing imported:
    /// `create_template`, `import_song`, `import_pro`, `list_output_windows` and
    /// `open_output_window`. "Every registered command has a frontend caller" was
    /// therefore true only at the WRAPPER level — the level `ipc.test.js` checks,
    /// and not the level that matters.
    ///
    /// **All five were deleted on 2026-08-30**, with their wrappers, rather than
    /// given a UI: each was superseded by a better path that every control already
    /// used (`upsert_template`; `parse_import` → `save_reviewed_songs`; the
    /// channel-keyed output API). The precedent is the five deleted before them —
    /// `lookup_verse`, `close_output_window`, `current_service` and the
    /// `*_template_active` pair.
    ///
    /// It is a security reduction as much as a tidy-up: every registered command is
    /// invokable from the webview, and `open_output_window` opened an arbitrary
    /// fullscreen window on any monitor. A command nothing calls is attack surface
    /// nobody is watching.
    #[test]
    fn no_registered_command_is_unreachable_from_a_rendered_component() {
        let main = std::fs::read_to_string("src/main.rs").unwrap();
        for gone in [
            "fn create_template",
            "fn import_song",
            "fn import_pro",
            "fn list_output_windows",
            "fn open_output_window",
        ] {
            assert!(
                !main.contains(gone),
                "{gone} is back — if it has a UI now, say so here; if it does not, it \
                 is attack surface nobody is watching"
            );
        }

        let store = std::fs::read_to_string("../src/lib/stores/capture.js").unwrap();
        for gone in [
            "function createTemplate",
            "function importSong",
            "function importProFile",
            "function listOutputWindows",
            "function openOutput",
        ] {
            assert!(
                !store.contains(gone),
                "the wrapper for {gone} outlived its command"
            );
        }
    }

    // ── 4. Persistence across a genuine reopen ───────────────────────────────

    /// Not a re-query: a real file, a closed connection, a second `migrate`.
    ///
    /// The bug class this catches is the one `run_migrations` already has a scar
    /// from — a change that is present in `schema.sql`, present in every
    /// in-memory test, and absent from the one database that matters, because the
    /// reopen path takes a different branch from the create path.
    #[test]
    fn everything_an_operator_builds_survives_closing_and_reopening_the_file() {
        let dir = std::env::temp_dir().join(format!(
            "relay-qa-reopen-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.db");
        let _ = std::fs::remove_file(&path);

        // FIRST LAUNCH — exactly what `db::open()` does for a file that is absent.
        let plan;
        // What the first launch ships on its own (DECISIONS §90), so what the
        // operator then builds is counted on top of it rather than instead of it.
        let starter_plans;
        let starter_notices;
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
            db::migrate(&conn, true).expect("first launch");
            starter_plans = db::list_plans(&conn).unwrap().len();
            starter_notices = db::list_announcements(&conn).unwrap().len();
            plan = db::create_plan(&conn, "Sunday Morning", "2026-08-16").unwrap();
            db::add_plan_item(&conn, plan, "scripture", "John 3:16", "{}", None).unwrap();
            db::save_announcement(&conn, None, "Midweek", "Wed 7pm", "2026-08-16").unwrap();
            db::create_voice_profile(&conn, "Pastor Ade", Some("yo")).unwrap();
            db::set_setting(&conn, "active_translation", "1").unwrap();
        } // connection dropped — the file is all that is left

        // SECOND LAUNCH — the `fresh = false` branch, which is the one every
        // church takes on every day after the first.
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
            db::migrate(&conn, false).expect("reopen must not fail");

            assert_eq!(db::list_plans(&conn).unwrap().len(), starter_plans + 1);
            assert_eq!(db::plan_items(&conn, plan).unwrap()[0].label, "John 3:16");
            assert_eq!(
                db::list_announcements(&conn).unwrap().len(),
                starter_notices + 1
            );
            assert!(db::active_voice_profile(&conn).unwrap().is_some());
            assert_eq!(
                db::get_setting(&conn, "active_translation")
                    .unwrap()
                    .as_deref(),
                Some("1")
            );
            // The seed is content, and it is still there.
            assert!(db::verse_count(&conn).unwrap() > 31_000);
            assert_eq!(
                count(&conn, "translations"),
                1,
                "the reopen re-seeded translations — a duplicate KJV"
            );
            assert!(!db::list_templates(&conn).unwrap().is_empty());
            assert_eq!(count(&conn, "output_channels"), 4);
            // The migration did not leave the §25 fingerprint behind.
            let (applied, scratch) = db::manual_status_report(&conn).unwrap();
            assert!(applied && !scratch);
        }

        // THIRD LAUNCH — reopening repeatedly must not accumulate anything. The
        // v0 sniff-based forward-fills used to re-run on every boot forever.
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
            db::migrate(&conn, false).expect("third launch");
            assert_eq!(count(&conn, "output_channels"), 4);
            assert_eq!(count(&conn, "translations"), 1);
            assert_eq!(db::list_plans(&conn).unwrap().len(), starter_plans + 1);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Awkward text, written through the real create paths, read back after a
    /// genuine reopen.
    ///
    /// Tier-1 languages are the product's differentiator, so a diacritic that
    /// does not survive a round trip is not a curiosity — it is the plan a
    /// Yoruba-speaking church typed on Tuesday coming back wrong on Sunday.
    #[test]
    fn unicode_diacritics_quotes_and_emoji_survive_a_reopen() {
        let dir = std::env::temp_dir().join(format!(
            "relay-qa-awkward-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("relay.db");
        let _ = std::fs::remove_file(&path);

        let long = "Ẹ".repeat(20_000);
        let cases: Vec<(&str, String)> = vec![
            ("yoruba", "Ọlọ́run Olódùmarè — Ẹ kú ìròlẹ́".into()),
            ("swahili", "Mungu ni Mwokozi wangu · Yesu ni Bwana".into()),
            ("hausa", "Allah Maɗaukaki — Sunkuɗe ƙarfi".into()),
            ("quotes", r#"He said "I AM" — it's 'done'"#.into()),
            ("emoji", "Welcome 🙏🏾🎺 — service at 9".into()),
            ("sqlish", "Robert'); DROP TABLE plan_items;--".into()),
            ("markup", "<script>alert(1)</script> & <b>bold</b>".into()),
            ("rtl", "\u{202E}gnihtemos\u{202C}".into()),
            ("very_long", long.clone()),
            ("whitespace_only", "   ".into()),
        ];

        let mut ids = Vec::new();
        // The plan this test writes, and what a first launch already holds. Both
        // used to be implicit — the plan was read back as id 1 and the cues were
        // counted over the whole table — which was exact while a fresh install
        // had no plan of its own. DECISIONS §90 gives it one, and id 1 is now
        // that one.
        let plan;
        let starter_cues;
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
            db::migrate(&conn, true).unwrap();
            starter_cues = count(&conn, "plan_items") as usize;
            plan = db::create_plan(&conn, "Ìsìn Ọjọ́ Àìkú", "2026-08-16").unwrap();
            for (name, text) in &cases {
                let id = db::add_plan_item(&conn, plan, "announce", text, "{}", None)
                    .unwrap_or_else(|e| panic!("{name} could not be written: {e}"));
                ids.push((*name, text.clone(), id));
            }
            // The library takes the same abuse, through its own path.
            db::save_announcement(
                &conn,
                None,
                "Ẹ̀bùn 🙏🏾",
                r#"Ọlọ́run "dára" — ìyìn"#,
                "2026-08-16",
            )
            .unwrap();
        }

        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
            db::migrate(&conn, false).unwrap();
            let items = db::plan_items(&conn, plan).unwrap();
            for (name, text, id) in &ids {
                let got = items
                    .iter()
                    .find(|i| i.id == *id)
                    .unwrap_or_else(|| panic!("{name}: the cue vanished across a reopen"));
                assert_eq!(
                    &got.label, text,
                    "{name}: came back different after closing and reopening the file"
                );
            }
            assert_eq!(
                count(&conn, "plan_items") as usize,
                cases.len() + starter_cues
            );
            let a = &db::list_announcements(&conn).unwrap()[0];
            assert_eq!(a.title, "Ẹ̀bùn 🙏🏾");
            assert!(a.body.contains('"'));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The empty case, through the COMMAND layer, because that is where the
    /// validation lives — the db layer will happily store an empty title.
    #[test]
    fn the_commands_refuse_empty_where_an_empty_row_would_be_unusable() {
        let app = bare_app();
        let h = app.handle().clone();

        assert!(
            create_plan(h.state::<Db>(), "   ".into(), "2026-08-16".into()).is_err(),
            "a plan with no title is a row an operator cannot find again"
        );
        assert!(save_announcement(
            h.state::<Db>(),
            None,
            "  ".into(),
            "  ".into(),
            "2026-08-16".into()
        )
        .is_err());
        assert!(save_song(
            h.state::<Db>(),
            1,
            " ".into(),
            "".into(),
            "".into(),
            "".into(),
            None,
            vec![]
        )
        .is_err());
        // GAP, deliberately recorded: a CHANNEL may be created with a blank name.
        // It then appears in Channels and in every screen picker as an unnamed
        // row. Nothing else refuses this.
        let blank = add_channel(h.clone(), h.state::<Db>(), "   ".into(), None, None)
            .expect("today a blank channel name is accepted");
        let db = h.state::<Db>();
        let conn = db.0.lock().unwrap();
        let made = db::list_output_channels(&conn)
            .unwrap()
            .into_iter()
            .find(|c| c.id == blank)
            .unwrap();
        assert_eq!(
            made.name, "",
            "add_channel now validates its name — good; delete this half of the test"
        );
    }

    // ── 5. Referential integrity ─────────────────────────────────────────────

    /// Delete a template a cue pins: the cue must degrade to the content default,
    /// never to a dangling id that a screen tries to resolve.
    ///
    /// `plan_items.template_id` has **no foreign key** (schema.sql), so nothing at
    /// the database level stops the reference dangling. The guarantee lives
    /// entirely in `cue_or_content_tpl`, which is exactly the kind of contract
    /// that is one refactor away from being lost — hence this test.
    #[test]
    fn deleting_a_template_a_cue_pins_degrades_the_cue_instead_of_breaking_it() {
        let app = bare_app();
        let h = app.handle().clone();
        let wall = Wall::watch(&h);
        let db = h.state::<Db>();

        let doomed = {
            let conn = db.0.lock().unwrap();
            db::upsert_template(
                &conn,
                &db::Template {
                    id: 0,
                    name: "Christmas".into(),
                    layout: serde_json::json!({}),
                    style: serde_json::json!({}),
                    active: false,
                },
            )
            .unwrap()
        };
        let plan = create_plan(h.state::<Db>(), "Carols".into(), "2026-12-24".into()).unwrap();
        let cue = add_plan_item(
            h.state::<Db>(),
            plan,
            "announce".into(),
            "Welcome".into(),
            "{}".into(),
            Some(doomed),
        )
        .unwrap();

        // Pinned: the cue's own choice rides out and overrides the screen.
        fire_content(
            h.clone(),
            h.state::<Db>(),
            "Welcome".into(),
            "Merry Christmas".into(),
            "announce".into(),
            None,
            Some(doomed),
            None,
        )
        .unwrap();
        settle();
        let shown = wall.last().unwrap();
        assert_eq!(shown["template_id"], doomed);
        assert_eq!(shown["template_pinned"], true);

        // Now delete the template out from under it.
        delete_template(
            h.state::<Db>(),
            h.state::<servicelock::ServiceLock>(),
            doomed,
        )
        .unwrap();

        // The cue still exists and still carries the dead id — no FK, no cascade.
        {
            let conn = db.0.lock().unwrap();
            let item = db::plan_items(&conn, plan)
                .unwrap()
                .into_iter()
                .find(|i| i.id == cue)
                .unwrap();
            assert_eq!(
                item.template_id,
                Some(doomed),
                "plan_items.template_id has no FK; if this now clears, a cascade \
                 was added and the note in the matrix is stale"
            );
        }

        // ...but firing it must not send a dangling id to a screen.
        fire_content(
            h.clone(),
            h.state::<Db>(),
            "Welcome".into(),
            "Merry Christmas".into(),
            "announce".into(),
            None,
            Some(doomed),
            None,
        )
        .unwrap();
        settle();
        let shown = wall.last().unwrap();
        assert!(
            shown["template_id"].is_null(),
            "a deleted template's id was broadcast to every screen: {shown:?}"
        );
        assert_eq!(shown["template_pinned"], false);

        // And a channel that pointed at it was unassigned, not orphaned.
        let conn = db.0.lock().unwrap();
        assert!(db::list_output_channels(&conn)
            .unwrap()
            .iter()
            .all(|c| c.template_id != Some(doomed)));
    }

    /// Delete a channel: nothing in the schema points AT `output_channels`, so
    /// the only question is whether the app still has somewhere to render.
    ///
    /// It does not stop you deleting the last one. A church with zero screens
    /// configured can still fire — the verse simply reaches nothing and nothing
    /// says so. Recorded here rather than argued.
    #[test]
    fn deleting_every_output_channel_is_allowed_and_a_fire_then_reaches_nothing_visible() {
        let app = bare_app();
        let h = app.handle().clone();
        let wall = Wall::watch(&h);
        let db = h.state::<Db>();

        let ids: Vec<i64> = {
            let conn = db.0.lock().unwrap();
            db::list_output_channels(&conn)
                .unwrap()
                .into_iter()
                .map(|c| c.id)
                .collect()
        };
        for id in ids {
            delete_channel(
                h.clone(),
                h.state::<Db>(),
                h.state::<servicelock::ServiceLock>(),
                id,
            )
            .expect("deleting a screen is allowed");
        }
        {
            let conn = db.0.lock().unwrap();
            assert_eq!(count(&conn, "output_channels"), 0);
        }

        // The fire still succeeds and still emits. `output://content` is a
        // BROADCAST, not a per-channel send — so with no channels configured the
        // event goes out and no window is listening.
        manual_fire(
            h.clone(),
            h.state::<Db>(),
            "John 3:16".into(),
            None,
            None,
            None,
        )
        .expect("firing with no screens configured is not refused");
        settle();
        assert_eq!(
            wall.count(),
            1,
            "the fire path is channel-agnostic: it broadcasts regardless of how \
             many screens exist. Nothing warns the operator that zero are set up."
        );
    }

    /// Delete a song that has arrangements and is cued in a plan.
    ///
    /// Two different mechanisms, and only one of them is written down in
    /// `delete_song`: sections are deleted explicitly, arrangements rely on the
    /// `ON DELETE CASCADE` in the schema, which only fires while
    /// `PRAGMA foreign_keys` is ON. That pragma is turned OFF and back on inside
    /// `ensure_manual_detection_status`, so the asymmetry is worth pinning.
    #[test]
    fn deleting_a_song_takes_its_sections_and_its_arrangements_but_not_the_plan_cue() {
        let app = bare_app();
        let h = app.handle().clone();
        let db = h.state::<Db>();

        let saved = save_reviewed_songs(
            h.state::<Db>(),
            h.state::<servicelock::ServiceLock>(),
            vec![SaveSong {
                title: "Way Maker".into(),
                author: "Sinach".into(),
                ccli: "".into(),
                song_key: "E".into(),
                bpm: None,
                sections: vec![songs::ParsedSection {
                    tag: "V1".into(),
                    label: "Verse 1".into(),
                    lyrics: "You are here".into(),
                }],
            }],
            "2026-08-16".into(),
        )
        .unwrap();
        assert_eq!(saved.added.len(), 1);
        let song_id = {
            let conn = db.0.lock().unwrap();
            db::song_id_by_title(&conn, "Way Maker").unwrap().unwrap()
        };

        // An arrangement, written at the db layer because no control can make one.
        {
            let conn = db.0.lock().unwrap();
            db::save_arrangement(&conn, song_id, None, "Live", &[0, 0]).unwrap();
        }

        // A plan that cues the song. The cue SNAPSHOTS the lyrics (cues.js
        // `songCue`), so it must survive the song being deleted — this is the
        // difference between a plan that still runs on Sunday and one that does
        // not.
        let plan = create_plan(h.state::<Db>(), "Sunday".into(), "2026-08-16".into()).unwrap();
        add_plan_item(
            h.state::<Db>(),
            plan,
            "song".into(),
            "Way Maker".into(),
            format!(
                r#"{{"song_id":{song_id},"title":"Way Maker","sections":[{{"tag":"V1","label":"Verse 1","lyrics":"You are here"}}]}}"#
            ),
            None,
        )
        .unwrap();

        delete_song(
            h.state::<Db>(),
            h.state::<servicelock::ServiceLock>(),
            song_id,
        )
        .unwrap();

        let conn = db.0.lock().unwrap();
        assert_eq!(count(&conn, "songs"), 0);
        assert_eq!(count(&conn, "song_sections"), 0, "sections were orphaned");
        assert_eq!(
            count(&conn, "song_arrangements"),
            0,
            "arrangements were orphaned — `delete_song` deletes sections \
             explicitly but leaves arrangements to the FK cascade, so this fails \
             the moment anything runs with foreign_keys OFF"
        );
        let items = db::plan_items(&conn, plan).unwrap();
        assert_eq!(items.len(), 1, "the plan cue was deleted with the song");
        assert!(
            items[0].payload_json.contains("You are here"),
            "the cue lost its snapshotted lyrics — Sunday's plan is now blank"
        );
    }

    /// Delete a media asset a plan item shows: `delete_media` removes the cue.
    ///
    /// The opposite policy from `delete_song`, and both are defensible — a media
    /// cue holds only an id, so it cannot degrade, while a song cue holds a
    /// snapshot and can. Pinned because the inconsistency is invisible otherwise.
    #[test]
    fn deleting_a_media_asset_takes_the_cue_that_showed_it() {
        let app = bare_app();
        let h = app.handle().clone();
        let db = h.state::<Db>();

        let (media_id, shipped) = {
            let conn = db.0.lock().unwrap();
            // The pictures a fresh install ships are already in this table
            // (DECISIONS §90); this test is about the one it imports.
            let shipped = count(&conn, "media_assets");
            (
                db::insert_media(&conn, "image", "backdrop.png", "2026-08-16", None).unwrap(),
                shipped,
            )
        };
        let plan = create_plan(h.state::<Db>(), "Sunday".into(), "2026-08-16".into()).unwrap();
        add_plan_item(
            h.state::<Db>(),
            plan,
            "media".into(),
            "Backdrop".into(),
            format!(r#"{{"media_id":{media_id}}}"#),
            None,
        )
        .unwrap();
        let keeper = add_plan_item(
            h.state::<Db>(),
            plan,
            "scripture".into(),
            "John 3:16".into(),
            "{}".into(),
            None,
        )
        .unwrap();

        {
            let conn = db.0.lock().unwrap();
            db::delete_media(&conn, media_id).unwrap();
            assert_eq!(count(&conn, "media_assets"), shipped);
            let items = db::plan_items(&conn, plan).unwrap();
            assert_eq!(
                items.len(),
                1,
                "deleting the asset must take its cue and nothing else"
            );
            assert_eq!(items[0].id, keeper);
        }
    }

    /// A media import that fails half-way leaves NOTHING behind.
    ///
    /// ── What this test used to assert, and why it was right to ─────────────
    ///
    /// It was named `a_half_finished_media_import_leaves_a_row_that_serves_a_404`
    /// and it pinned the defect rather than the fix. `import_media` did, in this
    /// order and with no transaction:
    ///
    /// ```text
    ///   db::insert_media(...)          ← committed immediately
    ///   std::fs::write(path, bytes)?   ← disk full / permissions → returns Err
    ///   db::set_media_path(...)        ← never reached
    /// ```
    ///
    /// The `?` propagated and the operator saw an error — but the row was already
    /// there with `path = ''`. `list_media` showed it in the Media library like any
    /// healthy asset, and `serve_media_file` does not consult `path` at all: it
    /// scans `media_dir()` for a `{id}_` prefix, finds nothing, and answers **404**.
    /// So the failure surfaced on Sunday, as a blank output, with no message.
    ///
    /// ── What closed it ─────────────────────────────────────────────────────
    ///
    /// The row still has to be inserted first — its id is half the on-disk name —
    /// so the ordering could not be reversed. `write_media_file` instead UNDOES the
    /// row when the write fails, and is a separate function precisely so that
    /// branch can be executed by a test (`import_guard_tests`) rather than reasoned
    /// about. Inverted, not deleted: the shape of the defect is the reason the
    /// guarantee exists.
    #[test]
    fn a_half_finished_media_import_leaves_no_row_behind() {
        let app = bare_app();
        let h = app.handle().clone();
        let db = h.state::<Db>();
        let conn = db.0.lock().unwrap();

        // The state the old code left: a row whose file never landed.
        // Counted against the pictures a fresh install already ships
        // (DECISIONS §90); what this test is about is the one row it adds.
        let shipped = db::list_media(&conn).unwrap().len();
        let id = db::insert_media(&conn, "image", "backdrop.png", "2026-08-16", None).unwrap();
        assert_eq!(db::list_media(&conn).unwrap().len(), shipped + 1);

        // The write fails the way a full disk fails. The row must not survive it.
        let nowhere = std::path::Path::new("/relay-no-such-directory-7c1b/media");
        let err = crate::write_media_file(&conn, nowhere, id, "backdrop.png", b"x")
            .expect_err("the write must fail");
        assert!(
            !matches!(err, crate::error::Error::Refused { .. }),
            "a disk that said no is a fault, not a refusal the operator can fix"
        );
        assert_eq!(
            db::list_media(&conn).unwrap().len(),
            shipped,
            "the Media library must not list an asset whose file never landed —              there is still no `path == \"\"` filter anywhere, and the media server              still ignores `path` entirely, so a row that survives here is a blank              output on Sunday with no message"
        );

        // The second half of the original finding, unchanged and still true: the
        // server OBS and the native window both hit does not read the DB at all, so
        // a repair that only fixed the column would have fixed nothing.
        //
        // The body lives in `serve_media_from_dir` since ranged replies were added
        // (RG-96) — `serve_media_file` is now the two-line wrapper that supplies
        // the real media directory. This reads BOTH, because a scanner pointed at
        // the wrapper alone finds no `404`, no SQL and nothing else either: it
        // passes by seeing nothing, which is the failure mode this repository has
        // already had twice in its own instruments.
        let ch = std::fs::read_to_string("src/channels.rs").unwrap();
        let serve = ch
            .split("async fn serve_media_from_dir")
            .nth(1)
            .and_then(|s| s.split("\n}\n").next())
            .expect("serve_media_from_dir");
        assert!(
            serve.len() > 500,
            "the media-serving body is not where this test is looking — it read {} \
             characters, which is a scanner that has stopped scanning",
            serve.len()
        );
        assert!(
            !serve.contains("media_assets") && !serve.contains("path FROM"),
            "the media server now reads the DB — re-check what it does with an \
             empty path"
        );
        assert!(serve.contains("404"), "the miss answers 404");
        // And the wrapper still routes to it, so the scan above is about the code
        // that actually runs.
        let wrapper = ch
            .split("async fn serve_media_file")
            .nth(1)
            .and_then(|s| s.split("\n}\n").next())
            .expect("serve_media_file");
        assert!(
            wrapper.contains("serve_media_from_dir"),
            "serve_media_file no longer delegates to the body this test reads"
        );
    }

    // ── 6. Migration retryability (CLAUDE.md §25) ────────────────────────────

    /// Every `ensure_*` rung runs on every boot. Run them repeatedly, on both a
    /// fresh database and a v0 one, and assert nothing accumulates and nothing
    /// errors.
    ///
    /// §25's failure was not "the migration is wrong" — it was "the migration
    /// cannot be run twice", which turns one bad boot into a permanent brick,
    /// before the window is shown. `ensure_manual_detection_status` is the only
    /// rung that rebuilds a table and it is already covered in `db::mod`; this
    /// covers the other nine as a set, which is the door nobody checked.
    #[test]
    fn every_ensure_rung_survives_being_run_over_and_over() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::init_fresh(&conn).unwrap();

        let before: Vec<i64> = [
            "templates",
            "output_channels",
            "voice_profiles",
            "app_settings",
        ]
        .iter()
        .map(|t| count(&conn, t))
        .collect();

        // Ten boots. `ensure_tables` is what `migrate` runs for every existing
        // database, on every launch, forever.
        for boot in 0..10 {
            db::migrate(&conn, false)
                .unwrap_or_else(|e| panic!("boot {boot} failed: {e} — the app would panic here"));
        }

        let after: Vec<i64> = [
            "templates",
            "output_channels",
            "voice_profiles",
            "app_settings",
        ]
        .iter()
        .map(|t| count(&conn, t))
        .collect();
        assert_eq!(
            before, after,
            "a rung is not idempotent — it added rows on a later boot"
        );

        // No scratch table anywhere. `detections_new` is the §25 fingerprint; the
        // assertion is generalised so a NEW rebuild that invents its own scratch
        // name is caught too.
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .flatten()
            .collect();
        for t in &tables {
            assert!(
                !t.ends_with("_new") && !t.ends_with("_old") && !t.ends_with("_tmp"),
                "a migration left the scratch table {t:?} behind — the §25 brick"
            );
        }

        // Foreign keys are ON when the boot returns. The pragma is a no-op inside
        // an open transaction, so a rung that failed without rolling back used to
        // return with FKs silently off for the rest of the session.
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1, "a boot left foreign keys OFF");
        conn.execute_batch("BEGIN; COMMIT;")
            .expect("a boot left a transaction dangling");
    }

    /// The v0 path, run twice. A pre-versioning database gets the sniff-based
    /// forward-fills; running them a second time must change nothing.
    #[test]
    fn the_v0_forward_fill_path_is_retryable_too() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::init_fresh(&conn).unwrap();
        // Pretend this database predates versioning, which is what every install
        // older than `SCHEMA_VERSION` looks like on the first launch after an
        // update.
        conn.execute_batch("PRAGMA user_version = 0;").unwrap();

        db::migrate(&conn, false).expect("first v0 boot");
        let channels = count(&conn, "output_channels");
        let verses = db::verse_count(&conn).unwrap();

        conn.execute_batch("PRAGMA user_version = 0;").unwrap();
        db::migrate(&conn, false).expect("a second v0 boot must not fail");

        assert_eq!(count(&conn, "output_channels"), channels);
        assert_eq!(
            db::verse_count(&conn).unwrap(),
            verses,
            "the KJV was re-imported"
        );
        assert_eq!(
            count(&conn, "translations"),
            1,
            "a duplicate KJV translation"
        );
    }

    /// LATENT HAZARD, pinned so the ordering that protects against it stops being
    /// a comment.
    ///
    /// `ensure_manual_detection_status` rebuilds `detections` from a **hard-coded
    /// seven-column list**. `detections.heard_text` is an eighth column, added by
    /// the v2 rung. Today the two cannot collide, because the rebuild only runs on
    /// the v0 baseline path and `run_migrations` is called *after* it — an ordering
    /// stated in a comment in `db/mod.rs` and enforced by nothing.
    ///
    /// If they ever do run in the other order, the rebuild silently drops the one
    /// column that makes a wrong verse on a wall diagnosable after the fact. Forty
    /// wrong verses reached a real congregation once and the log could not say what
    /// any of them heard; `heard_text` exists because of that service.
    ///
    /// This test asserts BOTH halves: the loss is real if the order is wrong, and
    /// the real `migrate` path does not produce it.
    #[test]
    fn the_detections_rebuild_would_drop_heard_text_and_only_the_call_order_stops_it() {
        // (a) The hazard, demonstrated. An old-CHECK table that has somehow already
        //     gained `heard_text` loses it to the rebuild.
        let conn = Connection::open_in_memory().unwrap();
        db::init_fresh(&conn).unwrap();
        conn.execute_batch(
            "DROP TABLE detections;
             CREATE TABLE detections (
                 id            INTEGER PRIMARY KEY,
                 transcript_id INTEGER NOT NULL,
                 verse_id      INTEGER,
                 method        TEXT NOT NULL CHECK (method IN ('direct', 'semantic')),
                 confidence    REAL NOT NULL,
                 status        TEXT NOT NULL CHECK (status IN ('auto','suggested','dismissed')),
                 fired_at      REAL,
                 heard_text    TEXT
             );
             INSERT INTO detections
                 (transcript_id, verse_id, method, confidence, status, fired_at, heard_text)
                 VALUES (1, 1, 'direct', 1.0, 'auto', 0.0, 'john three sixteen');
             PRAGMA user_version = 0;",
        )
        .unwrap();
        let has_heard = |c: &Connection| -> i64 {
            c.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('detections') WHERE name = 'heard_text'",
                [],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(has_heard(&conn), 1);
        db::migrate(&conn, false).expect("v0 boot");
        assert_eq!(
            has_heard(&conn),
            1,
            "the v2 rung re-adds the column after the rebuild dropped it — so the \
             SHAPE recovers. What does not recover is the DATA:"
        );
        let evidence: Option<String> = conn
            .query_row("SELECT heard_text FROM detections", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            evidence, None,
            "if this is now Some(...), the rebuild learned to carry `heard_text` \
             across and this hazard is closed — delete the test"
        );

        // (b) The real path does NOT hit it: on a genuine v0 database the column
        //     does not exist yet when the rebuild runs, so nothing is lost.
        let real = Connection::open_in_memory().unwrap();
        real.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::init_fresh(&real).unwrap();
        real.execute_batch(
            "INSERT INTO services (id, date, title) VALUES (1,'2026-08-16','Sunday');
             INSERT INTO transcripts (id, service_id, timestamp, text, language)
                 VALUES (1, 1, 0.0, 'john three sixteen', 'en');
             INSERT INTO detections
                 (transcript_id, verse_id, method, confidence, status, fired_at, heard_text)
                 VALUES (1, NULL, 'direct', 1.0, 'manual', 0.0, 'john three sixteen');
             PRAGMA user_version = 0;",
        )
        .unwrap();
        db::migrate(&real, false).expect("v0 boot on a current-shaped database");
        let kept: Option<String> = real
            .query_row("SELECT heard_text FROM detections", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            kept.as_deref(),
            Some("john three sixteen"),
            "a real boot lost the detection evidence"
        );
    }

    // ── 7. The seed, asserted ────────────────────────────────────────────────

    /// What a church actually receives, item by item, so a change to the seed is
    /// a decision and not an accident.
    ///
    /// The counts are the R1 claim, now in `docs/qa/QA_HARNESS.md` Part 3 — it
    /// superseded the three Working-Agent documents, which are gone. If one
    /// moves, the audit's seed section is out of date.
    #[test]
    fn the_seed_is_exactly_what_the_audit_says_it_is() {
        let app = bare_app();
        let h = app.handle().clone();
        let db = h.state::<Db>();
        let conn = db.0.lock().unwrap();

        assert_eq!(db::verse_count(&conn).unwrap(), 31_102, "the bundled KJV");
        assert_eq!(count(&conn, "translations"), 1);
        assert_eq!(
            count(&conn, "verses_fts"),
            31_102,
            "the FTS mirror is built"
        );
        // THE SHELF. The exact total is asserted in
        // `db::mod::seeds_the_builtin_templates` against the code's own count; here
        // it is the ROLES that matter, because the seed audit's claim is "a church
        // finds a look for every kind of content it fires", not "some number of
        // rows exist".
        let names: Vec<String> = db::list_templates(&conn)
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        // 31 at the time of the cold-start audit, 39 after REBRAND wave 4 added the
        // shelf, 40 since wave 5 rebuilt the shelf whole: eight roles of five, every
        // one layer-model, replacing the five region-model built-ins and the
        // twenty-five family rows. The figure is prose, not an assertion, for the
        // reason stated above.
        for role in [
            "Scripture · ",
            "Song · ",
            "Media · ",
            "Announce · ",
            "Timer · ",
            "Scroll · ",
            "Source · ",
            "Stage · ",
        ] {
            assert_eq!(
                names.iter().filter(|n| n.starts_with(role)).count(),
                5,
                "the seed does not ship five {role:?} looks"
            );
        }
        assert_eq!(count(&conn, "output_channels"), 4);
        assert_eq!(count(&conn, "voice_profiles"), 1);
        assert!(db::active_voice_profile(&conn).unwrap().is_some());

        // The ONE app_settings row a fresh install ships, and the reason for it.
        let mut stmt = conn
            .prepare("SELECT key FROM app_settings ORDER BY key")
            .unwrap();
        let keys: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(
            keys,
            vec!["tpl_song".to_string()],
            "a fresh install writes exactly one setting — the lyrics content-look. \
             Anything else here is a preference nobody chose."
        );

        // WHAT A FRESH INSTALL CONTAINS, AND WHAT IT STILL DOES NOT.
        //
        // This was one list of twelve tables asserted at zero. DECISIONS §90
        // gives a first launch a starter set, so three of the twelve now carry
        // rows and the list is split rather than shortened: the starter set is
        // asserted by identity in
        // `the_bare_fixture_is_a_first_launch_and_nothing_more`, and everything
        // else is still held at zero here. The service-record tables in
        // particular are not negotiable — a fresh install has no history of a
        // service that never happened.
        for table in [
            "songs",
            "song_sections",
            "song_arrangements",
            "saved_scripture",
            "services",
            "transcripts",
            "detections",
            "cues",
        ] {
            assert_eq!(
                count(&conn, table),
                0,
                "{table} is not empty on a fresh install"
            );
        }
        // The starter set, counted here only so this audit's own walk is
        // complete; its identity is the tripwire's job, not this test's.
        assert_eq!(count(&conn, "service_plans"), 1);
        assert_eq!(count(&conn, "plan_items"), 3);
        assert_eq!(count(&conn, "announcements"), 5);
        assert_eq!(
            count(&conn, "media_assets") as usize,
            channels::bundled_backgrounds().len()
        );
    }

    /// The seeded voice profile's thresholds must BE the one baseline, not a
    /// second copy of it. `Thresholds::default() == from_sensitivity(50)` by
    /// construction (router.rs); a seed row that hard-codes different numbers
    /// would be a second baseline that nothing compares against.
    #[test]
    fn the_seeded_voice_profile_is_the_one_baseline_and_not_a_second_copy() {
        let app = bare_app();
        let h = app.handle().clone();
        let db = h.state::<Db>();
        let conn = db.0.lock().unwrap();

        let p = db::active_voice_profile(&conn)
            .unwrap()
            .expect("one active profile");
        let baseline = Thresholds::default();
        assert_eq!(p.sensitivity, 50);
        assert!((p.auto_fire - baseline.auto_fire as f64).abs() < 1e-6);
        assert!((p.suggest - baseline.suggest as f64).abs() < 1e-6);
    }

    /// ONE CONVENIENCE, AND IT IS THE FRESH-INSTALL CASE RATHER THAN A SHORTCUT.
    ///
    /// `bare_app` does not manage `Stt` because the harness has no whisper in it.
    /// `Stt(Mutex::new(None))` is precisely what `build_stt` returns on a machine
    /// where no model has been downloaded yet — which is every church, on the day
    /// the installer runs. The two tests below are about what happens BEFORE the
    /// first download, so that is the honest state to put the fixture in.
    fn app_with_no_speech_model() -> tauri::App<tauri::test::MockRuntime> {
        let app = bare_app();
        app.handle().manage(Stt(Mutex::new(None)));
        app
    }

    /// THE RECOGNITION LANGUAGE AN OPERATOR PICKS SURVIVES THE LAUNCH THEY PICKED
    /// IT IN (RG-138).
    ///
    /// `set_stt_language` used to take no `Db` at all: it set a field on the live
    /// engine and nothing else, and `stt_status` read that field back, so the
    /// choice looked sticky for the rest of the run and was gone at the next
    /// launch. RG-116 names this control as the mitigation for a real field
    /// failure, so the register named a fix that did not survive a relaunch.
    ///
    /// The assertion is on the DATABASE rather than on the engine on purpose:
    /// `main.rs`'s setup hook applies `db::active_voice_profile` to whisper before the first
    /// word, so the profile row IS what the next launch will recognise in.
    #[test]
    fn the_recognition_language_is_written_to_the_active_voice_profile() {
        let app = app_with_no_speech_model();
        let h = app.handle().clone();

        // A fresh install: the seeded `Default` is active and set to auto-detect.
        let db = h.state::<Db>();
        {
            let conn = db.0.lock().unwrap();
            let p = db::active_voice_profile(&conn).unwrap().unwrap();
            assert_eq!(p.name, "Default");
            assert!(p.language.is_none());
        }

        let saved = set_stt_language(h.state::<Stt>(), h.state::<Db>(), Some("en".into()))
            .expect("a fresh install must be able to pin the recognition language");
        assert_eq!(saved.language.as_deref(), Some("en"));
        assert_eq!(
            saved.name, "Default",
            "the control writes to whichever profile is ACTIVE, and on a fresh \
             install that is the seeded one"
        );

        {
            let conn = db.0.lock().unwrap();
            assert_eq!(
                db::active_voice_profile(&conn)
                    .unwrap()
                    .unwrap()
                    .language
                    .as_deref(),
                Some("en"),
                "the language is not on the profile, so the next launch will \
                 recognise in whatever whisper elects — which is the defect this \
                 test exists for"
            );
        }

        // Auto-detect is a choice too, and it has to be able to come back.
        set_stt_language(h.state::<Stt>(), h.state::<Db>(), None).unwrap();
        let conn = db.0.lock().unwrap();
        assert!(db::active_voice_profile(&conn)
            .unwrap()
            .unwrap()
            .language
            .is_none());
    }

    /// NO ENGINE IS NOT "NO LANGUAGE" (rule 35).
    ///
    /// Before a model is downloaded there is no engine to ask, and the recognition
    /// language is still a real stored fact. Reporting `None` here would print
    /// "Auto-detect" on the Settings select over a profile that says English — a
    /// status line that reads the same when the setting is missing as when it is
    /// set.
    #[test]
    fn the_stored_language_is_reported_when_no_speech_model_is_loaded() {
        let app = app_with_no_speech_model();
        let h = app.handle().clone();

        let fresh = stt_status(h.state::<Stt>(), h.state::<Db>()).unwrap();
        assert!(!fresh.loaded);
        assert_eq!(fresh.language, None, "a fresh install is on auto-detect");

        set_stt_language(h.state::<Stt>(), h.state::<Db>(), Some("yo".into())).unwrap();
        let after = stt_status(h.state::<Stt>(), h.state::<Db>()).unwrap();
        assert!(!after.loaded, "still no model — nothing downloaded one");
        assert_eq!(
            after.language.as_deref(),
            Some("yo"),
            "the pin is stored and will be applied the moment an engine exists, so \
             the control must not read back as Auto-detect"
        );
    }
}

#[cfg(test)]
mod kiosk_headers {

    /// The LAN pages must be served with a CSP, and it must be at least as tight as
    /// the packaged app's on the directives that keep a template offline.
    ///
    /// The packaged webview had one and this server had none — so `output.html`
    /// inside Relay was constrained and the *same page* handed to an OBS browser
    /// source, a kiosk screen or a phone was not. Those clients are ordinary
    /// browsers rendering a look assembled from template JSON that may have arrived
    /// in an email, which makes them the half of the audience that needed it most.
    #[test]
    fn the_lan_pages_carry_a_policy_and_it_forbids_the_network() {
        let src = include_str!("channels.rs");
        assert!(
            src.contains("Content-Security-Policy: {}"),
            "the kiosk/OBS pages are served with no CSP at all"
        );

        // The one way it is deliberately TIGHTER than `tauri.conf.json`: the desktop
        // app allows `http:` images for operator-chosen local sources; a page on the
        // LAN has no such need, and Relay renders offline or it does not render.
        let policy = crate::channels::KIOSK_CSP;
        for directive in ["img-src", "media-src"] {
            let d = policy
                .split(';')
                .map(str::trim)
                .find(|d| d.starts_with(directive))
                .unwrap_or_else(|| panic!("{directive} is missing from the kiosk CSP"));
            assert!(
                !d.contains("http:"),
                "{directive} allows the network: {d:?} — an imported template could \
                 beacon from a kiosk, and would render blank the Sunday the wifi is out"
            );
        }
        assert!(policy.contains("object-src 'none'"));
        assert!(policy.contains("script-src 'self'"));
    }

    /// RG-85 · THE OPERATOR CONSOLE'S POLICY NAMES THE LAN, NOT THE INTERNET.
    ///
    /// `tauri.conf.json` granted `http:` — every host, every port — to `img-src`,
    /// `media-src` and `connect-src`, and the reason was real: the media server and
    /// the kiosk hub cannot be TLS on a LAN appliance. The grant was written far
    /// wider than the need, and it was **reachable**: `TemplateRender`'s `bgPaint`
    /// emits `url("${L.image}")` straight from template JSON, so a template that
    /// arrived by email beaconed an attacker's `http://` URL the moment an operator
    /// previewed it — the church's IP and the time it was opened, out of an app
    /// whose first promise is that nothing leaves the device.
    ///
    /// The narrowing is by PORT, because a LAN address cannot be named in a static
    /// policy: `http://*:8032` is Relay's own media server on any host, and
    /// `ws://*:8031` its kiosk hub. Both are the ports this binary opens.
    ///
    /// **This test cannot prove a kiosk screen still renders** — `tauri dev` does
    /// not exercise the CSP at all, and only a packaged build driven through real
    /// media playback can answer that. What it proves is that the policy still
    /// permits the URL shape Relay itself generates, and no longer permits an
    /// arbitrary host, which is the half that a change could silently get wrong.
    #[test]
    fn the_console_policy_allows_relays_own_media_url_and_no_other_host() {
        let conf = include_str!("../tauri.conf.json");
        let csp = conf
            .split("\"csp\"")
            .nth(1)
            .and_then(|s| s.split('"').nth(1))
            .expect("no csp in tauri.conf.json");
        assert!(
            csp.len() > 100,
            "the policy read back as {csp:?} — this test is not reading a policy"
        );

        let directive = |name: &str| -> String {
            csp.split(';')
                .map(str::trim)
                .find(|d| d.starts_with(name))
                .unwrap_or_else(|| panic!("{name} is missing"))
                .to_string()
        };

        for name in ["img-src", "media-src", "connect-src"] {
            let d = directive(name);
            // A bare `http:` is the whole internet. It is the thing that was there.
            assert!(
                !d.split_whitespace().any(|t| t == "http:" || t == "ws:"),
                "{name} still grants every host: {d:?}"
            );
        }

        // The URL `main::fire_media` actually builds, on a real LAN address.
        let media = "http://192.168.1.9:8032/media/12";
        for name in ["img-src", "media-src"] {
            assert!(
                csp_allows(&directive(name), media),
                "{name} would block Relay's own media URL — a background video on the \
                 wall goes black and nothing says why"
            );
        }
        assert!(
            csp_allows(&directive("connect-src"), "ws://192.168.1.9:8031"),
            "connect-src would block the kiosk hub"
        );
        // And the beacon RG-85 was actually about.
        assert!(
            !csp_allows(&directive("img-src"), "http://evil.example.com/beacon.png"),
            "an emailed template can still call home"
        );
        assert!(
            !csp_allows(&directive("img-src"), "http://192.168.1.9:9999/beacon.png"),
            "any port on the LAN is still the network"
        );
    }

    /// Does one CSP directive permit one URL? Enough of the host-source grammar for
    /// the sources this policy uses: `scheme:`, `scheme://host[:port]`, and `*` as a
    /// whole host. Deliberately small — a full CSP parser here would be a second
    /// implementation to be wrong in.
    fn csp_allows(directive: &str, url: &str) -> bool {
        let (scheme, rest) = url.split_once("://").expect("url with a scheme");
        let (host, port) = match rest.split('/').next().unwrap_or("").split_once(':') {
            Some((h, p)) => (h.to_string(), p.to_string()),
            None => (
                rest.split('/').next().unwrap_or("").to_string(),
                String::new(),
            ),
        };
        directive.split_whitespace().skip(1).any(|src| {
            if let Some(s) = src.strip_suffix(':') {
                return s == scheme; // `http:` — the whole scheme
            }
            let Some((s, hostpart)) = src.split_once("://") else {
                return false; // 'self', 'none', data:, blob: — handled above or not a host
            };
            if s != scheme {
                return false;
            }
            let (shost, sport) = match hostpart.split_once(':') {
                Some((h, p)) => (h, p),
                None => (hostpart, ""),
            };
            let host_ok = shost == "*" || shost == host;
            let port_ok = sport.is_empty() || sport == "*" || sport == port;
            host_ok && port_ok
        })
    }
}
