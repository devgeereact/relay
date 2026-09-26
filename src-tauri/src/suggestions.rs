//! **HOW MANY SUGGESTIONS A REAL SERVICE PRODUCES, MEASURED BEFORE ANY ROW IS
//! WRITTEN FOR ONE.**
//!
//! Two comments in this repository justify not persisting a suggestion with a
//! number nobody had measured — `dismiss_detection`'s *"hundreds of rows a
//! minute"* and `report.js`'s copy of the same sentence. This module is the
//! measurement those sentences were owed, and it is here rather than in a
//! scratch script because the conclusion decides a schema.
//!
//! ```text
//! RELAY_SERVICE_CORPUS=<file> cargo test --release suggestion_volume -- --ignored --nocapture
//! ```
//!
//! The corpus is the same `<seconds>\t<text>` file `passage_guard_bench` takes,
//! and it is read the same way, from the author's own database:
//!
//! ```text
//! sqlite3 -readonly relay.db -noheader -separator $'\t' \
//!   "select round(timestamp,1), replace(text, char(10),' ')
//!      from transcripts where service_id = 40 and trim(text) <> '' order by timestamp;"
//! ```
//!
//! **What it measures and what it cannot.** It drives the real
//! `candidates_for_window` and the real `Router`, per rule 13, so the question it
//! answers is the product's own: how many gate decisions, of what kind, at what
//! score. It is FINALS ONLY — the live path also detects on every partial — so
//! every count here is a **floor**. The multiplier is not guessed at either: the
//! live run's own `perf_samples` carry `stt_decode` (every decode pass) and
//! `transcript_to_reference_detection` (every pass that produced a candidate),
//! and the row this measurement supports quotes those beside these.
//!
//! There is no audio in it, so nothing here is about accuracy.

#![cfg(test)]

use super::*;
use detection::{DetectionMethod, VerseRef};

/// The KJV as `(ref, text)`, canonical book names — the same corpus the live
/// indexes are built from, built the same way `passage_guard_bench` builds it.
fn kjv_corpus() -> Vec<(VerseRef, String)> {
    let kjv: serde_json::Value =
        serde_json::from_str(include_str!("../data/kjv.json").trim_start_matches('\u{feff}'))
            .expect("kjv");
    let mut corpus: Vec<(VerseRef, String)> = Vec::new();
    for (bi, book) in kjv.as_array().expect("books").iter().enumerate() {
        let name = detection::CANONICAL_BOOKS[bi].to_string();
        for (ci, chapter) in book["chapters"].as_array().expect("ch").iter().enumerate() {
            for (vi, verse) in chapter.as_array().expect("vs").iter().enumerate() {
                corpus.push((
                    VerseRef {
                        book: name.clone(),
                        chapter: ci as i64 + 1,
                        verse: vi as i64 + 1,
                    },
                    verse.as_str().unwrap_or("").to_string(),
                ));
            }
        }
    }
    corpus
}

#[derive(Default)]
struct Tally {
    /// Every `Suggest` the gate produced, one per candidate per window — what a
    /// row-per-decision schema would write.
    decisions: usize,
    /// The same set collapsed the way a PERSON experienced it: one per
    /// (reference, method) that had gone quiet for `SUGGESTION_EPISODE_MS`.
    episodes: usize,
    fired: usize,
    /// Per wire method name: decisions, then episodes.
    by_method: Vec<(&'static str, usize, usize)>,
    /// Paraphrase (`semantic`) episode scores, for the distribution.
    semantic_scores: Vec<f32>,
}

impl Tally {
    fn note(&mut self, wire: &'static str, episode: bool) {
        match self.by_method.iter_mut().find(|(m, _, _)| *m == wire) {
            Some((_, d, e)) => {
                *d += 1;
                if episode {
                    *e += 1;
                }
            }
            None => self.by_method.push((wire, 1, usize::from(episode))),
        }
    }
}

fn replay(lines: &[(f32, String)], episode_ms: u64) -> Tally {
    let corpus = kjv_corpus();
    let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(
        &corpus,
    )));
    let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&corpus)));
    let mut context = ContextMemory::default();
    let mut router = Router::default();
    let mut t = Tally::default();
    // The episode gate, modelled exactly as `Offers` implements it: keyed on the
    // pair, not on the reference, because a verse first paraphrased and later
    // heard is two different claims about the same words and the second is the one
    // that matters.
    let mut seen: Vec<(String, &'static str, u64)> = Vec::new();

    for (at, text) in lines {
        let now_ms = (at * 1000.0) as u64;
        let WindowCandidates { kept, .. } =
            candidates_for_window(text, true, &sem, &phrases, &context, router.wall(), false);
        if kept.is_empty() {
            continue;
        }
        let mut best: Vec<(String, Cand)> = Vec::new();
        for c in kept {
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
            let wire = c.method.wire();
            match router.decide_live(&key, c.conf, c.method, now_ms, true) {
                RouteDecision::AutoFire if rank == 0 => {
                    t.fired += 1;
                    context.note_passage(&c.r, None);
                    router.note_wall(&key);
                }
                RouteDecision::AutoFire | RouteDecision::Suggest => {
                    t.decisions += 1;
                    let fresh = match seen.iter_mut().find(|(k, m, _)| *k == key && *m == wire) {
                        Some((_, _, last)) => {
                            let fresh = now_ms.saturating_sub(*last) >= episode_ms;
                            *last = now_ms;
                            fresh
                        }
                        None => {
                            seen.push((key.clone(), wire, now_ms));
                            true
                        }
                    };
                    if fresh {
                        t.episodes += 1;
                        if c.method == DetectionMethod::Semantic {
                            t.semantic_scores.push(c.conf);
                        }
                    }
                    t.note(wire, fresh);
                }
                RouteDecision::Drop => {}
            }
        }
    }
    t
}

/// The interval after which the same claim, offered again, is a NEW offer.
///
/// Not `DEFAULT_DEBOUNCE_MS` and deliberately longer than it. That constant is a
/// cooldown on the WALL — how long before saying a verse again means the preacher
/// said it again (`router.rs`) — and a suggestion is not a wall. What this has to
/// bound is a record: how long a claim must be absent from the operator's list
/// before its reappearance is a second thing that happened rather than the same
/// thing still happening. Measured at three values below, because the right answer
/// is the one the data supports and not the one that sounds tidy.
const SUGGESTION_EPISODE_MS: u64 = 60_000;

#[test]
#[ignore]
fn suggestion_volume() {
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
    let span = lines.last().map(|(a, _)| *a).unwrap_or(0.0);
    println!(
        "\n{} final transcript lines over {:.0} s ({:.1} h)\n",
        lines.len(),
        span,
        span / 3600.0
    );

    for ms in [10_000_u64, SUGGESTION_EPISODE_MS, 300_000] {
        let t = replay(&lines, ms);
        println!(
            "  episode window {:>6} ms   decisions {:>5}   episodes {:>4}   fires {:>4}",
            ms, t.decisions, t.episodes, t.fired
        );
        if ms == SUGGESTION_EPISODE_MS {
            println!("\n  per method, at {ms} ms:");
            let mut by = t.by_method.clone();
            by.sort_by_key(|(_, d, _)| std::cmp::Reverse(*d));
            for (m, d, e) in by {
                println!("    {m:<18} decisions {d:>5}   episodes {e:>4}");
            }
            let mut s = t.semantic_scores.clone();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if s.is_empty() {
                println!("\n  paraphrase episodes: none");
            } else {
                let at = |q: f64| s[((s.len() - 1) as f64 * q) as usize];
                println!(
                    "\n  paraphrase episode cosines: n {}  min {:.3}  p50 {:.3}  p95 {:.3}  max {:.3}",
                    s.len(),
                    s[0],
                    at(0.5),
                    at(0.95),
                    s[s.len() - 1]
                );
            }
        }
    }
}

/// **THE RECORD OF WHAT RELAY OFFERED — RG-309.**
///
/// These drive the real `emit_detections` against a real in-memory database
/// through `qa::bare_app()`, for the reason `e2e.rs` gives: the question is what a
/// service's own record says afterwards, and no unit test of a pure function can
/// answer it. They live here rather than in `e2e.rs` because `e2e.rs` is about what
/// a congregation sees and nothing here reaches a screen.
#[cfg(test)]
mod record {
    use super::*;

    /// A paraphrase — no reference in the words, a verse found by cosine. Measured
    /// by `what_do_these_windows_produce`: `Hebrews 13:5` at 0.348 and
    /// `1 Kings 8:57` at 0.443, both `Semantic`, both `Suggest` at the shipped
    /// default. Neither may ever auto-fire (rule 10).
    const PARAPHRASE: &str =
        "God has promised that he will never leave you nor forsake you in any circumstance";

