//! End-to-end: the path that actually puts scripture in front of a congregation.
//!
//! ## Why this file exists
//!
//! `main.rs` is ~2,900 lines and 101 commands, and it had **zero tests**. There was no
//! `tests/` directory, no driver, no integration test anywhere in the repo. Every
//! module *below* the orchestration layer is well covered — detection, the router, the
//! pipeline, the db — but the wiring that turns "the preacher said John 3:16" into
//! "John 3:16 is on the wall" was verified only by a human driving the app by hand.
//!
//! That is the one path where a regression is measured in Sundays.
//!
//! So these tests drive the REAL commands — `fire_content`, `nav`, `clear_screens`,
//! `blackout` — against a REAL (in-memory) database, through the REAL router and
//! pipeline, and assert on the events that actually leave the machine. Nothing is
//! mocked except the window: `tauri::test::mock_builder` gives a headless app with the
//! same managed state `main()` builds.
//!
//! What they cover, deliberately:
//!   - fire → the verse reaches the outputs, with its text and its template
//!   - nav   → next/back walk the passage, and SAY SO when they cannot
//!   - clear → the screens go blank AND the command reports that they did
//!   - the gate → a paraphrase can never auto-fire, however confident it claims to be
//!   - rehearsal → nothing reaches the congregation, from any of the above

use super::*;
use std::sync::Arc;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::{Listener, Manager};

/// A headless Relay with the same state `main()` manages, and a real database.
///
/// The DB is in-memory and seeded exactly as a fresh install is — so the verses these
/// tests fire are the verses a church would get.
fn app() -> tauri::App<tauri::test::MockRuntime> {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    db::init_fresh(&conn).expect("seed schema + KJV + templates + channels");

    // A fresh install seeds templates but does NOT assign a per-content-type
    // override — `tpl_scripture` is only written when the operator picks one, and
    // without it the channel's own template is used (docs/DECISIONS.md). So pick one
    // here, which is what makes the "every fire carries its template" invariant
    // testable at all: with no override set, `template_id` is legitimately None and
    // the assertion would be vacuous.
    let tpl: i64 = conn
        .query_row("SELECT id FROM templates ORDER BY id LIMIT 1", [], |r| {
            r.get(0)
        })
        .expect("a fresh install seeds templates");
    db::set_content_template(&conn, "scripture", Some(tpl)).expect("set scripture template");

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

    let app = mock_builder()
        .manage(Db(Mutex::new(conn)))
        .manage(Routing::default())
        .manage(Outputs::default())
        .manage(Detecting(AtomicBool::new(true)))
        .manage(channels::Rehearsal::default())
        .manage(Session::default())
        .manage(Semantic(SemanticIndex::build(&corpus)))
        .manage(Context(Mutex::new(ContextMemory::default())))
        // mock_context, NOT generate_context!(): the real macro embeds Info.plist as a
        // link symbol, and expanding it a second time fails with
        // "symbol `_EMBED_INFO_PLIST` is already defined".
        .build(mock_context(noop_assets()))
        .expect("mock app");

    // The kiosk hub and the audio engine are deliberately NOT managed: `channels`
    // reaches for them with `try_state`, so their absence is the "no LAN, no mic"
    // case — which is exactly what a headless test is.
    app
}

/// Records everything that leaves the machine through the output layer.
///
/// This is the assertion surface that matters: not "did the function return Ok", but
/// "what did the congregation actually see".
#[derive(Default)]
struct Wall {
    content: Arc<Mutex<Vec<serde_json::Value>>>,
    cleared: Arc<AtomicBool>,
    blacked: Arc<AtomicBool>,
}

