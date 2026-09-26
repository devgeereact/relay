//! Timer persistence: the registry's rows, written on every change and read
//! once at launch.
//!
//! Single responsibility: **keep `timers::TimerRegistry` on disk so a relaunch
//! mid-service does not lose every clock**, and say which of the saved rows are
//! worth restoring. It decides nothing about what a timer means and publishes
//! nothing to any screen — restoring a congregation countdown into the registry
//! is not the same as putting it back on a wall, and the two are deliberately
//! kept apart (DECISIONS §112).
//!
//! ── The shape ─────────────────────────────────────────────────────────────
//!
//! One row per timer, the whole `Timer` as JSON in `body`. The registry's
//! `next_id` rides in `app_settings` under `timers.next_id`, because ids are
//! never reused (a reused id is a `+1` landing on the wrong clock) and a fresh
//! process would otherwise start counting from one again.
//!
//! Every save REPLACES the table: the registry announces a snapshot, not a
//! delta, so a message lost on the way is repaired by the next one and there is
//! no ordering to get wrong.

use crate::timers::Timer;
use rusqlite::{params, Connection, OptionalExtension};

const NEXT_ID_KEY: &str = "timers.next_id";

/// A clock older than this at launch is last week's, not this morning's.
pub const RESTORE_HORIZON_MS: i64 = 6 * 60 * 60 * 1000;

pub(super) fn ensure_timers(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS timers (
            id   INTEGER PRIMARY KEY,
            body TEXT NOT NULL
        );",
    )
}

/// Replace the saved set with this one. One transaction, so a crash between the
/// delete and the inserts cannot leave half a registry.
pub fn save_timers(conn: &Connection, next_id: i64, timers: &[Timer]) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM timers", [])?;
    {
        let mut stmt = tx.prepare("INSERT INTO timers (id, body) VALUES (?1, ?2)")?;
        for t in timers {
            let body = serde_json::to_string(t)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            stmt.execute(params![t.id, body])?;
        }
    }
    tx.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![NEXT_ID_KEY, next_id.to_string()],
    )?;
    tx.commit()
}

/// Everything saved, oldest first, with the `next_id` to resume from. A row whose
/// body no longer parses (an older build's shape, a hand edit) is skipped rather
/// than failing the launch: a clock that cannot be read back is a clock lost, and
/// losing one is better than refusing to open.
pub fn load_timers(conn: &Connection) -> rusqlite::Result<(i64, Vec<Timer>)> {
    let next_id: i64 = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [NEXT_ID_KEY],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut stmt = conn.prepare("SELECT body FROM timers ORDER BY id")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for body in rows.flatten() {
        if let Ok(t) = serde_json::from_str::<Timer>(&body) {
            out.push(t);
        }
    }
    Ok((next_id, out))
}

/// WHICH SAVED TIMERS A LAUNCH SHOULD BRING BACK.
///
/// - A timer started inside a rehearsal is dropped: rehearsal itself does not
///   survive a relaunch, so restoring its clock would present it as a real one.
/// - A timer whose every instant is older than [`RESTORE_HORIZON_MS`] is dropped:
///   it belongs to a service that has ended, and a preacher's tablet showing
///   last Sunday's overrun is worse than showing nothing.
///
/// Everything else comes back exactly as saved — running or held, at its saved
/// figure — because the deadline model makes a running timer correct after any
/// gap: `target_ms` is an instant, not a count.
pub fn restorable(saved: Vec<Timer>, now_ms: i64) -> Vec<Timer> {
    saved
        .into_iter()
        .filter(|t| !t.started_in_rehearsal)
        .filter(|t| t.target_ms.max(t.from_ms) >= now_ms - RESTORE_HORIZON_MS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timers::Scope;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        crate::db::init_fresh(&c).unwrap();
        c
    }

    fn timer(id: i64, now: i64, scope: Scope) -> Timer {
        Timer {
            id,
            label: format!("t{id}"),
            done_msg: String::new(),
            target_ms: now + 300_000,
            from_ms: now,
            paused_ms: None,
            warn_ms: Some(60_000),
            scope,
            configured_ms: 300_000,
            until_ms: None,
            plan_item_id: Some(3),
            started_in_rehearsal: false,
            channels: Some(vec![1, 2]),
        }
    }

    #[test]
    fn a_saved_registry_reads_back_field_for_field() {
        let c = conn();
        let now = 1_700_000_000_000;
        let mut held = timer(2, now, Scope::Stage);
        held.paused_ms = Some(-90_000); // a stage clock held ninety seconds over
        let set = vec![timer(1, now, Scope::Both), held];
        save_timers(&c, 9, &set).unwrap();
        let (next, back) = load_timers(&c).unwrap();
        assert_eq!(next, 9);
        assert_eq!(back, set);

        // A second save REPLACES, never appends.
        save_timers(&c, 10, &set[1..]).unwrap();
        let (next, back) = load_timers(&c).unwrap();
        assert_eq!((next, back.len()), (10, 1));
        assert_eq!(back[0].id, 2);
    }

    #[test]
    fn an_empty_install_has_nothing_to_restore() {
        let (next, back) = load_timers(&conn()).unwrap();
        assert_eq!((next, back.len()), (0, 0));
    }

    #[test]
    fn a_row_that_no_longer_parses_is_skipped_not_fatal() {
        let c = conn();
        save_timers(&c, 3, &[timer(1, 5, Scope::Both)]).unwrap();
        c.execute("INSERT INTO timers (id, body) VALUES (2, 'not json')", [])
            .unwrap();
        let (_, back) = load_timers(&c).unwrap();
        assert_eq!(back.len(), 1);
    }

    #[test]
    fn restore_drops_rehearsal_clocks_and_last_weeks_clocks_and_keeps_the_rest() {
        let now = 1_700_000_000_000;
        let mut reh = timer(1, now, Scope::Stage);
        reh.started_in_rehearsal = true;
        let last_week = timer(2, now - 7 * 24 * 3_600_000, Scope::Stage);
        // A stage clock held for an hour, aimed two hours ago: this morning's.
        let mut held = timer(3, now - 2 * 3_600_000, Scope::Stage);
        held.paused_ms = Some(600_000);
        let running = timer(4, now, Scope::Both);
        // Expired twenty minutes ago and still inside the horizon: kept, so the
        // done message a room was reading is still on the preacher's rail.
        let over = timer(5, now - 1_500_000, Scope::Stage);
        let kept: Vec<i64> = restorable(vec![reh, last_week, held, running, over], now)
            .iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(kept, vec![3, 4, 5]);
    }
}
