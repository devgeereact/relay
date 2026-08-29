//! Local SQLite: the single source of truth, on the operator's own machine.
//!
//! Relay is local-first — transcripts, verse text, templates and service history
//! never leave the device (CLAUDE.md). This module owns the connection lifecycle,
//! the per-OS file locations, and the forward-fill migrations; the actual queries
//! live in one submodule per aggregate.
//!
//! Split out of a single 2,700-line file. Everything is re-exported below, so
//! callers still say `db::list_plans(...)` and never need to know which file it
//! lives in — the split is for the people reading it, not for the call sites.

mod channels;
mod environments;
mod library;
mod plans;
mod profiles;
mod services;
mod settings;
mod songs;
mod templates;
mod verses;

pub use channels::*;
pub use environments::*;
pub use library::*;
pub use plans::*;
pub use profiles::*;
pub use services::*;
pub use settings::*;
pub use songs::*;
pub use templates::*;
pub use verses::*;

use rusqlite::{Connection, OptionalExtension};
use std::path::PathBuf;

// Migration + seed helpers, pulled from the aggregates they belong to.
use channels::seed_channels;
#[cfg(test)]
use serde_json::Value;
use templates::{
    ensure_lyrics_template, ensure_preset_templates, reset_builtin_templates, seed_templates,
};
#[cfg(test)]
use verses::clean_verse;
use verses::{rebuild_verses_fts, reimport_full_kjv, seed};

/// The canonical schema, baked into the binary at compile time.
const SCHEMA: &str = include_str!("../../../docs/data/schema.sql");

/// The schema version this build expects. Bump it and add a rung to
/// `run_migrations` when you change the schema.
pub const SCHEMA_VERSION: i64 = 2;

fn user_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}

fn set_user_version(conn: &Connection, v: i64) -> rusqlite::Result<()> {
    // PRAGMA doesn't take bound parameters.
    conn.execute_batch(&format!("PRAGMA user_version = {v};"))
}

/// The migration ladder, for every version ABOVE the baseline.
///
/// Add a rung here (and bump `SCHEMA_VERSION`) for each schema change. Rungs run
/// in order, only the ones newer than the DB's recorded version, exactly once.
fn run_migrations(conn: &Connection, from: i64) -> rusqlite::Result<()> {
    // v2: every detection records the text that caused it.
    //
    // THIS is where a schema change goes — not into `baseline_forward_fill`.
    // That function is the v0-ONLY path, and putting this rung there meant it ran
    // on brand-new databases and on nothing else: an existing install took the
    // `else` branch in `migrate` and never saw it. The column was present in
    // `schema.sql`, present in every test, and absent from the one database that
    // mattered — the operator's. Caught by launching the real app against the
    // real file, which no unit test was doing.
    if from < 2 {
        ensure_detection_evidence(conn)?;
    }
    Ok(())
}

/// Bring a pre-versioning database up to the baseline (v1).
///
/// These are the original forward-fills. They are **sniff-based** — they infer
/// what a database needs by counting rows and `LIKE`-matching JSON blobs, because
/// there was no version to ask. That worked, but it grew a new sniff per change
/// and re-ran all of them on every single boot, forever.
///
/// They are kept (old databases genuinely still need them) but now they are
/// BOUNDED: they run once, against a v0 database, and then the version is stamped
/// and they never run again. Everything after this point uses `run_migrations`,
/// which asks the database what version it is instead of guessing.
fn baseline_forward_fill(conn: &Connection) -> rusqlite::Result<()> {
    {
        // Forward-fill for DBs created before templates were seeded (Phase 8).
        // Idempotent: only seeds when the table is empty.
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))?;
        if n == 0 {
            seed_templates(conn)?;
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
                reset_builtin_templates(conn)?;
            }
            // Lyrics belong in the lower-third band, centred (ProPresenter's
            // "Lower 3rd Lyrics"). Forward-fill the built-in "Lower Third"
            // template from its old left-aligned default → centred. Only touches
            // the unedited built-in (still left + lowerThird), never a custom one.
            // ID-SCOPED, like its sibling check above. Matching on the NAME alone
            // would rewrite a template the OPERATOR made and happened to call
            // "Lower Third" — silently changing the look of their congregation's
            // screen during a migration they never asked for. Only the built-ins
            // (ids 1-4) are ours to fix.
            conn.execute(
                "UPDATE templates
                    SET region_config_json = '{\"regions\":[\"verse_text\",\"reference\"],\"align\":\"center\",\"lowerThird\":true,\"refFirst\":false}'
                  WHERE id IN (1,2,3,4)
                    AND name = 'Lower Third'
                    AND region_config_json LIKE '%\"align\":\"left\"%'
                    AND region_config_json LIKE '%\"lowerThird\":true%'",
                [],
            )?;
        }
        // Forward-fill default output channels for pre-existing DBs.
        let cn: i64 = conn.query_row("SELECT COUNT(*) FROM output_channels", [], |r| r.get(0))?;
        if cn == 0 {
            seed_channels(conn)?;
        }
        // Forward-fill the full Bible for DBs created with the old 15-verse seed.
        if verse_count(conn)? < 30_000 {
            reimport_full_kjv(conn)?;
        }
        // One-time re-clean: DBs imported before the gloss stripper baked the KJV
        // marginal notes ("... Heb. ...") into the verse text. Re-import to strip.
        let polluted: i64 = conn.query_row(
            "SELECT COUNT(*) FROM verses WHERE text LIKE '% Heb.%' OR text LIKE '%: Gr.%'",
            [],
            |r| r.get(0),
        )?;
        if polluted > 0 {
            reimport_full_kjv(conn)?;
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
            rebuild_verses_fts(conn)?;
        }
    }
    // Tables added after the original schema. Pure `CREATE TABLE IF NOT EXISTS` /
    // `ALTER … ADD COLUMN` — idempotent and cheap, and they also cover a v0 DB
    // that predates each table.
    ensure_tables(conn)?;
    // detections.status gained 'manual' — see the fn doc.
    ensure_manual_detection_status(conn)?;
    // `detections.heard_text` is deliberately NOT here. It is a v2 rung and lives
    // in `run_migrations`, which runs for a v0 database (via `from = 0 < 2`) AND
    // for an existing versioned one. This function only ever runs for v0, so a
    // schema change placed here reaches new installs and no others.
    //
    // The ORDER still holds either way: `ensure_manual_detection_status` rebuilds
    // the table from a hard-coded seven-column list, so it must run before the
    // column is added, and `run_migrations` is called after this returns.
    Ok(())
}

/// Additive table/column creation. Idempotent by construction.
fn ensure_tables(conn: &Connection) -> rusqlite::Result<()> {
    // app_settings FIRST: ensure_lyrics_template writes to it unconditionally
    // (INSERT/DELETE, not tolerant .ok() reads). On a pre-app_settings v0 DB the
    // reverse order hit `no such table: app_settings`, propagated out of migrate,
    // and panicked at every boot before the window was shown. Its own
    // CREATE TABLE IF NOT EXISTS must run before any writer touches it.
    ensure_app_settings(conn)?; // key/value settings
    ensure_voice_profiles(conn)?; // per-preacher accent + gate calibration
    ensure_template_active(conn)?; // console-active templates (max 4)
    ensure_lyrics_template(conn)?; // the song template — see templates.rs
    ensure_preset_templates(conn)?; // ready-to-use preset designs (additive, by name)
    ensure_service_plans(conn)?; // Planner
    ensure_songs(conn)?; // Lyrics
    ensure_saved_scripture(conn)?; // Library
    ensure_media(conn)?;
    ensure_announcements(conn)?;
    ensure_service_events(conn)?; // the service timeline + latency snapshots
    ensure_environment_profiles(conn)?; // a room, remembered
    Ok(())
}

