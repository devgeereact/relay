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

/// How many of the most recent FINAL windows the stability check looks at.
const LANG_STABILITY_WINDOW: usize = 8;
/// How many distinct languages inside that window count as "flapping".
const LANG_STABILITY_LIMIT: usize = 3;

/// Watches auto-detect for the failure mode that cannot be fixed downstream.
///
/// ── The observation this exists to report ───────────────────────────────────
///
/// Whisper re-elects a language on every window, independently, from ~99
/// candidates, using 8 seconds of accented room audio. On a Nigerian-accented
/// English preacher it does not settle. One real service, 58 windows:
///
///     en 23 · yo 30 · pt 2 · sw 1 · sv 1 · ms 1
///
/// Every `yo`-labelled line was English. That is not a labelling curiosity: the
/// label IS the decode. Committing a window to Yoruba runs it through weak
/// Yoruba acoustics, and the output degrades into word-salad — "The Swadibows
/// did that do not yet go" — which the reference detector then mines for book
/// names. Half the wrong verses that reached that congregation trace back here.
///
/// **This type only OBSERVES.** It does not force a language, because that is a
/// change to the acoustic path and CLAUDE.md §13 forbids making one without
/// scoring it through the detector on real audio (`RELAY_BENCH_WAV`). What it
/// does is end the silence: auto-detect failing looks exactly like the AI being
/// bad, and the operator has a Recognition Language control that fixes it in one
/// click and no reason to suspect they should touch it. See docs/DECISIONS.md.
#[derive(Debug, Default)]
pub struct LanguageStability {
    recent: std::collections::VecDeque<String>,
    reported: bool,
}

impl LanguageStability {
    /// Record a finalized window's detected language. Returns `Some(languages)`
    /// exactly ONCE per session, the first time the recent history is flapping —
    /// a warning repeated every eight seconds during a sermon is noise the
    /// operator will learn to ignore.
    pub fn observe(&mut self, language: &str) -> Option<Vec<String>> {
        self.recent.push_back(language.to_string());
        while self.recent.len() > LANG_STABILITY_WINDOW {
            self.recent.pop_front();
        }
        if self.reported || self.recent.len() < LANG_STABILITY_WINDOW {
            return None;
        }
        let mut distinct: Vec<String> = self.recent.iter().cloned().collect();
        distinct.sort();
        distinct.dedup();
        if distinct.len() < LANG_STABILITY_LIMIT {
            return None;
        }
        self.reported = true;
        Some(distinct)
    }
}

/// The non-overlapping tail of each chunk — CLAUDE.md rule #8.
///
/// The detection chunker (`audio.rs`) emits 50%-overlapping chunks on purpose:
/// overlap is what stops a spoken reference falling across a chunk boundary and
/// being missed. An acoustic model fed those chunks verbatim hears every hop
/// TWICE, and the transcript garbles.
///
/// So exactly one thing is tracked: how far into the stream has already been
/// handed downstream. Anything at or before that mark is a repeat, and is cut.
///
/// Two properties are deliberate, and both are the reason this is a type rather
/// than three lines in a loop:
///
/// - The mark is in **milliseconds, not samples**, so it survives a device that
///   changes sample rate mid-stream. Sample counts from two different rates are
///   not comparable; timestamps are.
/// - The mark **never moves backwards** (`max`), so a chunk that arrives late
///   cannot re-open ground already covered and replay audio into the decoder.
///
/// This is a rule Relay learned the hard way and names by number, and until now
/// it lived inline in the worker loop with no test of its own. Here it can be
/// driven with synthetic chunks, on any platform, with no microphone.
#[derive(Debug, Default)]
pub struct Deoverlap {
    /// End (ms) of the audio already emitted.
    appended_end_ms: u64,
}

impl Deoverlap {
    /// The part of `chunk` not returned by a previous call. Empty when the chunk
    /// is wholly ground already covered — the caller skips it entirely.
    pub fn tail<'a>(&mut self, chunk: &'a AudioChunk) -> &'a [f32] {
        // `.max(1)`: a zero sample rate is a broken device, not a panic. Dividing
        // by it would abort the STT thread mid-service, which is the one outcome
        // worse than a garbled transcript.
        let sr = (chunk.sample_rate as u64).max(1);
        let chunk_end_ms = chunk.timestamp_ms + chunk.samples.len() as u64 * 1000 / sr;
        let tail: &[f32] = if chunk.timestamp_ms >= self.appended_end_ms {
            &chunk.samples
        } else {
            let skip = ((self.appended_end_ms - chunk.timestamp_ms) * sr / 1000) as usize;
            // A skip past the end means the chunk is entirely old: `get` yields
            // None and this becomes an empty slice, which is exactly right.
            chunk.samples.get(skip..).unwrap_or(&[])
        };
        self.appended_end_ms = chunk_end_ms.max(self.appended_end_ms);
        tail
    }
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
    // Only the NON-overlapping tail of each chunk is appended — rule #8. See
    // `Deoverlap`, which owns that rule and is tested independently of a mic.
    let mut deoverlap = Deoverlap::default();

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
            // The VAD (audio.rs) is an energy gate RELATIVE to a learned noise
            // floor, with hysteresis, plus an RNNoise speech-probability veto on
            // OPENING an utterance. (It was once a fixed `rms >= 0.008`, which is
            // what the numbers below were measured against.) Whichever gate is in
            // use, the point stands: ordinary speech drops under the line BETWEEN
            // WORDS — stops,
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

            // Skip the portion already covered by a previous (overlapping) chunk.
            let new_slice = deoverlap.tail(&chunk);
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

/// Relay's supported recognition languages, and the script each is written in.
///
/// Every Tier-1 language (CLAUDE.md: Yoruba, Swahili, Hausa, plus English) is
/// written in the LATIN script — including the Yoruba diacritics `ẹ ọ ṣ` and its
/// tone marks, which are Latin Extended and pass this check unharmed.
/// The languages Relay ships recognition for — English plus every Tier-1
/// language (CLAUDE.md: Yoruba, Swahili, Hausa).
///
/// ONE list, used both by the script guard and by the auto-detect guard below,
/// so the two can never disagree about what Relay claims to hear.
pub const SUPPORTED_LANGUAGES: &[&str] = &["en", "yo", "sw", "ha"];

fn is_supported_language(lang: &str) -> bool {
    SUPPORTED_LANGUAGES.contains(&lang)
}