    /// A verbatim run, which is a DIFFERENT detector reaching a DIFFERENT verdict
    /// about the same kind of claim: `Romans 8:37` at 0.630, `Quoted`. It persists
    /// as `semantic` today, which is the conflation this module exists to end.
    const QUOTATION: &str =
        "we are more than conquerors through the one who loved us and gave himself";

    fn running_service(h: &tauri::AppHandle<tauri::test::MockRuntime>) -> i64 {
        start_service(
            h.clone(),
            h.state::<Session>(),
            h.state::<Db>(),
            h.state::<channels::Rehearsal>(),
            h.state::<servicelock::ServiceLock>(),
            "Sunday Service".into(),
            "2026-09-25".into(),
        )
        .expect("start")
    }

    fn recorded(
        h: &tauri::AppHandle<tauri::test::MockRuntime>,
        svc: i64,
    ) -> Vec<db::ServiceDetection> {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        db::service_detections(&conn, svc).expect("detections")
    }

    /// **THE LIVE SEQUENCE, NOT A CONVENIENCE.** `stt.rs`'s worker calls
    /// `persist_transcript` and THEN `emit_detections` for a final window
    /// (`main.rs`, one call site each), which is what makes F-2's "reuse the row
    /// when it IS that text" reachable at all. A test that emitted a detection
    /// against a service with no transcript in it would be testing a sequence the
    /// product does not have.
    fn heard(h: &tauri::AppHandle<tauri::test::MockRuntime>, text: &str, at_ms: u64, final_: bool) {
        if final_ {
            persist_transcript(h, text, "en");
        }
        emit_detections(h, text, at_ms, final_, None);
        qa::settle();
    }

    fn transcript_rows(h: &tauri::AppHandle<tauri::test::MockRuntime>, svc: i64) -> usize {
        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        db::service_transcripts(&conn, svc)
            .expect("transcripts")
            .len()
    }

    /// **THE DEFECT.** `persist_fire` is called inside `if fire.may_broadcast()`,
    /// and a paraphrase can never broadcast — so sixteen hours of a preacher
    /// paraphrasing produced no record of the paraphrase detector doing anything,
    /// right or wrong. `detections.status` permits `'suggested'`; nothing had ever
    /// written it, on any machine, in the life of the project.
    #[test]
    fn a_paraphrase_the_ai_offered_reaches_the_record() {
        let app = qa::bare_app();
        let h = app.handle().clone();
        let svc = running_service(&h);

        heard(&h, PARAPHRASE, 1_000, true);

        let rows = recorded(&h, svc);
        let offered: Vec<_> = rows.iter().filter(|d| d.status == "suggested").collect();
        assert!(
            !offered.is_empty(),
            "a paraphrase Relay offered left no record at all: {rows:?}"
        );
        // THE EVIDENCE, not just the count. A row that cannot say what the detector
        // was reading is not diagnosable after the fact, which is the whole reason
        // `heard_text` exists (FIELD F-2).
        assert!(
            offered
                .iter()
                .all(|d| d.heard_text.as_deref() == Some(PARAPHRASE)),
            "a suggestion's row must carry the window it was read from: {offered:?}"
        );
        // And it must not be mistaken for something a human did, or for something
        // that reached a screen. Rule 14.
        assert!(
            rows.iter()
                .all(|d| d.status != "auto" && d.status != "manual"),
            "nothing here reached a screen and nobody pressed anything: {rows:?}"
        );
    }

    /// **THE HALF THAT MAKES THE OTHER HALF WORTH HAVING.** `db_method` collapsed
    /// seven `DetectionMethod` variants into two, so `semantic` in the database
    /// meant *paraphrase OR quotation OR followed reading*. A record of
    /// suggestions written through that column could not answer the question it was
    /// written for — which of these came from the paraphrase detector — and the
    /// conflation is already filed as a known gap in `db_method`'s own comment.
    #[test]
    fn a_paraphrase_and_a_quotation_are_not_the_same_row() {
        let app = qa::bare_app();
        let h = app.handle().clone();
        let svc = running_service(&h);

        heard(&h, PARAPHRASE, 1_000, true);
        heard(&h, QUOTATION, 40_000, true);

        let rows = recorded(&h, svc);
        let methods: Vec<&str> = rows.iter().map(|d| d.method.as_str()).collect();
        assert!(
            methods.contains(&"semantic"),
            "the paraphrase detector must be nameable in the record: {methods:?}"
        );
        assert!(
            methods.contains(&"quoted") || methods.contains(&"reading"),
            "a verbatim run is not a cosine and the record must say which: {methods:?}"
        );
    }

    /// **A PARTIAL WINDOW MAY NOT GROW THE TRANSCRIPT.** Only finals are persisted,
    /// on purpose — a partial is mid-word and is decoded again a moment later. The
    /// live path detects on every partial, so a suggestion that inserted its own
    /// transcript row would add thousands of rows of half-heard text per service
    /// and change what every history surface shows. The window travels in
    /// `heard_text`, which is what that column is for.
    #[test]
    fn a_suggestion_from_a_partial_window_adds_no_transcript_row() {
        let app = qa::bare_app();
        let h = app.handle().clone();
        let svc = running_service(&h);
        // One final, so there is something for an offer to hang off — the ordinary
        // state of a service a second after the microphone opens.
        heard(&h, "good morning church it is good to be here", 500, true);
        let before = transcript_rows(&h, svc);

        // `is_final: false` — and twice, because the same window really is decoded
        // again a step later and that is the case that multiplies.
        heard(&h, PARAPHRASE, 1_000, false);
        heard(&h, PARAPHRASE, 1_600, false);

        let after = transcript_rows(&h, svc);
        assert_eq!(
            before, after,
            "a partial window's suggestion inserted a transcript row"
        );
        // …and the suggestion is still recorded, with its evidence. An absence is
        // not the way to satisfy the assertion above.
        let rows = recorded(&h, svc);
        assert!(
            rows.iter()
                .any(|d| d.status == "suggested" && d.heard_text.as_deref() == Some(PARAPHRASE)),
            "the partial's suggestion was dropped rather than recorded: {rows:?}"
        );
    }

    /// **THE PRIVACY LINE DOES NOT MOVE.** `detections.heard_text` carries what was
    /// said and always has; `service_events.detail` and `cues.payload_json` carry a
    /// phrase Relay composes and must keep carrying nothing else. Suggestions now
    /// flow through the same service, so the guarantee is re-checked with them
    /// flowing rather than assumed to survive them.
    #[test]
    fn suggestions_put_nothing_a_preacher_said_into_the_timeline() {
        let app = qa::bare_app();
        let h = app.handle().clone();
        let svc = running_service(&h);

        heard(&h, PARAPHRASE, 1_000, true);
        heard(&h, QUOTATION, 40_000, true);

        let db = h.state::<Db>();
        let conn = db.0.lock().expect("db");
        let rows = db::service_timeline(&conn, svc).expect("timeline");
        // Every distinctive word of the two windows, checked against every string
        // the timeline hands out. `forsake` and `conquerors` appear in neither a
        // composed phrase nor a reference.
        for row in &rows {
            let hay = format!("{row:?}").to_lowercase();
            for word in ["forsake", "conquerors", "circumstance", "promised"] {
                assert!(
                    !hay.contains(word),
                    "the timeline carried a preacher's word: {word} in {row:?}"
                );
            }
        }
    }
}

/// **THE TWO CONTRACTS RG-309 RESTS ON.**
///
/// These guard `detection.rs` and belong beside it; they are here because
/// `detection.rs` was being edited by another agent on the day this landed, and a
/// test in the wrong file is a smaller problem than a merge that loses one. Move
/// them into `detection.rs`'s own test module at the next opportunity.
#[cfg(test)]
mod contracts {
    use super::*;