/// Open (or create) the on-device database at the default per-OS data path,
/// applying the schema and dev seed on first creation, then migrating it up to
/// `SCHEMA_VERSION`.
///
/// Called once at startup (not on a live-service path), so surfacing a hard
/// error here is correct — a broken DB must fail loudly before a service, not
/// silently mid-sermon.
pub fn open() -> rusqlite::Result<Connection> {
    let path = default_db_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // A RESTORE HAPPENS HERE OR NOWHERE.
    //
    // Copying a file over a database that is open is how you get a corrupt database
    // AND a corrupt backup, so a restore is a REQUEST (a marker file) that is acted
    // on at the one moment the file is provably unused: before it is opened. The
    // database being replaced is copied aside first, so pressing "restore" is never
    // an irreversible gamble.
    if let Some(from) = crate::updates::take_restore(&path) {
        println!(
            "db: restored from a pre-update snapshot ({})",
            from.display()
        );
    }
    let fresh = !path.exists();
    let conn = Connection::open(&path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrate(&conn, fresh)?;
    Ok(conn)
}

/// Bring a connection to `SCHEMA_VERSION`. Split out of `open` so it is testable
/// against an in-memory DB without touching the real one on disk.
pub fn migrate(conn: &Connection, fresh: bool) -> rusqlite::Result<()> {
    if fresh {
        init_fresh(conn)?;
        set_user_version(conn, SCHEMA_VERSION)?;
        return Ok(());
    }
    let from = user_version(conn)?;
    if from == 0 {
        // Pre-versioning database: sniff it up to the baseline, once, ever.
        baseline_forward_fill(conn)?;
    } else {
        ensure_tables(conn)?;
    }
    if from < SCHEMA_VERSION {
        run_migrations(conn, from)?;
        set_user_version(conn, SCHEMA_VERSION)?;
    }
    Ok(())
}

/// The translation the operator is reading, for browse queries.
///
/// Falls back to the lowest translation id rather than erroring: a fresh install
/// has never chosen one, and a Library that refuses to open because nobody has
/// picked a Bible yet would be absurd.
pub fn active_translation_id(conn: &Connection) -> rusqlite::Result<i64> {
    if let Some(v) = get_setting(conn, "active_translation")? {
        if let Ok(id) = v.parse::<i64>() {
            return Ok(id);
        }
    }
    conn.query_row("SELECT MIN(id) FROM translations", [], |r| r.get(0))
}

/// The tables the Database Migration screen verifies, with the rung that owns
/// each one. Kept next to `ensure_tables` so adding a rung without adding it
/// here is an obvious omission rather than a silent one.
/// An entry is either a table (`"songs"`) or a column (`"templates.console_active"`).
/// Both are real schema objects and both are checked by asking SQLite.
///
/// The first version of this list was written from memory and named two objects
/// that do not exist — `media` (it is `media_assets`) and `template_active`
/// (it is a COLUMN, `templates.console_active`). The screen would have reported
/// them as applied regardless, which is precisely the failure being fixed.
pub const MIGRATION_TABLES: &[(&str, &str)] = &[
    ("Core tables", "detections"),
    ("Detection evidence", "detections.heard_text"),
    ("Service history", "services"),
    ("Transcripts", "transcripts"),
    ("Scripture", "verses"),
    ("Templates", "templates"),
    ("Console-active templates", "templates.console_active"),
    ("Output channels", "output_channels"),
    ("Voice profiles", "voice_profiles"),
    ("App settings", "app_settings"),
    ("Service plans", "service_plans"),
    ("Planner cues", "plan_items"),
    ("Plan sections", "plan_items.section_title"),
    ("Plan running times", "plan_items.duration_sec"),
    ("Songs", "songs"),
    ("Song arrangements", "song_arrangements"),
    ("Saved scripture", "saved_scripture"),
    ("Media", "media_assets"),
    ("Announcements", "announcements"),
    ("Service timeline", "service_events"),
    ("Latency history", "perf_samples"),
    ("Room profiles", "environment_profiles"),
];

/// Does a table — or a `table.column` — exist right now?
fn object_present(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    if let Some((table, column)) = name.split_once('.') {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
            [table, column],
            |r| r.get(0),
        )?;
        return Ok(n > 0);
    }
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// What the schema ACTUALLY looks like right now.
///
/// The Database Migration screen used to assert its rungs from a hard-coded list
/// — it drew six green ticks whether or not the tables were there, because the
/// only evidence it had was that the app had started. This asks the database.
///
/// Returns `(recorded_version, expected_version, [(label, table, present)])`.
#[allow(clippy::type_complexity)]
pub fn schema_report(
    conn: &Connection,
) -> rusqlite::Result<(i64, i64, Vec<(&'static str, &'static str, bool)>)> {
    let mut rows = Vec::with_capacity(MIGRATION_TABLES.len());
    for (label, object) in MIGRATION_TABLES {
        rows.push((*label, *object, object_present(conn, object)?));
    }
    Ok((user_version(conn)?, SCHEMA_VERSION, rows))
}

/// Did the `detections.status` rebuild (CLAUDE.md §25) actually land?
///
/// Read from the stored DDL, not from "the app booted". The scar this comes from
/// is a migration that left a `detections_new` scratch table behind and bricked
/// every subsequent boot — so this ALSO reports a leftover scratch table, which
/// is the fingerprint of that failure.
pub fn manual_status_report(conn: &Connection) -> rusqlite::Result<(bool, bool)> {
    let ddl: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'detections'",
            [],
            |r| r.get(0),
        )
        .ok();
    let applied = ddl.map(|d| d.contains("'manual'")).unwrap_or(false);
    let scratch: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'detections_new'",
        [],
        |r| r.get(0),
    )?;
    Ok((applied, scratch > 0))
}

/// Give every detection the TEXT THAT CAUSED IT.
///
/// ── The gap this closes ─────────────────────────────────────────────────────
///
/// A detection row recorded which verse fired, how confident, and when — but not
/// what the detector was reading. And `persist_fire` attaches the row to
/// `SessionState::last_transcript`, the most recent FINAL transcript, while
/// detection also runs on every partial STT hypothesis. Partials are never
/// persisted. So a fire is routinely stamped onto a final transcript from
/// minutes earlier that provably could not have produced it.
///
/// From the live service of 2026-07-26: nine `direct` auto-fires — Job 1:1,
/// Jude 1:1, John 1:1, 1 Samuel 2:1 and more — all attributed to
///
///     "I am not in the ward when the Lord to the children of Israel."
///
/// Replaying that sentence through `detect_direct` yields NOTHING. The text that
/// actually fired them was a partial, discarded the moment it was decoded. The
/// whole service was un-diagnosable: forty wrong verses reached a congregation
/// and the log could not say what any of them heard.
///
/// A nullable column, so it is additive and needs no table rebuild — and
/// therefore no scratch table to strand (contrast `ensure_manual_detection_status`
/// and CLAUDE.md §25). Idempotent: it asks SQLite whether the column is there.
fn ensure_detection_evidence(conn: &Connection) -> rusqlite::Result<()> {
    let present: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('detections') WHERE name = 'heard_text'",
        [],
        |r| r.get(0),
    )?;
    if present > 0 {
        return Ok(());
    }
    conn.execute_batch("ALTER TABLE detections ADD COLUMN heard_text TEXT;")
}

