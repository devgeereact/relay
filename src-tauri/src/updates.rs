//! UPDATE SAFETY — snapshot before, verify after, and a way back if it went wrong.
//!
//! Single responsibility: make an app update safe to attempt, and recoverable when
//! it is not.
//!
//! ## What "rollback" honestly means here
//!
//! **The binary is replaceable. The church's data is not.** That asymmetry decides
//! the whole design.
//!
//! A previous *version* of Relay can always be got back: the installers are public,
//! they are signed, and reinstalling one is a five-minute job somebody can do from a
//! release page. Nobody needs this module for that, and pretending to keep a second
//! copy of the app bundle would be a large amount of machinery for the problem that
//! is already solved.
//!
//! What cannot be got back is the **database** — every service, transcript, plan,
//! song, saved verse and template a church has built up — if a migration in a new
//! version goes wrong on their particular data. There is no undo, there is no copy
//! anywhere else (that is what offline-first means), and it happens on the launch
//! *after* the update, when the person who pressed the button has gone home.
//!
//! So this module does three things, in the order they matter:
//!
//! 1. **Preflight.** Refuse to start an update onto a database that is not already
//!    healthy. Migrating a half-migrated database is how the §25 failure happened,
//!    and an update is exactly when a pending problem gets stepped on.
//! 2. **Snapshot.** Take a consistent copy of the database immediately before, and
//!    record which version it came from.
//! 3. **Verify, then offer the way back.** On the first launch after an update,
//!    check the database is still what it should be. If it is not, say so plainly
//!    and offer to restore the snapshot — rather than the operator discovering it
//!    with a congregation in the room.
//!
//! ## Why a restore happens at startup, not on demand
//!
//! Copying a file over a database that is open is how you get a corrupt database
//! *and* a corrupt backup. A restore is therefore a REQUEST — a marker file — and
//! `db::open` acts on it before it opens anything. That is the only moment the file
//! is provably unused.

use crate::db;
use rusqlite::Connection;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// How many pre-update snapshots to keep.
///
/// Three, not one: a church that updates twice in a fortnight and only notices the
/// damage on the second Sunday needs the snapshot from *before* the first update,
/// not after it. Not thirty, because these are full copies of the database and disk
/// on a church laptop is finite.
pub const KEEP_SNAPSHOTS: usize = 3;

/// Wall-clock milliseconds, for naming a snapshot. Deliberately not the monotonic
/// clock `latency.rs` uses: a filename has to still make sense to a person reading
/// a directory listing next year.
fn epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Settings keys. Namespaced, so a glance at `app_settings` says which subsystem
/// owns a row.
const K_FROM: &str = "update.from_version";
const K_SNAPSHOT: &str = "update.snapshot";
const K_SCHEMA: &str = "update.from_schema";
const K_STARTED: &str = "update.started_at";

/// One preflight answer. Mirrors the boot checks' severity ladder deliberately —
/// an operator has seen this shape before, on the launch screen.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub id: &'static str,
    pub label: &'static str,
    /// `ok` · `warn` · `fail`. Only `fail` stops an update.
    pub state: &'static str,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preflight {
    /// True when nothing failed. A `warn` never blocks: an update that refuses over
    /// something survivable is an update a church stops attempting.
    pub ok: bool,
    pub checks: Vec<Check>,
}

fn check(
    id: &'static str,
    label: &'static str,
    state: &'static str,
    note: impl Into<String>,
) -> Check {
    Check {
        id,
        label,
        state,
        note: note.into(),
    }
}

