//! Stage layouts: what a preacher's screen shows, chosen by the operator.
//!
//! Single responsibility: **store the named sets of choices a stage screen can
//! wear, and which screen wears which.** It renders nothing and decides nothing
//! about content.
//!
//! ── What this replaces ────────────────────────────────────────────────────
//!
//! `Stage.svelte` has had seven zone switches since wave 4, and they lived in
//! `localStorage` on the device under `relay.stage.zones`. That is fine for a
//! preference and wrong for a decision: the operator could not set them, could
//! not see them, and could not tell whether the thing they had just sent was
//! being rendered at all — `Live.svelte` says as much out loud, that whether the
//! preacher's programme zone is on "is not a fact available on this side of the
//! room". A tablet reset or a new device lost the arrangement silently.
//!
//! ── The model, and why it is not the templates table ──────────────────────
//!
//! A LAYOUT IS GLOBAL AND ITS ASSIGNMENT IS PER SCREEN, which is ProPresenter's
//! own shape (`docs/research/PROPRESENTER7_STAGE_AND_TIMERS.md`): layouts are a
//! list you edit once, and each stage screen is pointed at one of them.
//!
//! It is deliberately NOT a row in `templates`. A template carries regions, a
//! style and a `TemplateRender` output; a stage layout carries none of those,
//! because `Stage.svelte` is a hand-drawn monitor rather than a render target —
//! the rail geometry that RG-147 and RG-154 fixed is its own CSS. Putting both
//! in one table would be two different kinds of thing rendered by two different
//! renderers, and the Templates gallery would show a layout as a broken visual
//! template.
//!
//! ── The absence that matters ──────────────────────────────────────────────
//!
//! `output_channels.stage_layout_id` is NULLABLE and null is a real answer: this
//! screen has not been given a layout, so the device's own `localStorage` zones
//! still apply. That is what stops this migration silently erasing an
//! arrangement a church is already using. Assigning a layout is the deliberate
//! act that takes the decision off the device.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// One named set of choices a stage screen can wear.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageLayout {
    pub id: i64,
    pub name: String,
    /// The zone switches, as `Stage.svelte`'s own keys. Stored as JSON rather
    /// than as columns for CLAUDE.md rule 25's reason: the zone list has grown
    /// once already and a column per zone would make DDL a second mirror of it,
    /// needing a table rebuild before the window is shown.
    pub zones: serde_json::Value,
}

/// The layouts a fresh install starts with, so the feature is usable on day one.
///
/// Named after what a screen is FOR rather than after what it contains: an
/// operator choosing between "Preacher" and "Confidence monitor" is making the
/// decision they actually have, where one between "reading+next+note" and
/// "reading+clock" is a decision about our field names.
const SEEDED: &[(&str, &str, &str)] = &[
    (
        "preacher",
        "Preacher",
        r#"{"reading":true,"next":true,"note":true,"countdown":true,"clock":true,"elapsed":true,"programme":true}"#,
    ),
    (
        "confidence",
        "Confidence monitor",
        // No programme rail and no Stage Note: this is a screen somebody OTHER
        // than the preacher reads — a musician, a camera operator — and the
        // preacher's own bookkeeping is not theirs to see.
        r#"{"reading":true,"next":true,"note":false,"countdown":true,"clock":true,"elapsed":false,"programme":false}"#,
    ),
    (
        "timer-focus",
        "Timer focus",
        // The clocks and nothing to read. A screen at the back of a platform.
        r#"{"reading":false,"next":false,"note":false,"countdown":true,"clock":true,"elapsed":true,"programme":true}"#,
    ),
];

