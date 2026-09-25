//! Verse/content detection: direct match, semantic match, context memory.
//!
//! Single responsibility: given a rolling transcript window, return zero or
//! more candidate verse detections with a confidence score and method
//! ("direct" or "semantic"). Does NOT decide what to do with a detection —
//! that's router.rs. Does NOT touch SQLite — it returns references; the caller
//! resolves them to a verse_id via db.rs. Kept DB- and IO-free so the whole
//! parser is unit-testable. See docs/SPEC.md §4 and PROMPT.md Phase 5/9.
//!
//! Phase 5: direct pattern match — spoken and written references
//! (`John 3:16`, `John three sixteen`, `Romans chapter eight verse twenty-eight`)
//! against a multilingual-ready book-alias table, with tolerance for common
//! ASR homophones ("free" → three). Semantic match + context memory are Phase 9.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// How a candidate was detected. This is NOT cosmetic metadata — the router
/// gates on it (see `router::Router::decide`), because confidences from these
/// three sources live on *incomparable scales* and a single scalar threshold
/// cannot safely gate all of them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionMethod {
    /// A spoken reference the parser actually heard ("John three sixteen").
    /// Confidence is a real parse confidence. May auto-fire.
    Direct,
    /// A TF-IDF paraphrase match. Its "confidence" is a raw cosine similarity —
    /// a distance in an arbitrary vector space, NOT a probability. May never
    /// auto-fire; see `router.rs`.
    Semantic,
    /// A CONTIGUOUS RUN OF WORDS the speaker said that is verbatim in one verse.
    ///
    /// Its "confidence" is derived from the run LENGTH, which is a count and not
    /// a probability either — so, like `Semantic`, no threshold on it means
    /// anything and it may never auto-fire. What separates it from `Semantic` is
    /// what it can show: a phrase an operator can read and agree with in the
    /// second they have, rather than a list of words that appear in the verse
    /// somewhere. See `PhraseIndex`.
    ///
    /// It is NOT `Direct` and the distinction is the whole of rule 10. Relay
    /// heard scripture being READ; it did not hear a reference. Quoting a verse
    /// is not asking for it to go on a wall, and a preacher quotes far more
    /// verses than a congregation is shown.
    Quoted,
    /// A reference that parsed but is genuinely ambiguous ("Revelation 22"
    /// → 22:1 or 2:2). Confidence is a hardcoded placeholder, not measured.
    /// May never auto-fire.
    Ambiguous,
    /// A reference whose BOOK NAME was guessed by `fuzzy_book`'s edit-distance
    /// repair rather than heard. May never auto-fire.
    ///
    /// ── Why this variant exists (the P0 of 2026-08-14) ──────────────────────
    ///
    /// `fuzzy_book` repairs a misheard book token against the alias table, gated
    /// on "the next token is a number". That gate is exactly the shape of a church
    /// announcement, and one edit from a three-letter typing shortcut is an
    /// enormous set of ordinary English words. Measured through the real router at
    /// the shipped default: "please turn to hymn **number** three sixteen" put
    /// **Numbers 3:16** on the wall at 0.840 against a 0.50 bar; "the youth meet in
    /// **room** two twelve" put up Romans 2:12; `row`, `van`, `day` and 34 more did
    /// the same. Nobody pressed anything.
    ///
    /// The mitigation the code claimed — "still marked FUZZY, which costs
    /// confidence downstream" — was worth 0.06 against a 0.34 margin. A contract
    /// stated in a comment is not a contract, and the repair is here restated as
    /// one the router can enforce.
    ///
    /// The principle is the one that already governs `Semantic`: **only what was
    /// actually heard may reach a congregation unattended.** A Levenshtein repair
    /// is not evidence about what was said, it is a guess about it — so it goes to
    /// the operator, who accepts it in one press if it is right. Note the deliberate
    /// narrowness: this covers a repaired **book name** only. A repaired *number*
    /// word ("john free sixteen" → John 3:16) still auto-fires, because the book
    /// was heard exactly and only the digit was in doubt.
    #[serde(rename = "uncertain_book")]
    UncertainBook,

    /// The book was heard; the NUMBERS were inferred, repaired, or did not line up.
    ///
    /// ── Why this variant exists (R4-01 · R4-02 · R4-03) ─────────────────────
    ///
    /// `UncertainBook` closed the case where Relay guessed the word. This closes
    /// the case where it guessed the digits — and the two failed for exactly the
    /// same reason: the confidence was a real parse confidence, honest about the
    /// parse, silent about whether the thing parsed was ever said.
    ///
    /// Four shapes reach it, all of them ordinary preaching:
    ///
    /// * **A run the book cannot support, split.** `split_run_into_chapter_verse`
    ///   exists because whisper writes "six sixty three" as `663` — but nothing
    ///   required the sentence to be reference-shaped, and the repair was handed
    ///   0.83 where the keyword-less reading it replaced is deliberately demoted
    ///   to 0.45. So a number the parser could NOT read as a chapter made Relay
    ///   MORE confident: *"Nehemiah, thirteen days they built the wall"* asked a
    ///   human at 0.45, and *"Nehemiah, fifty two days they built the wall"* put
    ///   **Nehemiah 5:2** on the wall at 0.77.
    /// * **A leftover number.** "Psalms 2, 3, 1" parses 2:3 and strands a "1" no
    ///   range could absorb — the shape of garbled speech, and already demoted
    ///   when whisper wrote digits. It writes WORDS on a large share of accented
    ///   decodes, which is this product's entire market, and the identical garble
    ///   spelled out scored 0.90 and went straight to the congregation.
    /// * **A whole chapter nobody named as one.** "Matthew, one of the twelve
    ///   disciples" → Matthew 1:1. The verse was supplied by Relay, not heard.
    /// * **A single-chapter book's lone number.** "jude four men came in" →
    ///   Jude 1:4. The chapter was supplied.
    ///
    /// Each of those was already demoted **as a score** — 0.45, against a 0.50
    /// default bar. That margin is 0.05 wide and the operator's own sensitivity
    /// dial closes it: `from_sensitivity(100)` returns an auto-fire bar of 0.30,
    /// which is the confidence FLOOR, so at the top of the dial no direct match
    /// could ever be a suggestion and every deliberate demotion in this file was
    /// inert. **A demotion expressed as a number is a demotion a dial can erase.**
    /// Expressed as a method, the router refuses it at any score and any dial —
    /// the same repair `UncertainBook` got, for the same reason.
    ///
    /// Deliberately NOT here: a bare pair that ends cleanly ("psalm 23 1"), and a
    /// number word the FSM repaired inside a complete reference ("john free
    /// sixteen"). Both numbers were heard in those; only the rendering was in
    /// doubt.
    #[serde(rename = "uncertain_number")]
    UncertainNumber,

    /// THE PREACHER IS READING THIS VERSE ALOUD, and Relay heard enough of it to
    /// say which verse without choosing between two.
    ///
    /// ── Why this reverses the cap `Quoted` carries (DECISIONS §118) ─────────
    ///
    /// `Quoted`'s doc comment argues the other way — *"quoting a verse is not
    /// asking for it to go on a wall, and a preacher quotes far more verses than
    /// a congregation is shown"* — and that argument was accepted until the
    /// operator overruled it on 2026-09-23, twice and in writing: *"when a
    /// scripture is quoted make sure to fire it to screen as its confirmed"*,
    /// and *"follow the verse whenever a preacher is reading a bible verse, you
    /// dont need to wait or suggest it if the reader is reading the verse."*
    /// That is theirs to decide. What is not theirs to decide is how much
    /// evidence counts as hearing, and that is what this variant is.
    ///
    /// **It is not rule 10 in a new costume, and the difference is the whole
    /// point.** Rule 10 is about a claim concerning words NOBODY SAID —
    /// `fuzzy_book` repairing `hymn` into `Numbers`, a bare verse hung on a
    /// remembered book. Its confidences were real parse confidences about a word
    /// that was never spoken, which is why no threshold could have saved them. A
    /// verbatim run is the opposite case: every word of it came out of the
    /// speaker's mouth, in that order, and the only question left is which verse
    /// holds them. `Semantic`, `Ambiguous`, `UncertainBook` and `UncertainNumber`
    /// are untouched, and so is `Quoted` for everything short of this bar.
    ///
    /// ── WHAT "ENOUGH" MEANS, MEASURED ──────────────────────────────────────
    ///
    /// Two conditions, both in `for_quotation`, both evidence and neither a
    /// score. See `READING_RUN_WORDS` for the run length and the measurements
    /// behind it, and `PhraseHit::sole` for why a tie is refused.
    Reading,
}

impl DetectionMethod {
    /// Whether a candidate detected this way is ever allowed onto the
    /// congregation's screen without a human confirming it first.
    ///
    /// Only `Direct` is. `Semantic`, `Ambiguous` and `Repaired` carry confidences
    /// that are not calibrated probabilities, so no threshold on them is
    /// meaningful — gating them by number would be gating them by noise.
    ///
    /// `Repaired` is the newest and was learned the hard way: its confidence *is*
    /// a real parse confidence, which is precisely the trap. The number is honest
    /// about the parse and says nothing about whether the word being parsed was
    /// the word that was spoken. See the variant's doc comment.
    pub fn may_auto_fire(&self) -> bool {
        matches!(self, DetectionMethod::Direct | DetectionMethod::Reading)
    }

    /// Is this method's `confidence` a parse probability — something the
    /// self-calibrating gate may learn an auto-fire bar from?
    ///
    /// ── Why this is NOT `may_auto_fire` ────────────────────────────────────
    ///
    /// It was, and the two questions only looked like one while `Direct` was the
    /// single answer to both. `Reading` splits them: it may reach a wall, and its
    /// number is `quoted_confidence(run)` — a word count on a scale of its own
    /// invention. `record_feedback` moves `Thresholds::auto_fire` toward the
    /// score the operator agreed or disagreed with, so letting a run length in
    /// there would drag the gate that governs SPOKEN REFERENCES using a figure
    /// that is not about them. That is rule 10's category error, arriving from
    /// the direction nobody was watching.
    ///
    /// Confirming or dismissing a reading is still a confirmation or a dismissal.
    /// It simply carries no number, which `record_feedback` already handles.
    pub fn confidence_is_calibrated(&self) -> bool {
        matches!(self, DetectionMethod::Direct)
    }

    /// How strong a claim on the ONE wall slot this window may fill (rule 29).
    ///
    /// A heard reference beats a reading, and both beat everything that may only
    /// be offered. It decides what a congregation sees when a preacher names one
    /// verse and reads a different one in the same breath — and the answer is the
    /// one they NAMED, because naming it is the words saying, which is rule 40's
    /// principle rather than a new one.
    ///
    /// `pipeline::better` and `rank_for_wall` both ask this. They used to ask
    /// `may_auto_fire`, a bool, which cannot express three tiers.
    pub fn unattended_rank(&self) -> u8 {
        match self {
            DetectionMethod::Direct => 2,
            DetectionMethod::Reading => 1,
            _ => 0,
        }
    }

    /// Was this candidate found by finding a VERSE'S OWN WORDS in what was said,
    /// rather than by hearing a reference?
    ///
    /// The line the passage guard is drawn on, and it is drawn here because it is
    /// the same line rule 40 draws. `Direct`, `Ambiguous`, `UncertainBook` and
    /// `UncertainNumber` all exist because something reference-shaped came out of
    /// the speaker's mouth — "Romans eight", "verse thirty-two", a book name the
    /// parser had to repair. The words said, however badly, so **nothing Relay
    /// remembers may stand in front of them**. `Semantic`, `Quoted` and `Reading`
    /// are the other direction entirely: Relay went looking for the verse whose
    /// text resembles what was said, and nobody asked for anything.
    pub fn came_from_the_verse_text(&self) -> bool {
        matches!(
            self,
            DetectionMethod::Semantic | DetectionMethod::Quoted | DetectionMethod::Reading
        )
    }

    /// Is this a contiguous run of the speaker's own words, verbatim in the verse?
    ///
    /// The ONLY evidence that may arm the passage guard. A paraphrase cosine is a
    /// bag of words in no order (rule 18) and is nowhere near enough to assert
    /// "the preacher is reading this chapter aloud"; a verbatim run is precisely
    /// that assertion, which is why `Reading` was allowed to exist at all.
    pub fn is_a_verbatim_run(&self) -> bool {
        matches!(self, DetectionMethod::Quoted | DetectionMethod::Reading)
    }

    /// Is this quotation the preacher READING, or merely quoting?
    ///
    /// Pure, and deliberately: the evidence decides and nothing else may. The
    /// church's switch is applied in `Router::decide`, the one gate every
    /// candidate passes through, rather than here — so a second construction site
    /// for quoted candidates could not slip past it (rule 36).
    pub fn for_quotation(run: usize, sole: bool) -> Self {
        if run >= READING_RUN_WORDS && sole {
            DetectionMethod::Reading
        } else {
            DetectionMethod::Quoted
        }
    }

    /// The honest label for a bare "verse N". A book named in this window was
    /// heard; a book taken from the passage on screen was assumed, which is what
    /// `UncertainBook` already means ("chapter and verse heard, the book not"),
    /// and the router caps it at Suggest at any score. FIELD 2026-09-20.
    pub fn for_bare_verse(source: BareVerseSource) -> Self {
        match source {
            BareVerseSource::Anchor => DetectionMethod::Direct,
            BareVerseSource::Memory => DetectionMethod::UncertainBook,
        }
    }

    /// THE NAME THIS METHOD GOES BY EVERYWHERE OUTSIDE RUST — the console's own
    /// vocabulary, and since RG-309 the database's as well.
    ///
    /// It existed only as a `#[serde(rename)]` attribute and a `from_wire` with no
    /// inverse, which is why `db_method` could collapse three variants into one
    /// without anything noticing: there was no function to reuse. `from_wire` is
    /// this function's inverse and `the_wire_name_is_one_mapping_in_two_directions`
    /// asserts both halves against serde itself over every variant, so the
    /// attribute, this match and the parser cannot drift apart.
    pub fn wire(&self) -> &'static str {
        match self {
            DetectionMethod::Direct => "direct",
            DetectionMethod::Semantic => "semantic",
            DetectionMethod::Quoted => "quoted",
            DetectionMethod::Ambiguous => "ambiguous",
            DetectionMethod::UncertainBook => "uncertain_book",
            DetectionMethod::UncertainNumber => "uncertain_number",
            DetectionMethod::Reading => "reading",
        }
    }

    /// Parse the wire name the console sends back when an operator accepts a
    /// suggestion. Anything unrecognised is treated as the most cautious reading —
    /// `Semantic` — because the question this answers is "may this number teach the
    /// auto-fire bar", and an unknown method has not earned a yes.
    pub fn from_wire(s: &str) -> Self {
        match s {
            "direct" => DetectionMethod::Direct,
            "ambiguous" => DetectionMethod::Ambiguous,
            "uncertain_book" => DetectionMethod::UncertainBook,
            "uncertain_number" => DetectionMethod::UncertainNumber,
            "quoted" => DetectionMethod::Quoted,
            "reading" => DetectionMethod::Reading,
            _ => DetectionMethod::Semantic,
        }
    }

    /// Demote a match whose NUMBERS were inferred rather than heard.
    ///
    /// Applied at the five sites in `parse_reference` that already demoted by
    /// score. It never overwrites `UncertainBook`: a guessed book name is the
    /// larger doubt and has its own sentence on screen, and stacking them would
    /// tell the operator the smaller of the two things that is wrong.
    fn uncertain_number(m: &mut RefMatch) {
        if m.method == DetectionMethod::Direct {
            m.method = DetectionMethod::UncertainNumber;
        }
    }

    /// The value written to `detections.method`, which is constrained to
    /// `('direct','semantic')` (docs/data/schema.sql).
    ///
    /// `Ambiguous` persists as `direct` — and that is honest, not a fudge: the
    /// reference genuinely *was* parsed from spoken words ("Revelation 22"). What
    /// is ambiguous is *which verse* it resolves to, not how it was found. The
    /// routing distinction (never auto-fire) is enforced in the router, which is
    /// where it belongs; it isn't a property of the historical record.
    /// `Repaired` persists as `direct` for the same reason `Ambiguous` does: it
    /// was parsed from spoken words. What the record loses is *which* word was
    /// guessed — and `heard_text` carries that, which is the column that exists
    /// because a service put forty wrong verses on a wall and the log could not
    /// say what any of them heard.
    /// The name this method goes by in `detections.method` — **which is now
    /// `wire()` and nothing else** (RG-309).
    ///
    /// ## What this used to do, and why it stopped
    ///
    /// It collapsed all seven variants into `'direct'` or `'semantic'`, because the
    /// column was `CHECK`ed to those two values and widening it meant rebuilding
    /// the table, which is the migration rule 25 was written about. Its own comment
    /// filed the cost as a KNOWN GAP: a church auditing a wrong verse could see
    /// WHAT was heard and could not tell a followed reading from a paraphrase by
    /// this column alone.
    ///
    /// **Recording suggestions turned that gap into a wall.** The record exists to
    /// answer *what did the paraphrase detector do during a real sermon*, and
    /// 2,325 of one 16-hour service's 2,790 suggestion episodes are `Semantic`
    /// while `Quoted` sat in the same word. A count you cannot split is not a
    /// measurement. So the table was rebuilt (`db::ensure_detection_method_names_
    /// its_detector`, the v6 rung) and this function became the identity it should
    /// always have been.
    ///
    /// It is kept as a NAMED function rather than replaced by `wire()` at the call
    /// site, deliberately: "the name this goes by in the database" and "the name
    /// the console speaks" are two questions that happen to share an answer today,
    /// and a future column that needs to differ should have one place to differ in.
    /// `the_database_name_is_the_wire_name` is the test that says they agree now.
    pub fn db_method(&self) -> &'static str {
        self.wire()
    }
}

/// A resolved scripture reference (canonical book name as stored in the DB).
#[derive(Debug, Clone, PartialEq)]
pub struct VerseRef {
    pub book: String,
    pub chapter: i64,
    pub verse: i64,
}

impl VerseRef {
    #[cfg(test)]
    fn reference_book_chapter_verse(&self) -> String {
        format!("{} {}:{}", self.book, self.chapter, self.verse)
    }
}

/// A candidate detection found in a transcript span.
///
/// `reference` is the anchor — the first verse to display. A multi-verse passage
/// is described by `verse_end` (explicit range, e.g. "John 3:16-18") or
/// `whole_chapter` ("Psalm 23", no verse). The caller fires the anchor verse and
/// stages the rest so "next" walks the passage (see ContextMemory). Keeping the
/// output single-verse means the template engine never special-cases passages.
#[derive(Debug, Clone)]
pub struct RefMatch {
    pub reference: VerseRef,
    /// Inclusive end verse of an explicit range; None for a single verse or a
    /// whole-chapter reference (whose end is resolved from the corpus at fire).
    pub verse_end: Option<i64>,
    /// True when only a book + chapter was spoken ("Psalm 23") — display verse 1
    /// and stage the whole chapter.
    pub whole_chapter: bool,
    pub confidence: f32,
    /// Always `Direct` — `detect_direct` is, by definition, the direct matcher.
    /// Kept because a `RefMatch` without its provenance is a footgun waiting for
    /// the day a second matcher produces one, and the routing gate keys on method
    /// (see `router::decide`). Callers state the method themselves rather than
    /// trusting this field, which is why nothing reads it today.
    #[allow(dead_code)]
    pub method: DetectionMethod,
    /// The exact span of transcript this reference was parsed from.
    ///
    /// This reaches the operator console (`DetectionEvent::matched_text`). Showing
    /// *what words* triggered a match is the clearest possible explanation of an AI
    /// decision: an operator can tell in a glance whether Relay heard "John three
    /// sixteen" or misheard "gone free sixty".
    pub matched_text: String,
    /// This match ran to the LAST WORD of the text it was parsed from — nothing
    /// followed it.
    ///
    /// Meaningless on its own; it matters only for a PARTIAL transcript, where the
    /// text is still growing. There, ending at the tail means the next word may
    /// still be part of this reference, so the reading is provisional. See
    /// `main::emit_detections`, which is where the transcript's finality is known.
    pub at_tail: bool,
}

impl RefMatch {
    /// True when this reading exists only because the transcript was CUT OFF, and
    /// so describes the window boundary rather than anything anyone said.
    ///
    /// "…turn to John chapter 3" is a complete, well-formed whole-chapter reference
    /// worth 0.88 — and it is also exactly what "John chapter 3 verse 16" looks like
    /// one second before the number arrives. The STT window is re-decoded about once
    /// a second and detection runs on every partial (DECISIONS.md), so whether a
    /// citation is seen whole or half depends only on where the boundary lands.
    /// Measured via `stt::bench::engine_shootout`, that coin toss put John 3:1 on the
    /// wall ahead of John 3:16.
    ///
    /// Narrow on purpose — a whole-chapter reading, with nothing after it, in text
    /// that can still grow. A complete "John 3:16" at the tail is NOT provisional and
    /// still fires instantly, so this costs no latency on the path that matters.
    /// Nothing is lost, either: the next partial carries the number, and a preacher
    /// who really did mean the chapter gets it when the utterance closes.
    ///
    /// Lives here, and is used by both `main::emit_detections` and
    /// `stt::bench::engine_shootout`, so the bench cannot score a policy the live
    /// path does not have.
    pub fn is_provisional(&self, is_final: bool) -> bool {
        self.whole_chapter && self.at_tail && !is_final
    }
}

/// The 66 canonical books in standard Protestant order. This is the source of
/// truth for both the direct-match alias table AND the full-Bible import
/// (db.rs maps the KJV JSON's books to these names by index), so a detected
/// reference and a stored verse always agree on spelling.
pub const CANONICAL_BOOKS: &[&str] = &[
    "Genesis",
    "Exodus",
    "Leviticus",
    "Numbers",
    "Deuteronomy",
    "Joshua",
    "Judges",
    "Ruth",
    "1 Samuel",
    "2 Samuel",
    "1 Kings",
    "2 Kings",
    "1 Chronicles",
    "2 Chronicles",
    "Ezra",
    "Nehemiah",
    "Esther",
    "Job",
    "Psalms",
    "Proverbs",
    "Ecclesiastes",
    "Song of Solomon",
    "Isaiah",
    "Jeremiah",
    "Lamentations",
    "Ezekiel",
    "Daniel",
    "Hosea",
    "Joel",
    "Amos",
    "Obadiah",
    "Jonah",
    "Micah",
    "Nahum",
    "Habakkuk",
    "Zephaniah",
    "Haggai",
    "Zechariah",
    "Malachi",
    "Matthew",
    "Mark",
    "Luke",
    "John",
    "Acts",
    "Romans",
    "1 Corinthians",
    "2 Corinthians",
    "Galatians",
    "Ephesians",
    "Philippians",
    "Colossians",
    "1 Thessalonians",
    "2 Thessalonians",
    "1 Timothy",
    "2 Timothy",
    "Titus",
    "Philemon",
    "Hebrews",
    "James",
    "1 Peter",
    "2 Peter",
    "1 John",
    "2 John",
    "3 John",
    "Jude",
    "Revelation",
];

/// Books with a single chapter — referenced by bare verse ("Jude 4" = Jude
/// 1:4, "Philemon verse 6" = Philemon 1:6). For these the first (or only)
/// number is the verse and the chapter defaults to 1.
const SINGLE_CHAPTER_BOOKS: &[&str] = &["Obadiah", "Philemon", "2 John", "3 John", "Jude"];

fn is_single_chapter(book: &str) -> bool {
    SINGLE_CHAPTER_BOOKS.contains(&book)
}

/// Verses per chapter, per book — indexed by `CANONICAL_BOOKS` position.
///
/// This is SHAPE, not scripture: 1189 chapter lengths, the same facts in every
/// translation. It is a `const` rather than a read of the bundled KJV so that
/// `detection` stays free of IO and of the 4 MB corpus, and a `#[cfg(test)]`
/// test parses `kjv.json` and asserts the two agree — so it cannot silently
/// drift from the Bible actually shipped.
///
/// It exists for `split_run_into_chapter_verse`, which cannot work without it:
/// deciding that "663" is 6:63 and not 66:3 requires knowing John has 21
/// chapters and that chapter 6 has at least 63 verses.
#[rustfmt::skip]
const VERSES_PER_CHAPTER: &[&[u8]] = &[
    &[31,25,24,26,32,22,24,22,29,32,32,20,18,24,21,16,27,33,38,18,34,24,20,67,34,35,46,22,35,43,55,32,20,31,29,43,36,30,23,23,57,38,34,34,28,34,31,22,33,26],
    &[22,25,22,31,23,30,25,32,35,29,10,51,22,31,27,36,16,27,25,26,36,31,33,18,40,37,21,43,46,38,18,35,23,35,35,38,29,31,43,38],
    &[17,16,17,35,19,30,38,36,24,20,47,8,59,57,33,34,16,30,37,27,24,33,44,23,55,46,34],
    &[54,34,51,49,31,27,89,26,23,36,35,16,33,45,41,50,13,32,22,29,35,41,30,25,18,65,23,31,40,16,54,42,56,29,34,13],
    &[46,37,29,49,33,25,26,20,29,22,32,32,18,29,23,22,20,22,21,20,23,30,25,22,19,19,26,68,29,20,30,52,29,12],
    &[18,24,17,24,15,27,26,35,27,43,23,24,33,15,63,10,18,28,51,9,45,34,16,33],
    &[36,23,31,24,31,40,25,35,57,18,40,15,25,20,20,31,13,31,30,48,25],
    &[22,23,18,22],
    &[28,36,21,22,12,21,17,22,27,27,15,25,23,52,35,23,58,30,24,42,15,23,29,22,44,25,12,25,11,31,13],
    &[27,32,39,12,25,23,29,18,13,19,27,31,39,33,37,23,29,33,43,26,22,51,39,25],
    &[53,46,28,34,18,38,51,66,28,29,43,33,34,31,34,34,24,46,21,43,29,53],
    &[18,25,27,44,27,33,20,29,37,36,21,21,25,29,38,20,41,37,37,21,26,20,37,20,30],
    &[54,55,24,43,26,81,40,40,44,14,47,40,14,17,29,43,27,17,19,8,30,19,32,31,31,32,34,21,30],
    &[17,18,17,22,14,42,22,18,31,19,23,16,22,15,19,14,19,34,11,37,20,12,21,27,28,23,9,27,36,27,21,33,25,33,27,23],
    &[11,70,13,24,17,22,28,36,15,44],
    &[11,20,32,23,19,19,73,18,38,39,36,47,31],
    &[22,23,15,17,14,14,10,17,32,3],
    &[22,13,26,21,27,30,21,22,35,22,20,25,28,22,35,22,16,21,29,29,34,30,17,25,6,14,23,28,25,31,40,22,33,37,16,33,24,41,30,24,34,17],
    &[6,12,8,8,12,10,17,9,20,18,7,8,6,7,5,11,15,50,14,9,13,31,6,10,22,12,14,9,11,12,24,11,22,22,28,12,40,22,13,17,13,11,5,26,17,11,9,14,20,23,19,9,6,7,23,13,11,11,17,12,8,12,11,10,13,20,7,35,36,5,24,20,28,23,10,12,20,72,13,19,16,8,18,12,13,17,7,18,52,17,16,15,5,23,11,13,12,9,9,5,8,28,22,35,45,48,43,13,31,7,10,10,9,8,18,19,2,29,176,7,8,9,4,8,5,6,5,6,8,8,3,18,3,3,21,26,9,8,24,13,10,7,12,15,21,10,20,14,9,6],
    &[33,22,35,27,23,35,27,36,18,32,31,28,25,35,33,33,28,24,29,30,31,29,35,34,28,28,27,28,27,33,31],
    &[18,26,22,16,20,12,29,17,18,20,10,14],
    &[17,17,11,16,16,13,13,14],
    &[31,22,26,6,30,13,25,22,21,34,16,6,22,32,9,14,14,7,25,6,17,25,18,23,12,21,13,29,24,33,9,20,24,17,10,22,38,22,8,31,29,25,28,28,25,13,15,22,26,11,23,15,12,17,13,12,21,14,21,22,11,12,19,12,25,24],
    &[19,37,25,31,31,30,34,22,26,25,23,17,27,22,21,21,27,23,15,18,14,30,40,10,38,24,22,17,32,24,40,44,26,22,19,32,21,28,18,16,18,22,13,30,5,28,7,47,39,46,64,34],
    &[22,22,66,22,22],
    &[28,10,27,17,17,14,27,18,11,22,25,28,23,23,8,63,24,32,14,49,32,31,49,27,17,21,36,26,21,26,18,32,33,31,15,38,28,23,29,49,26,20,27,31,25,24,23,35],
    &[21,49,30,37,31,28,28,27,27,21,45,13],
    &[11,23,5,19,15,11,16,14,17,15,12,14,16,9],
    &[20,32,21],
    &[15,16,15,13,27,14,17,14,15],
    &[21],
    &[17,10,10,11],
    &[16,13,12,13,15,16,20],
    &[15,13,19],
    &[17,20,19],
    &[18,15,20],
    &[15,23],
    &[21,13,10,14,11,15,14,23,17,12,17,14,9,21],
    &[14,17,18,6],
    &[25,23,17,25,48,34,29,34,38,42,30,50,58,36,39,28,27,35,30,34,46,46,39,51,46,75,66,20],
    &[45,28,35,41,43,56,37,38,50,52,33,44,37,72,47,20],
    &[80,52,38,44,39,49,50,56,62,42,54,59,35,35,32,31,37,43,48,47,38,71,56,53],
    &[51,25,36,54,47,71,53,59,41,42,57,50,38,31,27,33,26,40,42,31,25],
    &[26,47,26,37,42,15,60,40,43,48,30,25,52,28,41,40,34,28,41,38,40,30,35,27,27,32,44,31],
    &[32,29,31,25,21,23,25,39,33,21,36,21,14,23,33,27],
    &[31,16,23,21,13,20,40,13,27,33,34,31,13,40,58,24],
    &[24,17,18,18,21,18,16,24,15,18,33,21,14],
    &[24,21,29,31,26,18],
    &[23,22,21,32,33,24],
    &[30,30,21,23],
    &[29,23,25,18],
    &[10,20,13,18,28],
    &[12,17,18],
    &[20,15,16,16,25,21],
    &[18,26,17,22],
    &[16,15,15],
    &[25],
    &[14,18,19,16,14,20,28,13,28,39,40,29,25],
    &[27,26,18,17,20],
    &[25,25,22,19,14],
    &[21,22,18],
    &[10,29,24,21,21],
    &[13],
    &[14],
    &[25],
    &[20,29,22,11,14,17,17,13,21,11,19,17,18,20,8,21,18,24,21,15,27,21],
];

/// How many chapters a book has (0 if the book is unknown).
fn chapter_count(book: &str) -> usize {
    CANONICAL_BOOKS
        .iter()
        .position(|b| *b == book)
        .and_then(|i| VERSES_PER_CHAPTER.get(i))
        .map(|c| c.len())
        .unwrap_or(0)
}

/// How many verses are in `book` chapter `chapter` (0 if either is out of range).
fn verse_count(book: &str, chapter: i64) -> usize {
    if chapter < 1 {
        return 0;
    }
    CANONICAL_BOOKS
        .iter()
        .position(|b| *b == book)
        .and_then(|i| VERSES_PER_CHAPTER.get(i))
        .and_then(|c| c.get((chapter - 1) as usize))
        .map(|n| *n as usize)
        .unwrap_or(0)
}

/// Repair a digit run that whisper ran together: "663" → 6:63.
///
/// ── The mishearing this exists for ──────────────────────────────────────────
///
/// A preacher says "John six sixty-three". Whisper does not write `6:63`; on a
/// fast or accented delivery it writes the digits it heard, joined: `663`. The
/// parser then read the whole run as a CHAPTER, and a congregation was offered
/// `John 663:1`. Observed live on 2026-07-26, five times in one service, and
/// every one of them was a verse the operator then fired BY HAND:
///
///     "john 663"     → John 663:1        was John 6:63
///     "hebrews 416"  → Hebrews 416:1     was Hebrews 4:16
///     "mark 1124"    → Mark 1124:1       was Mark 11:24
///     "romans 828"   → Romans 828:1      was Romans 8:28
///     "john 1623"    → John 1623:1       was John 16:23
///
/// Relay had heard the reference perfectly and then mangled the number.
///
/// ── Why this is safe, and where it stops ────────────────────────────────────
///
/// **A run that IS a valid chapter of this book is never touched.** "Psalm 23"
/// is a whole-chapter reference and must stay one — splitting it into 2:3 would
/// be a new bug of exactly the kind being fixed. The repair only runs where
/// reading the run as a chapter is IMPOSSIBLE, which is a fact about the book,
/// not a guess about the speaker.
///
/// **An ambiguous split is refused**, the same rule `fuzzy_book` follows: if two
/// different chapter:verse pairs are both real, there is no evidence to choose,
/// and guessing is the failure mode. Measured over every book and every 3- and
/// 4-digit run, 95% of repairable runs have exactly one valid split.
fn split_run_into_chapter_verse(book: &str, run: i64) -> Option<(i64, i64)> {
    let chapters = chapter_count(book);
    if chapters == 0 {
        return None;
    }
    // A real chapter of this book. Not a mishearing — leave it entirely alone.
    if run >= 1 && (run as usize) <= chapters {
        return None;
    }
    let digits = run.to_string();
    let mut found: Option<(i64, i64)> = None;
    for i in 1..digits.len() {
        let (c, v) = digits.split_at(i);
        // A leading zero is not how anyone says or writes a number: "1005" is
        // not 100:5 by way of chapter 1 verse 005.
        if c.starts_with('0') || v.starts_with('0') {
            continue;
        }
        let (Ok(c), Ok(v)) = (c.parse::<i64>(), v.parse::<i64>()) else {
            continue;
        };
        if c < 1 || c as usize > chapters || v < 1 || v as usize > verse_count(book, c) {
            continue;
        }
        if found.is_some() {
            return None; // ambiguous — refuse, do not guess
        }
        found = Some((c, v));
    }
    found
}

/// Alias → canonical-book map, built once. Covers the lowercase full name, the
/// spoken/written forms of numbered books ("first"/"i"/"1", "1john"), plus a few
/// common variants and ASR mishears. Multilingual-ready: add rows per language.
static ALIAS_MAP: OnceLock<HashMap<String, &'static str>> = OnceLock::new();

fn alias_map() -> &'static HashMap<String, &'static str> {
    ALIAS_MAP.get_or_init(|| {
        let mut m: HashMap<String, &'static str> = HashMap::new();
        for &book in CANONICAL_BOOKS {
            let lower = book.to_lowercase();
            m.insert(lower.clone(), book);
            // Numbered books: "1 John" → "first john" / "i john" / "1john".
            for (digit, words) in [
                ("1", ["first", "i"]),
                ("2", ["second", "ii"]),
                ("3", ["third", "iii"]),
            ] {
                if let Some(rest) = lower.strip_prefix(&format!("{digit} ")) {
                    for w in words {
                        m.insert(format!("{w} {rest}"), book);
                    }
                    m.insert(format!("{digit}{rest}"), book); // "1john"
                }
            }
        }
        // Common written abbreviations for fast manual-override typing
        // ("ps 23 1", "rom 8 1", "1 jn 3 1"). Single-token ones here; numbered
        // ones ("1 jn") are added in the loop below.
        let abbr: &[(&str, &str)] = &[
            ("gen", "Genesis"),
            ("exo", "Exodus"),
            ("ex", "Exodus"),
            ("lev", "Leviticus"),
            ("lv", "Leviticus"),
            ("num", "Numbers"),
            ("nm", "Numbers"),
            ("deut", "Deuteronomy"),
            ("deu", "Deuteronomy"),
            ("dt", "Deuteronomy"),
            ("josh", "Joshua"),
            ("jos", "Joshua"),
            ("judg", "Judges"),
            ("jdg", "Judges"),
            ("rth", "Ruth"),
            ("ps", "Psalms"),
            ("psa", "Psalms"),
            ("pss", "Psalms"),
            ("prov", "Proverbs"),
            ("prv", "Proverbs"),
            ("pro", "Proverbs"),
            ("eccl", "Ecclesiastes"),
            ("ecc", "Ecclesiastes"),
            ("song", "Song of Solomon"),
            ("sos", "Song of Solomon"),
            ("isa", "Isaiah"),
            ("jer", "Jeremiah"),
            ("jr", "Jeremiah"),
            ("lam", "Lamentations"),
            ("ezek", "Ezekiel"),
            ("eze", "Ezekiel"),
            ("ezk", "Ezekiel"),
            ("dan", "Daniel"),
            ("dn", "Daniel"),
            ("hos", "Hosea"),
            ("jl", "Joel"),
            ("amo", "Amos"),
            ("obad", "Obadiah"),
            ("oba", "Obadiah"),
            ("jnh", "Jonah"),
            ("mic", "Micah"),
            ("nah", "Nahum"),
            ("hab", "Habakkuk"),
            ("zeph", "Zephaniah"),
            ("zep", "Zephaniah"),
            ("hag", "Haggai"),
            ("zech", "Zechariah"),
            ("zec", "Zechariah"),
            ("mal", "Malachi"),
            ("matt", "Matthew"),
            ("mat", "Matthew"),
            ("mt", "Matthew"),
            ("mrk", "Mark"),
            ("mk", "Mark"),
            ("luk", "Luke"),
            ("lk", "Luke"),
            ("jhn", "John"),
            ("jn", "John"),
            ("acts", "Acts"),
            ("ac", "Acts"),
            ("rom", "Romans"),
            ("rm", "Romans"),
            ("gal", "Galatians"),
            ("ga", "Galatians"),
            ("eph", "Ephesians"),
            ("phil", "Philippians"),
            ("php", "Philippians"),
            ("col", "Colossians"),
            ("tit", "Titus"),
            ("phm", "Philemon"),
            ("heb", "Hebrews"),
            ("jas", "James"),
            ("jde", "Jude"),
            ("rev", "Revelation"),
            ("rv", "Revelation"),
        ];
        for (a, canon) in abbr {
            m.insert((*a).into(), canon);
        }
        // Numbered-book abbreviations: "1 sa"/"1sa" → 1 Samuel, "1 jn"/"1jn" →
        // 1 John, etc. Two-letter stems keep them short to type.
        let numbered: &[(&str, &str)] = &[
            ("sa", "Samuel"),
            ("sm", "Samuel"),
            ("ki", "Kings"),
            ("kg", "Kings"),
            ("ch", "Chronicles"),
            ("chr", "Chronicles"),
            ("co", "Corinthians"),
            ("cor", "Corinthians"),
            ("th", "Thessalonians"),
            ("thess", "Thessalonians"),
            ("ti", "Timothy"),
            ("tim", "Timothy"),
            ("pe", "Peter"),
            ("pet", "Peter"),
            ("jn", "John"),
            ("jo", "John"),
        ];
        for d in ['1', '2', '3'] {
            for (stem, word) in numbered {
                let canon = CANONICAL_BOOKS
                    .iter()
                    .find(|b| b.starts_with(d) && b.ends_with(word))
                    .copied();
                if let Some(canon) = canon {
                    m.insert(format!("{d} {stem}"), canon); // "1 jn"
                    m.insert(format!("{d}{stem}"), canon); // "1jn"
                }
            }
        }

        // Extra spoken variants + ASR/accent mishears. The silent "P" in Psalms
        // is frequently dropped by ASR on African-accented speech ("sam",
        // "salm"), so those map to Psalms.
        m.insert("psalm".into(), "Psalms");
        m.insert("palms".into(), "Psalms");
        m.insert("sam".into(), "Psalms");
        m.insert("salm".into(), "Psalms");
        m.insert("salms".into(), "Psalms");
        m.insert("sams".into(), "Psalms");
        m.insert("jon".into(), "John");
        m.insert("mathew".into(), "Matthew");
        m.insert("mathews".into(), "Matthew");
        m.insert("proverb".into(), "Proverbs");
        m.insert("song of songs".into(), "Song of Solomon");
        m.insert("canticles".into(), "Song of Solomon");
        m.insert("revelations".into(), "Revelation");
        // Common spelling/accent variants whisper emits.
        m.insert("collosians".into(), "Colossians");
        m.insert("colosians".into(), "Colossians");
        m.insert("phillipians".into(), "Philippians");
        m.insert("philipians".into(), "Philippians");
        m.insert("efesians".into(), "Ephesians");
        m.insert("ephesus".into(), "Ephesians");
        m.insert("deutronomy".into(), "Deuteronomy");
        m.insert("ecclesiastis".into(), "Ecclesiastes");
        m.insert("thessalonians".into(), "1 Thessalonians"); // bare → most-common
        m.insert("galatia".into(), "Galatians");

        // ── Tier-1 languages: Yorùbá, Kiswahili, Hausa ──────────────────────
        //
        // THE thing that was missing. Relay's stated differentiator is
        // African-language speech, and until now the detector spoke only English:
        // a preacher could say "Jòhánù orí kẹta" with a perfect Yorùbá model
        // behind them and Relay would detect NOTHING, because the alias table had
        // no idea what "Jòhánù" was. Fine-tuning the acoustic model would not have
        // fixed that by a single verse — the moat was blocked on this table, not
        // on the model.
        //
        // Loaded from data, not hardcoded here: see the _readme in the JSON.
        for (alias, canonical) in language_aliases() {
            m.insert(alias, canonical);
        }
        m
    })
}

/// The book names Relay knows for a language, for biasing the STT decoder.
///
/// `lang` is a Whisper language code ("yo", "sw", "ha"); anything else (including
/// None/auto) yields the English canon.
///
/// ALWAYS includes English alongside the local names, because code-switching is
/// the normal case for this market, not an edge case (CLAUDE.md): a Yorùbá sermon
/// routinely names the book in Yorùbá and the chapter and verse in English.
pub fn bias_vocabulary(lang: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = CANONICAL_BOOKS.iter().map(|b| b.to_string()).collect();
    let Some(lang) = lang else { return out };
    const RAW: &str = include_str!("../data/book_aliases.json");
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(RAW) else {
        return out;
    };
    if let Some(books) = doc.get(lang).and_then(|v| v.as_object()) {
        for (english, names) in books {
            if english.starts_with('_') {
                continue;
            }
            // Only the FIRST spelling — the properly-accented one. The prompt is a
            // hint to the decoder, not a lookup table, and stuffing it with every
            // ASCII fallback dilutes the signal.
            if let Some(first) = names
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
            {
                out.push(first.to_string());
            }
        }
    }
    out
}

/// Book names in Relay's tier-1 languages, from `data/book_aliases.json`.
///
/// Data rather than code on purpose. The maintainer does not speak all three of
/// these languages fluently, and a WRONG alias does not fail safely — it puts the
/// wrong scripture on a wall. Keeping the names in JSON lets a native speaker fix
/// them in a one-line pull request without touching Rust or knowing what a
/// HashMap is. That is the only path by which this table ever becomes trustworthy.
///
/// Baked into the binary (`include_str!`), so it stays fully offline.
fn language_aliases() -> Vec<(String, &'static str)> {
    const RAW: &str = include_str!("../data/book_aliases.json");
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(RAW) else {
        eprintln!("detection: book_aliases.json is not valid JSON — tier-1 languages disabled");
        return Vec::new();
    };
    let Some(langs) = doc.as_object() else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for (lang, books) in langs {
        if lang.starts_with('_') {
            continue; // _readme
        }
        let Some(books) = books.as_object() else {
            continue;
        };
        for (english, names) in books {
            if english.starts_with('_') {
                continue; // _language, _complete, _todo
            }
            // Key against the canonical spelling, so a typo in the data file is a
            // no-op rather than a phantom book that can never resolve to a verse.
            let Some(canonical) = CANONICAL_BOOKS.iter().find(|b| *b == english).copied() else {
                eprintln!("detection: book_aliases.json has unknown book {english:?} — ignored");
                continue;
            };
            for n in names.as_array().into_iter().flatten() {
                if let Some(n) = n.as_str() {
                    // normalize() folds the tone marks and dots-below, so the
                    // table is keyed on exactly what a transcript will produce.
                    let key = normalize(n);
                    if !key.is_empty() {
                        out.push((key, canonical));
                    }
                }
            }
        }
    }
    out
}

/// ── THE TRANSLATION THE SPEAKER NAMED, IF THEY NAMED ONE (RG-135) ──────────
///
/// A preacher who says *"the Passion Translation says…"* and then reads aloud is
/// telling the room which words are coming. Relay has one translation installed on
/// a fresh install, so what went on the wall on 2026-09-13 was the King James
/// Version, at 0.88, `direct`, wearing the same badge as the seven correct fires
/// around it. **The reference was right.** No instrument in the product treats that
/// as a fault of any kind, and none could: the only thing wrong was that the words
/// on the screen were not the words being read out.
///
/// That is rule 35 on the AI Detection panel — a claim that reads identically
/// whether or not it is the thing the preacher asked for.
///
/// This answers only the first half: WAS a translation named. Whether Relay has it
/// is a database question and is asked by the caller, because this module is
/// DB-free and is staying that way.
///
/// ── WHAT IT DELIBERATELY DOES NOT DO ───────────────────────────────────────
///
/// It does not fetch, license or render another translation. RG-50 is an operator
/// unable to ADD one and is a different row with a different answer; this is Relay
/// unable to SAY that the one it used is not the one that was named. A label, not a
/// licensing decision.
///
/// It also does not guess. Only whole-token matches against a fixed list of names
/// and abbreviations count, because a false positive here puts a warning on a
/// correct fire — and an operator who learns to ignore this line has lost the one
/// case it exists for. `message` is an ordinary English word and is not on the
/// list for that reason; `msg` is, because nobody says it by accident.
pub fn named_translation(text: &str) -> Option<String> {
    // (canonical abbreviation, the things a decoder actually produces for it).
    // Spoken forms matter more than written ones here: this reads a TRANSCRIPT, so
    // "the Passion Translation" is far likelier than "TPT".
    const NAMES: &[(&str, &[&str])] = &[
        (
            "KJV",
            &[
                "king james",
                "kjv",
                "authorised version",
                "authorized version",
            ],
        ),
        ("NKJV", &["new king james", "nkjv"]),
        ("NIV", &["new international", "niv"]),
        ("ESV", &["english standard", "esv"]),
        ("NASB", &["new american standard", "nasb"]),
        ("NLT", &["new living translation", "nlt"]),
        ("TPT", &["passion translation", "the passion", "tpt"]),
        ("MSG", &["the message translation", "msg"]),
        ("AMP", &["amplified bible", "amplified version", "amp"]),
        ("CSB", &["christian standard", "csb"]),
        ("RSV", &["revised standard", "rsv"]),
        ("NRSV", &["new revised standard", "nrsv"]),
        (
            "GNB",
            &["good news bible", "good news translation", "gnb", "gnt"],
        ),
        ("YLT", &["young's literal", "youngs literal", "ylt"]),
    ];
    let hay = text.to_lowercase();
    // Longest phrase first, so "new king james" is not answered by "king james".
    let mut best: Option<(usize, &str)> = None;
    for (canon, forms) in NAMES {
        for form in *forms {
            if contains_token_run(&hay, form) {
                let len = form.len();
                if best.map(|(n, _)| len > n).unwrap_or(true) {
                    best = Some((len, canon));
                }
            }
        }
    }
    best.map(|(_, c)| c.to_string())
}

/// `needle` appears in `hay` on whole-token boundaries.
///
/// A plain `contains` would answer "amp" inside "example" and "example" is a word a
/// preacher says. The boundary test is "not alphanumeric on either side", which is
/// enough for a transcript: whisper emits words separated by spaces and punctuation.
fn contains_token_run(hay: &str, needle: &str) -> bool {
    let bytes = hay.as_bytes();
    let mut from = 0;
    while let Some(rel) = hay[from..].find(needle) {
        let at = from + rel;
        let end = at + needle.len();
        let before_ok = at == 0 || !(bytes[at - 1] as char).is_alphanumeric();
        let after_ok = end == bytes.len() || !(bytes[end] as char).is_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

/// Find all direct scripture references in `text`. Returns them left-to-right.
pub fn detect_direct(text: &str) -> Vec<RefMatch> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if let Some((canonical, book_end, book_ev)) = match_book(&tokens, i) {
            if let Some((mut m, next)) = parse_reference(&tokens, book_end, canonical, i, book_ev) {
                // Nothing followed this reference in the text it came from. On a
                // partial transcript that means the next word might still belong to
                // it — see `RefMatch::at_tail`.
                m.at_tail = next >= tokens.len();
                out.push(m);
                i = next;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Fold a character down to its plain-ASCII skeleton.
///
/// Yorùbá and Hausa orthography carries marks that ASR reproduces unreliably:
/// tone marks (`ò á ń`), dots-below (`ẹ ọ ṣ`), and hooked consonants (`ɓ ɗ ƙ`).
/// Whisper will emit `Jòhánù`, `Johánù` or `Johanu` for the same audio depending
/// on the recording. If those are three different tokens, the alias table matches
/// none of them and Relay detects nothing — which is precisely the state it was
/// in before the multilingual table existed.
///
/// So all three fold to `johanu` and match once.
///
/// Deliberately lossy, and that is fine: this folds text for MATCHING, never for
/// display. What the congregation sees is always the canonical corpus text.
fn fold_char(c: char) -> Option<char> {
    match c {
        // Hausa hooked consonants are distinct letters, not accented ones, so NFD
        // will not decompose them. They have to be mapped by hand.
        'ɓ' | 'Ɓ' => Some('b'),
        'ɗ' | 'Ɗ' => Some('d'),
        'ƙ' | 'Ƙ' => Some('k'),
        'ƴ' | 'Ƴ' => Some('y'),
        // Combining marks left behind by NFD — tone marks, dots-below. Drop them.
        c if ('\u{0300}'..='\u{036F}').contains(&c) => None,
        c => Some(c),
    }
}

/// Lowercase, fold diacritics, strip punctuation except the digit-pairing colon,
/// split hyphens ("twenty-eight" → two tokens), collapse whitespace.
/// `pub(crate)` so the WER bench can fold a reference transcript EXACTLY the way the
/// detector folds a hypothesis. A scorer that normalises the two sides differently is
/// not measuring anything.
pub(crate) fn normalize(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let mut s = String::with_capacity(text.len());
    // NFD first, so `ọ` becomes `o` + combining-dot-below and the mark can be
    // dropped generically instead of via a 200-row lookup table.
    for ch in text.nfd().filter_map(fold_char) {
        match ch {
            c if c.is_alphanumeric() => s.extend(c.to_lowercase()),
            ':' => s.push(':'),
            // Apostrophes are DROPPED (not split) so ASR possessives stay one
            // token: "Sam's" → "sams" (→ Psalms). This also folds the Hausa
            // glottal in "Ru'ya" → "ruya".
            '\'' | '\u{2019}' | '\u{02BC}' => {}
            _ => s.push(' '), // hyphen, comma, period, etc. → separator
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Ordinary English words that are ALSO one-token book aliases.
///
/// `NEVER_FUZZY` already records the insight — *"'mark', 'job' and 'will' are all
/// ordinary English AND book names, so an approximate match on them is never a
/// repair — it is a coincidence"* — but it only guards the fuzzy path. An **exact**
/// match on `song` was still a heard reference, so "we'll sing song two twelve" put
/// Song of Solomon 2:12 on the wall at 0.900, unattended.
///
/// `eval.rs` already carries this exact case as a negative — `neg-yo-everyday-song`,
/// *"Ẹ jẹ́ ká kọ orin 3"* — **in Yorùbá**. The English twin was never written, which
/// is how a corpus can see a trap and still miss it.
///
/// The list is deliberately tiny and evidence-led: these two are what a 118-noun
/// sweep through the real router actually caught. `psalm` is NOT here and must never
/// be — "Psalm twenty three" is how preachers say it, and demoting it would trade a
/// false positive for a false negative on the single most-quoted book in use.
const ORDINARY_WORD_ALIASES: &[&str] = &["song", "job"];

/// How much the BOOK NAME is worth as evidence — the input to whether a candidate
/// may reach a congregation unattended. See `DetectionMethod::UncertainBook`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum BookEvidence {
    /// The alias matched exactly and the word means nothing else. Heard.
    Heard,
    /// `fuzzy_book` guessed it by edit distance. Never auto-fires, keyword or not:
    /// the repair is a guess about the acoustics and no amount of surrounding
    /// grammar makes the guessed word more likely to be the spoken one.
    Repaired,
    /// The alias matched exactly, but the word is ordinary English
    /// (`ORDINARY_WORD_ALIASES`). Auto-fires ONLY with an explicit chapter/verse
    /// keyword — unlike a repair, the word really was heard, so "song chapter two
    /// verse twelve" removes the doubt entirely while "song two twelve" does not.
    OrdinaryWord,
}

/// Match the longest book alias starting at `start`. Returns (canonical, index
/// just past the alias, how good the book evidence is).
fn match_book(tokens: &[&str], start: usize) -> Option<(&'static str, usize, BookEvidence)> {
    // Scan longest-first (up to 3 tokens) so multi-word books ("song of
    // solomon") and numbered forms ("first corinthians") match before a shorter
    // prefix would.
    for len in (1..=3).rev() {
        if start + len > tokens.len() {
            continue;
        }
        let candidate = tokens[start..start + len].join(" ");
        if let Some(&canonical) = alias_map().get(&candidate) {
            // Only a ONE-token alias can be an ordinary word. "song of solomon" is
            // three tokens and is nobody's everyday phrase.
            let ev = if len == 1 && ORDINARY_WORD_ALIASES.contains(&candidate.as_str()) {
                BookEvidence::OrdinaryWord
            } else {
                BookEvidence::Heard
            };
            return Some((canonical, start + len, ev));
        }
    }
    // Nothing matched exactly. Try to REPAIR a misheard book name.
    fuzzy_book(tokens, start).map(|c| (c, start + 1, BookEvidence::Repaired))
}

/// Words an ordinary sermon says constantly, which must never be repaired into a
/// book name however close they look.
///
/// This list is the difference between a helpful repair and putting the wrong
/// scripture on a wall. "among" is two edits from "amos"; "same" is two from
/// "james"; "act" and "acts" differ by one; "mark", "job" and "will" are all
/// ordinary English AND book names, so an approximate match on them is never a
/// repair — it is a coincidence.
const NEVER_FUZZY: &[&str] = &[
    "a", "am", "among", "amongst", "an", "and", "are", "as", "at", "be", "been", "but", "by",
    "call", "called", "came", "come", "did", "do", "does", "done", "for", "from", "gone", "good",
    "had", "has", "have", "he", "her", "here", "him", "his", "how", "i", "if", "in", "is", "it",
    "its", "just", "know", "let", "like", "look", "made", "make", "man", "many", "may", "me",
    "more", "most", "much", "must", "my", "name", "no", "not", "now", "of", "on", "one", "only",
    "or", "our", "out", "over", "own", "said", "same", "say", "says", "see", "shall", "she",
    "should", "so", "some", "son", "such", "take", "than", "that", "the", "their", "them", "then",
    "there", "these", "they", "thing", "this", "those", "thou", "time", "to", "up", "upon", "us",
    "very", "was", "way", "we", "well", "went", "were", "what", "when", "where", "which", "while",
    "who", "why", "will", "with", "word", "work", "would", "ye", "yes", "you", "your",
];

/// Repair a single misheard book token — but ONLY where a reference could
/// actually be.
///
/// ── Why this is gated on a following number ────────────────────────────────
///
/// Relay's whole promise is that a DIRECT match may go on a screen without a
/// human confirming it (CLAUDE.md §10). Approximate book matching is therefore
/// the single most dangerous thing in this file: get it wrong and the wrong
/// scripture is in front of a congregation, confidently.
///
/// So the repair may only run where the sentence is already reference-SHAPED —
/// the very next token is a chapter number or a chapter word. "sam" alone stays
/// an ordinary word; "sam twenty three" is a reference. That one condition
/// removes almost all of the risk, and as a side effect removes almost all of
/// the cost: the scan runs a handful of times per sermon, not once per token.
///
/// It is still marked FUZZY, which costs confidence downstream, so a repaired
/// reference needs to be otherwise strong to reach the auto-fire line.
fn fuzzy_book(tokens: &[&str], start: usize) -> Option<&'static str> {
    let token = *tokens.get(start)?;
    // Too short to repair safely: at two characters everything is one edit from
    // everything else.
    if token.len() < 3 || NEVER_FUZZY.contains(&token) {
        return None;
    }
    // Reference-shaped context only. See above.
    let next = tokens.get(start + 1)?;
    let numeric = next.chars().all(|c| c.is_ascii_digit())
        || classify_num_word(next).is_some()
        || is_chapter_word(next);
    if !numeric {
        return None;
    }

    // Budget scales with length: one edit for a short name, two for a long one.
    let budget = if token.len() <= 5 { 1 } else { 2 };

    let mut best: Option<(usize, &'static str)> = None;
    let mut second = usize::MAX;
    for (alias, &canonical) in alias_map().iter() {
        // Single-word aliases only; a multi-word mishear is a different problem.
        if alias.contains(' ') {
            continue;
        }
        // `continue`, NOT `?`. With `?` the whole search abandoned itself on the
        // first alias that happened to be far away — which is nearly always the
        // first one — so the repair never ran at all.
        let Some(d) = edit_distance_within(token, alias, budget) else {
            continue;
        };
        match best {
            Some((bd, _)) if d < bd => {
                second = bd;
                best = Some((d, canonical));
            }
            Some((bd, bc)) if d == bd && bc != canonical => second = second.min(d),
            None => best = Some((d, canonical)),
            _ => {}
        }
    }
    let (d, canonical) = best?;
    // AMBIGUOUS REPAIRS ARE REFUSED. If two different books are equally close,
    // there is no evidence to choose between them, and guessing is exactly the
    // failure mode this whole function is trying not to be.
    if second == d {
        return None;
    }
    let _ = d;
    Some(canonical)
}

/// Is `a` reachable from `b` in a single edit?
///
/// Exposed for the scripture search, which uses it to let a typed word land on a
/// near word without inventing a match. Same arithmetic the alias repair uses, so
/// there is one definition of "nearly the same word" in the product.
pub fn one_edit_apart(a: &str, b: &str) -> bool {
    matches!(edit_distance_within(a, b, 1), Some(d) if d <= 1)
}

/// Levenshtein distance, abandoning early once it exceeds `budget`.
///
/// Returns `Some(distance)` when within budget, `None` when it cannot be — the
/// `?` at the call site then skips the candidate. Bounded so a 31k-alias scan
/// stays cheap.
fn edit_distance_within(a: &str, b: &str, budget: usize) -> Option<usize> {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > budget {
        return None;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        let mut row_min = cur[0];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            row_min = row_min.min(cur[j]);
        }
        if row_min > budget {
            return None;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    let d = prev[b.len()];
    (d <= budget).then_some(d)
}

/// Parse a chapter:verse reference beginning at `idx` (just past the book).
/// `book_start` is the book's first token index, used for the matched span.
/// Parse a reference, and demote it if any of its numbers came from a numeral
/// table nobody has reviewed.
///
/// **This is a wrapper and not a line inside the parser, on purpose (rule 36).**
/// `parse_reference_inner` has six exits that already demote by other rules; a
/// seventh added next year would be the one that forgot this. Putting the check
/// on the door means it cannot be.
///
/// `matched_text` is the exact span the reference was parsed from — the field the
/// operator is shown — so it is the right thing to test, and it needs no new
/// argument threaded through the FSM.
fn parse_reference(
    tokens: &[&str],
    idx: usize,
    canonical: &str,
    book_start: usize,
    book_ev: BookEvidence,
) -> Option<(RefMatch, usize)> {
    let (mut m, end) = parse_reference_inner(tokens, idx, canonical, book_start, book_ev)?;
    if parsed_an_unreviewed_numeral(&m.matched_text) {
        DetectionMethod::uncertain_number(&mut m);
    }
    Some((m, end))
}

/// Did this span use a numeral word from an `unreviewed` language block?
///
/// The words were HEARD; what they are worth is Relay's guess, and an unchecked
/// guess about a number is the failure `data/numerals.json`'s own header warns
/// about — it does not fail safely, it shows a different verse. So the same cap
/// `UncertainNumber` applies everywhere else applies here: offered to the
/// operator, never fired unattended, at any score and any dial setting.
fn parsed_an_unreviewed_numeral(matched_text: &str) -> bool {
    let n = numerals();
    if n.unreviewed.is_empty() {
        return false;
    }
    normalize(matched_text)
        .split_whitespace()
        .any(|t| n.unreviewed.contains(t))
}

fn parse_reference_inner(
    tokens: &[&str],
    idx: usize,
    canonical: &str,
    book_start: usize,
    book_ev: BookEvidence,
) -> Option<(RefMatch, usize)> {
    let mut i = idx;
    let mut used_kw = false;
    // A REPAIRED book name is a weaker claim than one that matched exactly, and
    // it is charged for exactly like a repaired number is.
    let mut phonetic = book_ev == BookEvidence::Repaired;

    // optional "chapter" — in English or a tier-1 language ("sura ya tatu").
    if let Some(t) = tokens.get(i) {
        if is_chapter_word(t) {
            used_kw = true;
            i += 1;
            i = skip_linkers(tokens, i); // "sura YA tatu"
        }
    }

    // Combined "3:16" token, with optional "-18" range end.
    if let Some((ch, vs, next)) = try_colon_pair(tokens, i) {
        let mut m = make_match(
            canonical, ch, vs, tokens, book_start, next, 0.96, used_kw, false, book_ev,
        );
        let mut end_idx = next;
        if let Some((e, after)) = parse_range_end(tokens, next, vs) {
            m.verse_end = Some(e);
            end_idx = after;
        }
        return Some((m, end_idx));
    }

    // Single-chapter books: "Jude 4" / "Jude verse four" → Jude 1:4. A leading
    // "verse" keyword, or a lone number, means the number is the verse (chapter
    // 1). An explicit second number ("Jude 1 4") is still read as chapter:verse.
    if is_single_chapter(canonical) {
        let mut j = i;
        let mut used_v = false;
        while let Some(t) = tokens.get(j) {
            if is_verse_word(t) {
                used_v = true;
                j += 1;
            } else {
                break;
            }
        }
        if used_v {
            let (verse, after, ph) = parse_number(tokens, j)?;
            return Some((
                make_match(
                    canonical, 1, verse, tokens, book_start, after, 0.95, true, ph, book_ev,
                ),
                after,
            ));
        }
        let (n1, after1, ph1) = parse_number(tokens, i)?;
        let mut k = after1;
        let mut kw2 = used_kw;
        // Same commitment as the general chapter path below: consuming a verse
        // marker here means a number is expected.
        let mut verse_marker = false;
        while let Some(t) = tokens.get(k) {
            if is_verse_word(t) || *t == ":" {
                verse_marker = true;
                if *t != ":" {
                    kw2 = true;
                }
                k += 1;
                k = skip_linkers(tokens, k); // "mstari WA kwanza"
            } else {
                break;
            }
        }
        if let Some((n2, after2, ph2)) = parse_number(tokens, k) {
            // Two numbers → chapter:verse.
            //
            // But BARE digits, with no "chapter"/"verse" keyword and no colon, are a
            // different animal from "Psalm 23 verse 1" and must not be trusted the
            // same way. That form exists for TYPED shorthand ("ps 23 1") — and typed
            // input goes through `manual_fire`, which bypasses the gate entirely, so
            // demoting it here costs the operator nothing.
            //
            // What it fixes is garbled speech. This is a real transcript, from a live
            // rehearsal:
            //
            //     "Verse 1, Psalms 2, 3, 1, Next verse, chapter 2,"
            //
            // It used to score 0.92 and put Psalms 2:3 on the wall, unasked. Nobody
            // SAYS "Psalms two three" — they say "Psalms two verse three". So a bare
            // pair now suggests, and a human decides.
            let base = if kw2 { 0.92 } else { 0.45 };
            let mut m = make_match(
                canonical,
                n1,
                n2,
                tokens,
                book_start,
                after2,
                base,
                kw2,
                ph1 || ph2,
                book_ev,
            );
            if !kw2 {
                // Nobody says "Psalms two three". The score said so already; the
                // method says it in a way the sensitivity dial cannot erase.
                DetectionMethod::uncertain_number(&mut m);
            }
            return Some((m, after2));
        }
        // TRUNCATED MID-REFERENCE, single-chapter twin. "Jude chapter 1 verse" —
        // the marker was consumed and the number never came, so falling through to
        // the lone-number reading would answer with Jude 1:1 at 0.95, which is
        // HIGHER than the 0.88 the general path was handing out. Same defect, worse
        // number. See the guard on the chapter path below for the full reasoning.
        if verse_marker {
            return None;
        }

        // Lone number → verse, chapter 1, with optional range ("Jude 4-6").
        //
        // ── Without a keyword this is the single-chapter twin of the bare whole
        //    chapter below, and it fails the same way ──────────────────────────
        //
        // Every single-chapter book is ALSO an ordinary word or a name in English
        // preaching — Jude, Philemon, Obadiah, and the "John" inside 2 John and
        // 3 John. Followed by a small spoken number, ordinary speech parses as a
        // complete reference. From the live service of 2026-07-26, all auto-fired
        // to a congregation: Jude 1:1, Jude 1:2, 2 John 1:2, 2 John 1:3.
        //
        // So the keyword rule is the same one, applied consistently: "Jude VERSE
        // four" (0.95 above) and "Jude chapter 1 verse 4" state referential
        // intent and still fire. A bare "Jude four" asks a human.
        //
        // This costs more here than it does for a whole chapter, and that is worth
        // being honest about: for a one-chapter book the bare form IS the natural
        // complete reference, so a genuine "Jude four" now needs a click. The
        // trade is accepted because these are 5 books of 66 and rarely preached,
        // while the words themselves are constant in sermon speech — the false
        // positives are frequent and the true positives are not.
        let base = if used_kw { 0.9 } else { 0.45 };
        let mut m = make_match(
            canonical, 1, n1, tokens, book_start, after1, base, used_kw, ph1, book_ev,
        );
        if !used_kw {
            // "jude four men came in" — the CHAPTER was supplied by Relay, not
            // heard. Every single-chapter book is also an ordinary English word.
            DetectionMethod::uncertain_number(&mut m);
        }
        let mut end_idx = after1;
        if let Some((e, after)) = parse_range_end(tokens, after1, n1) {
            m.verse_end = Some(e);
            end_idx = after;
        }
        return Some((m, end_idx));
    }

    // Chapter number.
    let (chapter, after_ch, ph1) = parse_number(tokens, i)?;
    phonetic |= ph1;
    i = after_ch;
    let chapter_was_digit = tokens
        .get(after_ch - 1)
        .map(|t| t.parse::<i64>().is_ok())
        .unwrap_or(false);

    // Colon-combined right after chapter? (e.g. tokens were "3" ":" "16" — rare)
    // optional "verse" / "verses" / "vs" / "v" / ":" separators
    //
    // `verse_marker` records that the grammar COMMITTED to a verse number here. If
    // one never arrives, this is not a whole-chapter reference — it is a reference
    // cut off mid-sentence. See the truncation guard below.
    let mut verse_marker = false;
    // "chapter nine AND verse twenty-four" — the connector is ordinary English and
    // it was throwing the verse away.
    //
    // Measured in a real service (2026-08-23): every reference spoken this way
    // reached the wall as VERSE 1, at 0.88, unattended. "1 Corinthians chapter 9
    // and verse 24" put 1 Corinthians 9:1 in front of a congregation; the same
    // phrasing produced 2 Chronicles 15:1 and 26:1, Proverbs 3:1, Isaiah 61:1,
    // Hebrews 6:1, Genesis 12:1 and Psalms 23:1 in one sitting. Without the "and"
    // the identical sentence parses correctly to 9:24 at 0.95 — so this was one
    // stop-word between the operator and the right verse.
    //
    // Skipped ONLY when a verse word actually follows. "Hebrews 12 and 13" is two
    // chapters and must stay two chapters; "and" before a digit is left alone.
    if tokens.get(i).is_some_and(|t| is_ref_connector(t))
        && tokens.get(i + 1).is_some_and(|t| is_verse_word(t))
    {
        i += 1;
    }
    while let Some(t) = tokens.get(i) {
        if is_verse_word(t) || *t == ":" {
            verse_marker = true;
            if *t != ":" {
                used_kw = true;
            }
            i += 1;
            i = skip_linkers(tokens, i); // "aya TA farko"
        } else {
            break;
        }
    }

    // Verse number — if absent, this is a whole-chapter reference ("Psalm 23"):
    // display verse 1, stage the chapter.
    //
    // ── WITHOUT the "chapter" keyword, this is the weakest shape in the file ──
    //
    // The same reasoning as the bare-pair demotion below, one step further. A bare
    // pair at least has two numbers that line up. This has ONE number after a book
    // name, and Relay answers it by putting verse 1 on a wall — a verse the
    // preacher never asked for specifically.
    //
    // In ordinary preaching that shape is far more often speech than reference.
    // From a live service, every one of these auto-fired to the congregation:
    //
    //     "Matthew, one of the twelve…"      → Matthew 1:1
    //     "…the Lord to the children…"       → John 2:1, 1 Samuel 2:1
    //     garbled window                      → Job 1:1, Job 11:1, Revelation 2:1
    //
    // And it actively DESTROYS good detections. The rolling window is decoded
    // about once a second, so one utterance is parsed repeatedly at varying
    // completeness. A preacher on Hebrews 4:2 produced, five seconds apart:
    //
    //     Hebrews 4:2  conf 0.55   ← correct, the whole reference was heard
    //     Hebrews 4:1  conf 0.83   ← only "Hebrews four" survived that pass
    //
    // The LESS complete parse scored higher and replaced the right verse on the
    // wall. A partial hearing of a reference must never outrank a full one.
    //
    // So a keyword-less whole chapter now asks a human (0.45, below the 0.50
    // default auto bar, above the 0.35 suggest bar) — the operator sees it in the
    // suggestion list and it is one click away. "Psalm CHAPTER 23" keeps its 0.88:
    // the keyword is proof of referential intent, and no one says it by accident.
    // A manual push is unaffected — it bypasses the gate entirely — and the
    // sensitivity dial still governs all of it.
    let Some((verse, after_vs, ph2)) = parse_number(tokens, i) else {
        // ── TRUNCATED MID-REFERENCE ─────────────────────────────────────────
        //
        // "…John chapter 3 verse" — the speaker said a verse number and the
        // TRANSCRIPT STOPPED BEFORE IT. There is no whole-chapter reading to fall
        // back to: a dangling verse marker is proof a verse was coming, which is
        // the exact opposite of the referential intent the keyword bonus below
        // rewards. Answering it with verse 1 invents a verse nobody asked for.
        //
        // This is not hypothetical, and it is not rare. Detection runs on every
        // PARTIAL hypothesis (DECISIONS.md), and the STT window is re-decoded
        // about once a second — so every reference anyone speaks is parsed at
        // least once in a state where the number has not arrived yet. Measured
        // through `stt::bench::engine_shootout`, one clip citing two verses
        // auto-fired John 3:1 and Romans 8:1 to the wall on the way to the right
        // answer. The congregation sees the wrong verse flash, then the right one.
        //
        // Worse, the marker was ACTIVELY PROMOTING the mistake. A bare "Romans 8"
        // scores 0.45 and asks a human — but the dangling "verse" set `used_kw`,
        // which bought the truncated parse 0.88 and a straight path to the screen.
        // The most incomplete reading of the sentence outranked every other.
        //
        // So the parse fails. `detect_direct`'s scanner advances a token and
        // carries on, the next second's window carries the whole reference, and
        // that one fires. This is the same principle the keyword-less demotion
        // below rests on, stated one step harder: A PARTIAL HEARING OF A REFERENCE
        // MUST NEVER OUTRANK A FULL ONE.
        if verse_marker {
            return None;
        }

        // Before treating this as a whole chapter: is it even a chapter of this
        // book? "john 663" is not John chapter 663 — John has 21 — it is whisper
        // running "six sixty-three" together. Repair it to 6:63 when exactly one
        // reading is real. See `split_run_into_chapter_verse`.
        if let Some((c, v)) = split_run_into_chapter_verse(canonical, chapter) {
            // A REPAIRED reference, so it is charged like one: `phonetic` costs
            // confidence downstream, and the run being unreadable as a chapter is
            // hard evidence the number was misheard.
            //
            // R4-01: it used to be charged 0.83 — MORE than the 0.45 given to the
            // keyword-less reading it replaced, so a number the parser could not
            // read as a chapter made Relay more confident rather than less. It is
            // now scored in the same band as that reading, and marked, because
            // splitting a run the book cannot support is a guess about what the
            // speaker said, not a fact about the book.
            let mut m = make_match(
                canonical, c, v, tokens, book_start, after_ch, 0.45, used_kw, true, book_ev,
            );
            DetectionMethod::uncertain_number(&mut m);
            return Some((m, after_ch));
        }
        let base = if used_kw { 0.88 } else { 0.45 };
        let mut m = make_match(
            canonical, chapter, 1, tokens, book_start, after_ch, base, false, phonetic, book_ev,
        );
        if !used_kw {
            // "Matthew, one of the twelve disciples" — the VERSE was supplied.
            DetectionMethod::uncertain_number(&mut m);
        }
        m.whole_chapter = true;
        return Some((m, after_ch));
    };
    phonetic |= ph2;
    let verse_was_digit = tokens
        .get(after_vs - 1)
        .map(|t| t.parse::<i64>().is_ok())
        .unwrap_or(false);

    // Optional range end ("John 3:16-18", "Psalm 23 verses 1 to 6"). Resolved
    // BEFORE scoring, because whether a following number was absorbed as a range
    // end is exactly what decides if a *leftover* one is a garble signal (below).
    let range = parse_range_end(tokens, after_vs, verse);
    let end_idx = range.map_or(after_vs, |(_, after)| after);

    // BARE DIGITS with no "chapter"/"verse" keyword ("psalm 23 1", "Acts 2, 1.").
    //
    // Preachers really do say these — "Romans eight one", "Psalm 23, 1" — and ASR
    // renders the pauses as commas and full stops, which `normalize` strips. So this
    // form has to reach the congregation, or the product misses ordinary preaching.
    //
    // But it is also the shape of garbled speech. A real transcript, from a live
    // rehearsal:
    //
    //     "Verse 1, Psalms 2, 3, 1, Next verse, chapter 2,"
    //
    // scored 0.92 and put Psalms 2:3 on the wall, unasked.
    //
    // What separates the two is not confidence — the parser sees the same shape —
    // it is the LEFTOVER number. "Psalm 23 1" ends cleanly; "Psalms 2, 3, 1" parses
    // 2:3 and leaves a stray "1" that no range could absorb (a range end must be
    // >= the verse). A trailing loose number means the numbers did not line up, and
    // that is the case that stays a suggestion.
    //
    // Note a bare pair off a REPAIRED book name lands at 0.55 - 0.06 = 0.49, still
    // under the default auto-fire line: a misheard book plus loose digits always
    // asks a human. And the sensitivity dial still governs all of it — a cautious
    // install (low dial, auto-fire 0.90) demotes bare pairs exactly as before.
    let bare_digits = chapter_was_digit && verse_was_digit && !used_kw;
    let trailing_number = parse_number(tokens, end_idx).is_some();
    // R4-02: the leftover number is the garble signal, and it does not care how
    // whisper spelled it. This was gated on `bare_digits`, so it only ever saw
    // `23`-style tokens — while whisper writes spoken numbers as WORDS on a large
    // share of accented decodes, which is this product's entire market. The
    // identical garble, spelled out, scored 0.90 and went straight to the
    // congregation. A KEYWORD still exempts it: "Psalm 23 verse 1" states
    // referential intent, and nobody says that by accident.
    let garbled = trailing_number && !used_kw;
    let base = if garbled {
        0.45 // the garble shape — reaches the operator, never the congregation
    } else if bare_digits {
        0.55 // above the default auto-fire (0.50), still dial-controllable
    } else if chapter_was_digit && verse_was_digit {
        0.92
    } else {
        0.90
    };
    let mut m = make_match(
        canonical, chapter, verse, tokens, book_start, after_vs, base, used_kw, phonetic, book_ev,
    );
    if garbled {
        DetectionMethod::uncertain_number(&mut m);
    }
    if let Some((e, _)) = range {
        m.verse_end = Some(e);
    }
    Some((m, end_idx))
}

#[allow(clippy::too_many_arguments)]
fn make_match(
    canonical: &str,
    chapter: i64,
    verse: i64,
    tokens: &[&str],
    book_start: usize,
    end: usize,
    base: f32,
    used_kw: bool,
    phonetic: bool,
    book_ev: BookEvidence,
) -> RefMatch {
    let mut conf = base;
    if used_kw {
        conf = conf.max(0.95);
    }
    if phonetic {
        conf -= 0.06;
    }
    // Floor is 0.30, NOT 0.50. It used to be 0.50 — which is exactly the
    // auto-fire threshold — so the weakest possible direct match still went
    // straight to the congregation's screen. Nothing could be demoted to a
    // suggestion even when the parser was barely confident, which made the whole
    // confidence scale decorative below that line.
    let conf = conf.clamp(0.30, 0.99);
    RefMatch {
        reference: VerseRef {
            book: canonical.to_string(),
            chapter,
            verse,
        },
        verse_end: None,
        whole_chapter: false,
        confidence: conf,
        // A guessed book name is not a heard one. This is the structural half of
        // the P0 repair: the router refuses to auto-fire `Repaired` whatever the
        // confidence says, so it cannot be undone by a threshold, by the operator's
        // sensitivity dial, or by the calibrator drifting. The 0.06 `phonetic`
        // penalty below is left in place because it still orders suggestions
        // sensibly — but it is no longer load-bearing, and it never should have been.
        // A repair is never rescued by grammar; an ordinary word is. See
        // `BookEvidence` for why those two are not the same question.
        method: match book_ev {
            BookEvidence::Repaired => DetectionMethod::UncertainBook,
            BookEvidence::OrdinaryWord if !used_kw => DetectionMethod::UncertainBook,
            _ => DetectionMethod::Direct,
        },
        matched_text: tokens[book_start..end].join(" "),
        // Set by `detect_direct`, which is the only place that knows where the
        // scan actually stopped once ranges have been absorbed.
        at_tail: false,
    }
}

/// After a parsed verse at `idx`, look for an optional range end: an explicit
/// connector ("John 3:16 to 18") or an immediate bare number (hyphen ranges like
/// "3:16-18" tokenize to adjacent numbers, the hyphen becoming whitespace).
/// Returns (end, next_index) only when the span is sane (`end >= start`).
fn parse_range_end(tokens: &[&str], idx: usize, start: i64) -> Option<(i64, usize)> {
    let mut j = idx;
    let mut connector = false;
    while let Some(t) = tokens.get(j) {
        if matches!(*t, "to" | "through" | "thru" | "til" | "until") {
            connector = true;
            j += 1;
        } else {
            break;
        }
    }
    // Without a connector word, only an IMMEDIATELY adjacent number counts (the
    // hyphen-range case) — otherwise a following number is a separate reference.
    if !connector && j != idx {
        return None;
    }
    // ...and that adjacent-number case exists for ONE reason: "3:16-18" tokenizes
    // to adjacent numbers because `normalize` turns the hyphen into whitespace.
    // A hyphen cannot survive into spelled-out words, so a bare adjacent number
    // WORD is not a recovered range — it is a third number in a row, which is the
    // garble shape (R4-02). "romans eight one two" was being read as Romans 8:1-2
    // and auto-fired; the leftover "two" could not be seen as leftover because the
    // range had already swallowed it.
    if !connector && !tokens.get(j).is_some_and(|t| t.parse::<i64>().is_ok()) {
        return None;
    }
    let (end, after, _) = parse_number(tokens, j)?;
    if end >= start && end - start <= 200 {
        Some((end, after))
    } else {
        None
    }
}

/// A single token of the form "3:16" (both sides numeric) → (chapter, verse).
fn try_colon_pair(tokens: &[&str], idx: usize) -> Option<(i64, i64, usize)> {
    let tok = tokens.get(idx)?;
    let (a, b) = tok.split_once(':')?;
    let ch = a.parse::<i64>().ok()?;
    let vs = b.parse::<i64>().ok()?;
    Some((ch, vs, idx + 1))
}

#[derive(Clone, Copy)]
enum NumWord {
    Ones(i64), // 1-9
    Teen(i64), // 10-19
    Ten(i64),  // 20,30,...,90
    Hundred,
    /// Swahili "mia", Hausa "ɗari" — the multiplier comes AFTER ("mia mbili" =
    /// 200, not 102). See parse_number.
    HundredPost,
    /// A word that is the WHOLE number and joins to nothing either side — Yorùbá
    /// `kẹrìndínlógún` is 16 outright. It completes the FSM where it stands, so
    /// it can neither absorb a following word nor be absorbed by a preceding one.
    Standalone(i64),
}

/// Spoken numbers in the tier-1 languages, from `data/numerals.json`.
///
/// Data, not Rust, for the same reason as the book names: a wrong numeral does
/// not fail safely — it silently shows a DIFFERENT VERSE. If `tisa` were mapped
/// to 8 instead of 9, nobody would find out until a service. A native speaker can
/// fix a number in a one-line pull request without touching this file.
///
/// The GRAMMAR stays here; only the WORDS live in the data.
pub struct Numerals {
    pub ones: HashMap<String, i64>,
    pub tens: HashMap<String, i64>,
    /// A word that IS a whole number and combines with nothing — Yorùbá, whose
    /// numerals are vigesimal and subtractive (16 is `ẹrìndínlógún`, "four taken
    /// from twenty", one word). There is no tens-plus-ones for a state machine to
    /// walk, so a `standalone` word completes the number where it stands and a
    /// wrong entry can never alter a neighbouring one.
    pub standalone: HashMap<String, i64>,
    /// Every numeral word that came from a language block marked `unreviewed`.
    ///
    /// **A reference parsed with one of these is capped at `Suggest`.** The file's
    /// own warning is that a wrong numeral does not fail safely — it silently
    /// shows a different verse — and nobody has checked the Yorùbá. That is the
    /// same doubt `UncertainNumber` already exists for (rule 10): the words were
    /// heard, what they MEAN is Relay's guess. Emptying this set lifts the cap,
    /// which is what a native-speaker review is for.
    pub unreviewed: HashSet<String>,
    pub hundred_post: HashSet<String>,
    pub connectors: HashSet<String>,
    pub chapter_words: HashSet<String>,
    pub verse_words: HashSet<String>,
    pub linkers: HashSet<String>,
}

static NUMERALS: OnceLock<Numerals> = OnceLock::new();

fn numerals() -> &'static Numerals {
    NUMERALS.get_or_init(|| {
        const RAW: &str = include_str!("../data/numerals.json");
        let mut n = Numerals {
            ones: HashMap::new(),
            tens: HashMap::new(),
            standalone: HashMap::new(),
            unreviewed: HashSet::new(),
            hundred_post: HashSet::new(),
            connectors: HashSet::new(),
            chapter_words: HashSet::new(),
            verse_words: HashSet::new(),
            linkers: HashSet::new(),
        };
        let Ok(doc) = serde_json::from_str::<serde_json::Value>(RAW) else {
            eprintln!("detection: numerals.json is not valid JSON — in-language numbers disabled");
            return n;
        };
        let Some(langs) = doc.as_object() else {
            return n;
        };
        for (lang, spec) in langs {
            if lang.starts_with('_') {
                continue;
            }
            let Some(spec) = spec.as_object() else {
                continue;
            };
            let nums = |key: &str, into: &mut HashMap<String, i64>| {
                if let Some(m) = spec.get(key).and_then(|v| v.as_object()) {
                    for (w, v) in m {
                        if let Some(v) = v.as_i64() {
                            // normalize() folds the hooked letters and diacritics,
                            // so `ɗaya` and `daya` become one key.
                            into.insert(normalize(w), v);
                        }
                    }
                }
            };
            nums("ones", &mut n.ones);
            nums("tens", &mut n.tens);
            nums("standalone", &mut n.standalone);
            // Record the words, not the language: by the time a reference has been
            // parsed the language is gone and only the tokens are left, and the
            // token is what the demotion has to key on.
            if spec.get("unreviewed").and_then(|v| v.as_bool()) == Some(true) {
                for key in ["ones", "tens", "standalone"] {
                    if let Some(m) = spec.get(key).and_then(|v| v.as_object()) {
                        for w in m.keys() {
                            n.unreviewed.insert(normalize(w));
                        }
                    }
                }
            }
            let words = |key: &str, into: &mut HashSet<String>| {
                for w in spec
                    .get(key)
                    .and_then(|v| v.as_array())
                    .into_iter()
                    .flatten()
                {
                    if let Some(w) = w.as_str() {
                        into.insert(normalize(w));
                    }
                }
            };
            words("hundred_post", &mut n.hundred_post);
            words("connectors", &mut n.connectors);
            words("chapter_words", &mut n.chapter_words);
            words("verse_words", &mut n.verse_words);
            words("linkers", &mut n.linkers);
        }
        n
    })
}

/// "chapter" in any tier-1 language: Swahili "sura", Hausa "sura"/"babi".
fn is_chapter_word(t: &str) -> bool {
    matches!(t, "chapter" | "chap" | "ch") || numerals().chapter_words.contains(t)
}

/// A word that can sit between a chapter number and the "verse" keyword without
/// meaning anything: "chapter 9 AND verse 24", "chapter 9, verse 24".
///
/// Deliberately tiny. This is only ever consulted when the very next token is a
/// verse word, so it cannot swallow a connector that joins two numbers.
fn is_ref_connector(t: &str) -> bool {
    matches!(t, "and" | "," | "&")
}

/// "verse" in any tier-1 language: Swahili "mstari"/"aya", Hausa "aya".
fn is_verse_word(t: &str) -> bool {
    matches!(t, "verse" | "verses" | "vs" | "v") || numerals().verse_words.contains(t)
}

/// Grammatical glue between a keyword and its number — "sura YA tatu", "mstari WA
/// kwanza", "aya TA farko". Carries no meaning; skipped only when it sits directly
/// between a chapter/verse word and its number, never anywhere else, because "ya"
/// and "na" are among the most common words in Swahili and would otherwise swallow
/// half a sentence.
fn skip_linkers(tokens: &[&str], mut i: usize) -> usize {
    while let Some(t) = tokens.get(i) {
        if numerals().linkers.contains(*t) {
            i += 1;
        } else {
            break;
        }
    }
    i
}

/// Connectors that glue a spoken number together without carrying a value.
///
/// English "and" ("one hundred AND thirteen"), Swahili "na" ("kumi NA tatu"),
/// Hausa "sha" (teens: "goma SHA uku") and "da" ("ashirin DA uku").
///
/// Only ever skipped when a number word genuinely follows, so "one hundred and
/// God is good" does not silently swallow the "and".
fn word_is_and(t: &str) -> bool {
    t == "and" || numerals().connectors.contains(t)
}

/// Parse a spoken/written number starting at `start`. Returns
/// (value, next_index, phonetic_correction_applied) or None.
///
/// A finite state walk, so "three sixteen" parses as 3 (stopping before
/// "sixteen", which is a separate verse) while "twenty eight" → 28 and
/// "one hundred nineteen" → 119.
///
/// ## Swahili and Hausa put the hundred MULTIPLIER AFTER the hundred word
///
/// This is the one place their grammar diverges from English, and it is not
/// cosmetic:
///
/// ```text
///   mia moja  = 100   (literally "hundred one")   NOT 101
///   ɗari biyu = 200                               NOT 102
/// ```
///
/// English puts the multiplier first ("two hundred"). So the English parser, run
/// on Swahili, would read "mia mbili" as 100 + 2 = **102** — and put Psalm 102 on
/// the wall when the preacher said Psalm 200. `HundredPost` exists for exactly
/// that, and a connector disambiguates the two readings: "mia moja" (no connector)
/// is 1×100, while "mia na tatu" (connector) is 100+3.
fn parse_number(tokens: &[&str], start: usize) -> Option<(i64, usize, bool)> {
    // Bare digits: take one token.
    if let Some(t) = tokens.get(start) {
        if let Ok(n) = t.parse::<i64>() {
            return Some((n, start + 1, false));
        }
    }

    enum St {
        Start,
        AfterOnesOrTeen,
        AfterTen,
        AfterHundred,
        AfterHundredTen,
        /// Just saw a Swahili/Hausa hundred word ("mia", "ɗari"). The MULTIPLIER
        /// may still be coming — "mia mbili" is 200, not 102.
        AfterHundredPost,
        Complete,
    }
    let mut state = St::Start;
    let mut value = 0i64;
    let mut idx = start;
    let mut consumed = 0;
    let mut phonetic = false;
    // Did a connector immediately precede this word? It is what tells "mia moja"
    // (1×100) apart from "mia na tatu" (100+3) — see the fn doc.
    let mut saw_connector = false;

    while let Some(&raw) = tokens.get(idx) {
        if tokens[idx].parse::<i64>().is_ok() {
            break; // a digit doesn't extend a spoken number
        }
        // "one hundred AND thirteen" = 113. The FSM used to break on "and" and
        // return 100 — so "sam one hundred and thirteen verse one" auto-fired
        // PSALM 100:1. A wrong verse, on the wall, at full confidence.
        //
        // This is not an edge case for this market: Nigerian, Kenyan and British
        // English all say "a hundred AND thirteen" as the default form. American
        // English drops it, which is presumably why it was never noticed.
        //
        // Only skipped when a number word genuinely follows, so "one hundred and
        // God is good" doesn't silently swallow the "and".
        // Connectors carry no value, they just glue: English "one hundred AND
        // thirteen", Swahili "kumi NA tatu", Hausa "goma SHA uku" / "ashirin DA
        // uku". The FSM used to BREAK on "and" and return what it had, so "sam one
        // hundred and thirteen verse one" auto-fired PSALM 100:1 — a wrong verse,
        // on the wall, at full confidence. (Nigerian, Kenyan and British English
        // all say "a hundred AND thirteen" by default. American English drops it,
        // which is presumably why it was never noticed.)
        //
        // Only skipped when a number word genuinely follows, so "one hundred and
        // God is good" doesn't silently swallow the "and". And never from Start —
        // a bare "na"/"da" is an ordinary word, not the beginning of a number.
        if word_is_and(raw)
            && !matches!(state, St::Start)
            && tokens
                .get(idx + 1)
                .map(|t| classify_num_word(correct_homophone(t).0).is_some())
                .unwrap_or(false)
        {
            idx += 1;
            consumed += 1;
            saw_connector = true;
            continue;
        }
        let (word, ph) = correct_homophone(raw);
        let Some(nw) = classify_num_word(word) else {
            break;
        };
        let next = match (&state, nw) {
            (St::Start, NumWord::Ones(v)) => {
                value = v;
                St::AfterOnesOrTeen
            }
            (St::Start, NumWord::Teen(v)) => {
                value = v;
                St::AfterOnesOrTeen
            }
            (St::Start, NumWord::Ten(v)) => {
                value = v;
                St::AfterTen
            }
            // The whole number, in one word. Straight to Complete: it may not
            // combine, in either direction. Anywhere but Start it ends the run,
            // which the catch-all below already does.
            (St::Start, NumWord::Standalone(v)) => {
                value = v;
                St::Complete
            }
            (St::Start, NumWord::Hundred) => {
                value = 100;
                St::AfterHundred
            }
            // "mia" / "ɗari" alone is 100; a multiplier may follow.
            (St::Start, NumWord::HundredPost) => {
                value = 100;
                St::AfterHundredPost
            }
            // "mia MBILI" = 200. No connector → this is the multiplier, not an
            // addend. Getting this wrong shows Psalm 102 for Psalm 200.
            (St::AfterHundredPost, NumWord::Ones(v)) if !saw_connector => {
                value = v * 100;
                St::AfterHundred
            }
            // "mia NA tatu" = 103. A connector means it is an addend after all.
            (St::AfterHundredPost, NumWord::Ones(v)) => {
                value += v;
                St::Complete
            }
            (St::AfterHundredPost, NumWord::Teen(v)) => {
                value += v;
                St::Complete
            }
            // "ɗari da GOMA sha uku" = 113.
            (St::AfterHundredPost, NumWord::Ten(v)) => {
                value += v;
                St::AfterHundredTen
            }
            (St::AfterTen, NumWord::Ones(v)) => {
                value += v;
                St::Complete
            }
            (St::AfterTen, NumWord::Hundred) => {
                value *= 100;
                St::AfterHundred
            }
            (St::AfterOnesOrTeen, NumWord::Hundred) => {
                value *= 100;
                St::AfterHundred
            }
            (St::AfterHundred, NumWord::Ones(v)) | (St::AfterHundred, NumWord::Teen(v)) => {
                value += v;
                St::Complete
            }
            (St::AfterHundred, NumWord::Ten(v)) => {
                value += v;
                St::AfterHundredTen
            }
            (St::AfterHundredTen, NumWord::Ones(v)) => {
                value += v;
                St::Complete
            }
            _ => break, // can't grammatically extend → stop here
        };
        phonetic |= ph;
        consumed += 1;
        idx += 1;
        state = next;
        saw_connector = false; // only ever applies to the word directly after it
        if matches!(state, St::Complete) {
            break;
        }
    }

    if consumed == 0 {
        None
    } else {
        Some((value, idx, phonetic))
    }
}

/// Map common ASR homophones to their number word. Conservative set — only
/// mishears unlikely to collide with ordinary sermon speech.
fn correct_homophone(word: &str) -> (&str, bool) {
    match word {
        "free" | "tree" => ("three", true),
        "fore" => ("four", true),
        "ate" => ("eight", true),
        "won" => ("one", true),
        other => (other, false),
    }
}

fn classify_num_word(w: &str) -> Option<NumWord> {
    let v = match w {
        "one" => NumWord::Ones(1),
        "two" => NumWord::Ones(2),
        "three" => NumWord::Ones(3),
        "four" => NumWord::Ones(4),
        "five" => NumWord::Ones(5),
        "six" => NumWord::Ones(6),
        "seven" => NumWord::Ones(7),
        "eight" => NumWord::Ones(8),
        "nine" => NumWord::Ones(9),
        "ten" => NumWord::Teen(10),
        "eleven" => NumWord::Teen(11),
        "twelve" => NumWord::Teen(12),
        "thirteen" => NumWord::Teen(13),
        "fourteen" => NumWord::Teen(14),
        "fifteen" => NumWord::Teen(15),
        "sixteen" => NumWord::Teen(16),
        "seventeen" => NumWord::Teen(17),
        "eighteen" => NumWord::Teen(18),
        "nineteen" => NumWord::Teen(19),
        "twenty" => NumWord::Ten(20),
        "thirty" => NumWord::Ten(30),
        "forty" => NumWord::Ten(40),
        "fifty" => NumWord::Ten(50),
        "sixty" => NumWord::Ten(60),
        "seventy" => NumWord::Ten(70),
        "eighty" => NumWord::Ten(80),
        "ninety" => NumWord::Ten(90),
        "hundred" => NumWord::Hundred,
        // Tier-1 languages. Words come from data/numerals.json so a native
        // speaker can correct a number without touching Rust — a wrong numeral
        // does not fail safely, it silently shows a different verse.
        w => {
            let n = numerals();
            if let Some(&v) = n.standalone.get(w) {
                NumWord::Standalone(v)
            } else if let Some(&v) = n.ones.get(w) {
                NumWord::Ones(v)
            } else if let Some(&v) = n.tens.get(w) {
                NumWord::Ten(v)
            } else if n.hundred_post.contains(w) {
                NumWord::HundredPost
            } else {
                return None;
            }
        }
    };
    Some(v)
}

// ===== Context memory (PROMPT.md Phase 9) =====

/// Tracks the current on-screen verse so a bare "verse 4" resolves against the
/// last book+chapter, and "next"/"back" step from it. Pure state — no IO. Fed
/// by whatever verse actually fires.
#[derive(Debug, Clone, Default)]
pub struct ContextMemory {
    current: Option<VerseRef>,
    /// Inclusive last verse of the passage being walked, if the current verse is
    /// part of a multi-verse range or whole chapter. `next` stops here.
    span_end: Option<i64>,
}

impl ContextMemory {
    /// Record a single verse currently shown (clears any active passage span).
    pub fn note(&mut self, r: &VerseRef) {
        self.current = Some(r.clone());
        self.span_end = None;
    }

    /// Record the anchor of a multi-verse passage (range or whole chapter). `end`
    /// is the last verse to walk to; None means step until the chapter runs out.
    pub fn note_passage(&mut self, r: &VerseRef, end: Option<i64>) {
        self.current = Some(r.clone());
        self.span_end = end;
    }

    /// Move the current verse within an active passage WITHOUT clearing the span
    /// — used by "next"/"back" so the range end still bounds the walk.
    pub fn advance(&mut self, r: &VerseRef) {
        self.current = Some(r.clone());
    }

    /// Forget the passage entirely.
    ///
    /// Called when something that is NOT scripture takes the wall — a song, a
    /// notice, a picture, a countdown. Until this existed, `current` was written
    /// only by scripture fires and cleared by nothing, so a passage stayed armed
    /// for the rest of the service: `next` would walk a reading the congregation
    /// stopped looking at twenty minutes earlier and report that it had FIRED,
    /// which is true of the wall and false of the sermon.
    pub fn forget(&mut self) {
        self.current = None;
        self.span_end = None;
    }

    /// Resolve a bare verse number against the current passage, if any.
    pub fn resolve_bare_verse(&self, verse: i64) -> Option<VerseRef> {
        self.current.as_ref().map(|c| VerseRef {
            book: c.book.clone(),
            chapter: c.chapter,
            verse,
        })
    }

    pub fn current(&self) -> Option<&VerseRef> {
        self.current.as_ref()
    }

    /// The next verse in the passage (verse + 1), if a current exists and we
    /// haven't reached an explicit range end.
    pub fn next_verse(&self) -> Option<VerseRef> {
        self.current.as_ref().and_then(|c| {
            if let Some(end) = self.span_end {
                if c.verse >= end {
                    return None; // reached the end of an explicit range
                }
            }
            Some(VerseRef {
                book: c.book.clone(),
                chapter: c.chapter,
                verse: c.verse + 1,
            })
        })
    }

    /// The previous verse (verse - 1), if a current exists and verse > 1.
    pub fn prev_verse(&self) -> Option<VerseRef> {
        self.current.as_ref().and_then(|c| {
            (c.verse > 1).then(|| VerseRef {
                book: c.book.clone(),
                chapter: c.chapter,
                verse: c.verse - 1,
            })
        })
    }
}

// ===== Topical concordance & cross-references (Phase A: A3/A4) =============

/// A topical theme: spoken trigger keywords → a ranked list of reference
/// strings. Offline, curated. Surfaces related scripture by theme even when the
/// preacher doesn't quote a verse directly (A4), and doubles as the cross-
/// reference source for a fired verse's theme (A3).
struct Theme {
    name: &'static str,
    keywords: &'static [&'static str],
    refs: &'static [&'static str],
}

#[rustfmt::skip]
const THEMES: &[Theme] = &[
    Theme { name: "Fear & Anxiety", keywords: &["afraid","fear","fearful","anxious","anxiety","worry","worried","scared","terrified","nervous","panic","dread"], refs: &["Isaiah 41:10","Philippians 4:6-7","John 14:27","2 Timothy 1:7","Psalm 56:3","Joshua 1:9"] },
    Theme { name: "Trouble & Storms", keywords: &["trouble","storm","storms","trial","trials","suffering","suffer","hardship","crisis","struggle","struggling","overwhelmed"], refs: &["Psalm 46:1","John 16:33","Romans 8:28","Isaiah 43:2","2 Corinthians 4:17","Psalm 34:18"] },
    Theme { name: "Refuge & Protection", keywords: &["refuge","shelter","protect","protection","safe","safety","fortress","shield","stronghold"], refs: &["Psalm 46:1","Psalm 91:1-2","Psalm 18:2","Nahum 1:7","Proverbs 18:10","Psalm 27:5"] },
    Theme { name: "Peace & Rest", keywords: &["peace","peaceful","rest","still","calm","quiet","weary","tired","burden","burdened"], refs: &["John 14:27","Matthew 11:28","Psalm 46:10","Isaiah 26:3","Philippians 4:7","Psalm 23:2"] },
    Theme { name: "Strength & Endurance", keywords: &["strength","strong","strengthen","weak","weakness","power","endure","persevere","overcome"], refs: &["Isaiah 40:31","Philippians 4:13","2 Corinthians 12:9","Psalm 46:1","Nehemiah 8:10","Ephesians 6:10"] },
    Theme { name: "Faith & Trust", keywords: &["faith","faithful","believe","believed","trust","trusting","doubt","doubting","confidence"], refs: &["Hebrews 11:1","Proverbs 3:5-6","Mark 11:24","2 Corinthians 5:7","Romans 10:17","Matthew 17:20"] },
    Theme { name: "Hope & Future", keywords: &["hope","hopeful","future","plans","tomorrow","expectation","promise","promises"], refs: &["Jeremiah 29:11","Romans 15:13","Romans 8:24-25","Lamentations 3:22-23","Psalm 39:7","Hebrews 6:19"] },
    Theme { name: "Love", keywords: &["love","loved","loves","loving","beloved","compassion","kindness"], refs: &["1 Corinthians 13:4-7","John 3:16","1 John 4:19","Romans 5:8","1 John 4:8","Romans 8:38-39"] },
    Theme { name: "Joy & Praise", keywords: &["joy","joyful","rejoice","glad","gladness","celebrate","praise","worship","thanksgiving","thankful","grateful"], refs: &["Psalm 16:11","Nehemiah 8:10","Philippians 4:4","Psalm 100:4","James 1:2","Psalm 30:5"] },
    Theme { name: "Forgiveness & Grace", keywords: &["forgive","forgiven","forgiveness","sin","sins","mercy","merciful","grace","guilt","repent","repentance"], refs: &["1 John 1:9","Ephesians 4:32","Psalm 103:12","Colossians 3:13","Romans 5:8","Micah 7:18"] },
    Theme { name: "Salvation", keywords: &["salvation","saved","save","saviour","savior","eternal","cross","gospel","redeemed"], refs: &["Ephesians 2:8-9","Romans 10:9","John 3:16","Acts 4:12","Titus 3:5","Romans 6:23"] },
    Theme { name: "Comfort & Grief", keywords: &["comfort","grief","grieve","grieving","mourn","mourning","sorrow","death","loss","brokenhearted","heartbroken","tears"], refs: &["Psalm 23:4","Matthew 5:4","Revelation 21:4","2 Corinthians 1:3-4","Psalm 34:18","Psalm 147:3"] },
    Theme { name: "Provision & Needs", keywords: &["provide","provision","need","needs","supply","money","finances","lack","hunger"], refs: &["Philippians 4:19","Matthew 6:33","Psalm 23:1","Malachi 3:10","Matthew 6:26","2 Corinthians 9:8"] },
    Theme { name: "Guidance & Direction", keywords: &["guide","guidance","direction","lead","leading","path","decision","wisdom","discern","purpose","calling"], refs: &["Proverbs 3:5-6","Psalm 119:105","Isaiah 30:21","Jeremiah 29:11","James 1:5","Psalm 32:8"] },
    Theme { name: "Prayer", keywords: &["pray","prayer","praying","intercede","petition","seek","knock"], refs: &["Philippians 4:6","1 Thessalonians 5:17","James 5:16","Matthew 7:7","Jeremiah 33:3","Matthew 6:9"] },
    Theme { name: "God's Faithfulness", keywords: &["faithfulness","forsake","unchanging","covenant","steadfast","forever","everlasting"], refs: &["Lamentations 3:22-23","Deuteronomy 31:6","Hebrews 13:5","Joshua 1:9","Psalm 100:5","2 Timothy 2:13"] },
    Theme { name: "New Life & Identity", keywords: &["identity","created","creation","transform","transformed","chosen","fearfully","wonderfully"], refs: &["2 Corinthians 5:17","Ephesians 2:10","1 Peter 2:9","Psalm 139:14","Galatians 2:20","Romans 12:2"] },
    Theme { name: "Light & Truth", keywords: &["light","darkness","dark","truth","lamp","shine","reveal"], refs: &["John 8:12","Psalm 119:105","John 1:5","Matthew 5:14","John 14:6","1 John 1:5"] },
];

/// A related-scripture suggestion: the matched theme and its references (already
/// parsed, so ranges carry through), with the anchor verse removed.
#[derive(Debug, Clone)]
pub struct RelatedSuggestion {
    pub theme: String,
    pub refs: Vec<RefMatch>,
}

/// Suggest related scripture for a transcript window by topical keyword match
/// (A4), also usable to cross-reference a fired verse's theme (A3). `exclude`
/// drops the currently-shown verse. Returns at most `max` refs, or None when no
/// theme is clearly indicated. Pure and offline.
pub fn suggest_related(
    text: &str,
    exclude: Option<&VerseRef>,
    max: usize,
) -> Option<RelatedSuggestion> {
    let norm = format!(" {} ", normalize(text));
    let mut best: Option<(&Theme, u32)> = None;
    for th in THEMES {
        let mut score = 0u32;
        for kw in th.keywords {
            if norm.contains(&format!(" {kw} ")) {
                // Multi-word keywords are stronger signal than single words.
                score += if kw.contains(' ') { 2 } else { 1 };
            }
        }
        if score > 0 && best.map(|(_, s)| score > s).unwrap_or(true) {
            best = Some((th, score));
        }
    }
    let (theme, _) = best?;
    let ex = exclude.map(|e| (e.book.as_str(), e.chapter, e.verse));
    let mut refs = Vec::new();
    for r in theme.refs {
        let Some(m) = detect_direct(r).into_iter().next() else {
            continue; // spelling that doesn't resolve → skip, never break
        };
        let vr = &m.reference;
        if Some((vr.book.as_str(), vr.chapter, vr.verse)) == ex {
            continue;
        }
        refs.push(m);
        if refs.len() >= max {
            break;
        }
    }
    if refs.is_empty() {
        None
    } else {
        Some(RelatedSuggestion {
            theme: theme.name.into(),
            refs,
        })
    }
}

/// Operator voice navigation commands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NavCommand {
    Next,
    Previous,
}

/// Detect a spoken navigation command in a short utterance ("next", "back",
/// "previous", "next verse"). Only fires on short utterances so it doesn't
/// trigger mid-sermon. Returns None for anything longer or unrelated.
pub fn detect_command(text: &str) -> Option<NavCommand> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    if tokens.is_empty() || tokens.len() > 5 {
        return None;
    }
    let has = |w: &str| tokens.contains(&w);
    if has("next") {
        Some(NavCommand::Next)
    } else if has("back") || has("previous") || has("prev") {
        Some(NavCommand::Previous)
    } else {
        None
    }
}

/// Detect a spoken "clear / blackout the screen" command (Phase D3). Deliberately
/// conservative — "clear" and "blank" alone are common sermon words, so they only
/// fire when paired with a screen object; "blackout" is unambiguous on its own.
/// Short-utterance guarded so it never triggers on prose.
pub fn detect_clear(text: &str) -> bool {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    if tokens.is_empty() || tokens.len() > 5 {
        return false;
    }
    let joined = format!(" {} ", tokens.join(" "));
    let has = |w: &str| tokens.contains(&w);
    let screen = has("screen") || has("screens");
    has("blackout")
        || joined.contains(" black out ")
        || (has("clear") && screen)
        || (has("blank") && screen)
        || joined.contains(" take it down ")
        || joined.contains(" take that down ")
        || joined.contains(" take it off ")
}

/// Find bare verse references ("verse 4", "verse twenty-eight") in `text`.
/// Returns the verse numbers; the caller resolves them via ContextMemory.
/// The reference this window names ITSELF, for a bare verse number to hang on.
///
/// ── FIELD F-1 · 2026-08-30 · a wrong verse on a real wall ───────────────────
///
/// A bare "verse 32" is resolved against `ContextMemory` — the passage last put
/// on screen. That is right for the case it was written for: a preacher reading
/// through Romans 8 who says "and verse eighteen".
///
/// It is wrong the moment the sentence names its own book. In a live service the
/// operator had manually fired **Proverbs 3:6** five minutes earlier; the preacher
/// moved on and said *"…what was going through in **Luke 10**. If you read from
/// **verse 32, 37**."* The bare 32 was resolved against the remembered Proverbs 3
/// and **Proverbs 3:32 auto-fired at 0.88**, unattended, while Luke 10 was sitting
/// in the very same window.
///
/// Memory is what Relay has when the words do not say. When the words DO say, the
/// words win — a reference named in this breath beats one from five minutes ago,
/// always, and no confidence number was ever going to express that.
///
/// Returns the LAST reference parsed from `text`, because a sentence that names
/// two moves forward through them: "we were in Romans 8, now turn to Luke 10,
/// verse 32" means Luke.
pub fn anchor_for_bare_verses(text: &str) -> Option<VerseRef> {
    detect_direct(text)
        .into_iter()
        .next_back()
        .map(|m| m.reference)
}

/// True if this window says the word "chapter" (in any priority language) with a
/// number after it — the speaker stated which chapter they are in.
///
/// `is_chapter_word` and `skip_linkers`, not an English literal, for the same
/// reason as `detect_bare_verses`: "sura ya tano" states a chapter as plainly as
/// "chapter five", and a check that only speaks English would silently apply to
/// half the priority languages.
pub fn chapter_named(text: &str) -> bool {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    (0..tokens.len()).any(|i| {
        is_chapter_word(tokens[i]) && parse_number(&tokens, skip_linkers(&tokens, i + 1)).is_some()
    })
}

/// Where a bare verse number ("verse ten") actually points — the ONE place that
/// decision is made.
///
/// Three sources, in strict order of authority:
///
///  1. **A reference parsed from this same window** (`anchor`). The words said it
///     in this breath. FIELD F-1.
///  2. **Nothing at all**, when the window states a CHAPTER that no parsed
///     reference accounts for. FIELD F-8 (2026-09-06): the preacher said *"is
///     taken from 4th Peter chapter 5 verse 10"*. There is no 4th Peter, so
///     `detect_direct` returned nothing and `anchor` was `None` — and the bare
///     `10` was then resolved against `Psalms 92`, the passage from ten minutes
///     earlier, putting **Psalms 92:10** up unattended at 0.88. The sentence
///     named chapter 5. Memory named chapter 92. Relay believed memory.
///
///     The phrasing is out of `detect_passage_nav`'s reach (it caps at 8 tokens,
///     deliberately, so sermon prose full of numbers cannot trigger a jump), so
///     nothing else was going to catch it. Declining is the only honest answer
///     available: the spoken chapter is known but the BOOK is not, and pairing a
///     heard chapter with a remembered book is the same lie in a smaller coat.
///  3. **Memory** (`memory`), only when the words do not say — "and verse
///     eighteen", mid-passage, which is the case the whole mechanism exists for.
///
/// Pure, so the rule can be tested without an app: the defect above lived in the
/// candidate assembly inside `emit_detections`, where no test could reach it.
/// Where a bare verse's BOOK came from. The label on the wire follows it
/// (`DetectionMethod::for_bare_verse`), because "heard" and "assumed" are
/// different claims and rule 10 says only the first may reach a wall unattended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BareVerseSource {
    /// A reference named in this window: "Luke 10 … verse 32". The book was said.
    Anchor,
    /// The passage already on the screen: "and verse eighteen". The book was not
    /// said in this breath; Relay assumed it.
    Memory,
}

/// `resolve_bare_verse_for_window`, and which of its three rules answered.
///
/// FIELD 2026-09-20 (service 24, 23.5 min): Psalms 55 on the wall, the preacher
/// quoting Hosea 6:1 with the book misheard as a word no alias knows. Memory
/// answered, correctly by rule 3 — and the candidate was then labelled `Direct`
/// at 0.88 and auto-fired. Rule 40 kept that label on purpose while one service
/// was the only evidence. This was the second. The resolution is unchanged; the
/// SOURCE now travels with it so the label can stop lying.
pub fn resolve_bare_verse_with_source(
    text: &str,
    n: i64,
    anchor: Option<&VerseRef>,
    memory: Option<&VerseRef>,
) -> Option<(VerseRef, BareVerseSource)> {
    if let Some(a) = anchor {
        return Some((
            VerseRef {
                book: a.book.clone(),
                chapter: a.chapter,
                verse: n,
            },
            BareVerseSource::Anchor,
        ));
    }
    if chapter_named(text) {
        return None;
    }
    memory.cloned().map(|m| (m, BareVerseSource::Memory))
}

/// Did this window say a reference of its own?
///
/// The disarming condition for the passage guard, and it is rule 40's sentence
/// verbatim: *when the words say, the words win.* Two ways to say, and the second
/// is the one FIELD F-8 was about — a window can STATE a chapter without any
/// reference parsing out of it ("4th Peter chapter 5 verse 10"), and a preacher who
/// has just announced a chapter is no longer reading the last one whatever Relay
/// managed to parse. `resolve_bare_verse_with_source` already treats those two
/// facts as one authority; this is the same pair asked for a different purpose, so
/// it is named once here rather than restated at the call site.
///
/// `anchor` is passed in rather than recomputed: `emit_detections` has already run
/// `detect_direct` over this window and the guard may not cost a second parse.
pub fn window_states_a_reference(text: &str, anchor: Option<&VerseRef>) -> bool {
    anchor.is_some() || chapter_named(text)
}

/// Why the passage guard held a candidate back. Reaches the operator verbatim
/// (`main::PassageHold`), because a guard that goes quiet without saying so is
/// indistinguishable from a detector that has gone deaf (rule 35).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeldReason {
    /// **The wall is already showing this verse.** A run of the verse's own words
    /// while that verse is on the screen puts nothing new there.
    AlreadyOnScreen,
    /// **The preacher is reading the passage on screen and this verse is not in
    /// it.** The words of one verse are the words of several.
    OutsideTheReading,
}

/// **THE PASSAGE GUARD.** Which of this window's candidates are held back while
/// the preacher reads?
///
/// One answer per candidate, in order — `None` for everything that gets through,
/// which is the overwhelmingly common case.
///
/// ── The complaint (the operator, 2026-09-25) ──────────────────────────────────
///
/// > *"I dont want suggestion to be changing when a bible verse is reading because
/// > it heard a phrase which is in another bible verse… verses needs to be guarded
/// > so when a preacher is reading a verse it stays within the verse/chapter until
/// > the preacher calls another verse… suggesting too many verses whilst the
/// > preacher is reading a verse will cause confusion"*
///
/// A verse read aloud is not only in the verse being read. The synoptic parallels,
/// the repeated formulae, `Psalms 107:8` and `107:21`, and plain sub-spans — a
/// twenty-five-word run in John 3:16 carries a ten-word run in John 3:15, because
/// those ten words are in both (`PhraseHit::sole`).
///
/// ── TWO RULES, AND ONLY ONE OF THEM IS ABOUT THE PASSAGE ──────────────────────
///
/// **Rule B — the wall already says it.** A candidate found in a verse's own text
/// (`came_from_the_verse_text`) whose reference is the one Relay last put on a
/// screen is held. It cannot be new information: the verse is already exactly
/// where it would have put it.
///
/// This is the rule the field evidence is about, and there is a lot of it. The
/// operator's own services 38, 39 and 40 (2026-09-24 and 2026-09-25) auto-fired
/// the SAME verse twice from one continuous reading, six times:
///
/// | verse | first | second | gap | first fire |
/// |---|---|---|---|---|
/// | `Psalms 23:4` | 206 s | 326 s | 120 s | reading |
/// | `Philippians 1:23` | 1497 s | 1508 s | 11 s | spoken reference |
/// | `Mark 6:2` | 615 s | 626 s | 11 s | reading |
/// | `Jeremiah 6:16` | 769 s | 780 s | 11 s | spoken reference |
/// | `Hebrews 6:12` | 897 s | 908 s | 11 s | spoken reference |
/// | `Romans 15:4` | 1034 s | 1051 s | 17 s | spoken reference |
///
/// Four of the six are *announce the reference, then read it* — the ordinary shape
/// of a sermon rather than an edge case.
///
/// **Nothing that already existed could catch them, and no number can.**
/// `DEFAULT_DEBOUNCE_MS` is `WINDOW_SECS + 2` — ten seconds — and its own reasoning
/// is that *"anything re-detected inside it is, definitionally, the same utterance
/// being heard again"*. That holds for a spoken reference, which is over in two
/// seconds, and fails for a reading, which keeps producing matching runs for as
/// long as the reading lasts. The measured gaps are 11, 11, 17 and 120 seconds: a
/// cooldown of 12 catches two of four and one of 20 starts swallowing genuine
/// second citations. It is not a timing problem. `pipeline::better` cannot help
/// either — `Mark 6:2` went 0.69 → 0.90 as more of the verse was heard, so the
/// duplicate is the STRONGER candidate by every ranking rule there is.
///
/// **Rule A — outside the reading.** While a window holds a run of the speaker's
/// own words verbatim INSIDE the book and chapter on the screen, candidates found
/// in a verse's own text from outside that chapter are held. Chapter, not book,
/// because the operator said *verse/chapter* and because Psalms 23 is a different
/// passage from Psalms 107.
///
/// ── WHAT ENDS IT, AND WHY NOTHING HAS TO BE CLEARED ───────────────────────────
///
///  * **A window that names a reference disarms rule A entirely** — not the
///    candidate, the whole window. "Turn to Romans eight" gets through in the same
///    instant it always did, whatever is on the screen. `window_states_a_reference`
///    is rule 40's sentence: when the words say, the words win. Checked first
///    rather than last, because a guard that could swallow a spoken reference would
///    be strictly worse than the churn it fixes, and rule 10 exists because that
///    class of mistake has reached a congregation three times.
///  * **Rule A is armed by EVIDENCE, not by state.** No lock, no flag, no timer,
///    nothing to clear — so there is nothing that can be left on. It stops biting
///    in the first window with no in-passage run in it, which is the window the
///    preacher moved on.
///  * **Rule B ends when the wall changes.** `on_the_wall` is the router's record
///    of the last verse it put on a screen. It is replaced by the next verse, and
///    dropped by `Router::forget_last_fire` (a clear or a blackout) and by
///    `Router::forget_wall` (anything that is not scripture taking the screen — the
///    same door rule 38 disarms the passage at). All three are automatic.
///
/// ── WHAT IS DELIBERATELY NOT HELD ─────────────────────────────────────────────
///
///  * **Anything reference-shaped, ever.** `Direct`, `Ambiguous`, `UncertainBook`
///    and `UncertainNumber` are never held under either rule, at any dial, in any
///    window. See `came_from_the_verse_text`.
///  * **The next verse of the passage being read.** `Philippians 1:23` at 1508 s
///    and `1:24` at 1517 s is one reading walking forward: rule B holds the first
///    (already on the wall) and the second fires. Freezing the wall on verse 23
///    until somebody names another reference would leave a verse the preacher had
///    finished in front of a congregation for as long as the reading lasted — a
///    wrong verse chosen on purpose — and it would reverse the instruction of
///    2026-09-23 that created `Reading` at all. The operator's own words are
///    *within the verse/chapter*, and the adjacent verse is inside it.
///
/// ── WHAT IT COSTS, MEASURED ───────────────────────────────────────────────────
///
/// `main::passage_guard_bench::what_the_guard_costs_a_real_service` replays a real
/// service through `candidates_for_window` and the real `Router` and prints both
/// columns. Three of the operator's own services, 971 finalized transcript lines:
///
/// | service | lines | suggestions | auto-fires | duplicates removed | verses lost |
/// |---|---|---|---|---|---|
/// | 35 | 332 | 220 → 197 | 38 → 34 | 4 | **0** |
/// | 39 | 246 | 111 → 90  | 34 → 33 | 1 | **0** |
/// | 40 | 393 | 178 → 161 | 37 → 31 | 6 | **0** |
///
/// **Eleven duplicate broadcasts removed, no verse lost, and about one suggestion
/// in eight held.** Every removed broadcast was of the verse the screens were
/// already showing, which the bench checks rather than assumes.
///
/// **The CI scorecard cannot see this change at all, and that is stated rather than
/// glossed.** `eval::print_scorecard` reports 100% recall, 53/53 and a 0.0%
/// wrong-verse rate with the guard on and exactly the same with it off, because it
/// scores one window at a time and has no wall for a verse to be already on. RG-296
/// records the identical blindness for the promotion that created `Reading`. An
/// instrument that returns the same answer either way is not evidence.
///
/// **What has NOT been measured.** These are FINALS out of the database, and the
/// live path also runs detection on every partial — roughly one a second — so every
/// count above is a floor on the churn rather than a measurement of it. Suggestions
/// are never persisted, so the database can corroborate the fire column and not the
/// suggestion column. There is no recorded church audio on this machine, so nothing
/// here is a claim about accuracy in any language.
///
/// ── WHAT IT IS NOT ────────────────────────────────────────────────────────────
///
/// It is NOT `quoted(text, Some(on_screen_book), k)`. Restricting the index by the
/// passage in memory was the obvious implementation and it is the wrong one, for
/// the reason that call site already records: *"a preacher reading Proverbs who
/// quotes Isaiah is quoting Isaiah"*, and a restriction would hide Isaiah until
/// something else moved memory — which needs an operator, which is the one thing a
/// guard may never need. This asks for positive evidence that the preacher is still
/// in the passage, so a preacher who has moved on is never held even once.
///
/// Rule A also holds no candidate it did not itself arm on: the in-passage run that
/// armed it is never held, so **something always survives to announce the hold**.
/// A guard whose quiet looks exactly like a detector going deaf is rule 35's own
/// failure, and neither rule here can reach that state.
pub fn hold_for_the_passage(
    on_screen: Option<&VerseRef>,
    on_the_wall: Option<&str>,
    window_states_a_reference: bool,
    cands: &[(&VerseRef, DetectionMethod)],
) -> Vec<Option<HeldReason>> {
    // RULE B. Independent of everything else — including a window that names a
    // reference, because a NAMED reference is not held by this rule at all and a
    // READ one adds nothing whether or not the name was said in the same breath.
    let mut out: Vec<Option<HeldReason>> = cands
        .iter()
        .map(|(r, m)| {
            (m.came_from_the_verse_text() && on_the_wall.is_some_and(|w| w == reference_key(r)))
                .then_some(HeldReason::AlreadyOnScreen)
        })
        .collect();
    // RULE A. The words said; nothing Relay remembers may stand in front of them.
    if window_states_a_reference {
        return out;
    }
    let Some(passage) = on_screen else {
        return out;
    };
    let inside = |r: &VerseRef| r.book == passage.book && r.chapter == passage.chapter;
    // Armed by evidence in THIS window, or not at all. A candidate rule B is
    // already holding still counts as evidence: the preacher IS reading the verse
    // on the wall, which is the very thing rule A needs to know.
    if !cands
        .iter()
        .any(|(r, m)| m.is_a_verbatim_run() && inside(r))
    {
        return out;
    }
    for (slot, (r, m)) in out.iter_mut().zip(cands) {
        if slot.is_none() && m.came_from_the_verse_text() && !inside(r) {
            *slot = Some(HeldReason::OutsideTheReading);
        }
    }
    out
}

/// `"Book Chapter:Verse"` — the same key `pipeline::Fire::key_for` builds and the
/// same one `Router` debounces on.
///
/// Stated here because the guard compares a candidate with what the router says is
/// on the wall, and two spellings of one key is how a guard silently stops
/// matching. This module is DB- and IO-free on purpose and `pipeline` is not, so
/// the format lives in one place on each side and `the_two_reference_keys_agree`
/// holds them to the same answer.
pub fn reference_key(r: &VerseRef) -> String {
    format!("{} {}:{}", r.book, r.chapter, r.verse)
}

pub fn detect_bare_verses(text: &str) -> Vec<i64> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        // `is_verse_word`, NOT an inline English literal list.
        //
        // This matched `"verse" | "verses" | "vs" | "v"` directly while its sibling
        // `parse_reference` asked `is_verse_word`, which consults `numerals.json`
        // and therefore already speaks Swahili and Hausa. The result was a preacher
        // whose FULL reference parsed correctly — "Yohana sura ya tatu mstari wa
        // kumi na sita" → John 3:16 — saying "mstari wa nne" to move within the
        // passage and getting nothing at all. CLAUDE.md: never write detection logic
        // that assumes single-language input.
        //
        // `skip_linkers` comes with it: "mstari WA nne" puts grammatical glue
        // between the keyword and its number, and that glue is why the English-only
        // version could not have worked even with the vocabulary added.
        if is_verse_word(tokens[i]) {
            let j = skip_linkers(&tokens, i + 1);
            if let Some((n, _, _)) = parse_number(&tokens, j) {
                out.push(n);
            }
        }
        i += 1;
    }
    out
}

/// A spoken jump WITHIN the current book — chapter and/or verse, no book name.
#[derive(Debug, Clone, PartialEq)]
pub struct PassageNav {
    pub chapter: Option<i64>,
    pub verse: Option<i64>,
}

/// True if any book alias appears in the tokens.
fn book_named(tokens: &[&str]) -> bool {
    (0..tokens.len()).any(|i| match_book(tokens, i).is_some())
}

/// Detect a spoken jump within the CURRENT book — "chapter 5 verse 1",
/// "chapter fifty verse two", "go to chapter 5", "verse 4" — WITHOUT naming a
/// book (a named book goes through the normal reference path). The caller
/// resolves the book from context (the last verse shown) and keeps the operator
/// in the same passage. Requires a chapter/verse keyword and a short utterance
/// so ordinary sermon prose with numbers never triggers.
pub fn detect_passage_nav(text: &str) -> Option<PassageNav> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    if tokens.is_empty() || tokens.len() > 8 {
        return None;
    }
    if book_named(&tokens) {
        return None; // an explicit "Psalm 5:1" is a full reference, not a jump
    }
    let mut chapter = None;
    let mut verse = None;
    let mut saw_kw = false;
    let mut i = 0;
    while i < tokens.len() {
        // `is_chapter_word` / `is_verse_word` + `skip_linkers`, for the same reason
        // as `detect_bare_verses` above: these were inline English literals while
        // `parse_reference` two functions away asked the multilingual helpers. So
        // "sura ya tano" and "aya ta huɗu" — a Swahili or Hausa preacher moving
        // within a passage they had already opened correctly — did nothing.
        if is_chapter_word(tokens[i]) {
            saw_kw = true;
            let j = skip_linkers(&tokens, i + 1);
            if let Some((n, next, _)) = parse_number(&tokens, j) {
                chapter = Some(n);
                i = next;
                continue;
            }
        } else if is_verse_word(tokens[i]) {
            saw_kw = true;
            let j = skip_linkers(&tokens, i + 1);
            if let Some((n, next, _)) = parse_number(&tokens, j) {
                verse = Some(n);
                i = next;
                continue;
            }
        }
        i += 1;
    }
    if !saw_kw || (chapter.is_none() && verse.is_none()) {
        return None;
    }
    Some(PassageNav { chapter, verse })
}

/// Generate candidate references for an AMBIGUOUS book+number with no verse,
/// e.g. "revelation 22" → [Revelation 22:1, Revelation 2:2]. Used only when no
/// full reference was detected, to surface operator-pickable suggestions. The
/// caller filters these against the corpus and gates them as suggestions.
pub fn detect_ambiguous(text: &str) -> Vec<VerseRef> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if let Some((canonical, book_end, _fuzzy)) = match_book(&tokens, i) {
            let mut j = book_end;
            if let Some(t) = tokens.get(j) {
                if matches!(*t, "chapter" | "chap" | "ch") {
                    j += 1;
                }
            }
            // A colon form is unambiguous — skip.
            if try_colon_pair(&tokens, j).is_none() {
                if let Some((n, after, _)) = parse_number(&tokens, j) {
                    // Is a verse already present? Then it's not ambiguous.
                    let mut k = after;
                    while let Some(t) = tokens.get(k) {
                        if matches!(*t, "verse" | "verses" | "vs" | "v" | ":") {
                            k += 1;
                        } else {
                            break;
                        }
                    }
                    let has_verse = parse_number(&tokens, k).is_some();
                    if !has_verse && !is_single_chapter(canonical) {
                        // chapter N, verse 1
                        out.push(VerseRef {
                            book: canonical.into(),
                            chapter: n,
                            verse: 1,
                        });
                        // two-digit split: 22 → 2:2, 21 → 2:1
                        if (11..=99).contains(&n) && n % 10 >= 1 {
                            out.push(VerseRef {
                                book: canonical.into(),
                                chapter: n / 10,
                                verse: n % 10,
                            });
                        }
                    }
                    i = after;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

// ===== Semantic match (PROMPT.md Phase 9) =====

/// A vector-similarity index over the verse corpus. Phase 9 uses a TF-IDF
/// bag-of-words embedding with cosine similarity — a genuine embedding+search
/// that runs fully offline with no model. A neural sentence-embedder is a
/// drop-in behind the same `top_k` seam (and the verses.embedding BLOB column)
/// later; it improves synonym/paraphrase recall that lexical overlap misses.
pub struct SemanticIndex {
    idf: HashMap<String, f32>,
    /// (reference, L2-normalized tf-idf vector) per verse.
    docs: Vec<(VerseRef, HashMap<String, f32>)>,
    /// stem → the readable word it came from, for the operator-facing "why".
    /// Stemming is right for matching and wrong for reading: Snowball turns
    /// "belly" into "belli". Rule #18 says the operator must be able to judge
    /// the claim, and nobody can judge "belli · husk".
    surface: HashMap<String, String>,
    /// STORIES. Overlapping windows of contiguous verses within one chapter,
    /// each with its own tf-idf vector, plus the range of `docs` it covers.
    /// Built from the SAME stemmed tokens as `docs`, so a story and its verses
    /// live in one vocabulary. See `PASSAGE_LEN` and `top_k_explained`.
    passages: Vec<(usize, usize, HashMap<String, f32>)>,
}

// THE `rare_terms` SET IS GONE (2026-09-20). It held the stems rare enough to
// stand as evidence ON THEIR OWN, and nothing needs that question answered any
// more — see `MIN_EVIDENCE_TERMS`. Deleted rather than left computed and unread:
// a set built at startup for a rule that no longer exists is how a reader comes
// to believe the rule is still there.

/// Modern English → KJV vocabulary, baked in (`include_str!`) so it stays
/// offline. See `data/kjv_gloss.json` for why this exists and what it is not.
/// Keys AND values are stemmed at load, and the gloss is applied after
/// stemming — so one entry ("pig") covers "pig" and "pigs", and the table stays
/// a list of concepts instead of a list of word forms.
fn kjv_gloss() -> &'static HashMap<String, Vec<String>> {
    static GLOSS: std::sync::OnceLock<HashMap<String, Vec<String>>> = std::sync::OnceLock::new();
    GLOSS.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Raw {
            gloss: HashMap<String, Vec<String>>,
        }
        const RAW: &str = include_str!("../data/kjv_gloss.json");
        let raw = serde_json::from_str::<Raw>(RAW)
            .map(|r| r.gloss)
            .unwrap_or_default();
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        for (k, vs) in raw {
            let key = stem_all(vec![k]).pop().unwrap_or_default();
            out.entry(key).or_default().extend(stem_all(vs));
        }
        for vs in out.values_mut() {
            vs.sort();
            vs.dedup();
        }
        out
    })
}

/// Expand a QUERY's tokens with their KJV equivalents.
///
/// Applied to the query only, never when building the index: glossing the corpus
/// would change the document frequencies, and it is exactly those frequencies
/// (how rare "Meribah" or "husks" is) that make biblical nouns such strong
/// signals. The original token is kept — a retelling that already uses the KJV
/// word must not get worse — so this can only ever ADD evidence.
fn expand_with_gloss(tokens: Vec<String>) -> Vec<String> {
    let gloss = kjv_gloss();
    let mut out = Vec::with_capacity(tokens.len() * 2);
    for t in tokens {
        if let Some(alts) = gloss.get(&t) {
            out.extend(alts.iter().cloned());
        }
        out.push(t);
    }
    out
}

/// How many verses make a "story".
///
/// A pericope — the boy with the loaves, the storm on the lake, David and
/// Goliath — is a handful of verses, not a chapter (Psalm 119 is 176) and not a
/// verse. Eight is about the span of a narrative unit in the KJV, and the
/// windows OVERLAP by half so a story is never sliced down the middle.
const PASSAGE_LEN: usize = 8;
const PASSAGE_STEP: usize = 4;

/// How much of the final score comes from the STORY rather than the verse.
///
/// Asked for directly: *"while searching for paraphrase, prioritise the most
/// relevant stories in the bible and subsequently narrow down to the relevant
/// verse."*
///
/// Why blend rather than filter to the best story and stop: a paraphrase is
/// sometimes a single famous verse with no story around it ("for God so loved
/// the world"), and a hard story-first filter would rank that by the accident of
/// what surrounds it. Blending keeps the verse's own evidence in charge while
/// letting the surrounding narrative break ties — which is what "narrow down to
/// the relevant verse" actually means.
/// MEASURED, not chosen — `story_search::story_weight_measured_against_verse_only`
/// sweeps it over seven spoken story-paraphrases against the full corpus:
///
/// ```text
///   weight   mean rank (missing = 6)   found in top 5
///   0.00     2.57                      6/7      ← verse-only, the old behaviour
///   0.25     2.00                      7/7
///   0.35     1.86                      7/7      ← shipped
///   0.50     2.29                      7/7
///   0.65     2.71                      5/7      ← the story starts drowning the verse
/// ```
///
/// Past ~0.5 the narrative overwhelms the line: every verse in a matching story
/// scores alike and the gate is asked to choose between eight equally-blessed
/// candidates. Re-run the sweep before moving this.
const STORY_WEIGHT: f32 = 0.35;

impl SemanticIndex {
    /// Build the index from the corpus: (reference, verse text).
    pub fn build(corpus: &[(VerseRef, String)]) -> Self {
        let n = corpus.len().max(1) as f32;
        // Document frequency per term.
        let mut df: HashMap<String, f32> = HashMap::new();
        // stem → { original word → times seen }, collapsed below to the most
        // common surface form so the explanation reads like English.
        let mut surface_counts: HashMap<String, HashMap<String, usize>> = HashMap::new();
        let tokenized: Vec<(VerseRef, Vec<String>)> = corpus
            .iter()
            .map(|(r, text)| {
                let raw = tokenize(text);
                let stems = stem_all(raw.clone());
                for (stem, word) in stems.iter().zip(raw.iter()) {
                    *surface_counts
                        .entry(stem.clone())
                        .or_default()
                        .entry(word.clone())
                        .or_insert(0) += 1;
                }
                (r.clone(), stems)
            })
            .collect();
        // Most frequent original wins; ties go to the shorter word, which is the
        // one closer to a dictionary form.
        let surface: HashMap<String, String> = surface_counts
            .into_iter()
            .map(|(stem, counts)| {
                let best = counts
                    .into_iter()
                    .max_by(|a, b| a.1.cmp(&b.1).then(b.0.len().cmp(&a.0.len())))
                    .map(|(w, _)| w)
                    .unwrap_or_else(|| stem.clone());
                (stem, best)
            })
            .collect();
        for (_, toks) in &tokenized {
            let mut seen = std::collections::HashSet::new();
            for t in toks {
                if seen.insert(t.clone()) {
                    *df.entry(t.clone()).or_insert(0.0) += 1.0;
                }
            }
        }
        let idf: HashMap<String, f32> = df
            .into_iter()
            .map(|(t, d)| (t, (n / d).ln() + 1.0))
            .collect();

        // Passage vectors are built from the same tokens, pooled. A story's
        // vocabulary is far richer than any one of its verses, which is exactly
        // why a paraphrase of the STORY matches it when it matches no single
        // verse strongly.
        let mut passages: Vec<(usize, usize, HashMap<String, f32>)> = Vec::new();
        let mut chapter_start = 0usize;
        for i in 0..=tokenized.len() {
            let boundary = i == tokenized.len()
                || (i > 0
                    && (tokenized[i].0.book != tokenized[i - 1].0.book
                        || tokenized[i].0.chapter != tokenized[i - 1].0.chapter));
            if !boundary {
                continue;
            }
            // One chapter spans [chapter_start, i). Window it.
            let mut w = chapter_start;
            while w < i {
                let end = (w + PASSAGE_LEN).min(i);
                let mut pooled: Vec<String> = Vec::new();
                for (_, toks) in &tokenized[w..end] {
                    pooled.extend(toks.iter().cloned());
                }
                if !pooled.is_empty() {
                    passages.push((w, end, tfidf_vector(&pooled, &idf)));
                }
                if end == i {
                    break;
                }
                w += PASSAGE_STEP;
            }
            chapter_start = i;
        }

        let docs: Vec<(VerseRef, HashMap<String, f32>)> = tokenized
            .into_iter()
            .map(|(r, toks)| (r, tfidf_vector(&toks, &idf)))
            .collect();

        SemanticIndex {
            idf,
            docs,
            surface,
            passages,
        }
    }

    /// Top-k verses by cosine similarity to `query`, highest first. Scores are
    /// in [0, 1]; the caller maps them to confidence and applies the gate.
    pub fn top_k(&self, query: &str, k: usize) -> Vec<(VerseRef, f32)> {
        self.top_k_explained(query, k)
            .into_iter()
            .map(|(r, s, _)| (r, s))
            .collect()
    }

    /// Repair query words the corpus has never seen.
    ///
    /// Asked for directly: *"the audio should be ultra sensitive to African tone
    /// — e.g. goden → golden"*. Whisper on accented speech drops and swaps
    /// consonants, and the result is a token that appears nowhere in the Bible.
    ///
    /// ── Why this is safe in a way that book-name repair is not ─────────────
    ///
    /// An out-of-vocabulary token has **no idf entry, so it contributes exactly
    /// nothing to the cosine today** — it is silently discarded. Anything this
    /// function does is therefore strictly additive: the worst case is that a
    /// word which was being ignored carries on being ignored.
    ///
    /// That is the opposite of `fuzzy_book`, where a wrong repair invents a
    /// reference that can auto-fire. Here a wrong repair merely adds a weak
    /// term to a paraphrase score which, by law, can never auto-fire at all
    /// (`DetectionMethod::Semantic`).
    ///
    /// Still conservative: known words are never touched, short words are left
    /// alone, and an ambiguous repair is refused rather than guessed.
    fn repair_query(&self, tokens: &[String]) -> Vec<String> {
        tokens
            .iter()
            .map(|t| {
                if t.len() < 4 || self.idf.contains_key(t) {
                    return t.clone();
                }
                let mut best: Option<(usize, &String)> = None;
                let mut tie = false;
                for cand in self.idf.keys() {
                    if cand.len().abs_diff(t.len()) > 1 {
                        continue;
                    }
                    let Some(d) = edit_distance_within(t, cand, 1) else {
                        continue;
                    };
                    match best {
                        Some((bd, _bc)) if d < bd => {
                            best = Some((d, cand));
                            tie = false;
                        }
                        Some((bd, bc)) if d == bd && bc != cand => tie = true,
                        None => best = Some((d, cand)),
                        _ => {}
                    }
                }
                match best {
                    // A tie is no evidence. Leave the word unknown, exactly as
                    // it is today.
                    Some((_, c)) if !tie => c.clone(),
                    _ => t.clone(),
                }
            })
            .collect()
    }

    /// `top_k`, plus the words that actually drove each match — strongest first.
    ///
    /// This is the paraphrase counterpart of `RefMatch::matched_text`, and the
    /// operator console needs it for the same reason. For a spoken reference, "what
    /// triggered this" is a span of transcript the parser read. A TF-IDF match has
    /// no span: its evidence is a handful of shared, rare words, and its score is a
    /// cosine — a distance in an arbitrary vector space, not a probability (see
    /// `DetectionMethod::Semantic`).
    ///
    /// So the operator is being asked to trust a machine's guess about *meaning*, on
    /// the strength of a number that does not mean what it looks like it means. The
    /// words it keyed on are the only thing that makes that judgeable in the second
    /// they have to judge it — "grace · saved · faith" is something a human can
    /// agree or disagree with. "0.61" is not.
    pub fn top_k_explained(&self, query: &str, k: usize) -> Vec<(VerseRef, f32, Vec<String>)> {
        // TOKENIZE → STEM → REPAIR → GLOSS, in that order, and the order is the
        // whole point. `repair_query` and the gloss both look words up in the
        // INDEX's vocabulary, and since the index is stemmed, they can only be
        // asked about stems. Repair before gloss so a misheard word is corrected
        // first and then expanded, rather than expanded as the wrong word.
        let qvec = tfidf_vector(
            &expand_with_gloss(self.repair_query(&stem_all(tokenize(query)))),
            &self.idf,
        );
        if qvec.is_empty() {
            return Vec::new();
        }
        // Sorted once per query, so every document is scored by summing the same
        // terms in the same order — identical input, identical score, every run.
        let mut qsorted: Vec<(String, f32)> = qvec.iter().map(|(t, w)| (t.clone(), *w)).collect();
        qsorted.sort_by(|a, b| a.0.cmp(&b.0));
        // ── STORY FIRST, THEN THE VERSE ──────────────────────────────────
        //
        // Score the STORIES, and let the best story lift the verses inside it.
        //
        // A single verse is a very short document: "and he took the five loaves"
        // shares few words with "jesus fed the crowd from a boy's lunch", so a
        // paraphrase of a NARRATIVE often matches no verse strongly while
        // matching its passage decisively. Pooling a pericope's vocabulary is
        // what makes the story findable; the verse-level score then decides
        // WHICH verse inside it goes on the screen.
        //
        // Each verse keeps the best score of any window containing it — windows
        // overlap, so a verse near a boundary is judged by the story it belongs
        // to rather than by where the window happened to be cut.
        let mut story: Vec<f32> = vec![0.0; self.docs.len()];
        for (from, to, pvec) in &self.passages {
            let s = cosine(&qsorted, pvec);
            if s <= 0.0 {
                continue;
            }
            for slot in story[*from..*to].iter_mut() {
                if s > *slot {
                    *slot = s;
                }
            }
        }

        let mut scored: Vec<(usize, f32)> = self
            .docs
            .iter()
            .enumerate()
            .map(|(i, (_, dvec))| {
                let verse = cosine(&qsorted, dvec);
                // The verse's own evidence stays in charge; the story it sits in
                // breaks ties. A verse with no evidence of its own is NOT
                // promoted just for having good neighbours — otherwise every
                // verse in a matching chapter would become a candidate, and the
                // gate would be asked to choose between eight equally-blessed
                // lines.
                let blended = if verse > 0.0 {
                    verse * (1.0 - STORY_WEIGHT) + story[i] * STORY_WEIGHT
                } else {
                    0.0
                };
                (i, blended)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // NARROW TO WHAT CAN BE JUSTIFIED, **BEFORE** TAKING THE TOP k.
        //
        // The evidence filter used to run after `truncate(k)`, and the live path
        // asks for exactly one candidate (`top_k_explained(text, 1)`). So an
        // unjustifiable top-1 did not step aside — it CONSUMED THE ONLY SLOT and
        // was then dropped, and the correct verse sitting at rank 2 was never
        // considered. The operator saw no suggestion at all and no reason why.
        //
        // Rejecting a candidate has to mean the next one gets its turn.
        // A short query cannot corroborate itself three ways, so the requirement
        // bends to it — but never below TWO, whatever was said. A single shared
        // word is a coincidence with a good score at any query length, and
        // `evidence_floor` exists to forbid exactly that.
        // THREE SHARED WORDS, FLAT. No bend for a short query, no exception for
        // a rare one. See `MIN_EVIDENCE_TERMS` for the measurement that removed
        // both on 2026-09-20; between them they were 71% of everything this
        // matcher offered across a real service.
        let required = MIN_EVIDENCE_TERMS;
        scored
            .into_iter()
            .filter_map(|(i, s)| {
                let (r, dvec) = &self.docs[i];
                // Judged on the STEMS, before they are made readable — rarity is
                // a fact about the index's vocabulary, and `surface` deliberately
                // maps several stems onto one word.
                let stems = top_terms(&qvec, dvec, EXPLAIN_TERMS);
                // NARROW TO WHAT CAN BE JUSTIFIED. `required` is the bar, and
                // NOTHING is exempt from it any more: a candidate is offered
                // because several independent words back it, or it is not
                // offered at all. DECISIONS.md §33, reversed 2026-09-20.
                if stems.len() < required {
                    return None;
                }
                let why: Vec<String> = stems
                    .into_iter()
                    .map(|t| self.surface.get(&t).cloned().unwrap_or(t))
                    .collect();
                Some((r.clone(), s, why))
            })
            .take(k)
            .collect()
    }
}

impl SemanticIndex {
    /// Rank with an explicit story weight. TEST ONLY — this is how the value of
    /// `STORY_WEIGHT` is measured rather than asserted, by running the same
    /// queries at `0.0` (verse-only, the old behaviour) and at the shipped value.
    #[cfg(test)]
    pub fn top_k_story_weighted(&self, query: &str, k: usize, w: f32) -> Vec<(VerseRef, f32)> {
        // Must mirror `top_k_explained`'s query pipeline exactly, or the number
        // it measures is not the number that ships.
        let qvec = tfidf_vector(
            &expand_with_gloss(self.repair_query(&stem_all(tokenize(query)))),
            &self.idf,
        );
        if qvec.is_empty() {
            return Vec::new();
        }
        let mut qsorted: Vec<(String, f32)> = qvec.iter().map(|(t, w)| (t.clone(), *w)).collect();
        qsorted.sort_by(|a, b| a.0.cmp(&b.0));
        let mut story: Vec<f32> = vec![0.0; self.docs.len()];
        for (from, to, pvec) in &self.passages {
            let sc = cosine(&qsorted, pvec);
            if sc <= 0.0 {
                continue;
            }
            for slot in story[*from..*to].iter_mut() {
                if sc > *slot {
                    *slot = sc;
                }
            }
        }
        let mut scored: Vec<(usize, f32)> = self
            .docs
            .iter()
            .enumerate()
            .map(|(i, (_, dvec))| {
                let verse = cosine(&qsorted, dvec);
                let blended = if verse > 0.0 {
                    verse * (1.0 - w) + story[i] * w
                } else {
                    0.0
                };
                (i, blended)
            })
            .filter(|(_, sc)| *sc > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
            .into_iter()
            .map(|(i, sc)| (self.docs[i].0.clone(), sc))
            .collect()
    }
}

/// How many overlapping words to show as the reason for a paraphrase match.
///
/// Raised from 4 to 6 on the operator's report that the console was showing a
/// single word: with a thin match there was one term to show, and one word is
/// not evidence a human can weigh. More terms cost nothing when they exist and
/// the UI truncates what will not fit.
const EXPLAIN_TERMS: usize = 6;

/// The least shared evidence a paraphrase may be suggested on.
///
/// ── Why a COUNT and not just a higher cosine ──────────────────────────────
///
/// A cosine can be respectable on ONE shared word if that word is rare enough —
/// the vector is short, so a single strong term dominates it. That is how the
/// console ended up offering a verse whose entire justification was one word,
/// which is not something an operator can agree or disagree with in the second
/// they have to judge it.
///
/// Requiring several independent overlapping words is a different kind of
/// evidence from requiring a bigger number: it asks the match to be corroborated
/// rather than merely confident. A verse sharing four content words with what
/// was said is defensible on its face; one sharing a single word is a
/// coincidence with a good score.
///
/// MEASURED, twice, and the second measurement moved it from 2 to 3.
///
/// It sat at 2 because "at 3 the shipped eval corpus loses recall". That was
/// true and it was an artifact: the filter ran AFTER `truncate(k)`, and the live
/// path asks for one candidate, so a rejected top-1 consumed the only slot and
/// left nothing rather than yielding to the verse behind it. Rejecting a
/// candidate now means the next one gets its turn, and at 3 the shipped corpus
/// holds 100% recall / 0% wrong-verse.
///
/// The value is set by the PARAPHRASE benchmark (`eval::paraphrase`), because
/// that is the behaviour it governs — the shipped corpus is almost entirely
/// direct references and cannot see this at all. Measured over 16 real preacher
/// paraphrases: **recall@1 rises 69% → 75%**, and the wrong top-1 answers fall
/// from 5 to 3. The two it kills are the exact shape the operator complained
/// about — a whole verse justified by two words:
///
///     ["flesh", "among"]  → Proverbs 23:20   for "the word became flesh and dwelt among us"
///     ["promise", "god"]  → Galatians 3:18   for "the promise of God … mixing with faith"
///
/// Corroboration, not confidence. A verse sharing four content words with what
/// was said is defensible on its face; one sharing two is a coincidence with a
/// good score, and no operator can weigh it in the second they have.
///
/// ── NO EXCEPTIONS, AS OF 2026-09-20 (DECISIONS.md §33, reversed) ──────────
///
/// There used to be two ways past this bar, and together they were most of what
/// this matcher offered. `RARE_DF_FRACTION` let a SINGLE rare word through, on
/// the argument that there was nowhere else in the corpus it could have come
/// from; and `required` bent down to 2 for a short query, on the argument that a
/// short query cannot corroborate itself three ways.
///
/// Both arguments are reasonable and both were wrong, and the operator found it
/// before any instrument here did: *"it is still suggesting just one word ...
/// we will have too much going on the preview and miss the right verse."*
///
/// MEASURED over all 816 transcript windows of the service of 2026-09-20 and
/// over both benchmarks. `@3` is listed because `SEMANTIC_SUGGESTIONS_MAX` is 3,
/// so it is the last rank an operator can ever see:
///
///                            offers  1-word   @1    @3    @5   story@1  modern@1
///   3 terms, or one rare        638     287   69%   81%   88%     84%       59%
///   3 terms, no rare word       302       0   75%    —    81%      —         —
///   3 terms, neither            186       0   75%   81%   81%     88%       71%
///   4 terms, neither            175       0   62%    —    69%      —         —
///
/// Seven offers a minute became two, every survivor is corroborated, and NOTHING
/// AN OPERATOR CAN SEE GOT WORSE: recall@3 is 13/16 either way and recall@1 went
/// UP. The @5 column is the whole cost, and it is two ranks no console renders.
///
/// The rare-word rule was not buying the recall it was defended with: a one-word
/// match was taking rank 1 in front of a properly corroborated verse, which is
/// the same failure the evidence filter was moved before `truncate(k)` to fix.
///
/// The old note claimed the KJV gloss "cannot work at all" without the single
/// rare word, because a modern retelling reaches its verse through one rare KJV
/// noun ("pigs" → "swine"). That is the `modern` column, and it is the biggest
/// single improvement in the table: 59% → 71%. A real retelling is a sentence,
/// and a sentence corroborates itself.
const MIN_EVIDENCE_TERMS: usize = 3;

/// The shared terms that contributed most to a cosine — the "why" of a paraphrase.
///
/// Contribution is `q_weight * d_weight` per term, which is exactly the summand in
/// `cosine`. So these are not "words that happen to appear in both"; they are, in
/// order, the terms that actually produced the score.
fn top_terms(q: &HashMap<String, f32>, d: &HashMap<String, f32>, n: usize) -> Vec<String> {
    let mut terms: Vec<(&String, f32)> = q
        .iter()
        .filter_map(|(t, qw)| d.get(t).map(|dw| (t, qw * dw)))
        .collect();
    terms.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(b.0)) // stable, so the UI does not flicker
    });
    terms.truncate(n);
    terms.into_iter().map(|(t, _)| t.clone()).collect()
}

/// Content-word tokenizer: lowercase, split on non-alphanumerics, drop short
/// tokens and function words. KJV archaisms (thou/hath/…) are dropped as
/// stopwords; scripture content words are kept.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter_map(|w| {
            let w = w.to_lowercase();
            if w.len() >= 3 && !is_stopword(&w) {
                Some(w)
            } else {
                None
            }
        })
        .collect()
}

/// Reduce tokens to their stems, so "pigs"/"pig", "flames"/"flame" and
/// "physicians"/"physician" stop being different words.
///
/// Applied to BOTH the index and the query — unlike the gloss, this is a
/// normalisation, and normalising only one side would simply stop them matching.
///
/// Deliberately a STANDARD Snowball stemmer rather than a hand-rolled suffix
/// stripper: a stemmer written here would inevitably get tuned against
/// data/paraphrase_corpus.json until the score looked good, which measures
/// nothing. An off-the-shelf algorithm cannot be fitted to our own benchmark.
fn stem_all(tokens: Vec<String>) -> Vec<String> {
    use rust_stemmers::{Algorithm, Stemmer};
    static STEMMER: std::sync::OnceLock<Stemmer> = std::sync::OnceLock::new();
    let s = STEMMER.get_or_init(|| Stemmer::create(Algorithm::English));
    tokens
        .into_iter()
        .map(|t| s.stem(&t).into_owned())
        .collect()
}

fn is_stopword(w: &str) -> bool {
    const STOP: &[&str] = &[
        "the", "and", "that", "for", "his", "him", "her", "she", "you", "your", "our", "with",
        "not", "but", "was", "were", "are", "this", "shall", "unto", "thou", "thy", "thee", "ye",
        "hath", "have", "had", "which", "will", "them", "they", "their", "there", "then", "than",
        "when", "who", "what", "all", "any", "from", "out", "into", "upon", "did", "does", "doth",
    ];
    STOP.contains(&w)
}

/// L2-normalized tf-idf vector for a token list, given a corpus idf map.
fn tfidf_vector(tokens: &[String], idf: &HashMap<String, f32>) -> HashMap<String, f32> {
    let mut tf: HashMap<String, f32> = HashMap::new();
    for t in tokens {
        if idf.contains_key(t) {
            *tf.entry(t.clone()).or_insert(0.0) += 1.0;
        }
    }
    let mut vec: HashMap<String, f32> = tf
        .into_iter()
        .map(|(t, f)| {
            let w = f * idf.get(&t).copied().unwrap_or(0.0);
            (t, w)
        })
        .collect();
    // Summed in a FIXED order. `vec.values()` iterates a HashMap, whose order
    // varies per instance, and float addition is not associative — so the norm
    // (and therefore every weight, and therefore every score) drifted in the last
    // decimal place between two runs of the same query. See `cosine`.
    let mut squares: Vec<f32> = vec.values().map(|v| v * v).collect();
    squares.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let norm: f32 = squares.into_iter().sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in vec.values_mut() {
            *v /= norm;
        }
    }
    vec
}

/// Cosine similarity of two L2-normalized sparse vectors (= dot product).
/// Cosine of a query (as a SORTED slice) against a document vector.
///
/// The query side is a sorted slice, not a HashMap, so the summation order is
/// fixed. Float addition is not associative, and `HashMap` iteration order
/// varies per map instance — so summing over one made the same query score
/// microscopically differently between two calls. That is a real problem, not a
/// test nit: `SEMANTIC_FLOOR` gates on this number, so a borderline paraphrase
/// could be suggested on one run and dropped on the next, from identical input.
/// Live software has to be predictable.
fn cosine(query: &[(String, f32)], doc: &HashMap<String, f32>) -> f32 {
    query
        .iter()
        .filter_map(|(t, qv)| doc.get(t).map(|dv| qv * dv))
        .sum()
}

#[cfg(test)]
mod tests {

    // ── RG-135 · the translation the speaker named ─────────────────────────
    mod named_translation_tests {
        use super::super::named_translation;

        #[test]
        fn the_field_sentence_that_produced_this_row() {
            // FIELD-2026-09-13 §2, verbatim in shape: the preacher named the
            // Passion Translation and Relay put the King James Version on the wall
            // at 0.88, wearing the same badge as the seven correct fires around it.
            assert_eq!(
                named_translation("it says here in the passion translation hebrews 11 verse 19"),
                Some("TPT".into())
            );
        }

        #[test]
        fn the_longest_name_wins_so_a_prefix_cannot_answer_for_it() {
            // "new king james" contains "king james". A shorter form answering
            // first would report the wrong translation, which is worse than
            // reporting none: it would put a confident, incorrect caveat on a fire.
            assert_eq!(
                named_translation("turn with me, new king james"),
                Some("NKJV".into())
            );
            assert_eq!(named_translation("the king james says"), Some("KJV".into()));
            assert_eq!(
                named_translation("read it from the new revised standard"),
                Some("NRSV".into())
            );
        }

        #[test]
        fn it_does_not_answer_from_inside_another_word() {
            // The whole reason for the boundary test. "amp" sits inside "example"
            // and "campus"; "msg" sits inside nothing a preacher says, which is why
            // it is on the list and "message" is not.
            assert_eq!(named_translation("for example, let us look"), None);
            assert_eq!(named_translation("on the campus last week"), None);
            assert_eq!(named_translation("the message of the gospel"), None);
            assert_eq!(named_translation("i have a message for you"), None);
        }

        #[test]
        fn an_ordinary_sermon_window_names_nothing() {
            // The case that must stay silent, because a caveat on a correct fire is
            // how an operator learns to ignore the line this exists to show them.
            assert_eq!(
                named_translation("turn with me to romans chapter eight verse twenty eight"),
                None
            );
            assert_eq!(named_translation(""), None);
            assert_eq!(named_translation("and the lord spoke to abraham"), None);
        }

        #[test]
        fn it_reads_a_transcript_so_case_and_punctuation_do_not_matter() {
            assert_eq!(
                named_translation("The Passion Translation, Hebrews 11."),
                Some("TPT".into())
            );
            assert_eq!(named_translation("(NIV)"), Some("NIV".into()));
            assert_eq!(named_translation("...ESV..."), Some("ESV".into()));
        }
    }

    use super::*;

    fn one(text: &str) -> RefMatch {
        let v = detect_direct(text);
        assert_eq!(v.len(), 1, "expected exactly one match in {text:?}: {v:?}");
        v.into_iter().next().unwrap()
    }

    fn refeq(m: &RefMatch, book: &str, ch: i64, vs: i64) {
        assert_eq!(
            m.reference,
            VerseRef {
                book: book.into(),
                chapter: ch,
                verse: vs
            }
        );
    }

    #[test]
    fn digit_colon_form() {
        let m = one("turn with me to John 3:16 this morning");
        refeq(&m, "John", 3, 16);
        assert!(m.confidence >= 0.95);
    }

    #[test]
    fn spoken_words_three_sixteen_is_not_nineteen() {
        // The crux: "three sixteen" = 3:16, not 19.
        refeq(&one("john three sixteen"), "John", 3, 16);
    }

    #[test]
    fn chapter_verse_keywords() {
        let m = one("romans chapter eight verse twenty-eight");
        refeq(&m, "Romans", 8, 28);
        assert!(m.confidence >= 0.95);
    }

    #[test]
    fn bare_digits_two_tokens() {
        refeq(&one("psalm 23 1"), "Psalms", 23, 1);
    }

    /// A preacher who never says the word "verse" must still reach the screen.
    /// ASR renders the pauses as commas and full stops; `normalize` strips them,
    /// so all of these are the same bare pair — and all must clear the default
    /// auto-fire line (0.50), not merely be offered as a suggestion.
    #[test]
    fn spoken_bare_pairs_auto_fire() {
        for (text, book, ch, vs) in [
            ("psalm 23 1", "Psalms", 23, 1),
            ("Acts 2, 1.", "Acts", 2, 1),
            ("John, 3, 16.", "John", 3, 16),
            ("Romans 8, 1", "Romans", 8, 1),
        ] {
            let m = one(text);
            refeq(&m, book, ch, vs);
            assert!(
                m.confidence > 0.50,
                "{text:?} scored {:.2} — at or below the default auto-fire line, so a \
                 preacher who never says \"verse\" would never reach the screen",
                m.confidence
            );
        }
    }

    /// The live-rehearsal regression this demotion exists for. "Psalms 2, 3, 1"
    /// parses 2:3 and leaves a stray "1" no range can absorb — the numbers did not
    /// line up, so it must reach the OPERATOR, never the congregation.
    ///
    /// Reintroduce the bug (drop the `trailing_number` guard) and this fails.
    #[test]
    fn a_garbled_number_run_never_auto_fires() {
        let m = one("Verse 1, Psalms 2, 3, 1, Next verse, chapter 2,");
        refeq(&m, "Psalms", 2, 3);
        assert!(
            m.confidence < 0.50,
            "garbled '2, 3, 1' scored {:.2} — this is the transcript that put \
             Psalms 2:3 on the wall unasked",
            m.confidence
        );
    }

    /// A repaired (misheard) book name plus loose digits is two guesses stacked:
    /// 0.55 - 0.06 = 0.49, just under the line. Always asks a human.
    #[test]
    fn a_repaired_book_with_bare_digits_still_asks_a_human() {
        let m = one("psam 23 1"); // "psam" → Psalms, a Levenshtein repair
        refeq(&m, "Psalms", 23, 1);
        assert!(
            m.confidence < 0.50,
            "repaired book + bare digits scored {:.2}",
            m.confidence
        );
    }

    // ---- A1/A2: whole-chapter references and verse ranges ------------------

    #[test]
    fn whole_chapter_reference_anchors_verse_one() {
        // "Psalm 23" (no verse) → display verse 1, flagged as a whole chapter.
        let m = one("turn to psalm 23");
        refeq(&m, "Psalms", 23, 1);
        assert!(m.whole_chapter);
        assert_eq!(m.verse_end, None);
        // Surfaces as a SUGGESTION, not a forced auto-fire.
        //
        // Asserted against the real gate, not a magic number. This test used to
        // say `< 0.90` and pass at 0.83 — which auto-fires at the 0.50 default
        // bar, the exact opposite of what the line above it claims. The comment
        // was right and the assertion could not fail.
        assert!(
            m.confidence < crate::router::Thresholds::default().auto_fire,
            "a keyword-less whole chapter must not reach the congregation unasked \
             (conf {} vs auto bar {})",
            m.confidence,
            crate::router::Thresholds::default().auto_fire
        );
    }

    #[test]
    fn whole_chapter_with_keyword_is_more_confident() {
        let m = one("psalm chapter 23");
        refeq(&m, "Psalms", 23, 1);
        assert!(m.whole_chapter);
        // "chapter" is proof of referential intent — nobody says it by accident —
        // so THIS one may still fire on its own.
        assert!(m.confidence >= crate::router::Thresholds::default().auto_fire);
    }

    /// A REFERENCE CUT OFF BEFORE ITS VERSE NUMBER IS NOT A WHOLE-CHAPTER
    /// REFERENCE.
    ///
    /// Measured, not imagined: `stt::bench::engine_shootout` drives real audio
    /// through the real pipeline, and because detection runs on every PARTIAL
    /// hypothesis of a window re-decoded once a second, every spoken reference is
    /// parsed at least once before its number has arrived. A clip citing Romans
    /// 8:28 and John 3:16 auto-fired **John 3:1** and **Romans 8:1** to the wall on
    /// the way to the right answers.
    #[test]
    fn a_transcript_that_stops_at_verse_does_not_invent_verse_one() {
        for text in [
            "and we read again in john chapter 3 verse",
            "turn with me in your bibles to romans 8 verse",
            "romans chapter 8 verses",
            "let us read psalm 23 v",
            "john 3:",
            // SINGLE-CHAPTER BOOKS take a different branch in `parse_reference`,
            // and the first version of this guard missed it entirely — where the
            // truncated reading scored 0.95, higher than the 0.88 on the path that
            // was fixed. Jude, Philemon, Obadiah, 2 John, 3 John.
            "turn to jude chapter 1 verse",
            "2 john chapter 1 verse",
            "philemon chapter 1 verse",
            "jude 1:",
        ] {
            assert!(
                detect_direct(text).is_empty(),
                "{text:?} is a reference cut off mid-sentence, not a whole chapter — \
                 got {:?}",
                detect_direct(text)
                    .iter()
                    .map(|m| format!(
                        "{} {}:{} @{:.2}",
                        m.reference.book, m.reference.chapter, m.reference.verse, m.confidence
                    ))
                    .collect::<Vec<_>>()
            );
        }
    }

    /// The dangling marker was not merely tolerated, it was PROMOTING the mistake.
    ///
    /// A bare "Romans 8" scores 0.45 and asks a human. The trailing "verse" set the
    /// keyword bonus, which bought the *less* complete parse 0.88 — straight past
    /// the 0.50 auto bar. The most truncated reading of the sentence outranked the
    /// honest one.
    #[test]
    fn a_dangling_verse_marker_cannot_buy_a_promotion_to_auto_fire() {
        let bar = crate::router::Thresholds::default().auto_fire;
        // The honest, complete form still fires.
        let full = one("turn to john chapter 3 verse 16");
        refeq(&full, "John", 3, 16);
        assert!(full.confidence >= bar);
        // The truncated form reaches nothing at all.
        assert!(detect_direct("turn to john chapter 3 verse").is_empty());
    }

    /// The guard must fire on the MISSING NUMBER, not on the word "verse". These
    /// all carry a verse number and must be untouched — this is what stops the fix
    /// above from being a silent recall regression.
    #[test]
    fn a_verse_marker_with_its_number_is_unaffected() {
        refeq(&one("john chapter 3 verse 16"), "John", 3, 16);
        refeq(&one("psalm 23 verse 1"), "Psalms", 23, 1);
        refeq(&one("john 3:16"), "John", 3, 16);
        refeq(&one("jude verse 4"), "Jude", 1, 4);
        // Single-chapter books, both shapes — the branch the guard also covers.
        refeq(&one("jude chapter 1 verse 4"), "Jude", 1, 4);
        refeq(&one("jude 4"), "Jude", 1, 4);
        refeq(&one("2 john chapter 1 verse 3"), "2 John", 1, 3);
        // A range still parses: "verses" then a number.
        let r = one("psalm 23 verses 1 to 6");
        refeq(&r, "Psalms", 23, 1);
        assert_eq!(r.verse_end, Some(6));
        // And a genuine whole chapter — no verse marker anywhere — is untouched.
        assert!(one("turn to psalm 23").whole_chapter);
        assert!(one("psalm chapter 23").whole_chapter);
    }

    /// Truncation must not swallow the rest of the sentence. `detect_direct`'s
    /// scanner advances a token on a failed parse, so a later, complete reference
    /// in the same window still lands — which is the whole reason returning None
    /// here is safe rather than lossy.
    #[test]
    fn a_truncated_reference_does_not_hide_a_complete_one_after_it() {
        let ms = detect_direct("romans 8 verse and then we turn to john chapter 3 verse 16");
        assert_eq!(ms.len(), 1, "got {ms:?}");
        refeq(&ms[0], "John", 3, 16);
    }

    /// `at_tail` is the fact `emit_detections` gates the whole-chapter guard on: it
    /// must mean "nothing followed this reference", and nothing more.
    #[test]
    fn at_tail_marks_only_a_reference_with_nothing_after_it() {
        assert!(one("and we read again in john chapter 3").at_tail);
        assert!(one("john 3:16").at_tail);
        // Trailing punctuation is not a word — `normalize` drops it, so a reference
        // ending a sentence is still at the tail. This is the shape the guard is
        // actually for: whisper emits "…John chapter 3." mid-utterance.
        assert!(one("and we read again in john chapter 3.").at_tail);
        // Something genuinely follows.
        assert!(!one("john chapter 3 verse 16 for god so loved the world").at_tail);
        assert!(!one("john 3:16 is the verse").at_tail);
        // A range is absorbed before the tail test, so the tail is past the range.
        let r = one("psalm 23 verses 1 to 6");
        assert_eq!(r.verse_end, Some(6));
        assert!(r.at_tail);
    }

    /// The guard is deliberately narrow, and THIS is the test that holds it narrow.
    ///
    /// A complete reference at the tail must keep firing instantly — that is the
    /// common case, and suppressing every tail match on a partial would delay
    /// essentially every auto-fire by about a second, trading this bug for a latency
    /// regression against SPEC's 3-second budget. Widen `is_provisional` past
    /// whole-chapter-at-tail and this fails.
    #[test]
    fn a_complete_reference_at_the_tail_is_not_treated_as_provisional() {
        let m = one("turn with me to john chapter 3 verse 16");
        assert!(m.at_tail, "precondition: it does end the text");
        assert!(
            !m.is_provisional(false),
            "a finished reference is not provisional just because it ends the window"
        );
        assert!(m.confidence >= crate::router::Thresholds::default().auto_fire);
    }

    /// `is_provisional` is the one place the rule lives — `emit_detections` and
    /// `stt::bench::engine_shootout` both call it — so its truth table is pinned
    /// here rather than inferred from either caller.
    #[test]
    fn only_a_growing_whole_chapter_at_the_tail_is_provisional() {
        // The shape the guard exists for: whole chapter, nothing after, still growing.
        let partial = one("and we read again in john chapter 3");
        assert!(partial.whole_chapter && partial.at_tail);
        assert!(partial.is_provisional(false));

        // Same words, but the utterance CLOSED. The preacher meant the chapter.
        assert!(
            !partial.is_provisional(true),
            "a final transcript will not grow — this must be allowed through"
        );

        // Whole chapter, but more text followed, so it was never truncated.
        let midtext = one("we read john chapter 3 and then we prayed");
        assert!(midtext.whole_chapter && !midtext.at_tail);
        assert!(!midtext.is_provisional(false));

        // Not a whole chapter at all.
        assert!(!one("john 3:16").is_provisional(false));
    }

    /// THE regression from the live service of 2026-07-26.
    ///
    /// The rolling STT window is decoded about once a second, so one utterance is
    /// parsed repeatedly at varying completeness. A preacher genuinely preaching
    /// Hebrews 4:2 produced, five seconds apart:
    ///
    ///     Hebrews 4:2  0.55   ← the whole reference was heard
    ///     Hebrews 4:1  0.83   ← only "Hebrews four" survived that pass
    ///
    /// The less complete parse outranked the complete one and replaced the right
    /// verse on the wall with the wrong one. A partial hearing of a reference may
    /// never outrank a full one.
    #[test]
    fn a_bare_chapter_never_outranks_the_full_reference_it_is_a_fragment_of() {
        let full = one("hebrews four two");
        refeq(&full, "Hebrews", 4, 2);
        let fragment = one("hebrews four");
        refeq(&fragment, "Hebrews", 4, 1);
        assert!(
            fragment.confidence < full.confidence,
            "the fragment ({}) outscored the full reference ({}) — it would \
             overwrite the correct verse on the projector",
            fragment.confidence,
            full.confidence
        );
    }

    /// THE SCREENSHOT BUG, live 2026-07-26. Whisper runs "six sixty-three"
    /// together into `663`, the parser read it as a chapter, and the operator was
    /// offered `John 663:1`. Every one of these was a verse they then fired BY
    /// HAND — Relay heard the reference correctly and mangled the number.
    #[test]
    fn a_run_together_chapter_verse_is_repaired_not_read_as_a_chapter() {
        for (text, book, ch, vs) in [
            ("john 663", "John", 6, 63),
            ("hebrews 416", "Hebrews", 4, 16),
            ("mark 1124", "Mark", 11, 24),
            ("romans 828", "Romans", 8, 28),
            ("john 1623", "John", 16, 23),
            ("john 316", "John", 3, 16),
            ("psalms 1191", "Psalms", 119, 1),
            ("matthew 2820", "Matthew", 28, 20),
        ] {
            let m = one(text);
            refeq(&m, book, ch, vs);
            assert!(!m.whole_chapter, "{text:?} is a verse, not a whole chapter");
        }
    }

    /// AND IT MUST STOP THERE. A run that is a real chapter of the book stays a
    /// whole-chapter reference — splitting "Psalm 23" into 2:3 would be the same
    /// class of bug pointing the other way.
    #[test]
    fn a_real_chapter_is_never_split_into_chapter_and_verse() {
        for (text, book, ch) in [
            ("psalm 23", "Psalms", 23),
            ("genesis 11", "Genesis", 11),
            ("psalm 119", "Psalms", 119),
            ("revelation 21", "Revelation", 21),
        ] {
            let m = one(text);
            refeq(&m, book, ch, 1);
            assert!(m.whole_chapter, "{text:?} must stay a whole chapter");
        }
    }

    /// An ambiguous split is refused rather than guessed — the same rule
    /// `fuzzy_book` follows. `Psalms 1015` is both 101:5 and 10:15, and both are
    /// real verses; there is no evidence to choose between them.
    #[test]
    fn an_ambiguous_run_is_refused_rather_than_guessed() {
        assert_eq!(split_run_into_chapter_verse("Psalms", 1015), None);
        // A run with no valid reading at all is also refused.
        assert_eq!(split_run_into_chapter_verse("Romans", 8128), None);
        // And an unknown book cannot be repaired.
        assert_eq!(split_run_into_chapter_verse("Nowhere", 663), None);
    }

    /// The verse-count table is a `const`, so nothing forces it to match the
    /// Bible actually shipped. This does. If `kjv.json` is ever replaced and the
    /// table is not regenerated, `split_run_into_chapter_verse` starts inventing
    /// references — silently, and only for the books that changed.
    #[test]
    fn the_verse_count_table_matches_the_bundled_kjv() {
        #[derive(serde::Deserialize)]
        struct KjvBook {
            chapters: Vec<Vec<String>>,
        }
        const RAW: &str = include_str!("../data/kjv.json");
        let books: Vec<KjvBook> =
            serde_json::from_str(RAW.trim_start_matches('\u{feff}')).expect("kjv.json parses");

        assert_eq!(books.len(), VERSES_PER_CHAPTER.len(), "book count");
        assert_eq!(books.len(), CANONICAL_BOOKS.len(), "book count vs names");
        for (i, book) in books.iter().enumerate() {
            let name = CANONICAL_BOOKS[i];
            assert_eq!(
                book.chapters.len(),
                VERSES_PER_CHAPTER[i].len(),
                "{name}: chapter count"
            );
            for (c, chapter) in book.chapters.iter().enumerate() {
                assert_eq!(
                    chapter.len(),
                    VERSES_PER_CHAPTER[i][c] as usize,
                    "{name} chapter {}: verse count",
                    c + 1
                );
            }
        }
        // Spot-check the lookups the repair actually depends on.
        assert_eq!(chapter_count("John"), 21);
        assert_eq!(chapter_count("Hebrews"), 13);
        assert_eq!(verse_count("John", 6), 71);
        assert_eq!(verse_count("Psalms", 119), 176);
        assert_eq!(verse_count("John", 22), 0, "out of range");
    }

    /// Every single-chapter book is also an ordinary word or a name in English
    /// preaching — Jude, Philemon, Obadiah, and the "John" inside 2/3 John. From
    /// the live service of 2026-07-26 these auto-fired to a congregation off
    /// ordinary speech: Jude 1:1, Jude 1:2, 2 John 1:2, 2 John 1:3.
    #[test]
    fn a_single_chapter_book_and_a_bare_number_asks_a_human_but_a_keyword_fires() {
        let auto_bar = crate::router::Thresholds::default().auto_fire;

        let bare = one("jude four");
        refeq(&bare, "Jude", 1, 4);
        assert!(
            bare.confidence < auto_bar,
            "bare 'jude four' at {} would reach the congregation unasked",
            bare.confidence
        );

        // Stating the intent still fires — that is the whole distinction.
        let kw = one("jude verse four");
        refeq(&kw, "Jude", 1, 4);
        assert!(
            kw.confidence >= auto_bar,
            "'jude verse four' is an explicit reference and must still fire ({})",
            kw.confidence
        );
    }

    /// Ordinary preaching that is NOT a reference, from the same live service.
    /// Every one of these auto-fired to a congregation.
    #[test]
    fn a_book_name_followed_by_a_spoken_number_does_not_fire_on_its_own() {
        let auto_bar = crate::router::Thresholds::default().auto_fire;
        for text in [
            "matthew one of the twelve disciples",
            "and the lord said to john two of them",
            "job one of the oldest books",
        ] {
            for m in detect_direct(text) {
                assert!(
                    m.confidence < auto_bar,
                    "{text:?} → {} {}:{} at {} would reach the congregation unasked",
                    m.reference.book,
                    m.reference.chapter,
                    m.reference.verse,
                    m.confidence
                );
            }
        }
    }

    #[test]
    fn hyphen_range_colon_form() {
        // "Psalm 23:1-6" tokenizes to "23:1" + "6"; range end = 6.
        let m = one("psalm 23:1-6");
        refeq(&m, "Psalms", 23, 1);
        assert_eq!(m.verse_end, Some(6));
    }

    #[test]
    fn hyphen_range_bare_digits() {
        let m = one("ps 23 1-6");
        refeq(&m, "Psalms", 23, 1);
        assert_eq!(m.verse_end, Some(6));
    }

    #[test]
    fn spoken_range_with_connector() {
        let m = one("john three sixteen to eighteen");
        refeq(&m, "John", 3, 16);
        assert_eq!(m.verse_end, Some(18));
    }

    #[test]
    fn descending_range_is_rejected() {
        // A trailing smaller number is not a valid range end (kept single).
        let m = one("john 3:16 to 2");
        refeq(&m, "John", 3, 16);
        assert_eq!(m.verse_end, None);
    }

    #[test]
    fn context_walks_whole_chapter_then_stops_at_span_end() {
        let mut ctx = ContextMemory::default();
        // Fire Psalm 23 as a whole chapter of 6 verses.
        ctx.note_passage(
            &VerseRef {
                book: "Psalms".into(),
                chapter: 23,
                verse: 1,
            },
            Some(6),
        );
        // Walk 1→2→…→6, preserving the span at each step.
        for expected in 2..=6 {
            let n = ctx.next_verse().expect("should have a next verse");
            assert_eq!(n.verse, expected);
            ctx.advance(&n);
        }
        // At verse 6 (span end) → no further next.
        assert!(ctx.next_verse().is_none());
    }

    // ---- A3/A4: topical / cross-reference suggestions ---------------------

    #[test]
    fn topical_theme_surfaces_related_scripture() {
        let s = suggest_related("i have been so afraid and anxious lately", None, 4).unwrap();
        assert_eq!(s.theme, "Fear & Anxiety");
        assert!(!s.refs.is_empty());
        // Ranges in the concordance carry through as verse_end.
        assert!(s.refs.iter().any(|m| m.verse_end.is_some()));
    }

    #[test]
    fn related_excludes_the_anchor_verse() {
        let anchor = VerseRef {
            book: "John".into(),
            chapter: 3,
            verse: 16,
        };
        let s = suggest_related("God's great love for us", Some(&anchor), 4).unwrap();
        assert!(!s.refs.iter().any(|m| m.reference == anchor));
    }

    #[test]
    fn no_theme_no_suggestion() {
        assert!(suggest_related("the quarterly budget meeting is on tuesday", None, 4).is_none());
    }

    // ---- D3: spoken clear / blackout command ------------------------------

    #[test]
    fn detects_clear_and_blackout_commands() {
        assert!(detect_clear("clear the screen"));
        assert!(detect_clear("blank the screens"));
        assert!(detect_clear("blackout"));
        assert!(detect_clear("black out"));
        assert!(detect_clear("take it down"));
    }

    #[test]
    fn clear_command_ignores_prose() {
        // "clear"/"blank" without a screen object must not fire mid-sermon.
        assert!(!detect_clear("the gospel makes it very clear to us"));
        assert!(!detect_clear("make it clear"));
        assert!(!detect_clear("a blank page before creation"));
    }

    // ---- In-passage voice jump: "chapter 5 verse 1" resolves in current book ----

    #[test]
    fn passage_nav_chapter_and_verse() {
        assert_eq!(
            detect_passage_nav("chapter 5 verse 1"),
            Some(PassageNav {
                chapter: Some(5),
                verse: Some(1)
            })
        );
        assert_eq!(
            detect_passage_nav("let's go to chapter fifty verse two"),
            Some(PassageNav {
                chapter: Some(50),
                verse: Some(2)
            })
        );
        // Chapter only → the caller defaults to verse 1.
        assert_eq!(
            detect_passage_nav("go to chapter 5"),
            Some(PassageNav {
                chapter: Some(5),
                verse: None
            })
        );
    }

    #[test]
    fn passage_nav_bare_verse_singular_and_plural() {
        // Both "verse" and "verses" — stay in the same chapter, change the verse.
        assert_eq!(
            detect_passage_nav("verse 4"),
            Some(PassageNav {
                chapter: None,
                verse: Some(4)
            })
        );
        assert_eq!(
            detect_passage_nav("verses 4"),
            Some(PassageNav {
                chapter: None,
                verse: Some(4)
            })
        );
    }

    #[test]
    fn passage_nav_ignores_named_book_and_prose() {
        assert!(detect_passage_nav("turn to psalm 5 verse 1").is_none()); // book named
        assert!(detect_passage_nav("the fifth chapter of our lives").is_none()); // no number
        assert!(detect_passage_nav("we had over five hundred members today").is_none());
        // no keyword
    }

    #[test]
    fn single_verse_note_clears_any_prior_span() {
        let mut ctx = ContextMemory::default();
        ctx.note_passage(
            &VerseRef {
                book: "John".into(),
                chapter: 3,
                verse: 16,
            },
            Some(18),
        );
        // A fresh single-verse detection resets the passage — next is unbounded.
        ctx.note(&VerseRef {
            book: "Romans".into(),
            chapter: 8,
            verse: 28,
        });
        assert_eq!(ctx.next_verse().unwrap().verse, 29);
    }

    #[test]
    fn psalm_alias_and_words() {
        refeq(
            &one("open your bibles to psalm twenty three verse four"),
            "Psalms",
            23,
            4,
        );
    }

    #[test]
    fn phonetic_free_for_three_lowers_confidence() {
        let m = one("john free sixteen");
        refeq(&m, "John", 3, 16);
        assert!(
            m.confidence < 0.90,
            "phonetic correction should cost confidence"
        );
    }

    #[test]
    fn hundreds() {
        refeq(
            &one("psalm one hundred nineteen verse one"),
            "Psalms",
            119,
            1,
        );
    }

    #[test]
    fn genesis_one_one() {
        refeq(&one("in genesis one one god created"), "Genesis", 1, 1);
    }

    #[test]
    fn two_references_in_one_window() {
        let v = detect_direct("as john 3:16 says and also romans eight twenty eight");
        assert_eq!(v.len(), 2);
        refeq(&v[0], "John", 3, 16);
        refeq(&v[1], "Romans", 8, 28);
    }

    #[test]
    fn no_false_positive_on_plain_prose() {
        assert!(detect_direct("for God so loved the world that he gave").is_empty());
    }

    #[test]
    fn book_without_numbers_is_ignored() {
        assert!(detect_direct("the gospel of john tells us").is_empty());
    }

    #[test]
    fn matched_text_is_captured() {
        let m = one("see John 3:16 now");
        assert_eq!(m.matched_text, "john 3:16");
    }

    #[test]
    fn numbered_books_spoken_and_written() {
        refeq(
            &one("turn to first corinthians thirteen four"),
            "1 Corinthians",
            13,
            4,
        );
        refeq(&one("2 timothy 3:16"), "2 Timothy", 3, 16);
        refeq(&one("second peter one twenty one"), "2 Peter", 1, 21);
    }

    #[test]
    fn multiword_and_variant_book_names() {
        refeq(&one("song of solomon two one"), "Song of Solomon", 2, 1);
        refeq(&one("revelations 22:21"), "Revelation", 22, 21);
    }

    #[test]
    fn single_chapter_books_bare_verse() {
        // Bare verse → chapter 1.
        refeq(&one("look at jude four"), "Jude", 1, 4);
        refeq(&one("jude 4"), "Jude", 1, 4);
        refeq(&one("philemon verse six"), "Philemon", 1, 6);
        refeq(&one("second john four"), "2 John", 1, 4);
        // Explicit forms still respected.
        refeq(&one("jude 1:4"), "Jude", 1, 4);
        refeq(&one("obadiah verse twenty one"), "Obadiah", 1, 21);
    }

    #[test]
    fn single_chapter_book_without_number_is_ignored() {
        assert!(detect_direct("the epistle of jude warns us").is_empty());
    }

    // --- context memory ---

    #[test]
    fn bare_verse_resolves_against_current_passage() {
        let mut ctx = ContextMemory::default();
        assert!(ctx.resolve_bare_verse(4).is_none()); // nothing yet
        ctx.note(&VerseRef {
            book: "Psalms".into(),
            chapter: 23,
            verse: 1,
        });
        assert_eq!(detect_bare_verses("now look at verse four"), vec![4]);
        assert_eq!(
            ctx.resolve_bare_verse(4).unwrap(),
            VerseRef {
                book: "Psalms".into(),
                chapter: 23,
                verse: 4
            }
        );
    }

    #[test]
    fn detect_bare_verses_reads_digits_and_words() {
        assert_eq!(
            detect_bare_verses("verse 4 and verse twenty-eight"),
            vec![4, 28]
        );
        assert!(detect_bare_verses("no reference here").is_empty());
    }

    #[test]
    fn nav_commands() {
        assert_eq!(detect_command("next"), Some(NavCommand::Next));
        assert_eq!(detect_command("go to the next"), Some(NavCommand::Next));
        assert_eq!(detect_command("back please"), Some(NavCommand::Previous));
        assert_eq!(detect_command("previous verse"), Some(NavCommand::Previous));
        // Long sentences are not treated as commands.
        assert_eq!(
            detect_command("and the next thing he said in his sermon"),
            None
        );
    }

    #[test]
    fn context_next_prev() {
        let mut ctx = ContextMemory::default();
        assert!(ctx.next_verse().is_none());
        ctx.note(&VerseRef {
            book: "John".into(),
            chapter: 3,
            verse: 16,
        });
        assert_eq!(
            ctx.next_verse().unwrap(),
            VerseRef {
                book: "John".into(),
                chapter: 3,
                verse: 17
            }
        );
        assert_eq!(
            ctx.prev_verse().unwrap(),
            VerseRef {
                book: "John".into(),
                chapter: 3,
                verse: 15
            }
        );
        ctx.note(&VerseRef {
            book: "Jude".into(),
            chapter: 1,
            verse: 1,
        });
        assert!(ctx.prev_verse().is_none()); // verse 1 → no previous
    }

    #[test]
    fn ambiguous_two_digit_gives_candidates() {
        let c = detect_ambiguous("turn to revelation twenty two");
        assert!(c.contains(&VerseRef {
            book: "Revelation".into(),
            chapter: 22,
            verse: 1
        }));
        assert!(c.contains(&VerseRef {
            book: "Revelation".into(),
            chapter: 2,
            verse: 2
        }));
    }

    #[test]
    fn full_reference_is_not_ambiguous() {
        assert!(detect_ambiguous("john 3:16").is_empty());
        assert!(detect_ambiguous("romans eight twenty eight").is_empty());
    }

    // ── REPAIRING A MISHEARD BOOK NAME ──────────────────────────────────
    //
    // Asked for directly: "the audio should be ultra sensitive to African tone
    // — e.g. Sam 23 → Psalm 23". The hand-written alias list covers the
    // mishears someone thought of; this covers the ones they did not, because a
    // list can only ever be as good as the last service that surprised it.

    #[test]
    fn repairs_book_names_the_alias_list_never_listed() {
        // None of these are in the alias table. All are one or two edits from a
        // real book, and all are the kind of thing whisper emits on
        // African-accented English.
        refeq(&one("psam 23 verse 1"), "Psalms", 23, 1);
        refeq(&one("salmon 23 verse 1"), "Psalms", 23, 1);
        refeq(&one("matthews 5 verse 3"), "Matthew", 5, 3);
        refeq(&one("romands 8 verse 1"), "Romans", 8, 1);
        refeq(&one("ephesian 2 verse 8"), "Ephesians", 2, 8);
    }

    #[test]
    fn a_repaired_book_is_a_weaker_claim_than_an_exact_one() {
        // It still detects — but it must not carry the same confidence, because
        // a Direct match is the only kind allowed onto a screen unattended.
        let exact = one("psalms 23 verse 1");
        let fixed = one("psam 23 verse 1");
        assert!(
            fixed.confidence < exact.confidence,
            "a guessed book name cost nothing: exact={} repaired={}",
            exact.confidence,
            fixed.confidence
        );
    }

    #[test]
    fn ordinary_sermon_words_are_never_repaired_into_books() {
        // THE FAILURE THIS MUST NOT HAVE. "among" is two edits from "amos",
        // "same" two from "james", "gone" close to "john". A preacher saying
        // "among 3 or 4 of them" must not put Amos on the wall.
        for phrase in [
            "among 3 of them",
            "same 5 people",
            "gone 4 times",
            "good 3 things",
            "word 3 times",
            "come 2 by 2",
        ] {
            assert!(
                detect_direct(phrase).is_empty(),
                "an ordinary phrase was repaired into a reference: {phrase:?}"
            );
        }
    }

    #[test]
    fn a_repair_needs_a_reference_shaped_sentence() {
        // The gate that makes the whole thing safe: no chapter number after it,
        // no repair. "psam" on its own is just a word.
        assert!(detect_direct("psam is a lovely word").is_empty());
        assert!(detect_direct("he read from psam and sat down").is_empty());
        // ...but the same token followed by a number is a reference.
        refeq(&one("psam 23"), "Psalms", 23, 1);
    }

    #[test]
    fn an_ambiguous_repair_is_refused_rather_than_guessed() {
        // "job" and "joel" are both one edit from "joe". With no evidence to
        // choose, guessing would put one of two unrelated books on a wall.
        let hits = detect_direct("joe 2 verse 1");
        assert!(
            hits.is_empty() || hits[0].reference.book != "Job",
            "an ambiguous book repair was guessed instead of refused"
        );
    }

    #[test]
    fn very_short_tokens_are_never_repaired() {
        // At two characters everything is one edit from everything.
        assert!(detect_direct("am 3 verse 1").is_empty());
        assert!(detect_direct("is 5 verse 2").is_empty());
    }

    #[test]
    fn the_distance_bound_actually_bounds() {
        assert_eq!(edit_distance_within("psam", "psalm", 2), Some(1));
        assert_eq!(edit_distance_within("sam", "psalms", 2), None);
        assert_eq!(edit_distance_within("abc", "xyz", 2), None);
        assert_eq!(edit_distance_within("same", "same", 0), Some(0));
    }

    #[test]
    fn phonetic_book_sam_is_psalms() {
        refeq(&one("sam twenty three verse one"), "Psalms", 23, 1);
    }

    #[test]
    fn fast_search_abbreviations() {
        refeq(&one("ps 23 1"), "Psalms", 23, 1);
        refeq(&one("rom 8 1"), "Romans", 8, 1);
        refeq(&one("1 jn 3 1"), "1 John", 3, 1);
        refeq(&one("1jn 3 1"), "1 John", 3, 1);
        refeq(&one("mt 5 3"), "Matthew", 5, 3);
        refeq(&one("rev 22 1"), "Revelation", 22, 1);
        refeq(&one("2 co 5 17"), "2 Corinthians", 5, 17);
    }

    #[test]
    fn asr_possessive_book_name() {
        // whisper often mishears "Psalms 23" as "Sam's 23"; the apostrophe must
        // not split the token.
        refeq(&one("read from Sam's 23, verse 1"), "Psalms", 23, 1);
        refeq(&one("sam\u{2019}s 23 verse 1"), "Psalms", 23, 1);
    }

    // --- semantic match ---

    fn seed_index() -> SemanticIndex {
        let corpus = vec![
            (
                VerseRef { book: "John".into(), chapter: 3, verse: 16 },
                "For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish, but have everlasting life.".to_string(),
            ),
            (
                VerseRef { book: "Psalms".into(), chapter: 23, verse: 1 },
                "The LORD is my shepherd; I shall not want.".to_string(),
            ),
            (
                VerseRef { book: "Romans".into(), chapter: 8, verse: 28 },
                "And we know that all things work together for good to them that love God.".to_string(),
            ),
        ];
        SemanticIndex::build(&corpus)
    }

    #[test]
    fn semantic_matches_paraphrase_by_overlap() {
        let idx = seed_index();
        // Paraphrase of John 3:16 with shared content words.
        let hits = idx.top_k("god loved the world and gave his son so we have life", 1);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0.book, "John");
        assert!(hits[0].1 > 0.2, "similarity too low: {}", hits[0].1);
    }

    /// A WHOLE SENTENCE, not two words. Since 2026-09-20 this matcher requires
    /// three shared words with no exception, so "the lord is my shepherd" —
    /// which is `lord` and `shepherd` once the stopwords are gone — is no longer
    /// its job. It is `PhraseIndex`'s, which finds it as a five-word quotation
    /// and can quote it back. The division is deliberate: one index answers
    /// "which verse did he READ", this one answers "which verse does he MEAN",
    /// and a two-word probe was only ever testing the first through the second.
    #[test]
    fn semantic_picks_shepherd_for_shepherd_query() {
        let idx = seed_index();
        let hits = idx.top_k("the lord is my shepherd and i shall not want", 1);
        assert_eq!(hits[0].0.reference_book_chapter_verse(), "Psalms 23:1");
    }

    #[test]
    fn semantic_empty_on_no_content_overlap() {
        let idx = seed_index();
        assert!(idx.top_k("xyzzy plugh frobnicate", 1).is_empty());
    }

    // ── KJV gloss: modern speech against a 1611 text ────────────────────────

    /// The whole point: a word that appears NOWHERE in the KJV still finds the
    /// verse. Reintroduce the un-glossed query path and this fails — "pigs" and
    /// "dad" share not one token with the text.
    #[test]
    fn gloss_finds_the_verse_through_modern_words() {
        let corpus =
            vec![
            (
                VerseRef { book: "Luke".into(), chapter: 15, verse: 16 },
                "And he would fain have filled his belly with the husks that the swine did eat."
                    .to_string(),
            ),
            (
                VerseRef { book: "Genesis".into(), chapter: 1, verse: 1 },
                "In the beginning God created the heaven and the earth.".to_string(),
            ),
        ];
        let idx = SemanticIndex::build(&corpus);
        // A RETELLING, not a probe. "pigs" is still the word that does it — it
        // appears nowhere in the KJV and only the gloss turns it into "swine" —
        // but three shared words are required with no exception since
        // 2026-09-20, and a preacher telling this story says a sentence.
        let hits = idx.top_k(
            "he was so hungry he would have filled his belly with what the pigs were eating",
            1,
        );
        assert_eq!(hits.len(), 1, "modern wording found nothing");
        assert_eq!(hits[0].0.reference_book_chapter_verse(), "Luke 15:16");
    }

    /// The gloss ADDS evidence, never replaces it: a retelling that already uses
    /// the KJV word must not be made worse by glossing something else in it.
    #[test]
    fn gloss_keeps_the_original_token() {
        let expanded = expand_with_gloss(vec!["swine".into(), "pigs".into()]);
        assert!(expanded.contains(&"swine".to_string()));
        assert!(expanded.contains(&"pigs".to_string()));
    }

    /// THE architectural invariant. The gloss is a QUERY-time expansion only.
    /// Glossing the corpus would change document frequencies, and it is exactly
    /// how rare a word like "husks" is that makes it such a strong signal —
    /// inflating those counts would quietly degrade every other match.
    #[test]
    fn gloss_never_touches_the_index() {
        // "boat" is modern; the corpus keeps it verbatim, so a KJV-word query
        // ("ship") must NOT reach it — expansion runs one way, on the query.
        let corpus = vec![(
            VerseRef {
                book: "Mark".into(),
                chapter: 4,
                verse: 37,
            },
            "the waves beat into the boat".to_string(),
        )];
        let idx = SemanticIndex::build(&corpus);
        assert!(
            idx.top_k("ship", 1).is_empty(),
            "the index was glossed — document frequencies are now wrong"
        );
        // ...while the modern query still finds the modern text unaided. A whole
        // clause rather than the bare word, because three shared words are now
        // required with no exception — which does not weaken what this test
        // claims: `boat` is still the word doing the work, and `ship` still
        // reaches nothing.
        assert!(!idx.top_k("the waves beat into the boat", 1).is_empty());
    }

    /// Identical input must produce a bit-identical score, every time.
    ///
    /// It did not: `cosine` summed over a HashMap, whose iteration order varies
    /// per instance, and float addition is not associative. `SEMANTIC_FLOOR`
    /// gates on this number, so a borderline paraphrase could be suggested on
    /// one run and silently dropped on the next from the same words.
    #[test]
    fn the_same_query_always_scores_the_same() {
        let idx = seed_index();
        let q = "god so loved the world that he gave his only son to have life";
        let first = idx.top_k(q, 3);
        for _ in 0..25 {
            let again = idx.top_k(q, 3);
            assert_eq!(first.len(), again.len());
            for (a, b) in first.iter().zip(again.iter()) {
                assert_eq!(a.1.to_bits(), b.1.to_bits(), "score drifted between runs");
            }
        }
    }

    /// A gloss that names an answer is a cheat, not a gloss. Story-specific
    /// proper nouns must never appear as keys, or the benchmark measures itself.
    #[test]
    fn gloss_contains_no_story_specific_giveaways() {
        for key in kjv_gloss().keys() {
            for banned in ["samaritan", "sycomore", "zacchaeus", "lazarus", "goliath"] {
                assert_ne!(key.as_str(), banned, "gloss key '{key}' names an answer");
            }
        }
    }

    /// A paraphrase match must be able to SAY WHY.
    ///
    /// Its score is a cosine, not a probability, so "61%" tells the operator nothing
    /// they can act on. The overlapping words do: an operator who sees
    /// `shepherd · lord` can agree or disagree with that in the second they have.
    #[test]
    fn a_paraphrase_can_explain_itself_in_words() {
        let idx = seed_index();
        const SAID: &str = "the lord is my shepherd and i shall not want";
        let hits = idx.top_k_explained(SAID, 1);
        let (r, _score, terms) = &hits[0];
        assert_eq!(r.reference_book_chapter_verse(), "Psalms 23:1");
        assert!(
            terms.contains(&"shepherd".to_string()),
            "the rarest shared word must be shown: {terms:?}"
        );
        // Only words the query and the verse actually SHARE — an "explanation"
        // listing words that were not in the sermon would be a fabricated one.
        for t in terms {
            assert!(SAID.contains(t.as_str()), "{t:?} was never spoken");
        }
    }

    /// The strongest evidence comes first, and it is the word that most narrows the
    /// corpus down — not the one the operator happens to say most often. "shepherd"
    /// identifies Psalm 23; "lord" is in half the Bible and identifies nothing.
    ///
    /// This needs a corpus where "lord" is actually common, which the 3-verse
    /// `seed_index` is not: there, both words appear exactly once, their idf is
    /// identical, and the ranking is a tie broken alphabetically. The distinction
    /// being asserted here only exists at corpus scale — so build one.
    #[test]
    fn the_explanation_is_ranked_by_evidence_not_by_frequency() {
        let corpus = vec![
            (
                VerseRef {
                    book: "Psalms".into(),
                    chapter: 23,
                    verse: 1,
                },
                "The LORD is my shepherd; I shall not want.".to_string(),
            ),
            (
                VerseRef {
                    book: "Psalms".into(),
                    chapter: 24,
                    verse: 1,
                },
                "The earth is the LORD's, and the fulness thereof.".to_string(),
            ),
            (
                VerseRef {
                    book: "Psalms".into(),
                    chapter: 27,
                    verse: 1,
                },
                "The LORD is my light and my salvation; whom shall I fear?".to_string(),
            ),
            (
                VerseRef {
                    book: "Psalms".into(),
                    chapter: 100,
                    verse: 2,
                },
                "Serve the LORD with gladness: come before his presence with singing.".to_string(),
            ),
        ];
        let idx = SemanticIndex::build(&corpus);
        let hits = idx.top_k_explained("the lord is my shepherd i shall not want", 1);
        assert_eq!(hits[0].0.reference_book_chapter_verse(), "Psalms 23:1");
        assert_eq!(
            hits[0].2.first().map(String::as_str),
            Some("shepherd"),
            "the rare word must lead the explanation: {:?}",
            hits[0].2
        );
    }

    /// The explanation is capped, so it stays readable in a dark booth.
    #[test]
    fn the_explanation_is_short_enough_to_read_at_a_glance() {
        let idx = seed_index();
        let hits =
            idx.top_k_explained("god loved the world and gave his only begotten son life", 1);
        assert!(hits[0].2.len() <= EXPLAIN_TERMS, "{:?}", hits[0].2);
    }

    /// `top_k` must keep agreeing with `top_k_explained` — it now delegates to it,
    /// and a divergence would mean the console explains a different verse than the
    /// one the gate actually routed.
    #[test]
    fn the_explained_and_plain_rankings_cannot_diverge() {
        let idx = seed_index();
        let q = "god loved the world and gave his son";
        let plain = idx.top_k(q, 3);
        let explained = idx.top_k_explained(q, 3);
        assert_eq!(plain.len(), explained.len());
        for (p, e) in plain.iter().zip(explained.iter()) {
            assert_eq!(
                p.0.reference_book_chapter_verse(),
                e.0.reference_book_chapter_verse()
            );
            assert_eq!(p.1, e.1);
        }
    }
}

#[cfg(test)]
mod perf {
    use super::*;
    use std::time::Instant;

    /// Not an assertion — a measurement, printed with `--nocapture`.
    ///
    /// SPEC's success criterion is "runs smoothly on an 8GB Windows laptop", and
    /// `top_k` is a full linear scan over ~31k verses that runs on EVERY
    /// transcript partial (roughly once a second, while a sermon is in progress).
    /// That *looks* like something to optimise, so it was measured before anyone
    /// did.
    ///
    /// Result (release, Apple silicon):
    ///   build  ≈ 112 ms — once, at startup, off the live path
    ///   top_k  ≈ 2.6 ms per query, at ~1 query/sec
    ///
    /// That is roughly a quarter of one percent of a core. Even several times
    /// slower on a weak Windows laptop it stays around 1%. **So the linear scan
    /// stays.** An inverted index would be real complexity bought with no
    /// measurable win — the scan is not the bottleneck, and this test exists so
    /// that claim can be re-checked rather than believed.
    ///
    /// (If the corpus ever grows well beyond one translation, re-run this first.)
    ///
    /// ── TWO THINGS THIS CORPUS CANNOT SEE, AND THE FIGURES IT GETS WRONG ─────
    ///
    /// The corpus below is 31,100 copies of ONE sentence with a serial number in
    /// it, which is the right size and the wrong shape. Measured against the real
    /// KJV by `measure_query_repair_on_the_real_corpus`, the same two numbers are
    /// **build ≈ 305 ms** and **top_k ≈ 4.6 ms** — not 112 and 2.6. The conclusion
    /// does not move (4.6 ms once a second is still under half a percent of a
    /// core, and the scan still stays), but the figures quoted above and in
    /// `CLAUDE.md` describe a synthetic corpus rather than the one that ships.
    ///
    /// And every word of the query below is in vocabulary, so `repair_query`
    /// returns at its first line for every token and is never exercised here at
    /// all — on the branch built for tier-1 languages, where unknown words are the
    /// normal case rather than the corner. That is measured in the other test too:
    /// **0.33 ms per unknown word**, which is why it is left alone.
    #[test]
    #[ignore = "measurement, not a test — run with --ignored --nocapture"]
    fn measure_semantic_top_k() {
        // A corpus the size of the real one.
        let corpus: Vec<(VerseRef, String)> = (0..31_100)
            .map(|i| {
                (
                    VerseRef {
                        book: "John".into(),
                        chapter: (i / 100) as i64 + 1,
                        verse: (i % 100) as i64 + 1,
                    },
                    format!(
                        "for god so loved the world that he gave his only begotten son {i} \
                         whosoever believeth in him should not perish everlasting life"
                    ),
                )
            })
            .collect();

        let t0 = Instant::now();
        let idx = SemanticIndex::build(&corpus);
        let build_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let query = "god loved the world and gave his son so we would not perish";
        // Warm, then time a realistic number of queries (one per transcript partial).
        let _ = idx.top_k(query, 1);
        let t1 = Instant::now();
        const N: usize = 100;
        for _ in 0..N {
            let _ = idx.top_k(query, 1);
        }
        let per_query_ms = t1.elapsed().as_secs_f64() * 1000.0 / N as f64;

        println!("\n  SemanticIndex over {} verses:", corpus.len());
        println!("    build:     {build_ms:.0} ms (once, at startup)");
        println!("    top_k:     {per_query_ms:.2} ms per query (~1 query/sec live)");
        println!();
    }

    /// THE SAME QUESTION, ASKED OF THE REAL CORPUS AND A REAL MISHEARING.
    ///
    /// `measure_semantic_top_k` above is the measurement every "the scan stays a
    /// linear scan" decision rests on, and it has one blind spot that matters more
    /// than the scan does: its corpus is 31,100 copies of ONE sentence and its
    /// query is entirely in-vocabulary, so `repair_query` returns at its first
    /// line (`self.idf.contains_key(t)`) for every token and is never measured at
    /// all.
    ///
    /// `repair_query` is the branch that cannot be reasoned about from the code.
    /// For every query token of four characters or more that the corpus has never
    /// seen, it walks **the whole vocabulary** and runs `edit_distance_within`
    /// against each candidate. There is a length pre-filter and no index. The
    /// tier-1 case is exactly the one that produces unknown tokens by the
    /// handful — whisper on Yorùbá, Swahili or code-switched preaching — so the
    /// worst case is not a corner, it is the target market.
    ///
    /// Why it is worth a number rather than an argument: this runs inside
    /// `emit_detections`, which holds `Db`, `Routing` and `Context` **together**
    /// for its whole body (`main.rs`), and ~120 of Relay's 167 Tauri commands open
    /// with `db.0.lock()`. Tauri runs a command that is not `async fn` on the MAIN
    /// THREAD, which on macOS is the UI run loop. So every millisecond spent here,
    /// once per transcript partial, is a millisecond in which an operator's click
    /// can be waiting — and "the console drags during a sermon" is what that looks
    /// like from the other side of the screen.
    ///
    /// Run: `cargo test --release perf::measure_query_repair -- --ignored --nocapture`
    #[test]
    #[ignore = "measurement, not a test — run with --ignored --nocapture"]
    fn measure_query_repair_on_the_real_corpus() {
        #[derive(serde::Deserialize)]
        struct KjvBook {
            chapters: Vec<Vec<String>>,
        }
        const RAW: &str = include_str!("../data/kjv.json");
        let books: Vec<KjvBook> =
            serde_json::from_str(RAW.trim_start_matches('\u{feff}')).expect("kjv.json parses");
        let mut corpus: Vec<(VerseRef, String)> = Vec::new();
        for (bi, b) in books.iter().enumerate() {
            for (ci, ch) in b.chapters.iter().enumerate() {
                for (vi, t) in ch.iter().enumerate() {
                    corpus.push((
                        VerseRef {
                            book: format!("Book{bi}"),
                            chapter: ci as i64 + 1,
                            verse: vi as i64 + 1,
                        },
                        t.clone(),
                    ));
                }
            }
        }

        let t0 = Instant::now();
        let idx = SemanticIndex::build(&corpus);
        let build_ms = t0.elapsed().as_secs_f64() * 1000.0;

        // IN VOCABULARY — what the existing benchmark measures.
        let known = "for god so loved the world that he gave his only begotten son                      that whosoever believeth in him should not perish";
        // OUT OF VOCABULARY — the same sentence as whisper hears it through an
        // accent. Every one of these is a word the Bible does not contain, and
        // every one is four characters or more, so every one walks the vocabulary.
        let misheard = "for godd soo lovedd the worlde thatt hee gavv hiss onlie begoten sunne                         thatt whoseover beleiveth inn himm shuld nott perishe";

        let bench = |q: &str| {
            let _ = idx.top_k(q, 1);
            let t = Instant::now();
            const N: usize = 20;
            for _ in 0..N {
                let _ = idx.top_k(q, 1);
            }
            t.elapsed().as_secs_f64() * 1000.0 / N as f64
        };
        // AND THE ORDINARY CASE, which is the one that decides whether any of this
        // matters: an English sentence with a couple of words whisper got wrong.
        // A worst case nobody meets is not a budget.
        let typical = "for god soo loved the world that he gave his only begoten son \
                       that whosoever believeth in him should not perish";
        let known_ms = bench(known);
        let typical_ms = bench(typical);
        let misheard_ms = bench(misheard);

        println!(
            "\n  REAL KJV corpus — {} verses, {} vocabulary terms",
            corpus.len(),
            idx.idf.len()
        );
        println!("    build:                    {build_ms:.0} ms (once, at startup)");
        println!("    top_k, every word known:  {known_ms:.2} ms per query");
        println!(
            "    top_k, 2 words misheard:  {typical_ms:.2} ms per query  <-- the ordinary case"
        );
        println!("    top_k, 20 words misheard: {misheard_ms:.2} ms per query <-- repair_query walks the vocabulary");
        println!(
            "    cost of one unknown word: {:.2} ms",
            (misheard_ms - known_ms) / 20.0
        );
        println!();
    }
}

#[cfg(test)]
mod tier1_languages {
    use super::*;

    fn refs(text: &str) -> Vec<String> {
        detect_direct(text)
            .iter()
            .map(|m| {
                format!(
                    "{} {}:{}",
                    m.reference.book, m.reference.chapter, m.reference.verse
                )
            })
            .collect()
    }

    /// THE test. Before the tier-1 alias table existed, every one of these
    /// returned NOTHING — a perfect Yorùbá acoustic model would have detected
    /// zero verses, because the detector had never heard of "Jòhánù".
    #[test]
    fn detects_a_verse_spoken_in_yoruba() {
        assert_eq!(refs("Jòhánù 3:16"), ["John 3:16"]);
        assert_eq!(refs("Sáàmù 23:1"), ["Psalms 23:1"]);
        assert_eq!(refs("Róòmù 8:28"), ["Romans 8:28"]);
        assert_eq!(refs("Ìfihàn 22:1"), ["Revelation 22:1"]);
    }

    #[test]
    fn detects_a_verse_spoken_in_swahili() {
        assert_eq!(refs("Yohana 3:16"), ["John 3:16"]);
        assert_eq!(refs("Zaburi 23:1"), ["Psalms 23:1"]);
        assert_eq!(refs("Warumi 8:28"), ["Romans 8:28"]);
        assert_eq!(refs("Mathayo 5:9"), ["Matthew 5:9"]);
        assert_eq!(refs("Ufunuo 22:1"), ["Revelation 22:1"]);
    }

    #[test]
    fn detects_a_verse_spoken_in_hausa() {
        assert_eq!(refs("Yahaya 3:16"), ["John 3:16"]);
        assert_eq!(refs("Zabura 23:1"), ["Psalms 23:1"]);
        assert_eq!(refs("Romawa 8:28"), ["Romans 8:28"]);
        assert_eq!(refs("Farawa 1:1"), ["Genesis 1:1"]);
    }

    /// Whisper emits tone marks unreliably — the same audio yields "Jòhánù",
    /// "Johánù" or "Johanu" depending on the recording. All must land on the same
    /// verse, or detection becomes a coin-flip on the quality of the microphone.
    #[test]
    fn tone_marks_and_dots_below_are_optional() {
        for spelling in ["Jòhánù", "Johánù", "Johanu", "JOHANU", "jòhanù"] {
            assert_eq!(
                refs(&format!("{spelling} 3:16")),
                ["John 3:16"],
                "failed on {spelling:?}"
            );
        }
        // Dots-below (Yorùbá) and the Hausa glottal both fold away.
        assert_eq!(refs("Jẹ́nẹ́sísì 1:1"), ["Genesis 1:1"]);
        assert_eq!(refs("Ru'ya ta Yohanna 22:1"), ["Revelation 22:1"]);
    }

    /// Multi-word book names must match as a unit — Swahili and Yorùbá are full
    /// of them, and a greedy single-token match would find the wrong book.
    #[test]
    fn multi_word_book_names_match_as_a_unit() {
        assert_eq!(refs("Matendo ya Mitume 2:38"), ["Acts 2:38"]);
        assert_eq!(refs("Mambo ya Walawi 19:18"), ["Leviticus 19:18"]);
        assert_eq!(refs("Ayyukan Manzanni 2:38"), ["Acts 2:38"]);
    }

    /// Numbered books, in-language.
    #[test]
    fn numbered_books_work_in_language() {
        assert_eq!(refs("1 Yohana 4:8"), ["1 John 4:8"]);
        assert_eq!(refs("2 Wakorintho 5:17"), ["2 Corinthians 5:17"]);
        assert_eq!(refs("1 Jòhánù 4:8"), ["1 John 4:8"]);
    }

    /// Code-switching is the NORMAL case, not an edge case (CLAUDE.md): a Yorùbá
    /// sermon routinely says the book in Yorùbá and the numbers in English.
    #[test]
    fn code_switching_mid_sentence_still_detects() {
        assert_eq!(refs("E jọ̀wọ́, ẹ ṣí Jòhánù 3:16"), ["John 3:16"]);
        // In-language numerals now work: "chapter three" in Swahili.
        assert_eq!(refs("Tugeukie Yohana sura ya tatu"), ["John 3:1"]);
        assert_eq!(
            refs("Let us turn to Yohana chapter 3 verse 16"),
            ["John 3:16"]
        );
    }

    /// English must not regress. The whole table is shared.
    #[test]
    fn english_still_works() {
        assert_eq!(refs("John 3:16"), ["John 3:16"]);
        assert_eq!(
            refs("turn to psalm twenty three verse one"),
            ["Psalms 23:1"]
        );
    }
}

#[cfg(test)]
mod alias_table_integrity {
    use super::*;
    use std::collections::HashMap;

    fn table() -> serde_json::Value {
        serde_json::from_str(include_str!("../data/book_aliases.json")).unwrap()
    }

    /// All three tier-1 languages must cover all 66 books. If a book is missing,
    /// Relay simply cannot hear it in that language.
    #[test]
    fn every_tier1_language_covers_all_66_books() {
        let t = table();
        for lang in ["yo", "sw", "ha"] {
            let books = t[lang].as_object().unwrap();
            let named: Vec<&str> = books
                .keys()
                .filter(|k| !k.starts_with('_'))
                .map(|s| s.as_str())
                .collect();
            let missing: Vec<&&str> = CANONICAL_BOOKS
                .iter()
                .filter(|b| !named.contains(*b))
                .collect();
            assert!(
                missing.is_empty(),
                "{lang}: {} books missing: {missing:?}",
                missing.len()
            );
            assert_eq!(named.len(), 66, "{lang} has {} books", named.len());
        }
    }

    /// THE safety test. If two different books share an alias, one of them wins
    /// arbitrarily and the other silently puts the WRONG SCRIPTURE on a wall.
    ///
    /// Cross-language collisions are the real hazard: Hausa "Mika" is Micah, and
    /// so is Swahili "Mika" — harmless, same book. But if Hausa "Luka" (Luke)
    /// collided with some other language's Luke-that-isn't, nobody would notice
    /// until a service.
    #[test]
    fn no_alias_maps_to_two_different_books() {
        let t = table();
        let mut seen: HashMap<String, (String, String)> = HashMap::new(); // alias -> (book, lang)
        for lang in ["yo", "sw", "ha"] {
            for (book, names) in t[lang].as_object().unwrap() {
                if book.starts_with('_') {
                    continue;
                }
                for n in names.as_array().unwrap() {
                    let key = normalize(n.as_str().unwrap());
                    if let Some((other_book, other_lang)) = seen.get(&key) {
                        assert_eq!(
                            other_book, book,
                            "alias {key:?} maps to BOTH {other_book} ({other_lang}) and \
                             {book} ({lang}) — one of them would put the wrong verse on a wall"
                        );
                    }
                    seen.insert(key, (book.clone(), lang.to_string()));
                }
            }
        }
    }

    /// An alias must not collide with an ENGLISH book that isn't the same book —
    /// the English table is merged into the same map.
    #[test]
    fn no_alias_hijacks_an_english_book() {
        let t = table();
        for lang in ["yo", "sw", "ha"] {
            for (book, names) in t[lang].as_object().unwrap() {
                if book.starts_with('_') {
                    continue;
                }
                for n in names.as_array().unwrap() {
                    let key = normalize(n.as_str().unwrap());
                    // If this alias is ALSO an English book name, it must be the
                    // same book. ("Amos" = "Amos" is fine. "Mark" = Luke is not.)
                    if let Some(english) = CANONICAL_BOOKS.iter().find(|b| normalize(b) == key) {
                        assert_eq!(
                            *english, book,
                            "{lang}: {key:?} is the English book {english} but is listed under {book}"
                        );
                    }
                }
            }
        }
    }

    /// Words that are also ORDINARY words must not be aliases. Yorùbá "iṣẹ́" means
    /// "work" and "orin" means "song" — in a church. An alias like that fires
    /// scripture off normal speech.
    #[test]
    fn no_alias_is_a_bare_everyday_word() {
        let t = table();
        // Known traps, deliberately excluded from the table.
        let banned = ["ise", "orin", "aiye", "oro"];
        for lang in ["yo", "sw", "ha"] {
            for (book, names) in t[lang].as_object().unwrap() {
                if book.starts_with('_') {
                    continue;
                }
                for n in names.as_array().unwrap() {
                    let key = normalize(n.as_str().unwrap());
                    assert!(
                        !banned.contains(&key.as_str()),
                        "{lang}: {key:?} (under {book}) is an everyday word — it would fire \
                         scripture off ordinary speech. Use the full book name instead."
                    );
                }
            }
        }
    }

    /// Spot-check the books a church actually reads, in every language.
    #[test]
    fn the_books_churches_actually_read_resolve() {
        let cases: &[(&str, &str)] = &[
            // Yorùbá — both translations in common use.
            ("Sáàmù 23:1", "Psalms"),
            ("Psalmu 23:1", "Psalms"),
            ("Orin Dafidi 23:1", "Psalms"),
            ("Jẹ́nẹ́sísì 1:1", "Genesis"),
            ("Genesisi 1:1", "Genesis"),
            ("Òwe 3:5", "Proverbs"),
            ("Aísáyà 40:31", "Isaiah"),
            ("Ìṣe àwọn Àpọ́sítélì 2:38", "Acts"),
            // Hausa
            ("Farawa 1:1", "Genesis"),
            ("Zabura 23:1", "Psalms"),
            ("Karin Magana 3:5", "Proverbs"),
            ("Ishaya 40:31", "Isaiah"),
            ("Ibraniyawa 11:1", "Hebrews"),
            ("Wahayin Yahaya 22:1", "Revelation"),
            ("Ru'ya ta Yohanna 22:1", "Revelation"),
            // Swahili
            ("Zaburi 23:1", "Psalms"),
            ("Mithali 3:5", "Proverbs"),
            ("Isaya 40:31", "Isaiah"),
            ("Waebrania 11:1", "Hebrews"),
        ];
        for (text, want) in cases {
            let got = detect_direct(text);
            assert_eq!(
                got.first().map(|m| m.reference.book.as_str()),
                Some(*want),
                "{text:?} should resolve to {want}, got {:?}",
                got.first().map(|m| &m.reference.book)
            );
        }
    }
}

#[cfg(test)]
mod numeral_table_integrity {
    use super::*;

    /// A wrong numeral does not fail safely — it silently shows a DIFFERENT VERSE.
    /// If "tisa" were mapped to 8 instead of 9, nobody would find out until a
    /// service. These are the cheap structural checks that catch a fat-finger.
    #[test]
    fn numeral_values_are_sane() {
        let n = numerals();
        assert!(!n.ones.is_empty() && !n.tens.is_empty());
        for (w, v) in &n.ones {
            assert!((1..=9).contains(v), "ones {w:?} = {v}, must be 1-9");
        }
        for (w, v) in &n.tens {
            assert!(
                (10..=90).contains(v) && v % 10 == 0,
                "tens {w:?} = {v}, must be a multiple of 10 in 10..=90"
            );
        }
    }

    /// A word cannot be both a number and the glue between numbers, or it would
    /// be consumed twice and change the value.
    #[test]
    fn no_word_is_both_a_number_and_a_connector() {
        let n = numerals();
        for c in &n.connectors {
            assert!(
                !n.ones.contains_key(c),
                "{c:?} is both a connector and a one"
            );
            assert!(
                !n.tens.contains_key(c),
                "{c:?} is both a connector and a ten"
            );
            assert!(!n.hundred_post.contains(c));
        }
        for l in &n.linkers {
            assert!(!n.ones.contains_key(l), "{l:?} is both a linker and a one");
            assert!(!n.tens.contains_key(l), "{l:?} is both a linker and a ten");
        }
    }

    /// The whole point of `hundred_post`. English puts the multiplier BEFORE the
    /// hundred word; Swahili and Hausa put it AFTER. Read the wrong way, "mia
    /// mbili" (200) becomes 102 — and Psalm 102 goes on the wall instead of 200.
    #[test]
    fn the_hundred_multiplier_comes_after_not_before() {
        let n = |t: &str| {
            let norm = normalize(t);
            let toks: Vec<&str> = norm.split_whitespace().collect();
            parse_number(&toks, 0).map(|(v, _, _)| v)
        };
        // Swahili
        assert_eq!(n("mia moja"), Some(100), "mia moja is 100, not 101");
        assert_eq!(n("mia mbili"), Some(200), "mia mbili is 200, NOT 102");
        assert_eq!(n("mia tano"), Some(500));
        assert_eq!(n("mia moja na kumi na tatu"), Some(113));
        // Hausa
        assert_eq!(n("dari"), Some(100));
        assert_eq!(n("dari biyu"), Some(200), "dari biyu is 200, NOT 102");
        assert_eq!(n("ɗari biyu"), Some(200), "hooked ɗ must fold");
        assert_eq!(n("dari da goma sha uku"), Some(113));
        // English is unchanged — multiplier BEFORE.
        assert_eq!(n("two hundred"), Some(200));
        assert_eq!(n("one hundred and thirteen"), Some(113));
    }

    #[test]
    fn tens_and_units_join_correctly() {
        let n = |t: &str| {
            let norm = normalize(t);
            let toks: Vec<&str> = norm.split_whitespace().collect();
            parse_number(&toks, 0).map(|(v, _, _)| v)
        };
        assert_eq!(n("kumi na tatu"), Some(13)); // sw teens
        assert_eq!(n("ishirini na tatu"), Some(23)); // sw tens
        assert_eq!(n("themanini na mbili"), Some(82));
        assert_eq!(n("goma sha uku"), Some(13)); // ha teens
        assert_eq!(n("ashirin da uku"), Some(23)); // ha tens
        assert_eq!(n("tis'in da tara"), Some(99));
    }

    /// The connectors and linkers ("na", "ya", "da", "ta") are among the most
    /// common words in these languages. A bare one must never start a number, or
    /// ordinary speech would manufacture verse references.
    #[test]
    fn a_bare_connector_is_not_a_number() {
        let n = |t: &str| {
            let norm = normalize(t);
            let toks: Vec<&str> = norm.split_whitespace().collect();
            parse_number(&toks, 0).map(|(v, _, _)| v)
        };
        for w in ["na", "da", "ya", "wa", "ta", "sha", "and"] {
            assert_eq!(n(w), None, "{w:?} alone must not parse as a number");
        }
    }
}

#[cfg(test)]
mod story_search {
    use super::*;

    /// Does story-first ranking actually find the STORY a preacher is
    /// describing?
    ///
    /// ```text
    /// cargo test story_search -- --ignored --nocapture
    /// ```
    ///
    /// Ignored because it builds the full-corpus index. It exists because the
    /// shipped eval corpus is already at 100% recall and 0 wrong verses — it can
    /// catch a regression but it cannot show an improvement, and `STORY_WEIGHT`
    /// should be a measured value rather than a taste.
    ///
    /// Each case is a paraphrase of a NARRATIVE, phrased the way a preacher
    /// recalls it out loud — not a quotation. That is the case a verse-only
    /// index is worst at: no single verse carries the words, the story does.
    #[test]
    #[ignore]
    fn story_weight_measured_against_verse_only() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        let corpus: Vec<(VerseRef, String)> = crate::db::all_verses(&conn)
            .unwrap()
            .into_iter()
            .map(|v| {
                (
                    VerseRef {
                        book: v.book,
                        chapter: v.chapter,
                        verse: v.verse,
                    },
                    v.text,
                )
            })
            .collect();
        assert!(
            corpus.len() > 30_000,
            "expected the full corpus, got {}",
            corpus.len()
        );
        let idx = SemanticIndex::build(&corpus);

        // (spoken paraphrase, book, chapter the story lives in)
        let cases: &[(&str, &str, i64)] = &[
            (
                "jesus fed the crowd with a boy's five loaves and two fishes",
                "John",
                6,
            ),
            (
                "david killed the giant with a stone and a sling",
                "1 Samuel",
                17,
            ),
            (
                "the son came home and his father ran out to meet him",
                "Luke",
                15,
            ),
            ("he rebuked the wind and the sea became calm", "Mark", 4),
            (
                "a woman touched the hem of his garment and was made whole",
                "Mark",
                5,
            ),
            (
                "the walls of the city fell down flat when they shouted",
                "Joshua",
                6,
            ),
            (
                "he was thrown into the den of lions and the mouths were shut",
                "Daniel",
                6,
            ),
        ];

        let hit = |rows: &[(VerseRef, f32)], book: &str, ch: i64| -> Option<usize> {
            rows.iter()
                .position(|(r, _)| r.book == book && r.chapter == ch)
        };

        // SWEEP the weight rather than trusting the shipped guess. The value
        // that ranks the most stories first, without pushing any off the page,
        // is the one worth shipping.
        println!("\n  weight   mean rank (missing = 6)   found-in-top-5");
        for w in [0.0f32, 0.15, 0.25, 0.35, 0.5, 0.65, 0.8] {
            let mut total = 0usize;
            let mut found = 0usize;
            for (q, book, ch) in cases {
                let rows = idx.top_k_story_weighted(q, 5, w);
                match hit(&rows, book, *ch) {
                    Some(i) => {
                        total += i + 1;
                        found += 1;
                    }
                    None => total += 6,
                }
            }
            println!(
                "  {w:<8.2} {:<24.2} {found}/{}",
                total as f32 / cases.len() as f32,
                cases.len()
            );
        }

        let mut better = 0;
        let mut worse = 0;
        println!("\n  story weight {STORY_WEIGHT} vs verse-only\n");
        for (q, book, ch) in cases {
            let story = idx.top_k_story_weighted(q, 5, STORY_WEIGHT);
            let verse = idx.top_k_story_weighted(q, 5, 0.0);
            let (a, b) = (hit(&story, book, *ch), hit(&verse, book, *ch));
            let f = |r: Option<usize>| match r {
                Some(i) => format!("#{}", i + 1),
                None => "MISSING".into(),
            };
            println!("  {q}");
            println!(
                "      target {book} {ch}    story: {:<8} verse-only: {}",
                f(a),
                f(b)
            );
            match (a, b) {
                (Some(x), Some(y)) if x < y => better += 1,
                (Some(x), Some(y)) if x > y => worse += 1,
                (Some(_), None) => better += 1,
                (None, Some(_)) => worse += 1,
                _ => {}
            }
        }
        println!(
            "\n  story-first better on {better}, worse on {worse}, of {}\n",
            cases.len()
        );
    }
}

#[cfg(test)]
mod query_repair {
    use super::*;

    fn idx() -> SemanticIndex {
        SemanticIndex::build(&[
            (
                VerseRef {
                    book: "Proverbs".into(),
                    chapter: 25,
                    verse: 11,
                },
                "A word fitly spoken is like apples of gold in pictures of silver".into(),
            ),
            (
                VerseRef {
                    book: "Exodus".into(),
                    chapter: 32,
                    verse: 4,
                },
                "and made it a molten calf of golden fashion".into(),
            ),
            (
                VerseRef {
                    book: "Psalms".into(),
                    chapter: 23,
                    verse: 1,
                },
                "The LORD is my shepherd I shall not want".into(),
            ),
        ])
    }

    #[test]
    fn repairs_a_misheard_content_word() {
        // "goden" is in no verse; today it contributes nothing at all and the
        // query is scored on whatever survives.
        let i = idx();
        let got = i.repair_query(&["goden".to_string()]);
        assert_eq!(got, vec!["golden".to_string()]);
    }

    #[test]
    fn a_misheard_word_now_finds_its_verse() {
        let i = idx();
        // "goden" is the misheard word and is still what this test is about; the
        // sentence around it is there because three shared words are required
        // with no exception since 2026-09-20.
        let hits = i.top_k("and they made it a goden calf", 2);
        assert_eq!(hits[0].0.book, "Exodus");
    }

    #[test]
    fn known_words_are_never_touched() {
        // The corpus knows "gold"; it must not become "golden" or anything else.
        let i = idx();
        assert_eq!(
            i.repair_query(&["gold".to_string()]),
            vec!["gold".to_string()]
        );
        assert_eq!(
            i.repair_query(&["shepherd".to_string()]),
            vec!["shepherd".to_string()]
        );
    }

    #[test]
    fn short_words_are_left_alone() {
        // At three characters the nearest neighbour is meaningless.
        let i = idx();
        assert_eq!(
            i.repair_query(&["xyz".to_string()]),
            vec!["xyz".to_string()]
        );
    }

    #[test]
    fn an_ambiguous_repair_is_refused() {
        // "word" and "gold" are both one edit from "wold"... construct a real
        // tie: "cald" is one edit from "calf" and from "cold"? Use the corpus we
        // have — "silver"/"sliver" style ties must not be guessed.
        let i = SemanticIndex::build(&[(
            VerseRef {
                book: "X".into(),
                chapter: 1,
                verse: 1,
            },
            "bald bold".into(),
        )]);
        // "bild" is one edit from neither; "bxld" is one from both bald and bold.
        assert_eq!(
            i.repair_query(&["bxld".to_string()]),
            vec!["bxld".to_string()]
        );
    }

    #[test]
    fn a_word_with_no_near_neighbour_is_left_unknown() {
        let i = idx();
        assert_eq!(
            i.repair_query(&["helicopter".to_string()]),
            vec!["helicopter".to_string()]
        );
    }
}

#[cfg(test)]
mod evidence_floor {
    use super::*;

    // Reported from the console: the paraphrase panel was offering a verse whose
    // whole justification was a single word under "MATCHED ON (FROM TRANSCRIPT)".
    //
    // One word is not evidence a human can weigh in the second they have to weigh
    // it. Neither the shipped eval corpus nor the story benchmark discriminates
    // this — the first is almost all DIRECT references, the second compares
    // story-vs-verse ranking at a fixed floor — so the floor is justified here,
    // against the behaviour it actually changes.

    fn idx() -> SemanticIndex {
        SemanticIndex::build(&[
            (
                VerseRef { book: "Isaiah".into(), chapter: 40, verse: 31 },
                "they that wait upon the LORD shall renew their strength they shall mount up with wings as eagles".into(),
            ),
            (
                VerseRef { book: "Leviticus".into(), chapter: 11, verse: 13 },
                "the eagle and the ossifrage and the ospray".into(),
            ),
            (
                VerseRef { book: "Psalms".into(), chapter: 23, verse: 1 },
                "The LORD is my shepherd I shall not want".into(),
            ),
        ])
    }

    /// A single COMMON word is still a coincidence with a good score, and is
    /// still refused. This is the half of the old one-word rule that survives
    /// DECISIONS.md §33.
    ///
    /// A LITERAL 2, not `MIN_EVIDENCE_TERMS`. Asserting against the constant
    /// under test is tautological — it passes at any value, including the old
    /// behaviour this exists to forbid.
    #[test]
    fn a_common_single_shared_word_is_not_offered_as_a_paraphrase() {
        // "lord" is in two of these three verses, so it names nothing in
        // particular — exactly the thin match to refuse.
        let hits = idx().top_k_explained("lord", 5);
        assert!(
            hits.iter().all(|(_, _, terms)| terms.len() >= 2),
            "a common one-word paraphrase survived: {:?}",
            hits.iter()
                .map(|(r, _, t)| (r.book.clone(), t.clone()))
                .collect::<Vec<_>>()
        );
    }

    /// THE OTHER HALF OF §33 IS REVERSED, AND THIS IS THE TEST THAT SAYS SO.
    ///
    /// A rare word used to be corroboration on its own, because there was
    /// nowhere else in the corpus it could have come from. It was measured out
    /// on 2026-09-20: see `MIN_EVIDENCE_TERMS`. "Ossifrage" is still in exactly
    /// one verse and that is still true; what changed is that being the only
    /// place a word appears is a fact about the BIBLE, not evidence that the
    /// preacher was talking about that verse. A one-word offer is not something
    /// an operator can agree or disagree with in the second they have.
    ///
    /// Written as the inverse of the test it replaces, so the two cannot both
    /// be in the file.
    #[test]
    fn a_rare_single_shared_word_is_no_longer_evidence_enough() {
        assert!(
            idx().top_k_explained("ossifrage", 5).is_empty(),
            "a one-word match was offered"
        );
        // The SAME rare word in a sentence that corroborates it three ways IS
        // offered, so what was removed is the exception and not the word.
        let hits = idx().top_k_explained("the ossifrage and the eagle and the osprey", 5);
        assert_eq!(
            hits.first().map(|h| h.0.book.clone()),
            Some("Leviticus".into())
        );
        assert!(
            hits[0].2.len() >= 3,
            "offered on thin evidence: {:?}",
            hits[0].2
        );
    }

    #[test]
    fn a_corroborated_paraphrase_still_matches_and_shows_its_words() {
        // Several independent shared words: defensible on its face, and the
        // operator gets more than one word to judge.
        let hits =
            idx().top_k_explained("they shall renew their strength and mount up with wings", 3);
        assert!(!hits.is_empty(), "a well-evidenced paraphrase was dropped");
        assert_eq!(hits[0].0.book, "Isaiah");
        assert!(
            hits[0].2.len() >= 3,
            "expected several matched words, got {:?}",
            hits[0].2
        );
    }

    #[test]
    fn up_to_six_matched_words_are_surfaced() {
        // The console showed at most four. More evidence costs nothing when it
        // exists, and the UI truncates what will not fit.
        let hits = idx().top_k_explained("wait upon the lord renew strength mount wings eagles", 3);
        assert!(hits[0].2.len() > 4, "still capped low: {:?}", hits[0].2);
        assert!(hits[0].2.len() <= EXPLAIN_TERMS);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// R4 · Detection & Language — audit evidence (QA run 2026-08-14)
// ════════════════════════════════════════════════════════════════════════════
//
// Every test below asserts the behaviour Relay SHOULD have, and every one of them
// is `#[ignore]`d because it fails today. They are evidence, not fixes: CI stays
// green, and one command reproduces the whole finding set.
//
//     cargo test detection::r4_audit -- --ignored --nocapture
//
// A test that passes when it is fixed is worth more than a test that passes now
// and blocks the fix, so none of these assert the defect.
#[cfg(test)]
mod r4_audit {
    use super::*;
    use crate::router::{RouteDecision, Router, Thresholds};

    /// (reference, confidence) pairs, as the router classified them.
    type Routed = Vec<(String, f32)>;

    /// Score a line the way the product does: the real parser, then the real
    /// router. Never by reading the text. Returns (auto-fired, suggested).
    fn wall(text: &str, dial: u8) -> (Routed, Routed) {
        let mut r = Router::default();
        r.set_thresholds(Thresholds::from_sensitivity(dial));
        let (mut fired, mut sugg) = (Vec::new(), Vec::new());
        for m in detect_direct(text) {
            let key = format!(
                "{} {}:{}",
                m.reference.book, m.reference.chapter, m.reference.verse
            );
            match r.decide(&key, m.confidence, m.method, 0) {
                RouteDecision::AutoFire => fired.push((key, m.confidence)),
                RouteDecision::Suggest => sugg.push((key, m.confidence)),
                RouteDecision::Drop => {}
            }
        }
        (fired, sugg)
    }

    fn fired(text: &str, dial: u8) -> Vec<String> {
        wall(text, dial).0.into_iter().map(|(k, _)| k).collect()
    }

    // ═══ FIELD · 2026-08-30 · A REAL SERVICE ════════════════════════════════
    //
    // Six references, transcribed by `ggml-large-v3-turbo` from a live sermon and
    // taken VERBATIM from `detections.heard_text` — the column that exists because
    // a service once put wrong verses on a wall and the log could not say what it
    // had heard. Full write-up: `docs/qa/audits/FIELD.md`.
    //
    // These are not invented sentences. Every earlier case in this file was written
    // by someone imagining how a preacher talks; these are how one actually did,
    // through a real microphone, in a real room, with real ASR errors in them.
    //
    // **Four of the five correct fires are the bare "Book 15, 7" shape** — no
    // chapter or verse keyword, deliberately scored 0.55, deliberately still
    // firing at the default dial over the standing objection that nobody speaks
    // that way. This preacher speaks that way, four times in thirty minutes. That
    // demotion must not be tightened without this evidence in front of you.

    /// The five the product got RIGHT, and they must stay right.
    ///
    /// A regression here is not a failed assertion, it is a church losing the
    /// thing it installed Relay for.
    #[test]
    fn field_the_references_a_real_preacher_spoke_still_reach_the_wall() {
        for (heard, want) in [
            (
                "David did the same thing in 2 Samuel 24, 24. When will this scripture be fulfilled in Yahweh?",
                "2 Samuel 24:24",
            ),
            (
                "Here we sing now. If your heart is enlarged, your hand will open. Look at Deuteronomy 15, 7 to 8.",
                "Deuteronomy 15:7",
            ),
            (
                "in the days of old and it got to a point the bible says exodus 35 verse 21 the bible",
                "Exodus 35:21",
            ),
            (
                "The church creates more than enough. Look at what the Bible says in 2 Corinthians 9, 8. Look at the word.",
                "2 Corinthians 9:8",
            ),
            (
                "Abundance is coming. But you have to enlarge your 10. That's why Isaiah 54, 2 to 3 was screaming at us.",
                "Isaiah 54:2",
            ),
        ] {
            let got = fired(heard, 50);
            assert!(
                got.iter().any(|k| k == want),
                "{want} was spoken and did not reach the wall — fired {got:?}"
            );
        }
    }

    /// FIELD F-1 · the mechanism, isolated.
    ///
    /// The wrong verse did NOT come from the parser mis-reading a sentence — the
    /// first diagnosis, and it was wrong. `detect_direct` never produces Proverbs
    /// from this text at all. It came from the bare-verse path: "verse 32" hung on
    /// `ContextMemory`, which still held **Proverbs 3:6** from a manual fire five
    /// minutes earlier, while **Luke 10** sat in the same sentence.
    ///
    /// This test states the repair as a fact about the text alone, which is why it
    /// can be a unit test at all: whatever memory holds, this window names Luke 10,
    /// and a bare verse in it belongs to Luke 10.
    #[test]
    fn field_a_bare_verse_belongs_to_the_book_this_sentence_names() {
        let heard = "The man that was wounded. That man was going through what was \
                     going through in Luke 10. If you read from verse 32, 37.";
        let anchor = anchor_for_bare_verses(heard).expect("this sentence names a reference");
        assert_eq!(
            anchor.book, "Luke",
            "the book named in this breath, not one from memory"
        );
        assert_eq!(anchor.chapter, 10);
        assert!(
            detect_bare_verses(heard).contains(&32),
            "precondition: 32 is seen as a bare verse"
        );
    }

    /// …and the case the bare-verse path was WRITTEN for still works.
    ///
    /// A preacher reading through a passage says "and verse eighteen" and names no
    /// book. Nothing in the window to anchor to, so memory is exactly right, and
    /// the repair above must not have taken it away.
    #[test]
    fn field_a_bare_verse_with_nothing_to_anchor_to_still_falls_back_to_memory() {
        assert!(
            anchor_for_bare_verses("and if you look at verse eighteen").is_none(),
            "no reference is named here — memory is all Relay has, and should be used"
        );
        assert!(detect_bare_verses("and if you look at verse eighteen").contains(&18));
    }

    /// FIELD F-1 · the one that was WRONG, in front of a congregation.
    ///
    /// The preacher cited **Luke 10:32-37** — the Good Samaritan, which is also
    /// what the sentence around it is about. Relay put **Proverbs 3:32** on the
    /// screen at 0.88 against a 0.50 bar, unattended.
    ///
    /// Two things make this worth a test of its own rather than a line in an
    /// audit. The correct reference was IN THE SAME WINDOW and lost the ranking
    /// (`rank_for_wall`, rule 29) to the wrong one. And the failure is not a
    /// threshold: 0.88 is above every bar the dial can set short of its most
    /// cautious end, and that end would also have held back four of the five
    /// correct fires above.
    ///
    /// The assertion is deliberately about what must NOT happen, not about what
    /// must: getting `Luke 10` onto the wall from this sentence would be a fine
    /// outcome and is not required. Putting a verse nobody said onto it is the
    /// defect.
    #[test]
    fn field_a_verse_nobody_said_does_not_reach_the_wall() {
        let heard = "The man that was wounded. That man was going through what was \
                     going through in Luke 10. If you read from verse 32, 37.";
        let got = fired(heard, 50);
        assert!(
            !got.iter().any(|k| k == "Proverbs 3:32"),
            "FIELD F-1 reproduced: Proverbs 3:32 auto-fired from a sentence citing \
             Luke 10:32-37 — fired {got:?}"
        );
    }

    /// FIELD F-8 · 2026-09-06 · the second wrong verse on a real wall, and it is
    /// F-1's own repair discovering the case it did not cover.
    ///
    /// The preacher said *"is taken from **4th Peter chapter 5 verse 10**"* — 1
    /// Peter 5:10, misheard as a book that does not exist. So `detect_direct`
    /// found nothing, `anchor_for_bare_verses` was `None`, and the bare `10` fell
    /// through to `ContextMemory`, which held **Psalms 92** from ten minutes
    /// earlier. **Psalms 92:10 auto-fired at 0.88.**
    ///
    /// F-1 taught that a book named in this breath beats memory. F-8 is the same
    /// sentence with the book name BROKEN: the window still states a chapter, and
    /// memory still stated a different one, and memory still won. "The words do
    /// not say" is not the same as "the words did not parse".
    ///
    /// Asserting `None` and not a corrected reference is the point. The spoken
    /// chapter is known (5); the BOOK is not. Pairing a heard chapter with a
    /// remembered book would put Psalms 5:10 up instead — a different wrong verse,
    /// and rule 10's lesson exactly: a real confidence about a word nobody said.
    #[test]
    fn field_f8_a_chapter_this_window_states_is_not_one_memory_may_answer_for() {
        let heard = "God that I read by the way of declaration, joining my faith with \
                     the faith of our Father and the Lord, is taken from 4th Peter \
                     chapter 5 verse 10.";
        assert!(
            anchor_for_bare_verses(heard).is_none(),
            "precondition: '4th Peter' is not a book, so nothing parses out of this"
        );
        assert!(
            detect_bare_verses(heard).contains(&10),
            "precondition: 10 is seen as a bare verse"
        );
        assert!(
            chapter_named(heard),
            "precondition: this window states a chapter out loud"
        );
        let memory = VerseRef {
            book: "Psalms".into(),
            chapter: 92,
            verse: 10,
        };
        assert_eq!(
            resolve_bare_verse_with_source(heard, 10, None, Some(&memory)),
            None,
            "FIELD F-8 reproduced: a verse was resolved against a chapter the \
             preacher did not say, while naming a different one out loud"
        );
    }

    /// …and the case the bare-verse path exists for is untouched.
    ///
    /// "and verse eighteen" states no chapter, so memory is all Relay has and is
    /// exactly right. If the F-8 repair took this away it would have broken the
    /// only phrasing the mechanism was written for.
    #[test]
    fn field_f8_memory_still_answers_when_the_window_states_no_chapter() {
        let heard = "and if you look at verse eighteen";
        assert!(!chapter_named(heard), "no chapter is stated here");
        let memory = VerseRef {
            book: "Romans".into(),
            chapter: 8,
            verse: 18,
        };
        assert_eq!(
            resolve_bare_verse_with_source(heard, 18, None, Some(&memory)),
            Some((memory.clone(), BareVerseSource::Memory)),
            "mid-passage 'verse eighteen' must still resolve against the passage on screen"
        );
    }

    /// …and a window that DOES name a book still beats memory, chapter word or
    /// not. This is F-1's guarantee, re-asserted through the one function that now
    /// owns the decision, so a future edit cannot keep F-8 and lose F-1.
    #[test]
    fn field_f8_a_book_named_in_this_breath_still_outranks_memory() {
        let heard = "is taken from 1 Peter chapter 5 verse 10";
        let anchor = anchor_for_bare_verses(heard).expect("1 Peter 5 parses");
        let memory = VerseRef {
            book: "Psalms".into(),
            chapter: 92,
            verse: 10,
        };
        let got = resolve_bare_verse_with_source(heard, 10, Some(&anchor), Some(&memory))
            .map(|(r, _)| r)
            .expect("the window names a book, so it resolves");
        assert_eq!(got.reference_book_chapter_verse(), "1 Peter 5:10");
    }

    /// `chapter_named` speaks the priority languages, like every other detection
    /// helper. An English-only check would have applied the F-8 repair to English
    /// preaching and silently skipped Swahili and Hausa — the exact asymmetry
    /// `detect_bare_verses` and `detect_passage_nav` were both fixed for.
    #[test]
    fn field_f8_chapter_named_is_not_english_only() {
        assert!(chapter_named("chapter 5 verse 10"));
        assert!(chapter_named("chap 5 verse 10"));
        assert!(chapter_named("sura ya tano mstari wa kumi"), "Swahili");
        assert!(
            !chapter_named("verse ten"),
            "a verse word alone is not a stated chapter"
        );
        assert!(
            !chapter_named("the fifth chapter of what he read"),
            "'chapter' with no number after it states nothing"
        );
    }

    // ── R4-01 · ordinary sermon speech auto-fires a wrong verse ────────────
    //
    // Two repairs meet, and both of them raise the score of the LESS plausible
    // reading of the same sentence.
    //
    // `split_run_into_chapter_verse` exists because whisper writes "six sixty
    // three" as `663`. It refuses to touch a run that IS a valid chapter of the
    // book — "a fact about the book, not a guess about the speaker". True. But it
    // has no analogue of the guard `fuzzy_book` carries: nothing requires the
    // sentence to be reference-SHAPED. And it hands the repaired reading 0.83,
    // where the keyword-less whole-chapter reading it replaced is deliberately
    // demoted to 0.45.
    //
    // So a number the parser could NOT read as a chapter makes Relay more
    // confident, not less:
    //
    //     "Nehemiah, thirteen days ..."   → Nehemiah 13:1 @0.45  (asks a human)
    //     "Nehemiah, fifty two days ..."  → Nehemiah  5:2 @0.77  (goes on the wall)
    //
    // `fuzzy_book` then compounds it. `NEVER_FUZZY` protects ordinary English
    // function words; it holds no PROPER NAMES, and a sermon is made of proper
    // names. "Mary" is one edit from "Mark", and `phonetic` is a bool, so a
    // reference that guessed the BOOK and split a run it could not read is
    // charged 0.06 once rather than twice.
    //
    // All three sentences below are ordinary preaching. All three put a verse in
    // front of a congregation at the shipped default dial, with no human asked.
    #[test]
    fn r4_01_ordinary_sermon_speech_does_not_auto_fire_a_verse() {
        // The control: the same shape, with a number that IS a real chapter.
        assert!(
            fired("Nehemiah, thirteen days they built the wall", 50).is_empty(),
            "control: a keyword-less whole chapter is correctly demoted"
        );
        for line in [
            "Nehemiah, fifty two days they built the wall", // → Nehemiah 5:2 @0.77
            "Mary, twenty two years of age, stood there",   // → Mark 2:2 @0.77
            "Mary, chapter two of her life had begun",      // → Mark 2:1 @0.82
        ] {
            assert!(
                fired(line, 50).is_empty(),
                "{line:?} auto-fired {:?} at the shipped default dial — a repair                  scored the less plausible reading of the sentence HIGHER than                  the reading it replaced",
                fired(line, 50)
            );
        }
    }

    // ── R4-02 · the garble guard covers digits and not spoken words ─────────
    //
    // `parse_reference` demotes a bare pair to 0.45 when a LEFTOVER number follows
    // it, because "Psalms 2, 3, 1" is the shape of garbled speech. That guard is
    // gated on `bare_digits`, so it only ever sees `23`-style tokens.
    //
    // Whisper writes spoken numbers as WORDS on a large share of decodes,
    // especially on accented speech — which is the market. The identical garble,
    // rendered in words, scores 0.90 and goes straight to the congregation.
    #[test]
    fn r4_02_a_trailing_loose_number_is_a_garble_signal_in_words_too() {
        // The digit rendering is correctly demoted...
        assert!(fired("Psalms 2 3 1", 50).is_empty());
        // ...and the word rendering of the SAME utterance is not.
        for line in [
            "Psalms two three one",
            "romans eight one two",
            "acts two thirty eight forty",
        ] {
            assert!(
                fired(line, 50).is_empty(),
                "{line:?} auto-fired {:?} — the leftover number says the numbers \
                 did not line up, in words exactly as in digits",
                fired(line, 50)
            );
        }
    }

    // ── R4-03 · at the top of the dial every demotion is inert ──────────────
    //
    // `make_match` clamps confidence to a 0.30 floor. `from_sensitivity(100)`
    // returns auto_fire = 0.30, and `decide` compares with `>=`. So at maximum
    // sensitivity NO direct match can ever be a suggestion: every deliberate
    // demotion in detection.rs — the bare whole chapter, the bare single-chapter
    // verse, the garble shape, the repaired book — auto-fires.
    //
    // These are the exact sentences the demotions were written for, taken from
    // the comments in `parse_reference`.
    #[test]
    fn r4_03_the_deliberate_demotions_survive_the_top_of_the_sensitivity_dial() {
        for line in [
            "Matthew one of the twelve disciples",
            "Jonah three days in the belly of the fish",
            "jude four men came in",
            "James two of the brothers were there",
            "Job one day said to his friends",
            "Verse 1, Psalms 2, 3, 1, Next verse, chapter 2,",
        ] {
            assert!(
                fired(line, 100).is_empty(),
                "{line:?} auto-fired {:?} at dial 100 — the safety margin for \
                 every demotion in this file is the 0.05 between 0.45 and the \
                 default 0.50 bar, and the operator's own dial closes it",
                fired(line, 100)
            );
        }
    }

    // ── R4-04 · CLOSED — the SPEC gate now covers the WHOLE dial ────────────
    //
    // `eval.rs::run` builds `Router::default()`, so the CI wrong-verse gate scored
    // the corpus at sensitivity 50 and nowhere else. **A church that moved the dial
    // was running a configuration nothing had ever measured** — and R4-03 was the
    // proof that mattered: at dial 100 the auto-fire bar is 0.30, which IS the
    // confidence floor, so every deliberate demotion in this file was inert and the
    // one gate that would have noticed was not looking there.
    //
    // This was `#[ignore]`d as "a measurement". It costs 20 milliseconds and it
    // asserts SPEC's <5% at SIX dial positions including both ends, so leaving it
    // out of CI was leaving the operator's own control ungated. It runs now.
    //
    // Measured 2026-08-31, on the corpus including the six real sermon lines:
    // **0.0% at every position, 0 through 100.**
    #[test]
    fn r4_04_measure_the_wrong_verse_rate_across_the_whole_dial() {
        #[derive(serde::Deserialize)]
        struct Case {
            id: String,
            text: String,
            expect: Vec<String>,
        }
        #[derive(serde::Deserialize)]
        struct Corpus {
            cases: Vec<Case>,
        }
        const RAW: &str = include_str!("../data/eval_corpus.json");
        let c: Corpus = serde_json::from_str(RAW).expect("eval_corpus.json");
        for dial in [0u8, 25, 50, 63, 75, 100] {
            let mut wrong = Vec::new();
            for case in &c.cases {
                for k in fired(&case.text, dial) {
                    if !case.expect.contains(&k) {
                        wrong.push(format!("[{}] {k} — {:?}", case.id, case.text));
                    }
                }
            }
            let rate = wrong.len() as f64 * 100.0 / c.cases.len() as f64;
            let t = Thresholds::from_sensitivity(dial);
            println!(
                "  dial {dial:>3}  auto {:.2} / suggest {:.2}   wrong-verse rate {rate:.1}%",
                t.auto_fire, t.suggest
            );
            for w in &wrong {
                println!("      {w}");
            }
            // Holds on THIS corpus, which was authored against dial 50. It is
            // recorded, not asserted away: the mechanism in R4-03 is real and the
            // corpus simply contains few of the shapes it lets through.
            assert!(
                rate < 5.0,
                "dial {dial}: wrong-verse rate {rate:.1}% exceeds SPEC's 5%"
            );
        }
    }

    // ── R4-05 · Yorùbá numerals ─────────────────────────────────────────────
    //
    // **This test was `#[ignore]`d and RED for the life of the project**, recording
    // that `data/numerals.json` carried `sw` and `ha` and no `yo` key: a reference
    // spoken entirely in Yorùbá parsed to nothing, because the book alias matched
    // and then the chapter number was not a number the FSM knew. It was filed here
    // because `eval_corpus.json`'s twelve `yo` cases all use digits or English
    // number words, so the scorecard printed a 100% Yorùbá row beside Swahili and
    // Hausa rows that really had parsed their own numerals.
    //
    // The `yo` block now exists and this runs in CI. What has NOT changed is that
    // nobody has reviewed those words — so the block is marked `unreviewed` and
    // everything resolved through it is capped at `Suggest`
    // (`parsed_an_unreviewed_numeral`). `r4_05b` is the half that matters more.
    #[test]
    fn r4_05_a_reference_spoken_entirely_in_yoruba_is_detected() {
        for line in [
            "Jòhánù orí kẹta ẹsẹ̀ kẹrìndínlógún", // John 3:16
            "Johanu ori keta ese kerindinlogun", // the same, tone marks dropped
            "Sáàmù orí ogún",                    // Psalm 20
        ] {
            let (f, s) = wall(line, 50);
            assert!(
                !f.is_empty() || !s.is_empty(),
                "{line:?} produced nothing at all — the Yorùbá book name matched \
                 and the Yorùbá numeral did not"
            );
        }
    }

    /// R4-05b · AN UNREVIEWED NUMERAL MAY NEVER REACH A WALL BY ITSELF.
    ///
    /// `data/numerals.json`'s own header states the risk this closes: a wrong
    /// numeral does not fail safely, it silently shows a DIFFERENT VERSE. Nobody
    /// has checked the Yorùbá in that file, so the words were heard and what they
    /// are worth is Relay's guess — which is the doubt `UncertainNumber` already
    /// exists for (rule 10).
    ///
    /// Asserted **across the whole dial**, because a demotion expressed as a score
    /// is one the operator's sensitivity slider erases: `from_sensitivity(100)`
    /// returns the confidence floor. Expressed as a method, the router refuses it
    /// at every setting. Deleting `unreviewed` from the `yo` block is what lifts
    /// this, and that is a native speaker's signature, not a code change.
    #[test]
    fn r4_05b_an_unreviewed_yoruba_numeral_is_offered_never_fired() {
        for dial in [0u8, 25, 50, 63, 75, 100] {
            for line in [
                "Jòhánù orí kẹta ẹsẹ̀ kẹrìndínlógún",
                "Johanu ori keta ese kerindinlogun",
                "Sáàmù orí ogún",
            ] {
                let (fired, suggested) = wall(line, dial);
                assert!(
                    fired.is_empty(),
                    "dial {dial}: {line:?} auto-fired {fired:?} — an unreviewed \
                     numeral table put a verse on a wall unattended"
                );
                assert!(
                    !suggested.is_empty(),
                    "dial {dial}: {line:?} was capped into silence rather than \
                     into a suggestion — the operator gets nothing at all"
                );
            }
        }
        // The control: English through the same parser still fires, so the cap is
        // the Yorùbá table's and not a change to the gate.
        let (fired, _) = wall("John chapter three verse sixteen", 50);
        assert!(!fired.is_empty(), "the cap leaked onto English");
    }

    // ── R4-06 · three sibling entry points are hardcoded to English ─────────
    //
    // `parse_reference` asks `is_chapter_word` / `is_verse_word`, which consult
    // `data/numerals.json` and so speak Swahili and Hausa. Its three siblings —
    // `detect_bare_verses`, `detect_passage_nav`, `detect_ambiguous` — match the
    // English literals inline instead. Code-switching is the normal case for this
    // market (CLAUDE.md), so a preacher who says "mstari wa nne" mid-passage gets
    // nothing where "verse four" would have moved the wall.
    #[test]
    fn r4_06_in_language_verse_and_chapter_words_reach_the_nav_entry_points() {
        // Swahili "mstari"/"aya" = verse, "sura" = chapter. Hausa "aya", "sura".
        assert_eq!(detect_bare_verses("verse four"), vec![4], "control");
        for line in ["mstari wa nne", "aya ta huɗu", "aya ya nne"] {
            assert_eq!(
                detect_bare_verses(line),
                vec![4],
                "{line:?} — a bare in-language verse does not resolve against the \
                 passage on screen"
            );
        }
        assert!(detect_passage_nav("chapter five").is_some(), "control");
        for line in ["sura ya tano", "sura ta biyar"] {
            assert!(
                detect_passage_nav(line).is_some(),
                "{line:?} — an in-language in-passage jump is invisible"
            );
        }
    }

    // ── R4-07 · CLOSED — two references in one window ───────────────────────
    //
    // `emit_detections` deduped candidates into a `std::collections::HashMap` and
    // then ranked `best.into_iter()`. `detect_direct` returns matches left to
    // right — the order the preacher said them — and the HashMap replaced that
    // with SipHash order, seeded per map instance.
    //
    // `rank_for_wall`'s sort is STABLE, so this only mattered for ties — and a tie
    // is the ordinary case: "turn to John 3:16 and Romans 8:28" yields two
    // `Direct` candidates at the same score. A window may put at most ONE verse on
    // a wall (DECISIONS §37), so the tie decided **what the congregation saw**,
    // and two runs of the same sentence could differ.
    //
    // The dedup is a `Vec` now, so ties break on what was said first. Asserted
    // where it can actually be asserted — `main::rank_for_wall_tests`, against the
    // real ranking function — rather than by a local re-implementation of a
    // HashMap, which is what this test used to do and which could never have gone
    // green no matter what the product did.
    #[test]
    fn r4_07_both_references_in_one_window_still_parse_in_spoken_order() {
        let text = "turn with me to John three sixteen and also Romans eight twenty eight";
        let parsed: Vec<String> = detect_direct(text)
            .iter()
            .map(|m| {
                format!(
                    "{} {}:{}",
                    m.reference.book, m.reference.chapter, m.reference.verse
                )
            })
            .collect();
        assert_eq!(
            parsed,
            vec!["John 3:16".to_string(), "Romans 8:28".to_string()],
            "the parser's own order is the evidence everything downstream relies \
             on for a tie-break"
        );
    }

    // ── R4-08 · the auto-fire path cannot be driven by a test ───────────────
    //
    // CLAUDE.md rule #24: the fire engine is generic over `tauri::Runtime`, and
    // that is what makes `e2e.rs` possible. Every HUMAN fire path honoured it —
    // `fire_manual`, `handle_nav`, `manual_fire`, `clear_or_report`. The two
    // paths where the AI decides did not:
    //
    //     fn emit_detections(handle: &tauri::AppHandle, ...)          // main.rs
    //     fn confirm_detection(app: tauri::AppHandle, ...)            // main.rs
    //
    // A bare `tauri::AppHandle` is the concrete desktop runtime, so neither can be
    // called with `qa::bare_app()`. The result is that all thirteen `e2e.rs` tests
    // drive `manual_fire`, and the ONE path that puts scripture on a wall with no
    // human in the loop has no end-to-end coverage at all.
    #[test]
    fn r4_08_the_ai_fire_path_is_generic_over_the_runtime_like_every_other() {
        let src = include_str!("main.rs");
        for f in ["fn emit_detections", "fn confirm_detection"] {
            let at = src.find(f).expect("function still exists");
            let sig = &src[at..src[at..].find('(').map(|i| at + i).unwrap_or(at)];
            assert!(
                sig.contains('<'),
                "{f} is welded to the concrete runtime ({sig:?}) — qa::bare_app() \
                 cannot drive it, so the auto-fire path is the only fire path in \
                 the product with no e2e test"
            );
        }
    }

    // ── R4-09 · CLOSED — what `confirm_detection` teaches the gate ──────────
    //
    // It received only the reference STRING and re-parsed it with `detect_direct`,
    // feeding THAT parse's confidence to `record_feedback` as "the score the
    // operator agreed with". A canonical "Book C:V" always re-parses through the
    // colon-pair branch at the same number, for all 66 books — so the confirm arm
    // of the self-calibrating gate always learned one constant, and because
    // `record_feedback` only corrects when `c < auto_fire` (0.50 at the default
    // dial, 0.90 at the most cautious) the correction never fired at all. Every
    // confirm was pure decay toward baseline. `router.rs`'s own unit test passed
    // throughout, because it calls `record_feedback` directly.
    //
    // The console always knew the suggestion's own confidence and method and threw
    // both away at the call site — the same shape as `NavResult`'s `Ok(_)`.
    //
    // The property below is unchanged and was never the defect: a canonical
    // reference really does re-parse to a constant. That is exactly why it could
    // not be the thing fed to the calibrator. The FIX is asserted end to end in
    // `e2e::confirming_a_suggestion_teaches_the_gate_what_was_accepted`, because
    // this is a claim about a command, not about the parser.
    #[test]
    fn r4_09_a_canonical_reference_reparses_to_a_constant_which_is_why_it_is_not_evidence() {
        let mut scores = std::collections::HashSet::new();
        for b in ["John", "Romans", "Psalms", "Jude", "1 John", "Revelation"] {
            let sref = format!("{b} 1:1");
            let m = detect_direct(&sref)
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("{sref:?} did not re-parse"));
            scores.insert(format!("{:.2}", m.confidence));
        }
        assert_eq!(
            scores.len(),
            1,
            "a canonical reference is expected to re-parse to ONE number ({scores:?}) — \
             if this ever becomes several, the reasoning in the comment above needs \
             revisiting, not the fix"
        );
    }

    // ── R4-10 · the operator's gate control must move all three facts ───────
    //
    // `set_sensitivity` moves the gate, re-anchors the baseline, AND writes
    // `voice_profiles.sensitivity` alongside the thresholds — its doc comment
    // explains at length why writing one without the other leaves the profile
    // "describing a state that never existed", found live.
    //
    // ── WHAT CHANGED HERE, AND WHY THE GUARANTEE DID NOT ────────────────────
    //
    // This test used to drive `set_thresholds`, the Settings two-slider twin,
    // because that was the door that moved the gate and the in-memory baseline
    // and wrote NOTHING: the next confirm/dismiss called
    // `persist_active_thresholds`, which wrote auto_fire/suggest and left
    // `sensitivity` stale, producing exactly the forbidden row — and at the next
    // launch `apply_profile` re-anchored the baseline from that stale dial, so
    // calibration decayed back toward a number the operator had overruled.
    //
    // That door is gone. The two sliders were a second control over one fact,
    // pointing the opposite way from the dial and reaching a range the dial cannot
    // express, so they were deleted and `set_thresholds` with them (DECISIONS
    // §96) — a registered command with no rendered control is attack surface
    // nobody is watching.
    //
    // The GUARANTEE is not about which control was dragged. It is that whatever
    // sets the gate by hand goes through `apply_thresholds`, which moves the gate,
    // the anchor and the stored row together. So the test drags the one control
    // that remains and asserts exactly what it asserted before. A future control
    // that reaches `Router::set_thresholds` directly instead fails this in the
    // same two ways it was written to catch.
    #[test]
    fn r4_10_the_gate_control_leaves_the_profile_in_a_state_the_router_was_in() {
        use tauri::Manager;
        let app = crate::qa::bare_app();
        let h = app.handle().clone();

        // A cautious dial position, chosen for the reason 0.80 was: nowhere near
        // the seeded default, so a row that agrees with it can only have been
        // written by the command under test.
        const DIAL: u8 = 13;
        let wanted = Thresholds::from_sensitivity(DIAL);

        // The operator drags the one gate control there is.
        crate::set_sensitivity(
            h.clone(),
            h.state::<crate::Routing>(),
            h.state::<crate::Db>(),
            DIAL,
        )
        .expect("set_sensitivity");

        // Later in the service they accept one suggestion, which persists.
        let live = h.state::<crate::Routing>().0.lock().unwrap().thresholds();
        let dbs = h.state::<crate::Db>();
        {
            let conn = dbs.0.lock().unwrap();
            crate::persist_active_thresholds(&conn, live);
        }

        let conn = dbs.0.lock().unwrap();
        let p = crate::db::active_voice_profile(&conn)
            .unwrap()
            .expect("a fresh install has one active profile");
        let implied = Thresholds::from_sensitivity(p.sensitivity.clamp(0, 100) as u8);

        // ONE DIAL STEP, not bit-exactness.
        //
        // `sensitivity` is an integer 0..=100 and the thresholds are floats, so a
        // round trip through the dial cannot be exact and must not be made exact:
        // snapping the operator's 0.80 to whatever the nearest dial position
        // implies would silently move a number they set deliberately. On the
        // cautious half the dial moves `auto_fire` by 0.40/50 = 0.008 per step, so
        // one step is the tightest honest bound.
        //
        // The defect this guards against was never a rounding gap. It was a row
        // reading `sensitivity = 50` (auto_fire 0.50) beside a stored auto_fire of
        // 0.80 — a state the router was never in, and one that `apply_profile`
        // re-anchors from at the next launch, decaying calibration back to the
        // dial position the operator had just overruled.
        let gap = (implied.auto_fire - p.auto_fire as f32).abs();
        assert!(
            gap <= 0.01,
            "profile says sensitivity={} (auto_fire {:.3}) beside a stored \
             auto_fire of {:.3} — a gap of {gap:.3}, far wider than the 0.008 one \
             dial step can explain. That is a state the router was never in, and \
             the next launch anchors the baseline at {:.3}.",
            p.sensitivity,
            implied.auto_fire,
            p.auto_fire,
            implied.auto_fire
        );

        // The other half, and the one that bites within the SAME session: the
        // dial is the anchor self-calibration decays toward (DECISIONS §26). Set
        // the gate without setting it and every later confirm or dismiss drags the
        // gate back toward the position the operator just left.
        let baseline = h
            .state::<crate::Routing>()
            .0
            .lock()
            .unwrap()
            .baseline()
            .auto_fire;
        assert!(
            (baseline - wanted.auto_fire).abs() < 1e-4,
            "the baseline is {baseline:.3}, not the {:.3} the operator set — the \
             gate control moved the gate and left the anchor behind",
            wanted.auto_fire
        );
    }
}

#[cfg(test)]
mod spoken_nav_speaks_the_tier1_languages {
    use super::*;

    /// R4-G, closed 2026-08-15.
    ///
    /// `parse_reference` asked `is_chapter_word`/`is_verse_word`, which consult
    /// `numerals.json` and so already spoke Swahili and Hausa. Its three siblings
    /// matched English literals inline. The visible result: a preacher whose FULL
    /// reference parsed — "Yohana sura ya tatu mstari wa kumi na sita" → John 3:16 —
    /// then said "mstari wa nne" to move within that passage, and nothing happened.
    ///
    /// Code-switching is the normal case in this product, not an edge case, and the
    /// half that failed was the half used mid-reading.
    #[test]
    fn an_in_passage_jump_works_in_swahili_and_hausa() {
        // English — the control. If this ever breaks, the repair overshot.
        assert_eq!(detect_bare_verses("verse four"), vec![4]);
        assert_eq!(
            detect_passage_nav("chapter five verse one"),
            Some(PassageNav {
                chapter: Some(5),
                verse: Some(1)
            })
        );

        // Swahili: "mstari wa nne" — verse four. `wa` is the grammatical linker,
        // and skipping it is why the English-only version could not have worked
        // even with the vocabulary added.
        assert_eq!(detect_bare_verses("mstari wa nne"), vec![4]);
        assert_eq!(
            detect_passage_nav("sura ya tano"),
            Some(PassageNav {
                chapter: Some(5),
                verse: None
            })
        );

        // Hausa: "aya ta huɗu" — verse four.
        assert_eq!(detect_bare_verses("aya ta huɗu"), vec![4]);
        assert_eq!(
            detect_passage_nav("sura ta biyar"),
            Some(PassageNav {
                chapter: Some(5),
                verse: None
            })
        );
    }

    /// The guard that keeps the widening honest: a jump is only a jump when no book
    /// was named, and only when a keyword was actually spoken. Widening the keyword
    /// vocabulary must not widen what COUNTS as a command.
    #[test]
    fn a_full_reference_is_still_not_a_jump_and_bare_numbers_are_still_not_commands() {
        assert_eq!(
            detect_passage_nav("Yohana sura ya tatu mstari wa kumi na sita"),
            None
        );
        assert_eq!(detect_passage_nav("tano"), None);
        assert_eq!(detect_passage_nav("nne"), None);
        assert!(detect_bare_verses("hakuna marejeo hapa").is_empty());
    }
}

#[cfg(test)]
mod chapter_and_verse {
    use super::*;

    /// THE BUG, from a real service on 2026-08-23.
    ///
    /// "chapter N **and** verse M" is ordinary English and the connector was
    /// throwing the verse away: the reference parsed as whole-chapter and Relay put
    /// **verse 1** on the wall at 0.88, unattended. One sitting produced
    /// 1 Corinthians 9:1, 2 Chronicles 15:1 and 26:1, Proverbs 3:1, Isaiah 61:1,
    /// Hebrews 6:1, Genesis 12:1 and Psalms 23:1 this way. Removing the word made
    /// the identical sentence parse correctly, which is what identified it.
    #[test]
    fn chapter_n_and_verse_m_keeps_the_verse() {
        for (text, book, ch, v) in [
            (
                "1 Corinthians chapter 9 and verse 24",
                "1 Corinthians",
                9,
                24,
            ),
            (
                "2 Chronicles chapter 15 and verse 12",
                "2 Chronicles",
                15,
                12,
            ),
            ("Proverbs chapter 3 and verse 5", "Proverbs", 3, 5),
            ("Isaiah chapter 61 and verse 2", "Isaiah", 61, 2),
        ] {
            let hits = detect_direct(text);
            let m = hits
                .first()
                .unwrap_or_else(|| panic!("{text:?} parsed to nothing"));
            assert!(
                !m.whole_chapter,
                "{text:?} threw the verse away and became a whole-chapter reading — \
                 that puts verse 1 on a wall"
            );
            assert_eq!(
                (
                    m.reference.book.as_str(),
                    m.reference.chapter,
                    m.reference.verse
                ),
                (book, ch, v),
                "{text:?} parsed to the wrong verse"
            );
        }
    }

    /// The connector must be worth the same as no connector. If these ever diverge
    /// again, one phrasing of the same sentence is reaching a different wall.
    #[test]
    fn the_connector_costs_nothing() {
        let with = detect_direct("1 Corinthians chapter 9 and verse 24");
        let without = detect_direct("1 Corinthians chapter 9 verse 24");
        assert_eq!(with.len(), without.len());
        assert_eq!(with[0].reference, without[0].reference);
        assert!(
            (with[0].confidence - without[0].confidence).abs() < f32::EPSILON,
            "the same reference scored differently with and without 'and': {} vs {}",
            with[0].confidence,
            without[0].confidence
        );
    }

    /// A bare chapter is still a real thing to say and must survive untouched —
    /// "turn to Psalm twenty three" is a whole-chapter reading, and verse 1 is the
    /// right answer to it. The fix above must not turn every chapter into a verse.
    #[test]
    fn a_bare_chapter_is_still_a_whole_chapter() {
        let hits = detect_direct("Let us turn together to Psalm twenty three.");
        let m = hits.first().expect("a bare chapter still parses");
        assert!(m.whole_chapter, "a bare chapter stopped being a chapter");
        assert_eq!(m.reference.chapter, 23);
        assert_eq!(m.reference.verse, 1);
    }

    /// "and" joining two NUMBERS is not a chapter/verse separator. The connector is
    /// only skipped when a verse word actually follows it, so this must be unchanged.
    #[test]
    fn and_between_two_numbers_is_left_alone() {
        let hits = detect_direct("Hebrews chapter 12 and 13");
        for m in &hits {
            assert_ne!(
                (m.reference.chapter, m.reference.verse),
                (12, 13),
                "'chapter 12 and 13' was read as 12:13 — the connector rule is too greedy"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// THE LANGUAGE REPORT — the moat, measured rather than asserted
// ─────────────────────────────────────────────────────────────────────────────
//
// `docs/LANGUAGES.md` states the truth about Relay's African-language support in
// prose, and it is unusually honest prose: 66 of 66 books in all three languages,
// **none reviewed by anyone who speaks them**, word error rate **never measured, in
// any language**, and Yorùbá numerals **not parsed at all**.
//
// Prose cannot be tracked. A contributor fixing eleven Yorùbá book names has no way
// to see that they moved anything, and a reader has no way to tell whether the
// document is current. This turns the same facts into numbers derived FROM THE
// SHIPPED DATA — so the report cannot flatter the product, because the only way to
// improve it is to improve the data the binary actually uses.
//
// **The two things it must never do**: report an unmeasured thing as a number, and
// count a language as reviewed because somebody wrote a file. Native review is a
// person's judgement and nothing here can observe it, so it is reported as an
// absence with the number of aliases that are waiting.

/// What is known about one language, all of it derived from the bundled data.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LanguageReport {
    /// The code a transcript carries (`yo`, `sw`, `ha`).
    pub code: String,
    /// The name in that language, from the data file.
    pub name: String,
    /// Books with at least one alias, out of 66.
    pub books: usize,
    pub books_total: usize,
    /// How many spoken forms are recognised in total. A book with five aliases is
    /// five ways a preacher can say it and be understood.
    pub aliases: usize,
    /// Does Relay parse numbers spoken IN this language ("sura ya tatu")? When
    /// false, a reference in this language only resolves when the numbers are said
    /// in English, which is common but not universal.
    pub numerals: bool,
    /// May a reference resolved through those numerals reach a wall UNATTENDED?
    ///
    /// **False for a numeral table no native speaker has signed off.** Yorùbá
    /// parses today and is capped at `Suggest` (`parsed_an_unreviewed_numeral`),
    /// and a screen that printed a bare "yes" beside Swahili would be claiming the
    /// two are the same thing. They are not: one fires, one asks.
    pub numerals_auto_fire: bool,
    /// **Always false, and reported as an absence rather than a score.** No record
    /// exists of a native speaker checking these, because none has. The day one
    /// does, this becomes a real field and not before.
    pub native_reviewed: bool,
    /// **Always `None`.** Word error rate has never been measured in any language,
    /// including English. `None` renders as "not measured"; a number here would be
    /// the single most misleading thing in the product.
    pub wer: Option<f32>,
}

/// The canonical book count every language is measured against.
pub const BOOKS_IN_THE_BIBLE: usize = 66;

/// Measure every tier-1 language against the data that actually ships.
pub fn language_report() -> Vec<LanguageReport> {
    const RAW: &str = include_str!("../data/book_aliases.json");
    const NUM: &str = include_str!("../data/numerals.json");

    let Ok(doc) = serde_json::from_str::<serde_json::Value>(RAW) else {
        // The same failure `language_aliases` reports. An empty report is honest:
        // nothing can be said about a file that would not parse.
        return Vec::new();
    };
    let numerals: serde_json::Value = serde_json::from_str(NUM).unwrap_or(serde_json::Value::Null);

    let mut out = Vec::new();
    for (lang, books) in doc.as_object().into_iter().flatten() {
        if lang.starts_with('_') {
            continue;
        }
        let Some(books) = books.as_object() else {
            continue;
        };

        let mut with_alias = 0usize;
        let mut aliases = 0usize;
        for (english, names) in books {
            if english.starts_with('_') {
                continue;
            }
            // Only count a book Relay can actually key on. A typo in the data file
            // is a name no transcript will ever match, and counting it would make
            // the report improve as the data got worse.
            if !CANONICAL_BOOKS.iter().any(|b| b == english) {
                continue;
            }
            let n = names.as_array().map(|a| a.len()).unwrap_or(0);
            if n > 0 {
                with_alias += 1;
                aliases += n;
            }
        }

        out.push(LanguageReport {
            code: lang.clone(),
            name: books
                .get("_language")
                .and_then(|v| v.as_str())
                .unwrap_or(lang)
                .to_string(),
            books: with_alias,
            books_total: BOOKS_IN_THE_BIBLE,
            aliases,
            // `ones` OR `standalone`: Yorùbá is vigesimal and carries the second,
            // so a check for `ones` alone reported "no" while the detector was
            // parsing them — the one disagreement between this screen and the app
            // that this whole function exists to prevent.
            numerals: ["ones", "standalone"].iter().any(|k| {
                numerals
                    .get(lang)
                    .and_then(|v| v.get(*k))
                    .and_then(|v| v.as_object())
                    .is_some_and(|o| !o.is_empty())
            }),
            numerals_auto_fire: ["ones", "standalone"].iter().any(|k| {
                numerals
                    .get(lang)
                    .and_then(|v| v.get(*k))
                    .and_then(|v| v.as_object())
                    .is_some_and(|o| !o.is_empty())
            }) && numerals
                .get(lang)
                .and_then(|v| v.get("unreviewed"))
                .and_then(|v| v.as_bool())
                != Some(true),
            native_reviewed: false,
            wer: None,
        });
    }
    out.sort_by(|a, b| a.code.cmp(&b.code));
    out
}

#[cfg(test)]
mod language_report_tests {
    use super::*;

    #[test]
    fn every_tier_one_language_is_measured_from_the_shipped_data() {
        let r = language_report();
        let codes: Vec<&str> = r.iter().map(|l| l.code.as_str()).collect();
        assert_eq!(codes, vec!["ha", "sw", "yo"]);
        for l in &r {
            assert!(!l.name.is_empty(), "{} has no name", l.code);
            assert!(l.books > 0, "{} has no books", l.code);
            assert!(l.aliases >= l.books, "each book needs at least one alias");
        }
    }

    /// THE REPORT MATCHES WHAT THE DETECTOR ACTUALLY LOADS.
    ///
    /// A report derived from a different reading of the same file would be a second
    /// answer to one question — and this one exists precisely to be trusted about
    /// the state of the data.
    #[test]
    fn the_count_is_the_detectors_own_view_of_the_data() {
        let loaded = language_aliases();
        let reported: usize = language_report().iter().map(|l| l.aliases).sum();
        // `language_aliases` normalises and may collapse duplicates, so the loaded
        // count can only ever be <= the reported one. It must never be MORE, which
        // would mean the report is missing aliases the detector is using.
        assert!(
            loaded.len() <= reported,
            "the report ({reported}) claims fewer aliases than the detector loaded ({})",
            loaded.len()
        );
        assert!(reported > 150, "sanity: the table is not empty");
    }

    /// YORÙBÁ NUMERALS ARE NOT PARSED, AND THE REPORT SAYS SO.
    ///
    /// This is the single largest known gap in the tier-1 list — Yorùbá is
    /// subtractive (16 = ẹrìndínlógún) and the largest addressable market of the
    /// three. **It fired, and this is the updated claim**: Yorùbá numerals now
    /// parse, and they are capped at `Suggest` until a native speaker signs the
    /// table off, so the report has to say BOTH — a bare "yes" beside Swahili
    /// would claim the two behave the same, and they do not.
    #[test]
    fn the_report_names_the_numerals_gap_rather_than_hiding_it() {
        let r = language_report();
        let l = |c: &str| r.iter().find(|x| x.code == c).unwrap();
        for c in ["sw", "ha"] {
            assert!(l(c).numerals, "{c} numerals are parsed");
            assert!(
                l(c).numerals_auto_fire,
                "{c} numerals are reviewed and fire"
            );
        }
        assert!(
            l("yo").numerals,
            "Yorùbá numerals are parsed as of the yo block"
        );
        assert!(
            !l("yo").numerals_auto_fire,
            "Yorùbá numerals are UNREVIEWED and must report as suggest-only — if \
             this fails, a native speaker has signed the table off, so update \
             LANGUAGES.md and this test"
        );
    }

    /// NOTHING UNMEASURED IS EVER REPORTED AS A NUMBER.
    ///
    /// Word error rate has never been measured in any language, and no native
    /// speaker has reviewed any of these tables. A score in either field would be
    /// the most misleading thing in the product — it is the moat, and the moat is
    /// currently an assertion.
    #[test]
    fn unmeasured_things_are_absent_not_zero() {
        for l in language_report() {
            assert!(l.wer.is_none(), "{}: WER has never been measured", l.code);
            assert!(
                !l.native_reviewed,
                "{}: no native speaker has reviewed this table",
                l.code
            );
        }
    }
}

// ── READING ON: the preacher has moved to the next verse without saying so ──
//
// An operator puts a passage up and the preacher reads through it. Today the only
// way the screen follows is the operator pressing Next, or the preacher SAYING
// "verse two" (`detect_passage_nav`, which already works). This is the third case,
// and the common one: they simply read on.
//
// ## Why this is not rule 10 being bent
//
// Rule 10 caps `Semantic` at Suggest because a TF-IDF cosine over 31,000 verses is
// not a probability. This asks a much smaller question: of the two verses that
// come NEXT in a passage an operator has already approved and put on a wall, do
// the words just spoken belong to one of them? It cannot start a passage, cannot
// leave one, and cannot go backwards. It is not the AI choosing what to show; it
// is the AI keeping up with a reading somebody already chose.
//
// ## What the numbers actually said
//
// Tuned against the two real readings in service 24 (2026-09-20) and the bundled
// KJV, not guessed:
//
// ```text
//   case                 6 words   12     20    all    first advance
//   Romans 8:28->29        +0.00  +0.43  +1.00  +1.00   after 12 words
//   Romans 8:29->30        -0.38  +0.00  +0.40  +0.60   after 20 words
//   Numbers 10:29->30      -1.00  -1.00  +0.00  +0.25   only at the end
//   Numbers 10:30->31      -0.60  -0.14  +0.50  +1.00   after 20 words
//   Numbers 10:31->32      -1.00  -0.50  +0.00  +0.14   never
// ```
//
// **Three things follow, and all three are in the design rather than in a hope.**
//
// It NEVER advances while the current verse is still being read: every such case
// scored -1.00. The failure mode is silence, and silence is the only failure mode
// worth having here — the operator presses Next exactly as they do today.
//
// It advances LATE, twelve to twenty words in, three to six seconds behind. That
// is structural: an eight-second window holds all of the verse being left and only
// the opening of the one being entered. Nothing about the threshold changes it;
// whole-sentence input would.
//
// And one case in five never advances at all. Numbers 10:32 is "goodness" and
// "lord" once stopwords are gone. There is nothing there to match, so no number
// reaches it. Narrative verses do this; Romans does not.
//
// ## The first attempt, and why it is not this one
//
// Scoring the WHOLE window against each candidate fails: the current verse is
// entirely present and the next one barely, so the next verse loses three times in
// five even when the preacher has plainly moved on. Only the NEWEST words
// discriminate, which is why `RECENCY` exists.

/// Words too common in the KJV to distinguish one verse from another.
///
/// Not a general stopword list — a list for THIS corpus. "shall", "thou", "thee",
/// "unto" and "hath" are on it because they appear in thousands of verses and
/// carry no evidence about which one is being read.
// ── READ-ON IS BUILT AND TESTED AND IS NOT WIRED TO ANYTHING YET ───────────
//
// `read_on` answers "has the preacher stopped reading the verse on the screen
// and started reading the next one", and it does it well enough to ship: see
// `read_on_tests`, and the tuning table in the module note. What it does NOT yet
// have is a caller. Advancing a passage by itself touches the fire path, needs a
// hold so the operator's own Next is never undone a moment later, and needs an
// `e2e.rs` case driving the real commands — none of which is written.
//
// `allow(dead_code)` RATHER THAN DELETING IT, and rather than wiring it in a
// hurry to make clippy quiet. The measurement it encodes came off two real
// readings in a real service and is the expensive part; the wiring is an
// afternoon. This comment is here so the next person finds a stated absence
// instead of guessing whether the feature is live — which, on the fire path, is
// the difference between reading the code and trusting it.
#[allow(dead_code)]
const READ_ON_STOPWORDS: &[&str] = &[
    "the", "and", "of", "to", "in", "that", "is", "was", "he", "for", "it", "with", "as", "his",
    "on", "be", "at", "by", "this", "had", "not", "are", "but", "from", "or", "have", "an", "they",
    "which", "one", "you", "were", "all", "her", "she", "there", "would", "their", "we", "him",
    "been", "has", "when", "who", "will", "no", "more", "if", "out", "so", "up", "said", "what",
    "its", "about", "into", "them", "then", "unto", "shall", "thou", "thee", "thy", "ye", "hath",
    "also", "her", "my", "me", "us", "our", "your",
];

/// How many of the newest content words decide it.
///
/// The measurement above is what sets this. Scoring the whole window lets the
/// verse being LEFT win on sheer presence, because all of it is in the window and
/// only the opening of the next one is.
#[allow(dead_code)]
const RECENCY: usize = 8;

/// How far ahead the next verse must be before the screen moves.
///
/// Zero would advance on a tie, and a tie is exactly what the middle of a verse
/// looks like. 0.15 is the smallest figure that cleared every still-reading case
/// in the measurement above, all of which scored -1.00 — so this is not a fence
/// sitting next to the data, it is a fence a long way from it.
#[allow(dead_code)]
const READ_ON_MARGIN: f32 = 0.15;

/// The content words of a line, in order, for read-on scoring.
#[allow(dead_code)]
fn read_on_tokens(text: &str) -> Vec<String> {
    normalize(text)
        .split_whitespace()
        .filter(|t| t.chars().count() >= 3 && !READ_ON_STOPWORDS.contains(t))
        .map(|t| t.to_string())
        .collect()
}

/// Of the words just spoken, how many belong to this verse?
///
/// The denominator is what was HEARD, not the verse. Asking "how much of this
/// verse has been said" rewards a verse already finished; asking "how much of what
/// was just said is this verse" is the question an advance actually turns on.
#[allow(dead_code)]
fn read_on_score(candidate: &str, recent: &[String]) -> f32 {
    if recent.is_empty() {
        return 0.0;
    }
    let cand: std::collections::HashSet<String> = read_on_tokens(candidate).into_iter().collect();
    if cand.is_empty() {
        return 0.0;
    }
    let hits = recent.iter().filter(|t| cand.contains(*t)).count();
    hits as f32 / recent.len() as f32
}

/// Has the preacher moved on? Returns the verse to advance TO.
///
/// `candidates` are the verses that may be moved to, in order, with their text —
/// the caller supplies at most the next two, and never anything outside the
/// passage. This function cannot reach further than it is given, which is what
/// keeps the bound a property of the code rather than of a comment.
#[allow(dead_code)]
pub fn read_on(
    heard: &str,
    current_text: &str,
    candidates: &[(VerseRef, String)],
) -> Option<VerseRef> {
    let all = read_on_tokens(heard);
    if all.is_empty() {
        return None;
    }
    let recent: Vec<String> = all.iter().rev().take(RECENCY).rev().cloned().collect();
    let here = read_on_score(current_text, &recent);
    let (best, score) = candidates
        .iter()
        .map(|(r, t)| (r, read_on_score(t, &recent)))
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    (score > 0.0 && score - here >= READ_ON_MARGIN).then(|| best.clone())
}

#[cfg(test)]
mod read_on_tests {
    use super::*;

    const R28: &str = "And we know that all things work together for good to them that love God, to them who are the called according to {his} purpose.";
    const R29: &str = "For whom he did foreknow, he also did predestinate {to be} conformed to the image of his Son, that he might be the firstborn among many brethren.";
    const R30: &str = "Moreover whom he did predestinate, them he also called: and whom he called, them he also justified: and whom he justified, them he also glorified.";
    const N32: &str = "And it shall be, if thou go with us, yea, it shall be, that what goodness the LORD shall do unto us, the same will we do unto thee.";

    fn vr(book: &str, chapter: i64, verse: i64) -> VerseRef {
        VerseRef {
            book: book.into(),
            chapter,
            verse,
        }
    }
    fn cands(list: &[(i64, &str)]) -> Vec<(VerseRef, String)> {
        list.iter()
            .map(|(v, t)| (vr("Romans", 8, *v), (*t).to_string()))
            .collect()
    }

    /// What the decoder's window actually holds part-way through a hand-over: the
    /// last eight words of the verse still on the screen, then `words` words of the
    /// one being read now.
    fn window_after(current: &str, next: &str, words: usize) -> String {
        let tail: Vec<&str> = current.split_whitespace().collect();
        let tail = tail[tail.len().saturating_sub(8)..].join(" ");
        let head = next
            .split_whitespace()
            .take(words)
            .collect::<Vec<_>>()
            .join(" ");
        format!("{tail} {head}")
    }

    /// THE ONE THAT MATTERS MOST, and it is a refusal.
    ///
    /// While the preacher is still reading the verse on the screen, nothing moves.
    /// Every still-reading case in the tuning scored -1.00, and this is that
    /// measurement turned into a test: the failure mode of this whole feature must
    /// be silence, because the operator's Next already works and a wrong verse
    /// does not undo itself.
    #[test]
    fn nothing_moves_while_the_current_verse_is_still_being_read() {
        let c = cands(&[(29, R29), (30, R30)]);
        assert_eq!(read_on(R28, R28, &c), None);
        // Even most of the way through it.
        let half = R28
            .split_whitespace()
            .take(14)
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(read_on(&half, R28, &c), None);
    }

    /// AND THE ADVANCE ITSELF, at the point the measurement said it happens.
    #[test]
    fn it_advances_once_enough_of_the_next_verse_has_been_heard() {
        let c = cands(&[(29, R29), (30, R30)]);
        // Twelve words in — the figure the tuning recorded for this reading.
        let heard = window_after(R28, R29, 12);
        assert_eq!(read_on(&heard, R28, &c).map(|r| r.verse), Some(29));
    }

    /// IT CANNOT REACH PAST WHAT IT WAS GIVEN.
    ///
    /// The bound is a property of the call, not of a comment: the caller hands it
    /// the verses that may be moved to, so a passage's end and "never backwards"
    /// are enforced by there being nothing else in the list.
    #[test]
    fn it_can_only_move_to_a_verse_it_was_offered() {
        // The words of verse 30 are ringing, but only 29 is on offer.
        let only_29 = cands(&[(29, R29)]);
        let heard = R30;
        let got = read_on(heard, R28, &only_29);
        assert!(
            got.is_none() || got.unwrap().verse == 29,
            "it reached a verse that was not a candidate"
        );
        // And with nothing on offer it does nothing at all — the end of a passage.
        assert_eq!(read_on(R29, R28, &[]), None);
    }

    /// A VERSE MADE OF STOPWORDS CANNOT BE HEARD, AND THAT IS RECORDED.
    ///
    /// Numbers 10:32 is "goodness" and "lord" once the common words are gone. The
    /// tuning said it never advances and this pins that, so the next person reads a
    /// known limit rather than finding it in a service. It is not a bug to be
    /// fixed by lowering the margin: lowering it far enough to catch this would
    /// advance on a tie, and the middle of a verse looks exactly like a tie.
    #[test]
    fn a_verse_that_is_almost_all_common_words_is_a_known_miss() {
        let c = vec![(vr("Numbers", 10, 32), N32.to_string())];
        const N31: &str = "And he said, Leave us not, I pray thee; forasmuch as thou knowest \
                           how we are to encamp in the wilderness, and thou mayest be to us \
                           instead of eyes.";
        // Built the way a real window is: the tail of the verse being read, then
        // the first words of the next one. Scored against N32's own words alone it
        // WOULD advance, and that difference is the point — the current verse's
        // tail is part of the evidence, not noise to be stripped out before asking.
        let heard = window_after(N31, N32, 14);
        assert_eq!(read_on(&heard, N31, &c), None);
    }

    /// Sermon prose about a verse is not the verse being read.
    #[test]
    fn expounding_a_verse_is_not_reading_the_next_one() {
        let c = cands(&[(29, R29), (30, R30)]);
        let heard = "so what Paul is telling us this morning is that God is working, \
                     he is working right now in your situation, whatever it looks like";
        assert_eq!(read_on(heard, R28, &c), None);
    }

    /// Silence, or a window with nothing in it, moves nothing.
    #[test]
    fn an_empty_window_moves_nothing() {
        let c = cands(&[(29, R29)]);
        assert_eq!(read_on("", R28, &c), None);
        assert_eq!(read_on("the and of to", R28, &c), None);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// QUOTED SCRIPTURE · A CONTIGUOUS PHRASE, NOT A BAG OF WORDS
// ═══════════════════════════════════════════════════════════════════════════
//
// `SemanticIndex` answers "which verse MEANS this", and it answers it with a
// TF-IDF cosine, which discards word order by construction. That is the right
// instrument for a preacher RETELLING a story ("he ended up feeding pigs"), and
// the wrong one for a preacher QUOTING. Its evidence reaches the console as
// `terms.join(" · ")` and is rendered inside quotation marks, so a verse whose
// whole justification was the words `lord` and `shepherd`, in neither order and
// nowhere near each other, appeared on the run surface as
//
//     “lord · shepherd”
//
// dressed as a quotation. The operator's instruction, 2026-09-20: *"it has to be
// three words together as in the scripture"*. This module is that sentence.
//
// ── WHY IT IS A SEPARATE INDEX AND NOT A TIGHTER SEMANTIC ─────────────────
//
// Because the two questions want opposite tokenizers. `tokenize` drops stopwords
// and two-letter words, which is correct for a cosine and fatal for a quotation:
// "the lord is my shepherd" reduces to `[lord, shepherd]` and "seek ye first"
// to `[seek, first]`. The very phrases the operator named are the ones a
// content-word index cannot see. So a phrase index keeps EVERY word.
//
// ── MEASURED, on the full KJV and on one real 93-minute service ────────────
//
// Uniqueness of an n-gram over all words (31,102 verses):
//
//     3 words   418,590 distinct   78.1% unique to a single verse
//     4 words   573,762 distinct   88.1%
//     5 words   625,020 distinct   92.9%
//
// Run over all 816 transcript windows of the service of 2026-09-20, taking the
// longest contiguous run each window shares with any verse:
//
//     run >= 3    300 fires   unusable
//     run >= 4     67 fires   one every 1.4 min, heavy noise ("was a good man")
//     run >= 5     24 fires   ~16 correct
//     run >= 5 and rarity      13 fires   11 correct
//
// ONE SERVICE IS ONE SAMPLE. These constants are configuration shaped by a
// measurement, not laws, and the honest statement of their status is that they
// have been fitted to a single morning of one preacher in one language.
//
// ── WHAT IT MAY DO ─────────────────────────────────────────────────────────
//
// Suggest. Never fire. A quoted phrase is strong evidence about WHICH VERSE and
// no evidence at all that the operator wants it on a wall, and rule 10 is not
// about how good the evidence is: only what Relay HEARD AS A REFERENCE may reach
// a congregation unattended. `DetectionMethod::Quoted` is capped in `router.rs`
// beside `Semantic`.

/// How many words make the index's unit. Short enough that a three-word
/// quotation is findable at all, which is the whole requirement.
const PHRASE_GRAM: usize = 3;

/// A gram this common is a phrase of the language, not a phrase of a verse, and
/// looking it up would cost more than it could ever return. "and it came to"
/// is in hundreds of verses.
const MAX_GRAM_VERSES: usize = 8;

/// The shortest run of words that may be offered as a quotation.
///
/// FIVE, measured (see the module note): at four the service produced a
/// suggestion every 1.4 minutes, most of them ordinary English that happens to
/// appear somewhere in a 790,000-word book. The cost of the wrong value here is
/// paid by a volunteer in a dark booth, so it is set where the list is short
/// enough to read.
pub const MIN_RUN_WORDS: usize = 5;

/// A run this long is its own corroboration and needs no rarity check.
/// "moreover it is required in stewards that a man be found" is twelve words and
/// nobody says that by accident.
const SELF_EVIDENT_RUN: usize = 7;

/// Below `SELF_EVIDENT_RUN`, the run must contain at least one word this rare —
/// present in at most this fraction of verses. 0.5% of the KJV is ~155 verses.
///
/// This is the same idea as `RARE_DF_FRACTION` one door along, at a different
/// value because it is doing a different job: there a single rare word stands in
/// for missing corroboration, here it separates a five-word quotation from five
/// words of ordinary speech. Measured: it removes "the poor out of the",
/// "for the fruit of the" and "is it in the world" while keeping
/// "wisdom is the principal thing" and "a solitary place and there".
const PHRASE_RARE_FRACTION: f32 = 0.005;

/// The shortest run that may be treated as the preacher READING the verse, and
/// so put on a wall without anybody pressing anything.
///
/// EIGHT, and here is what was measured for it (2026-09-23, against the bundled
/// KJV — `src-tauri/data/kjv.json`, 31,102 verses, 835,656 words, the same
/// tokenizer `phrase_words` uses):
///
/// **How often a run of N words belongs to exactly one verse**
///
/// | words | distinct runs | unique to ONE verse |
/// |-------|---------------|---------------------|
/// | 5     | 625,020       | 93.0%               |
/// | 7     | 617,197       | 96.6%               |
/// | **8** | **596,170**   | **97.4%**           |
/// | 10    | 544,918       | 98.3%               |
/// | 12    | 489,445       | 98.8%               |
///
/// **What the other 2.6% actually are.** 1,500 randomly chosen verses were read
/// back N+2 words at a time and put through `quoted` with no book named. At a
/// run of 8, forty-two of them named a different verse — and **every single one
/// was an exact tie**: the same words, verbatim, in two places (Judges 1:12 and
/// Joshua 15:16; Psalms 107:8 and 107:21; the synoptic parallels). Not one was a
/// case of a LONGER run being found somewhere else. Refusing a tie removed all
/// forty-two, and at every floor from 5 to 12 the wrong-verse count in that
/// simulation went to **zero**. That is why the rule has two halves and why the
/// second half is not optional.
///
/// **The other direction.** 38,887 windows of twenty-five words of ordinary
/// modern English (this repository's own prose) produced a quoted hit at all in
/// 62 of them; the 28 at a run of seven or more were, on inspection, the docs
/// literally quoting scripture. And on the 43-case paraphrase corpus, every case
/// whose longest run reached seven named the labelled passage — the single miss
/// was at a run of five.
///
/// **Why eight and not seven.** `SELF_EVIDENT_RUN` is 7: the length at which a
/// run needs no rare word to stand as a SUGGESTION. Reaching an operator and
/// reaching a congregation are not the same bar, so the floor for a wall is set
/// strictly above it. 1.2% of verses (376) are shorter than eight words and can
/// therefore never be followed; they are still offered, exactly as before.
///
/// **What has NOT been measured.** None of this is speech. There is no
/// word-error-rate figure in any language, and the one real-service figure that
/// exists for this module — 13 suggestions in 93 minutes at a run of five plus a
/// rare word, 11 of them correct — was taken before the transcript was kept, so
/// nobody can say how many of those thirteen would clear eight words and a sole
/// verse. See `docs/LANGUAGES.md`; this is configuration shaped by measurements
/// over TEXT, not a law.
pub const READING_RUN_WORDS: usize = 8;

/// One verse whose words the speaker said, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct PhraseHit {
    /// The verse those words are in.
    pub r: VerseRef,
    /// How many words ran together. The ranking key, and the operator-facing
    /// measure of how much of the verse was actually said.
    pub run: usize,
    /// THE WORDS THEMSELVES, as the window had them, in order.
    ///
    /// This is what reaches `matched_text`, and it is the point of the whole
    /// module: the field means "the span this was parsed from" everywhere else in
    /// this file, and for a paraphrase it was being filled with a word list.
    pub phrase: String,
    /// Does any OTHER verse hold a run this long from the same window?
    ///
    /// ── The measurement that made this a field (2026-09-23) ────────────────
    ///
    /// Reading 1,500 real verses back through `quoted` produced a wrong verse 42
    /// times at a run of eight words, and **all 42 were exact ties** — the words
    /// really are in two places, and the deterministic tie-break picked the other
    /// one. Not one was a longer run found elsewhere. So the residual error of
    /// this module is not "Relay misread the quotation"; it is "Relay chose".
    ///
    /// Choosing between two verses that both hold the words is a guess about
    /// WHICH, and rule 10 is exactly the rule that a guess may reach the operator
    /// and never a wall. It is also RG-178's shape one door along: `Numbers 10:29`
    /// and `Genesis 10:29` from one sentence, offered rather than fired.
    ///
    /// Computed BEFORE the list is truncated, so a third verse falling off the
    /// end of `k` cannot make a tied hit look sole. Computed AFTER the book
    /// filter, so a book the window named settles the tie — the words said which,
    /// and when the words say, the words win (rule 40).
    ///
    /// ── AND A SUB-SPAN OF A LONGER RUN IS NOT A SECOND HEARING ─────────────
    ///
    /// Found by the scorecard the day the promotion landed. A preacher reading
    /// John 3:16 aloud produces a 25-word run in John 3:16 **and** a 10-word run
    /// in John 3:15, because those ten words are in both verses — and the ten sit
    /// wholly inside the twenty-five. Two verses, one span of speech. That is
    /// rule 29's own finding in a new shape ("a chapter-only reading rides along
    /// with the verse — it is the first half of that reference, not a second
    /// one"), so a hit whose span is contained in a longer hit's is not sole. It
    /// is still OFFERED: John 3:15 is a perfectly reasonable thing for the
    /// operator to want, and a silent discard is a different lie.
    pub sole: bool,
}

/// Every contiguous word-run the KJV shares with a sentence, found by a sorted
/// gram table rather than by scanning 31,102 verses.
///
/// Built once at startup beside `SemanticIndex` and never mutated.
pub struct PhraseIndex {
    /// (gram hash, verse index), sorted by hash. A `Vec` and a binary search
    /// rather than a `HashMap`: iteration order has bitten this file twice
    /// (see `cosine`), the table is built once, and 730,000 pairs cost ~9 MB
    /// here against tens of megabytes as a map of string tuples.
    grams: Vec<(u64, u32)>,
    /// Word ids for every verse, concatenated. Comparing ids rather than
    /// `String`s is what makes extending a run free.
    flat: Vec<u32>,
    /// Where each verse starts in `flat`; has one extra entry so the last
    /// verse's end needs no special case.
    starts: Vec<u32>,
    refs: Vec<VerseRef>,
    /// word → id, and how many verses that word appears in.
    ids: HashMap<String, u32>,
    df: Vec<u32>,
}

/// Every word, lowercased, in order. NOT `tokenize`: stopwords and two-letter
/// words are exactly what makes a quotation a quotation.
fn phrase_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

/// FNV-1a over three word ids. Deterministic across processes and builds, unlike
/// anything seeded from `RandomState`.
fn gram_hash(a: u32, b: u32, c: u32) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for id in [a, b, c] {
        for byte in id.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

impl PhraseIndex {
    /// Build from the same `(reference, text)` corpus `SemanticIndex` takes.
    pub fn build(corpus: &[(VerseRef, String)]) -> Self {
        let mut ids: HashMap<String, u32> = HashMap::new();
        let mut df: Vec<u32> = Vec::new();
        let mut flat: Vec<u32> = Vec::new();
        let mut starts: Vec<u32> = Vec::with_capacity(corpus.len() + 1);
        let mut refs: Vec<VerseRef> = Vec::with_capacity(corpus.len());
        let mut grams: Vec<(u64, u32)> = Vec::new();

        for (vi, (r, text)) in corpus.iter().enumerate() {
            starts.push(flat.len() as u32);
            refs.push(r.clone());
            let words = phrase_words(text);
            let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
            let first = flat.len();
            for w in words {
                let next = ids.len() as u32;
                let id = *ids.entry(w).or_insert(next);
                if id as usize >= df.len() {
                    df.push(0);
                }
                if seen.insert(id) {
                    df[id as usize] += 1;
                }
                flat.push(id);
            }
            let n = flat.len() - first;
            for i in 0..n.saturating_sub(PHRASE_GRAM - 1) {
                let s = first + i;
                grams.push((gram_hash(flat[s], flat[s + 1], flat[s + 2]), vi as u32));
            }
        }
        starts.push(flat.len() as u32);
        grams.sort_unstable();
        PhraseIndex {
            grams,
            flat,
            starts,
            refs,
            ids,
            df,
        }
    }

    fn verse_words(&self, vi: usize) -> &[u32] {
        let a = self.starts[vi] as usize;
        let b = self.starts[vi + 1] as usize;
        &self.flat[a..b]
    }

    /// Verses holding this gram. Empty when it is too common to be evidence.
    fn verses_with(&self, h: u64) -> &[(u64, u32)] {
        let lo = self.grams.partition_point(|(g, _)| *g < h);
        let hi = self.grams.partition_point(|(g, _)| *g <= h);
        let slice = &self.grams[lo..hi];
        if slice.len() > MAX_GRAM_VERSES {
            &[]
        } else {
            slice
        }
    }

    /// Is any word of this run rare enough to stand as evidence on its own?
    fn has_a_rare_word(&self, run: &[u32]) -> bool {
        let cap = ((self.refs.len() as f32 * PHRASE_RARE_FRACTION) as u32).max(1);
        run.iter().any(|id| {
            self.df
                .get(*id as usize)
                .copied()
                .is_some_and(|d| d > 0 && d <= cap)
        })
    }

    /// The longest runs `heard` shares with any verse, longest first.
    ///
    /// `book` narrows the answer to one book when the words or the context have
    /// named one. THAT IS RULE 40 IN THIS MODULE: the preacher who said
    /// "Verse 7 says, Be not wise in your own eyes" while reading Proverbs 3 was
    /// quoting a phrase that is verbatim in Romans 12:16 as well, and the longest
    /// run alone answers Romans. When the words say, the words win.
    pub fn quoted(&self, heard: &str, book: Option<&str>, k: usize) -> Vec<PhraseHit> {
        let words = phrase_words(heard);
        if words.len() < MIN_RUN_WORDS {
            return Vec::new();
        }
        let q: Vec<u32> = words
            .iter()
            .map(|w| self.ids.get(w).copied().unwrap_or(u32::MAX))
            .collect();

        // verse index → (run length, where it started in the window)
        let mut best: HashMap<u32, (usize, usize)> = HashMap::new();
        for i in 0..q.len().saturating_sub(PHRASE_GRAM - 1) {
            if q[i] == u32::MAX || q[i + 1] == u32::MAX || q[i + 2] == u32::MAX {
                continue;
            }
            let h = gram_hash(q[i], q[i + 1], q[i + 2]);
            for (_, vi) in self.verses_with(h) {
                if let Some(b) = book {
                    if !self.refs[*vi as usize].book.eq_ignore_ascii_case(b) {
                        continue;
                    }
                }
                let vw = self.verse_words(*vi as usize);
                for j in 0..vw.len().saturating_sub(PHRASE_GRAM - 1) {
                    if vw[j..j + PHRASE_GRAM] != q[i..i + PHRASE_GRAM] {
                        continue;
                    }
                    let mut n = PHRASE_GRAM;
                    while i + n < q.len() && j + n < vw.len() && q[i + n] == vw[j + n] {
                        n += 1;
                    }
                    let e = best.entry(*vi).or_insert((0, 0));
                    if n > e.0 {
                        *e = (n, i);
                    }
                }
            }
        }

        // (hit, where its run started in the window). The start is kept beside the
        // hit rather than on it: `sole` below is the only thing that needs it, and
        // a public field is a promise to keep it meaningful for ever.
        let mut hits: Vec<(PhraseHit, usize)> = best
            .into_iter()
            .filter(|(_, (n, i))| {
                *n >= MIN_RUN_WORDS
                    && (*n >= SELF_EVIDENT_RUN || self.has_a_rare_word(&q[*i..*i + *n]))
            })
            .map(|(vi, (n, i))| {
                (
                    PhraseHit {
                        r: self.refs[vi as usize].clone(),
                        run: n,
                        phrase: words[i..i + n].join(" "),
                        // Filled in below, once the whole set is known.
                        sole: false,
                    },
                    i,
                )
            })
            .collect();
        // IS THIS HIT THE ONLY VERSE THIS EVIDENCE POINTS AT — asked over the
        // WHOLE set and before the truncation, because a rival that falls off the
        // end of `k` would otherwise leave the survivor looking like the only
        // answer. Two ways to fail it, and `PhraseHit::sole` has the reasoning
        // and the measurement for both:
        //   * another verse holds a run of the SAME length (they are identical
        //     verses, or share the phrase word for word), or
        //   * a LONGER run from this same window contains this one's span (the
        //     same speech, attributed to a second verse).
        let spans: Vec<(usize, usize)> = hits.iter().map(|(h, i)| (h.run, *i)).collect();
        for (h, start) in hits.iter_mut() {
            let ties = spans.iter().filter(|(n, _)| *n == h.run).count() > 1;
            let inside = spans
                .iter()
                .any(|(n, s)| *n > h.run && *s <= *start && *s + *n >= *start + h.run);
            h.sole = !ties && !inside;
        }
        // Longest run first. Ties broken by the reference so two runs of equal
        // length cannot swap places between calls — the same determinism rule
        // `cosine` learned the hard way.
        hits.sort_by(|(a, _), (b, _)| {
            b.run.cmp(&a.run).then_with(|| {
                (&a.r.book, a.r.chapter, a.r.verse).cmp(&(&b.r.book, b.r.chapter, b.r.verse))
            })
        });
        hits.truncate(k);
        hits.into_iter().map(|(h, _)| h).collect()
    }
}

#[cfg(test)]
mod phrase_tests {
    use super::*;

    fn vr(book: &str, chapter: i64, verse: i64) -> VerseRef {
        VerseRef {
            book: book.into(),
            chapter,
            verse,
        }
    }

    /// Real KJV text, braces and all — the `{is}` of Psalms 23:1 is a supplied
    /// word the translators italicised, and the preacher says it out loud.
    fn corpus() -> Vec<(VerseRef, String)> {
        vec![
            (vr("Psalms", 23, 1), "[A Psalm of David.] The LORD {is} my shepherd; I shall not want.".into()),
            (vr("Matthew", 6, 33), "But seek ye first the kingdom of God, and his righteousness; and all these things shall be added unto you.".into()),
            (vr("Proverbs", 3, 7), "Be not wise in thine own eyes: fear the LORD, and depart from evil.".into()),
            (vr("Romans", 12, 16), "Be of the same mind one toward another. Mind not high things, but condescend to men of low estate. Be not wise in your own conceits.".into()),
            (vr("1 Corinthians", 4, 2), "Moreover it is required in stewards, that a man be found faithful.".into()),
            (vr("Hebrews", 10, 25), "Not forsaking the assembling of ourselves together, as the manner of some {is}; but exhorting {one another}: and so much the more, as ye see the day approaching.".into()),
            (vr("Genesis", 1, 1), "In the beginning God created the heaven and the earth.".into()),
        ]
    }

    /// THE OPERATOR'S OWN EXAMPLE, 2026-09-20.
    #[test]
    fn the_lord_is_my_shepherd_is_found_as_one_phrase() {
        let idx = PhraseIndex::build(&corpus());
        let hits = idx.quoted(
            "and david said the lord is my shepherd i shall not want",
            None,
            3,
        );
        assert_eq!(hits.first().map(|h| h.r.clone()), Some(vr("Psalms", 23, 1)));
        // AND THE EVIDENCE IS THE WORDS, IN ORDER. This is the half the console
        // was getting wrong: `lord · shepherd` rendered inside quotation marks.
        assert_eq!(hits[0].phrase, "the lord is my shepherd i shall not want");
    }

    /// The operator's second example. Three words together, not `seek · first`.
    #[test]
    fn seek_ye_first_is_found_although_every_word_but_two_is_a_stopword() {
        let idx = PhraseIndex::build(&corpus());
        let hits = idx.quoted(
            "he told us to seek ye first the kingdom of god this morning",
            None,
            3,
        );
        assert_eq!(
            hits.first().map(|h| h.r.clone()),
            Some(vr("Matthew", 6, 33))
        );
        assert!(
            hits[0]
                .phrase
                .starts_with("seek ye first the kingdom of god"),
            "phrase was {:?}",
            hits[0].phrase
        );
    }

    /// THE DEFECT ITSELF, stated as a test. The same words in the wrong order are
    /// not a quotation, and the TF-IDF path cannot tell the difference.
    #[test]
    fn the_same_words_scattered_are_not_a_quotation() {
        let idx = PhraseIndex::build(&corpus());
        assert!(idx
            .quoted("shepherd my is lord the want not shall i", None, 3)
            .is_empty());
        assert!(idx
            .quoted("the shepherd spoke and the lord was there", None, 3)
            .is_empty());
    }

    /// RULE 40 IN THIS MODULE. "Be not wise in your own eyes" is verbatim in
    /// Romans 12:16, and the preacher reading Proverbs 3 meant Proverbs 3:7.
    /// Verified by removing the book filter and watching Romans win.
    #[test]
    fn a_book_the_words_named_beats_a_longer_run_somewhere_else() {
        let idx = PhraseIndex::build(&corpus());
        let heard = "verse seven says be not wise in your own eyes fear the lord";
        let loose = idx.quoted(heard, None, 3);
        assert_eq!(
            loose.first().map(|h| h.r.clone()),
            Some(vr("Romans", 12, 16)),
            "without the anchor the longest run is Romans — that is the trap"
        );
        let anchored = idx.quoted(heard, Some("Proverbs"), 3);
        assert_eq!(
            anchored.first().map(|h| h.r.clone()),
            Some(vr("Proverbs", 3, 7))
        );
        assert!(
            anchored.iter().all(|h| h.r.book == "Proverbs"),
            "the anchor must exclude, not merely re-rank"
        );
    }

    /// Ordinary speech does not become scripture by sharing four words with it.
    #[test]
    fn a_run_shorter_than_the_floor_is_not_offered() {
        let idx = PhraseIndex::build(&corpus());
        // "in the beginning god" is four words of Genesis 1:1 and stops there.
        assert!(idx
            .quoted(
                "right in the beginning god was moving in this church",
                None,
                3
            )
            .is_empty());
    }

    /// Longest run first, and the ranking is stable.
    #[test]
    fn the_longest_run_is_offered_first_and_the_order_never_moves() {
        let idx = PhraseIndex::build(&corpus());
        let heard = "moreover it is required in stewards that a man be found faithful \
                     and not forsaking the assembling of ourselves together";
        let first = idx.quoted(heard, None, 5);
        assert!(first.len() >= 2, "expected both verses, got {first:?}");
        assert!(first[0].run >= first[1].run);
        for _ in 0..5 {
            assert_eq!(idx.quoted(heard, None, 5), first);
        }
    }

    /// Silence, and a window with no verse in it at all.
    #[test]
    fn nothing_heard_offers_nothing() {
        let idx = PhraseIndex::build(&corpus());
        assert!(idx.quoted("", None, 3).is_empty());
        assert!(idx
            .quoted(
                "good morning everybody welcome to the service today",
                None,
                3
            )
            .is_empty());
    }
}

#[cfg(test)]
mod phrase_bench {
    use super::*;

    /// WHAT THE SHIPPED CODE FINDS IN A REAL SERVICE.
    ///
    /// The constants in this module were tuned against a model of this algorithm
    /// written in another language, which is a measurement of the model and not
    /// of the product. This runs the REAL index over a real transcript and prints
    /// what an operator would have been offered.
    ///
    ///     RELAY_PHRASE_WAV=/path/to/transcript.txt \
    ///       cargo test phrase_bench -- --ignored --nocapture
    ///
    /// One line per transcript window. Needs the bundled KJV, so it builds the
    /// whole 31,102-verse index; that is the point, because `MAX_GRAM_VERSES` and
    /// `PHRASE_RARE_FRACTION` mean nothing on a seven-verse fixture.
    /// **READ THE BIBLE BACK AND COUNT THE WRONG VERSES — the measurement that
    /// says whether a quoted verse may reach a wall.**
    ///
    /// `READING_RUN_WORDS`' own doc records this simulation and its result (42
    /// wrong verses at a run of eight, every one an exact tie, all 42 removed by
    /// the sole rule). It records them in PROSE. This repository's rule is that a
    /// contract stated in a comment is not a contract, and that gap cost it the
    /// `stopCapture` throw-vs-swallow bug for as long as the comment existed.
    ///
    /// **The CI scorecard cannot see this change at all** — measured, not
    /// assumed: `eval::print_scorecard` reports 0.0% with the promotion on and
    /// 0.0% with `for_quotation` forced back to `Quoted`, because its 82 cases
    /// are spoken references and not a preacher reading a verse aloud. An
    /// instrument that returns the same number either way is not evidence about
    /// this gate, and quoting it as though it were would be the worst kind of
    /// reassurance. So the number lives here, beside the rule it is about.
    ///
    /// `cargo test read_the_bible_back -- --ignored --nocapture`. Ignored because
    /// it builds a phrase index over 31,102 verses and reads a sample of them
    /// back; it is a benchmark, not a gate.
    #[test]
    #[ignore]
    fn read_the_bible_back_and_count_wrong_verses() {
        let kjv: serde_json::Value =
            serde_json::from_str(include_str!("../data/kjv.json").trim_start_matches('\u{feff}'))
                .expect("kjv");
        let mut corpus: Vec<(VerseRef, String)> = Vec::new();
        for book in kjv.as_array().expect("books") {
            let abbrev = book["abbrev"].as_str().unwrap_or("?").to_string();
            for (ci, chapter) in book["chapters"]
                .as_array()
                .expect("chapters")
                .iter()
                .enumerate()
            {
                for (vi, verse) in chapter.as_array().expect("verses").iter().enumerate() {
                    corpus.push((
                        VerseRef {
                            book: abbrev.clone(),
                            chapter: ci as i64 + 1,
                            verse: vi as i64 + 1,
                        },
                        verse.as_str().unwrap_or("").to_string(),
                    ));
                }
            }
        }
        let idx = PhraseIndex::build(&corpus);
        println!("index over {} verses", corpus.len());

        // A DETERMINISTIC SAMPLE, so the figure is the same on every machine and
        // in every run. A random one would make a regression look like luck.
        let step = corpus.len() / 1500;
        let sample: Vec<&(VerseRef, String)> = corpus.iter().step_by(step.max(1)).collect();

        // NO BOOK NAMED, which is the case this rule is about: the preacher is
        // READING, not announcing. A book in the window settles a tie by itself
        // (rule 40), so including one would measure the easy half.
        let mut would_fire_wrong = 0usize;
        let mut would_fire_right = 0usize;
        let mut ties_refused = 0usize;
        for (r, text) in &sample {
            if text.split_whitespace().count() < READING_RUN_WORDS {
                continue;
            }
            for h in idx.quoted(text, None, 3) {
                let method = DetectionMethod::for_quotation(h.run, h.sole);
                if method != DetectionMethod::Reading {
                    if h.run >= READING_RUN_WORDS && &h.r != r {
                        ties_refused += 1;
                    }
                    continue;
                }
                if &h.r == r {
                    would_fire_right += 1;
                } else {
                    would_fire_wrong += 1;
                    println!(
                        "  WRONG: read {} {}:{} → would fire {} {}:{} (run {})",
                        r.book, r.chapter, r.verse, h.r.book, h.r.chapter, h.r.verse, h.run
                    );
                }
            }
        }
        let total = would_fire_right + would_fire_wrong;
        let rate = if total == 0 {
            0.0
        } else {
            would_fire_wrong as f64 * 100.0 / total as f64
        };
        println!(
            "read back {} verses · {would_fire_right} correct · {would_fire_wrong} wrong \
             · {rate:.1}% wrong-verse rate (SPEC target: <5%) · {ties_refused} shared runs \
             refused a wall by the sole rule",
            sample.len()
        );
        // The claim `READING_RUN_WORDS` makes, asserted rather than described.
        assert_eq!(
            would_fire_wrong, 0,
            "a verse read aloud would have put a DIFFERENT verse on a wall"
        );
        assert!(
            would_fire_right > 100,
            "the simulation fired almost nothing, so its zero means nothing"
        );
    }

    #[test]
    #[ignore]
    fn what_a_real_service_would_have_been_offered() {
        let path =
            std::env::var("RELAY_PHRASE_WAV").unwrap_or_else(|_| "/tmp/service24.txt".to_string());
        let Ok(text) = std::fs::read_to_string(&path) else {
            println!("no transcript at {path}; set RELAY_PHRASE_WAV");
            return;
        };
        let kjv: serde_json::Value =
            // `trim_start_matches` the BOM: the bundled KJV carries one, and
            // serde_json rejects it as "expected value" at line 1 column 1.
            serde_json::from_str(include_str!("../data/kjv.json").trim_start_matches('\u{feff}'))
                .expect("kjv");
        let mut corpus: Vec<(VerseRef, String)> = Vec::new();
        for book in kjv.as_array().expect("books") {
            let abbrev = book["abbrev"].as_str().unwrap_or("?").to_string();
            for (ci, chapter) in book["chapters"]
                .as_array()
                .expect("chapters")
                .iter()
                .enumerate()
            {
                for (vi, verse) in chapter.as_array().expect("verses").iter().enumerate() {
                    corpus.push((
                        VerseRef {
                            book: abbrev.clone(),
                            chapter: ci as i64 + 1,
                            verse: vi as i64 + 1,
                        },
                        verse.as_str().unwrap_or("").to_string(),
                    ));
                }
            }
        }
        println!("index over {} verses", corpus.len());
        let built = std::time::Instant::now();
        let idx = PhraseIndex::build(&corpus);
        println!("built in {:?}", built.elapsed());

        let mut fires = 0usize;
        let t0 = std::time::Instant::now();
        let mut windows = 0usize;
        for line in text.lines() {
            windows += 1;
            for h in idx.quoted(line, None, 3) {
                fires += 1;
                println!(
                    "  [{}] {} {}:{}  “{}”",
                    h.run, h.r.book, h.r.chapter, h.r.verse, h.phrase
                );
            }
        }
        println!(
            "{fires} offers over {windows} windows · {:?} total · {:?}/window",
            t0.elapsed(),
            t0.elapsed() / windows.max(1) as u32
        );
    }
}

/// FIELD 2026-09-20, service 24, 23.5 minutes in. The bare-verse path's second
/// wrong verse on a real wall, and the one rule 40 said one service could not
/// justify closing.
///
/// Psalms 55 had been on the wall since 19.4 minutes. The preacher then said
/// *"Out of a prophet called Osir. In verse 1 he says, Come and let us return unto
/// the Lord."* — Hosea 6:1, with the book misheard as a word no alias knows. Nothing
/// parsed, no chapter was stated, so memory answered: **Psalms 55:1 at 0.88, labelled
/// `Direct`, auto-fired.** The label was the lie: Relay did not hear "Psalms", it
/// assumed it. Rule 10 says `Direct` means HEARD, and `UncertainBook` already exists
/// for "chapter and verse heard, the book not" — which is exactly what a bare verse
/// answered from memory is. It is capped at Suggest by the router at any score, so
/// the operator gets the offer and the click, and the wall gets nothing unattended.
#[cfg(test)]
mod field_2026_09_20 {
    use super::*;

    const HEARD: &str = "Out of a prophet called Osir. In verse 1 he says, Come and let us \
                         return unto the Lord.";

    fn on_the_wall() -> VerseRef {
        VerseRef {
            book: "Psalms".into(),
            chapter: 55,
            verse: 22,
        }
    }

    #[test]
    fn a_verse_answered_from_memory_says_so() {
        assert!(
            anchor_for_bare_verses(HEARD).is_none(),
            "precondition: 'Osir' parses to nothing"
        );
        assert!(!chapter_named(HEARD), "precondition: no chapter is stated");
        assert!(
            detect_bare_verses(HEARD).contains(&1),
            "precondition: 'verse 1' is seen"
        );
        // `emit_detections` hands this function the passage on the wall with the
        // bare number already substituted (`ContextMemory::resolve_bare_verse`), so
        // "memory" here is Psalms 55 *verse 1*, not the verse that is up.
        let memory = VerseRef {
            verse: 1,
            ..on_the_wall()
        };
        let got = resolve_bare_verse_with_source(HEARD, 1, None, Some(&memory));
        assert_eq!(
            got,
            Some((
                VerseRef {
                    book: "Psalms".into(),
                    chapter: 55,
                    verse: 1,
                },
                BareVerseSource::Memory
            )),
            "memory still answers, and it must say that it did"
        );
    }

    #[test]
    fn a_verse_hung_on_a_book_named_in_this_breath_is_heard() {
        let heard = "going through in Luke 10. If you read from verse 32";
        let anchor = anchor_for_bare_verses(heard).expect("Luke 10 parses");
        let got = resolve_bare_verse_with_source(heard, 32, Some(&anchor), Some(&on_the_wall()));
        assert_eq!(got.map(|(_, s)| s), Some(BareVerseSource::Anchor));
    }

    /// The label follows the source. Memory is not hearing.
    #[test]
    fn memory_is_not_a_heard_book_and_an_anchor_is() {
        assert_eq!(
            DetectionMethod::for_bare_verse(BareVerseSource::Memory),
            DetectionMethod::UncertainBook
        );
        assert_eq!(
            DetectionMethod::for_bare_verse(BareVerseSource::Anchor),
            DetectionMethod::Direct
        );
        assert!(
            !DetectionMethod::for_bare_verse(BareVerseSource::Memory).may_auto_fire(),
            "a verse whose book nobody said may never reach a wall unattended"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// FOLLOWING THE READER · WHEN A QUOTATION IS THE PREACHER READING
// ═══════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod reading_tests {
    use super::*;

    fn vr(book: &str, chapter: i64, verse: i64) -> VerseRef {
        VerseRef {
            book: book.into(),
            chapter,
            verse,
        }
    }

    /// Two verses that share a long phrase verbatim, and two that do not.
    /// Judges 1:12 and Joshua 15:16 are a real KJV pair — the measurement below
    /// found that EVERY wrong verse at a run of eight words or more is a pair
    /// like this one, never a longer run somewhere else.
    fn corpus() -> Vec<(VerseRef, String)> {
        vec![
            (vr("Romans", 8, 28), "And we know that all things work together for good to them that love God, to them who are the called according to his purpose.".into()),
            (vr("Judges", 1, 12), "And Caleb said, He that smiteth Kirjathsepher, and taketh it, to him will I give Achsah my daughter to wife.".into()),
            (vr("Joshua", 15, 16), "And Caleb said, He that smiteth Kirjathsepher, and taketh it, to him will I give Achsah my daughter to wife.".into()),
            (vr("Psalms", 23, 1), "The LORD is my shepherd; I shall not want.".into()),
        ]
    }

    /// A run held by ONE verse is sole. A run two verses share is not, and that
    /// is the whole of the second half of the rule.
    #[test]
    fn a_run_two_verses_share_is_not_sole() {
        let idx = PhraseIndex::build(&corpus());
        let hits = idx.quoted(
            "and Caleb said he that smiteth Kirjathsepher and taketh it to him will I give",
            None,
            5,
        );
        assert!(hits.len() >= 2, "expected the shared pair, got {hits:?}");
        assert!(
            hits.iter().all(|h| !h.sole),
            "a phrase two verses hold verbatim reported itself as sole: {hits:?}"
        );

        let alone = idx.quoted(
            "all things work together for good to them that love God",
            None,
            5,
        );
        assert_eq!(alone.len(), 1);
        assert!(alone[0].sole, "a run only one verse holds is not sole");
    }

    /// A NAMED BOOK SETTLES A TIE, because the words said which. Rule 40 in this
    /// module, and it is why the flag is computed after the book filter rather
    /// than over the whole Bible.
    #[test]
    fn a_book_named_in_the_window_makes_a_shared_run_sole_again() {
        let idx = PhraseIndex::build(&corpus());
        let hits = idx.quoted(
            "and Caleb said he that smiteth Kirjathsepher and taketh it to him will I give",
            Some("Joshua"),
            5,
        );
        assert_eq!(hits.len(), 1);
        assert!(hits[0].sole);
    }

    /// THE PROMOTION, AS A PURE FUNCTION. The evidence decides, and nothing else
    /// does: no score, no dial, no threshold. A demotion expressed as a number is
    /// one a dial can erase (see `UncertainNumber`), and a promotion expressed as
    /// a number would be erasable in exactly the same way.
    #[test]
    fn only_a_long_sole_run_is_a_reading() {
        // Long enough and held by one verse → Relay heard the verse being read.
        assert_eq!(
            DetectionMethod::for_quotation(READING_RUN_WORDS, true),
            DetectionMethod::Reading
        );
        assert_eq!(
            DetectionMethod::for_quotation(30, true),
            DetectionMethod::Reading
        );
        // One word short of the floor → still a quotation, still capped.
        assert_eq!(
            DetectionMethod::for_quotation(READING_RUN_WORDS - 1, true),
            DetectionMethod::Quoted
        );
        // Long, but two verses hold the same words → Relay would be choosing
        // between them, which is a guess about which and not a hearing.
        assert_eq!(
            DetectionMethod::for_quotation(30, false),
            DetectionMethod::Quoted
        );
        // And the floor is above the length at which a run stands on its own as a
        // SUGGESTION, deliberately. Reaching an operator and reaching a
        // congregation are not the same bar.
        //
        // AS A COMPILE-TIME CHECK, not a runtime one. These are all `const`, so a
        // runtime `assert!` over them is a test that cannot fail at test time —
        // it fails at BUILD time or never, and clippy says so. Written this way
        // the relationship is enforced for every build, including one where
        // nobody runs the tests.
        const _: () = assert!(READING_RUN_WORDS > SELF_EVIDENT_RUN);
        const _: () = assert!(READING_RUN_WORDS > MIN_RUN_WORDS);
    }

    /// A reading is the only thing this widens. Every other cap stands.
    #[test]
    fn nothing_else_gained_the_right_to_fire() {
        for m in [
            DetectionMethod::Semantic,
            DetectionMethod::Quoted,
            DetectionMethod::Ambiguous,
            DetectionMethod::UncertainBook,
            DetectionMethod::UncertainNumber,
        ] {
            assert!(!m.may_auto_fire(), "{m:?} may now auto-fire");
        }
        assert!(DetectionMethod::Direct.may_auto_fire());
        assert!(DetectionMethod::Reading.may_auto_fire());
    }

    /// ── WHOSE NUMBER MAY TEACH THE GATE ────────────────────────────────────
    ///
    /// `may_auto_fire` used to answer three different questions at once, and
    /// letting a reading through it would have answered the wrong one. A
    /// reading's "confidence" is derived from a WORD COUNT; the auto-fire bar is
    /// a parse probability. Feeding one to the other is the category error rule
    /// 10 is about, arriving from the opposite direction.
    #[test]
    fn only_a_parse_confidence_may_teach_the_gate() {
        assert!(DetectionMethod::Direct.confidence_is_calibrated());
        assert!(!DetectionMethod::Reading.confidence_is_calibrated());
        assert!(!DetectionMethod::Semantic.confidence_is_calibrated());
        assert!(!DetectionMethod::Quoted.confidence_is_calibrated());
    }

    /// A HEARD REFERENCE OUTRANKS A READING, and both outrank the rest. One
    /// window may put at most one verse on a wall (rule 29), so this decides
    /// what a congregation sees when a preacher names one verse and reads
    /// another in the same breath. The words naming a reference are the words
    /// saying, which is rule 40's own principle.
    #[test]
    fn a_named_reference_outranks_a_reading() {
        assert!(
            DetectionMethod::Direct.unattended_rank() > DetectionMethod::Reading.unattended_rank()
        );
        assert!(
            DetectionMethod::Reading.unattended_rank()
                > DetectionMethod::Semantic.unattended_rank()
        );
        assert_eq!(
            DetectionMethod::Quoted.unattended_rank(),
            DetectionMethod::Semantic.unattended_rank()
        );
    }

    /// ONE SPAN OF SPEECH IS ONE HEARING, however many verses hold part of it.
    ///
    /// Found by the scorecard on the day the promotion landed, which is the point
    /// of having one. A preacher reading John 3:16 aloud makes a 25-word run in
    /// John 3:16 and a 10-word run in John 3:15 — those ten words are in both
    /// verses, and they sit wholly inside the twenty-five. Both were sole at
    /// their own lengths, so both were readings, so both fired. Rule 29's own
    /// finding in a new shape.
    #[test]
    fn a_shorter_run_inside_a_longer_one_is_not_a_second_reading() {
        let idx = PhraseIndex::build(&[
            (vr("John", 3, 15), "That whosoever believeth in him should not perish, but have eternal life.".into()),
            (vr("John", 3, 16), "For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish, but have everlasting life.".into()),
        ]);
        let hits = idx.quoted(
            "For God so loved the world that he gave his only begotten Son that whosoever believeth in him should not perish but have everlasting life",
            None,
            5,
        );
        let sixteen = hits.iter().find(|h| h.r.verse == 16).expect("John 3:16");
        assert!(sixteen.sole, "the verse actually read was not sole");
        assert_eq!(
            DetectionMethod::for_quotation(sixteen.run, sixteen.sole),
            DetectionMethod::Reading
        );
        if let Some(fifteen) = hits.iter().find(|h| h.r.verse == 15) {
            assert!(
                !fifteen.sole,
                "the same span of speech was counted as a second hearing, in John 3:15"
            );
            // Still OFFERED. A silent discard is a different lie, and the operator
            // may genuinely want the neighbouring verse.
            assert_eq!(
                DetectionMethod::for_quotation(fifteen.run, fifteen.sole),
                DetectionMethod::Quoted
            );
        }
    }

    #[test]
    fn a_reading_survives_the_round_trip_the_console_sends_back() {
        assert_eq!(
            DetectionMethod::from_wire("reading"),
            DetectionMethod::Reading
        );
        assert_eq!(
            serde_json::to_string(&DetectionMethod::Reading).unwrap(),
            "\"reading\""
        );
    }
}

/// **THE PASSAGE GUARD** — `hold_for_the_passage`. The decision and the register
/// row are dated 2026-09-25; they carry no section number here on purpose, because
/// the two this was drafted against were taken by other work the same day and a
/// citation that resolves to the WRONG place is worse than one that resolves to
/// nothing.
///
/// Every case here is either the operator's instruction of 2026-09-25 or a row out
/// of their own database. The three the design deliberately does NOT catch are
/// tested too, as negatives, because a guard nobody can state the limits of is one
/// that will be widened by the next person who reads it.
#[cfg(test)]
mod passage_guard {
    use super::*;

    fn vr(book: &str, chapter: i64, verse: i64) -> VerseRef {
        VerseRef {
            book: book.into(),
            chapter,
            verse,
        }
    }

    /// Shorthand: `hold(on_screen, on_the_wall, named_a_reference, candidates)`.
    fn hold(
        on_screen: Option<&VerseRef>,
        wall: Option<&str>,
        named: bool,
        cands: &[(&VerseRef, DetectionMethod)],
    ) -> Vec<Option<HeldReason>> {
        hold_for_the_passage(on_screen, wall, named, cands)
    }

    // ── RULE B · THE WALL ALREADY SAYS IT ────────────────────────────────────

    /// **FIELD, service 40, 2026-09-25, 769 s → 780 s.** The preacher said
    /// *"Jeremiah chapter 6 verse 16"*, Relay fired it `Direct` at 0.95, and eleven
    /// seconds later the preacher read the verse aloud — *"And see. And ask. For the
    /// old paths. Where is the good way?"* — and Relay put the SAME verse on the
    /// wall a second time. The same morning did it again with `Hebrews 6:12` at
    /// 897 s → 908 s and `Romans 15:4` at 1034 s → 1051 s.
    ///
    /// Announce the reference, then read it, is the ordinary shape of a sermon.
    #[test]
    fn field_2026_09_25_a_verse_read_while_it_is_on_the_wall_is_held() {
        let jer = vr("Jeremiah", 6, 16);
        let out = hold(
            Some(&jer),
            Some("Jeremiah 6:16"),
            false,
            &[(&jer, DetectionMethod::Reading)],
        );
        assert_eq!(out, vec![Some(HeldReason::AlreadyOnScreen)]);
    }

    /// The same shape with nothing but a reading on either side — **service 40,
    /// 615 s → 626 s, `Mark 6:2` twice**, eleven seconds apart, the second at a
    /// HIGHER confidence (0.69 → 0.90) because more of the verse had been heard. So
    /// the duplicate is the stronger candidate by every rule in `pipeline::better`,
    /// and ranking could never have chosen against it.
    #[test]
    fn field_2026_09_25_mark_6_2_twice_from_one_reading() {
        let mark = vr("Mark", 6, 2);
        assert_eq!(
            hold(
                Some(&mark),
                Some("Mark 6:2"),
                false,
                &[(&mark, DetectionMethod::Reading)]
            ),
            vec![Some(HeldReason::AlreadyOnScreen)]
        );
    }

    /// Rule B covers the short quotation and the paraphrase as well, and the reason
    /// is one sentence: a candidate found in the verse's own text cannot tell the
    /// operator anything about the verse they are already looking at.
    #[test]
    fn every_kind_of_text_match_is_held_by_the_wall_but_no_reference_is() {
        let r = vr("Psalms", 23, 4);
        for m in [
            DetectionMethod::Reading,
            DetectionMethod::Quoted,
            DetectionMethod::Semantic,
        ] {
            assert_eq!(
                hold(Some(&r), Some("Psalms 23:4"), false, &[(&r, m)]),
                vec![Some(HeldReason::AlreadyOnScreen)],
                "{m:?} of the verse on the wall should be held"
            );
        }
        // …and NOTHING reference-shaped is, at any time, under any rule. This is
        // constraint 1 and rule 10: only what Relay HEARD may reach a congregation,
        // and only what Relay heard may reach the OPERATOR unguarded either.
        for m in [
            DetectionMethod::Direct,
            DetectionMethod::Ambiguous,
            DetectionMethod::UncertainBook,
            DetectionMethod::UncertainNumber,
        ] {
            assert_eq!(
                hold(Some(&r), Some("Psalms 23:4"), false, &[(&r, m)]),
                vec![None],
                "{m:?} was parsed from the words and may never be held"
            );
        }
    }

    /// Nothing on the wall → rule B has nothing to compare with. This is the state
    /// after a clear or a blackout (`Router::forget_last_fire`) and after a song
    /// (`Router::forget_wall`), and it is what makes a re-reading of a verse the
    /// operator cleared come straight back.
    #[test]
    fn a_cleared_wall_holds_nothing() {
        let r = vr("Psalms", 23, 4);
        assert_eq!(
            hold(Some(&r), None, false, &[(&r, DetectionMethod::Reading)]),
            vec![None]
        );
    }

    // ── RULE A · OUTSIDE THE READING ─────────────────────────────────────────

    /// The operator's own complaint. `Psalms 107:8` is being read; the same words are
    /// verbatim in `Psalms 107:21`, and a run also lands in `Ephesians 5:20`, which is
    /// a different passage entirely. The one inside the chapter survives; the one
    /// outside it is held.
    #[test]
    fn while_a_passage_is_being_read_a_verse_from_elsewhere_is_held() {
        let on = vr("Psalms", 107, 8);
        let same_chapter = vr("Psalms", 107, 21);
        let elsewhere = vr("Ephesians", 5, 20);
        assert_eq!(
            hold(
                Some(&on),
                Some("Psalms 107:8"),
                false,
                &[
                    (&on, DetectionMethod::Reading),
                    (&same_chapter, DetectionMethod::Quoted),
                    (&elsewhere, DetectionMethod::Quoted),
                ]
            ),
            vec![
                Some(HeldReason::AlreadyOnScreen),
                None,
                Some(HeldReason::OutsideTheReading),
            ]
        );
    }

    /// **`PhraseHit::sole`'s own example, and it is a within-chapter one.** A
    /// preacher reading John 3:16 aloud produces a 25-word run in 3:16 and a 10-word
    /// run in 3:15, because those ten words are in both. 3:15 stays offered: it is a
    /// perfectly reasonable thing for the operator to want, it is inside the chapter,
    /// and the operator asked for suggestions to stay *within the verse/chapter*.
    #[test]
    fn a_sub_span_inside_the_same_chapter_is_still_offered() {
        let on = vr("John", 3, 16);
        let sub = vr("John", 3, 15);
        assert_eq!(
            hold(
                Some(&on),
                Some("John 3:16"),
                false,
                &[
                    (&on, DetectionMethod::Reading),
                    (&sub, DetectionMethod::Quoted),
                ]
            ),
            vec![Some(HeldReason::AlreadyOnScreen), None]
        );
    }

    /// **CONSTRAINT 1, AND IT IS THE WHOLE OF RULE 40.** "Turn to Romans eight"
    /// while Psalms 107 is being read gets through in the same instant it always
    /// did — and so does everything else in that window, because a window that names
    /// a reference disarms rule A entirely rather than exempting one candidate.
    #[test]
    fn a_window_that_names_a_reference_holds_nothing_at_all() {
        let on = vr("Psalms", 107, 8);
        let spoken = vr("Romans", 8, 28);
        let elsewhere = vr("Ephesians", 5, 20);
        assert_eq!(
            hold(
                Some(&on),
                Some("Psalms 107:8"),
                // the words said
                true,
                &[
                    (&on, DetectionMethod::Reading),
                    (&spoken, DetectionMethod::Direct),
                    (&elsewhere, DetectionMethod::Quoted),
                ]
            ),
            // Rule B still holds the verse that is literally on the wall — that is
            // not about the passage and a named reference does not make a re-showing
            // of the current verse informative. Rule A holds nothing.
            vec![Some(HeldReason::AlreadyOnScreen), None, None]
        );
    }

    /// **WHAT ENDS IT, MEASURED BY ITS ABSENCE.** The preacher has stopped reading
    /// Psalms 107 and is now quoting Isaiah without naming it. There is no
    /// in-passage run in this window, so the guard does not arm and Isaiah is
    /// offered — the first window, not the second, and with nothing to clear.
    ///
    /// This is why the guard is not `quoted(text, Some(on_screen_book), k)`. That
    /// restriction would hide Isaiah until something else moved `ContextMemory`,
    /// which needs an operator.
    #[test]
    fn a_preacher_who_has_moved_on_is_never_held_even_once() {
        let on = vr("Psalms", 107, 8);
        let isaiah = vr("Isaiah", 58, 6);
        assert_eq!(
            hold(
                Some(&on),
                Some("Psalms 107:8"),
                false,
                &[(&isaiah, DetectionMethod::Reading)]
            ),
            vec![None]
        );
    }

    /// A paraphrase inside the passage is NOT evidence that the preacher is reading
    /// it. A cosine is a bag of words in no order (rule 18); the assertion "this
    /// chapter is being read aloud" needs words in the speaker's own order, which is
    /// what a verbatim run is and the only reason `Reading` was allowed to exist.
    #[test]
    fn only_a_verbatim_run_may_arm_the_guard() {
        let on = vr("Psalms", 107, 8);
        let same_chapter = vr("Psalms", 107, 21);
        let elsewhere = vr("Ephesians", 5, 20);
        assert_eq!(
            hold(
                Some(&on),
                None,
                false,
                &[
                    (&same_chapter, DetectionMethod::Semantic),
                    (&elsewhere, DetectionMethod::Quoted),
                ]
            ),
            vec![None, None],
            "a paraphrase in the passage must not arm the guard"
        );
        // Swap the in-passage evidence for a real run and the same window holds.
        assert_eq!(
            hold(
                Some(&on),
                None,
                false,
                &[
                    (&same_chapter, DetectionMethod::Quoted),
                    (&elsewhere, DetectionMethod::Quoted),
                ]
            ),
            vec![None, Some(HeldReason::OutsideTheReading)],
        );
    }

    /// Nothing on the screen at all — the first reading of a service — holds
    /// nothing. A guard that bit before anything had ever been fired would make the
    /// first verse of every service harder to reach than the rest.
    #[test]
    fn nothing_on_screen_holds_nothing() {
        let a = vr("Psalms", 107, 8);
        let b = vr("Ephesians", 5, 20);
        assert_eq!(
            hold(
                None,
                None,
                false,
                &[
                    (&a, DetectionMethod::Reading),
                    (&b, DetectionMethod::Quoted)
                ]
            ),
            vec![None, None]
        );
    }

    /// Chapter, not book. The operator said *verse/chapter*, and Psalms 23 is a
    /// different passage from Psalms 107 however much the book name agrees.
    #[test]
    fn the_scope_is_the_chapter_and_not_the_book() {
        let on = vr("Psalms", 107, 8);
        let other_chapter = vr("Psalms", 23, 1);
        assert_eq!(
            hold(
                Some(&on),
                None,
                false,
                &[
                    (&on, DetectionMethod::Reading),
                    (&other_chapter, DetectionMethod::Quoted),
                ]
            ),
            vec![None, Some(HeldReason::OutsideTheReading)]
        );
    }

    // ── THE THREE THINGS IT DELIBERATELY DOES NOT CATCH ─────────────────────

    /// **FIELD, service 39, 1508 s → 1517 s: `Philippians 1:23` then `1:24`.** One
    /// reading walking forward. Rule B holds the first (it is on the wall) and the
    /// second is NOT held, so the wall follows the reader.
    ///
    /// Freezing on verse 23 until somebody named another reference would leave a
    /// verse the preacher had finished in front of a congregation for as long as the
    /// reading lasted — a wrong verse chosen on purpose — and it would reverse the
    /// instruction of 2026-09-23 that created `Reading`. **If this assertion is ever
    /// inverted, it is a product decision and not a bug fix.**
    #[test]
    fn field_2026_09_24_the_reader_walking_to_the_next_verse_is_followed() {
        let on = vr("Philippians", 1, 23);
        let next = vr("Philippians", 1, 24);
        assert_eq!(
            hold(
                Some(&on),
                Some("Philippians 1:23"),
                false,
                &[
                    (&on, DetectionMethod::Reading),
                    (&next, DetectionMethod::Reading),
                ]
            ),
            vec![Some(HeldReason::AlreadyOnScreen), None],
        );
    }

    /// **FIELD, service 39, 955 s → 968 s: `Micah 4:1` as a reading, then the SAME
    /// verse thirteen seconds later as a spoken reference.** A duplicate fire, and
    /// this guard will not touch it: the second was heard as a reference, and
    /// constraint 1 says a spoken reference always gets through. Fixing it means
    /// changing `DEFAULT_DEBOUNCE_MS`, which governs every `Direct` in the product.
    #[test]
    fn a_spoken_reference_repeating_a_reading_is_not_this_guards_business() {
        let micah = vr("Micah", 4, 1);
        assert_eq!(
            hold(
                Some(&micah),
                Some("Micah 4:1"),
                true,
                &[(&micah, DetectionMethod::Direct)]
            ),
            vec![None]
        );
    }

    /// **THE INVARIANT THAT MAKES THE ANNOUNCEMENT REACHABLE.** Rule A arms only on
    /// an in-passage verbatim run, and rule A never holds an in-passage candidate —
    /// so whenever rule A holds anything, something survives to carry the report.
    ///
    /// Rule B can hold a window's every candidate, and that is correct and needs no
    /// escape: the one thing it holds is the verse the screens are already showing,
    /// so the evidence the operator needs is the wall.
    #[test]
    fn rule_a_can_never_hold_a_whole_window() {
        let on = vr("Psalms", 107, 8);
        let a = vr("Ephesians", 5, 20);
        let b = vr("Romans", 12, 2);
        let out = hold(
            Some(&on),
            None,
            false,
            &[
                (&on, DetectionMethod::Reading),
                (&a, DetectionMethod::Quoted),
                (&b, DetectionMethod::Semantic),
            ],
        );
        assert!(
            out.iter().any(Option::is_none),
            "rule A held every candidate, so nothing could announce it"
        );
        assert_eq!(
            out.iter()
                .filter(|h| **h == Some(HeldReason::OutsideTheReading))
                .count(),
            2
        );
    }

    /// The order of the answers is the order of the candidates, because the caller
    /// zips them. An off-by-one here would hold the wrong verse silently.
    #[test]
    fn the_answers_line_up_with_the_candidates() {
        let on = vr("Psalms", 107, 8);
        let out_of = vr("Ephesians", 5, 20);
        let cands = [
            (&out_of, DetectionMethod::Quoted),
            (&on, DetectionMethod::Reading),
        ];
        let out = hold(Some(&on), None, false, &cands);
        assert_eq!(out.len(), cands.len());
        assert_eq!(out[0], Some(HeldReason::OutsideTheReading));
        assert_eq!(out[1], None);
    }

    /// `window_states_a_reference` is rule 40's pair, and the second half is the one
    /// FIELD F-8 was about: a window can STATE a chapter with nothing parsing out of
    /// it, and a preacher who has just announced a chapter is no longer reading the
    /// last one.
    #[test]
    fn a_stated_chapter_disarms_the_guard_even_when_nothing_parsed() {
        assert!(!window_states_a_reference("and he said unto them", None));
        assert!(window_states_a_reference(
            "is taken from 4th peter chapter 5 verse 10",
            None
        ));
        let anchor = vr("Romans", 8, 28);
        assert!(window_states_a_reference("romans 8 28", Some(&anchor)));
        // Swahili, for the same reason `chapter_named` is not an English literal.
        assert!(window_states_a_reference("yohana sura ya tatu", None));
    }
}
