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
        // Losing a migration race is not a failure — see the note on
        // `plans::add_plan_item_column`. Two processes can hold this file.
        match conn.execute_batch(
            "ALTER TABLE templates ADD COLUMN console_active INTEGER NOT NULL DEFAULT 0;",
        ) {
            Err(e)
                if e.to_string()
                    .to_lowercase()
                    .contains("duplicate column name") => {}
            other => other?,
        }
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

/// Give an EXISTING database the lyrics template, and point songs at it.
///
/// Idempotent and additive: it APPENDS a row rather than rewriting ids, because
/// `output_channels` and `app_settings` hold template ids as foreign keys and
/// renumbering them would silently repoint a church's projector at a different
/// design (CLAUDE.md §25 — a migration must be safe to re-run).
///
/// It also sets the per-content-type mapping for `song` **only if the operator
/// has not chosen one**. Overwriting a deliberate choice would be worse than the
/// bug this fixes.
pub(super) fn ensure_lyrics_template(conn: &Connection) -> rusqlite::Result<()> {
    const NAME: &str = "Worship Lyrics";
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM templates WHERE name = ?1", [NAME], |r| {
            r.get(0)
        })
        .ok();
    let id = match existing {
        Some(id) => id,
        None => {
            let (_, layout, style) = builtin_templates()
                .iter()
                .find(|(n, _, _)| *n == NAME)
                .copied()
                .unwrap_or((NAME, "{}", "{}"));
            conn.execute(
                "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
                (NAME, layout, style),
            )?;
            conn.last_insert_rowid()
        }
    };
    // The song content-type default is read via the canonical `tpl_{kind}` key
    // (`tpl_song`, see settings.rs::content_template_id). An earlier version of
    // this seed wrote it under `content_template_song` instead, so the read side
    // never found it and the lyrics default silently never applied. Write the
    // canonical key, and migrate any value left under the legacy key first so an
    // operator's earlier choice is preserved.
    let chosen: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'tpl_song'",
            [],
            |r| r.get(0),
        )
        .ok();
    if chosen.is_none() {
        let legacy: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'content_template_song'",
                [],
                |r| r.get(0),
            )
            .ok();
        let value = legacy.unwrap_or_else(|| id.to_string());
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('tpl_song', ?1)",
            [value],
        )?;
    }
    // Drop the dead legacy key so it can't shadow or confuse later reads.
    conn.execute(
        "DELETE FROM app_settings WHERE key = 'content_template_song'",
        [],
    )?;
    Ok(())
}

