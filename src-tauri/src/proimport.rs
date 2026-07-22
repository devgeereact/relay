//! ProPresenter import.
//!
//! Single responsibility: pull lyric text out of ProPresenter files so it can be
//! saved as songs. A `.proplaylist` is a ZIP of `.pro` (ProPresenter 7)
//! presentations; the lyric of each slide is stored as an RTF blob inside the
//! (otherwise protobuf) `.pro` bytes. We don't parse the protobuf — we scan for
//! `{\rtf1 … }` blocks and strip the RTF to plain text. Fully offline.
//!
//! The RTF stripper and slide scanner are pure and unit-tested; the ZIP walk is
//! the only IO-ish part. Best-effort by nature (proprietary format) — good
//! enough to get a church's existing songs into Relay without retyping.

use std::io::Read;

/// A song pulled from an import: a title and its ordered slide texts.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedSong {
    pub title: String,
    pub slides: Vec<String>,
}

/// Strip an RTF blob to plain text. Handles the subset ProPresenter emits:
/// ignorable destinations ({\fonttbl}, {\colortbl}, {\*\…}), `\par`/`\line`
/// breaks, `\'xx` hex bytes, `\uN` unicode, and control words with params.
pub fn rtf_to_text(rtf: &str) -> String {
    let chars: Vec<char> = rtf.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut ignore_stack: Vec<bool> = Vec::new();
    let mut ignore = false;
    let mut i = 0;

    while i < n {
        match chars[i] {
            '{' => {
                ignore_stack.push(ignore);
                i += 1;
            }
            '}' => {
                ignore = ignore_stack.pop().unwrap_or(false);
                i += 1;
            }
            '\\' if i + 1 < n => {
                let d = chars[i + 1];
                match d {
                    '\\' | '{' | '}' => {
                        if !ignore {
                            out.push(d);
                        }
                        i += 2;
                    }
                    '*' => {
                        // {\*\dest …} — an ignorable destination.
                        ignore = true;
                        i += 2;
                    }
                    '\'' => {
                        // \'xx — a hex-encoded byte (Latin-1 for our purposes).
                        if i + 3 < n {
                            let hex: String = chars[i + 2..i + 4].iter().collect();
                            if let Ok(b) = u8::from_str_radix(&hex, 16) {
                                if !ignore {
                                    out.push(cp1252(b));
                                }
                            }
                            i += 4;
                        } else {
                            i += 2;
                        }
                    }
                    c if c.is_ascii_alphabetic() => {
                        i = control_word(&chars, i, &mut ignore, &mut out);
                    }
                    '\n' | '\r' => {
                        // ProPresenter encodes an in-slide line break as a lone
                        // backslash before a newline ("song\<newline>Praising").
                        if !ignore {
                            out.push('\n');
                        }
                        i += 2;
                    }
                    _ => {
                        // A control symbol like \~ or \- — skip it.
                        i += 2;
                    }
                }
            }
            '\r' | '\n' => i += 1, // raw line breaks in the RTF source aren't text
            c => {
                if !ignore {
                    out.push(c);
                }
                i += 1;
            }
        }
    }
    tidy(&out)
}

/// Consume a control word starting at `i` (which points at the '\\'); apply its
/// effect and return the next index.
fn control_word(chars: &[char], i: usize, ignore: &mut bool, out: &mut String) -> usize {
    let n = chars.len();
    let mut j = i + 1;
    while j < n && chars[j].is_ascii_alphabetic() {
        j += 1;
    }
    let word: String = chars[i + 1..j].iter().collect();

    // Optional numeric parameter (possibly negative).
    let mut k = j;
    let neg = k < n && chars[k] == '-';
    if neg {
        k += 1;
    }
    let mut num = String::new();
    while k < n && chars[k].is_ascii_digit() {
        num.push(chars[k]);
        k += 1;
    }
    // A single trailing space is the control-word delimiter and is consumed.
    let mut next = k;
    if next < n && chars[next] == ' ' {
        next += 1;
    }

    match word.as_str() {
        "fonttbl" | "colortbl" | "stylesheet" | "expandedcolortbl" | "pict" | "info"
        | "filetbl" | "listtable" | "listoverridetable" | "generator" | "themedata"
        | "colorschememapping" => {
            *ignore = true;
        }
        "par" | "line" => {
            if !*ignore {
                out.push('\n');
            }
        }
        "tab" => {
            if !*ignore {
                out.push('\t');
            }
        }
        "u" => {
            if !*ignore {
                if let Ok(cp) = num.parse::<i64>() {
                    let cp = if neg { cp + 65536 } else { cp };
                    if let Some(ch) = u32::try_from(cp).ok().and_then(char::from_u32) {
                        out.push(ch);
                    }
                }
            }
            // \uc default is 1: skip the ONE fallback character that follows.
            //
            // But only if the parameter was not already delimited by a space. A
            // space ends the number AND is consumed above; treating the next
            // character as a fallback then swallows a real letter — which is how
            // an imported lyric read "ntil the day" instead of "until the day",
            // on a screen, in front of a congregation.
            let space_delimited = k < n && chars[k] == ' ';
            if !space_delimited && next < n && !matches!(chars[next], '\\' | '{' | '}') {
                next += 1;
            }
        }
        _ => {}
    }
    next
}