impl Wall {
    fn watch(app: &tauri::AppHandle<tauri::test::MockRuntime>) -> Self {
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

    fn last(&self) -> Option<serde_json::Value> {
        self.content.lock().unwrap().last().cloned()
    }
    fn count(&self) -> usize {
        self.content.lock().unwrap().len()
    }
}

/// Tauri delivers events to listeners asynchronously on the mock runtime; give the
/// loop a moment to drain before asserting on what the wall received.
fn settle() {
    std::thread::sleep(std::time::Duration::from_millis(60));
}

#[test]
fn a_verse_the_operator_fires_reaches_the_congregation_with_its_text() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None)
        .expect("fire John 3:16");
    settle();

    let shown = wall.last().expect("nothing reached the outputs");
    assert_eq!(shown["reference"], "John 3:16");
    assert!(
        shown["text"]
            .as_str()
            .unwrap_or("")
            .contains("God so loved"),
        "the verse arrived without its text: {:?}",
        shown["text"]
    );
    // EVERY fire path must carry the scripture template. Two nav paths once forgot it,
    // so the same verse looked different depending on how the preacher phrased it.
    assert!(
        !shown["template_id"].is_null(),
        "the scripture template was dropped on the way to the wall"
    );
    // The NEXT verse rides along for a stage/confidence monitor's "up next" line.
    // John 3:17 exists in the seeded KJV, so firing 3:16 must carry it. The
    // congregation template ignores it; only a monitor template renders it.
    assert_eq!(
        shown["next_reference"], "John 3:17",
        "the next verse did not ride to the monitors"
    );
    assert!(
        shown["next_text"]
            .as_str()
            .unwrap_or("")
            .contains("condemn"),
        "the next verse arrived without its text: {:?}",
        shown["next_text"]
    );
}

/// The "up next" line on a stage/confidence monitor respects the READ RANGE: it
/// previews the next verse mid-passage, but shows nothing once the last verse of
/// the bounded range is up — it must not spill into the verse after the reading.
#[test]
fn a_bounded_passage_shows_no_next_verse_past_its_end() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16-17".into(),
        None,
        None,
    )
    .expect("fire the range John 3:16-17");
    settle();
    let first = wall.last().expect("first fire reached the wall");
    assert_eq!(first["reference"], "John 3:16");
    assert_eq!(
        first["next_reference"], "John 3:17",
        "mid-range, the next verse should preview"
    );

    // Walk to the last verse of the read range.
    match nav(h.clone(), "next".into()).expect("nav next") {
        NavResult::Fired { reference } => assert_eq!(reference, "John 3:17"),
        other => panic!("expected to fire 3:17, got {:?}", other.kind()),
    }
    settle();
    let last = wall.last().expect("second fire reached the wall");
    assert_eq!(last["reference"], "John 3:17");
    assert!(
        last["next_reference"].is_null(),
        "at the end of the bounded range there is no next verse — got {:?}",
        last["next_reference"]
    );
}

#[test]
fn a_plan_cues_own_template_reaches_the_wall_not_just_the_content_default() {
    // THE PLANNER FIX. A plan item can carry its own `template_id` (the operator
    // picked a specific look for that cue). Every fire path used to resolve the
    // template from the content TYPE only, so the per-cue choice never left the
    // machine. Fire a scripture cue WITH a template override and assert that exact
    // id is what the outputs receive.
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    // A real, seeded template id that is NOT the scripture content default.
    let override_id: i64 = {
        let db = h.state::<Db>();
        let conn = db.0.lock().unwrap();
        crate::db::list_templates(&conn).unwrap()[1].id // Stage Mono
    };

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        Some(override_id),
    )
    .expect("fire with a template override");
    settle();

    let shown = wall.last().expect("nothing reached the outputs");
    assert_eq!(
        shown["template_id"].as_i64(),
        Some(override_id),
        "the cue's own template was dropped; the wall got {:?}",
        shown["template_id"]
    );
    // And the override JSON rode along, so the output actually re-styles.
    assert!(
        !shown["template_json"].is_null(),
        "the override id crossed but its style JSON did not"
    );
}

#[test]
fn a_lyric_slide_projects_the_lyric_and_not_the_song_title() {
    // "Blessed Assurance · Slide 1" across the top of the wall is the
    // operator's bookkeeping leaking onto a screen full of people. The label
    // still names the cue; it just does not go out.
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    fire_content(
        h.clone(),
        h.state::<Db>(),
        "Blessed Assurance · Verse 1".into(),
        "Blessed assurance, Jesus is mine".into(),
        "song".into(),
        None,
        None,
    )
    .expect("fire the lyric");
    settle();

    let shown = wall.last().expect("nothing reached the outputs");
    assert_eq!(
        shown["reference"], "",
        "the song title was projected: {:?}",
        shown["reference"]
    );
    assert!(
        shown["text"]
            .as_str()
            .unwrap_or("")
            .contains("Blessed assurance"),
        "the lyric arrived without its words"
    );
}

