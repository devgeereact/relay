//! Output templates: the look of what the congregation sees.
//!
//! One template = one styled render target config. `TemplateRender.svelte` is the
//! single renderer that consumes these, so a template looks identical in the
//! editor preview and on a 4K wall.

use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An output template: layout (regions + alignment) and style (fonts, colors,
/// sizes). `layout` and `style` are opaque JSON blobs interpreted by the shared
/// renderer (Output.svelte) — the DB doesn't care about their internals, which
/// keeps the template shape editable without a migration. See docs/SPEC.md §5.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub layout: Value,
    pub style: Value,
    /// Whether this template is one of the (max 4) styles previewed on the
    /// console Output grid. Users can keep many templates but activate only 4.
    #[serde(default)]
    pub active: bool,
}

/// All templates, ordered by id.
pub fn list_templates(conn: &Connection) -> rusqlite::Result<Vec<Template>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, region_config_json, style_json, console_active FROM templates ORDER BY id",
    )?;
    let rows = stmt.query_map([], row_to_template)?;
    rows.collect()
}

/// The active templates shown on the console Output grid (max 4).
pub fn list_active_templates(conn: &Connection) -> rusqlite::Result<Vec<Template>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, region_config_json, style_json, console_active
           FROM templates WHERE console_active = 1 ORDER BY id LIMIT 4",
    )?;
    let rows = stmt.query_map([], row_to_template)?;
    rows.collect()
}

/// A single template by id.
pub fn get_template(conn: &Connection, id: i64) -> rusqlite::Result<Option<Template>> {
    conn.query_row(
        "SELECT id, name, region_config_json, style_json, console_active FROM templates WHERE id = ?1",
        [id],
        row_to_template,
    )
    .optional()
}

/// Count of currently-active templates (for the max-4 rule).
pub fn active_template_count(conn: &Connection, excluding: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM templates WHERE console_active = 1 AND id != ?1",
        [excluding],
        |r| r.get(0),
    )
}

/// Set a template's active flag. Max-4 enforcement lives in the command layer.
pub fn set_template_active(conn: &Connection, id: i64, active: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE templates SET console_active = ?1 WHERE id = ?2",
        (active as i64, id),
    )?;
    Ok(())
}

/// Create a new template with sensible starting style. Returns its id.
pub fn create_template(conn: &Connection, name: &str) -> rusqlite::Result<i64> {
    let layout = r##"{"regions":["verse_text","reference"],"align":"center"}"##;
    let style = r##"{"verseSize":6,"refSize":2.6,"verseColor":"#f4e4c8","accent":"#f5a623","background":"#0b0906","font":"Fraunces"}"##;
    conn.execute(
        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
        (name, layout, style),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Delete a template. Any output channel pointing at it is unassigned first so
/// the foreign key stays valid.
pub fn delete_template(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE output_channels SET template_id = NULL WHERE template_id = ?1",
        [id],
    )?;
    conn.execute("DELETE FROM templates WHERE id = ?1", [id])?;
    Ok(())
}

/// Create the `console_active` column if missing and guarantee up to 4 defaults
/// are active. Idempotent — safe on every open, covers pre-existing DBs.
pub fn ensure_template_active(conn: &Connection) -> rusqlite::Result<()> {
    let has: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('templates') WHERE name = 'console_active'",
        [],
        |r| r.get(0),
    )?;
    if has == 0 {
        conn.execute_batch(
            "ALTER TABLE templates ADD COLUMN console_active INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM templates WHERE console_active = 1",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute(
            "UPDATE templates SET console_active = 1
               WHERE id IN (SELECT id FROM templates ORDER BY id LIMIT 4)",
            [],
        )?;
    }
    Ok(())
}

/// Insert (id <= 0) or update (id > 0) a template. Returns its id.
pub fn upsert_template(conn: &Connection, t: &Template) -> rusqlite::Result<i64> {
    let layout = t.layout.to_string();
    let style = t.style.to_string();
    if t.id > 0 {
        conn.execute(
            "UPDATE templates SET name = ?1, region_config_json = ?2, style_json = ?3 WHERE id = ?4",
            (&t.name, &layout, &style, t.id),
        )?;
        Ok(t.id)
    } else {
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (&t.name, &layout, &style),
        )?;
        Ok(conn.last_insert_rowid())
    }
}

