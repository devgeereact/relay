//! Audio capture + voice-activity-detection gate.
//!
//! Single responsibility: turn a raw microphone/mixer input into a stream of
//! 200-500ms overlapping audio chunks, with silence already filtered out by
//! VAD. This module knows nothing about transcription or detection — it only
//! hands clean audio chunks upstream. See PROMPT.md Phase 3.
//!
//! Design: one dedicated capture thread owns the (non-Send) cpal stream. The
//! stream's realtime callback does the minimum — downmix to mono, forward
//! samples over a channel — and the same thread runs the DSP front-end +
//! chunking + VAD off the realtime path. Nothing here `unwrap()`s on a running
//! path (CLAUDE.md); a device failure surfaces to the caller as an error string.
//!
//! The captured stream is cleaned by `dsp::FrontEnd` (noise suppression +
//! auto-gain) before chunking, so VAD and STT see cleaner audio. Capture prefers
//! a 48 kHz config so the RNNoise denoiser runs frame-aligned (see dsp.rs).

use crate::dsp;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// A voiced, time-stamped chunk of mono audio handed upstream to STT.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    /// Position in the AUDIO stream (ms). Not a wall clock — see `received_at_us`.
    pub timestamp_ms: u64,
    pub sample_rate: u32,
    pub rms: f32,
    pub is_voice: bool,
    /// WALL time this chunk left the capture callback (monotonic microseconds,
    /// `latency::now_us`). The two clocks are both needed and are not
    /// interchangeable: `timestamp_ms` says where in the sermon this audio is,
    /// and this says when the machine got it. Every latency this pipeline is
    /// judged on is a difference of wall times, and the audio clock cannot
    /// answer it — under load the STT worker consumes its backlog in one pass,
    /// so audio time jumps seconds while a fraction of a second of real time
    /// passes (the same trap `main::router_clock_ms` documents).
    pub received_at_us: u64,
}

/// An available capture device, shaped for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
}

// --- chunking parameters (see docs/SPEC.md §4 step 1: 200-500ms overlapping) ---
pub const CHUNK_MS: u32 = 400;
pub const HOP_MS: u32 = 200; // 50% overlap

/// How many peak readings one chunk is described by, for the console's waveform.
///
/// ── WHY THERE IS AN ENVELOPE AT ALL ────────────────────────────────────────
///
/// The console drew ONE rms number per delivered chunk, and `start_capture`
/// delivered every third one, so the "waveform" was a reading every ~600 ms
/// joined up with straight lines. At a normal speaking rate that is about one
/// point per word. It is a level history, and it was drawn and labelled as a
/// waveform, which is the operator's complaint of 2026-09-20: *"this is not
/// giving an original live audio wave"*. They were right, and the fault was
/// never in the drawing.
///
/// SIXTEEN over 400 ms is 25 ms a reading, which is inside a syllable, and it
/// costs sixteen floats on an event that already carries a struct. The event
/// RATE is what freezes a webview, and that is governed separately (see
/// `main.rs::start_capture`, which now sends every SECOND chunk so the readings
/// cover the timeline exactly once at 50% overlap, rather than every third,
/// which left two thirds of the audio undrawn).
pub const CHUNK_PEAKS: usize = 16;

/// The loudest sample in each of `n` equal slices of `samples`.
///
/// PEAK, not rms. An rms over 25 ms is already a smoothing, and smoothing twice
/// is what produced a picture with no transients in it: a consonant, a plosive
/// and a tap on the microphone all read as a gentle rise. The peak is the
/// measurement a meter is expected to show, and it is the one that makes
/// clipping visible at all.
///
/// Absolute value, so the envelope is drawn symmetrically about the centre line
/// the way every audio tool draws one. A slice with no samples in it reads 0.0,
/// which is a real answer: there was nothing there.
pub fn envelope(samples: &[f32], n: usize) -> Vec<f32> {
    if n == 0 {
        return Vec::new();
    }
    (0..n)
        .map(|i| {
            let a = samples.len() * i / n;
            let b = samples.len() * (i + 1) / n;
            samples[a..b].iter().fold(0.0f32, |m, v| m.max(v.abs()))
        })
        .collect()
}
/// ABSOLUTE floor for the voice gate, on f32 samples in [-1, 1]. This is NOT the
/// speech threshold — the real threshold is learned from the room's noise floor (see
/// `Vad`). This only stops a dead or unplugged microphone from having its own dither
/// tracked down to zero and then reported as speech.
///
/// It was previously the speech threshold itself, and it silently deleted most of a
/// quiet preacher's sermon — see the doc comment on `Vad`.
const VAD_RMS_THRESHOLD: f32 = 0.0015;

/// WHICH MICROPHONE A SERVICE ACTUALLY USED (RG-122).
///
/// Every audit before this one had to take the operator's word for it. All the
/// startup log said was `audio: capture @ 48000 Hz · denoise on (RNNoise)`, and on
/// 2026-09-06 both candidate inputs — a laptop microphone and a Blackmagic desk
/// feed — run at 48 kHz, so no instrument in the run could tell them apart. That
/// invalidates the most important caveat a field audit carries: comparing detection
/// accuracy between two services is comparing two unknown microphones.
///
/// `was_default` is the second half and it is not decoration. RG-121 is that the
/// selected device was never persisted, so a launch could silently fall back to the
/// system default; "which device" and "was that the one anybody chose" are
/// different questions and a record needs both.
///
/// Hardware, never content: a device name is not anything a preacher said, so this
/// is safe for the log and for the diagnostic bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub name: String,
    pub was_default: bool,
}

/// The last input capture actually opened, or `None` before the first capture of
/// this run. An ABSENCE, not a guess at the default: a report that named a device
/// Relay had never opened would be exactly the false confidence this exists to end.
static LAST_INPUT: Mutex<Option<Input>> = Mutex::new(None);

/// What capture last opened. `None` until it has opened something.
pub fn last_input() -> Option<Input> {
    LAST_INPUT.lock().ok()?.clone()
}

/// One phrase for a log line or a diagnostic bundle. Separated from the print so a
/// test can hold the wording without a microphone.
pub fn describe_input(input: &Input) -> String {
    format!(
        "\"{}\" ({})",
        input.name,
        if input.was_default {
            "system default"
        } else {
            "chosen"
        }
    )
}

/// Enumerate input devices on the default host. Safe to call anytime; returns
/// an empty list rather than erroring if the host has no inputs.
pub fn list_input_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let default_name = host.default_input_device().and_then(|d| d.name().ok());
    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|d| d.name().ok())
        .map(|name| DeviceInfo {
            is_default: Some(&name) == default_name.as_ref(),
            name,
        })
        .collect()
}

/// Root-mean-square amplitude of a sample block.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Voice-activity gate that learns the room.
///
/// ## Why this is not a fixed threshold any more
///
/// It used to be `rms >= 0.008`. That is a guess about a microphone and a room the
/// developer never heard, and it fails in exactly the way that is hardest to
/// diagnose: **silently, and only for quiet people.**
///
/// The gate decides which audio whisper is allowed to hear. Measured on real speech
/// pushed through the real front-end (see the `gate` tests below), the same words,
/// differing only in level:
///
/// ```text
///   studio level   94% voiced      the developer's machine. Looks perfect.
///   ×0.2           17% voiced      a church laptop mic. Most of the sermon deleted.
///   ×0.05           2% voiced      a lightly-driven desk feed. Effectively deaf.
/// ```
///
/// The auto-gain (dsp.rs) caps at ×6, so a feed sitting at RMS 0.001 lifts to 0.006
/// and never reaches the 0.008 line at all. Nothing errors. The level meter moves.
/// The transcript just quietly turns to nonsense, and the operator is told the AI
/// "isn't very good".
///
/// ## What it does instead
///
/// Track the noise floor and gate RELATIVE to it. Silence is not an absolute level;
/// it is whatever this room is doing when nobody is talking.
///
/// * The floor falls fast and rises slowly, so it settles onto the quiet parts and
///   is not dragged up by speech.
/// * **Hysteresis**: it takes more energy to open the gate than to hold it open. A
///   dip between two words — a stop consonant, a breath — must not slam it shut, and
///   that is precisely what the old gate did, chopping a sentence into fragments.
/// * An absolute floor remains, but only to stop the gate chasing digital silence
///   into infinite sensitivity when the microphone is unplugged.
///
/// This is deliberately still a plain energy gate, not a neural VAD (silero/webrtc)
/// — it just no longer assumes it knows how loud the preacher is. A real VAD slots
/// in behind the same seam.
/// How speech-like RNNoise must find the audio before it may START an utterance.
///
/// Deliberately LOW. This is not "is this definitely speech?" — it is "is this
/// definitely NOT speech?". RNNoise is confident about clear speech and about
/// clear noise, and unsure about plenty of real preaching: a quiet aside, a sung
/// line, a heavy accent, a room with a hum. A high bar here would recreate the
/// bug this codebase already paid for once — silently deaf to a quiet preacher —
/// so the threshold only rejects audio the model is fairly sure has no voice in
/// it at all.
const SPEECH_OPEN_MIN: f32 = 0.30;

#[derive(Debug, Clone, Copy)]
pub struct Vad {
    /// Absolute floor. Not the speech threshold — only a guard against tracking a
    /// dead microphone's noise floor down to zero and then calling hiss "speech".
    pub threshold_rms: f32,
    noise: f32,
    speaking: bool,
}