#[test]
fn an_announcement_still_shows_its_title() {
    // The lyric rule is for lyrics only. A notice without its heading is a
    // sentence floating on a wall with nothing to say what it is.
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    fire_content(
        h.clone(),
        h.state::<Db>(),
        "Midweek service".into(),
        "Wednesday at 7pm".into(),
        "announce".into(),
        None,
        None,
    )
    .expect("fire the notice");
    settle();

    assert_eq!(
        wall.last().expect("nothing reached")["reference"],
        "Midweek service"
    );
}

#[test]
fn next_and_back_walk_the_passage() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None).unwrap();

    match nav(h.clone(), "next".into()).expect("nav next") {
        NavResult::Fired { reference } => assert_eq!(reference, "John 3:17"),
        other => panic!("next did not fire the next verse: {:?}", other.kind()),
    }
    match nav(h.clone(), "back".into()).expect("nav back") {
        NavResult::Fired { reference } => assert_eq!(reference, "John 3:16"),
        other => panic!("back did not return: {:?}", other.kind()),
    }
    settle();
    assert_eq!(wall.count(), 3, "each step must reach the wall");
}

/// THE SILENT NO-OP. `nav` used to return `()` and simply do nothing at the end of a
/// passage — no error, no toast, no log — on the key the operator presses most.
///
/// Note the two DIFFERENT honest answers here, which is exactly why a bool would have
/// been the wrong repair:
///
/// - nothing staged → `NoPassage` ("fire a verse first")
/// - the end of a known passage → `EndOfPassage` ("that's the end of it")
///
/// Both mean "the wall did not change". Only one of them is a problem, and the
/// operator is entitled to know which.
#[test]
fn nav_says_so_when_it_cannot_move_instead_of_doing_nothing() {
    let app = app();
    let h = app.handle().clone();

    // Nothing on screen: there is no passage to step through, and the operator must
    // be told THAT, not left wondering whether the key is broken.
    assert!(
        matches!(nav(h.clone(), "next".into()).unwrap(), NavResult::NoPassage),
        "stepping with nothing staged must report NoPassage"
    );

    // "Psalm 23" is a whole-chapter reference, so the passage is BOUNDED — Relay knows
    // it ends at verse 6. Walk to the end of it and step off.
    manual_fire(h.clone(), h.state::<Db>(), "Psalm 23".into(), None, None).unwrap();
    for _ in 0..5 {
        assert!(
            matches!(
                nav(h.clone(), "next".into()).unwrap(),
                NavResult::Fired { .. }
            ),
            "should still be walking Psalm 23"
        );
    }
    assert!(
        matches!(
            nav(h.clone(), "next".into()).unwrap(),
            NavResult::EndOfPassage
        ),
        "stepping off the end of a bounded passage must report EndOfPassage, not silence"
    );
}

/// A verse beyond the end of an UNBOUNDED passage (a single verse, no known span)
/// resolves to something that is not in the corpus. That is still not silence: the
/// operator is told the verse does not exist and that the screen was left alone —
/// which matters, because the previous verse is still up there.
#[test]
fn stepping_past_the_last_verse_of_a_book_is_reported_not_swallowed() {
    let app = app();
    let h = app.handle().clone();

    manual_fire(h.clone(), h.state::<Db>(), "Jude 1:25".into(), None, None).unwrap(); // last verse of Jude
    let r = nav(h.clone(), "next".into()).unwrap();

    assert!(
        !matches!(r, NavResult::Fired { .. }),
        "there is no Jude 1:26 — nothing may reach the wall"
    );
    assert!(
        matches!(r, NavResult::NotInLibrary { .. }),
        "the operator must be told WHY nothing moved, got {}",
        r.kind()
    );
}

