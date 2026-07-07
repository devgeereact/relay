//! SQLite access layer.
//!
//! Single responsibility: local-first persistence against the schema in
//! docs/data/schema.sql. Nothing else in this codebase should touch SQLite
//! directly — go through this module. See PROMPT.md Phase 2.
//!
//! Offline-first: the schema is compiled in via `include_str!`, so there is no
//! runtime dependency on the docs/ file being shipped alongside the binary.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// The canonical schema, baked into the binary at compile time.
const SCHEMA: &str = include_str!("../../docs/data/schema.sql");

/// A single verse row, shaped for the frontend (serialized across the Tauri
/// bridge). `reference` is the human-facing citation, e.g. "John 3:16".
#[derive(Debug, Clone, Serialize)]
pub struct VerseRow {
    pub id: i64,
    pub book: String,
    pub chapter: i64,
    pub verse: i64,
    pub text: String,
    pub reference: String,
    pub translation: String,
}

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

/// A configured output channel (name + render target + assigned template).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputChannel {
    pub id: i64,
    pub name: String,
    pub render_target: String, // native_window | ndi_encode | network_client
    pub template_id: Option<i64>,
    pub display_target: Option<String>,
    pub status: String,
}

/// All configured output channels.
pub fn list_output_channels(conn: &Connection) -> rusqlite::Result<Vec<OutputChannel>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, render_target, template_id, display_target, status
           FROM output_channels ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(OutputChannel {
            id: r.get(0)?,
            name: r.get(1)?,
            render_target: r.get(2)?,
            template_id: r.get(3)?,
            display_target: r.get(4)?,
            status: r.get(5)?,
        })
    })?;
    rows.collect()
}

/// Assign a template to a channel (the "make outputs assignable" control).
pub fn set_channel_template(conn: &Connection, id: i64, template_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE output_channels SET template_id = ?1 WHERE id = ?2",
        (template_id, id),
    )?;
    Ok(())
}

/// Assign a physical display to a channel (HDMI screen assignment). `display` is
/// the monitor index as a string, or None to clear (use the primary display).
pub fn set_channel_display(
    conn: &Connection,
    id: i64,
    display: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE output_channels SET display_target = ?1 WHERE id = ?2",
        (display, id),
    )?;
    Ok(())
}

/// Add a new output channel; returns its id. `render_target` must be one of
/// native_window / ndi_encode / network_client (enforced by the schema CHECK).
pub fn add_channel(
    conn: &Connection,
    name: &str,
    render_target: &str,
    template_id: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO output_channels (name, render_target, template_id, status)
         VALUES (?1, ?2, ?3, 'offline')",
        (name, render_target, template_id),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Delete an output channel.
pub fn delete_channel(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM output_channels WHERE id = ?1", [id])?;
    Ok(())
}

// ===== Voice profiles (Phase B — accent & speaker calibration) ==============

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

/// Seed the default output channels (idempotent — only when empty). Template ids
/// 1..4 match the seeded templates.
fn seed_channels(conn: &Connection) -> rusqlite::Result<()> {
    let channels: &[(&str, &str, i64, Option<&str>)] = &[
        ("Main screen", "native_window", 1, Some("Display 1")),
        ("Stage display", "network_client", 2, None),
        ("Streaming", "network_client", 3, None),
        ("Lobby screen", "network_client", 4, None),
    ];
    let mut stmt = conn.prepare(
        "INSERT INTO output_channels (name, render_target, template_id, display_target, status)
         VALUES (?1, ?2, ?3, ?4, 'offline')",
    )?;
    for (name, target, tmpl, disp) in channels {
        stmt.execute((name, target, tmpl, disp))?;
    }
    Ok(())
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

/// Open (or create) the on-device database at the default per-OS data path,
/// applying the schema and dev seed on first creation.
///
/// Called once at startup (not on a live-service path), so surfacing a hard
/// error here is correct — a broken DB must fail loudly before a service, not
/// silently mid-sermon.
pub fn open() -> rusqlite::Result<Connection> {
    let path = default_db_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let fresh = !path.exists();
    let conn = Connection::open(&path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    if fresh {
        init_fresh(&conn)?;
    } else {
        // Forward-fill for DBs created before templates were seeded (Phase 8).
        // Idempotent: only seeds when the table is empty.
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))?;
        if n == 0 {
            seed_templates(&conn)?;
        } else {
            // One-time migration: the old seed stored sizes as "verseSize":"4.6vw";
            // the new renderer uses cqw numbers. Match that SPECIFIC old format on
            // the built-ins only, so a user's custom template that merely contains
            // the substring "vw" (a font name, a colour) is never clobbered.
            let old: i64 = conn.query_row(
                "SELECT COUNT(*) FROM templates
                  WHERE id IN (1,2,3,4) AND style_json LIKE '%\"verseSize\":\"%vw\"%'",
                [],
                |r| r.get(0),
            )?;
            if old > 0 {
                reset_builtin_templates(&conn)?;
            }
            // Lyrics belong in the lower-third band, centred (ProPresenter's
            // "Lower 3rd Lyrics"). Forward-fill the built-in "Lower Third"
            // template from its old left-aligned default → centred. Only touches
            // the unedited built-in (still left + lowerThird), never a custom one.
            conn.execute(
                "UPDATE templates
                    SET region_config_json = '{\"regions\":[\"verse_text\",\"reference\"],\"align\":\"center\",\"lowerThird\":true,\"refFirst\":false}'
                  WHERE name = 'Lower Third'
                    AND region_config_json LIKE '%\"align\":\"left\"%'
                    AND region_config_json LIKE '%\"lowerThird\":true%'",
                [],
            )?;
        }
        // Forward-fill default output channels for pre-existing DBs.
        let cn: i64 = conn.query_row("SELECT COUNT(*) FROM output_channels", [], |r| r.get(0))?;
        if cn == 0 {
            seed_channels(&conn)?;
        }
        // Forward-fill the full Bible for DBs created with the old 15-verse seed.
        if verse_count(&conn)? < 30_000 {
            reimport_full_kjv(&conn)?;
        }
        // One-time re-clean: DBs imported before the gloss stripper baked the KJV
        // marginal notes ("... Heb. ...") into the verse text. Re-import to strip.
        let polluted: i64 = conn.query_row(
            "SELECT COUNT(*) FROM verses WHERE text LIKE '% Heb.%' OR text LIKE '%: Gr.%'",
            [],
            |r| r.get(0),
        )?;
        if polluted > 0 {
            reimport_full_kjv(&conn)?;
        }
        // FTS5 index for fast word/phrase scripture search. Build once; rebuild
        // if its row count drifts from `verses` (fresh index, or after a
        // reimport that repopulated verses without touching the index).
        let has_fts: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='verses_fts'",
            [],
            |r| r.get(0),
        )?;
        let stale = has_fts == 0 || {
            let fc: i64 = conn
                .query_row("SELECT COUNT(*) FROM verses_fts", [], |r| r.get(0))
                .unwrap_or(-1);
            let vc: i64 = conn.query_row("SELECT COUNT(*) FROM verses", [], |r| r.get(0))?;
            fc != vc
        };
        if stale {
            rebuild_verses_fts(&conn)?;
        }
    }
    // Phase B: voice-profiles table + a guaranteed active profile. Idempotent,
    // and covers DBs created before this table existed.
    ensure_voice_profiles(&conn)?;
    // Console-active templates (max 4) — column + defaults. Idempotent.
    ensure_template_active(&conn)?;
    // Key/value app settings (active Bible translation, …). Idempotent.
    ensure_app_settings(&conn)?;
    // Service plans + cues (Planner). Idempotent; covers pre-existing DBs.
    ensure_service_plans(&conn)?;
    // Songs + sections (Lyrics). Idempotent.
    ensure_songs(&conn)?;
    // Saved scripture + media assets (Library). Idempotent.
    ensure_saved_scripture(&conn)?;
    ensure_media(&conn)?;
    ensure_announcements(&conn)?;
    Ok(conn)
}

