//! What a church finds in Relay the first time it is opened.
//!
//! Single responsibility: write the starter set — a few announcements, the
//! pictures Relay ships, and one example service plan — into a fresh database,
//! once, and never write it into a database that already has it.
//!
//! ## This reverses a rule, deliberately
//!
//! `init_fresh` used to say *"NOTHING SEEDS DEMO CONTENT HERE, and nothing ever
//! may."* The operator has taken the opposite decision and it is written up as
//! **DECISIONS §90**, with the three reasons. The short form: an empty install
//! is not neutral, the instruments this repository trusts are about drift rather
//! than emptiness, and starter content is not demo content.
//!
//! ## Starter content is not demo content, and the difference is in the code
//!
//! `db::demo` writes a labelled sample dataset behind a Settings button, records
//! every row in a ledger, and can take it all back out again. This module writes
//! **the church's own content**: no `Demo · ` prefix, no ledger, no removal
//! path. An operator edits or deletes a starter announcement the way they would
//! edit or delete one they typed, because after the first Sunday that is what it
//! is. `demo.rs` is untouched by this module and neither calls the other.
//!
//! ## What it will never write
//!
//! No `services`, `transcripts`, `detections`, `cues`, `service_events` or
//! `perf_samples` — the same line `demo.rs` holds, for the same reason. A
//! service record is a claim about what happened in a building on a morning, and
//! nothing a preacher said may ever reach it. Starter content is something to
//! press; it is not something that happened.

use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde_json::json;

/// The starter announcements: **(the operator's title, the words that reach the
/// room)**, per REBRAND §10.
///
/// The separation is real and it is worth being exact about what it is. The
/// title is what an operator searches the Library for and recognises at 9am; the
/// body is the message. Neither repeats the other, so an operator editing the
/// words does not have to edit them twice — and a church that renames
/// *"Children's groups"* to whatever they call it changes nothing a congregation
/// reads.
///
/// What it is NOT: the title is not invisible to the room. `fire_content` sends
/// an announce cue's label out as `reference`, so an announce template with a
/// `reference`-bound layer draws it as the slide's heading. That is the existing
/// behaviour of the fire path and this module does not change it; these titles
/// are written to read correctly in both jobs.
pub const STARTER_ANNOUNCEMENTS: &[(&str, &str)] = &[
    ("Welcome", "We are glad you are here with us this morning."),
    (
        "Children's groups",
        "Children are welcome to go through to their groups now.",
    ),
    (
        "Giving",
        "There is a box at the back for anyone who would like to give.",
    ),
    (
        "Prayer meeting",
        "Wednesday at seven, in the hall. Everyone is welcome.",
    ),
    (
        "After the service",
        "Tea and coffee are served at the back. Please stay a while.",
    ),
];

/// The example plan's title. A church renames it or deletes it; what it may not
/// do is wonder whether Relay built it or they did.
pub const STARTER_PLAN_TITLE: &str = "Sunday Morning (example)";

/// The `seed_key` of the Timer look the example countdown cue pins.
///
/// Pinned by IDENTITY rather than by id or name, because Track A's whole point
/// is that a seeded row keeps an identity a rename or a conversion cannot
/// destroy (RG-142). `timer.titled` is the one whose `reference`-bound layer
/// draws the words a countdown cue carries, so a cue with words and a look that
/// cannot show them would be the feature demonstrating its own absence.
const COUNTDOWN_SEED_KEY: &str = "timer.titled";

/// The `path` prefix that marks a media row as a picture Relay ships rather than
/// a file the operator imported.
///
/// An imported asset's `path` is an absolute path in the media directory and its
/// bytes are served from there. A bundled one has no file of its own on disk:
/// it lives in the embedded bundle and is served straight out of it, so the
/// prefix is what stops `delete_media`'s caller trying to unlink something that
/// was never a file.
pub const BUNDLED_PREFIX: &str = "bundled:";

/// The stored `path` for a bundled picture.
pub fn bundled_path(file: &str) -> String {
    format!(
        "{BUNDLED_PREFIX}{}/{file}",
        crate::channels::BUNDLED_BACKGROUND_DIR
    )
}

/// Seed the starter set. Idempotent and retryable (rule 25): every piece asks
/// whether it is already there before writing, so a second call is a no-op and a
/// third is identical to the second.
///
/// It is called from `init_fresh` only. An existing install does not acquire
/// starter content on an update — a church that has been running Relay for a
/// year does not want five notices and an example plan appearing in a library
/// they have curated, and an update is not a first launch.
pub(super) fn seed_starter(conn: &Connection) -> rusqlite::Result<()> {
    let announcements = seed_announcements(conn)?;
    seed_bundled_backgrounds(conn)?;
    seed_example_plan(conn, &announcements)?;
    Ok(())
}