/// Map an RTF `\'xx` byte to a character using **Windows-1252**, which is what
/// ProPresenter actually writes — not Latin-1.
///
/// The two encodings agree everywhere except 0x80–0x9F, where Latin-1 has
/// invisible C1 control codes and Windows-1252 has the punctuation people
/// actually type: curly quotes, apostrophes, en/em dashes, ellipsis.
///
/// Treating those bytes as raw code points turned every apostrophe in an
/// imported song into U+0092 — an invisible control character that `tidy()` then
/// collapsed to whitespace. "they're" reached the projector as "they   re".
fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}',
        '\u{017D}', '\u{FFFD}', '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
        '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
    ];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// Normalize extracted text: trim each line, drop blank lines, collapse runs of
/// spaces, and trim the whole.
fn tidy(s: &str) -> String {
    let lines: Vec<String> = s
        .split('\n')
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect();
    lines.join("\n")
}

/// Scan raw `.pro` bytes for `{\rtf1 … }` blocks and return each slide's text,
/// dropping consecutive duplicates (ProPresenter often stores the same text
/// more than once). Order follows the byte order of the blocks.
pub fn extract_pro_slides(bytes: &[u8]) -> Vec<String> {
    let s = String::from_utf8_lossy(bytes);
    let mut slides: Vec<String> = Vec::new();
    let mut search_from = 0;

    while let Some(rel) = s[search_from..].find("{\\rtf1") {
        let start = search_from + rel;
        // Walk to the matching closing brace.
        let mut depth = 0i32;
        let mut end = None;
        for (k, c) in s[start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + k + c.len_utf8());
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else { break };
        let text = rtf_to_text(&s[start..end]);
        let text = text.trim().to_string();
        if !text.is_empty() && slides.last() != Some(&text) {
            slides.push(text);
        }
        search_from = end;
    }
    slides
}

/// Parse a `.proplaylist` (a ZIP of `.pro` files) into songs. Each `.pro` entry
/// becomes a song titled by its file stem, with its slides as sections.
pub fn parse_proplaylist(zip_bytes: &[u8]) -> Result<Vec<ImportedSong>, String> {
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut zip = zip::ZipArchive::new(cursor).map_err(|e| format!("not a valid playlist: {e}"))?;
    let mut songs = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().to_string();
        if !name.to_lowercase().ends_with(".pro") {
            continue;
        }
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        let slides = extract_pro_slides(&buf);
        if !slides.is_empty() {
            songs.push(ImportedSong {
                title: stem(&name),
                slides,
            });
        }
    }
    Ok(songs)
}

/// Decide what a set of bytes is and extract songs. `.proplaylist` (ZIP, starts
/// with `PK`) → many songs; a bare `.pro` → one song titled by `filename`.
pub fn import_bytes(filename: &str, bytes: &[u8]) -> Result<Vec<ImportedSong>, String> {
    if bytes.starts_with(b"PK") {
        parse_proplaylist(bytes)
    } else {
        let slides = extract_pro_slides(bytes);
        if slides.is_empty() {
            Ok(vec![])
        } else {
            Ok(vec![ImportedSong {
                title: stem(filename),
                slides,
            }])
        }
    }
}

