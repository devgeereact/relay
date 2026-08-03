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
//! - **Cancellable — including when the network has silently died**, which is the
//!   only time it matters. Cancel used to be checked only after a chunk arrived, so
//!   on a half-open TCP connection (a dropped wifi: the single most likely event in a
//!   church hall) the check was never reached. The bar froze, Cancel did nothing, and
//!   the `running` flag stayed set for the rest of the process — so even after the
//!   wifi came back, every retry was refused with "A model download is already
//!   running" until Relay was quit and reopened.
//! - **Never blocking the UI thread.**
//!
//! The failure mode this module must survive is not "the download fails". It is
//! "the download neither succeeds nor fails, forever, and the operator cannot get
//! out of it" — a volunteer, an hour before the service, with no terminal.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    /// Does this model need GPU acceleration to keep up with a live sermon?
    ///
    /// Static: a property of the model, not of the machine. `catalog()` combines it
    /// with what THIS build can actually accelerate to produce `caution`.
    pub needs_acceleration: bool,
    /// Filled in per-request: is it already on this machine?
    pub installed: bool,
    /// Filled in per-request: why this model may be a bad idea *here*, in words a
    /// volunteer can act on. `None` means no known problem on this machine.
    ///
    /// This exists because a bigger model is a DOWNGRADE WEARING AN UPGRADE'S LABEL
    /// when nothing can accelerate it, and the failure is silent: the STT worker
    /// warns once to stderr that it is slower than real time, then quietly drops
    /// audio it can no longer catch up on. Nobody sees an error. The operator sees a
    /// transcript that thins out, an hour into a service, having done nothing wrong
    /// except pick the model that said it was more accurate.
    pub caution: Option<String>,
}

/// The catalogue.
///
/// Checksums and sizes are the REAL values — for `base` and `base.en`, computed
/// from the files this project has been run against; for the rest, the Git-LFS
/// object ids published by `ggerganov/whisper.cpp`, which ARE the sha256 of the
/// file (verified: the published ids for `base` and `base.en` reproduce the two
/// hashes below exactly, which is what makes the source trustworthy for the
/// others). If a download does not match these bytes, it is not the model we
/// meant and we refuse it.
///
/// ── WHY THERE IS MORE THAN `base` HERE ──────────────────────────────────────
///
/// Relay shipped only `base` — the smallest useful whisper — for its entire life,
/// while `docs/PRODUCT_AUDIT.md` called African-language accuracy the biggest
/// weakness in the product. Those two facts were never connected, because nothing
/// had ever measured what a larger model buys. `stt::bench::engine_shootout` is
/// that measurement, and these are the models it has to choose between.
///
/// `medium` is deliberately absent. `large-v3-turbo` is both faster and more
/// accurate, so offering `medium` would be offering a strictly worse option with
/// a friendlier-sounding name.
const CATALOG: &[ModelInfo] = &[
    ModelInfo {
        id: "base",
        filename: "ggml-base.bin",
        label: "Multilingual (recommended)",
        detail: "Understands English plus Yoruba, Swahili and Hausa, including switching between them mid-sentence. Runs on any laptop.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
        sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
        bytes: 147_951_465,
        recommended: true,
        needs_acceleration: false,
        installed: false,
        caution: None,
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
        needs_acceleration: false,
        installed: false,
        caution: None,
    },
    ModelInfo {
        id: "small",
        filename: "ggml-small.bin",
        label: "Multilingual, larger",
        detail: "Understands the same languages as the recommended model but hears them more accurately, especially over a poor microphone. Three times the download, and needs a reasonably quick computer.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
        bytes: 487_601_967,
        recommended: false,
        needs_acceleration: false,
        installed: false,
        caution: None,
    },
    ModelInfo {
        id: "large-v3-turbo-q5_0",
        filename: "ggml-large-v3-turbo-q5_0.bin",
        label: "Most accurate that still fits a modest laptop",
        detail: "The most accurate model, compressed so it downloads and loads in about a third of the space. Best choice for African languages. Works best on a computer with graphics acceleration.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        bytes: 574_041_195,
        recommended: false,
        needs_acceleration: true,
        installed: false,
        caution: None,
    },
    ModelInfo {
        id: "large-v3-turbo",
        filename: "ggml-large-v3-turbo.bin",
        label: "Most accurate",
        detail: "The best speech recognition Relay can run, uncompressed. A 1.6 GB download, and it needs a fast computer with graphics acceleration to keep up with a live sermon.",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
        sha256: "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
        bytes: 1_624_555_275,
        recommended: false,
        needs_acceleration: true,
        installed: false,
        caution: None,
    },
];

