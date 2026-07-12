//! Speech-model acquisition: get the STT model onto the operator's machine.
//!
//! Single responsibility: know which models exist, whether they are installed,
//! and download one safely with progress the operator can watch.
//!
//! ## Why this module exists
//!
//! Relay's entire reason to exist is that it hears the preacher. Until this
//! module, the only way to enable that was to open a terminal and run:
//!
//! ```text
//! mkdir -p models
//! curl -L -o models/ggml-base.bin https://huggingface.co/.../ggml-base.bin
//! ```
//!
//! A church volunteer will not do that. Worse, in a PACKAGED app there is no repo
//! `models/` directory at all — that instruction only ever worked for someone who
//! had cloned the repository with git. So for the actual target user, the AI
//! silently did not exist. The operator guide never even mentioned the model, and
//! Settings told them to go and read the developer README.
//!
//! This is the highest-value code in the product: it is the difference between a
//! good engine and a usable one.
//!
//! ## Design constraints, all learned from the target market
//!
//! - **Resumable.** A 148 MB download over a church's connection WILL be
//!   interrupted. Restarting from zero each time is not acceptable; we send a
//!   `Range` header and continue.
//! - **Verified.** The file is checked against a known SHA-256 before it is
//!   accepted. A truncated or corrupted model does not fail loudly — whisper
//!   loads garbage and transcribes nonsense, which is far worse than not working.
//! - **Atomic.** Downloads land in `<name>.part` and are renamed into place only
//!   after the checksum passes. Relay can never see a half-written model.
//! - **Cancellable**, and never blocking the UI thread.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager};

/// One downloadable speech model.
#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    /// Stable id used by the frontend and the download command.
    pub id: &'static str,
    /// Filename on disk — must match `stt::MODEL_CANDIDATES`.
    pub filename: &'static str,
    pub label: &'static str,
    /// Plain-language description for a volunteer, not a machine-learning person.
    pub detail: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
    /// Whether this is the one we want most people to pick.
    pub recommended: bool,
    /// Filled in per-request: is it already on this machine?
    pub installed: bool,
}

/// The catalogue.
///
/// Checksums and sizes are the REAL values of the two models this project has
/// actually been run against — computed from the files on disk, not copied from a
/// README. If a download does not match these bytes, it is not the model we
/// tested and we refuse it.
const CATALOG: &[ModelInfo] = &[
    ModelInfo {
        id: "base",
        filename: "ggml-base.bin",
        label: "Multilingual (recommended)",
        detail: "Understands English plus Yoruba, Swahili and Hausa, including switching between them mid-sentence.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
        sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
        bytes: 147_951_465,
        recommended: true,
        installed: false,
    },
    ModelInfo {
        id: "base.en",
        filename: "ggml-base.en.bin",
        label: "English only",
        detail: "Slightly sharper on English, but cannot understand any other language.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
        sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
        bytes: 147_964_211,
        recommended: false,
        installed: false,
    },
];

/// Where models live: the per-OS app-data dir. Never the repo `models/` folder —
/// that does not exist in a packaged app (see `db::app_data_dir`).
pub fn models_dir() -> PathBuf {
    crate::db::app_data_dir().join("models")
}

/// The catalogue, with `installed` resolved against this machine.
pub fn catalog() -> Vec<ModelInfo> {
    let dir = models_dir();
    CATALOG
        .iter()
        .map(|m| ModelInfo {
            installed: dir.join(m.filename).exists()
                // Also count a model the developer put in the repo `models/` dir.
                || crate::stt::default_model_path()
                    .map(|p| p.ends_with(m.filename))
                    .unwrap_or(false),
            ..m.clone()
        })
        .collect()
}

fn find(id: &str) -> Option<&'static ModelInfo> {
    CATALOG.iter().find(|m| m.id == id)
}

/// Progress, pushed to the UI as `model://progress`.
#[derive(Clone, Serialize)]
struct Progress {
    id: String,
    downloaded: u64,
    total: u64,
}

/// Set while a download runs; flipped to cancel it.
#[derive(Default)]
pub struct DownloadState {
    pub cancel: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
}

/// Download `id` into the models dir: resumable, checksummed, atomic.
///
/// Emits `model://progress` throughout, then exactly one of `model://done` or
/// `model://error`. Runs on the async runtime; never blocks the UI thread.
pub async fn download(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let model = find(&id).ok_or_else(|| format!("unknown model '{id}'"))?;
    let state = app.state::<DownloadState>();

    if state.running.swap(true, Ordering::SeqCst) {
        return Err("A model download is already running.".into());
    }
    state.cancel.store(false, Ordering::SeqCst);
    let cancel = state.cancel.clone();
    let running = state.running.clone();

    let result = download_inner(&app, model, cancel).await;
    running.store(false, Ordering::SeqCst);

    match &result {
        Ok(()) => {
            let _ = app.emit("model://done", &id);
        }
        Err(e) => {
            let _ = app.emit("model://error", e);
        }
    }
    result
}

