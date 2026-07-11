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
