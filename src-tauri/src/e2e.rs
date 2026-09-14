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

use super::qa::{self, settle, Wall};
use super::*;
use tauri::Manager;

/// A headless Relay with the same state `main()` manages, and a real database.
///
/// This is [`qa::bare_app`] — a genuine first launch — plus **one** deliberate
/// difference, which is the reason this wrapper exists rather than the fixture being
/// used directly.
///
/// A fresh install seeds templates but does NOT assign a per-content-type override:
/// `tpl_scripture` is only written when the operator picks one, and without it the
/// channel's own template is used (docs/DECISIONS.md). So one is picked here, which
/// is what makes the "every fire carries its template" invariant testable at all —
/// with no override set, `template_id` is legitimately None and the assertion would
/// be vacuous.
///
/// That convenience is correct for this suite and wrong for a cold-start audit. Use
/// [`qa::bare_app`] directly when the question is "can a new operator get here?".
fn app() -> tauri::App<tauri::test::MockRuntime> {
    let app = qa::bare_app();
    {
        let db = app.state::<Db>();
        let conn = db.0.lock().expect("db");
        let tpl: i64 = conn
            .query_row("SELECT id FROM templates ORDER BY id LIMIT 1", [], |r| {
                r.get(0)
            })
            .expect("a fresh install seeds templates");
        db::set_content_template(&conn, "scripture", Some(tpl)).expect("set scripture template");
    }
    app
}

/// A throwaway template, created the way the app creates one.
///
/// `create_template` was a second create path for the same table and was deleted
/// on 2026-08-30 — the gallery and the editor both go through `upsert_template`,
/// and no control ever reached the other one. This helper keeps these tests using
/// what the product actually uses.
fn scratch_template<R: tauri::Runtime>(h: &tauri::AppHandle<R>, name: &str) -> i64 {
    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    db::upsert_template(
        &conn,
        &db::Template {
            id: 0,
            name: name.into(),
            layout: serde_json::json!({ "regions": ["verse_text"] }),
            style: serde_json::json!({ "verseSize": 6 }),
            active: false,
        },
    )
    .expect("create")
}

/// RG-05 · SAFE SCREEN — an unshowable cue leaves the wall alone, and says so.
///
/// Driven through `fire_content`, the real command the Planner and the Library use.
/// The claim is the one that matters: a refused payload must leave the previous
/// content exactly where it was, and must NOT be followed by anything telling the
/// operator it went out.
#[test]
fn an_unshowable_cue_is_refused_and_the_wall_is_left_alone() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    // Something real on the wall first, so "left alone" has something to mean.
    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None).expect("fire");
    settle();
    assert_eq!(
        wall.last().expect("nothing on the wall")["reference"],
        "John 3:16"
    );
    let before = wall.count();

    // A cue with no label, no text and no media: it would paint an empty screen
    // while the console reported a successful fire.
    let err = fire_content(
        h.clone(),
        h.state::<Db>(),
        "   ".into(),
        "  \n ".into(),
        "announce".into(),
        None,
        None,
    )
    .expect_err("an empty cue must not reach a congregation");
    let msg = err.to_string();
    assert!(
        msg.contains("empty screen"),
        "must say what it means for the wall: {msg}"
    );

    settle();
    assert_eq!(
        wall.count(),
        before,
        "a refused cue must not send anything at all"
    );
    assert_eq!(
        wall.last().expect("the wall")["reference"],
        "John 3:16",
        "the previous content must still be up — clearing it would be worse than refusing"
    );

    // A real announcement still goes out. The gate refuses the broken payload, not
    // the feature.
    fire_content(
        h.clone(),
        h.state::<Db>(),
        "Car park".into(),
        "Please move the blue Fiesta".into(),
        "announce".into(),
        None,
        None,
    )
    .expect("a real announcement must still fire");
    settle();
    assert_eq!(
        wall.last().expect("the wall")["text"],
        "Please move the blue Fiesta"
    );
}

/// A CUE WHOSE TEMPLATE CANNOT BE READ IS REFUSED, NOT SILENTLY RE-LOOKED.
///
/// The output page does not fail loudly on a broken template — it falls back. So
/// the wall shows the right words in the wrong look and nobody is told, which is
/// the same silence the panic-control rule exists to end.
#[test]
fn a_cue_carrying_a_broken_template_does_not_reach_the_wall() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    // `fire_media` and `fire_content` take a template ID, not JSON, so the broken
    // JSON is injected the only way a real one can arrive: through the payload the
    // choke point sees.
    let bad = channels::OutputContent {
        kind: Some("announce".into()),
        reference: "Notice".into(),
        text: Some("The hall is open".into()),
        template_json: Some("{\"regions\": ".into()),
        template_pinned: true,
        ..Default::default()
    };
    assert!(
        broadcast_with_clock(&h, bad).is_err(),
        "a template the output page cannot parse must not go out"
    );
    settle();
    assert_eq!(wall.count(), 0, "nothing reached the wall");
}

/// RG-04 · THE SERVICE TIMELINE — what happened, kept past the end of the app.
///
/// Driven through the real commands. The claim is not "the table exists" but "a
/// service that actually ran can be reconstructed afterwards" — which is the only
/// version of this that helps a church say *"the projector was blank for a bit,
/// when?"* three days later.
#[test]
fn a_service_records_what_happened_and_it_survives_the_service() {
    let app = app();
    let h = app.handle().clone();

    let svc = start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Sunday Service".into(),
        "2026-08-29".into(),
    )
    .expect("start");

    // Things an operator does, and one thing Relay notices about itself.
    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None).expect("fire");
    settle();
    clear_screens(h.clone()).expect("clear");
    set_service_lock(h.clone(), h.state::<servicelock::ServiceLock>(), false);
    end_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<servicelock::ServiceLock>(),
    )
    .expect("end");

    let rows = service_timeline(h.state::<Db>(), svc).expect("timeline");
    let kinds: Vec<&str> = rows.iter().map(|r| r.kind.as_str()).collect();

    assert_eq!(
        rows.first().map(|r| r.kind.as_str()),
        Some("service_started")
    );
    assert_eq!(rows.last().map(|r| r.kind.as_str()), Some("service_ended"));
    assert!(
        kinds.contains(&"lock_lifted"),
        "the override belongs in the record: {kinds:?}"
    );
    assert!(
        kinds.contains(&"clear_screens"),
        "the operator's own actions merge in from `cues`: {kinds:?}"
    );
    // A human's fire is recorded AS a human's — never as the AI's. The router
    // learns from that column, and a replay that confused the two would be
    // describing a different service.
    assert!(
        rows.iter()
            .any(|r| r.source == "detection" && r.kind == "manual"),
        "a manual fire must appear as manual: {rows:?}"
    );

    // Ordered, and every row still knows which store it came from.
    assert!(
        rows.windows(2).all(|w| w[0].at_ms <= w[1].at_ms),
        "the timeline must be in time order"
    );

    // Nothing the preacher said is in it. This is the part of the history most
    // likely to be sent to somebody.
    let dump = format!("{rows:?}");
    assert!(
        !dump.contains("For God so loved"),
        "no verse text in the timeline"
    );

    // And the SECOND service keeps its own record rather than continuing the first.
    let next = start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Evening".into(),
        "2026-08-29".into(),
    )
    .expect("start again");
    assert_ne!(next, svc);
    assert_eq!(
        service_timeline(h.state::<Db>(), next)
            .expect("timeline")
            .len(),
        1,
        "a new service starts a new record"
    );
}

/// R4-09 · the self-calibrating gate must learn from what was ACCEPTED.
///
/// `confirm_detection` used to receive only the reference string, re-parse it, and
/// feed that parse's confidence to `record_feedback`. Every canonical "Book C:V"
/// re-parses to the same number, and `record_feedback` only corrects when the
/// confidence is BELOW the auto-fire bar — so the confirm arm of the calibration
/// could never fire. The gate advertised itself as self-calibrating and, on the
/// confirm side, was not.
///
/// Driven through the real command, because the defect was never in the router:
/// `router.rs::confirming_a_suggestion_lowers_the_auto_bar_toward_it` passed the
/// whole time by calling `record_feedback` directly. The bug was one call site up.
#[test]
fn confirming_a_suggestion_teaches_the_gate_what_was_accepted() {
    let app = app();
    let h = app.handle().clone();

    let before = {
        let r = h.state::<Routing>();
        let g = r.0.lock().unwrap();
        g.thresholds()
    };

    // The operator accepts a suggestion that only ever reached them AS a
    // suggestion — it scored below the auto-fire bar. That is evidence the bar
    // sits too high, and it is the only kind of evidence this arm can act on.
    let low = before.auto_fire - 0.20;
    let after = confirm_detection(
        h.clone(),
        h.state::<Db>(),
        h.state::<Routing>(),
        h.state::<channels::Rehearsal>(),
        "John 3:16".into(),
        Some(low),
        Some("direct".into()),
    )
    .expect("accepting a real suggestion fires");
    settle();

    assert!(
        after.auto_fire < before.auto_fire,
        "the bar did not move: {} -> {} after confirming at {low} — the confirm arm \
         of the self-calibrating gate is still learning a re-parsed constant",
        before.auto_fire,
        after.auto_fire
    );
    assert!(
        after.auto_fire > low,
        "it moved TOWARD what was accepted, not onto it — one confirmation is not \
         a new baseline"
    );
}