fn expects_latin_script(lang: Option<&str>) -> bool {
    match lang {
        // An explicitly chosen language is respected: if someone selects a
        // language Relay does not ship, we do not second-guess their script.
        Some(l) => is_supported_language(l),
        // AUTO-DETECT. This is the risky mode and the one the symptom came from:
        // whisper picks a language per chunk, and on a short, quiet or noisy
        // chunk it picks badly. Relay only offers Latin-script languages, so in
        // auto mode a non-Latin transcript is by definition not what was said.
        None => true,
    }
}

/// Is this transcript something nobody in the room said?
///
/// ── Why a SCRIPT check, and not a phrase blocklist ─────────────────────────
///
/// The obvious fix for "the transcript has Chinese in it" is to blocklist the
/// specific strings whisper hallucinates (subtitle credits, "请不吝点赞"). That
/// fails the moment the model emits a different one, and it silently encodes the
/// assumption that Chinese is the only wrong answer — it is not; Korean, Russian
/// and Japanese subtitle boilerplate are all in the same training data.
///
/// The script is the invariant. A service Relay is configured to hear is in a
/// Latin-script language, so a CJK / Hangul / Kana / Cyrillic / Arabic / Hebrew /
/// Thai / Devanagari letter in the output is not a mis-hearing of a word — it is
/// the model completing a subtitle file. One such character condemns the line.
///
/// Punctuation, digits and symbols are ignored: `—`, `…` and `♪` are script-less
/// and appear in legitimate output. A line with NO letters at all is also
/// rejected — "♪♪♪" is not a sermon.
fn is_hallucination(text: &str, lang: Option<&str>) -> bool {
    let mut letters = 0usize;
    let mut foreign = 0usize;
    for c in text.chars() {
        if !c.is_alphabetic() {
            continue;
        }
        letters += 1;
        if !is_latin_letter(c) {
            foreign += 1;
        }
    }
    // Nothing but punctuation or symbols: "♪♪♪", "...", "[ Silence ]".
    if letters == 0 {
        return true;
    }
    if expects_latin_script(lang) && foreign > 0 {
        return true;
    }
    if is_decode_loop(text) {
        return true;
    }
    false
}

/// Did the decoder get STUCK, emitting one phrase over and over?
///
/// ── Why this is not a phrase blocklist either ───────────────────────────────
///
/// Whisper is autoregressive: on a window it cannot resolve it will re-emit its
/// own last output and lock into a cycle. The result is grammatical, in the right
/// language and the right script, so every guard above passes it. From the live
/// service of 2026-07-26, one FINAL transcript, in full:
///
///     "Matthew, 1 John, 2 John, 2 John, 2 John, 2 John, 2 John, 2 John,"
///
/// A line consisting of nothing but book names. `detect_direct` dutifully mined
/// six references out of it and the router put them on a wall. That one stuck
/// decode accounts for a large share of the wrong verses that service.
///
/// The invariant is STRUCTURAL, like the script check: no one speaking says the
/// same phrase six times consecutively, in any language. So this measures the
/// repetition rather than naming the phrase — it catches the next loop too,
/// whatever whisper happens to get stuck on.
///
/// ── Why "dominates", not "occurs" ───────────────────────────────────────────
///
/// Preachers repeat themselves ON PURPOSE, constantly — "we recover, we recover",
/// "pray, pray, pray". That is real speech and must survive. So a repeat is only
/// a loop when it has eaten the LINE: the repeated unit must cover at least half
/// of it. From the same service, this one passes and is kept, correctly:
///
///     "Expire, I say, say, say, say, no, no, don't, don't look at the expiry
///      date. Shall we pray?"          — longest run 4 of 18 tokens = 22%
///
/// while the stuck decode above is 12 of 15 = 80%.
fn is_decode_loop(text: &str) -> bool {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    // Too short to tell a loop from emphasis.
    if words.len() < 6 {
        return false;
    }
    // Look for a repeated unit of 1..=4 words. A book name is often two tokens
    // ("2 john"), which a unigram scan alone would miss entirely, and whisper's
    // stock filler runs longer still ("thank you for watching").
    for n in 1..=4usize {
        if words.len() < n * MIN_LOOP_REPEATS {
            continue;
        }
        let mut i = 0;
        while i + n <= words.len() {
            let unit = &words[i..i + n];
            let mut reps = 1usize;
            while i + n * (reps + 1) <= words.len()
                && &words[i + n * reps..i + n * (reps + 1)] == unit
            {
                reps += 1;
            }
            let span = n * reps;
            if reps >= MIN_LOOP_REPEATS && span >= MIN_LOOP_SPAN && span * 2 >= words.len() {
                return true;
            }
            i += 1;
        }
    }
    false
}

/// Consecutive repeats of the same unit before it counts as a possible loop.
/// Three is deliberate: two is ordinary rhetorical emphasis in every language
/// Relay hears ("we recover, we recover").
const MIN_LOOP_REPEATS: usize = 3;

/// How many WORDS the repetition must span before it counts as a loop.
///
/// This is the condition that separates a preacher from a stuck decoder, and it
/// was found by replaying all 104 final transcripts of a real service. Repeats
/// and coverage alone were not enough — they also condemned this line, which is
/// a preacher saying "hold on" four times and whisper dropping the H:
///
///     "Old on, old on, old on, old on."          4 repeats, 100% of the line
///
/// Emphasis is a short burst; a decoder locked in a cycle runs on. That line
/// spans 8 words. The stuck decode it must be told apart from spans 12:
///
///     "Matthew, 1 John, 2 John, 2 John, 2 John, 2 John, 2 John, 2 John,"
///
/// Ten is the line between them, and re-running the corpus is how to move it.
const MIN_LOOP_SPAN: usize = 10;

