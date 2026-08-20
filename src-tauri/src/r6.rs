//! R6 · Independent Auditor — layer-A evidence, written without reading R1–R5.
//!
//! These drive the REAL `remote_api` control plane (the preacher's phone) against a
//! real in-memory DB and the real fire/clear engine, in the `e2e.rs` style. They exist
//! to answer one question the other doors have already taught this repo to ask:
//!
//!   > A guarantee is only kept on the doors you checked.
//!
//! The console honours "what is on the wall". The LAN remote answers the same question
//! from a DIFFERENT source of truth — `Context::current()`, the passage anchor — and
//! nothing has ever compared the two.
//!
//! Tests marked RED-ON-PURPOSE are findings, not regressions. Do not "fix" them by
//! changing the assertion.

use super::*;
use std::sync::Arc;
use tauri::{Listener, Manager};

/// A headless Relay seeded EXACTLY as a fresh install, with no fixture convenience.
///
/// This was a hand-rolled copy of `qa::bare_app()` — written independently during the
/// audit, and by the next day already out of date: it did not manage `WallState`, so
/// `/api/live` answered "clear" for every test built on it. That is the drift `qa.rs`
/// exists to prevent, committed inside the audit that recommended `qa.rs`. One
/// fixture, or two fixtures that disagree.
fn first_launch() -> tauri::App<tauri::test::MockRuntime> {
    crate::qa::bare_app()
}

fn settle() {
    std::thread::sleep(std::time::Duration::from_millis(60));
}

/// What actually left the machine through the Tauri door.
#[derive(Default)]
struct Wall {
    content: Arc<Mutex<Vec<serde_json::Value>>>,
    cleared: Arc<AtomicBool>,
}
impl Wall {
    fn watch(app: &tauri::AppHandle<tauri::test::MockRuntime>) -> Self {
        let w = Wall::default();
        let c = w.content.clone();
        app.listen("output://content", move |e| {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(e.payload()) {
                c.lock().unwrap().push(v);
            }
        });
        let cl = w.cleared.clone();
        app.listen("output://clear", move |_| cl.store(true, Ordering::SeqCst));
        w
    }
    fn count(&self) -> usize {
        self.content.lock().unwrap().len()
    }
}

fn api(app: &tauri::AppHandle<tauri::test::MockRuntime>, rest: &str) -> serde_json::Value {
    serde_json::from_str(&remote_api(app, crate::remote_verb(rest), rest).body)
        .expect("the remote always answers JSON")
}

// ───────────────────────────────────────────────────────────────────────────────
// R6-1 · The remote's "what is live" is the passage anchor, not the wall.
// ───────────────────────────────────────────────────────────────────────────────

/// FIXED 2026-08-15 (R2-A / R6-1). Kept as the regression guard.
///
/// `/api/live` read `Context::current()` — the passage ANCHOR, which deliberately
/// survives a clear because that is what makes `→` resume rather than restart — and
/// published it under the key `live`. So after the operator hit Escape, the
/// preacher's phone still named a verse under the word "live". Cued ≠ On Air,
/// violated on the one surface whose holder cannot look up and check.
///
/// The repair added `channels::WallState`, maintained at the three choke points that
/// change what a congregation sees (`broadcast_content`, `clear`, `black`) and
/// nowhere else. The anchor still rides, under the honest name `cued`.
#[test]
fn r6_1_the_remote_still_names_a_live_verse_after_the_screens_are_cleared() {
    let app = first_launch();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None).expect("fire");
    settle();
    assert_eq!(wall.count(), 1, "the verse reached the wall");
    assert_eq!(api(&h, "live")["live"]["reference"], "John 3:16");

    clear_screens(h.clone()).expect("clear");
    settle();
    assert!(
        wall.cleared.load(Ordering::SeqCst),
        "the wall really was cleared"
    );

    let after = api(&h, "live");
    assert_eq!(
        after["live"],
        serde_json::Value::Null,
        "R6-1: the wall is blank, so the remote must not still be naming a live verse. \
         `/api/live` reads Context::current(), which `clear_screens` never resets, so \
         the preacher's phone reads `John 3:16` over an empty screen — and the two \
         panic routes on the SAME remote (`/api/clear`, `/api/black`) leave it that way."
    );
}

