//! Output templates: the look of what the congregation sees.
//!
//! One template = one styled render target config. `TemplateRender.svelte` is the
//! single renderer that consumes these, so a template looks identical in the
//! editor preview and on a 4K wall.

use super::settings::get_setting;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;

/// An output template: layout (regions + alignment) and style (fonts, colors,
/// sizes). `layout` and `style` are opaque JSON blobs interpreted by the shared
/// renderer (Output.svelte) — the DB doesn't care about their internals, which
/// keeps the template shape editable without a migration. See docs/SPEC.md §5.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub layout: Value,
    pub style: Value,
    /// Whether this template is one of the (max 4) styles previewed on the
    /// console Output grid. Users can keep many templates but activate only 4.
    #[serde(default)]
    pub active: bool,
}

/// All templates, ordered by id.
pub fn list_templates(conn: &Connection) -> rusqlite::Result<Vec<Template>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, region_config_json, style_json, console_active FROM templates ORDER BY id",
    )?;
    let rows = stmt.query_map([], row_to_template)?;
    rows.collect()
}

/// A single template by id.
pub fn get_template(conn: &Connection, id: i64) -> rusqlite::Result<Option<Template>> {
    conn.query_row(
        "SELECT id, name, region_config_json, style_json, console_active FROM templates WHERE id = ?1",
        [id],
        row_to_template,
    )
    .optional()
}

/// Delete a template. Any output channel pointing at it is unassigned first so
/// the foreign key stays valid.
pub fn delete_template(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE output_channels SET template_id = NULL WHERE template_id = ?1",
        [id],
    )?;
    conn.execute("DELETE FROM templates WHERE id = ?1", [id])?;
    Ok(())
}

/// Create the `console_active` column if missing and guarantee up to 4 defaults
/// are active. Idempotent — safe on every open, covers pre-existing DBs.
pub fn ensure_template_active(conn: &Connection) -> rusqlite::Result<()> {
    let has: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('templates') WHERE name = 'console_active'",
        [],
        |r| r.get(0),
    )?;
    if has == 0 {
        // Losing a migration race is not a failure — see the note on
        // `plans::add_plan_item_column`. Two processes can hold this file.
        match conn.execute_batch(
            "ALTER TABLE templates ADD COLUMN console_active INTEGER NOT NULL DEFAULT 0;",
        ) {
            Err(e)
                if e.to_string()
                    .to_lowercase()
                    .contains("duplicate column name") => {}
            other => other?,
        }
    }
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM templates WHERE console_active = 1",
        [],
        |r| r.get(0),
    )?;
    if n == 0 {
        conn.execute(
            "UPDATE templates SET console_active = 1
               WHERE id IN (SELECT id FROM templates ORDER BY id LIMIT 4)",
            [],
        )?;
    }
    Ok(())
}

/// Insert (id <= 0) or update (id > 0) a template. Returns its id.
pub fn upsert_template(conn: &Connection, t: &Template) -> rusqlite::Result<i64> {
    let layout = t.layout.to_string();
    let style = t.style.to_string();
    if t.id > 0 {
        conn.execute(
            "UPDATE templates SET name = ?1, region_config_json = ?2, style_json = ?3 WHERE id = ?4",
            (&t.name, &layout, &style, t.id),
        )?;
        Ok(t.id)
    } else {
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (&t.name, &layout, &style),
        )?;
        Ok(conn.last_insert_rowid())
    }
}

fn row_to_template(r: &rusqlite::Row) -> rusqlite::Result<Template> {
    let layout: String = r.get(2)?;
    let style: String = r.get(3)?;
    Ok(Template {
        id: r.get(0)?,
        name: r.get(1)?,
        layout: serde_json::from_str(&layout).unwrap_or(Value::Null),
        style: serde_json::from_str(&style).unwrap_or(Value::Null),
        active: r.get::<_, i64>(4).unwrap_or(0) != 0,
    })
}

/// The four built-in output templates (SPEC §5, cqw sizes). Match the frontend
/// defaults in src/lib/templates.js. Source of truth for both fresh seed and
/// the in-place migration.
fn builtin_templates() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        (
            "Classic Serif",
            r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":false,"refFirst":false}"##,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)","accent":"#e8a33d","verseColor":"#f4e4c8","verseSize":"5.5","refSize":"2.6","italicRef":true}"##,
        ),
        (
            "Stage Mono",
            r##"{"regions":["reference","verse_text"],"align":"left","lowerThird":false,"refFirst":true}"##,
            r##"{"font":"var(--f-display)","background":"#000000","accent":"#4fa8c9","verseColor":"#ffffff","verseSize":"6","refSize":"2.6","italicRef":false}"##,
        ),
        (
            "Lower Third",
            r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":true,"refFirst":false}"##,
            // Neutral band, light type — matching the whole `Lower Third · …`
            // family in `theme_templates()` below and, byte for byte, the same
            // template in `src/lib/templates.js`. Was `#b080e0` here and
            // `#8b5cf6` there (two purples for one template), and amethyst is
            // rule 18's rehearsal colour either way. The family replaced the
            // `Lower Third Night` preset this line used to point at.
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#101319","verseColor":"#f2f4f8","verseSize":"2.6","refSize":"1.7","italicRef":false}"##,
        ),
        (
            // WORSHIP LYRICS — the fifth built-in, and the one every previous
            // template was wrong for.
            //
            // Until now every built-in was SCRIPTURE-shaped: a `reference` region
            // and verse text sized around 5cqw, because a verse is a paragraph
            // with a citation. A lyric is neither. It is three or four short
            // lines that a room full of people has to read while singing, from
            // the back, often over a lit stage — and it has no reference at all.
            // Rendering lyrics through a scripture template put a large gold
            // "Song Title · Slide 7" where the words should be and shrank the
            // words to a caption.
            //
            //   · NO reference region. The congregation is not singing the title.
            //   · Large (9cqw) — roughly twice the scripture size. Short lines can
            //     afford it, and TemplateRender auto-shrinks anything that would
            //     overflow, so a long line is safe.
            //   · White on near-black: the highest contrast available, which is
            //     what a projector in a lit room actually needs.
            //   · Sans, not serif. Serifs are for reading a paragraph; a lyric is
            //     scanned in a second and a half between breaths.
            "Worship Lyrics",
            r##"{"regions":["verse_text"],"align":"center","lowerThird":false,"refFirst":false}"##,
            r##"{"font":"var(--f-body)","background":"#07070a","accent":"#ffffff","verseColor":"#ffffff","verseSize":"9","refSize":"2","italicRef":false}"##,
        ),
        (
            "Lobby Warm",
            r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":false}"##,
            r##"{"font":"var(--f-serif)","background":"linear-gradient(160deg, #241419, #120a0e)","accent":"#e27d93","verseColor":"#f0dfe3","verseSize":"4","refSize":"2","italicRef":false}"##,
        ),
    ]
}

/// Give an EXISTING database the lyrics template, and point songs at it.
///
/// Idempotent and additive: it APPENDS a row rather than rewriting ids, because
/// `output_channels` and `app_settings` hold template ids as foreign keys and
/// renumbering them would silently repoint a church's projector at a different
/// design (CLAUDE.md §25 — a migration must be safe to re-run).
///
/// It also sets the per-content-type mapping for `song` **only if the operator
/// has not chosen one**. Overwriting a deliberate choice would be worse than the
/// bug this fixes.
pub(super) fn ensure_lyrics_template(conn: &Connection) -> rusqlite::Result<()> {
    const NAME: &str = "Worship Lyrics";
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM templates WHERE name = ?1", [NAME], |r| {
            r.get(0)
        })
        .ok();
    let id = match existing {
        Some(id) => id,
        None => {
            let (_, layout, style) = builtin_templates()
                .iter()
                .find(|(n, _, _)| *n == NAME)
                .copied()
                .unwrap_or((NAME, "{}", "{}"));
            conn.execute(
                "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
                (NAME, layout, style),
            )?;
            conn.last_insert_rowid()
        }
    };
    // The song content-type default is read via the canonical `tpl_{kind}` key
    // (`tpl_song`, see settings.rs::content_template_id). An earlier version of
    // this seed wrote it under `content_template_song` instead, so the read side
    // never found it and the lyrics default silently never applied. Write the
    // canonical key, and migrate any value left under the legacy key first so an
    // operator's earlier choice is preserved.
    let chosen: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'tpl_song'",
            [],
            |r| r.get(0),
        )
        .ok();
    if chosen.is_none() {
        let legacy: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'content_template_song'",
                [],
                |r| r.get(0),
            )
            .ok();
        let value = legacy.unwrap_or_else(|| id.to_string());
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('tpl_song', ?1)",
            [value],
        )?;
    }
    // Drop the dead legacy key so it can't shadow or confuse later reads.
    conn.execute(
        "DELETE FROM app_settings WHERE key = 'content_template_song'",
        [],
    )?;
    Ok(())
}

/// THE STANDALONE PRESETS — RETIRED. This list is deliberately empty.
///
/// Fourteen ready-to-use designs used to ship here, each a complete template but
/// each a look on its own: pick `Midnight Blue` for scripture and you had nothing
/// coordinated to put a lyric, a notice or a countdown on. The five families in
/// `theme_templates()` answer all five content kinds in one palette, which is
/// what an operator was actually reaching for, and twenty-five coordinated rows
/// plus fourteen loose ones is a gallery nobody can read.
///
/// The fourteen, named so the retirement migration can be read against this list
/// rather than against a commit message:
///
///   `Midnight Blue` · `Royal Amethyst` · `Deep Teal` · `Crimson Grace` ·
///   `Emerald Word` · `Indigo Night` · `Slate Minimal` · `Pure Contrast` ·
///   `Lyric Bold` · `Lyric Glow` · `Lower Third Light` · `Lower Third Night` ·
///   `Stage Confidence` · `Lobby Sunrise`
///
/// Their exact bytes are frozen in `data/retired_presets.json`, because seeds
/// insert BY NAME and only when absent: emptying this list reaches a fresh
/// install and no existing one, so removing them from a church that already has
/// them is a migration, and that migration matches on bytes so it can tell a
/// leftover from a row somebody edited.
///
/// That file holds twenty-one rows, not fourteen: this wave also stopped
/// shipping six rows out of `theme_templates()` — the `Lower Third` member the
/// old four-kind Aurora and Ember families carried, and all four `Nocturne · …`
/// rows, Nocturne not being one of the five families — plus the shelf's
/// `Lower Third · Scripture`, whose name the keyed family takes. A row this
/// wave stops shipping and does not freeze is a row stranded in every existing
/// install with nothing left that names it.
///
/// The function stays rather than being deleted. `region_presets()` chains it,
/// `there_are_presets_across_every_screen_type` asserts it is empty, and a
/// standalone preset that is genuinely not a family member — a one-off a church
/// asks for by name — has somewhere to go that is not the middle of a family.
fn preset_templates() -> &'static [(&'static str, &'static str, &'static str)] {
    &[]
}

