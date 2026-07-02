//! SQLite access layer.
//!
//! Single responsibility: local-first persistence against the schema in
//! docs/data/schema.sql. Nothing else in this codebase should touch SQLite
//! directly — go through this module. See PROMPT.md Phase 2.
//!
//! TODO(phase 2): connection setup, migrations, seed data (KJV + a handful
//! of verses is enough to develop against before loading the full Bible).

use rusqlite::Connection;

pub fn open() -> rusqlite::Result<Connection> {
    // TODO: real path resolution (app data dir), run schema.sql if not present.
    Connection::open_in_memory()
}