/// Speech must exceed the noise floor by this much to OPEN the gate.
const VAD_OPEN_RATIO: f32 = 3.0;
/// …and only fall below this much to CLOSE it. The gap is the hysteresis that keeps
/// a sentence intact across the pauses inside it.
const VAD_CLOSE_RATIO: f32 = 1.7;
/// Noise floor tracking. Falls quickly onto quiet passages, rises slowly so a long
/// loud passage cannot teach the gate that the room is loud and go deaf.
const VAD_FLOOR_DOWN: f32 = 0.30;
const VAD_FLOOR_UP: f32 = 0.005;

impl Vad {
    pub fn new(threshold_rms: f32) -> Self {
        Self {
            threshold_rms,
            noise: -1.0, // unseeded — see is_voice
            speaking: false,
        }
    }

    /// Feed one chunk's RMS. Stateful — call once per chunk, in order.
    ///
    /// `speech` is RNNoise's smoothed speech probability for this audio, or
    /// `None` when the real neural VAD is not running (the device is not 48 kHz,
    /// so `dsp.rs` degrades to an energy proxy — feeding that back in here would
    /// be circular, judging energy by energy).
    ///
    /// ── What the probability is allowed to do ─────────────────────────────
    ///
    /// It may VETO OPENING the gate. It may never close it.
    ///
    /// An energy gate cannot tell a voice from a sound: a door, a chair, a music
    /// bed and an air-conditioner surge all have energy, they open the gate, and
    /// whisper is then handed non-speech — which it does not decline to
    /// transcribe. It answers with the most likely token sequence for audio that
    /// contains no words, and that is where hallucinated subtitle text (in any
    /// language) comes from. So: something must be BOTH loud enough and
    /// speech-like to start an utterance.
    ///
    /// The asymmetry is deliberate and load-bearing. Once the preacher is
    /// speaking, only the energy gate and its hysteresis decide when the
    /// utterance ends. If a low probability could also SHUT the gate, then every
    /// moment RNNoise was unsure — a shout, a whisper, a sung line, a heavy
    /// accent, a bad room — would chop the sentence, which is precisely the
    /// failure `VAD_CLOSE_RATIO` and the "append every chunk" rule in `stt.rs`
    /// exist to prevent. Being wrong about the START of an utterance costs one
    /// late word; being wrong about the MIDDLE mangles the transcript.
    pub fn is_voice(&mut self, rms: f32, speech: Option<f32>) -> bool {
        // SEED the floor from the first chunk.
        //
        // Without this the floor starts at the absolute minimum, so in a merely NOISY
        // room the very first chunk already clears `open`, the gate latches open, and
        // — because the floor only learns while not speaking — it then never learns
        // anything at all. It would sit wide open on room tone for the whole service.
        //
        // Assuming the first chunk is not speech is safe in practice: the operator
        // presses Start Listening before the preacher begins. And it self-heals if they
        // don't — a too-high floor holds the gate shut, which is exactly the state in
        // which the floor tracks downward (fast), so it recovers in about two seconds.
        if self.noise < 0.0 {
            self.noise = rms.max(1e-5);
        }

        let open = (self.noise * VAD_OPEN_RATIO).max(self.threshold_rms);
        let close = (self.noise * VAD_CLOSE_RATIO).max(self.threshold_rms * 0.6);

        self.speaking = if self.speaking {
            // Mid-utterance: energy alone, as before. See the note above on why
            // the probability is not consulted here.
            rms >= close
        } else {
            // Starting an utterance: loud enough AND speech-like.
            rms >= open && speech.is_none_or(|p| p >= SPEECH_OPEN_MIN)
        };

        // The floor may ALWAYS fall, but it may only RISE while we believe nobody is
        // speaking. Otherwise a long passage teaches the gate that the preacher IS the
        // background, and it goes deaf to him halfway through the sermon.
        if rms < self.noise {
            self.noise += VAD_FLOOR_DOWN * (rms - self.noise);
        } else if !self.speaking {
            self.noise += VAD_FLOOR_UP * (rms - self.noise);
        }
        // Never track all the way to zero: a silent (or unplugged) input would drive
        // `open` to 0 and then classify its own dither as speech.
        self.noise = self.noise.max(1e-5);

        self.speaking
    }
}

/// Splits a continuous mono stream into fixed-size overlapping chunks. Pure and
/// deterministic — timestamps derive from a running sample count, so it's fully
/// testable without any audio hardware.
pub struct Chunker {
    sample_rate: u32,
    chunk_len: usize,
    hop_len: usize,
    buf: Vec<f32>,
    consumed: u64, // global index of buf[0]
}

impl Chunker {
    pub fn new(sample_rate: u32, chunk_ms: u32, hop_ms: u32) -> Self {
        let chunk_len = (sample_rate as u64 * chunk_ms as u64 / 1000) as usize;
        let hop_len = (sample_rate as u64 * hop_ms as u64 / 1000).max(1) as usize;
        Chunker {
            sample_rate,
            chunk_len: chunk_len.max(1),
            hop_len,
            buf: Vec::new(),
            consumed: 0,
        }
    }

    /// Feed mono samples; returns any complete chunks now available.
    pub fn push(&mut self, samples: &[f32]) -> Vec<(Vec<f32>, u64)> {
        self.buf.extend_from_slice(samples);
        let mut out = Vec::new();
        while self.buf.len() >= self.chunk_len {
            let chunk = self.buf[..self.chunk_len].to_vec();
            let ts_ms = self.consumed * 1000 / self.sample_rate as u64;
            out.push((chunk, ts_ms));
            let drop = self.hop_len.min(self.buf.len());
            self.buf.drain(..drop);
            self.consumed += drop as u64;
        }
        out
    }
}

/// Turn a continuous mono stream into the exact `AudioChunk` sequence the live
/// capture path would produce: same chunk length, same 50% overlap, same learned
/// voice gate, same timestamps.
///
/// TEST AND BENCH ONLY — and it exists so that a benchmark cannot quietly drift
/// from the thing it claims to measure. A decoder scored on hand-rolled chunks is
/// a measurement of a pipeline no congregation will ever hear, and the constants
/// that would have to be copied to hand-roll them (`CHUNK_MS`, `HOP_MS`,
/// `VAD_RMS_THRESHOLD`) are private precisely because there must be one copy.
///
/// The one thing it does NOT do is run `dsp::FrontEnd` — callers feed it audio
/// that has already been cleaned, because the bench degrades the signal itself
/// and then cleans it, in that order, exactly as a room would.
#[cfg(test)]
pub(crate) fn chunks_as_captured(cleaned: &[f32], sample_rate: u32) -> Vec<AudioChunk> {
    let mut chunker = Chunker::new(sample_rate, CHUNK_MS, HOP_MS);
    let mut vad = Vad::new(VAD_RMS_THRESHOLD);
    let mut out = Vec::new();
    // Push in blocks rather than all at once so the chunker's internal buffering
    // behaves as it does live; the VAD is stateful and must see chunks in order.
    for block in cleaned.chunks(1024) {
        for (samples, ts_ms) in chunker.push(block) {
            let level = rms(&samples);
            out.push(AudioChunk {
                // `None`: the neural speech probability only counts at 48 kHz
                // (dsp.rs), and bench audio is 16 kHz — same as the live path on
                // a device that cannot give us 48.
                is_voice: vad.is_voice(level, None),
                rms: level,
                timestamp_ms: ts_ms,
                sample_rate,
                samples,
                received_at_us: crate::latency::now_us(),
            });
        }
    }
    out
}

/// **THE RUN OF CHANCES A LOST DEVICE GETS — RG-291.**
///
/// One wait per re-open attempt, in order. It backs off because a device that is
/// mid-teardown is made worse by being reopened at once, and it ENDS because an
/// unbounded retry is a microphone that never admits it is gone — the operator
/// would watch "Reconnecting" for the rest of the service with no banner and no
/// way to tell it from a working one (rule 35).
///
/// The total is 15.5 s, and that number is the one a person takes to notice a
/// cable, push it back in and let the OS re-enumerate the device. Shorter and a
/// re-plug arrives after Relay has already given up; much longer and the banner
/// that says the microphone is gone arrives after the operator has worked it out
/// for themselves, which is the same as not having one.
const REOPEN_BACKOFF_MS: &[u64] = &[500, 1_000, 2_000, 4_000, 8_000];

/// How long to wait before re-opening, for the `attempt`-th chance (0-based), or
/// `None` when the bound is spent. The bound is `REOPEN_BACKOFF_MS.len()`.
fn reopen_backoff_ms(attempt: usize) -> Option<u64> {
    REOPEN_BACKOFF_MS.get(attempt).copied()
}

/// **THE THREE FACTS AN OPERATOR HAS TO BE ABLE TO TELL APART — rule 35, RG-291.**
///
/// "The microphone stopped and I am trying again", "audio is arriving again, from
/// this input" and "I could not get it back" are three different situations, and a
/// single line that reads the same in all three is not a status line. They ride
/// their own channel for that reason; `audio://error` keeps meaning exactly what
/// it means today, which is the last of the three.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Recovery {
    /// The device stopped and Relay is going to try again. Rule 5's guarantees are
    /// already kept by the time this is sent: the loop has exited, the reason is
    /// this `reason`, and the debug recording for that segment is on disk.
    Lost {
        reason: String,
        /// 1-based, for reading aloud: "attempt 2 of 5".
        attempt: usize,
        of: usize,
        retry_in_ms: u64,
    },
    /// Audio is arriving again. Built ONLY by `Resumption::audio_arrived`, so it
    /// cannot be said about a device that merely opened.
    Listening {
        input: String,
        /// RG-121: a silent fallback to the laptop microphone once put Relay at the
        /// back of a booth with a desk feed plugged in. "It came back" and "it came
        /// back on a microphone nobody chose" are two different facts.
        was_default: bool,
        /// How long the transcript was down, measured from the loss.
        missed_ms: u64,
    },
    /// The bound is spent. This is the one that reaches `on_error`, carrying the
    /// FIRST reason rather than the last — same rule as `note_stream_error`.
    GaveUp {
        reason: String,
        attempts: usize,
        after_ms: u64,
    },
}