/// Widen `detections.status` to allow `'manual'`.
///
/// Operator-driven fires (an override, a confirmed suggestion, a next/back nav)
/// were previously written with `status = 'auto'` because that was the only value
/// the CHECK constraint permitted for a fired row — so the service history could
/// not tell an AI decision from a human one. The self-calibrating router learns
/// from exactly that distinction, so it was quietly training on a corrupted log.
///
/// SQLite cannot ALTER a CHECK, so this rebuilds the table. Idempotent: detected
/// by sniffing the stored DDL, and a no-op once the new constraint is in place.
fn ensure_manual_detection_status(conn: &Connection) -> rusqlite::Result<()> {
    let ddl: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='detections'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let Some(ddl) = ddl else { return Ok(()) };
    if ddl.contains("'manual'") {
        return Ok(()); // already migrated
    }
    // foreign_keys must be toggled OUTSIDE a transaction to take effect.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

    // `DROP TABLE IF EXISTS detections_new` first: a previous attempt that died
    // mid-rebuild can leave the scratch table behind, and a bare CREATE would then
    // fail with "table detections_new already exists" — on EVERY subsequent boot.
    // A migration that cannot be retried is a brick, and this one runs before the
    // window is even shown.
    let res = conn.execute_batch(
        "BEGIN;
         DROP TABLE IF EXISTS detections_new;
         CREATE TABLE detections_new (
             id            INTEGER PRIMARY KEY,
             transcript_id INTEGER NOT NULL REFERENCES transcripts(id),
             verse_id      INTEGER REFERENCES verses(id),
             method        TEXT NOT NULL CHECK (method IN ('direct', 'semantic')),
             confidence    REAL NOT NULL,
             status        TEXT NOT NULL CHECK (status IN ('auto', 'suggested', 'dismissed', 'manual')),
             fired_at      REAL
         );
         INSERT INTO detections_new (id, transcript_id, verse_id, method, confidence, status, fired_at)
             SELECT id, transcript_id, verse_id, method, confidence, status, fired_at FROM detections;
         DROP TABLE detections;
         ALTER TABLE detections_new RENAME TO detections;
         COMMIT;",
    );

    // ROLLBACK on failure — this was missing, and its absence was the nastiest part.
    //
    // `execute_batch` stops at the first failing statement, so a failure anywhere in
    // the batch left the transaction OPEN on this connection. The `PRAGMA
    // foreign_keys = ON` below then executed *inside* that open transaction, where the
    // pragma is a documented no-op. The Err propagated up to `open()`'s `expect` and
    // panicked the app at startup — with foreign keys off and a transaction dangling.
    //
    // Both cleanup statements are best-effort: if the COMMIT itself is what failed,
    // there may be no transaction left to roll back, and that is fine.
    if res.is_err() {
        let _ = conn.execute_batch("ROLLBACK;");
        let _ = conn.execute_batch("DROP TABLE IF EXISTS detections_new;");
    }

    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    res
}

/// Apply the full schema and seed a fresh connection. Public so tests (and any
/// future in-memory scratch DB) can build a ready-to-query database directly.
pub fn init_fresh(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)?;
    seed(conn)?;
    // Guarantee an active voice profile exists even on a bare in-memory DB.
    ensure_tables(conn)?;
    // Stamp it, so a brand-new DB is never mistaken for a v0 one and put through
    // the legacy sniff-based forward-fills it has no need of.
    set_user_version(conn, SCHEMA_VERSION)?;
    Ok(())
}

/// Relay's per-OS application-data directory (`…/com.relay.app`).
///
/// THE single source of truth for where Relay keeps its files — the SQLite DB,
/// imported media, and the STT models. Kept dependency-free deliberately:
/// standard app-data locations, no `dirs` crate needed.
///
/// Every caller must use this. `stt.rs` previously hand-rolled its own
/// macOS-only variant (`$HOME/Library/Application Support/…`), which meant that
/// on a packaged **Windows** build — a day-one platform (docs/DECISIONS.md) —
/// the STT model was never found and Relay silently ran with no speech
/// recognition at all. Don't re-derive this path anywhere else.
pub fn app_data_dir() -> PathBuf {
    app_data_root(std::env::consts::OS, |k| std::env::var(k).ok()).join("com.relay.app")
}

/// The OS app-data root, as a PURE function of the OS name and the environment.
///
/// Pure on purpose. The Windows path bug (STT silently dead on every packaged
/// Windows build) could not be caught by any test, on any machine, because the
/// behaviour was welded to `cfg!(target_os)` — so a Mac could only ever test the Mac
/// branch, and CI's Windows runner had no test to run. The bug was found by a human
/// reading the code, which is not a strategy.
///
/// Taking the OS and the environment as arguments means **every platform's behaviour
/// is testable from every platform**, including the ones nobody here owns.
fn app_data_root(os: &str, env: impl Fn(&str) -> Option<String>) -> PathBuf {
    match os {
        "macos" => env("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support"))
            .unwrap_or_else(|| PathBuf::from(".")),

        // Windows has no HOME — except when it does, and that is the trap. Git Bash,
        // MSYS2 and Cygwin all set HOME to a Unix-shaped path, so a Windows build that
        // reaches for HOME "because it works on my machine" writes the database and the
        // 148 MB STT model somewhere no packaged app will ever look. APPDATA is the
        // roaming app-data root; USERPROFILE is the fallback. HOME is never consulted.
        "windows" => env("APPDATA")
            .or_else(|| env("USERPROFILE").map(|p| format!("{p}\\AppData\\Roaming")))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".")),

        _ => env("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| env("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from(".")),
    }
}

/// The user's Downloads folder, if it exists. `None` → the caller should fall back
/// to app-data (never fail outright: exporting a service must not depend on the shape
/// of someone's home directory).
pub fn downloads_dir() -> Option<PathBuf> {
    downloads_root(std::env::consts::OS, |k| std::env::var(k).ok()).filter(|d| d.is_dir())
}

/// Pure, for the same reason as `app_data_root`.
fn downloads_root(os: &str, env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    // USERPROFILE FIRST on Windows. Git Bash sets HOME to a Unix-shaped path
    // (`/c/Users/Ada`), which is not a path Windows can open — so reaching for HOME
    // first means the export lands nowhere, or in a directory the user cannot find.
    let home = if os == "windows" {
        env("USERPROFILE").or_else(|| env("HOME"))
    } else {
        env("HOME")
    }?;
    Some(PathBuf::from(home).join("Downloads"))
}

/// Resolve the default database file path per OS, honoring a RELAY_DB_PATH
/// override (handy for tests and dev).
pub fn db_path() -> PathBuf {
    default_db_path()
}

fn default_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("RELAY_DB_PATH") {
        return PathBuf::from(p);
    }
    app_data_dir().join("relay.db")
}

