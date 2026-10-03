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

/// The app-settings key holding whether a paraphrase must echo a run of the
/// verse's own words before Relay offers it (`detection::PARAPHRASE_RUN_WORDS`).
///
/// **Absent means OFF, and that is the opposite of `FOLLOW_THE_READER_KEY` on
/// purpose.** This bar removes roughly three quarters of what the paraphrase path
/// offers and, with it, five of the 43 labelled retellings — so a church that has
/// never opened Settings must get the behaviour it has been running all along, not
/// a tighter one it never asked for. An upgrade may not silently take a feature
/// away.
pub const PARAPHRASE_RUN_KEY: &str = "detection.paraphrase_needs_a_run";

/// Must a paraphrase echo the verse before Relay offers it?
///
/// Only a stored `"1"` is ON, and that is deliberate in the same way
/// `follow_the_reader`'s rule is deliberate and for the same reason read in the
/// other direction: the absence and a value nobody recognises both read as the
/// SHIPPED behaviour, so the failure mode of a misread row is what the church was
/// already running rather than a rule that starts silencing retellings overnight.
/// The only way to be on is to have explicitly asked to be.
pub fn paraphrase_needs_a_run(conn: &Connection) -> bool {
    matches!(
        get_setting(conn, PARAPHRASE_RUN_KEY)
            .ok()
            .flatten()
            .as_deref(),
        Some("1")
    )
}

/// Write the switch. `"1"` / `"0"`, as every other flag in `app_settings` is
/// stored — `value` is `TEXT NOT NULL`.
pub fn set_paraphrase_needs_a_run(conn: &Connection, on: bool) -> rusqlite::Result<()> {
    set_setting(conn, PARAPHRASE_RUN_KEY, if on { "1" } else { "0" })
}

#[cfg(test)]
mod paraphrase_run_tests {
    use super::*;

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        ensure_app_settings(&c).unwrap();
        c
    }

    /// **A FRESH INSTALL IS OFF, AND THIS IS HALF THE PROOF THAT OFF IS A NO-OP.**
    /// A church that never opens Settings runs the recall it ran yesterday — 77%
    /// ALL, 41% MODERN — because nothing has asked for the bar.
    #[test]
    fn a_church_that_has_never_touched_it_is_off() {
        assert!(!paraphrase_needs_a_run(&db()));
    }

    #[test]
    fn it_round_trips_and_only_an_explicit_yes_is_on() {
        let c = db();
        set_paraphrase_needs_a_run(&c, true).unwrap();
        assert!(paraphrase_needs_a_run(&c));
        set_paraphrase_needs_a_run(&c, false).unwrap();
        assert!(!paraphrase_needs_a_run(&c));
        // A row nobody recognises reads as OFF — an unreadable setting must not
        // quietly start removing suggestions nobody asked it to remove. This is the
        // mirror of `follow_the_reader`'s rule, inverted because the default is.
        set_setting(&c, PARAPHRASE_RUN_KEY, "banana").unwrap();
        assert!(!paraphrase_needs_a_run(&c));
    }

    /// The two switches are independent rows. They govern opposite ends of the
    /// detector — one what a QUOTATION may do unattended, the other what a
    /// PARAPHRASE may be offered for — and a shared key would silently couple them.
    #[test]
    fn it_does_not_share_a_row_with_follow_the_reader() {
        assert_ne!(PARAPHRASE_RUN_KEY, FOLLOW_THE_READER_KEY);
        let c = db();
        set_paraphrase_needs_a_run(&c, true).unwrap();
        assert!(follow_the_reader(&c), "the reader switch must be untouched");
        set_follow_the_reader(&c, false).unwrap();
        assert!(
            paraphrase_needs_a_run(&c),
            "the paraphrase bar must be untouched"
        );
    }
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
