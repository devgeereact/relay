//! Service history: transcripts, detections, and operator cues, per service.
//!
//! `detections.status` records who decided — 'auto' (the AI) vs 'manual' (a
//! human). The self-calibrating router learns from that column, so the
//! distinction is load-bearing, not archival colour.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

/// A row for the Library service list. `duration_secs` is derived from the last
/// transcript timestamp; `verses` counts fired detections; `overrides` counts
/// manual-override cues.
#[derive(Debug, Clone, Serialize)]
pub struct ServiceSummary {
    pub id: i64,
    pub date: String,
    pub title: String,
    pub duration_secs: f64,
    pub verses: i64,
    pub overrides: i64,
}

/// A transcript line in a service detail view.
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptRow {
    pub timestamp: f64,
    pub text: String,
    pub language: String,
}

/// A fired detection in a service detail view (verse ref resolved if known).
#[derive(Debug, Clone, Serialize)]
pub struct ServiceDetection {
    pub reference: Option<String>,
    pub method: String,
    pub confidence: f32,
    pub status: String,
    pub fired_at: f64,
    /// **The evidence.** The exact text the detector was looking at when it decided
    /// — written since the field service, and until now readable only by opening the
    /// SQLite file by hand.
    ///
    /// `RELAY_GAP.md` §24 makes *"write every wrong verse into the register, verbatim
    /// from `heard_text`"* a condition of the supervised pilot, and that condition
    /// was unimplementable: the column was in every INSERT and in no SELECT.
    ///
    /// It is deliberately NOT the transcript. Detection runs on every partial STT
    /// hypothesis and only finals are persisted, so in a real service the two
    /// routinely have nothing to do with each other — nine auto-fires were once
    /// logged against a final from three minutes earlier which, replayed through the
    /// detector, produces no matches at all. The transcript says where the service
    /// was; only this says what was heard.
    ///
    /// **Stays out of the timeline and the diagnostic bundle**: this is a fragment of
    /// what a preacher said, so it belongs on the local history screen and nowhere
    /// that leaves the machine (`timeline_tests::nothing_a_preacher_said_reaches_the_timeline`).
    pub heard_text: Option<String>,
}

/// Create a service and return its id.
pub fn create_service(conn: &Connection, date: &str, title: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO services (date, title) VALUES (?1, ?2)",
        (date, title),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert a transcript line; returns its id.
/// The text of one transcript row, if it still exists.
///
/// Used to decide whether a detection's window is the same words as the last
/// persisted transcript — see FIELD F-2. Returns `None` for a missing row rather
/// than erroring: a detection must never fail to be recorded because its
/// provenance could not be checked.
pub fn transcript_text(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT text FROM transcripts WHERE id = ?1", [id], |r| {
        r.get(0)
    })
    .optional()
}

pub fn insert_transcript(
    conn: &Connection,
    service_id: i64,
    timestamp: f64,
    text: &str,
    language: &str,
    confidence: Option<f32>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO transcripts (service_id, timestamp, text, language, confidence)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (service_id, timestamp, text, language, confidence),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert a fired detection linked to a transcript.
/// Record a detection.
///
/// `heard_text` is THE EVIDENCE: the exact text the detector was looking at when
/// this fired. See `ensure_detection_evidence` for why a detection without it is
/// not diagnosable after the fact.
#[allow(clippy::too_many_arguments)]
pub fn insert_detection(
    conn: &Connection,
    transcript_id: i64,
    verse_id: Option<i64>,
    method: &str,
    confidence: f32,
    status: &str,
    fired_at: Option<f64>,
    heard_text: Option<&str>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO detections (transcript_id, verse_id, method, confidence, status, fired_at, heard_text)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (
            transcript_id,
            verse_id,
            method,
            confidence,
            status,
            fired_at,
            heard_text,
        ),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert an operator cue (e.g. "manual_override", "clear_screens").
pub fn insert_cue(
    conn: &Connection,
    service_id: i64,
    cue_type: &str,
    payload_json: Option<&str>,
    triggered_at: f64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO cues (service_id, type, payload_json, triggered_at) VALUES (?1, ?2, ?3, ?4)",
        (service_id, cue_type, payload_json, triggered_at),
    )?;
    Ok(conn.last_insert_rowid())
}

/// All services, newest first, with derived Library counts.
pub fn list_services(conn: &Connection) -> rusqlite::Result<Vec<ServiceSummary>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.date, s.title,
                COALESCE((SELECT MAX(timestamp) FROM transcripts WHERE service_id = s.id), 0.0),
                (SELECT COUNT(*) FROM detections d
                   JOIN transcripts t ON t.id = d.transcript_id
                  WHERE t.service_id = s.id),
                (SELECT COUNT(*) FROM cues c
                  WHERE c.service_id = s.id AND c.type = 'manual_override')
           FROM services s
          ORDER BY s.id DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ServiceSummary {
            id: r.get(0)?,
            date: r.get(1)?,
            title: r.get(2)?,
            duration_secs: r.get(3)?,
            verses: r.get(4)?,
            overrides: r.get(5)?,
        })
    })?;
    rows.collect()
}

/// Transcript lines for a service, in order.
pub fn service_transcripts(
    conn: &Connection,
    service_id: i64,
) -> rusqlite::Result<Vec<TranscriptRow>> {
    let mut stmt = conn.prepare(
        "SELECT timestamp, text, language FROM transcripts
          WHERE service_id = ?1 ORDER BY timestamp",
    )?;
    let rows = stmt.query_map([service_id], |r| {
        Ok(TranscriptRow {
            timestamp: r.get(0)?,
            text: r.get(1)?,
            language: r.get(2)?,
        })
    })?;
    rows.collect()
}