/// File stem without directory or extension.
fn stem(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    base.rsplit_once('.')
        .map(|(a, _)| a)
        .unwrap_or(base)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_fonttbl_and_colortbl() {
        let rtf = r"{\rtf1\ansi{\fonttbl\f0\fnil X;}{\colortbl;\red255\green255\blue255;}\f0\fs354 \cf2 \up0 Praising my Saviour all the day long}";
        assert_eq!(rtf_to_text(rtf), "Praising my Saviour all the day long");
    }

    #[test]
    fn par_and_line_break_to_newlines() {
        assert_eq!(
            rtf_to_text(r"{\rtf1 line one\par line two\line line three}"),
            "line one\nline two\nline three"
        );
    }

    #[test]
    fn backslash_newline_is_a_line_break() {
        // ProPresenter's in-slide break: a lone backslash before a real newline.
        let rtf = "{\\rtf1 This is my story, this is my song\\\nPraising my Saviour}";
        assert_eq!(
            rtf_to_text(rtf),
            "This is my story, this is my song\nPraising my Saviour"
        );
    }

    #[test]
    fn ignores_starred_destinations() {
        let rtf = r"{\rtf1{\*\expandedcolortbl;;}Blessed assurance}";
        assert_eq!(rtf_to_text(rtf), "Blessed assurance");
    }

    #[test]
    fn extract_multiple_slides_and_dedup() {
        // Two distinct blocks plus binary junk between them, then a duplicate.
        let raw = b"\x00\x08{\\rtf1 Verse one}\xff\xff{\\rtf1 Chorus}\x00{\\rtf1 Chorus}";
        let slides = extract_pro_slides(raw);
        assert_eq!(slides, vec!["Verse one".to_string(), "Chorus".to_string()]);
    }

    #[test]
    fn stem_strips_path_and_ext() {
        assert_eq!(
            stem("Songs/Blessed assurance, Jesus is mine.pro"),
            "Blessed assurance, Jesus is mine"
        );
    }

    #[test]
    fn bare_bytes_with_no_rtf_yield_nothing() {
        assert!(import_bytes("x.pro", b"no lyrics here").unwrap().is_empty());
    }
}

#[cfg(test)]
mod rtf_corruption {
    use super::*;

    // Found by LOOKING at imported lyrics on screen: "until" had become "ntil"
    // and "they're" had become "they   re". Both are the decoder eating real
    // characters, and both reached a congregation's screen.

    #[test]
    fn a_unicode_escape_with_a_space_delimiter_does_not_eat_the_next_letter() {
        // The parameter is terminated by the space. Consuming the space AND a
        // "fallback" character swallows the first letter of the next word —
        // which is how "until" became "ntil" on a real screen.
        let bs = '\\';
        assert_eq!(
            rtf_to_text(&format!("me {bs}u8217 until the day")),
            "me \u{2019}until the day"
        );
    }

    #[test]
    fn a_unicode_escape_with_a_literal_fallback_still_skips_it() {
        // Here `?` IS the fallback and must not survive.
        let bs = '\\';
        assert_eq!(
            rtf_to_text(&format!("they{bs}u8217?re here")),
            "they\u{2019}re here"
        );
    }

    #[test]
    fn windows_1252_punctuation_survives() {
        // `\'92` is a curly apostrophe in Windows-1252, the encoding
        // ProPresenter actually emits. Treating the byte as a raw code point
        // yields U+0092 — an invisible control character — which is why
        // "they're" arrived as "they   re".
        assert_eq!(rtf_to_text(r"they\'92re"), "they\u{2019}re");
        assert_eq!(rtf_to_text(r"\'93quoted\'94"), "\u{201C}quoted\u{201D}");
        assert_eq!(rtf_to_text(r"dash \'96 here"), "dash \u{2013} here");
    }

    #[test]
    fn latin1_accents_still_work() {
        // Above 0x9F, Windows-1252 and Latin-1 agree — accented letters must be
        // unaffected by the fix.
        assert_eq!(rtf_to_text(r"caf\'e9"), "café");
    }

    #[test]
    fn ordinary_text_is_untouched() {
        assert_eq!(rtf_to_text("Hallelujah!"), "Hallelujah!");
    }
}