/// THE FIVE FAMILIES — a coordinated set for every kind of content a service
/// fires, so an operator picks a LOOK once instead of matching five templates by
/// hand. Named `Family · Kind` so the gallery groups them visually.
///
/// This is what replaced the theme layer. A theme had no field a template does
/// not already have (DECISIONS §87), so the thing carrying "a look somebody
/// picked" had to become the coordinated family, and a family is only a look you
/// can pick if it answers every kind. `CONTENT_KINDS` (src/lib/layers.js) has
/// five; the seed had four, and the two it never covered — Media and Timer —
/// were exactly the two an operator could not dress.
///
/// Five families, five kinds, twenty-five rows:
///
///   · **Classic** — the serif look Relay opens with, carried forward from
///     `builtin_templates()`'s `Classic Serif`.
///   · **Aurora** — teal / emerald. Its Scripture, Lyrics and Announcement are
///     the previously-seeded rows VERBATIM, so a church already using one sees
///     no change on update.
///   · **Ember** — amber / crimson, on the same terms.
///   · **Lower Third** — the keyed family, from `builtin_templates()`'s
///     `Lower Third`. It is now the only source of keyed templates in the seed,
///     which the transparency law in `resolveOutputTemplate` depends on.
///   · **High Visibility** — the answer to a lit room and a cheap lens, taken
///     from `data/legacy_themes.json`'s frozen `High Visibility` theme, which is
///     the same look `src/lib/legibility.test.js` already measures at 21:1.
///
/// COLOURS ARE FIXED PER FAMILY; SIZES ARE FREE PER KIND. That is what makes a
/// family a set rather than five templates that happen to share a prefix — a
/// lyric is scanned in a second and a verse is read, so they are not the same
/// size, but they are the same look. A member takes its family's accent, verse
/// colour and ground; where it varies, it varies inside the family's own hexes.
/// The two Lyrics rows carried forward from the old Aurora and Ember families are
/// the honest exception: both go white-on-a-darker-mix-of-their-own-hexes, which
/// is the lyrics convention the built-in `Worship Lyrics` also follows, and they
/// are reproduced byte for byte so a church already using one sees nothing move.
///
/// NO SEEDED TEMPLATE WEARS A LAW COLOUR ON ITS BAND (rule 18): the keyed family
/// fills with `#101319`, the neutral `ensure_lower_third_band_is_not_a_law_colour`
/// already enforces, because a seeded band shipped in amethyst once and amethyst
/// means rehearsal.
///
/// EVERY LAYOUT DECLARES ALL FIVE KINDS, AND THAT IS NOT A SHRUG. `layout.shows`
/// is a per-SCREEN filter, not a record of what a template was designed for:
/// `Output.svelte` gates incoming content on `templateShows(own_template, kind)`
/// BEFORE `resolveOutputTemplate` is consulted, so a narrow list on the template
/// an operator assigned to the Main screen makes that screen silently ignore
/// every other kind — including when a content look correctly routes songs to
/// that family's Lyrics member, because the message is dropped before the
/// override ever resolves. The thing that says "this is the scripture look" is
/// the `Used for` binding, which is a different register entirely.
///
/// Writing all five down still does the job the list exists for. `templateShows`
/// returns true for every kind when `shows` is ABSENT, so a silent template
/// claims all five implicitly and the editor's filter register has to
/// materialise a list on the operator's first click — which is what made that
/// click look as though it had wiped four. An explicit list is the same
/// behaviour with the register told the truth from the start.
///
/// The keyed family is on the same rule, deliberately. A keyed template with no
/// `shows` list already shows every kind today — the shelf's `Lower Third · Lyric`
/// carries none — so listing five is not a new exposure, and the transparency law
/// in `resolveOutputTemplate` still protects the override path.
fn theme_templates() -> &'static [(&'static str, &'static str, &'static str)] {
    // FULL-SCREEN SCRIPTURE — verse and reference, centred. The common case.
    const SCRIPTURE: &str = r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":false,"refFirst":false,"shows":["scripture","song","media","announce","countdown"]}"##;
    // LYRIC — the words alone. A room does not sing the title.
    const LYRIC: &str = r##"{"regions":["verse_text"],"align":"center","lowerThird":false,"refFirst":false,"shows":["scripture","song","media","announce","countdown"]}"##;
    // MEDIA — a fired picture or video fills the frame. There is no caption:
    // `TemplateRender`'s region branch takes the media path before any region is
    // rendered, and `fire_media` carries a url and nothing else (no reference, no
    // text), so a caption line would be blank at every fire. The `reference`
    // region is what this template falls back to showing when it is handed
    // something that is not a picture.
    const MEDIA: &str = r##"{"regions":["reference"],"align":"center","lowerThird":false,"refFirst":true,"shows":["scripture","song","media","announce","countdown"]}"##;
    // ANNOUNCEMENT — a title over a scrolling body line (the ticker).
    const ANNOUNCE: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":true,"shows":["scripture","song","media","announce","countdown"]}"##;
    // TIMER — the countdown, with its label above it. The renderer draws the
    // digits at `verseSize * 2`, so a Timer's `verseSize` is HALF the size the
    // clock ends up.
    //
    // THE FOUR FULL-SCREEN TIMER ROWS ARE THE ONLY FAMILY MEMBERS SEEDED IN THE
    // LAYER MODEL, and the reason is what a church sees on the first Sunday.
    // (The fifth, the keyed one, is a separate case — see `BAND_TIMER` below.)
    //
    // The REGION branch draws a countdown at a fixed `cqw` with no fit loop around
    // either of its two lines. Measured on a fresh install at 1920x1080, the
    // region-model `Classic · Timer` put its label out at 7.88px — still clipped at
    // both ends — and its digits at 30.3px against the 192px the template asks for:
    // a 312x43px blob in the middle of an otherwise black lobby screen. That defect is in the region countdown
    // path and predates this wave; what is new is that a fresh install now ships
    // templates that take it, where before it shipped none.
    //
    // The layer model does not have it. `TemplateRender`'s `.cd-default` block
    // declares its designed size, clips, sits inside the fit loop and reports
    // through `onFit` (rule 37). So these four are seeded as exactly what
    // `TemplateGallery.upgradeLegacyToLayers` would have converted them into on the
    // operator's first visit to the Templates tab: the bytes `regionsToLayers`
    // produces from the region shape below, with stable layer ids in place of its
    // generated ones. The conversion is moved to seed time so that first Sunday
    // does not depend on somebody having opened a tab.
    //
    // NOTHING ELSE MOVES. Every region-model key is kept, so everything that reads
    // that shape reads the same answer it did before — `templateKind` still derives
    // `scripture` for the four full-screen members and `lower-third` for the keyed
    // one, `isKeyedTemplate` still answers the same, and the `shows` list is
    // untouched. `upgradeLegacyToLayers` skips a template that is already layered,
    // so an install that has run it and one that has not now agree.
    const TIMER_CLASSIC: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":true,"shows":["scripture","song","media","announce","countdown"],"layers":[{"id":"clt-background","type":"background","name":"Background","visible":true,"x":0,"y":0,"w":100,"h":100,"fill":"radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)","image":null,"opacity":1,"dim":0},{"id":"clt-reference","type":"text","name":"Reference","visible":true,"x":8,"y":24,"w":84,"h":10,"bind":"reference","font":"var(--f-body)","color":"#e8a33d","size":2.6,"align":"center","valign":"middle","transform":"uppercase","lineHeight":1.2,"letterSpacing":0.06,"shadow":0,"italic":false,"scroll":false,"text":""},{"id":"clt-verse","type":"text","name":"Verse","visible":true,"x":8,"y":36,"w":84,"h":52,"bind":"verse","font":"var(--f-body)","color":"#f4e4c8","size":5,"align":"center","valign":"middle","transform":"none","lineHeight":1.32,"letterSpacing":0,"shadow":0,"italic":false,"scroll":false,"text":"","quote":true}]}"##;
    const TIMER_AURORA: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":true,"shows":["scripture","song","media","announce","countdown"],"layers":[{"id":"aut-background","type":"background","name":"Background","visible":true,"x":0,"y":0,"w":100,"h":100,"fill":"radial-gradient(130% 130% at 50% 15%, #0b3330, #04110f)","image":null,"opacity":1,"dim":0},{"id":"aut-reference","type":"text","name":"Reference","visible":true,"x":8,"y":24,"w":84,"h":10,"bind":"reference","font":"var(--f-display)","color":"#6ee7c4","size":2.6,"align":"center","valign":"middle","transform":"uppercase","lineHeight":1.2,"letterSpacing":0.06,"shadow":0.4,"italic":false,"scroll":false,"text":""},{"id":"aut-verse","type":"text","name":"Verse","visible":true,"x":8,"y":36,"w":84,"h":52,"bind":"verse","font":"var(--f-display)","color":"#eafff8","size":5,"align":"center","valign":"middle","transform":"none","lineHeight":1.32,"letterSpacing":0,"shadow":0.4,"italic":false,"scroll":false,"text":"","quote":true}]}"##;
    const TIMER_EMBER: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":true,"shows":["scripture","song","media","announce","countdown"],"layers":[{"id":"emt-background","type":"background","name":"Background","visible":true,"x":0,"y":0,"w":100,"h":100,"fill":"radial-gradient(130% 130% at 50% 18%, #3a1508, #140603)","image":null,"opacity":1,"dim":0},{"id":"emt-reference","type":"text","name":"Reference","visible":true,"x":8,"y":24,"w":84,"h":10,"bind":"reference","font":"var(--f-display)","color":"#ffb066","size":2.6,"align":"center","valign":"middle","transform":"uppercase","lineHeight":1.2,"letterSpacing":0.06,"shadow":0.45,"italic":false,"scroll":false,"text":""},{"id":"emt-verse","type":"text","name":"Verse","visible":true,"x":8,"y":36,"w":84,"h":52,"bind":"verse","font":"var(--f-display)","color":"#fdeede","size":5,"align":"center","valign":"middle","transform":"none","lineHeight":1.32,"letterSpacing":0,"shadow":0.45,"italic":false,"scroll":false,"text":"","quote":true}]}"##;
    const TIMER_HIVIS: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":true,"shows":["scripture","song","media","announce","countdown"],"layers":[{"id":"hvt-background","type":"background","name":"Background","visible":true,"x":0,"y":0,"w":100,"h":100,"fill":"#000000","image":null,"opacity":1,"dim":0},{"id":"hvt-reference","type":"text","name":"Reference","visible":true,"x":8,"y":24,"w":84,"h":10,"bind":"reference","font":"var(--f-display)","color":"#ffffff","size":3.2,"align":"center","valign":"middle","transform":"uppercase","lineHeight":1.2,"letterSpacing":0.06,"shadow":0,"italic":false,"scroll":false,"text":""},{"id":"hvt-verse","type":"text","name":"Verse","visible":true,"x":8,"y":36,"w":84,"h":52,"bind":"verse","font":"var(--f-display)","color":"#ffffff","size":6,"align":"center","valign":"middle","transform":"none","lineHeight":1.4,"letterSpacing":0,"shadow":0,"italic":false,"scroll":false,"text":"","quote":true}]}"##;
    // The keyed family replaces each of the five with its banded twin. On a band
    // `accent` IS the fill and `verseColor` the text, and `background` stays
    // `transparent` — the camera underneath is the point.
    const BAND_SCRIPTURE: &str = r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":true,"refFirst":false,"shows":["scripture","song","media","announce","countdown"]}"##;
    const BAND_LYRIC: &str = r##"{"regions":["verse_text"],"align":"center","lowerThird":true,"refFirst":false,"shows":["scripture","song","media","announce","countdown"]}"##;
    // THREE BANDED MEMBERS DO SOMETHING WORTH KNOWING BEFORE YOU PICK THEM, and
    // all three are the renderer's existing, deliberate behaviour rather than a
    // defect introduced here:
    //
    //   · `Lower Third · Media` paints the picture FULL FRAME. The region branch
    //     answers `content.media_url` before it ever reaches the band, so this is
    //     a keyed screen that stops being keyed for the duration of a picture.
    //     It exists because a family must answer every kind, and because a screen
    //     showing media was asked to show the picture.
    //   · `Lower Third · Timer` paints NOTHING. `countdownAllowed` is
    //     `!!countdownTo && !bandMode`: a countdown never reaches a lower third,
    //     because the band is keyed over a live camera and a clock ticking across
    //     the preacher belongs on the lobby screen. The member exists so the
    //     family is complete and so an operator who assigns it gets the same
    //     clean camera the transparency law would have given them anyway.
    //   · `Lower Third · Announcement` never renders the band geometry its
    //     `lowerThird` flag asserts. `scroll` is true, and the footer-ticker
    //     branch is tested BEFORE the band content block, so what paints is the
    //     ticker pinned to the bottom of the frame rather than the band. It does
    //     still KEY — nothing paints the rest of the frame — so a crawl over a
    //     live camera is exactly what an operator gets, which is what a keyed
    //     announcement should be. The flag is what keeps it on the keyed side of
    //     the transparency law; it is not what draws it.
    const BAND_MEDIA: &str = r##"{"regions":["reference"],"align":"center","lowerThird":true,"refFirst":true,"shows":["scripture","song","media","announce","countdown"]}"##;
    const BAND_ANNOUNCE: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":true,"refFirst":true,"shows":["scripture","song","media","announce","countdown"]}"##;
    // THE KEYED TIMER STAYS REGION-MODEL, and it is the one member of the five
    // that does. It has no blob to fix: `countdownAllowed = !!countdownTo &&
    // !bandMode` means the region path paints NOTHING under a countdown, and
    // `.bandless` makes the band itself transparent when there are no words, so
    // what an operator gets is the clean camera the transparency law would have
    // given them anyway. Converting it would take that away rather than fix
    // anything — the layer model has no equivalent refusal: `.cd-default` does not
    // ask about the band, and a band drawn as a `shape` layer paints whether or
    // not it holds words, so a converted keyed Timer shows either the digits or an
    // empty bar over a live camera. One of the five is not broken; it is not
    // touched.
    const BAND_TIMER: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":true,"refFirst":true,"shows":["scripture","song","media","announce","countdown"]}"##;

    &[
        // ── Classic — the serif look Relay opens with ─────────────────────────
        // Palette from `builtin_templates()`'s `Classic Serif`, unchanged. The
        // serif is the reading face and carries the verse; the other four kinds
        // are scanned rather than read, which is the argument `Worship Lyrics`
        // already makes in this file, so they take the body face.
        (
            "Classic · Scripture",
            SCRIPTURE,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)","accent":"#e8a33d","verseColor":"#f4e4c8","verseSize":"5.5","refSize":"2.6","italicRef":true}"##,
        ),
        (
            "Classic · Lyrics",
            LYRIC,
            r##"{"font":"var(--f-body)","background":"radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)","accent":"#e8a33d","verseColor":"#f4e4c8","verseSize":"8.5","refSize":"2","italicRef":false,"verseLineHeight":1.2}"##,
        ),
        (
            "Classic · Media",
            MEDIA,
            r##"{"font":"var(--f-body)","background":"radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)","accent":"#e8a33d","verseColor":"#f4e4c8","verseSize":"5.5","refSize":"2.6","italicRef":false}"##,
        ),
        (
            "Classic · Announcement",
            ANNOUNCE,
            r##"{"font":"var(--f-body)","background":"radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)","accent":"#e8a33d","verseColor":"#f4e4c8","verseSize":"3.4","refSize":"2.6","italicRef":false,"refTransform":"uppercase","refLetterSpacing":0.06,"scroll":true}"##,
        ),
        (
            "Classic · Timer",
            TIMER_CLASSIC,
            r##"{"font":"var(--f-body)","background":"radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)","accent":"#e8a33d","verseColor":"#f4e4c8","verseSize":"5","refSize":"2.6","italicRef":false,"refTransform":"uppercase","refLetterSpacing":0.06}"##,
        ),
        // ── Aurora — teal / emerald ───────────────────────────────────────────
        // The first three are the previously-seeded rows byte for byte, so a
        // church already pointing a screen at one sees nothing change. Media and
        // Timer are new and borrow their grounds from the two above rather than
        // introducing a colour the family does not have.
        (
            "Aurora · Scripture",
            SCRIPTURE,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(130% 130% at 50% 15%, #0b3330, #04110f)","accent":"#6ee7c4","verseColor":"#eafff8","verseSize":"5.2","refSize":"2.5","italicRef":true,"textShadow":0.5,"verseLineHeight":1.34}"##,
        ),
        (
            "Aurora · Lyrics",
            LYRIC,
            r##"{"font":"var(--f-display)","background":"linear-gradient(165deg, #08302b, #03110f)","accent":"#ffffff","verseColor":"#ffffff","verseSize":"8.5","refSize":"2","textShadow":0.4,"verseLineHeight":1.2}"##,
        ),
        (
            "Aurora · Media",
            MEDIA,
            r##"{"font":"var(--f-display)","background":"linear-gradient(180deg, #0b3330, #04110f)","accent":"#6ee7c4","verseColor":"#eafff8","verseSize":"5.2","refSize":"2.5","textShadow":0.4}"##,
        ),
        (
            "Aurora · Announcement",
            ANNOUNCE,
            r##"{"font":"var(--f-display)","background":"linear-gradient(180deg, #0b3330, #04110f)","accent":"#6ee7c4","verseColor":"#eafff8","verseSize":"3.4","refSize":"2.6","refTransform":"uppercase","refLetterSpacing":0.06,"scroll":true,"textShadow":0.4}"##,
        ),
        (
            "Aurora · Timer",
            TIMER_AURORA,
            r##"{"font":"var(--f-display)","background":"radial-gradient(130% 130% at 50% 15%, #0b3330, #04110f)","accent":"#6ee7c4","verseColor":"#eafff8","verseSize":"5","refSize":"2.6","refTransform":"uppercase","refLetterSpacing":0.06,"textShadow":0.4}"##,
        ),
        // ── Ember — amber / crimson ───────────────────────────────────────────
        // Same terms as Aurora: three carried forward verbatim, two new.
        (
            "Ember · Scripture",
            SCRIPTURE,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(130% 130% at 50% 18%, #3a1508, #140603)","accent":"#ffb066","verseColor":"#fdeede","verseSize":"5.2","refSize":"2.5","italicRef":true,"textShadow":0.55,"verseLineHeight":1.34}"##,
        ),
        (
            "Ember · Lyrics",
            LYRIC,
            r##"{"font":"var(--f-display)","background":"linear-gradient(165deg, #2c0f06, #130603)","accent":"#ffffff","verseColor":"#ffffff","verseSize":"8.5","refSize":"2","textShadow":0.45,"verseLineHeight":1.2}"##,
        ),
        (
            "Ember · Media",
            MEDIA,
            r##"{"font":"var(--f-display)","background":"linear-gradient(180deg, #3a1508, #140603)","accent":"#ffb066","verseColor":"#fdeede","verseSize":"5.2","refSize":"2.5","textShadow":0.45}"##,
        ),
        (
            "Ember · Announcement",
            ANNOUNCE,
            r##"{"font":"var(--f-display)","background":"linear-gradient(180deg, #3a1508, #140603)","accent":"#ffb066","verseColor":"#fdeede","verseSize":"3.4","refSize":"2.6","refTransform":"uppercase","refLetterSpacing":0.06,"scroll":true,"textShadow":0.45}"##,
        ),
        (
            "Ember · Timer",
            TIMER_EMBER,
            r##"{"font":"var(--f-display)","background":"radial-gradient(130% 130% at 50% 18%, #3a1508, #140603)","accent":"#ffb066","verseColor":"#fdeede","verseSize":"5","refSize":"2.6","refTransform":"uppercase","refLetterSpacing":0.06,"textShadow":0.45}"##,
        ),
        // ── Lower Third — the keyed family ────────────────────────────────────
        // Palette from `builtin_templates()`'s `Lower Third`. `background` is
        // `transparent` in all five, which is not decoration: a keyed channel that
        // paints a background covers the camera it exists to caption. The band
        // fill is `#101319` — neutral, and deliberately not a law colour.
        (
            "Lower Third · Scripture",
            BAND_SCRIPTURE,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#101319","verseColor":"#f2f4f8","verseSize":"2.6","refSize":"1.7","italicRef":false}"##,
        ),
        (
            "Lower Third · Lyrics",
            BAND_LYRIC,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#101319","verseColor":"#f2f4f8","verseSize":"3","refSize":"1.7","italicRef":false}"##,
        ),
        (
            "Lower Third · Media",
            BAND_MEDIA,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#101319","verseColor":"#f2f4f8","verseSize":"2.6","refSize":"1.7","italicRef":false}"##,
        ),
        (
            "Lower Third · Announcement",
            BAND_ANNOUNCE,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#101319","verseColor":"#f2f4f8","verseSize":"2.2","refSize":"1.5","italicRef":false,"refTransform":"uppercase","refLetterSpacing":0.08,"scroll":true}"##,
        ),
        (
            "Lower Third · Timer",
            BAND_TIMER,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#101319","verseColor":"#f2f4f8","verseSize":"2.6","refSize":"1.7","italicRef":false}"##,
        ),
        // ── High Visibility — the answer to a lit room and a cheap lens ───────
        // Every key is taken from `data/legacy_themes.json`'s frozen
        // `High Visibility` theme, which is the look a church that chose that
        // theme now carries inlined inside its own template, and which
        // `src/lib/legibility.test.js` measures at 21:1. White on pure black in
        // all five kinds, no shadow (a soft edge IS a contrast reduction) and no
        // transition (long enough to notice is long enough to disorient). The
        // reference is the SAME white as the verse rather than a tint, because a
        // coloured reference on black is the first thing to disappear for
        // somebody with low vision and it is the least important text on screen.
        //
        // ITS ANNOUNCEMENT IS THE ONE MEMBER THAT DOES NOT SCROLL, and that is
        // the deviation this family exists to make. `scroll` is absent, so the
        // notice renders through the centred `.content` block: it WRAPS, and it
        // sits inside the fit loop that reports when it has had to shrink
        // (rule 37). The ticker branch is tested before that block and carries no
        // fit reporting at all.
        //
        // The deciding fact is what the crawl does under `prefers-reduced-motion`,
        // which is the setting a low-vision or vestibular user's machine is most
        // likely to be carrying: `.ticker-run` gets `animation: none` and
        // `.ticker-track` gets `text-overflow: ellipsis` over `white-space: nowrap;
        // overflow: hidden`. The crawl stops, the notice becomes one line, and
        // anything longer than the track is CUT OFF — silently, on the one render
        // path this family is most likely to take. A static notice that wraps
        // cannot do that.
        //
        // THE COST, STATED: with no `scroll` this member is shaped exactly like a
        // full-screen verse — a large line over a small one — so `templateKind`
        // derives it as `scripture` and it shares the gallery's Scripture row
        // rather than sitting with the other four Announcements. That is the right
        // way round. The derivation is a heuristic about shape; what a
        // congregation can read is not, and a heuristic is the thing that bends.
        // `Notice Board` already records the same shape as honest rather than a
        // miss.
        (
            "High Visibility · Scripture",
            SCRIPTURE,
            r##"{"font":"var(--f-display)","background":"#000000","accent":"#ffffff","verseColor":"#ffffff","refColor":"#ffffff","verseSize":"8","refSize":"3.2","verseLineHeight":"1.4","refGap":"1.6","verseShadow":"0","refShadow":"0","transitionMs":"0","italicRef":false}"##,
        ),
        (
            "High Visibility · Lyrics",
            LYRIC,
            r##"{"font":"var(--f-display)","background":"#000000","accent":"#ffffff","verseColor":"#ffffff","refColor":"#ffffff","verseSize":"9","refSize":"3.2","verseLineHeight":"1.4","refGap":"1.6","verseShadow":"0","refShadow":"0","transitionMs":"0","italicRef":false}"##,
        ),
        (
            "High Visibility · Media",
            MEDIA,
            r##"{"font":"var(--f-display)","background":"#000000","accent":"#ffffff","verseColor":"#ffffff","refColor":"#ffffff","verseSize":"8","refSize":"3.2","verseLineHeight":"1.4","refGap":"1.6","verseShadow":"0","refShadow":"0","transitionMs":"0","italicRef":false}"##,
        ),
        (
            "High Visibility · Announcement",
            ANNOUNCE,
            r##"{"font":"var(--f-display)","background":"#000000","accent":"#ffffff","verseColor":"#ffffff","refColor":"#ffffff","verseSize":"4","refSize":"3.2","verseLineHeight":"1.4","refGap":"1.6","verseShadow":"0","refShadow":"0","transitionMs":"0","italicRef":false,"refTransform":"uppercase","refLetterSpacing":0.06}"##,
        ),
        (
            "High Visibility · Timer",
            TIMER_HIVIS,
            r##"{"font":"var(--f-display)","background":"#000000","accent":"#ffffff","verseColor":"#ffffff","refColor":"#ffffff","verseSize":"6","refSize":"3.2","verseLineHeight":"1.4","refGap":"1.6","verseShadow":"0","refShadow":"0","transitionMs":"0","italicRef":false,"refTransform":"uppercase","refLetterSpacing":0.06}"##,
        ),
    ]
}

/// THE SHELF — the prototype's own looks, in the LAYER model.
///
/// Everything above is region-model JSON written as a Rust string literal. These
/// are not, for one reason: the shapes the prototype's remaining looks are made
/// of — a `band` that names the words inside it (DECISIONS §75), a `region` that
/// is its own container (DECISIONS §74), a screen carrying monitor-only bindings
/// — exist only in the layer model, and hand-writing them twice (once for the
/// seed, once for whatever renders them in a test) is exactly how `BUILTINS` and
/// `builtin_templates()` drifted by a row and a kiosk rendered a song through a
/// scripture look.
///
/// So the shapes live in ONE file that both sides read: Rust `include_str!`s it
/// here, and `src/lib/shelf.test.js` reads the same bytes and renders every entry
/// through the real `TemplateRender`. Added by NAME like every other preset, so
/// no id an operator's channel points at is ever disturbed.
const SHELF_JSON: &str = include_str!("../../data/shelf_templates.json");

#[derive(Deserialize)]
struct ShelfFile {
    templates: Vec<ShelfEntry>,
}

#[derive(Deserialize)]
struct ShelfEntry {
    name: String,
    layout: Value,
    style: Value,
}

/// The shelf, parsed once. A malformed file yields an EMPTY shelf rather than a
/// panic — this runs on every database open, and a church whose app will not
/// start is a worse outcome than a church missing eight designs. It cannot ship
/// broken: `the_shelf_file_parses_and_every_entry_is_a_layer_stack` fails the
/// build, and the frontend reads the same bytes.
fn shelf_templates() -> &'static [(String, String, String)] {
    static SHELF: OnceLock<Vec<(String, String, String)>> = OnceLock::new();
    SHELF.get_or_init(|| match serde_json::from_str::<ShelfFile>(SHELF_JSON) {
        Ok(f) => f
            .templates
            .into_iter()
            .map(|e| (e.name, e.layout.to_string(), e.style.to_string()))
            .collect(),
        Err(e) => {
            eprintln!("shelf_templates.json could not be read ({e}) — shipping without the shelf");
            Vec::new()
        }
    })
}

/// Add every preset that is not already present, matched BY NAME so it is safe on
/// every boot and never disturbs the ids an operator's channels point at.
/// Additive, like `ensure_lyrics_template` — an operator who deleted or renamed a
/// preset does not get it silently resurrected under a different name, only the
/// ones genuinely absent are inserted.
/// Every ready-to-use design that ships on top of the five built-ins: the
/// standalone presets, the coordinated theme families, and the shelf.
fn all_presets() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    region_presets().chain(
        shelf_templates()
            .iter()
            .map(|(n, l, s)| (n.as_str(), l.as_str(), s.as_str())),
    )
}