/// Apply the full schema and seed a fresh connection. Public so tests (and any
/// future in-memory scratch DB) can build a ready-to-query database directly.
pub fn init_fresh(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)?;
    seed(conn)?;
    // Guarantee an active voice profile exists even on a bare in-memory DB.
    ensure_voice_profiles(conn)?;
    ensure_template_active(conn)?;
    ensure_app_settings(conn)?;
    ensure_service_plans(conn)?;
    ensure_songs(conn)?;
    ensure_saved_scripture(conn)?;
    ensure_media(conn)?;
    ensure_announcements(conn)?;
    Ok(())
}

/// Resolve the default database file path per OS, honoring a RELAY_DB_PATH
/// override (handy for tests and dev). Kept dependency-free deliberately —
/// standard app-data locations, no `dirs` crate needed.
fn default_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("RELAY_DB_PATH") {
        return PathBuf::from(p);
    }
    let base = if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support"))
            .unwrap_or_else(|_| PathBuf::from("."))
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else {
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|_| PathBuf::from("."))
    };
    base.join("com.relay.app").join("relay.db")
}

/// Directory where imported media/document files are stored (next to the DB).
pub fn media_dir() -> PathBuf {
    default_db_path()
        .parent()
        .map(|p| p.join("media"))
        .unwrap_or_else(|| PathBuf::from("media"))
}

/// Look up a single verse by canonical reference. Returns None if absent.
pub fn lookup_verse(
    conn: &Connection,
    book: &str,
    chapter: i64,
    verse: i64,
) -> rusqlite::Result<Option<VerseRow>> {
    // Prefer the operator-selected translation (app_settings.active_translation);
    // fall back to whatever translation has the verse. No caller needs to know.
    conn.query_row(
        "SELECT v.id, v.book, v.chapter, v.verse, v.text, t.abbreviation
           FROM verses v JOIN translations t ON t.id = v.translation_id
          WHERE v.book = ?1 AND v.chapter = ?2 AND v.verse = ?3
          ORDER BY (CAST(t.id AS TEXT) =
                     COALESCE((SELECT value FROM app_settings WHERE key = 'active_translation'), '')) DESC
          LIMIT 1",
        (book, chapter, verse),
        row_to_verse,
    )
    .optional()
}

/// A Bible translation available in the corpus (Settings → Bible translations).
#[derive(Debug, Clone, Serialize)]
pub struct Translation {
    pub id: i64,
    pub name: String,
    pub abbreviation: String,
    pub language: String,
}

/// All translations present in the DB.
pub fn list_translations(conn: &Connection) -> rusqlite::Result<Vec<Translation>> {
    let mut stmt =
        conn.prepare("SELECT id, name, abbreviation, language FROM translations ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok(Translation {
            id: r.get(0)?,
            name: r.get(1)?,
            abbreviation: r.get(2)?,
            language: r.get(3)?,
        })
    })?;
    rows.collect()
}

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

// ===== Per-content-type default templates =====
//
// ProPresenter picks a template by content type (Lower-3rd Lyrics vs Lower-3rd
// Scripture). Relay stores that mapping as app_settings `tpl_<kind>` → template
// id. When a cue fires, the pipeline resolves the type's template and carries it
// as an override so the output renders content in its type's look — the channel
// template is the fallback when no mapping is set. One renderer, still.

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

/// The highest verse number in a chapter — the end of a whole-chapter passage
/// walk (Phase A). None when the book/chapter isn't in the corpus.
pub fn chapter_last_verse(
    conn: &Connection,
    book: &str,
    chapter: i64,
) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT MAX(verse) FROM verses WHERE book = ?1 AND chapter = ?2",
        (book, chapter),
        |r| r.get::<_, Option<i64>>(0),
    )
}

/// Total verses currently seeded — a cheap health check for the data layer.
pub fn verse_count(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM verses", [], |r| r.get(0))
}

/// Full-text (LIKE) scripture search — the fallback when a query isn't a
/// parseable reference (e.g. "shepherd"). Offline, corpus-only. Prefers the
/// operator-selected translation, same as `lookup_verse`.
pub fn search_verses_text(
    conn: &Connection,
    needle: &str,
    limit: i64,
) -> rusqlite::Result<Vec<VerseRow>> {
    // Strip LIKE wildcards from user input so they're matched literally.
    let pat = format!("%{}%", needle.replace(['%', '_'], ""));
    let mut stmt = conn.prepare(
        "SELECT v.id, v.book, v.chapter, v.verse, v.text, t.abbreviation
           FROM verses v JOIN translations t ON t.id = v.translation_id
          WHERE v.text LIKE ?1
          ORDER BY (CAST(t.id AS TEXT) =
                     COALESCE((SELECT value FROM app_settings WHERE key = 'active_translation'), '')) DESC,
                   v.id
          LIMIT ?2",
    )?;
    let rows = stmt.query_map((pat, limit), row_to_verse)?;
    rows.collect()
}

// ===== Service plans (Presentations) =====
//
// A service plan is an ordered list of cues of ANY content type (scripture,
// song, media, announcement). One polymorphic table — `plan_items` — preserves
// the "Cue" abstraction from the architecture plan: `cue_type` + `payload_json`,
// so rendering and routing never branch per type (CLAUDE.md). Local-first.

/// A row for the Planner plans list.
#[derive(Debug, Clone, Serialize)]
pub struct PlanSummary {
    pub id: i64,
    pub title: String,
    pub plan_date: String,
    pub cue_count: i64,
}

/// One cue in a plan. `payload_json` carries the type-specific data (for a
/// scripture cue: the reference + verse-text snapshot); `label` is the display
/// title. `template_id` picks the template that renders it (null = default).
#[derive(Debug, Clone, Serialize)]
pub struct PlanItem {
    pub id: i64,
    pub plan_id: i64,
    pub position: i64,
    pub cue_type: String,
    pub label: String,
    pub payload_json: String,
    pub template_id: Option<i64>,
}

/// Create the service-plan tables if missing. Idempotent; forward-fills DBs
/// created before plans existed (same pattern as `ensure_voice_profiles`).
pub fn ensure_service_plans(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS service_plans (
            id         INTEGER PRIMARY KEY,
            title      TEXT NOT NULL,
            plan_date  TEXT NOT NULL DEFAULT '',
            notes      TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT ''
         );
         CREATE TABLE IF NOT EXISTS plan_items (
            id           INTEGER PRIMARY KEY,
            plan_id      INTEGER NOT NULL REFERENCES service_plans(id) ON DELETE CASCADE,
            position     INTEGER NOT NULL,
            cue_type     TEXT NOT NULL,
            label        TEXT NOT NULL,
            payload_json TEXT NOT NULL DEFAULT '{}',
            template_id  INTEGER
         );
         CREATE INDEX IF NOT EXISTS idx_plan_items ON plan_items(plan_id, position);",
    )
}

