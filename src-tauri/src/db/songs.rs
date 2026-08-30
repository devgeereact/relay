//! Songs, sections, and arrangements.
//!
//! Arrangements are named play-orders stored as section-index sequences, not
//! copied lyrics — so editing a lyric propagates into every plan that uses the
//! song, and re-expands into the right (possibly repeated) slots.

use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::Serialize;
use serde_json::Value;

/// A row for the Lyrics list.
#[derive(Debug, Clone, Serialize)]
pub struct SongSummary {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub song_key: String,
    pub bpm: Option<i64>,
    pub section_count: i64,
}

/// One stored section of a song.
#[derive(Debug, Clone, Serialize)]
pub struct SongSection {
    pub id: i64,
    pub position: i64,
    pub tag: String,
    pub label: String,
    pub lyrics: String,
}

/// A full song with its ordered sections (Planner detail / add-as-cue).
#[derive(Debug, Clone, Serialize)]
pub struct Song {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub ccli: String,
    pub song_key: String,
    pub bpm: Option<i64>,
    pub sections: Vec<SongSection>,
}

/// Create the song tables if missing. Idempotent; forward-fills old DBs.
pub fn ensure_songs(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS songs (
            id         INTEGER PRIMARY KEY,
            title      TEXT NOT NULL,
            author     TEXT NOT NULL DEFAULT '',
            ccli       TEXT NOT NULL DEFAULT '',
            song_key   TEXT NOT NULL DEFAULT '',
            bpm        INTEGER,
            tags       TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT ''
         );
         CREATE TABLE IF NOT EXISTS song_sections (
            id       INTEGER PRIMARY KEY,
            song_id  INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            tag      TEXT NOT NULL,
            label    TEXT NOT NULL,
            lyrics   TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_song_sections ON song_sections(song_id, position);
         CREATE TABLE IF NOT EXISTS song_arrangements (
            id       INTEGER PRIMARY KEY,
            song_id  INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
            name     TEXT NOT NULL,
            sequence TEXT NOT NULL DEFAULT '[]'
         );
         CREATE INDEX IF NOT EXISTS idx_song_arrangements ON song_arrangements(song_id);",
    )?;
    // `built_shape` arrived after the table did. Sniffed rather than a bare
    // `ALTER TABLE ADD COLUMN`, which errors with "duplicate column name" on the
    // second boot and panics the app before the window is shown (rule 25).
    let has_shape: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('song_arrangements') WHERE name = 'built_shape'",
        [],
        |r| r.get(0),
    )?;
    if has_shape == 0 {
        conn.execute_batch(
            "ALTER TABLE song_arrangements ADD COLUMN built_shape TEXT NOT NULL DEFAULT '';",
        )?;
    }
    Ok(())
}

/// The STRUCTURAL fingerprint of a section list — its tags and labels, in order.
///
/// **The lyrics are deliberately excluded.** Editing the words of a verse must
/// not invalidate an arrangement: storing indices rather than copied lyrics is
/// the whole reason the schema is shaped this way, and a lyric edit really does
/// re-expand into the right slots. What this catches is the other case —
/// reordering, inserting, deleting or renaming a section — where index 3 quietly
/// stops meaning what it meant when somebody chose it.
pub fn shape_of_parsed(sections: &[crate::songs::ParsedSection]) -> String {
    shape_of(sections.iter().map(|s| (s.tag.as_str(), s.label.as_str())))
}

fn shape_of<'a>(sections: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let pairs: Vec<[&str; 2]> = sections.into_iter().map(|(t, l)| [t, l]).collect();
    serde_json::to_string(&pairs).unwrap_or_default()
}