/// Is this database in a state it is safe to update on top of?
///
/// **Only database health can FAIL here.** Everything else is reported and lets the
/// update proceed, because the alternative — a church that cannot take a security
/// fix because their disk is 80% full — is worse than the thing being guarded
/// against.
pub fn preflight(conn: &Connection, free_disk_bytes: u64) -> Preflight {
    let mut checks = Vec::new();

    match db::schema_report(conn) {
        Ok((version, expected, rows)) => {
            let missing: Vec<&str> = rows
                .iter()
                .filter(|(_, _, present)| !present)
                .map(|(_, t, _)| *t)
                .collect();
            if version != expected {
                checks.push(check(
                    "schema",
                    "Database version",
                    "fail",
                    format!(
                        "this database is at v{version} and this build expects v{expected}. \
                         Updating on top of a half-migrated database is how one gets broken."
                    ),
                ));
            } else if !missing.is_empty() {
                checks.push(check(
                    "schema",
                    "Database version",
                    "fail",
                    format!("missing: {}", missing.join(", ")),
                ));
            } else {
                checks.push(check(
                    "schema",
                    "Database version",
                    "ok",
                    format!("v{version} — matches this build"),
                ));
            }
        }
        Err(e) => checks.push(check(
            "schema",
            "Database version",
            "fail",
            format!("the database would not answer ({e})"),
        )),
    }

    // A leftover scratch table means a previous rebuild did not finish. Rule 25: the
    // next boot fails on "table already exists", forever, before the window is shown.
    match db::manual_status_report(conn) {
        Ok((manual, scratch)) => {
            if scratch {
                checks.push(check(
                    "scratch",
                    "Previous migration",
                    "fail",
                    "a `detections_new` table was left behind by a rebuild that did not finish",
                ));
            } else if !manual {
                checks.push(check(
                    "scratch",
                    "Previous migration",
                    "fail",
                    "`detections.status` cannot record a human's fire — the last rebuild did not complete",
                ));
            } else {
                checks.push(check(
                    "scratch",
                    "Previous migration",
                    "ok",
                    "completed cleanly",
                ));
            }
        }
        Err(e) => checks.push(check(
            "scratch",
            "Previous migration",
            "fail",
            e.to_string(),
        )),
    }

    // Room for the snapshot AND the download. Warned about, never blocking.
    let db_bytes = std::fs::metadata(db::db_path())
        .map(|m| m.len())
        .unwrap_or(0);
    let need = db_bytes.saturating_mul(2).saturating_add(300_000_000);
    checks.push(if free_disk_bytes == 0 {
        check(
            "disk",
            "Disk space",
            "warn",
            "could not read the free space on this volume",
        )
    } else if free_disk_bytes < need {
        check(
            "disk",
            "Disk space",
            "warn",
            format!(
                "{:.1} GB free — a copy of your database plus the download wants about {:.1} GB",
                free_disk_bytes as f64 / 1e9,
                need as f64 / 1e9
            ),
        )
    } else {
        check(
            "disk",
            "Disk space",
            "ok",
            format!("{:.1} GB free", free_disk_bytes as f64 / 1e9),
        )
    });

    let ok = !checks.iter().any(|c| c.state == "fail");
    Preflight { ok, checks }
}

/// Where snapshots live. Beside the database, never inside a webroot or anywhere
/// synced.
pub fn snapshot_dir() -> PathBuf {
    db::app_data_dir().join("snapshots")
}

/// Take a consistent copy of the database and record what it came from.
///
/// `VACUUM INTO` rather than copying the file: it produces a coherent database even
/// with the connection open and writes in flight, which a filesystem copy does not.
/// A backup that is a torn copy of a live database is worse than none, because it
/// will be trusted.
pub fn begin(conn: &Connection, from_version: &str) -> rusqlite::Result<PathBuf> {
    let dir = snapshot_dir();
    let _ = std::fs::create_dir_all(&dir);
    let stamp = epoch_ms();
    let safe: String = from_version
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = dir.join(format!("pre-update-{safe}-{stamp}.db"));

    // `VACUUM INTO` refuses an existing file, which is what we want — never
    // overwrite a snapshot.
    conn.execute("VACUUM INTO ?1", [path.to_string_lossy().as_ref()])?;

    let (version, _, _) = db::schema_report(conn)?;
    db::set_setting(conn, K_FROM, from_version)?;
    db::set_setting(conn, K_SNAPSHOT, &path.to_string_lossy())?;
    db::set_setting(conn, K_SCHEMA, &version.to_string())?;
    db::set_setting(conn, K_STARTED, &epoch_ms().to_string())?;

    prune(&dir);
    Ok(path)
}