/// FIXED 2026-08-15 (R2-B / R6-2). Kept as the regression guard.
///
/// Rehearsal's contract is "nothing leaves the machine". `broadcast_content`, `clear`,
/// `black` and `stage_next` all honoured it. `/api/live` was a fifth door — a PULL
/// rather than a push, which is exactly why every enumeration missed it — and it
/// handed the rehearsal's verse to any LAN client that asked, byte-identically to a
/// real fire. The answer now carries `rehearsing` so the phone can say why it is
/// empty, rather than being silently blank.
#[test]
fn r6_2_a_rehearsal_verse_is_readable_over_the_lan() {
    let app = first_launch();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    h.state::<channels::Rehearsal>().set(true);
    manual_fire(h.clone(), h.state::<Db>(), "Psalm 23:1".into(), None, None).expect("fire");
    settle();

    // The gate held on the push doors: the console saw it, the wall did not.
    // (In this headless app there is no kiosk hub, so the only push door is Tauri.)
    let pulled = api(&h, "live");
    assert_eq!(
        pulled["live"],
        serde_json::Value::Null,
        "R6-2: during a rehearsal the LAN must learn nothing. `/api/live` answered \
         {pulled} — the rehearsal verse, off the machine, to anyone on the church wifi. \
         Same class as the `stage_next` leak: a door nobody put on the list."
    );
    let _ = wall.count();
}

// ───────────────────────────────────────────────────────────────────────────────
// R6-3 · The route surface has not grown, and unknown routes are refused.
//        (GREEN — this is the decided position holding.)
// ───────────────────────────────────────────────────────────────────────────────

#[test]
fn r6_3_the_remote_answers_exactly_the_seven_decided_routes() {
    let app = first_launch();
    let h = app.handle().clone();

    for r in ["search", "fire", "next", "prev", "clear", "black", "live"] {
        let v = api(&h, r);
        assert!(
            v.get("ok").is_some(),
            "{r} must be a real route that answers"
        );
    }
    for r in [
        "",
        "status",
        "settings",
        "db",
        "shutdown",
        "history",
        "templates",
        "channels",
        "logs",
        "telemetry",
        "capture",
        "rehearsal",
    ] {
        assert_eq!(
            api(&h, r)["error"],
            "unknown",
            "R6-3: /api/{r} must not exist — the decided surface is exactly \
             search/fire/next/prev/clear/black/live"
        );
    }
}

/// The remote's JSON must survive hostile input, because a broken body is a phone
/// that shows nothing and an operator who does not know why.
#[test]
fn r6_4_a_hostile_search_query_still_produces_valid_json() {
    let app = first_launch();
    let h = app.handle().clone();
    for q in [
        "search?q=%22%5C%22%22",
        "search?q=%3Cscript%3Ealert(1)%3C/script%3E",
        "search?q=John%203%3A16%00",
        "search?q=%F0%9F%94%A5%20fire",
        "search?q=Yor%C3%B9b%C3%A1%20%E1%BB%8Dl%E1%BB%8Dr%E1%BB%8Dn",
        "fire?ref=%3Cimg%20src%3Dx%20onerror%3Dalert(1)%3E",
    ] {
        let raw = remote_api(&h, crate::remote_verb(q), q).body;
        serde_json::from_str::<serde_json::Value>(&raw)
            .unwrap_or_else(|e| panic!("R6-4: /api/{q} produced invalid JSON ({e}): {raw}"));
    }
}

/// A reference the remote fires must be refused when it is not in the Bible, and the
/// refusal must SAY so — not blank the wall.
#[test]
fn r6_5_the_remote_refuses_a_verse_that_does_not_exist_without_blanking_the_wall() {
    let app = first_launch();
    let h = app.handle().clone();
    let wall = Wall::watch(&h);

    manual_fire(h.clone(), h.state::<Db>(), "John 3:16".into(), None, None).expect("fire");
    settle();
    let before = wall.count();

    let v = api(&h, "fire?ref=John%203%3A99");
    assert_eq!(v["ok"], false, "John 3:99 is not a verse");
    assert!(
        v["error"].as_str().unwrap_or("").contains("John 3:99"),
        "the refusal must name the reference: {v}"
    );
    settle();
    assert_eq!(
        wall.count(),
        before,
        "a refused fire must not push anything — least of all an empty verse"
    );
}

