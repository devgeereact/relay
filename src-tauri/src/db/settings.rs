//! App settings: the key/value store, and the per-content-type default templates
//! (scripture looks like scripture, lyrics look like lyrics — without any
//! per-channel branching in the renderer).

use super::templates::get_template;
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

/// Resolve a content type's template to (id, serialized-JSON) for the broadcast
/// override. None when unmapped or the mapped template was deleted.
pub fn content_template(conn: &Connection, kind: &str) -> rusqlite::Result<Option<(i64, String)>> {
    if let Some(id) = content_template_id(conn, kind)? {
        if let Some(t) = get_template(conn, id)? {
            if let Ok(j) = serde_json::to_string(&t) {
                return Ok(Some((id, j)));
            }
        }
    }
    Ok(None)
}