/// …and a PARAPHRASE carries no number into the gate.
///
/// A semantic "confidence" is a raw cosine — a distance in an arbitrary vector
/// space, not a probability (rule 10). Confirming one is still a confirmation; it
/// simply cannot teach the auto-fire bar where to sit, and letting it would be a
/// category error dressed up as calibration.
#[test]
fn confirming_a_paraphrase_does_not_drag_the_auto_fire_bar() {
    let app = app();
    let h = app.handle().clone();
    let before = {
        let r = h.state::<Routing>();
        let g = r.0.lock().unwrap();
        g.thresholds()
    };
    let after = confirm_detection(
        h.clone(),
        h.state::<Db>(),
        h.state::<Routing>(),
        h.state::<channels::Rehearsal>(),
        "John 3:16".into(),
        Some(before.auto_fire - 0.30),
        Some("semantic".into()),
    )
    .expect("accepting a paraphrase still fires it");
    settle();
    assert!(
        after.auto_fire >= before.auto_fire,
        "a cosine moved the auto-fire bar: {} -> {}",
        before.auto_fire,
        after.auto_fire
    );
}

/// FIELD F-2 · a detection must point at the words it actually read.
///
/// Only FINAL transcripts are persisted, and a detection born in a PARTIAL window
/// used to be attached to whatever final happened to be last. In a real service
/// that put a verse next to a sentence containing no book, no number and no
/// keyword: 72 finals in that service contained "verse", "chapter" or "bible"
/// exactly zero times, while the detections' own `heard_text` contained all three.
///
/// Every history and replay surface is built on `detections -> transcripts`, so
/// every one of them was reporting a sentence that did not produce the verse
/// beside it.
#[test]
fn a_detection_points_at_the_words_that_produced_it() {
    let app = app();
    let h = app.handle().clone();
    let db = h.state::<Db>();

    let svc = start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Sunday Service".into(),
        "2026-08-30".into(),
    )
    .expect("start_service");

    // A final transcript lands: this is what `last_transcript` will point at, and
    // it says nothing about any verse.
    persist_transcript(&h, "I left, aha, yesterday you back, did you see?", "en");

    // …then a fire whose window said something else entirely.
    manual_fire(h.clone(), h.state::<Db>(), "Psalm 23".into(), None, None).expect("fire");
    settle();

    let conn = db.0.lock().unwrap();
    let mut st = conn
        .prepare(
            "SELECT t.text FROM detections d JOIN transcripts t ON t.id = d.transcript_id
             WHERE t.service_id = ?1 ORDER BY d.id DESC LIMIT 1",
        )
        .unwrap();
    let text: String = st
        .query_row([svc], |r| r.get(0))
        .expect("a detection exists");
    assert_ne!(
        text, "I left, aha, yesterday you back, did you see?",
        "the detection is hanging off a sentence that did not produce it — the \
         exact shape of FIELD F-2"
    );
}

/// RG-03 · SERVICE LOCK — held back, but never in the operator's way.
///
/// Driven through the real commands, on a real database, exactly as the frontend
/// drives them. Two claims, and the second is the one that matters more:
///
/// 1. Starting a service holds back the irreversible things, with a message that
///    names the action and says how to proceed.
/// 2. **It cannot touch anything used to run the service.** A lock that could
///    refuse a blackout would be a lock that can hurt a congregation, so the fire
///    path, the transport and both panic controls are exercised here WHILE the lock
///    is engaged — a unit test asserting "these names are not on the list" proves
///    the list, not the wiring.
#[test]
fn a_recorded_service_holds_back_a_deletion_but_never_the_wall() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);
    let lock = h.state::<servicelock::ServiceLock>();

    // A template nobody minds losing, created before the service starts.
    let doomed = scratch_template(&h, "Scratch");
    assert!(!lock.engaged(), "a fresh app is not protecting anything");

    let svc = start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Sunday Service".into(),
        "2026-08-29".into(),
    )
    .expect("start service");
    assert!(svc > 0);
    assert!(lock.engaged(), "recording a service arms the lock");

    // 1 · The irreversible thing is refused, and the refusal is usable.
    let err = delete_template(
        h.state::<Db>(),
        h.state::<servicelock::ServiceLock>(),
        doomed,
    )
    .expect_err("deleting a template mid-service must be held back");
    let msg = err.to_string();
    assert!(
        msg.contains("deleting a template"),
        "must name the action: {msg}"
    );
    assert!(msg.contains("unlock"), "must say how to proceed: {msg}");
    // …and it really did not happen.
    assert!(
        get_template(h.state::<Db>(), doomed).is_ok(),
        "the template must still be there"
    );

    // 2 · Everything the operator runs a service with still works, right now.
    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None)
        .expect("the operator must always be able to fire");
    settle();
    assert_eq!(
        wall.last().expect("nothing reached the wall")["reference"],
        "John 3:16"
    );

    nav(h.clone(), "next".into()).expect("the transport must still walk");
    settle();

    clear_screens(h.clone()).expect("CLEAR must work while a service is locked");
    blackout(h.clone()).expect("BLACKOUT must work while a service is locked");
    settle();

    // 3 · The person in the room outranks the lock.
    assert!(!set_service_lock(
        h.clone(),
        h.state::<servicelock::ServiceLock>(),
        false
    ));
    delete_template(
        h.state::<Db>(),
        h.state::<servicelock::ServiceLock>(),
        doomed,
    )
    .expect("an operator who lifts the lock may delete");

    // 4 · …and the override is scoped to the service it was made in.
    end_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<servicelock::ServiceLock>(),
    )
    .expect("end");
    let again = scratch_template(&h, "Scratch 2");
    start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Evening Service".into(),
        "2026-08-29".into(),
    )
    .expect("start again");
    assert!(
        delete_template(
            h.state::<Db>(),
            h.state::<servicelock::ServiceLock>(),
            again
        )
        .is_err(),
        "an override made last service must not silently disarm Relay for the next one"
    );
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
fn r10_a_suppressed_label_is_still_in_the_service_record() {
    // The other half of the sentence above — "the label still names the cue" —
    // which was never asserted, and was not true from the run surface: Live
    // passed an empty string for a song cue, suppressing the label a second time
    // in the wrong place. The record then said "Manual override" about nothing,
    // and a Sunday report could not name what had been on the screens.
    let app = app();
    let h = app.handle().clone();

    let svc = start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Sunday".into(),
        "2026-09-13".into(),
    )
    .expect("start");

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

    let named = service_timeline(h.state::<Db>(), svc)
        .expect("timeline")
        .into_iter()
        .any(|r| r.detail.as_deref() == Some("Blessed Assurance · Verse 1"));
    assert!(
        named,
        "the cue that was fired is not named anywhere in the service record"
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
    assert!(wall.cleared(), "the screens never cleared");

    blackout(h.clone()).expect("blackout must report success");
    settle();
    assert!(wall.blacked(), "the screens never blacked out");
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
/// This test therefore subscribes to the hub itself, through `qa::Kiosk` — the
/// second door. The e2e app deliberately does not manage a `KioskHub` (headless =
/// "no LAN"), so attaching one is part of watching it.
#[test]
fn nothing_reaches_the_stage_monitor_during_a_rehearsal() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    // Not rehearsing: the stage monitor is supposed to get it. Assert that FIRST, so
    // this test cannot pass by the publish path being broken outright.
    set_stage_next(h.clone(), Some("Up next".into()), Some("John 3:16".into()));
    settle();
    let live = kiosk.next().expect("a real service must reach the stage");
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
        kiosk.silent(),
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
    let hits = super::remote_api(&h, "GET", "search?q=John%203:16").body;
    let hits: serde_json::Value = serde_json::from_str(&hits).expect("search json");
    assert_eq!(hits["ok"], true);
    assert_eq!(
        hits["results"][0]["reference"], "John 3:16",
        "search did not surface the reference the preacher typed"
    );

    // Tapping the result fires it — through the real pipeline, onto the wall.
    let fired = super::remote_api(&h, "POST", "fire?ref=John%203:16").body;
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
    let nexted = super::remote_api(&h, "POST", "next").body;
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
    let cold = super::remote_api(&h, "POST", "next").body;
    let cold: serde_json::Value = serde_json::from_str(&cold).expect("next json");
    assert_eq!(cold["ok"], true, "a boundary is not a transport failure");
    assert_eq!(
        cold["nav"]["kind"], "no_passage",
        "the remote reported ok with nothing staged, and named no outcome"
    );

    // Stage a passage, then walk off the end of the BOOK. Jude has one chapter and
    // 25 verses, so 25 is the last verse there is.
    let fired = super::remote_api(&h, "POST", "fire?ref=Jude%2025").body;
    let fired: serde_json::Value = serde_json::from_str(&fired).expect("fire json");
    assert_eq!(fired["ok"], true);
    assert_eq!(fired["live"]["reference"], "Jude 1:25");

    let past = super::remote_api(&h, "POST", "next").body;
    let past: serde_json::Value = serde_json::from_str(&past).expect("next json");
    assert_eq!(past["ok"], true);
    // Named EXACTLY, not merely "not fired". The three non-firing outcomes mean
    // three different things to a preacher holding the phone — "there is no more of
    // this reading", "you have not put anything up yet", and "that verse is not in
    // your Bible" — and a test that accepts any of them would pass just as happily
    // if the wall reported the wrong one, which is the bug `NavResult` exists to
    // prevent one layer down.
    //
    // Jude 1:25 is fired as a SINGLE verse, so the passage is unbounded: the step
    // resolves Jude 1:26, which is not in the corpus. Hence `not_in_library` here
    // and not `end_of_passage` — the bounded case is asserted below.
    assert_eq!(
        past["nav"]["kind"], "not_in_library",
        "the remote gave the wrong name to a step past the last verse of Jude"
    );
    assert_eq!(
        past["live"]["reference"], "Jude 1:25",
        "the wall moved when the remote had nowhere to move to"
    );

    // A BOUNDED reading, walked off its own end. This is the outcome a preacher
    // meets most often — the reading finished — and it must not be reported with
    // the same word as a verse that does not exist.
    let _ = super::remote_api(&h, "POST", "fire?ref=John%203:16-17").body;
    let step: serde_json::Value =
        serde_json::from_str(&super::remote_api(&h, "POST", "next").body).expect("next json");
    assert_eq!(step["nav"]["kind"], "fired", "precondition: 3:16 -> 3:17");
    assert_eq!(step["live"]["reference"], "John 3:17");

    let end: serde_json::Value =
        serde_json::from_str(&super::remote_api(&h, "POST", "next").body).expect("next json");
    assert_eq!(
        end["ok"], true,
        "the end of a reading is not a transport failure"
    );
    assert_eq!(
        end["nav"]["kind"], "end_of_passage",
        "the remote did not name the end of a bounded reading"
    );
    assert_eq!(
        end["live"]["reference"], "John 3:17",
        "the wall moved past the end of the reading"
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

// ════════════════════════════════════════════════════════════════════════════
// R2 · LIVE PATH AUDIT — evidence, 2026-08-14
//
// Four findings, each pinned as a test asserting the CORRECT behaviour, following
// the precedent in `src/lib/liveoutputrail.test.js`: a known defect is pinned as a
// skipped test with the repair written out, so the fix has a target and CI stays
// green until a human chooses one.
//
// **Two of the four have since been fixed (R2-A and R2-B), and their tests are no
// longer `#[ignore]`d** — they run on every `cargo test` and now guard the repair
// instead of describing the defect. An ignored test that has started passing is
// worse than no test: it protects nothing while reading, in its own reason string,
// as an open bug.
//
// **R2-C and R2-D were closed too** (DECISIONS §54, CLAUDE.md rule 38), so this
// file now carries NO ignored test — `cargo test e2e::` runs all of them. The
// sentence here used to say the two "remain open, remain RED, and remain ignored"
// and stayed that way after they were fixed; `grep -c '#\[ignore\]'` on this file
// answers the question the sentence was answering badly.
//
// All four are the SAME class of bug this repo has now had four times: a rule
// enforced on one surface and skipped on its twin.
// ════════════════════════════════════════════════════════════════════════════

/// R2-A · `/api/live` REPORTS A VERSE ON A WALL THAT IS CLEAR.
///
/// `live_json` reads the CONTEXT's current passage anchor, not what is on the
/// screens. The context deliberately survives a clear — that is what makes the
/// next `→` resume the passage instead of restarting it (CLAUDE.md: position and
/// on-air-ness are separate facts). But the remote publishes that position under
/// the key `live`, so the preacher's control plane answers "John 3:16 is up"
/// after the operator has hit Esc, and again after a blackout.
///
/// This is Cued ≠ On Air, violated on the one surface where the operator cannot
/// see the wall to check.
///
/// It also means the existing test `the_remote_says_which_outcome_its_nav_had…`
/// asserts on the wrong surface: its `past["live"]["reference"] == "Jude 1:25"`
/// check reads "the wall did not move", and would pass just as happily if the
/// wall had been cleared.
///
/// **FIXED, and this test now guards the repair.** `live_json` reports what the
/// outputs are showing rather than where the playhead is, so a cleared wall answers
/// `live: null` while the context keeps the position for the next `→`. It ran
/// `#[ignore]`d and RED while the defect stood; leaving the ignore on after the fix
/// would have left the repair unprotected and the reason string lying about it.
#[test]
fn r2_the_remote_must_not_call_a_cleared_wall_live() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    let _ = super::remote_api(&h, "POST", "fire?ref=John%203:16").body;
    settle();
    let up: serde_json::Value =
        serde_json::from_str(&super::remote_api(&h, "GET", "live").body).unwrap();
    assert_eq!(up["live"]["reference"], "John 3:16", "precondition");

    // The operator clears the screens. The wall really does go blank.
    let cleared: serde_json::Value =
        serde_json::from_str(&super::remote_api(&h, "POST", "clear").body).unwrap();
    assert_eq!(cleared["ok"], true);
    settle();
    assert!(wall.cleared(), "precondition: the wall actually cleared");

    let after: serde_json::Value =
        serde_json::from_str(&super::remote_api(&h, "GET", "live").body).unwrap();
    assert!(
        after["live"].is_null(),
        "the remote told the preacher {} is on the wall, and the wall is empty",
        after["live"]["reference"]
    );
}

/// R2-B · A REHEARSAL FIRE ANSWERS THE PHONE AS THOUGH IT WENT OUT.
///
/// Rehearsal is gated at the four kiosk publishers, and it holds: `wall.count()`
/// is 0 and the hub is silent. The HTTP control plane is a fifth door, and it is
/// not a publisher — it is a REPORTER — so nobody enumerated it. `/api/fire`
/// during a rehearsal returns `{"ok":true,"live":{"reference":"John 3:16",…}}`,
/// which is indistinguishable from the answer it gives during a service.
///
/// The failure is the quiet kind, exactly like `stage_next` was: nothing escapes,
/// so the sandbox looks intact — while the preacher, holding the phone during a
/// Thursday rehearsal, is told the congregation's wall has John 3:16 on it.
///
/// **FIXED, and this test now guards the repair.** The remote's answer carries the
/// sandbox the way every console surface does, so the phone can no longer report a
/// rehearsal fire as though the congregation saw it. It ran `#[ignore]`d and RED
/// while the defect stood.
#[test]
fn r2_the_remote_must_say_a_rehearsal_fire_reached_nobody() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);

    set_rehearsal(
        h.clone(),
        h.state::<Session>(),
        h.state::<channels::Rehearsal>(),
        true,
    )
    .expect("enter rehearsal");

    let fired: serde_json::Value =
        serde_json::from_str(&super::remote_api(&h, "POST", "fire?ref=John%203:16").body).unwrap();
    settle();

    // Containment itself is intact — assert that first, so this test cannot be
    // read as a leak.
    assert_eq!(wall.count(), 0, "containment held on the wall");
    assert!(kiosk.silent(), "containment held on the kiosk");

    assert!(
        fired["rehearsing"] == true || fired["live"].is_null(),
        "the remote answered {fired} — identical to a real fire, during a rehearsal"
    );
}