async fn download_inner(
    app: &tauri::AppHandle,
    model: &'static ModelInfo,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    use futures_util::StreamExt;

    let dir = models_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create {dir:?}: {e}"))?;
    let final_path = dir.join(model.filename);
    if final_path.exists() {
        return Ok(()); // already have it
    }
    let part_path = dir.join(format!("{}.part", model.filename));

    // Resume from whatever a previous attempt managed to fetch.
    let already = std::fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
    let already = if already > model.bytes { 0 } else { already };

    let client = reqwest::Client::builder()
        .user_agent("relay-church/0.1")
        .build()
        .map_err(|e| e.to_string())?;

    let mut req = client.get(model.url);
    if already > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={already}-"));
    }
    let resp = req.send().await.map_err(friendly_net_error)?;

    // If the server ignored our Range header, start over rather than append into
    // the middle of the file and silently produce a corrupt model.
    let resuming = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    if !resp.status().is_success() {
        return Err(format!(
            "The download server returned {}. Try again later.",
            resp.status()
        ));
    }
    let start = if resuming { already } else { 0 };

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(!resuming)
        .append(resuming)
        .open(&part_path)
        .map_err(|e| format!("Could not write to {part_path:?}: {e}"))?;

    let mut downloaded = start;
    let mut stream = resp.bytes_stream();
    let mut last_emit = 0u64;

    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::SeqCst) {
            let _ = file.flush();
            return Err("Download cancelled.".into());
        }
        let chunk = chunk.map_err(friendly_net_error)?;
        file.write_all(&chunk)
            .map_err(|e| format!("Could not write the model to disk: {e}"))?;
        downloaded += chunk.len() as u64;

        // Throttle: one event per ~1 MB, not per TCP chunk.
        if downloaded - last_emit > 1_000_000 {
            last_emit = downloaded;
            let _ = app.emit(
                "model://progress",
                Progress {
                    id: model.id.to_string(),
                    downloaded,
                    total: model.bytes,
                },
            );
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);

    // Verify BEFORE accepting it. A truncated model does not fail loudly — whisper
    // loads it and transcribes nonsense, which is far worse than not working.
    let got = sha256_file(&part_path)?;
    if !got.eq_ignore_ascii_case(model.sha256) {
        let _ = std::fs::remove_file(&part_path);
        return Err(
            "The downloaded file didn't match its checksum, so it was discarded. \
             This usually means the download was interrupted — please try again."
                .into(),
        );
    }

    std::fs::rename(&part_path, &final_path)
        .map_err(|e| format!("Could not finish installing the model: {e}"))?;

    let _ = app.emit(
        "model://progress",
        Progress {
            id: model.id.to_string(),
            downloaded: model.bytes,
            total: model.bytes,
        },
    );
    println!("models: installed {}", final_path.display());
    Ok(())
}

/// Turn a network error into something a volunteer can act on.
fn friendly_net_error(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "The download timed out. Check the internet connection and try again.".into()
    } else if e.is_connect() {
        "Couldn't reach the download server. Is this machine online?".into()
    } else {
        format!("The download failed: {e}")
    }
}

fn sha256_file(path: &PathBuf) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut f, &mut hasher).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The catalogue must stay in step with what stt.rs actually looks for. If a
    /// filename drifts, we would download a model the engine then can't find —
    /// the exact silent failure this module exists to end.
    #[test]
    fn every_catalogued_filename_is_one_stt_looks_for() {
        for m in CATALOG {
            assert!(
                crate::stt::MODEL_CANDIDATES.contains(&m.filename),
                "{} is not in stt::MODEL_CANDIDATES",
                m.filename
            );
        }
    }

    #[test]
    fn exactly_one_model_is_recommended() {
        assert_eq!(CATALOG.iter().filter(|m| m.recommended).count(), 1);
    }

    /// The recommended model must be the multilingual one — the tier-1 languages
    /// (Yoruba, Swahili, Hausa) are the product's whole differentiator, and the
    /// English-only model cannot do them at all.
    #[test]
    fn the_recommended_model_is_the_multilingual_one() {
        let rec = CATALOG.iter().find(|m| m.recommended).unwrap();
        assert_eq!(rec.filename, "ggml-base.bin");
        assert!(!rec.filename.contains(".en."));
    }

    #[test]
    fn checksums_look_like_sha256() {
        for m in CATALOG {
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert!(m.sha256.chars().all(|c| c.is_ascii_hexdigit()), "{}", m.id);
            assert!(m.bytes > 100_000_000, "{} size looks wrong", m.id);
        }
    }

    #[test]
    fn models_live_in_app_data_never_in_the_repo() {
        // A packaged app has no repo `models/` dir — that assumption is exactly
        // what made the AI unreachable for real users.
        assert!(models_dir().starts_with(crate::db::app_data_dir()));
        assert!(models_dir().ends_with("models"));
    }
}

/// Config invariants that are only discovered at RUNTIME, and would otherwise
/// ship. `cargo test` and `tauri build` both pass on a config that panics the app
/// on startup — a compile is not a boot.
#[cfg(test)]
mod config_boots {
    /// Registering the updater plugin with a null `plugins.updater` PANICS the app
    /// at startup:
    ///
    /// ```text
    ///   PluginInitialization("updater", "invalid type: null, expected struct Config")
    /// ```
    ///
    /// It did exactly that. The config lived only in the release-only overlay, so
    /// `tauri dev` died on boot — and the PACKAGED app would have too, because CI
    /// only ever compiles the release build and never launches it.
    ///
    /// The base config must therefore always carry an updater block, even an inert
    /// one (empty pubkey). The real key is injected at release time.
    #[test]
    fn the_base_config_has_an_updater_block_or_the_app_panics_on_startup() {
        const CONF: &str = include_str!("../tauri.conf.json");
        let c: serde_json::Value = serde_json::from_str(CONF).expect("tauri.conf.json");
        let updater = &c["plugins"]["updater"];
        assert!(
            updater.is_object(),
            "plugins.updater is missing from tauri.conf.json — the app will PANIC on \
             startup. It must exist even with an empty pubkey."
        );
        assert!(
            updater["pubkey"].is_string(),
            "plugins.updater.pubkey must be a string (may be empty)"
        );
        assert!(
            updater["endpoints"]
                .as_array()
                .is_some_and(|a| !a.is_empty()),
            "plugins.updater.endpoints must be non-empty"
        );
    }
}