    /// A real schema with the ONE row a transcript needs to exist. `verses` and
    /// `templates` come from `init_fresh`; `services` is seeded by nothing, because a
    /// fresh install has run no services.
    pub(super) fn scratch_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        conn.execute_batch("PRAGMA foreign_keys = ON;").expect("fk");
        db::init_fresh(&conn).expect("schema");
        conn.execute(
            "INSERT INTO services (id, date, title) VALUES (1, '2026-09-25', 'Sunday Service')",
            [],
        )
        .expect("service");
        conn
    }

    const EVERY: [DetectionMethod; 7] = [
        DetectionMethod::Direct,
        DetectionMethod::Semantic,
        DetectionMethod::Quoted,
        DetectionMethod::Reading,
        DetectionMethod::Ambiguous,
        DetectionMethod::UncertainBook,
        DetectionMethod::UncertainNumber,
    ];

    /// `wire()` is the one mapping and `from_wire` is its inverse — and serde is the
    /// third party that has to agree, because `#[serde(rename)]` is what actually
    /// puts the name on the bridge. Three places, one answer, asserted rather than
    /// hoped for: a variant added without a `wire()` arm cannot compile, and one
    /// whose rename disagrees fails here.
    #[test]
    fn the_wire_name_is_one_mapping_in_two_directions() {
        for m in EVERY {
            let name = m.wire();
            assert_eq!(
                serde_json::to_string(&m).expect("serialise"),
                format!("\"{name}\""),
                "serde and wire() disagree about {m:?}"
            );
            assert_eq!(
                DetectionMethod::from_wire(name),
                m,
                "from_wire is not wire()'s inverse for {m:?}"
            );
        }
        // Seven distinct names. A copy-paste that gave two variants the same string
        // would pass every assertion above and silently re-create the conflation
        // RG-309 exists to end.
        let mut names: Vec<&str> = EVERY.iter().map(|m| m.wire()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(
            before,
            names.len(),
            "two methods share a wire name: {names:?}"
        );
    }

    /// `db_method` is `wire()`. Kept as its own test rather than deleted with the
    /// old mapping, because the two questions are allowed to diverge again one day
    /// and the day they do, this is the test that has to be changed on purpose.
    #[test]
    fn the_database_name_is_the_wire_name() {
        for m in EVERY {
            assert_eq!(m.db_method(), m.wire(), "{m:?}");
        }
        // And every one of them is a value the CHECK constraint will take. This is
        // the assertion that would have failed before the v6 rung, and it is the
        // one that fails if a variant is added without following the migration.
        let conn = scratch_db();
        let tid = db::insert_transcript(&conn, 1, 0.0, "x", "en", None).expect("transcript");
        for m in EVERY {
            db::insert_detection(
                &conn,
                tid,
                None,
                m.db_method(),
                0.5,
                "suggested",
                Some(1.0),
                Some("x"),
            )
            .unwrap_or_else(|e| {
                panic!("detections.method rejects {:?} ({}): {e}", m, m.db_method())
            });
        }
        // NOT an early return on a missing row. An earlier draft of this test did
        // `if tid.is_none() { return; }` because a bare in-memory DB has no service
        // row to hang a transcript off — and a test that returns Ok on the setup
        // failing is a test that cannot fail, which is the thing this repository
        // records as the way a theory goes untested. `scratch_db` inserts the
        // service, and the `expect` above is the tripwire if that stops working.
        let kinds: i64 = conn
            .query_row("SELECT COUNT(DISTINCT method) FROM detections", [], |r| {
                r.get(0)
            })
            .expect("count");
        assert_eq!(
            kinds,
            EVERY.len() as i64,
            "seven methods, seven stored words"
        );
    }
}

/// **WHAT THE WRITE COSTS, ON THE THREAD THAT MUST NOT STALL.**
///
/// `cargo test --release the_write_a_suggestion_costs -- --ignored --nocapture`
///
/// Rule 33's question, asked with a number rather than reasoned about: a suggestion
/// row is one `INSERT` into `detections`, inside the `Db` lock `emit_detections`
/// already holds, on `relay-detect` behind the bounded queue. The thing it must not
/// do is approach a cadence step — `139 ms` for `ggml-base` and `~600 ms` for
/// `turbo` (rule 32) — because a detect thread that falls behind sheds partials.
#[cfg(test)]
mod cost {
    use super::*;

    #[test]
    #[ignore]
    fn the_write_a_suggestion_costs() {
        let conn = super::contracts::scratch_db();
        let tid = db::insert_transcript(&conn, 1, 0.0, "x", "en", None).expect("transcript");
        // A real window, because `heard_text` is most of the row's bytes.
        let window = "God has promised that he will never leave you nor forsake you in any \
                      circumstance and that is the word we are standing on this morning";
        let n = 20_000;
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            let _ = db::insert_detection(
                &conn,
                tid,
                None,
                "semantic",
                0.348,
                "suggested",
                Some(1.0),
                Some(window),
            );
        }
        let each = t0.elapsed().as_secs_f64() / n as f64;
        println!(
            "\n  {n} suggestion rows in {:.3} s → {:.1} µs each",
            t0.elapsed().as_secs_f64(),
            each * 1e6
        );
        let bytes: i64 = conn
            .query_row(
                "SELECT SUM(pgsize) FROM dbstat WHERE name = 'detections'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if bytes > 0 {
            println!(
                "  {:.0} bytes per row on disk → {:.1} MB for a 16-hour service's ~8000 offers",
                bytes as f64 / n as f64,
                bytes as f64 / n as f64 * 8000.0 / 1.048_576e6
            );
        }
        println!("  one cadence step is 139 ms (ggml-base) / ~600 ms (turbo) — rule 32\n");
    }
}

/// **THE THREE READERS THAT MEANT SOMETHING ELSE UNTIL SUGGESTIONS EXISTED.**
///
/// Every row in `detections` had reached a screen, so three queries could ask a
/// cheap proxy question and get the right answer. Recording offers made all three
/// wrong at once, and none of them is in the file this change is about — which is
/// the *"enumerate every caller of the thing you fixed"* rule, arriving as the cost
/// of ignoring it.
#[cfg(test)]
mod readers {
    use super::*;

    /// **THE REPEAT TRACKER.** `count_verse_in_service` asks `fired_at IS NOT NULL`,
    /// and `persist_fire` stamps a time on every row it writes, offers included. So
    /// the series/repeat tracker would tell an operator a verse had already been up
    /// four times when the AI had merely guessed at it four times.
    #[test]
    fn the_repeat_tracker_counts_screens_and_not_guesses() {
        let conn = super::contracts::scratch_db();
        let tid = db::insert_transcript(&conn, 1, 0.0, "x", "en", None).expect("transcript");
        let vid: i64 = conn
            .query_row("SELECT id FROM verses LIMIT 1", [], |r| r.get(0))
            .expect("a verse");
        // One that reached a screen…
        db::insert_detection(
            &conn,
            tid,
            Some(vid),
            "direct",
            0.9,
            "auto",
            Some(1.0),
            None,
        )
        .expect("auto");
        // …and three the AI only ever offered, each with a real timestamp, because
        // WHEN it was offered is part of the record.
        for at in [2.0, 3.0, 4.0] {
            db::insert_detection(
                &conn,
                tid,
                Some(vid),
                "semantic",
                0.35,
                "suggested",
                Some(at),
                None,
            )
            .expect("offer");
        }
        assert_eq!(
            db::count_verse_in_service(&conn, 1, vid).expect("count"),
            1,
            "the repeat tracker counted suggestions as times the verse was shown"
        );
    }

    /// **THE TIMELINE.** `service_timeline` merged every detection row in as an
    /// entry, and `History.svelte` renders it as a list and indexes into it for the
    /// replay. A 16-hour service offers in the region of 8,000 suggestions against
    /// 365 fires, so the one ordered record of what happened would have been 95%
    /// things that did not happen — and the report's own counts are derived from
    /// `detail.detections`, not from here, so nothing needed them in it.
    #[test]
    fn the_timeline_holds_what_happened_and_not_what_was_guessed() {
        let conn = super::contracts::scratch_db();
        let tid = db::insert_transcript(&conn, 1, 0.0, "x", "en", None).expect("transcript");
        let vid: i64 = conn
            .query_row("SELECT id FROM verses LIMIT 1", [], |r| r.get(0))
            .expect("a verse");
        db::insert_detection(
            &conn,
            tid,
            Some(vid),
            "direct",
            0.9,
            "auto",
            Some(1.0),
            None,
        )
        .expect("auto");
        db::insert_detection(
            &conn,
            tid,
            Some(vid),
            "direct",
            0.9,
            "manual",
            Some(2.0),
            None,
        )
        .expect("manual");
        for at in [3.0, 4.0, 5.0] {
            db::insert_detection(
                &conn,
                tid,
                Some(vid),
                "semantic",
                0.35,
                "suggested",
                Some(at),
                None,
            )
            .expect("offer");
        }
        let rows = db::service_timeline(&conn, 1).expect("timeline");
        let dets: Vec<&str> = rows
            .iter()
            .filter(|r| r.source == "detection")
            .map(|r| r.kind.as_str())
            .collect();
        assert_eq!(
            dets,
            vec!["auto", "manual"],
            "the timeline carried suggestions as things that happened"
        );
    }