/// R2-C · THE SPOKEN IN-PASSAGE JUMP IS THE FOURTH SILENT NO-OP.
///
/// `NavResult` exists because `nav` used to return `()` and do nothing. The
/// console was repaired, then the remote (`Ok(_)`), and both are now covered.
/// `handle_passage_nav` — the spoken "chapter five verse one" / "verse four" —
/// still returns a bare `bool`, and the STT callback discards it:
///
///     if handle_passage_nav(&handle, &update.text) { return; }
///
/// A `false` means one of three things and says none of them: the context lock
/// was poisoned, nothing is staged so there is no book to resolve against, or
/// the verse parsed and is not in the corpus. Its sibling two lines above emits
/// `nav://blocked` for exactly these cases.
///
/// So the preacher says "verse four" before anything has been fired, the wall
/// does not move, and there is no toast, no banner and no log line — which is
/// the original bug, verbatim, on the fourth door.
///
/// Repair direction: return a `NavResult` (or fold the jump into `handle_nav`)
/// and emit `nav://blocked` for every non-firing outcome.
#[test]
fn r2_a_spoken_passage_jump_that_cannot_move_must_say_so() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    // A blocked-nav notice is pushed to the operator as an event, because the STT
    // thread has no caller to return to. Count them.
    let blocked = std::sync::Arc::new(AtomicBool::new(false));
    let b = blocked.clone();
    tauri::Listener::listen(&h, "nav://blocked", move |_| {
        b.store(true, Ordering::SeqCst)
    });

    // "verse ninety nine" parses to a real jump target …
    assert!(
        super::detection::detect_passage_nav("verse ninety nine").is_some(),
        "precondition: the phrase is understood as a jump"
    );
    manual_fire(h.clone(), h.state::<Db>(), "Psalm 23".into(), None, None).unwrap();
    settle();
    let before = wall.count();

    // … but Psalm 23 has six verses, so it cannot be fired.
    let handled = super::handle_passage_nav(&h, "verse ninety nine");
    settle();

    assert!(
        handled.is_some(),
        "precondition: it was recognised as a jump"
    );
    assert_eq!(
        wall.count(),
        before,
        "precondition: the wall was left alone"
    );
    assert!(
        blocked.load(Ordering::SeqCst),
        "the wall did not move and nothing anywhere said why — `nav` would have \
         reported NotInLibrary for the same target"
    );
}