/// Keep the newest `KEEP_SNAPSHOTS`. Best-effort: failing to tidy up must never
/// fail an update.
fn prune(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "db")
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("pre-update-"))
        })
        .collect();
    // Newest first. The timestamp is in the name, so a lexical sort is a time sort.
    files.sort();
    files.reverse();
    for old in files.into_iter().skip(KEEP_SNAPSHOTS) {
        let _ = std::fs::remove_file(old);
    }
}

/// An update that was started and has not been confirmed to have landed well.
#[derive(Debug, Clone, Serialize)]
pub struct Pending {
    pub from_version: String,
    pub snapshot: String,
    pub from_schema: i64,
}

pub fn pending(conn: &Connection) -> Option<Pending> {
    let from_version = db::get_setting(conn, K_FROM).ok().flatten()?;
    let snapshot = db::get_setting(conn, K_SNAPSHOT).ok().flatten()?;
    let from_schema = db::get_setting(conn, K_SCHEMA)
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    Some(Pending {
        from_version,
        snapshot,
        from_schema,
    })
}

/// Forget a pending update — it landed, or the operator dismissed it.
pub fn clear(conn: &Connection) -> rusqlite::Result<()> {
    for k in [K_FROM, K_SNAPSHOT, K_SCHEMA, K_STARTED] {
        db::set_setting(conn, k, "")?;
    }
    Ok(())
}

/// What the first launch after an update found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    /// Nothing was being updated.
    Idle,
    /// An update was started and this is still the old version — it did not install.
    /// Not a fault: the operator may simply have quit before restarting.
    DidNotInstall { from_version: String },
    /// The new version is running and the database is healthy.
    Landed { from_version: String },
    /// The new version is running and the database is NOT healthy. The one case this
    /// whole module exists for.
    Broken {
        from_version: String,
        snapshot: String,
        reason: String,
    },
}

/// Check, on the launch after an update, whether it actually worked.
///
/// Called with the version this binary reports. Deliberately does NOT act on its own
/// conclusion: a restore replaces a church's entire history, and that is a decision
/// with a person's name on it, not an automatic one.
pub fn verify(conn: &Connection, current_version: &str) -> Verdict {
    let Some(p) = pending(conn).filter(|p| !p.from_version.is_empty()) else {
        return Verdict::Idle;
    };
    if p.from_version == current_version {
        return Verdict::DidNotInstall {
            from_version: p.from_version,
        };
    }
    // The same health question the preflight asked, now on the other side.
    let health = preflight(conn, u64::MAX);
    match health.checks.iter().find(|c| c.state == "fail") {
        Some(bad) => Verdict::Broken {
            from_version: p.from_version,
            snapshot: p.snapshot,
            reason: bad.note.clone(),
        },
        None => Verdict::Landed {
            from_version: p.from_version,
        },
    }
}

/// The marker `db::open` looks for BEFORE it opens anything.
pub fn restore_marker() -> PathBuf {
    db::app_data_dir().join("restore-me")
}

/// Ask for a snapshot to be restored on the next launch.
///
/// A request, not an action. Copying a file over an open database is how you get a
/// corrupt database *and* a corrupt backup; the only moment the file is provably
/// unused is before it has been opened.
pub fn request_restore(snapshot: &Path) -> std::io::Result<()> {
    request_restore_at(&restore_marker(), snapshot)
}

/// The marker path is a parameter so this is testable without writing into the
/// operator's real app-data — and so two tests cannot race over one global file,
/// which is exactly how the first version of these tests failed.
pub fn request_restore_at(marker: &Path, snapshot: &Path) -> std::io::Result<()> {
    if !snapshot.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "that snapshot is no longer on this machine",
        ));
    }
    if let Some(dir) = marker.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(marker, snapshot.to_string_lossy().as_bytes())
}

/// Act on a pending restore request. Called by `db::open` before the database is
/// opened, and by nothing else.
///
/// The database being replaced is itself copied aside first. A restore that loses
/// the state it replaced would make "try the restore" an irreversible gamble, and an
/// operator would be right never to press it.
pub fn take_restore(db_path: &Path) -> Option<PathBuf> {
    take_restore_at(&restore_marker(), db_path)
}