/// The REGION-model half of the shelf — the standalone presets and the theme
/// families. Kept nameable on its own because the properties a region template
/// can be checked for (`style.background`, `style.verseColor`) simply do not
/// exist on a layer template, where the same facts live per element. A test that
/// asserted them over both would either fail on the layer entries or be softened
/// until it asserted nothing about either.
fn region_presets() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    preset_templates()
        .iter()
        .chain(theme_templates().iter())
        .map(|(n, l, s)| (*n, *l, *s))
}

/// How many presets ship on top of the five built-ins — so tests can assert the
/// seeded total without hard-coding a number that drifts.
#[cfg(test)]
pub(super) fn preset_template_count() -> usize {
    all_presets().count()
}

/// THE SEEDED LOWER THIRD'S BAND MUST NOT WEAR A LAW COLOUR.
///
/// The seeds insert only when a name is ABSENT, so correcting the shipped value
/// reaches a fresh install and no existing one. That is right for anything an
/// operator might have edited — and wrong here, because of what else changed in
/// the same wave.
///
/// The band on a lower third had never painted: `panelBg` fell through to the
/// string `transparent` for any keyed template and was written inline, where it
/// beat the stylesheet rule meant to fill it. Fixing that makes the fill VISIBLE
/// for the first time — and on an existing install the fill is still `#b080e0`,
/// amethyst, which rule 18 reserves for REHEARSAL. So the repair would have put a
/// lilac bar on every stream, in a colour that already means something else, for
/// exactly the churches already running Relay.
///
/// NARROW ON PURPOSE. It rewrites the accent only where the value is still one of
/// the two shipped purples (the JS and Rust seed lists had drifted to `#8b5cf6`
/// and `#b080e0` for one template). A church that chose its own band colour has a
/// value in neither set and is left alone — this corrects a default nobody picked,
/// it does not overwrite a decision somebody made.
///
/// Idempotent and retryable (rule 25): a second run matches nothing.
pub(super) fn ensure_lower_third_band_is_not_a_law_colour(
    conn: &Connection,
) -> rusqlite::Result<()> {
    // The two values the two seed lists shipped, and nothing else.
    for old in ["#b080e0", "#8b5cf6"] {
        conn.execute(
            "UPDATE templates
                SET style_json = replace(style_json, ?1, '#101319')
              WHERE name = 'Lower Third'
                AND style_json LIKE '%' || ?1 || '%'",
            [old],
        )?;
    }
    // The dark type that went with the light band would be invisible on it.
    conn.execute(
        "UPDATE templates
            SET style_json = replace(style_json, '#1c1224', '#f2f4f8')
          WHERE name = 'Lower Third'
            AND style_json LIKE '%#101319%'
            AND style_json LIKE '%#1c1224%'",
        [],
    )?;
    Ok(())
}

/// THE THEMES AS THEY STOOD WHEN THEY WERE FOLDED INTO TEMPLATES.
///
/// A FROZEN SNAPSHOT on purpose: the migration below inlines the values a
/// template actually rendered with, so it must not read a definition that keeps
/// moving. The file carries two required fields, `themes` (the nine builtins,
/// ids -1 to -9) and `style_keys` (the whitelist `applyTheme` filtered a theme
/// through at render time). Its own `_readme` says why. It was pinned against
/// the live JS table by `src/lib/legacythemes.test.js` while both existed; that
/// table is now deleted (DECISIONS §87) and this file is the only copy, so what
/// holds it is `the_shipped_snapshot_parses_so_that_branch_is_never_taken_in_a_real_build`
/// below — which is the guard that matters, because a snapshot that stops
/// parsing makes this whole migration a silent no-op.
const LEGACY_THEMES_JSON: &str = include_str!("../../data/legacy_themes.json");

#[derive(Deserialize)]
struct FrozenThemes {
    /// `THEME_STYLE_KEYS` as `src/lib/themes.js` carried it at the moment it was
    /// frozen. That file is gone (DECISIONS §87 — themes were folded into
    /// templates once this migration had run), so the snapshot is the only copy,
    /// which is the point of a snapshot. Required: without it this cannot
    /// reproduce what reached a screen, and guessing is how a look changes on an
    /// update.
    style_keys: Vec<String>,
    themes: Vec<FrozenTheme>,
}

/// Only the id and the style travel. A builtin theme's NAME is never needed
/// here: a builtin could reach a wall only through a template's `themeRef`, so
/// it has nothing to be preserved as. The custom themes, which do, carry their
/// own names out of `app_settings`.
#[derive(Deserialize)]
struct FrozenTheme {
    id: i64,
    style: serde_json::Map<String, Value>,
}

/// The layout a preserved custom theme becomes: the same scripture shape every
/// seeded theme family uses, so the operator's look opens as a usable template
/// rather than a style blob with no regions.
const ORPHAN_THEME_LAYOUT: &str = r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":false,"refFirst":false}"##;

/// One of the operator's own themes as it comes out of `app_settings`: its id,
/// the name it will be preserved under if nothing referenced it, and its style.
type CustomTheme = (i64, String, serde_json::Map<String, Value>);