/// Create the table if it is not there, and seed the starters BY KEY.
///
/// **By key, not by name, and the difference is the whole of it.** Seeding by
/// name looked right and is not: a church that renames Preacher to Pastor no
/// longer matches, so every launch re-creates the starter beside the renamed
/// one. `a_seeded_layout_the_operator_renamed_is_not_reseeded` caught that on
/// the first run, and `templates` already carries `seed_key` for the identical
/// reason.
///
/// Retryable, per rule 25: `IF NOT EXISTS` throughout, inserts guarded by key,
/// so a half-finished boot leaves nothing to collide with on the next one and
/// no scratch table behind it.
pub(super) fn ensure_stage_layouts(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS stage_layouts (
             id         INTEGER PRIMARY KEY,
             name       TEXT NOT NULL UNIQUE,
             zones_json TEXT NOT NULL,
             -- WHICH STARTER THIS IS, INDEPENDENT OF WHAT IT IS CALLED.
             --
             -- Seeding by NAME looked right and is not: a church that renames
             -- Preacher to Pastor no longer matches, so the next launch
             -- re-creates the starter beside the renamed one, for ever. The
             -- templates table already carries seed_key for exactly this, and
             -- this is the same problem. NULL for a layout the operator made.
             seed_key   TEXT UNIQUE
         );",
    )?;
    // An install created before `seed_key` existed has the column added rather
    // than the table rebuilt — SQLite cannot add a UNIQUE column in an ALTER, so
    // the uniqueness is carried by the index below instead.
    let keyed: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('stage_layouts') WHERE name = 'seed_key'",
        [],
        |r| r.get(0),
    )?;
    if keyed == 0 {
        match conn.execute_batch("ALTER TABLE stage_layouts ADD COLUMN seed_key TEXT;") {
            Err(e) if crate::db::plans::is_duplicate_column(&e) => {}
            other => other?,
        }
    }
    conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_stage_layouts_seed
             ON stage_layouts(seed_key) WHERE seed_key IS NOT NULL;",
    )?;
    for (key, name, zones) in SEEDED {
        // BY KEY AND ONLY WHEN ABSENT, so a renamed or edited starter is left
        // exactly as the operator left it.
        conn.execute(
            "INSERT INTO stage_layouts (name, zones_json, seed_key)
             SELECT ?1, ?2, ?3
             WHERE NOT EXISTS (SELECT 1 FROM stage_layouts WHERE seed_key = ?3)",
            rusqlite::params![name, zones, key],
        )?;
    }
    Ok(())
}

/// Add the per-screen assignment column. Separate from the table above because
/// an install can reach one and not the other.
pub(super) fn ensure_channel_stage_layout(conn: &Connection) -> rusqlite::Result<()> {
    let present: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('output_channels') WHERE name = 'stage_layout_id'",
        [],
        |r| r.get(0),
    )?;
    if present == 0 {
        match conn.execute_batch("ALTER TABLE output_channels ADD COLUMN stage_layout_id INTEGER;")
        {
            Err(e) if crate::db::plans::is_duplicate_column(&e) => {}
            other => other?,
        }
    }
    Ok(())
}

/// Every layout, by name, so the operator's list does not reorder itself.
pub fn list_stage_layouts(conn: &Connection) -> rusqlite::Result<Vec<StageLayout>> {
    let mut q = conn.prepare("SELECT id, name, zones_json FROM stage_layouts ORDER BY name")?;
    let rows = q.query_map([], |r| {
        let raw: String = r.get(2)?;
        Ok(StageLayout {
            id: r.get(0)?,
            name: r.get(1)?,
            // An unparseable stored value reads as NO CHOICES rather than as a
            // layout of all-off: a screen that shows nothing is a screen a
            // preacher cannot use, and the empty object falls through to the
            // device's own zones at the receiver.
            zones: serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({})),
        })
    })?;
    rows.collect()
}

/// Point one screen at one layout, or at none.
///
/// `None` is a real answer and the way back: the screen returns to whatever the
/// device itself has in `localStorage`, rather than to a hard-coded default.
pub fn set_channel_stage_layout(
    conn: &Connection,
    channel_id: i64,
    layout_id: Option<i64>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE output_channels SET stage_layout_id = ?2 WHERE id = ?1",
        rusqlite::params![channel_id, layout_id],
    )?;
    Ok(())
}