pub fn take_restore_at(marker: &Path, db_path: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(marker).ok()?;
    // Consume the marker FIRST. A restore that fails must not be retried on every
    // subsequent launch — that would turn one bad update into a machine that will
    // not start.
    let _ = std::fs::remove_file(marker);

    let snapshot = PathBuf::from(raw.trim());
    if !snapshot.is_file() {
        return None;
    }
    if db_path.exists() {
        let aside = db_path.with_extension(format!("replaced-{}.db", epoch_ms()));
        let _ = std::fs::copy(db_path, aside);
    }
    std::fs::copy(&snapshot, db_path).ok()?;
    Some(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A database with the shape `preflight` interrogates.
    fn healthy() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::migrate(&conn, true).unwrap();
        conn
    }

    #[test]
    fn a_healthy_database_passes() {
        let p = preflight(&healthy(), 50_000_000_000);
        assert!(p.ok, "{:?}", p.checks);
        assert!(p.checks.iter().all(|c| c.state == "ok"), "{:?}", p.checks);
    }

    /// A HALF-MIGRATED DATABASE STOPS THE UPDATE.
    ///
    /// This is the one thing that fails, and it is the §25 failure a step earlier:
    /// running a new version's migrations over a database whose last rebuild did not
    /// finish is how a church ends up with an app that will not start.
    #[test]
    fn a_leftover_scratch_table_stops_an_update() {
        let conn = healthy();
        conn.execute_batch("CREATE TABLE detections_new (id INTEGER PRIMARY KEY)")
            .unwrap();
        let p = preflight(&conn, 50_000_000_000);
        assert!(!p.ok);
        let bad = p.checks.iter().find(|c| c.state == "fail").unwrap();
        assert_eq!(bad.id, "scratch");
        assert!(bad.note.contains("did not finish"));
    }

    /// LOW DISK WARNS AND LETS THE UPDATE PROCEED.
    ///
    /// An update that refuses over something survivable is an update a church stops
    /// attempting — and the next one they skip may be the security fix.
    #[test]
    fn a_full_disk_warns_but_never_blocks() {
        let p = preflight(&healthy(), 1);
        assert!(p.ok, "a warning must not stop an update");
        assert_eq!(
            p.checks.iter().find(|c| c.id == "disk").unwrap().state,
            "warn"
        );
    }

    #[test]
    fn nothing_is_pending_on_a_fresh_install() {
        let conn = healthy();
        assert!(pending(&conn).is_none() || pending(&conn).unwrap().from_version.is_empty());
        assert_eq!(verify(&conn, "0.1.0-4"), Verdict::Idle);
    }

    /// THE SAME VERSION AFTER AN ATTEMPT IS NOT A FAILURE.
    ///
    /// The operator may simply have quit before restarting. Reporting that as a
    /// broken update would send somebody restoring a database over a perfectly good
    /// one — which is a far worse outcome than the thing being reported.
    #[test]
    fn an_update_that_never_installed_is_not_reported_as_broken() {
        let conn = healthy();
        db::set_setting(&conn, K_FROM, "0.1.0-4").unwrap();
        db::set_setting(&conn, K_SNAPSHOT, "/tmp/x.db").unwrap();
        assert_eq!(
            verify(&conn, "0.1.0-4"),
            Verdict::DidNotInstall {
                from_version: "0.1.0-4".into()
            }
        );
    }

    #[test]
    fn an_update_onto_a_healthy_database_reads_as_landed() {
        let conn = healthy();
        db::set_setting(&conn, K_FROM, "0.1.0-4").unwrap();
        db::set_setting(&conn, K_SNAPSHOT, "/tmp/x.db").unwrap();
        assert_eq!(
            verify(&conn, "0.2.0"),
            Verdict::Landed {
                from_version: "0.1.0-4".into()
            }
        );
    }

    /// THE CASE THE WHOLE MODULE EXISTS FOR.
    #[test]
    fn a_new_version_on_a_broken_database_offers_the_snapshot_back() {
        let conn = healthy();
        db::set_setting(&conn, K_FROM, "0.1.0-4").unwrap();
        db::set_setting(&conn, K_SNAPSHOT, "/tmp/relay-snap.db").unwrap();
        conn.execute_batch("CREATE TABLE detections_new (id INTEGER PRIMARY KEY)")
            .unwrap();
        match verify(&conn, "0.2.0") {
            Verdict::Broken {
                from_version,
                snapshot,
                reason,
            } => {
                assert_eq!(from_version, "0.1.0-4");
                assert_eq!(snapshot, "/tmp/relay-snap.db");
                assert!(
                    !reason.is_empty(),
                    "the operator must be told what is wrong"
                );
            }
            other => panic!("expected Broken, got {other:?}"),
        }
    }

    #[test]
    fn clearing_forgets_the_pending_update() {
        let conn = healthy();
        db::set_setting(&conn, K_FROM, "0.1.0-4").unwrap();
        db::set_setting(&conn, K_SNAPSHOT, "/tmp/x.db").unwrap();
        clear(&conn).unwrap();
        assert_eq!(verify(&conn, "0.2.0"), Verdict::Idle);
    }

    /// A RESTORE REQUEST FOR A SNAPSHOT THAT IS GONE IS REFUSED AT THE ASK.
    ///
    /// Not at the next launch, where the app is already restarting and there is
    /// nobody to tell.
    #[test]
    fn a_missing_snapshot_cannot_be_requested() {
        let missing = std::env::temp_dir().join("relay-no-such-snapshot.db");
        let _ = std::fs::remove_file(&missing);
        assert!(request_restore(&missing).is_err());
    }

    /// THE MARKER IS CONSUMED EVEN IF THE RESTORE FAILS.
    ///
    /// Otherwise one bad update becomes a machine that will not start: every launch
    /// retries the same failing restore, forever.
    #[test]
    fn a_failed_restore_does_not_repeat_forever() {
        let dir = std::env::temp_dir().join(format!("relay-restore-{}", epoch_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("restore-me");
        std::fs::write(&marker, dir.join("gone.db").to_string_lossy().as_bytes()).unwrap();

        let target = dir.join("relay.db");
        assert!(
            take_restore_at(&marker, &target).is_none(),
            "a missing snapshot restores nothing"
        );
        assert!(!marker.exists(), "the marker must be consumed regardless");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A RESTORE KEEPS WHAT IT REPLACED.
    ///
    /// Otherwise "try the restore" is an irreversible gamble, and an operator would
    /// be right never to press it.
    #[test]
    fn restoring_copies_the_replaced_database_aside_first() {
        let dir = std::env::temp_dir().join(format!("relay-restore2-{}", epoch_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let snap = dir.join("snap.db");
        let live = dir.join("relay.db");
        std::fs::write(&snap, b"SNAPSHOT").unwrap();
        std::fs::write(&live, b"BROKEN-LIVE").unwrap();

        let marker = dir.join("restore-me");
        request_restore_at(&marker, &snap).unwrap();
        assert!(take_restore_at(&marker, &live).is_some());
        assert_eq!(std::fs::read(&live).unwrap(), b"SNAPSHOT");

        let aside: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("replaced-"))
            .collect();
        assert_eq!(aside.len(), 1, "the database being replaced must be kept");
        assert_eq!(std::fs::read(aside[0].path()).unwrap(), b"BROKEN-LIVE");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_the_newest_snapshots_are_kept() {
        let dir = std::env::temp_dir().join(format!("relay-prune-{}", epoch_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..6 {
            std::fs::write(dir.join(format!("pre-update-0.1.0-{i}-100{i}.db")), b"x").unwrap();
        }
        // Something else living beside them must survive.
        std::fs::write(dir.join("relay.db"), b"live").unwrap();
        prune(&dir);
        let left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            left.iter().filter(|n| n.starts_with("pre-update-")).count(),
            KEEP_SNAPSHOTS
        );
        assert!(
            left.iter().any(|n| n == "relay.db"),
            "only snapshots are pruned"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
