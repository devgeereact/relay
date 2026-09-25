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
            candidates_for_window(text, true, &sem, &phrases, &context, router.wall());
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
