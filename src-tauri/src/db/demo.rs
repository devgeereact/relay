//! The demo dataset, and the ledger that lets it be taken back.
//!
//! Single responsibility: write one known set of sample content into a real
//! database, remember exactly which rows it wrote, and be able to remove them
//! again without touching anything an operator made.
//!
//! ## It never loads itself
//!
//! Nothing in `init_fresh`, `migrate`, `baseline_forward_fill` or `run_migrations`
//! calls [`load`]. The only caller is the `load_demo_content` command, behind a
//! button in Settings → Getting started. A fresh install has the `demo_content`
//! table and nothing in it — held from two directions by
//! `qa::the_bare_fixture_is_a_first_launch_and_nothing_more` (which asserts what a
//! first launch contains) and by `a_fresh_install_carries_no_demo_content` below
//! (which asserts this module did not run).
//!
//! ## The ledger is the marker, not the name
//!
//! Every seeded row is recorded as `(table_name, row_id)` plus a fingerprint. The
//! titles do carry a visible `Demo · ` so a church can tell sample content from
//! their own at a glance — but an operator may rename anything, so a removal keyed
//! on a prefix would either miss a renamed demo row or delete one of theirs that
//! happened to be called the same thing. A row id cannot be renamed.
//!
//! ## What happens to a demo item the operator has since edited
//!
//! It is **kept**, and released from the ledger so it becomes theirs.
//!
//! `fingerprint` hashes the row as it was seeded, including the parts an operator
//! reaches through a different surface — a plan's cues, a song's sections and
//! arrangements. On removal each row is fingerprinted again: unchanged rows are
//! deleted, changed ones are kept. This product has no undo, and a church that
//! retyped the welcome notice into the demo announcement and then pressed *Remove*
//! would have no way back. The report says how many of each, so "kept" is never
//! silent.
//!
//! ## What it deliberately does NOT write
//!
//! No `services`, `transcripts`, `detections`, `cues`, `service_events` or
//! `perf_samples`. A fabricated service record is a claim about what happened in a
//! building on a morning, and the history surface is where a church looks to answer
//! exactly that. Demo content is something to press; it is not something that
//! happened. Pinned by `demo_content_never_fabricates_a_service_record`.

use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;

/// The prefix every demo row carries in the field an operator reads.
///
/// Honesty at a glance, and nothing more — see the module note. It is written out
/// literally in each name below (a `const` cannot be spliced into another one), so
/// this is the RULE's copy rather than the source: `every_titled_demo_row_is_-
/// visibly_demo_content` holds every seeded title against it, which is what stops a
/// new item being added without one. Test-only for exactly that reason.
///
/// `saved_scripture` is the one seeded table with no such field: its titles are
/// Bible references and a reference that said `Demo ·` would be a lie about the
/// reference. Those rows are marked in the ledger only, and the Settings panel
/// names the exception in words.
#[cfg(test)]
const MARK: &str = "Demo · ";

const PLANS: &str = "service_plans";
const SONGS: &str = "songs";
const ANNOUNCEMENTS: &str = "announcements";
const SAVED_SCRIPTURE: &str = "saved_scripture";
const MEDIA: &str = "media_assets";

/// The tables this module writes, in the order a removal walks them.
///
/// Plans first: `delete_media` also deletes every cue that pointed at the asset,
/// so removing the plan before the asset means that sweep has nothing left to do
/// and can never reach into a plan that was kept.
const TABLES: &[&str] = &[PLANS, SONGS, ANNOUNCEMENTS, SAVED_SCRIPTURE, MEDIA];

/// The plan's title, and the one string the Settings panel quotes back.
pub const PLAN_TITLE: &str = "Demo · Sunday Morning Service";

/// What is loaded right now, as the operator's panel renders it.
#[derive(Debug, Clone, Serialize, Default)]
pub struct DemoStatus {
    /// Any demo row at all is still in the ledger.
    pub loaded: bool,
    /// Rows the ledger still holds.
    pub total: i64,
    /// Of those, how many have been edited since they were seeded. These will be
    /// KEPT by a removal, not deleted.
    pub edited: i64,
    /// One line per table, for the panel. `what` is already in operator words.
    pub groups: Vec<DemoGroup>,
}

/// One counted line of [`DemoStatus`].
#[derive(Debug, Clone, Serialize)]
pub struct DemoGroup {
    pub table: String,
    pub what: String,
    pub count: i64,
}

/// The outcome of a removal. Both numbers are reported; neither is inferable from
/// the other, and a silent "kept" would be a demo item a church cannot account for.
#[derive(Debug, Clone, Serialize, Default)]
pub struct DemoRemoval {
    /// Rows deleted because they were still exactly as seeded.
    pub removed: i64,
    /// Rows left alone because they had been edited, and released from the ledger.
    pub kept: i64,
    /// Files removed from the media directory (the caller deletes them).
    pub files: Vec<String>,
}

/// Create the ledger if missing. Idempotent, and retryable by construction.
///
/// A plain `CREATE TABLE IF NOT EXISTS` — no table rebuild, so there is no scratch
/// table to strand and nothing for a failed run to leave behind (CLAUDE.md rule
/// 25). Called from `ensure_tables`, which every install path reaches: a fresh one
/// through `init_fresh`, a versioned one through `migrate`, and a pre-versioning
/// one through `baseline_forward_fill`.
pub fn ensure_demo_ledger(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS demo_content (
            id          INTEGER PRIMARY KEY,
            table_name  TEXT NOT NULL,
            row_id      INTEGER NOT NULL,
            fingerprint TEXT NOT NULL,
            loaded_at   TEXT NOT NULL DEFAULT '',
            UNIQUE (table_name, row_id)
         );",
    )
}