/// R2-D (backend half) · THE STAGED PASSAGE OUTLIVES EVERYTHING THAT REPLACES IT.
///
/// `Context` is only written by scripture fires (`PassageUpdate`). A song, a
/// notice, a picture, a countdown and a blackout all leave it exactly as it was,
/// forever — so `nav` will happily walk a passage that left the wall an hour ago
/// and report `Fired`, which is true of the wall and false of the sermon.
///
/// That is harmless while the console is in SLIDE mode. It stops being harmless
/// when the console flips to VERSE mode without the operator asking, and it does:
/// `Live.svelte:124` reads `mode = … !($live && !planOnAir) ? 'slide' : 'verse'`,
/// and a BLACKOUT clears `planOnAir` (panicRun → leavePlan) while leaving `$live`
/// set (only `output://clear` nulls it). Esc → SLIDE, B → VERSE, from the same
/// state, and only the Esc behaviour is documented.
///
/// So: running a plan, operator hits B to kill the wall, then presses `→` to pick
/// the service back up. They get the next verse of a passage from earlier in the
/// service, on a wall they had just blacked out.
///
/// This test asserts only the backend half, which is the part layer A can see.
#[test]
fn r2_a_passage_must_not_stay_armed_under_unrelated_content() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    // Sermon scripture, twenty minutes ago.
    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None).unwrap();
    // …then the closing song takes the wall. Nothing scripture-shaped since.
    fire_content(
        h.clone(),
        h.state::<Db>(),
        "Blessed Assurance · Verse 1".into(),
        "Blessed assurance, Jesus is mine".into(),
        "song".into(),
        None,
        None,
    )
    .unwrap();
    settle();
    assert!(
        wall.last().unwrap()["text"]
            .as_str()
            .unwrap_or("")
            .contains("Blessed assurance"),
        "precondition: the song is what is on the wall"
    );

    let r = nav(h.clone(), "next".into()).unwrap();
    settle();
    assert!(
        matches!(r, NavResult::NoPassage),
        "`next` walked a passage the congregation stopped looking at: {} — the wall \
         now shows {:?}",
        r.kind(),
        wall.last().unwrap()["reference"]
    );
}

// ── THE AUTO-FIRE PATH ──────────────────────────────────────────────────────
//
// Everything above drives a HUMAN path: `manual_fire`, `nav`, `clear_screens`.
// Until 2026-08-14 that was every e2e test in the file, and it meant the one path
// where **the AI decides on its own** — transcript in, scripture on a wall, nobody
// pressing anything — had never been driven end to end by anything.
//
// That was not an oversight anybody could see, because it looked like coverage.
// `emit_detections` took a concrete `tauri::AppHandle`, so it *could not* be driven
// on the mock runtime; architecture rule 24 predicts exactly this ("a concrete
// AppHandle quietly re-welds it") and it had happened to the most dangerous function
// in the product. It is now generic, and these are the first tests through it.
//
// The P0 that shipped in the meantime — "please turn to hymn number three sixteen"
// putting Numbers 3:16 on a wall, unattended — is the reason to be specific about
// what this file is for: not "does detection work" (detection.rs owns that) but
// "what leaves the machine when nobody is watching".

/// A heard reference in an ordinary sentence reaches the congregation by itself.
///
/// The positive control. Without it the three negative tests below could all pass
/// by the auto-fire path being broken outright, which is the failure mode a
/// suppression test cannot distinguish from success.
#[test]
fn a_spoken_reference_auto_fires_all_the_way_to_the_wall() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    emit_detections(
        &h,
        "turn with me to John chapter three verse sixteen",
        0,
        true,
        None,
    );
    settle();

    let shown = wall
        .last()
        .expect("a heard reference never reached the outputs");
    assert_eq!(shown["reference"], "John 3:16");
    assert!(
        shown["text"]
            .as_str()
            .unwrap_or("")
            .contains("God so loved"),
        "the verse arrived without its text: {:?}",
        shown["text"]
    );
}

/// THE P0 OF 2026-08-14, pinned end to end rather than at the router.
///
/// `r6_11`/`r6_12` prove the router refuses these. This proves nothing reaches the
/// wall — which is a different claim, and the only one a congregation experiences.
#[test]
fn ordinary_church_announcements_reach_nobody() {
    let announcements = [
        "please turn to hymn number three sixteen",
        "we will sing hymn number one one",
        "the youth meet in room two twelve after the service",
        "the crèche is in room one one for under fives",
        "there are free seats on row three sixteen",
        "we will sing song two twelve this morning",
        "welcome to our nine thirty service",
    ];
    for text in announcements {
        let app = app();
        let h = app.handle().clone();
        let wall = Wall::watch(&h);

        emit_detections(&h, text, 0, true, None);
        settle();

        assert_eq!(
            wall.count(),
            0,
            "{text:?} put {:?} in front of a congregation with nobody pressing anything",
            wall.last().map(|v| v["reference"].clone())
        );
    }
}

/// The gate, through the real path this time. `router.rs` proves `decide` refuses a
/// paraphrase; `e2e` proves the refusal survives everything between `decide` and the
/// projector.
#[test]
fn a_paraphrase_never_auto_fires_through_the_real_transcript_path() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    // Words from John 3:16 with no reference spoken — the semantic index's job, and
    // the one thing it may never do unattended.
    emit_detections(
        &h,
        "because God loved the world so much that he gave his only son",
        0,
        true,
        None,
    );
    settle();

    assert_eq!(
        wall.count(),
        0,
        "a paraphrase reached the congregation on its own: {:?}",
        wall.last()
    );
}

/// Rehearsal contains the AI, not just the operator.
///
/// Every previous rehearsal test drove a human path. This one lets the machine
/// decide during a rehearsal — the case where a preacher is practising, says a real
/// reference out loud, and the sandbox has to hold with nobody watching it.
#[test]
fn nothing_the_ai_decides_escapes_a_rehearsal() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);

    set_rehearsal(
        h.clone(),
        h.state::<Session>(),
        h.state::<channels::Rehearsal>(),
        true,
    )
    .expect("enter rehearsal");

    emit_detections(
        &h,
        "turn with me to John chapter three verse sixteen",
        0,
        true,
        None,
    );
    settle();

    assert_eq!(
        wall.count(),
        0,
        "the AI put {:?} on the congregation's wall during a rehearsal",
        wall.last().map(|v| v["reference"].clone())
    );
    assert!(
        kiosk.silent(),
        "the AI's fire escaped to the kiosk hub during a rehearsal — the second door"
    );
}

/// ACCEPTING A SUGGESTION REPORTS WHAT IT ACTUALLY DID.
///
/// `confirm_detection` returned `Ok(thresholds)` on two paths that put nothing on
/// any screen: an unparseable reference (the `if let` fell through) and a verse
/// outside the corpus (`fire_manual`'s bool was discarded — no binding, no `if`).
///
/// The second is reachable in ordinary use. `emit_detections` deliberately demotes
/// a parsed-but-absent verse to a suggestion rather than dropping it, and no
/// frontend file reads the `in_library` flag that says so — so a garbled
/// "Psalms 23:99" looked like any other card, Accept was enabled, the backend said
/// Ok, and the console flashed "Now live: Psalms 23:99" over the verse that was
/// still on the wall.
///
/// These tests are only possible because P1-10 made the command generic over `R`.
#[test]
fn accepting_a_suggestion_that_cannot_fire_says_so() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    // Precondition: a real acceptance works, so the two refusals below cannot pass
    // by the command being broken outright.
    confirm_detection(
        h.clone(),
        h.state::<Db>(),
        h.state::<Routing>(),
        h.state::<channels::Rehearsal>(),
        "John 3:16".into(),
        None,
        None,
    )
    .expect("accepting a real suggestion must fire");
    settle();
    assert_eq!(
        wall.last().expect("nothing reached the wall")["reference"],
        "John 3:16"
    );
    let after_real = wall.count();

    // A verse that parsed but does not exist. Psalms 23 has six verses.
    let err = confirm_detection(
        h.clone(),
        h.state::<Db>(),
        h.state::<Routing>(),
        h.state::<channels::Rehearsal>(),
        "Psalms 23:99".into(),
        None,
        None,
    )
    .expect_err("accepting a verse outside the corpus must NOT report success");
    assert!(
        err.to_string().contains("isn't in the Bible text"),
        "the operator needs the same sentence manual_fire gives them, got: {err}"
    );

    // Nothing new left the machine, and — the part that mattered — the previous
    // verse is untouched. The console's "Now live" flash was over a wall that had
    // never changed.
    settle();
    assert_eq!(
        wall.count(),
        after_real,
        "a refused accept still broadcast something"
    );
    assert_eq!(wall.last().unwrap()["reference"], "John 3:16");

    // And a reference the parser cannot read at all.
    let err = confirm_detection(
        h.clone(),
        h.state::<Db>(),
        h.state::<Routing>(),
        h.state::<channels::Rehearsal>(),
        "the pastor's third point".into(),
        None,
        None,
    )
    .expect_err("an unreadable reference must NOT report success");
    assert!(
        err.to_string().contains("could not read a reference"),
        "got: {err}"
    );
    settle();
    assert_eq!(wall.count(), after_real);
}

// ════════════════════════════════════════════════════════════════════════════
// ONE WINDOW, ONE WALL — evidence from a real service, 2026-08-23.
//
// 58 broadcasts reached the congregation's screens in 45 minutes and the wall
// visibly flickered. Both causes are pinned below with the phrasing that produced
// them, taken verbatim from the transcript.
// ════════════════════════════════════════════════════════════════════════════

