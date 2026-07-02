//! Speech-to-text.
//!
//! Single responsibility: turn AudioChunks into a rolling transcript with
//! per-chunk language identification. Local-first (whisper.cpp-class model),
//! optional cloud fallback when online. Never assumes single-language input —
//! code-switching (English mixed with a local language mid-sentence) is the
//! normal case for the target market. See PROMPT.md Phase 4.
//!
//! Phase 4 scope: local whisper model, English only. A dedicated worker thread
//! owns the (blocking, CPU-heavy) whisper state off every other path. Voiced
//! audio accumulates into a rolling window that is re-transcribed periodically
//! (partial results) and finalized after a run of silence. Language ID and the
//! Yoruba/Swahili/Hausa models come in Phase 10.

use crate::audio::AudioChunk;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const TARGET_RATE: u32 = 16_000; // whisper input rate
const WINDOW_SECS: usize = 8; // max rolling context re-fed to whisper
const STEP_SAMPLES: usize = TARGET_RATE as usize; // re-transcribe every ~1s of new voice
const MIN_SAMPLES: usize = TARGET_RATE as usize / 2; // don't run whisper on <0.5s
/// Consecutive silent chunks that end an utterance. Chunks hop ~200ms, so
/// ~5 ≈ 1s of silence — matches natural sentence pauses.
const SILENCE_FINALIZE: u32 = 5;

/// A transcript update pushed to the UI. `is_final` marks an utterance closed
/// by a silence gap; partials update the same in-progress line.
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptUpdate {
    pub text: String,
    pub language: String,
    pub is_final: bool,
    pub timestamp_ms: u64,
}

/// Owns the whisper worker thread and the channel feeding it audio.
pub struct SttEngine {
    tx: Sender<AudioChunk>,
    handle: Option<JoinHandle<()>>,
    model_path: PathBuf,
}

impl SttEngine {
    /// Load the model at `model_path` and start the worker. Fails if the model
    /// file is missing or unreadable — capture can still run without STT, so
    /// callers treat this as optional (audio-only mode).
    pub fn try_load<F>(model_path: PathBuf, on_update: F) -> Result<Self, String>
    where
        F: Fn(TranscriptUpdate) + Send + 'static,
    {
        if !model_path.exists() {
            return Err(format!("STT model not found: {}", model_path.display()));
        }
        let ctx = WhisperContext::new_with_params(
            &model_path.to_string_lossy(),
            WhisperContextParameters::default(),
        )
        .map_err(|e| format!("failed to load whisper model: {e}"))?;

        let (tx, rx) = mpsc::channel::<AudioChunk>();
        let handle = std::thread::spawn(move || worker(ctx, rx, on_update));
        Ok(SttEngine {
            tx,
            handle: Some(handle),
            model_path,
        })
    }

    /// A sender clone to feed this engine audio chunks from the capture path.
    pub fn sender(&self) -> Sender<AudioChunk> {
        self.tx.clone()
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }
}

impl Drop for SttEngine {
    fn drop(&mut self) {
        // Dropping the canonical sender lets the worker's recv() error out and
        // exit — but only once every clone (held by capture closures) is gone.
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// The worker loop: accumulate voiced audio, emit partials on a cadence, and
/// finalize on silence. Runs on its own thread; whisper's blocking `full()`
/// never touches the audio or UI threads.
fn worker<F>(ctx: WhisperContext, rx: Receiver<AudioChunk>, on_update: F)
where
    F: Fn(TranscriptUpdate) + Send + 'static,
{
    let mut state = match ctx.create_state() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("stt: failed to create whisper state: {e}");
            return;
        }
    };
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(4) as i32;

    let mut window: Vec<f32> = Vec::with_capacity(TARGET_RATE as usize * WINDOW_SECS);
    let max_window = TARGET_RATE as usize * WINDOW_SECS;
    let mut new_since_step = 0usize;
    let mut silence_run = 0u32;
    let mut last_ts_ms = 0u64;