// ───────────────────────────────────────────────────────────────────────────────
// R6-6 · The auto-fire cap, attacked from the remote rather than the detector.
// ───────────────────────────────────────────────────────────────────────────────

#[test]
fn r6_6_no_confidence_lets_a_paraphrase_past_the_cap() {
    use crate::detection::DetectionMethod;
    use crate::router::{RouteDecision, Router};
    let mut r = Router::default();
    for m in [DetectionMethod::Semantic, DetectionMethod::Ambiguous] {
        for c in [0.5_f32, 0.9, 0.999, 1.0, 1.5, f32::INFINITY] {
            assert_eq!(
                r.decide("John 3:16", c, m, 0),
                RouteDecision::Suggest,
                "R6-6 (P0 if this ever fails): {m:?} at confidence {c} must cap at Suggest"
            );
        }
    }
    // NaN is the one input a `>=` comparison answers "no" to for every threshold —
    // confirm it drops rather than fires.
    for m in [DetectionMethod::Semantic, DetectionMethod::Ambiguous] {
        assert_ne!(
            r.decide("John 3:16", f32::NAN, m, 0),
            RouteDecision::AutoFire,
            "R6-6: a NaN confidence must never auto-fire"
        );
    }
    assert_ne!(
        r.decide("John 3:16", f32::NAN, DetectionMethod::Direct, 0),
        RouteDecision::AutoFire,
        "R6-6: a NaN confidence must never auto-fire, on the Direct path either"
    );
}

/// There is exactly ONE baseline (CLAUDE.md), and it must be true by construction.
#[test]
fn r6_7_one_baseline_only() {
    use crate::router::Thresholds;
    // `Thresholds` derives no `PartialEq`, so the invariant cannot be asserted as an
    // equality — compare the fields, and note in the report that "by construction"
    // rests on `Default` delegating to `from_sensitivity(DEFAULT_SENSITIVITY)`, which
    // it does (router.rs:33-37). That IS by construction; this test is the belt.
    let d = Thresholds::default();
    let s = Thresholds::from_sensitivity(50);
    assert_eq!((d.auto_fire, d.suggest), (s.auto_fire, s.suggest));
}

// ───────────────────────────────────────────────────────────────────────────────
// R6-8 · Detection, scored THROUGH the router — never by reading the transcript.
// ───────────────────────────────────────────────────────────────────────────────

/// What Relay would actually put on the wall for this sentence, and what it would
/// merely offer. Nothing here reads the transcript; the only question is which verse
/// reaches a screen.
fn wall_and_offer(text: &str) -> (Vec<String>, Vec<String>) {
    use crate::router::{RouteDecision, Router};
    let mut r = Router::default();
    let (mut fired, mut offered) = (vec![], vec![]);
    for m in detection::detect_direct(text) {
        let key = format!(
            "{} {}:{}",
            m.reference.book, m.reference.chapter, m.reference.verse
        );
        match r.decide(&key, m.confidence, m.method, 0) {
            RouteDecision::AutoFire => fired.push(key),
            RouteDecision::Suggest => offered.push(key),
            RouteDecision::Drop => {}
        }
    }
    (fired, offered)
}

