//! Find a ProPresenter library that is already on this computer.
//!
//! One responsibility: look in the places a ProPresenter library actually lives,
//! count what is there, and report it. **It imports nothing and opens no song.**
//! The operator decides; this only answers "is there something here to offer?"
//!
//! ## Why a scan and not a file picker
//!
//! The same answer `models.rs` gives for the STT model directory, and worth
//! repeating because it looks like the lazier choice and is not. A church's
//! ProPresenter library is 726 files in a folder whose path the volunteer running
//! Relay has very likely never seen. Asking them to find
//! `~/Documents/ProPresenter/Libraries` is asking them to give up, and the folder
//! picker that already exists (requirement 13's `Import folder`) is the answer for
//! somebody who DOES know where it is. This is the answer for everybody else.
//!
//! ## What it will not do
//!
//! **It does not crawl the disk.** It looks in a short list of known roots and
//! walks a bounded depth below each. A scan that searched everywhere would take
//! minutes on a machine with a big Documents folder, would find copies in
//! backups and downloads that are not the library anybody means, and would read as
//! a hang on the one surface a volunteer is already unsure about.
//!
//! **It does not read a single song.** Counting `.pro` files by name is enough to
//! say "there is a library here with 726 songs in it", and opening 726 protobuf
//! containers to say the same thing would be slower and would claim more.
//!
//! ## Rule 17 lives here
//!
//! On macOS `~/Documents` is TCC-gated. A build with no
//! `NSDocumentsFolderUsageDescription` in `Info.plist` does not get a polite
//! refusal — the directory simply reads as empty, or the process is killed, and
//! **neither is visible in `tauri dev`**. The string is in `src-tauri/Info.plist`
//! beside the microphone one, and `scripts/sign-local.sh` is how it gets checked
//! without a certificate.

use std::path::{Path, PathBuf};

/// How deep below a known root to look.
///
/// A real library is `<root>/Libraries/<NAME>/song.pro` — three levels. Four gives
/// one level of slack for a church that has nested a folder inside their library,
/// and stops well short of the general-purpose disk crawl this is not.
const MAX_DEPTH: usize = 4;

/// How many directory entries one scan may look at, across every root.
///
/// A bound rather than a guess about anybody's disk. A Documents folder with a
/// photo library in it has hundreds of thousands of entries, and a scan that
/// walked all of them would read as a hang. Hitting this is reported rather than
/// silently truncating the answer: a count that stopped early and does not say so
/// is a wrong count.
const MAX_ENTRIES: usize = 50_000;

/// A ProPresenter library found on this machine.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct FoundLibrary {
    /// The folder to hand to the importer.
    pub path: String,
    /// How many `.pro` files are under it.
    pub songs: usize,
    /// Whether the walk stopped at `MAX_ENTRIES` before it finished. A count with
    /// this set is a FLOOR, and the surface must say so rather than printing it as
    /// though it were the answer.
    pub truncated: bool,
}

/// Is this a file the importer would actually read?
///
/// The junk filter from the folder import, stated once more on this side because
/// counting AppleDouble stubs would report a library of 726 songs beside a
/// `__MACOSX` tree as having 1452 — and the operator would find out only after
/// waiting for half of them to come in empty.
pub fn is_song_file(name: &str) -> bool {
    !name.starts_with('.') && name.to_lowercase().ends_with(".pro")
}

/// Should the walk go into this directory at all?
///
/// `__MACOSX` is an archive artefact holding one stub per real file; descending it
/// doubles the work to find nothing. A dot-directory is somebody's business and
/// not ours.
pub fn worth_descending(name: &str) -> bool {
    name != "__MACOSX" && !name.starts_with('.')
}

/// The places a ProPresenter library actually lives on this machine.
///
/// Both ProPresenter 6 and 7 defaults, plus the application-support location the
/// installer uses. Returned in the order they are worth trying, and a path that
/// does not exist is simply not offered — a list of places that were looked in and
/// found empty is a diagnostic, not an answer.
pub fn candidate_roots(home: &Path) -> Vec<PathBuf> {
    let mut out = vec![
        home.join("Documents").join("ProPresenter"),
        home.join("Documents").join("ProPresenter7"),
        home.join("Documents").join("ProPresenter6"),
        home.join("Library")
            .join("Application Support")
            .join("RenewedVision")
            .join("ProPresenter"),
        // Windows and Linux put Documents in the same relative place often enough
        // that this is worth one entry rather than a `cfg` fork.
        home.join("ProPresenter"),
    ];
    out.retain(|p| p.is_dir());
    out
}

