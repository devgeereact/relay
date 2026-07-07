//! Song lyric parsing.
//!
//! Single responsibility: turn a block of pasted/imported lyrics into ordered
//! sections (Verse 1, Chorus, Bridge…). Pure and DB/IO-free so it is heavily
//! unit-tested, exactly like `detection.rs`. Persistence lives in `db.rs`; the
//! import command in `main.rs` wires this to the DB.
//!
//! Two input shapes are handled: labelled (headers like `[Verse 1]`, `Chorus:`,
//! `V1`, `Pre-Chorus 2`) and unlabelled (blank-line-separated blocks become
//! `Part 1`, `Part 2`, …). Code stays offline and dependency-free (CLAUDE.md).

use serde::{Deserialize, Serialize};

/// One parsed section: a short `tag` (V1 / C1 / BR…), a human `label`
/// (Verse 1 / Chorus / Bridge), and the section's lyric lines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedSection {
    pub tag: String,
    pub label: String,
    pub lyrics: String,
}

/// Parse a lyric blob into ordered sections. Never panics; returns an empty vec
/// for empty input.
pub fn parse_song(text: &str) -> Vec<ParsedSection> {
    let mut sections: Vec<ParsedSection> = Vec::new();
    let mut cur: Option<(String, String)> = None; // (tag, label) of the open section
    let mut buf: Vec<String> = Vec::new();
    let mut saw_header = false;

    for line in text.lines() {
        if let Some((tag, label)) = parse_header(line) {
            saw_header = true;
            flush(&mut cur, &mut buf, &mut sections);
            cur = Some((tag, label));
        } else {
            if cur.is_none() && !line.trim().is_empty() {
                // Content before the first header → an implicit opening section.
                cur = Some(("S".into(), "Section".into()));
            }
            if cur.is_some() {
                buf.push(line.to_string());
            }
        }
    }
    flush(&mut cur, &mut buf, &mut sections);

    if !saw_header {
        return split_blocks(text);
    }
    sections
}

/// Close the open section, trimming blank edges, and push it if non-empty.
fn flush(cur: &mut Option<(String, String)>, buf: &mut Vec<String>, out: &mut Vec<ParsedSection>) {
    if let Some((tag, label)) = cur.take() {
        let lyrics = trim_blank_edges(buf);
        if !lyrics.trim().is_empty() {
            out.push(ParsedSection { tag, label, lyrics });
        }
    }
    buf.clear();
}

/// Join buffered lines, dropping leading/trailing blank lines.
fn trim_blank_edges(buf: &[String]) -> String {
    let start = buf.iter().position(|l| !l.trim().is_empty());
    let end = buf.iter().rposition(|l| !l.trim().is_empty());
    match (start, end) {
        (Some(s), Some(e)) => buf[s..=e].join("\n"),
        _ => String::new(),
    }
}

/// Fallback for header-less lyrics: blank-line-separated blocks → Part N.
fn split_blocks(text: &str) -> Vec<ParsedSection> {
    let mut out: Vec<ParsedSection> = Vec::new();
    let mut buf: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            push_block(&mut buf, &mut out);
        } else {
            buf.push(line.to_string());
        }
    }
    push_block(&mut buf, &mut out);
    if out.len() == 1 {
        out[0].tag = "S".into();
        out[0].label = "Song".into();
    }
    out
}

fn push_block(buf: &mut Vec<String>, out: &mut Vec<ParsedSection>) {
    let lyrics = trim_blank_edges(buf);
    buf.clear();
    if !lyrics.trim().is_empty() {
        let n = out.len() + 1;
        out.push(ParsedSection {
            tag: format!("P{n}"),
            label: format!("Part {n}"),
            lyrics,
        });
    }
}

/// Recognize a section header line. Returns (tag, label) or None. Deliberately
/// strict — a header must be short (≤3 words) and lead with a known section
/// keyword or code, so an ordinary lyric line ("Chorus of angels sing") is not
/// mistaken for one.
fn parse_header(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    if t.is_empty() {
        return None;
    }
    // Strip a surrounding [ ... ] or a trailing colon.
    let inner = if t.starts_with('[') && t.ends_with(']') && t.len() >= 2 {
        &t[1..t.len() - 1]
    } else if let Some(stripped) = t.strip_suffix(':') {
        stripped
    } else {
        t
    };

    let norm = inner.to_lowercase().replace('-', " ");
    let words: Vec<&str> = norm.split_whitespace().collect();
    if words.is_empty() || words.len() > 3 {
        return None;
    }

    // Two-word "pre chorus [n]". Reject a trailing non-numeric word so a lyric
    // like "pre chorus of glory" is not treated as a header.
    if words[0] == "pre" && words.get(1) == Some(&"chorus") {
        return match words.get(2) {
            None => Some(labelled("PC", "Pre-Chorus", None)),
            Some(w) => w
                .parse::<i64>()
                .ok()
                .map(|n| labelled("PC", "Pre-Chorus", Some(n))),
        };
    }

    // Keyword form: the keyword alone, or keyword + a number ONLY. Anything else
    // after the keyword means it's an ordinary lyric line ("verse of text").
    if let Some((tag, label)) = keyword_base(words[0]) {
        return match words.get(1) {
            None => Some(labelled(tag, label, None)),
            Some(w) => w.parse::<i64>().ok().map(|n| labelled(tag, label, Some(n))),
        };
    }

    // Code form on a single token: v1 / c2 / br / pc3 / b1.
    if words.len() == 1 {
        if let Some(h) = parse_code(words[0]) {
            return Some(h);
        }
    }
    None
}