#[test]
fn clear_blanks_the_screens_and_reports_that_it_did() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None).unwrap();
    settle();
    assert!(wall.last().is_some());

    // Returns a Result now. It used to return `()`, so a failed clear was
    // indistinguishable from a successful one and the console cheerfully flashed
    // "Screens cleared" over a wall that still had scripture on it.
    clear_screens(h.clone()).expect("clear must report success");
    settle();
    assert!(
        wall.cleared.load(Ordering::SeqCst),
        "the screens never cleared"
    );

    blackout(h.clone()).expect("blackout must report success");
    settle();
    assert!(
        wall.blacked.load(Ordering::SeqCst),
        "the screens never blacked out"
    );
}

/// A verse that parses but does not exist must NEVER be broadcast — it would render
/// with no text and blank the projector mid-service. Garbled speech readily produces
/// these ("Psalms 23:99").
#[test]
fn a_verse_that_does_not_exist_never_reaches_the_wall() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    let _ = manual_fire(
        h.clone(),
        h.state::<Db>(),
        "Psalms 23:99".into(),
        None,
        None,
    );
    settle();

    assert_eq!(
        wall.count(),
        0,
        "a non-existent verse was broadcast — the projector would have gone blank"
    );
}

/// REHEARSAL. The single most important guarantee in the product: during a rehearsal,
/// nothing reaches the congregation. Gated at `channels::broadcast_content` — the one
/// function content leaves the machine through — so every future fire path is
/// sandboxed by construction (docs/DECISIONS.md §18).
#[test]
fn nothing_reaches_the_congregation_during_a_rehearsal() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    set_rehearsal(
        h.clone(),
        h.state::<Session>(),
        h.state::<channels::Rehearsal>(),
        true,
    )
    .expect("enter rehearsal");

    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None).unwrap();
    let _ = nav(h.clone(), "next".into());
    settle();

    assert_eq!(
        wall.count(),
        0,
        "content escaped to the outputs during a rehearsal"
    );
}

/// REHEARSAL, THROUGH THE OTHER DOOR — the WebSocket hub, which the wall cannot see.
///
/// `Wall` listens for Tauri events. That is the whole assertion surface of the test
/// above, and it is why `channels::stage_next` leaked for as long as it did: the
/// stage/confidence monitor is ALWAYS a network client (stage.html over :8032, state
/// over the :8031 hub), so `stage_next` publishes to the kiosk hub and emits no Tauri
/// event at all. It was invisible to the gate and invisible to the guard.
///
/// The failure it allowed is the quiet kind. Nothing on the congregation wall moves —
/// so the sandbox looks intact — while the preacher's own tablet, still connected
/// from the last service, is handed the real "up next" mid-rehearsal.
///
/// This test therefore subscribes to the hub itself. The e2e app deliberately does
/// not manage a `KioskHub` (headless = "no LAN"), so it is attached here.
#[test]
fn nothing_reaches_the_stage_monitor_during_a_rehearsal() {
    let app = app();
    let h = app.handle().clone();
    h.manage(channels::KioskHub::default());
    let mut kiosk = h.state::<channels::KioskHub>().sender().subscribe();

    // Not rehearsing: the stage monitor is supposed to get it. Assert that FIRST, so
    // this test cannot pass by the publish path being broken outright.
    set_stage_next(h.clone(), Some("Up next".into()), Some("John 3:16".into()));
    settle();
    let live = kiosk
        .try_recv()
        .expect("a real service must reach the stage");
    assert!(
        live.contains("stage_next") && live.contains("John 3:16"),
        "the stage monitor got something other than the up-next it was sent: {live}"
    );

    set_rehearsal(
        h.clone(),
        h.state::<Session>(),
        h.state::<channels::Rehearsal>(),
        true,
    )
    .expect("enter rehearsal");

    set_stage_next(h.clone(), Some("Up next".into()), Some("Psalm 23:1".into()));
    settle();
    assert!(
        kiosk.try_recv().is_err(),
        "the up-next preview escaped to a live stage monitor during a rehearsal"
    );
}