/// Directory where imported media/document files are stored (next to the DB).
pub fn media_dir() -> PathBuf {
    default_db_path()
        .parent()
        .map(|p| p.join("media"))
        .unwrap_or_else(|| PathBuf::from("media"))
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

    // ── The Database Migration screen's evidence ───────────────────────────
    //
    // These guard the thing that screen got wrong before it was made real: it
    // reported "already applied" from a hard-coded list, so it drew green ticks
    // whether or not the tables existed.

    #[test]
    fn schema_report_finds_every_table_on_a_fresh_database() {
        let conn = fresh_db();
        let (_, expected, rows) = schema_report(&conn).unwrap();
        assert_eq!(expected, SCHEMA_VERSION);
        let missing: Vec<_> = rows
            .iter()
            .filter(|(_, _, present)| !present)
            .map(|(_, t, _)| *t)
            .collect();
        assert!(
            missing.is_empty(),
            "MIGRATION_TABLES names objects a fresh database does not create: {missing:?}"
        );
    }

    #[test]
    fn schema_report_notices_a_missing_column() {
        // `templates.console_active` is a COLUMN rung, not a table one. Naming it
        // as a table (the original bug) made it unconditionally "present".
        let conn = fresh_db();
        assert!(object_present(&conn, "templates.console_active").unwrap());
        assert!(!object_present(&conn, "templates.no_such_column").unwrap());
        assert!(!object_present(&conn, "no_such_table.anything").unwrap());
    }

    #[test]
    fn schema_report_notices_a_missing_table() {
        // The bug: the screen ticks a row it never checked. Drop a real table and
        // the report must say so — if this passes with the table gone, the screen
        // is decorative again.
        let conn = fresh_db();
        conn.execute_batch("DROP TABLE announcements;").unwrap();
        let (_, _, rows) = schema_report(&conn).unwrap();
        let row = rows.iter().find(|(_, t, _)| *t == "announcements").unwrap();
        assert!(!row.2, "a dropped table still reported as present");
    }

    #[test]
    fn manual_status_report_reads_the_ddl_not_the_boot() {
        let conn = fresh_db();
        let (applied, scratch) = manual_status_report(&conn).unwrap();
        assert!(applied, "fresh schema should already allow 'manual'");
        assert!(!scratch, "fresh database has no scratch table");
    }

    #[test]
    fn manual_status_report_spots_an_unmigrated_database() {
        let conn = db_with_old_detections();
        let (applied, _) = manual_status_report(&conn).unwrap();
        assert!(!applied, "the old CHECK constraint reported as migrated");
    }

    #[test]
    fn manual_status_report_spots_the_leftover_scratch_table() {
        // CLAUDE.md §25: a `detections_new` left behind is the fingerprint of the
        // failure that bricked every subsequent boot. The screen must surface it.
        let conn = fresh_db();
        conn.execute_batch("CREATE TABLE detections_new (id INTEGER PRIMARY KEY);")
            .unwrap();
        let (_, scratch) = manual_status_report(&conn).unwrap();
        assert!(scratch, "a leftover detections_new went unreported");
    }

    #[test]
    fn a_v0_database_without_app_settings_migrates_without_panicking() {
        // The bug: `ensure_lyrics_template` writes to `app_settings`
        // (INSERT + an unconditional DELETE) but ran BEFORE `ensure_app_settings`
        // created it. A pre-`app_settings` v0 DB — exactly the case the v0 path
        // exists to fix — hit `no such table: app_settings`, which propagated out
        // of `migrate` and panicked at startup on EVERY boot, forever, before the
        // window was shown. Reorder the guarantee (app_settings first) and the v0
        // path completes.
        //
        // This test fails if the ordering regresses: drop the table and force the
        // v0 baseline path, which is what a real old install triggers.
        let conn = fresh_db();
        conn.execute_batch("DROP TABLE app_settings;").unwrap();
        set_user_version(&conn, 0).unwrap();

        // Must NOT error. Before the fix this returned Err(no such table) and the
        // real app turned that Err into a boot panic.
        migrate(&conn, false).expect("v0 migration must not fail on a missing app_settings");

        // And the table the writer needed is now present.
        assert!(
            object_present(&conn, "app_settings").unwrap(),
            "app_settings should be recreated by the v0 migration"
        );
    }

    /// A database with the OLD `detections` table — the one whose CHECK constraint
    /// could not express `'manual'`, so a human's fire was logged as the AI's.
    fn db_with_old_detections() -> Connection {
        let conn = fresh_db();
        conn.execute_batch(
            "DROP TABLE detections;
             CREATE TABLE detections (
                 id            INTEGER PRIMARY KEY,
                 transcript_id INTEGER NOT NULL REFERENCES transcripts(id),
                 verse_id      INTEGER REFERENCES verses(id),
                 method        TEXT NOT NULL CHECK (method IN ('direct', 'semantic')),
                 confidence    REAL NOT NULL,
                 status        TEXT NOT NULL CHECK (status IN ('auto', 'suggested', 'dismissed')),
                 fired_at      REAL
             );",
        )
        .unwrap();
        conn
    }

    fn manual_is_allowed(conn: &Connection) -> bool {
        let ddl: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='detections'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        ddl.contains("'manual'")
    }

    #[test]
    fn the_detections_rebuild_widens_the_check_constraint() {
        let conn = db_with_old_detections();
        assert!(!manual_is_allowed(&conn));
        ensure_manual_detection_status(&conn).unwrap();
        assert!(manual_is_allowed(&conn));
    }

    #[test]
    fn the_detections_rebuild_is_idempotent() {
        let conn = db_with_old_detections();
        ensure_manual_detection_status(&conn).unwrap();
        ensure_manual_detection_status(&conn).unwrap(); // must be a no-op, not an error
        assert!(manual_is_allowed(&conn));
    }

    /// THE BRICK. A previous attempt that died mid-rebuild leaves the scratch table
    /// behind. A bare `CREATE TABLE detections_new` then fails with "already exists"
    /// — forever, on every subsequent boot, before the window is even shown. A
    /// migration that cannot be retried is not a migration; it is a brick.
    #[test]
    fn a_leftover_scratch_table_does_not_brick_every_future_boot() {
        let conn = db_with_old_detections();
        conn.execute_batch("CREATE TABLE detections_new (id INTEGER PRIMARY KEY);")
            .unwrap();

        ensure_manual_detection_status(&conn)
            .expect("a crashed previous attempt must be retryable");

        assert!(manual_is_allowed(&conn));
    }

    /// Foreign keys must be back ON when the migration returns, and no transaction
    /// may be left dangling. The pragma is a no-op inside an open transaction, so a
    /// migration that failed without rolling back used to return with FKs silently
    /// OFF — every later write in that session unchecked.
    #[test]
    fn foreign_keys_are_on_again_afterwards_and_no_transaction_is_left_open() {
        let conn = db_with_old_detections();
        ensure_manual_detection_status(&conn).unwrap();

        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1, "foreign keys were left OFF");

        // If a transaction were still open, BEGIN would fail with "cannot start a
        // transaction within a transaction".
        conn.execute_batch("BEGIN; COMMIT;")
            .expect("a transaction was left dangling");
    }

    /// The whole point of the rebuild: a human's decision must be recordable as a
    /// human's. The self-calibrating router learns from this column.
    #[test]
    fn a_manual_fire_can_actually_be_written_after_the_migration() {
        let conn = db_with_old_detections();
        ensure_manual_detection_status(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO services (id, date, title) VALUES (1, '2026-07-12', 'Sunday');
             INSERT INTO transcripts (id, service_id, timestamp, text, language)
                 VALUES (1, 1, 0.0, 'john three sixteen', 'en');",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO detections (transcript_id, verse_id, method, confidence, status, fired_at)
             VALUES (1, NULL, 'direct', 1.0, 'manual', 0.0)",
            [],
        )
        .expect("'manual' must be a legal status after the migration");
    }

    #[test]
    fn seeds_full_kjv() {
        let conn = fresh_db();
        // Full KJV is 31,102 verses; the bundled file has 31,100.
        assert!(verse_count(&conn).unwrap() > 31_000);
    }

    #[test]
    fn seeds_the_builtin_templates() {
        // Five now, not four: "Worship Lyrics" was added because every previous
        // built-in was scripture-shaped (a reference region and small type), and
        // lyrics rendered through one put the song title where the words should
        // be. See templates.rs.
        let conn = fresh_db();
        let ts = list_templates(&conn).unwrap();
        // Five built-ins plus the ready-to-use presets, all seeded on a fresh DB.
        assert_eq!(ts.len(), 5 + templates::preset_template_count());
        assert!(
            ts.iter().any(|t| t.name == "Worship Lyrics"),
            "the lyrics template is missing from the seed"
        );
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
        let seeded = 5 + templates::preset_template_count() as i64;
        let id = upsert_template(&conn, &t).unwrap();
        // The new row's id follows every seeded template (built-ins + presets).
        assert_eq!(id, seeded + 1);
        assert_eq!(list_templates(&conn).unwrap().len() as i64, seeded + 1);
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

    // ── Plan sections and running time ────────────────────────────────────
    //
    // Sections are derived from the cue order (a cue with a `section_title`
    // begins one), so these guard the places that derivation can silently rot.

    #[test]
    fn ensure_service_plans_is_retryable() {
        // It runs on EVERY boot. A bare ALTER TABLE ADD COLUMN would error with
        // "duplicate column name" the second time and panic the app at startup —
        // the §25 failure mode, one layer down.
        let conn = Connection::open_in_memory().unwrap();
        ensure_service_plans(&conn).unwrap();
        ensure_service_plans(&conn).unwrap();
        ensure_service_plans(&conn).unwrap();

        let pid = create_plan(&conn, "Sunday", "2026-07-05").unwrap();
        let id = add_plan_item(&conn, pid, "song", "Way Maker", "{}", None).unwrap();
        set_plan_section(&conn, id, "Welcome & Worship").unwrap();
        assert_eq!(
            plan_items(&conn, pid).unwrap()[0].section_title,
            "Welcome & Worship"
        );
    }

    #[test]
    fn a_plan_predating_sections_gains_the_columns() {
        // A DB created before sections existed: the old CREATE TABLE, then the
        // real migration on top. This is what every existing install does on the
        // first launch after updating.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE service_plans (
                id INTEGER PRIMARY KEY, title TEXT NOT NULL,
                plan_date TEXT NOT NULL DEFAULT '', notes TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE plan_items (
                id INTEGER PRIMARY KEY, plan_id INTEGER NOT NULL,
                position INTEGER NOT NULL, cue_type TEXT NOT NULL,
                label TEXT NOT NULL, payload_json TEXT NOT NULL DEFAULT '{}',
                template_id INTEGER
             );
             INSERT INTO service_plans (id, title) VALUES (1, 'Old Plan');
             INSERT INTO plan_items (plan_id, position, cue_type, label)
                VALUES (1, 0, 'scripture', 'John 3:16');",
        )
        .unwrap();

        ensure_service_plans(&conn).unwrap();

        // The pre-existing cue survives and defaults to untimed / no section.
        let items = plan_items(&conn, 1).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "John 3:16");
        assert_eq!(items[0].section_title, "");
        assert_eq!(items[0].duration_sec, 0);
    }

    #[test]
    fn deleting_a_sections_first_cue_hands_the_heading_down() {
        // Otherwise the section dissolves and its surviving cues get silently
        // absorbed by the section above it.
        let conn = fresh_db();
        ensure_service_plans(&conn).unwrap();
        let pid = create_plan(&conn, "Sunday", "2026-07-05").unwrap();
        let a = add_plan_item(&conn, pid, "song", "Opener", "{}", None).unwrap();
        let b = add_plan_item(&conn, pid, "song", "Second", "{}", None).unwrap();
        set_plan_section(&conn, a, "Worship").unwrap();

        remove_plan_item(&conn, a).unwrap();

        let items = plan_items(&conn, pid).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, b);
        assert_eq!(items[0].section_title, "Worship");
    }

    #[test]
    fn deleting_a_cue_never_overwrites_the_next_sections_heading() {
        // The inheritance above must not clobber a heading that already exists —
        // deleting the last cue of "Worship" would otherwise rename "Sermon".
        let conn = fresh_db();
        ensure_service_plans(&conn).unwrap();
        let pid = create_plan(&conn, "Sunday", "2026-07-05").unwrap();
        let a = add_plan_item(&conn, pid, "song", "Opener", "{}", None).unwrap();
        let b = add_plan_item(&conn, pid, "sermon", "Message", "{}", None).unwrap();
        set_plan_section(&conn, a, "Worship").unwrap();
        set_plan_section(&conn, b, "Sermon").unwrap();

        remove_plan_item(&conn, a).unwrap();

        assert_eq!(plan_items(&conn, pid).unwrap()[0].section_title, "Sermon");
    }

    #[test]
    fn duration_is_clamped_and_duplicated_with_the_plan() {
        let conn = fresh_db();
        ensure_service_plans(&conn).unwrap();
        let pid = create_plan(&conn, "Sunday", "2026-07-05").unwrap();
        let a = add_plan_item(&conn, pid, "song", "Way Maker", "{}", None).unwrap();
        set_plan_section(&conn, a, "Worship").unwrap();
        set_plan_duration(&conn, a, 360).unwrap();
        // A negative running time is worse than none.
        let b = add_plan_item(&conn, pid, "media", "Loop", "{}", None).unwrap();
        set_plan_duration(&conn, b, -5).unwrap();
        assert_eq!(plan_items(&conn, pid).unwrap()[1].duration_sec, 0);

        // Duplicating last week's order must carry headings and times across.
        let copy = duplicate_plan(&conn, pid, "Next Sunday", "2026-07-12").unwrap();
        let dup = plan_items(&conn, copy).unwrap();
        assert_eq!(dup[0].section_title, "Worship");
        assert_eq!(dup[0].duration_sec, 360);
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
        // Counted with SQL: the `console_active` column outlived the console
        // Output grid it was built for (superseded by per-channel templates), so
        // there is no longer a query helper to ask through — but the migration
        // must still be idempotent for every already-installed database.
        fn active_count(conn: &Connection) -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM templates WHERE console_active = 1",
                [],
                |r| r.get(0),
            )
            .unwrap()
        }
        ensure_template_active(&conn).unwrap();
        assert_eq!(list_templates(&conn).unwrap().len(), 6);
        assert_eq!(active_count(&conn), 4);
        // Idempotent: running again neither errors nor re-activates.
        ensure_template_active(&conn).unwrap();
        assert_eq!(active_count(&conn), 4);
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
        insert_detection(
            &conn,
            t1,
            Some(john.id),
            "direct",
            0.96,
            "auto",
            Some(13.0),
            None,
        )
        .unwrap();
        insert_detection(&conn, t1, None, "semantic", 0.62, "auto", Some(41.0), None).unwrap();
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

    /// The app-data dir must resolve on EVERY platform, and must never be the
    /// bare fallback (".") on a normally-configured machine. A macOS-only path
    /// here is what silently killed STT on packaged Windows builds.
    #[test]
    fn app_data_dir_resolves_per_os_and_is_never_bare() {
        let d = app_data_dir();
        assert!(
            d.ends_with("com.relay.app"),
            "app data dir must be namespaced: {d:?}"
        );
        // On a real machine (CI included) an app-data root always exists, so the
        // "." fallback means we failed to resolve — the exact silent failure mode.
        assert_ne!(
            d,
            PathBuf::from(".").join("com.relay.app"),
            "fell back to CWD — the per-OS lookup did not resolve"
        );
        // The DB and the STT models must live under the SAME root. They drifted
        // apart once, and only the models branch was macOS-only.
        assert!(default_db_path().starts_with(&d));
        assert!(crate::stt::model_install_dir().starts_with(&d));
    }

    /// A human putting a verse on screen must be recordable AS a human decision.
    /// The self-calibrating router trains on this column, so if operator
    /// overrides land here as 'auto' it is learning from a falsified log.
    #[test]
    fn detections_accept_manual_status() {
        let conn = Connection::open_in_memory().unwrap();
        init_fresh(&conn).unwrap();
        let sid = create_service(&conn, "Test", "2026-07-11").unwrap();
        let tid = insert_transcript(&conn, sid, 0.0, "john three sixteen", "en", None).unwrap();

        for status in ["auto", "suggested", "dismissed", "manual"] {
            insert_detection(&conn, tid, None, "direct", 1.0, status, Some(0.0), None)
                .unwrap_or_else(|e| panic!("status {status:?} rejected: {e}"));
        }
        // And a bogus one is still refused — the constraint was widened, not dropped.
        assert!(
            insert_detection(&conn, tid, None, "direct", 1.0, "nonsense", Some(0.0), None).is_err()
        );
    }

    /// Exercises the ACTUAL rebuild path, from a real pre-migration database —
    /// an existing install with a service already recorded in it. A migration
    /// that is only ever tested against a fresh schema is not tested at all.
    #[test]
    fn manual_status_migration_upgrades_an_old_db_without_losing_rows() {
        let conn = Connection::open_in_memory().unwrap();
        init_fresh(&conn).unwrap();

        // Reconstruct the OLD detections table (no 'manual' in the CHECK), as it
        // exists in every database created before this change.
        conn.execute_batch(
            "DROP TABLE detections;
             CREATE TABLE detections (
                 id            INTEGER PRIMARY KEY,
                 transcript_id INTEGER NOT NULL REFERENCES transcripts(id),
                 verse_id      INTEGER REFERENCES verses(id),
                 method        TEXT NOT NULL CHECK (method IN ('direct', 'semantic')),
                 confidence    REAL NOT NULL,
                 status        TEXT NOT NULL CHECK (status IN ('auto', 'suggested', 'dismissed')),
                 fired_at      REAL
             );",
        )
        .unwrap();

        let sid = create_service(&conn, "Last Sunday", "2026-07-05").unwrap();
        let tid = insert_transcript(&conn, sid, 0.0, "john three sixteen", "en", None).unwrap();
        // RAW SQL, deliberately: the table above is genuinely pre-migration, so it
        // has no `heard_text` and the typed helper cannot address it. Writing these
        // rows the legacy way is what keeps this test an honest test of an OLD
        // database rather than of a freshly-built one wearing an old name.
        let legacy = |method: &str, conf: f64, status: &str, fired: Option<f64>| {
            conn.execute(
                "INSERT INTO detections (transcript_id, verse_id, method, confidence, status, fired_at)
                 VALUES (?1, NULL, ?2, ?3, ?4, ?5)",
                (tid, method, conf, status, fired),
            )
        };
        legacy("direct", 0.9, "auto", Some(1.0)).unwrap();
        legacy("semantic", 0.4, "suggested", None).unwrap();
        // Pre-migration, 'manual' is rejected — this is the bug being fixed.
        assert!(legacy("direct", 1.0, "manual", Some(2.0)).is_err());

        ensure_manual_detection_status(&conn).unwrap();
        // Both rungs run, in this order, on every real boot (see `migrate`).
        ensure_detection_evidence(&conn).unwrap();

        // The operator's history survived the rebuild intact...
        let rows: Vec<(String, f64)> = conn
            .prepare("SELECT status, confidence FROM detections ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "auto");
        assert_eq!(rows[1].0, "suggested");

        // ...and 'manual' is now accepted.
        insert_detection(&conn, tid, None, "direct", 1.0, "manual", Some(2.0), None).unwrap();
        // Re-running is a safe no-op.
        ensure_manual_detection_status(&conn).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM detections", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 3);
    }

    /// THE TEST THAT WAS MISSING, and the reason a shipped build reached the
    /// operator's machine with the migration silently not applied.
    ///
    /// `migrate` has two arms: a v0 database goes through `baseline_forward_fill`,
    /// an already-versioned one goes through `ensure_tables` and then the
    /// `run_migrations` ladder. Every test for `heard_text` either called
    /// `ensure_detection_evidence` directly or used `init_fresh`, which builds
    /// from `schema.sql` — where the column had also been added. So the column
    /// existed in every test and in no existing install.
    ///
    /// This drives the REAL boot path against an EXISTING database, which is the
    /// only thing that could have failed and the only thing nothing was doing.
    #[test]
    fn migrating_an_existing_versioned_db_adds_the_evidence_column() {
        let conn = Connection::open_in_memory().unwrap();
        init_fresh(&conn).unwrap();

        // Reconstruct an installed v1 database: the column does not exist yet and
        // the file is stamped at the previous schema version.
        conn.execute_batch(
            "DROP TABLE detections;
             CREATE TABLE detections (
                 id            INTEGER PRIMARY KEY,
                 transcript_id INTEGER NOT NULL REFERENCES transcripts(id),
                 verse_id      INTEGER REFERENCES verses(id),
                 method        TEXT NOT NULL CHECK (method IN ('direct', 'semantic')),
                 confidence    REAL NOT NULL,
                 status        TEXT NOT NULL CHECK (status IN ('auto', 'suggested', 'dismissed', 'manual')),
                 fired_at      REAL
             );",
        )
        .unwrap();
        set_user_version(&conn, 1).unwrap();
        assert!(
            !object_present(&conn, "detections.heard_text").unwrap(),
            "precondition: the v1 database must not have the column"
        );

        // The real boot path, on a NON-fresh database.
        migrate(&conn, false).unwrap();

        assert!(
            object_present(&conn, "detections.heard_text").unwrap(),
            "an existing install booted without gaining detections.heard_text"
        );
        assert_eq!(
            user_version(&conn).unwrap(),
            SCHEMA_VERSION,
            "version stamp"
        );

        // And booting again is a no-op, not an error.
        migrate(&conn, false).unwrap();
        assert!(object_present(&conn, "detections.heard_text").unwrap());
    }

    /// Every object the Database Migration screen claims must actually be
    /// reachable by the migration path an existing install takes. A rung added to
    /// the v0-only branch passes every other test in this file and ships broken.
    #[test]
    fn every_advertised_schema_object_exists_after_migrating_a_v1_db() {
        let conn = Connection::open_in_memory().unwrap();
        init_fresh(&conn).unwrap();
        set_user_version(&conn, 1).unwrap();
        migrate(&conn, false).unwrap();
        for (label, object) in MIGRATION_TABLES {
            assert!(
                object_present(&conn, object).unwrap(),
                "{label}: {object} missing after migrating an existing install"
            );
        }
    }

    /// A detection must be able to say WHAT IT HEARD.
    ///
    /// `transcript_id` cannot: detection runs on partial STT hypotheses that are
    /// never persisted, so a fire is stamped onto whichever FINAL transcript was
    /// most recent — in a live service, one from minutes earlier that does not
    /// contain the reference at all. Nine wrong verses reached a congregation and
    /// the log could not explain a single one.
    #[test]
    fn a_detection_records_the_text_that_caused_it() {
        let conn = Connection::open_in_memory().unwrap();
        init_fresh(&conn).unwrap();
        let s = create_service(&conn, "2026-07-26", "Sunday Service").unwrap();
        let tid =
            insert_transcript(&conn, s, 0.0, "unrelated final transcript", "en", None).unwrap();

        insert_detection(
            &conn,
            tid,
            None,
            "direct",
            0.83,
            "auto",
            Some(1.0),
            Some("and the numbers two three"),
        )
        .unwrap();

        let heard: Option<String> = conn
            .query_row("SELECT heard_text FROM detections", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            heard.as_deref(),
            Some("and the numbers two three"),
            "the evidence did not survive the insert"
        );
    }

    /// The evidence column is added AFTER the status rebuild, and the rebuild
    /// copies a hard-coded column list. Get that order wrong and `heard_text`
    /// exists, then silently does not. Running the whole migration twice must
    /// leave it — and its contents — intact.
    #[test]
    fn the_evidence_column_survives_a_re_run_of_every_migration() {
        let conn = Connection::open_in_memory().unwrap();
        init_fresh(&conn).unwrap();
        let s = create_service(&conn, "2026-07-26", "Sunday Service").unwrap();
        let tid = insert_transcript(&conn, s, 0.0, "t", "en", None).unwrap();
        insert_detection(
            &conn,
            tid,
            None,
            "direct",
            0.9,
            "auto",
            Some(1.0),
            Some("psalm 23"),
        )
        .unwrap();

        for _ in 0..3 {
            ensure_manual_detection_status(&conn).unwrap();
            ensure_detection_evidence(&conn).unwrap();
        }

        let heard: Option<String> = conn
            .query_row("SELECT heard_text FROM detections", [], |r| r.get(0))
            .unwrap();
        assert_eq!(heard.as_deref(), Some("psalm 23"));
        // And no scratch table was stranded (CLAUDE.md §25).
        let (_, scratch) = manual_status_report(&conn).unwrap();
        assert!(!scratch, "a scratch table was left behind");
    }

    /// A DIAL MOVE MUST LEAVE THE PROFILE SELF-CONSISTENT.
    ///
    /// `sensitivity` and `auto_fire`/`suggest` are not independent: the dial is
    /// the baseline, and `Thresholds::from_sensitivity` is the one mapping
    /// between them. Persisting the thresholds without the dial (or the other way
    /// round) records a state the router was never in — a live service left
    /// `auto_fire = 0.832` sitting beside `sensitivity = 50`, whose mapping is
    /// 0.50, and the stale value was reloaded at next launch, silently undoing
    /// the operator's change.
    #[test]
    fn saving_the_sensitivity_dial_keeps_the_profile_self_consistent() {
        let conn = Connection::open_in_memory().unwrap();
        init_fresh(&conn).unwrap();
        let p = active_voice_profile(&conn)
            .unwrap()
            .expect("active profile");

        // The self-calibration has ratcheted the gate up over a service...
        save_profile_thresholds(&conn, p.id, 0.832, 0.60).unwrap();

        // ...and now the operator deliberately moves the dial to 20.
        let t = crate::router::Thresholds::from_sensitivity(20);
        save_profile_sensitivity(&conn, p.id, 20, t.auto_fire as f64, t.suggest as f64).unwrap();

        let after = active_voice_profile(&conn).unwrap().unwrap();
        assert_eq!(after.sensitivity, 20, "the dial did not stick");
        let implied = crate::router::Thresholds::from_sensitivity(after.sensitivity as u8);
        assert!(
            (after.auto_fire - implied.auto_fire as f64).abs() < 1e-6,
            "profile says sensitivity {} but auto_fire {} — that maps to {}",
            after.sensitivity,
            after.auto_fire,
            implied.auto_fire
        );
        assert!((after.suggest - implied.suggest as f64).abs() < 1e-6);
        // The learned value is gone, deliberately: the operator overruled it.
        assert!(after.auto_fire < 0.832);
    }

    /// A fresh DB is stamped at the current version, so it is never mistaken for
    /// a pre-versioning one and dragged through the legacy sniffs.
    /// The DB's profile defaults MUST equal the router's baseline. They were
    /// hardcoded separately in five places in db/profiles.rs, which is exactly how
    /// the original calibration bug happened: two copies of the baseline drifted
    /// apart and a profile save snapped between them. This fails the moment a
    /// sixth copy appears.
    #[test]
    fn the_db_profile_defaults_are_the_router_baseline() {
        let conn = Connection::open_in_memory().unwrap();
        init_fresh(&conn).unwrap();
        let p = active_voice_profile(&conn)
            .unwrap()
            .expect("an active profile");
        let base = crate::router::Thresholds::default();

        assert!((p.auto_fire - base.auto_fire as f64).abs() < 1e-6, "{p:?}");
        assert!((p.suggest - base.suggest as f64).abs() < 1e-6, "{p:?}");
        assert_eq!(p.sensitivity, crate::router::DEFAULT_SENSITIVITY as i64);

        // ...and so does a newly created one.
        let id = create_voice_profile(&conn, "Pastor Ade", Some("yo")).unwrap();
        let made = list_voice_profiles(&conn)
            .unwrap()
            .into_iter()
            .find(|x| x.id == id)
            .unwrap();
        assert!(
            (made.auto_fire - base.auto_fire as f64).abs() < 1e-6,
            "{made:?}"
        );
        assert!(
            (made.suggest - base.suggest as f64).abs() < 1e-6,
            "{made:?}"
        );
    }

    /// A cue dragged after a delete must actually MOVE.
    ///
    /// Deleting a cue leaves a gap in the positions (0, 1, 3). move_plan_item used
    /// to look for a neighbour at exactly position±1, find nothing, and silently do
    /// nothing — the operator drags a cue and it doesn't budge, with no error.
    #[test]
    fn a_cue_still_moves_after_a_delete_leaves_a_position_gap() {
        let conn = Connection::open_in_memory().unwrap();
        init_fresh(&conn).unwrap();
        let plan = create_plan(&conn, "Sunday", "2026-07-12").unwrap();
        let a = add_plan_item(&conn, plan, "announce", "A", "{}", None).unwrap();
        let b = add_plan_item(&conn, plan, "announce", "B", "{}", None).unwrap();
        let c = add_plan_item(&conn, plan, "announce", "C", "{}", None).unwrap();
        let d = add_plan_item(&conn, plan, "announce", "D", "{}", None).unwrap();

        // Delete C -> positions are now 0, 1, 3. A gap.
        remove_plan_item(&conn, c).unwrap();

        // Move D up. Under the old arithmetic this looked for position 2 and gave up.
        move_plan_item(&conn, d, -1).unwrap();

        let order: Vec<String> = plan_items(&conn, plan)
            .unwrap()
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(
            order,
            ["A", "D", "B"],
            "the cue did not move across the gap"
        );
        let _ = (a, b);
    }

    /// Deleting media must not leave a cue that looks fine and explodes when fired.
    #[test]
    fn deleting_media_removes_the_plan_cues_that_pointed_at_it() {
        let conn = Connection::open_in_memory().unwrap();
        init_fresh(&conn).unwrap();
        let plan = create_plan(&conn, "Sunday", "2026-07-12").unwrap();
        let m = insert_media(&conn, "image", "slide.png", "2026-07-12").unwrap();
        add_plan_item(
            &conn,
            plan,
            "media",
            "slide.png",
            &format!(r#"{{"media_id":{m},"kind":"image"}}"#),
            None,
        )
        .unwrap();
        add_plan_item(&conn, plan, "announce", "Notices", "{}", None).unwrap();
        assert_eq!(plan_items(&conn, plan).unwrap().len(), 2);

        delete_media(&conn, m).unwrap();

        let left = plan_items(&conn, plan).unwrap();
        assert_eq!(left.len(), 1, "the orphaned media cue survived the delete");
        assert_eq!(left[0].label, "Notices");
    }

    /// A song and its sections are ONE thing. A half-imported song is a song whose
    /// second chorus is missing — discovered mid-song, on a Sunday.
    #[test]
    fn importing_a_song_is_all_or_nothing() {
        use crate::songs::ParsedSection;
        let conn = Connection::open_in_memory().unwrap();
        init_fresh(&conn).unwrap();
        let sections = vec![
            ParsedSection {
                tag: "v1".into(),
                label: "Verse 1".into(),
                lyrics: "a".into(),
            },
            ParsedSection {
                tag: "c".into(),
                label: "Chorus".into(),
                lyrics: "b".into(),
            },
        ];
        let id = import_song(
            &conn,
            "Amazing Grace",
            "Newton",
            "",
            "",
            None,
            "2026-07-12",
            &sections,
        )
        .unwrap();
        let song = get_song(&conn, id).unwrap().expect("song");
        assert_eq!(song.sections.len(), 2, "sections were lost");
    }

    #[test]
    fn a_fresh_db_is_stamped_at_the_current_version() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn, true).unwrap();
        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    /// The point of the ladder: a pre-versioning (v0) database gets the legacy
    /// forward-fills ONCE, is stamped, and never sniffs again. Before this, every
    /// boot re-ran every sniff, forever, and each schema change added another.
    #[test]
    fn a_v0_db_is_migrated_once_and_then_left_alone() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        init_fresh(&conn).unwrap();
        // Pretend it predates versioning.
        set_user_version(&conn, 0).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 0);

        migrate(&conn, false).unwrap();
        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);

        // Data survived the baseline pass...
        assert!(verse_count(&conn).unwrap() > 31_000);
        assert!(!list_templates(&conn).unwrap().is_empty());

        // ...and re-opening is now a no-op that leaves everything intact.
        migrate(&conn, false).unwrap();
        migrate(&conn, false).unwrap();
        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
        assert!(verse_count(&conn).unwrap() > 31_000);
    }

    /// An operator's own edits must survive migration — the baseline pass is
    /// allowed to fix the built-ins, never to clobber the user's work.
    #[test]
    fn migrating_a_v0_db_preserves_the_operators_own_data() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        init_fresh(&conn).unwrap();
        let plan = create_plan(&conn, "Easter Sunday", "2026-04-05").unwrap();
        let song = import_song(
            &conn,
            "Amazing Grace",
            "John Newton",
            "",
            "",
            None,
            "2026-04-05",
            &[],
        )
        .unwrap();
        set_user_version(&conn, 0).unwrap();

        migrate(&conn, false).unwrap();

        assert!(list_plans(&conn).unwrap().iter().any(|p| p.id == plan));
        assert!(get_song(&conn, song).unwrap().is_some());
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