    while let Ok(chunk) = rx.recv() {
        last_ts_ms = chunk.timestamp_ms;
        if chunk.is_voice {
            silence_run = 0;
            let resampled = resample_linear(&chunk.samples, chunk.sample_rate, TARGET_RATE);
            window.extend_from_slice(&resampled);
            if window.len() > max_window {
                let drop = window.len() - max_window;
                window.drain(..drop);
            }
            new_since_step += resampled.len();

            if new_since_step >= STEP_SAMPLES && window.len() >= MIN_SAMPLES {
                if let Some(text) = transcribe(&mut state, &window, threads) {
                    on_update(TranscriptUpdate {
                        text,
                        language: "en".into(),
                        is_final: false,
                        timestamp_ms: last_ts_ms,
                    });
                }
                new_since_step = 0;
            }
        } else {
            silence_run += 1;
            if silence_run == SILENCE_FINALIZE && !window.is_empty() {
                if window.len() >= MIN_SAMPLES {
                    if let Some(text) = transcribe(&mut state, &window, threads) {
                        on_update(TranscriptUpdate {
                            text,
                            language: "en".into(),
                            is_final: true,
                            timestamp_ms: last_ts_ms,
                        });
                    }
                }
                window.clear();
                new_since_step = 0;
            }
        }
    }
}

/// Run whisper over the window, returning trimmed text (None if empty/blank).
fn transcribe(state: &mut whisper_rs::WhisperState, audio: &[f32], threads: i32) -> Option<String> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_n_threads(threads);
    params.set_translate(false);
    params.set_single_segment(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    if state.full(params, audio).is_err() {
        return None;
    }
    let n = state.full_n_segments().unwrap_or(0);
    let mut text = String::new();
    for i in 0..n {
        if let Ok(seg) = state.full_get_segment_text(i) {
            text.push_str(&seg);
        }
    }
    let text = text.trim().to_string();
    if text.is_empty() || text == "[BLANK_AUDIO]" {
        None
    } else {
        Some(text)
    }
}

/// Linear resample to `dst_rate`. Adequate to prove the loop; a windowed-sinc
/// (rubato) is the quality upgrade path and slots in behind this one function.
pub fn resample_linear(input: &[f32], src_rate: u32, dst_rate: u32) -> Vec<f32> {
    if src_rate == dst_rate || input.is_empty() {
        return input.to_vec();
    }
    let ratio = dst_rate as f64 / src_rate as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = input[idx.min(input.len() - 1)];
        let b = input[(idx + 1).min(input.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}

/// Resolve the default model path: RELAY_MODEL_PATH override, then the
/// repo-local dev model (compile-time path), then the per-OS app-data dir.
pub fn default_model_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RELAY_MODEL_PATH") {
        return Some(PathBuf::from(p));
    }
    // Dev: model downloaded to <repo>/models (see README). CARGO_MANIFEST_DIR
    // is <repo>/src-tauri at compile time.
    let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models/ggml-base.en.bin");
    if dev.exists() {
        return Some(dev);
    }
    // Prod: alongside the SQLite DB in the app-data dir.
    if let Some(home) = std::env::var_os("HOME") {
        let p = PathBuf::from(home)
            .join("Library/Application Support/com.relay.app/models/ggml-base.en.bin");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_passthrough_when_rates_match() {
        let sig = vec![0.1, 0.2, 0.3];
        assert_eq!(resample_linear(&sig, 16000, 16000), sig);
    }

    #[test]
    fn resample_halves_length_at_half_rate() {
        // 48k -> 16k is a 1/3 ratio.
        let input: Vec<f32> = (0..300).map(|i| i as f32).collect();
        let out = resample_linear(&input, 48000, 16000);
        assert_eq!(out.len(), 100);
        assert!((out[0] - 0.0).abs() < 1e-3);
    }

    #[test]
    fn resample_interpolates_linearly() {
        // Upsample 1->2: midpoints appear between samples.
        let out = resample_linear(&[0.0, 10.0], 1, 2);
        assert_eq!(out.len(), 4);
        assert!((out[0] - 0.0).abs() < 1e-3);
        assert!((out[1] - 5.0).abs() < 1.0); // ~midpoint
    }

    #[test]
    fn resample_handles_empty() {
        assert_eq!(resample_linear(&[], 48000, 16000), Vec::<f32>::new());
    }
}
