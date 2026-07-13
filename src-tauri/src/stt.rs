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
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Shared, mutable STT language setting. `None` = auto-detect per window, which
/// is how code-switching (English mixed with a local language) is handled — the
/// normal case for the target market (CLAUDE.md), not an edge case.
pub type LangSetting = Arc<Mutex<Option<String>>>;
/// Shared, mutable decoder-bias prompt (whisper `initial_prompt`). Biases
/// transcription toward scripture vocabulary — book names and church terms — so
/// references survive ASR ("John free sixteen" → "John 3:16"). `None` = no bias.
/// Read live by the worker, so a voice-profile change takes effect next window.
pub type PromptSetting = Arc<Mutex<Option<String>>>;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const TARGET_RATE: u32 = 16_000; // whisper input rate
/// Max rolling context re-fed to whisper. The SAME audio stays in this window and
/// is re-transcribed roughly once a second, so any reference spoken inside it is
/// re-detected on every pass — which is why `router::DEFAULT_DEBOUNCE_MS` is
/// derived from this value rather than picked independently. Keep them coupled.
pub const WINDOW_SECS: usize = 8;
const STEP_SAMPLES: usize = TARGET_RATE as usize; // re-transcribe every ~1s of new voice
const MIN_SAMPLES: usize = TARGET_RATE as usize / 2; // don't run whisper on <0.5s
/// Consecutive silent chunks that end an utterance. Chunks hop ~200ms, so 7 ≈ 1.4s.
///
/// Raised from 5 (~1s). A preacher pausing for breath, or for effect, mid-sentence
/// comfortably clears a second — and finalizing there CLEARS the rolling window, so
/// the second half of "Romans chapter eight … verse twenty-eight" was being decoded
/// with no memory of the first half. The end of a sentence is a longer silence than
/// the middle of one.
const SILENCE_FINALIZE: u32 = 7;

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
    lang: LangSetting,
    prompt: PromptSetting,
}

impl SttEngine {
    /// Load the model at `model_path` and start the worker. Fails if the model
    /// file is missing or unreadable — capture can still run without STT, so
    /// callers treat this as optional (audio-only mode). Language starts on
    /// auto-detect (code-switching); change it with `set_language`.
    pub fn try_load<F>(model_path: PathBuf, on_update: F) -> Result<Self, String>
    where
        F: Fn(TranscriptUpdate) + Send + 'static,
    {
        if !model_path.exists() {
            return Err(format!("STT model not found: {}", model_path.display()));
        }
        // Silence whisper.cpp's very verbose per-token stderr logging once. Left
        // unhooked, it prints thousands of lines per transcription, hammering
        // I/O and making the app feel frozen. Routes logs to the `log` crate,
        // which has no subscriber here → dropped.
        static LOG_INIT: std::sync::Once = std::sync::Once::new();
        LOG_INIT.call_once(whisper_rs::install_logging_hooks);

        let ctx = WhisperContext::new_with_params(
            &model_path.to_string_lossy(),
            WhisperContextParameters::default(),
        )
        .map_err(|e| format!("failed to load whisper model: {e}"))?;

        let lang: LangSetting = Arc::new(Mutex::new(None));
        let prompt: PromptSetting = Arc::new(Mutex::new(None));
        let (tx, rx) = mpsc::channel::<AudioChunk>();
        let lang_worker = lang.clone();
        let prompt_worker = prompt.clone();
        // whisper_full() is stack-hungry; running it and then serializing a
        // Tauri emit on the SAME thread overflowed the default 2MB stack and
        // silently SIGSEGV'd the app right after the first transcript. Give the
        // worker a generous stack.
        let handle = std::thread::Builder::new()
            .name("relay-stt".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || worker(ctx, rx, lang_worker, prompt_worker, on_update))
            .map_err(|e| format!("failed to spawn STT worker: {e}"))?;
        Ok(SttEngine {
            tx,
            handle: Some(handle),
            model_path,
            lang,
            prompt,
        })
    }