/// Fired detections for a service, in order.
pub fn service_detections(
    conn: &Connection,
    service_id: i64,
) -> rusqlite::Result<Vec<ServiceDetection>> {
    let mut stmt = conn.prepare(
        "SELECT v.book, v.chapter, v.verse, d.method, d.confidence, d.status, d.fired_at,
                d.heard_text
           FROM detections d
           JOIN transcripts t ON t.id = d.transcript_id
           LEFT JOIN verses v ON v.id = d.verse_id
          WHERE t.service_id = ?1
          ORDER BY d.fired_at",
    )?;
    let rows = stmt.query_map([service_id], |r| {
        let book: Option<String> = r.get(0)?;
        let chapter: Option<i64> = r.get(1)?;
        let verse: Option<i64> = r.get(2)?;
        let reference = match (book, chapter, verse) {
            (Some(b), Some(c), Some(v)) => Some(format!("{b} {c}:{v}")),
            _ => None,
        };
        Ok(ServiceDetection {
            reference,
            method: r.get(3)?,
            confidence: r.get(4)?,
            status: r.get(5)?,
            fired_at: r.get::<_, Option<f64>>(6)?.unwrap_or(0.0),
            heard_text: r.get(7)?,
        })
    })?;
    rows.collect()
}

/// How many times a verse has already fired in a service (Phase A6 — the
/// series/repeat tracker). Counts only detections that actually fired.
pub fn count_verse_in_service(
    conn: &Connection,
    service_id: i64,
    verse_id: i64,
) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*)
           FROM detections d
           JOIN transcripts t ON t.id = d.transcript_id
          WHERE t.service_id = ?1 AND d.verse_id = ?2 AND d.fired_at IS NOT NULL",
        (service_id, verse_id),
        |r| r.get(0),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// THE SERVICE TIMELINE
// ─────────────────────────────────────────────────────────────────────────────
//
// One ordered, append-only record of what happened, for the facts that had no
// home anywhere else. It does NOT duplicate `detections` or `cues`: those
// already hold what the AI decided and what the operator pressed, and a second
// copy would be a second answer to one question. `service_timeline` merges all
// three on the way out.
//
// What is written here is deliberately small and content-free. `detail` is a
// short phrase Relay composes — "Main screen", "3 held back" — never verse text,
// never a transcript, never a lyric. This table is the one thing in the history
// that might reasonably be sent to somebody for support, and PRIVACY.md's promise
// does not get an exception for being useful.

/// Every kind of event with nowhere else to live.
///
/// A closed enum rather than free strings: a timeline whose kinds are typos is a
/// timeline nobody can query, and this list is short on purpose. Anything that
/// already lands in `detections` or `cues` does not belong here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    ServiceStarted,
    ServiceEnded,
    RehearsalOn,
    RehearsalOff,
    /// A panic control did not reach the outputs. The single most important row
    /// this table can hold.
    PanicFailed,
    /// A screen stopped answering (RG-01/RG-02).
    OutputLost,
    /// …and came back.
    OutputRecovered,
    /// The operator lifted the service lock.
    LockLifted,
    LockRestored,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::ServiceStarted => "service_started",
            EventKind::ServiceEnded => "service_ended",
            EventKind::RehearsalOn => "rehearsal_on",
            EventKind::RehearsalOff => "rehearsal_off",
            EventKind::PanicFailed => "panic_failed",
            EventKind::OutputLost => "output_lost",
            EventKind::OutputRecovered => "output_recovered",
            EventKind::LockLifted => "lock_lifted",
            EventKind::LockRestored => "lock_restored",
        }
    }
}

/// Append one event. `at_ms` is milliseconds since the service started.
///
/// Returns the sequence number it was given. `seq` is derived inside the same
/// statement as the insert so two events in one millisecond still have an order —
/// a real service produced two fires sharing a timestamp to the tenth of a second,
/// and afterwards nothing could say which came first.
pub fn log_event(
    conn: &Connection,
    service_id: i64,
    at_ms: f64,
    kind: EventKind,
    detail: Option<&str>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO service_events (service_id, seq, at_ms, kind, detail)
         VALUES (?1,
                 (SELECT COALESCE(MAX(seq), 0) + 1 FROM service_events WHERE service_id = ?1),
                 ?2, ?3, ?4)",
        rusqlite::params![service_id, at_ms, kind.as_str(), detail],
    )?;
    conn.query_row(
        "SELECT seq FROM service_events WHERE id = ?1",
        [conn.last_insert_rowid()],
        |r| r.get(0),
    )
}

/// One row of the merged timeline.
///
/// `source` says which store it came from, because the three have different
/// evidential weight and flattening that away is how a replay starts to lie: a
/// `detection` row is what the AI claimed, a `cue` row is what the operator
/// pressed, an `event` row is what Relay observed about itself.
#[derive(Debug, Clone, Serialize)]
pub struct TimelineRow {
    pub at_ms: f64,
    pub source: &'static str,
    pub kind: String,
    pub detail: Option<String>,
}

/// Everything that happened in one service, in order.
///
/// Merged from three tables rather than written to a fourth. Transcripts are
/// deliberately excluded — they are voluminous, they are the church's material,
/// and `service_detail` already serves them to the one screen that shows them.
pub fn service_timeline(conn: &Connection, service_id: i64) -> rusqlite::Result<Vec<TimelineRow>> {
    let mut out: Vec<TimelineRow> = Vec::new();

    let mut ev = conn.prepare(
        "SELECT at_ms, kind, detail FROM service_events WHERE service_id = ?1 ORDER BY seq",
    )?;
    for r in ev.query_map([service_id], |r| {
        Ok(TimelineRow {
            at_ms: r.get(0)?,
            source: "event",
            kind: r.get(1)?,
            detail: r.get(2)?,
        })
    })? {
        out.push(r?);
    }

    let mut cu = conn.prepare(
        "SELECT triggered_at, type, payload_json FROM cues WHERE service_id = ?1 ORDER BY id",
    )?;
    for r in cu.query_map([service_id], |r| {
        Ok(TimelineRow {
            // `cues.triggered_at` is SECONDS since the start; the timeline is ms.
            at_ms: r.get::<_, f64>(0)? * 1000.0,
            source: "cue",
            kind: r.get(1)?,
            detail: r.get(2)?,
        })
    })? {
        out.push(r?);
    }

    // A detection's `status` is the useful kind here — auto, suggested, dismissed
    // or manual — because "the AI fired this" and "a human fired this" are the two
    // facts a replay is trying to separate.
    let mut de = conn.prepare(
        "SELECT COALESCE(d.fired_at, t.timestamp) * 1000.0, d.status, v.book, v.chapter, v.verse
           FROM detections d
           JOIN transcripts t ON t.id = d.transcript_id
           LEFT JOIN verses v ON v.id = d.verse_id
          WHERE t.service_id = ?1
          ORDER BY d.id",
    )?;
    for r in de.query_map([service_id], |r| {
        let book: Option<String> = r.get(2)?;
        let chapter: Option<i64> = r.get(3)?;
        let verse: Option<i64> = r.get(4)?;
        let detail = match (book, chapter, verse) {
            (Some(b), Some(c), Some(v)) => Some(format!("{b} {c}:{v}")),
            _ => None,
        };
        Ok(TimelineRow {
            at_ms: r.get(0)?,
            source: "detection",
            kind: r.get(1)?,
            detail,
        })
    })? {
        out.push(r?);
    }

    // Stable sort: rows from one source keep their insertion order when two share
    // a timestamp, which is exactly the case `seq` exists to survive.
    out.sort_by(|a, b| a.at_ms.total_cmp(&b.at_ms));
    Ok(out)
}