/// The announcements, returning the id of each by position so the plan can cue
/// one. Skips any whose title is already present.
fn seed_announcements(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    let mut ids = Vec::with_capacity(STARTER_ANNOUNCEMENTS.len());
    for (title, body) in STARTER_ANNOUNCEMENTS {
        if let Some(id) = announcement_id(conn, title)? {
            ids.push(id);
            continue;
        }
        ids.push(super::library::save_announcement(
            conn, None, title, body, "",
        )?);
    }
    Ok(ids)
}

fn announcement_id(conn: &Connection, title: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM announcements WHERE title = ?1",
        [title],
        |r| r.get(0),
    )
    .optional()
}

/// One `media_assets` row per picture Relay ships.
///
/// THIS IS THE GAP THE ROWS CLOSE. The template editor's background picker reads
/// `BACKGROUNDS` — a build-time glob — and cannot reach `media_assets` at all,
/// so a picture that ships with Relay was usable in exactly one place and a
/// picture in the Library was usable everywhere else. With a row each, a
/// background is an ordinary library item: it can be fired, put in a plan, or
/// bound to a template's media layer.
///
/// The row carries no file of its own. `path` is `bundled:backgrounds/<file>`
/// and the bytes are already in the binary, inside `dist/`, where the embedded
/// HTTP server serves them to every screen. Copying them into the media
/// directory would put fourteen megabytes on disk that are already in the
/// binary, and embedding them a second time would put fourteen megabytes into
/// every download.
fn seed_bundled_backgrounds(conn: &Connection) -> rusqlite::Result<()> {
    for file in crate::channels::bundled_backgrounds() {
        let path = bundled_path(file);
        let present: Option<i64> = conn
            .query_row(
                "SELECT id FROM media_assets WHERE path = ?1",
                [&path],
                |r| r.get(0),
            )
            .optional()?;
        if present.is_some() {
            continue;
        }
        let id = super::library::insert_media(conn, "image", file, "", None)?;
        super::library::set_media_path(conn, id, &path)?;
    }
    Ok(())
}

/// The example plan: three cues, one of each of the kinds a first Sunday
/// actually needs in front of the sermon.
///
/// It is deliberately short. A plan is the surface an operator builds on, and a
/// long running order Relay invented is more to delete than to learn from. The
/// countdown is the point of it: it carries its own words and its own length,
/// which is the Planner interface wave 5 gave it (Track G), and pins the Timer
/// look that can draw them.
fn seed_example_plan(conn: &Connection, announcements: &[i64]) -> rusqlite::Result<()> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM service_plans WHERE title = ?1",
            [STARTER_PLAN_TITLE],
            |r| r.get(0),
        )
        .optional()?;
    if existing.is_some() {
        return Ok(());
    }
    // No date. A plan Relay wrote is not scheduled for a morning, and a date
    // here would be a claim about a Sunday nobody chose.
    let plan_id = super::plans::create_plan(conn, STARTER_PLAN_TITLE, "")?;

    let countdown_tpl = conn
        .query_row(
            "SELECT id FROM templates WHERE seed_key = ?1",
            [COUNTDOWN_SEED_KEY],
            |r| r.get::<_, i64>(0),
        )
        .optional()?;

    let push = |cue_type: &str,
                label: &str,
                payload: serde_json::Value,
                section: &str,
                seconds: i64,
                template_id: Option<i64>|
     -> rusqlite::Result<()> {
        let id = super::plans::add_plan_item(
            conn,
            plan_id,
            cue_type,
            label,
            &payload.to_string(),
            template_id,
        )?;
        super::plans::set_plan_section(conn, id, section)?;
        if seconds > 0 {
            super::plans::set_plan_duration(conn, id, seconds)?;
        }
        Ok(())
    };

    // THE COUNTDOWN CARRIES ITS OWN WORDS. They used to be two constants nobody
    // could edit; the Planner now has a field for each and blank is a real
    // answer that puts the digits on the wall alone. An example that left both
    // empty would demonstrate the absence of the feature rather than the
    // feature, so this one says something and a church edits it.
    push(
        "countdown",
        "Countdown · 10 min",
        json!({ "minutes": 10, "label": "Service begins in", "done": "Welcome" }),
        "Gathering",
        600,
        countdown_tpl,
    )?;

    if let Some(id) = announcements.first() {
        let (title, body) = STARTER_ANNOUNCEMENTS[0];
        push(
            "announce",
            title,
            json!({ "announce_id": id, "title": title, "body": body }),
            "Gathering",
            120,
            None,
        )?;
    }

    // Untimed on purpose: scripture is fired when the preacher reaches it, not
    // on a clock, and the Planner renders a zero as an em dash.
    push(
        "scripture",
        "Psalms 100:1-5",
        scripture_payload(conn, "Psalms", 100, 1, Some("Psalms 100:1-5"))?,
        "The Word",
        0,
        None,
    )?;
    Ok(())
}