/// "X chapter N and verse M" must put ONE verse on the wall, not two.
///
/// Live, this fired `1 Corinthians 9:1` alongside `9:24` — the chapter-only reading
/// resolves to verse 1, scores the same 0.88, and has a different key, so neither
/// the debounce nor the corroboration rule could see it. The wall showed
/// 9:24 -> 9:1 -> 9:24 over six seconds. The same shape produced 2 Chronicles 15:1
/// and 26:1, Proverbs 3:1, Isaiah 61:1, Hebrews 6:1, Genesis 12:1 and Psalms 23:1
/// in the same service.
#[test]
fn a_chapter_and_verse_reference_does_not_also_fire_verse_one() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    super::emit_detections(
        &h,
        "The Bible says in 1 Corinthians chapter 9 and verse 24. It says,",
        1_000,
        true,
        None,
    );
    settle();

    let seen = wall.references();
    assert!(
        !seen.iter().any(|r| r == "1 Corinthians 9:1"),
        "verse 1 reached the wall beside the verse that was actually named: {seen:?}"
    );
    assert_eq!(
        seen.iter()
            .filter(|r| r.starts_with("1 Corinthians"))
            .count(),
        1,
        "one utterance, one verse on the wall — got {seen:?}"
    );
}

/// Two unrelated references in ONE window may inform the operator, but only one of
/// them may reach a wall.
///
/// Live, `Matthew 13:10` and `2 Chronicles 15:1` fired at the same timestamp to the
/// tenth of a second. A wall can only show one thing, so the second was not
/// information — it erased the first before anybody could read it.
#[test]
fn two_references_in_one_window_put_one_verse_on_the_wall() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    super::emit_detections(
        &h,
        "as it says in Matthew 13:10, and again in 2 Chronicles 15:12,",
        1_000,
        true,
        None,
    );
    settle();

    assert_eq!(
        wall.count(),
        1,
        "one window put {} verses on the wall: {:?}",
        wall.count(),
        wall.references()
    );
}

// ── THE INSTRUMENT ITSELF ─────────────────────────────────────────────────────
//
// A latency report is only as true as its wiring, and wiring is exactly the class
// of thing this repository keeps finding broken with every test still green: a
// rule enforced on one surface and skipped on its twin (CLAUDE.md, "a guarantee is
// only kept on the doors you checked"). An instrument that silently stops
// reporting does not look broken — it looks fast, because the samples that would
// have been slow are the ones that stopped arriving.
//
// So these drive the real fire path and assert that the pass which HEARD a verse
// is still attached to it when it leaves the machine.

/// The verse that reaches the wall carries the decode pass that heard it.
///
/// Without this, `output_rendered_at` can never be attributed and the last leg —
/// fire sent to pixels on a projector — quietly stops being measured. That is the
/// leg with the church's own network in it.
#[test]
fn a_verse_that_reaches_the_wall_carries_the_pass_that_heard_it() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    let trace = crate::latency::begin_pass(crate::latency::now_us(), None);
    super::emit_detections(
        &h,
        "turn with me to John chapter three verse sixteen",
        0,
        true,
        Some(trace),
    );
    settle();

    let shown = wall
        .last()
        .expect("the reference never reached the outputs");
    assert_eq!(shown["reference"], "John 3:16");
    assert_eq!(
        shown["trace_id"].as_u64(),
        Some(trace),
        "the wall content lost the pass that heard it, so its render can never be timed"
    );
}

/// A verse a HUMAN fired carries no pass, and must not.
///
/// The end-to-end percentile is a claim about how fast the AI is. An operator's
/// own keypress is not part of that path, and folding it in would flatter every
/// number in the report with the one action that never waits for a decoder.
#[test]
fn a_verse_a_human_fired_carries_no_pass() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    let r = crate::detection::VerseRef {
        book: "John".into(),
        chapter: 3,
        verse: 16,
    };
    assert!(super::fire_manual(
        &h,
        r,
        1.0,
        super::PassageUpdate::Note(None),
        None,
        None
    ));
    settle();

    let shown = wall
        .last()
        .expect("a manual fire never reached the outputs");
    assert_eq!(shown["reference"], "John 3:16");
    assert!(
        shown["trace_id"].is_null(),
        "a human's own fire was attributed to a decode pass: {:?}",
        shown["trace_id"]
    );
}

/// The two halves of the chain are measured SEPARATELY, and the report says which
/// is which.
///
/// This is the specific failure of the previous field test: one number, a healthy
/// one, and no way to tell an STT problem from a routing problem. A span that is
/// only ever reported as part of a total cannot be diagnosed.
#[test]
fn the_fire_half_of_the_chain_is_measured_on_its_own() {
    // The recorder is process-wide and tests run in parallel — see `test_lock`.
    let _recorder = crate::latency::test_lock();
    crate::latency::reset();
    let app = app();
    let h = app.handle().clone();
    let _wall = Wall::watch(&h);

    let trace = crate::latency::begin_pass(crate::latency::now_us(), None);
    crate::latency::transcript_emitted(trace, 1_000, 8_000, 1, true);
    super::emit_detections(
        &h,
        "turn with me to John chapter three verse sixteen",
        0,
        true,
        Some(trace),
    );
    settle();
    crate::latency::close(trace);

    let report = crate::latency::report(4);
    let span = |name: &str| {
        report
            .metrics
            .iter()
            .find(|m| m.metric == name)
            .map(|m| m.samples)
            .unwrap_or(0)
    };
    assert!(
        span("transcript_to_reference_detection") >= 1,
        "the parser's share of the delay is not being measured"
    );
    assert!(
        span("reference_detection_to_fire") >= 1,
        "the router and the broadcast are not being measured"
    );
}

/// Rejecting a suggestion has to leave a mark, and accepting one has to say whose
/// idea it was.
///
/// ## The defect this closes
///
/// `detections.status` permits four values and `docs/data/schema.sql`'s `CHECK`
/// enforces them, `db/services.rs` documents all four, `service_timeline` reads
/// them and the Sunday report counted them. **Two of the four were structurally
/// unreachable in production**: the only insert is inside `persist_fire`, which is
/// called for a fire that reaches a screen, so a real service can only ever write
/// `'auto'` or `'manual'`.
///
/// So the report printed `0 suggested` and `0 dismissed` for every service ever
/// run — which does not read as "nothing recorded that", it reads as *"Relay never
/// offered you anything"*. That is the inversion DECISIONS §44 exists to forbid,
/// and it was invisible because the report's own tests fed it synthetic rows the
/// product could not produce.
///
/// The fix is not to persist every suggestion: suggestions are deliberately not
/// debounced (CLAUDE.md rule 28), so one spoken paraphrase yields one per decode
/// pass. What is bounded and meaningful is what the OPERATOR did.
#[test]
fn what_the_operator_did_with_a_suggestion_reaches_the_record() {
    let app = app();
    let h = app.handle().clone();

    let svc = start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Sunday Service".into(),
        "2026-08-29".into(),
    )
    .expect("start");

    // Accept one, reject another — the two halves of an acceptance rate.
    confirm_detection(
        h.clone(),
        h.state::<Db>(),
        h.state::<Routing>(),
        h.state::<channels::Rehearsal>(),
        "John 3:16".into(),
        Some(0.91),
        Some("direct".into()),
    )
    .expect("confirm");
    settle();
    dismiss_detection(
        h.clone(),
        h.state::<Routing>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        Some("Psalms 23:1".into()),
    )
    .expect("dismiss");

    let rows = service_timeline(h.state::<Db>(), svc).expect("timeline");
    let kinds: Vec<&str> = rows.iter().map(|r| r.kind.as_str()).collect();

    assert!(
        kinds.contains(&"suggestion_accepted"),
        "taking Relay's suggestion is not the same act as typing a verse by hand, \
         and the record has to be able to tell them apart: {kinds:?}"
    );
    assert!(
        kinds.contains(&"suggestion_dismissed"),
        "a rejection used to leave no trace anywhere — not in `detections`, not in \
         `cues`, not in `service_events`: {kinds:?}"
    );

    // Both arrive as CUES. `service_events` deliberately does not duplicate what
    // `cues` already holds, and these are things the operator pressed.
    for kind in ["suggestion_accepted", "suggestion_dismissed"] {
        assert!(
            rows.iter().any(|r| r.kind == kind && r.source == "cue"),
            "{kind} belongs in cues, not in service_events"
        );
    }

    // The accepted one still fires, and still fires AS a human decision — the cue
    // records whose idea it was; it does not change who decided.
    assert!(
        rows.iter()
            .any(|r| r.source == "detection" && r.kind == "manual"),
        "confirming a suggestion is a human decision: {rows:?}"
    );

    // Still nothing a preacher said.
    let dump = format!("{rows:?}");
    assert!(
        !dump.contains("For God so loved"),
        "no verse text in the timeline"
    );
}

/// A rejection carries a canonical reference or nothing — never a sentence.
///
/// `cues.payload_json` is read back by `service_timeline`, which is the part of the
/// history most likely to be emailed to somebody. Every other cue writer satisfies
/// `db/services.rs`'s "a short phrase Relay composes" by construction — `manual_fire`
/// builds its key from an already-parsed `VerseRef`. `dismiss_detection` takes a
/// string across the bridge, so it would have been the first cue payload whose shape
/// was trusted rather than guaranteed.
#[test]
fn a_rejection_records_a_reference_and_never_a_sentence() {
    let app = app();
    let h = app.handle().clone();

    let svc = start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Sunday Service".into(),
        "2026-08-29".into(),
    )
    .expect("start");

    // A whole sentence with a reference buried in it: the reference survives,
    // canonicalised. The sentence does not.
    dismiss_detection(
        h.clone(),
        h.state::<Routing>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        Some("and beloved if you turn with me to psalm twenty three verse one".into()),
    )
    .expect("dismiss");
    // And something with no reference in it at all stores no payload — the
    // rejection is still counted; only the "which verse" is lost.
    dismiss_detection(
        h.clone(),
        h.state::<Routing>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        Some("the sermon text nobody may export".into()),
    )
    .expect("dismiss");

    let rows = service_timeline(h.state::<Db>(), svc).expect("timeline");
    let dump = format!("{rows:?}");
    assert!(
        !dump.contains("beloved") && !dump.contains("turn with me"),
        "a sentence reached the service record: {dump}"
    );
    assert!(
        !dump.contains("sermon text nobody may export"),
        "free text reached the service record: {dump}"
    );
    assert!(
        dump.contains("Psalms 23:1"),
        "the reference itself should survive, canonicalised: {dump}"
    );
    assert_eq!(
        rows.iter()
            .filter(|r| r.kind == "suggestion_dismissed")
            .count(),
        2,
        "both rejections are counted, even the one that named no verse"
    );
}

