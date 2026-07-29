//! Output channels: the physical/virtual destinations a template renders to
//! (main screen, stage monitor, streaming lower-third, lobby).

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

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

/// Seed the default output channels (idempotent — only when empty). Template ids
/// 1..4 match the seeded templates.
pub(super) fn seed_channels(conn: &Connection) -> rusqlite::Result<()> {
    let channels: &[(&str, &str, i64, Option<&str>)] = &[
        // "0", not "Display 1": `display_target` is parsed as a monitor INDEX, and
        // the human-readable form silently failed to parse, so the seeded main
        // screen always opened on the primary display instead of the one it was
        // configured with. (`parse_display` now accepts both, but the seed should
        // still write the canonical form.)
        ("Main screen", "native_window", 1, Some("0")),
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
