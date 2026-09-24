//! App settings: the key/value store, and the per-content-type default templates
//! (scripture looks like scripture, lyrics look like lyrics — without any
//! per-channel branching in the renderer).

use rusqlite::Connection;
use rusqlite::OptionalExtension;

/// Create the key/value app_settings table if missing. Idempotent.
pub fn ensure_app_settings(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )
}

/// Read a setting value.
pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [key],
        |r| r.get(0),
    )
    .optional()
}

/// Write a setting value (upsert).
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, value),
    )?;
    Ok(())
}

/// The id mapped to a content type (`scripture` | `song` | `media` | `announce`).
pub fn content_template_id(conn: &Connection, kind: &str) -> rusqlite::Result<Option<i64>> {
    Ok(get_setting(conn, &format!("tpl_{kind}"))?.and_then(|s| s.parse().ok()))
}

/// Set (Some) or clear (None) the template for a content type.
pub fn set_content_template(
    conn: &Connection,
    kind: &str,
    id: Option<i64>,
) -> rusqlite::Result<()> {
    let key = format!("tpl_{kind}");
    match id {
        Some(v) => set_setting(conn, &key, &v.to_string()),
        None => {
            conn.execute("DELETE FROM app_settings WHERE key = ?1", [&key])?;
            Ok(())
        }
    }
}

/// The app-settings key holding whether Relay may follow a preacher who is
/// READING a verse (DECISIONS §118). Absent means ON — the operator's default,
/// so a church that has never opened the switch gets the behaviour they asked
/// for, and an upgrade does not silently turn it off.
pub const FOLLOW_THE_READER_KEY: &str = "detection.follow_the_reader";

/// May Relay put a verse it heard being read on a wall without being asked?
///
/// Anything other than a stored `"0"` is ON, deliberately: the absence is ON and
/// so is a value nobody recognises. The failure mode of a misread row is then the
/// operator's own default rather than a switch that turned itself off overnight,
/// and the only way to be off is to have explicitly asked to be.
pub fn follow_the_reader(conn: &Connection) -> bool {
    !matches!(
        get_setting(conn, FOLLOW_THE_READER_KEY)
            .ok()
            .flatten()
            .as_deref(),
        Some("0")
    )
}

/// Write the switch. `"1"` / `"0"` rather than a bare boolean, because
/// `app_settings.value` is `TEXT NOT NULL` and every other flag in it is stored
/// this way.
pub fn set_follow_the_reader(conn: &Connection, on: bool) -> rusqlite::Result<()> {
    set_setting(conn, FOLLOW_THE_READER_KEY, if on { "1" } else { "0" })
}

#[cfg(test)]
mod follow_the_reader_tests {
    use super::*;

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        ensure_app_settings(&c).unwrap();
        c
    }

    /// A FRESH INSTALL FOLLOWS THE READER. The operator asked for it on by
    /// default, and a church that never opens Settings must get what they asked
    /// for rather than what is easiest to store.
    #[test]
    fn a_church_that_has_never_touched_it_is_on() {
        assert!(follow_the_reader(&db()));
    }

    #[test]
    fn it_round_trips_and_only_an_explicit_no_is_off() {
        let c = db();
        set_follow_the_reader(&c, false).unwrap();
        assert!(!follow_the_reader(&c));
        set_follow_the_reader(&c, true).unwrap();
        assert!(follow_the_reader(&c));
        // A row nobody recognises reads as ON, not OFF — an unreadable setting
        // must not quietly disable a feature the operator asked for.
        set_setting(&c, FOLLOW_THE_READER_KEY, "banana").unwrap();
        assert!(follow_the_reader(&c));
    }
}