/// Is any demo content loaded?
pub fn is_loaded(conn: &Connection) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM demo_content", [], |r| r.get(0))?;
    Ok(n > 0)
}

/// What the Settings panel renders.
pub fn status(conn: &Connection) -> rusqlite::Result<DemoStatus> {
    let mut out = DemoStatus::default();
    for table in TABLES {
        let rows = ledgered(conn, table)?;
        if rows.is_empty() {
            continue;
        }
        for (row_id, seeded) in &rows {
            match fingerprint(conn, table, *row_id)? {
                // The row is gone — the operator deleted it themselves. It is not
                // loaded and it is not edited; it simply is not there. The ledger
                // entry is tidied by the next removal.
                None => continue,
                Some(now) if now != *seeded => {
                    out.total += 1;
                    out.edited += 1;
                }
                Some(_) => out.total += 1,
            }
        }
        let present = rows
            .iter()
            .filter(|(id, _)| matches!(fingerprint(conn, table, *id), Ok(Some(_))))
            .count() as i64;
        if present > 0 {
            out.groups.push(DemoGroup {
                table: (*table).to_string(),
                what: words_for(table).to_string(),
                count: present,
            });
        }
    }
    out.loaded = is_loaded(conn)?;
    Ok(out)
}

/// The operator's word for a table.
fn words_for(table: &str) -> &'static str {
    match table {
        PLANS => "service plan",
        SONGS => "songs",
        ANNOUNCEMENTS => "announcements",
        SAVED_SCRIPTURE => "saved verses",
        MEDIA => "background image",
        _ => "items",
    }
}