/// Map a leading keyword to its (tag, label) base. None if not a section word.
fn keyword_base(word: &str) -> Option<(&'static str, &'static str)> {
    Some(match word {
        "verse" => ("V", "Verse"),
        "chorus" => ("C", "Chorus"),
        "bridge" => ("BR", "Bridge"),
        "prechorus" => ("PC", "Pre-Chorus"),
        "intro" => ("INT", "Intro"),
        "outro" => ("OUT", "Outro"),
        "tag" => ("TAG", "Tag"),
        "refrain" => ("REF", "Refrain"),
        "interlude" => ("IL", "Interlude"),
        "vamp" => ("VMP", "Vamp"),
        "ending" => ("END", "Ending"),
        "part" => ("P", "Part"),
        _ => return None,
    })
}

/// Parse a compact code token like "v1", "c2", "br", "pc3".
fn parse_code(token: &str) -> Option<(String, String)> {
    let split = token
        .find(|c: char| c.is_ascii_digit())
        .unwrap_or(token.len());
    let (alpha, digits) = token.split_at(split);
    let num = if digits.is_empty() {
        None
    } else {
        digits.parse::<i64>().ok()
    };
    let (tag, label) = match alpha {
        "v" => ("V", "Verse"),
        "c" => ("C", "Chorus"),
        "b" | "br" => ("BR", "Bridge"),
        "pc" => ("PC", "Pre-Chorus"),
        _ => return None,
    };
    Some(labelled(tag, label, num))
}

/// Compose the (tag, label) with an optional number.
fn labelled(tag: &str, label: &str, num: Option<i64>) -> (String, String) {
    match num {
        Some(n) => (format!("{tag}{n}"), format!("{label} {n}")),
        None => (tag.to_string(), label.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(secs: &[ParsedSection]) -> Vec<&str> {
        secs.iter().map(|s| s.tag.as_str()).collect()
    }

    #[test]
    fn bracketed_headers() {
        let s = parse_song("[Verse 1]\nYou are here\n\n[Chorus]\nWay maker\nMiracle worker");
        assert_eq!(tags(&s), vec!["V1", "C"]);
        assert_eq!(s[0].label, "Verse 1");
        assert_eq!(s[0].lyrics, "You are here");
        assert_eq!(s[1].lyrics, "Way maker\nMiracle worker");
    }

    #[test]
    fn colon_and_code_headers() {
        let s = parse_song("Verse 1:\nline a\nChorus:\nline b\nV2\nline c");
        assert_eq!(tags(&s), vec!["V1", "C", "V2"]);
        assert_eq!(s[2].label, "Verse 2");
    }

    #[test]
    fn pre_chorus_variants() {
        assert_eq!(
            parse_header("Pre-Chorus"),
            Some(("PC".into(), "Pre-Chorus".into()))
        );
        assert_eq!(
            parse_header("[Pre Chorus 2]"),
            Some(("PC2".into(), "Pre-Chorus 2".into()))
        );
        assert_eq!(
            parse_header("pc1"),
            Some(("PC1".into(), "Pre-Chorus 1".into()))
        );
    }

    #[test]
    fn lyric_line_is_not_a_header() {
        // Leads with a section word but is a real lyric line (too many words).
        assert_eq!(parse_header("Chorus of angels singing praise"), None);
        assert_eq!(parse_header("You are here, moving in our midst"), None);
    }

    #[test]
    fn unlabelled_blocks_become_parts() {
        let s = parse_song("first block\nline two\n\nsecond block\n\nthird");
        assert_eq!(tags(&s), vec!["P1", "P2", "P3"]);
        assert_eq!(s[0].lyrics, "first block\nline two");
    }

    #[test]
    fn single_block_is_one_song_section() {
        let s = parse_song("just one\nverse of text");
        assert_eq!(tags(&s), vec!["S"]);
        assert_eq!(s[0].label, "Song");
    }

    #[test]
    fn content_before_first_header_is_kept() {
        let s = parse_song("orphan line\n[Chorus]\nhook");
        assert_eq!(tags(&s), vec!["S", "C"]);
        assert_eq!(s[0].lyrics, "orphan line");
    }

    #[test]
    fn empty_input_is_empty() {
        assert!(parse_song("").is_empty());
        assert!(parse_song("   \n  \n").is_empty());
    }
}