/// All plans, newest first, with a live cue count.
pub fn list_plans(conn: &Connection) -> rusqlite::Result<Vec<PlanSummary>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.title, p.plan_date,
                (SELECT COUNT(*) FROM plan_items i WHERE i.plan_id = p.id)
           FROM service_plans p ORDER BY p.id DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(PlanSummary {
            id: r.get(0)?,
            title: r.get(1)?,
            plan_date: r.get(2)?,
            cue_count: r.get(3)?,
        })
    })?;
    rows.collect()
}

/// Create a plan, returning its id.
pub fn create_plan(conn: &Connection, title: &str, date: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO service_plans (title, plan_date, created_at) VALUES (?1, ?2, ?3)",
        (title, date, date),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Clone a plan and all its cues (preserving order) into a new plan. Returns the
/// new plan id. The operator's usual workflow: start from last week's order.
pub fn duplicate_plan(
    conn: &Connection,
    src_id: i64,
    new_title: &str,
    date: &str,
) -> rusqlite::Result<i64> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO service_plans (title, plan_date, created_at) VALUES (?1, ?2, ?3)",
        (new_title, date, date),
    )?;
    let new_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO plan_items (plan_id, position, cue_type, label, payload_json, template_id)
         SELECT ?1, position, cue_type, label, payload_json, template_id
           FROM plan_items WHERE plan_id = ?2",
        (new_id, src_id),
    )?;
    tx.commit()?;
    Ok(new_id)
}

/// Delete a plan and its items. Explicit child delete so it works regardless of
/// the connection's foreign_keys pragma.
pub fn delete_plan(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM plan_items WHERE plan_id = ?1", [id])?;
    tx.execute("DELETE FROM service_plans WHERE id = ?1", [id])?;
    tx.commit()
}

/// Ordered items of a plan.
pub fn plan_items(conn: &Connection, plan_id: i64) -> rusqlite::Result<Vec<PlanItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, plan_id, position, cue_type, label, payload_json, template_id
           FROM plan_items WHERE plan_id = ?1 ORDER BY position, id",
    )?;
    let rows = stmt.query_map([plan_id], |r| {
        Ok(PlanItem {
            id: r.get(0)?,
            plan_id: r.get(1)?,
            position: r.get(2)?,
            cue_type: r.get(3)?,
            label: r.get(4)?,
            payload_json: r.get(5)?,
            template_id: r.get(6)?,
        })
    })?;
    rows.collect()
}

/// Append a cue to a plan (position = next slot). Returns the new item id.
pub fn add_plan_item(
    conn: &Connection,
    plan_id: i64,
    cue_type: &str,
    label: &str,
    payload_json: &str,
    template_id: Option<i64>,
) -> rusqlite::Result<i64> {
    let pos: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM plan_items WHERE plan_id = ?1",
        [plan_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO plan_items (plan_id, position, cue_type, label, payload_json, template_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (plan_id, pos, cue_type, label, payload_json, template_id),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Set (or clear, when blank) a cue's operator stage note — stored on the item's
/// payload under `stage_note`. Confidence-monitor only; never on main output.
pub fn set_plan_note(conn: &Connection, item_id: i64, note: &str) -> rusqlite::Result<()> {
    let payload: String = conn.query_row(
        "SELECT payload_json FROM plan_items WHERE id = ?1",
        [item_id],
        |r| r.get(0),
    )?;
    let mut v: Value = serde_json::from_str(&payload).unwrap_or(Value::Object(Default::default()));
    if !v.is_object() {
        v = Value::Object(Default::default());
    }
    let note = note.trim();
    if note.is_empty() {
        v.as_object_mut().map(|o| o.remove("stage_note"));
    } else {
        v["stage_note"] = Value::String(note.to_string());
    }
    conn.execute(
        "UPDATE plan_items SET payload_json = ?1 WHERE id = ?2",
        (v.to_string(), item_id),
    )?;
    Ok(())
}

/// Remove one cue.
pub fn remove_plan_item(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM plan_items WHERE id = ?1", [id])?;
    Ok(())
}

/// Swap a cue with its neighbor in `direction` (-1 up, +1 down). No-op at ends.
pub fn move_plan_item(conn: &Connection, id: i64, direction: i64) -> rusqlite::Result<()> {
    let (plan_id, pos): (i64, i64) = conn.query_row(
        "SELECT plan_id, position FROM plan_items WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let target = pos + direction;
    let neighbor: Option<i64> = conn
        .query_row(
            "SELECT id FROM plan_items WHERE plan_id = ?1 AND position = ?2",
            (plan_id, target),
            |r| r.get(0),
        )
        .optional()?;
    if let Some(nid) = neighbor {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE plan_items SET position = ?1 WHERE id = ?2",
            (target, id),
        )?;
        tx.execute(
            "UPDATE plan_items SET position = ?1 WHERE id = ?2",
            (pos, nid),
        )?;
        tx.commit()?;
    }
    Ok(())
}

/// Rewrite the whole order of a plan from a drag-reorder: `ids` is the new order,
/// positions become the array index. One transaction.
pub fn reorder_plan_items(conn: &Connection, plan_id: i64, ids: &[i64]) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (pos, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE plan_items SET position = ?1 WHERE id = ?2 AND plan_id = ?3",
            (pos as i64, id, plan_id),
        )?;
    }
    tx.commit()
}

// ===== Songs (Lyrics) =====
//
// A song is metadata + ordered sections (Verse 1 / Chorus / Bridge…). Sections
// are parsed by the pure `songs` module at import; here we only persist and read
// them. A song becomes a plan cue (payload = its sections) via the Planner, and
// renders through the one shared template engine — no per-type render branch.

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
    )
}

/// A named play-order for a song (ProPresenter arrangements). `sequence` is the
/// ordered list of section positions to play — repeats allowed (V1 C1 V2 C1).
#[derive(Debug, Clone, Serialize)]
pub struct Arrangement {
    pub id: i64,
    pub name: String,
    pub sequence: Vec<i64>,
}

