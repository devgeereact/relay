//! Environment profiles — a room, remembered.
//!
//! Single responsibility: store the configuration one room needs, so a church that
//! runs in the main hall on Sunday and the youth room on Wednesday stops rebuilding
//! it twice a week.
//!
//! ## What is remembered, and what is deliberately NOT
//!
//! Remembered: the microphone, the recognition language, the planned service
//! length, the active voice profile, and which physical display each screen goes
//! to. Every one of those is an operator's *choice*, it survives being wrong (they
//! change it back), and every one of them currently has to be re-made from scratch.
//! The microphone in particular is not persisted anywhere at all today — it lives
//! in memory and is gone the moment Relay closes.
//!
//! **Not remembered as something to APPLY: the audio thresholds.**
//!
//! It is tempting, and it is the single most dangerous thing this table could do.
//! DECISIONS §19 and CLAUDE.md rule 12 exist because three individually-reasonable
//! thresholds together made Relay **deaf to a quiet preacher, silently** — 94%
//! voiced at studio level, 2% at a church-laptop level — and the rule that came out
//! of it is absolute: *nothing may compare a signal to a stored level.* A noise
//! floor captured in this hall three weeks ago, applied to this hall today with the
//! heating on and forty more people in it, is exactly the assumption that rule
//! forbids.
//!
//! So the observed levels are stored as a **note the operator can read** — "this
//! room ran at about this" — and nothing reads them back into the gate. Seeding the
//! learner from them is a real idea and might well be right; it is not being done on
//! a hunch, because the instrument that could show it was safe
//! (`cargo test audio::gate -- --ignored`, against real room audio) has never been
//! run against a real room. See `docs/qa/RELAY_GAP.md` §23, RG-10.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub id: i64,
    pub name: String,
    pub is_active: bool,
    /// The remembered settings, as JSON. A blob rather than columns on purpose:
    /// what is worth remembering about a room will keep changing, and a migration
    /// per field is a migration per field.
    pub settings_json: String,
    /// What Relay OBSERVED in this room, in words, for a person to read. Never read
    /// back into the audio gate — see the module comment.
    pub notes: String,
    pub updated_at: String,
}

/// RETRYABLE and additive — `CREATE TABLE IF NOT EXISTS`, no rebuild, nothing
/// dropped (rule 25).
pub fn ensure_environment_profiles(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS environment_profiles (
            id            INTEGER PRIMARY KEY,
            name          TEXT NOT NULL,
            is_active     INTEGER NOT NULL DEFAULT 0,
            settings_json TEXT NOT NULL DEFAULT '{}',
            notes         TEXT NOT NULL DEFAULT '',
            updated_at    TEXT NOT NULL DEFAULT ''
         );",
    )
}