/// INLINE EVERY THEME A TEMPLATE POINTED AT, THEN DROP THE THEMES.
///
/// A theme reached a screen exactly one way: a template's `style.themeRef`,
/// resolved at render time as `{ ...theme.style, ...template.style }`
/// (`applyTheme`). Writing that same merge into the template leaves the
/// effective look unchanged BY CONSTRUCTION. The template's own keys still win
/// and only the keys it left unset are filled, which is the precedence the
/// renderer already applied.
///
/// Three details the merge would be wrong without:
///
/// * **The theme side is filtered to `style_keys`.** `applyTheme` copied only
///   the whitelisted keys, so a key off that list was dropped on the way to the
///   screen (`templatedoors.test.js`, "the theme door"). Inlining one would put
///   a value on a wall that never received it. The nine builtins are all inside
///   the list; a church's own imported theme need not be. The TEMPLATE's own
///   keys are never filtered, because they were never filtered.
/// * **A dangling ref is dropped and nothing is invented.** `resolveThemed`
///   degraded to the template's own look rather than blanking, so that is what
///   the wall was already doing and what the migration must agree with.
/// * **A custom theme nothing referenced becomes a template.** It changed no
///   screen (there is no active-theme concept), but it is a look the operator
///   built and the key it lives in is being deleted. Added BY NAME and only when
///   absent, like every other seed.
///
/// Retryable (rule 25). There is no scratch table to strand, and the whole
/// thing, the rewrites, the preserved themes and the removal of
/// `themes.custom`, is ONE transaction: a failure or a process death anywhere
/// leaves the database exactly as it was, and the next boot runs a clean first
/// attempt. That atomicity is not a tidiness point. Rewriting the templates and
/// deleting the setting in two steps has a window in which the refs are gone but
/// `themes.custom` is still there, and a retry landing in it would find no ref
/// pointing at any custom theme and duplicate every one of them as a template.
/// `unchecked_transaction` rolls back when it is dropped, so every error path
/// out of here, `?` included, closes the transaction rather than leaving it open
/// for the `PRAGMA foreign_keys = ON` that follows to no-op inside.
///
/// Idempotent: a second run finds no `themeRef` to act on and no
/// `themes.custom` to preserve.
pub(super) fn ensure_themes_are_inlined(conn: &Connection) -> rusqlite::Result<()> {
    inline_themes(conn, LEGACY_THEMES_JSON)
}

/// The body, taking the snapshot as an argument so the malformed-file path is a
/// thing a test can actually drive. Reading it from a `const` made the one branch
/// that decides whether an operator's themes survive a broken build the one branch
/// no test could reach.
fn inline_themes(conn: &Connection, snapshot: &str) -> rusqlite::Result<()> {
    // A MALFORMED SNAPSHOT MUST NOT STOP THE APP BOOTING, AND MUST NOT DESTROY
    // ANYTHING EITHER. It returns having touched nothing at all. A `themeRef`
    // left in a style is harmless (the renderer ignores the key, and with no
    // themes to resolve it against a template shows its own look, which is what
    // a dangling ref always did), so the whole job simply waits for a release
    // that fixes the file, with the operator's themes still in `app_settings`.
    // The earlier version of this branch carried on with no known themes: it
    // dropped every ref, inlined nothing, and then deleted `themes.custom`,
    // which is a look changing AND the look being erased. It cannot ship broken:
    // `legacythemes.test.js` reads the same bytes.
    let (whitelist, mut known) = match serde_json::from_str::<FrozenThemes>(snapshot) {
        Ok(f) => (
            f.style_keys,
            f.themes
                .into_iter()
                .map(|t| (t.id, t.style))
                .collect::<Vec<_>>(),
        ),
        Err(e) => {
            eprintln!("legacy_themes.json could not be read ({e}); themes are left alone");
            return Ok(());
        }
    };
    let whitelisted = |k: &String| whitelist.iter().any(|w| w == k);

    // The operator's own themes, from the key that is about to be deleted. An
    // entry with no numeric id or no style object is skipped, because
    // `parseThemes` skipped it too and a template pointing at one was already
    // rendering as a dangling ref.
    let custom_raw = get_setting(conn, "themes.custom")?;
    // `None` here means the blob did not parse, which is NOT the same as an
    // operator with no custom themes. Only the second may have its key deleted:
    // destroying a blob nobody could read is destroying the only copy of
    // whatever was in it.
    let parsed: Option<Vec<CustomTheme>> = custom_raw
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Vec<Value>>(raw).ok())
        .map(|list| {
            list.into_iter()
                .filter_map(|t| {
                    let id = t.get("id").and_then(Value::as_i64)?;
                    let style = t.get("style").and_then(Value::as_object)?.clone();
                    let name = t.get("name").and_then(Value::as_str).unwrap_or("").trim();
                    let name = if name.is_empty() { "Theme" } else { name };
                    Some((id, name.to_string(), style))
                })
                .collect()
        });
    let custom: &[CustomTheme] = parsed.as_deref().unwrap_or(&[]);
    known.extend(custom.iter().map(|(id, _, s)| (*id, s.clone())));

    let tx = conn.unchecked_transaction()?;

    // ONLY the styles that can possibly carry a ref. After the one-off pass this
    // matches nothing, and the scan costs a `LIKE` rather than every template's
    // style blob through serde on every boot: a single blob in this repository
    // has reached 13 MB (a `data:` URL background), and a church can have many.
    let rows: Vec<(i64, String)> = {
        let mut stmt =
            tx.prepare("SELECT id, style_json FROM templates WHERE style_json LIKE '%themeRef%'")?;
        let it = stmt.query_map([], |r| Ok((r.get(0)?, r.get::<_, String>(1)?)))?;
        it.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut inlined: Vec<i64> = Vec::new();
    for (id, style_json) in rows {
        let Ok(Value::Object(mut style)) = serde_json::from_str::<Value>(&style_json) else {
            continue; // not a style object: leave it exactly as it is
        };
        // The key is reserved, the renderer already ignored it, and it is being
        // retired, so it is removed whatever it held. Only a NUMBER ever named a
        // theme (`templateThemeRef` returns null for anything else), so only a
        // number can inline one.
        let Some(theme_ref) = style.remove("themeRef") else {
            continue; // nothing pinned here: no write, which is the idempotency
        };
        if let Some(theme_ref) = theme_ref.as_i64() {
            if let Some((_, theme_style)) = known.iter().find(|(tid, _)| *tid == theme_ref) {
                let mut kept = 0usize;
                for (k, v) in theme_style {
                    if whitelisted(k) && !style.contains_key(k) {
                        style.insert(k.clone(), v.clone());
                        kept += 1;
                    }
                }
                // `inlined` means A KEY OF THIS THEME NOW LIVES IN A TEMPLATE, not
                // merely that the id matched. A theme whose keys were all off the
                // whitelist, or all already set by the template itself, gave this
                // template nothing: counting it as preserved here would skip it in
                // the loop below and then delete it, which is the operator's look
                // erased on the strength of a match that saved none of it.
                if kept > 0 {
                    inlined.push(theme_ref);
                }
            }
        }
        let Ok(next) = serde_json::to_string(&Value::Object(style)) else {
            continue; // unserialisable: better the old look than an empty one
        };
        tx.execute(
            "UPDATE templates SET style_json = ?1 WHERE id = ?2",
            (next, id),
        )?;
    }

    for (id, name, style) in custom {
        if inlined.contains(id) {
            continue; // already preserved, inside the template that used it
        }
        // A TAKEN NAME IS RENAMED, NEVER SKIPPED. Two custom themes can share a
        // name, and one can share a name with a template that already exists;
        // skipping on a collision meant the key was deleted a few lines later
        // and a look the operator built was gone, silently and with no way
        // back. Nothing this migration touches is discarded, so the theme goes
        // in under a name that is free and the line below says which one.
        let mut chosen = name.clone();
        if name_is_taken(&tx, &chosen)? {
            chosen = format!("{name} (theme)");
            let mut n = 2;
            while name_is_taken(&tx, &chosen)? {
                chosen = format!("{name} (theme {n})");
                n += 1;
            }
        }
        if chosen == *name {
            eprintln!("themes: keeping {name:?} as a template (nothing referenced it)");
        } else {
            eprintln!(
                "themes: keeping {name:?} as a template named {chosen:?} (the name was taken)"
            );
        }
        // NOT whitelist-filtered, deliberately: this one never rendered, so
        // there is no look to reproduce, only saved work to keep whole.
        tx.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (
                &chosen,
                ORPHAN_THEME_LAYOUT,
                Value::Object(style.clone()).to_string(),
            ),
        )?;
    }
    // Gated on having PARSED the blob, not on its being present. An unreadable
    // blob is left exactly where it is: it is the only copy of whatever the
    // operator saved, and this migration cannot preserve what it cannot read.
    if parsed.is_some() {
        tx.execute("DELETE FROM app_settings WHERE key = 'themes.custom'", [])?;
    }

    tx.commit()
}

/// Whether a template of this exact name already exists. Its own function only so
/// the rename loop above reads as the one question it asks repeatedly.
fn name_is_taken(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM templates WHERE name = ?1",
        [name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub(super) fn ensure_preset_templates(conn: &Connection) -> rusqlite::Result<()> {
    let mut check = conn.prepare("SELECT COUNT(*) FROM templates WHERE name = ?1")?;
    let mut insert = conn.prepare(
        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
    )?;
    for (name, layout, style) in all_presets() {
        let present: i64 = check.query_row([name], |r| r.get(0))?;
        if present == 0 {
            insert.execute((name, layout, style))?;
        }
    }
    Ok(())
}

/// THE ROWS THIS WAVE STOPPED SHIPPING, as the bytes they were inserted with.
///
/// A frozen record on purpose, like `legacy_themes.json` above it: the migration
/// below decides whether a row is a leftover or somebody's work by comparing it
/// with what the seed actually wrote, so it must not read a definition that
/// keeps moving. The seed lists it came from are empty now, which is exactly why
/// it exists: emptying a list reaches a fresh install and no existing one.
const RETIRED_PRESETS_JSON: &str = include_str!("../../data/retired_presets.json");

/// The frozen triples, or an EMPTY list if the record cannot be read.
///
/// `layout` and `style` are RAW STRINGS, not parsed JSON, and that is
/// load-bearing. The match below is on BYTES, and `serde_json`'s map is a
/// BTreeMap: parsing and re-serialising sorts the keys, producing a string that
/// equals nothing in any database, so the migration would silently retire
/// nothing. The same trap the `starts_with` frame matcher fell into (rule 43):
/// a check that looks exhaustive and matches nothing.
///
/// An unreadable record yields an empty list and the migration therefore retires
/// nothing at all. That is the right way for this one to fail: a church keeps a
/// few templates it does not want, rather than losing ones it does.
fn retired_presets() -> Vec<(String, String, String)> {
    #[derive(Deserialize)]
    struct Retired {
        templates: Vec<RetiredEntry>,
    }
    #[derive(Deserialize)]
    struct RetiredEntry {
        name: String,
        layout: String,
        style: String,
    }
    match serde_json::from_str::<Retired>(RETIRED_PRESETS_JSON) {
        Ok(r) => r
            .templates
            .into_iter()
            .map(|e| (e.name, e.layout, e.style))
            .collect(),
        Err(e) => {
            eprintln!("retired_presets.json could not be read ({e}); retiring nothing");
            Vec::new()
        }
    }
}

/// Whether this table exists AND still carries a `template_id` a row could point
/// through. `pragma_table_info` returns no rows at all for a table that is not
/// there, so one question answers both.
///
/// It has to be asked. This migration runs early in `ensure_tables`, and it has
/// to: the seed that replaces these rows runs a line later, and a name it finds
/// present is a name it will not insert. `ensure_service_plans` creates
/// `plan_items` further down that same ladder. `docs/data/schema-baseline.sql`,
/// the oldest schema Relay can upgrade FROM, creates neither `plan_items` nor
/// `app_settings`, and `migrate` sends a `user_version == 0` database through
/// `baseline_forward_fill`, which calls `ensure_tables`, which calls this. So a
/// guard naming `plan_items` would be `no such table: plan_items`, propagated
/// out of `migrate`, panicking the app at startup before the window is shown:
/// rule 25's failure, reached by a different road.
///
/// Skipping an absent door loses no guarantee. A table that does not exist holds
/// no rows, and the one that is created later in this same boot is created
/// empty, so nothing can be pointing at a template through it.
fn points_at_a_template(conn: &Connection, table: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = 'template_id'",
        [table],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// REMOVE A PRESET NOBODY CHOSE, and nothing else.
///
/// Seeds insert by name and only when absent, so an install that predates the
/// five families keeps every one of its old rows AND receives the twenty-five on
/// top: a gallery an operator scrolls on a Sunday morning, at twice the length,
/// with the leftovers interleaved among the rows that replaced them.
///
/// A row goes only when all three hold:
/// 1. its name is one of the frozen twenty-one;
/// 2. its `region_config_json` and `style_json` still equal the bytes the seed
///    wrote, so a single edited colour makes it the operator's and it stays;
/// 3. nothing points at it, through any of the FOUR doors.
///
/// Those doors are a channel, a plan cue, a content look and the configured
/// default. Three of four is the bug this repository has had four times, and two
/// of the four are `app_settings` rows rather than foreign keys, so no
/// `NOT IN (SELECT ...)` can reach them and they are asked separately below.
///
/// Rule 25: the whole loop is ONE transaction, and `unchecked_transaction` rolls
/// back when it is dropped, so every error path out of here, `?` included,
/// closes it rather than leaving it open for the `PRAGMA foreign_keys = ON` that
/// follows to no-op inside. Atomicity is not tidiness here either: a run that
/// died after three of twenty-one deletes would commit a half-retired gallery
/// AND still propagate the error that stopped the boot, leaving an operator with
/// an install nobody can describe. Idempotent: a second run finds no row whose
/// name and bytes both still match, and a fresh install never had these names.
///
/// ONE PERMANENT CONSEQUENCE, WRITTEN DOWN BECAUSE IT IS NOT REVERSIBLE AND NOT
/// VISIBLE. A single name is on both lists: the shelf's `Lower Third · Scripture`
/// is retired, and the keyed family's member of that name is seeded. Where the
/// old row is edited or in use it is correctly KEPT, and `ensure_preset_templates`
/// inserts by name only when ABSENT, so the family member is not installed on
/// that boot. The name never becomes absent, so it is not installed on any later
/// boot either: that church has a Lower Third family of FOUR, for good, and
/// nothing on screen says why. Both halves are deliberate (a row somebody uses is
/// never deleted; a seed never overwrites a name that is taken), so this is a
/// consequence rather than a bug, but a church counting its gallery should not
/// have to derive it. The loop at the end of this function says it once per boot,
/// and `data/retired_presets.json`'s `_readme` records it beside the bytes.
/// Is the RETIRED row of this name still in the table — name AND BYTES?
///
/// THE QUESTION MATTERS MORE THAN THE QUERY. Its one caller prints the single
/// diagnostic for the one permanent consequence of this wave: a name the seed
/// still ships that could not be installed because the old row of that name had
/// to be kept. Asking only "is a row of this name present" answers a DIFFERENT
/// question, and answers it wrong on every healthy install from the second boot
/// onwards — boot 1 deletes the retired row and `ensure_preset_templates` inserts
/// the family's own member of that name a line later, so the name is present for
/// ever after and the line printed "is not installed" over an install where the
/// template demonstrably is. A sentence that reads the same whether the thing
/// behind it is fine or broken is not a diagnostic (rule 35).
///
/// The triple is the same one the DELETE above it matches on, which is what makes
/// the two agree by construction rather than by reading alike. It is a function so
/// that the tests can ask it the question the migration asks, rather than a
/// question of their own that no edit to this file could ever falsify.
fn retired_row_is_still_present(
    conn: &Connection,
    name: &str,
    layout: &str,
    style: &str,
) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM templates
          WHERE name = ?1 AND region_config_json = ?2 AND style_json = ?3",
        (name, layout, style),
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub(super) fn ensure_retired_presets_are_gone(conn: &Connection) -> rusqlite::Result<()> {
    let retired = retired_presets();
    if retired.is_empty() {
        return Ok(()); // an unreadable record, or nothing left to retire
    }

    // THE TWO DOORS THAT ARE SETTINGS ROWS, NOT FOREIGN KEYS. Read with `?`, not
    // `.ok()`: a failed read is not the same fact as "no look is bound", and
    // treating it as one deletes a template a screen is wearing. Stopping here
    // retires nothing, which is the safe direction to fail in.
    let mut looks: Vec<i64> = Vec::new();
    for kind in ["scripture", "song", "media", "announce", "countdown"] {
        if let Some(id) = crate::db::settings::content_template_id(conn, kind)? {
            looks.push(id);
        }
    }
    // The default is CLEARED by writing an EMPTY STRING, so a value that does not
    // parse is "no default", not an error. This key DOES have a typed writer now:
    // `set_default_template` (main.rs) writes `""` for `None`. It used to be a
    // bare `set_setting` from `setDefaultTemplate` (src/lib/stores/capture.js),
    // and the empty-string convention survived that change — which is why the
    // tolerant parse below is still the right read.
    let default_id = get_setting(conn, "default_template_id")?.and_then(|s| s.parse::<i64>().ok());

    // THE TWO THAT ARE FOREIGN KEYS, folded into the DELETE so the check and the
    // removal are one statement.
    let mut guards = String::new();
    for table in ["output_channels", "plan_items"] {
        if points_at_a_template(conn, table)? {
            guards.push_str(&format!(
                " AND id NOT IN (SELECT template_id FROM {table} WHERE template_id IS NOT NULL)"
            ));
        }
    }
    let delete_sql = format!("DELETE FROM templates WHERE id = ?1{guards}");

    let tx = conn.unchecked_transaction()?;
    for (name, layout, style) in &retired {
        // Every row with this name AND these exact bytes. Plural because a name
        // is not unique in this table: one that matched with different bytes is
        // a template somebody edited and is not selected at all.
        let ids: Vec<i64> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM templates
                  WHERE name = ?1 AND region_config_json = ?2 AND style_json = ?3",
            )?;
            let it = stmt.query_map((name, layout, style), |r| r.get(0))?;
            it.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for id in ids {
            if looks.contains(&id) || default_id == Some(id) {
                continue;
            }
            tx.execute(&delete_sql, [id])?;
        }
    }

    // A NAME THE SEED STILL SHIPS, KEPT AS THE OLD ROW. See the note on this
    // function: `ensure_preset_templates` runs a line later and inserts by name
    // only when absent, so the seeded template of this name is not installed on
    // this boot, and the name never becomes absent, so it is not installed on any
    // later boot either. One line, because a church counting its gallery should
    // not have to diagnose a permanently missing family member from scratch. It
    // costs one COUNT per contested name per boot, and there is exactly one
    // contested name: the whole-triple disjointness of the two lists is asserted
    // by `the_frozen_record_parses_and_names_nothing_the_seed_still_ships`.
    for (name, layout, style) in &retired {
        if !all_presets().any(|(n, _, _)| n == name.as_str()) {
            continue;
        }
        if retired_row_is_still_present(&tx, name, layout, style)? {
            eprintln!(
                "templates: {name:?} is kept as the retired row (it is in use, or it was \
                 edited), so the seeded template of that name is not installed"
            );
        }
    }

    tx.commit()
}

/// Seed the built-in templates into a fresh DB (ids 1..4).
pub(super) fn seed_templates(conn: &Connection) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
    )?;
    for (name, layout, style) in builtin_templates() {
        stmt.execute((name, layout, style))?;
    }
    Ok(())
}

