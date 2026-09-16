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
    /// WHAT THIS SCREEN IS FOR — `main`, `stage`, or nothing at all.
    ///
    /// A setting, not a guess. Live's programme pane used to pick "the main
    /// screen" by taking the first channel whose `render_target` is
    /// `native_window` and falling through to `channels[0]`, so renaming or
    /// deleting that channel moved the console's idea of the wall in silence.
    ///
    /// It is also the fact `Output.svelte` filters a Stage Message on: the kiosk
    /// hub records nothing about who connected (DECISIONS §35), so the only place
    /// a private message can be refused is the receiver, and the receiver needs
    /// to know what it is.
    ///
    /// `render_target` is unrelated and stays unrelated: that is how a screen is
    /// WIRED. So is `channels.rs`'s `MonitorInfo::primary`, which is a property of
    /// a physical display.
    pub role: Option<String>,
}

/// What `set_channel_role` did — the enforcement, in one place, phrased so the
/// caller can write a sentence for a volunteer rather than relay a constraint
/// violation.
#[derive(Debug)]
pub enum RoleOutcome {
    /// Written.
    Set,
    /// Refused: another channel already holds `main`, and this is its name. At
    /// most one screen may be the main screen — enforced HERE rather than by a
    /// partial index, because an index can only fail, and failing is not the same
    /// as explaining.
    MainTaken(String),
    /// Refused: Relay has no such role. The schema CHECK is a backstop behind
    /// this, not the thing an operator is supposed to read.
    NotARole(String),
}

/// The roles a screen may hold. `None` — no role — is the fourth possibility and
/// is the right answer for a streaming feed or a lobby TV.
pub const CHANNEL_ROLES: &[&str] = &["main", "stage"];

/// All configured output channels.
pub fn list_output_channels(conn: &Connection) -> rusqlite::Result<Vec<OutputChannel>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, render_target, template_id, display_target, status, role
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
            role: r.get(6)?,
        })
    })?;
    rows.collect()
}

/// Assign a template to a channel (the "make outputs assignable" control).
pub fn set_channel_template(
    conn: &Connection,
    id: i64,
    template_id: Option<i64>,
) -> rusqlite::Result<()> {
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
        "INSERT INTO output_channels (name, render_target, template_id, display_target, status, role)
         VALUES (?1, ?2, ?3, ?4, 'offline', ?5)",
    )?;
    for (name, target, tmpl, disp) in channels {
        stmt.execute((name, target, tmpl, disp, seeded_role(name)))?;
    }
    Ok(())
}

/// The role a seeded channel is born with, by name. The same table the back-fill
/// reads, so a fresh install and an upgraded one cannot disagree about which
/// screen is the main one.
fn seeded_role(name: &str) -> Option<&'static str> {
    match name {
        "Main screen" => Some("main"),
        "Stage display" => Some("stage"),
        // Streaming and Lobby screen have no role, deliberately. A feed and a
        // foyer TV are congregation screens; neither is the wall the console
        // previews and neither may ever be handed a word meant for the platform.
        _ => None,
    }
}

