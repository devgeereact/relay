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
                                    out.push(b as char);
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
            // \uc default is 1: skip the following fallback char.
            if next < n && !matches!(chars[next], '\\' | '{' | '}') {
                next += 1;
            }
        }
        _ => {}
    }
    next
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