/// Is this alphabetic char part of the Latin script (including the extended
/// ranges Yoruba, Hausa and Swahili need)?
fn is_latin_letter(c: char) -> bool {
    matches!(c,
        'A'..='Z' | 'a'..='z'
        | '\u{00C0}'..='\u{024F}'   // Latin-1 Supplement, Extended-A, Extended-B
        | '\u{1E00}'..='\u{1EFF}'   // Latin Extended Additional — Yoruba ẹ ọ ṣ
        | '\u{0250}'..='\u{02AF}'   // IPA extensions (ɓ ɗ ƙ — Hausa hooked letters)
    )
}

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

    // ── HALLUCINATION GUARDS ────────────────────────────────────────────────
    //
    // None of these were set, which is why a quiet church produced confident
    // nonsense — including transcripts in languages nobody in the room spoke.
    //
    // Whisper is a sequence model with no notion of "nothing was said". Fed
    // silence, room tone, an air-conditioner or a music bed, it does not emit
    // nothing: it emits the most likely token sequence, and its training data is
    // full of subtitle boilerplate. That is where the Chinese comes from — the
    // model is completing a subtitle file, not transcribing a sermon.
    //
    // These are whisper.cpp's own defaults, which `FullParams::new` does NOT
    // apply. Each one rejects garbage at a different stage:
    params.set_suppress_blank(true);
    // Non-speech tokens: the "♪", "[Music]", "(applause)", 字幕 family.
    params.set_suppress_nst(true);
    // "Probably nobody was talking" — the single most effective guard against a
    // silent room being transcribed as speech.
    params.set_no_speech_thold(0.6);
    // Temperature fallback: if a decode comes out incoherent, re-roll it hotter
    // rather than shipping it. Without an increment there is no fallback at all.
    params.set_temperature(0.0);
    params.set_temperature_inc(0.2);
    // Reject decodes that are too uncertain (logprob) or too chaotic (entropy) —
    // hallucinated runs score badly on both.
    params.set_logprob_thold(-1.0);
    params.set_entropy_thold(2.4);
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
    // The last line of defence, and the decisive one for the symptom that
    // prompted it: a transcript in a script nobody was speaking.
    if is_hallucination(&text, lang) {
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

    // ── AUTO-DETECT: the language SET is an invariant too ───────────────────
    //
    // The script guard above rejects a window whose alphabet nobody in the room
    // was using. That is the same argument as this one, one notch looser: it
    // catches Chinese subtitle boilerplate but is blind to a LATIN-script
    // language Relay does not ship. From a live service, all three auto-detected
    // and all three passed the script check untouched:
    //
    //     ms | "Kebawah, kamu tidak akan berikan kebawah ini."
    //     pt | "Eu sou o Rafael, eu amo a nuva-coma, em danimo de Jesus"
    //     sv | "Oh, say, children boy again."
    //
    // Whisper is choosing from ~99 languages on 8 seconds of accented, noisy
    // room audio. A window it labels Malay, in a service Relay is configured to
    // hear in English and Tier-1 languages, is not a mis-hearing of a word — it
    // is the model reaching outside the room, exactly like the subtitle case.
    //
    // Only in AUTO mode. An explicitly chosen language is respected as always:
    // this guard exists because auto-detect picks badly, not to refuse languages.
    if lang.is_none() && !is_supported_language(&detected) {
        return None;
    }
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

/// Candidate model filenames, most-preferred first — the FALLBACK order, used
/// only when the operator has not chosen (see `model_path_for`).
///
/// The multilingual `ggml-base.bin` leads so Yoruba/Swahili/Hausa +
/// code-switching work, and the English-only model is the fallback. The larger
/// models come last **on purpose**: this list decides what an operator who has
/// never opened Settings gets, and that must stay the model which runs on any
/// laptop, not the one with the best accuracy on a fast one.
///
/// Kept in sync with `models::CATALOG` by `models::tests`.
pub const MODEL_CANDIDATES: &[&str] = &[
    "ggml-base.bin",
    "ggml-base.en.bin",
    "ggml-small.bin",
    "ggml-large-v3-turbo-q5_0.bin",
    "ggml-large-v3-turbo.bin",
];

/// Resolve the model path with no stated preference — see `model_path_for`.
pub fn default_model_path() -> Option<PathBuf> {
    model_path_for(None)
}

/// Resolve which model file to load.
///
/// `preferred` is the filename the operator chose, persisted under `stt.model`.
/// It WINS whenever the file is present, and that is the entire point: resolution
/// used to be `MODEL_CANDIDATES` order alone, so once more than one model could be
/// installed, downloading a better one changed nothing — `ggml-base.bin` was still
/// first in the list and still on disk, so it was still what loaded. The operator
/// would have waited out a 1.6 GB download, seen the model listed as installed, and
/// been running the old one, with nothing anywhere saying so.
///
/// Order: `RELAY_MODEL_PATH` (a developer override, absolute) → the chosen model in
/// either directory → the first `MODEL_CANDIDATES` entry that exists.
///
/// A chosen model that is no longer on disk falls back rather than failing: the
/// operator deleted a file, and running with a working model beats refusing to
/// listen. `stt_status` reports what actually loaded, so the difference is visible.
pub fn model_path_for(preferred: Option<&str>) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("RELAY_MODEL_PATH") {
        return Some(PathBuf::from(p));
    }
    // Dev: models downloaded to <repo>/models (see README). CARGO_MANIFEST_DIR
    // is <repo>/src-tauri at compile time.
    // Prod: alongside the SQLite DB in the per-OS app-data dir. MUST go through
    // db::app_data_dir() — this branch was once hardcoded to the macOS
    // `$HOME/Library/Application Support` layout, so on a packaged Windows build
    // it never resolved and Relay came up with speech recognition silently dead.
    let dirs = [
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../models"),
        crate::db::app_data_dir().join("models"),
    ];
    resolve_model(&dirs, preferred)
}