/// Ready-to-use preset templates, added on top of the five built-ins.
///
/// Every one is a real, complete template the operator can put on a wall as-is —
/// not a placeholder. The whole set is built around one constraint the model
/// imposes and one the room imposes:
///
///   - the model has **no image asset store**, so a background is a CSS gradient
///     or a solid, never a photo. Gradients are what keep the words legible: a
///     photo behind scripture is the single most common way a verse becomes
///     unreadable, so these deliberately don't use one;
///   - a projector in a LIT room needs contrast, not decoration. Every preset is
///     light text on a dark field (or a solid band for a lower third), because
///     that is the combination that survives ambient light and a cheap lens.
///
/// Sizes are `cqw`, so they scale to any output, and TemplateRender auto-shrinks
/// anything that would overflow — a long verse is safe at these sizes.
///
/// Kept SEPARATE from `builtin_templates()` and added by name (never by id), so
/// the five originals keep their stable ids and a church's channel/plan foreign
/// keys are never repointed.
fn preset_templates() -> &'static [(&'static str, &'static str, &'static str)] {
    // Scripture layout: reference + verse, centered. The common case.
    const SCRIPTURE: &str = r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":false,"refFirst":false}"##;
    // Lyric layout: verse text ONLY, no citation — a room does not sing the title.
    const LYRIC: &str =
        r##"{"regions":["verse_text"],"align":"center","lowerThird":false,"refFirst":false}"##;
    // Lower-third band: transparent, pinned bottom, keys out for OBS/ATEM.
    const BAND: &str = r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":true,"refFirst":false}"##;
    // Stage/confidence: reference first, left-aligned, for the preacher's monitor.
    const STAGE: &str = r##"{"regions":["reference","verse_text"],"align":"left","lowerThird":false,"refFirst":true}"##;

    &[
        // ── Scripture, dark colorways (light text, high contrast) ──────────────
        (
            "Midnight Blue",
            SCRIPTURE,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(130% 130% at 50% 18%, #12253f, #05080f)","accent":"#f0b74a","verseColor":"#eef2f8","verseSize":"5.2","refSize":"2.5","italicRef":true}"##,
        ),
        (
            "Royal Amethyst",
            SCRIPTURE,
            r##"{"font":"var(--f-serif)","background":"linear-gradient(155deg, #2a1a45, #0e0818)","accent":"#b79bff","verseColor":"#f2ecff","verseSize":"5.2","refSize":"2.5","italicRef":true}"##,
        ),
        (
            "Deep Teal",
            SCRIPTURE,
            r##"{"font":"var(--f-display)","background":"linear-gradient(160deg, #06231f, #03100d)","accent":"#46d6b3","verseColor":"#e8fbf5","verseSize":"5.2","refSize":"2.4","italicRef":false}"##,
        ),
        (
            "Crimson Grace",
            SCRIPTURE,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(120% 130% at 50% 22%, #3a0f16, #120507)","accent":"#f08a7a","verseColor":"#fbe9e6","verseSize":"5.2","refSize":"2.5","italicRef":true}"##,
        ),
        (
            "Emerald Word",
            SCRIPTURE,
            r##"{"font":"var(--f-display)","background":"radial-gradient(120% 130% at 50% 22%, #0c2a1c, #05100b)","accent":"#56d98f","verseColor":"#eafff4","verseSize":"5.2","refSize":"2.4","italicRef":false}"##,
        ),
        (
            "Indigo Night",
            SCRIPTURE,
            r##"{"font":"var(--f-serif)","background":"linear-gradient(155deg, #141a3a, #070912)","accent":"#8fa2ff","verseColor":"#eef0ff","verseSize":"5.2","refSize":"2.5","italicRef":true}"##,
        ),
        (
            "Slate Minimal",
            SCRIPTURE,
            r##"{"font":"var(--f-body)","background":"linear-gradient(180deg, #1a1f26, #0b0e12)","accent":"#9fb4c9","verseColor":"#eef1f5","verseSize":"5","refSize":"2.4","italicRef":false}"##,
        ),
        (
            "Pure Contrast",
            SCRIPTURE,
            r##"{"font":"var(--f-display)","background":"#000000","accent":"#7fd4ff","verseColor":"#ffffff","verseSize":"5.4","refSize":"2.5","italicRef":false}"##,
        ),
        // ── Songs / lyrics (verse only, large, no reference) ───────────────────
        (
            "Lyric Bold",
            LYRIC,
            r##"{"font":"var(--f-body)","background":"#060608","accent":"#ffffff","verseColor":"#ffffff","verseSize":"9","refSize":"2","italicRef":false}"##,
        ),
        (
            "Lyric Glow",
            LYRIC,
            r##"{"font":"var(--f-body)","background":"radial-gradient(120% 120% at 50% 35%, #241548, #08040f)","accent":"#c9a6ff","verseColor":"#ffffff","verseSize":"8.5","refSize":"2","italicRef":false}"##,
        ),
        // ── Lower thirds (solid band, dark-on-light and light-on-dark) ─────────
        (
            // On a band `accent` IS the band fill and `verseColor` the text.
            "Lower Third Light",
            BAND,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#f4f4f6","verseColor":"#12151b","verseSize":"2.6","refSize":"1.6","italicRef":false}"##,
        ),
        (
            "Lower Third Night",
            BAND,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#101319","verseColor":"#f2f4f8","verseSize":"2.6","refSize":"1.6","italicRef":false}"##,
        ),
        // ── Stage / confidence monitor (ref-first, left, big) ──────────────────
        (
            "Stage Confidence",
            STAGE,
            r##"{"font":"var(--f-display)","background":"#000000","accent":"#62c9ff","verseColor":"#ffffff","verseSize":"6","refSize":"2.6","italicRef":false}"##,
        ),
        // ── Lobby / pre-service (warm, gentle) ─────────────────────────────────
        (
            "Lobby Sunrise",
            SCRIPTURE,
            r##"{"font":"var(--f-serif)","background":"linear-gradient(160deg, #2b1a12, #0f0a08)","accent":"#f0a85c","verseColor":"#f6e7d6","verseSize":"4.6","refSize":"2.2","italicRef":true}"##,
        ),
    ]
}

/// Cohesive THEME families — each is a coordinated set (scripture · lyrics ·
/// lower-third · announcement) sharing one palette, so an operator can dress a
/// whole service in one look instead of matching four templates by hand. Named
/// `Theme · Kind` so the gallery groups them visually.
///
/// These also exercise the style properties the editor now exposes — a soft text
/// shadow for legibility over a gradient, an uppercase lower third, and a
/// scrolling announcement ticker — so a fresh install ships working examples of
/// each rather than only documenting them.
fn theme_templates() -> &'static [(&'static str, &'static str, &'static str)] {
    const SCRIPTURE: &str = r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":false,"refFirst":false}"##;
    const LYRIC: &str =
        r##"{"regions":["verse_text"],"align":"center","lowerThird":false,"refFirst":false}"##;
    const BAND: &str = r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":true,"refFirst":false}"##;
    // Announcement: title above a scrolling body line (a ticker).
    const ANNOUNCE: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":true}"##;

    &[
        // ── Aurora — teal / emerald ────────────────────────────────────────────
        (
            "Aurora · Scripture",
            SCRIPTURE,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(130% 130% at 50% 15%, #0b3330, #04110f)","accent":"#6ee7c4","verseColor":"#eafff8","verseSize":"5.2","refSize":"2.5","italicRef":true,"textShadow":0.5,"verseLineHeight":1.34}"##,
        ),
        (
            "Aurora · Lyrics",
            LYRIC,
            r##"{"font":"var(--f-display)","background":"linear-gradient(165deg, #08302b, #03110f)","accent":"#ffffff","verseColor":"#ffffff","verseSize":"8.5","refSize":"2","textShadow":0.4,"verseLineHeight":1.2}"##,
        ),
        (
            "Aurora · Lower Third",
            BAND,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#eafaf5","verseColor":"#0c211d","verseSize":"2.6","refSize":"1.5","refTransform":"uppercase","refLetterSpacing":0.08}"##,
        ),
        (
            "Aurora · Announcement",
            ANNOUNCE,
            r##"{"font":"var(--f-display)","background":"linear-gradient(180deg, #0b3330, #04110f)","accent":"#6ee7c4","verseColor":"#eafff8","verseSize":"3.4","refSize":"2.6","refTransform":"uppercase","refLetterSpacing":0.06,"scroll":true,"textShadow":0.4}"##,
        ),
        // ── Ember — amber / crimson ────────────────────────────────────────────
        (
            "Ember · Scripture",
            SCRIPTURE,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(130% 130% at 50% 18%, #3a1508, #140603)","accent":"#ffb066","verseColor":"#fdeede","verseSize":"5.2","refSize":"2.5","italicRef":true,"textShadow":0.55,"verseLineHeight":1.34}"##,
        ),
        (
            "Ember · Lyrics",
            LYRIC,
            r##"{"font":"var(--f-display)","background":"linear-gradient(165deg, #2c0f06, #130603)","accent":"#ffffff","verseColor":"#ffffff","verseSize":"8.5","refSize":"2","textShadow":0.45,"verseLineHeight":1.2}"##,
        ),
        (
            "Ember · Lower Third",
            BAND,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#1a0d06","verseColor":"#ffe9d2","verseSize":"2.6","refSize":"1.5","refTransform":"uppercase","refLetterSpacing":0.08}"##,
        ),
        (
            "Ember · Announcement",
            ANNOUNCE,
            r##"{"font":"var(--f-display)","background":"linear-gradient(180deg, #3a1508, #140603)","accent":"#ffb066","verseColor":"#fdeede","verseSize":"3.4","refSize":"2.6","refTransform":"uppercase","refLetterSpacing":0.06,"scroll":true,"textShadow":0.45}"##,
        ),
        // ── Nocturne — indigo / blue ───────────────────────────────────────────
        (
            "Nocturne · Scripture",
            SCRIPTURE,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(130% 130% at 50% 15%, #141a3a, #05070f)","accent":"#9db4ff","verseColor":"#eef1ff","verseSize":"5.2","refSize":"2.5","italicRef":true,"textShadow":0.5,"verseLineHeight":1.34}"##,
        ),
        (
            "Nocturne · Lyrics",
            LYRIC,
            r##"{"font":"var(--f-display)","background":"linear-gradient(165deg, #10163a, #04060f)","accent":"#ffffff","verseColor":"#ffffff","verseSize":"8.5","refSize":"2","textShadow":0.4,"verseLineHeight":1.2}"##,
        ),
        (
            "Nocturne · Lower Third",
            BAND,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#0d1226","verseColor":"#e6ecff","verseSize":"2.6","refSize":"1.5","refTransform":"uppercase","refLetterSpacing":0.08}"##,
        ),
        (
            "Nocturne · Announcement",
            ANNOUNCE,
            r##"{"font":"var(--f-display)","background":"linear-gradient(180deg, #141a3a, #05070f)","accent":"#9db4ff","verseColor":"#eef1ff","verseSize":"3.4","refSize":"2.6","refTransform":"uppercase","refLetterSpacing":0.06,"scroll":true,"textShadow":0.4}"##,
        ),
    ]
}

/// Add every preset that is not already present, matched BY NAME so it is safe on
/// every boot and never disturbs the ids an operator's channels point at.
/// Additive, like `ensure_lyrics_template` — an operator who deleted or renamed a
/// preset does not get it silently resurrected under a different name, only the
/// ones genuinely absent are inserted.
/// Every ready-to-use design that ships on top of the five built-ins: the
/// standalone presets plus the coordinated theme families.
fn all_presets() -> impl Iterator<Item = &'static (&'static str, &'static str, &'static str)> {
    preset_templates().iter().chain(theme_templates().iter())
}

/// How many presets ship on top of the five built-ins — so tests can assert the
/// seeded total without hard-coding a number that drifts.
#[cfg(test)]
pub(super) fn preset_template_count() -> usize {
    all_presets().count()
}

pub(super) fn ensure_preset_templates(conn: &Connection) -> rusqlite::Result<()> {
    let mut check = conn.prepare("SELECT COUNT(*) FROM templates WHERE name = ?1")?;
    let mut insert = conn.prepare(
        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
    )?;
    for (name, layout, style) in all_presets() {
        let present: i64 = check.query_row([name], |r| r.get(0))?;
        if present == 0 {
            insert.execute((name, layout, style))?;
        }
    }
    Ok(())
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

#[cfg(test)]
mod lyrics_template_tests {
    use super::*;
    use crate::db::SCHEMA;

    /// An OLD database — seeded with the built-ins as they were BEFORE the
    /// lyrics template existed. That is the only state the migration is for; a
    /// fresh install gets it from `seed_templates` directly.
    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch("COMMIT;").ok();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .unwrap();
        let mut stmt = conn
            .prepare(
                "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            )
            .unwrap();
        for (name, layout, style) in builtin_templates()
            .iter()
            .filter(|(n, _, _)| *n != "Worship Lyrics")
        {
            stmt.execute((name, layout, style)).unwrap();
        }
        drop(stmt);
        conn
    }

    #[test]
    fn a_fresh_install_already_has_it() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch("COMMIT;").ok();
        seed_templates(&conn).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM templates WHERE name = 'Worship Lyrics'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn a_lyric_template_has_no_reference_region() {
        // THE BUG THIS FIXES: lyrics rendered through a scripture template put a
        // large "Song Title · Slide 7" where the words should be. The
        // congregation is not singing the title.
        let (_, layout, _) = builtin_templates()
            .iter()
            .find(|(n, _, _)| *n == "Worship Lyrics")
            .copied()
            .expect("no lyrics template");
        assert!(layout.contains("verse_text"));
        assert!(
            !layout.contains("reference"),
            "the lyrics template still draws a reference"
        );
    }

    #[test]
    fn lyrics_are_set_much_larger_than_scripture() {
        // A lyric is a few short lines read from the back of a lit room, not a
        // paragraph with a citation.
        let size = |name: &str| -> f32 {
            let (_, _, style) = builtin_templates()
                .iter()
                .find(|(n, _, _)| *n == name)
                .copied()
                .unwrap();
            let v: serde_json::Value = serde_json::from_str(style).unwrap();
            v["verseSize"].as_str().unwrap().parse().unwrap()
        };
        assert!(
            size("Worship Lyrics") >= size("Classic Serif") * 1.5,
            "lyrics are not meaningfully larger than scripture"
        );
    }

    #[test]
    fn the_migration_is_idempotent() {
        // CLAUDE.md §25: a migration must be safe to re-run. This one runs on
        // every boot.
        let conn = db();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        ensure_lyrics_template(&conn).unwrap();
        let once: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        ensure_lyrics_template(&conn).unwrap();
        ensure_lyrics_template(&conn).unwrap();
        let thrice: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        assert_eq!(once, before + 1, "the lyrics template was not added");
        assert_eq!(once, thrice, "re-running the migration duplicated it");
    }

    #[test]
    fn the_migration_never_overrules_an_operators_choice() {
        // Silently repointing a church's song template at ours would be worse
        // than the bug being fixed. The operator's choice lives under the
        // canonical `tpl_song` key (set_content_template writes `tpl_{kind}`).
        let conn = db();
        crate::db::settings::set_setting(&conn, "tpl_song", "2").unwrap();
        ensure_lyrics_template(&conn).unwrap();
        let v = crate::db::settings::get_setting(&conn, "tpl_song")
            .unwrap()
            .unwrap();
        assert_eq!(
            v, "2",
            "the operator's chosen song template was overwritten"
        );
    }

    #[test]
    fn songs_point_at_the_lyrics_template_when_nothing_was_chosen() {
        // The whole point of the fix: the seed must land under the SAME key the
        // read side uses, so `content_template_id("song")` actually resolves. It
        // previously wrote `content_template_song` and read `tpl_song`, so this
        // returned None and the lyrics default silently never applied.
        let conn = db();
        ensure_lyrics_template(&conn).unwrap();
        let id = crate::db::settings::content_template_id(&conn, "song")
            .unwrap()
            .expect("song content-type default should resolve after seeding");
        let name: String = conn
            .query_row("SELECT name FROM templates WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "Worship Lyrics");
    }

    #[test]
    fn a_value_stranded_under_the_legacy_key_is_migrated() {
        // An existing install may still carry the operator-invisible seed under
        // the old `content_template_song` key. The migration moves it to the
        // canonical key (so it finally takes effect) and drops the dead key.
        let conn = db();
        crate::db::settings::set_setting(&conn, "content_template_song", "3").unwrap();
        ensure_lyrics_template(&conn).unwrap();
        assert_eq!(
            crate::db::settings::content_template_id(&conn, "song").unwrap(),
            Some(3),
            "the legacy value was not migrated to the canonical key"
        );
        assert!(
            crate::db::settings::get_setting(&conn, "content_template_song")
                .unwrap()
                .is_none(),
            "the dead legacy key was left behind"
        );
    }
}

#[cfg(test)]
mod preset_template_tests {
    use super::*;
    use crate::db::SCHEMA;

    fn fresh() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch("COMMIT;").ok();
        conn
    }

    #[test]
    fn every_preset_is_added_exactly_once_and_re_running_adds_nothing() {
        // Runs on every boot — it MUST be idempotent (CLAUDE.md §25).
        let conn = fresh();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        ensure_preset_templates(&conn).unwrap();
        let once: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        ensure_preset_templates(&conn).unwrap();
        ensure_preset_templates(&conn).unwrap();
        let thrice: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            once,
            before + preset_template_count() as i64,
            "presets were not all added"
        );
        assert_eq!(once, thrice, "re-running duplicated presets");
    }

    #[test]
    fn there_are_presets_across_every_screen_type() {
        // Standalone presets (10–15) plus the theme families. Guard that each
        // derived kind is represented, so a future edit can't quietly drop a
        // category, and that the standalone set stays in its stated range.
        assert!(
            (12..=15).contains(&preset_templates().len()),
            "expected 12–15 standalone presets, got {}",
            preset_templates().len()
        );

        let kind = |layout: &str| -> &'static str {
            let v: serde_json::Value = serde_json::from_str(layout).unwrap();
            let band = v["lowerThird"].as_bool().unwrap_or(false);
            let regions: Vec<String> = v["regions"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let has = |r: &str| regions.iter().any(|x| x == r);
            if band {
                "lower-third"
            } else if has("reference") && has("verse_text") {
                "scripture"
            } else if has("verse_text") {
                "song"
            } else {
                "custom"
            }
        };
        let mut kinds: Vec<&str> = all_presets().map(|(_, l, _)| kind(l)).collect();
        kinds.sort();
        kinds.dedup();
        for want in ["scripture", "song", "lower-third"] {
            assert!(kinds.contains(&want), "no preset of kind {want}");
        }
    }

    #[test]
    fn every_theme_is_a_complete_coordinated_family() {
        // A theme is a SET — scripture, lyrics, lower-third and announcement all
        // sharing a look. A half-built theme (missing the announcement, say) is
        // worse than none, because the operator picks it and one content type
        // falls back to a mismatched default mid-service.
        use std::collections::HashSet;
        let mut themes: HashSet<&str> = HashSet::new();
        for (name, _, _) in theme_templates() {
            if let Some((theme, _)) = name.split_once(" · ") {
                themes.insert(theme);
            } else {
                panic!("theme template {name:?} is not named 'Theme · Kind'");
            }
        }
        assert!(!themes.is_empty(), "no themes defined");
        for theme in themes {
            for kind in ["Scripture", "Lyrics", "Lower Third", "Announcement"] {
                let want = format!("{theme} · {kind}");
                assert!(
                    theme_templates().iter().any(|(n, _, _)| *n == want),
                    "theme {theme:?} is missing its {kind} template"
                );
            }
        }
        // At least one announcement template scrolls — the ticker the editor's
        // scroll control drives has a real, seeded example.
        assert!(
            theme_templates()
                .iter()
                .any(|(_, _, style)| style.contains("\"scroll\":true")),
            "no theme ships a scrolling announcement"
        );
    }

    #[test]
    fn every_preset_is_valid_json_with_a_readable_verse_colour() {
        // A preset ships to a wall as-is, so a malformed one is a black screen in
        // front of a congregation. Parse each, and require a light verse colour on
        // a dark field — the contrast a lit room needs (lower-third bands excepted,
        // where the text sits on a solid accent and is dark on purpose).
        for (name, layout, style) in all_presets() {
            let l: serde_json::Value =
                serde_json::from_str(layout).unwrap_or_else(|_| panic!("{name}: bad layout json"));
            let s: serde_json::Value =
                serde_json::from_str(style).unwrap_or_else(|_| panic!("{name}: bad style json"));
            assert!(s["background"].is_string(), "{name}: no background");
            assert!(s["verseColor"].is_string(), "{name}: no verseColor");
            // No preset uses an image (the model has no asset store to hold one).
            assert!(
                s.get("bgImage").is_none(),
                "{name}: presets must not embed images"
            );

            let is_band = l["lowerThird"].as_bool().unwrap_or(false);
            if !is_band {
                let vc = s["verseColor"].as_str().unwrap().to_lowercase();
                // A light text colour — cheap luminance proxy: starts high.
                let bright =
                    ["#f", "#e", "#ffffff"].iter().any(|p| vc.starts_with(p)) || vc == "#ffffff";
                assert!(
                    bright,
                    "{name}: verse colour {vc} is not light-on-dark readable"
                );
            }
        }
    }

    #[test]
    fn preset_names_are_unique_and_do_not_collide_with_the_builtins() {
        // Added by name, so a collision would mean a preset is never inserted (or
        // an operator sees two identical names).
        use std::collections::HashSet;
        let mut seen: HashSet<&str> = builtin_templates().iter().map(|(n, _, _)| *n).collect();
        for (name, _, _) in all_presets() {
            assert!(
                seen.insert(name),
                "duplicate/colliding template name: {name}"
            );
        }
    }
}