/// Code-switching is the NORMAL case: a Yoruba/Swahili/Hausa book name with the
/// chapter and verse spoken in English. This records what actually happens today —
/// it is the honest state of the moat, not an aspiration.
#[test]
fn r6_8_code_switched_references_are_scored_through_the_router() {
    let cases: &[(&str, &str)] = &[
        ("Let us turn to Yohana 3:16", "John 3:16"),
        ("open your Bibles to Zaburi 23:1", "Psalms 23:1"),
        ("we read from Ìwé Jóhánù 3:16 this morning", "John 3:16"),
        ("Yahaya 3:16 says", "John 3:16"),
        ("turn with me to Mwanzo 1:1", "Genesis 1:1"),
    ];
    let mut report = String::new();
    for (text, want) in cases {
        let (fired, offered) = wall_and_offer(text);
        report.push_str(&format!(
            "  {text:60} → wall {fired:?} offer {offered:?} (want {want})\n"
        ));
    }
    println!("R6-8 code-switching, through the router:\n{report}");
    // Not asserted as a pass/fail: the point is the RECORD. `cargo test r6_8 -- --nocapture`
    // prints it, and the audit quotes it.
}

/// FALSE POSITIVES are the failure a congregation sees. Text that merely SOUNDS
/// scriptural must reach the wall exactly never.
#[test]
fn r6_9_scripture_flavoured_speech_does_not_reach_the_wall() {
    let noise = [
        "and I say to you today, brothers and sisters, be of good courage",
        "we sang hymn number three sixteen this morning",
        "the offering last week was three thousand one hundred and sixteen pounds",
        "my son turned twenty three on the first of March",
        "see you at half past six in room two twelve",
        "call the church office on zero one two one three four five",
        "Peter said he would bring eight chairs and twenty eight cups",
        "romans built roads across the whole empire, chapter and verse",
        "let us welcome brother John, three of his children are here",
        "the bus leaves at John Street at three sixteen",
    ];
    let mut leaks = vec![];
    for t in noise {
        let (fired, _) = wall_and_offer(t);
        if !fired.is_empty() {
            leaks.push(format!("{t:?} → {fired:?}"));
        }
    }
    assert!(
        leaks.is_empty(),
        "R6-9: ordinary speech reached the congregation's wall:\n  {}",
        leaks.join("\n  ")
    );
}

/// A garbled ASR reference must not be laundered into confidence. The repo's own
/// cautionary tale is a hallucinated "Peter 8 verse 28" being scored a success.
#[test]
fn r6_10_a_garbled_reference_does_not_auto_fire_a_confident_wrong_verse() {
    for t in [
        "Peter 8 verse 28",
        "second Timothy chapter ninety nine verse four",
        "the book of Hezekiah chapter two",
        "Psalms one hundred and seventy verse one",
    ] {
        let (fired, offered) = wall_and_offer(t);
        println!("R6-10 {t:40} → wall {fired:?} offer {offered:?}");
        for k in &fired {
            assert!(
                !k.starts_with("Hezekiah"),
                "R6-10: invented a book and fired it: {k}"
            );
        }
    }
}

/// R6-11 · A SWEEP, not a spot check. Ordinary church-service sentences whose only
/// crime is putting a number after an English noun.
#[test]
fn r6_11_the_fuzzy_book_repair_sweep() {
    let nouns = [
        "number", "room", "page", "song", "hymn", "point", "part", "year", "line", "track", "seat",
        "age", "row", "box", "week", "day", "item", "level", "gate", "exit", "bus", "flat", "unit",
        "form", "team", "group", "class", "grade", "section", "volume", "lot", "code", "phone",
        "table", "note", "card", "door", "hall", "date", "grant", "band", "cost", "total", "aisle",
        "block", "camp", "coach", "desk", "email", "entry", "extra", "field", "floor", "fund",
        "half", "hour", "issue", "job", "key", "list", "mile", "month", "order", "page", "pair",
        "panel", "phase", "place", "plan", "post", "price", "queue", "rank", "rate", "round",
        "rule", "scene", "score", "seat", "shelf", "shift", "side", "sign", "size", "slot",
        "space", "stage", "stall", "step", "stop", "suite", "table", "tape", "term", "test",
        "text", "ticket", "tier", "title", "tone", "topic", "tour", "train", "type", "van",
        "verse", "video", "view", "visit", "voice", "wall", "zone", "grade", "hymnal", "chorus",
        // "psalm" was in this list and should not have been. It is not an ordinary
        // noun that happens to collide with a book — it IS how every preacher says
        // the book, and "Psalm three sixteen" → Psalms 3:16 is the single most
        // common correct detection Relay makes. The sweep flagged four true
        // positives; removing it is a fix to the ruler, not to the thing measured.
        // (Contrast "song" and "job", which really are everyday words and are now
        // gated on a chapter/verse keyword — see `ORDINARY_WORD_ALIASES`.)
        "reading", "lesson", "prayer", "notice",
    ];
    let numbers = [
        "three sixteen",
        "two twelve",
        "one one",
        "twenty three",
        "four five",
    ];
    let mut hits: Vec<String> = Vec::new();
    for n in nouns {
        for num in numbers {
            let s = format!("{n} {num}");
            let (fired, _) = wall_and_offer(&s);
            if !fired.is_empty() {
                hits.push(format!("{s:32} → WALL {fired:?}"));
            }
        }
    }
    hits.sort();
    hits.dedup();
    println!(
        "R6-11 · {} ordinary phrases auto-fire scripture:",
        hits.len()
    );
    for h in &hits {
        println!("   {h}");
    }
    assert!(
        hits.is_empty(),
        "R6-11: {} ordinary English phrases put scripture on the congregation's wall \
         with no operator involvement. See the printed list.",
        hits.len()
    );
}