/// A rehearsal is not evidence, and that has to hold for the acceptance rate too.
///
/// `record_feedback` already refuses to learn from a rehearsal. The same reasoning
/// applies exactly to the record: a volunteer practising accepts verses they chose
/// themselves, so an acceptance rate inflated by practice would make the AI look
/// like it was earning its place when nobody had tested it.
#[test]
fn a_rehearsed_decision_is_not_counted_as_one() {
    let app = app();
    let h = app.handle().clone();

    let svc = start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Sunday Service".into(),
        "2026-08-29".into(),
    )
    .expect("start");

    h.state::<channels::Rehearsal>().set(true);
    dismiss_detection(
        h.clone(),
        h.state::<Routing>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        Some("Psalms 23:1".into()),
    )
    .expect("dismiss");
    settle();

    let kinds: Vec<String> = service_timeline(h.state::<Db>(), svc)
        .expect("timeline")
        .into_iter()
        .map(|r| r.kind)
        .collect();
    assert!(
        !kinds.iter().any(|k| k == "suggestion_dismissed"),
        "a rehearsal is not evidence: {kinds:?}"
    );
}

/// W4 acceptance — a screen set to FOLLOW renders scripture, lyrics and
/// announcements through three different templates, with nobody touching it.
///
/// `r4_a_screen_may_follow_the_content_look` proves the setting can be made and
/// is published. It does not fire anything, so it cannot see whether the map is
/// then READ — which is precisely the defect that phase closed one level down:
/// the content-look map could be filled in, saved, and change nothing. A test
/// that only checks the control passed throughout that too.
///
/// What rides is the ID and nothing else. A content-look default must never
/// serialize its template JSON: a default carrying an embedded image can be
/// megabytes (one was 13 MB) and broadcasting it on every fire made verses take
/// seconds. Only a PINNED cue template ships its JSON (CLAUDE.md, DECISIONS §29),
/// so this asserts the absence as hard as it asserts the presence.
#[test]
fn r4_a_following_screen_wears_a_different_look_for_each_kind() {
    let app = app();
    let h = app.handle().clone();
    let wall = qa::Wall::watch(&h);

    let scripture = scratch_template(&h, "Nocturne");
    let song = scratch_template(&h, "Hymnal");
    let announce = scratch_template(&h, "Noticeboard");
    {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        db::set_content_template(&conn, "scripture", Some(scripture)).expect("scripture look");
        db::set_content_template(&conn, "song", Some(song)).expect("song look");
        db::set_content_template(&conn, "announce", Some(announce)).expect("announce look");
    }

    // Nobody touches a screen between these three fires. That is the claim.
    super::manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None)
        .expect("scripture fires");
    settle();
    let a = wall.last().expect("scripture reached the wall");

    super::fire_content(
        h.clone(),
        h.state::<Db>(),
        "Verse 1".into(),
        "Great is thy faithfulness".into(),
        "song".into(),
        None,
        None,
    )
    .expect("a song fires");
    settle();
    let b = wall.last().expect("the song reached the wall");

    super::fire_content(
        h.clone(),
        h.state::<Db>(),
        "Car park".into(),
        "Please move the blue Fiesta".into(),
        "announce".into(),
        None,
        None,
    )
    .expect("an announcement fires");
    settle();
    let c = wall.last().expect("the announcement reached the wall");

    assert_eq!(a["template_id"], scripture, "scripture wears its own look");
    assert_eq!(b["template_id"], song, "a song wears its own look");
    assert_eq!(c["template_id"], announce, "a notice wears its own look");

    let ids = [&a["template_id"], &b["template_id"], &c["template_id"]];
    assert!(
        ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2],
        "three kinds, three different looks, nobody touching a screen: {ids:?}"
    );

    for (name, out) in [("scripture", &a), ("song", &b), ("announce", &c)] {
        assert!(
            out.get("template").is_none_or(|t| t.is_null()),
            "a content look rides as an ID only — {name} carried its JSON: {out}"
        );
    }
}

/// DECISIONS §70 — a screen with no look of its own follows the content look.
///
/// The defect this holds closed is not a crash and was invisible to every
/// existing test: `set_channel_template` took an `i64`, so a screen always had a
/// template, and since a screen's own template wins over a content-type default
/// (§29) the whole content-look map could be filled in and do nothing. A test
/// that only checked "assigning a template works" passed throughout.
#[test]
fn r4_a_screen_may_follow_the_content_look() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    let (chan, tpl) = {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        let ch = db::list_output_channels(&conn).expect("channels");
        let c = ch.first().expect("a fresh install seeds screens");
        (c.id, c.template_id)
    };
    assert!(
        tpl.is_some(),
        "a seeded screen starts with a look of its own"
    );

    // Set it to follow.
    super::set_channel_template(
        h.clone(),
        h.state::<Db>(),
        h.state::<channels::KioskHub>(),
        chan,
        None,
    )
    .expect("a screen may be set to follow");

    {
        let dbs = h.state::<Db>();
        let conn = dbs.0.lock().expect("db");
        let ch = db::list_output_channels(&conn).expect("channels");
        assert_eq!(
            ch.iter().find(|c| c.id == chan).and_then(|c| c.template_id),
            None,
            "the screen now has no look of its own"
        );
    }

    // CLEARING IS NEWS. A screen that is already open has to be told, or it keeps
    // wearing the look it was given until something reloads it.
    let msg = kiosk
        .next()
        .expect("clearing a screen's template is published");
    assert!(
        msg.contains("\"kind\":\"channel_template\"") && msg.contains("\"template\":null"),
        "the screens must be told the template was cleared: {msg}"
    );

    // And back again: giving it a look of its own publishes the template itself.
    super::set_channel_template(
        h.clone(),
        h.state::<Db>(),
        h.state::<channels::KioskHub>(),
        chan,
        tpl,
    )
    .expect("a screen may be given its own look again");
    let msg = kiosk.next().expect("assigning a template is published");
    assert!(
        msg.contains("\"kind\":\"channel_template\"") && !msg.contains("\"template\":null"),
        "assigning must carry the template, not a null: {msg}"
    );
}

/// A WORD TO THE PREACHER reaches the stage monitor, and a rehearsal holds it.
///
/// Two separate guarantees, and they fail in opposite directions:
///
///   - it must ARRIVE, or the operator types a message to somebody standing in
///     front of a congregation and nothing happens;
///   - it must not arrive during a REHEARSAL. The same defect `stage_next` had:
///     the congregation wall does not move, so the sandbox looks intact, while
///     the preacher's own tablet is handed a message from a practice run.
///
/// "No congregation screen can show it" is held on the other side, by
/// `r6-contracts.test.js`, which requires every hub message to have an explicit
/// per-client verdict — the output page's verdict for this one is `false`.
#[test]
fn r5_a_word_to_the_preacher_reaches_the_stage_and_not_a_rehearsal() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    // Assert arrival FIRST, so this cannot pass by the publish path being broken.
    super::send_stage_alert(h.clone(), Some("  Wrap up — 5 minutes  ".into())).expect("send");
    settle();
    let sent = kiosk
        .next()
        .expect("the stage monitor must get the message");
    assert!(
        sent.contains("\"kind\":\"stage_alert\"") && sent.contains("Wrap up — 5 minutes"),
        "the stage monitor got something else: {sent}"
    );
    assert!(
        !sent.contains("  Wrap up"),
        "a line is trimmed before it is 8.5cqw across somebody's monitor: {sent}"
    );

    // Blank clears rather than painting a red screen with nothing on it.
    super::send_stage_alert(h.clone(), Some("   ".into())).expect("clear");
    settle();
    let cleared = kiosk.next().expect("clearing is also a message");
    assert!(
        cleared.contains("\"kind\":\"stage_alert\"") && cleared.contains("\"text\":null"),
        "whitespace must clear the alert, not send it: {cleared}"
    );

    set_rehearsal(
        h.clone(),
        h.state::<Session>(),
        h.state::<channels::Rehearsal>(),
        true,
    )
    .expect("enter rehearsal");

    super::send_stage_alert(h.clone(), Some("Rehearsing".into())).expect("send in rehearsal");
    settle();
    assert!(
        kiosk.silent(),
        "a rehearsal's word to the preacher escaped to a live stage monitor"
    );
}