/// **AN OPEN IS NOT A RESUMPTION — RG-291.**
///
/// A device can be resolved, report every config it supports, accept `play()` and
/// deliver no frames at all; that is what `DEAD_INPUT_MS` exists for. So the claim
/// "Listening again" is made by a buffer arriving and by nothing else, and this
/// type is the only thing in the crate that can make it. One announcement per
/// attempt: a status that repeats on every buffer is a status nobody reads.
#[derive(Debug, Default)]
struct Resumption {
    announced: bool,
}

impl Recovery {
    /// One line, in the words an operator would use. Shared by the stderr sink and
    /// by anything that puts this in front of a person, so the two cannot come to
    /// describe the same event differently.
    pub fn describe(&self) -> String {
        match self {
            Recovery::Lost {
                reason,
                attempt,
                of,
                retry_in_ms,
            } => format!(
                "microphone lost ({reason}) — reconnecting, attempt {attempt} of {of}, \
                 in {retry_in_ms}ms"
            ),
            Recovery::Listening {
                input,
                was_default,
                missed_ms,
            } => format!(
                "listening again on {:?}{} after {missed_ms}ms",
                input,
                if *was_default {
                    " (system default — NOT the input that was chosen)"
                } else {
                    ""
                }
            ),
            Recovery::GaveUp {
                reason,
                attempts,
                after_ms,
            } => format!(
                "could not get the microphone back after {attempts} attempts over \
                 {after_ms}ms — {reason}"
            ),
        }
    }
}

/// Wait, but never past an operator's Stop.
///
/// `AudioEngine::stop()` joins this thread, so a plain `sleep` in the backoff is a
/// Stop button that hangs for as long as the longest wait. Returns false when the
/// wait was cut short, which is the caller's signal to stop rather than re-open.
fn sleep_unless_stopped(stop: &AtomicBool, ms: u64) -> bool {
    let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    loop {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        let left = end.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return true;
        }
        std::thread::sleep(left.min(std::time::Duration::from_millis(50)));
    }
}

impl Resumption {
    fn audio_arrived(&mut self, input: &Input, missed_ms: u64) -> Option<Recovery> {
        if std::mem::replace(&mut self.announced, true) {
            return None;
        }
        Some(Recovery::Listening {
            input: input.name.clone(),
            was_default: input.was_default,
            missed_ms,
        })
    }
}

/// Owns the running capture + processing thread. Drop or call `stop()` to end.
pub struct AudioEngine {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl AudioEngine {
    /// Start capturing from `device_name` (or the default input when None),
    /// invoking `on_chunk` for every chunk produced (voiced and unvoiced — the
    /// `is_voice` flag lets upstream drop silence while the UI still meters
    /// level).
    ///
    /// NON-BLOCKING: returns immediately after spawning the capture thread. The
    /// stream is built on that thread, so a slow/blocked device init (e.g. a
    /// macOS mic-permission prompt) never stalls the caller — critical because
    /// this runs inside a synchronous Tauri command on the UI thread. Device
    /// errors are reported asynchronously via `on_error`.
    ///
    /// `on_quality` receives an audio-quality snapshot per processed block
    /// (denoise/gain/SNR/warnings) — additive, and the caller may throttle or
    /// ignore it.
    #[cfg(test)]
    pub fn start<F, Q, E>(
        device_name: Option<String>,
        on_chunk: F,
        on_quality: Q,
        on_error: E,
    ) -> Self
    where
        F: Fn(&AudioChunk) + Send + 'static,
        Q: Fn(&dsp::AudioQuality) + Send + 'static,
        E: Fn(String) + Send + 'static,
    {
        Self::start_with_recovery(device_name, on_chunk, on_quality, on_error, |r| {
            eprintln!("audio: {}", r.describe());
        })
    }