/// Reset the built-in templates IN PLACE — keeps ids stable so output_channels
/// FKs stay valid. Used by the vw→cqw migration.
///
/// NOTE: this rewrites ids 1..N positionally. "Worship Lyrics" was added as the
/// FOURTH entry, so on an existing database this renames whatever sat at id 4
/// (Lobby Warm) — which is why `ensure_lyrics_template` appends instead of
/// relying on this, and why this is only called by the one migration that
/// already intended a full rewrite.
pub(super) fn reset_builtin_templates(conn: &Connection) -> rusqlite::Result<()> {
    for (i, (name, layout, style)) in builtin_templates().iter().enumerate() {
        conn.execute(
            "UPDATE templates SET name = ?1, region_config_json = ?2, style_json = ?3 WHERE id = ?4",
            (name, layout, style, i as i64 + 1),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod lyrics_template_tests {
    use super::*;
    use crate::db::SCHEMA;

    /// An OLD database — seeded with the built-ins as they were BEFORE the
    /// lyrics template existed. That is the only state the migration is for; a
    /// fresh install gets it from `seed_templates` directly.
    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch("COMMIT;").ok();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .unwrap();
        let mut stmt = conn
            .prepare(
                "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            )
            .unwrap();
        for (name, layout, style) in builtin_templates()
            .iter()
            .filter(|(n, _, _)| *n != "Worship Lyrics")
        {
            stmt.execute((name, layout, style)).unwrap();
        }
        drop(stmt);
        conn
    }

    #[test]
    fn a_fresh_install_already_has_it() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch("COMMIT;").ok();
        seed_templates(&conn).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM templates WHERE name = 'Worship Lyrics'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn a_lyric_template_has_no_reference_region() {
        // THE BUG THIS FIXES: lyrics rendered through a scripture template put a
        // large "Song Title · Slide 7" where the words should be. The
        // congregation is not singing the title.
        let (_, layout, _) = builtin_templates()
            .iter()
            .find(|(n, _, _)| *n == "Worship Lyrics")
            .copied()
            .expect("no lyrics template");
        assert!(layout.contains("verse_text"));
        assert!(
            !layout.contains("reference"),
            "the lyrics template still draws a reference"
        );
    }

    #[test]
    fn lyrics_are_set_much_larger_than_scripture() {
        // A lyric is a few short lines read from the back of a lit room, not a
        // paragraph with a citation.
        let size = |name: &str| -> f32 {
            let (_, _, style) = builtin_templates()
                .iter()
                .find(|(n, _, _)| *n == name)
                .copied()
                .unwrap();
            let v: serde_json::Value = serde_json::from_str(style).unwrap();
            v["verseSize"].as_str().unwrap().parse().unwrap()
        };
        assert!(
            size("Worship Lyrics") >= size("Classic Serif") * 1.5,
            "lyrics are not meaningfully larger than scripture"
        );
    }

    #[test]
    fn the_migration_is_idempotent() {
        // CLAUDE.md §25: a migration must be safe to re-run. This one runs on
        // every boot.
        let conn = db();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        ensure_lyrics_template(&conn).unwrap();
        let once: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        ensure_lyrics_template(&conn).unwrap();
        ensure_lyrics_template(&conn).unwrap();
        let thrice: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        assert_eq!(once, before + 1, "the lyrics template was not added");
        assert_eq!(once, thrice, "re-running the migration duplicated it");
    }

    #[test]
    fn the_migration_never_overrules_an_operators_choice() {
        // Silently repointing a church's song template at ours would be worse
        // than the bug being fixed. The operator's choice lives under the
        // canonical `tpl_song` key (set_content_template writes `tpl_{kind}`).
        let conn = db();
        crate::db::settings::set_setting(&conn, "tpl_song", "2").unwrap();
        ensure_lyrics_template(&conn).unwrap();
        let v = crate::db::settings::get_setting(&conn, "tpl_song")
            .unwrap()
            .unwrap();
        assert_eq!(
            v, "2",
            "the operator's chosen song template was overwritten"
        );
    }

    #[test]
    fn songs_point_at_the_lyrics_template_when_nothing_was_chosen() {
        // The whole point of the fix: the seed must land under the SAME key the
        // read side uses, so `content_template_id("song")` actually resolves. It
        // previously wrote `content_template_song` and read `tpl_song`, so this
        // returned None and the lyrics default silently never applied.
        let conn = db();
        ensure_lyrics_template(&conn).unwrap();
        let id = crate::db::settings::content_template_id(&conn, "song")
            .unwrap()
            .expect("song content-type default should resolve after seeding");
        let name: String = conn
            .query_row("SELECT name FROM templates WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "Worship Lyrics");
    }

    #[test]
    fn a_value_stranded_under_the_legacy_key_is_migrated() {
        // An existing install may still carry the operator-invisible seed under
        // the old `content_template_song` key. The migration moves it to the
        // canonical key (so it finally takes effect) and drops the dead key.
        let conn = db();
        crate::db::settings::set_setting(&conn, "content_template_song", "3").unwrap();
        ensure_lyrics_template(&conn).unwrap();
        assert_eq!(
            crate::db::settings::content_template_id(&conn, "song").unwrap(),
            Some(3),
            "the legacy value was not migrated to the canonical key"
        );
        assert!(
            crate::db::settings::get_setting(&conn, "content_template_song")
                .unwrap()
                .is_none(),
            "the dead legacy key was left behind"
        );
    }
}

#[cfg(test)]
mod preset_template_tests {
    use super::*;
    use crate::db::SCHEMA;

    fn fresh() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch("COMMIT;").ok();
        conn
    }

    #[test]
    fn every_preset_is_added_exactly_once_and_re_running_adds_nothing() {
        // Runs on every boot — it MUST be idempotent (CLAUDE.md §25).
        let conn = fresh();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        ensure_preset_templates(&conn).unwrap();
        let once: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        ensure_preset_templates(&conn).unwrap();
        ensure_preset_templates(&conn).unwrap();
        let thrice: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            once,
            before + preset_template_count() as i64,
            "presets were not all added"
        );
        assert_eq!(once, thrice, "re-running duplicated presets");
    }

    #[test]
    fn there_are_presets_across_every_screen_type() {
        // The families are now the whole region-model seed. The standalone
        // presets were retired into them, so this asserts the ABSENCE rather than
        // a range: a count of twelve-to-fifteen loose looks would mean somebody
        // re-seeded a list whose rows a migration is busy deleting from every
        // existing install, which is a row deleted and re-inserted under a new id
        // on every boot, repointing whatever channel or plan cue pointed at it.
        assert!(
            preset_templates().is_empty(),
            "the standalone presets were retired into the five families; {} came back",
            preset_templates().len()
        );

        // Kind coverage still has to hold, and it is the reason this test did not
        // simply go away with the list it used to count: the families must between
        // them still cover the shapes a screen can be, so a future edit cannot
        // quietly drop a category.

        let kind = |layout: &str| -> &'static str {
            let v: serde_json::Value = serde_json::from_str(layout).unwrap();
            let band = v["lowerThird"].as_bool().unwrap_or(false);
            let regions: Vec<String> = v["regions"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let has = |r: &str| regions.iter().any(|x| x == r);
            if band {
                "lower-third"
            } else if has("reference") && has("verse_text") {
                "scripture"
            } else if has("verse_text") {
                "song"
            } else {
                "custom"
            }
        };
        let mut kinds: Vec<&str> = region_presets().map(|(_, l, _)| kind(l)).collect();
        kinds.sort();
        kinds.dedup();
        for want in ["scripture", "song", "lower-third"] {
            assert!(kinds.contains(&want), "no preset of kind {want}");
        }
    }

    #[test]
    fn every_family_is_complete_across_every_content_kind() {
        // A FAMILY IS A SET. An operator picks a look and fires five kinds of
        // content at it over a morning; a family missing its Timer means the
        // pre-service countdown falls back to a mismatched default in front of
        // a filling room, which is worse than the family not existing.
        //
        // Five kinds, because CONTENT_KINDS has five (src/lib/layers.js:228) and
        // the seed had four — Media and Timer were the two nobody could pick a
        // coordinated look for.
        use std::collections::HashSet;
        let mut families: HashSet<&str> = HashSet::new();
        for (name, _, _) in theme_templates() {
            match name.split_once(" · ") {
                Some((f, _)) => {
                    families.insert(f);
                }
                None => panic!("seed template {name:?} is not named 'Family · Kind'"),
            }
        }
        let mut want: Vec<&str> = vec![
            "Classic",
            "Aurora",
            "Ember",
            "Lower Third",
            "High Visibility",
        ];
        want.sort();
        let mut got: Vec<&str> = families.into_iter().collect();
        got.sort();
        assert_eq!(
            got, want,
            "the five families are fixed by DECISIONS, not by taste"
        );
        for family in &want {
            for kind in ["Scripture", "Lyrics", "Media", "Announcement", "Timer"] {
                let name = format!("{family} · {kind}");
                assert!(
                    theme_templates().iter().any(|(n, _, _)| *n == name),
                    "family {family:?} is missing its {kind} template"
                );
            }
        }
        assert_eq!(theme_templates().len(), 25);
        // CARRIED OVER from `every_theme_is_a_complete_coordinated_family`, which
        // this test replaces. Nothing else in either suite holds it, and the
        // ticker is a real render path (`TemplateRender.svelte`'s `scroll` branch)
        // whose only seeded example lives here. Dropping it with the old test
        // would have been coverage lost silently in a rename.
        assert!(
            theme_templates()
                .iter()
                .any(|(_, _, style)| style.contains("\"scroll\":true")),
            "no family ships a scrolling announcement"
        );
    }

    #[test]
    fn the_full_screen_timers_are_seeded_in_the_layer_model() {
        // WHAT A LOBBY SCREEN DOES ON THE FIRST SUNDAY. A region-model countdown is
        // painted by a branch with no fit loop around either of its lines, so the
        // label clips and the digits come out at a fraction of the size the
        // template asks for — 7.88px and 30.3px against 192px, measured at
        // 1920x1080. The layer model's `.cd-default` block declares its designed
        // size, clips, and reports through `onFit`. These four take that path
        // because they are seeded already converted; nothing else in either suite
        // says so, and a hand edit that dropped a `layers` array would put the blob
        // back silently.
        let want = [
            "Classic · Timer",
            "Aurora · Timer",
            "Ember · Timer",
            "High Visibility · Timer",
        ];
        for name in want {
            let (_, layout, _) = theme_templates()
                .iter()
                .find(|(n, _, _)| *n == name)
                .unwrap_or_else(|| panic!("no seeded {name:?}"));
            let v: serde_json::Value = serde_json::from_str(layout).unwrap();
            let layers = v["layers"]
                .as_array()
                .unwrap_or_else(|| panic!("{name}: no layer stack — it is region-model again"));
            assert!(!layers.is_empty(), "{name}: an empty layer stack");
            let mut ids: Vec<&str> = Vec::new();
            for l in layers {
                let id = l["id"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{name}: a layer with no id"));
                assert!(l["type"].is_string(), "{name}: a layer with no type");
                assert!(!ids.contains(&id), "{name}: two layers share the id {id:?}");
                ids.push(id);
            }
            assert!(
                layers.iter().any(|l| l["type"] == "background"),
                "{name}: no background layer, so the countdown would key over a camera"
            );
            // THE REGION KEYS STAY. `regionsToLayers` returns `{ ...layout, layers }`
            // and everything that reads the old shape still reads it — `templateKind`
            // derives `scripture` from these two regions, `templateShows` reads
            // `shows`, and `upgradeLegacyToLayers` looks for `regions` to decide
            // whether there is anything left to convert.
            assert!(
                v["regions"].is_array(),
                "{name}: the region keys were dropped, not added to"
            );
            assert_eq!(
                v["lowerThird"], false,
                "{name}: a full-screen timer is not keyed"
            );
        }
    }

    #[test]
    fn the_keyed_timer_is_deliberately_not_converted() {
        // THE ONE MEMBER OF THE FIVE THAT HAS NOTHING TO FIX. `countdownAllowed` is
        // `!!countdownTo && !bandMode`, so the region path paints NOTHING under a
        // countdown, and `.bandless` makes the band itself transparent when there
        // are no words: a clean camera, which is what a keyed screen should give an
        // operator who fires a countdown at it. The layer model has no equivalent
        // refusal — `.cd-default` does not ask about the band and a `shape` layer
        // paints whether or not it holds words — so converting this row for
        // symmetry would replace "nothing" with the digits or an empty bar, over a
        // live camera. Pinned so a later tidy-up does not convert the whole family
        // on the grounds that four of it are converted.
        let (_, layout, _) = theme_templates()
            .iter()
            .find(|(n, _, _)| *n == "Lower Third · Timer")
            .expect("no seeded Lower Third · Timer");
        let v: serde_json::Value = serde_json::from_str(layout).unwrap();
        assert!(
            v.get("layers").is_none(),
            "Lower Third · Timer was converted to layers — read the comment on BAND_TIMER first"
        );
        assert_eq!(v["lowerThird"], true);
    }

    #[test]
    fn every_seeded_template_says_which_kinds_it_renders() {
        // `templateShows` returns true for EVERY kind when `layout.shows` is
        // absent (src/lib/layers.js), so a seeded template with no list claims all
        // five implicitly and the editor's filter register has to materialise one
        // on the operator's first click — which is what made that click look as
        // though it had wiped four. The list is written down so the register
        // starts from a real one.
        //
        // AND IT IS ALL FIVE, EVERY TIME. This is the half worth asserting rather
        // than just the vocabulary. `layout.shows` is a per-SCREEN filter, not a
        // record of what a template was designed for: `Output.svelte` gates
        // incoming content on `templateShows(own_template, kind)` BEFORE
        // `resolveOutputTemplate` is consulted, so a narrow list on the template
        // an operator assigned to the Main screen makes that screen silently drop
        // every other kind — including when a content look correctly routes songs
        // to that family's Lyrics member, because the message never survives to
        // reach the override. A family whose Scripture member mutes songs is not
        // a family you can fire five kinds at over a morning, which is what the
        // test above says a family is for. What a template is FOR is recorded by
        // the `Used for` binding, a different register entirely.
        let all = ["scripture", "song", "media", "announce", "countdown"];
        for (name, layout, _) in theme_templates() {
            let v: serde_json::Value =
                serde_json::from_str(layout).unwrap_or_else(|e| panic!("{name}: {e}"));
            let shows = v["shows"]
                .as_array()
                .unwrap_or_else(|| panic!("{name} has no shows list"));
            assert!(
                !shows.is_empty(),
                "{name}: an empty list shows nothing at all"
            );
            for k in shows {
                let k = k.as_str().unwrap_or("");
                assert!(all.contains(&k), "{name}: {k:?} is not a content kind");
            }
            for want in all {
                assert!(
                    shows.iter().any(|k| k.as_str() == Some(want)),
                    "{name}: does not show {want:?} — a seeded template that mutes \
                     a kind mutes it on every screen it is assigned to, before any \
                     content look can override it"
                );
            }
        }
    }

    #[test]
    fn the_keyed_family_is_keyed_in_every_kind() {
        // The transparency law (src/lib/layers.js:262) needs keyed templates to
        // exist at all, and this is now the only family that supplies them.
        for (name, layout, _) in theme_templates() {
            if let Some(rest) = name.strip_prefix("Lower Third · ") {
                let v: serde_json::Value = serde_json::from_str(layout).unwrap();
                assert_eq!(
                    v["lowerThird"], true,
                    "Lower Third · {rest} is not keyed, so the family cannot caption a camera"
                );
            }
        }
    }

    #[test]
    fn every_preset_is_valid_json_with_a_readable_verse_colour() {
        // A preset ships to a wall as-is, so a malformed one is a black screen in
        // front of a congregation. Parse each, and require a light verse colour on
        // a dark field — the contrast a lit room needs (lower-third bands excepted,
        // where the text sits on a solid accent and is dark on purpose).
        for (name, layout, style) in region_presets() {
            let l: serde_json::Value =
                serde_json::from_str(layout).unwrap_or_else(|_| panic!("{name}: bad layout json"));
            let s: serde_json::Value =
                serde_json::from_str(style).unwrap_or_else(|_| panic!("{name}: bad style json"));
            assert!(s["background"].is_string(), "{name}: no background");
            assert!(s["verseColor"].is_string(), "{name}: no verseColor");
            // No preset uses an image (the model has no asset store to hold one).
            assert!(
                s.get("bgImage").is_none(),
                "{name}: presets must not embed images"
            );

            let is_band = l["lowerThird"].as_bool().unwrap_or(false);
            if !is_band {
                let vc = s["verseColor"].as_str().unwrap().to_lowercase();
                // A light text colour — cheap luminance proxy: starts high.
                let bright =
                    ["#f", "#e", "#ffffff"].iter().any(|p| vc.starts_with(p)) || vc == "#ffffff";
                assert!(
                    bright,
                    "{name}: verse colour {vc} is not light-on-dark readable"
                );
            }
        }
    }

    // ── THE SHELF (the prototype's own looks, layer model) ───────────────────

    /// A fresh install actually CONTAINS these looks, by name.
    ///
    /// Not a count: `preset_template_count()` is derived from `all_presets()`, so
    /// dropping the shelf out of that chain moves the expectation with it and
    /// every count-based test stays green over a shelf nobody ships. Watched to
    /// fail by removing the shelf from `all_presets`.
    #[test]
    fn a_fresh_database_ships_every_shelf_look() {
        let conn = fresh();
        ensure_preset_templates(&conn).unwrap();
        let mut stmt = conn
            .prepare("SELECT region_config_json FROM templates WHERE name = ?1")
            .unwrap();
        for (name, _, _) in shelf_templates() {
            let layout: String = stmt
                .query_row([name], |r| r.get(0))
                .unwrap_or_else(|_| panic!("a fresh install is missing {name:?}"));
            assert!(
                layout.contains("\"layers\""),
                "{name} was seeded without its layer stack"
            );
        }
    }

    /// The shelf is a DATA FILE, so the build has to be the thing that proves it
    /// parses. `shelf_templates()` deliberately degrades to an empty vec rather
    /// than panicking at database-open time, which means a malformed file is
    /// SILENT at runtime — this is the instrument that makes it loud instead.
    #[test]
    fn the_shelf_file_parses_and_every_entry_is_a_layer_stack() {
        let parsed: ShelfFile = serde_json::from_str(SHELF_JSON)
            .expect("src-tauri/data/shelf_templates.json must parse");
        assert!(
            !parsed.templates.is_empty(),
            "the shelf file parsed to nothing"
        );
        assert_eq!(
            shelf_templates().len(),
            parsed.templates.len(),
            "shelf_templates() swallowed an entry"
        );
        for e in &parsed.templates {
            let layers = e.layout["layers"].as_array().unwrap_or_else(|| {
                panic!("{}: no layout.layers — the shelf is layer-model", e.name)
            });
            assert!(!layers.is_empty(), "{}: an empty layer stack", e.name);
            for l in layers {
                assert!(l["id"].is_string(), "{}: a layer with no id", e.name);
                assert!(l["type"].is_string(), "{}: a layer with no type", e.name);
            }
        }
    }

    /// Every box is inside the frame. Geometry is percent of the 16:9 stage, and
    /// a layer that starts at 96 and is 10 wide is four points off the screen —
    /// arithmetic the renderer will happily obey and nobody will see until it is
    /// on a wall. Background layers are exempt (they ignore geometry).
    #[test]
    fn no_shelf_layer_hangs_off_the_frame() {
        let parsed: ShelfFile = serde_json::from_str(SHELF_JSON).unwrap();
        for e in &parsed.templates {
            for l in e.layout["layers"].as_array().unwrap() {
                if l["type"] == "background" {
                    continue;
                }
                let n = |k: &str| l[k].as_f64().unwrap_or(0.0);
                let (x, y, w, h) = (n("x"), n("y"), n("w"), n("h"));
                let who = format!("{} / {}", e.name, l["name"].as_str().unwrap_or("?"));
                assert!(x >= 0.0 && y >= 0.0, "{who}: negative origin");
                assert!(w > 0.0 && h > 0.0, "{who}: a zero-size box renders nothing");
                assert!(x + w <= 100.0001, "{who}: runs off the right edge");
                assert!(y + h <= 100.0001, "{who}: runs off the bottom edge");
            }
        }
    }

    /// A band's `members` must name layers that are actually in the template
    /// (DECISIONS §75). A dead id lays out perfectly and silently drops a line.
    #[test]
    fn every_band_on_the_shelf_names_words_that_exist() {
        let parsed: ShelfFile = serde_json::from_str(SHELF_JSON).unwrap();
        let mut bands = 0;
        for e in &parsed.templates {
            let layers = e.layout["layers"].as_array().unwrap();
            let ids: Vec<&str> = layers.iter().filter_map(|l| l["id"].as_str()).collect();
            for l in layers.iter().filter(|l| l["type"] == "band") {
                bands += 1;
                let members = l["members"]
                    .as_array()
                    .unwrap_or_else(|| panic!("{}: a band with no members list", e.name));
                assert!(!members.is_empty(), "{}: a band that holds nothing", e.name);
                for m in members {
                    let id = m.as_str().unwrap();
                    assert!(
                        ids.contains(&id),
                        "{}: band member {id} does not exist",
                        e.name
                    );
                }
            }
        }
        // ONE, not two. `Lower Third · Scripture` left this shelf when the five
        // families landed — the keyed family's member takes that exact name, and
        // a seed that inserts by name cannot hold both. `Lower Third · Lyric` is
        // what keeps a real band, with real declared members, in the seed and
        // therefore under this check; if it ever goes too, the layer model's
        // `members` contract (DECISIONS §75) has no shipped example left and this
        // assertion is the thing that says so rather than passing over an empty
        // set.
        assert!(bands >= 1, "the shelf lost its lower thirds");
    }

    /// A composite's fill must be a BUILT-IN (DECISIONS §74): a kiosk or OBS page
    /// has no database and resolves `templateRef` against the bundled list, so a
    /// region pointing anywhere else renders one thing on the wall and another in
    /// the stream. And a composite may not be another composite's fill.
    #[test]
    fn every_region_on_the_shelf_names_a_builtin_that_is_not_itself_a_composite() {
        let parsed: ShelfFile = serde_json::from_str(SHELF_JSON).unwrap();
        let n_builtins = builtin_templates().len() as i64;
        let mut regions = 0;
        for e in &parsed.templates {
            for l in e.layout["layers"].as_array().unwrap() {
                if l["type"] != "region" {
                    continue;
                }
                regions += 1;
                let r = l["templateRef"]
                    .as_i64()
                    .unwrap_or_else(|| panic!("{}: a region with no templateRef", e.name));
                assert!(
                    (1..=n_builtins).contains(&r),
                    "{}: templateRef {r} is not one of the {n_builtins} built-ins — \
                     a kiosk would render something else",
                    e.name
                );
            }
        }
        assert!(regions >= 2, "the shelf lost its SuperSource composites");
        // The built-ins are region-model and carry no `region` layer of their own,
        // so no composite on the shelf can nest. Asserted rather than assumed.
        for (name, layout, _) in builtin_templates() {
            assert!(
                !layout.contains(r#""region""#),
                "built-in {name} became a composite — a composite may not be a composite's fill"
            );
        }
    }

    #[test]
    fn preset_names_are_unique_and_do_not_collide_with_the_builtins() {
        // Added by name, so a collision would mean a preset is never inserted (or
        // an operator sees two identical names).
        use std::collections::HashSet;
        let mut seen: HashSet<&str> = builtin_templates().iter().map(|(n, _, _)| *n).collect();
        for (name, _, _) in all_presets() {
            assert!(
                seen.insert(name),
                "duplicate/colliding template name: {name}"
            );
        }
    }
    /// THE BAND THE SEEDS COULD NOT REACH.
    ///
    /// `ensure_preset_templates` inserts only when a name is ABSENT, so correcting
    /// the shipped accent reached a fresh install and no existing one. In the same
    /// wave the band became VISIBLE for the first time — `panelBg` had fallen
    /// through to `transparent` for every keyed template — so the repair would have
    /// put a lilac bar on every stream of every church already running Relay, in
    /// amethyst, which rule 18 reserves for REHEARSAL.
    ///
    /// Watched to fail by removing the call from `ensure_tables`.
    #[test]
    fn an_existing_lower_third_loses_the_rehearsal_colour_it_never_showed() {
        let conn = Connection::open_in_memory().expect("db");
        conn.execute_batch(
            "CREATE TABLE templates (id INTEGER PRIMARY KEY, name TEXT, region_config_json TEXT, style_json TEXT);
             INSERT INTO templates (name, region_config_json, style_json) VALUES
               ('Lower Third', '{}', '{\"accent\":\"#b080e0\",\"verseColor\":\"#1c1224\"}'),
               ('Mine',        '{}', '{\"accent\":\"#00ff88\"}');",
        )
        .expect("seed");

        ensure_lower_third_band_is_not_a_law_colour(&conn).expect("fill");

        let lt: String = conn
            .query_row(
                "SELECT style_json FROM templates WHERE name='Lower Third'",
                [],
                |r| r.get(0),
            )
            .expect("read");
        assert!(
            !lt.contains("#b080e0"),
            "the rehearsal colour must be gone: {lt}"
        );
        assert!(
            lt.contains("#101319"),
            "and replaced by the neutral band: {lt}"
        );
        assert!(
            lt.contains("#f2f4f8"),
            "dark type on a near-black band is invisible — it must move too: {lt}"
        );

        // A church that chose its own colour is NOT touched.
        let mine: String = conn
            .query_row(
                "SELECT style_json FROM templates WHERE name='Mine'",
                [],
                |r| r.get(0),
            )
            .expect("read");
        assert!(
            mine.contains("#00ff88"),
            "a deliberate choice must survive: {mine}"
        );

        // Idempotent (rule 25): a second run matches nothing and changes nothing.
        ensure_lower_third_band_is_not_a_law_colour(&conn).expect("again");
        let twice: String = conn
            .query_row(
                "SELECT style_json FROM templates WHERE name='Lower Third'",
                [],
                |r| r.get(0),
            )
            .expect("read");
        assert_eq!(lt, twice, "running it twice must be a no-op");
    }
}

#[cfg(test)]
mod theme_inlining_tests {
    use super::*;
    use crate::db::settings::set_setting;

    #[test]
    fn a_themed_template_keeps_exactly_the_look_it_had() {
        // `{ ...theme.style, ...template.style }` is what `applyTheme` computed at
        // render time, so inlining it is the effective look BY CONSTRUCTION — the
        // template's own keys still win, and only the keys it left unset are filled.
        // Anything else here is a church's wall changing appearance on an update,
        // with nothing in the building able to say why.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (
                "Themed",
                r#"{"regions":["verse_text","reference"]}"#,
                // Pins Modern Dark (-1) and overrides ONE of its keys.
                r##"{"themeRef":-1,"verseColor":"#ff0000"}"##,
            ),
        )
        .unwrap();
        ensure_themes_are_inlined(&conn).unwrap();
        let style: String = conn
            .query_row(
                "SELECT style_json FROM templates WHERE name = 'Themed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&style).unwrap();
        assert!(v.get("themeRef").is_none(), "the ref must be gone");
        assert_eq!(
            v["verseColor"], "#ff0000",
            "the template's own key still wins"
        );
        assert_eq!(v["accent"], "#22d3ee", "the theme's unset keys are inlined");
    }

    #[test]
    fn an_unknown_theme_ref_is_dropped_without_touching_the_style() {
        // A dangling ref already rendered as the template's own look (`resolveThemed`
        // degrades rather than blanking). The migration must agree with what the
        // wall was doing, not invent a look for it.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (
                "Dangling",
                r#"{"regions":["verse_text"]}"#,
                r##"{"themeRef":123456,"verseColor":"#abc"}"##,
            ),
        )
        .unwrap();
        ensure_themes_are_inlined(&conn).unwrap();
        let style: String = conn
            .query_row(
                "SELECT style_json FROM templates WHERE name = 'Dangling'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&style).unwrap();
        assert!(v.get("themeRef").is_none());
        assert_eq!(v["verseColor"], "#abc");
        assert_eq!(v.as_object().unwrap().len(), 1, "no keys invented");
    }

    #[test]
    fn a_custom_theme_nothing_referenced_becomes_a_template_rather_than_being_lost() {
        // A custom theme's ONLY effect anywhere is through a template's themeRef
        // (there is no active-theme concept). One that nothing references therefore
        // changed no screen — but it is still a look the operator built, and the
        // key it lives in is about to be deleted. It becomes one real template,
        // named after the theme.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        set_setting(
            &conn,
            "themes.custom",
            r##"[{"id":5,"name":"Harvest","style":{"accent":"#e08b2a","verseColor":"#fff5e6"}}]"##,
        )
        .unwrap();
        ensure_themes_are_inlined(&conn).unwrap();
        let style: String = conn
            .query_row(
                "SELECT style_json FROM templates WHERE name = 'Harvest'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(style.contains("#e08b2a"));
        assert!(
            get_setting(&conn, "themes.custom").unwrap().is_none(),
            "the key is dropped once its contents are preserved"
        );
    }

    #[test]
    fn inlining_is_retryable_and_idempotent() {
        // Rule 25. Run it three times: the second and third must be no-ops, not
        // errors, and must not stack a second copy of the theme's keys or a second
        // template per custom theme. A migration that fails every boot after a
        // half-run is a church whose app will not start.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (
                "Themed",
                r#"{"regions":["verse_text"]}"#,
                r#"{"themeRef":-1}"#,
            ),
        )
        .unwrap();
        ensure_themes_are_inlined(&conn).unwrap();
        let after_one: String = conn
            .query_row(
                "SELECT style_json FROM templates WHERE name = 'Themed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        ensure_themes_are_inlined(&conn).unwrap();
        ensure_themes_are_inlined(&conn).unwrap();
        let after_three: String = conn
            .query_row(
                "SELECT style_json FROM templates WHERE name = 'Themed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after_one, after_three);
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM templates WHERE name = 'Themed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }
    #[test]
    fn a_theme_key_the_wall_never_received_is_not_inlined() {
        // `applyTheme` copied only `THEME_STYLE_KEYS` off a theme, so a key
        // outside that list was dropped on the way to the screen: the theme
        // editor could show it and the wall never painted it (`templatedoors
        // .test.js`, "the theme door"). Inlining one would be a value appearing
        // on a congregation screen for the first time, on an update, because of
        // a migration whose entire promise is that nothing changes. The nine
        // builtins are all inside the list; an imported custom theme need not be.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        set_setting(
            &conn,
            "themes.custom",
            r##"[{"id":7,"name":"Imported","style":{"accent":"#0f0","scroll":true}}]"##,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (
                "Ticker",
                r##"{"regions":["verse_text"]}"##,
                r##"{"themeRef":7}"##,
            ),
        )
        .unwrap();
        ensure_themes_are_inlined(&conn).unwrap();
        let style: String = conn
            .query_row(
                "SELECT style_json FROM templates WHERE name = 'Ticker'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&style).unwrap();
        assert_eq!(v["accent"], "#0f0", "a whitelisted key is inlined");
        assert!(
            v.get("scroll").is_none(),
            "a key applyTheme filtered out must not reach the wall now: {style}"
        );
    }

    #[test]
    fn a_custom_theme_a_template_used_is_not_also_kept_as_a_second_template() {
        // The two halves are one transaction for this reason among others: a
        // theme preserved INSIDE the template that used it must not also appear
        // beside it as a duplicate look the operator never made.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        set_setting(
            &conn,
            "themes.custom",
            r##"[{"id":9,"name":"Harvest","style":{"accent":"#e08b2a"}}]"##,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (
                "Autumn",
                r##"{"regions":["verse_text"]}"##,
                r##"{"themeRef":9}"##,
            ),
        )
        .unwrap();
        ensure_themes_are_inlined(&conn).unwrap();
        let used: String = conn
            .query_row(
                "SELECT style_json FROM templates WHERE name = 'Autumn'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(used.contains("#e08b2a"), "the look was not inlined: {used}");
        let orphan: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM templates WHERE name = 'Harvest'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphan, 0, "a referenced theme was duplicated as a template");
    }
    #[test]
    fn a_snapshot_that_will_not_parse_leaves_every_theme_exactly_where_it_was() {
        // FINDING 1. The old bail-out carried on with no known themes: it dropped
        // every ref, inlined nothing, and then deleted `themes.custom`. That is
        // the operator's look changed AND the only copy of it erased, on a boot
        // where the one thing Relay knew was that it could not read its own
        // snapshot. It must touch nothing: a ref left in a style renders exactly
        // as a dropped one would, and a release can fix the file later.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        let blob = r##"[{"id":5,"name":"Harvest","style":{"accent":"#e08b2a"}}]"##;
        set_setting(&conn, "themes.custom", blob).unwrap();
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (
                "Themed",
                r##"{"regions":["verse_text"]}"##,
                r##"{"themeRef":5}"##,
            ),
        )
        .unwrap();

        inline_themes(&conn, "{ this is not the snapshot }").unwrap();

        let style: String = conn
            .query_row(
                "SELECT style_json FROM templates WHERE name = 'Themed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(style.contains("themeRef"), "the ref was dropped: {style}");
        assert_eq!(
            get_setting(&conn, "themes.custom").unwrap().as_deref(),
            Some(blob),
            "the operator's themes were deleted by a run that preserved nothing"
        );
        let orphan: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM templates WHERE name = 'Harvest'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphan, 0, "nothing should have been written at all");
    }

    #[test]
    fn the_shipped_snapshot_parses_so_that_branch_is_never_taken_in_a_real_build() {
        // The bail-out above is for a broken build, not for a normal one. This is
        // the Rust half of the guard; `legacythemes.test.js` reads the same bytes.
        serde_json::from_str::<FrozenThemes>(LEGACY_THEMES_JSON)
            .expect("the shipped snapshot must parse");
    }

    #[test]
    fn a_theme_that_ended_up_contributing_no_key_is_still_preserved() {
        // FINDING 2. `inlined` used to mean "the id matched". A theme whose keys
        // are all off the whitelist matches, contributes nothing, and was then
        // skipped by the preservation loop and deleted: the look was erased on
        // the strength of a match that saved none of it.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        set_setting(
            &conn,
            "themes.custom",
            r##"[{"id":5,"name":"Harvest","style":{"scroll":true}}]"##,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (
                "Themed",
                r##"{"regions":["verse_text"]}"##,
                r##"{"themeRef":5}"##,
            ),
        )
        .unwrap();
        ensure_themes_are_inlined(&conn).unwrap();
        let kept: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM templates WHERE name = 'Harvest'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kept, 1, "a theme that saved nothing anywhere was deleted");
    }

    #[test]
    fn a_taken_name_is_renamed_rather_than_the_look_being_thrown_away() {
        // FINDING 3. Two themes called "Harvest", and the second used to be
        // skipped and then deleted with the key: silent, irreversible, and a look
        // the operator built. Both survive, under names that are free.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        set_setting(
            &conn,
            "themes.custom",
            r##"[{"id":5,"name":"Harvest","style":{"accent":"#111111"}},
                 {"id":6,"name":"Harvest","style":{"accent":"#222222"}},
                 {"id":7,"name":"Harvest","style":{"accent":"#333333"}}]"##,
        )
        .unwrap();
        ensure_themes_are_inlined(&conn).unwrap();
        for (name, colour) in [
            ("Harvest", "#111111"),
            ("Harvest (theme)", "#222222"),
            ("Harvest (theme 2)", "#333333"),
        ] {
            let style: String = conn
                .query_row(
                    "SELECT style_json FROM templates WHERE name = ?1",
                    [name],
                    |r| r.get(0),
                )
                .unwrap_or_else(|e| panic!("{name} was not kept: {e}"));
            assert!(
                style.contains(colour),
                "{name} holds the wrong look: {style}"
            );
        }
    }

    #[test]
    fn a_failure_part_way_through_rolls_back_and_the_next_boot_completes_it() {
        // FINDING 4, and rule 25 proper. Running it three times cleanly proves
        // idempotency, not retryability. This kills it MID-RUN: TWO themed
        // templates, and a trigger that lets the first through and aborts the
        // second. Without one transaction around the whole thing the first
        // rewrite stands, committed, while `themes.custom` still holds the theme
        // it came from, and a retry landing there duplicates every custom theme.
        // Modelled on `db::tests::foreign_keys_are_on_again_afterwards_and_no_
        // transaction_is_left_open`.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        let blob = r##"[{"id":5,"name":"Harvest","style":{"accent":"#e08b2a"}}]"##;
        set_setting(&conn, "themes.custom", blob).unwrap();
        for name in ["First", "Second"] {
            conn.execute(
                "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
                (
                    name,
                    r##"{"regions":["verse_text"]}"##,
                    r##"{"themeRef":-1}"##,
                ),
            )
            .unwrap();
        }
        conn.execute_batch(
            "CREATE TRIGGER boom BEFORE UPDATE ON templates WHEN NEW.name = 'Second'
             BEGIN SELECT RAISE(ABORT, 'boom'); END;",
        )
        .unwrap();

        assert!(
            ensure_themes_are_inlined(&conn).is_err(),
            "the failure must be reported, not swallowed"
        );
        assert!(
            conn.is_autocommit(),
            "a transaction was left open; the PRAGMA that follows would no-op inside it"
        );
        let read = |name: &str| -> String {
            conn.query_row(
                "SELECT style_json FROM templates WHERE name = ?1",
                [name],
                |r| r.get(0),
            )
            .unwrap()
        };
        // THE ONE THAT SUCCEEDED MUST HAVE BEEN UNDONE TOO.
        assert!(
            read("First").contains("themeRef"),
            "a half-run was committed: {}",
            read("First")
        );
        assert!(read("Second").contains("themeRef"));
        assert_eq!(
            get_setting(&conn, "themes.custom").unwrap().as_deref(),
            Some(blob),
            "the themes were deleted by a run that rolled back"
        );

        // The next boot, with whatever broke it gone.
        conn.execute_batch("DROP TRIGGER boom;").unwrap();
        ensure_themes_are_inlined(&conn).expect("the retry must complete");
        for name in ["First", "Second"] {
            let v: serde_json::Value = serde_json::from_str(&read(name)).unwrap();
            assert!(v.get("themeRef").is_none(), "{name} kept its ref");
            assert_eq!(v["accent"], "#22d3ee", "the retry inlined the wrong look");
        }
        assert!(get_setting(&conn, "themes.custom").unwrap().is_none());
        // And exactly one copy of the preserved theme, not one per retry.
        let harvest: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM templates WHERE name = 'Harvest'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(harvest, 1);
    }
    #[test]
    fn an_unreadable_themes_blob_is_left_alone_rather_than_destroyed() {
        // The DELETE used to be gated on the key being PRESENT. A blob that will
        // not parse preserves nothing and is then the only copy of whatever the
        // operator saved, so deleting it is the one irreversible thing this
        // migration could do to data it never even read.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        let junk = "[{\"id\":5, truncated";
        set_setting(&conn, "themes.custom", junk).unwrap();
        ensure_themes_are_inlined(&conn).unwrap();
        assert_eq!(
            get_setting(&conn, "themes.custom").unwrap().as_deref(),
            Some(junk),
            "an unreadable blob was destroyed"
        );
    }
}