fn row_to_template(r: &rusqlite::Row) -> rusqlite::Result<Template> {
    let layout: String = r.get(2)?;
    let style: String = r.get(3)?;
    Ok(Template {
        id: r.get(0)?,
        name: r.get(1)?,
        layout: serde_json::from_str(&layout).unwrap_or(Value::Null),
        style: serde_json::from_str(&style).unwrap_or(Value::Null),
        active: r.get::<_, i64>(4).unwrap_or(0) != 0,
    })
}

/// The four built-in output templates (SPEC §5, cqw sizes). Match the frontend
/// defaults in src/lib/templates.js. Source of truth for both fresh seed and
/// the in-place migration.
fn builtin_templates() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        (
            "Classic Serif",
            r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":false,"refFirst":false}"##,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)","accent":"#e8a33d","verseColor":"#f4e4c8","verseSize":"5.5","refSize":"2.6","italicRef":true}"##,
        ),
        (
            "Stage Mono",
            r##"{"regions":["reference","verse_text"],"align":"left","lowerThird":false,"refFirst":true}"##,
            r##"{"font":"var(--f-display)","background":"#000000","accent":"#4fa8c9","verseColor":"#ffffff","verseSize":"6","refSize":"2.6","italicRef":false}"##,
        ),
        (
            "Lower Third",
            r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":true,"refFirst":false}"##,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#b080e0","verseColor":"#1c1224","verseSize":"2.6","refSize":"1.7","italicRef":false}"##,
        ),
        (
            // WORSHIP LYRICS — the fifth built-in, and the one every previous
            // template was wrong for.
            //
            // Until now every built-in was SCRIPTURE-shaped: a `reference` region
            // and verse text sized around 5cqw, because a verse is a paragraph
            // with a citation. A lyric is neither. It is three or four short
            // lines that a room full of people has to read while singing, from
            // the back, often over a lit stage — and it has no reference at all.
            // Rendering lyrics through a scripture template put a large gold
            // "Song Title · Slide 7" where the words should be and shrank the
            // words to a caption.
            //
            //   · NO reference region. The congregation is not singing the title.
            //   · Large (9cqw) — roughly twice the scripture size. Short lines can
            //     afford it, and TemplateRender auto-shrinks anything that would
            //     overflow, so a long line is safe.
            //   · White on near-black: the highest contrast available, which is
            //     what a projector in a lit room actually needs.
            //   · Sans, not serif. Serifs are for reading a paragraph; a lyric is
            //     scanned in a second and a half between breaths.
            "Worship Lyrics",
            r##"{"regions":["verse_text"],"align":"center","lowerThird":false,"refFirst":false}"##,
            r##"{"font":"var(--f-body)","background":"#07070a","accent":"#ffffff","verseColor":"#ffffff","verseSize":"9","refSize":"2","italicRef":false}"##,
        ),
        (
            "Lobby Warm",
            r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":false}"##,
            r##"{"font":"var(--f-serif)","background":"linear-gradient(160deg, #241419, #120a0e)","accent":"#e27d93","verseColor":"#f0dfe3","verseSize":"4","refSize":"2","italicRef":false}"##,
        ),
    ]
}

/// Seed the built-in templates into a fresh DB (ids 1..4).
pub(super) fn seed_templates(conn: &Connection) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
    )?;
    for (name, layout, style) in builtin_templates() {
        stmt.execute((name, layout, style))?;
    }
    Ok(())
}

/// Reset the built-in templates IN PLACE — keeps ids stable so output_channels
/// FKs stay valid. Used by the vw→cqw migration.
///
/// NOTE: this rewrites ids 1..N positionally. "Worship Lyrics" was added as the
/// FOURTH entry, so on an existing database this renames whatever sat at id 4
/// (Lobby Warm) — which is why `ensure_lyrics_template` appends instead of
/// relying on this, and why this is only called by the one migration that
/// already intended a full rewrite.
pub(super) fn reset_builtin_templates(conn: &Connection) -> rusqlite::Result<()> {
    for (i, (name, layout, style)) in builtin_templates().iter().enumerate() {
        conn.execute(
            "UPDATE templates SET name = ?1, region_config_json = ?2, style_json = ?3 WHERE id = ?4",
            (name, layout, style, i as i64 + 1),
        )?;
    }
    Ok(())
}