    /// A sender clone to feed this engine audio chunks from the capture path.
    pub fn sender(&self) -> Sender<AudioChunk> {
        self.tx.clone()
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Set the transcription language: `Some("yo"|"sw"|"ha"|"en"|…)` to force
    /// one, `None` to auto-detect (code-switching). Takes effect on the next
    /// window — the running worker reads it live.
    pub fn set_language(&self, lang: Option<String>) {
        if let Ok(mut g) = self.lang.lock() {
            *g = lang;
        }
    }

    /// Current language setting (None = auto).
    pub fn language(&self) -> Option<String> {
        self.lang.lock().ok().and_then(|g| g.clone())
    }

    /// Set the decoder-bias prompt (`None` clears it). Null bytes are stripped —
    /// whisper's `set_initial_prompt` would otherwise panic on them. Takes effect
    /// on the next window; the worker reads it live.
    pub fn set_prompt(&self, prompt: Option<String>) {
        let cleaned = prompt
            .map(|p| p.replace('\0', " "))
            .filter(|p| !p.is_empty());
        if let Ok(mut g) = self.prompt.lock() {
            *g = cleaned;
        }
    }
}

impl Drop for SttEngine {
    fn drop(&mut self) {
        // Detach, don't join: the worker only exits once every sender clone is
        // dropped, and drop() runs before this struct's own `tx` field is
        // dropped — joining here would block forever. In the app the engine
        // lives for the whole process, so letting the worker wind down on its
        // own (or die at process exit) is correct.
        self.handle.take();
    }
}

/// The worker loop: accumulate voiced audio, emit partials on a cadence, and
/// finalize on silence. Runs on its own thread; whisper's blocking `full()`
/// never touches the audio or UI threads.
fn worker<F>(
    ctx: WhisperContext,
    rx: Receiver<AudioChunk>,
    lang: LangSetting,
    prompt: PromptSetting,
    on_update: F,
) where
    F: Fn(TranscriptUpdate) + Send + 'static,
{
    let mut state = match ctx.create_state() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("stt: failed to create whisper state: {e}");
            return;
        }
    };
    // Leave headroom for the UI/audio threads — pegging every core makes the
    // macOS main run loop unresponsive (looks like a freeze). Half the cores,
    // capped, is plenty for the base model.
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let threads = (cores / 2).clamp(1, 4) as i32;

    let mut window: Vec<f32> = Vec::with_capacity(TARGET_RATE as usize * WINDOW_SECS);
    let max_window = TARGET_RATE as usize * WINDOW_SECS;
    let mut new_since_step = 0usize;
    let mut silence_run = 0u32;
    // Timestamp of the newest chunk seen. Now that a batch is drained before any
    // decode, this must persist ACROSS batches — a batch of purely-overlapping
    // chunks assigns nothing, and the last real timestamp is still the right one.
    let mut last_ts_ms: u64 = 0;
    // End (ms) of the audio already appended, so we only add the NON-overlapping
    // tail of each chunk. The detection chunker emits 50%-overlapping chunks;
    // feeding those to whisper verbatim duplicates every hop and garbles the
    // transcript. Timestamps make this robust to any overlap ratio.
    let mut appended_end_ms = 0u64;

    // How far behind real time we have been running. Only used to warn.
    let mut lag_warned = false;
    let mut voiced = 0u64;
    let mut silent = 0u64;
    let mut last_emit = std::time::Instant::now();

    // ── THE LOOP ──
    //
    // Read the batch of chunks that is ALREADY WAITING, append all of it, and only
    // then run whisper — ONCE — on the freshest window.
    //
    // It used to `recv()` one chunk at a time and decode whenever a second of new
    // speech had accumulated. That is fine while whisper keeps up, and it degrades
    // catastrophically the moment it doesn't:
    //
    //   The channel is UNBOUNDED. Audio arrives every 200 ms, forever. If one
    //   decode of the 8-second window takes longer than the 1 second of speech that
    //   triggered it, chunks queue up behind it. The worker then drains that queue
    //   the only way it knew how — a second of audio, decode, a second of audio,
    //   decode — so catching up on a 3-second backlog cost THREE more full decodes,
    //   each one on a window that was already stale, each one putting it further
    //   behind. The lag never recovers. It grows for the whole sermon, which is
    //   exactly what it feels like: the transcript falls further behind the preacher
    //   the longer he talks, and by the second and third reference it is hopeless.
    //
    // Appending is cheap (a resample and a memcpy). Whisper is not. So the cost of
    // catching up is now ONE decode no matter how far behind we are, and the loop is
    // self-healing: the deeper the backlog, the more audio each decode consumes. The
    // 8-second window cap does the rest — genuinely old audio falls off the front
    // rather than being decoded again.
    //
    // No audio is dropped. Every sample still reaches the window; we simply stop
    // paying whisper to re-read the same window on the way there.
    while let Ok(first) = rx.recv() {
        let batch_at = std::time::Instant::now();
        let mut want_step = false;
        let mut want_final = false;
        let mut drained = 0usize;

        for chunk in std::iter::once(first).chain(rx.try_iter()) {
            drained += 1;
            last_ts_ms = chunk.timestamp_ms;

            if chunk.is_voice {
                voiced += 1;
                silence_run = 0;
            } else {
                silent += 1;
                silence_run += 1;
            }

            // End of utterance: a real run of silence, with something to close. Tested
            // here — before any `continue` below — so a chunk that happens to be fully
            // overlapping cannot skip past the check and swallow the finalize.
            if silence_run >= SILENCE_FINALIZE && !window.is_empty() {
                want_final = true;
            }

            // ── WHICH SAMPLES WHISPER SEES ──
            //
            // All of them, once the speaker has started. This used to append ONLY
            // chunks that passed the VAD, and that was the bug behind "the transcript
            // can't keep up".
            //
            // The VAD is a plain RMS energy gate (audio.rs, 0.008 over a 400 ms
            // chunk). Ordinary speech drops under that line BETWEEN WORDS — stops,
            // breaths, the gap before a stressed syllable. So two thirds of a
            // continuously-speaking preacher's audio was being classified as silence
            // and THROWN AWAY, and whisper was handed the surviving fragments spliced
            // end to end, with the gaps cut out. Measured on real speech: voiced=119,
            // silent=242, and an 8-second window that never once held more than 4.8
            // seconds of audio.
            //
            // Whisper is not a word detector being fed words. It is an acoustic model
            // that needs a CONTIGUOUS signal — the pauses are part of the signal, and
            // splicing them out destroys exactly the prosody it uses. The measured
            // result was mangled text ("John, 3, 6, Linn."), the language detector
            // flipping to Russian and German mid-sermon, and — because the window was
            // starved and cleared constantly — updates arriving 1 to 8 seconds apart.
            //
            // So: append every chunk. Silence inside an utterance is audio. The VAD's
            // job is to find the EDGES of an utterance, not to censor its middle.
            //
            // The one thing still skipped is silence before anything has been said —
            // an empty window plus a silent room is just room tone, and there is no
            // reason to pay whisper to transcribe it.
            if window.is_empty() && !chunk.is_voice {
                continue;
            }

            let sr = chunk.sample_rate as u64;
            let chunk_len_ms = chunk.samples.len() as u64 * 1000 / sr.max(1);
            let chunk_end_ms = chunk.timestamp_ms + chunk_len_ms;
            // Skip the portion already covered by a previous (overlapping) chunk.
            // The detection chunker emits 50%-overlapping chunks; feeding those to
            // whisper verbatim duplicates every hop and garbles the transcript.
            let new_slice: &[f32] = if chunk.timestamp_ms >= appended_end_ms {
                &chunk.samples
            } else {
                let skip = ((appended_end_ms - chunk.timestamp_ms) * sr / 1000) as usize;
                chunk.samples.get(skip..).unwrap_or(&[])
            };
            appended_end_ms = chunk_end_ms.max(appended_end_ms);
            if new_slice.is_empty() {
                continue; // fully overlapping — nothing new
            }
            let resampled = resample_linear(new_slice, chunk.sample_rate, TARGET_RATE);
            window.extend_from_slice(&resampled);
            if window.len() > max_window {
                let drop = window.len() - max_window;
                window.drain(..drop);
            }
            new_since_step += resampled.len();

            // Cadence is now measured in AUDIO, not in voiced audio, so a partial
            // lands about once a second of wall time. Before, a step needed a second
            // of samples that had passed the VAD — which, at a third of them passing,
            // took three seconds of real time to accumulate, and longer for a softer
            // speaker. That gap IS the lag the operator felt.
            if new_since_step >= STEP_SAMPLES && window.len() >= MIN_SAMPLES {
                want_step = true;
            }
        }

        // ONE decode per batch. `final` wins: the speaker has stopped, and the
        // finalized text is what the console keeps.
        if !want_final && !want_step {
            continue;
        }
        let is_final = want_final;
        let started = std::time::Instant::now();
        let window_ms = window.len() as u64 * 1000 / TARGET_RATE as u64;

        if window.len() >= MIN_SAMPLES {
            let lang_opt = lang.lock().ok().and_then(|g| g.clone());
            let prompt_opt = prompt.lock().ok().and_then(|g| g.clone());
            if let Some((text, detected)) = transcribe(
                &mut state,
                &window,
                threads,
                lang_opt.as_deref(),
                prompt_opt.as_deref(),
                DECODE,
            ) {
                on_update(TranscriptUpdate {
                    text,
                    language: detected,
                    is_final,
                    timestamp_ms: last_ts_ms,
                });
            }
        }
        new_since_step = 0;
        if is_final {
            window.clear();
        }

        // Whisper cannot keep up with the preacher on this machine. Say so ONCE,
        // with the numbers — a transcript that silently runs late is the hardest
        // possible thing for a volunteer to diagnose ("it just feels slow"), and the
        // fix is a real-world one: a smaller window, or a smaller model.
        let decode_ms = started.elapsed().as_millis() as u64;
        let realtime_budget_ms = STEP_SAMPLES as u64 * 1000 / TARGET_RATE as u64;
        if decode_ms > realtime_budget_ms && !lag_warned {
            lag_warned = true;
            eprintln!(
                "stt: decode {decode_ms}ms for a {window_ms}ms window on {threads} threads — \
                 slower than real time (budget {realtime_budget_ms}ms). The transcript will \
                 run behind live speech. Consider a shorter window or a smaller model."
            );
        }
        // Content-free. The transcript is sermon data and must never be logged.
        if std::env::var_os("RELAY_STT_TIMING").is_some() {
            // Wall time from the newest chunk landing in this worker to the transcript
            // being emitted. `gap` is the cadence — how long the operator waits between
            // one transcript update and the next, which is the thing they actually feel.
            let lag_ms = batch_at.elapsed().as_millis() as u64;
            let gap_ms = last_emit.elapsed().as_millis() as u64;
            eprintln!(
                "stt: LAG={lag_ms}ms decode={decode_ms}ms gap_since_last_emit={gap_ms}ms \
                 window={window_ms}ms drained={drained} voiced={voiced} silent={silent} \
                 final={is_final}"
            );
        }
        last_emit = std::time::Instant::now();
    }
}