    /// **AND THE ONE THAT MUST KEEP THEM.** `service_detections` is the forensic
    /// list — the whole point of RG-309 — so the fix above must not be applied here
    /// by reflex. If both were filtered, the offers would be written and unreadable,
    /// which is a worse state than not writing them.
    #[test]
    fn the_detections_list_keeps_every_offer() {
        let conn = super::contracts::scratch_db();
        let tid = db::insert_transcript(&conn, 1, 0.0, "x", "en", None).expect("transcript");
        let vid: i64 = conn
            .query_row("SELECT id FROM verses LIMIT 1", [], |r| r.get(0))
            .expect("a verse");
        db::insert_detection(
            &conn,
            tid,
            Some(vid),
            "direct",
            0.9,
            "auto",
            Some(1.0),
            None,
        )
        .expect("auto");
        db::insert_detection(
            &conn,
            tid,
            Some(vid),
            "semantic",
            0.35,
            "suggested",
            Some(2.0),
            Some("the words that caused it"),
        )
        .expect("offer");
        let rows = db::service_detections(&conn, 1).expect("detections");
        assert_eq!(rows.len(), 2);
        let offer = rows
            .iter()
            .find(|d| d.status == "suggested")
            .expect("the offer must still be readable");
        assert_eq!(offer.method, "semantic");
        assert_eq!(
            offer.heard_text.as_deref(),
            Some("the words that caused it")
        );
    }
}

// ── WHAT THE PARAPHRASE BAR COSTS, AND WHAT IT BUYS ─────────────────────────
//
// `SEMANTIC_FLOOR`'s own doc comment names the reason it has never been moved:
// *"the corpus has no negative cases yet (transcript that mentions no scripture
// at all), so the noise it would cost is currently UNMEASURED."* Three services
// on 2026-09-25 left 13,393 final transcript lines of real church speech in
// `transcripts` — prayer, worship, announcements, testimony and sermon — so the
// negative cases exist now and this is the measurement that constant was set
// without.
//
// ```text
// RELAY_SERVICE_CORPUS=40=svc40.tsv,37=svc37.tsv \
// RELAY_ACCEPTED=accepted.tsv \
//   cargo test --release paraphrase_bar -- --ignored --nocapture
// ```
//
// `RELAY_ACCEPTED` is `<service>\t<seconds>\t<reference>` — the `cues` rows of
// type `suggestion_accepted`. **It is the only ground truth this project has
// about what the operator wanted**, so every policy is priced in it: a bar that
// removes offers is worth nothing if it removes the ones somebody pressed.
//
// THREE THINGS THIS CANNOT DO, stated here because each of them bounds a number
// printed below.
//   * **Finals only.** The live path detects on every partial, so a claim the
//     operator accepted may have been offered by a partial this replay never
//     sees. Every count is a floor and a reproduced acceptance is a subset.
//   * **It cannot say an offer was WRONG.** A window that names no reference and
//     quotes nothing is exactly where a paraphrase is supposed to work, so the
//     partition below is a population, not a verdict. Judging it needs a person
//     reading the words, and where that was done the report says so.
//   * **The cap sweeps DOWN only.** `top_k_explained` applies the evidence
//     filter before it takes k, so a list longer than
//     `SEMANTIC_SUGGESTIONS_MAX` cannot be reconstructed from this pass.
#[cfg(test)]
mod bar {
    use super::*;
    use crate::eval::para_cases;
    use detection::SemanticIndex;

    /// One paraphrase row the shipped gate put in front of the operator, with
    /// every fact a tighter policy would ask about it.
    #[derive(Clone)]
    struct Offer {
        svc: i64,
        at_ms: u64,
        key: String,
        conf: f32,
        /// Independent shared words behind the cosine, as the console renders
        /// them. Never fewer than `MIN_EVIDENCE_TERMS`, never more than
        /// `EXPLAIN_TERMS`.
        terms: usize,
        /// The best semantic cosine the same window produced — the denominator
        /// `SEMANTIC_RELATIVE_FLOOR` is applied against.
        best: f32,
        /// Position among that window's semantic hits, best first.
        rank: usize,
        /// The window named nothing reference-shaped and held no verbatim run:
        /// the paraphrase detector was the only thing that spoke.
        bare: bool,
        /// THE LONGEST CONTIGUOUS RUN OF WORDS this window shares with the verse
        /// it named — the one fact `top_k_explained` computes nothing about and
        /// the console therefore cannot show. A cosine is a bag of words in no
        /// order (rule 18); this asks whether any of them were said in the verse's
        /// order, which is a different question about the same evidence.
        run: usize,
    }

    #[derive(Clone, Copy)]
    struct Policy {
        floor: f32,
        ratio: f32,
        cap: usize,
        /// Least evidence terms. The shipped bar is `MIN_EVIDENCE_TERMS` = 3 and
        /// is applied inside the index, so 3 here is "no extra requirement".
        terms: usize,
        /// Least contiguous shared run. 0 is the shipped behaviour: the
        /// paraphrase path asks nothing about word order at all.
        run: usize,
    }

    /// Longest run of tokens `a` and `b` share, in order. Quadratic and used only
    /// on one window against one verse, which is tens by tens.
    fn longest_run(a: &[&str], b: &[&str]) -> usize {
        let mut best = 0;
        for i in 0..a.len() {
            for j in 0..b.len() {
                let mut k = 0;
                while i + k < a.len() && j + k < b.len() && a[i + k] == b[j + k] {
                    k += 1;
                }
                best = best.max(k);
            }
        }
        best
    }

    fn toks(s: &str) -> Vec<&str> {
        s.split(|c: char| !c.is_ascii_alphabetic())
            .filter(|w| !w.is_empty())
            .collect()
    }

    /// Lowercased once per call site, because `longest_run` compares by equality.
    fn lower(s: &str) -> String {
        s.to_ascii_lowercase()
    }

    impl Policy {
        const SHIPPED: Policy = Policy {
            floor: SEMANTIC_FLOOR,
            ratio: SEMANTIC_RELATIVE_FLOOR,
            cap: SEMANTIC_SUGGESTIONS_MAX,
            terms: 3,
            run: 0,
        };
        fn keeps(&self, o: &Offer) -> bool {
            o.conf >= self.floor
                && o.conf >= o.best * self.ratio
                && o.rank < self.cap
                && o.terms >= self.terms
                && o.run >= self.run
        }
        fn label(&self) -> String {
            format!(
                "{:.2} / {:.2} / {} / {} / {}",
                self.floor, self.ratio, self.cap, self.terms, self.run
            )
        }
    }

    /// `<seconds>\t<text>` lines, as `sqlite3 -separator $'\t'` writes them.
    fn corpus(path: &str) -> Vec<(f32, String)> {
        let body = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        body.lines()
            .filter_map(|l| l.split_once('\t'))
            .filter(|(_, t)| !t.trim().is_empty())
            .map(|(a, t)| (a.trim().parse().unwrap_or(0.0), t.to_string()))
            .collect()
    }

    /// What a service's finals produce, in the product's own order, through the
    /// real `candidates_for_window` and the real `Router`.
    struct Census {
        windows: usize,
        named: usize,
        run: usize,
        bare: usize,
        /// Windows in the bare partition that produced a paraphrase offer.
        bare_offering: usize,
        span_s: f32,
        offers: Vec<Offer>,
        /// EVERY offer, whatever found it. Without this, "the policy did not keep
        /// this acceptance" cannot be told from "the paraphrase path never found
        /// it and something better did" — and 27 of the 30 turn out to be the
        /// second, which is the finding rather than a footnote.
        every: Vec<(u64, String, &'static str)>,
        /// One line per window, for a person to read: the words, and what the
        /// paraphrase path offered for them. Written only when asked.
        dump: Vec<String>,
        /// How many paraphrase candidates `candidates_for_window` held back under
        /// `HeldReason::NoSharedRun` — the church's bar, when it is on.
        ///
        /// COUNTED rather than inferred from the difference between two runs: a mask
        /// that never fired and a bar that happened to keep everything are the same
        /// arithmetic and very different facts, and "OFF is a no-op" is the claim
        /// this whole bench exists to check.
        held_by_the_bar: usize,
    }