/// The shape a song has in the database right now.
pub fn current_shape(conn: &Connection, song_id: i64) -> rusqlite::Result<String> {
    let mut stmt =
        conn.prepare("SELECT tag, label FROM song_sections WHERE song_id = ?1 ORDER BY position")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([song_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(shape_of(rows.iter().map(|(t, l)| (t.as_str(), l.as_str()))))
}

/// A named play-order for a song (ProPresenter arrangements). `sequence` is the
/// ordered list of section positions to play — repeats allowed (V1 C1 V2 C1).
#[derive(Debug, Clone, Serialize)]
pub struct Arrangement {
    pub id: i64,
    pub name: String,
    pub sequence: Vec<i64>,
    /// The song's structure when this arrangement was built — see `shape_of_parsed`.
    pub built_shape: String,
    /// The song's sections have been reordered, added to, removed or renamed
    /// since. The stored indices therefore point at different sections than the
    /// person who built it chose, and Relay will not guess which.
    ///
    /// An arrangement written before this column existed carries an empty
    /// `built_shape` and is reported as NOT stale: nothing recorded what it was
    /// built against, and claiming staleness from an absence is the same lie as
    /// claiming freshness from one.
    pub stale: bool,
}

/// All arrangements for a song.
pub fn list_arrangements(conn: &Connection, song_id: i64) -> rusqlite::Result<Vec<Arrangement>> {
    let now = current_shape(conn, song_id)?;
    let mut stmt = conn.prepare(
        "SELECT id, name, sequence, built_shape FROM song_arrangements
         WHERE song_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map([song_id], |r| {
        let seq: String = r.get(2)?;
        let built_shape: String = r.get(3)?;
        Ok(Arrangement {
            id: r.get(0)?,
            name: r.get(1)?,
            sequence: serde_json::from_str(&seq).unwrap_or_default(),
            stale: !built_shape.is_empty() && built_shape != now,
            built_shape,
        })
    })?;
    rows.collect()
}

/// Create (id None) or update an arrangement. Returns its id.
pub fn save_arrangement(
    conn: &Connection,
    song_id: i64,
    id: Option<i64>,
    name: &str,
    sequence: &[i64],
) -> rusqlite::Result<i64> {
    let seq = serde_json::to_string(sequence).unwrap_or_else(|_| "[]".into());
    // Saving is also how a stale arrangement is repaired: it is re-recorded
    // against the song as it is NOW, which is the only moment a person has
    // actually looked at both.
    let shape = current_shape(conn, song_id)?;
    match id {
        Some(aid) => {
            conn.execute(
                "UPDATE song_arrangements SET name = ?1, sequence = ?2, built_shape = ?3
                 WHERE id = ?4 AND song_id = ?5",
                (name, &seq, &shape, aid, song_id),
            )?;
            Ok(aid)
        }
        None => {
            conn.execute(
                "INSERT INTO song_arrangements (song_id, name, sequence, built_shape)
                 VALUES (?1, ?2, ?3, ?4)",
                (song_id, name, &seq, &shape),
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

/// Delete an arrangement.
pub fn delete_arrangement(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM song_arrangements WHERE id = ?1", [id])?;
    Ok(())
}

/// All songs, alphabetical, with a live section count.
pub fn list_songs(conn: &Connection) -> rusqlite::Result<Vec<SongSummary>> {
    query_song_summaries(conn, "1 = 1", "")
}

/// Search songs by title or author.
pub fn search_songs(conn: &Connection, needle: &str) -> rusqlite::Result<Vec<SongSummary>> {
    let pat = format!("%{}%", needle.replace(['%', '_'], ""));
    query_song_summaries(conn, "s.title LIKE ?1 OR s.author LIKE ?1", &pat)
}

/// Shared summary query (empty `pat` = no bind param used, `where_sql` = "1=1").
fn query_song_summaries(
    conn: &Connection,
    where_sql: &str,
    pat: &str,
) -> rusqlite::Result<Vec<SongSummary>> {
    let sql = format!(
        "SELECT s.id, s.title, s.author, s.song_key, s.bpm,
                (SELECT COUNT(*) FROM song_sections x WHERE x.song_id = s.id)
           FROM songs s WHERE {where_sql} ORDER BY s.title COLLATE NOCASE"
    );
    let mut stmt = conn.prepare(&sql)?;
    let map = |r: &rusqlite::Row| {
        Ok(SongSummary {
            id: r.get(0)?,
            title: r.get(1)?,
            author: r.get(2)?,
            song_key: r.get(3)?,
            bpm: r.get(4)?,
            section_count: r.get(5)?,
        })
    };
    let rows = if pat.is_empty() {
        stmt.query_map([], map)?.collect()
    } else {
        stmt.query_map([pat], map)?.collect()
    };
    rows
}

/// A full song with sections, or None.
pub fn get_song(conn: &Connection, id: i64) -> rusqlite::Result<Option<Song>> {
    let base = conn
        .query_row(
            "SELECT id, title, author, ccli, song_key, bpm FROM songs WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                ))
            },
        )
        .optional()?;
    let Some((id, title, author, ccli, song_key, bpm)) = base else {
        return Ok(None);
    };
    let mut stmt = conn.prepare(
        "SELECT id, position, tag, label, lyrics FROM song_sections
           WHERE song_id = ?1 ORDER BY position, id",
    )?;
    let sections = stmt
        .query_map([id], |r| {
            Ok(SongSection {
                id: r.get(0)?,
                position: r.get(1)?,
                tag: r.get(2)?,
                label: r.get(3)?,
                lyrics: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Some(Song {
        id,
        title,
        author,
        ccli,
        song_key,
        bpm,
        sections,
    }))
}

/// Import a song and its parsed sections in one transaction. Returns the id.
#[allow(clippy::too_many_arguments)]
pub fn import_song(
    conn: &Connection,
    title: &str,
    author: &str,
    ccli: &str,
    song_key: &str,
    bpm: Option<i64>,
    date: &str,
    sections: &[crate::songs::ParsedSection],
) -> rusqlite::Result<i64> {
    // ATOMIC: a song and its sections are one thing. Without a transaction, a
    // failure partway through leaves a song row with SOME of its verses — and the
    // operator finds out when the second chorus isn't there, mid-song, on a Sunday.
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO songs (title, author, ccli, song_key, bpm, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (title, author, ccli, song_key, bpm, date),
    )?;
    let song_id = tx.last_insert_rowid();
    for (i, s) in sections.iter().enumerate() {
        tx.execute(
            "INSERT INTO song_sections (song_id, position, tag, label, lyrics)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (song_id, i as i64, &s.tag, &s.label, &s.lyrics),
        )?;
    }
    tx.commit()?;
    Ok(song_id)
}

/// The id of an existing song with this title (case-insensitive), or None.
/// Used to dedupe on re-import — replace rather than duplicate.
pub fn song_id_by_title(conn: &Connection, title: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM songs WHERE title = ?1 COLLATE NOCASE ORDER BY id LIMIT 1",
        [title],
        |r| r.get(0),
    )
    .optional()
}

/// Update a song's metadata and replace all its sections in one transaction.
/// The editor holds the full section list and saves it wholesale (simplest
/// correct model — no per-row diffing). Positions are the array order.
#[allow(clippy::too_many_arguments)]
pub fn update_song(
    conn: &Connection,
    id: i64,
    title: &str,
    author: &str,
    ccli: &str,
    song_key: &str,
    bpm: Option<i64>,
    sections: &[crate::songs::ParsedSection],
) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE songs SET title = ?1, author = ?2, ccli = ?3, song_key = ?4, bpm = ?5 WHERE id = ?6",
        (title, author, ccli, song_key, bpm, id),
    )?;
    tx.execute("DELETE FROM song_sections WHERE song_id = ?1", [id])?;
    for (i, s) in sections.iter().enumerate() {
        tx.execute(
            "INSERT INTO song_sections (song_id, position, tag, label, lyrics)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (id, i as i64, &s.tag, &s.label, &s.lyrics),
        )?;
    }
    tx.commit()
}

/// Delete a song and its sections.
pub fn delete_song(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM song_sections WHERE song_id = ?1", [id])?;
    tx.execute("DELETE FROM songs WHERE id = ?1", [id])?;
    tx.commit()
}

/// Expand a song's sections into a played order. `seq` is an optional JSON array
/// of 0-based section indices (an arrangement) — repeats allowed, out-of-range
/// dropped. `None` (no arrangement) yields the sections verbatim, in order.
fn expand_sections(sections: &[crate::songs::ParsedSection], seq: Option<&Value>) -> Value {
    match seq.and_then(Value::as_array) {
        Some(idxs) => {
            let ordered: Vec<&crate::songs::ParsedSection> = idxs
                .iter()
                .filter_map(Value::as_u64)
                .filter_map(|i| sections.get(i as usize))
                .collect();
            serde_json::to_value(ordered).unwrap_or(Value::Array(vec![]))
        }
        None => serde_json::to_value(sections).unwrap_or(Value::Array(vec![])),
    }
}

/// Propagate a song's edits to every plan that cues it: rewrite each matching
/// song cue's snapshot (title + sections) so a lyric edit shows up in the
/// Planner and anywhere else the song is used. Returns how many cues updated.
pub fn sync_song_in_plans(
    conn: &Connection,
    song_id: i64,
    title: &str,
    sections: &[crate::songs::ParsedSection],
) -> rusqlite::Result<usize> {
    let mut stmt =
        conn.prepare("SELECT id, payload_json FROM plan_items WHERE cue_type = 'song'")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    let shape = shape_of_parsed(sections);
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    for (item_id, payload) in rows {
        let mut v: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
        if v.get("song_id").and_then(Value::as_i64) == Some(song_id) {
            v["title"] = Value::String(title.to_string());
            // Re-expand through the cue's arrangement so a lyric edit lands in
            // the right (possibly repeated) slots; no arrangement → straight order.
            //
            // UNLESS the song's STRUCTURE changed. The cue stores indices, so a
            // reorder or an inserted section makes index 3 mean a different verse
            // than the person who built the arrangement chose — and re-expanding
            // through it would put the wrong words on a wall, on a Sunday, with
            // nothing anywhere saying so. Relay will not guess which section was
            // meant: the cue falls back to the song's own order, which is always
            // the right WORDS even if it is not the intended repeats, and it is
            // marked so the Planner can say it out loud.
            //
            // A cue with no recorded shape is left alone rather than assumed
            // stale — the same rule the column follows. (No such cue can exist in
            // the wild: nothing could create an arrangement until the editor did.)
            let built = v.get("arrangement_shape").and_then(Value::as_str);
            let drifted = built.is_some_and(|b| !b.is_empty() && b != shape);
            if drifted {
                v["sections"] = expand_sections(sections, None);
                v["arrangement_stale"] = Value::Bool(true);
            } else {
                v["sections"] = expand_sections(sections, v.get("arrangement_seq"));
                v["arrangement_stale"] = Value::Bool(false);
            }
            tx.execute(
                "UPDATE plan_items SET label = ?1, payload_json = ?2 WHERE id = ?3",
                (title, v.to_string(), item_id),
            )?;
            n += 1;
        }
    }
    tx.commit()?;
    Ok(n)
}