/// Store one snapshot of the latency instrument.
///
/// Percentiles only. A raw trace carries what was heard; this table is the one
/// part of the history that might reasonably be sent to somebody for support, and
/// "nothing leaves the device without an explicit, visible reason" does not get an
/// exception for being useful.
///
/// `p50`/`p95`/`worst` are `Option` because **a stage that was never reached is an
/// absence, not a zero** — `latency.rs` learned this, and writing 0 here would make
/// every service look instantaneous on the stages it never performed.
pub fn log_perf_sample(
    conn: &Connection,
    service_id: i64,
    at_ms: f64,
    s: &PerfSample<'_>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO perf_samples
           (service_id, at_ms, metric, samples, p50_ms, p95_ms, p99_ms, worst_ms, last_minute_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            service_id,
            at_ms,
            s.metric,
            s.samples,
            s.p50_ms,
            s.p95_ms,
            s.p99_ms,
            s.worst_ms,
            s.last_minute_ms
        ],
    )?;
    Ok(())
}

/// One metric's percentiles, on the way in.
///
/// A struct rather than six positional arguments: three of them are
/// `Option<f64>` in a row, and swapping p50 with p95 at a call site would compile,
/// pass every test, and quietly mis-describe every service ever recorded.
pub struct PerfSample<'a> {
    pub metric: &'a str,
    pub samples: i64,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    /// One window in a hundred — roughly one visibly late verse per service, which
    /// is the thing a congregation notices and a median cannot show.
    pub p99_ms: Option<f64>,
    pub worst_ms: Option<f64>,
    /// The mean of the most recent complete MINUTE, not of the whole service.
    ///
    /// FIELD F-3: everything else in this row is cumulative since app start
    /// (`latency::report(0)`), and a cumulative percentile is structurally
    /// insensitive to drift — `worst_ms` can only ever rise, and a p50 diluted by
    /// thirty earlier minutes barely moves when the last five get worse. Stage
    /// F11 asks precisely whether the per-minute line RISES, and answering it
    /// meant inferring from a flat p50 under a growing denominator.
    ///
    /// The per-minute means already existed in the live report and were simply
    /// never written down, so they vanished on quit — which is the same defect
    /// RG-04 was created to fix, one level in.
    pub last_minute_ms: Option<f64>,
}

/// Every latency snapshot for one service, oldest first.
#[derive(Debug, Clone, Serialize)]
pub struct PerfRow {
    pub at_ms: f64,
    pub metric: String,
    pub samples: i64,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub p99_ms: Option<f64>,
    pub worst_ms: Option<f64>,
    /// The mean of the most recent complete minute — the per-minute line Stage F11
    /// actually asks about. Null on rows written before the column existed, and
    /// that is an absence, not a zero.
    pub last_minute_ms: Option<f64>,
}

pub fn service_perf(conn: &Connection, service_id: i64) -> rusqlite::Result<Vec<PerfRow>> {
    let mut st = conn.prepare(
        "SELECT at_ms, metric, samples, p50_ms, p95_ms, p99_ms, worst_ms, last_minute_ms
           FROM perf_samples WHERE service_id = ?1 ORDER BY at_ms, metric",
    )?;
    let rows = st.query_map([service_id], |r| {
        Ok(PerfRow {
            at_ms: r.get(0)?,
            metric: r.get(1)?,
            samples: r.get(2)?,
            p50_ms: r.get(3)?,
            p95_ms: r.get(4)?,
            p99_ms: r.get(5)?,
            worst_ms: r.get(6)?,
            last_minute_ms: r.get(7)?,
        })
    })?;
    rows.collect()
}

/// One row per SERVICE for a metric — its last snapshot — newest first.
///
/// The question a single service cannot answer: **is this getting slower week by
/// week?** A church that adds a bigger model, or whose laptop fills up over a
/// winter, degrades gradually, and every individual Sunday looks fine. `latency.rs`
/// answers "did it grow *during* this service"; this answers "did it grow across
/// them".
#[derive(Debug, Clone, Serialize)]
pub struct PerfTrend {
    pub service_id: i64,
    pub date: String,
    pub samples: i64,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub p99_ms: Option<f64>,
}

pub fn perf_history(
    conn: &Connection,
    metric: &str,
    limit: i64,
) -> rusqlite::Result<Vec<PerfTrend>> {
    // The LAST sample of each service. `latency.rs`'s percentiles are cumulative, so
    // the final snapshot already covers everything before it — averaging the
    // snapshots would weight a service's first minute as heavily as its eightieth.
    let mut st = conn.prepare(
        "SELECT p.service_id, s.date, p.samples, p.p50_ms, p.p95_ms, p.p99_ms
           FROM perf_samples p
           JOIN services s ON s.id = p.service_id
          WHERE p.metric = ?1
            AND p.at_ms = (SELECT MAX(at_ms) FROM perf_samples
                            WHERE service_id = p.service_id AND metric = ?1)
          ORDER BY p.service_id DESC
          LIMIT ?2",
    )?;
    let rows = st.query_map(rusqlite::params![metric, limit], |r| {
        Ok(PerfTrend {
            service_id: r.get(0)?,
            date: r.get(1)?,
            samples: r.get(2)?,
            p50_ms: r.get(3)?,
            p95_ms: r.get(4)?,
            p99_ms: r.get(5)?,
        })
    })?;
    rows.collect()
}