    /// `needs_a_run` is the church's paraphrase bar, passed straight through to the
    /// shipped `candidates_for_window`. FALSE is what every install runs today.
    fn replay_service(
        svc: i64,
        lines: &[(f32, String)],
        dumping: bool,
        needs_a_run: bool,
    ) -> Census {
        let kjv = kjv_corpus();
        let text_of: std::collections::HashMap<String, String> = kjv
            .iter()
            .map(|(r, t)| (Fire::key_for(r), lower(t)))
            .collect();
        let phrases = Phrases(std::sync::RwLock::new(detection::PhraseIndex::build(&kjv)));
        let sem = Semantic(std::sync::RwLock::new(SemanticIndex::build(&kjv)));
        let mut context = ContextMemory::default();
        let mut router = Router::default();
        let mut c = Census {
            windows: lines.len(),
            named: 0,
            run: 0,
            bare: 0,
            bare_offering: 0,
            span_s: lines.last().map(|(a, _)| *a).unwrap_or(0.0),
            offers: Vec::new(),
            every: Vec::new(),
            dump: Vec::new(),
            held_by_the_bar: 0,
        };

        for (at, text) in lines {
            let now_ms = (at * 1000.0) as u64;
            let WindowCandidates { kept, held, .. } = candidates_for_window(
                text,
                true,
                &sem,
                &phrases,
                &context,
                router.wall(),
                needs_a_run,
            );
            c.held_by_the_bar += held
                .iter()
                .filter(|(_, why)| *why == detection::HeldReason::NoSharedRun)
                .count();
            let named = kept.iter().any(|k| !k.method.came_from_the_verse_text());
            let run = kept.iter().any(|k| k.method.is_a_verbatim_run());
            if named {
                c.named += 1;
            }
            if run {
                c.run += 1;
            }
            let bare = !named && !run;
            if bare {
                c.bare += 1;
            }
            // `best` and `rank` over the WHOLE semantic set this window produced,
            // held rows included — `worth_suggesting` ran before the passage
            // guard, so a guard that held the top hit must not move the
            // denominator a relative floor is applied against.
            let mut sem_all: Vec<f32> = kept
                .iter()
                .chain(held.iter().map(|(k, _)| k))
                .filter(|k| k.method == DetectionMethod::Semantic)
                .map(|k| k.conf)
                .collect();
            sem_all.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            let best = sem_all.first().copied().unwrap_or(0.0);

            // The product's own dedup and rank, so a paraphrase this window
            // found a second way is counted the once.
            let mut dedup: Vec<(String, Cand)> = Vec::new();
            for k in kept {
                let key = Fire::key_for(&k.r);
                match dedup.iter_mut().find(|(j, _)| *j == key) {
                    Some((_, e)) => {
                        if !pipeline::better(e, &k) {
                            *e = k;
                        }
                    }
                    None => dedup.push((key, k)),
                }
            }
            let mut offered_here = false;
            let mut here: Vec<String> = Vec::new();
            for (rank, (key, k)) in rank_for_wall(dedup).into_iter().enumerate() {
                match router.decide_live(&key, k.conf, k.method, now_ms, true) {
                    RouteDecision::AutoFire if rank == 0 => {
                        context.note_passage(&k.r, None);
                        router.note_wall(&key);
                    }
                    RouteDecision::AutoFire | RouteDecision::Suggest => {
                        c.every.push((now_ms, key.clone(), k.method.wire()));
                        if k.method != DetectionMethod::Semantic {
                            continue;
                        }
                        offered_here = true;
                        here.push(format!("{} {:.3}", key, k.conf));
                        let said = lower(text);
                        let run = text_of
                            .get(&key)
                            .map(|v| longest_run(&toks(&said), &toks(v)))
                            .unwrap_or(0);
                        c.offers.push(Offer {
                            svc,
                            at_ms: now_ms,
                            key,
                            conf: k.conf,
                            terms: k
                                .matched
                                .as_deref()
                                .map(|m| m.split(" · ").filter(|s| !s.is_empty()).count())
                                .unwrap_or(0),
                            best,
                            rank: sem_all
                                .iter()
                                .position(|s| (*s - k.conf).abs() < f32::EPSILON)
                                .unwrap_or(0),
                            bare,
                            run,
                        });
                    }
                    RouteDecision::Drop => {}
                }
            }
            if bare && offered_here {
                c.bare_offering += 1;
            }
            if dumping {
                c.dump.push(format!(
                    "{svc}\t{at:.1}\t{}\t{}\t{}",
                    if bare {
                        "bare"
                    } else if named {
                        "named"
                    } else {
                        "run"
                    },
                    here.join(" | "),
                    text
                ));
            }
        }
        c
    }

    fn quantiles(mut v: Vec<f32>) -> String {
        if v.is_empty() {
            return "none".into();
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let at = |q: f64| v[(((v.len() - 1) as f64) * q) as usize];
        format!(
            "n {:<5} min {:.3}  p25 {:.3}  p50 {:.3}  p75 {:.3}  p95 {:.3}  max {:.3}",
            v.len(),
            v[0],
            at(0.25),
            at(0.5),
            at(0.75),
            at(0.95),
            v[v.len() - 1]
        )
    }

    /// Distinct claims, the way a person experienced them: the same reference
    /// re-offered after a quiet minute is a second offer, re-offered a step later
    /// is the same one still on the list.
    fn episodes(offers: &[Offer]) -> usize {
        let mut seen: Vec<(i64, String, u64)> = Vec::new();
        let mut n = 0;
        for o in offers {
            match seen.iter_mut().find(|(s, k, _)| *s == o.svc && *k == o.key) {
                Some((_, _, last)) => {
                    if o.at_ms.saturating_sub(*last) >= SUGGESTION_EPISODE_MS {
                        n += 1;
                    }
                    *last = o.at_ms;
                }
                None => {
                    seen.push((o.svc, o.key.clone(), o.at_ms));
                    n += 1;
                }
            }
        }
        n
    }

    /// The 45 seconds a suggestion stays on the operator's list
    /// (`SUGGESTION_TTL_MS`, `src/lib/stores/capture.js`). An acceptance at T
    /// consumed an offer made inside it, so that is the window a policy has to
    /// keep an accepted claim alive in.
    const TTL_MS: u64 = 45_000;

    fn accepted() -> Vec<(i64, u64, String)> {
        let Ok(path) = std::env::var("RELAY_ACCEPTED") else {
            return Vec::new();
        };
        std::fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let mut f = l.split('\t');
                Some((
                    f.next()?.trim().parse().ok()?,
                    (f.next()?.trim().parse::<f32>().ok()? * 1000.0) as u64,
                    f.next()?.trim().to_string(),
                ))
            })
            .collect()
    }

    /// Of the acceptances this replay can see at all, how many survive `p`.
    fn survivors(offers: &[Offer], acc: &[(i64, u64, String)], p: Policy, ttl: u64) -> usize {
        acc.iter()
            .filter(|(svc, at, key)| {
                offers.iter().any(|o| {
                    o.svc == *svc
                        && o.key == *key
                        && o.at_ms <= *at
                        && at.saturating_sub(o.at_ms) <= ttl
                        && p.keeps(o)
                })
            })
            .count()
    }

    /// What a policy costs the thing the paraphrase path exists for: the right
    /// passage reachable on the labelled retellings. Returns (all %, modern %,
    /// rows shown per case).
    fn recall(idx: &SemanticIndex, phrases: &detection::PhraseIndex, p: Policy) -> (f32, f32, f32) {
        let cases = para_cases();
        let (mut hit, mut shown, mut m_hit, mut m_n) = (0usize, 0usize, 0usize, 0usize);
        for case in &cases {
            let hits = idx.top_k_explained(&case.text, p.cap);
            let best = hits.first().map(|(_, s, _)| *s).unwrap_or(0.0);
            let kept: Vec<_> = hits
                .iter()
                .filter(|(r, s, w)| {
                    *s >= p.floor
                        && *s >= best * p.ratio
                        && w.len() >= p.terms
                        // THE PRODUCT'S OWN RUN TEST, not this file's `longest_run`.
                        // The two tokenize differently (`phrase_words` keeps digits;
                        // `toks` does not), and a bench that priced the bar with its
                        // own tokenizer would be reporting the cost of a rule nobody
                        // ships. `longest_run` stays, because it is what produces the
                        // `Offer::run` distribution above, which is a fact about the
                        // OFFERS rather than about the shipped rule.
                        && (p.run == 0 || phrases.shared_run_with(&case.text, r) >= p.run)
                })
                .collect();
            let got = kept.iter().any(|(r, _, _)| case.contains(r));
            shown += kept.len();
            hit += got as usize;
            if case.vocab == "modern" {
                m_n += 1;
                m_hit += got as usize;
            }
        }
        let n = cases.len().max(1);
        (
            hit as f32 * 100.0 / n as f32,
            m_hit as f32 * 100.0 / m_n.max(1) as f32,
            shown as f32 / n as f32,
        )
    }