/// ADD `output_channels.role`, AND GIVE AN EXISTING INSTALL THE TWO IT ALREADY
/// HAD IN ALL BUT NAME.
///
/// Retryable and idempotent (rule 25): the column is sniffed for before the
/// `ALTER`, a lost race against a second Relay process is the desired end state
/// rather than a panic before the window is shown, and the back-fill runs only
/// while NO row carries a role at all.
///
/// That last condition is what stops this fighting the operator. A back-fill
/// keyed on "this row is NULL" would re-seed `Main screen` every launch, so
/// clearing its role — or moving `main` to another screen and clearing this one —
/// would last until the next boot and no message would say why.
pub(super) fn ensure_channel_role(conn: &Connection) -> rusqlite::Result<()> {
    let present: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('output_channels') WHERE name = 'role'",
        [],
        |r| r.get(0),
    )?;
    if present == 0 {
        // AND TOLERATE LOSING THE RACE — the same reasoning, and the same
        // comment, as `db::plans::add_plan_item_column`. Two Relay processes can
        // hold one file (`tauri dev` respawning, or a church laptop launched
        // twice); both read the pragma, both ALTER, and the loser panicked at
        // startup. The column existing IS the desired end state.
        match conn.execute_batch(
            "ALTER TABLE output_channels ADD COLUMN role TEXT              CHECK (role IS NULL OR role IN ('main','stage'));",
        ) {
            Err(e) if crate::db::plans::is_duplicate_column(&e) => {}
            other => other?,
        }
    }
    let any: i64 = conn.query_row(
        "SELECT COUNT(*) FROM output_channels WHERE role IS NOT NULL",
        [],
        |r| r.get(0),
    )?;
    if any > 0 {
        return Ok(());
    }
    // By NAME, because that is the only identity a seeded channel has ever had.
    // A church that renamed its screens gets no roles from this and sets them in
    // Outputs, which is strictly better than guessing at a render target.
    for (name, role) in [("Main screen", "main"), ("Stage display", "stage")] {
        conn.execute(
            "UPDATE output_channels SET role = ?1 WHERE name = ?2 AND role IS NULL",
            (role, name),
        )?;
    }
    // …and if two rows were somehow called `Main screen`, only one may keep it.
    // The UPDATE above is by name, so this is the one case it can overshoot.
    conn.execute(
        "UPDATE output_channels SET role = NULL
          WHERE role = 'main'
            AND id <> (SELECT MIN(id) FROM output_channels WHERE role = 'main')",
        [],
    )?;
    Ok(())
}