#[cfg(test)]
mod retired_preset_tests {
    use super::*;
    use crate::db::settings::{set_content_template, set_setting};

    /// Insert a retired preset exactly as an older version seeded it, read from
    /// the frozen record so the test cannot drift from what the migration matches.
    fn insert_retired_fixture(conn: &Connection, name: &str) -> i64 {
        let (n, l, s) = retired_presets()
            .into_iter()
            .find(|(n, _, _)| n == name)
            .unwrap_or_else(|| panic!("{name} is not in retired_presets.json"));
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (n, l, s),
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn count_named(conn: &Connection, name: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM templates WHERE name = ?1",
            [name],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn the_frozen_record_parses_and_names_nothing_the_seed_still_ships() {
        // TWO ways this migration becomes silently wrong, neither of which any
        // other test in here can see.
        //
        // A record that stops parsing makes the whole thing a permanent no-op:
        // it retires nothing, reports nothing to anybody, and every install keeps
        // a doubled gallery. `retired_presets` swallows that deliberately (losing
        // a template is worse than keeping one), so the parse has to be pinned
        // where a broken build fails, the way
        // `the_shipped_snapshot_parses_so_that_branch_is_never_taken_in_a_real_build`
        // pins the themes snapshot.
        let frozen = retired_presets();
        assert_eq!(
            frozen.len(),
            21,
            "the frozen record did not parse, or it stopped holding the twenty-one rows this wave retired"
        );

        // And the opposite mistake, which is worse: a frozen TRIPLE that the seed
        // still ships. The row would be deleted here and re-inserted a line later
        // in `ensure_tables` under a NEW id, on every single boot, for the life of
        // the install. A frozen NAME the seed still ships is fine and deliberate:
        // `Lower Third · Scripture` is exactly that, the shelf row's name taken
        // over by a family member whose bytes differ, so this compares the whole
        // triple, not the name.
        for (name, layout, style) in &frozen {
            let clash = all_presets()
                .any(|(n, l, s)| n == name.as_str() && l == layout.as_str() && s == style.as_str());
            assert!(
                !clash,
                "{name} is frozen as retired AND still seeded, byte for byte: it would be deleted and re-created under a new id on every boot"
            );
        }
    }

    #[test]
    fn an_untouched_unreferenced_preset_is_retired() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        // A row exactly as an older version seeded it.
        insert_retired_fixture(&conn, "Midnight Blue");
        ensure_retired_presets_are_gone(&conn).unwrap();
        assert_eq!(count_named(&conn, "Midnight Blue"), 0);
    }

    #[test]
    fn a_preset_the_operator_edited_is_never_retired() {
        // No `updated_at` column exists, so "edited" is decided by comparing the
        // bytes with what the seed shipped. A single changed colour makes the row
        // somebody's work, and somebody's work is not a leftover.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        insert_retired_fixture(&conn, "Midnight Blue");
        let touched = conn
            .execute(
                "UPDATE templates SET style_json = replace(style_json, '#f0b74a', '#00ff99')
                  WHERE name = 'Midnight Blue' AND style_json LIKE '%#f0b74a%'",
                [],
            )
            .unwrap();
        assert_eq!(
            touched, 1,
            "the fixture no longer carries the colour this edit changes"
        );
        ensure_retired_presets_are_gone(&conn).unwrap();
        assert_eq!(
            count_named(&conn, "Midnight Blue"),
            1,
            "an edited preset is the operator's, not a leftover"
        );
    }

    #[test]
    fn a_referenced_preset_is_never_retired() {
        // Four doors point at a template: a channel, a plan cue, a content look and
        // the configured default. Checking three of them is this repository's
        // recurring bug, a guarantee kept on one surface and skipped on its twin,
        // so each is asserted separately rather than in one case that could pass on
        // the first door alone.
        //
        // The look door is FIVE cases, not one, for the same reason one level
        // down: the kinds are a hardcoded array in the migration, so a single
        // `scripture` case leaves the other four untested and a sixth kind added
        // to `CONTENT_KINDS` later would escape the door in silence. Deleting any
        // one kind from that array now reddens the case named after it.
        for door in [
            "channel",
            "cue",
            "look:scripture",
            "look:song",
            "look:media",
            "look:announce",
            "look:countdown",
            "default",
        ] {
            let conn = Connection::open_in_memory().unwrap();
            crate::db::migrate(&conn, true).unwrap();
            let id = insert_retired_fixture(&conn, "Midnight Blue");
            match door {
                "channel" => {
                    conn.execute(
                        "INSERT INTO output_channels (name, render_target, template_id)
                         VALUES ('Main','native_window',?1)",
                        [id],
                    )
                    .unwrap();
                }
                "cue" => {
                    conn.execute("INSERT INTO service_plans (title) VALUES ('Sunday')", [])
                        .unwrap();
                    let plan = conn.last_insert_rowid();
                    conn.execute(
                        "INSERT INTO plan_items (plan_id, position, cue_type, label, template_id)
                         VALUES (?1,0,'scripture','Reading',?2)",
                        [plan, id],
                    )
                    .unwrap();
                }
                _ if door.starts_with("look:") => {
                    set_content_template(&conn, &door["look:".len()..], Some(id)).unwrap()
                }
                _ => set_setting(&conn, "default_template_id", &id.to_string()).unwrap(),
            }
            ensure_retired_presets_are_gone(&conn).unwrap();
            let n: i64 = conn
                .query_row("SELECT COUNT(*) FROM templates WHERE id = ?1", [id], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(n, 1, "a preset reachable through the {door} was retired");
        }
    }

    #[test]
    fn retiring_is_retryable_and_a_fresh_install_is_unaffected() {
        // Rule 25, and the fresh-install case: a first launch has never seeded the
        // retired names, so this must find nothing and change nothing, three times
        // over. It is also the one test that would catch the worst possible
        // version of this migration: one whose frozen bytes match a row the seed
        // still ships, deleting a family member on every boot.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        ensure_retired_presets_are_gone(&conn).unwrap();
        ensure_retired_presets_are_gone(&conn).unwrap();
        ensure_retired_presets_are_gone(&conn).unwrap();
        let after: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))
            .unwrap();
        assert_eq!(before, after);
    }