/// Windows is a day-one platform (docs/DECISIONS.md) that nobody in this repo can
/// actually run. These tests are how it gets defended anyway: `app_data_root` is a
/// pure function of (OS, environment), so every platform's behaviour is exercised
/// from whatever machine happens to be running the suite.
#[cfg(test)]
mod platform_paths {
    use super::app_data_root;
    use std::path::PathBuf;

    /// A fake environment. `None` = variable not set.
    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| {
            pairs
                .iter()
                .find(|(name, _)| *name == k)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn windows_uses_appdata() {
        let p = app_data_root(
            "windows",
            env(&[("APPDATA", r"C:\Users\Ada\AppData\Roaming")]),
        );
        assert_eq!(p, PathBuf::from(r"C:\Users\Ada\AppData\Roaming"));
    }

    #[test]
    fn windows_falls_back_to_userprofile() {
        let p = app_data_root("windows", env(&[("USERPROFILE", r"C:\Users\Ada")]));
        assert_eq!(p, PathBuf::from(r"C:\Users\Ada\AppData\Roaming"));
    }

    /// THE TRAP. "Windows has no HOME" is what everybody believes, and it is false.
    /// Git Bash, MSYS2 and Cygwin all set HOME to a Unix-shaped path. A Windows build
    /// that reaches for HOME because "it works on my machine" would then write the
    /// database and the 148 MB STT model into a directory that the packaged app never
    /// looks in — and the app would come up with speech recognition silently dead,
    /// exactly as it once did.
    ///
    /// So: on Windows, HOME is not merely deprioritised. It is never read at all.
    #[test]
    fn windows_ignores_home_even_when_git_bash_sets_it() {
        let p = app_data_root(
            "windows",
            env(&[
                ("HOME", "/c/Users/Ada"), // what Git Bash exports
                ("APPDATA", r"C:\Users\Ada\AppData\Roaming"),
            ]),
        );
        assert_eq!(p, PathBuf::from(r"C:\Users\Ada\AppData\Roaming"));

        // …and with no APPDATA, it must fall to USERPROFILE, NOT to that HOME.
        let p = app_data_root(
            "windows",
            env(&[("HOME", "/c/Users/Ada"), ("USERPROFILE", r"C:\Users\Ada")]),
        );
        assert_eq!(p, PathBuf::from(r"C:\Users\Ada\AppData\Roaming"));
    }

    #[test]
    fn macos_uses_library_application_support() {
        let p = app_data_root("macos", env(&[("HOME", "/Users/ada")]));
        assert_eq!(p, PathBuf::from("/Users/ada/Library/Application Support"));
    }

    #[test]
    fn linux_prefers_xdg_then_home() {
        assert_eq!(
            app_data_root("linux", env(&[("XDG_DATA_HOME", "/x")])),
            PathBuf::from("/x")
        );
        assert_eq!(
            app_data_root("linux", env(&[("HOME", "/home/ada")])),
            PathBuf::from("/home/ada/.local/share")
        );
    }

    /// An empty environment must not panic and must not produce an absolute path into
    /// somewhere surprising. "." is a poor location, but it is a SAFE one — and the
    /// alternative (unwrap) is a panic on startup, which is the worst failure this app
    /// can have.
    #[test]
    fn no_environment_at_all_is_survivable_everywhere() {
        for os in ["windows", "macos", "linux"] {
            assert_eq!(app_data_root(os, env(&[])), PathBuf::from("."), "{os}");
        }
    }

    /// Everything that persists must live under the ONE app-data root — the model
    /// downloader and the model loader especially, because if those two disagree the
    /// operator downloads 148 MB into a folder nothing ever reads and is told only
    /// that speech recognition is unavailable.
    #[test]
    fn every_persistent_path_hangs_off_app_data_dir() {
        let root = super::app_data_dir();
        assert!(root.ends_with("com.relay.app"));
        assert!(crate::models::models_dir().starts_with(&root));
        assert!(super::media_dir().starts_with(&root));
        // The downloader writes where the loader reads. Same dir, by construction.
        assert_eq!(crate::models::models_dir(), root.join("models"));
    }
}

/// Enforces the rule, rather than trusting a doc comment to be read.
///
/// `stt.rs` once hand-rolled its own `$HOME/Library/Application Support/…`, which
/// compiled, passed every test, ran perfectly on the author's Mac, and shipped a
/// Windows build with speech recognition **silently dead** — because Windows has no
/// HOME and the model was never found. A comment saying "don't re-derive this path"
/// was already there. It did not help.
///
/// So the rule is now a test. There is exactly one module allowed to know where an OS
/// keeps its files.
#[cfg(test)]
mod path_rule {
    /// Reading any of these outside `db/mod.rs` means re-deriving an OS path by hand.
    const FORBIDDEN: [&str; 4] = ["APPDATA", "USERPROFILE", "XDG_DATA_HOME", "\"HOME\""];

    #[test]
    fn only_db_knows_where_the_os_keeps_its_files() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders: Vec<String> = Vec::new();

        let mut stack = vec![src.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in rd.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs") {
                    continue;
                }
                // db/mod.rs IS the sanctioned place.
                if path.ends_with("db/mod.rs") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for (i, line) in text.lines().enumerate() {
                    // Only actual env reads — not prose in a comment about them.
                    let code = line.split("//").next().unwrap_or("");
                    if !code.contains("env::var") {
                        continue;
                    }
                    for needle in FORBIDDEN {
                        if code.contains(needle) {
                            offenders.push(format!(
                                "{}:{} — {}",
                                path.strip_prefix(&src).unwrap_or(&path).display(),
                                i + 1,
                                code.trim()
                            ));
                        }
                    }
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "OS paths must be resolved by db::app_data_dir() / db::downloads_dir(), \
             never re-derived. Windows has no HOME, and a build that assumes it does \
             ships with STT silently dead.\n  {}",
            offenders.join("\n  ")
        );
    }
}