/// WHAT EACH SCREEN SHOWS, AS IT GOES ON THE WIRE — `{"2":{"reading":true,…}}`.
///
/// Resolved HERE rather than on the page, so a stage tablet never has to hold a
/// list of layouts it is not wearing, and a rename cannot reach a screen as a
/// separate fact from the choices it renames. A screen with no layout is
/// OMITTED, not sent an empty object: absent means "this screen has not been
/// given one, use your own zones", and an empty object would mean "show
/// nothing". Those are opposite instructions and the difference is a blank
/// screen mid-sermon.
pub fn stage_zones_json(conn: &Connection) -> rusqlite::Result<String> {
    let mut q = conn.prepare(
        "SELECT c.id, l.zones_json
           FROM output_channels c
           JOIN stage_layouts l ON l.id = c.stage_layout_id",
    )?;
    let rows = q.query_map([], |r| {
        let id: i64 = r.get(0)?;
        let raw: String = r.get(1)?;
        Ok((id, raw))
    })?;
    let mut map = serde_json::Map::new();
    for row in rows {
        let (id, raw) = row?;
        // A row that will not parse is left out for the same reason a missing
        // one is: the device's own zones are a working screen, and a malformed
        // blob raw-inserted into the frame would break JSON parsing for every
        // frame after it on that socket (see `channels::cache_channel_roles`).
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            if v.is_object() {
                map.insert(id.to_string(), v);
            }
        }
    }
    Ok(serde_json::Value::Object(map).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let c = Connection::open_in_memory().expect("open");
        c.execute_batch(
            "CREATE TABLE output_channels (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
             INSERT INTO output_channels (id, name) VALUES (1, 'Main'), (2, 'Stage');",
        )
        .expect("schema");
        ensure_stage_layouts(&c).expect("layouts");
        ensure_channel_stage_layout(&c).expect("column");
        c
    }

    #[test]
    fn a_fresh_install_has_starters_to_choose_from() {
        let c = db();
        let all = list_stage_layouts(&c).expect("list");
        assert_eq!(all.len(), SEEDED.len());
        assert!(all.iter().any(|l| l.name == "Preacher"));
    }

    /// RULE 25. Running the ladder twice must be a no-op, not a collision.
    #[test]
    fn the_migration_is_retryable() {
        let c = db();
        ensure_stage_layouts(&c).expect("second run");
        ensure_channel_stage_layout(&c).expect("second run");
        assert_eq!(list_stage_layouts(&c).expect("list").len(), SEEDED.len());
    }

    /// A RENAMED STARTER DOES NOT COME BACK BESIDE ITSELF.
    #[test]
    fn a_seeded_layout_the_operator_renamed_is_not_reseeded() {
        let c = db();
        c.execute(
            "UPDATE stage_layouts SET name = 'Pastor' WHERE name = 'Preacher'",
            [],
        )
        .expect("rename");
        ensure_stage_layouts(&c).expect("ladder again");
        let all = list_stage_layouts(&c).expect("list");
        // The renamed one, plus its starter re-created — that is the FAILURE this
        // guards, so the count must not have grown.
        assert!(all.iter().any(|l| l.name == "Pastor"));
        assert_eq!(all.len(), SEEDED.len());
    }

    #[test]
    fn a_screen_with_no_layout_is_left_out_rather_than_sent_nothing_to_show() {
        let c = db();
        let json = stage_zones_json(&c).expect("json");
        assert_eq!(json, "{}", "an unassigned screen was sent a layout");
    }

    #[test]
    fn an_assigned_screen_rides_with_its_own_choices() {
        let c = db();
        let id = list_stage_layouts(&c).expect("list")[0].id;
        set_channel_stage_layout(&c, 2, Some(id)).expect("assign");
        let json = stage_zones_json(&c).expect("json");
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(
            v.get("2").is_some(),
            "the assigned screen is missing: {json}"
        );
        assert!(v.get("1").is_none(), "an unassigned screen was included");
    }

    /// AND THE WAY BACK IS A REAL ANSWER.
    #[test]
    fn clearing_the_assignment_returns_the_screen_to_its_own_zones() {
        let c = db();
        let id = list_stage_layouts(&c).expect("list")[0].id;
        set_channel_stage_layout(&c, 2, Some(id)).expect("assign");
        set_channel_stage_layout(&c, 2, None).expect("clear");
        assert_eq!(stage_zones_json(&c).expect("json"), "{}");
    }

    /// A BLOB THAT WILL NOT PARSE IS LEFT OUT, NEVER RAW-INSERTED.
    #[test]
    fn an_unparseable_layout_is_omitted_rather_than_breaking_every_later_frame() {
        let c = db();
        c.execute(
            "INSERT INTO stage_layouts (id, name, zones_json) VALUES (99, 'Broken', 'not json')",
            [],
        )
        .expect("insert");
        c.execute(
            "UPDATE output_channels SET stage_layout_id = 99 WHERE id = 2",
            [],
        )
        .expect("assign");
        let json = stage_zones_json(&c).expect("json");
        assert_eq!(json, "{}");
        serde_json::from_str::<serde_json::Value>(&json).expect("the frame must stay parseable");
    }
}