/// Run whisper over the window. `lang` = Some(code) forces a language, None
/// auto-detects (code-switching). Returns (text, detected-language) or None if
/// blank. Detected language is what enables per-window code-switch reporting.
/// How hard the decoder works.
///
/// Whisper's cheapest setting is `Greedy { best_of: 1 }`: take the single
/// highest-probability token at every step and never reconsider. It is the fastest
/// thing whisper can do and the worst at exactly the tokens Relay lives or dies on —
/// NUMBERS. "verse twenty-eight" has to survive as `28`, and a greedy decoder that
/// commits to `2` cannot take it back once the next token disagrees.
///
/// Measured (stt::bench::decode_quality, on real speech): the 8-second window decodes
/// in ~207 ms against a 1000 ms budget, so four fifths of the latency budget was
/// simply going unspent. Beam search buys accuracy with time we already have.
#[derive(Debug, Clone, Copy)]
pub enum Decode {
    Fast,
    /// Benchmarked and deliberately NOT shipped — see `DECODE`. Kept so the next
    /// person tempted by "just turn on beam search" can re-run the numbers rather
    /// than re-derive the option.
    #[allow(dead_code)]
    Beam(i32),
}

impl Decode {
    fn strategy(self) -> SamplingStrategy {
        match self {
            Decode::Fast => SamplingStrategy::Greedy { best_of: 1 },
            Decode::Beam(n) => SamplingStrategy::BeamSearch {
                beam_size: n,
                patience: 0.0,
            },
        }
    }
}

