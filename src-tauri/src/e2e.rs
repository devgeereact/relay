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
use tauri::{Listener, Manager};

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
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .expect("fire");
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
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .expect("fire");
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

/// S1 · THE DOCK'S "End service" BUTTON MUST READ THE SERVICE, NOT THE LOCK.
///
/// `docs/REBRAND.md` §1 asks the Controls card for a fourth button that "owns the
/// on-air session". The obvious fact to drive it off was `service_lock.engaged` —
/// it is armed by `start_service` and released by `end_service`, so the two agree
/// almost all of the time.
///
/// Almost. The operator can lift the lock in one action and often will: it holds
/// back deletions and model changes, and somebody who needs one mid-service turns
/// it off. From that moment `engaged` is false over a service that is still
/// recording, and a button driven by it would say there is nothing to end while
/// the church's history is still open — CLAUDE.md rule 35, on the one control
/// that closes the record.
///
/// So `service_lock` carries `recording`, read from the session itself. Watched to
/// fail by driving it off `engaged`: the third assertion below is the one that
/// catches it.
#[test]
fn r3_the_service_is_still_recording_after_the_operator_lifts_the_lock() {
    let app = app();
    let h = app.handle().clone();

    // A fresh install: no service, and nothing for the button to end.
    let before = service_lock(h.state::<Session>(), h.state::<servicelock::ServiceLock>());
    assert!(!before.recording, "a fresh install is not recording");
    assert!(!before.engaged);

    start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Sunday Service".into(),
        "2026-09-14".into(),
    )
    .expect("start");
    let running = service_lock(h.state::<Session>(), h.state::<servicelock::ServiceLock>());
    assert!(running.recording, "a started service is recording");
    assert!(running.engaged, "and starting one arms the lock");

    // THE OVERRIDE. The two facts come apart here, and only one of them is the
    // one `end_service` acts on.
    set_service_lock(h.clone(), h.state::<servicelock::ServiceLock>(), false);
    let lifted = service_lock(h.state::<Session>(), h.state::<servicelock::ServiceLock>());
    assert!(!lifted.engaged, "the operator lifted the lock");
    assert!(
        lifted.recording,
        "and the service is STILL recording — a button off `engaged` would now \
         offer nothing to end while the record is open"
    );

    end_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<servicelock::ServiceLock>(),
    )
    .expect("end");
    let after = service_lock(h.state::<Session>(), h.state::<servicelock::ServiceLock>());
    assert!(
        !after.recording,
        "ending it is what makes the button go quiet"
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
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "Psalm 23".into(),
        None,
        None,
        None,
    )
    .expect("fire");
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
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
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

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
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
        None,
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

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .unwrap();

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
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "Psalm 23".into(),
        None,
        None,
        None,
    )
    .unwrap();
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

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "Jude 1:25".into(),
        None,
        None,
        None,
    )
    .unwrap(); // last verse of Jude
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

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .unwrap();
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

