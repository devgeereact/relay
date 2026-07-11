//! Voice profiles: per-preacher accent + gate calibration.
//!
//! `auto_fire`/`suggest` here are what the self-calibrating router has LEARNED;
//! `sensitivity` is the operator's dial, the baseline that learning decays back
//! toward. They are different things — conflating them once silently wiped an
//! operator's calibration on every profile save (docs/DECISIONS.md).

use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

/// A per-preacher calibration profile. Bundles the STT language hint, decoder-
/// bias vocabulary, the sensitivity dial, and the live feedback-adapted
/// thresholds so accent + threshold learning persists per speaker across
/// services and restarts. See docs/SPEC.md §4.6 and router.rs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceProfile {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    /// None = auto-detect / code-switch; else an ISO code ("en"/"yo"/"sw"/"ha").
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default = "default_sensitivity")]
    pub sensitivity: i64,
    #[serde(default = "default_auto_fire")]
    pub auto_fire: f64,
    #[serde(default = "default_suggest")]
    pub suggest: f64,
    #[serde(default)]
    pub bias_terms: String,
    #[serde(default)]
    pub is_active: bool,
}

fn default_sensitivity() -> i64 {
    50
}

fn default_auto_fire() -> f64 {
    0.50
}

fn default_suggest() -> f64 {
    0.35
}

const PROFILE_COLS: &str =
    "id, name, language, sensitivity, auto_fire, suggest, bias_terms, is_active";

fn row_to_profile(r: &rusqlite::Row) -> rusqlite::Result<VoiceProfile> {
    Ok(VoiceProfile {
        id: r.get(0)?,
        name: r.get(1)?,
        language: r.get(2)?,
        sensitivity: r.get(3)?,
        auto_fire: r.get(4)?,
        suggest: r.get(5)?,
        bias_terms: r.get(6)?,
        is_active: r.get::<_, i64>(7)? != 0,
    })
}

/// Create the `voice_profiles` table if missing and guarantee exactly one active
/// profile (seeding a "Default" if the table is empty). Idempotent — safe to run
/// on every open, including DBs created before Phase B existed.
pub fn ensure_voice_profiles(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS voice_profiles (
            id          INTEGER PRIMARY KEY,
            name        TEXT NOT NULL,
            language    TEXT,
            sensitivity INTEGER NOT NULL DEFAULT 50,
            auto_fire   REAL NOT NULL DEFAULT 0.50,
            suggest     REAL NOT NULL DEFAULT 0.35,
            bias_terms  TEXT NOT NULL DEFAULT '',
            is_active   INTEGER NOT NULL DEFAULT 0
        );",
    )?;
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM voice_profiles", [], |r| r.get(0))?;
    if n == 0 {
        conn.execute(
            "INSERT INTO voice_profiles (name, language, auto_fire, suggest, is_active)
             VALUES ('Default', NULL, 0.50, 0.35, 1)",
            [],
        )?;
    } else {
        ensure_one_active(conn)?;
        // Migrate profiles still at the legacy conservative seed (0.90/0.60) to
        // the new "push above ~50%" default. Only touches untouched seeds — a
        // profile the operator tuned won't match these exact values.
        conn.execute(
            "UPDATE voice_profiles SET auto_fire = 0.50, suggest = 0.35
               WHERE auto_fire = 0.90 AND suggest = 0.60",
            [],
        )?;
    }
    Ok(())
}

/// Guarantee a single active profile: if none is active, promote the lowest id.
fn ensure_one_active(conn: &Connection) -> rusqlite::Result<()> {
    let active: i64 = conn.query_row(
        "SELECT COUNT(*) FROM voice_profiles WHERE is_active = 1",
        [],
        |r| r.get(0),
    )?;
    if active == 0 {
        conn.execute(
            "UPDATE voice_profiles SET is_active = 1
               WHERE id = (SELECT MIN(id) FROM voice_profiles)",
            [],
        )?;
    }
    Ok(())
}

/// All voice profiles, ordered by id.
pub fn list_voice_profiles(conn: &Connection) -> rusqlite::Result<Vec<VoiceProfile>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PROFILE_COLS} FROM voice_profiles ORDER BY id"
    ))?;
    let rows = stmt.query_map([], row_to_profile)?;
    rows.collect()
}

/// The currently active profile, if any.
pub fn active_voice_profile(conn: &Connection) -> rusqlite::Result<Option<VoiceProfile>> {
    conn.query_row(
        &format!("SELECT {PROFILE_COLS} FROM voice_profiles WHERE is_active = 1 LIMIT 1"),
        [],
        row_to_profile,
    )
    .optional()
}

/// Create a new profile (with default calibration) and return its id.
pub fn create_voice_profile(
    conn: &Connection,
    name: &str,
    language: Option<&str>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO voice_profiles (name, language, auto_fire, suggest) VALUES (?1, ?2, 0.50, 0.35)",
        (name, language),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Persist editable fields of a profile (name, language, sensitivity, bias
/// terms). Thresholds are saved separately via `save_profile_thresholds` because
/// they are machine-adapted, not user-edited.
pub fn update_voice_profile(conn: &Connection, p: &VoiceProfile) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE voice_profiles
            SET name = ?1, language = ?2, sensitivity = ?3, bias_terms = ?4
          WHERE id = ?5",
        (&p.name, &p.language, p.sensitivity, &p.bias_terms, p.id),
    )?;
    Ok(())
}

/// Persist the live, feedback-adapted thresholds for a profile (the
/// self-calibrating loop — router.rs `record_feedback`).
pub fn save_profile_thresholds(
    conn: &Connection,
    id: i64,
    auto_fire: f64,
    suggest: f64,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE voice_profiles SET auto_fire = ?1, suggest = ?2 WHERE id = ?3",
        (auto_fire, suggest, id),
    )?;
    Ok(())
}

/// Make `id` the sole active profile.
pub fn set_active_profile(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE voice_profiles SET is_active = 0", [])?;
    conn.execute(
        "UPDATE voice_profiles SET is_active = 1 WHERE id = ?1",
        [id],
    )?;
    ensure_one_active(conn)?;
    Ok(())
}

/// Delete a profile, then guarantee a profile still exists and one is active
/// (re-seeds a Default if the last one was removed).
pub fn delete_voice_profile(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM voice_profiles WHERE id = ?1", [id])?;
    ensure_voice_profiles(conn)?;
    Ok(())
}