    #[test]
    #[ignore]
    fn paraphrase_bar() {
        let Ok(spec) = std::env::var("RELAY_SERVICE_CORPUS") else {
            println!("set RELAY_SERVICE_CORPUS=<service>=<path>[,<service>=<path>…]");
            return;
        };
        let acc = accepted();
        let dump_to = std::env::var("RELAY_DUMP").ok();
        let mut dump = String::new();
        let mut all: Vec<Offer> = Vec::new();
        let mut every: Vec<(i64, u64, String, &'static str)> = Vec::new();
        let (mut w, mut named, mut run, mut bare, mut bare_off, mut span) = (0, 0, 0, 0, 0, 0.0);
        // **WHAT THE BAR HELD WITH THE SWITCH OFF.** Must be zero. Counted in the
        // same replay that produces every other figure below, so it cannot be a
        // different run of a different corpus.
        let mut held_off = 0usize;

        println!("\n  per service — windows, and what each one produced");
        println!("  ──────────────────────────────────────────────────────────────────");
        for part in spec.split(',') {
            let (svc, path) = part.split_once('=').expect("<service>=<path>");
            let svc: i64 = svc.trim().parse().expect("service id");
            let lines = corpus(path.trim());
            let c = replay_service(svc, &lines, dump_to.is_some(), false);
            println!(
                "  svc {:<3} {:>6} windows over {:>6.0} s   named {:>5}  run {:>5}  \
                 bare {:>5}   paraphrase offers {:>5}",
                svc,
                c.windows,
                c.span_s,
                c.named,
                c.run,
                c.bare,
                c.offers.len()
            );
            w += c.windows;
            named += c.named;
            run += c.run;
            bare += c.bare;
            bare_off += c.bare_offering;
            span += c.span_s;
            held_off += c.held_by_the_bar;
            all.extend(c.offers);
            every.extend(c.every.into_iter().map(|(a, k, m)| (svc, a, k, m)));
            for line in c.dump {
                dump.push_str(&line);
                dump.push('\n');
            }
        }
        if let Some(path) = &dump_to {
            std::fs::write(path, &dump).expect("dump");
            println!("\n  wrote {} window lines to {path}", dump.lines().count());
        }

        let pc = |a: usize, b: usize| a as f32 * 100.0 / b.max(1) as f32;
        println!("\n  THE NEGATIVE PARTITION — no reference named, no verbatim run");
        println!("  ──────────────────────────────────────────────────────────────────");
        println!(
            "  {w} windows over {:.1} h · named a reference {named} ({:.1}%) · \
             held a verbatim run {run} ({:.1}%)",
            span / 3600.0,
            pc(named, w),
            pc(run, w)
        );
        println!(
            "  bare windows {bare} ({:.1}% of all) · {bare_off} of them produced a \
             paraphrase offer = {:.1}%",
            pc(bare, w),
            pc(bare_off, bare)
        );
        let bare_offers: Vec<&Offer> = all.iter().filter(|o| o.bare).collect();
        println!(
            "  offers from bare windows {} of {} ({:.1}%)",
            bare_offers.len(),
            all.len(),
            pc(bare_offers.len(), all.len())
        );
        println!(
            "\n  cosines, bare windows   {}",
            quantiles(bare_offers.iter().map(|o| o.conf).collect())
        );
        println!(
            "  cosines, other windows  {}",
            quantiles(
                all.iter()
                    .filter(|o| !o.bare)
                    .map(|o| o.conf)
                    .collect::<Vec<_>>()
            )
        );
        println!("\n  shared-run distribution, bare windows vs the rest");
        for r in 0..=5 {
            let b = bare_offers.iter().filter(|o| o.run == r).count();
            let o = all.iter().filter(|o| !o.bare && o.run == r).count();
            println!(
                "  run = {r}: bare {b:>5} ({:>5.1}%)   other {o:>5} ({:>5.1}%)",
                pc(b, bare_offers.len()),
                pc(o, all.iter().filter(|o| !o.bare).count())
            );
        }
        let b6 = bare_offers.iter().filter(|o| o.run >= 6).count();
        println!(
            "  run >= 6: bare {b6:>4} ({:>5.1}%)   other {:>5} ({:>5.1}%)",
            pc(b6, bare_offers.len()),
            all.iter().filter(|o| !o.bare && o.run >= 6).count(),
            pc(
                all.iter().filter(|o| !o.bare && o.run >= 6).count(),
                all.iter().filter(|o| !o.bare).count()
            )
        );
        for t in 3..=6 {
            println!(
                "  evidence terms = {t}: bare {:>5} ({:>5.1}%)   other {:>5} ({:>5.1}%)",
                bare_offers.iter().filter(|o| o.terms == t).count(),
                pc(
                    bare_offers.iter().filter(|o| o.terms == t).count(),
                    bare_offers.len()
                ),
                all.iter().filter(|o| !o.bare && o.terms == t).count(),
                pc(
                    all.iter().filter(|o| !o.bare && o.terms == t).count(),
                    all.iter().filter(|o| !o.bare).count()
                ),
            );
        }

        println!("\n  ACCEPTANCES THIS REPLAY CAN SEE — the only ground truth there is");
        println!("  ──────────────────────────────────────────────────────────────────");
        let base = survivors(&all, &acc, Policy::SHIPPED, TTL_MS);
        println!(
            "  {} acceptances on record · {base} reproduced inside the {} s list TTL \
             ({} at 120 s)",
            acc.len(),
            TTL_MS / 1000,
            survivors(&all, &acc, Policy::SHIPPED, 120_000)
        );
        for (svc, at, key) in &acc {
            let hit = all.iter().any(|o| {
                o.svc == *svc
                    && o.key == *key
                    && o.at_ms <= *at
                    && at.saturating_sub(o.at_ms) <= TTL_MS
            });
            let cos = all
                .iter()
                .filter(|o| o.svc == *svc && o.key == *key && at.saturating_sub(o.at_ms) <= TTL_MS)
                .map(|o| o.conf)
                .fold(f32::NAN, f32::max);
            // WHAT THE OPERATOR ACCEPTED, not merely whether a paraphrase
            // policy would have kept it. An acceptance the paraphrase path never
            // produced cannot be a casualty of a paraphrase bar, and reporting a
            // bare "lost" over it would price a change in a currency it does not
            // spend.
            let mut ways: Vec<&str> = every
                .iter()
                .filter(|(s, a, k, _)| {
                    s == svc && k == key && a <= at && at.saturating_sub(*a) <= TTL_MS
                })
                .map(|(_, _, _, m)| *m)
                .collect();
            ways.sort_unstable();
            ways.dedup();
            println!(
                "    svc {:<3} {:>8.1} s  {:<22} {}",
                svc,
                *at as f32 / 1000.0,
                key,
                if hit {
                    format!("paraphrase, cosine {cos:.3}   [{}]", ways.join(" "))
                } else if ways.is_empty() {
                    // AND WHETHER THE PARAPHRASE PATH EVER FOUND IT AT ALL, at
                    // any point in the same service. Twenty of the thirty are
                    // absent from a finals-only replay and the question that
                    // decides how much of the ground truth is really missing is
                    // whether a paraphrase found them a minute earlier or never.
                    let ever: Vec<String> = every
                        .iter()
                        .filter(|(s, _, k, _)| s == svc && k == key)
                        .map(|(_, a, _, m)| format!("{m}@{:.0}s", *a as f32 / 1000.0))
                        .collect();
                    if ever.is_empty() {
                        "NOT OFFERED AT ALL, anywhere in the service".into()
                    } else {
                        format!("not on the list at T; elsewhere: {}", ever.join(" "))
                    }
                } else {
                    format!(
                        "offered, but not by the paraphrase path [{}]",
                        ways.join(" ")
                    )
                }
            );
        }

        println!("\n  POLICY SWEEP");
        println!("  ──────────────────────────────────────────────────────────────────");
        let idx = SemanticIndex::build(&kjv_corpus());
        let phrases_idx = detection::PhraseIndex::build(&kjv_corpus());
        let (r0, m0, s0) = recall(&idx, &phrases_idx, Policy::SHIPPED);
        println!(
            "  floor/ratio/cap/terms/run   offers  removed  episodes | accepted | \
             recall ALL  MODERN  rows"
        );
        let mut policies: Vec<Policy> = vec![Policy::SHIPPED];
        for floor in [0.35_f32, 0.40, 0.45, 0.50] {
            policies.push(Policy {
                floor,
                ..Policy::SHIPPED
            });
        }
        for cap in [1_usize, 2] {
            policies.push(Policy {
                cap,
                ..Policy::SHIPPED
            });
        }
        for ratio in [0.75_f32, 0.85, 1.00] {
            policies.push(Policy {
                ratio,
                ..Policy::SHIPPED
            });
        }
        for terms in [4_usize, 5] {
            policies.push(Policy {
                terms,
                ..Policy::SHIPPED
            });
        }
        policies.push(Policy {
            floor: 0.40,
            cap: 1,
            ..Policy::SHIPPED
        });
        policies.push(Policy {
            floor: 0.40,
            terms: 4,
            ..Policy::SHIPPED
        });
        policies.push(Policy {
            floor: 0.45,
            cap: 2,
            terms: 4,
            ..Policy::SHIPPED
        });
        // ── THE ONE DIMENSION THAT IS NOT A BAR ON THE SCORE ──────────────
        //
        // A cosine is a bag of words in no order (rule 18). This asks the other
        // question about the same evidence — were any of those words said in the
        // verse's own order — and it is here because on 150 hand-read windows of
        // real church speech it separates a defensible offer from noise better
        // than any value of the three constants above.
        for run in [3_usize, 4, 5] {
            policies.push(Policy {
                run,
                ..Policy::SHIPPED
            });
        }
        policies.push(Policy {
            run: 3,
            cap: 1,
            ..Policy::SHIPPED
        });
        for p in policies {
            let kept: Vec<Offer> = all.iter().filter(|o| p.keeps(o)).cloned().collect();
            let (r, m, s) = recall(&idx, &phrases_idx, p);
            println!(
                "  {:<26} {:>6}  {:>6.1}%  {:>7} | {:>3}/{:<3} | {:>8.0}% {:>6.0}% {:>6.2}",
                p.label(),
                kept.len(),
                100.0 - pc(kept.len(), all.len()),
                episodes(&kept),
                survivors(&kept, &acc, p, TTL_MS),
                base,
                r,
                m,
                s
            );
        }
        println!(
            "\n  shipped recall {r0:.0}% ALL / {m0:.0}% MODERN at {s0:.2} rows per case — \
             the number a tighter bar spends"
        );

        // ── THE SWITCH ITSELF, THROUGH THE SHIPPED PATH, BOTH WAYS ────────────
        //
        // Every row in the sweep above is a POST-FILTER over one replay: it asks
        // what a policy would have kept. That is not the same as running the
        // product with the rule in it, because removing a `Semantic` candidate
        // before the router changes the dedup and the `rank_for_wall` order the
        // remaining candidates arrive in. So the corpus is replayed a second time
        // with `detection.paraphrase_needs_a_run` genuinely ON, and both columns are
        // printed side by side.
        //
        // **THE FIRST COLUMN IS THE PROOF THAT OFF IS A NO-OP** and it is asserted
        // rather than eyeballed: `held by the bar` must be 0 with the switch off, in
        // every service, or the mask is firing where nothing asked it to.
        println!("\n  THE SWITCH, THROUGH THE SHIPPED PATH");
        println!("  ──────────────────────────────────────────────────────────────────");
        println!(
            "  state                     offers  removed  episodes | accepted | held by the bar"
        );
        let mut on: Vec<Offer> = Vec::new();
        let mut held_on = 0usize;
        for part in spec.split(',') {
            let (svc, path) = part.split_once('=').expect("<service>=<path>");
            let svc: i64 = svc.trim().parse().expect("service id");
            let c = replay_service(svc, &corpus(path.trim()), false, true);
            held_on += c.held_by_the_bar;
            on.extend(c.offers);
        }
        let bar = Policy {
            run: detection::PARAPHRASE_RUN_WORDS,
            ..Policy::SHIPPED
        };
        let (r_on, m_on, s_on) = recall(&idx, &phrases_idx, bar);
        println!(
            "  OFF (every install today)  {:>6}  {:>6.1}%  {:>7} | {:>3}/{:<3} | {:>6}",
            all.len(),
            0.0,
            episodes(&all),
            base,
            base,
            held_off
        );
        println!(
            "  ON  (run >= {})            {:>6}  {:>6.1}%  {:>7} | {:>3}/{:<3} | {:>6}",
            detection::PARAPHRASE_RUN_WORDS,
            on.len(),
            100.0 - pc(on.len(), all.len()),
            episodes(&on),
            survivors(&on, &acc, Policy::SHIPPED, TTL_MS),
            base,
            held_on
        );
        println!(
            "\n  recall  OFF {r0:.0}% ALL / {m0:.0}% MODERN   ON {r_on:.0}% ALL / {m_on:.0}% MODERN \
             ({s0:.2} -> {s_on:.2} rows per case)"
        );
        // Asserted, not printed: a mask that fires with the switch off is the one
        // failure this whole design is built around not having.
        assert_eq!(
            held_off, 0,
            "the paraphrase bar held a candidate with the switch OFF"
        );
    }
}

/// **WHY THE PARAPHRASE FLOOR MAY NOT BE RAISED, AS A TEST RATHER THAN A NOTE.**
///
/// `bar::paraphrase_bar` is `#[ignore]`d and needs a church's own database, so on
/// its own it leaves the conclusion in a report nobody runs. This is the one part
/// of it that is reproducible from the bundled KJV alone, and it is the part the
/// decision turns on: **the cosine's ordering is inverted at both ends that
/// matter.**
///
/// Measured on 150 hand-read windows of real church speech (services 37 and 40,
/// 2026-09-25): the highest-scoring false positives in the whole sample are stock
/// liturgical formulae, and they outscore all but two of the twenty-eight offers a
/// person judged correct. So there is no value of `SEMANTIC_FLOOR` that keeps the
/// citations and drops the boilerplate — a floor set high enough to silence
/// *"Hallelujah, praise the Lord"* silences *"ten times better than their
/// colleagues"* first, and that is arithmetic rather than tuning.
///
/// **If this test ever fails, the index has changed and the floor is worth
/// revisiting.** That is the only thing it is for. It asserts nothing about
/// whether either score is good.
#[cfg(test)]
mod why_the_floor_holds {
    use super::*;
    use detection::SemanticIndex;

