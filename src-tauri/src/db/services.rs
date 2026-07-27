//! Service history: transcripts, detections, and operator cues, per service.
//!
//! `detections.status` records who decided — 'auto' (the AI) vs 'manual' (a
//! human). The self-calibrating router learns from that column, so the
//! distinction is load-bearing, not archival colour.

use rusqlite::Connection;
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
        "SELECT v.book, v.chapter, v.verse, d.method, d.confidence, d.status, d.fired_at
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