/// SET (or clear) A SCREEN'S ROLE, with the one-main rule enforced here.
///
/// Not a partial index. An index would refuse the write with a constraint
/// violation, and `src/lib/errors.js` would then be asked to humanise a sentence
/// SQLite wrote — which is the shape `error.rs` exists to stop. This returns what
/// happened and the caller writes the sentence.
pub fn set_channel_role(
    conn: &Connection,
    id: i64,
    role: Option<&str>,
) -> rusqlite::Result<RoleOutcome> {
    if let Some(r) = role {
        if !CHANNEL_ROLES.contains(&r) {
            return Ok(RoleOutcome::NotARole(r.to_string()));
        }
        if r == "main" {
            let taken: Option<String> = conn
                .query_row(
                    "SELECT name FROM output_channels WHERE role = 'main' AND id <> ?1",
                    [id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(name) = taken {
                return Ok(RoleOutcome::MainTaken(name));
            }
        }
    }
    conn.execute(
        "UPDATE output_channels SET role = ?1 WHERE id = ?2",
        (role, id),
    )?;
    Ok(RoleOutcome::Set)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A database shaped like the one an install that predates the role column
    /// has: the table as `schema.sql` defined it before this wave, seeded by name.
    fn old_install() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE output_channels (
                 id             INTEGER PRIMARY KEY,
                 name           TEXT NOT NULL,
                 render_target  TEXT NOT NULL,
                 template_id    INTEGER,
                 display_target TEXT,
                 status         TEXT NOT NULL DEFAULT 'offline'
             );
             INSERT INTO output_channels (name, render_target, template_id, status) VALUES
                 ('Main screen','native_window',1,'offline'),
                 ('Stage display','network_client',2,'offline'),
                 ('Streaming','network_client',3,'offline'),
                 ('Lobby screen','network_client',4,'offline');",
        )
        .unwrap();
        conn
    }

    fn roles(conn: &Connection) -> Vec<(String, Option<String>)> {
        list_output_channels(conn)
            .unwrap()
            .into_iter()
            .map(|c| (c.name, c.role))
            .collect()
    }

    /// RULE 25 — a migration that runs on every boot must be a no-op the second
    /// time and identical the third.
    #[test]
    fn the_role_column_migration_is_retryable() {
        let conn = old_install();
        ensure_channel_role(&conn).unwrap();
        let once = roles(&conn);
        ensure_channel_role(&conn).unwrap();
        let twice = roles(&conn);
        ensure_channel_role(&conn).unwrap();
        assert_eq!(once, twice);
        assert_eq!(twice, roles(&conn));
    }

    /// The back-fill names the two screens the seed has always named, and leaves
    /// the congregation screens without a role at all.
    #[test]
    fn an_existing_install_gains_the_two_roles_it_already_had_by_name() {
        let conn = old_install();
        ensure_channel_role(&conn).unwrap();
        assert_eq!(
            roles(&conn),
            vec![
                ("Main screen".into(), Some("main".into())),
                ("Stage display".into(), Some("stage".into())),
                ("Streaming".into(), None),
                ("Lobby screen".into(), None),
            ]
        );
    }

    /// AND IT NEVER FIGHTS THE OPERATOR. Once any row carries a role, the
    /// back-fill has done its job and must never run again — otherwise clearing
    /// the main screen's role would last until the next launch.
    #[test]
    fn the_back_fill_never_undoes_a_choice_the_operator_has_made() {
        let conn = old_install();
        ensure_channel_role(&conn).unwrap();
        // The operator moves `main` to the Lobby screen and clears the seeded one.
        assert!(matches!(
            set_channel_role(&conn, 1, None).unwrap(),
            RoleOutcome::Set
        ));
        assert!(matches!(
            set_channel_role(&conn, 4, Some("main")).unwrap(),
            RoleOutcome::Set
        ));
        ensure_channel_role(&conn).unwrap();
        assert_eq!(
            roles(&conn),
            vec![
                ("Main screen".into(), None),
                ("Stage display".into(), Some("stage".into())),
                ("Streaming".into(), None),
                ("Lobby screen".into(), Some("main".into())),
            ]
        );
    }

    /// AT MOST ONE MAIN — refused in the helper, with the name of the screen that
    /// already holds it, so the operator reads a sentence rather than a
    /// constraint violation.
    #[test]
    fn a_second_main_screen_is_refused_by_name() {
        let conn = old_install();
        ensure_channel_role(&conn).unwrap();
        match set_channel_role(&conn, 3, Some("main")).unwrap() {
            RoleOutcome::MainTaken(name) => assert_eq!(name, "Main screen"),
            other => panic!("a second main screen was allowed: {other:?}"),
        }
        // …and nothing was written.
        assert_eq!(roles(&conn)[2].1, None);
    }

    /// Re-setting the SAME channel to main is not a conflict with itself.
    #[test]
    fn the_main_screen_may_be_set_to_main_again() {
        let conn = old_install();
        ensure_channel_role(&conn).unwrap();
        assert!(matches!(
            set_channel_role(&conn, 1, Some("main")).unwrap(),
            RoleOutcome::Set
        ));
    }

    /// SEVERAL STAGES ARE FINE — a church may have a confidence monitor and a
    /// preacher's tablet, and nothing about a stage role is exclusive.
    #[test]
    fn several_screens_may_be_stages() {
        let conn = old_install();
        ensure_channel_role(&conn).unwrap();
        assert!(matches!(
            set_channel_role(&conn, 3, Some("stage")).unwrap(),
            RoleOutcome::Set
        ));
        assert!(matches!(
            set_channel_role(&conn, 4, Some("stage")).unwrap(),
            RoleOutcome::Set
        ));
        let r = roles(&conn);
        assert_eq!(
            r.iter()
                .filter(|(_, x)| x.as_deref() == Some("stage"))
                .count(),
            3
        );
    }

    /// A role Relay does not have is refused before SQLite sees it, so the CHECK
    /// is a backstop rather than the error message.
    #[test]
    fn a_role_relay_does_not_have_is_refused_rather_than_written() {
        let conn = old_install();
        ensure_channel_role(&conn).unwrap();
        match set_channel_role(&conn, 3, Some("lobby")).unwrap() {
            RoleOutcome::NotARole(r) => assert_eq!(r, "lobby"),
            other => panic!("an unknown role was accepted: {other:?}"),
        }
        assert_eq!(roles(&conn)[2].1, None);
    }

    /// A FRESH INSTALL IS SEEDED WITH ITS ROLES, not back-filled into them.
    #[test]
    fn a_fresh_seed_names_the_main_screen_and_the_stage_display() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE output_channels (
                 id             INTEGER PRIMARY KEY,
                 name           TEXT NOT NULL,
                 render_target  TEXT NOT NULL,
                 template_id    INTEGER,
                 display_target TEXT,
                 status         TEXT NOT NULL DEFAULT 'offline',
                 role           TEXT CHECK (role IS NULL OR role IN ('main','stage'))
             );",
        )
        .unwrap();
        seed_channels(&conn).unwrap();
        assert_eq!(
            roles(&conn),
            vec![
                ("Main screen".into(), Some("main".into())),
                ("Stage display".into(), Some("stage".into())),
                ("Streaming".into(), None),
                ("Lobby screen".into(), None),
            ]
        );
    }
}