/// Every `(row_id, fingerprint-as-seeded)` the ledger holds for one table.
fn ledgered(conn: &Connection, table: &str) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT row_id, fingerprint FROM demo_content WHERE table_name = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map([table], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

/// The row as it is right now, hashed. `None` means the row is no longer there.
///
/// **It reaches past the row itself on purpose.** A plan's content is its cues and
/// a song's content is its sections — an operator who reorders a demo plan or
/// retypes a verse has edited it just as surely as if they had renamed it, and a
/// fingerprint that only read the parent row would call that plan untouched and
/// delete their work.
fn fingerprint(conn: &Connection, table: &str, row_id: i64) -> rusqlite::Result<Option<String>> {
    let mut h = Sha256::new();
    let mut feed = |s: &str| {
        h.update(s.as_bytes());
        h.update([0x1f]);
    };
    match table {
        PLANS => {
            let head: Option<(String, String)> = conn
                .query_row(
                    "SELECT title, plan_date FROM service_plans WHERE id = ?1",
                    [row_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((title, date)) = head else {
                return Ok(None);
            };
            feed(&title);
            feed(&date);
            let mut stmt = conn.prepare(
                "SELECT cue_type, label, payload_json, section_title, duration_sec
                   FROM plan_items WHERE plan_id = ?1 ORDER BY position, id",
            )?;
            let rows = stmt.query_map([row_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?;
            for row in rows {
                let (ty, label, payload, section, secs) = row?;
                feed(&ty);
                feed(&label);
                feed(&payload);
                feed(&section);
                feed(&secs.to_string());
            }
        }
        SONGS => {
            let head: Option<(String, String, String, String, Option<i64>)> = conn
                .query_row(
                    "SELECT title, author, ccli, song_key, bpm FROM songs WHERE id = ?1",
                    [row_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()?;
            let Some((title, author, ccli, key, bpm)) = head else {
                return Ok(None);
            };
            feed(&title);
            feed(&author);
            feed(&ccli);
            feed(&key);
            feed(&bpm.map(|b| b.to_string()).unwrap_or_default());
            let mut stmt = conn.prepare(
                "SELECT tag, label, lyrics FROM song_sections WHERE song_id = ?1 ORDER BY position, id",
            )?;
            let rows = stmt.query_map([row_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?;
            for row in rows {
                let (tag, label, lyrics) = row?;
                feed(&tag);
                feed(&label);
                feed(&lyrics);
            }
            // An arrangement built against a demo song is work somebody did. It
            // rides on the fingerprint so adding one counts as an edit and the
            // song (and the arrangement that cascades with it) survives removal.
            let mut stmt = conn.prepare(
                "SELECT name, sequence FROM song_arrangements WHERE song_id = ?1 ORDER BY id",
            )?;
            let rows = stmt.query_map([row_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (name, seq) = row?;
                feed(&name);
                feed(&seq);
            }
        }
        ANNOUNCEMENTS => {
            let head: Option<(String, String)> = conn
                .query_row(
                    "SELECT title, body FROM announcements WHERE id = ?1",
                    [row_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((title, body)) = head else {
                return Ok(None);
            };
            feed(&title);
            feed(&body);
        }
        SAVED_SCRIPTURE => {
            let head: Option<(String, String, String)> = conn
                .query_row(
                    "SELECT reference, text, translation FROM saved_scripture WHERE id = ?1",
                    [row_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let Some((reference, text, translation)) = head else {
                return Ok(None);
            };
            feed(&reference);
            feed(&text);
            feed(&translation);
        }
        MEDIA => {
            let head: Option<(String, String)> = conn
                .query_row(
                    "SELECT kind, filename FROM media_assets WHERE id = ?1",
                    [row_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((kind, filename)) = head else {
                return Ok(None);
            };
            feed(&kind);
            feed(&filename);
        }
        _ => return Ok(None),
    }
    Ok(Some(format!("{:x}", h.finalize())))
}

// ── The dataset ──────────────────────────────────────────────────────────────

/// A demo song: title, author, and its lyrics in the same `[Verse 1]` form the
/// ProPresenter importer produces, parsed by the app's own parser so a demo song
/// has exactly the shape an imported one does.
struct DemoSong {
    title: &'static str,
    author: &'static str,
    key: &'static str,
    lyrics: &'static str,
}

/// Three hymns, all long out of copyright, all with real verse/chorus structure so
/// the section keys, the reflow editor and arrangements have something to operate
/// on. Nothing modern: a worship set a church does not hold a licence for is not a
/// demo, it is a liability.
const DEMO_SONGS: &[DemoSong] = &[
    DemoSong {
        title: "Demo · Amazing Grace",
        author: "John Newton (1779)",
        key: "G",
        lyrics: "[Verse 1]\nAmazing grace, how sweet the sound\nThat saved a wretch like me\nI once was lost, but now am found\nWas blind, but now I see\n\n[Verse 2]\n'Twas grace that taught my heart to fear\nAnd grace my fears relieved\nHow precious did that grace appear\nThe hour I first believed\n\n[Verse 3]\nThrough many dangers, toils and snares\nI have already come\n'Tis grace hath brought me safe thus far\nAnd grace will lead me home\n\n[Verse 4]\nWhen we've been there ten thousand years\nBright shining as the sun\nWe've no less days to sing God's praise\nThan when we'd first begun\n",
    },
    DemoSong {
        title: "Demo · Great Is Thy Faithfulness",
        author: "Thomas O. Chisholm (1923)",
        key: "D",
        lyrics: "[Verse 1]\nGreat is thy faithfulness, O God my Father\nThere is no shadow of turning with thee\nThou changest not, thy compassions they fail not\nAs thou hast been thou forever wilt be\n\n[Chorus]\nGreat is thy faithfulness\nGreat is thy faithfulness\nMorning by morning new mercies I see\nAll I have needed thy hand hath provided\nGreat is thy faithfulness, Lord, unto me\n\n[Verse 2]\nSummer and winter, and springtime and harvest\nSun, moon and stars in their courses above\nJoin with all nature in manifold witness\nTo thy great faithfulness, mercy and love\n\n[Verse 3]\nPardon for sin and a peace that endureth\nThine own dear presence to cheer and to guide\nStrength for today and bright hope for tomorrow\nBlessings all mine, with ten thousand beside\n",
    },
    DemoSong {
        title: "Demo · Be Thou My Vision",
        author: "Irish, 8th c.; tr. Mary Byrne (1905)",
        key: "Eb",
        lyrics: "[Verse 1]\nBe thou my vision, O Lord of my heart\nNaught be all else to me, save that thou art\nThou my best thought, by day or by night\nWaking or sleeping, thy presence my light\n\n[Verse 2]\nBe thou my wisdom, and thou my true word\nI ever with thee and thou with me, Lord\nThou my great Father, I thy true son\nThou in me dwelling, and I with thee one\n\n[Verse 3]\nRiches I heed not, nor man's empty praise\nThou mine inheritance, now and always\nThou and thou only, first in my heart\nHigh King of heaven, my treasure thou art\n\n[Verse 4]\nHigh King of heaven, my victory won\nMay I reach heaven's joys, O bright heaven's sun\nHeart of my own heart, whatever befall\nStill be my vision, O Ruler of all\n",
    },
];

/// The notices. `title` is the operator's, `body` is what the room reads — and the
/// body says so, because an announcement really does reach a congregation screen
/// and a demo notice that reads like a real one is the honesty failure this whole
/// feature has to avoid.
const DEMO_ANNOUNCEMENTS: &[(&str, &str)] = &[
    (
        "Demo · Welcome & notices",
        "Welcome — we are glad you are here.\nThis is sample text from Relay's demo content.",
    ),
    (
        "Demo · Offering & response",
        "Our response, in giving and in prayer.\nThis is sample text from Relay's demo content.",
    ),
    (
        "Demo · Parents & children",
        "Children are welcome to stay, or to join us next door.\nThis is sample text from Relay's demo content.",
    ),
];

/// The verses the demo plan cites, plus two an operator would plausibly keep to
/// hand. Their TEXT is read out of the bundled KJV rather than written here, so a
/// demo verse is the same verse the detector would fire.
const DEMO_SCRIPTURE: &[(&str, i64, i64)] = &[
    ("Psalms", 23, 1),
    ("Romans", 8, 28),
    ("John", 3, 16),
    ("Isaiah", 40, 31),
    ("Philippians", 4, 6),
];

/// The one file the demo writes to disk: an abstract backdrop, drawn rather than
/// bundled so nothing binary enters the repository.
///
/// SVG on purpose — `channels::media_csp_header` serves it under a CSP that gives
/// it nothing to run and nowhere to send, which is the one imported type that
/// needs one. Shapes and a gradient only; no script, no external reference.
const DEMO_BACKDROP_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080" width="1920" height="1080">
  <defs>
    <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#0b1020"/>
      <stop offset="0.55" stop-color="#131c33"/>
      <stop offset="1" stop-color="#1d2742"/>
    </linearGradient>
    <radialGradient id="h" cx="0.72" cy="0.22" r="0.6">
      <stop offset="0" stop-color="#4a6fa5" stop-opacity="0.35"/>
      <stop offset="1" stop-color="#4a6fa5" stop-opacity="0"/>
    </radialGradient>
  </defs>
  <rect width="1920" height="1080" fill="url(#g)"/>
  <rect width="1920" height="1080" fill="url(#h)"/>
  <g fill="none" stroke="#8fa6c8" stroke-opacity="0.10" stroke-width="2">
    <circle cx="1450" cy="300" r="220"/>
    <circle cx="1450" cy="300" r="340"/>
    <circle cx="1450" cy="300" r="470"/>
  </g>
  <rect x="0" y="1040" width="1920" height="40" fill="#0b1020" fill-opacity="0.5"/>
</svg>
"##;

/// The backdrop's filename, which is also the label its plan cue carries.
pub const DEMO_BACKDROP_FILE: &str = "Demo · sermon backdrop.svg";

// ── Loading ──────────────────────────────────────────────────────────────────

/// Write the demo dataset and ledger every row.
///
/// `media_dir` is a parameter rather than `db::media_dir()` so a test can drive the
/// real code path into a temporary directory instead of the machine's app-data
/// folder. The directory is created if absent; a write that fails leaves the media
/// row out rather than leaving a Library entry that plays nothing.
///
/// **Not wrapped in one transaction, deliberately.** `import_song` and the plan
/// writers each open their own, and rusqlite cannot nest them.
///
/// **The ledger is written once, at the end, and that is not a convenience.** A
/// plan's fingerprint covers its cues and a song's covers its sections and
/// arrangements, so a row recorded the moment it was inserted would be recorded
/// half-built and every subsequent load would read as an edit of itself. What a
/// partial load costs is therefore honest: rows written before the failure are not
/// ledgered, so [`remove`] will not take them, and the panel says nothing is
/// loaded. That is the safe direction — a demo row left behind is visible and
/// deletable from the Library; a ledger entry for a row that is half of something
/// is not.
///
/// Rows an operator already has are left alone and NOT ledgered — a song with the
/// same title, a verse already saved (`saved_scripture.reference` is UNIQUE and
/// `save_scripture` upserts, so seeding one blind would overwrite their row and
/// then delete it on removal). Those are skipped and the count reflects it.
pub fn load(conn: &Connection, date: &str, media_dir: &Path) -> rusqlite::Result<DemoStatus> {
    let mut seeded: Vec<(&str, i64)> = Vec::new();
    let plan_id = super::plans::create_plan(conn, PLAN_TITLE, date)?;
    seeded.push((PLANS, plan_id));

    // Songs.
    let mut song_ids: Vec<(i64, &DemoSong)> = Vec::new();
    for s in DEMO_SONGS {
        if super::songs::song_id_by_title(conn, s.title)?.is_some() {
            continue;
        }
        let sections = crate::songs::parse_song(s.lyrics);
        let id =
            super::songs::import_song(conn, s.title, s.author, "", s.key, None, date, &sections)?;
        seeded.push((SONGS, id));
        song_ids.push((id, s));
    }

    // One arrangement, so the arrangement surface and the staleness rule have
    // something real to operate on. Built against the song as it was just written,
    // so it is not stale (DECISIONS §55).
    if let Some((id, s)) = song_ids.first() {
        let sections = crate::songs::parse_song(s.lyrics);
        let short: Vec<i64> = (0..sections.len() as i64).step_by(2).collect();
        super::songs::save_arrangement(conn, *id, None, "Demo · Short (v1, v3)", &short)?;
    }

    // Announcements.
    let mut announce_ids: Vec<i64> = Vec::new();
    for (title, body) in DEMO_ANNOUNCEMENTS {
        let id = super::library::save_announcement(conn, None, title, body, date)?;
        seeded.push((ANNOUNCEMENTS, id));
        announce_ids.push(id);
    }

    // Saved scripture, read out of the bundled corpus.
    for (book, chapter, verse) in DEMO_SCRIPTURE {
        let already: Option<i64> = conn
            .query_row(
                "SELECT id FROM saved_scripture WHERE book = ?1 AND chapter = ?2 AND verse = ?3",
                (book, chapter, verse),
                |r| r.get(0),
            )
            .optional()?;
        if already.is_some() {
            continue;
        }
        let Some(v) = super::verses::lookup_verse(conn, book, *chapter, *verse)? else {
            continue;
        };
        let id = super::library::save_scripture(conn, &v, date)?;
        seeded.push((SAVED_SCRIPTURE, id));
    }

    // The backdrop.
    let media_id = write_backdrop(conn, date, media_dir)?;
    if let Some(id) = media_id {
        seeded.push((MEDIA, id));
    }

    build_plan(conn, plan_id, &song_ids, &announce_ids, media_id)?;

    // THE LEDGER. Every row, fingerprinted as it FINALLY is.
    for (table, row_id) in seeded {
        let fp = fingerprint(conn, table, row_id)?.unwrap_or_default();
        conn.execute(
            "INSERT OR REPLACE INTO demo_content (table_name, row_id, fingerprint, loaded_at)
             VALUES (?1, ?2, ?3, ?4)",
            (table, row_id, fp, date),
        )?;
    }

    status(conn)
}

/// Write the backdrop file and its row, or `None` if the file could not be written.
fn write_backdrop(
    conn: &Connection,
    date: &str,
    media_dir: &Path,
) -> rusqlite::Result<Option<i64>> {
    if std::fs::create_dir_all(media_dir).is_err() {
        return Ok(None);
    }
    let id = super::library::insert_media(conn, "image", DEMO_BACKDROP_FILE, date)?;
    // Same `{id}_{safe}` convention `main::write_media_file` uses, because
    // `channels::serve_media_from_dir` finds a file by that prefix and nothing else.
    let safe: String = DEMO_BACKDROP_FILE
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = media_dir.join(format!("{id}_{safe}"));
    if std::fs::write(&path, DEMO_BACKDROP_SVG).is_err() {
        // Same repair as `main::write_media_file`: a row whose file is missing is a
        // Library entry that lists and shows nothing.
        super::library::delete_media(conn, id)?;
        return Ok(None);
    }
    super::library::set_media_path(conn, id, &path.to_string_lossy())?;
    Ok(Some(id))
}

/// The running order: four sections, every cue type, and a passage range in the
/// middle of it so `nav` next/back has somewhere to walk.
fn build_plan(
    conn: &Connection,
    plan_id: i64,
    songs: &[(i64, &DemoSong)],
    announcements: &[i64],
    media_id: Option<i64>,
) -> rusqlite::Result<()> {
    let push = |cue_type: &str,
                label: &str,
                payload: serde_json::Value,
                section: &str,
                seconds: i64|
     -> rusqlite::Result<()> {
        let id = super::plans::add_plan_item(
            conn,
            plan_id,
            cue_type,
            label,
            &payload.to_string(),
            None,
        )?;
        // Every cue records the section it is IN — the convention `plan.js`'s
        // `sectionsOf` folds into four headings rather than nine.
        super::plans::set_plan_section(conn, id, section)?;
        if seconds > 0 {
            super::plans::set_plan_duration(conn, id, seconds)?;
        }
        Ok(())
    };

    push(
        "countdown",
        "Demo · Countdown · 5 min",
        json!({ "minutes": 5, "label": "Service begins in", "done": "Welcome" }),
        "Gathering",
        300,
    )?;
    if let Some(id) = announcements.first() {
        let (title, body) = DEMO_ANNOUNCEMENTS[0];
        push(
            "announce",
            title,
            json!({ "announce_id": id, "title": title, "body": body }),
            "Gathering",
            180,
        )?;
    }
    if let Some((id, s)) = songs.first() {
        push("song", s.title, song_payload(*id, s), "Gathering", 300)?;
    }

    // The WORD. Two scripture cues: one passage range for the transport to walk,
    // one single verse. Both untimed — scripture fires when the preacher reaches
    // it, not on a clock, and the Planner renders a 0 as an em dash.
    push(
        "scripture",
        "Psalms 23:1-6",
        scripture_payload(conn, "Psalms", 23, 1, Some("Psalms 23:1-6"))?,
        "The Word",
        0,
    )?;
    push(
        "scripture",
        "Romans 8:28",
        scripture_payload(conn, "Romans", 8, 28, None)?,
        "The Word",
        0,
    )?;
    if let Some(id) = media_id {
        push(
            "media",
            DEMO_BACKDROP_FILE,
            json!({ "media_id": id, "kind": "image", "filename": DEMO_BACKDROP_FILE }),
            "The Word",
            0,
        )?;
    }

    if let Some((id, s)) = songs.get(1) {
        push("song", s.title, song_payload(*id, s), "Response", 300)?;
    }
    if let Some(id) = announcements.get(1) {
        let (title, body) = DEMO_ANNOUNCEMENTS[1];
        push(
            "announce",
            title,
            json!({ "announce_id": id, "title": title, "body": body }),
            "Response",
            120,
        )?;
    }
    if let Some((id, s)) = songs.get(2) {
        push("song", s.title, song_payload(*id, s), "Sending", 240)?;
    }
    Ok(())
}

/// The song cue payload — the same shape `src/lib/cues.js::songCue` builds for
/// "Standard" (no arrangement): every section once, in order, with a null
/// `arrangement_shape` so `sync_song_in_plans` treats it exactly as it treats a
/// cue an operator added from the Library.
fn song_payload(song_id: i64, s: &DemoSong) -> serde_json::Value {
    let sections = crate::songs::parse_song(s.lyrics);
    let slides: Vec<serde_json::Value> = sections
        .iter()
        .map(|x| json!({ "tag": x.tag, "label": x.label, "lyrics": x.lyrics }))
        .collect();
    // `arrangement_shape` stays NULL, which is what `cues.js::songCue` writes for a
    // Standard cue (`arr ? built_shape : null`). It is not an oversight: Standard is
    // the ABSENCE of an arrangement, so there are no stored indices for a section
    // reorder to invalidate, and a shape recorded here would make
    // `sync_song_in_plans` compare a claim nobody made. DECISIONS §55.
    json!({
        "song_id": song_id,
        "title": s.title,
        "author": s.author,
        "song_key": s.key,
        "sections": slides,
        "arrangement_name": "Standard",
        "arrangement_seq": serde_json::Value::Null,
        "arrangement_shape": serde_json::Value::Null,
        "arrangement_stale": false,
    })
}

/// The scripture cue payload. `reference` may state a RANGE while `book/chapter/
/// verse` name the anchor — that is exactly what `manual_fire` reads, so the range
/// stages a passage and the transport can walk it.
fn scripture_payload(
    conn: &Connection,
    book: &str,
    chapter: i64,
    verse: i64,
    reference: Option<&str>,
) -> rusqlite::Result<serde_json::Value> {
    let v = super::verses::lookup_verse(conn, book, chapter, verse)?;
    let (text, translation, anchor_ref) = match &v {
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
        "reference": reference.unwrap_or(&anchor_ref),
        "text": text,
        "translation": translation,
    }))
}

// ── Removal ──────────────────────────────────────────────────────────────────

/// Take the demo dataset back out.
///
/// Every ledgered row is fingerprinted again. Unchanged → deleted. Changed → kept
/// and released from the ledger, because it holds work somebody did. Gone already →
/// its ledger entry is tidied and counted as neither.
///
/// **Not one big transaction, and it must not become one.** `delete_plan`,
/// `delete_song` and `delete_media` each open their own `unchecked_transaction`,
/// and rusqlite cannot nest one inside another — an outer `BEGIN` here fails with
/// *"cannot start a transaction within a transaction"* on the very first delete,
/// which is how this was found. What makes it safe instead is the ORDER: each row
/// is deleted and only then released from the ledger, so a failure part-way leaves
/// the rows it has not reached still ledgered and still removable. Pressing Remove
/// again finishes the job; it never half-forgets a row it did not delete.
///
/// Returns the media files the caller should delete; this module does not remove
/// files, for the same reason `delete_media` does not.
pub fn remove(conn: &Connection) -> rusqlite::Result<DemoRemoval> {
    let mut out = DemoRemoval::default();
    for table in TABLES {
        for (row_id, seeded) in ledgered(conn, table)? {
            match fingerprint(conn, table, row_id)? {
                None => {}
                Some(now) if now != seeded => {
                    out.kept += 1;
                }
                Some(_) => {
                    delete_row(conn, table, row_id, &mut out)?;
                    out.removed += 1;
                }
            }
            conn.execute(
                "DELETE FROM demo_content WHERE table_name = ?1 AND row_id = ?2",
                (table, row_id),
            )?;
        }
    }
    Ok(out)
}

fn delete_row(
    conn: &Connection,
    table: &str,
    row_id: i64,
    out: &mut DemoRemoval,
) -> rusqlite::Result<()> {
    match table {
        PLANS => super::plans::delete_plan(conn, row_id),
        SONGS => super::songs::delete_song(conn, row_id),
        ANNOUNCEMENTS => super::library::delete_announcement(conn, row_id),
        SAVED_SCRIPTURE => super::library::delete_saved_scripture(conn, row_id),
        MEDIA => {
            if let Some(p) = super::library::delete_media(conn, row_id)? {
                out.files.push(p);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::songs::ParsedSection;

    fn scratch_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "relay-demo-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn fresh() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::init_fresh(&conn).unwrap();
        conn
    }

    /// THE RULE THIS WHOLE MODULE IS BUILT AROUND.
    ///
    /// A fresh install has the ledger and nothing in it, and nothing anywhere in
    /// it carries the mark this module puts on every row it writes. If this
    /// fails, something taught Relay to load the demo dataset for itself.
    ///
    /// **It used to assert that five tables were EMPTY, and that stopped being
    /// the right question on 2026-09-16.** DECISIONS §90 gives a fresh install a
    /// starter set — announcements, the pictures Relay ships, one example plan —
    /// so emptiness would now fail here while saying nothing at all about this
    /// module. The claim being made was never "the database is empty"; it was
    /// "`db::demo::load` did not run", and the ledger plus the mark say exactly
    /// that over a database with content in it. `db::starter`'s own tests hold
    /// the other half: what a first launch DOES contain.
    #[test]
    fn a_fresh_install_carries_no_demo_content() {
        let conn = fresh();
        assert!(!is_loaded(&conn).unwrap());
        let st = status(&conn).unwrap();
        assert_eq!(st.total, 0);
        assert!(st.groups.is_empty());
        // …and nothing in the tables it writes into is one of its rows.
        for (what, sql) in [
            ("plan", "SELECT title FROM service_plans"),
            ("song", "SELECT title FROM songs"),
            ("announcement", "SELECT title FROM announcements"),
            ("media asset", "SELECT filename FROM media_assets"),
            ("cue", "SELECT label FROM plan_items"),
        ] {
            let mut s = conn.prepare(sql).unwrap();
            let rows: Vec<String> = s
                .query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            for row in rows {
                assert!(
                    !row.starts_with(MARK),
                    "a fresh install has a demo {what}: {row}"
                );
            }
        }
    }

    /// The tables the demo dataset touches, counted, so a load and a removal can
    /// be measured as a DELTA rather than against zero.
    ///
    /// Against zero was right while a fresh install was empty. Since DECISIONS
    /// §90 it is not: a starter set sits under everything this module writes, and
    /// a removal that returned those tables to zero would be deleting the
    /// church's content. The delta is also the stronger claim — "the demo dataset
    /// put exactly this much in and took exactly that much out" is what the
    /// removal path actually promises.
    fn tally(conn: &Connection) -> Vec<(&'static str, i64)> {
        [
            "service_plans",
            "plan_items",
            "songs",
            "song_sections",
            "song_arrangements",
            "announcements",
            "saved_scripture",
            "media_assets",
        ]
        .into_iter()
        .map(|t| {
            let n: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
                .unwrap();
            (t, n)
        })
        .collect()
    }

    /// The demo plan's own cues, by the plan it wrote.
    fn demo_plan_id(conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT id FROM service_plans WHERE title = ?1",
            [PLAN_TITLE],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn loading_fills_every_workspace_and_removing_empties_them_again() {
        let dir = scratch_dir("roundtrip");
        let conn = fresh();
        let before = tally(&conn);
        let st = load(&conn, "2026-09-14", &dir).unwrap();
        assert!(st.loaded);
        assert_eq!(st.edited, 0, "nothing is edited the moment it is loaded");
        // one plan + three songs + three notices + five verses + one backdrop
        assert_eq!(st.total, 13, "{:?}", st.groups);

        let plan = demo_plan_id(&conn);
        let cues: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM plan_items WHERE plan_id = ?1",
                [plan],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cues, 9);
        let kinds: Vec<String> = {
            let mut s = conn
                .prepare(
                    "SELECT DISTINCT cue_type FROM plan_items WHERE plan_id = ?1
                      ORDER BY cue_type",
                )
                .unwrap();
            let r = s
                .query_map([plan], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            r
        };
        assert_eq!(
            kinds,
            vec!["announce", "countdown", "media", "scripture", "song"],
            "every cue type a workspace renders has something to show"
        );
        let sections: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT section_title) FROM plan_items
                  WHERE plan_id = ?1 AND section_title <> ''",
                [plan],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sections, 4, "Gathering · The Word · Response · Sending");
        let arrangements: i64 = conn
            .query_row("SELECT COUNT(*) FROM song_arrangements", [], |r| r.get(0))
            .unwrap();
        assert_eq!(arrangements, 1);

        let gone = remove(&conn).unwrap();
        assert_eq!(gone.removed, 13);
        assert_eq!(gone.kept, 0);
        assert!(!is_loaded(&conn).unwrap());
        assert_eq!(
            tally(&conn),
            before,
            "a removal must put every table back exactly where it found it"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// NOTHING AN OPERATOR MADE.
    #[test]
    fn removal_leaves_the_operators_own_content_exactly_where_it_was() {
        let dir = scratch_dir("theirs");
        let conn = fresh();
        // Their content, made BEFORE the demo is loaded and again after.
        let theirs = db::create_plan(&conn, "Easter Sunday", "2026-04-05").unwrap();
        let their_song = db::import_song(
            &conn,
            "Our Own Song",
            "",
            "",
            "",
            None,
            "2026-04-01",
            &[ParsedSection {
                tag: "V1".into(),
                label: "Verse 1".into(),
                lyrics: "ours".into(),
            }],
        )
        .unwrap();
        load(&conn, "2026-09-14", &dir).unwrap();
        let their_notice =
            db::save_announcement(&conn, None, "Ours", "our words", "2026-09-14").unwrap();

        remove(&conn).unwrap();

        assert!(db::plan_items(&conn, theirs).is_ok());
        let plans: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM service_plans WHERE id = ?1",
                [theirs],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(plans, 1, "their plan survived");
        assert!(db::get_song(&conn, their_song).unwrap().is_some());
        let notices: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM announcements WHERE id = ?1",
                [their_notice],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(notices, 1, "their notice survived");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A DEMO ITEM THE OPERATOR HAS EDITED IS THEIRS NOW.
    ///
    /// It is kept, released from the ledger, and counted in `kept` so nothing about
    /// it is silent. Watched to fail by making `fingerprint` ignore `song_sections`:
    /// the edited song was then deleted and `kept` read 0.
    #[test]
    fn an_edited_demo_item_is_kept_and_reported() {
        let dir = scratch_dir("edited");
        let conn = fresh();
        load(&conn, "2026-09-14", &dir).unwrap();

        // They retype a lyric in a demo song, and rename a demo notice.
        let song_id: i64 = conn
            .query_row(
                "SELECT row_id FROM demo_content WHERE table_name = 'songs' ORDER BY id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "UPDATE song_sections SET lyrics = 'our own words' WHERE song_id = ?1 AND position = 0",
            [song_id],
        )
        .unwrap();
        let notice_id: i64 = conn
            .query_row(
                "SELECT row_id FROM demo_content WHERE table_name = 'announcements' ORDER BY id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "UPDATE announcements SET body = 'our own notice' WHERE id = ?1",
            [notice_id],
        )
        .unwrap();

        let before = status(&conn).unwrap();
        assert_eq!(
            before.edited, 2,
            "the panel says so before the operator presses"
        );

        let gone = remove(&conn).unwrap();
        assert_eq!(gone.kept, 2);
        assert_eq!(gone.removed, 11);
        // Both are still there, and neither is in the ledger any more.
        assert!(db::get_song(&conn, song_id).unwrap().is_some());
        let notices: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM announcements WHERE id = ?1",
                [notice_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(notices, 1);
        assert!(!is_loaded(&conn).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A RENAME IS NOT HOW THE LEDGER FINDS ITS ROWS — but it IS an edit.
    #[test]
    fn a_renamed_demo_plan_is_still_found_and_is_treated_as_edited() {
        let dir = scratch_dir("renamed");
        let conn = fresh();
        load(&conn, "2026-09-14", &dir).unwrap();
        conn.execute(
            "UPDATE service_plans SET title = 'Our Sunday' WHERE title = ?1",
            [PLAN_TITLE],
        )
        .unwrap();
        let st = status(&conn).unwrap();
        assert_eq!(st.edited, 1, "the ledger still sees it, by id");
        let gone = remove(&conn).unwrap();
        assert_eq!(gone.kept, 1);
        let survived: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM service_plans WHERE title = 'Our Sunday'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(survived, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THE PASSAGE CUE MUST ACTUALLY BE A PASSAGE.
    ///
    /// The whole reason one scripture cue states a range is so `nav` next/back has
    /// somewhere to walk. That is only true if `detect_direct` — the parser
    /// `manual_fire` runs on a cue's reference — reads an end verse out of it.
    #[test]
    fn the_passage_cue_parses_into_a_range_the_transport_can_walk() {
        let dir = scratch_dir("passage");
        let conn = fresh();
        load(&conn, "2026-09-14", &dir).unwrap();
        let refs: Vec<String> = {
            let mut s = conn
                .prepare(
                    "SELECT json_extract(payload_json, '$.reference') FROM plan_items
                      WHERE cue_type = 'scripture' AND plan_id = ?1 ORDER BY position",
                )
                .unwrap();
            s.query_map([demo_plan_id(&conn)], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        assert_eq!(refs, vec!["Psalms 23:1-6", "Romans 8:28"]);
        let m = crate::detection::detect_direct(&refs[0])
            .into_iter()
            .next()
            .expect("the passage cue's reference parses");
        assert_eq!(m.reference.book, "Psalms");
        assert_eq!(m.reference.chapter, 23);
        assert_eq!(m.reference.verse, 1);
        assert_eq!(
            m.verse_end,
            Some(6),
            "without an end verse the transport has nothing to walk"
        );
        // …and the single-verse cue is deliberately NOT a range.
        let single = crate::detection::detect_direct(&refs[1])
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(single.verse_end, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// DEMO CONTENT IS NOT A SERVICE THAT HAPPENED.
    #[test]
    fn demo_content_never_fabricates_a_service_record() {
        let dir = scratch_dir("history");
        let conn = fresh();
        load(&conn, "2026-09-14", &dir).unwrap();
        for sql in [
            "SELECT COUNT(*) FROM services",
            "SELECT COUNT(*) FROM transcripts",
            "SELECT COUNT(*) FROM detections",
            "SELECT COUNT(*) FROM cues",
            "SELECT COUNT(*) FROM service_events",
            "SELECT COUNT(*) FROM perf_samples",
        ] {
            let n: i64 = conn.query_row(sql, [], |r| r.get(0)).unwrap();
            assert_eq!(
                n, 0,
                "{sql} — demo content may never look like a service record"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// EVERY SEEDED ROW AN OPERATOR READS A TITLE FOR SAYS SO.
    #[test]
    fn every_titled_demo_row_is_visibly_demo_content() {
        let dir = scratch_dir("marked");
        let conn = fresh();
        load(&conn, "2026-09-14", &dir).unwrap();
        // SCOPED TO THE LEDGER, which is what the module's own note says the
        // marker is. Before DECISIONS §90 the tables held nothing but demo rows,
        // so "every row" and "every demo row" were the same set; they are not any
        // more, and a starter announcement reading `Demo · ` would be a lie in the
        // opposite direction. Cues are not ledgered individually, so they are
        // scoped to the demo plan, which is.
        let plan = demo_plan_id(&conn);
        for (sql, what) in [
            (
                "SELECT title FROM service_plans WHERE id IN
                   (SELECT row_id FROM demo_content WHERE table_name = 'service_plans')",
                "plan",
            ),
            (
                "SELECT title FROM songs WHERE id IN
                   (SELECT row_id FROM demo_content WHERE table_name = 'songs')",
                "song",
            ),
            (
                "SELECT title FROM announcements WHERE id IN
                   (SELECT row_id FROM demo_content WHERE table_name = 'announcements')",
                "announcement",
            ),
            (
                "SELECT filename FROM media_assets WHERE id IN
                   (SELECT row_id FROM demo_content WHERE table_name = 'media_assets')",
                "media asset",
            ),
            ("SELECT label FROM plan_items WHERE plan_id = ?1", "cue"),
        ] {
            let mut s = conn.prepare(sql).unwrap();
            let rows: Vec<String> = s
                .query_map(
                    rusqlite::params_from_iter(if sql.contains("?1") {
                        vec![plan]
                    } else {
                        vec![]
                    }),
                    |r| r.get::<_, String>(0),
                )
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert!(!rows.is_empty(), "{sql} produced nothing");
            for row in rows {
                // A scripture cue is the documented exception: its label is a Bible
                // reference and must stay a true one.
                if what == "cue" && (row.starts_with("Psalms") || row.starts_with("Romans")) {
                    continue;
                }
                assert!(
                    row.starts_with(MARK),
                    "{what} \"{row}\" does not say it is demo content"
                );
            }
        }
        // And the notices say so in the words the ROOM reads, not only in the
        // operator's title.
        let mut s = conn
            .prepare(
                "SELECT body FROM announcements WHERE id IN
                   (SELECT row_id FROM demo_content WHERE table_name = 'announcements')",
            )
            .unwrap();
        for body in s
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
        {
            assert!(
                body.contains("demo content"),
                "a notice reaches a congregation screen; it must not read like a real one: {body}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// It does not overwrite a verse the operator had already saved.
    #[test]
    fn a_verse_the_operator_already_saved_is_left_alone_and_never_ledgered() {
        let dir = scratch_dir("dedupe");
        let conn = fresh();
        let v = db::lookup_verse(&conn, "Romans", 8, 28).unwrap().unwrap();
        let theirs = db::save_scripture(&conn, &v, "2026-01-01").unwrap();
        load(&conn, "2026-09-14", &dir).unwrap();
        let ledgered: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM demo_content WHERE table_name = 'saved_scripture' AND row_id = ?1",
                [theirs],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ledgered, 0, "their row must never be claimed as ours");
        remove(&conn).unwrap();
        let still: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM saved_scripture WHERE id = ?1",
                [theirs],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(still, 1, "and it must survive the removal");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_ledger_migration_is_retryable() {
        let conn = Connection::open_in_memory().unwrap();
        // Twice in a row, and again after the table has rows in it.
        ensure_demo_ledger(&conn).unwrap();
        ensure_demo_ledger(&conn).unwrap();
        conn.execute(
            "INSERT INTO demo_content (table_name, row_id, fingerprint) VALUES ('songs', 1, 'x')",
            [],
        )
        .unwrap();
        ensure_demo_ledger(&conn).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM demo_content", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "a re-run must not disturb what is there");
    }

    /// The backdrop is written where `channels::serve_media_from_dir` looks.
    #[test]
    fn the_backdrop_lands_under_the_id_prefix_the_media_server_searches_for() {
        let dir = scratch_dir("backdrop");
        let conn = fresh();
        load(&conn, "2026-09-14", &dir).unwrap();
        // THE DEMO'S OWN ASSET, by the ledger. `LIMIT 1` over the whole table was
        // unambiguous while the demo backdrop was the only media row there could
        // be; since DECISIONS §90 a fresh install ships a picture library and the
        // bare `LIMIT 1` picked one of those instead.
        let (id, path): (i64, String) = conn
            .query_row(
                "SELECT id, path FROM media_assets WHERE id IN
                   (SELECT row_id FROM demo_content WHERE table_name = 'media_assets')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let name = std::path::Path::new(&path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(name.starts_with(&format!("{id}_")), "{name}");
        assert!(name.ends_with(".svg"), "{name}");
        assert!(std::fs::read_to_string(&path).unwrap().contains("<svg"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