    /// The single name that is on BOTH the retired list and the seed list. There is
    /// exactly one; `the_frozen_record_parses_and_names_nothing_the_seed_still_ships`
    /// asserts the whole-triple disjointness that makes it so.
    fn the_contested_name() -> (String, String, String) {
        let mut both: Vec<(String, String, String)> = retired_presets()
            .into_iter()
            .filter(|(n, _, _)| all_presets().any(|(sn, _, _)| sn == n.as_str()))
            .collect();
        assert_eq!(
            both.len(),
            1,
            "exactly one name is on both lists; if that changed, so has the premise of these tests"
        );
        both.remove(0)
    }

    #[test]
    fn the_contested_name_reports_nothing_once_the_retired_row_is_actually_gone() {
        // RULE 35, IN A LOG. `ensure_retired_presets_are_gone` prints one line for the
        // single name on both lists — `Lower Third · Scripture` — when the retired row
        // is KEPT and the seeded family member of that name therefore cannot be
        // installed. That line is the only diagnostic for the one permanent
        // consequence of this wave, so it has to be able to be silent.
        //
        // It could not. The count asked "is a row of this name present", and on a
        // healthy install the answer is yes from the second boot onwards: boot 1
        // deletes the retired row and `ensure_preset_templates` inserts the family's
        // own member of that name a line later, so the name is present for ever after
        // and the message printed over an install where the template demonstrably IS
        // installed. The question it means to ask is "is the RETIRED row still there",
        // which is a name AND BYTES question — the one the DELETE above it already
        // asks.
        //
        // Asserted as the predicate rather than as captured stderr: the old count
        // returns 1 here and the new one returns 0.
        let (name, layout, style) = the_contested_name();

        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        // An install that still carries the old row, as an older version seeded it.
        insert_retired_fixture(&conn, &name);
        assert_eq!(count_named(&conn, &name), 2, "the fixture did not land");

        ensure_retired_presets_are_gone(&conn).unwrap();
        ensure_preset_templates(&conn).unwrap();

        // The family member of that name is installed — which is exactly what made the
        // old count non-zero, and the sentence it guards false.
        assert_eq!(
            count_named(&conn, &name),
            1,
            "the seeded family member of the contested name is missing"
        );
        assert!(
            !retired_row_is_still_present(&conn, &name, &layout, &style).unwrap(),
            "the retired row is gone, so nothing may be reported as kept"
        );
    }