/// A scripture cue's payload, read out of the bundled corpus. Same shape
/// `src/lib/cues.js` builds, and the same shape the demo dataset writes — the
/// two are deliberately independent copies of a payload format rather than a
/// shared helper, because `demo.rs` is untouched by this wave.
fn scripture_payload(
    conn: &Connection,
    book: &str,
    chapter: i64,
    verse: i64,
    reference: Option<&str>,
) -> rusqlite::Result<serde_json::Value> {
    let v = super::verses::lookup_verse(conn, book, chapter, verse)?;
    let (text, translation, anchor) = match &v {
        Some(row) => (
            row.text.clone(),
            row.translation.clone(),
            row.reference.clone(),
        ),
        None => (
            String::new(),
            String::new(),
            format!("{book} {chapter}:{verse}"),
        ),
    };
    Ok(json!({
        "book": book,
        "chapter": chapter,
        "verse": verse,
        "reference": reference.unwrap_or(&anchor),
        "text": text,
        "translation": translation,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn fresh() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::init_fresh(&conn).unwrap();
        conn
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    /// The announcements, by identity. Title and body are two jobs and the test
    /// asserts both, because a starter set whose body repeated its title would
    /// look fine in a list and read like a stutter on a wall.
    #[test]
    fn a_first_launch_carries_announcements_a_church_can_fire_or_edit() {
        let conn = fresh();
        let rows = db::list_announcements(&conn).unwrap();
        assert_eq!(rows.len(), 5, "the starter announcements");
        let mut titles: Vec<&str> = rows.iter().map(|a| a.title.as_str()).collect();
        titles.sort_unstable();
        assert_eq!(
            titles,
            vec![
                "After the service",
                "Children's groups",
                "Giving",
                "Prayer meeting",
                "Welcome",
            ]
        );
        for a in &rows {
            assert!(
                !a.body.trim().is_empty(),
                "{}: no words for the room",
                a.title
            );
            assert_ne!(
                a.body.trim(),
                a.title.trim(),
                "{}: the operator's title and the room's words are the same string",
                a.title
            );
        }
    }

    /// Every seeded picture points at something a screen can actually load.
    ///
    /// This is the assertion that makes the whole approach honest: the row says
    /// `bundled:backgrounds/<file>` and the bundle is asked whether that file is
    /// in it. If the build stopped emitting them at a stable path, or sanitised
    /// a name differently, every one of these fails rather than shipping a
    /// library full of pictures that 404 on a Sunday.
    #[test]
    fn every_bundled_picture_is_a_row_that_resolves_to_a_real_file() {
        let conn = fresh();
        let media = db::list_media(&conn).unwrap();
        assert!(
            !media.is_empty(),
            "no bundled pictures were seeded — was `npm run build` run before \
             this crate was compiled? (RG-127, and `dist/backgrounds/` is where \
             they live)"
        );
        assert_eq!(
            media.len(),
            crate::channels::bundled_backgrounds().len(),
            "a picture in the bundle with no row, or a row with no picture"
        );
        for m in &media {
            assert_eq!(m.kind, "image");
            let Some(rest) = m.path.strip_prefix(BUNDLED_PREFIX) else {
                panic!(
                    "{} is not marked as a bundled picture: {}",
                    m.filename, m.path
                );
            };
            assert!(
                crate::channels::bundle_holds(rest),
                "{} points at {rest}, which is not in the bundle",
                m.filename
            );
            assert!(
                rest.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/')),
                "{rest} needs URL-encoding, and the embedded server does not decode"
            );
        }
    }

    /// The example plan, by shape. A countdown that carries its own words and
    /// its own length, an announcement, and a passage.
    #[test]
    fn a_first_launch_carries_one_example_plan_with_a_countdown_that_has_words() {
        let conn = fresh();
        let plans = db::list_plans(&conn).unwrap();
        assert_eq!(plans.len(), 1, "one example plan and no more");
        assert_eq!(plans[0].title, STARTER_PLAN_TITLE);

        let items = db::plan_items(&conn, plans[0].id).unwrap();
        let kinds: Vec<&str> = items.iter().map(|i| i.cue_type.as_str()).collect();
        assert_eq!(kinds, vec!["countdown", "announce", "scripture"]);

        let cd = &items[0];
        let p: serde_json::Value = serde_json::from_str(&cd.payload_json).unwrap();
        assert_eq!(p["minutes"], 10);
        assert_eq!(p["label"], "Service begins in");
        assert_eq!(p["done"], "Welcome");
        assert_eq!(
            cd.duration_sec, 600,
            "the running-time estimate a countdown knows at build time"
        );
        // Pinned to the Timer look whose reference-bound layer draws the words.
        let titled: i64 = conn
            .query_row(
                "SELECT id FROM templates WHERE seed_key = 'timer.titled'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cd.template_id, Some(titled));

        // The passage is real text out of the bundled corpus, not a placeholder.
        let sp: serde_json::Value = serde_json::from_str(&items[2].payload_json).unwrap();
        assert!(sp["text"].as_str().unwrap().contains("joyful"));
        assert_eq!(sp["reference"], "Psalms 100:1-5");
    }

    /// RULE 25. A second run writes nothing, and a third is identical to the
    /// second.
    #[test]
    fn seeding_the_starter_set_again_changes_nothing() {
        let conn = fresh();
        let before = (
            count(&conn, "SELECT COUNT(*) FROM announcements"),
            count(&conn, "SELECT COUNT(*) FROM media_assets"),
            count(&conn, "SELECT COUNT(*) FROM service_plans"),
            count(&conn, "SELECT COUNT(*) FROM plan_items"),
        );
        seed_starter(&conn).unwrap();
        seed_starter(&conn).unwrap();
        let after = (
            count(&conn, "SELECT COUNT(*) FROM announcements"),
            count(&conn, "SELECT COUNT(*) FROM media_assets"),
            count(&conn, "SELECT COUNT(*) FROM service_plans"),
            count(&conn, "SELECT COUNT(*) FROM plan_items"),
        );
        assert_eq!(before, after, "the starter set doubled");
    }

    /// THE SECOND LAUNCH, ON THE REAL PATH, THROUGH A REAL FILE.
    ///
    /// The test above proves the seeding step itself is idempotent. This one
    /// proves the thing a church would actually experience: Relay is opened,
    /// closed, and opened again. `db::open` decides `fresh` by whether the file
    /// exists and hands that to `migrate`, so the second launch goes down the
    /// migration branch — and a starter set that was written by a migration, or
    /// by a forward-fill, or by anything else that runs every boot, would double
    /// exactly here and nowhere else.
    #[test]
    fn opening_relay_a_second_time_does_not_double_the_starter_set() {
        let path = std::env::temp_dir().join(format!(
            "relay-starter-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);

        let first = Connection::open(&path).unwrap();
        first.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::migrate(&first, true).unwrap();
        let after_first = (
            count(&first, "SELECT COUNT(*) FROM announcements"),
            count(&first, "SELECT COUNT(*) FROM media_assets"),
            count(&first, "SELECT COUNT(*) FROM service_plans"),
            count(&first, "SELECT COUNT(*) FROM plan_items"),
        );
        assert_eq!(after_first.0, STARTER_ANNOUNCEMENTS.len() as i64);
        assert_eq!(after_first.2, 1);
        drop(first);

        // Second launch: the file is there, so `open` passes `fresh = false`.
        let second = Connection::open(&path).unwrap();
        second.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::migrate(&second, false).unwrap();
        let after_second = (
            count(&second, "SELECT COUNT(*) FROM announcements"),
            count(&second, "SELECT COUNT(*) FROM media_assets"),
            count(&second, "SELECT COUNT(*) FROM service_plans"),
            count(&second, "SELECT COUNT(*) FROM plan_items"),
        );
        assert_eq!(
            after_first, after_second,
            "the second launch wrote the starter set again"
        );

        // And a church that deleted a starter notice does not get it back.
        let welcome = announcement_id(&second, "Welcome").unwrap().unwrap();
        db::delete_announcement(&second, welcome).unwrap();
        drop(second);
        let third = Connection::open(&path).unwrap();
        third.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::migrate(&third, false).unwrap();
        assert_eq!(
            count(&third, "SELECT COUNT(*) FROM announcements"),
            after_first.0 - 1,
            "a starter notice the operator deleted came back on the next launch"
        );
        drop(third);
        let _ = std::fs::remove_file(&path);
    }

    /// AND IT STILL WRITES NO SERVICE RECORD. The one line `demo.rs` holds, held
    /// here too — a starter plan is something to press, not something that
    /// happened, and `service_events.detail` is the part of the history most
    /// likely to be emailed to somebody.
    #[test]
    fn a_first_launch_has_no_history_of_a_service_that_never_happened() {
        let conn = fresh();
        for table in [
            "services",
            "transcripts",
            "detections",
            "cues",
            "service_events",
            "perf_samples",
        ] {
            let n = count(&conn, &format!("SELECT COUNT(*) FROM {table}"));
            assert_eq!(n, 0, "a fresh install has a row in {table}");
        }
        assert!(
            !db::demo::is_loaded(&conn).unwrap(),
            "starter content is not demo content, and must not touch its ledger"
        );
    }
}