    /// Verbatim from the sample, transcript spelling included, because a cleaned-up
    /// paraphrase of a window is not the window.
    const BOILERPLATE: &str = "Hallelujah. Hallelujah. Praise the Lord.";
    const A_REAL_CITATION: &str =
        "Those three brothers who are ten times better than their colleagues understand that they are";

    fn top(idx: &SemanticIndex, q: &str) -> (String, f32) {
        idx.top_k_explained(q, 3)
            .first()
            .map(|(r, s, _)| (Fire::key_for(r), *s))
            .unwrap_or_default()
    }

    #[test]
    fn a_stock_liturgical_phrase_outscores_a_real_citation() {
        let idx = SemanticIndex::build(&kjv_corpus());
        let (noise_ref, noise) = top(&idx, BOILERPLATE);
        let (cite_ref, cite) = top(&idx, A_REAL_CITATION);
        // Both are genuinely offered today, and that is not the finding.
        assert!(
            noise >= SEMANTIC_FLOOR && cite >= SEMANTIC_FLOOR,
            "both windows must clear the shipped floor for this to be about ordering: \
             {noise_ref} {noise:.3} · {cite_ref} {cite:.3}"
        );
        // THE FINDING. Five words of congregational response beat a retelling of
        // Daniel 1:20 that names the distinctive phrase of the verse.
        assert!(
            noise > cite,
            "the cosine no longer ranks liturgical boilerplate above a real \
             citation ({noise_ref} {noise:.3} vs {cite_ref} {cite:.3}) — the index \
             has changed and whether SEMANTIC_FLOOR can now separate the two \
             populations is worth measuring again with `bar::paraphrase_bar`"
        );
    }

    /// The other half of the same fact, and the half a floor is usually raised
    /// against: the boilerplate's verse is a real, correct, lexical answer. It is
    /// not a bug in the index and cannot be fixed by disbelieving the score.
    #[test]
    fn the_boilerplate_match_is_a_psalm_that_really_does_say_it() {
        let idx = SemanticIndex::build(&kjv_corpus());
        let (r, _) = top(&idx, BOILERPLATE);
        let text = kjv_corpus()
            .into_iter()
            .find(|(v, _)| Fire::key_for(v) == r)
            .map(|(_, t)| t.to_ascii_lowercase())
            .unwrap_or_default();
        assert!(
            text.contains("praise") && text.contains("lord"),
            "{r} was expected to be a praise psalm: {text}"
        );
    }
}

/// **WHAT THE PARAPHRASE BAR COSTS, NAMED RATHER THAN SUMMARISED** — the price of
/// `detection.paraphrase_needs_a_run` in the currency the product's claim is made in.
///
/// `bar::paraphrase_bar` needs a church's own database. This needs nothing: the
/// bundled KJV, the bundled 43-case retelling corpus, and the shipped run test. It
/// runs in CI, and it exists so the cost cannot drift into something warmer — the
/// same reason `detection::which_field_instances_this_rule_can_reach` asserts its
/// ceiling instead of describing it.
#[cfg(test)]
mod what_the_bar_silences {
    use super::*;
    use crate::eval::para_cases;
    use detection::{PhraseIndex, SemanticIndex};

