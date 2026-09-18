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

/// THE ROLE MAP, AS IT GOES ON THE WIRE — `{"1":"main","2":"stage"}`.
///
/// Channel ids against roles and nothing else: no names, no addresses, nothing a
/// client chose and nothing about who is connected. It is published to every
/// kiosk client because the hub cannot address one (DECISIONS §35), and each
/// client picks out its own id. A screen learning that ANOTHER screen is the
/// stage tells it nothing it could not already read off the Outputs desk.
///
/// Channels with no role are omitted rather than written as null. An absent key
/// and an explicit null would have to mean the same thing at the receiver, and
/// two spellings of one fact is how a filter comes to have two answers.
pub fn channel_roles_json(conn: &Connection) -> rusqlite::Result<String> {
    let mut stmt =
        conn.prepare("SELECT id, role FROM output_channels WHERE role IS NOT NULL ORDER BY id")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    let mut map = serde_json::Map::new();
    for row in rows {
        let (id, role) = row?;
        map.insert(id.to_string(), serde_json::Value::String(role));
    }
    Ok(serde_json::Value::Object(map).to_string())
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

/// RENAME A SCREEN. Returns whether a row was actually renamed.
///
/// The bool is not decoration. A rename against an id that has been deleted on
/// another surface writes nothing, and `UPDATE` reports that by affecting zero
/// rows rather than by failing — so without this the operator would be shown the
/// new name on a screen that no longer exists, which is the "reports a success it
/// did not achieve" shape in a place nobody would look for it.
///
/// The name is stored EXACTLY as the caller hands it over. Trimming and refusing
/// an empty name are the command's job, in one place, for the same reason
/// `save_environment` gives: two layers that both validate are two layers that can
/// disagree about what is legal.
pub fn rename_channel(conn: &Connection, id: i64, name: &str) -> rusqlite::Result<bool> {
    let n = conn.execute(
        "UPDATE output_channels SET name = ?1 WHERE id = ?2",
        (name, id),
    )?;
    Ok(n > 0)
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

// ─────────────────────────────────────────────────────────────────────────────
// PER-KIND LOOKS — what a screen wears for ONE kind of content
// ─────────────────────────────────────────────────────────────────────────────

/// CREATE `channel_looks`. A ROW PER (SCREEN, KIND), NOT A COLUMN PER KIND.
///
/// `CONTENT_KINDS` is already mirrored by hand in three places with no test
/// linking them (`src/lib/layers.js`, `main::ContentTemplates`,
/// `main::CONTENT_LOOK_KINDS`); a column per kind would make DDL the fourth
/// mirror and the least editable of the four, because SQLite cannot drop or
/// rename one without the table rebuild rule 25 is the scar of.
///
/// `template_id` is `NOT NULL` deliberately: **NO ROW is the only way to say
/// "this kind inherits".** An absent row and a NULL row would have to mean the
/// same thing at every reader, and two spellings of one fact is exactly what
/// `channel_roles_json` already refuses one screen at a time.
///
/// `kind` carries no `CHECK`. SQLite cannot `ALTER` a `CHECK`, so a sixth content
/// kind would mean a table rebuild at boot — rule 25's own failure, for the sake
/// of refusing a row nothing would ever read. A kind nobody reads is an inert
/// row; a rebuild before the window is shown is not.
///
/// **RULE 25, AND IT IS RETRYABLE BY HAVING NOTHING TO RETRY.** One
/// `CREATE TABLE IF NOT EXISTS`: no scratch table, no rebuild, no transaction
/// opened, so a mid-batch failure cannot leave one open for the following
/// `PRAGMA foreign_keys = ON` to no-op inside.
///
/// **THERE IS NO BACK-FILL, AND REFUSING ONE IS THE WHOLE OF THIS MIGRATION.**
/// An install with four channels each carrying a `template_id` ends here with
/// zero rows: every kind falls through to the screen's own template and first
/// launch is identical by construction rather than by comparison. The tempting
/// back-fill — write each screen's current template into all five kinds so the
/// shape is explicit — changes nothing on day one and everything on day two,
/// because the operator then changes that screen's template and four kinds
/// silently keep the old one with no control having been touched.
pub(super) fn ensure_channel_looks(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS channel_looks (
             channel_id  INTEGER NOT NULL REFERENCES output_channels(id) ON DELETE CASCADE,
             kind        TEXT    NOT NULL,
             template_id INTEGER NOT NULL REFERENCES templates(id),
             PRIMARY KEY (channel_id, kind)
         );",
    )
}

/// THE PER-KIND LOOK MAP, AS IT GOES ON THE WIRE —
/// `{"1":{"scripture":9,"song":12}}`.
///
/// Channel ids against kind → template id, and nothing else: no names, no
/// addresses, nothing a client chose and nothing about who is connected. Shaped
/// exactly like `channel_roles_json` and published the same way, because the hub
/// cannot address one client (DECISIONS §35) and each client picks out its own id.
///
/// A screen with no per-kind look is OMITTED rather than written as `{}`, and a
/// kind with no row is omitted rather than written as null — the same rule, for
/// the same reason, one level down. The empty map `{}` is still an ANSWER and is
/// sent: a page that cannot tell "nobody has told me" from "I have no per-kind
/// look" paints the wrong template for one frame (rule 35).
pub fn channel_looks_json(conn: &Connection) -> rusqlite::Result<String> {
    let mut stmt = conn.prepare(
        "SELECT channel_id, kind, template_id FROM channel_looks ORDER BY channel_id, kind",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    let mut map = serde_json::Map::new();
    for row in rows {
        let (id, kind, tpl) = row?;
        let entry = map
            .entry(id.to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
        if let Some(obj) = entry.as_object_mut() {
            obj.insert(kind, serde_json::Value::from(tpl));
        }
    }
    Ok(serde_json::Value::Object(map).to_string())
}

/// SET (or CLEAR) what one screen wears for one kind of content.
///
/// `None` DELETES the row, because no row is the only spelling of "this kind
/// inherits" — see `ensure_channel_looks`. There is no second spelling to write.
pub fn set_channel_look(
    conn: &Connection,
    channel_id: i64,
    kind: &str,
    template_id: Option<i64>,
) -> rusqlite::Result<()> {
    match template_id {
        Some(tpl) => conn.execute(
            "INSERT INTO channel_looks (channel_id, kind, template_id) VALUES (?1, ?2, ?3)
               ON CONFLICT(channel_id, kind) DO UPDATE SET template_id = excluded.template_id",
            (channel_id, kind, tpl),
        )?,
        None => conn.execute(
            "DELETE FROM channel_looks WHERE channel_id = ?1 AND kind = ?2",
            (channel_id, kind),
        )?,
    };
    Ok(())
}

/// THE DISTINCT TEMPLATE IDS THE PER-KIND LOOKS NAME.
///
/// This is what the kiosk hub owes a client on connect beyond the content looks:
/// a look reaches a screen as an id and NOTHING ELSE (the 13 MB reason is at
/// `main::cue_or_content_tpl`), so the bytes have to be at the receiver before
/// the id arrives. Distinct, because the same look on three screens is the same
/// bytes and must not be counted three times against the bound on a hello reply.
pub fn channel_look_ids(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    let mut stmt =
        conn.prepare("SELECT DISTINCT template_id FROM channel_looks ORDER BY template_id")?;
    let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
    rows.collect()
}

/// THE TEMPLATE IDS SCREENS ARE WEARING FOR ONE KIND OR ANOTHER — the FIFTH
/// RETIREMENT DOOR (`db::templates::ensure_retired_presets_are_gone`).
///
/// Identical in shape to `channel_look_ids` and deliberately a separate function
/// from it: that one answers "what must the hub send", this one answers "what may
/// the retirement not delete", and a reader who has to work out that one function
/// serves both is a reader who can quietly narrow one of them.
pub(super) fn template_ids_in_use_by_looks(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    channel_look_ids(conn)
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

#[cfg(test)]
mod look_tests {
    use super::*;
    use crate::db::SCHEMA;

    /// A database at the shape `schema.sql` ships, with the shelf and the four
    /// seeded screens in it — because both foreign keys on `channel_looks` are
    /// real and `schema.sql` turns the pragma on. A fixture without the rows a
    /// real install has would pass on a table with no foreign keys at all.
    fn install() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch("COMMIT;").ok();
        crate::db::seed_templates(&conn).unwrap();
        crate::db::seed_channels(&conn).unwrap();
        conn
    }

    /// RULE 25, in the form this migration takes it: it is retryable by having
    /// nothing to retry. One `CREATE TABLE IF NOT EXISTS`, no scratch table, no
    /// rebuild, no transaction opened — so the failure rule 25 is the scar of (a
    /// half-applied rebuild leaving `…_new` behind, and every subsequent boot
    /// failing before the window is shown) has no way to arise here.
    ///
    /// Three calls on one connection, because two would not distinguish "runs
    /// twice" from "is a no-op after the first". The rows written in between must
    /// survive: a migration that quietly recreated the table would drop them, and
    /// on a real install those rows are what four screens are wearing.
    #[test]
    fn ensure_channel_looks_is_retryable() {
        let conn = install();
        ensure_channel_looks(&conn).unwrap();
        set_channel_look(&conn, 1, "scripture", Some(9)).unwrap();
        ensure_channel_looks(&conn).unwrap();
        ensure_channel_looks(&conn).unwrap();
        assert_eq!(
            channel_looks_json(&conn).unwrap(),
            r#"{"1":{"scripture":9}}"#,
            "a second or third run of the migration moved what a screen wears"
        );
    }

    /// **THERE IS NO BACK-FILL, AND REFUSING ONE IS THE WHOLE OF THE MIGRATION.**
    ///
    /// Four seeded channels each carry a `template_id`. After the migration this
    /// table must hold ZERO rows, so every kind falls through to the screen's own
    /// template and first launch is identical BY CONSTRUCTION rather than by
    /// comparison.
    ///
    /// The tempting back-fill — write each screen's current template into all
    /// five kinds so the shape is explicit — changes nothing on day one and
    /// everything on day two: the operator then changes that screen's template
    /// and four kinds silently keep the old one, with no control having been
    /// touched and nothing on any surface saying so.
    #[test]
    fn an_upgraded_install_behaves_identically_on_first_launch() {
        let conn = install();
        ensure_channel_looks(&conn).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM channel_looks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            n, 0,
            "the migration back-filled {n} rows — every screen now carries a \
             per-kind look nobody chose, and changing that screen's template will \
             move nothing"
        );
        assert_eq!(channel_looks_json(&conn).unwrap(), "{}");
    }

    /// A screen with no per-kind look is not written as an empty object, and a
    /// kind with no row is not written as a null. An absent key and an explicit
    /// null would have to mean the same thing at every receiver, which is the
    /// reason the row itself is `NOT NULL` — two spellings of one fact is what
    /// the role map already refuses.
    #[test]
    fn the_map_omits_what_it_has_nothing_to_say_about() {
        let conn = install();
        ensure_channel_looks(&conn).unwrap();
        set_channel_look(&conn, 2, "song", Some(12)).unwrap();
        set_channel_look(&conn, 2, "scripture", Some(9)).unwrap();
        assert_eq!(
            channel_looks_json(&conn).unwrap(),
            r#"{"2":{"scripture":9,"song":12}}"#
        );
        // None DELETES the row. It is the only way to say "this kind inherits".
        set_channel_look(&conn, 2, "song", None).unwrap();
        assert_eq!(
            channel_looks_json(&conn).unwrap(),
            r#"{"2":{"scripture":9}}"#
        );
        set_channel_look(&conn, 2, "scripture", None).unwrap();
        assert_eq!(
            channel_looks_json(&conn).unwrap(),
            "{}",
            "a screen whose last per-kind look was cleared is still named in the map"
        );
    }

    /// DELETING A SCREEN TAKES ITS LOOKS WITH IT.
    ///
    /// `ON DELETE CASCADE` on the foreign key, and it is asserted rather than
    /// assumed: SQLite enforces foreign keys only with the pragma on, which
    /// `db::mod` sets for every real connection. A row left behind would be sent
    /// to every client on every hello, naming a channel no page can be, for ever.
    #[test]
    fn a_deleted_screen_takes_its_looks_with_it() {
        let conn = install();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        ensure_channel_looks(&conn).unwrap();
        set_channel_look(&conn, 3, "announce", Some(9)).unwrap();
        delete_channel(&conn, 3).unwrap();
        assert_eq!(channel_looks_json(&conn).unwrap(), "{}");
    }

    /// The distinct template ids the per-kind looks name — the whole of what the
    /// hub has to put in a hello reply beyond the content looks.
    #[test]
    fn the_ids_a_hello_reply_owes_are_distinct_and_ordered() {
        let conn = install();
        ensure_channel_looks(&conn).unwrap();
        set_channel_look(&conn, 1, "scripture", Some(9)).unwrap();
        set_channel_look(&conn, 1, "song", Some(12)).unwrap();
        // The same look on a second screen is the SAME bytes; it must not be
        // counted twice against the bound on a hello reply.
        set_channel_look(&conn, 2, "scripture", Some(9)).unwrap();
        assert_eq!(channel_look_ids(&conn).unwrap(), vec![9, 12]);
    }
}
