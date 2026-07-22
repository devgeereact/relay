//! Service plans: an ordered list of typed cues.
//!
//! Every content type reduces to the same polymorphic cue (`cue_type` +
//! `payload_json`), so the Planner and the renderer never branch per type —
//! adding a content type is a new payload shape, not new plumbing.

use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::Serialize;
use serde_json::Value;

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
///
/// `section_title` is how a plan gets its running order *headings* ("Welcome &
/// Worship", "Sermon") without a second table: a non-empty value means this cue
/// BEGINS a new section, and the section runs until the next cue that has one.
/// Grouping is therefore derived from the same ordered list the transport walks,
/// so drag-reorder, `move_plan_item` and `stepFrom` need no section awareness and
/// a section can never desynchronise from the cues it claims to contain.
///
/// `duration_sec` is the planned length used for the running-time estimate. 0
/// means "not timed" — a scripture cue fires when the preacher reaches it, not
/// on a clock, and the Planner renders that as an em dash rather than 0:00.
#[derive(Debug, Clone, Serialize)]
pub struct PlanItem {
    pub id: i64,
    pub plan_id: i64,
    pub position: i64,
    pub cue_type: String,
    pub label: String,
    pub payload_json: String,
    pub template_id: Option<i64>,
    pub section_title: String,
    pub duration_sec: i64,
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
    )?;
    add_plan_item_column(conn, "section_title", "TEXT NOT NULL DEFAULT ''")?;
    add_plan_item_column(conn, "duration_sec", "INTEGER NOT NULL DEFAULT 0")
}

/// Add a column to `plan_items` only if it is absent.
///
/// A bare `ALTER TABLE … ADD COLUMN` is NOT retryable — it errors with "duplicate
/// column name" on the second run, and this function runs on every single boot.
/// Asking `pragma_table_info` first is what makes it idempotent. Deliberately a
/// plain ADD COLUMN and not a table rebuild: the rebuild path is the one that
/// stranded a scratch table and bricked every subsequent boot (CLAUDE.md §25).
fn add_plan_item_column(conn: &Connection, name: &str, decl: &str) -> rusqlite::Result<()> {
    let present: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('plan_items') WHERE name = ?1",
        [name],
        |r| r.get(0),
    )?;
    if present > 0 {
        return Ok(());
    }
    // AND TOLERATE LOSING THE RACE.
    //
    // The `pragma_table_info` check above makes this retryable, which is what
    // CLAUDE.md §25 asks for — but "already checked" is not "cannot happen".
    // Two Relay processes can hold the same file: `tauri dev` respawns on a
    // rebuild while the previous binary is still shutting down, and a church
    // laptop can have the app launched twice. Both read the pragma, both see
    // the column missing, both ALTER — and the loser panicked the app at
    // startup with "duplicate column name", **before the window is shown**,
    // forever, which is precisely the class of failure §25 exists to forbid.
    //
    // The column existing IS the desired end state. Whoever created it, the
    // migration has succeeded.
    match conn.execute_batch(&format!("ALTER TABLE plan_items ADD COLUMN {name} {decl};")) {
        Err(e) if is_duplicate_column(&e) => Ok(()),
        other => other,
    }
}

/// True for SQLite's "duplicate column name" — the benign outcome of losing a
/// migration race. Matched on the message because rusqlite reports it as a
/// generic `SqliteFailure` with extended code 1, which is shared with every
/// other SQL error and so cannot be matched on the code alone.
fn is_duplicate_column(e: &rusqlite::Error) -> bool {
    e.to_string()
        .to_lowercase()
        .contains("duplicate column name")
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
    // Every column a cue carries, or duplicating last week's order silently drops
    // its section headings and running times — the operator's whole reason for
    // duplicating instead of starting empty.
    tx.execute(
        "INSERT INTO plan_items (plan_id, position, cue_type, label, payload_json,
                                 template_id, section_title, duration_sec)
         SELECT ?1, position, cue_type, label, payload_json,
                template_id, section_title, duration_sec
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
        "SELECT id, plan_id, position, cue_type, label, payload_json, template_id,
                section_title, duration_sec
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
            section_title: r.get(7)?,
            duration_sec: r.get(8)?,
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

/// Start (or, when blank, stop starting) a section at this cue.
///
/// Setting a title makes this cue the first of a new section; clearing it merges
/// the cue back into the section above. Nothing else moves — the cue order is the
/// section order.
pub fn set_plan_section(conn: &Connection, item_id: i64, title: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE plan_items SET section_title = ?1 WHERE id = ?2",
        (title.trim(), item_id),
    )?;
    Ok(())
}

/// Set a cue's planned length in seconds. Negative input is clamped to 0
/// ("untimed"), never stored — a plan that reports a negative running time is
/// worse than one that reports none.
pub fn set_plan_duration(conn: &Connection, item_id: i64, seconds: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE plan_items SET duration_sec = ?1 WHERE id = ?2",
        (seconds.max(0), item_id),
    )?;
    Ok(())
}

/// Point a cue at a template, or clear it back to the channel's own.
///
/// `None` is meaningful and is NOT the same as "no template": the per-cue value
/// is an *override*, and clearing it re-inherits whatever the channel is set to
/// rather than leaving the cue unrenderable.
pub fn set_plan_template(
    conn: &Connection,
    item_id: i64,
    template_id: Option<i64>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE plan_items SET template_id = ?1 WHERE id = ?2",
        (template_id, item_id),
    )?;
    Ok(())
}