/// THE TRANSITION CONTROL REACHES BOTH DOORS, AND GATES NEITHER PANIC CONTROL.
///
/// Two claims, driven through the real `set_live_transition` command against the
/// real hub, because they are the two ways this feature could hurt a congregation.
///
/// **Both doors.** The wall is two kinds of screen: a native output window, which
/// has the Tauri bridge and no socket, and a kiosk/OBS browser source, which has
/// the socket and no backend at all. A control wired to one of them is the
/// "guarantee kept on one door" mistake this repository has now made four times —
/// and on these two doors it would be a projector on HDMI and an OBS source in the
/// same room transitioning differently. `Wall` watches one; `Kiosk` watches the
/// other, which is the only reason the second claim is testable at all.
///
/// **No panic control waits for it.** An 800 ms fade-through-black is in force and
/// `clear_screens` and `blackout` still report success and still reach the wall —
/// nothing on their path reads the override (rule 15, DECISIONS §20).
///
/// Watched to fail: dropping the `app.emit` in `channels::transition` (the native
/// window never learns), and dropping the `hub.set_transition` (every browser
/// source keeps cutting) — one assertion each, which is the point of watching both.
#[test]
fn the_transition_control_reaches_both_doors_and_delays_no_panic_control() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);

    // The native output window's door. `output://transition` has no home on `Wall`
    // (it is not content, a clear or a black), so it is listened for here directly —
    // which is also the honest thing: a bespoke listener says out loud that this is
    // a fourth kind of message and not a fifth kind of content.
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let s = seen.clone();
    use tauri::Listener as _;
    h.listen("output://transition", move |e| {
        s.lock().expect("seen").push(e.payload().to_string());
    });

    set_live_transition(h.clone(), Some("fadeblack".into()), Some(800));
    settle();

    let native = seen.lock().expect("seen").join("|");
    assert!(
        native.contains("fadeblack") && native.contains("800"),
        "the native output window never learned the operator's choice: {native}"
    );

    let mut hub_frames = Vec::new();
    while let Some(m) = kiosk.next() {
        hub_frames.push(m);
    }
    assert!(
        hub_frames
            .iter()
            .any(|m| m.contains(r#""kind":"transition""#) && m.contains("fadeblack")),
        "every OBS/kiosk browser source kept cutting: {hub_frames:?}"
    );

    // AND THE PANIC CONTROLS ARE EXACTLY AS UNCONDITIONAL AS THEY WERE.
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .unwrap();
    settle();
    assert!(wall.last().is_some(), "the verse never reached the wall");

    clear_screens(h.clone()).expect("clear must work with a transition in force");
    settle();
    assert!(wall.cleared(), "an 800 ms transition swallowed a clear");

    blackout(h.clone()).expect("blackout must work with a transition in force");
    settle();
    assert!(wall.blacked(), "an 800 ms transition swallowed a blackout");

    // And the override is still in force afterwards: a panic control takes the
    // screens down, it does not quietly re-arm every template's own transition.
    assert_eq!(
        live_transition(h.state::<channels::KioskHub>()),
        Some(("fadeblack".to_string(), Some(800))),
        "a panic control threw the operator's transition away"
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

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .unwrap();
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
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "Psalm 23".into(),
        None,
        None,
        None,
    )
    .unwrap();
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
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .unwrap();
    // …then the closing song takes the wall. Nothing scripture-shaped since.
    fire_content(
        h.clone(),
        h.state::<Db>(),
        "Blessed Assurance · Verse 1".into(),
        "Blessed assurance, Jesus is mine".into(),
        "song".into(),
        None,
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

/// A PAYLOAD THAT FORGOT TO NAME ITS KIND STILL DISARMS THE PASSAGE.
///
/// Rule 38's guard read `kind.as_deref().is_some_and(|k| k != "scripture")`, and
/// `is_some_and` is **false for `None`** — so content built the way
/// `..Default::default()` invites, with every field the caller cared about and
/// `kind` left unset, walked straight past the one place a passage is disarmed.
/// Every caller in the tree happens to set it; nothing said so, and the test above
/// cannot see the gap because it fires a song, which names itself.
///
/// The failure is reached by FORGETTING A FIELD rather than by adding a content
/// kind, which is why it survived the sweep that produced rule 38: the choke point
/// exists precisely so a kind added next year is disarmed by construction (rule
/// 36), and an ABSENT kind was the one shape that choke point did not cover.
///
/// Unspecified is treated as NOT scripture, which is the fail-safe direction and
/// deliberately not a refusal. A passage wrongly disarmed makes `nav` answer
/// `NoPassage` — a correct boundary the operator is told about (rule 38b). A
/// passage wrongly left armed walks a reading the congregation stopped looking at
/// twenty minutes ago and answers `Fired`, which is true of the wall and false of
/// the sermon. Only one of those two reaches a congregation. Refusing the
/// broadcast instead would blank a screen over content that renders perfectly
/// well, which is not what `preflight` is for (rule 36).
#[test]
fn r2_a_payload_that_forgot_its_kind_still_disarms_the_passage() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    // Sermon scripture, and the passage armed behind it.
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .unwrap();
    settle();
    assert!(
        matches!(
            nav(h.clone(), "next".into()).unwrap(),
            NavResult::Fired { .. }
        ),
        "precondition: the passage is armed, so this test can tell the two answers apart"
    );
    settle();

    // …then a notice takes the wall, built by a caller that filled in what it
    // cared about and left `kind` at its default.
    broadcast_with_clock(
        &h,
        channels::OutputContent {
            reference: "Notice".into(),
            text: Some("The hall is open after the service".into()),
            ..Default::default()
        },
    )
    .expect("a payload with a reference and text is not an empty screen");
    settle();
    assert!(
        wall.last().unwrap()["text"]
            .as_str()
            .unwrap_or("")
            .contains("The hall is open"),
        "precondition: the notice is what is on the wall"
    );

    let r = nav(h.clone(), "next".into()).unwrap();
    settle();
    assert!(
        matches!(r, NavResult::NoPassage),
        "`next` walked a passage under content that named no kind: {} — the wall now \
         shows {:?}",
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
    crate::latency::transcript_emitted(trace, 1_000, 8_000, 1, true, 1);
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

/// ONE AUTO-FIRE THAT REACHED A SCREEN IS ONE END-TO-END SAMPLE.
///
/// RG-120. `end_to_end_speech_to_scripture` is the stage that answers the only
/// question a church actually asks — *how long after the preacher says it does it
/// appear* — and in a real service it stamped **0 samples against three auto-fires**
/// (service 14) and **7 against nine** (service 15). Nothing asserted this, which
/// is why 0 of 3 shipped unnoticed: every other metric was populated, so the report
/// looked healthy and the one number a church would quote was missing.
///
/// The whole span in one test, in order, exactly as the live path runs it: a decode
/// pass begins, its transcript is emitted, a reference is detected and fired, and
/// the screen reports it painted through the same command `latency.js` calls.
#[test]
fn one_auto_fire_that_reached_a_screen_is_one_end_to_end_sample() {
    let _recorder = crate::latency::test_lock();
    crate::latency::reset();
    let app = app();
    let h = app.handle().clone();
    let _wall = Wall::watch(&h);

    let trace = crate::latency::begin_pass(crate::latency::now_us(), None);
    crate::latency::transcript_emitted(trace, 1_000, 8_000, 1, true, 1);
    super::emit_detections(
        &h,
        "turn with me to John chapter three verse sixteen",
        0,
        true,
        Some(trace),
    );
    settle();

    // The screen answers, the way `latency.js::markOutput` does over the bridge.
    crate::latency::frontend_mark(
        trace,
        crate::latency::Stage::OutputRendered,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    );

    let report = crate::latency::report(4);
    let e2e = report
        .metrics
        .iter()
        .find(|m| m.metric == "end_to_end_speech_to_scripture")
        .expect("metric");
    assert_eq!(
        e2e.samples, 1,
        "a verse that was heard, fired and painted produced no end-to-end sample"
    );
}

/// A SECOND SCREEN PAINTING THE SAME VERSE IS NOT A SECOND SAMPLE.
///
/// The mark closes the trace, deliberately, and that is what makes a church with a
/// projector AND a stage monitor AND an OBS source report one measurement per verse
/// rather than three. Worth pinning: the obvious "fix" for RG-120 is to stop
/// closing on the first mark, and it would silently triple every count in the
/// report while looking like more data.
#[test]
fn a_second_screen_painting_the_same_verse_does_not_double_count() {
    let _recorder = crate::latency::test_lock();
    crate::latency::reset();
    let app = app();
    let h = app.handle().clone();
    let _wall = Wall::watch(&h);

    let trace = crate::latency::begin_pass(crate::latency::now_us(), None);
    crate::latency::transcript_emitted(trace, 1_000, 8_000, 1, true, 1);
    super::emit_detections(
        &h,
        "turn with me to John chapter three verse sixteen",
        0,
        true,
        Some(trace),
    );
    settle();
    for _ in 0..3 {
        crate::latency::frontend_mark(
            trace,
            crate::latency::Stage::OutputRendered,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        );
    }

    let report = crate::latency::report(4);
    let e2e = report
        .metrics
        .iter()
        .find(|m| m.metric == "end_to_end_speech_to_scripture")
        .expect("metric");
    assert_eq!(e2e.samples, 1, "three screens, one verse, one measurement");
}

/// A VERSE NOTHING PAINTED IS COUNTED, SO A MISSING SAMPLE HAS A CAUSE.
///
/// The rest of RG-120, and the part a commit can actually settle. Service 14 ran
/// its three auto-fires before any output window existed — `service_events` records
/// no attachment until 1457.7 s, and all three fires were at 181.9 s, 212.0 s and
/// 460.4 s — so nothing could have painted them and zero end-to-end samples is the
/// CORRECT answer, not a broken stage.
///
/// But zero with no cause is unreadable. "The AI never fired" and "nothing was
/// attached to paint it" are the same number today, and they are completely
/// different reports about a church. So a fire that leaves the machine and is never
/// reported painted is counted, and the count is in Diagnostics and in the
/// diagnostic bundle beside the metric it explains.
#[test]
fn a_verse_that_no_screen_painted_is_counted_rather_than_silently_absent() {
    let _recorder = crate::latency::test_lock();
    crate::latency::reset();
    let app = app();
    let h = app.handle().clone();
    let _wall = Wall::watch(&h);

    let trace = crate::latency::begin_pass(crate::latency::now_us(), None);
    crate::latency::transcript_emitted(trace, 1_000, 8_000, 1, true, 1);
    super::emit_detections(
        &h,
        "turn with me to John chapter three verse sixteen",
        0,
        true,
        Some(trace),
    );
    settle();
    // No screen answers — the console-only setup of a real service. The trace is
    // retired the way `expire_stale` and `push_open` retire one.
    crate::latency::close(trace);

    let report = crate::latency::report(4);
    let e2e = report
        .metrics
        .iter()
        .find(|m| m.metric == "end_to_end_speech_to_scripture")
        .expect("metric");
    assert_eq!(
        e2e.samples, 0,
        "nothing painted it, so there is nothing to time"
    );
    assert_eq!(
        report.fires_never_painted, 1,
        "the absence has no cause, which is what made 0 of 3 unreadable in the field"
    );

    // And a render that arrives after its trace has gone is the OTHER cause, kept
    // apart from the first: one is a fact about the room, the other about this
    // recorder.
    crate::latency::frontend_mark(
        trace,
        crate::latency::Stage::OutputRendered,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    );
    assert_eq!(crate::latency::report(4).marks_after_close, 1);
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

/// W4 acceptance — scripture, lyrics and announcements leave the machine naming
/// three different templates, with nobody touching anything.
///
/// ── WHAT THIS TEST DOES NOT SAY, CORRECTED ──────────────────────────────────
///
/// It said "a screen set to FOLLOW **renders** … through three different
/// templates", and it has never been able to see a renderer. `qa::Wall` watches
/// the Tauri events, so the whole of its evidence is what is ON THE WIRE: the id
/// rides, and the JSON does not. Both were true, both are still true, and neither
/// is a claim about what a congregation sees.
///
/// It passed, green, for the entire period in which "Follow the content look"
/// put NOTHING on any screen. Nothing read `template_id` at either receiver:
/// `Output.svelte` derived its override from `template_json` alone, which is null
/// by construction here, and the kiosk door did not copy the field off the frame
/// at all. A test's assertion surface is part of its claim, and this one could
/// not have failed on the defect its own summary line described.
///
/// The rendering claim is made where a renderer exists — `src/lib/contentlook.test.js`
/// mounts the output page on BOTH doors and reads the painted DOM — and the
/// hub's half in `channels::tests::a_joining_screen_is_sent_the_content_looks_before_what_is_on_the_screens`.
/// This test keeps the wire claim, which is the half those two cannot make.
///
/// `r4_a_screen_may_follow_the_content_look` proves the setting can be made and
/// is published. It does not fire anything, so it cannot see whether the map is
/// then read — which is precisely the defect that phase closed one level down:
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
    super::manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
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
    super::send_stage_alert(h.clone(), Some("  Wrap up — 5 minutes  ".into()), None).expect("send");
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
    super::send_stage_alert(h.clone(), Some("   ".into()), None).expect("clear");
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

    super::send_stage_alert(h.clone(), Some("Rehearsing".into()), None).expect("send in rehearsal");
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
///   - **the Tauri door carries the same thing and no more.** This point USED to
///     read *"the Tauri door stays shut"*, and that was true and was the defect
///     (RG-156): a screen wired as a native window and given the `stage` role heard
///     nothing at all, while the console reported a Stage Message sent. The door is
///     open since 2026-09-21 and `output://stage_alert` carries `text` and nothing
///     else — no `content_kind`, no `reference`, no `template_json` — so what a
///     congregation renderer binds is unchanged, which is what this asserts;
///   - **and the wall is undisturbed either way.** `Wall` watches `output://content`,
///     so a stage alert that ever became content would move the count, and a clear
///     or a black provoked by one would show here too.
///
/// The last two points are the ones a source scan can never make. An alert is
/// broadcast to every WebSocket client including `output.html` AND emitted to every
/// webview, and neither door can address one screen — the hub records nothing about
/// who connected (DECISIONS §35) and a Tauri emit is app-wide. What stops a
/// congregation seeing it is `channelroles::acceptsStageMessage`, asked on the page
/// at both doors from one function, plus the fact asserted here: the payload holds
/// nothing a congregation template binds. `stagemessagenative.test.js` drives the
/// page half; this drives the engine half.
#[test]
fn r5_a_word_to_the_preacher_reaches_no_congregation_channel() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);
    // The native door, which this test used to prove shut by watching the Wall.
    let alerts: std::sync::Arc<std::sync::Mutex<Vec<serde_json::Value>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    {
        let sink = alerts.clone();
        h.listen("output://stage_alert", move |e| {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(e.payload()) {
                sink.lock().unwrap().push(v);
            }
        });
    }

    // A real verse first, so the test is run against a wall that HAS something on
    // it — the case where a leak would be indistinguishable from the verse.
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .unwrap();
    settle();
    let before = wall.count();
    assert_eq!(before, 1, "the fixture's own fire did not reach the wall");
    while kiosk.next().is_some() {} // drain the fire's own frames

    super::send_stage_alert(h.clone(), Some("Wrap up — 5 minutes".into()), None).expect("send");
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

    // THE OTHER DOOR, WATCHED RATHER THAN ASSUMED SHUT (RG-156). It is open now,
    // so "the Wall did not move" is no longer the whole claim about it: the event
    // itself has to carry nothing a congregation renderer binds.
    assert_eq!(
        alerts.lock().unwrap().len(),
        1,
        "the native door got no alert, or got more than one"
    );
    let payload = alerts.lock().unwrap()[0].clone();
    assert_eq!(
        payload.get("text").and_then(|v| v.as_str()),
        Some("Wrap up — 5 minutes"),
        "the native door did not carry the words: {payload}"
    );
    for field in ["content_kind", "reference", "template_json", "media_url"] {
        assert!(
            payload.get(field).is_none(),
            "the alert event carries `{field}`, which is congregation content: {payload}"
        );
    }
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

/// …AND THE SCREEN THAT IS NOT A STAGE IS TOLD IT IS NOT — which is the fact the
/// refusal is taken on.
///
/// The sibling above watches the two DOORS and proves the alert carries nothing a
/// congregation renderer binds. That was the whole guarantee for as long as
/// `Output.svelte` had no `stage_alert` branch at all: the frame went past every
/// congregation screen because none of them had anywhere to put it. `stage_message`
/// ends that — a layer binding means a renderer reads the value — so the refusal
/// stops being an omission and becomes a decision the page takes, on a fact the
/// backend has to supply.
///
/// The hub cannot address one client: it records nothing about who connected and
/// DECISIONS §35 is not being reversed. So this asserts the backend half of the
/// filter, at the doors it owns:
///
///   - a fresh install names ONE main screen and ONE stage, and the congregation
///     screens — Streaming and Lobby screen — hold no role at all. `null` is what
///     they arrive with, and `acceptsStageMessage` answers no to it;
///   - the role map that goes on the wire names them and nothing else — ids and
///     roles, no names, no addresses, nothing about who is connected;
///   - a channel that is NOT a stage stays absent from it after an alert has been
///     published, so nothing about sending one can promote a screen;
///   - `OutputContent` has no stage-message field, so the value cannot reach a
///     congregation screen the way `stage_note` does. **That is the one this pair
///     could not have caught before**: a field added to the content struct would
///     be broadcast to every screen with the verse, and the frame assertions above
///     look only at the alert.
///
/// What a Rust test cannot reach is whether the page PAINTS it, because the filter
/// is in JavaScript on a page with no backend. `src/lib/stagemessage.test.js`
/// drives the real `output.html` for that half, on a lobby channel and on the main
/// screen, and was watched to fail with the check removed.
#[test]
fn r5_a_word_to_the_preacher_reaches_no_screen_that_is_not_a_stage() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    let (list, roles) = {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        (
            db::list_output_channels(&conn).expect("channels"),
            db::channel_roles_json(&conn).expect("roles"),
        )
    };

    // A FRESH INSTALL, ROLE BY ROLE. The two congregation screens are the ones
    // this test exists for: they have no role, and "no role" must never be the
    // answer a filter says yes to.
    let role_of = |name: &str| {
        list.iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("a fresh install seeds `{name}`"))
            .role
            .clone()
    };
    assert_eq!(role_of("Main screen").as_deref(), Some("main"));
    assert_eq!(role_of("Stage display").as_deref(), Some("stage"));
    assert_eq!(role_of("Streaming"), None, "a stream is not a stage");
    assert_eq!(role_of("Lobby screen"), None, "a foyer TV is not a stage");
    assert_eq!(
        list.iter()
            .filter(|c| c.role.as_deref() == Some("main"))
            .count(),
        1,
        "exactly one screen is the main screen"
    );

    // WHAT GOES ON THE WIRE. Ids against roles, and nothing else — no names, no
    // addresses, nothing a client chose (DECISIONS §35).
    let map: serde_json::Value = serde_json::from_str(&roles).expect("the role map is JSON");
    let obj = map.as_object().expect("an object keyed by channel id");
    assert_eq!(
        obj.len(),
        2,
        "only the two screens with a role appear: {roles}"
    );
    for c in &list {
        let present = obj.contains_key(&c.id.to_string());
        assert_eq!(
            present,
            c.role.is_some(),
            "`{}` is {} the role map and {} a role",
            c.name,
            if present { "in" } else { "not in" },
            if c.role.is_some() { "has" } else { "has no" }
        );
        for (_, v) in obj.iter() {
            assert_ne!(
                v.as_str(),
                Some(c.name.as_str()),
                "a screen's NAME reached the role map: {roles}"
            );
        }
    }

    // …AND PUBLISHING AN ALERT CHANGES NONE OF IT. Nothing about sending a word to
    // the preacher may promote a screen into being one.
    super::send_stage_alert(h.clone(), Some("Wrap up — 5 minutes".into()), None).expect("send");
    settle();
    let frame = kiosk.next().expect("the alert is published");
    assert!(frame.contains(r#""kind":"stage_alert""#), "{frame}");
    let after = {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        db::channel_roles_json(&conn).expect("roles")
    };
    assert_eq!(after, roles, "publishing an alert moved a screen's role");

    // THE FIELD THAT MUST NOT EXIST. `stage_note`, `next_reference` and
    // `service_started_at` all ride on the content to every screen, kept private
    // by nothing but which layers a template happens to have. A stage message may
    // not join them: it is the one monitor-only value that is addressed to a
    // person rather than describing the slide, and a template is a thing an
    // operator can copy onto a lobby TV in two clicks.
    let content = channels::kiosk_content_json(&channels::OutputContent {
        kind: Some("scripture".into()),
        reference: "John 3:16".into(),
        text: Some("For God so loved the world".into()),
        stage_note: Some("hold for prayer".into()),
        ..Default::default()
    });
    assert!(
        !content.contains("stage_message") && !content.contains("stage_alert"),
        "the word to the preacher has become a field on OutputContent, so it now \
         travels to every screen with the verse: {content}"
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
        search_verses(&conn, &sem.0.read().expect("semantic index"), q)
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
    let hits = search_verses(&conn, &sem.0.read().expect("semantic index"), "john 3:16");
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
    let junk = search_verses(
        &conn,
        &sem.0.read().expect("semantic index"),
        "quantum shepherd tractor engine banana",
    );
    assert!(
        junk.len() <= 8,
        "a query with one real word in five came back with {} verses",
        junk.len()
    );

    // A word that is in no verse at all finds nothing, and says so by being empty.
    assert!(search_verses(
        &conn,
        &sem.0.read().expect("semantic index"),
        "flibbertigibbet"
    )
    .is_empty());

    // And the thing the floor must NOT break: a real phrase still lands.
    let psalm = search_verses(
        &conn,
        &sem.0.read().expect("semantic index"),
        "the lord is my shepherd",
    );
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
        let hits = search_verses(&conn, &sem.0.read().expect("semantic index"), q);
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
        search_verses(&conn, &sem.0.read().expect("semantic index"), q)
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
        let hits = search_verses(&conn, &sem.0.read().expect("semantic index"), query);
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
        let hits = search_verses(&conn, &sem.0.read().expect("semantic index"), q);
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
    let typed = search_verses(&conn, &sem.0.read().expect("semantic index"), "rom 8 28");
    let first = typed.first().expect("rom 8 28");
    assert_eq!(first.method, "reference");
    assert!(!first.guess);
    assert!(first.why.contains("rom 8 28"), "{:?}", first.why);

    // A prefix Relay expanded IS a guess, and names the book it chose.
    let pref = search_verses(
        &conn,
        &sem.0.read().expect("semantic index"),
        "philipp 4 13",
    );
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
        &sem.0.read().expect("semantic index"),
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
    let words = search_verses(
        &conn,
        &sem.0.read().expect("semantic index"),
        "lamp unto my feet",
    );
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
        for hit in search_verses(&conn, &sem.0.read().expect("semantic index"), query) {
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

    let hits = search_verses(&conn, &sem.0.read().expect("semantic index"), "ps 23 1");
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
/// Field service 2026-09-13 (`audits/FIELD.md` §3) recorded three
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
        None,
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

// ════════════════════════════════════════════════════════════════════════════
//  THE COUNTDOWN, AND THE TRANSPORT THAT IS NO LONGER HERE
//
//  Eleven tests stood here and drove `adjust_countdown` and `show_timer` — hold and
//  release, a held countdown surviving a re-aim, a transport that could never START
//  one, the way back after a verse, and a screen joining while it was held. They
//  went with those two commands on 2026-09-21 (DECISIONS §115), when the operator
//  asked for the Screen Countdown off Live for the second time and the commands lost
//  their only caller.
//
//  WHAT IS STILL TESTED BELOW, because this is the part that matters to a room: a
//  countdown can be STARTED — by the dock's helper here and by a plan cue — and it
//  reaches the screens it was aimed at, wearing the right template, and comes down
//  with a panic control. `start_countdown` keeps its caller and its tests.
//
//  WHAT IS NO LONGER TESTED, because it no longer exists: holding, re-aiming or
//  putting back a countdown already in front of a congregation. The only thing that
//  takes one off a wall now is Clear screens or Blackout.
// ════════════════════════════════════════════════════════════════════════════

/// Start the five-minute countdown the dock starts, so each test below begins where
/// an operator does.
fn start_five(h: &tauri::AppHandle<tauri::test::MockRuntime>) {
    start_countdown(
        h.clone(),
        h.state::<Db>(),
        5.0,
        "Service begins in".into(),
        "Welcome".into(),
        None,
        None,
        None,
        None,
    )
    .expect("start a countdown");
}

/// A PICTURE IS A FIRE PATH, AND IT HAD NO TEST.
///
/// `fire_media` puts an image on a congregation's wall through the same
/// `broadcast_with_clock` door as a verse, so the pre-air validator, the passage
/// disarm (rule 38) and the rehearsal gate all apply to it. None of that was ever
/// driven, because the function took a concrete `AppHandle` and `e2e.rs` runs on a
/// mock runtime — rule 24's own failure mode, in the one place rule 24's four-name
/// enforcement could not look.
///
/// `qa::bare_app()` (via this file's `app()`) seeds no media asset, on purpose — a
/// fresh install has none, and a second fixture is how two suites start
/// disagreeing about what a fresh install contains. So one is inserted here,
/// directly through `db::insert_media`, the way the media library command itself
/// would.
#[test]
fn r0_a_picture_reaches_the_wall_and_disarms_the_passage() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    // Sermon scripture, on screen a moment ago — a passage is armed.
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .unwrap();

    let media_id = {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        db::insert_media(&conn, "image", "slide.png", "2026-09-15", None).expect("seed a media row")
    };

    fire_media(h.clone(), h.state::<Db>(), media_id, None, None).expect("fire the picture");
    settle();

    assert_eq!(
        wall.last().expect("the wall")["kind"],
        "media",
        "the picture reached the wall"
    );

    // Rule 38: anything that is not scripture disarms the passage.
    let r = nav(h.clone(), "next".into()).expect("nav answers");
    assert!(
        matches!(r, NavResult::NoPassage),
        "a picture replaced the reading, so `next` must not walk it: {}",
        r.kind()
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  THE BACKGROUND LAYER, DRIVEN THROUGH THE REAL COMMANDS
//
//  `channels.rs`'s own tests hold the wire form and the retention rule. These
//  four ask the questions only the real commands can answer: does a verse
//  survive the backdrop, does the backdrop survive the verse, and does a panic
//  control take both.
//
//  They watch the HUB rather than the wall wherever the claim is about a LAN
//  device, for the reason `nothing_reaches_the_stage_monitor_during_a_rehearsal`
//  records: a test's assertion surface is part of its claim, and `Wall` counts
//  Tauri events.
// ════════════════════════════════════════════════════════════════════════════

/// A picture in the library, the way the import command would leave one.
fn seed_picture(h: &tauri::AppHandle<tauri::test::MockRuntime>) -> i64 {
    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    db::insert_media(&conn, "image", "sanctuary.jpg", "2026-09-17", None).expect("seed a picture")
}

/// The picture the hub would replay to a screen that joined just now.
fn retained_backdrop(h: &tauri::AppHandle<tauri::test::MockRuntime>) -> Option<String> {
    h.state::<channels::KioskHub>()
        .last_background_handle()
        .lock()
        .ok()
        .and_then(|b| b.clone())
}

/// A BACKGROUND OUTLIVES THE WORDS PAINTED ON IT.
///
/// The defect this whole change exists to close, stated as the behaviour rather
/// than as the mechanism: a verse and a picture used to be mutually exclusive
/// payloads, so scripture over a church's own background could not be expressed.
/// Firing the verse must now leave the backdrop exactly where it is.
///
/// It also asserts the half that is easy to lose the other way — the passage
/// stays ARMED. A backdrop is furniture and does not replace the reading, so
/// rule 38's disarm must not reach it; if it did, putting a picture up behind a
/// preacher mid-reading would make the next `next` answer `NoPassage`.
#[test]
fn r0_a_background_survives_the_verse_painted_on_it() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);
    let pic = seed_picture(&h);

    show_background(h.clone(), h.state::<Db>(), Some(pic)).expect("put the backdrop up");
    settle();
    let frame = kiosk.next().expect("the backdrop must reach a LAN screen");
    assert!(
        frame.contains(r#""kind":"background""#) && frame.contains("/media/"),
        "the backdrop reached the hub as something else: {frame}"
    );

    // Now the reading, over the top of it.
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .expect("fire");
    settle();
    assert_eq!(
        wall.last().expect("the wall")["reference"],
        "John 3:16",
        "the verse did not reach the screens"
    );
    assert!(
        retained_backdrop(&h)
            .unwrap_or_default()
            .contains("/media/"),
        "the verse took the church's backdrop down with it — which is the defect, \
         not the fix"
    );

    // And the reading is still a reading: furniture may not disarm a passage.
    let r = nav(h.clone(), "next".into()).expect("nav answers");
    assert!(
        matches!(r, NavResult::Fired { .. }),
        "a background disarmed the passage under a live reading: {}",
        r.kind()
    );
}

/// AND A PANIC CONTROL TAKES IT OFF EVERY SCREEN.
///
/// The invariant, at the level a congregation experiences it: `Clear screens`
/// and `Blackout` remove EVERYTHING, and the background is part of everything. A
/// clear that left the church's picture on the wall would be the worst class of
/// bug in this product — the operator has pressed the control that means "take it
/// all down" and something is still up there.
///
/// Asserted on the retained slot rather than on a frame, because the retained
/// slot is what a screen joining a second later would be painted. No new frame is
/// sent to achieve this and none should be: a panic control that needed two
/// frames is one that can half succeed.
#[test]
fn r0_a_panic_control_takes_the_background_off_every_screen() {
    for (name, wipe) in [("clear_screens", 0), ("blackout", 1)] {
        let app = app();
        let h = app.handle().clone();
        let _kiosk = qa::Kiosk::attach(&h);
        let pic = seed_picture(&h);

        show_background(h.clone(), h.state::<Db>(), Some(pic)).expect("backdrop up");
        manual_fire(
            h.clone(),
            h.state::<Db>(),
            "John 3:16".into(),
            None,
            None,
            None,
        )
        .expect("fire");
        settle();
        assert!(
            retained_backdrop(&h).is_some(),
            "{name}: nothing to take down — the test would pass for the wrong reason"
        );

        if wipe == 0 {
            clear_screens(h.clone()).expect("clear");
        } else {
            blackout(h.clone()).expect("blackout");
        }
        settle();
        assert!(
            retained_backdrop(&h).is_none(),
            "{name} left the church's picture on the wall"
        );
    }
}

/// THE PREACHER'S OWN SLIDE REACHES THE STAGE AND SURVIVES A RECONNECT.
///
/// Requirement 10. An announcement the preacher has to read out, or their own
/// deck, on the stage screen and nowhere else.
///
/// Rule 43 is the half worth asserting: a stage tablet whose wifi drops mid-sermon
/// comes back and is sent what is on its screen, which now includes this. Before
/// the retention slot existed it came back with the reading and no slide and
/// stayed that way until the operator happened to push it again.
#[test]
fn the_preachers_slide_is_replayed_to_a_screen_that_joins_after_it() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);
    let pic = seed_picture(&h);

    send_stage_media(h.clone(), h.state::<Db>(), Some(pic)).expect("put the slide up");
    settle();
    let frame = kiosk.next().expect("the slide reached no screen at all");
    assert!(
        frame.contains(r#""kind":"stage_media""#),
        "the slide did not leave the machine: {frame}"
    );

    // AND THE SCREEN THAT JOINS A MOMENT LATER IS TOLD.
    let retained = h
        .state::<channels::KioskHub>()
        .last_stage_media_handle()
        .lock()
        .ok()
        .and_then(|m| m.clone());
    assert!(
        retained.is_some_and(|f| f.contains(r#""kind":"stage_media""#)),
        "a stage screen joining mid-sermon would come back with no slide"
    );
}

/// A PANIC CONTROL TAKES THE PREACHER'S SLIDE TOO.
///
/// `Clear screens` means everything. The slide goes through the same retention
/// door as the backdrop, so `clear` and `black` empty it by construction rather
/// than by a second message somebody has to remember to send.
#[test]
fn a_panic_control_takes_the_preachers_slide_off_the_stage() {
    for (name, wipe) in [("clear_screens", 0), ("blackout", 1)] {
        let app = app();
        let h = app.handle().clone();
        let _kiosk = qa::Kiosk::attach(&h);
        let pic = seed_picture(&h);

        send_stage_media(h.clone(), h.state::<Db>(), Some(pic)).expect("slide up");
        settle();
        assert!(
            retained_stage_media(&h).is_some(),
            "{name}: nothing to take down — the test would pass for the wrong reason"
        );

        if wipe == 0 {
            clear_screens(h.clone()).expect("clear");
        } else {
            blackout(h.clone()).expect("blackout");
        }
        settle();
        assert!(
            retained_stage_media(&h).is_none(),
            "{name} left the preacher's slide on the stage screen"
        );
    }
}

/// NOTHING OF A STAGE SLIDE REACHES A SCREEN DURING A REHEARSAL.
///
/// The same guarantee as `stage_next` and `stage_alert`, and asserted on the hub
/// rather than through `Wall`, because this publisher emits no Tauri event at all
/// — watching the wall would have watched nothing and passed.
#[test]
fn nothing_of_the_preachers_slide_reaches_a_screen_during_a_rehearsal() {
    let app = app();
    let h = app.handle().clone();
    let _kiosk = qa::Kiosk::attach(&h);
    let pic = seed_picture(&h);

    // THE LIVE CASE FIRST, so this cannot pass by the publisher being broken.
    send_stage_media(h.clone(), h.state::<Db>(), Some(pic)).expect("slide up");
    settle();
    assert!(
        retained_stage_media(&h).is_some(),
        "the live case never worked"
    );

    set_rehearsal(
        h.clone(),
        h.state::<Session>(),
        h.state::<channels::Rehearsal>(),
        true,
    )
    .expect("enter rehearsal");
    // ONE OPERATION, IN ONE DIRECTION, and that is the whole design of this
    // assertion. My first version took the slide down and then put it back up
    // inside the rehearsal, and asserted the slot was still full — which is true
    // whether or not the gate exists, because the second call refills what the
    // first emptied. It passed with the rehearsal gate deleted, which makes it a
    // theory nobody tested (rule 40's own lesson, in a new file).
    //
    // A take-down alone cannot be undone by anything later in the test, so the
    // slot staying full is only possible if the publisher genuinely refused.
    send_stage_media(h.clone(), h.state::<Db>(), None).expect("ask for it to come down");
    settle();
    assert!(
        retained_stage_media(&h).is_some(),
        "a rehearsal reached the stage: the take-down left the machine and emptied the slot"
    );
}

/// A DOCUMENT IS REFUSED AT THE DOOR, NOT ON A SUNDAY.
///
/// Nothing in the product renders a PDF to a screen, so a slide built from one
/// would look correct in the Library and do nothing when it was reached.
#[test]
fn a_document_cannot_be_put_on_the_stage_screen() {
    let app = app();
    let h = app.handle().clone();
    let doc = {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        db::insert_media(&conn, "document", "notices.pdf", "2026-09-20", None)
            .expect("seed a document")
    };
    assert!(
        send_stage_media(h.clone(), h.state::<Db>(), Some(doc)).is_err(),
        "a PDF was accepted onto the stage screen"
    );
}

/// The slide the hub would replay to a stage screen that joined just now.
fn retained_stage_media(h: &tauri::AppHandle<tauri::test::MockRuntime>) -> Option<String> {
    h.state::<channels::KioskHub>()
        .last_stage_media_handle()
        .lock()
        .ok()
        .and_then(|m| m.clone())
}

/// NOTHING OF A BACKGROUND REACHES A LAN SCREEN DURING A REHEARSAL.
///
/// `set_background` publishes to the hub and emits a Tauri event, so `Wall` would
/// see half of it and a hub leak would be invisible — the shape of failure
/// `stage_next` shipped with. This watches the hub, and it asserts the live case
/// FIRST so it cannot pass by the publish path being broken outright.
#[test]
fn nothing_of_a_background_reaches_a_screen_during_a_rehearsal() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);
    let pic = seed_picture(&h);

    show_background(h.clone(), h.state::<Db>(), Some(pic)).expect("backdrop up");
    settle();
    assert!(
        kiosk
            .next()
            .unwrap_or_default()
            .contains(r#""kind":"background""#),
        "a real service must reach the screens"
    );

    set_rehearsal(
        h.clone(),
        h.state::<Session>(),
        h.state::<channels::Rehearsal>(),
        true,
    )
    .expect("enter rehearsal");

    show_background(h.clone(), h.state::<Db>(), Some(pic)).expect("backdrop up");
    show_background(h.clone(), h.state::<Db>(), None).expect("backdrop down");
    settle();
    assert!(
        kiosk.silent(),
        "a rehearsal put a picture on a live congregation screen"
    );
    // AND IT MOVED NOTHING IN EITHER DIRECTION. The retained slot still holds the
    // picture the LIVE service put up — the rehearsal neither replaced it nor took
    // it down. The take-down is the half worth spelling out: a rehearsing operator
    // pressing "clear background" must no more strip a real congregation screen
    // than a rehearsing `clear` may take a real wall down, which is the verdict
    // `clear`'s own rehearsal branch already carries.
    assert!(
        retained_backdrop(&h)
            .unwrap_or_default()
            .contains("/media/"),
        "a rehearsal changed what the next screen to join would be painted"
    );
}

/// A DOCUMENT CAN NEVER BECOME A BACKGROUND.
///
/// The same fact about the same table `fire_media` already refuses on, said in
/// the same sentence: a PDF has no frame to paint. A refusal here is a refusal
/// the operator can read, rather than every screen fetching a file no browser
/// will render and painting black.
#[test]
fn a_document_can_never_become_a_background() {
    let app = app();
    let h = app.handle().clone();
    let _kiosk = qa::Kiosk::attach(&h);
    let doc = {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        db::insert_media(&conn, "document", "notices.pdf", "2026-09-17", None).expect("seed")
    };

    assert!(
        show_background(h.clone(), h.state::<Db>(), Some(doc)).is_err(),
        "a PDF was accepted as a congregation background"
    );
    assert!(
        retained_backdrop(&h).is_none(),
        "a refused background was retained anyway"
    );
}

// ════════════════════════════════════════════════════════════════════════════
//  THE THREE DOORS STOP FORGETTING — Wave 3, Track A, Task 3
//
//  `note_countdown` kept the countdown only while the LIVE CONTENT was a countdown,
//  which is why a verse forgot it. The two tests below are that lifetime stated at
//  the doors rather than at the registry: one says a verse must not take a timer,
//  the other says a panic control must.
//
//  They live here rather than in `channels.rs`'s own `mod tests` on purpose. The
//  claim is about what the real commands do to the real doors, and driving it from
//  here means no `OutputContent` is built by hand to make the point — which is the
//  thing five hand-rolled copies drifted apart doing.
// ════════════════════════════════════════════════════════════════════════════

/// FIRING A VERSE DOES NOT FORGET THE CONGREGATION TIMER.
///
/// The door-level half of the reported defect. `broadcast_content` used to forget
/// the countdown for any content that was not one, so the lifetime of a timer was
/// decided by whatever happened to be on the screens.
#[test]
fn firing_a_verse_does_not_forget_the_congregation_timer() {
    let app = app();
    let h = app.handle().clone();

    start_five(&h);
    assert_eq!(list_timers(h.clone()).expect("list").len(), 1);

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .expect("fire");
    settle();
    assert_eq!(
        list_timers(h.clone()).expect("list").len(),
        1,
        "a verse took the congregation timer with it"
    );

    // …and so does everything else a service puts on a screen.
    fire_content(
        h.clone(),
        h.state::<Db>(),
        "Notices".into(),
        "Tea afterwards".into(),
        "announcement".into(),
        None,
        None,
        None,
    )
    .expect("a notice");
    settle();
    assert_eq!(
        list_timers(h.clone()).expect("list").len(),
        1,
        "a notice took the congregation timer with it"
    );
}

/// STARTING A COUNTDOWN REPLACES THE ONE BEFORE IT RATHER THAN STACKING.
///
/// The old single slot kept this by construction — there was one countdown because
/// there was one slot. A registry is a map, so the rule has to be said out loud, and
/// this is what says it. Without it every press of Start leaves a dead clock behind:
/// `list_timers` is the surface an operator would use to find the timer counting
/// down to the wrong thing, and a list that fills with abandoned ones is how they
/// stop reading it.
#[test]
fn starting_a_second_countdown_replaces_the_first_rather_than_stacking() {
    let app = app();
    let h = app.handle().clone();

    start_five(&h);
    let first = list_timers(h.clone()).expect("list")[0].timer.id;
    start_five(&h);
    settle();

    let after = list_timers(h.clone()).expect("list");
    assert_eq!(
        after.len(),
        1,
        "a second countdown stacked on the first instead of replacing it"
    );
    assert_ne!(
        after[0].timer.id, first,
        "the new countdown must be a new timer, not the old one re-aimed in place"
    );

    // A programme timer is a different question and Start must not take one.
    let programme = start_timer(
        h.clone(),
        20.0,
        "Sermon".into(),
        "Wrap up".into(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("a programme timer");
    start_five(&h);
    settle();
    assert!(
        list_timers(h.clone())
            .expect("list")
            .iter()
            .any(|t| t.timer.id == programme),
        "starting a congregation countdown took the preacher's clock with it"
    );
}

/// A CLEAR TAKES THE CONGREGATION TIMER AND LEAVES THE PROGRAMME TIMER.
///
/// The congregation guarantee does not move: what `clear` and `black` take off a
/// congregation screen stays off it, and
/// `r7_a_cleared_countdown_cannot_be_brought_back_by_the_transport` still pins that
/// from the transport's side.
///
/// A `Stage`-scoped timer is a different question, and the answer is a property of
/// the TIMER rather than a branch inside the panic control — a control that has to
/// ask which screen it is talking to can fail to answer. Track C is where the stage
/// tablet learns to show one; this is where its lifetime is decided.
#[test]
fn a_clear_takes_the_congregation_timer_and_leaves_the_programme_timer() {
    let app = app();
    let h = app.handle().clone();

    start_five(&h);
    let programme = start_timer(
        h.clone(),
        20.0,
        "Sermon".into(),
        "Wrap up".into(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("a programme timer");
    settle();
    assert_eq!(list_timers(h.clone()).expect("list").len(), 2);

    clear_screens(h.clone()).expect("clear");
    settle();
    assert_eq!(
        list_timers(h.clone())
            .expect("list")
            .iter()
            .map(|t| t.timer.id)
            .collect::<Vec<_>>(),
        vec![programme],
        "a clear must take the congregation timer and leave the programme one"
    );

    // Blackout is the harsher of the two and must do exactly as much, and no more.
    start_five(&h);
    blackout(h.clone()).expect("black");
    settle();
    assert_eq!(
        list_timers(h.clone())
            .expect("list")
            .iter()
            .map(|t| t.timer.id)
            .collect::<Vec<_>>(),
        vec![programme],
        "a blackout must take the congregation timer and leave the programme one"
    );
}

/// A REHEARSAL REACHES NO STAGE TABLET, AND THIS TEST WATCHES THE DOOR IT LEAVES BY.
///
/// **The assertion surface is the claim.** `Wall` listens for Tauri events, and
/// `publish_timers` emits none — it publishes to the kiosk hub and nothing else,
/// which is exactly the shape that let `stage_next` ship gated in name only and leak
/// "up next" to a live stage tablet mid-rehearsal while the e2e rehearsal test
/// stayed green. So this watches `qa::Kiosk`, the hub itself.
///
/// The live case is asserted FIRST, so this cannot pass by the publish path being
/// broken outright — the failure mode of every "assert nothing happened" test.
#[test]
fn a_programme_timer_published_during_a_rehearsal_reaches_no_stage_tablet() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    // A real service: the programme timer must reach the tablet.
    start_timer(
        h.clone(),
        20.0,
        "Sermon".into(),
        "Wrap up".into(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("a programme timer");
    settle();
    let live = kiosk
        .next()
        .expect("a real service must reach the stage tablet");
    assert!(
        live.contains(r#""kind":"timer""#) && live.contains("Sermon"),
        "the stage tablet got something other than the programme timer: {live}"
    );

    set_rehearsal(
        h.clone(),
        h.state::<Session>(),
        h.state::<channels::Rehearsal>(),
        true,
    )
    .expect("enter rehearsal");

    // Every door into the registry that publishes, not only the one that creates.
    let rehearsed = start_timer(
        h.clone(),
        10.0,
        "Offering".into(),
        String::new(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("a second programme timer");
    adjust_timer(h.clone(), rehearsed, Some(5 * 60_000), None).expect("re-aim it");
    stop_timer(h.clone(), rehearsed).expect("stop it");
    settle();
    assert!(
        kiosk.silent(),
        "a rehearsal's programme clock escaped to a live stage tablet — the same \
         leak as `stage_next`, on the same screen"
    );
}
/// A TIMER STARTED INSIDE A REHEARSAL DOES NOT OUTLIVE IT — RG-150.
///
/// The operator decision of 2026-09-17: a rehearsal is a sandbox in every other
/// respect, and a clock it started is not an exception. The alternative was to
/// republish the set on the way out on the grounds the timers were real all along,
/// which means an operator who practises a twenty-minute sermon clock at ten
/// o'clock finds it on the preacher's tablet when the service starts, counting
/// toward a moment that has passed.
///
/// **The assertion surface is the claim**, for the same reason as the rehearsal
/// test above: `publish_timers` emits no Tauri event, so `qa::Wall` cannot see
/// this at all and a test written against it would pass over the defect. The
/// measured defect (`audits/DESIGN.md` §6) is two separate
/// failures and both are asserted here — the registry kept `Rehearsal only`, and
/// the exit published `clear` and `stage_next` and NO `timer` frame, so the
/// tablet's set and the registry disagreed silently until something unrelated
/// republished.
#[test]
fn a_timer_started_inside_a_rehearsal_does_not_outlive_it() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    // A real programme timer, before anybody rehearses anything.
    start_timer(
        h.clone(),
        20.0,
        "Sermon".into(),
        "Wrap up".into(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("a programme timer");
    settle();
    assert!(
        kiosk.next().is_some_and(|f| f.contains("Sermon")),
        "the live case must work first, or this test passes by the publish path \
         being broken outright"
    );

    set_rehearsal(
        h.clone(),
        h.state::<Session>(),
        h.state::<channels::Rehearsal>(),
        true,
    )
    .expect("enter rehearsal");

    start_timer(
        h.clone(),
        10.0,
        "Rehearsal only".into(),
        String::new(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("a timer started inside the rehearsal");

    set_rehearsal(
        h.clone(),
        h.state::<Session>(),
        h.state::<channels::Rehearsal>(),
        false,
    )
    .expect("leave rehearsal");
    settle();

    // HALF ONE: the registry. `list_timers` is the surface an operator would use
    // to find out what is counting, and it returned `Rehearsal only` alongside the
    // real ones.
    let labels: Vec<String> = list_timers(h.clone())
        .expect("list the timers")
        .into_iter()
        .map(|v| v.timer.label)
        .collect();
    assert!(
        !labels.iter().any(|l| l == "Rehearsal only"),
        "a rehearsal's timer is still in the registry after the rehearsal ended: \
         {labels:?}"
    );
    assert!(
        labels.iter().any(|l| l == "Sermon"),
        "the rehearsal exit took a timer that predates it: {labels:?}"
    );

    // HALF TWO: the wire. The exit must publish the real set, and the tablet must
    // never be shown one that is about to change — so the LAST timer frame to
    // leave is what it is holding, and it must be the real set.
    let mut last_timer_frame = None;
    while let Some(frame) = kiosk.next() {
        if frame.contains(r#""kind":"timer""#) {
            last_timer_frame = Some(frame);
        }
    }
    let frame = last_timer_frame.expect(
        "ending a rehearsal told the stage tablet nothing about its programme \
         timers, so its set and the registry disagree in silence",
    );
    let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");
    let on_the_tablet: Vec<&str> = v["timers"]
        .as_array()
        .expect("a timer frame carries a set")
        .iter()
        .filter_map(|t| t["label"].as_str())
        .collect();
    assert_eq!(
        on_the_tablet,
        vec!["Sermon"],
        "the stage tablet was left holding the wrong set after a rehearsal: {frame}"
    );
}

/// A TIMER THAT PREDATES A REHEARSAL SURVIVES THE END OF IT — the half RG-150's
/// decision does not settle, decided here and pinned rather than left to a reading.
///
/// The stated rule is that ending a rehearsal stops every timer STARTED INSIDE IT.
/// A timer started before the rehearsal began was never a rehearsal's timer, so on
/// that rule it survives, and `TimerRegistry::stop_started_in_rehearsal` is written
/// to take exactly the ones that were stamped and no others. The alternative
/// reading — that a rehearsal exit clears everything — is the quiet widening this
/// repository keeps finding, and it would take a real service's sermon clock off
/// the preacher's tablet because somebody opened the rehearsal switch for ten
/// seconds.
///
/// `Scope::Stage` deliberately: leaving a rehearsal clears the screens, and a
/// clear takes every congregation timer with it (DECISIONS §27,
/// `channels::stop_congregation_timers`). That is an older guarantee and not this
/// one, so the survival is asserted on the scope where it is actually visible.
#[test]
fn a_timer_that_predates_a_rehearsal_survives_the_end_of_it() {
    let app = app();
    let h = app.handle().clone();

    let before = start_timer(
        h.clone(),
        20.0,
        "Sermon".into(),
        "Wrap up".into(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("a programme timer");

    for on in [true, false] {
        set_rehearsal(
            h.clone(),
            h.state::<Session>(),
            h.state::<channels::Rehearsal>(),
            on,
        )
        .expect("flip rehearsal");
    }
    settle();

    let listed = list_timers(h.clone()).expect("list the timers");
    let still = listed
        .iter()
        .find(|v| v.timer.id == before)
        .expect("a timer nobody started in a rehearsal was taken by the end of one");
    assert_eq!(still.timer.label, "Sermon");
    assert!(
        still.remaining_ms > 19 * 60_000,
        "it survived and was re-aimed, which is a different kind of wrong"
    );
}

/// STOPPING THE LAST PROGRAMME TIMER TAKES IT OFF THE PREACHER'S SCREEN.
///
/// An absent frame cannot say "there are none now". Publishing nothing on a stop
/// would leave the clock on the tablet, counting, for the rest of the service — and
/// nothing on that screen could tell the preacher it was stale. So the whole
/// stage-visible SET is published every time, and the empty set is a real frame.
#[test]
fn stopping_the_last_programme_timer_publishes_an_empty_set_to_the_stage() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    let programme = start_timer(
        h.clone(),
        20.0,
        "Sermon".into(),
        "Wrap up".into(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("a programme timer");
    settle();
    kiosk.next().expect("the start reached the tablet");

    stop_timer(h.clone(), programme).expect("stop");
    settle();
    let frame = kiosk
        .next()
        .expect("stopping the last programme timer told the tablet nothing");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");
    assert_eq!(v["kind"], "timer");
    assert_eq!(
        v["timers"].as_array().map(|a| a.len()),
        Some(0),
        "the stage tablet was not told the programme clock is gone: {frame}"
    );
}

/// A SERMON THAT HAS RUN OVER CAN BE HELD, THROUGH THE REAL COMMAND (RG-175).
///
/// The registry test proves the arithmetic and the mounted test proves a control
/// reaches it; this proves the command path in between, and that the figure
/// survives onto the wire the preacher's tablet actually reads.
///
/// It matters because every one of the four things that made this impossible
/// lived on a different layer — the clamp in `timers::remaining_ms`, the
/// `TooShort` refusal in `adjust`, the `> 0` in `countdown.js`, and no rendered
/// control at all. Three of them could be fixed with this path still broken.
#[test]
fn a_programme_timer_past_zero_can_be_held_at_the_figure_it_is_showing() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    // The shortest timer `start_timer` will accept without substituting its own
    // length: a tenth of a second, so it is over before the hold is asked for.
    let programme = start_timer(
        h.clone(),
        0.002,
        "Sermon".into(),
        String::new(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("a programme timer");
    settle();
    kiosk.next().expect("the start reached the tablet");
    std::thread::sleep(std::time::Duration::from_millis(250));

    // No figure named: the engine reads the clock. Naming one is what a caller
    // on the far side of the bridge cannot do, because its own reading is
    // clamped at zero and a REQUESTED zero is refused as `TooShort`.
    adjust_timer(h.clone(), programme, None, Some(true))
        .expect("a programme timer past zero refused to be held");
    settle();
    let held = list_timers(h.clone())
        .expect("list")
        .into_iter()
        .find(|t| t.timer.id == programme)
        .expect("the held timer vanished");
    assert!(
        held.timer.paused_ms.is_some_and(|p| p < 0),
        "the hold did not freeze the overrun figure: {:?}",
        held.timer.paused_ms
    );

    let frame = kiosk.next().expect("the hold told the tablet nothing");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");
    assert_eq!(v["kind"], "timer");
    let row = &v["timers"][0];
    let paused = row["countdown_paused_ms"]
        .as_i64()
        .expect("no held figure on the wire");
    assert!(
        paused < 0,
        "the wire carried {paused}: the tablet cannot render a row it is not sent"
    );

    // AND LETTING GO CARRIES ON FROM THERE, rather than restarting at zero.
    adjust_timer(h.clone(), programme, None, Some(false)).expect("resume");
    let run = list_timers(h.clone())
        .expect("list")
        .into_iter()
        .find(|t| t.timer.id == programme)
        .expect("the resumed timer vanished");
    assert_eq!(
        run.timer.paused_ms, None,
        "a resumed timer is still holding a figure"
    );
    assert!(
        run.timer.target_ms < crate::now_epoch_ms(),
        "a resumed overrun timer was re-aimed into the future, losing the elapsed \
         figure the hold existed to keep"
    );
}

/// A CUE THAT NAMES A SCREEN SAYS SO ON THE WIRE (RG-161).
///
/// The unit tests cover the retention rule and the receiver filter; this is the
/// command path between them — that a screen set given to `manual_fire`
/// survives `resolve_fire`, `Fire::output` and `broadcast_content` and arrives
/// on the frame a screen actually reads. Every one of those was a place it
/// could have been dropped silently, and a dropped set is a notice on the
/// preacher's tablet.
#[test]
fn a_cue_that_names_a_screen_carries_it_to_the_wire() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        Some(vec![4]),
    )
    .expect("fire");
    settle();

    let frame = kiosk.next().expect("the fire reached no screen at all");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");
    assert_eq!(v["kind"], "content");
    assert_eq!(
        v["channels"],
        serde_json::json!([4]),
        "the screen set was dropped somewhere between the command and the wire: {frame}"
    );
}

/// AND A FIRE THAT NAMES NONE SAYS NULL, WHICH IS EVERY SCREEN.
///
/// The half that must not regress: every existing path — a detected verse, the
/// operator's reference box, the preacher's phone — names no screens, and a
/// frame that arrived with an empty list instead of a null would reach nothing.
#[test]
fn a_fire_that_names_no_screen_reaches_every_screen() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .expect("fire");
    settle();

    let frame = kiosk.next().expect("the fire reached no screen at all");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");
    assert!(
        v["channels"].is_null(),
        "an untargeted fire carried a screen set: {frame}"
    );
}

/// AND THE SAME QUESTION ASKED OF THE OTHER TWO CUE KINDS, WHICH ANSWERED WRONG.
///
/// The Planner's `Screens` row is not gated by cue type, so it renders for all
/// five kinds. Scripture, song and announce passed the set through; **media and
/// countdown did not**, because `fire_media` and `start_countdown` never took the
/// argument at all and `OutputContent.channels` was left `None` by
/// `..Default::default()`. An operator ticked one screen of three, read "Other
/// screens keep what they are showing", and all three got it.
///
/// This is the fourth time in this repository that a guarantee has been kept on
/// some doors and not on its twin, which is why the test names the KIND rather
/// than the command: a sixth cue kind added next year fails here.
#[test]
fn a_media_cue_that_names_a_screen_carries_it_to_the_wire() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);
    let pic = seed_picture(&h);

    fire_media(h.clone(), h.state::<Db>(), pic, None, Some(vec![4])).expect("fire the picture");
    settle();

    let frame = kiosk.next().expect("the fire reached no screen at all");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");
    assert_eq!(v["kind"], "content");
    assert_eq!(
        v["channels"],
        serde_json::json!([4]),
        "a media cue's screen set was dropped between the command and the wire: {frame}"
    );
}

/// AND A MEDIA CUE THAT NAMES NONE STILL REACHES EVERY SCREEN.
///
/// The half that must not regress. `None` is every screen and `[]` is no screen,
/// and every media cue written before targeting existed carries neither.
#[test]
fn a_media_cue_that_names_no_screen_reaches_every_screen() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);
    let pic = seed_picture(&h);

    fire_media(h.clone(), h.state::<Db>(), pic, None, None).expect("fire the picture");
    settle();

    let frame = kiosk.next().expect("the fire reached no screen at all");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");
    assert!(
        v["channels"].is_null(),
        "an untargeted media cue carried a screen set: {frame}"
    );
}

/// A CONGREGATION TIMER IS NOT THE PROGRAMME, AND THE STAGE FRAME SAYS SO.
///
/// The two scopes share a registry and a wire vocabulary, which is precisely why
/// this needs asserting: projecting a `Both` timer into the stage frame would put
/// the pre-service countdown in the preacher's programme rail, and it would look
/// entirely plausible there.
#[test]
fn a_congregation_countdown_never_appears_in_the_programme_rail() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    start_timer(
        h.clone(),
        5.0,
        "Service begins in".into(),
        "Welcome".into(),
        "both".into(),
        None,
        None,
        None,
    )
    .expect("a congregation timer");
    settle();

    // The frame is still published — the set simply has nothing in it — because the
    // publisher asks no question about what changed. See `start_timer`.
    let mut seen = Vec::new();
    while let Some(m) = kiosk.next() {
        seen.push(m);
    }
    let timer_frames: Vec<&String> = seen
        .iter()
        .filter(|m| m.contains(r#""kind":"timer""#))
        .collect();
    assert!(
        !timer_frames.is_empty(),
        "no timer frame reached the hub at all: {seen:?}"
    );
    for frame in timer_frames {
        let v: serde_json::Value = serde_json::from_str(frame).expect("valid JSON");
        assert_eq!(
            v["timers"].as_array().map(|a| a.len()),
            Some(0),
            "a congregation countdown was published into the preacher's programme \
             rail: {frame}"
        );
    }
}

/// A BLACKOUT ANSWERS THE SAME WAY AS A CLEAR.
///
/// `a_clear_takes_the_congregation_timer_and_leaves_the_programme_timer` already
/// presses `blackout` in its second half, and that half does catch a `black` that
/// forgets to stop the congregation timer. It catches it on an app where a CLEAR
/// HAS ALREADY RUN, and under a name that claims the clear. Two things follow from
/// that, and both are the reason this test exists beside it rather than inside it.
///
/// The first is ordering. `stop_congregation_timers` had already been called once
/// down the clear path before the blackout half started, so the blackout was only
/// ever asked the question second. A control that behaves correctly on a registry
/// something else has already touched, and wrongly on a fresh one, is not a shape
/// anybody would predict — which is exactly why it should not be left untested.
/// Here the app has never seen a panic control before `blackout` is pressed.
///
/// The second is the name. `Stage.svelte`'s clear/black branch says in as many
/// words that if Relay ever lets the stage survive a panic, it must survive BOTH
/// controls, deliberately, in both branches, "not by one of them being forgotten"
/// — and the guarantee for the harsher of the two was carried by the tail of a
/// test named after the milder one. A guarantee is only kept on the doors you
/// checked, and a door nobody named is the one that gets tidied away.
///
/// DECISIONS §91.
#[test]
fn a_blackout_answers_the_same_way_as_a_clear() {
    let app = app();
    let h = app.handle().clone();

    let programme = start_timer(
        h.clone(),
        20.0,
        "Sermon".into(),
        "Wrap up".into(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("a programme timer");
    start_five(&h);
    settle();
    assert_eq!(
        list_timers(h.clone()).expect("list").len(),
        2,
        "the fixture needs one timer of each scope for the question to mean anything"
    );

    // The FIRST panic control this app has seen, and it is the harsher one.
    blackout(h.clone()).expect("black");
    settle();
    assert_eq!(
        list_timers(h.clone())
            .expect("list")
            .iter()
            .map(|t| t.timer.id)
            .collect::<Vec<_>>(),
        vec![programme],
        "a blackout must take the congregation timer and leave the programme one, \
         on a registry no other panic control has touched first"
    );
}

/// THE CONSOLE IS TOLD WHAT EVERY SCREEN WAS TOLD — RG-260.
///
/// The operator: *"what's on screen is different from what's on the console...
/// when the replay button is clicked, only the operator's screen replays... I
/// want the media to work using just one control for all screens or output"*.
///
/// **The divergence was structural, not a race.** `set_media_transport` built the
/// frame, published it to every screen, and returned `()`. The console then
/// rebuilt its own copy out of the arguments it had passed in — `{paused, loop,
/// volume}` and nothing else — so the console's preview was handed an object
/// with no `replay_epoch` and no `seek_epoch`, and `applyMediaTransport` acts on
/// a replay or a scrub ONLY when it sees an epoch it has not seen. Two shapes for
/// one instruction, and only one of them complete.
///
/// So the command hands the frame back. **It still says nothing about whether a
/// screen obeyed** — that distinction is the reason the old doc comment refused a
/// return value, and it survives: a frame is the INSTRUCTION, the beat is the
/// outcome, and Live still reads the effect from `MediaBeat`. What changes is
/// that the console stops guessing at the instruction it just gave.
#[test]
fn the_transport_command_hands_back_the_frame_it_published() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    let first = set_media_transport(
        h.clone(),
        h.state::<channels::MediaTransport>(),
        None,
        None,
        Some(true),
        None,
        None,
    )
    .expect("replay");
    settle();

    // THE FRAME THE CONSOLE IS HANDED AND THE FRAME THE SCREENS RECEIVED ARE THE
    // SAME FRAME. Asserted field by field against the published JSON rather than
    // against the struct, because the wire is what a screen actually acts on and
    // a serialiser that dropped a field would satisfy any assertion made against
    // the struct alone.
    let mut published = None;
    while let Some(m) = kiosk.next() {
        if m.contains(r#""kind":"media_transport""#) {
            published = Some(m);
        }
    }
    let wire: serde_json::Value =
        serde_json::from_str(&published.expect("nothing reached the screens")).expect("valid JSON");
    assert_eq!(
        wire["replay_epoch"].as_u64(),
        Some(first.replay_epoch),
        "the console was handed a different replay epoch from the screens"
    );
    assert!(
        first.replay_epoch > 0,
        "a replay that bumped no counter cannot reach a screen at all"
    );
    assert!(!first.paused, "replay means the clip is running");

    // AND A SCRUB CARRIES ITS BASELINE. Without `started_at` the corrector on
    // every page drags the clip back to where the fire implied within two
    // seconds — the operator moves the handle, the picture jumps back, and the
    // product looks broken. The console needs it for exactly the same reason:
    // its preview runs the same corrector.
    let scrubbed = set_media_transport(
        h.clone(),
        h.state::<channels::MediaTransport>(),
        None,
        None,
        None,
        Some(42_000),
        None,
    )
    .expect("scrub");
    assert_eq!(scrubbed.seek_ms, 42_000);
    assert!(
        scrubbed.seek_epoch > first.seek_epoch,
        "a scrub that bumped no counter is a frame a screen has already seen"
    );
    assert!(
        scrubbed.started_at.is_some(),
        "the scrub carried no baseline, so every screen will undo it"
    );
    assert_eq!(
        scrubbed.replay_epoch, first.replay_epoch,
        "a scrub moved the replay counter, which would restart every clip"
    );
}

/// THE DESK SETS A SECOND CLOCK AND THE RAIL CARRIES ONE — RG-250, END TO END.
///
/// `timers::tests` proves the registry rule and the stage page's own suites prove
/// the render. Neither drives the DOOR: `start_timer` is what the dock, the timer
/// desk, a plan cue and a room all call, and until this test existed the claim
/// "an operator sets a second timer and the first one goes" was assembled from
/// two halves that had never been run together.
///
/// It asserts on the FRAME rather than the registry, because the frame is what a
/// preacher's screen is painted from — a registry that is right and a frame that
/// is stale is exactly the failure rule 35 keeps finding.
#[test]
fn a_second_stage_timer_set_from_the_desk_leaves_one_clock_on_the_rail() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    start_timer(
        h.clone(),
        25.0,
        "Sermon".into(),
        String::new(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("the first programme timer");
    settle();

    let congregation = start_timer(
        h.clone(),
        5.0,
        "Service begins in".into(),
        "Welcome".into(),
        "both".into(),
        None,
        None,
        None,
    )
    .expect("a congregation countdown");

    let second = start_timer(
        h.clone(),
        2.0,
        "Notices".into(),
        String::new(),
        "stage".into(),
        None,
        None,
        None,
    )
    .expect("a second programme timer");
    settle();

    // THE LAST FRAME IS WHAT THE SCREEN IS HOLDING. An earlier one carrying two
    // rows would be a rail that flickered rather than a rail that is wrong, and
    // the assertion has to be about the state it settles in.
    let mut frames = Vec::new();
    while let Some(m) = kiosk.next() {
        if m.contains(r#""kind":"timer""#) {
            frames.push(m);
        }
    }
    let last = frames
        .last()
        .expect("setting a timer told the stage tablet nothing");
    let v: serde_json::Value = serde_json::from_str(last).expect("valid JSON");
    let rows = v["timers"].as_array().expect("a timers array");
    assert_eq!(
        rows.len(),
        1,
        "the preacher's rail is carrying more than one clock: {last}"
    );
    assert_eq!(
        rows[0]["id"].as_i64(),
        Some(second),
        "the rail kept the clock the operator replaced: {last}"
    );
    assert_eq!(rows[0]["label"].as_str(), Some("Notices"));

    // AND THE CONGREGATION'S COUNTDOWN IS UNTOUCHED. It is not on this frame at
    // all — the stage frame is `Scope::Stage` only — so the registry is where
    // that half is read.
    let ids: Vec<i64> = list_timers(h.clone())
        .expect("list")
        .iter()
        .map(|t| t.timer.id)
        .collect();
    assert!(
        ids.contains(&congregation),
        "setting a stage clock took the congregation's countdown: {ids:?}"
    );
}

/// ENDING THE SERVICE TAKES THE PROGRAMME CLOCKS, AND LEAVES THE CONGREGATION'S.
///
/// `TimerRegistry` never reaps, and until this landed nothing ever stopped a
/// `Stage` timer: a service's cue clocks stayed on the preacher's rail, counting
/// past zero, for as long as Relay was open — and the rail's floor keeps the
/// OLDEST cells, so by the middle of a morning the clock a preacher was looking
/// for was the one inside `+N more` (RG-163, filed by wave 4 as RG-147).
///
/// `Live::retireCueTimer` ends each cue's clock as the plan walks past it, which
/// is the half that matters during a service. This is the sweep behind it, at the
/// one moment the whole programme really is over — and it is HERE rather than at
/// the two controls that call `end_service` (the dock, and the History list),
/// because a rule kept at call sites is the shape of four separate bugs in this
/// repository.
///
/// The `Both` half is the other half of the claim and is not decoration: a
/// congregation countdown is on a wall, and emptying it from here would be a
/// second door onto that screen. `Clear screens` is how a wall is taken back.
#[test]
fn ending_a_service_takes_the_programme_clocks_off_the_preachers_rail() {
    let app = app();
    let h = app.handle().clone();

    start_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<Db>(),
        h.state::<channels::Rehearsal>(),
        h.state::<servicelock::ServiceLock>(),
        "Sunday Service".into(),
        "2026-09-20".into(),
    )
    .expect("start");

    let sermon = start_timer(
        h.clone(),
        25.0,
        "Sermon".into(),
        String::new(),
        "stage".into(),
        Some(120_000),
        Some(7),
        None,
    )
    .expect("a programme timer");
    // ONE STAGE CLOCK, BECAUSE THERE CAN ONLY BE ONE (RG-250). This started a
    // second programme timer, `Notices`, and asserted that ending the service
    // took both. A new stage timer now replaces the one that was running, so the
    // second start would take `Sermon` before `end_service` was ever called and
    // the assertion below would pass over a clock nothing had ended. A test that
    // cannot fail is the thing rule 34's neighbours keep warning about.
    let wall_clock = start_timer(
        h.clone(),
        5.0,
        "Service begins in".into(),
        "Welcome".into(),
        "both".into(),
        None,
        None,
        None,
    )
    .expect("a congregation countdown");
    settle();
    assert_eq!(list_timers(h.clone()).expect("list").len(), 2);

    let mut kiosk = qa::Kiosk::attach(&h);
    end_service(
        h.clone(),
        h.state::<Session>(),
        h.state::<servicelock::ServiceLock>(),
    )
    .expect("end");
    settle();

    let left: Vec<i64> = list_timers(h.clone())
        .expect("list")
        .iter()
        .map(|t| t.timer.id)
        .collect();
    assert!(
        !left.contains(&sermon),
        "a finished service left its programme clock running on the preacher's \
         screen: {left:?}"
    );
    assert!(
        left.contains(&wall_clock),
        "ending the service reached a congregation countdown, which is a second \
         door onto a wall"
    );

    // AND THE TABLET IS TOLD. An absent frame cannot say "there are none now": a
    // rail that is never sent the empty set goes on painting the clocks it has.
    let mut frames = Vec::new();
    while let Some(m) = kiosk.next() {
        if m.contains(r#""kind":"timer""#) {
            frames.push(m);
        }
    }
    let last = frames
        .last()
        .expect("ending the service told the stage tablet nothing");
    let v: serde_json::Value = serde_json::from_str(last).expect("valid JSON");
    assert_eq!(
        v["timers"].as_array().map(|a| a.len()),
        Some(0),
        "the preacher's rail was not told the programme is over: {last}"
    );
}

// ══ PER-SCREEN CLEAR AND BLACKOUT ═══════════════════════════════════════════
//
// "Take the lobby TV down but leave the wall live" is an ordinary request and was
// impossible: `clear_screens` and `blackout` take no channel argument. These
// drive the three new commands against the real app, through the real hub, and
// the first thing every one of them asserts is what did NOT happen to the wall.

/// ONE SCREEN GOES DOWN AND THE WALL DOES NOT.
///
/// The claim in one sentence: after `clear_screen(4)` the lobby TV is told to
/// blank and nothing else in the building is told anything at all.
///
/// Watched to fail by having `clear_screen` call `channels::clear` (which is what
/// "add a channel argument to the panic control" would collapse into): the global
/// `clear` frame appears, `live_content` goes to `None`, and both assertions below
/// go red — which is the whole reason the split is in the CALL.
#[test]
fn one_screen_goes_down_and_the_wall_stays_live() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .expect("fire");
    settle();
    while kiosk.next().is_some() {} // drain the fire

    clear_screen(h.clone(), 4).expect("one screen must be able to go down");
    settle();

    let frames: Vec<String> = std::iter::from_fn(|| kiosk.next()).collect();
    let joined = frames.join("|");
    assert!(
        joined.contains(r#""kind":"screen_state""#) && joined.contains(r#""4":"clear""#),
        "the lobby TV was never told to blank: {frames:?}"
    );
    // THE PART THAT MATTERS MORE. A per-screen control that published a global
    // clear would look identical on the lobby TV and would have taken the
    // congregation's wall with it.
    assert!(
        !joined.contains(r#""kind":"clear""#),
        "taking one screen down published a WALL clear: {frames:?}"
    );
    assert!(
        !wall.cleared(),
        "taking one screen down cleared every native output window too"
    );
    // And Relay still knows what is on the screens, because it still is. Forgetting
    // it here would make the next spoken "next verse" answer `NoPassage` over a
    // verse the congregation can see.
    assert!(
        channels::live_content(&h).is_some(),
        "taking one screen down made Relay forget the verse that is still on the wall"
    );
}

/// THE TOTAL CONTROLS ARE EXACTLY WHAT THEY WERE.
///
/// Rule 15 and DECISIONS §20: first, largest, one action, every screen, no
/// question asked about which. This asserts the panic path did not learn about
/// channels — the frame it publishes names none, and it reaches the wall whether
/// or not a screen has been taken down on its own.
#[test]
fn the_total_controls_never_learned_about_channels() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .expect("fire");
    settle();
    clear_screen(h.clone(), 4).expect("take the lobby TV down first");
    settle();
    while kiosk.next().is_some() {}

    clear_screens(h.clone()).expect("the panic control must work with a screen already down");
    settle();

    let frames: Vec<String> = std::iter::from_fn(|| kiosk.next()).collect();
    let joined = frames.join("|");
    assert!(
        joined.contains(r#"{"kind":"clear"}"#),
        "the panic control published something other than the whole-wall clear: {frames:?}"
    );
    assert!(
        wall.cleared(),
        "the panic control did not reach the native output windows"
    );
    assert!(
        channels::live_content(&h).is_none(),
        "the panic control left Relay believing a verse is still on the screens"
    );

    // AND BLACKOUT, SEPARATELY. The two are handled in one branch on every client
    // and have been forgotten one at a time before now (DECISIONS §91).
    blackout(h.clone()).expect("blackout must work with a screen already down");
    settle();
    assert!(wall.blacked(), "the blackout did not reach the wall");
}

/// A SCREEN TAKEN DOWN STAYS DOWN ACROSS A FIRE.
///
/// The durability that makes the control worth having. A one-shot frame would be
/// undone by the next verse — which during a service is within a minute — so
/// "take the lobby TV down for the sermon" would be a control nobody could use.
///
/// The way back is a control and not a side effect: `restore_screen` publishes the
/// set with that screen gone from it, which is how a page learns it is up again.
#[test]
fn a_screen_taken_down_stays_down_until_it_is_put_back() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    blackout_screen(h.clone(), 4).expect("take the lobby TV down");
    settle();
    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .expect("fire");
    settle();

    assert_eq!(
        h.state::<channels::ScreensDown>().get(4),
        Some(channels::ScreenState::Black),
        "firing a verse brought a screen the operator had taken down back up"
    );
    while kiosk.next().is_some() {}

    restore_screen(h.clone(), 4).expect("put it back");
    settle();
    let frames: Vec<String> = std::iter::from_fn(|| kiosk.next()).collect();
    let joined = frames.join("|");
    assert!(
        joined.contains(r#""kind":"screen_state""#) && joined.contains(r#""screens":{}"#),
        "restoring published no set at all, so no screen could learn it was up: {frames:?}"
    );
    assert_eq!(
        h.state::<channels::ScreensDown>().get(4),
        None,
        "a restored screen is still recorded as down"
    );
}

/// A REHEARSAL REFUSES IT, AND SAYS SO.
///
/// Every other publisher in `channels.rs` suppresses during a rehearsal and
/// reports success, because what it is suppressing is content. This one refuses.
/// Suppressing it would leave the Outputs desk showing the lobby TV down while the
/// lobby TV showed the last thing it was sent, with nothing anywhere saying so —
/// rule 35's shape, on a control an operator pressed on purpose. It is not a panic
/// control, so it is allowed to refuse; `clear_screens` and `blackout` are, and
/// the test above holds that they still cannot.
#[test]
fn a_rehearsal_refuses_to_take_a_real_screen_down() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);
    h.state::<channels::Rehearsal>().set(true);

    let refused = clear_screen(h.clone(), 4);
    settle();
    assert!(
        refused.is_err(),
        "a rehearsal took a real screen out of a real wall"
    );
    assert!(
        kiosk.silent(),
        "a rehearsal published a screen state to a live LAN"
    );
    assert_eq!(
        h.state::<channels::ScreensDown>().get(4),
        None,
        "a refused control still changed Relay's own belief about the screen"
    );
}

// ══ RENAMING A SCREEN ═══════════════════════════════════════════════════════
//
// There was no command at all, and the name is the only handle anybody in the
// building has on a screen: the card, the badge, the shell's degraded banner
// ("3 is not responding" was the defect that put the name on `ChannelLiveness`)
// and the service's own timeline all say it.

/// A SCREEN CAN BE RENAMED, AND THE NAME IS TRIMMED ON THE WAY IN.
///
/// **What this deliberately does NOT reach:** `channel_status`, the read every
/// console surface actually makes, is not generic over `tauri::Runtime` and so
/// cannot be driven from a mock app at all (rule 24's argument, on a command that
/// is not on the fire path). It reads the name straight off the row this asserts
/// on, through `db::list_output_channels`, so nothing sits between the two — but
/// that is a reading of the code and not a run of it, and it is said here rather
/// than implied.
#[test]
fn a_screen_can_be_renamed_and_the_name_is_trimmed() {
    let app = app();
    let h = app.handle().clone();

    rename_channel(h.state::<Db>(), 4, "  Crèche  ".into()).expect("rename");

    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    let names: Vec<String> = db::list_output_channels(&conn)
        .expect("list")
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert!(
        names.contains(&"Crèche".to_string()),
        "the rename did not reach the row, or did not trim: {names:?}"
    );
}

/// THE THREE REFUSALS, EACH IN WORDS AN OPERATOR CAN ACT ON.
///
/// A blank name, a name longer than the four places that render it are sized for,
/// and an id that is not there any more. The third is the one that is easy to
/// leave out: `UPDATE` against a deleted row writes nothing and reports success,
/// so without the affected-rows check the operator would be shown a new name on a
/// screen that does not exist.
#[test]
fn a_screen_rename_refuses_in_words_rather_than_writing_nothing_quietly() {
    let app = app();
    let h = app.handle().clone();

    assert!(
        rename_channel(h.state::<Db>(), 4, "   ".into()).is_err(),
        "a screen was renamed to nothing at all"
    );
    assert!(
        rename_channel(h.state::<Db>(), 4, "x".repeat(61)).is_err(),
        "a name too long for every surface that renders it was accepted"
    );
    let gone = rename_channel(h.state::<Db>(), 9_999, "Ghost".into());
    assert!(
        gone.is_err(),
        "renaming a screen that does not exist reported success"
    );

    // And the refusals left the row alone. A validator that refuses AFTER writing
    // is a validator that has already done the damage.
    let db = h.state::<Db>();
    let conn = db.0.lock().expect("db");
    let names: Vec<String> = db::list_output_channels(&conn)
        .expect("list")
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert!(
        names.contains(&"Lobby screen".to_string()),
        "a refused rename still changed the screen: {names:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// DECISIONS §97 — a screen wears a different look for EACH KIND of content
// ─────────────────────────────────────────────────────────────────────────────

/// THE MAP LEAVES THE MACHINE ON BOTH DOORS, AND THE CONTENT FRAME DOES NOT MOVE.
///
/// The per-kind look is decided at the RECEIVER, because the receiver is the
/// screen: the hub broadcasts to everybody and records nothing about who
/// connected (DECISIONS §35), so the only party that knows which screen this is,
/// is that screen. What has to leave the machine is therefore the MAP, not a
/// routed fire — and it has to leave on both doors, because a native output
/// window has the Tauri bridge and no socket while a browser source has the
/// socket and no bridge.
///
/// This is the e2e-level claim: the real command, against a real database,
/// watched at the two doors. What each door then PAINTS is
/// `src/lib/channellookspage.test.js`, which drives the real page — a test's
/// assertion surface is part of its claim, and these two are different claims.
#[test]
fn r4_a_screen_wears_a_different_look_for_each_kind_on_both_doors() {
    let app = app();
    let h = app.handle().clone();
    let wall = qa::Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);

    // `qa::Wall` records what a congregation SEES — content, clear, black — so the
    // configuration event is listened for here rather than widened into the shared
    // fixture. A map of looks is not something anybody watches happen.
    let announced: std::sync::Arc<std::sync::Mutex<Vec<serde_json::Value>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    {
        let seen = announced.clone();
        h.listen("output://channel_looks", move |e| {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(e.payload()) {
                seen.lock().expect("lock").push(v);
            }
        });
    }

    let scripture = scratch_template(&h, "Meridian");
    let song = scratch_template(&h, "Chorus");
    let chan = {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        db::list_output_channels(&conn)
            .expect("channels")
            .first()
            .expect("a fresh install seeds screens")
            .id
    };

    super::set_channel_look(
        h.clone(),
        h.state::<Db>(),
        h.state::<channels::KioskHub>(),
        chan,
        "scripture".into(),
        Some(scripture),
    )
    .expect("a screen may wear a look for scripture");
    super::set_channel_look(
        h.clone(),
        h.state::<Db>(),
        h.state::<channels::KioskHub>(),
        chan,
        "song".into(),
        Some(song),
    )
    .expect("a screen may wear a look for songs");
    settle();

    // THE TAURI DOOR — what a native output window is told.
    let last = announced.lock().expect("lock").last().cloned().expect(
        "a native output window was never told what it wears for each kind, so \
             the projector on HDMI resolves every fire against an empty map",
    );
    assert_eq!(last["looks"][chan.to_string()]["scripture"], scripture);
    assert_eq!(last["looks"][chan.to_string()]["song"], song);

    // THE KIOSK DOOR — what every browser source is told, ids only.
    let frame = kiosk
        .drain()
        .into_iter()
        .rfind(|m| m.contains(r#""kind":"channel_looks""#))
        .expect(
            "no browser source was told what it wears for each kind — the OBS \
             source and the projector beside it would disagree about the same verse",
        );
    assert!(
        frame.contains(&format!(r#""scripture":{scripture}"#))
            && frame.contains(&format!(r#""song":{song}"#)),
        "got {frame}"
    );
    assert!(
        !frame.contains("Meridian") && !frame.contains("Chorus"),
        "the look map carried template BYTES — one look in the field was 13 MB, \
         and this frame is sent to every client on every hello: {frame}"
    );

    // AND THE CONTENT FRAME DOES NOT MOVE. A fire is unchanged by any of this:
    // per-kind looks are configuration, and a routed fire would be RG-161's
    // per-cue targeting arriving through the back door with none of its pieces.
    super::manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .expect("scripture fires");
    settle();
    let out = wall.last().expect("scripture reached the wall");
    assert!(
        out.get("channel").is_none(),
        "a fire named a screen — the publish is global and unrouted, and that is \
         what keeps RG-161 closed: {out}"
    );
}

/// A PANIC CONTROL TAKES EVERY SCREEN, WHATEVER LOOK IT WAS WEARING.
///
/// `clear_screens` and `blackout` address every screen and ask nothing about
/// which — that is what makes them panic controls (rule 15, DECISIONS §20). There
/// is no channel and no kind anywhere on that path, and the moment there is one,
/// a screen can be configured out of a panic control.
///
/// Asserted by shape rather than by outcome: the frames a wipe publishes are
/// checked for the absence of the two words, so a future "clear this screen's
/// scripture" would fail here even if it happened to clear everything today.
#[test]
fn r4_a_panic_control_takes_every_screen_whatever_look_it_was_wearing() {
    let app = app();
    let h = app.handle().clone();
    let mut kiosk = qa::Kiosk::attach(&h);

    let look = scratch_template(&h, "Meridian");
    let chan = {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        db::list_output_channels(&conn)
            .expect("channels")
            .first()
            .expect("seeded screens")
            .id
    };
    super::set_channel_look(
        h.clone(),
        h.state::<Db>(),
        h.state::<channels::KioskHub>(),
        chan,
        "scripture".into(),
        Some(look),
    )
    .expect("a look is set");
    super::manual_fire(
        h.clone(),
        h.state::<Db>(),
        "John 3:16".into(),
        None,
        None,
        None,
    )
    .expect("scripture fires");
    settle();
    let _ = kiosk.drain();

    assert!(super::clear_screens(h.clone()).is_ok());
    assert!(super::blackout(h.clone()).is_ok());
    settle();

    let after = kiosk.drain();
    let wipes: Vec<&String> = after
        .iter()
        .filter(|m| m.contains(r#""kind":"clear""#) || m.contains(r#""kind":"black""#))
        .collect();
    assert_eq!(
        wipes.len(),
        2,
        "a panic control did not reach the kiosk hub at all: {after:?}"
    );
    for frame in wipes {
        assert!(
            !frame.contains("channel") && !frame.contains("kind\":\"scripture"),
            "a panic control named a screen or a kind — a screen an operator can \
             configure out of a blackout is rule 15's exact failure: {frame}"
        );
    }
}
/// FIELD 2026-09-20 · service 24 · 23.5 min. Psalms 55 on the wall by hand; the
/// preacher quotes Hosea 6:1 with the book misheard. **Psalms 55:1 auto-fired at
/// 0.88.** Through the real path: the bare verse is OFFERED as `uncertain_book`,
/// the wall is left alone, and the operator decides. RG-32's "wants a second
/// Sunday" got its second Sunday.
#[test]
fn a_verse_answered_from_memory_is_offered_never_fired() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);
    let offered: std::sync::Arc<std::sync::Mutex<Vec<serde_json::Value>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = offered.clone();
    h.listen("detection://match", move |e| {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(e.payload()) {
            sink.lock().unwrap().push(v);
        }
    });

    manual_fire(
        h.clone(),
        h.state::<Db>(),
        "Psalms 55:22".into(),
        None,
        None,
        None,
    )
    .expect("the operator's own fire");
    settle();
    assert_eq!(wall.references(), vec!["Psalms 55:22".to_string()]);

    emit_detections(
        &h,
        "Out of a prophet called Osir. In verse 1 he says, Come and let us return unto the Lord.",
        0,
        true,
        None,
    );
    settle();

    assert_eq!(
        wall.references(),
        vec!["Psalms 55:22".to_string()],
        "a verse nobody said the book of reached the congregation unattended: {:?}",
        wall.last().map(|v| v["reference"].clone())
    );
    let got = offered.lock().unwrap();
    let psalm = got
        .iter()
        .find(|v| v["reference"] == "Psalms 55:1")
        .unwrap_or_else(|| panic!("Psalms 55:1 was not even offered: {got:?}"));
    assert_eq!(
        psalm["method"], "uncertain_book",
        "the offer must say the book came from memory, not from the preacher"
    );
    assert_eq!(psalm["status"], "suggested");
}

/// A RELAUNCH BRINGS THE CLOCKS BACK, AND PUTS NONE OF THEM ON A WALL — F28, DECISIONS §112.
///
/// The registry was in memory and nothing else. Here a stage clock and a
/// congregation countdown are saved the way the sink saves them, the process
/// "relaunches" (a fresh `app()` over the same rows), and `restore_timers` runs
/// as `setup` runs it. The stage tablet is told, because a programme clock reaches
/// it without a content frame; the congregation screens receive NOTHING, because
/// restoring is not re-airing — the countdown is in the registry for Put back.
/// Watched to fail with `publish_timers` removed from `restore_timers` (the stage
/// half) and with a `broadcast_content` added to it (the wall half).
#[test]
fn a_relaunch_brings_the_clocks_back_without_putting_one_on_a_wall() {
    let app = app();
    let h = app.handle().clone();
    let now = 1_700_000_000_000;
    let stage = timers::Timer {
        id: 3,
        label: "Sermon".into(),
        done_msg: String::new(),
        target_ms: now + 1_200_000,
        from_ms: now - 600_000,
        paused_ms: None,
        warn_ms: Some(300_000),
        scope: timers::Scope::Stage,
        configured_ms: 1_800_000,
        until_ms: None,
        plan_item_id: None,
        started_in_rehearsal: false,
        channels: None,
    };
    let wall_clock = timers::Timer {
        id: 5,
        scope: timers::Scope::Both,
        label: "Service starts in".into(),
        ..stage.clone()
    };
    {
        let db = h.state::<Db>();
        let conn = db.0.lock().unwrap();
        db::save_timers(&conn, 5, &[stage.clone(), wall_clock.clone()]).unwrap();
    }
    let wall = Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);

    let n = restore_timers(&h, now);
    settle();

    assert_eq!(n, 2);
    let reg = h.state::<timers::TimerRegistry>();
    assert_eq!(reg.get(3), Some(stage));
    assert_eq!(reg.get(5), Some(wall_clock));
    assert_eq!(
        reg.start(timers::Timer {
            id: 0,
            ..reg.get(3).unwrap()
        }),
        6,
        "ids resume above the restored ones"
    );

    // The stage was told, with the stage clock and only the stage clock.
    let frames = kiosk.drain();
    let timer_frames: Vec<&String> = frames.iter().filter(|f| f.contains("\"timers\"")).collect();
    assert!(
        !timer_frames.is_empty(),
        "the stage tablet must learn its clock on relaunch: {frames:?}"
    );
    assert!(timer_frames[0].contains("\"Sermon\""));
    assert!(
        !timer_frames[0].contains("Service starts in"),
        "a congregation countdown is not a stage frame"
    );
    // And no content frame left by either door.
    assert!(
        frames.iter().all(|f| !f.contains("content_kind")),
        "restoring put content on the kiosk: {frames:?}"
    );
    assert_eq!(wall.count(), 0, "restoring put content on the wall");
}

/// A CLEAN EXIT TAKES THE CLOCK A RELAUNCH WOULD PAINT BY ITSELF — RG-269.
///
/// The operator: *"When Application close clear all active timer running or if
/// not its running it should display on the right output...stage"*.
///
/// The test above is the promise (§112) and this one is its edge. `restore_timers`
/// publishes to the stage unconditionally, so a running stage clock left in the
/// registry at quitting time came back on a preacher's screen at the next launch,
/// counting from a moment that had passed, with nobody having asked for it. A
/// congregation countdown never had that problem: it comes back to the DESK and
/// waits behind **Put back on screens**, which is why it is deliberately left
/// alone here and the assertions below say so.
///
/// Three things are asserted, and the middle one is the load-bearing one: the
/// SAVED ROWS no longer carry the running clock. Stopping it only in memory would
/// look identical on this side of the exit and be worth nothing on the other,
/// because the sink writes on a thread of its own and a process that is quitting
/// does not wait for it.
///
/// Watched to fail with the exit hook not called: the stage clock is still in the
/// registry, still in the saved rows, and no frame goes to the tablet.
#[test]
fn a_clean_exit_stops_the_running_stage_clock_and_leaves_the_rest() {
    let app = app();
    let h = app.handle().clone();
    let now = 1_700_000_000_000;
    let running = timers::Timer {
        id: 3,
        label: "Sermon".into(),
        done_msg: String::new(),
        target_ms: now + 1_200_000,
        from_ms: now - 600_000,
        paused_ms: None,
        warn_ms: Some(300_000),
        scope: timers::Scope::Stage,
        configured_ms: 1_800_000,
        until_ms: None,
        plan_item_id: None,
        started_in_rehearsal: false,
        channels: None,
    };
    let held = timers::Timer {
        id: 4,
        label: "Notices".into(),
        paused_ms: Some(now + 300_000),
        ..running.clone()
    };
    let wall_clock = timers::Timer {
        id: 5,
        scope: timers::Scope::Both,
        label: "Service starts in".into(),
        ..running.clone()
    };
    {
        let db = h.state::<Db>();
        let conn = db.0.lock().unwrap();
        db::save_timers(&conn, 5, &[running, held, wall_clock]).unwrap();
    }
    assert_eq!(restore_timers(&h, now), 3);
    let wall = Wall::watch(&h);
    let mut kiosk = qa::Kiosk::attach(&h);
    settle();
    kiosk.drain();

    let n = stop_clocks_a_relaunch_would_paint(&h);
    settle();

    assert_eq!(n, 1, "one running stage clock, and only it");
    let reg = h.state::<timers::TimerRegistry>();
    assert!(
        reg.get(3).is_none(),
        "the sermon clock survived the quit and comes back counting"
    );
    assert!(
        reg.get(4).is_some(),
        "a HELD clock was taken — it was not running, and its figure is the one somebody parked"
    );
    assert!(
        reg.get(5).is_some(),
        "the congregation countdown was taken; DECISIONS §112 brings it back to the desk, never to a wall"
    );

    // THE HALF THAT SURVIVES THE PROCESS. The sink writes on its own thread and a
    // quitting process does not wait for it, so the exit hook writes here.
    let (next_id, rows) = {
        let db = h.state::<Db>();
        let conn = db.0.lock().unwrap();
        db::load_timers(&conn).unwrap()
    };
    assert_eq!(
        rows.iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![4, 5],
        "the saved rows still carry the running clock — the next launch paints it"
    );
    assert_eq!(next_id, 5, "an id may never be handed out twice");

    // AND THE STAGE IS TOLD. A browser source on another machine outlives Relay's
    // own window; without this it keeps rendering the last frame it was sent, so
    // the clock stays on the preacher's screen after Relay has gone.
    let frames = kiosk.drain();
    let timer_frames: Vec<&String> = frames.iter().filter(|f| f.contains("\"timers\"")).collect();
    assert!(
        !timer_frames.is_empty(),
        "the stage was not told its clock had stopped: {frames:?}"
    );
    assert!(
        !timer_frames.last().unwrap().contains("\"Sermon\""),
        "the stage was told, and the stopped clock was still in the frame: {timer_frames:?}"
    );
    assert!(
        frames.iter().all(|f| !f.contains("content_kind")),
        "quitting put content on a screen: {frames:?}"
    );
    assert_eq!(wall.count(), 0, "quitting put content on the wall");
}

// ═══════════════════════════════════════════════════════════════════════════
// FOLLOWING THE READER · WHAT LEAVES THE MACHINE (DECISIONS §118)
// ═══════════════════════════════════════════════════════════════════════════
//
// The operator's instruction of 2026-09-23, twice and in writing: *"when a
// scripture is quoted make sure to fire it to screen as its confirmed"* and
// *"follow the verse whenever a preacher is reading a bible verse."*
//
// `router.rs` proves the gate decides correctly and `detection.rs` proves the
// evidence is what it says it is. This file answers the only question a
// congregation experiences: what came out.

/// A PREACHER READING A VERSE ALOUD PUTS IT ON THE WALL.
///
/// The positive control, and the whole of what was asked for. Nobody says a
/// reference here and nobody presses anything — the words are simply Romans 8:28,
/// in order, as the KJV has them.
#[test]
fn a_verse_read_aloud_goes_to_the_wall_with_nobody_pressing_anything() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    emit_detections(
        &h,
        "all things work together for good to them that love God to them who are the called according to his purpose",
        0,
        true,
        None,
    );
    settle();

    let shown = wall.last().expect("a verse read aloud reached nobody");
    assert_eq!(shown["reference"], "Romans 8:28");
}

/// A PHRASE TWO VERSES HOLD IS OFFERED, NEVER FIRED.
///
/// Judges 1:12 and Joshua 15:16 are word-for-word identical in the KJV, and this
/// is not a curiosity: reading 1,500 real verses back through `PhraseIndex`
/// produced 42 wrong verses at a run of eight words and **every one of them was a
/// pair like this**, never a longer run found elsewhere. Relay is not mishearing
/// here, it is choosing — and choosing between two verses is a guess about which,
/// which is rule 10 and RG-178 one door along.
#[test]
fn a_phrase_two_verses_share_word_for_word_is_offered_never_fired() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    emit_detections(
        &h,
        "and Caleb said he that smiteth Kirjathsepher and taketh it to him will I give Achsah my daughter to wife",
        0,
        true,
        None,
    );
    settle();

    assert_eq!(
        wall.count(),
        0,
        "Relay chose between two identical verses and put {:?} up unattended",
        wall.last().map(|v| v["reference"].clone())
    );
}

/// A SHORT QUOTATION IS STILL ONLY A SUGGESTION. The cap moved for one new
/// method, not for the class.
///
/// *"was a good man and a just"* is SEVEN words of Luke 23:50 and it is the
/// module's own example of the noise a shorter floor produced — a sentence about
/// somebody who died, spoken at a funeral. One word below `READING_RUN_WORDS`, so
/// it may be offered and can never be fired. Chosen deliberately over a run of
/// four, which would test `MIN_RUN_WORDS` and tell us nothing about the floor
/// this test is named for.
#[test]
fn a_short_quotation_still_asks_before_it_shows() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    emit_detections(
        &h,
        "Now our brother was a good man and a just, and he served this church faithfully for many years",
        0,
        true,
        None,
    );
    settle();

    assert_eq!(
        wall.count(),
        0,
        "a seven-word accidental quotation reached a congregation: {:?}",
        wall.last().map(|v| v["reference"].clone())
    );
}

/// AND THE CHURCH MAY TURN IT OFF, all the way out to the screens.
///
/// A switch that changes a setting and not the behaviour is the "Screens cleared"
/// lie in a different coat (rule 15), so the test is on what came out rather than
/// on what was stored.
#[test]
fn a_church_that_turns_it_off_gets_a_suggestion_instead() {
    let app = app();
    let h = app.handle().clone();

    set_follow_the_reader(h.clone(), app.state::<Db>(), app.state::<Routing>(), false)
        .expect("the switch refused");

    let wall = Wall::watch(&h);
    emit_detections(
        &h,
        "all things work together for good to them that love God to them who are the called according to his purpose",
        0,
        true,
        None,
    );
    settle();
    assert_eq!(
        wall.count(),
        0,
        "the switch was off and {:?} still reached a wall",
        wall.last().map(|v| v["reference"].clone())
    );

    // Back on, and the same words go up — so the test above is about the switch
    // and not about the path being broken.
    set_follow_the_reader(h.clone(), app.state::<Db>(), app.state::<Routing>(), true)
        .expect("the switch refused");
    let wall = Wall::watch(&h);
    emit_detections(
        &h,
        "all things work together for good to them that love God to them who are the called according to his purpose",
        60_000,
        true,
        None,
    );
    settle();
    assert_eq!(
        wall.last().expect("nothing came back with the switch on")["reference"],
        "Romans 8:28"
    );
}

/// THE NAMED REFERENCE WINS WHEN A WINDOW HOLDS BOTH.
///
/// One window may put at most ONE verse on a wall (rule 29), and a preacher who
/// says "Romans 8:28" while reading a different verse aloud has told Relay which
/// one they mean. Naming it is the words saying, and when the words say, the
/// words win (rule 40).
#[test]
fn a_reference_the_preacher_named_beats_one_they_only_read() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    // TWO THINGS HAD TO BE TRUE BEFORE THIS TEST COULD MEAN ANYTHING, and the
    // first two attempts had neither.
    //
    // (1) The reading must OUTSCORE the reference. Romans 8:38 gives a 21-word
    // run, which `quoted_confidence` caps at 0.95 against the spoken reference's
    // 0.88 — so a comparator ranking on "may this fire at all" and then on the
    // number puts the reading first. A seven-word run never becomes a reading and
    // the test passed with the comparator broken.
    //
    // (2) The reading must be in the SAME BOOK. Rule 40 is applied one step
    // earlier than this: a book the window names RESTRICTS the phrase index, so a
    // window saying "Romans" can produce no quotation outside Romans at all. A
    // cross-book version of this test also passed with the comparator broken, and
    // it was measuring the anchor rather than the ranking.
    emit_detections(
        &h,
        "our text this morning is Romans chapter eight verse twenty eight but first hear this for I am persuaded that neither death nor life nor angels nor principalities nor powers nor things present nor things to come",
        0,
        true,
        None,
    );
    settle();

    assert_eq!(
        wall.last().expect("nothing reached the wall")["reference"],
        "Romans 8:28",
        "the verse the preacher read aloud displaced the one they named"
    );
    assert_eq!(wall.count(), 1, "one window put two verses on a wall");
}
#[test]
fn dbg_rank() {
    let app = app();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);
    emit_detections(&h, "our text this morning is Romans chapter eight verse twenty eight but first hear this for I am persuaded that neither death nor life nor angels nor principalities nor powers nor things present nor things to come", 0, true, None);
    settle();
    println!("WALL count={} last={:?}", wall.count(), wall.last());
}

// ── SWITCHING TRANSLATION MUST MOVE THE DETECTORS TOO — RG-300 ──────────────
//
// `Semantic` and `Phrases` were built exactly ONCE, in `main.rs`'s `setup`, from
// `db::all_verses` — which scopes itself to the active translation.
// `set_active_translation` wrote the setting and stopped there, so after an
// operator switched from the KJV to the BSB both detectors went on scanning the
// PREVIOUS translation's corpus until the app was relaunched, while every verse
// READ was correctly scoped to the new one.
//
// The console would then show BSB words under a reference the paraphrase
// detector found using KJV vocabulary, and nothing on any surface would say the
// two disagreed — because both halves look exactly as they do when they agree.
// A wrong-verse risk wearing a settings bug's clothes.
//
// THE TEST ASKS THE INDEX, not the setting. Asserting that the command wrote
// `active_translation` is asserting the half that was never broken.
#[test]
fn switching_translation_rebuilds_the_indexes_that_read_it() {
    let app = qa::bare_app();
    let h = app.handle().clone();

    // Two translations, and the second one's words are NOT the first's. The text
    // is deliberately unlike anything in the KJV so a hit can only come from the
    // new corpus.
    let (other_id, marker) = {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        conn.execute(
            "INSERT INTO translations (name, abbreviation, language, license_type)
             VALUES ('Test Version', 'TSTV', 'en', 'public_domain')",
            [],
        )
        .expect("insert translation");
        let tid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO verses (translation_id, book, chapter, verse, text)
             VALUES (?1, 'John', 3, 16, 'zarquon vellichor sonder kenopsia liberosis')",
            rusqlite::params![tid],
        )
        .expect("insert verse");
        (tid, "zarquon vellichor sonder kenopsia liberosis")
    };

    // BEFORE: the words are in the database and the index has never seen them.
    {
        let sem = h.state::<Semantic>();
        let idx = sem.0.read().expect("semantic");
        assert!(
            idx.top_k_explained(marker, 5).is_empty(),
            "the index already knew a translation nobody has switched to"
        );
    }

    set_active_translation(
        h.clone(),
        h.state::<Db>(),
        h.state::<servicelock::ServiceLock>(),
        other_id,
    )
    .expect("set_active_translation");

    // AFTER: the detectors are reading the corpus the operator chose.
    let sem = h.state::<Semantic>();
    let idx = sem.0.read().expect("semantic");
    let hits = idx.top_k_explained(marker, 5);
    assert!(
        !hits.is_empty(),
        "the paraphrase detector is still scanning the translation that was \
         switched away from"
    );

    let phrases = h.state::<Phrases>();
    let pidx = phrases.0.read().expect("phrases");
    assert!(
        !pidx.quoted(marker, None, 5).is_empty(),
        "the quotation detector is still scanning the old translation"
    );
}