/// …AND IT REACHES NO CONGREGATION CHANNEL — asserted at the DOORS, not at the
/// place the code happens to live.
///
/// `r5_…` above proves the alert arrives and that a rehearsal holds it. Neither is
/// the claim in docs/REBRAND.md §5 — "no congregation screen can show it" — and
/// until now that claim rested on two things that are not tests of the running
/// system: a sentence about which `.svelte` file the markup sits in, and
/// `r6-contracts.test.js`, which reads source text.
///
/// CLAUDE.md is explicit about why that is not enough: *"A test's assertion surface
/// is part of its claim."* `stage_next` was gated, tested and leaking for as long as
/// it was, because the test watched the wall and the leak went out of the other
/// door. So this one watches BOTH doors at once and asserts the whole shape of what
/// an alert does:
///
///   - the kiosk hub gets exactly ONE frame, and it is a `stage_alert`;
///   - it carries no field a congregation renderer binds — no `content_kind`, no
///     `reference`, no `template_json`. `Output.svelte` reads `text` only under
///     `kind === 'content'`, so a frame with no content kind cannot paint;
///   - **the Tauri door stays shut**. A native output window is driven by
///     `output://content` / `clear` / `black` and nothing else, so a projector on
///     HDMI is unreachable from here by construction — and the Wall is what proves
///     it, because the Wall is that door.
///
/// The last point is the one a source scan can never make. An alert published to
/// the hub is broadcast to every WebSocket client including `output.html`; what
/// stops a congregation seeing it is that the frame is not a content frame and no
/// congregation renderer has a branch for it. A future `emit` added here would pass
/// `r6-contracts` untouched and fail this.
#[test]
fn r5_a_word_to_the_preacher_reaches_no_congregation_channel() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);

    // A real verse first, so the test is run against a wall that HAS something on
    // it — the case where a leak would be indistinguishable from the verse.
    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None).unwrap();
    settle();
    let before = wall.count();
    assert_eq!(before, 1, "the fixture's own fire did not reach the wall");
    while kiosk.next().is_some() {} // drain the fire's own frames

    super::send_stage_alert(h.clone(), Some("Wrap up — 5 minutes".into())).expect("send");
    settle();

    let frame = kiosk
        .next()
        .expect("the stage monitor must get the message");
    assert!(
        frame.contains(r#""kind":"stage_alert""#),
        "the alert went out as something else: {frame}"
    );
    assert!(
        kiosk.silent(),
        "an alert published more than one frame; only the stage frame may leave: {frame}"
    );

    // Nothing a congregation template binds. `content_kind` is the field every
    // congregation renderer switches on; `reference` and `template_json` are how a
    // verse and its look travel.
    for field in ["content_kind", "reference", "template_json", "media_url"] {
        assert!(
            !frame.contains(field),
            "the alert frame carries `{field}`, which is congregation content: {frame}"
        );
    }

    // THE OTHER DOOR. A native output window hears Tauri events and nothing else.
    assert_eq!(
        wall.count(),
        before,
        "a word to the preacher reached the congregation wall"
    );
    assert!(
        !wall.cleared() && !wall.blacked(),
        "an alert must not disturb what is on the screens"
    );
}

/// THE SCRIPTURE SEARCH — the same parser as the live pipeline, and never a fire.
///
/// `search_verses` is the one search: the Planner's box, the preacher's remote
/// and the run surface all go through it. It reads what a person typed and
/// decides which verse that is, which makes it the same class of code as
/// `detection.rs` — so it gets the same kind of test.
#[test]
fn r9_the_search_finds_a_reference_however_it_is_typed() {
    let app = app();
    let h = app.handle().clone();
    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    let sem = h.state::<Semantic>();
    let top = |q: &str| {
        search_verses(&conn, &sem.0, q)
            .first()
            .map(|h| format!("{} {}:{}", h.verse.book, h.verse.chapter, h.verse.verse))
    };

    // Full name, fast abbreviation, non-prefix alias, and the spoken words a
    // person types without thinking.
    assert_eq!(top("john 3:16").as_deref(), Some("John 3:16"));
    assert_eq!(top("psa 23 1").as_deref(), Some("Psalms 23:1"));
    assert_eq!(top("jn 3:16").as_deref(), Some("John 3:16"));
    assert_eq!(top("rom 8 verse 1").as_deref(), Some("Romans 8:1"));

    // GLUED DIGITS. `ps23:1` is one token to the parser, so the quickest way to
    // type a reference used to return nothing at all — an empty list, with
    // nothing to say it had not understood.
    assert_eq!(top("ps23:1").as_deref(), Some("Psalms 23:1"));
    assert_eq!(top("jn3:16").as_deref(), Some("John 3:16"));
}

#[test]
fn r9_a_reference_outranks_a_phrase() {
    let app = app();
    let h = app.handle().clone();
    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    let sem = h.state::<Semantic>();

    // "John 3:16" is also a phrase that appears in no verse; the reference must
    // win, and win FIRST, because that is what the person typing it meant.
    let hits = search_verses(&conn, &sem.0, "john 3:16");
    let first = hits.first().expect("a reference always finds its verse");
    assert_eq!(
        (
            first.verse.book.as_str(),
            first.verse.chapter,
            first.verse.verse
        ),
        ("John", 3, 16)
    );
    assert_eq!(first.method, "reference");
}

#[test]
fn r9_a_query_that_is_mostly_not_scripture_returns_nothing_rather_than_guessing() {
    let app = app();
    let h = app.handle().clone();
    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    let sem = h.state::<Semantic>();

    // This used to come back with NINETEEN verses, Ezekiel 26:9 at the top,
    // because the full-text index returns anything that matched any term. A
    // confident wrong answer is worse than an empty list: the operator acts on it.
    let junk = search_verses(&conn, &sem.0, "quantum shepherd tractor engine banana");
    assert!(
        junk.len() <= 8,
        "a query with one real word in five came back with {} verses",
        junk.len()
    );

    // A word that is in no verse at all finds nothing, and says so by being empty.
    assert!(search_verses(&conn, &sem.0, "flibbertigibbet").is_empty());

    // And the thing the floor must NOT break: a real phrase still lands.
    let psalm = search_verses(&conn, &sem.0, "the lord is my shepherd");
    let first = psalm
        .first()
        .expect("a real phrase must still find its verse");
    assert_eq!(
        (
            first.verse.book.as_str(),
            first.verse.chapter,
            first.verse.verse
        ),
        ("Psalms", 23, 1)
    );
}

#[test]
fn r9_searching_never_puts_anything_on_a_screen() {
    // A search is an OFFER. Nothing it does may reach an output — not the wall,
    // not the stage monitor — until an operator chooses a result. Asserted on
    // both doors rather than inferred from the absence of a call, because the
    // absence of a call is exactly what four separate bugs in this repository
    // looked like.
    let app = app();
    let h = app.handle().clone();
    let wall = qa::Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);

    for q in ["john 3:16", "the lord is my shepherd", "ps23:1"] {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        let sem = h.state::<Semantic>();
        let hits = search_verses(&conn, &sem.0, q);
        assert!(!hits.is_empty(), "{q} found nothing");
    }
    settle();

    assert_eq!(wall.count(), 0, "a search reached a congregation screen");
    assert!(kiosk.silent(), "a search reached the kiosk hub");
}

/// THE ACCEPTANCE CLAUSE for `docs/REBRAND.md` §9, read against the real corpus.
///
/// `search.rs` proves the PARSE of each of these; this proves the VERSE. The two
/// halves are deliberately separate: a reference that parses to a book/chapter
/// nothing in the corpus answers for is a search that returns nothing, and a
/// parser test cannot see that.
#[test]
fn r9_every_shape_in_the_brief_finds_its_verse() {
    let app = app();
    let h = app.handle().clone();
    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    let sem = h.state::<Semantic>();
    let top = |q: &str| {
        search_verses(&conn, &sem.0, q)
            .first()
            .map(|h| format!("{} {}:{}", h.verse.book, h.verse.chapter, h.verse.verse))
    };

    for (query, want) in [
        ("ps 23 1", "Psalms 23:1"),
        ("ps23:1", "Psalms 23:1"),
        ("psalm 23", "Psalms 23:1"),
        ("rom 8 28", "Romans 8:28"),
        ("mt 6 33", "Matthew 6:33"),
        ("1 cor 13 4", "1 Corinthians 13:4"),
        ("see ye first the kingdom", "Matthew 6:33"),
        ("lamp unto my feet", "Psalms 119:105"),
    ] {
        assert_eq!(top(query).as_deref(), Some(want), "searching {query:?}");
    }
}

/// A BOOK PREFIX IS A SEARCH FEATURE AND NOWHERE ELSE.
///
/// "philipp 4 13" is not an alias in `book_aliases.json` and never will be —
/// the table is for what a preacher SAYS. Expanding a prefix is the widest book
/// match in the product, so it lives in the search path only, and this test
/// holds both halves of that: the search finds it, and the live detector, given
/// the identical string, finds nothing at all (CLAUDE.md rule 10).
#[test]
fn r9_a_book_prefix_is_a_search_feature_and_never_a_detection() {
    let app = app();
    let h = app.handle().clone();
    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    let sem = h.state::<Semantic>();

    for (query, want) in [
        ("philipp 4 13", "Philippians 4:13"),
        ("thessal 4 16", "1 Thessalonians 4:16"),
        ("revela 22 13", "Revelation 22:13"),
    ] {
        let hits = search_verses(&conn, &sem.0, query);
        let found = hits
            .iter()
            .any(|h| format!("{} {}:{}", h.verse.book, h.verse.chapter, h.verse.verse) == want);
        assert!(found, "searching {query:?} did not offer {want}");
        // …and the same words, spoken into a sermon, resolve to nothing.
        assert!(
            detection::detect_direct(query).is_empty(),
            "the LIVE detector resolved the book prefix in {query:?} — rule 10"
        );
    }
}

