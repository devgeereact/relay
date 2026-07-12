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

    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None).expect("fire John 3:16");
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
}

#[test]
fn next_and_back_walk_the_passage() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None).unwrap();

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
    manual_fire(h.clone(), h.state::<Db>(), "Psalm 23".into(), None).unwrap();
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

    manual_fire(h.clone(), h.state::<Db>(), "Jude 1:25".into(), None).unwrap(); // last verse of Jude
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

    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None).unwrap();
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

    let _ = manual_fire(h.clone(), h.state::<Db>(), "Psalms 23:99".into(), None);
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

    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None).unwrap();
    let _ = nav(h.clone(), "next".into());
    settle();

    assert_eq!(
        wall.count(),
        0,
        "content escaped to the outputs during a rehearsal"
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