/// Remove one cue.
///
/// If the cue began a section, its heading is inherited by the cue that follows
/// so the section survives losing its first item. Without this, deleting cue 1 of
/// "Welcome & Worship" silently dissolves the whole section and its remaining
/// cues get absorbed by the section above.
pub fn remove_plan_item(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    let row: Option<(i64, i64, String)> = conn
        .query_row(
            "SELECT plan_id, position, section_title FROM plan_items WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;

    let tx = conn.unchecked_transaction()?;
    if let Some((plan_id, pos, title)) = row {
        if !title.is_empty() {
            // The next cue in order, whatever its position number — same reason
            // `move_plan_item` looks up neighbours by order and not arithmetic.
            let next: Option<i64> = tx
                .query_row(
                    "SELECT id FROM plan_items
                      WHERE plan_id = ?1 AND position > ?2 AND id != ?3
                      ORDER BY position ASC LIMIT 1",
                    (plan_id, pos, id),
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(nid) = next {
                tx.execute(
                    "UPDATE plan_items SET section_title = ?1 WHERE id = ?2 AND section_title = ''",
                    (title, nid),
                )?;
            }
        }
    }
    tx.execute("DELETE FROM plan_items WHERE id = ?1", [id])?;
    tx.commit()
}

/// Swap a cue with its neighbor in `direction` (-1 up, +1 down). No-op at ends.
/// Move a cue one place up or down the plan.
///
/// Finds the ADJACENT cue by order, not by position arithmetic.
///
/// It used to look for a neighbour at exactly `position + direction`. But deleting
/// a cue leaves a gap — positions become 0, 1, 3 — and then moving the cue at 3
/// looked for position 2, found nothing, and **silently did nothing at all.** The
/// operator drags a cue and it just doesn't move, with no error and no explanation.
/// Rebuilding the plan was the only way out.
pub fn move_plan_item(conn: &Connection, id: i64, direction: i64) -> rusqlite::Result<()> {
    let (plan_id, pos): (i64, i64) = conn.query_row(
        "SELECT plan_id, position FROM plan_items WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    // The nearest cue in that direction — whatever its position number happens to
    // be. Gaps are irrelevant.
    let sql = if direction < 0 {
        "SELECT id, position FROM plan_items
          WHERE plan_id = ?1 AND position < ?2
          ORDER BY position DESC LIMIT 1"
    } else {
        "SELECT id, position FROM plan_items
          WHERE plan_id = ?1 AND position > ?2
          ORDER BY position ASC LIMIT 1"
    };
    let neighbor: Option<(i64, i64)> = conn
        .query_row(sql, (plan_id, pos), |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;

    // None = already at the top or bottom. Correctly a no-op.
    let Some((nid, npos)) = neighbor else {
        return Ok(());
    };

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE plan_items SET position = ?1 WHERE id = ?2",
        (npos, id),
    )?;
    tx.execute(
        "UPDATE plan_items SET position = ?1 WHERE id = ?2",
        (pos, nid),
    )?;
    tx.commit()?;
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

#[cfg(test)]
mod migration_tests {
    use super::*;

    /// The plan tables, as a v0 database would have them: no `section_title`,
    /// no `duration_sec`.
    fn v0(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE plan_items (
                id           INTEGER PRIMARY KEY,
                plan_id      INTEGER NOT NULL,
                position     INTEGER NOT NULL,
                cue_type     TEXT NOT NULL,
                label        TEXT NOT NULL,
                payload_json TEXT NOT NULL DEFAULT '{}',
                template_id  INTEGER
             );",
        )
        .unwrap();
    }

    fn has_column(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('plan_items') WHERE name = ?1",
            [name],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            > 0
    }

    #[test]
    fn the_column_is_added_once() {
        let conn = Connection::open_in_memory().unwrap();
        v0(&conn);
        add_plan_item_column(&conn, "section_title", "TEXT NOT NULL DEFAULT ''").unwrap();
        assert!(has_column(&conn, "section_title"));
    }

    #[test]
    fn running_it_again_is_a_no_op() {
        // It runs on EVERY boot.
        let conn = Connection::open_in_memory().unwrap();
        v0(&conn);
        for _ in 0..3 {
            add_plan_item_column(&conn, "section_title", "TEXT NOT NULL DEFAULT ''").unwrap();
        }
        assert!(has_column(&conn, "section_title"));
    }

    /// LOSING THE RACE IS NOT A FAILURE.
    ///
    /// Two Relay processes can hold the same file — `tauri dev` respawns while
    /// the old binary is still shutting down, and a church laptop can have the
    /// app launched twice. Both read the pragma, both see the column missing,
    /// both ALTER. The loser used to panic at startup with "duplicate column
    /// name" **before the window is shown**, forever. This reproduces that
    /// exactly: the column appears between the check and the write.
    #[test]
    fn a_column_created_by_someone_else_mid_migration_does_not_brick_the_boot() {
        let conn = Connection::open_in_memory().unwrap();
        v0(&conn);
        // The other process got there first, after our pragma would have said
        // "absent" — simulated by adding it directly, then running the real
        // migration path against a schema that already has it.
        conn.execute_batch(
            "ALTER TABLE plan_items ADD COLUMN section_title TEXT NOT NULL DEFAULT '';",
        )
        .unwrap();
        let raw = conn.execute_batch(
            "ALTER TABLE plan_items ADD COLUMN section_title TEXT NOT NULL DEFAULT '';",
        );
        assert!(
            raw.is_err(),
            "the bare ALTER must still be the thing that fails"
        );
        assert!(
            is_duplicate_column(&raw.unwrap_err()),
            "the failure this tolerates must be recognised as a duplicate column"
        );
        // And the guarded path survives it.
        add_plan_item_column(&conn, "section_title", "TEXT NOT NULL DEFAULT ''")
            .expect("the migration must not fail when the column already exists");
        assert!(has_column(&conn, "section_title"));
    }
}