    /// As `start`, plus the running commentary an operator needs while a
    /// microphone is being picked back up (RG-291).
    ///
    /// `on_recovery` is called off the capture thread with each of the three facts
    /// rule 35 requires be distinguishable: `Lost` (and trying again), `Listening`
    /// (audio really is arriving, from this input) and `GaveUp`. `on_error` still
    /// fires exactly once, at the end, with the FIRST reason — so everything that
    /// reads `audio://error` today keeps meaning what it means today.
    ///
    /// `start` routes the commentary to stderr, which is where it went before this
    /// existed. Wire a real emitter to put it in front of the operator.
    pub fn start_with_recovery<F, Q, E, R>(
        device_name: Option<String>,
        on_chunk: F,
        on_quality: Q,
        on_error: E,
        on_recovery: R,
    ) -> Self
    where
        F: Fn(&AudioChunk) + Send + 'static,
        Q: Fn(&dsp::AudioQuality) + Send + 'static,
        E: Fn(String) + Send + 'static,
        R: Fn(Recovery) + Send + 'static,
    {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();

        let handle = std::thread::spawn(move || {
            // Attempts since the last time audio was actually flowing. RESET on a
            // resume, deliberately: a glitch in the first minute must not spend the
            // budget for a different glitch an hour later. The bound is per loss,
            // not per service.
            let mut attempt = 0usize;
            let mut first_reason: Option<String> = None;
            let mut lost_at: Option<std::time::Instant> = None;

            loop {
                let dead = Arc::new(AtomicBool::new(false));
                let resumed = Arc::new(AtomicBool::new(false));
                let mut resume = Resumption::default();
                let (seen, since) = (resumed.clone(), lost_at);
                let outcome = {
                    let mut on_live = |input: &Input| {
                        seen.store(true, Ordering::Relaxed);
                        // Nothing is announced on a FIRST start: there was no loss
                        // to recover from, and a "Listening again" over an ordinary
                        // Start is a status that cried wolf.
                        let Some(t) = since else { return };
                        if let Some(r) = resume.audio_arrived(input, t.elapsed().as_millis() as u64)
                        {
                            on_recovery(r);
                        }
                    };
                    build_and_run(
                        device_name.clone(),
                        stop_thread.clone(),
                        dead,
                        &on_chunk,
                        &on_quality,
                        &mut on_live,
                    )
                };

                // Audio flowed at some point in that attempt, so the run of chances
                // starts again from here.
                if resumed.load(Ordering::Relaxed) {
                    attempt = 0;
                    first_reason = None;
                    lost_at = None;
                }

                // Ok means the loop ended without a device error — an operator's
                // Stop, or a sender that went away. Neither is something to retry.
                let Err(reason) = outcome else { break };
                if stop_thread.load(Ordering::Relaxed) {
                    break;
                }

                // Rule 5 is already kept by the time we are here: the loop exited,
                // the debug recording for that segment is on disk (and on its own
                // `-2`, `-3` … path, so a resume can never truncate the one before
                // it), and this is the reason it stored.
                let started = *lost_at.get_or_insert_with(std::time::Instant::now);
                let first = first_reason.get_or_insert(reason.clone()).clone();

                match reopen_backoff_ms(attempt) {
                    Some(wait_ms) => {
                        on_recovery(Recovery::Lost {
                            reason,
                            attempt: attempt + 1,
                            of: REOPEN_BACKOFF_MS.len(),
                            retry_in_ms: wait_ms,
                        });
                        if !sleep_unless_stopped(&stop_thread, wait_ms) {
                            break;
                        }
                        attempt += 1;
                    }
                    None => {
                        // The bound is spent. This is the banner, and it is the
                        // same one rule 5 has always produced — the FIRST reason,
                        // because cpal's later messages are consequences.
                        on_recovery(Recovery::GaveUp {
                            reason: first.clone(),
                            attempts: attempt,
                            after_ms: started.elapsed().as_millis() as u64,
                        });
                        on_error(first);
                        break;
                    }
                }
            }
        });

        AudioEngine {
            stop,
            handle: Some(handle),
        }
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// WHAT A RUNTIME STREAM ERROR IS ALLOWED TO DO (RG-117).
///
/// cpal reports a device that has died after capture started through the stream's
/// error callback, and that callback was the single line
/// `eprintln!("audio stream error: {e}")`. It set no flag, emitted no event and
/// reached no operator, so three things happened at once and none of them were
/// visible:
///
/// 1. The capture loop is `while !stop` around a 100 ms `recv_timeout`, and the
///    stream object stays alive, so the channel never disconnects. It spun on
///    `Timeout => continue` for ever. The transcript simply stopped mid-sermon,
///    indistinguishable from a preacher who had gone quiet.
/// 2. `RELAY_RECORD_WAV` writes AFTER the loop exits, so a loop that never exits
///    never writes. The instrument for the one measurement this project most needs
///    is destroyed by the failure it would most want to have captured. On
///    2026-09-06 the desk feed was unplugged and 26 minutes of a service sat
///    buffered in RAM while the app looked alive.
/// 3. CLAUDE.md rule 5 says device errors come back via `audio://error`. That was
///    true of START errors and false of runtime ones, so the handbook overstated
///    the coverage.
///
/// The message is stored BEFORE the flag is set, so the loop cannot exit and look
/// for a reason that has not been written yet. The FIRST error wins: cpal can fire
/// this repeatedly while a device tears down, and the first one is the cause while
/// the rest are consequences.
///
/// **It never blocks.** This can be called on the device's own real-time thread,
/// where blocking is what kills a capture stream outright, so a contended lock
/// loses the message rather than waiting for it — and the stop flag is still set,
/// because stopping is the half that saves the recording.
fn note_stream_error(stop: &AtomicBool, sink: &Mutex<Option<String>>, message: String) {
    eprintln!("audio stream error: {message}");
    if let Ok(mut slot) = sink.try_lock() {
        slot.get_or_insert(message);
    }
    stop.store(true, Ordering::Relaxed);
}

/// Where a debug recording may actually be written.
///
/// **Never over an existing file.** `RELAY_RECORD_WAV` names one path and the
/// capture thread used to truncate it on every Stop, so a second Start/Stop cycle
/// silently destroyed the first: on 2026-09-06 that turned 1837.8 s of a real
/// service into a valid-looking 170.0 s file, and the audio was gone. It is worse
/// than losing the recording outright, because it looks exactly like it worked.
///
/// So an existing name is never reused; the next free `-2`, `-3` … is taken and
/// the write line prints where the audio actually went.
fn free_recording_path(requested: &std::path::Path) -> std::path::PathBuf {
    if !requested.exists() {
        return requested.to_path_buf();
    }
    let stem = requested
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "recording".into());
    let ext = requested
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let dir = requested.parent().unwrap_or(std::path::Path::new("."));
    // Bounded: a loop that cannot fail is a loop that hangs on a full disk or a
    // read-only directory. After this many the answer is not a better filename.
    for n in 2..1000 {
        let candidate = dir.join(format!("{stem}-{n}{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    requested.to_path_buf()
}

/// Resolve the device, build the cpal stream, then run the chunk/VAD loop until
/// stopped. Everything touching the non-Send `Device`/`Stream` stays on this
/// one thread.
fn build_and_run<F, Q>(
    device_name: Option<String>,
    stop: Arc<AtomicBool>,
    // RG-291. TWO flags, and they used to be one. An operator pressing Stop and a
    // microphone dying are different events with different consequences: the first
    // ends the capture for good, the second is a reason to try again. Conflated,
    // there is no way for the retry loop above to tell "the device went" from
    // "somebody asked me to stop", so it would either hammer a stopped engine or
    // never resume a lost one. `stop` belongs to `AudioEngine` and is never reset;
    // `dead` is this attempt's, and `note_stream_error` sets it.
    dead: Arc<AtomicBool>,
    on_chunk: &F,
    on_quality: &Q,
    on_live: &mut dyn FnMut(&Input),
) -> Result<(), String>
where
    F: Fn(&AudioChunk) + Send + 'static,
    Q: Fn(&dsp::AudioQuality) + Send + 'static,
{
    let host = cpal::default_host();
    // Answered before the match consumes the name (RG-122): "was this the system
    // default" is a question about what the CALLER asked for, not about what cpal
    // handed back, and the two differ the moment a stored device has vanished.
    let asked_for_default = device_name.is_none();
    let device = match device_name {
        Some(name) => host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| format!("input device not found: {name}"))?,
        None => host
            .default_input_device()
            .ok_or_else(|| "no default input device".to_string())?,
    };
    let preferred = pick_input_config(&device)?;
    let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(CAPTURE_QUEUE);

    // Prefer the 48 kHz config (RNNoise runs frame-aligned), but if that exact
    // config can't actually be opened on this device — common with USB mics,
    // aggregate/virtual devices, or unusual channel layouts — fall back to the
    // device's own default config so audio STILL flows (denoise self-disables).
    // Without this, selecting a non-default device silently produced no audio.
    // Where a runtime stream failure leaves its reason. Set by the stream's error
    // callback, read once the loop has exited and the recording is safely written.
    let runtime_err: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let (stream, used) = match build_stream(&device, &preferred, &tx, &dead, &runtime_err) {
        Ok(s) => (s, preferred),
        Err(e1) => {
            eprintln!("audio: preferred 48 kHz config failed ({e1}); using device default");
            let def = device.default_input_config().map_err(|e| e.to_string())?;
            let s = build_stream(&device, &def, &tx, &dead, &runtime_err)?;
            (s, def)
        }
    };
    let sample_rate = used.sample_rate().0;
    stream.play().map_err(|e| e.to_string())?;

    // Record WHICH input this is, now that one is definitely open (RG-122). After
    // `play()` on purpose: a device that resolved and then would not start is not
    // the microphone this service used, and writing it here would name it as one.
    let opened = Input {
        name: device.name().unwrap_or_else(|_| "unknown device".into()),
        was_default: asked_for_default,
    };
    if let Ok(mut last) = LAST_INPUT.lock() {
        *last = Some(opened.clone());
    }

    // ── DEBUG RECORDER ──
    //
    // Off unless RELAY_RECORD_WAV names a path. Writes the CLEANED mono stream — the
    // exact samples the VAD and whisper see — as a 32-bit-float WAV.
    //
    // This exists because every audio bug so far has been invisible from the code and
    // only reproducible with a specific microphone in a specific room. Synthetic speech
    // is too clean to trigger them; the developer's laptop is too loud. Without a
    // recording of the audio that actually failed, the only debugging tool is asking a
    // human to say the same sentence over and over.
    //
    // PRIVACY: this is sermon audio. It is off by default, writes only to a local path
    // the operator names explicitly, is never uploaded, and is never enabled by any UI —
    // it exists for someone diagnosing their own installation. See PRIVACY.md.
    let mut rec = std::env::var_os("RELAY_RECORD_WAV").map(|p| {
        // The name is where it will TRY to write. An existing file is never
        // overwritten (`free_recording_path`), and the write line at the end of
        // capture prints where the audio actually went.
        println!(
            "audio: RECORDING cleaned input to {} (existing files are never overwritten)",
            p.to_string_lossy()
        );
        (std::path::PathBuf::from(p), Vec::<f32>::new())
    });

    let dbg_rms = std::env::var_os("RELAY_AUDIO_RMS").is_some();
    let mut dbg_seen = 0u32;
    let mut dbg_voiced = 0u32;
    let mut dbg_min = f32::MAX;
    let mut dbg_max = 0.0f32;
    let mut dbg_sum = 0.0f32;
    let mut vad = Vad::new(VAD_RMS_THRESHOLD);
    let mut chunker = Chunker::new(sample_rate, CHUNK_MS, HOP_MS);
    // Clean the stream (denoise + auto-gain) before chunking/VAD. Runs on this
    // same off-realtime thread. Frame-aligned at 48 kHz; degrades to gain-only
    // at other rates (see dsp.rs).
    let mut frontend = dsp::FrontEnd::new(sample_rate);
    eprintln!(
        "audio: capture @ {sample_rate} Hz · denoise {} · input {}",
        if frontend.denoise_active() {
            "on (RNNoise)"
        } else {
            "off (device not 48 kHz — auto-gain only)"
        },
        describe_input(&opened)
    );

    // WHEN DID AUDIO LAST ARRIVE? See `DEAD_INPUT_MS`. Started here rather than at
    // `play()` so that the device's own start-up latency is inside the grace period.
    let mut last_data = std::time::Instant::now();

    // RG-291. `dead` is checked beside `stop` so the attempt ends on either, and
    // the caller can tell which happened by whether an error comes back.
    let mut live_announced = false;
    while !stop.load(Ordering::Relaxed) && !dead.load(Ordering::Relaxed) {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(samples) => {
                last_data = std::time::Instant::now();
                // AUDIO IS THE EVIDENCE, NOT THE OPEN (RG-291). A device can be
                // found, accept every config, start, and deliver nothing — that is
                // what `DEAD_INPUT_MS` below exists for. So the resume is announced
                // from here, on a buffer, and once.
                if !live_announced {
                    live_announced = true;
                    on_live(&opened);
                }
                let cleaned = frontend.process(&samples);
                if let Some((_, buf)) = rec.as_mut() {
                    buf.extend_from_slice(&cleaned.samples);
                }
                on_quality(&cleaned.quality);
                for (chunk, ts_ms) in chunker.push(&cleaned.samples) {
                    let level = rms(&chunk);
                    // Measure, don't guess: what does the VAD actually see?
                    if dbg_rms {
                        dbg_seen += 1;
                        if level >= VAD_RMS_THRESHOLD {
                            dbg_voiced += 1;
                        }
                        dbg_min = dbg_min.min(level);
                        dbg_max = dbg_max.max(level);
                        dbg_sum += level;
                        if dbg_seen.is_multiple_of(25) {
                            eprintln!(
                                "audio: chunk rms min={dbg_min:.4} mean={:.4} max={dbg_max:.4} \
                                 gate={VAD_RMS_THRESHOLD:.4} voiced={}/{} denoise={}",
                                dbg_sum / dbg_seen as f32,
                                dbg_voiced,
                                dbg_seen,
                                frontend.denoise_active()
                            );
                            dbg_min = f32::MAX;
                            dbg_max = 0.0;
                            dbg_sum = 0.0;
                            dbg_seen = 0;
                            dbg_voiced = 0;
                        }
                    }
                    let ac = AudioChunk {
                        // The neural VAD only counts when it is actually neural:
                        // below 48 kHz `speech_prob` is an energy proxy (dsp.rs).
                        is_voice: vad.is_voice(
                            level,
                            frontend
                                .denoise_active()
                                .then_some(cleaned.quality.speech_prob),
                        ),
                        rms: level,
                        timestamp_ms: ts_ms,
                        sample_rate,
                        samples: chunk,
                        received_at_us: crate::latency::now_us(),
                    };
                    on_chunk(&ac);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // A SILENT DEATH IS STILL A DEATH, AND UNTIL NOW NOTHING NOTICED.
                //
                // RG-117 taught the loop to exit when cpal REPORTS an error. On
                // macOS an input that is unplugged, re-plugged or taken by another
                // application frequently reports nothing at all: the stream object
                // stays alive, its callback simply stops being called. The loop then
                // times out for ever, every instrument reads normal, and the console
                // goes on saying "Listening" over a microphone that is gone. That is
                // rule 35 exactly — a status that reads the same when the thing
                // behind it is broken.
                //
                // Reported through `note_stream_error`, so it takes the ONE path a
                // dead input already had: the recording is written first, then
                // `audio://error` reaches the operator, and the frontend clears
                // `capturing` so the microphone control offers Start again.
                if last_data.elapsed().as_millis() as u64 >= DEAD_INPUT_MS {
                    note_stream_error(
                        &dead,
                        &runtime_err,
                        format!(
                            "no audio from {} for {} seconds — it may have been \
                             unplugged or taken by another application",
                            describe_input(&opened),
                            DEAD_INPUT_MS / 1_000
                        ),
                    );
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(stream);
    if let Some((path, buf)) = rec {
        // Resolved HERE and not at Start: the previous segment's file exists by
        // now, and this is what stops a second Stop from writing over it.
        let path = free_recording_path(&path);
        match write_wav_f32(&path, &buf, sample_rate) {
            Ok(()) => println!(
                "audio: wrote {:.1}s to {}",
                buf.len() as f32 / sample_rate as f32,
                path.display()
            ),
            Err(e) => eprintln!("audio: could not write recording: {e}"),
        }
    }
    // The recording is on disk before the error is reported, deliberately. This
    // returns through `AudioEngine::start`'s `on_error`, which emits
    // `audio://error` — so by the time an operator is told the microphone died,
    // the audio that proves what happened has already been saved.
    if let Some(msg) = runtime_err.lock().ok().and_then(|mut m| m.take()) {
        return Err(msg);
    }
    Ok(())
}

/// Minimal 32-bit-float mono WAV writer, for the debug recorder only.
fn write_wav_f32(path: &std::path::Path, samples: &[f32], rate: u32) -> std::io::Result<()> {
    use std::io::Write;
    let data_len = (samples.len() * 4) as u32;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?; // fmt chunk size
    f.write_all(&3u16.to_le_bytes())?; // format 3 = IEEE float
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&rate.to_le_bytes())?;
    f.write_all(&(rate * 4).to_le_bytes())?; // byte rate
    f.write_all(&4u16.to_le_bytes())?; // block align
    f.write_all(&32u16.to_le_bytes())?; // bits per sample
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for s in samples {
        f.write_all(&s.to_le_bytes())?;
    }
    f.flush()
}

/// How many raw capture buffers may wait for the clean/chunk thread.
///
/// RG-84. This was an UNBOUNDED `mpsc::channel` fed from the cpal callback, which
/// is a real-time thread. An unbounded queue in front of a consumer does not
/// prevent a backlog — it converts one into memory, and then into a transcript
/// arriving minutes after the sentence (CLAUDE.md rule 31: a pipeline permanently
/// behind reports a *zero backlog* for a whole service). And CLAUDE.md rule 33
/// described this path as bounded with a shed counter, which was true of the third
/// hop only.
///
/// cpal delivers buffers of a few milliseconds and the consumer does denoise, gain,
/// VAD and chunking — work measured in microseconds — so in normal operation this
/// queue holds one or two. 512 buffers is several seconds of grace: far more than
/// any scheduling hiccup needs, and a hard ceiling on what a genuine stall can cost.
///
/// A full queue DROPS, and drops are counted (`latency::note_dropped_audio`). It is
/// never allowed to block: blocking here would stall the audio device's own
/// callback, which is how a capture stream is killed outright.
/// How long the loop will wait for ANY audio before calling the input dead.
///
/// A device callback is driven by the device's own clock, not by the signal, so a
/// healthy input delivers buffers of digital silence while nobody is talking. No
/// buffer at all for this long is not a quiet room; it is a device that has gone.
///
/// Four seconds because the wrong answer here is expensive in both directions: too
/// short and a device that stutters once loses the microphone mid-sermon, too long
/// and the operator preaches to an instrument that stopped listening.
const DEAD_INPUT_MS: u64 = 4_000;

const CAPTURE_QUEUE: usize = 512;

/// Build a cpal input stream for `supported`, downmixing to mono and forwarding
/// samples over `tx`. Kept separate so the caller can try a preferred config and
/// fall back to the device default if the preferred one won't open.
fn build_stream(
    device: &cpal::Device,
    supported: &cpal::SupportedStreamConfig,
    tx: &mpsc::SyncSender<Vec<f32>>,
    stop: &Arc<AtomicBool>,
    runtime_err: &Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, String> {
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.clone().into();
    let channels = config.channels as usize;
    // A DEAD DEVICE MUST STOP THE LOOP (RG-117). See `note_stream_error`: this
    // used to be an `eprintln!` and nothing else, so capture spun for ever on a
    // device that had been unplugged, the operator was told nothing, and the debug
    // recording — which is written after the loop exits — was never written at all.
    let (stop_on_err, err_sink) = (stop.clone(), runtime_err.clone());
    let err_fn =
        move |e: cpal::StreamError| note_stream_error(&stop_on_err, &err_sink, e.to_string());
    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let tx = tx.clone();
            device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // NEVER block: this is the device's real-time callback, and
                    // stalling it kills the capture stream. FULL means the
                    // clean/chunk thread has stopped keeping up — a gap in the
                    // sermon, and counted as one. DISCONNECTED is the ordinary end
                    // of a capture and is not counted, or every Stop would put a
                    // four-figure number in the report.
                    if let Err(mpsc::TrySendError::Full(_)) =
                        tx.try_send(downmix_f32(data, channels))
                    {
                        crate::latency::note_dropped_audio();
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let tx = tx.clone();
            device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    // NEVER block: this is the device's real-time callback, and
                    // stalling it kills the capture stream. FULL means the
                    // clean/chunk thread has stopped keeping up — a gap in the
                    // sermon, and counted as one. DISCONNECTED is the ordinary end
                    // of a capture and is not counted, or every Stop would put a
                    // four-figure number in the report.
                    if let Err(mpsc::TrySendError::Full(_)) =
                        tx.try_send(downmix_i16(data, channels))
                    {
                        crate::latency::note_dropped_audio();
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let tx = tx.clone();
            device.build_input_stream(
                &config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    // NEVER block: this is the device's real-time callback, and
                    // stalling it kills the capture stream. FULL means the
                    // clean/chunk thread has stopped keeping up — a gap in the
                    // sermon, and counted as one. DISCONNECTED is the ordinary end
                    // of a capture and is not counted, or every Stop would put a
                    // four-figure number in the report.
                    if let Err(mpsc::TrySendError::Full(_)) =
                        tx.try_send(downmix_u16(data, channels))
                    {
                        crate::latency::note_dropped_audio();
                    }
                },
                err_fn,
                None,
            )
        }
        other => return Err(format!("unsupported sample format: {other:?}")),
    }
    .map_err(|e| e.to_string())?;
    Ok(stream)
}

/// Choose a capture config, preferring 48 kHz so the RNNoise front-end runs
/// frame-aligned with no resampling. Falls back to the device default when no
/// 48 kHz config exists (denoise then self-disables — see dsp.rs).
fn pick_input_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, String> {
    let target = cpal::SampleRate(dsp::RNNOISE_RATE);
    if let Ok(ranges) = device.supported_input_configs() {
        let ranges: Vec<_> = ranges.collect();
        // Prefer an f32 config at 48 kHz, then any config at 48 kHz.
        for want_f32 in [true, false] {
            for r in &ranges {
                if want_f32 && r.sample_format() != cpal::SampleFormat::F32 {
                    continue;
                }
                if r.min_sample_rate() <= target && target <= r.max_sample_rate() {
                    return Ok((*r).with_sample_rate(target));
                }
            }
        }
    }
    device.default_input_config().map_err(|e| e.to_string())
}

fn downmix_f32(data: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return data.to_vec();
    }
    data.chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

fn downmix_i16(data: &[i16], channels: usize) -> Vec<f32> {
    let to_f = |s: i16| s as f32 / 32768.0;
    if channels <= 1 {
        return data.iter().copied().map(to_f).collect();
    }
    data.chunks(channels)
        .map(|frame| frame.iter().copied().map(to_f).sum::<f32>() / channels as f32)
        .collect()
}

fn downmix_u16(data: &[u16], channels: usize) -> Vec<f32> {
    let to_f = |s: u16| (s as f32 - 32768.0) / 32768.0;
    if channels <= 1 {
        return data.iter().copied().map(to_f).collect();
    }
    data.chunks(channels)
        .map(|frame| frame.iter().copied().map(to_f).sum::<f32>() / channels as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A DEAD DEVICE MUST STOP THE LOOP, AND SAY WHY (RG-117).
    ///
    /// The old error callback was `|e| eprintln!("audio stream error: {e}")` and
    /// nothing else. The loop is `while !stop` around a 100 ms `recv_timeout` and
    /// the stream stays alive, so the channel never disconnects and capture spun
    /// on a device that had been unplugged: no banner, no event, and — because
    /// the debug recorder writes after the loop exits — no recording either.
    #[test]
    fn a_stream_error_stops_capture_and_keeps_its_reason() {
        let stop = AtomicBool::new(false);
        let sink = Mutex::new(None);
        note_stream_error(
            &stop,
            &sink,
            "The requested device is no longer available.".into(),
        );
        assert!(
            stop.load(Ordering::Relaxed),
            "the loop must be able to exit"
        );
        assert_eq!(
            sink.lock().unwrap().as_deref(),
            Some("The requested device is no longer available."),
            "and the operator must be able to be told what happened"
        );
    }

    /// **HAVING STOPPED CLEANLY, RELAY PICKS THE AUDIO BACK UP — RG-291.**
    ///
    /// The operator: *"make sure live transcript dosent stop. as long as there is
    /// audio coming in… when audio input switch, continue transcript once audio is
    /// dected…"*
    ///
    /// Rule 5 stops the loop on a dead device, stores the reason, writes the
    /// recording and tells the operator. All of that is right and none of it moves.
    /// What it never had is the step after: a USB mic that glitches, an input taken
    /// for a moment by another application, a desk feed re-plugged — each one ended
    /// the transcript for the rest of the service, and the only way back was an
    /// operator noticing and pressing Start.
    ///
    /// **Bounded, because an unbounded retry is a microphone that never admits it
    /// is gone.** The schedule backs off so a device tearing down is not hammered,
    /// and it ends — at which point `Recovery::GaveUp` carries the original reason
    /// to `on_error` exactly as today.
    #[test]
    fn a_lost_device_is_given_a_bounded_run_of_chances() {
        // It ends. Nothing here may return Some for ever.
        assert!(reopen_backoff_ms(REOPEN_BACKOFF_MS.len()).is_none());
        assert!(reopen_backoff_ms(usize::MAX).is_none());

        // It backs off rather than spinning: a device that is mid-teardown is made
        // worse by being reopened immediately, and a tight loop is a busy thread.
        let mut prev = 0;
        for a in 0..REOPEN_BACKOFF_MS.len() {
            let ms = reopen_backoff_ms(a).expect("attempt within the bound");
            assert!(
                ms > prev,
                "attempt {a} does not back off: {ms} after {prev}"
            );
            prev = ms;
        }

        // And the whole of it fits inside the time a person takes to re-plug a
        // cable, which is what this is for. Longer and the banner arrives after
        // the operator has already worked out that something is wrong.
        let total: u64 = REOPEN_BACKOFF_MS.iter().sum();
        assert!(
            (5_000..=30_000).contains(&total),
            "the whole run of chances is {total}ms"
        );
    }

    /// **AN OPEN IS NOT A RESUMPTION — and this is the half that would have been
    /// got wrong.**
    ///
    /// A device can be found, report every config it supports, accept `play()` and
    /// then deliver no frames at all. That is not a hypothetical: it is the exact
    /// failure `DEAD_INPUT_MS` exists for, on macOS, on an input another
    /// application has taken. So "Listening again" is said by AUDIO ARRIVING and by
    /// nothing else — and it is said ONCE per attempt, not on every buffer.
    #[test]
    fn a_resume_is_announced_by_audio_and_never_by_an_open() {
        let input = Input {
            name: "Blackmagic Web Presenter 4K".into(),
            was_default: false,
        };
        let mut r = Resumption::default();
        // Buffer one: this is the evidence, and it names the input it resumed ON —
        // "it came back" and "it came back on a different microphone" are two
        // different facts (RG-121).
        let announced = r
            .audio_arrived(&input, 2_400)
            .expect("the first buffer must announce the resume");
        if let Recovery::Listening {
            input: n,
            was_default,
            missed_ms,
        } = &announced
        {
            assert_eq!(n, "Blackmagic Web Presenter 4K");
            assert!(!was_default);
            assert_eq!(*missed_ms, 2_400);
        } else {
            panic!("wrong fact reported: {announced:?}");
        }
        // Every buffer after it is silent. A status that repeats is a status nobody
        // reads.
        assert!(r.audio_arrived(&input, 2_400).is_none());
        assert!(r.audio_arrived(&input, 9_999).is_none());
    }

    /// **AND THE ONE PLACE IT IS SAID.** Rule 36's shape: the guarantee goes on the
    /// door, not at the call sites. `Recovery::Listening` is built by `Resumption`
    /// and by nothing else, so a future caller cannot announce a resume it has no
    /// evidence for — which is the whole of the test above, defeated by one extra
    /// construction site.
    ///
    /// The scanner asserts what it found before asserting anything about it: one
    /// that quietly matched nothing would pass whatever the file said.
    #[test]
    fn only_the_evidence_may_build_a_resume() {
        let src = include_str!("audio.rs");
        // Split, so the scanner cannot match its own filter.
        let needle = concat!("Some(Recovery::", "Listening {");
        let sites: Vec<&str> = src
            .lines()
            .map(str::trim)
            .filter(|l| l.contains(needle) && !l.starts_with("///"))
            .collect();
        assert_eq!(
            sites.len(),
            1,
            "a resume may be built in one place and only by the evidence, found {sites:?}"
        );

        // AND IT MUST BE CALLED FROM THE ARM THAT RECEIVED AUDIO. The test above
        // holds where a `Listening` is BUILT; this holds where the liveness hook is
        // FIRED, which is the half a mutation got past: moving `on_live` up beside
        // `play()` compiled, ran, and announced a resume on a device that had
        // delivered nothing.
        let lines: Vec<&str> = src.lines().map(str::trim).collect();
        let call = concat!("on_live", "(&opened)");
        let at: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.starts_with(call))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(at.len(), 1, "one liveness hook, found {at:?}");
        let arm = lines[..at[0]]
            .iter()
            .rposition(|l| l.contains("=> {"))
            .expect("the hook must sit inside a match arm");
        assert!(
            lines[arm].starts_with("Ok(samples)"),
            "the resume is announced from {:?}, not from the arm that received audio",
            lines[arm]
        );
    }

    /// **AND STOP MUST NOT WAIT OUT THE BACKOFF.**
    ///
    /// `AudioEngine::stop()` JOINS this thread. A plain `sleep(8000)` in the retry
    /// schedule is therefore a Stop button that hangs for eight seconds, on the one
    /// control an operator reaches for when something has already gone wrong.
    #[test]
    fn an_operator_stop_cuts_the_backoff_short() {
        let stop = AtomicBool::new(true);
        let t = std::time::Instant::now();
        assert!(
            !sleep_unless_stopped(&stop, 8_000),
            "a stopped engine must not re-open"
        );
        assert!(
            t.elapsed().as_millis() < 200,
            "Stop waited {:?} out",
            t.elapsed()
        );

        // And an untouched wait really does wait.
        let running = AtomicBool::new(false);
        let t = std::time::Instant::now();
        assert!(sleep_unless_stopped(&running, 120));
        assert!(t.elapsed().as_millis() >= 100);
    }

    /// THE FIRST ERROR IS THE CAUSE; THE REST ARE CONSEQUENCES.
    ///
    /// cpal can fire the error callback repeatedly while a device tears down, and
    /// the last message in that burst is usually the least informative. Reporting
    /// it would replace "the device was unplugged" with something generic.
    #[test]
    fn the_first_reason_is_the_one_kept() {
        let stop = AtomicBool::new(false);
        let sink = Mutex::new(None);
        note_stream_error(&stop, &sink, "device unplugged".into());
        note_stream_error(&stop, &sink, "backend error".into());
        assert_eq!(sink.lock().unwrap().as_deref(), Some("device unplugged"));
    }

    /// A SECOND RECORDING MAY NEVER OVERWRITE THE FIRST.
    ///
    /// `RELAY_RECORD_WAV` names one path and the capture thread truncated it on
    /// every Stop, so a second Start/Stop cycle destroyed the first segment. On
    /// 2026-09-06 that turned 1837.8 s of a real service into a valid-looking
    /// 170.0 s file — worse than losing it outright, because it looks like it
    /// worked.
    #[test]
    fn a_recording_never_lands_on_a_file_that_already_exists() {
        let dir = std::env::temp_dir().join(format!("relay-rec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let asked = dir.join("service.wav");

        // Nothing there yet: the operator gets the name they asked for.
        assert_eq!(free_recording_path(&asked), asked);

        std::fs::write(&asked, b"first segment").expect("write");
        let second = free_recording_path(&asked);
        assert_ne!(second, asked, "the first segment must survive");
        assert_eq!(second.file_name().unwrap(), "service-2.wav");

        std::fs::write(&second, b"second segment").expect("write");
        assert_eq!(
            free_recording_path(&asked).file_name().unwrap(),
            "service-3.wav"
        );

        // The first segment is still exactly what it was.
        assert_eq!(std::fs::read(&asked).expect("read"), b"first segment");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A LOG LINE THAT NAMES THE INPUT, AND SAYS WHETHER ANYBODY CHOSE IT.
    ///
    /// RG-122. `audio: capture @ 48000 Hz · denoise on (RNNoise)` was the whole
    /// startup record, and both candidate inputs on 2026-09-06 ran at 48 kHz, so
    /// `FIELD.md` (was `FIELD-2026-09-06.md`) had to state its input on the operator's word alone.
    /// The default-ness is the half RG-121 needs: a launch that silently fell back
    /// to the system default reads identically to one an operator set up, and only
    /// this phrase distinguishes them.
    #[test]
    fn the_log_says_which_input_and_whether_it_was_chosen() {
        assert_eq!(
            describe_input(&Input {
                name: "Blackmagic Web Presenter 4K".into(),
                was_default: false
            }),
            "\"Blackmagic Web Presenter 4K\" (chosen)"
        );
        assert_eq!(
            describe_input(&Input {
                name: "MacBook Pro Microphone".into(),
                was_default: true
            }),
            "\"MacBook Pro Microphone\" (system default)"
        );
    }

    /// AN INPUT NOBODY OPENED IS AN ABSENCE, NOT THE DEFAULT.
    ///
    /// `latency.rs` learned this distinction the hard way and it is the same one:
    /// a diagnostic bundle that named a device Relay had never opened would be the
    /// exact false confidence this record exists to remove. Nothing in this test
    /// binary starts capture, so the global must still be empty.
    #[test]
    fn nothing_is_claimed_about_an_input_before_capture_opens_one() {
        assert_eq!(last_input(), None);
    }

    #[test]
    fn rms_of_silence_is_zero() {
        assert_eq!(rms(&[0.0; 512]), 0.0);
        assert_eq!(rms(&[]), 0.0);
    }

    #[test]
    fn rms_of_full_scale_is_one() {
        let sig: Vec<f32> = (0..100)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        assert!((rms(&sig) - 1.0).abs() < 1e-6);
    }

    /// Feed a level for `n` chunks, return the last verdict.
    fn hold(vad: &mut Vad, level: f32, n: usize) -> bool {
        let mut v = false;
        for _ in 0..n {
            v = vad.is_voice(level, None);
        }
        v
    }

    #[test]
    fn vad_ignores_a_steady_room() {
        // Room tone, whatever its level, is not speech. The gate learns it.
        let mut vad = Vad::new(VAD_RMS_THRESHOLD);
        assert!(!hold(&mut vad, 0.004, 40));
        assert!(!vad.is_voice(0.0, None));
    }

    #[test]
    fn vad_hears_a_quiet_preacher_over_a_quiet_room() {
        // THE BUG. A feed sitting at 0.004 with speech peaking at 0.02 never once
        // crossed the old fixed 0.008 line often enough to be heard — 90% of a real
        // sermon was classified as silence and thrown away. Speech is not a level; it
        // is a RISE above whatever the room is doing.
        let mut vad = Vad::new(VAD_RMS_THRESHOLD);
        hold(&mut vad, 0.004, 40); // settle on the room
        assert!(
            vad.is_voice(0.02, None),
            "quiet speech over a quiet room is speech"
        );
    }

    #[test]
    fn vad_is_not_deafened_by_a_loud_room() {
        // And the converse: a loud room (a fan, a band packing down) must not become
        // the new "speech". Same 5x rise, ten times the noise floor.
        let mut vad = Vad::new(VAD_RMS_THRESHOLD);
        hold(&mut vad, 0.04, 40);
        assert!(
            !vad.is_voice(0.05, None),
            "room tone is not speech, however loud"
        );
        assert!(
            vad.is_voice(0.20, None),
            "speech above a loud room still reads as speech"
        );
    }

    #[test]
    fn vad_holds_through_a_dip_between_words() {
        // Hysteresis. The gap between two words, or a stop consonant, dips the energy
        // for a chunk or two. Slamming the gate shut there is what chopped sentences
        // into fragments and made whisper transcribe "John, 3, 6, Linn."
        let mut vad = Vad::new(VAD_RMS_THRESHOLD);
        hold(&mut vad, 0.004, 40);
        assert!(vad.is_voice(0.05, None)); // speaking
        assert!(
            vad.is_voice(0.012, None),
            "a dip mid-sentence must not close the gate"
        );
        // But a real pause, well down toward the floor, does close it.
        assert!(!vad.is_voice(0.005, None));
    }

    #[test]
    fn vad_does_not_hallucinate_speech_from_a_dead_mic() {
        // An unplugged input tracks its noise floor toward zero. Without the absolute
        // floor, the gate's own ratios would then treat dither as a sermon.
        let mut vad = Vad::new(VAD_RMS_THRESHOLD);
        assert!(!hold(&mut vad, 0.0, 60));
        assert!(
            !vad.is_voice(0.0002, None),
            "dither on a dead mic is not speech"
        );
    }

    // ── THE NEURAL VETO ───────────────────────────────────────────────────
    //
    // Reported from a real service: "the transcript is getting Chinese words and
    // other languages that aren't heard". An energy gate cannot tell a voice from
    // a sound, so a door, a chair or a music bed opens it, whisper is handed audio
    // containing no words, and it answers with the most likely token sequence —
    // subtitle boilerplate, often not in English.

    #[test]
    fn loud_non_speech_no_longer_opens_the_gate() {
        let mut vad = Vad::new(VAD_RMS_THRESHOLD);
        vad.is_voice(0.001, Some(0.0)); // seed the floor on a quiet room
                                        // A door slam: plenty of energy, and RNNoise is sure there is no voice.
        assert!(
            !vad.is_voice(0.20, Some(0.02)),
            "loud non-speech opened the gate — whisper would be asked to transcribe a bang"
        );
    }

    #[test]
    fn speech_still_opens_the_gate() {
        let mut vad = Vad::new(VAD_RMS_THRESHOLD);
        vad.is_voice(0.001, Some(0.0));
        assert!(
            vad.is_voice(0.20, Some(0.9)),
            "clear speech must open the gate"
        );
    }

    #[test]
    fn an_unsure_model_does_not_silence_a_quiet_preacher() {
        // THE REGRESSION THIS GUARDS. Relay has already shipped one gate that was
        // silently deaf to a quiet voice; a high probability bar would rebuild it
        // in a new place. RNNoise is unsure about plenty of real preaching.
        let mut vad = Vad::new(VAD_RMS_THRESHOLD);
        vad.is_voice(0.001, Some(0.0));
        assert!(
            vad.is_voice(0.20, Some(SPEECH_OPEN_MIN)),
            "a merely-unsure model must not veto audible speech"
        );
    }

    #[test]
    fn the_veto_can_never_cut_off_a_sentence_in_progress() {
        // THE MOST IMPORTANT ONE. Once speaking, only energy decides. If a low
        // probability could close the gate, every moment the model was unsure —
        // a shout, a whisper, a sung line — would chop the sentence, which is
        // exactly what `stt.rs`'s "append every chunk" rule exists to prevent.
        let mut vad = Vad::new(VAD_RMS_THRESHOLD);
        vad.is_voice(0.001, Some(0.0));
        assert!(vad.is_voice(0.20, Some(0.9)), "should be speaking");
        assert!(
            vad.is_voice(0.20, Some(0.0)),
            "a mid-utterance probability dip closed the gate"
        );
    }

    #[test]
    fn without_a_real_neural_vad_behaviour_is_unchanged() {
        // Below 48 kHz `speech_prob` is an energy proxy, so it is passed as None
        // and the gate behaves exactly as it did before this change.
        let mut a = Vad::new(VAD_RMS_THRESHOLD);
        let mut b = Vad::new(VAD_RMS_THRESHOLD);
        for lvl in [0.001f32, 0.02, 0.2, 0.05, 0.001] {
            assert_eq!(a.is_voice(lvl, None), b.is_voice(lvl, None));
        }
    }

    #[test]
    fn chunker_emits_overlapping_chunks_with_timestamps() {
        // 1000 Hz sample rate → chunk 400ms = 400 samples, hop 200ms = 200.
        let mut c = Chunker::new(1000, 400, 200);
        // Feed 800 samples at once.
        let input: Vec<f32> = (0..800).map(|i| i as f32).collect();
        let chunks = c.push(&input);
        // Chunk starts at 0,200,400 → three complete 400-sample chunks fit.
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].0.len(), 400);
        assert_eq!(chunks[0].1, 0); // ts 0ms
        assert_eq!(chunks[1].1, 200); // 200 samples @1000Hz = 200ms
        assert_eq!(chunks[2].1, 400);
        // Overlap: chunk[1] starts where chunk[0] hopped to.
        assert_eq!(chunks[1].0[0], 200.0);
        assert_eq!(chunks[0].0[200], chunks[1].0[0]);
    }

    #[test]
    fn chunker_accumulates_across_pushes() {
        let mut c = Chunker::new(1000, 400, 200);
        assert_eq!(c.push(&vec![0.0; 100]).len(), 0); // not enough yet
        assert_eq!(c.push(&vec![0.0; 100]).len(), 0); // 200, still < 400
        assert_eq!(c.push(&vec![0.0; 200]).len(), 1); // 400 → one chunk
    }

    #[test]
    fn downmix_averages_stereo_to_mono() {
        // interleaved L,R: (1.0,-1.0),(0.5,0.5) → 0.0, 0.5
        let mono = downmix_f32(&[1.0, -1.0, 0.5, 0.5], 2);
        assert_eq!(mono, vec![0.0, 0.5]);
    }

    // Hardware/permission-dependent — run manually:
    //   cargo test smoke_capture -- --ignored --nocapture
    #[test]
    #[ignore]
    fn smoke_capture() {
        use std::sync::atomic::AtomicUsize;
        eprintln!("devices: {:?}", list_input_devices());
        let count = Arc::new(AtomicUsize::new(0));
        let c2 = count.clone();
        let engine = AudioEngine::start(
            None,
            move |_chunk| {
                c2.fetch_add(1, Ordering::Relaxed);
            },
            |q| eprintln!("quality: {q:?}"),
            |e| eprintln!("audio error: {e}"),
        );
        std::thread::sleep(std::time::Duration::from_secs(3));
        engine.stop();
        eprintln!("chunks captured: {}", count.load(Ordering::Relaxed));
    }
}

/// What the VAD actually sees, measured on real speech.
///
/// ```text
/// RELAY_BENCH_WAV=/path/speech.f32 \
///   cargo test audio::gate -- --ignored --nocapture
/// ```
///
/// The energy gate decides which audio whisper is allowed to hear, and getting it
/// wrong is invisible from the code — the threshold is just a number, and whether
/// speech clears it depends entirely on the microphone in the room. So it gets
/// measured against a real waveform, pushed through the REAL front-end, the REAL
/// chunker and the REAL `Vad`, exactly as the capture thread does it.
#[cfg(test)]
mod gate {
    use super::*;
    use crate::dsp::FrontEnd;

    /// Locate the real `data` chunk. A fixed 44-byte skip is wrong for any WAV with
    /// extra chunks, and the header bytes then arrive as absurd float samples — which
    /// is not merely noisy, it poisons the level trackers on the very first frame.
    fn load_f32(path: &str) -> Vec<f32> {
        let bytes = std::fs::read(path).expect("read wav");
        let mut start = 0usize;
        if bytes.starts_with(b"RIFF") {
            let mut i = 12;
            while i + 8 <= bytes.len() {
                let id = &bytes[i..i + 4];
                let sz =
                    u32::from_le_bytes([bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]])
                        as usize;
                if id == b"data" {
                    start = i + 8;
                    break;
                }
                i += 8 + sz + (sz & 1);
            }
        }
        let (frames, _tail) = bytes[start..].as_chunks::<4>();
        frames
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .filter(|v| v.is_finite())
            .collect()
    }

    /// RG-77 — the debug recorder is the pilot's single highest-value instruction,
    /// and its WAV writer had no test at all.
    ///
    /// `RELAY_RECORD_WAV` is what turns *"word error rate has never been measured in
    /// any language"* — the sentence the moat's 3/10 rests on — into a number.
    /// `KNOWN_ISSUES.md` §1 calls it the highest-value line on that page, and `RELAY_GAP.md`
    /// §24 makes it a condition of the supervised pilot.
    ///
    /// The writer underneath is a hand-rolled 44-byte RIFF header. **A single wrong
    /// field produces a file that opens as noise, or does not open at all — and the
    /// church has already given the service.** There is no second take.
    ///
    /// Round-trips through this module's own `load_f32`, which is the reader every
    /// bench in here uses, so the format is checked by the thing that consumes it.
    #[test]
    fn the_debug_recorder_writes_a_wav_that_can_be_read_back() {
        let dir = std::env::temp_dir().join(format!("relay-wav-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tmpdir");
        let path = dir.join("rec.wav");

        // Values a real capture produces: full scale, silence, and negatives.
        let samples: Vec<f32> = (0..2048)
            .map(|i| ((i as f32) / 2048.0 * std::f32::consts::TAU).sin() * 0.75)
            .collect();
        write_wav_f32(&path, &samples, 48_000).expect("write");

        let bytes = std::fs::read(&path).expect("read back");
        assert!(bytes.starts_with(b"RIFF"), "not a RIFF file");
        assert_eq!(&bytes[8..12], b"WAVE", "not a WAVE file");
        // Format 3 = IEEE float, mono, 48 kHz, 32-bit. A player that trusts the header
        // and gets these wrong renders the sermon as static.
        assert_eq!(
            u16::from_le_bytes([bytes[20], bytes[21]]),
            3,
            "format must be IEEE float"
        );
        assert_eq!(
            u16::from_le_bytes([bytes[22], bytes[23]]),
            1,
            "must be mono"
        );
        assert_eq!(
            u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
            48_000,
            "sample rate in the header must be the capture rate"
        );
        assert_eq!(
            u16::from_le_bytes([bytes[34], bytes[35]]),
            32,
            "32 bits per sample"
        );
        // The RIFF size field is `36 + data`, not the file length — getting this wrong
        // is the classic hand-rolled-header bug and truncates playback.
        assert_eq!(
            u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize,
            36 + samples.len() * 4,
            "RIFF size field"
        );

        let read = load_f32(path.to_str().unwrap());
        assert_eq!(
            read.len(),
            samples.len(),
            "sample count survived the round trip"
        );
        for (a, b) in samples.iter().zip(read.iter()) {
            assert!((a - b).abs() < f32::EPSILON, "sample changed: {a} != {b}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore]
    fn voiced_ratio_on_real_speech() {
        let Some(wav) = std::env::var_os("RELAY_BENCH_WAV") else {
            eprintln!("set RELAY_BENCH_WAV");
            return;
        };
        let sr = 16_000u32;
        let mut audio = load_f32(wav.to_str().unwrap());
        // Simulate the microphone actually in the room. A church laptop's built-in
        // mic, or a lightly-driven feed off the desk, is many times quieter than a
        // clean studio waveform — and that is the input the gate has to survive.
        let scale: f32 = std::env::var("RELAY_BENCH_SCALE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0);
        for s in audio.iter_mut() {
            *s *= scale;
        }

        let mut frontend = FrontEnd::new(sr);
        let mut chunker = Chunker::new(sr, CHUNK_MS, HOP_MS);
        let mut vad = Vad::new(VAD_RMS_THRESHOLD);

        let mut levels: Vec<f32> = Vec::new();
        let mut probs: Vec<f32> = Vec::new();
        let mut noises: Vec<f32> = Vec::new();
        let mut peaks: Vec<f32> = Vec::new();
        // Feed it the way cpal does: in small blocks, not one giant slice.
        for block in audio.chunks(1024) {
            let cleaned = frontend.process(block);
            probs.push(cleaned.quality.speech_prob);
            noises.push(cleaned.quality.noise_level);
            peaks.push(cleaned.quality.peak_level);
            for (chunk, _ts) in chunker.push(&cleaned.samples) {
                levels.push(rms(&chunk));
            }
        }
        let maxp = probs.iter().cloned().fold(0.0f32, f32::max);
        let hi = probs.iter().filter(|&&p| p >= 0.55).count();
        println!(
            "  agc: speech_prob max={maxp:.2}  frames>=0.55: {hi}/{}  noise[first={:.5} last={:.5} min={:.5}]  peak[max={:.5} last={:.5}]",
            probs.len(),
            noises.first().copied().unwrap_or(0.0),
            noises.last().copied().unwrap_or(0.0),
            noises.iter().cloned().fold(f32::MAX, f32::min),
            peaks.iter().cloned().fold(0.0f32, f32::max),
            peaks.last().copied().unwrap_or(0.0),
        );

        let voiced = levels.iter().filter(|&&l| vad.is_voice(l, None)).count();
        let mut sorted = levels.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pct = |p: f32| sorted[((sorted.len() - 1) as f32 * p) as usize];

        println!(
            "\n  CONTINUOUS SPEECH — {} chunks · input scaled ×{scale}\n",
            levels.len()
        );
        println!("  gate (VAD_RMS_THRESHOLD) = {VAD_RMS_THRESHOLD:.4}");
        println!(
            "  chunk rms   p05={:.4}  p25={:.4}  p50={:.4}  p75={:.4}  p95={:.4}",
            pct(0.05),
            pct(0.25),
            pct(0.50),
            pct(0.75),
            pct(0.95)
        );
        println!(
            "\n  VOICED: {}/{}  ({:.0}%)   <-- this is speech, end to end. It should be high.\n",
            voiced,
            levels.len(),
            voiced as f32 / levels.len() as f32 * 100.0
        );
    }
}

#[cfg(test)]
mod envelope_tests {
    use super::*;

    /// A PEAK IS NOT AN AVERAGE, and this is the difference the operator saw.
    ///
    /// One loud sample inside an otherwise quiet 25 ms slice is a transient: a
    /// consonant, a plosive, a knock on the stand. An rms over the same slice
    /// hides it almost completely, which is how a level history came to be drawn
    /// and labelled as a waveform.
    #[test]
    fn a_single_loud_sample_survives_its_slice() {
        let mut buf = vec![0.02f32; 1600];
        buf[800] = 0.9;
        let env = envelope(&buf, CHUNK_PEAKS);
        assert_eq!(env.len(), CHUNK_PEAKS);
        let loud = env.iter().filter(|v| **v > 0.5).count();
        assert_eq!(loud, 1, "the transient should land in exactly one slice");
        // And the same buffer's rms cannot see it at all.
        assert!(rms(&buf) < 0.05, "rms was {}", rms(&buf));
    }

    /// Symmetric about zero: an envelope is drawn both sides of the centre line,
    /// so the sign of the loudest sample must not change the picture.
    #[test]
    fn the_envelope_is_the_absolute_value() {
        let up = envelope(&[0.0, 0.7, 0.0, 0.1], 2);
        let down = envelope(&[0.0, -0.7, 0.0, -0.1], 2);
        assert_eq!(up, down);
        assert_eq!(up, vec![0.7, 0.1]);
    }

    /// Silence reads as silence, not as an absence. A slice with nothing in it is
    /// a real measurement of a room nobody was talking in.
    #[test]
    fn silence_reads_zero_and_the_shape_is_still_the_full_width() {
        assert_eq!(envelope(&[0.0; 800], CHUNK_PEAKS), vec![0.0; CHUNK_PEAKS]);
    }

    /// Every sample is inside exactly one slice, so nothing the microphone heard
    /// is dropped on the way to the picture and nothing is counted twice.
    #[test]
    fn the_slices_cover_the_whole_buffer_exactly_once() {
        // 1000 samples into 16 slices does not divide evenly, which is the case
        // that loses or repeats samples when the arithmetic is done with a stride.
        let buf: Vec<f32> = (0..1000).map(|i| (i as f32) / 1000.0).collect();
        let env = envelope(&buf, CHUNK_PEAKS);
        assert_eq!(env.len(), CHUNK_PEAKS);
        // The last slice holds the largest sample, the first the smallest.
        assert_eq!(*env.last().unwrap(), buf[999]);
        assert!(
            env[0] < env[1] && env[1] < env[2],
            "slices are not in order"
        );
        // Monotone input means each slice's peak is its own last sample.
        for (i, v) in env.iter().enumerate() {
            let b = buf.len() * (i + 1) / CHUNK_PEAKS;
            assert_eq!(
                *v,
                buf[b - 1],
                "slice {i} did not end where the next begins"
            );
        }
    }

    /// Asked for nothing, answers nothing, rather than dividing by zero.
    #[test]
    fn an_envelope_of_no_slices_is_empty() {
        assert!(envelope(&[0.5, 0.5], 0).is_empty());
        assert_eq!(envelope(&[], 4), vec![0.0; 4]);
    }
}