/// THE PREACHER'S REMOTE, end to end. The phone talks to the same HTTP handler
/// (`remote_api`) that `main.rs` wires onto :8032, which drives the SAME fire and
/// nav commands the console does — one engine, no second code path. This proves a
/// search result the preacher taps actually reaches the wall, and that the
/// remote's Next walks the staged passage exactly like the console's.
#[test]
fn the_preacher_remote_searches_fires_and_walks_the_passage() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    // Search resolves an explicit reference to a real verse the remote can offer.
    let hits = super::remote_api(&h, "search?q=John%203:16");
    let hits: serde_json::Value = serde_json::from_str(&hits).expect("search json");
    assert_eq!(hits["ok"], true);
    assert_eq!(
        hits["results"][0]["reference"], "John 3:16",
        "search did not surface the reference the preacher typed"
    );

    // Tapping the result fires it — through the real pipeline, onto the wall.
    let fired = super::remote_api(&h, "fire?ref=John%203:16");
    let fired: serde_json::Value = serde_json::from_str(&fired).expect("fire json");
    assert_eq!(fired["ok"], true);
    assert_eq!(fired["live"]["reference"], "John 3:16");
    settle();
    let shown = wall
        .last()
        .expect("the remote's fire never reached the wall");
    assert_eq!(shown["reference"], "John 3:16");
    assert!(
        !shown["template_id"].is_null(),
        "remote fire dropped the template"
    );

    // The remote's Next walks the staged passage, same as the console's transport.
    let nexted = super::remote_api(&h, "next");
    let nexted: serde_json::Value = serde_json::from_str(&nexted).expect("next json");
    assert_eq!(nexted["ok"], true);
    assert_eq!(
        nexted["live"]["reference"], "John 3:17",
        "the remote's Next did not walk to the next verse"
    );
    assert_eq!(
        nexted["nav"]["kind"], "fired",
        "the remote did not say WHICH outcome its Next had"
    );
}

/// THE REMOTE MUST SAY WHY NOTHING MOVED — the same repair `NavResult` was built
/// for, applied to the surface it was never applied to.
///
/// `nav` used to return `()`, so an operator pressed Next mid-sermon, the wall did
/// not change, and nothing anywhere said why. That was fixed for the console and
/// left standing on the preacher's phone: `remote_api` matched `Ok(_)` and threw
/// the outcome away, so the end of a reading answered `{"ok":true}` exactly like a
/// successful step. `Stage.svelte`'s only handler was a `catch`, which fires on a
/// transport error and never on this — so the preacher tapped Next at the end of a
/// reading and got silence, which is the original bug verbatim.
///
/// Every non-firing outcome must be nameable by the phone.
#[test]
fn the_remote_says_which_outcome_its_nav_had_not_merely_ok() {
    let app = app();
    let h = app.handle().clone();

    // Nothing staged at all — stepping has nowhere to go, and must say so.
    let cold = super::remote_api(&h, "next");
    let cold: serde_json::Value = serde_json::from_str(&cold).expect("next json");
    assert_eq!(cold["ok"], true, "a boundary is not a transport failure");
    assert_eq!(
        cold["nav"]["kind"], "no_passage",
        "the remote reported ok with nothing staged, and named no outcome"
    );

    // Stage a passage, then walk off the end of the BOOK. Jude has one chapter and
    // 25 verses, so 25 is the last verse there is.
    let fired = super::remote_api(&h, "fire?ref=Jude%2025");
    let fired: serde_json::Value = serde_json::from_str(&fired).expect("fire json");
    assert_eq!(fired["ok"], true);
    assert_eq!(fired["live"]["reference"], "Jude 1:25");

    let past = super::remote_api(&h, "next");
    let past: serde_json::Value = serde_json::from_str(&past).expect("next json");
    assert_eq!(past["ok"], true);
    assert_ne!(
        past["nav"]["kind"], "fired",
        "the remote claimed it advanced past the last verse of Jude"
    );
    assert_eq!(
        past["live"]["reference"], "Jude 1:25",
        "the wall moved when the remote had nowhere to move to"
    );
}

impl NavResult {
    /// For test failure messages only.
    fn kind(&self) -> &'static str {
        match self {
            NavResult::Fired { .. } => "Fired",
            NavResult::EndOfPassage => "EndOfPassage",
            NavResult::NoPassage => "NoPassage",
            NavResult::NotInLibrary { .. } => "NotInLibrary",
        }
    }
}