/// Count the songs under one root, bounded.
pub fn scan_root(root: &Path) -> FoundLibrary {
    let mut songs = 0usize;
    let mut seen = 0usize;
    let mut truncated = false;
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            // An unreadable directory is skipped, not fatal. On macOS this is
            // exactly what a TCC refusal looks like from in here, and the scan
            // reporting zero for that root is the honest outcome — the surface
            // says nothing was found rather than claiming there is nothing there.
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen > MAX_ENTRIES {
                truncated = true;
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                if depth + 1 < MAX_DEPTH && worth_descending(&name) {
                    stack.push((entry.path(), depth + 1));
                }
            } else if is_song_file(&name) {
                songs += 1;
            }
        }
        if truncated {
            break;
        }
    }
    FoundLibrary {
        path: root.to_string_lossy().to_string(),
        songs,
        truncated,
    }
}

/// Every library worth offering the operator, best first.
///
/// A root with no songs in it is dropped: ProPresenter leaves its folder behind
/// when it is uninstalled, and offering an empty one would have the operator
/// import nothing and wonder which part failed.
pub fn find(home: &Path) -> Vec<FoundLibrary> {
    let mut out: Vec<FoundLibrary> = candidate_roots(home)
        .iter()
        .map(|r| scan_root(r))
        .filter(|f| f.songs > 0)
        .collect();
    // Biggest first: the library with the most songs in it is the one a church
    // means, and a backup copy with nine songs should not be the first thing offered.
    out.sort_by_key(|f| std::cmp::Reverse(f.songs));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_appledouble_stub_is_not_a_song() {
        // `._Blessed be the Lord .pro` carries the same extension as the song
        // beside it and is 4 KB of resource fork. Counting them reports a 726-song
        // library as 1452, and the operator finds out when half come in empty.
        assert!(is_song_file("Blessed be the Lord .pro"));
        assert!(!is_song_file("._Blessed be the Lord .pro"));
        assert!(!is_song_file(".DS_Store"));
        // A library's own config files have no extension at all.
        assert!(!is_song_file("Timers"));
        assert!(!is_song_file("Library"));
    }

    #[test]
    fn the_extension_check_is_case_insensitive() {
        // A library copied off a Windows machine, or through a case-insensitive
        // archive, arrives shouting.
        assert!(is_song_file("SONG.PRO"));
        assert!(is_song_file("Song.Pro"));
    }

    #[test]
    fn the_archive_artefact_tree_is_not_descended() {
        // One stub per real file, so walking it doubles the work to find nothing.
        assert!(!worth_descending("__MACOSX"));
        assert!(!worth_descending(".git"));
        assert!(worth_descending("Libraries"));
        assert!(worth_descending("SONGS"));
    }

    #[test]
    fn a_root_that_is_not_there_is_not_offered() {
        // A list of places that were looked in and found missing is a diagnostic,
        // not an answer, and an operator offered a path that does not exist would
        // reasonably conclude the scan was broken.
        let nowhere = PathBuf::from("/nonexistent-relay-test-home");
        assert!(candidate_roots(&nowhere).is_empty());
        assert!(find(&nowhere).is_empty());
    }

    #[test]
    fn a_real_tree_is_counted_and_the_junk_is_not() {
        let dir = std::env::temp_dir().join(format!("relay-prodiscover-{}", std::process::id()));
        let lib = dir.join("Documents").join("ProPresenter").join("SONGS");
        std::fs::create_dir_all(&lib).expect("make a library");
        for n in 0..3 {
            std::fs::write(lib.join(format!("Song {n}.pro")), b"x").expect("song");
            std::fs::write(lib.join(format!("._Song {n}.pro")), b"x").expect("stub");
        }
        std::fs::write(lib.join(".DS_Store"), b"x").expect("junk");
        std::fs::write(lib.join("Timers"), b"x").expect("config");
        std::fs::create_dir_all(lib.join("__MACOSX")).expect("artefact tree");
        std::fs::write(lib.join("__MACOSX").join("._x.pro"), b"x").expect("stub");

        let found = find(&dir);
        assert_eq!(found.len(), 1, "the library under Documents was not found");
        assert_eq!(found[0].songs, 3, "junk was counted as songs");
        assert!(!found[0].truncated);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_empty_library_folder_is_not_offered() {
        // ProPresenter leaves its folder behind when it is uninstalled. Offering it
        // would have the operator import nothing and wonder which part failed.
        let dir =
            std::env::temp_dir().join(format!("relay-prodiscover-empty-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Documents").join("ProPresenter")).expect("make it");
        assert!(find(&dir).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