pub fn list_environments(conn: &Connection) -> rusqlite::Result<Vec<Environment>> {
    let mut st = conn.prepare(
        "SELECT id, name, is_active, settings_json, notes, updated_at
           FROM environment_profiles ORDER BY name COLLATE NOCASE",
    )?;
    let rows = st.query_map([], |r| {
        Ok(Environment {
            id: r.get(0)?,
            name: r.get(1)?,
            is_active: r.get::<_, i64>(2)? != 0,
            settings_json: r.get(3)?,
            notes: r.get(4)?,
            updated_at: r.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn active_environment(conn: &Connection) -> rusqlite::Result<Option<Environment>> {
    Ok(list_environments(conn)?.into_iter().find(|e| e.is_active))
}

/// Create a room, or overwrite the one with this name.
///
/// Named rather than numbered from the operator's side: "Main hall" is what they
/// will look for, and two rooms called "Main hall" is a mistake nobody meant to
/// make. Saving over one they already have is the intended behaviour — it is how
/// "I moved the microphone, remember that" works.
pub fn save_environment(
    conn: &Connection,
    name: &str,
    settings_json: &str,
    notes: &str,
    now: &str,
) -> rusqlite::Result<i64> {
    let name = name.trim();
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM environment_profiles WHERE name = ?1 COLLATE NOCASE",
            [name],
            |r| r.get(0),
        )
        .ok();
    match existing {
        Some(id) => {
            conn.execute(
                "UPDATE environment_profiles
                    SET settings_json = ?2, notes = ?3, updated_at = ?4
                  WHERE id = ?1",
                rusqlite::params![id, settings_json, notes, now],
            )?;
            Ok(id)
        }
        None => {
            conn.execute(
                "INSERT INTO environment_profiles (name, settings_json, notes, updated_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![name, settings_json, notes, now],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

/// Mark one room as the one in use. Exactly one, or none.
pub fn set_active_environment(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE environment_profiles SET is_active = 0", [])?;
    conn.execute(
        "UPDATE environment_profiles SET is_active = 1 WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn delete_environment(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM environment_profiles WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_environment_profiles(&conn).unwrap();
        conn
    }

    #[test]
    fn ensure_is_retryable() {
        let conn = db();
        ensure_environment_profiles(&conn).unwrap();
        ensure_environment_profiles(&conn).unwrap();
        assert!(list_environments(&conn).unwrap().is_empty());
    }

    #[test]
    fn a_room_is_saved_and_read_back() {
        let conn = db();
        let id = save_environment(
            &conn,
            "Main hall",
            r#"{"language":"yo"}"#,
            "quiet",
            "2026-08-29",
        )
        .unwrap();
        let all = list_environments(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
        assert_eq!(all[0].name, "Main hall");
        assert!(!all[0].is_active, "saving a room does not switch to it");
    }

    /// SAVING THE SAME ROOM AGAIN UPDATES IT — it does not make a second one.
    ///
    /// "I moved the microphone, remember that" is the normal case, and a list with
    /// three rooms called "Main hall" is worse than no list.
    #[test]
    fn saving_a_room_twice_updates_it() {
        let conn = db();
        save_environment(&conn, "Main hall", r#"{"a":1}"#, "", "2026-08-29").unwrap();
        save_environment(&conn, "  main HALL ", r#"{"a":2}"#, "louder", "2026-09-05").unwrap();
        let all = list_environments(&conn).unwrap();
        assert_eq!(all.len(), 1, "matched case-insensitively and trimmed");
        assert_eq!(all[0].settings_json, r#"{"a":2}"#);
        assert_eq!(all[0].notes, "louder");
    }

    #[test]
    fn exactly_one_room_is_active_at_a_time() {
        let conn = db();
        let a = save_environment(&conn, "Main hall", "{}", "", "").unwrap();
        let b = save_environment(&conn, "Youth room", "{}", "", "").unwrap();
        set_active_environment(&conn, a).unwrap();
        assert_eq!(active_environment(&conn).unwrap().unwrap().id, a);
        set_active_environment(&conn, b).unwrap();
        assert_eq!(active_environment(&conn).unwrap().unwrap().id, b);
        assert_eq!(
            list_environments(&conn)
                .unwrap()
                .iter()
                .filter(|e| e.is_active)
                .count(),
            1
        );
    }

    #[test]
    fn deleting_a_room_leaves_the_others() {
        let conn = db();
        let a = save_environment(&conn, "Main hall", "{}", "", "").unwrap();
        save_environment(&conn, "Youth room", "{}", "", "").unwrap();
        delete_environment(&conn, a).unwrap();
        let all = list_environments(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Youth room");
    }

    /// THE AUDIO THRESHOLDS ARE NOT IN HERE, AND THAT IS THE POINT.
    ///
    /// DECISIONS §19 / rule 12: nothing may compare a signal to a stored level. A
    /// noise floor captured three weeks ago, applied to the same hall today with the
    /// heating on and forty more people in it, is exactly that. Observed levels are
    /// a NOTE for a person; this test fails if a column ever appears that could be
    /// read back into the gate.
    #[test]
    fn no_column_here_can_become_an_audio_threshold() {
        let conn = db();
        let cols: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('environment_profiles')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for banned in [
            "noise_floor",
            "threshold",
            "gain",
            "vad",
            "speech_level",
            "rms",
        ] {
            assert!(
                !cols.iter().any(|c| c.contains(banned)),
                "`{banned}` must not be a column here — see the module comment and DECISIONS §19"
            );
        }
    }
}