/// Create the timeline tables on an existing database.
///
/// RETRYABLE, and additive only: `CREATE TABLE IF NOT EXISTS` plus its index, no
/// table rebuild, nothing dropped. CLAUDE.md rule 25 exists because a migration
/// that could half-apply left a scratch table behind and bricked every subsequent
/// boot; the safest migration is the one with no intermediate state to leave.
pub fn ensure_service_events(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS service_events (
            id         INTEGER PRIMARY KEY,
            service_id INTEGER NOT NULL REFERENCES services(id),
            seq        INTEGER NOT NULL,
            at_ms      REAL NOT NULL,
            kind       TEXT NOT NULL,
            detail     TEXT
         );
         CREATE INDEX IF NOT EXISTS idx_service_events ON service_events(service_id, seq);
         CREATE TABLE IF NOT EXISTS perf_samples (
            id         INTEGER PRIMARY KEY,
            service_id INTEGER NOT NULL REFERENCES services(id),
            at_ms      REAL NOT NULL,
            metric     TEXT NOT NULL,
            samples    INTEGER NOT NULL,
            p50_ms     REAL,
            p95_ms     REAL,
            worst_ms   REAL
         );
         CREATE INDEX IF NOT EXISTS idx_perf_samples ON perf_samples(service_id, at_ms);",
    )?;
    // p99 arrived after the table did. A bare `ALTER TABLE ADD COLUMN` errors with
    // "duplicate column name" on the second boot and panics the app before the
    // window is shown (rule 25), so it is sniffed first — the same shape as
    // `ensure_detection_evidence`.
    let has_p99: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('perf_samples') WHERE name = 'p99_ms'",
        [],
        |r| r.get(0),
    )?;
    if has_p99 == 0 {
        conn.execute_batch("ALTER TABLE perf_samples ADD COLUMN p99_ms REAL;")?;
    }
    // `last_minute_ms` arrived later still, and by the same rule: sniffed, never a
    // bare ALTER, or the second boot dies with "duplicate column name" before the
    // window is shown (rule 25).
    let has_minute: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('perf_samples') WHERE name = 'last_minute_ms'",
        [],
        |r| r.get(0),
    )?;
    if has_minute == 0 {
        conn.execute_batch("ALTER TABLE perf_samples ADD COLUMN last_minute_ms REAL;")?;
    }
    Ok(())
}

#[cfg(test)]
mod timeline_tests {
    use super::*;
    use rusqlite::Connection;