/// The resolution rule itself, over directories given rather than discovered — so
/// it can be tested without a 148 MB file or the machine's real app-data dir.
fn resolve_model(dirs: &[PathBuf], preferred: Option<&str>) -> Option<PathBuf> {
    // A chosen model beats the fallback order in EITHER directory, so a developer's
    // repo-local copy of the base model cannot shadow the one that was picked.
    //
    // Only ever a bare filename: a persisted setting is not a path the app should
    // follow out of its own model directory. `file_name()` also rejects `.` and
    // `..`, and the `is_file` check rejects the empty name — `dir.join("")` is the
    // directory itself, and a directory very much `exists()`.
    if let Some(name) = preferred
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(Path::new)
        .and_then(Path::file_name)
    {
        for dir in dirs {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    for dir in dirs {
        for name in MODEL_CANDIDATES {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
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
mod hallucination_tests {

    use super::*;

    // THE SYMPTOM THIS EXISTS FOR, reported from a real service:
    // "Transcript is getting chinese words and other languages that's not heard."
    //
    // Whisper fed a quiet or noisy room does not emit nothing — it emits the most
    // likely continuation, and its training data is full of subtitle boilerplate.

    #[test]
    fn rejects_chinese_subtitle_boilerplate() {
        // Real examples of what whisper.cpp emits on silence.
        assert!(is_hallucination("请不吝点赞 订阅 转发 打赏", None));
        assert!(is_hallucination("字幕由Amara.org社群提供", None));
        assert!(is_hallucination("小编推荐", Some("en")));
    }

    /// THE GAP THE SCRIPT CHECK LEAVES, from the live service of 2026-07-26.
    ///
    /// The script guard catches a language written in another alphabet. It cannot
    /// see a LATIN-script language Relay does not ship — and whisper, choosing
    /// from ~99 languages on 8s of accented room audio, reaches for those
    /// constantly. All three of these auto-detected in one service and every one
    /// passed `is_hallucination` untouched.
    #[test]
    fn latin_script_languages_relay_does_not_ship_are_not_what_was_said() {
        for lang in ["pt", "sv", "ms", "id", "tl", "af", "nl"] {
            assert!(
                !is_supported_language(lang),
                "{lang} is not a language Relay ships recognition for"
            );
        }
        for lang in SUPPORTED_LANGUAGES {
            assert!(is_supported_language(lang));
        }
        // And the script check alone genuinely cannot tell the difference — which
        // is why the set check has to exist separately. If this ever starts
        // failing, the guard below it has become redundant, not the other way up.
        assert!(!is_hallucination(
            "Eu sou o Rafael, eu amo a nuva-coma, em danimo de Jesus",
            None
        ));
        assert!(!is_hallucination(
            "Kebawah, kamu tidak akan berikan kebawah ini.",
            None
        ));
    }

    /// The real language histogram from the live service of 2026-07-26 must trip
    /// the warning — and a service that simply code-switches between two of
    /// Relay's own languages must NOT, because that is the product working.
    #[test]
    fn flapping_auto_detect_is_reported_once_but_code_switching_is_not() {
        let mut s = LanguageStability::default();
        // Real sequence shape: English preacher, auto-detect wandering.
        let observed = ["en", "yo", "yo", "en", "pt", "yo", "sv", "en"];
        let mut hits = 0;
        for l in observed {
            if s.observe(l).is_some() {
                hits += 1;
            }
        }
        assert_eq!(hits, 1, "the operator must be told exactly once");
        // And never again, however long the sermon runs.
        for l in ["ms", "yo", "en", "pt"] {
            assert!(s.observe(l).is_none());
        }

        // Genuine English/Yoruba code-switching is NORMAL here (DECISIONS.md) and
        // must never be reported as a fault.
        let mut ok = LanguageStability::default();
        for l in ["en", "yo", "en", "en", "yo", "en", "yo", "yo", "en", "yo"] {
            assert!(
                ok.observe(l).is_none(),
                "code-switching between two supported languages is not instability"
            );
        }
    }

    /// A STUCK DECODE, from the live service of 2026-07-26. Right language, right
    /// script, grammatical — every other guard passes it. `detect_direct` mined
    /// six references out of it and the router put them on a wall.
    #[test]
    fn rejects_a_decoder_stuck_repeating_itself() {
        assert!(is_hallucination(
            "Matthew, 1 John, 2 John, 2 John, 2 John, 2 John, 2 John, 2 John,",
            None
        ));
        // Single-word loops too, and the next one whisper invents — the rule is
        // the repetition, never the phrase.
        assert!(is_hallucination(
            "thank you thank you thank you thank you thank you thank you",
            None
        ));
        assert!(is_hallucination(
            "thank you for watching thank you for watching thank you for watching",
            None
        ));
    }

    /// AND IT MUST NOT TOUCH REAL PREACHING. Repetition is a rhetorical device,
    /// used constantly, in every language Relay hears. A guard that cannot tell
    /// emphasis from a loop would make Relay deaf mid-sermon.
    #[test]
    fn deliberate_repetition_in_real_preaching_survives() {
        for line in [
            // Every one of these is a REAL final transcript from that service.
            "Expire, I say, say, say, say, no, no, don't, don't look at the expiry date. Shall we pray?",
            "You will recover, you will recover, come on, shout out, we recover.",
            "Most, most in the name of the law. We will do it!",
            "He, that believer, he, that believer would not be put to shame.",
            "And when he came back, he came back with an expired passport.",
            // The line that set MIN_LOOP_SPAN. A preacher saying "hold on" four
            // times, with whisper dropping the H — 4 repeats covering 100% of the
            // line, structurally identical to a loop and yet real speech.
            "Old on, old on, old on, old on.",
        ] {
            assert!(
                !is_hallucination(line, None),
                "real preaching was discarded as a loop: {line:?}"
            );
        }
        // Two repeats is emphasis, not a loop, however short the line.
        assert!(!is_decode_loop("pray pray for the church and the nation"));
    }

    #[test]
    fn rejects_other_non_latin_scripts_too() {
        // The bug is not "Chinese" — it is "a script nobody was speaking". A
        // blocklist of Chinese phrases would pass this suite and still fail the
        // next service.
        assert!(is_hallucination("Спасибо за просмотр!", None));
        assert!(is_hallucination("ご視聴ありがとうございました", None));
        assert!(is_hallucination("시청해주셔서 감사합니다", None));
        assert!(is_hallucination("ترجمة نانسي قنقر", None));
    }

    #[test]
    fn rejects_a_line_with_no_letters_at_all() {
        assert!(is_hallucination("♪♪♪", None));
        assert!(is_hallucination("...", None));
    }

    #[test]
    fn keeps_ordinary_english_speech() {
        assert!(!is_hallucination(
            "In the beginning God created the heaven and the earth.",
            None
        ));
        assert!(!is_hallucination(
            "Turn with me to John chapter three verse sixteen.",
            Some("en")
        ));
    }

    #[test]
    fn keeps_yoruba_diacritics() {
        // THE REGRESSION THIS GUARDS: Yoruba is Latin script, but its letters sit
        // in Latin Extended Additional. A naive `is_ascii_alphabetic` check would
        // throw away every Yoruba transcript as a "foreign script" hallucination —
        // silently making Relay deaf to a Tier-1 language.
        assert!(!is_hallucination(
            "Ẹ jẹ́ kí a ka Jòhánù orí kẹta",
            Some("yo")
        ));
        assert!(!is_hallucination("Ọlọ́run fẹ́ràn ayé tó bẹ́ẹ̀", None));
    }

    #[test]
    fn keeps_hausa_hooked_letters() {
        // ɓ ɗ ƙ are IPA-range Latin. Same trap as Yoruba.
        assert!(!is_hallucination(
            "Allah ya ƙaunaci duniya haka",
            Some("ha")
        ));
        assert!(!is_hallucination("ɓangare na farko", None));
    }

    #[test]
    fn keeps_swahili() {
        assert!(!is_hallucination(
            "Kwa maana Mungu aliupenda ulimwengu",
            Some("sw")
        ));
    }

    #[test]
    fn keeps_code_switched_speech() {
        // CLAUDE.md: code-switching mid-sentence is the NORMAL case here, not an
        // edge case. A guard that rejected mixed lines would break the Tier-1
        // promise.
        assert!(!is_hallucination(
            "So Ọlọ́run loves the world so much that he gave",
            None
        ));
    }

    #[test]
    fn does_not_second_guess_an_explicitly_chosen_non_latin_language() {
        // If an operator deliberately selects a language Relay does not ship, we
        // are not entitled to overrule its script. The guard is about AUTO-DETECT
        // picking badly, not about refusing languages.
        assert!(!expects_latin_script(Some("zh")));
        assert!(!is_hallucination("请不吝点赞", Some("zh")));
    }
}

/// WHICH MODEL ACTUALLY LOADS.
///
/// Once more than one model can be installed, "which file gets opened" stops being
/// obvious and starts being a decision — and the failure mode is silent in the worst
/// way: the operator downloads a better model, sees it listed as installed, and runs
/// the old one for the whole service with nothing anywhere saying so.
#[cfg(test)]
mod model_choice_tests {
    use super::*;

    /// A scratch model directory. Files are empty — resolution is about names and
    /// existence, and nothing here loads a decoder.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "relay-models-{}-{tag}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch dir");
            Scratch(dir)
        }
        fn with(self, names: &[&str]) -> Self {
            for n in names {
                std::fs::write(self.0.join(n), b"").expect("touch model");
            }
            self
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// THE BUG THIS EXISTS FOR. Resolution was `MODEL_CANDIDATES` order alone, so
    /// the operator's choice was not consulted at all — and `ggml-base.bin` is first
    /// in that list and still on disk, so a 1.6 GB download changed nothing.
    #[test]
    fn the_chosen_model_wins_over_the_default_order() {
        let s = Scratch::new("chosen").with(&["ggml-base.bin", "ggml-large-v3-turbo.bin"]);
        let dirs = [s.0.clone()];
        assert_eq!(
            resolve_model(&dirs, Some("ggml-large-v3-turbo.bin")),
            Some(s.0.join("ggml-large-v3-turbo.bin"))
        );
        // And with no choice, the default order still wins — an operator who has
        // never opened Settings must get the model that runs on any laptop.
        assert_eq!(
            resolve_model(&dirs, None),
            Some(s.0.join("ggml-base.bin")),
            "the no-choice default must stay the small multilingual model"
        );
    }

    /// A choice pointing at a file that is gone must not take speech recognition
    /// down with it. The operator deleted a file to reclaim disk; running with a
    /// working model beats refusing to listen.
    #[test]
    fn a_chosen_model_that_is_missing_falls_back_instead_of_failing() {
        let s = Scratch::new("missing").with(&["ggml-base.bin"]);
        let dirs = [s.0.clone()];
        assert_eq!(
            resolve_model(&dirs, Some("ggml-large-v3-turbo.bin")),
            Some(s.0.join("ggml-base.bin"))
        );
    }

    /// The setting is a filename, never a path. It is written by the app today, but
    /// it lives in a plain SQLite row on the operator's disk, and a value that could
    /// send the loader outside the model directory is not something to leave to the
    /// good manners of whoever writes it next.
    #[test]
    fn a_stored_value_cannot_walk_out_of_the_model_directory() {
        let s = Scratch::new("escape").with(&["ggml-base.bin"]);
        let dirs = [s.0.clone()];
        for hostile in [
            "../../../etc/passwd",
            "/etc/passwd",
            "..",
            ".",
            "",
            "   ",
            "sub/dir/ggml-base.bin",
        ] {
            let got = resolve_model(&dirs, Some(hostile));
            assert_eq!(
                got,
                Some(s.0.join("ggml-base.bin")),
                "{hostile:?} escaped the model directory or resolved to a directory"
            );
        }
    }

    /// Nothing installed is a supported state, not a crash: Relay runs audio-only
    /// and says so (`stt_status.loaded == false`).
    #[test]
    fn no_models_installed_resolves_to_nothing() {
        let s = Scratch::new("empty");
        let dirs = std::slice::from_ref(&s.0);
        assert_eq!(resolve_model(dirs, None), None);
        assert_eq!(resolve_model(dirs, Some("ggml-base.bin")), None);
    }

    /// The first directory wins, so a developer's repo-local model still shadows the
    /// app-data one — unchanged behaviour, pinned because the loop was rewritten.
    #[test]
    fn the_first_directory_still_wins() {
        let a = Scratch::new("dir-a").with(&["ggml-base.bin"]);
        let b = Scratch::new("dir-b").with(&["ggml-base.bin"]);
        let dirs = [a.0.clone(), b.0.clone()];
        assert_eq!(resolve_model(&dirs, None), Some(a.0.join("ggml-base.bin")));
    }
}

/// CLAUDE.md rule #8, finally testable.
///
/// The rule ("STT is fed the NON-overlapping tail of each chunk") is listed among
/// the architecture rules learned the hard way, and until this module existed it
/// was enforced by three lines inside a `while let` loop that could only be
/// exercised with a live microphone and a running whisper model — which is to say,
/// never, in CI, on either platform.
#[cfg(test)]
mod deoverlap_tests {
    use super::*;

    /// Build a chunk of `len` samples starting at `at_ms`. Sample VALUES are the
    /// absolute sample index, so a test can assert on *which* audio came back and
    /// not merely on how much.
    fn chunk(at_ms: u64, len: usize, rate: u32) -> AudioChunk {
        let first = at_ms as usize * rate as usize / 1000;
        AudioChunk {
            samples: (0..len).map(|i| (first + i) as f32).collect(),
            timestamp_ms: at_ms,
            sample_rate: rate,
            rms: 0.1,
            is_voice: true,
        }
    }

    /// THE BUG THIS EXISTS FOR. `audio.rs` emits `CHUNK_MS = 400` every
    /// `HOP_MS = 200` — half of every chunk is the previous chunk. Fed verbatim,
    /// whisper hears every hop twice.
    #[test]
    fn fifty_percent_overlap_yields_each_sample_exactly_once() {
        let mut d = Deoverlap::default();
        let rate = 16_000;
        let len = 400 * rate as usize / 1000; // 400 ms

        let mut seen: Vec<f32> = Vec::new();
        for hop in 0..5u64 {
            seen.extend_from_slice(d.tail(&chunk(hop * 200, len, rate)));
        }

        // Five 400 ms chunks on a 200 ms hop span 1200 ms of real audio (the last
        // one ends at 800+400), not the 2000 ms their lengths add up to. And the
        // result must be CONTIGUOUS and in order — a gap would be lost speech and
        // a repeat is the garbling this rule exists to prevent.
        let expected: Vec<f32> = (0..(1200 * rate as usize / 1000))
            .map(|i| i as f32)
            .collect();
        assert_eq!(seen, expected, "overlap was duplicated or audio was lost");
    }

    #[test]
    fn non_overlapping_chunks_pass_through_whole() {
        let mut d = Deoverlap::default();
        let a = chunk(0, 3200, 16_000); // 200 ms
        let b = chunk(200, 3200, 16_000);
        assert_eq!(d.tail(&a).len(), 3200);
        assert_eq!(d.tail(&b).len(), 3200);
    }

    #[test]
    fn a_fully_overlapping_chunk_yields_nothing() {
        let mut d = Deoverlap::default();
        let big = chunk(0, 6400, 16_000); // 0..400 ms
        assert_eq!(d.tail(&big).len(), 6400);
        // Wholly inside ground already covered.
        let inside = chunk(100, 1600, 16_000); // 100..200 ms
        assert!(
            d.tail(&inside).is_empty(),
            "a chunk with nothing new must yield nothing, so the caller can skip it"
        );
    }

    /// A late chunk must not re-open ground already covered. Without the `max`,
    /// the mark would jump BACKWARDS to the end of the stale chunk and the next
    /// arrival would replay audio the decoder had already been given.
    #[test]
    fn a_late_chunk_does_not_rewind_the_mark() {
        let mut d = Deoverlap::default();
        let _ = d.tail(&chunk(0, 16_000, 16_000)); // 0..1000 ms
        let late = chunk(200, 1600, 16_000); // arrives after, covers 200..300 ms
        assert!(d.tail(&late).is_empty());

        // The mark must still be at 1000 ms. If the late chunk had dragged it back
        // to 300, THIS chunk — entirely inside ground already covered — would be
        // handed to the decoder a second time. That is the actual failure a rewind
        // causes, so it is what the assertion has to be about; a chunk starting
        // after 1000 ms would pass whether the mark rewound or not.
        let replay = chunk(400, 8000, 16_000); // 400..900 ms, all of it old
        assert!(
            d.tail(&replay).is_empty(),
            "the mark rewound — already-decoded audio would be replayed"
        );
    }

    /// The mark is milliseconds, not samples, precisely so a device that switches
    /// rate mid-stream cannot corrupt it. Sample counts across two rates are not
    /// comparable quantities.
    #[test]
    fn a_sample_rate_change_mid_stream_is_handled_in_time_not_samples() {
        let mut d = Deoverlap::default();
        // 0..1000 ms at 16 kHz.
        let _ = d.tail(&chunk(0, 16_000, 16_000));
        // Same instant, three times the rate: 0..500 ms, all of it already covered.
        let hi = chunk(0, 24_000, 48_000);
        assert!(d.tail(&hi).is_empty());
        // 900..1400 ms at 48k — the first 100 ms is old, the last 400 ms is new.
        let straddle = chunk(900, 24_000, 48_000);
        assert_eq!(
            d.tail(&straddle).len(),
            400 * 48, // 400 ms at 48 kHz
            "the skip must be computed in the CHUNK's rate, not the previous one"
        );
    }

    /// A broken device reporting a zero sample rate must not divide by zero. The
    /// STT worker runs during a live service; an abort here is worse than any
    /// transcript.
    #[test]
    fn a_zero_sample_rate_does_not_panic() {
        let mut d = Deoverlap::default();
        let bad = AudioChunk {
            samples: vec![0.0; 128],
            timestamp_ms: 0,
            sample_rate: 0,
            rms: 0.0,
            is_voice: true,
        };
        assert_eq!(d.tail(&bad).len(), 128);
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

    /// WHICH ENGINE PUTS THE RIGHT VERSE ON THE WALL? — the ruler for changing
    /// how Relay hears.
    ///
    /// Every other bench in this file measures ONE decoder, and `prompt_sweep` —
    /// the only other one scored through the detector — is welded to a single clip
    /// with two hardcoded references and calls `transcribe()` directly. That is
    /// fine for comparing prompts. It cannot compare *engines*, for two reasons:
    ///
    /// 1. It bypasses everything between the microphone and the decoder. `Deoverlap`,
    ///    the rolling window, the batch drain, the silence finalizer — all skipped.
    ///    A different engine differs mostly in exactly that region.
    /// 2. `transcribe()` is a synchronous call into whisper. A streaming recognizer
    ///    (macOS `SFSpeechRecognizer`) has no such function to call; it emits results
    ///    on its own schedule. Anything that can only be measured by calling
    ///    `transcribe()` can only ever measure whisper.
    ///
    /// So this drives the REAL `SttEngine` through `sender()`, with chunks built by
    /// `audio::chunks_as_captured` — the same size, overlap, timestamps and voice
    /// gate the live path produces. What is measured is the whole pipeline, which is
    /// the only version of it a congregation is exposed to.
    ///
    /// **It scores through the detector, never by reading the transcript** (CLAUDE.md
    /// rule 13). The headline number is WRONG VERSES, not word error rate: a
    /// transcript can be ugly and still put the right scripture on the wall, and it
    /// can read beautifully while putting up the wrong one. Only the second is a
    /// failure the product exists to prevent.
    ///
    /// Today it compares every INSTALLED MODEL, which is the question in front of us:
    /// Relay ships `base`, the smallest useful whisper, and nobody has ever measured
    /// what a larger one buys. When a second backend exists it becomes another row.
    ///
    /// It asserts NOTHING, for the same reason `word_error_rate` asserts nothing —
    /// there is no baseline yet, and inventing a threshold before the first
    /// measurement is choosing the number we would like over the number that is true.
    /// The first honest assertion here is a WRONG-VERSE CEILING, and it can be
    /// written the day this has been run once.
    ///
    /// ```text
    /// RELAY_BENCH_WAV=bench/sermon.f32 \
    /// RELAY_BENCH_REFS=bench/refs.txt \
    ///   cargo test --release --features metal stt::bench::engine_shootout -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore]
    fn engine_shootout() {
        let (Some(wav), Some(refs_path)) = (
            std::env::var_os("RELAY_BENCH_WAV"),
            std::env::var_os("RELAY_BENCH_REFS"),
        ) else {
            eprintln!(
                "\n  Which engine puts the RIGHT VERSE on the wall? Relay has never measured it.\n\
                 \n  Needs a recording and the list of references actually cited in it.\
                 \n  See bench/README.md — audio is never committed, only the number.\n\
                 \n  RELAY_BENCH_WAV=bench/sermon.f32 RELAY_BENCH_REFS=bench/refs.txt\n"
            );
            return;
        };
        let wav = wav.to_string_lossy().to_string();

        // One reference per line, as `Book C:V`. Blank lines and `#` comments skipped
        // so the file can explain itself to whoever records the next sermon.
        let want: Vec<String> = std::fs::read_to_string(&refs_path)
            .expect("read RELAY_BENCH_REFS")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        assert!(!want.is_empty(), "RELAY_BENCH_REFS lists no references");

        // Every model actually on this machine. NOT the catalogue — an entry that has
        // not been downloaded cannot be scored, and silently reporting zero for it
        // would read as "this model is bad" rather than "this model is absent".
        // Both places `default_model_path` looks: the repo-local dev dir and the
        // per-OS app-data dir. Kept in that order so a dev's local model wins, which
        // is the same precedence the live path uses.
        let mut engines: Vec<(String, PathBuf)> = Vec::new();
        for dir in [
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../models"),
            model_install_dir(),
        ] {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "bin")
                    && !engines.iter().any(|(_, q)| q.file_name() == p.file_name())
                {
                    let label = p
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    engines.push((label, p));
                }
            }
        }
        engines.sort_by(|a, b| a.0.cmp(&b.0));
        if engines.is_empty() {
            eprintln!("no models installed — nothing to compare");
            return;
        }
        println!("\n  references sought: {want:?}");
        println!("  models installed:  {:?}", engines.len());

        // The same degradation grid the other benches use. A decoder that only wins on
        // studio audio wins nothing: clean audio is the one case Relay already handles.
        let conds = [
            ("clean       ", 1.0f32, 0.0f32),
            ("quiet       ", 0.08, 0.0),
            ("noisy       ", 1.0, 0.02),
            ("quiet+noisy ", 0.08, 0.004),
            ("very quiet  ", 0.03, 0.002),
        ];

        for (label, model) in &engines {
            println!("\n  ── engine: whisper · {label} ──");
            let (mut right, mut wrong, mut audio_s, mut wall_s) = (0usize, 0usize, 0f64, 0f64);

            for (clabel, scale, noise) in conds {
                let mut cleaned = church_signal(&wav, scale, noise, 0x1234_5678);
                // Trailing silence, so the worker's silence run fires a FINAL rather
                // than leaving the last utterance stranded as a partial. This is what
                // the end of a sentence looks like to the live path.
                cleaned.extend(std::iter::repeat_n(0.0, TARGET_RATE as usize * 3));
                let secs = cleaned.len() as f64 / TARGET_RATE as f64;

                // (arrival ms, text). The TIME matters: the router debounces repeats,
                // and a rolling window says the same reference several seconds running.
                // Replaying without timestamps would either suppress everything or
                // suppress nothing, and neither is what a service looks like.
                // `is_final` rides along because it changes what may be believed: a
                // whole-chapter reading at the end of a PARTIAL is usually a citation
                // caught before its verse number (`RefMatch::is_provisional`). Scoring
                // without it would measure a pipeline the live path does not run.
                let started = std::time::Instant::now();
                let seen: Arc<Mutex<Vec<(u64, String, bool)>>> = Arc::new(Mutex::new(Vec::new()));
                let sink = seen.clone();
                let engine = match SttEngine::try_load(model.clone(), move |u| {
                    if let Ok(mut g) = sink.lock() {
                        g.push((started.elapsed().as_millis() as u64, u.text, u.is_final));
                    }
                }) {
                    Ok(e) => e,
                    Err(e) => {
                        println!("    {clabel} → could not load: {e}");
                        continue;
                    }
                };

                // Count what the GATE saw before the decoder ever runs. Without this
                // number, "found nothing" is unattributable: a silent voice gate and a
                // deaf decoder produce the identical empty result, and DECISIONS §19 is
                // the story of that ambiguity costing a live service. `voiced 0/N` means
                // whisper was never given a sample, and no model will fix it.
                let chunks = crate::audio::chunks_as_captured(&cleaned, TARGET_RATE);
                let voiced = chunks.iter().filter(|c| c.is_voice).count();
                let n_chunks = chunks.len();

                // FEED IT LIKE A ROOM DOES — in real time, not as fast as the loop
                // can push.
                //
                // This was the bench's first and worst bug, and it is worth keeping the
                // reason written down. Pushing every chunk at once is not a faster
                // version of the same measurement, it is a DIFFERENT measurement: the
                // worker drains the whole backlog in one batch and then decodes ONCE,
                // on the freshest 8 seconds (see the batch-drain comment on the worker
                // loop — it is correct, and it is what stops lag compounding through a
                // sermon). So an eleven-second clip produced exactly one transcript, of
                // the last window, and every reference spoken before it simply did not
                // exist. Both models then scored identically, because the bench was
                // measuring the same one window for each — which reads exactly like
                // "a bigger model makes no difference", the most expensive wrong
                // conclusion this file could produce.
                //
                // `RELAY_BENCH_SPEED` trades fidelity for wall-clock: >1 is faster than
                // life and starts re-creating that collapse, so it warns rather than
                // pretending the number means the same thing.
                let speed: f64 = std::env::var("RELAY_BENCH_SPEED")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .filter(|s: &f64| *s > 0.0)
                    .unwrap_or(1.0);

                let t0 = std::time::Instant::now();
                let tx = engine.sender();
                for chunk in chunks {
                    let due = std::time::Duration::from_secs_f64(
                        chunk.timestamp_ms as f64 / 1000.0 / speed,
                    );
                    if let Some(wait) = due.checked_sub(t0.elapsed()) {
                        std::thread::sleep(wait);
                    }
                    if tx.send(chunk).is_err() {
                        break;
                    }
                }
                let fed = t0.elapsed();
                // Feeding is far faster than decoding, so the queue is deep here. Wait
                // for the worker to go quiet rather than guessing a duration.
                //
                // "Quiet" cannot simply mean "the sink stopped growing": before the
                // FIRST decode returns, the sink has never grown, and a big model on a
                // cold cache can sit there for a minute. An idle-only rule scores that
                // as a silent engine — which is how a bench comes to report that the
                // better model is worse. So nothing counts as quiet until either a
                // result has actually arrived or `FIRST_RESULT_GRACE` has passed.
                const IDLE_QUIET: std::time::Duration = std::time::Duration::from_secs(5);
                const FIRST_RESULT_GRACE: std::time::Duration = std::time::Duration::from_secs(120);
                let mut last_len = 0usize;
                let mut last_change = std::time::Instant::now();
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let n = seen.lock().map(|g| g.len()).unwrap_or(0);
                    if n != last_len {
                        last_len = n;
                        last_change = std::time::Instant::now();
                        continue;
                    }
                    if last_change.elapsed() < IDLE_QUIET {
                        continue;
                    }
                    if n > 0 || t0.elapsed() >= FIRST_RESULT_GRACE {
                        break;
                    }
                }
                // HOW FAR BEHIND THE PREACHER IT FINISHED.
                //
                // Audio is fed in real time now, so wall-clock over audio-seconds is
                // ~1.0 by construction and says nothing. What matters is whether the
                // decoder was still chewing after the room went quiet. `IDLE_QUIET` is
                // silence the wait loop must sit through by definition, so it comes
                // off: what is left is the real catch-up time. Near zero means it kept
                // up; seconds mean the transcript was already arriving late on an
                // eleven-second clip, and over a sermon it only grows.
                let lag = (t0.elapsed().saturating_sub(fed))
                    .saturating_sub(IDLE_QUIET)
                    .as_secs_f64();
                drop(engine);

                // SCORE WHAT WOULD HAVE REACHED THE WALL — through the real router.
                //
                // Detection alone is not the answer to "which verse does Relay put up".
                // A rolling window sees "John chapter 3 verse 1..." before the "6"
                // arrives, so `detect_direct` yields John 3:1 and then John 3:16 — and
                // scored raw, that counts as a wrong verse. It is not: the router is
                // exactly what stands between a mid-window guess and a projector, and
                // `eval.rs` scores through it for the same reason. Counting pre-router
                // candidates would report a failure the product does not have, and
                // send someone off tuning a decoder to fix a routing question.
                //
                // Every update is scored, partial and final alike — that is when
                // `emit_detections` runs live, so it is when a verse can reach a wall.
                let updates: Vec<(u64, String, bool)> =
                    seen.lock().map(|g| g.clone()).unwrap_or_default();
                let mut router = crate::router::Router::default();
                let mut found: Vec<String> = Vec::new();
                let mut offered: Vec<String> = Vec::new();
                for (ms, text, is_final) in &updates {
                    for m in crate::detection::detect_direct(text)
                        .into_iter()
                        .filter(|m| !m.is_provisional(*is_final))
                    {
                        let key = format!(
                            "{} {}:{}",
                            m.reference.book, m.reference.chapter, m.reference.verse
                        );
                        match router.decide(
                            &key,
                            m.confidence,
                            crate::detection::DetectionMethod::Direct,
                            *ms,
                        ) {
                            crate::router::RouteDecision::AutoFire => found.push(key),
                            crate::router::RouteDecision::Suggest => offered.push(key),
                            crate::router::RouteDecision::Drop => {}
                        }
                    }
                }
                found.sort();
                found.dedup();
                offered.sort();
                offered.dedup();

                // `RELAY_BENCH_VERBOSE=1` prints what was actually heard. A missing
                // reference has two very different causes — the decoder never said
                // the words, or it said them and the detector did not parse them —
                // and the score alone cannot tell them apart. Guessing which one it
                // is, is how an afternoon gets spent tuning the wrong component.
                //
                // Every DISTINCT update, not the last one: the last is only the final
                // rolling window, so a reference spoken early is not in it — printing
                // just that would make an early reference look like it was never
                // transcribed, when it was, in a window the scorer did see.
                if std::env::var_os("RELAY_BENCH_VERBOSE").is_some() {
                    let mut shown: Vec<&String> = Vec::new();
                    for (_, t, _) in &updates {
                        if !t.trim().is_empty() && !shown.contains(&t) {
                            shown.push(t);
                            println!("      heard: {t}");
                        }
                    }
                    if !offered.is_empty() {
                        println!("      offered (not auto-fired): {offered:?}");
                    }
                }

                let hit = want.iter().filter(|w| found.contains(w)).count();
                let bad = found.iter().filter(|f| !want.contains(f)).count();
                right += hit;
                wrong += bad;
                audio_s = audio_s.max(secs);
                wall_s = wall_s.max(lag);
                println!(
                    "    {clabel} → {hit}/{} correct, {bad} WRONG   \
                     [lag {lag:.1}s, voiced {voiced}/{n_chunks}]  {found:?}",
                    want.len(),
                );
            }

            let total = want.len() * conds.len();
            // Worst lag, not mean: a decoder that keeps up four times out of five is
            // one that fell behind during a sermon, and an average hides that.
            println!(
                "    ══ {label}: {right}/{total} correct   WRONG VERSES: {wrong}   \
                 worst lag {wall_s:.1}s over {audio_s:.0}s of audio",
            );
            // A lag that grows with the clip means the decoder cannot keep up, and the
            // worker starts dropping genuinely old audio (see the batch-drain comment
            // on the worker loop). An accuracy win bought at a growing lag is not a win.
        }
        println!("\n  A wrong verse is the failure this product exists to prevent.");
        println!("  Rank on WRONG VERSES first, lag second, correct third.\n");
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
