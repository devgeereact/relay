//! The content library: saved scripture, announcements, and media assets.
//!
//! Announcements snapshot into a cue for offline reliability, but edits still
//! propagate (`sync_*_in_plans`) so a Library edit is never stale in a plan.

use super::verses::VerseRow;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::Serialize;
use serde_json::Value;

/// A verse the operator saved to the library.
#[derive(Debug, Clone, Serialize)]
pub struct SavedScripture {
    pub id: i64,
    pub reference: String,
    pub book: String,
    pub chapter: i64,
    pub verse: i64,
    pub text: String,
    pub translation: String,
}

/// Create the saved-scripture table if missing. Idempotent.
pub fn ensure_saved_scripture(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS saved_scripture (
            id          INTEGER PRIMARY KEY,
            reference   TEXT NOT NULL UNIQUE,
            book        TEXT NOT NULL,
            chapter     INTEGER NOT NULL,
            verse       INTEGER NOT NULL,
            text        TEXT NOT NULL,
            translation TEXT NOT NULL DEFAULT '',
            created_at  TEXT NOT NULL DEFAULT ''
         );",
    )
}

/// Saved verses, newest first.
pub fn list_saved_scripture(conn: &Connection) -> rusqlite::Result<Vec<SavedScripture>> {
    let mut stmt = conn.prepare(
        "SELECT id, reference, book, chapter, verse, text, translation
           FROM saved_scripture ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(SavedScripture {
            id: r.get(0)?,
            reference: r.get(1)?,
            book: r.get(2)?,
            chapter: r.get(3)?,
            verse: r.get(4)?,
            text: r.get(5)?,
            translation: r.get(6)?,
        })
    })?;
    rows.collect()
}

/// Save a verse (dedupe by reference). Returns the row id.
pub fn save_scripture(conn: &Connection, v: &VerseRow, date: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO saved_scripture (reference, book, chapter, verse, text, translation, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(reference) DO UPDATE SET text = excluded.text, translation = excluded.translation",
        (
            &v.reference,
            &v.book,
            v.chapter,
            v.verse,
            &v.text,
            &v.translation,
            date,
        ),
    )?;
    conn.query_row(
        "SELECT id FROM saved_scripture WHERE reference = ?1",
        [&v.reference],
        |r| r.get(0),
    )
}

/// Remove a saved verse.
pub fn delete_saved_scripture(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM saved_scripture WHERE id = ?1", [id])?;
    Ok(())
}

/// A saved announcement / notice.
#[derive(Debug, Clone, Serialize)]
pub struct Announcement {
    pub id: i64,
    pub title: String,
    pub body: String,
}

/// Create the announcements table if missing. Idempotent.
pub fn ensure_announcements(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS announcements (
            id         INTEGER PRIMARY KEY,
            title      TEXT NOT NULL DEFAULT '',
            body       TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT ''
         );",
    )
}

/// Announcements, newest first.
pub fn list_announcements(conn: &Connection) -> rusqlite::Result<Vec<Announcement>> {
    let mut stmt = conn.prepare("SELECT id, title, body FROM announcements ORDER BY id DESC")?;
    let rows = stmt.query_map([], |r| {
        Ok(Announcement {
            id: r.get(0)?,
            title: r.get(1)?,
            body: r.get(2)?,
        })
    })?;
    rows.collect()
}

/// Create (id None) or update an announcement. Returns its id.
pub fn save_announcement(
    conn: &Connection,
    id: Option<i64>,
    title: &str,
    body: &str,
    date: &str,
) -> rusqlite::Result<i64> {
    match id {
        Some(aid) => {
            conn.execute(
                "UPDATE announcements SET title = ?1, body = ?2 WHERE id = ?3",
                (title, body, aid),
            )?;
            Ok(aid)
        }
        None => {
            conn.execute(
                "INSERT INTO announcements (title, body, created_at) VALUES (?1, ?2, ?3)",
                (title, body, date),
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

/// Remove an announcement.
pub fn delete_announcement(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM announcements WHERE id = ?1", [id])?;
    Ok(())
}

/// Propagate an announcement edit to every plan that cues it — rewrite each
/// matching announce cue's snapshot (title + body) so a Library edit shows up in
/// the Planner. Mirrors `sync_song_in_plans`. Returns how many cues updated.
pub fn sync_announcement_in_plans(
    conn: &Connection,
    announce_id: i64,
    title: &str,
    body: &str,
) -> rusqlite::Result<usize> {
    let mut stmt =
        conn.prepare("SELECT id, payload_json FROM plan_items WHERE cue_type = 'announce'")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    for (item_id, payload) in rows {
        let mut v: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
        if v.get("announce_id").and_then(Value::as_i64) == Some(announce_id) {
            v["title"] = Value::String(title.to_string());
            v["body"] = Value::String(body.to_string());
            let label = if title.is_empty() {
                "Announcement"
            } else {
                title
            };
            tx.execute(
                "UPDATE plan_items SET label = ?1, payload_json = ?2 WHERE id = ?3",
                (label, v.to_string(), item_id),
            )?;
            n += 1;
        }
    }
    tx.commit()?;
    Ok(n)
}

/// A media/document asset pointer.
#[derive(Debug, Clone, Serialize)]
pub struct MediaAsset {
    pub id: i64,
    pub kind: String, // image | video | document
    pub filename: String,
    pub path: String,
    pub created_at: String,
}

/// Create the media table if missing. Idempotent.
pub fn ensure_media(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS media_assets (
            id         INTEGER PRIMARY KEY,
            kind       TEXT NOT NULL,
            filename   TEXT NOT NULL,
            path       TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT ''
         );",
    )
}

/// All media assets, newest first.
pub fn list_media(conn: &Connection) -> rusqlite::Result<Vec<MediaAsset>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, filename, path, created_at FROM media_assets ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(MediaAsset {
            id: r.get(0)?,
            kind: r.get(1)?,
            filename: r.get(2)?,
            path: r.get(3)?,
            created_at: r.get(4)?,
        })
    })?;
    rows.collect()
}

/// Insert a media row (the file is written by the command layer). Returns id.
pub fn insert_media(
    conn: &Connection,
    kind: &str,
    filename: &str,
    date: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO media_assets (kind, filename, created_at) VALUES (?1, ?2, ?3)",
        (kind, filename, date),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Record the on-disk path once the file is written.
pub fn set_media_path(conn: &Connection, id: i64, path: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE media_assets SET path = ?1 WHERE id = ?2",
        (path, id),
    )?;
    Ok(())
}

/// Remove a media row and return its path (so the command can delete the file).
/// Delete a media asset — and every plan cue that pointed at it.
///
/// It used to delete only the asset row and the file, leaving `plan_items` cues
/// with the same `media_id` behind. Those cues then sat in a service plan looking
/// perfectly fine, and failed with "media not found" **at the moment the operator
/// fired them, live**. A broken cue that looks healthy until you press it is worse
/// than one that is visibly gone.
///
/// Songs and announcements already propagate their edits into plans
/// (`sync_*_in_plans`); media was the one that didn't.
pub fn delete_media(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    let tx = conn.unchecked_transaction()?;
    let path: Option<String> = tx
        .query_row("SELECT path FROM media_assets WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .optional()?;

    // Drop any cue that referenced this asset. json_extract is available in the
    // bundled SQLite (JSON1 is compiled in).
    tx.execute(
        "DELETE FROM plan_items
          WHERE cue_type = 'media'
            AND CAST(json_extract(payload_json, '$.media_id') AS INTEGER) = ?1",
        [id],
    )?;
    tx.execute("DELETE FROM media_assets WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(path)
}