    /// A database with just enough of the schema to hold a timeline.
    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE services (id INTEGER PRIMARY KEY, date TEXT NOT NULL, title TEXT NOT NULL);
             CREATE TABLE transcripts (id INTEGER PRIMARY KEY, service_id INTEGER NOT NULL,
                timestamp REAL NOT NULL, text TEXT NOT NULL, language TEXT NOT NULL, confidence REAL);
             CREATE TABLE verses (id INTEGER PRIMARY KEY, translation_id INTEGER, book TEXT,
                chapter INTEGER, verse INTEGER, text TEXT);
             CREATE TABLE detections (id INTEGER PRIMARY KEY, transcript_id INTEGER NOT NULL,
                verse_id INTEGER, method TEXT NOT NULL, confidence REAL NOT NULL,
                status TEXT NOT NULL, fired_at REAL, heard_text TEXT);
             CREATE TABLE cues (id INTEGER PRIMARY KEY, service_id INTEGER NOT NULL,
                type TEXT NOT NULL, payload_json TEXT, triggered_at REAL NOT NULL);
             INSERT INTO services (id, date, title) VALUES (1, '2026-08-29', 'Sunday');",
        )
        .unwrap();
        ensure_service_events(&conn).unwrap();
        conn
    }

    /// THE MIGRATION MUST BE RETRYABLE — CLAUDE.md rule 25.
    ///
    /// It runs on every boot. The failure it guards against is not theoretical:
    /// a rung that errored the second time panicked the app at startup, before the
    /// window was shown, on every subsequent launch, forever.
    #[test]
    fn ensure_service_events_is_retryable() {
        let conn = db();
        ensure_service_events(&conn).unwrap();
        ensure_service_events(&conn).unwrap();
        log_event(&conn, 1, 0.0, EventKind::ServiceStarted, Some("Sunday")).unwrap();
        assert_eq!(service_timeline(&conn, 1).unwrap().len(), 1);
    }

    /// FIELD F-3 · the per-minute line has to survive a quit.
    ///
    /// Everything else in a `perf_samples` row is cumulative since app start, and
    /// a cumulative percentile cannot answer "did it get worse" — `worst_ms` only
    /// rises, and a p50 diluted by thirty good minutes barely moves when the last
    /// five are bad. Stage F11 asks exactly that question, and answering it in the
    /// field meant inferring from a flat p50 under a growing denominator.
    #[test]
    fn a_perf_sample_carries_the_last_minute_not_only_the_whole_service() {
        let conn = db();
        ensure_service_events(&conn).unwrap();
        log_perf_sample(
            &conn,
            1,
            60_000.0,
            &PerfSample {
                metric: "stt_decode",
                samples: 100,
                p50_ms: Some(687.0),
                p95_ms: Some(2673.0),
                p99_ms: Some(2801.0),
                worst_ms: Some(3016.0),
                last_minute_ms: Some(1490.0),
            },
        )
        .unwrap();
        let rows = service_perf(&conn, 1).unwrap();
        assert_eq!(rows.len(), 1);
        // The cumulative p50 says the service is fine. The last minute says it is
        // not, and only one of those two is the question F11 asks.
        assert_eq!(rows[0].p50_ms, Some(687.0));
        assert_eq!(rows[0].last_minute_ms, Some(1490.0));
    }

    /// A row written before the column existed reports an ABSENCE, not a zero.
    /// A 0 ms minute would read as the fastest minute of the service.
    #[test]
    fn a_perf_row_from_before_the_column_says_nothing_rather_than_zero() {
        let conn = db();
        ensure_service_events(&conn).unwrap();
        conn.execute(
            "INSERT INTO perf_samples (service_id, at_ms, metric, samples, p50_ms)
             VALUES (1, 0.0, 'stt_decode', 5, 700.0)",
            [],
        )
        .unwrap();
        assert_eq!(service_perf(&conn, 1).unwrap()[0].last_minute_ms, None);
    }

    /// TWO EVENTS IN THE SAME MILLISECOND STILL HAVE AN ORDER.
    ///
    /// A real service produced two fires sharing a timestamp to the tenth of a
    /// second, and afterwards nothing could say which came first. `seq` is what
    /// makes the record reconstructable rather than merely plausible.
    #[test]
    fn events_are_ordered_even_at_the_same_instant() {
        let conn = db();
        let a = log_event(&conn, 1, 5.0, EventKind::OutputLost, Some("Main")).unwrap();
        let b = log_event(&conn, 1, 5.0, EventKind::OutputRecovered, Some("Main")).unwrap();
        assert!(b > a, "seq must be monotonic within a service");

        let t = service_timeline(&conn, 1).unwrap();
        assert_eq!(t[0].kind, "output_lost");
        assert_eq!(t[1].kind, "output_recovered");
    }

    /// SEQUENCES ARE PER SERVICE, so last Sunday cannot renumber this one.
    #[test]
    fn each_service_numbers_its_own_events() {
        let conn = db();
        conn.execute(
            "INSERT INTO services (id, date, title) VALUES (2, '2026-09-05', 'Next')",
            [],
        )
        .unwrap();
        log_event(&conn, 1, 0.0, EventKind::ServiceStarted, None).unwrap();
        log_event(&conn, 1, 1.0, EventKind::ServiceEnded, None).unwrap();
        assert_eq!(
            log_event(&conn, 2, 0.0, EventKind::ServiceStarted, None).unwrap(),
            1
        );
    }

    /// THE THREE SOURCES STAY DISTINGUISHABLE.
    ///
    /// "The AI fired this" and "a human fired this" are the two facts a replay
    /// exists to separate, and a merged timeline that forgot which store a row came
    /// from would be exactly the flattening that makes a record untrustworthy.
    #[test]
    fn the_timeline_merges_three_stores_and_says_which_is_which() {
        let conn = db();
        conn.execute_batch(
            "INSERT INTO verses (id, translation_id, book, chapter, verse, text)
                VALUES (10, 1, 'John', 3, 16, 'For God so loved…');
             INSERT INTO transcripts (id, service_id, timestamp, text, language)
                VALUES (100, 1, 30.0, 'turn to john three sixteen', 'en');
             INSERT INTO detections (transcript_id, verse_id, method, confidence, status, fired_at)
                VALUES (100, 10, 'direct', 0.9, 'auto', 30.5);
             INSERT INTO cues (service_id, type, payload_json, triggered_at)
                VALUES (1, 'clear_screens', NULL, 40.0);",
        )
        .unwrap();
        log_event(&conn, 1, 0.0, EventKind::ServiceStarted, Some("Sunday")).unwrap();
        log_event(&conn, 1, 60_000.0, EventKind::ServiceEnded, None).unwrap();

        let t = service_timeline(&conn, 1).unwrap();
        let sources: Vec<&str> = t.iter().map(|r| r.source).collect();
        assert_eq!(
            sources,
            vec!["event", "detection", "cue", "event"],
            "in time order, and each row still knows where it came from"
        );
        // Seconds vs milliseconds: `cues.triggered_at` is seconds and the timeline
        // is milliseconds. Getting this wrong put every operator action in the
        // first second of the service.
        assert_eq!(t[2].at_ms, 40_000.0);
        assert_eq!(t[1].detail.as_deref(), Some("John 3:16"));
    }

    /// A STAGE NEVER REACHED IS STORED AS NULL, NOT AS ZERO.
    ///
    /// The same rule `latency.rs` enforces in memory. Writing 0 would make every
    /// service look instantaneous on the stages it never performed — and the report
    /// would improve as the pipeline got worse.
    #[test]
    fn an_unreached_stage_is_absent_in_history_too() {
        let conn = db();
        log_perf_sample(
            &conn,
            1,
            0.0,
            &PerfSample {
                metric: "reference_detection_to_fire",
                samples: 0,
                p50_ms: None,
                p95_ms: None,
                p99_ms: None,
                last_minute_ms: None,
                worst_ms: None,
            },
        )
        .unwrap();
        log_perf_sample(
            &conn,
            1,
            0.0,
            &PerfSample {
                metric: "audio_to_partial_transcript",
                samples: 50,
                p50_ms: Some(139.0),
                p95_ms: Some(339.0),
                p99_ms: Some(498.0),
                last_minute_ms: None,
                worst_ms: Some(543.0),
            },
        )
        .unwrap();
        let rows = service_perf(&conn, 1).unwrap();
        let unreached = rows
            .iter()
            .find(|r| r.metric == "reference_detection_to_fire")
            .unwrap();
        assert!(unreached.p50_ms.is_none(), "an absence, never a zero");
        let reached = rows
            .iter()
            .find(|r| r.metric == "audio_to_partial_transcript")
            .unwrap();
        assert_eq!(reached.p50_ms, Some(139.0));
    }

    /// THE p99 COLUMN IS ADDED RETRYABLY.
    ///
    /// It arrived after the table did, and `ensure_service_events` runs on EVERY
    /// boot. A bare `ALTER TABLE ADD COLUMN` errors with "duplicate column name" the
    /// second time and panics the app before the window is shown — rule 25, one
    /// layer down and for the third time.
    #[test]
    fn adding_p99_to_an_existing_table_is_retryable() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE services (id INTEGER PRIMARY KEY, date TEXT NOT NULL, title TEXT NOT NULL);
             -- the table as it shipped BEFORE p99 existed
             CREATE TABLE perf_samples (
                id INTEGER PRIMARY KEY, service_id INTEGER NOT NULL, at_ms REAL NOT NULL,
                metric TEXT NOT NULL, samples INTEGER NOT NULL,
                p50_ms REAL, p95_ms REAL, worst_ms REAL);
             INSERT INTO services (id, date, title) VALUES (1, '2026-08-29', 'Sunday');",
        )
        .unwrap();
        ensure_service_events(&conn).unwrap();
        ensure_service_events(&conn).unwrap();
        ensure_service_events(&conn).unwrap();

        // …and the old rows survive, reading p99 as the absence it is.
        conn.execute(
            "INSERT INTO perf_samples (service_id, at_ms, metric, samples, p50_ms, p95_ms, worst_ms)
             VALUES (1, 0.0, 'm', 5, 100.0, 200.0, 300.0)",
            [],
        )
        .unwrap();
        let rows = service_perf(&conn, 1).unwrap();
        assert_eq!(rows[0].p50_ms, Some(100.0));
        assert!(
            rows[0].p99_ms.is_none(),
            "a column added later is absent, not zero"
        );
    }

    /// THE TREND TAKES ONE ROW PER SERVICE, AND IT IS THE LAST ONE.
    ///
    /// The percentiles are cumulative, so a service's final snapshot already covers
    /// everything before it. Averaging the snapshots would weight the first minute
    /// of a service as heavily as the eightieth.
    #[test]
    fn the_trend_is_one_row_per_service_newest_first() {
        let conn = db();
        conn.execute(
            "INSERT INTO services (id, date, title) VALUES (2, '2026-09-05', 'Next')",
            [],
        )
        .unwrap();
        let put = |svc: i64, at: f64, p50: f64| {
            log_perf_sample(
                &conn,
                svc,
                at,
                &PerfSample {
                    metric: "audio_to_partial_transcript",
                    samples: 100,
                    p50_ms: Some(p50),
                    p95_ms: Some(p50 * 2.0),
                    p99_ms: Some(p50 * 3.0),
                    last_minute_ms: None,
                    worst_ms: Some(p50 * 4.0),
                },
            )
            .unwrap();
        };
        put(1, 60_000.0, 140.0);
        put(1, 600_000.0, 152.0); // the one that counts for service 1
        put(2, 60_000.0, 300.0);

        let t = perf_history(&conn, "audio_to_partial_transcript", 12).unwrap();
        assert_eq!(t.len(), 2, "one row per service, not one per snapshot");
        assert_eq!(t[0].service_id, 2, "newest first");
        assert_eq!(
            t[1].p50_ms,
            Some(152.0),
            "the LAST snapshot of that service"
        );
        assert_eq!(t[0].date, "2026-09-05");
    }

    #[test]
    fn the_trend_is_per_metric_and_bounded() {
        let conn = db();
        log_perf_sample(
            &conn,
            1,
            0.0,
            &PerfSample {
                metric: "stt_decode",
                samples: 10,
                p50_ms: Some(140.0),
                p95_ms: None,
                p99_ms: None,
                last_minute_ms: None,
                worst_ms: None,
            },
        )
        .unwrap();
        assert_eq!(perf_history(&conn, "stt_decode", 12).unwrap().len(), 1);
        assert!(perf_history(&conn, "audio_to_partial_transcript", 12)
            .unwrap()
            .is_empty());
        assert!(perf_history(&conn, "stt_decode", 0).unwrap().is_empty());
    }

    /// RG-81 — the evidence must reach the OPERATOR and nothing else.
    ///
    /// `heard_text` was written on every fire and returned by no query: `RELAY_GAP.md`
    /// §24 makes *"write every wrong verse into the register, verbatim from
    /// `heard_text`"* a pilot condition, and it could only be satisfied by opening the
    /// SQLite file by hand.
    ///
    /// Adding it to `service_detections` is the fix. **The risk it creates is the one
    /// this module is most careful about**: it is a fragment of what a preacher said,
    /// so it must reach the local history screen and NOT the timeline, which is the
    /// part of the record most likely to be sent to somebody. Both halves are asserted
    /// here, in one test, so neither can be satisfied alone.
    #[test]
    fn the_evidence_reaches_the_history_screen_and_not_the_timeline() {
        let conn = db();
        conn.execute_batch(
            "INSERT INTO transcripts (id, service_id, timestamp, text, language)
                VALUES (200, 1, 30.0, 'a final from three minutes ago', 'en');
             INSERT INTO detections (transcript_id, verse_id, method, confidence, status,
                fired_at, heard_text)
                VALUES (200, NULL, 'direct', 0.9, 'auto', 30.5,
                        'turn with me to psalm twenty three');",
        )
        .unwrap();

        // The operator CAN read it — this is the pilot condition.
        let dets = service_detections(&conn, 1).unwrap();
        let found = dets
            .iter()
            .find_map(|d| d.heard_text.as_deref())
            .expect("heard_text must be readable without opening the database by hand");
        assert_eq!(found, "turn with me to psalm twenty three");

        // …and it still does not reach the timeline.
        let dump = format!("{:?}", service_timeline(&conn, 1).unwrap());
        assert!(
            !dump.contains("turn with me"),
            "heard_text must never reach the timeline: {dump}"
        );
    }

    /// THE TIMELINE CARRIES NO CONTENT.
    ///
    /// This is the part of the history most likely to travel — it is what a church
    /// would send back with "it went wrong at 10:31". PRIVACY.md's promise does not
    /// get an exception for being useful, so `detail` is a phrase Relay composes,
    /// never something a preacher said.
    #[test]
    fn nothing_a_preacher_said_reaches_the_timeline() {
        let conn = db();
        conn.execute_batch(
            "INSERT INTO transcripts (id, service_id, timestamp, text, language)
                VALUES (100, 1, 30.0, 'the sermon text nobody may export', 'en');
             INSERT INTO detections (transcript_id, verse_id, method, confidence, status, fired_at,
                heard_text)
                VALUES (100, NULL, 'direct', 0.9, 'auto', 30.5, 'the exact words heard');",
        )
        .unwrap();
        let t = service_timeline(&conn, 1).unwrap();
        let all = format!("{t:?}");
        assert!(
            !all.contains("sermon text"),
            "no transcript in the timeline"
        );
        assert!(
            !all.contains("exact words"),
            "no heard_text in the timeline"
        );
    }
}