/// The same thing said the way a person actually says it, in a whole sentence.
#[test]
fn r6_12_real_church_sentences() {
    let sentences = [
        "please turn to hymn number three sixteen",
        "we will sing hymn number one one",
        "the youth meet in room two twelve after the service",
        "the notice is on page four five of your bulletin",
        "our giving code is two twelve if you are using the app",
        "the coach leaves from gate three sixteen",
        "the reading today is a long one, so settle in",
        "welcome to our nine thirty service",
        "the crèche is in room one one for under fives",
    ];
    let mut hits = vec![];
    for s in sentences {
        let (fired, offered) = wall_and_offer(s);
        println!("R6-12 {s:60} → wall {fired:?} offer {offered:?}");
        if !fired.is_empty() {
            hits.push(format!("{s:?} → {fired:?}"));
        }
    }
    assert!(
        hits.is_empty(),
        "R6-12: these sentences put scripture on the wall by themselves:\n  {}",
        hits.join("\n  ")
    );
}

/// R6-13 · The confidence the repair actually carries, against the auto-fire bar.
///
/// `fuzzy_book`'s doc comment says a repaired reference "is still marked FUZZY, which
/// costs confidence downstream, so a repaired reference needs to be otherwise strong to
/// reach the auto-fire line." This measures that claim.
#[test]
fn r6_13_what_the_fuzzy_penalty_is_actually_worth() {
    use crate::router::Thresholds;
    let bar = Thresholds::default();
    println!(
        "auto_fire bar = {:.2}, suggest bar = {:.2} (sensitivity 50, the shipped default)",
        bar.auto_fire, bar.suggest
    );
    for t in [
        "hymn number three sixteen",
        "room two twelve",
        "row three sixteen",
        "van three sixteen",
        "john three sixteen",
        "romans two twelve",
    ] {
        for m in detection::detect_direct(t) {
            println!(
                "  {t:28} → {} {}:{}  conf {:.3}  method {:?}  {}",
                m.reference.book,
                m.reference.chapter,
                m.reference.verse,
                m.confidence,
                m.method,
                if m.confidence >= bar.auto_fire {
                    "ON THE WALL"
                } else {
                    "suggestion only"
                }
            );
        }
    }
}

/// R6-14 · Verifying R4-B independently, and testing the class R4 did NOT test:
/// ordinary COMMON nouns against the 3-letter TYPING abbreviations.
#[test]
fn r6_14_confirming_r4b_and_extending_it() {
    for t in [
        "Nehemiah, fifty two days",                 // R4-B's case
        "Mary, twenty two years of age",            // R4-B's case
        "please turn to hymn number three sixteen", // R6's case
        "the youth meet in room two twelve",        // R6's case
    ] {
        let (fired, offered) = wall_and_offer(t);
        println!("R6-14 {t:45} → WALL {fired:?}  offer {offered:?}");
    }
}