    #[test]
    fn the_contested_name_still_reports_when_the_retired_row_really_is_kept() {
        // The other half: silence has to mean something. A retired row the migration
        // may not touch — here, the operator's configured default — is the case the
        // line exists to announce, and the narrower count must still find it.
        let (name, layout, style) = the_contested_name();

        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        let id = insert_retired_fixture(&conn, &name);
        set_setting(&conn, "default_template_id", &id.to_string()).unwrap();

        ensure_retired_presets_are_gone(&conn).unwrap();
        ensure_preset_templates(&conn).unwrap();

        assert!(
            retired_row_is_still_present(&conn, &name, &layout, &style).unwrap(),
            "the retired row is the operator's default: it must be kept, and reported"
        );
    }

    #[test]
    fn a_database_that_predates_the_planner_is_still_retired_from() {
        // This migration has to run BEFORE the seed (one retired name is taken by
        // a new family member), and the seed runs well before `ensure_service_plans`
        // creates `plan_items`. A guard naming a table that is not there yet is
        // `no such table: plan_items`, propagated out of `migrate`, at every boot,
        // before the window is shown: the shape of rule 25's original failure.
        //
        // THE DATABASE THAT REACHES IT IS A BASELINE-ERA ONE, and the route was
        // checked rather than assumed. `docs/data/schema-baseline.sql` is the
        // oldest schema Relay can upgrade FROM, and it creates eight tables, of
        // which `templates` and `output_channels` are two and `plan_items` is not
        // one (nor is `app_settings`). `migrate` sends a `user_version == 0`
        // database through `baseline_forward_fill`, which calls `ensure_tables`,
        // which calls this. An earlier version of this comment credited
        // `db::tests::migrates_pre_console_active_db` with covering that path; it
        // does not call `migrate` at all, it calls `ensure_template_active` twice,
        // so it would have stayed green over the bug. A citation that resolves to
        // nothing is worse than an uncited claim.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE templates (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL,
                region_config_json TEXT NOT NULL, style_json TEXT NOT NULL
             );
             CREATE TABLE output_channels (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL, render_target TEXT NOT NULL,
                template_id INTEGER REFERENCES templates(id)
             );",
        )
        .unwrap();
        insert_retired_fixture(&conn, "Midnight Blue");
        ensure_retired_presets_are_gone(&conn)
            .expect("a database with no plan_items table must still boot");
        assert_eq!(count_named(&conn, "Midnight Blue"), 0);

        // And the door that IS there is still honoured on that same database.
        let id = insert_retired_fixture(&conn, "Deep Teal");
        conn.execute(
            "INSERT INTO output_channels (name, render_target, template_id)
             VALUES ('Main','native_window',?1)",
            [id],
        )
        .unwrap();
        ensure_retired_presets_are_gone(&conn).unwrap();
        assert_eq!(
            count_named(&conn, "Deep Teal"),
            1,
            "the channel door was dropped along with the absent one"
        );
    }

    #[test]
    fn a_failure_mid_run_leaves_every_retired_row_where_it_was() {
        // Rule 25 proper. Running it three times cleanly proves idempotency, not
        // retryability. This kills it MID-RUN: two retirable rows, and a trigger
        // that lets the first through and aborts the second. Without one
        // transaction around the whole loop the first DELETE stands, committed,
        // and the error still propagates out of `migrate`, which on this ladder
        // means the app panics at startup with a transaction left open for the
        // `PRAGMA foreign_keys = ON` that follows to no-op inside (rule 25's
        // original failure, one migration along). Modelled on
        // `theme_inlining_tests::a_failure_part_way_through_rolls_back_and_the_next_boot_completes_it`.
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        insert_retired_fixture(&conn, "Midnight Blue");
        insert_retired_fixture(&conn, "Royal Amethyst");
        // A third that is spoken for, to prove the references themselves survive
        // a rollback as well as the rows.
        let kept = insert_retired_fixture(&conn, "Deep Teal");
        conn.execute(
            "INSERT INTO output_channels (name, render_target, template_id)
             VALUES ('Main','native_window',?1)",
            [kept],
        )
        .unwrap();

        conn.execute_batch(
            "CREATE TRIGGER boom BEFORE DELETE ON templates WHEN OLD.name = 'Royal Amethyst'
             BEGIN SELECT RAISE(ABORT, 'boom'); END;",
        )
        .unwrap();

        assert!(
            ensure_retired_presets_are_gone(&conn).is_err(),
            "the failure must be reported, not swallowed"
        );
        assert!(
            conn.is_autocommit(),
            "a transaction was left open; the PRAGMA that follows would no-op inside it"
        );
        // THE ONE THAT SUCCEEDED MUST HAVE BEEN UNDONE TOO.
        assert_eq!(
            count_named(&conn, "Midnight Blue"),
            1,
            "a half-run was committed: the first row is gone and the second is not"
        );
        assert_eq!(count_named(&conn, "Royal Amethyst"), 1);
        assert_eq!(count_named(&conn, "Deep Teal"), 1);
        let still_pointed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM output_channels WHERE template_id = ?1",
                [kept],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(still_pointed, 1, "the channel lost the template it named");

        // The next boot, with whatever broke it gone.
        conn.execute_batch("DROP TRIGGER boom;").unwrap();
        ensure_retired_presets_are_gone(&conn).expect("the retry must complete");
        assert_eq!(count_named(&conn, "Midnight Blue"), 0);
        assert_eq!(count_named(&conn, "Royal Amethyst"), 0);
        assert_eq!(
            count_named(&conn, "Deep Teal"),
            1,
            "the referenced row was retired on the retry"
        );
    }
}