/// Where models live: the per-OS app-data dir. Never the repo `models/` folder —
/// that does not exist in a packaged app (see `db::app_data_dir`).
pub fn models_dir() -> PathBuf {
    crate::db::app_data_dir().join("models")
}

/// The catalogue, with `installed` and `caution` resolved against this machine.
pub fn catalog() -> Vec<ModelInfo> {
    let dir = models_dir();
    // Compile-time, not a GPU probe: a graphics card whisper.cpp was not built to
    // use is not acceleration, it is decoration (see `sysprobe`).
    let accelerated = !crate::sysprobe::gpu_backends().is_empty();
    CATALOG
        .iter()
        .map(|m| ModelInfo {
            installed: dir.join(m.filename).exists()
                // Also count a model the developer put in the repo `models/` dir.
                || crate::stt::default_model_path()
                    .map(|p| p.ends_with(m.filename))
                    .unwrap_or(false),
            caution: (m.needs_acceleration && !accelerated).then(|| {
                "This copy of Relay has no graphics acceleration, so this model will \
                 probably fall behind a live sermon — the transcript thins out instead \
                 of stopping, so it is easy to miss. Pick a smaller model unless you \
                 have tested this one on this computer."
                    .to_string()
            }),
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

/// How long to wait for the next byte before declaring the connection dead.
///
/// This is NOT an overall deadline — a 148 MB model on a slow church connection can
/// legitimately take an hour, and `reqwest`'s whole-request `.timeout()` would abort
/// exactly the download this module exists to make possible. What must never happen
/// is waiting FOREVER for a connection that has silently gone away.
const STALL_TIMEOUT: Duration = Duration::from_secs(45);

/// How often to wake up while waiting for data, to notice a cancel.
///
/// The cancel flag used to be checked only *after* `stream.next()` produced a chunk.
/// On a half-open TCP connection — a dropped wifi, which is the single most likely
/// thing to happen in a church hall — no chunk ever arrives, so the check was never
/// reached and **Cancel did nothing at all**. The operator watched a frozen progress
/// bar and a dead button. Waking on a tick makes cancel responsive whether or not the
/// network is delivering anything.
const CANCEL_POLL: Duration = Duration::from_millis(400);

/// Clears the `running` flag no matter how we leave `download` — including a panic
/// or a dropped future.
///
/// It used to be a bare `store(false)` after the await. When the download hung
/// forever (see `STALL_TIMEOUT`), that line was never reached, so `running` stayed
/// `true` for the rest of the process — and every subsequent attempt, including after
/// the operator reconnected the wifi, was refused with "A model download is already
/// running." until they quit and reopened Relay. A stuck flag turned a recoverable
/// network blip into a dead feature.
struct RunningGuard(Arc<AtomicBool>);
impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// How a download ended. Cancelling is not an error — see `download`.
enum Outcome {
    Done,
    Cancelled,
}

/// What to do with whatever a previous attempt left in `<name>.part`.
#[derive(Debug, PartialEq, Eq)]
enum Resume {
    /// Nothing usable on disk — fetch the whole file.
    Fresh,
    /// Continue from byte N with a `Range` header.
    From(u64),
    /// The `.part` is ALREADY the full size. Settle it by checksum.
    Verify,
}

/// Decide how to resume, from the size of the partial file alone.
///
/// Pure, so the case that actually bit us is testable without a network: a `.part`
/// of *exactly* `model.bytes` (we died on the final chunk, or on the rename). The
/// old code guarded with `already > model.bytes`, so this fell into the `From` arm
/// and asked the server for `Range: bytes=<total>-`. The server answered **416 Range
/// Not Satisfiable**, the code hard-errored — and did not delete the file. Every
/// retry produced the same 416, forever. The download was permanently bricked, and
/// the only fix was deleting a file the user did not know existed.
///
/// A full-size `.part` is never a resume point. It is a question — "is this the
/// model?" — and the answer is a checksum, not an HTTP request.
fn resume_plan(part_len: u64, model_bytes: u64) -> Resume {
    match part_len {
        0 => Resume::Fresh,
        n if n >= model_bytes => Resume::Verify,
        n => Resume::From(n),
    }
}

/// Download `id` into the models dir: resumable, checksummed, atomic, and
/// genuinely cancellable.
///
/// Emits `model://progress` throughout, then exactly one of `model://done`,
/// `model://cancelled` or `model://error`. Runs on the async runtime; never blocks
/// the UI thread.
pub async fn download(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let model = find(&id).ok_or_else(|| format!("unknown model '{id}'"))?;
    let state = app.state::<DownloadState>();

    if state.running.swap(true, Ordering::SeqCst) {
        return Err("A model download is already running.".into());
    }
    state.cancel.store(false, Ordering::SeqCst);
    let cancel = state.cancel.clone();
    let _guard = RunningGuard(state.running.clone());

    let result = download_inner(&app, model, cancel).await;

    match &result {
        Ok(Outcome::Done) => {
            let _ = app.emit("model://done", &id);
        }
        // Cancelling is a thing the operator CHOSE. It used to be emitted down the
        // error channel, so deliberately stopping a download painted a red failure
        // box that could not even be dismissed.
        Ok(Outcome::Cancelled) => {
            let _ = app.emit("model://cancelled", &id);
        }
        Err(e) => {
            let _ = app.emit("model://error", e);
        }
    }
    result.map(|_| ())
}

async fn download_inner(
    app: &tauri::AppHandle,
    model: &'static ModelInfo,
    cancel: Arc<AtomicBool>,
) -> Result<Outcome, String> {
    use futures_util::StreamExt;

    let dir = models_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create {dir:?}: {e}"))?;
    let final_path = dir.join(model.filename);
    if final_path.exists() {
        return Ok(Outcome::Done); // already have it
    }
    let part_path = dir.join(format!("{}.part", model.filename));

    // Resume from whatever a previous attempt managed to fetch. See `resume_plan`.
    let part_len = std::fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
    let already = match resume_plan(part_len, model.bytes) {
        Resume::Fresh => 0,
        Resume::From(n) => n,
        // A full-size .part is either a complete download whose checksum we never got
        // to run, or garbage. Hash it — do NOT ask the server to resume from the end
        // of it, which is a 416 and used to brick the download permanently.
        Resume::Verify => {
            if sha256_file(&part_path)?.eq_ignore_ascii_case(model.sha256) {
                std::fs::rename(&part_path, &final_path)
                    .map_err(|e| format!("Could not finish installing the model: {e}"))?;
                println!("models: installed {} (recovered)", final_path.display());
                return Ok(Outcome::Done);
            }
            let _ = std::fs::remove_file(&part_path);
            0
        }
    };

    let client = reqwest::Client::builder()
        .user_agent("relay-church/0.1")
        // Fail fast when the server is simply unreachable. Deliberately NOT a
        // whole-request `.timeout()` — that would abort a legitimately slow 148 MB
        // download. Stalls are handled by STALL_TIMEOUT below, which measures the gap
        // between bytes rather than the length of the download.
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let mut req = client.get(model.url);
    if already > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={already}-"));
    }
    let resp = req.send().await.map_err(friendly_net_error)?;

    // The server rejected our resume point. Throw the partial file away rather than
    // leaving it to poison every future attempt.
    if resp.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        let _ = std::fs::remove_file(&part_path);
        return Err(
            "The part-finished download couldn't be resumed, so it was discarded. \
                    Please try again — it will start from the beginning."
                .into(),
        );
    }

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
    let mut last_byte_at = Instant::now();

    loop {
        // Checked BEFORE waiting, and again on every tick below — so Cancel works
        // whether the connection is fast, slow, or silently dead.
        if cancel.load(Ordering::SeqCst) {
            let _ = file.flush();
            // The .part file is KEPT. Cancelling is not "throw away my 90 MB"; the
            // next attempt resumes from here.
            return Ok(Outcome::Cancelled);
        }

        match tokio::time::timeout(CANCEL_POLL, stream.next()).await {
            // No byte arrived within the tick. Normal on a slow connection — go round
            // again, re-check cancel, and only give up once nothing has arrived for
            // STALL_TIMEOUT.
            Err(_tick) => {
                if last_byte_at.elapsed() >= STALL_TIMEOUT {
                    let _ = file.flush();
                    return Err("The download stopped responding. Check the internet \
                                connection and try again — it will pick up where it left off."
                        .into());
                }
                continue;
            }
            Ok(None) => break, // stream finished
            Ok(Some(chunk)) => {
                let chunk = chunk.map_err(friendly_net_error)?;
                file.write_all(&chunk)
                    .map_err(|e| format!("Could not write the model to disk: {e}"))?;
                downloaded += chunk.len() as u64;
                last_byte_at = Instant::now();

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
    Ok(Outcome::Done)
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

    /// The default must stay the model that RUNS ON ANY LAPTOP, not the most
    /// accurate one. `MODEL_CANDIDATES` is what an operator who never opens Settings
    /// gets, and the large models were added below the small ones deliberately —
    /// this pins that ordering so a future edit cannot quietly promote a 1.6 GB
    /// model into the default position on a donated church laptop.
    #[test]
    fn the_default_order_leads_with_the_model_that_runs_anywhere() {
        let first = crate::stt::MODEL_CANDIDATES[0];
        assert_eq!(first, "ggml-base.bin");
        let heavy: Vec<_> = CATALOG
            .iter()
            .filter(|m| m.needs_acceleration)
            .map(|m| m.filename)
            .collect();
        let light_end = crate::stt::MODEL_CANDIDATES
            .iter()
            .position(|n| heavy.contains(n))
            .unwrap_or(crate::stt::MODEL_CANDIDATES.len());
        for name in &crate::stt::MODEL_CANDIDATES[..light_end] {
            assert!(
                !heavy.contains(name),
                "{name} needs acceleration but sits above one that does not"
            );
        }
    }

    /// A model that cannot keep up must SAY SO on the machine it cannot keep up on.
    /// The failure it prevents is silent — the STT worker warns once to stderr and
    /// then drops audio — so an operator who is never told will read it as Relay
    /// being bad at their language rather than as the model being too big.
    #[test]
    fn heavy_models_are_flagged_when_nothing_can_accelerate_them() {
        let accelerated = !crate::sysprobe::gpu_backends().is_empty();
        for m in catalog() {
            match (m.needs_acceleration, accelerated) {
                (true, false) => {
                    let c = m.caution.unwrap_or_default();
                    assert!(!c.is_empty(), "{} needs a caution here", m.id);
                    // Written for a volunteer: it must name the way it fails, or it
                    // is just a scary noise they will click past.
                    assert!(
                        c.contains("behind") || c.contains("thins out"),
                        "{}: the caution does not describe the failure: {c}",
                        m.id
                    );
                }
                // Nothing else may carry one: a caution on every row is a caution on
                // no row.
                _ => assert!(m.caution.is_none(), "{} should have no caution", m.id),
            }
        }
    }

    /// Every model must be reachable by its own name. A catalogue entry whose
    /// filename the resolver would not pick is a download that changes nothing.
    #[test]
    fn every_catalogued_model_can_actually_be_selected() {
        for m in CATALOG {
            assert!(
                crate::stt::MODEL_CANDIDATES.contains(&m.filename),
                "{} cannot be resolved by name",
                m.id
            );
        }
    }

    #[test]
    fn models_live_in_app_data_never_in_the_repo() {
        // A packaged app has no repo `models/` dir — that assumption is exactly
        // what made the AI unreachable for real users.
        assert!(models_dir().starts_with(crate::db::app_data_dir()));
        assert!(models_dir().ends_with("models"));
    }

    /// THE BRICK. A `.part` of exactly the model's size must never be used as a
    /// resume point.
    ///
    /// It used to be: the guard was `> model.bytes`, so an exactly-full `.part` sent
    /// `Range: bytes=147951465-`, got **416 Range Not Satisfiable**, and hard-errored
    /// WITHOUT deleting the file. Every retry hit the same 416 forever. The only
    /// escape was deleting a file the user did not know existed — which, for a church
    /// volunteer, means the model simply never installs, ever.
    #[test]
    fn a_full_size_part_file_is_verified_never_resumed() {
        let total = CATALOG[0].bytes;
        assert_eq!(resume_plan(total, total), Resume::Verify);
        // ...and anything larger is likewise not a resume point (corrupt/garbage).
        assert_eq!(resume_plan(total + 1, total), Resume::Verify);
    }

    #[test]
    fn a_partial_file_resumes_from_where_it_stopped() {
        let total = CATALOG[0].bytes;
        assert_eq!(resume_plan(90_000_000, total), Resume::From(90_000_000));
        assert_eq!(resume_plan(1, total), Resume::From(1));
    }

    #[test]
    fn nothing_on_disk_starts_from_the_beginning() {
        assert_eq!(resume_plan(0, CATALOG[0].bytes), Resume::Fresh);
    }

    /// A stalled download must be given up on, not waited on forever — but the poll
    /// that notices a cancel has to be much shorter than the stall deadline, or
    /// pressing Cancel appears to do nothing for most of a minute.
    #[test]
    fn cancel_is_noticed_long_before_a_stall_is_declared() {
        assert!(CANCEL_POLL < STALL_TIMEOUT);
        assert!(
            CANCEL_POLL <= Duration::from_secs(1),
            "Cancel must feel instant"
        );
        // And the stall deadline must be generous enough that a slow church
        // connection is not mistaken for a dead one.
        assert!(STALL_TIMEOUT >= Duration::from_secs(30));
    }

    /// `running` must clear however we leave the download — including a panic.
    ///
    /// It used to be a bare `store(false)` after the await, which the infinite hang
    /// never reached. The flag stayed set for the life of the process, so every later
    /// attempt — even after the wifi came back — was refused with "A model download is
    /// already running." A network blip became a dead feature until Relay was
    /// restarted.
    #[test]
    fn the_running_flag_clears_even_if_the_download_panics() {
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();
        let hit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _guard = RunningGuard(flag);
            panic!("network exploded");
        }));
        assert!(hit.is_err());
        assert!(
            !running.load(Ordering::SeqCst),
            "a panicking download left `running` set — every retry would be refused"
        );
    }

    #[test]
    fn the_running_flag_clears_on_a_normal_return() {
        let running = Arc::new(AtomicBool::new(true));
        {
            let _guard = RunningGuard(running.clone());
        }
        assert!(!running.load(Ordering::SeqCst));
    }
}

/// Config invariants that are only discovered at RUNTIME, and would otherwise
/// ship. `cargo test` and `tauri build` both pass on a config that panics the app
/// on startup — a compile is not a boot.
#[cfg(test)]
mod config_boots {
    /// Strip XML comments before asserting on a plist.
    ///
    /// Both of these files explain themselves at length, and those explanations
    /// naturally quote the very keys being asserted on. Without this, a test that
    /// greps the raw text happily matches the COMMENT and passes on a file whose
    /// `<dict>` is empty — the exact class of vacuous test that lets the real bug
    /// through. (It did: the first version of these tests read the prose.)
    fn strip_comments(xml: &str) -> String {
        let mut out = String::with_capacity(xml.len());
        let mut rest = xml;
        while let Some(start) = rest.find("<!--") {
            out.push_str(&rest[..start]);
            match rest[start..].find("-->") {
                Some(end) => rest = &rest[start + end + 3..],
                None => return out, // unterminated comment: nothing real can follow
            }
        }
        out.push_str(rest);
        out
    }

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

    /// THE MICROPHONE, ON THE ONLY BUILD THAT COUNTS.
    ///
    /// Notarization requires the hardened runtime (Tauri enables it by default), and
    /// under the hardened runtime a process that opens an audio input device WITHOUT
    /// the `com.apple.security.device.audio-input` entitlement is killed by TCC.
    ///
    /// Which means:
    ///
    /// ```text
    ///   tauri dev             microphone works    (no hardened runtime)
    ///   unsigned pre-release  microphone works    (ad-hoc signed, no hardened runtime)
    ///   SIGNED + NOTARIZED    microphone DEAD
    /// ```
    ///
    /// The first build correct enough to give to a church is the first one where Relay
    /// cannot hear the preacher — and every build we are able to test locally would
    /// have looked perfect. This cannot be caught by compiling, by `cargo test`, or by
    /// running the app. It can only be caught by an assertion, so here it is.
    #[test]
    fn the_macos_build_can_actually_open_a_microphone() {
        const CONF: &str = include_str!("../tauri.conf.json");
        const ENTITLEMENTS: &str = include_str!("../relay.entitlements");

        let c: serde_json::Value = serde_json::from_str(CONF).expect("tauri.conf.json");
        let mac = &c["bundle"]["macOS"];

        assert_eq!(
            mac["entitlements"].as_str(),
            Some("relay.entitlements"),
            "bundle.macOS.entitlements is not set — a notarized build's microphone is DEAD, \
             and no build you can test locally will show it"
        );
        // The real <dict> — NOT the comment above it, which quotes this same key.
        let plist = strip_comments(ENTITLEMENTS);
        let after = plist
            .split("com.apple.security.device.audio-input")
            .nth(1)
            .expect(
                "the audio-input entitlement is missing — Relay cannot hear anything under \
                 the hardened runtime, which notarization requires",
            );

        // Present-but-`<false/>` is worse than absent: it reads as a deliberate choice.
        let value = after
            .split_whitespace()
            .find(|t| t.starts_with("<true/>") || t.starts_with("<false/>"))
            .unwrap_or("");
        assert!(
            value.starts_with("<true/>"),
            "the audio-input entitlement must be <true/>, found {value:?}"
        );
    }

    /// The permission dialog macOS shows the volunteer.
    ///
    /// Without `NSMicrophoneUsageDescription` the app is not "denied the microphone" —
    /// it is TERMINATED the instant it asks. And the string is not boilerplate: it is
    /// the only explanation a church ever gets for why this software wants to listen to
    /// their service, so it must actually answer that.
    #[test]
    fn the_microphone_permission_dialog_explains_itself_to_a_volunteer() {
        const CONF: &str = include_str!("../tauri.conf.json");
        const PLIST: &str = include_str!("../Info.plist");

        let c: serde_json::Value = serde_json::from_str(CONF).expect("tauri.conf.json");
        assert_eq!(
            c["bundle"]["macOS"]["infoPlist"].as_str(),
            Some("Info.plist"),
            "bundle.macOS.infoPlist is not set"
        );
        // Strip the comments first — they quote this key while explaining it, and a
        // grep of the raw text would match the prose and pass on an empty <dict>.
        let plist = strip_comments(PLIST);

        // A non-empty sentence, not a placeholder. Apple rejects empty/absent strings,
        // and a volunteer deserves better than "Relay needs the microphone."
        let body = plist
            .split("NSMicrophoneUsageDescription")
            .nth(1)
            .expect(
                "no NSMicrophoneUsageDescription — macOS KILLS the app when it asks for the mic",
            )
            .split("<string>")
            .nth(1)
            .and_then(|s| s.split("</string>").next())
            .unwrap_or("")
            .trim()
            .to_string();
        assert!(
            body.len() > 40,
            "the microphone usage string is missing or too thin to explain anything: {body:?}"
        );
        // It must say where the audio goes. That is the question being asked.
        assert!(
            body.to_lowercase().contains("never sent")
                || body.to_lowercase().contains("this computer"),
            "the usage string must say what happens to the audio — it is the one thing \
             a church actually wants to know: {body:?}"
        );
    }
}