/// All arrangements for a song.
pub fn list_arrangements(conn: &Connection, song_id: i64) -> rusqlite::Result<Vec<Arrangement>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, sequence FROM song_arrangements WHERE song_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map([song_id], |r| {
        let seq: String = r.get(2)?;
        Ok(Arrangement {
            id: r.get(0)?,
            name: r.get(1)?,
            sequence: serde_json::from_str(&seq).unwrap_or_default(),
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
    match id {
        Some(aid) => {
            conn.execute(
                "UPDATE song_arrangements SET name = ?1, sequence = ?2 WHERE id = ?3 AND song_id = ?4",
                (name, &seq, aid, song_id),
            )?;
            Ok(aid)
        }
        None => {
            conn.execute(
                "INSERT INTO song_arrangements (song_id, name, sequence) VALUES (?1, ?2, ?3)",
                (song_id, name, &seq),
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
    conn.execute(
        "INSERT INTO songs (title, author, ccli, song_key, bpm, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (title, author, ccli, song_key, bpm, date),
    )?;
    let song_id = conn.last_insert_rowid();
    for (i, s) in sections.iter().enumerate() {
        conn.execute(
            "INSERT INTO song_sections (song_id, position, tag, label, lyrics)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (song_id, i as i64, &s.tag, &s.label, &s.lyrics),
        )?;
    }
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

/// Replace only a song's sections (keeps its metadata). For re-importing a
/// source file: fresh slides, but any author/key the operator set is preserved.
pub fn replace_song_sections(
    conn: &Connection,
    id: i64,
    sections: &[crate::songs::ParsedSection],
) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
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
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    for (item_id, payload) in rows {
        let mut v: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
        if v.get("song_id").and_then(Value::as_i64) == Some(song_id) {
            v["title"] = Value::String(title.to_string());
            // Re-expand through the cue's arrangement so a lyric edit lands in
            // the right (possibly repeated) slots; no arrangement → straight order.
            v["sections"] = expand_sections(sections, v.get("arrangement_seq"));
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

// ===== Saved scripture (Library → Scripture) =====
//
// The Scripture tab shows verses the operator has saved (not the whole corpus).
// A saved item snapshots the reference + text so it renders without a re-lookup.

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

// ===== Announcements (Library → Announcements) =====
//
// Text notice slides (title + body) the operator drafts ahead of a service and
// fires like any other cue. A plain content type — the template engine renders
// it; nothing here is special-cased downstream.

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

// ===== Media assets (Library → Media) =====
//
// Non-lyric imports live here as file pointers (offline-first: bytes stay on
// disk, DB holds the path + kind). Images/video render as backgrounds; pdf/pptx
// are stored as document decks (slide extraction is a later phase).

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
pub fn delete_media(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    let path: Option<String> = conn
        .query_row("SELECT path FROM media_assets WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .optional()?;
    conn.execute("DELETE FROM media_assets WHERE id = ?1", [id])?;
    Ok(path)
}

// ===== Service-session persistence =====
//
// Local-first service history: transcripts, fired detections, and operator cues
// (manual overrides, clear-screens) are written to the current service and
// surfaced in the Library tab. Nothing leaves the device (CLAUDE.md).

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
pub fn insert_detection(
    conn: &Connection,
    transcript_id: i64,
    verse_id: Option<i64>,
    method: &str,
    confidence: f32,
    status: &str,
    fired_at: Option<f64>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO detections (transcript_id, verse_id, method, confidence, status, fired_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (
            transcript_id,
            verse_id,
            method,
            confidence,
            status,
            fired_at,
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

/// Every verse, for building the semantic index (Phase 9).
pub fn all_verses(conn: &Connection) -> rusqlite::Result<Vec<VerseRow>> {
    let mut stmt = conn.prepare(
        "SELECT v.id, v.book, v.chapter, v.verse, v.text, t.abbreviation
           FROM verses v JOIN translations t ON t.id = v.translation_id
          ORDER BY v.id",
    )?;
    let rows = stmt.query_map([], row_to_verse)?;
    rows.collect()
}

fn row_to_verse(r: &rusqlite::Row) -> rusqlite::Result<VerseRow> {
    let book: String = r.get(1)?;
    let chapter: i64 = r.get(2)?;
    let verse: i64 = r.get(3)?;
    Ok(VerseRow {
        id: r.get(0)?,
        reference: format!("{book} {chapter}:{verse}"),
        book,
        chapter,
        verse,
        text: r.get(4)?,
        translation: r.get(5)?,
    })
}

/// The full public-domain KJV, bundled at compile time (offline-first — no
/// runtime file dependency). Structure: array of books in canonical order, each
/// `{ "chapters": [[verse, …], …] }`. Book names come from CANONICAL_BOOKS by
/// index, so a stored verse and a detected reference always agree on spelling.
const KJV_JSON: &str = include_str!("../data/kjv.json");

#[derive(serde::Deserialize)]
struct KjvBook {
    chapters: Vec<Vec<String>>,
}

/// Seed a fresh database: one KJV translation + the full Bible + templates.
fn seed(conn: &Connection) -> rusqlite::Result<()> {
    let translation_id = kjv_translation_id(conn)?;
    import_full_kjv(conn, translation_id)?;
    seed_templates(conn)?;
    seed_channels(conn)?;
    Ok(())
}

/// The KJV translation id, creating the row if absent.
fn kjv_translation_id(conn: &Connection) -> rusqlite::Result<i64> {
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM translations WHERE abbreviation = 'KJV'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO translations (name, abbreviation, language, license_type)
         VALUES (?1, ?2, ?3, ?4)",
        ("King James Version", "KJV", "en", "public domain"),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Parse the bundled KJV and bulk-insert every verse (one transaction — 31k
/// rows). Strips the `{…}` italic markers KJV uses for supplied words. Returns
/// the verse count inserted.
fn import_full_kjv(conn: &Connection, translation_id: i64) -> rusqlite::Result<usize> {
    let raw = KJV_JSON.trim_start_matches('\u{feff}'); // strip UTF-8 BOM
    let books: Vec<KjvBook> = serde_json::from_str(raw)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    let tx = conn.unchecked_transaction()?;
    let mut count = 0usize;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO verses (translation_id, book, chapter, verse, text)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (bi, book) in books.iter().enumerate() {
            let name = crate::detection::CANONICAL_BOOKS
                .get(bi)
                .copied()
                .unwrap_or("Unknown");
            for (ci, chapter) in book.chapters.iter().enumerate() {
                for (vi, text) in chapter.iter().enumerate() {
                    stmt.execute((
                        translation_id,
                        name,
                        ci as i64 + 1,
                        vi as i64 + 1,
                        clean_verse(text),
                    ))?;
                    count += 1;
                }
            }
        }
    }
    tx.commit()?;
    rebuild_verses_fts(conn)?;
    Ok(count)
}

/// (Re)build the FTS5 full-text index over `verses` for fast word/phrase search.
/// External-content table (no duplicated text); 'rebuild' repopulates from
/// `verses`. Porter stemmer so "shepherd" also matches "shepherds".
fn rebuild_verses_fts(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS verses_fts USING fts5(
            text, content='verses', content_rowid='id', tokenize='porter unicode61');
         INSERT INTO verses_fts(verses_fts) VALUES('rebuild');",
    )
}

/// Full-text scripture search (FTS5, ranked by bm25). Terms are quoted so the
/// user's punctuation/operators are treated literally, then OR'd for recall —
/// bm25 floats the verse carrying the most (and rarest) of the words to the top.
/// So "the lord is my shepherd" and loose "lord shepherd" both surface Ps 23:1.
pub fn search_verses_fts(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> rusqlite::Result<Vec<VerseRow>> {
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t.to_lowercase()))
        .collect();
    if terms.is_empty() {
        return Ok(vec![]);
    }
    let match_q = terms.join(" OR ");
    let mut stmt = conn.prepare(
        "SELECT v.id, v.book, v.chapter, v.verse, v.text, t.abbreviation
           FROM verses_fts
           JOIN verses v ON v.id = verses_fts.rowid
           JOIN translations t ON t.id = v.translation_id
          WHERE verses_fts MATCH ?1
          ORDER BY (CAST(t.id AS TEXT) =
                     COALESCE((SELECT value FROM app_settings WHERE key = 'active_translation'), '')) DESC,
                   bm25(verses_fts)
          LIMIT ?2",
    )?;
    let rows = stmt.query_map((match_q, limit), row_to_verse)?;
    rows.collect()
}

/// Clean a raw KJV verse. The source data brackets two very different things in
/// `{ }`: supplied-word italics (real text: `{it was}`, `{and}`) and translator
/// marginal glosses (NOT verse text: `{green...: Heb. pastures of tender grass}`).
/// Keep the supplied words (drop only the braces); drop the glosses entirely;
/// then collapse the whitespace the removed glosses leave behind.
fn clean_verse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let inner = &after[..close];
                if !is_gloss(inner) {
                    out.push_str(inner); // supplied word — keep, minus braces
                }
                rest = &after[close + 1..];
            }
            None => {
                // Unbalanced brace — keep the remainder verbatim, sans '{'.
                out.push_str(after);
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    // Collapse the double spaces a dropped gloss leaves and trim the ends.
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A brace group is a marginal gloss (not verse text) if it carries a
/// translator note. Supplied-word italics are short and never contain a colon
/// or a language marker — verified against the full corpus.
fn is_gloss(inner: &str) -> bool {
    inner.contains(": ")
        || inner.starts_with("Or,")
        || inner.contains("Heb.")
        || inner.contains("Gr.")
        || inner.contains("Chaldee")
        || inner.contains("Syriac")
}

/// Forward-fill the full corpus for DBs created before the full-Bible import
/// (they hold only the old 15-verse dev seed). FK-safe: nulls any detection
/// verse links first, then replaces the verses.
fn reimport_full_kjv(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("UPDATE detections SET verse_id = NULL", [])?;
    conn.execute("DELETE FROM verses", [])?;
    let tid = kjv_translation_id(conn)?;
    import_full_kjv(conn, tid)?;
    Ok(())
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
            "Lobby Warm",
            r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":false}"##,
            r##"{"font":"var(--f-serif)","background":"linear-gradient(160deg, #241419, #120a0e)","accent":"#e27d93","verseColor":"#f0dfe3","verseSize":"4","refSize":"2","italicRef":false}"##,
        ),
    ]
}

/// Seed the built-in templates into a fresh DB (ids 1..4).
fn seed_templates(conn: &Connection) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
    )?;
    for (name, layout, style) in builtin_templates() {
        stmt.execute((name, layout, style))?;
    }
    Ok(())
}

/// Reset the built-in templates IN PLACE (ids 1..4) — keeps ids stable so
/// output_channels FKs stay valid. Used by the vw→cqw migration.
fn reset_builtin_templates(conn: &Connection) -> rusqlite::Result<()> {
    for (i, (name, layout, style)) in builtin_templates().iter().enumerate() {
        conn.execute(
            "UPDATE templates SET name = ?1, region_config_json = ?2, style_json = ?3 WHERE id = ?4",
            (name, layout, style, i as i64 + 1),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        init_fresh(&conn).unwrap();
        conn
    }

    #[test]
    fn seeds_full_kjv() {
        let conn = fresh_db();
        // Full KJV is 31,102 verses; the bundled file has 31,100.
        assert!(verse_count(&conn).unwrap() > 31_000);
    }

    #[test]
    fn seeds_four_templates() {
        let conn = fresh_db();
        let ts = list_templates(&conn).unwrap();
        assert_eq!(ts.len(), 4);
        assert_eq!(ts[0].name, "Classic Serif");
        assert_eq!(ts[0].style["font"], "var(--f-serif)");
        assert_eq!(ts[0].layout["align"], "center");
    }

    #[test]
    fn upsert_updates_existing_template() {
        let conn = fresh_db();
        let mut t = get_template(&conn, 1).unwrap().unwrap();
        t.name = "Classic Serif (edited)".into();
        t.style["accent"] = serde_json::json!("#ffffff");
        let id = upsert_template(&conn, &t).unwrap();
        assert_eq!(id, 1);
        let reloaded = get_template(&conn, 1).unwrap().unwrap();
        assert_eq!(reloaded.name, "Classic Serif (edited)");
        assert_eq!(reloaded.style["accent"], "#ffffff");
    }

    #[test]
    fn upsert_inserts_new_template() {
        let conn = fresh_db();
        let t = Template {
            id: 0,
            name: "Custom".into(),
            layout: serde_json::json!({ "regions": ["verse_text"] }),
            style: serde_json::json!({ "font": "var(--f-body)" }),
            active: false,
        };
        let id = upsert_template(&conn, &t).unwrap();
        assert_eq!(id, 5);
        assert_eq!(list_templates(&conn).unwrap().len(), 5);
    }

    #[test]
    fn active_templates_capped_and_toggle() {
        let conn = fresh_db();
        // Fresh seed activates up to 4 of the default templates.
        assert_eq!(list_active_templates(&conn).unwrap().len(), 4);
        // Deactivate one, activate a freshly created one.
        set_template_active(&conn, 1, false).unwrap();
        let id = create_template(&conn, "Extra").unwrap();
        set_template_active(&conn, id, true).unwrap();
        let active = list_active_templates(&conn).unwrap();
        assert_eq!(active.len(), 4);
        assert!(active.iter().any(|t| t.id == id && t.active));
        // Count excluding self supports the command-layer max-4 rule.
        assert_eq!(active_template_count(&conn, id).unwrap(), 3);
    }

    #[test]
    fn deleting_template_unassigns_channels() {
        let conn = fresh_db();
        // Channel 1 (Main) points at template 1 in the seed.
        delete_template(&conn, 1).unwrap();
        assert!(get_template(&conn, 1).unwrap().is_none());
        let ch = list_output_channels(&conn).unwrap();
        assert!(ch.iter().all(|c| c.template_id != Some(1)));
    }

    #[test]
    fn looks_up_john_3_16_verbatim() {
        let conn = fresh_db();
        let v = lookup_verse(&conn, "John", 3, 16).unwrap().unwrap();
        assert_eq!(v.reference, "John 3:16");
        assert_eq!(v.translation, "KJV");
        assert!(v.text.starts_with("For God so loved the world"));
    }

    #[test]
    fn psalm_23_is_complete_for_context_memory() {
        // Six consecutive verses — the fixture context-memory logic (Phase 9)
        // resolves a bare "verse 4" against.
        let conn = fresh_db();
        for verse in 1..=6 {
            assert!(
                lookup_verse(&conn, "Psalms", 23, verse).unwrap().is_some(),
                "Psalms 23:{verse} should be seeded"
            );
        }
    }

    #[test]
    fn missing_verse_returns_none() {
        let conn = fresh_db();
        // Genesis has 50 chapters — 999 is safely out of range.
        assert!(lookup_verse(&conn, "Genesis", 999, 1).unwrap().is_none());
    }

    #[test]
    fn service_plan_crud_and_reorder() {
        let conn = fresh_db();
        ensure_service_plans(&conn).unwrap();

        let pid = create_plan(&conn, "Sunday Morning", "2026-07-05").unwrap();
        assert_eq!(list_plans(&conn).unwrap().len(), 1);

        // Append three cues; positions are assigned 0,1,2.
        let a = add_plan_item(&conn, pid, "scripture", "Psalm 23:1", "{}", None).unwrap();
        let b = add_plan_item(&conn, pid, "scripture", "John 3:16", "{}", None).unwrap();
        let _c = add_plan_item(&conn, pid, "announce", "Tithe", "{}", None).unwrap();
        let items = plan_items(&conn, pid).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].label, "Psalm 23:1");
        assert_eq!(list_plans(&conn).unwrap()[0].cue_count, 3);

        // Move the first cue down — order becomes John, Psalm, Tithe.
        move_plan_item(&conn, a, 1).unwrap();
        let items = plan_items(&conn, pid).unwrap();
        assert_eq!(items[0].id, b);
        assert_eq!(items[1].id, a);

        // Moving the top cue up is a no-op (already at position 0).
        move_plan_item(&conn, b, -1).unwrap();
        assert_eq!(plan_items(&conn, pid).unwrap()[0].id, b);

        // Remove one; deleting the plan clears its items.
        remove_plan_item(&conn, a).unwrap();
        assert_eq!(plan_items(&conn, pid).unwrap().len(), 2);
        delete_plan(&conn, pid).unwrap();
        assert!(list_plans(&conn).unwrap().is_empty());
        assert!(plan_items(&conn, pid).unwrap().is_empty());
    }

    #[test]
    fn duplicate_plan_clones_all_cues_in_order() {
        let conn = fresh_db();
        ensure_service_plans(&conn).unwrap();
        let src = create_plan(&conn, "Sunday AM", "2026-07-07").unwrap();
        add_plan_item(
            &conn,
            src,
            "scripture",
            "Psalm 23:1",
            r#"{"reference":"Psalm 23:1"}"#,
            None,
        )
        .unwrap();
        add_plan_item(&conn, src, "song", "Way Maker", r#"{"song_id":1}"#, Some(2)).unwrap();

        let copy = duplicate_plan(&conn, src, "Sunday AM (copy)", "2026-07-14").unwrap();
        assert_ne!(copy, src);
        let orig = plan_items(&conn, src).unwrap();
        let dup = plan_items(&conn, copy).unwrap();
        assert_eq!(dup.len(), orig.len());
        assert_eq!(dup[0].label, "Psalm 23:1");
        assert_eq!(dup[1].cue_type, "song");
        assert_eq!(dup[1].template_id, Some(2)); // template + payload copied
        assert!(dup[1].payload_json.contains("song_id"));
        // Independent copies — editing one plan doesn't touch the other.
        delete_plan(&conn, copy).unwrap();
        assert_eq!(plan_items(&conn, src).unwrap().len(), 2);
    }

    #[test]
    fn editing_announcement_propagates_to_plans() {
        let conn = fresh_db();
        ensure_service_plans(&conn).unwrap();
        ensure_announcements(&conn).unwrap();
        let aid = save_announcement(&conn, None, "Bake sale", "Saturday", "2026-07-07").unwrap();
        let pid = create_plan(&conn, "Sunday", "2026-07-07").unwrap();
        let payload = format!(r#"{{"announce_id":{aid},"title":"Bake sale","body":"Saturday"}}"#);
        add_plan_item(&conn, pid, "announce", "Bake sale", &payload, None).unwrap();

        // Edit the announcement → the cue's snapshot follows.
        save_announcement(
            &conn,
            Some(aid),
            "Bake Sale",
            "Sunday after service",
            "2026-07-07",
        )
        .unwrap();
        let n =
            sync_announcement_in_plans(&conn, aid, "Bake Sale", "Sunday after service").unwrap();
        assert_eq!(n, 1);
        let item = &plan_items(&conn, pid).unwrap()[0];
        assert_eq!(item.label, "Bake Sale");
        assert!(item.payload_json.contains("Sunday after service"));
        assert!(!item.payload_json.contains("\"body\":\"Saturday\""));
    }

    #[test]
    fn announcements_crud() {
        let conn = fresh_db();
        ensure_announcements(&conn).unwrap();
        assert!(list_announcements(&conn).unwrap().is_empty());

        let id = save_announcement(&conn, None, "Midweek", "Wed 7pm", "2026-07-07").unwrap();
        let list = list_announcements(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Midweek");
        assert_eq!(list[0].body, "Wed 7pm");

        // Update by id (not a new row).
        save_announcement(
            &conn,
            Some(id),
            "Midweek Service",
            "Wed 7:30pm",
            "2026-07-07",
        )
        .unwrap();
        let list = list_announcements(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Midweek Service");
        assert_eq!(list[0].body, "Wed 7:30pm");

        delete_announcement(&conn, id).unwrap();
        assert!(list_announcements(&conn).unwrap().is_empty());
    }

    #[test]
    fn fts_finds_verses_by_loose_words() {
        let conn = fresh_db(); // seed() builds the FTS index
                               // Loose, non-contiguous words — a substring LIKE ('%lord shepherd%')
                               // would miss this; FTS surfaces the verses carrying both.
        let hits = search_verses_fts(&conn, "lord shepherd", 20).unwrap();
        assert!(hits
            .iter()
            .any(|v| v.book == "Psalms" && v.chapter == 23 && v.verse == 1));
        // A distinctive phrase ranks its verse at the top.
        let top = &search_verses_fts(&conn, "valley of the shadow of death", 5).unwrap()[0];
        assert_eq!(
            (top.book.as_str(), top.chapter, top.verse),
            ("Psalms", 23, 4)
        );
        // Punctuation/operators are treated literally, not as FTS syntax — no error.
        assert!(search_verses_fts(&conn, "\"lord\" OR (shepherd*", 5).is_ok());
    }

    #[test]
    fn stage_note_set_and_clear_on_payload() {
        let conn = fresh_db();
        ensure_service_plans(&conn).unwrap();
        let pid = create_plan(&conn, "Sunday", "2026-07-07").unwrap();
        let id = add_plan_item(&conn, pid, "song", "Way Maker", r#"{"song_id":1}"#, None).unwrap();

        // Set a note — merged into the existing payload, not clobbering it.
        set_plan_note(&conn, id, "  hold for prayer  ").unwrap();
        let p = &plan_items(&conn, pid).unwrap()[0].payload_json;
        let v: Value = serde_json::from_str(p).unwrap();
        assert_eq!(v["stage_note"], "hold for prayer"); // trimmed
        assert_eq!(v["song_id"], 1); // other keys preserved

        // Blank clears the key entirely.
        set_plan_note(&conn, id, "   ").unwrap();
        let v: Value =
            serde_json::from_str(&plan_items(&conn, pid).unwrap()[0].payload_json).unwrap();
        assert!(v.get("stage_note").is_none());
        assert_eq!(v["song_id"], 1);
    }

    #[test]
    fn drag_reorder_rewrites_positions() {
        let conn = fresh_db();
        ensure_service_plans(&conn).unwrap();
        let pid = create_plan(&conn, "P", "d").unwrap();
        let a = add_plan_item(&conn, pid, "scripture", "A", "{}", None).unwrap();
        let b = add_plan_item(&conn, pid, "scripture", "B", "{}", None).unwrap();
        let c = add_plan_item(&conn, pid, "scripture", "C", "{}", None).unwrap();
        // Drag into a new order: C, A, B.
        reorder_plan_items(&conn, pid, &[c, a, b]).unwrap();
        let order: Vec<i64> = plan_items(&conn, pid)
            .unwrap()
            .iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(order, vec![c, a, b]);
    }

    #[test]
    fn text_search_finds_verses() {
        let conn = fresh_db();
        // Free-text fallback: a distinctive phrase unique to Psalm 23:1. ("shepherd"
        // alone appears in ~15 verses corpus-wide; a phrase pins the one we assert.)
        let hits = search_verses_text(&conn, "shepherd; I shall not want", 25).unwrap();
        assert!(
            hits.iter().any(|v| v.reference == "Psalms 23:1"),
            "text search should find Psalm 23:1 by its distinctive phrase"
        );
    }

    #[test]
    fn song_dedupe_replaces_sections_keeps_meta() {
        use crate::songs::ParsedSection;
        let conn = fresh_db();
        ensure_songs(&conn).unwrap();
        let sec = |t: &str, l: &str| ParsedSection {
            tag: t.into(),
            label: l.into(),
            lyrics: l.into(),
        };
        let id = import_song(
            &conn,
            "Way Maker",
            "Sinach",
            "",
            "E",
            Some(68),
            "2026-07-06",
            &[sec("1", "a")],
        )
        .unwrap();

        // Same title, any case, resolves to the existing id.
        assert_eq!(song_id_by_title(&conn, "way maker").unwrap(), Some(id));

        // Re-import replaces sections but preserves the metadata.
        replace_song_sections(&conn, id, &[sec("1", "x"), sec("2", "y")]).unwrap();
        let song = get_song(&conn, id).unwrap().unwrap();
        assert_eq!(song.author, "Sinach");
        assert_eq!(song.song_key, "E");
        assert_eq!(song.sections.len(), 2);
        assert_eq!(song.sections[1].lyrics, "y");
        assert_eq!(list_songs(&conn).unwrap().len(), 1, "no duplicate created");
    }

    #[test]
    fn clean_verse_strips_glosses_keeps_supplied_words() {
        // Marginal glosses dropped, whitespace collapsed.
        assert_eq!(
            clean_verse(
                "He maketh me to lie down in green pastures: he leadeth me beside the still waters. {green...: Heb. pastures of tender grass} {still...: Heb. waters of quietness}"
            ),
            "He maketh me to lie down in green pastures: he leadeth me beside the still waters."
        );
        // Supplied-word italics kept (braces removed).
        assert_eq!(
            clean_verse("And God saw the light, that {it was} good"),
            "And God saw the light, that it was good"
        );
        // A gloss mid-string doesn't leave a double space.
        assert_eq!(
            clean_verse("Let there be a firmament {firmament: Heb. expansion} here"),
            "Let there be a firmament here"
        );
        // "Or," alternative-reading notes are glosses too.
        assert!(!clean_verse("a word {Or, another} tail").contains("Or,"));
        // Plain verse untouched.
        assert_eq!(
            clean_verse("In the beginning God created"),
            "In the beginning God created"
        );
    }

    #[test]
    fn arrangements_round_trip() {
        use crate::songs::ParsedSection;
        let conn = fresh_db();
        ensure_songs(&conn).unwrap();
        let sec = |t: &str| ParsedSection {
            tag: t.into(),
            label: t.into(),
            lyrics: t.into(),
        };
        let id = import_song(
            &conn,
            "Great Are You Lord",
            "",
            "",
            "",
            None,
            "2026-07-06",
            &[sec("V1"), sec("C"), sec("V2")],
        )
        .unwrap();

        assert!(list_arrangements(&conn, id).unwrap().is_empty());

        // Create — repeats allowed (V1 C V2 C).
        let aid = save_arrangement(&conn, id, None, "Live", &[0, 1, 2, 1]).unwrap();
        let list = list_arrangements(&conn, id).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Live");
        assert_eq!(list[0].sequence, vec![0, 1, 2, 1]);

        // Update by id.
        save_arrangement(&conn, id, Some(aid), "Short", &[0, 1]).unwrap();
        let list = list_arrangements(&conn, id).unwrap();
        assert_eq!(list.len(), 1, "update, not insert");
        assert_eq!(list[0].name, "Short");
        assert_eq!(list[0].sequence, vec![0, 1]);

        // Delete.
        delete_arrangement(&conn, aid).unwrap();
        assert!(list_arrangements(&conn, id).unwrap().is_empty());
    }

    #[test]
    fn editing_a_song_propagates_to_plans() {
        use crate::songs::ParsedSection;
        let conn = fresh_db();
        ensure_service_plans(&conn).unwrap();
        ensure_songs(&conn).unwrap();
        let sec = |l: &str| ParsedSection {
            tag: "1".into(),
            label: "Slide 1".into(),
            lyrics: l.into(),
        };
        let sid = import_song(&conn, "Way Maker", "", "", "", None, "d", &[sec("old")]).unwrap();

        // Add the song as a cue in a plan (payload references the song id).
        let pid = create_plan(&conn, "Sunday", "d").unwrap();
        let payload = format!(
            r#"{{"song_id":{sid},"title":"Way Maker","sections":[{{"tag":"1","label":"Slide 1","lyrics":"old"}}]}}"#
        );
        add_plan_item(&conn, pid, "song", "Way Maker", &payload, None).unwrap();

        // Edit the song → propagate. The cue's snapshot updates.
        update_song(
            &conn,
            sid,
            "Way Maker",
            "",
            "",
            "",
            None,
            &[sec("new"), sec("extra")],
        )
        .unwrap();
        let n = sync_song_in_plans(&conn, sid, "Way Maker", &[sec("new"), sec("extra")]).unwrap();
        assert_eq!(n, 1);
        let item = &plan_items(&conn, pid).unwrap()[0];
        assert!(
            item.payload_json.contains("new"),
            "cue should carry the edited lyric"
        );
        assert!(
            item.payload_json.contains("extra"),
            "cue should carry the new slide"
        );
        assert!(!item.payload_json.contains("\"old\""), "old lyric gone");
    }

    #[test]
    fn app_settings_roundtrip_and_translations() {
        let conn = fresh_db();
        // Settings table exists + upsert works.
        assert!(get_setting(&conn, "active_translation").unwrap().is_none());
        set_setting(&conn, "active_translation", "1").unwrap();
        assert_eq!(
            get_setting(&conn, "active_translation").unwrap().as_deref(),
            Some("1")
        );
        set_setting(&conn, "active_translation", "2").unwrap();
        assert_eq!(
            get_setting(&conn, "active_translation").unwrap().as_deref(),
            Some("2")
        );
        // Corpus lists the bundled KJV translation, and lookup still resolves with
        // an active-translation preference set (falls back when that id is absent).
        let trs = list_translations(&conn).unwrap();
        assert!(trs.iter().any(|t| t.abbreviation == "KJV"));
        assert!(lookup_verse(&conn, "John", 3, 16).unwrap().is_some());
    }

    #[test]
    fn migrates_pre_console_active_db() {
        // Simulate a DB created BEFORE the console_active column existed (the
        // exact upgrade path a user's real dev DB takes on next launch): a
        // templates table with the OLD shape + some rows, then migrate.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE templates (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL,
                region_config_json TEXT NOT NULL, style_json TEXT NOT NULL
             );
             CREATE TABLE output_channels (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL, render_target TEXT NOT NULL
                    CHECK (render_target IN ('native_window','ndi_encode','network_client')),
                template_id INTEGER REFERENCES templates(id), display_target TEXT,
                status TEXT NOT NULL DEFAULT 'offline'
             );",
        )
        .unwrap();
        for i in 1..=6 {
            conn.execute(
                "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, '{}', '{}')",
                [format!("T{i}")],
            )
            .unwrap();
        }
        // The ALTER branch runs and activates the first 4.
        ensure_template_active(&conn).unwrap();
        assert_eq!(list_templates(&conn).unwrap().len(), 6);
        assert_eq!(list_active_templates(&conn).unwrap().len(), 4);
        // Idempotent: running again neither errors nor re-activates.
        ensure_template_active(&conn).unwrap();
        assert_eq!(list_active_templates(&conn).unwrap().len(), 4);
    }

    #[test]
    fn service_persistence_and_library_counts() {
        let conn = fresh_db();
        let sid = create_service(&conn, "2026-07-03", "Sunday Service").unwrap();
        let t1 = insert_transcript(
            &conn,
            sid,
            12.5,
            "for god so loved the world",
            "en",
            Some(0.9),
        )
        .unwrap();
        insert_transcript(&conn, sid, 40.0, "turn to romans eight", "en", None).unwrap();

        let john = lookup_verse(&conn, "John", 3, 16).unwrap().unwrap();
        insert_detection(&conn, t1, Some(john.id), "direct", 0.96, "auto", Some(13.0)).unwrap();
        insert_detection(&conn, t1, None, "semantic", 0.62, "auto", Some(41.0)).unwrap();
        // A manual override cue counts toward "overrides".
        insert_cue(&conn, sid, "manual_override", Some("John 3:16"), 13.0).unwrap();
        insert_cue(&conn, sid, "clear_screens", None, 60.0).unwrap();

        let services = list_services(&conn).unwrap();
        assert_eq!(services.len(), 1);
        let s = &services[0];
        assert_eq!(s.title, "Sunday Service");
        assert_eq!(s.verses, 2); // two fired detections
        assert_eq!(s.overrides, 1); // one manual_override cue (clear_screens not counted)
        assert!((s.duration_secs - 40.0).abs() < 1e-6); // max transcript timestamp

        assert_eq!(service_transcripts(&conn, sid).unwrap().len(), 2);
        let dets = service_detections(&conn, sid).unwrap();
        assert_eq!(dets.len(), 2);
        assert_eq!(dets[0].reference.as_deref(), Some("John 3:16"));
        assert_eq!(dets[1].reference, None); // out-of-library verse
    }

    #[test]
    fn fresh_db_has_one_active_default_profile() {
        let conn = fresh_db();
        let all = list_voice_profiles(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Default");
        assert!(all[0].language.is_none()); // auto-detect / code-switch
        let active = active_voice_profile(&conn).unwrap().unwrap();
        assert_eq!(active.id, all[0].id);
    }

    #[test]
    fn create_select_and_single_active_invariant() {
        let conn = fresh_db();
        let default_id = active_voice_profile(&conn).unwrap().unwrap().id;
        let pastor = create_voice_profile(&conn, "Pastor John", Some("yo")).unwrap();

        // Creating doesn't steal active.
        assert_eq!(active_voice_profile(&conn).unwrap().unwrap().id, default_id);

        set_active_profile(&conn, pastor).unwrap();
        let active = active_voice_profile(&conn).unwrap().unwrap();
        assert_eq!(active.id, pastor);
        assert_eq!(active.language.as_deref(), Some("yo"));
        // Exactly one active row.
        let n_active = list_voice_profiles(&conn)
            .unwrap()
            .iter()
            .filter(|p| p.is_active)
            .count();
        assert_eq!(n_active, 1);
    }

    #[test]
    fn threshold_calibration_persists_per_profile() {
        let conn = fresh_db();
        let id = active_voice_profile(&conn).unwrap().unwrap().id;
        save_profile_thresholds(&conn, id, 0.93, 0.55).unwrap();
        let p = active_voice_profile(&conn).unwrap().unwrap();
        assert!((p.auto_fire - 0.93).abs() < 1e-9);
        assert!((p.suggest - 0.55).abs() < 1e-9);
    }

    #[test]
    fn deleting_active_promotes_another_and_last_reseeds() {
        let conn = fresh_db();
        let default_id = active_voice_profile(&conn).unwrap().unwrap().id;
        let second = create_voice_profile(&conn, "Guest", None).unwrap();
        set_active_profile(&conn, second).unwrap();

        // Delete the active one → the remaining profile becomes active.
        delete_voice_profile(&conn, second).unwrap();
        let active = active_voice_profile(&conn).unwrap().unwrap();
        assert_eq!(active.id, default_id);

        // Delete the last one → a Default is re-seeded and made active.
        delete_voice_profile(&conn, default_id).unwrap();
        let all = list_voice_profiles(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert!(active_voice_profile(&conn).unwrap().is_some());
    }

    #[test]
    fn ensure_voice_profiles_is_idempotent() {
        let conn = fresh_db();
        ensure_voice_profiles(&conn).unwrap();
        ensure_voice_profiles(&conn).unwrap();
        assert_eq!(list_voice_profiles(&conn).unwrap().len(), 1);
    }

    #[test]
    fn channel_add_assign_display_and_delete() {
        let conn = fresh_db();
        let before = list_output_channels(&conn).unwrap().len();

        // Add a native-window channel.
        let id = add_channel(&conn, "Balcony", "native_window", 1).unwrap();
        let after = list_output_channels(&conn).unwrap();
        assert_eq!(after.len(), before + 1);
        let ch = after.iter().find(|c| c.id == id).unwrap();
        assert_eq!(ch.render_target, "native_window");
        assert!(ch.display_target.is_none());

        // Assign it to display index 1, then clear.
        set_channel_display(&conn, id, Some("1")).unwrap();
        let ch = list_output_channels(&conn)
            .unwrap()
            .into_iter()
            .find(|c| c.id == id)
            .unwrap();
        assert_eq!(ch.display_target.as_deref(), Some("1"));
        set_channel_display(&conn, id, None).unwrap();
        assert!(list_output_channels(&conn)
            .unwrap()
            .into_iter()
            .find(|c| c.id == id)
            .unwrap()
            .display_target
            .is_none());

        // Delete it.
        delete_channel(&conn, id).unwrap();
        assert_eq!(list_output_channels(&conn).unwrap().len(), before);
    }

    #[test]
    fn open_creates_and_seeds_a_real_file_db() {
        // Exercise the real file path (not in-memory): open() must create the
        // parent dir, apply the schema, seed once, and be idempotent on reopen.
        let dir = std::env::temp_dir().join(format!("relay-test-{}", std::process::id()));
        let file = dir.join("nested").join("relay.db");
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("RELAY_DB_PATH", &file);

        let count = {
            let conn = open().unwrap();
            verse_count(&conn).unwrap()
        };
        assert!(count > 31_000, "full corpus should be seeded");
        assert!(file.exists(), "db file should be created on disk");
        {
            // Reopen: not fresh, so no re-seed / no duplicate-key error.
            let conn = open().unwrap();
            assert_eq!(verse_count(&conn).unwrap(), count);
        }

        std::env::remove_var("RELAY_DB_PATH");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