/// Index the foreign keys every history query walks down.
///
/// `transcripts.service_id`, `detections.transcript_id` and `cues.service_id` had
/// no index. SQLite indexes a PRIMARY KEY and a UNIQUE constraint automatically; it
/// does **not** index a `REFERENCES` column. So opening a service in History,
/// building its timeline, replaying a moment in it, and erasing it all scanned the
/// whole table — and `transcripts` and `detections` are the two tables that grow
/// without limit, one row per utterance, for as long as a church keeps using Relay.
///
/// `detections.verse_id` is here for a second reason, and it is a startup one:
/// the corpus repair (`db::mod::ensure_corpus_repair`) deletes every verse, and
/// SQLite must scan `detections` once per parent row to enforce the reference
/// unless that column is indexed. Measured on a synthetic database of 31,102
/// verses and 20,000 detections: **8.3 s without the index, 0.007 s with it** —
/// and that time is spent at boot, before the window exists, with nothing on
/// screen to say why.
///
/// RETRYABLE and additive only: `CREATE INDEX IF NOT EXISTS`, no table rebuild,
/// nothing dropped, no intermediate state to strand (CLAUDE.md rule 25).
///
/// Also in `docs/data/schema.sql`, which IS the shipped baseline — so a fresh
/// install gets them at creation and an existing one gets them on the next boot.
pub fn ensure_history_indexes(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_transcripts_service ON transcripts(service_id);
         CREATE INDEX IF NOT EXISTS idx_detections_transcript ON detections(transcript_id);
         CREATE INDEX IF NOT EXISTS idx_cues_service ON cues(service_id);
         CREATE INDEX IF NOT EXISTS idx_detections_verse ON detections(verse_id);",
    )
}