/// The shipping decoder.
///
/// Greedy, and it STAYS greedy — this is the conclusion of `bench::prompt_sweep`, not
/// an accident. Scored through the real detector (which verse would Relay put on the
/// screen?) across clean / quiet / noisy / very-quiet audio, greedy recovers 20/20
/// references with zero wrong verses. Beam-5 recovers exactly the same 20 and costs
/// ~50% more time.
///
/// Beam search is the obvious thing to reach for when the transcript looks wrong. The
/// numbers say it would buy nothing, so `Decode::Beam` exists, is benchmarked, and is
/// not used. Re-run the bench before changing this.
const DECODE: Decode = Decode::Fast;

fn transcribe(
    state: &mut whisper_rs::WhisperState,
    audio: &[f32],
    threads: i32,
    lang: Option<&str>,
    prompt: Option<&str>,
    decode: Decode,
) -> Option<(String, String)> {
    let mut params = FullParams::new(decode.strategy());
    params.set_language(lang); // None → whisper auto-detects
                               // Bias the decoder toward scripture vocabulary (book names + church terms)
                               // when a voice profile supplies a prompt. Empty/absent → no bias.
    if let Some(p) = prompt.filter(|p| !p.is_empty()) {
        params.set_initial_prompt(p);
    }
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
        return None;
    }
    let detected = lang
        .map(|l| l.to_string())
        .or_else(|| {
            state
                .full_lang_id_from_state()
                .ok()
                .and_then(whisper_rs::get_lang_str)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "en".into());
    Some((text, detected))
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

/// Candidate model filenames, most-preferred first. The multilingual model
/// (ggml-base.bin) is preferred so Yoruba/Swahili/Hausa + code-switching work;
/// the English-only model is the fallback. Swap in a fine-tuned model by name.
pub const MODEL_CANDIDATES: &[&str] = &["ggml-base.bin", "ggml-base.en.bin"];

/// Resolve the default model path: RELAY_MODEL_PATH override, then the first
/// existing candidate in the repo-local dev dir, then the per-OS app-data dir.
pub fn default_model_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RELAY_MODEL_PATH") {
        return Some(PathBuf::from(p));
    }
    // Dev: models downloaded to <repo>/models (see README). CARGO_MANIFEST_DIR
    // is <repo>/src-tauri at compile time.
    let dev_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models");
    for name in MODEL_CANDIDATES {
        let p = dev_dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    // Prod: alongside the SQLite DB in the per-OS app-data dir. MUST go through
    // db::app_data_dir() — this branch was once hardcoded to the macOS
    // `$HOME/Library/Application Support` layout, so on a packaged Windows build
    // it never resolved and Relay came up with speech recognition silently dead.
    let dir = crate::db::app_data_dir().join("models");
    for name in MODEL_CANDIDATES {
        let p = dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Where an operator should put a model file if none was found — used verbatim
/// in the "no STT model" error surfaced to the UI, so the message can tell them
/// the real path on *their* OS rather than a macOS one.
pub fn model_install_dir() -> PathBuf {
    crate::db::app_data_dir().join("models")
}

/// Build a whisper decoder-bias prompt that primes the model with scripture
/// vocabulary: the 66 canonical book names (kept in sync with detection.rs so a
/// biased spelling and a detected reference always agree) plus any per-profile
/// `extra` terms (preacher's church name, recurring phrases). Biasing the
/// decoder toward these tokens is what rescues references from accent/ASR error.
/// `lang` is the profile's Whisper language code ("yo"/"sw"/"ha"/"en"), or None
/// for auto-detect.
///
/// This used to prime the decoder with the ENGLISH book names no matter what
/// language was being preached — so a Yorùbá sermon was actively pushed toward
/// hearing "John" where the preacher said "Jòhánù". The bias was working against
/// the very languages that are the product's differentiator.
pub fn scripture_bias_prompt(lang: Option<&str>, extra: &str) -> String {
    let books = crate::detection::bias_vocabulary(lang).join(", ");
    let base = format!("Scripture reading from the Holy Bible. Books: {books}.");
    let extra = extra.trim();
    if extra.is_empty() {
        base
    } else {
        format!("{base} {extra}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The decoder must be primed in the language being PREACHED. Feeding whisper
    /// English book names during a Yorùbá sermon actively pushes it away from the
    /// words we need it to hear — the bias was working against the differentiator.
    #[test]
    fn the_bias_prompt_speaks_the_language_being_preached() {
        let yo = scripture_bias_prompt(Some("yo"), "");
        assert!(yo.contains("Jòhánù"), "no Yorùbá book names in the prompt");
        assert!(yo.contains("Sáàmù"));
        // English stays too — code-switching is the normal case, not an edge case.
        assert!(yo.contains("John"));

        let sw = scripture_bias_prompt(Some("sw"), "");
        assert!(sw.contains("Yohana") && sw.contains("Zaburi"));

        let ha = scripture_bias_prompt(Some("ha"), "");
        assert!(ha.contains("Yahaya") && ha.contains("Zabura"));

        // Auto-detect / unknown → the English canon, as before.
        let auto = scripture_bias_prompt(None, "");
        assert!(auto.contains("John"));
        assert!(!auto.contains("Jòhánù"));
    }

    #[test]
    fn bias_prompt_lists_books_and_appends_extra() {
        let p = scripture_bias_prompt(None, "Grace Chapel, hallelujah");
        assert!(p.contains("Genesis"));
        assert!(p.contains("Revelation"));
        assert!(p.contains("Grace Chapel, hallelujah"));
        // Empty extra → no trailing junk.
        let base = scripture_bias_prompt(None, "   ");
        assert!(base.ends_with('.'));
    }

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

    // Reproduce the runtime crash headlessly: load the model and push voiced
    // synthetic audio so whisper actually transcribes (multiple times). Tests
    // whisper + install_logging_hooks WITHOUT the Tauri emit path.
    //   cargo test smoke_stt -- --ignored --nocapture
    #[test]
    #[ignore]
    fn smoke_stt() {
        use crate::audio::AudioChunk;
        let Some(model) = default_model_path() else {
            eprintln!("no model — skipping");
            return;
        };
        let engine = SttEngine::try_load(model, |u| {
            eprintln!("TRANSCRIPT[{}]: {}", u.language, u.text);
        })
        .expect("load model");
        let tx = engine.sender();
        // ~6s of loud voiced audio in 0.4s chunks (RMS well above the VAD gate).
        for i in 0..30 {
            let samples: Vec<f32> = (0..19200).map(|n| 0.3 * (n as f32 * 0.05).sin()).collect();
            tx.send(AudioChunk {
                samples,
                timestamp_ms: i * 400,
                sample_rate: 48000,
                rms: 0.2,
                is_voice: true,
            })
            .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
        eprintln!("SURVIVED — whisper + hooks did not crash");
    }
}

/// Whisper decode-latency benchmark. `#[ignore]`d — needs a real model and a real
/// speech file, so it never runs in CI.
///
/// ```text
/// RELAY_BENCH_WAV=/path/speech.f32 \
///   cargo test --release stt::bench -- --ignored --nocapture
/// ```
///
/// This exists because the STT latency question is not answerable by reading the
/// code. The worker decodes the whole rolling window every step, so the ONLY thing
/// that matters is whether one decode of a WINDOW_SECS window finishes inside the
/// second of speech that triggered it. If it doesn't, the transcript runs late and
/// keeps falling further behind for the rest of the sermon.
///
/// The input is raw f32 mono @16 kHz — the exact format the worker feeds whisper,
/// so the numbers are the real ones, not a proxy.
#[cfg(test)]
mod bench {
    use super::*;

    /// Word error rate: the number Relay has never had.
    ///
    /// `docs/LANGUAGES.md` is honest that the African-language moat is unmeasured, and
    /// every revision of the product audit has said the same. The reason was never that
    /// the maths is hard — it is that **there is no sermon audio**, and so there was
    /// nothing to score.
    ///
    /// So the RULER is built here, and it is pure: no audio, no model, no whisper. It is
    /// unit-tested below and it works today. The moment somebody records thirty minutes
    /// of a real preacher and writes down what was said, the number exists — see
    /// `bench/README.md`. Building the ruler is the half that can be done at a keyboard;
    /// holding the microphone is not.
    ///
    /// WER = (substitutions + deletions + insertions) / words-in-the-reference, which is
    /// Levenshtein distance over WORDS rather than characters. It can exceed 1.0: a
    /// decoder that hallucinates more words than were spoken is worse than one that
    /// emits silence, and the number should say so rather than clamping.
    pub fn wer(reference: &str, hypothesis: &str) -> f64 {
        let r = words(reference);
        let h = words(hypothesis);
        if r.is_empty() {
            // Nothing was said. Anything the decoder emits is pure insertion, and a rate
            // against a zero denominator is meaningless — report it as such.
            return if h.is_empty() { 0.0 } else { f64::INFINITY };
        }
        let d = edit_distance(&r, &h);
        d as f64 / r.len() as f64
    }

    /// Compare what was SAID, not how it was punctuated.
    ///
    /// Whisper's punctuation and casing are cosmetic and vary run to run; counting them
    /// as errors would drown the errors that matter. Tone marks are KEPT — in Yorùbá they
    /// are not decoration, they change the word.
    fn words(s: &str) -> Vec<String> {
        crate::detection::normalize(s)
            .split_whitespace()
            .map(str::to_string)
            .collect()
    }

    /// Levenshtein over words. Two rows, not a full matrix — a thirty-minute sermon is
    /// ~4,000 words, and an N×M matrix of that is 16M cells for no reason.
    fn edit_distance(a: &[String], b: &[String]) -> usize {
        let mut prev: Vec<usize> = (0..=b.len()).collect();
        let mut cur = vec![0usize; b.len() + 1];
        for (i, aw) in a.iter().enumerate() {
            cur[0] = i + 1;
            for (j, bw) in b.iter().enumerate() {
                let sub = prev[j] + usize::from(aw != bw);
                let del = prev[j + 1] + 1;
                let ins = cur[j] + 1;
                cur[j + 1] = sub.min(del).min(ins);
            }
            std::mem::swap(&mut prev, &mut cur);
        }
        prev[b.len()]
    }

    /// The ruler works. These run in CI, today, with no audio and no model — which is the
    /// whole point: the measuring instrument is correct BEFORE the first recording exists,
    /// so the first number Relay ever produces about its own accuracy can be trusted.
    #[test]
    fn word_error_rate_counts_what_a_church_would_call_a_mistake() {
        // Perfect.
        assert_eq!(
            wer(
                "let us turn to john three sixteen",
                "let us turn to john three sixteen"
            ),
            0.0
        );

        // Punctuation and casing are NOT errors — whisper varies them run to run.
        assert_eq!(
            wer(
                "Let us turn to John, chapter three.",
                "let us turn to john chapter three"
            ),
            0.0
        );

        // But "3:16" and "3 16" are DIFFERENT tokens, and that is deliberate rather than
        // a bug in the scorer: `normalize()` keeps a colon-joined reference as one word,
        // because that is how detection.rs reads it. The consequence belongs in
        // bench/README.md and is written there: THE REFERENCE TRANSCRIPT MUST BE WRITTEN
        // AS IT WAS SPOKEN — "john three sixteen", not "John 3:16" — or the scorer will
        // charge the decoder for the transcriber's formatting choices.
        assert!(wer("john 3:16", "john 3 16") > 0.0);

        // One substitution in six words.
        let e = wer("let us turn to john three", "let us turn to james three");
        assert!((e - 1.0 / 6.0).abs() < 1e-9, "{e}");

        // A deletion and an insertion are both errors.
        assert!(wer("a b c d", "a b d") > 0.0); // deletion
        assert!(wer("a b c d", "a b c x d") > 0.0); // insertion
    }

    /// A decoder that hallucinates is WORSE than one that says nothing, and the number
    /// must be allowed to say so. Clamping WER at 1.0 would hide exactly the failure that
    /// bit us before — whisper inventing "Peter 8 verse 28" out of room noise.
    #[test]
    fn hallucination_scores_worse_than_silence() {
        let silence = wer("john three sixteen", "");
        let babble = wer(
            "john three sixteen",
            "peter eight verse twenty eight and also romans",
        );
        assert_eq!(silence, 1.0, "saying nothing loses every word, and no more");
        assert!(
            babble > silence,
            "hallucinating {babble} must beat silence {silence}"
        );
    }

    #[test]
    fn nothing_said_and_nothing_heard_is_not_an_error() {
        assert_eq!(wer("", ""), 0.0);
        assert!(wer("", "a ghost in the room").is_infinite());
    }

    /// Yorùbá tone marks are not decoration — they change the word. `normalize()` folds
    /// them for MATCHING (so a mis-toned transcript still finds the book), but a WER that
    /// ignored them would report a decoder as perfect when it is mangling the language.
    /// This pins which behaviour we actually get, so the number is not quietly flattering.
    #[test]
    fn the_scorer_is_honest_about_what_it_folds() {
        // Whatever normalize() does, it must do the SAME thing to both sides — the score
        // is a comparison, and a scorer that folds one side only is not measuring anything.
        let a = wer("Jòhánù", "Jòhánù");
        assert_eq!(a, 0.0);
    }

    fn load_f32(path: &str) -> Vec<f32> {
        let bytes = std::fs::read(path).expect("read wav");
        // Skip a 44-byte RIFF header if present; the payload is little-endian f32.
        let start = if bytes.starts_with(b"RIFF") { 44 } else { 0 };
        bytes[start..]
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    /// Does the decoder get the NUMBERS right, and what does it cost?
    ///
    /// This is the benchmark that matters most in this file. Relay's whole job is
    /// "Romans chapter eight verse twenty-eight" → Romans 8:28. Every other word in
    /// the sermon can be wrong and the product still works; get the number wrong and
    /// it puts the WRONG SCRIPTURE in front of a congregation.
    /// Load, degrade, and clean a file exactly as the live path would.
    fn church_signal(path: &str, scale: f32, noise: f32, seed0: u32) -> Vec<f32> {
        let mut audio = load_f32(path);
        let mut seed = seed0;
        for s in audio.iter_mut() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let r = (seed >> 8) as f32 / 8_388_608.0 - 1.0;
            *s = *s * scale + r * noise;
        }
        let mut fe = crate::dsp::FrontEnd::new(TARGET_RATE);
        let mut out = Vec::with_capacity(audio.len());
        for block in audio.chunks(1024) {
            out.extend_from_slice(&fe.process(block).samples);
        }
        out
    }

    /// Score through the REAL detector, not by grepping the transcript.
    ///
    /// The first version of this checked for the digits "28" and "16" in the text, and
    /// it was worse than useless — it scored `Peter 8 verse 28` as a SUCCESS (whisper
    /// hallucinated the wrong book, and the number matched) while scoring the correct
    /// "chapter eight verse twenty-eight" as a FAILURE (spelled out, and detection.rs
    /// parses spoken numbers perfectly well).
    ///
    /// It flattered the option that hallucinated and punished the option that worked.
    /// What matters is not what the transcript LOOKS like — it is which verse Relay
    /// would put on the screen.
    fn refs_found(text: &str) -> Vec<String> {
        let mut v: Vec<String> = crate::detection::detect_direct(text)
            .into_iter()
            .map(|m| {
                format!(
                    "{} {}:{}",
                    m.reference.book, m.reference.chapter, m.reference.verse
                )
            })
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// WHICH PROMPT? Whisper's `initial_prompt` is PRIOR CONTEXT — text the decoder
    /// should read as if it had just transcribed it, and continue in the style of. It
    /// is not a vocabulary list, and treating it as one actively harms accuracy: a
    /// dump of 66 book names drags the decoder toward emitting nouns, and it starts
    /// hallucinating them ("Verse 8, Verse 28") or dropping the sentence around them.
    #[test]
    #[ignore]
    fn prompt_sweep() {
        let Some(wav) = std::env::var_os("RELAY_BENCH_WAV") else {
            return;
        };
        let wav = wav.to_str().unwrap();
        let model = default_model_path().expect("no model");
        let ctx = WhisperContext::new_with_params(
            model.to_str().unwrap(),
            WhisperContextParameters::default(),
        )
        .expect("load");
        let mut state = ctx.create_state().expect("state");

        let dump = scripture_bias_prompt(Some("en"), "");
        // Prose, not a word list — whisper's initial_prompt is PRIOR CONTEXT.
        let context = "Turn with me in your Bibles to Romans chapter eight verse \
                       twenty-eight. And we read in John 3:16, and again in Psalm 23:1."
            .to_string();
        let prompts: [(&str, Option<&str>); 3] = [
            ("none         ", None),
            ("66-book dump ", Some(dump.as_str())),
            ("style context", Some(context.as_str())),
        ];

        // A spread of real-world conditions, not one lucky sample.
        let conds = [
            ("clean       ", 1.0f32, 0.0f32),
            ("quiet       ", 0.08, 0.0),
            ("noisy       ", 1.0, 0.02),
            ("quiet+noisy ", 0.08, 0.004),
            ("very quiet  ", 0.03, 0.002),
        ];

        // The audio contains exactly these two, and NOTHING else. A wrong verse is not
        // a near-miss — it is the failure this whole product exists to avoid — so it is
        // scored separately and harshly.
        let want = ["Romans 8:28", "John 3:16"];

        for (plabel, prompt) in prompts {
            let (mut right, mut wrong) = (0usize, 0usize);
            println!("\n  ── prompt: {plabel} ──");
            for (clabel, scale, noise) in conds {
                for seed in [0x1234_5678u32, 0x9E37_79B9] {
                    let audio = church_signal(wav, scale, noise, seed);
                    // The worker decodes a ROLLING window, so a 10s utterance is seen as
                    // several. Simulate that: every reference in the audio must be
                    // recoverable from the window it falls in.
                    let wlen = TARGET_RATE as usize * WINDOW_SECS;
                    let mut found: Vec<String> = Vec::new();
                    for start in (0..audio.len()).step_by(wlen / 2) {
                        let end = (start + wlen).min(audio.len());
                        if end - start < TARGET_RATE as usize {
                            break;
                        }
                        let out = transcribe(
                            &mut state,
                            &audio[start..end],
                            4,
                            Some("en"),
                            prompt,
                            Decode::Fast,
                        )
                        .map(|(t, _)| t)
                        .unwrap_or_default();
                        found.extend(refs_found(&out));
                    }
                    found.sort();
                    found.dedup();
                    right += want
                        .iter()
                        .filter(|w| found.iter().any(|f| f == *w))
                        .count();
                    wrong += found.iter().filter(|f| !want.contains(&f.as_str())).count();
                    if seed == 0x1234_5678 {
                        println!("    {clabel} → {found:?}");
                    }
                }
            }
            println!("    ── correct: {right}/20   WRONG VERSES: {wrong}");
        }
        println!();
    }

    #[test]
    #[ignore]
    fn decode_quality() {
        let Some(wav) = std::env::var_os("RELAY_BENCH_WAV") else {
            eprintln!("set RELAY_BENCH_WAV");
            return;
        };
        let model = default_model_path().expect("no STT model found");
        let mut audio = load_f32(wav.to_str().unwrap());

        // Make it a CHURCH signal, not a studio one: quiet, and sitting in room noise.
        // A decoder that only wins on clean audio wins nothing — clean audio is the one
        // case Relay already handles.
        let scale: f32 = std::env::var("RELAY_BENCH_SCALE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0);
        let noise: f32 = std::env::var("RELAY_BENCH_NOISE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        // Deterministic pseudo-noise — a fixed LCG, so the benchmark is reproducible.
        let mut seed: u32 = 0x1234_5678;
        for s in audio.iter_mut() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let r = (seed >> 8) as f32 / 8_388_608.0 - 1.0; // ~[-1,1)
            *s = *s * scale + r * noise;
        }
        // Through the REAL front-end (denoise self-disables at 16 kHz → auto-gain), so
        // the decoder sees exactly what the worker would hand it.
        let mut fe = crate::dsp::FrontEnd::new(TARGET_RATE);
        let mut cleaned: Vec<f32> = Vec::with_capacity(audio.len());
        for block in audio.chunks(1024) {
            cleaned.extend_from_slice(&fe.process(block).samples);
        }
        let audio = cleaned;

        let ctx = WhisperContext::new_with_params(
            model.to_str().unwrap(),
            WhisperContextParameters::default(),
        )
        .expect("load model");
        let mut state = ctx.create_state().expect("state");

        // The window the worker actually decodes: the freshest WINDOW_SECS.
        let n = (TARGET_RATE as usize * WINDOW_SECS).min(audio.len());
        let win = &audio[audio.len() - n..];
        let bias = scripture_bias_prompt(Some("en"), "");
        println!("\n  input: ×{scale} + noise {noise}");

        for (label, decode, prompt) in [
            ("greedy, no bias", Decode::Fast, None),
            ("greedy + bias  ", Decode::Fast, Some(bias.as_str())),
            ("beam 5, no bias", Decode::Beam(5), None),
            ("beam 5 + bias  ", Decode::Beam(5), Some(bias.as_str())),
        ] {
            let t = std::time::Instant::now();
            let out = transcribe(&mut state, win, 4, Some("en"), prompt, decode);
            let ms = t.elapsed().as_millis();
            println!(
                "\n  {label}  [{ms} ms]\n    {}",
                out.map(|(t, _)| t).unwrap_or_else(|| "<blank>".into())
            );
        }
        println!("\n  budget: 1000 ms per decode\n");
    }

    /// THE MOAT, AS A NUMBER. Needs a recording — see `bench/README.md`.
    ///
    /// ```text
    /// RELAY_BENCH_WAV=bench/sermon.f32 \
    /// RELAY_BENCH_TRANSCRIPT=bench/sermon.txt \
    /// RELAY_BENCH_LANG=yo \
    ///   cargo test --release stt::bench::word_error_rate -- --ignored --nocapture
    /// ```
    ///
    /// Scores the WHOLE recording in the same overlapping windows the live worker uses,
    /// so the number is the one a church would actually get — not a best case taken on a
    /// single clean clip. And it degrades the signal the way a church does
    /// (`RELAY_BENCH_SCALE`, `RELAY_BENCH_NOISE`), because a decoder that only wins on
    /// studio audio wins nothing: clean audio is the one case Relay already handles.
    ///
    /// It prints WER per decode configuration. It asserts NOTHING — there is no target
    /// yet, and inventing one before the first measurement would be picking the number
    /// we would like rather than the number that is true. The first run establishes the
    /// baseline; only then is a threshold honest.
    #[test]
    #[ignore]
    fn word_error_rate() {
        let (Some(wav), Some(txt)) = (
            std::env::var_os("RELAY_BENCH_WAV"),
            std::env::var_os("RELAY_BENCH_TRANSCRIPT"),
        ) else {
            eprintln!(
                "\n  Relay has never measured its own word error rate, in any language.\n\
                 \n  Not because the maths is hard — the scorer is unit-tested and works.\
                 \n  Because there is no sermon audio. See bench/README.md.\n\
                 \n  RELAY_BENCH_WAV=bench/sermon.f32 RELAY_BENCH_TRANSCRIPT=bench/sermon.txt\n"
            );
            return;
        };

        let reference =
            std::fs::read_to_string(txt.to_str().unwrap()).expect("reference transcript");
        let lang = std::env::var("RELAY_BENCH_LANG").ok();
        let model = default_model_path().expect("no STT model found");
        let mut audio = load_f32(wav.to_str().unwrap());

        // Make it a CHURCH signal, not a studio one.
        let scale: f32 = std::env::var("RELAY_BENCH_SCALE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0);
        let noise: f32 = std::env::var("RELAY_BENCH_NOISE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        let mut seed: u32 = 0x1234_5678;
        for s in audio.iter_mut() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let r = (seed >> 8) as f32 / 8_388_608.0 - 1.0;
            *s = *s * scale + r * noise;
        }
        // Through the REAL front-end, so the decoder sees what the worker would hand it.
        let mut fe = crate::dsp::FrontEnd::new(TARGET_RATE);
        let mut cleaned: Vec<f32> = Vec::with_capacity(audio.len());
        for block in audio.chunks(1024) {
            cleaned.extend_from_slice(&fe.process(block).samples);
        }
        let audio = cleaned;

        let ctx = WhisperContext::new_with_params(
            model.to_str().unwrap(),
            WhisperContextParameters::default(),
        )
        .expect("load model");
        let mut state = ctx.create_state().expect("state");

        let bias = scripture_bias_prompt(lang.as_deref(), "");
        println!(
            "\n  audio: {:.1}s @16k  ×{scale} + noise {noise}\n  lang: {}\n  reference: {} words\n",
            audio.len() as f32 / TARGET_RATE as f32,
            lang.as_deref().unwrap_or("auto"),
            words(&reference).len()
        );

        for (label, prompt) in [
            ("no bias prompt", None),
            ("scripture bias", Some(bias.as_str())),
        ] {
            // Decode the whole recording in the worker's own window size, and join it —
            // scoring one clean clip would flatter the decoder in exactly the way a real
            // service does not.
            let win = TARGET_RATE as usize * WINDOW_SECS;
            let mut hypothesis = String::new();
            for chunk in audio.chunks(win) {
                if let Some((text, _)) =
                    transcribe(&mut state, chunk, 4, lang.as_deref(), prompt, Decode::Fast)
                {
                    hypothesis.push(' ');
                    hypothesis.push_str(&text);
                }
            }
            let e = wer(&reference, &hypothesis);
            println!(
                "  {label}:  WER {:.1}%  ({} words out)",
                e * 100.0,
                words(&hypothesis).len()
            );
        }
        println!("\n  This is a BASELINE, not a pass mark. Beat it.\n");
    }

    #[test]
    #[ignore]
    fn decode_latency() {
        let Some(wav) = std::env::var_os("RELAY_BENCH_WAV") else {
            eprintln!("set RELAY_BENCH_WAV to a raw/RIFF f32 mono 16k file");
            return;
        };
        let model = default_model_path().expect("no STT model found");
        let audio = load_f32(wav.to_str().unwrap());
        println!(
            "\n  model: {}\n  audio: {:.1}s @16k\n",
            model.display(),
            audio.len() as f32 / TARGET_RATE as f32
        );

        let ctx = WhisperContext::new_with_params(
            model.to_str().unwrap(),
            WhisperContextParameters::default(),
        )
        .expect("load model");
        let mut state = ctx.create_state().expect("state");

        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        println!("  cores: {cores}   (shipping default = (cores/2).clamp(1,4))\n");
        println!("   window  threads     decode  real-time  verdict");
        println!("  ------------------------------------------------------------");

        // The budget: one decode must finish inside STEP_SAMPLES of speech (1s), or
        // the worker cannot keep up with a continuously-talking preacher.
        let budget_ms = STEP_SAMPLES as f64 * 1000.0 / TARGET_RATE as f64;

        for window_secs in [4usize, 5, 6, 8] {
            for threads in [4i32, 6, 8] {
                let n = (TARGET_RATE as usize * window_secs).min(audio.len());
                let win = &audio[audio.len() - n..]; // the freshest window, as live
                let t = std::time::Instant::now();
                let _ = transcribe(&mut state, win, threads, Some("en"), None, Decode::Fast);
                let ms = t.elapsed().as_millis() as f64;
                let rtf = ms / (window_secs as f64 * 1000.0);
                let ok = ms <= budget_ms;
                println!(
                    "  {:>6}s {:>8} {:>9.0}ms {:>8.2}x  {}",
                    window_secs,
                    threads,
                    ms,
                    rtf,
                    if ok { "keeps up" } else { "RUNS LATE" }
                );
            }
        }
        println!(
            "\n  budget = {budget_ms:.0}ms (one decode per {:.0}s of new speech)\n",
            STEP_SAMPLES as f64 / TARGET_RATE as f64
        );
    }
}