    /// Which labelled retellings STOP reaching the right passage when the bar is on,
    /// and what the whole corpus costs. Both are asserted.
    ///
    /// The reachable set is computed exactly as `main::candidates_for_window` computes
    /// it — `top_k_explained` at `SEMANTIC_SUGGESTIONS_MAX`, `worth_suggesting`'s two
    /// floors, then `PhraseIndex::shared_run_with` against
    /// `detection::PARAPHRASE_RUN_WORDS`.
    #[test]
    fn the_retellings_this_bar_silences_are_named_and_counted() {
        let kjv = kjv_corpus();
        let sem = SemanticIndex::build(&kjv);
        let phrases = PhraseIndex::build(&kjv);
        let cases = para_cases();

        let reaches = |case: &crate::eval::ParaCase, bar: bool| -> bool {
            let hits = sem.top_k_explained(&case.text, SEMANTIC_SUGGESTIONS_MAX);
            let best = hits.first().map(|(_, s, _)| *s).unwrap_or(0.0);
            hits.iter().any(|(r, s, _)| {
                *s >= SEMANTIC_FLOOR
                    && *s >= best * SEMANTIC_RELATIVE_FLOOR
                    && case.contains(r)
                    && (!bar
                        || phrases.shared_run_with(&case.text, r)
                            >= detection::PARAPHRASE_RUN_WORDS)
            })
        };

        let mut silenced: Vec<(&str, &str)> = Vec::new();
        let (mut off, mut on, mut m_off, mut m_on, mut m_n) = (0, 0, 0, 0, 0);
        for case in &cases {
            let (a, b) = (reaches(case, false), reaches(case, true));
            off += a as usize;
            on += b as usize;
            if case.vocab == "modern" {
                m_n += 1;
                m_off += a as usize;
                m_on += b as usize;
            }
            if a && !b {
                silenced.push((&case.id, &case.vocab));
            }
        }
        println!("\n  the bar's price on the 43 labelled retellings");
        println!("  ─────────────────────────────────────────────────────────────");
        for (id, vocab) in &silenced {
            println!("  SILENCED  {id:<26} vocab: {vocab}");
        }
        println!(
            "\n  reachable  OFF {off}/{} ({:.0}%)   ON {on}/{} ({:.0}%)",
            cases.len(),
            off as f32 * 100.0 / cases.len() as f32,
            cases.len(),
            on as f32 * 100.0 / cases.len() as f32
        );
        println!(
            "  modern     OFF {m_off}/{m_n} ({:.0}%)   ON {m_on}/{m_n} ({:.0}%)\n",
            m_off as f32 * 100.0 / m_n as f32,
            m_on as f32 * 100.0 / m_n as f32
        );

        // ── AND WHY IT IS THREE AND NOT THE FIVE §125 NAMED ──────────────────
        //
        // DECISIONS §125 and RG-311 record this bar as silencing *"5 of the 43
        // labelled retellings"* and name them: the four friends tearing open a roof,
        // the prodigal son, Zacchaeus up a tree, the fiery furnace, Paul and Silas at
        // midnight. Measured through the shipped filter chain it is THREE, and the
        // difference is not a disagreement about the bar — it is that two of the five
        // never reached the right passage in the first place. A case the paraphrase
        // path already could not answer cannot be a casualty of a rule that only
        // removes answers, and counting it as one prices the bar in a currency it
        // does not spend.
        //
        // Printed rather than asserted, because it is a fact about the INDEX and will
        // move when the index does.
        println!("  the five §125 named, and whether the paraphrase path reaches them at all");
        for id in [
            "roof-paralytic-modern",
            "prodigal-modern",
            "zacchaeus-modern",
            "furnace-modern",
            "paul-silas-modern",
        ] {
            match cases.iter().find(|c| c.id == id) {
                Some(c) => println!(
                    "    {id:<24} OFF {}   ON {}",
                    if reaches(c, false) {
                        "reached "
                    } else {
                        "MISSED "
                    },
                    if reaches(c, true) {
                        "reached"
                    } else {
                        "MISSED"
                    }
                ),
                None => println!("    {id:<24} not in the corpus under that id"),
            }
        }
        println!();

        // **EVERY CASUALTY IS A MODERN-WORDING RETELLING.** That is the finding, and
        // it is the reason this is a switch and not a constant: the narrative case in
        // modern words is the one the operator asked for by name, and the bar is
        // exactly blind to it — a retelling that uses none of the verse's words
        // cannot share a run of them.
        assert!(
            silenced.iter().all(|(_, vocab)| *vocab == "modern"),
            "the bar silenced a retelling that is not modern-wording: {silenced:?}"
        );
        // ASSERTED, not printed. A later reader who widens or narrows anything on
        // this path should be shown the price before they quote a number for it.
        assert_eq!(
            silenced.len(),
            3,
            "the set of retellings this bar silences changed: {silenced:?}"
        );
        assert_eq!((off, on), (33, 30), "corpus recall moved");
        assert_eq!((m_off, m_on), (7, 4), "modern recall moved");
    }

    /// **WHAT THE RULE COSTS THE DECODER'S BUDGET.** `PhraseIndex::verse_index` is a
    /// linear scan of 31,102 references and `shared_run_with` is quadratic over a
    /// window against one verse, so both are worth a number rather than an argument.
    ///
    /// `cargo test --release run_lookup_cost -- --ignored --nocapture`. Ignored
    /// because it is a benchmark: it builds the whole index and times it, and a timing
    /// assertion in CI is a flake waiting for a loaded machine.
    #[test]
    #[ignore]
    fn run_lookup_cost() {
        let phrases = PhraseIndex::build(&kjv_corpus());
        // A REAL WINDOW, at the length the live path actually hands over: 8 seconds of
        // speech off the operator's own database (service 40, 2026-09-25).
        let heard = "Acts 8, 12, I wisdom dwell with prudence and find out the knowledge of                      witty inventions. Now, what";
        let refs = [
            detection::VerseRef {
                book: "Proverbs".into(),
                chapter: 8,
                verse: 12,
            },
            // The worst case for the scan: the last book in canonical order.
            detection::VerseRef {
                book: "Revelation".into(),
                chapter: 22,
                verse: 21,
            },
            // And a reference the corpus does not hold, which scans the whole table
            // and finds nothing.
            detection::VerseRef {
                book: "Nowhere".into(),
                chapter: 1,
                verse: 1,
            },
        ];
        for r in &refs {
            let n = 2_000;
            let t = std::time::Instant::now();
            let mut sink = 0usize;
            for _ in 0..n {
                sink += phrases.shared_run_with(heard, r);
            }
            let each = t.elapsed().as_secs_f64() * 1e6 / n as f64;
            println!(
                "  {} {}:{:<4} {each:>8.1} µs   (run {})",
                r.book,
                r.chapter,
                r.verse,
                sink / n
            );
        }
        println!(
            "\n  Up to {} of these per window, against a decode of 139-600 ms \
             (DECISIONS §38). The bar is only asked at all when the church turned it \
             on.\n",
            SEMANTIC_SUGGESTIONS_MAX
        );
    }

    /// **AND THE ONE ACCEPTANCE IT COSTS IS NOT A VOCABULARY PROBLEM AT ALL.**
    ///
    /// Service 39, 2026-09-25, 64.6 s: the operator pressed a paraphrase offer of
    /// `John 15:2`, the only one of the three reproducible acceptances the bar
    /// removes. The words WERE the verse's words; whisper mangled two of them —
    /// *"every branch a man that bearer not fruit"* against *"Every branch in me that
    /// beareth not fruit"* — so the longest shared run is two, and the bar cannot tell
    /// that from a coincidence.
    ///
    /// This is the honest half of the trade and it belongs in a test rather than in
    /// prose: **a run test over a transcript is a run test over what the decoder
    /// heard.** Word error rate has never been measured in any language
    /// (`docs/LANGUAGES.md`), so how much of the bar's cost is ASR rather than
    /// vocabulary is unknown, and this is the one instance anybody has looked at.
    #[test]
    fn the_acceptance_it_costs_was_lost_to_a_misheard_word_not_to_modern_wording() {
        let phrases = PhraseIndex::build(&kjv_corpus());
        let heard =
            "It's every branch responsibility. Every. Every branch a man that bearer not fruit in";
        let r = detection::VerseRef {
            book: "John".into(),
            chapter: 15,
            verse: 2,
        };
        assert_eq!(phrases.shared_run_with(heard, &r), 2);
        assert!(
            phrases.shared_run_with(heard, &r) < detection::PARAPHRASE_RUN_WORDS,
            "the bar no longer removes the one acceptance it was measured to remove — \
             re-measure before quoting 2 of 3"
        );
        // And the verse's own words DO clear it, which is what makes the case a
        // decoder problem rather than a rule problem.
        assert!(
            phrases.shared_run_with("every branch in me that beareth not fruit", &r)
                >= detection::PARAPHRASE_RUN_WORDS
        );
    }
}