/// Erase one service and everything recorded under it.
///
/// ## Why this exists
///
/// Relay's most sensitive holding is not a password. It is `transcripts.text` —
/// verbatim, near-real-time text of what a preacher said to their congregation,
/// plus `detections.heard_text`, the exact sentence behind every verse that went
/// on a wall. Every document in this repository says that content never leaves the
/// device. None of them said how to get rid of it, because there was no way:
/// `PRIVACY.md` answered the question with *"delete that folder"* — quit Relay,
/// find `~/Library/Application Support/com.relay.app`, and delete the database
/// holding every service ever recorded, or nothing.
///
/// "All of it, from the Finder" is not a deletion feature. A church that wants one
/// sermon gone — a pastoral conversation read into the room, a visiting speaker
/// who asked, a service recorded by mistake — could not have it.
///
/// ## What it removes, and the order it has to remove it in
///
/// Children first: `detections` hang off `transcripts`, and the rest hang off
/// `services`. Foreign keys are ON (`db/mod.rs`), so a wrong order is a constraint
/// error rather than a silent orphan — but the order is written out rather than
/// discovered, because the error would arrive at a person pressing Delete.
///
/// All of it in ONE transaction. A half-deleted service is a worse state than
/// either end: the timeline would render, the transcript behind it would not, and
/// nothing would say why.
///
/// Returns the number of transcript rows removed, so the surface that asked can
/// say what it did rather than claiming a success in the abstract.
pub fn delete_service(conn: &Connection, id: i64) -> rusqlite::Result<i64> {
    let tx = conn.unchecked_transaction()?;
    let transcripts: i64 = tx.query_row(
        "SELECT COUNT(*) FROM transcripts WHERE service_id = ?1",
        [id],
        |r| r.get(0),
    )?;
    tx.execute(
        "DELETE FROM detections
          WHERE transcript_id IN (SELECT id FROM transcripts WHERE service_id = ?1)",
        [id],
    )?;
    tx.execute("DELETE FROM transcripts WHERE service_id = ?1", [id])?;
    tx.execute("DELETE FROM cues WHERE service_id = ?1", [id])?;
    tx.execute("DELETE FROM service_events WHERE service_id = ?1", [id])?;
    tx.execute("DELETE FROM perf_samples WHERE service_id = ?1", [id])?;
    tx.execute("DELETE FROM services WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(transcripts)
}

#[cfg(test)]
mod erase_tests {
    use super::*;
    use rusqlite::Connection;

    /// The real schema, and foreign keys ON — the same two conditions the app runs
    /// under. A delete tested with FKs off is a delete whose ORDER was never checked.
    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../../../docs/data/schema.sql"))
            .unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        conn
    }

    /// Populate one service with a row in every table that hangs off it.
    fn service_with_everything(conn: &Connection, title: &str) -> i64 {
        let id = create_service(conn, "2026-09-02", title).unwrap();
        let t = insert_transcript(conn, id, 1.0, "and if you turn to romans eight", "en", None)
            .unwrap();
        insert_detection(
            conn,
            t,
            None,
            "direct",
            0.9,
            "auto",
            Some(1.5),
            Some("romans eight"),
        )
        .unwrap();
        insert_cue(conn, id, "manual_override", None, 2.0).unwrap();
        log_event(conn, id, 0.0, EventKind::ServiceStarted, Some("Sunday")).unwrap();
        log_perf_sample(
            conn,
            id,
            60_000.0,
            &PerfSample {
                metric: "fire",
                samples: 12,
                p50_ms: Some(1.0),
                p95_ms: Some(2.0),
                p99_ms: Some(3.0),
                worst_ms: Some(4.0),
                last_minute_ms: Some(1.5),
            },
        )
        .unwrap();
        id
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    /// THE GAP. There was no way, from inside Relay, to remove a recorded sermon.
    /// PRIVACY.md's only answer was "delete that folder" — every service or none.
    #[test]
    fn erasing_a_service_removes_every_trace_of_it() {
        let conn = db();
        let id = service_with_everything(&conn, "Sunday");
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM transcripts"), 1);

        let gone = delete_service(&conn, id).unwrap();
        assert_eq!(gone, 1, "reports what it removed, not just that it worked");

        for table in [
            "services",
            "transcripts",
            "detections",
            "cues",
            "service_events",
            "perf_samples",
        ] {
            assert_eq!(
                count(&conn, &format!("SELECT COUNT(*) FROM {table}")),
                0,
                "{table} still holds a row from the erased service"
            );
        }
    }

    /// Erasing one sermon must not erase the year. The children are found by
    /// `service_id` (and, for detections, by their transcript's), so a query that
    /// forgot its WHERE would take everything and the operator would be told the
    /// same cheerful sentence either way.
    #[test]
    fn erasing_one_service_leaves_the_others_untouched() {
        let conn = db();
        let a = service_with_everything(&conn, "The one to erase");
        let b = service_with_everything(&conn, "The one to keep");

        delete_service(&conn, a).unwrap();

        assert_eq!(count(&conn, "SELECT COUNT(*) FROM services"), 1);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM transcripts"), 1);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM detections"), 1);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM cues"), 1);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM service_events"), 1);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM perf_samples"), 1);
        let kept: i64 = conn
            .query_row("SELECT service_id FROM transcripts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(kept, b);
    }

    /// A service that recorded nothing is still a row, and still erasable. The
    /// count it reports is honestly zero rather than a failure.
    #[test]
    fn an_empty_service_erases_cleanly() {
        let conn = db();
        let id = create_service(&conn, "2026-09-02", "Cancelled").unwrap();
        assert_eq!(delete_service(&conn, id).unwrap(), 0);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM services"), 0);
    }

    /// Erasing something that is not there is not an error — two clicks on the same
    /// armed button, or two windows open on the same service, must not produce a
    /// failure the operator has to interpret.
    #[test]
    fn erasing_a_service_that_is_already_gone_is_not_a_failure() {
        let conn = db();
        let id = service_with_everything(&conn, "Sunday");
        delete_service(&conn, id).unwrap();
        assert_eq!(delete_service(&conn, id).unwrap(), 0);
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;
    use rusqlite::Connection;

    fn indexes(conn: &Connection) -> Vec<String> {
        let mut st = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'index' ORDER BY name")
            .unwrap();
        let rows = st.query_map([], |r| r.get::<_, String>(0)).unwrap();
        rows.filter_map(|r| r.ok()).collect()
    }

    /// The three foreign keys every history query walks down. SQLite indexes a
    /// PRIMARY KEY and a UNIQUE constraint on its own; it does NOT index a
    /// `REFERENCES` column, and these three had nothing — so opening a service,
    /// building its timeline, replaying a moment and erasing it all scanned the two
    /// tables that grow one row per utterance for the life of the installation.
    #[test]
    fn the_shipped_baseline_indexes_the_history_foreign_keys() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../../../docs/data/schema.sql"))
            .unwrap();
        let have = indexes(&conn);
        for want in [
            "idx_transcripts_service",
            "idx_detections_transcript",
            "idx_cues_service",
        ] {
            assert!(have.iter().any(|n| n == want), "missing {want} in {have:?}");
        }
    }

    /// An existing installation gets them on the next boot, and the rung is
    /// RETRYABLE (CLAUDE.md rule 25) — run it repeatedly and nothing errors and
    /// nothing accumulates.
    #[test]
    fn the_rung_adds_them_to_an_old_database_and_survives_being_run_again() {
        let conn = Connection::open_in_memory().unwrap();
        // A database from before the indexes existed: the same tables, no indexes.
        conn.execute_batch(
            "CREATE TABLE services (id INTEGER PRIMARY KEY, date TEXT NOT NULL, title TEXT NOT NULL);
             CREATE TABLE transcripts (id INTEGER PRIMARY KEY, service_id INTEGER NOT NULL,
                timestamp REAL NOT NULL, text TEXT NOT NULL, language TEXT NOT NULL, confidence REAL);
             CREATE TABLE detections (id INTEGER PRIMARY KEY, transcript_id INTEGER NOT NULL,
                verse_id INTEGER, method TEXT NOT NULL, confidence REAL NOT NULL,
                status TEXT NOT NULL, fired_at REAL, heard_text TEXT);
             CREATE TABLE cues (id INTEGER PRIMARY KEY, service_id INTEGER NOT NULL,
                type TEXT NOT NULL, payload_json TEXT, triggered_at REAL NOT NULL);",
        )
        .unwrap();
        assert!(indexes(&conn).is_empty());

        ensure_history_indexes(&conn).unwrap();
        let after_once = indexes(&conn);
        // Four: the three foreign keys a history query reads through, plus
        // `detections.verse_id`, which is not for a query at all — it is what
        // keeps the corpus repair's `DELETE FROM verses` from scanning
        // `detections` once per deleted row, at boot.
        assert_eq!(after_once.len(), 4, "{after_once:?}");

        ensure_history_indexes(&conn).unwrap();
        ensure_history_indexes(&conn).unwrap();
        assert_eq!(indexes(&conn), after_once, "the rung is not idempotent");
    }

    /// And the planner actually USES one — an index nothing plans over is an index
    /// that was added on a hunch. `EXPLAIN QUERY PLAN` is the only witness that can
    /// tell a created index from a used one.
    #[test]
    fn the_transcript_lookup_stops_scanning_the_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../../../docs/data/schema.sql"))
            .unwrap();
        let plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN SELECT id FROM transcripts WHERE service_id = 1",
                [],
                |r| r.get(3),
            )
            .unwrap();
        assert!(
            plan.contains("idx_transcripts_service"),
            "the planner still scans transcripts: {plan}"
        );
    }
}