/// EVERY HIT SAYS WHY IT MATCHED — and says which KIND of claim it is.
///
/// The half of §9 that DECISIONS §72 recorded as not built, because it changes
/// the shape three surfaces read. Rule 18 in the search's clothing: the operator
/// must be able to tell a reference they typed from a verse Relay guessed at,
/// and a paraphrase carries **no percentage** because a cosine is not one.
#[test]
fn r9_every_hit_says_why_it_matched() {
    let app = app();
    let h = app.handle().clone();
    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    let sem = h.state::<Semantic>();

    // Every hit of every query, whatever branch produced it.
    for q in [
        "ps 23 1",
        "ps23:1",
        "philipp 4 13",
        "the lord is my shepherd",
        "lamp unto my feet",
        "there is therefore no condemnation in christ",
    ] {
        let hits = search_verses(&conn, &sem.0, q);
        assert!(!hits.is_empty(), "{q} found nothing");
        for hit in &hits {
            assert!(
                !hit.why.trim().is_empty(),
                "a hit for {q:?} had no reason: {:?}",
                hit.verse.reference
            );
            assert!(
                matches!(
                    hit.method,
                    "reference" | "prefix" | "phrase" | "words" | "paraphrase"
                ),
                "unknown method {:?} for {q:?}",
                hit.method
            );
            // A number that lies is worse than no number (rule 18).
            assert!(
                !hit.why.contains('%'),
                "a search hit quoted a percentage: {:?}",
                hit.why
            );
        }
    }

    // A reference the operator typed is NOT a guess, and says so.
    let typed = search_verses(&conn, &sem.0, "rom 8 28");
    let first = typed.first().expect("rom 8 28");
    assert_eq!(first.method, "reference");
    assert!(!first.guess);
    assert!(first.why.contains("rom 8 28"), "{:?}", first.why);

    // A prefix Relay expanded IS a guess, and names the book it chose.
    let pref = search_verses(&conn, &sem.0, "philipp 4 13");
    let hit = pref
        .iter()
        .find(|h| h.method == "prefix")
        .expect("a prefix hit");
    assert!(hit.guess);
    assert!(hit.why.contains("Philippians"), "{:?}", hit.why);

    // A PARAPHRASE is a guess, says so in words, and carries no number. This is
    // the branch rule 18 is really about: a TF-IDF cosine is not a probability,
    // and the operator has to be able to tell it from a reference they typed.
    let para = search_verses(
        &conn,
        &sem.0,
        "there is therefore no condemnation in christ",
    );
    let guess = para
        .iter()
        .find(|h| h.method == "paraphrase")
        .expect("a close paraphrase must reach the semantic branch");
    assert!(guess.guess);
    assert!(guess.why.contains("guess"), "{:?}", guess.why);
    assert!(guess.matched.is_empty(), "{:?}", guess.matched);

    // A word hit quotes the words that landed, and never the weak ones.
    let words = search_verses(&conn, &sem.0, "lamp unto my feet");
    let w = words
        .iter()
        .find(|h| h.method == "words" || h.method == "phrase")
        .expect("a literal hit for a real phrase");
    if w.method == "words" {
        assert!(w.matched.contains(&"lamp".to_string()), "{:?}", w.matched);
        assert!(!w.matched.contains(&"my".to_string()), "{:?}", w.matched);
    }
}

/// THE BOUNDARY, STATED AT THE BOUNDARY.
///
/// Search does approximate matching over book names and over verse text — the
/// same CLASS of code as the `fuzzy_book` repair that put Numbers 3:16 on a wall
/// unattended. The whole safety argument is that a person typed it and chose
/// from the list, so the guarantee that has to hold is that **no route out of a
/// search reaches `AutoFire`**.
///
/// Asserted by driving the router itself with what a search produces: the widest
/// thing this feature can offer, put to `decide`, and never allowed to fire.
#[test]
fn r9_nothing_a_search_offers_can_reach_an_auto_fire() {
    let app = app();
    let h = app.handle().clone();
    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    let sem = h.state::<Semantic>();

    // A search HAS no route to the router: it returns rows. The thing that could
    // change that is somebody deciding a prefix expansion is good enough to
    // detect with — so ask the router what it would do with one.
    let mut router = crate::router::Router::default();
    for query in ["philipp 4 13", "gene 1 1", "revela 22 13"] {
        for hit in search_verses(&conn, &sem.0, query) {
            if hit.method != "prefix" {
                continue;
            }
            let decision = router.decide(
                &hit.verse.reference,
                0.99,
                crate::detection::DetectionMethod::UncertainBook,
                0,
            );
            assert!(
                !matches!(decision, crate::router::RouteDecision::AutoFire),
                "a book-prefix guess reached AutoFire for {query:?}"
            );
        }
    }
}

/// A HIT IS STILL A VERSE ROW ON THE WIRE.
///
/// `why` was added as `#[serde(flatten)]` over `VerseRow` precisely so the four
/// surfaces that already read this command — the Library, the Planner, the Live
/// rail and the preacher's remote — keep reading the fields they read before.
/// DECISIONS §72 deferred this work for exactly that reason, so the flattening
/// is the decision, not an implementation detail: nest it and four surfaces
/// silently render blanks, with every Rust test still green.
#[test]
fn r9_a_hit_is_still_a_verse_row_on_the_wire() {
    let app = app();
    let h = app.handle().clone();
    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    let sem = h.state::<Semantic>();

    let hits = search_verses(&conn, &sem.0, "ps 23 1");
    let first = hits.first().expect("ps 23 1");
    let json = serde_json::to_value(first).expect("a hit serialises");
    let obj = json.as_object().expect("an object");

    // Every field a surface read before this change, at the top level.
    for key in [
        "id",
        "book",
        "chapter",
        "verse",
        "text",
        "reference",
        "translation",
    ] {
        assert!(
            obj.contains_key(key),
            "{key} is no longer on the wire: {obj:?}"
        );
    }
    // …and the new ones beside them, not nested under anything.
    for key in ["method", "guess", "why", "matched"] {
        assert!(
            obj.contains_key(key),
            "{key} did not reach the wire: {obj:?}"
        );
    }
    assert!(
        !obj.contains_key("verse_row") && !obj.values().any(|v| v.get("reference").is_some()),
        "the verse row was nested instead of flattened: {obj:?}"
    );
}

/// RG-136 — A RECOVERY IN THE SERVICE RECORD MUST HAVE A LOSS TO RECOVER FROM.
///
/// Field service 2026-09-13 (`audits/FIELD-2026-09-13.md` §3) recorded three
/// events for a 110.5 minute service: `service_started`, then `output_recovered`
/// for the Streaming screen, then `output_lost`. **A screen came back from an
/// outage the timeline never recorded**, so the report and the replay — which are
/// both built on this table — understate the outage count by at least one.
///
/// The two halves of the mechanism are individually reasonable and wrong together:
///
/// - `OutputHealth::transition` CONSUMES the edge it reports. `reported` advances
///   whether or not anything writes the event down.
/// - `log_event` is a **silent no-op when no service is recording** — the whole
///   body sits inside `if let Some(st) = sess.as_ref()`.
///
/// So a screen that was already dead before the operator pressed record had its
/// `output_lost` computed, thrown away, and its edge marked as reported. The
/// matching recovery then landed inside the service with no partner.
///
/// This drives `record_output_edges`, the real body of the status poll, rather
/// than `channel_status` itself: the command additionally needs a `KioskHub` and a
/// webview to answer at all, and neither has anything to do with what this decides.
#[test]
fn r136_a_recovery_in_the_record_always_has_a_loss_to_recover_from() {
    let app = app();
    let h = app.handle().clone();
    let health = h.state::<channels::OutputHealth>();

    let list = {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        db::list_output_channels(&conn).expect("channels")
    };
    // A `network_client` screen is treated as attached at all times, so it is the
    // one whose edges are polled from launch — before any service exists. This is
    // the Streaming channel of the field service.
    let screen = list
        .iter()
        .find(|c| c.render_target == "network_client")
        .expect("a fresh install seeds a network_client channel")
        .id;

    // ── 1. Before the service. The screen is attached and not answering. ──
    record_output_edges(&h, &health, &list, &[], false);
    health.expire_grace(screen);
    record_output_edges(&h, &health, &list, &[], false);

    // ── 2. The operator starts recording. ──
    let svc = start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Sunday Service".into(),
        "2026-09-13".into(),
    )
    .expect("start_service");

    // Still dead, and now inside a service that can record it.
    health.expire_grace(screen);
    record_output_edges(&h, &health, &list, &[], true);

    // ── 3. The screen starts answering. ──
    health.beat(
        screen,
        channels::PaintState::Content,
        "kiosk",
        channels::BeatGap::default(),
    );
    record_output_edges(&h, &health, &list, &[], true);

    let kinds: Vec<String> = service_timeline(h.state::<Db>(), svc)
        .expect("timeline")
        .into_iter()
        .map(|r| r.kind)
        .collect();

    // The invariant, stated as the audit found it violated: walking the timeline,
    // a screen may never be recovered before it has been lost.
    let mut lost = false;
    for k in &kinds {
        match k.as_str() {
            "output_lost" => lost = true,
            "output_recovered" => assert!(
                lost,
                "a recovery with no loss before it — the timeline understates the \
                 outage count, which is RG-136: {kinds:?}"
            ),
            _ => {}
        }
    }
    assert!(
        kinds.iter().any(|k| k == "output_recovered"),
        "the screen came back and the record must say so: {kinds:?}"
    );
}
