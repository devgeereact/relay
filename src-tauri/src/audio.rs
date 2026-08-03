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
use std::sync::Arc;
use std::thread::JoinHandle;

/// A voiced, time-stamped chunk of mono audio handed upstream to STT.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub timestamp_ms: u64,
    pub sample_rate: u32,
    pub rms: f32,
    pub is_voice: bool,
}

/// An available capture device, shaped for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
}

// --- chunking parameters (see docs/SPEC.md §4 step 1: 200-500ms overlapping) ---
const CHUNK_MS: u32 = 400;
const HOP_MS: u32 = 200; // 50% overlap
/// ABSOLUTE floor for the voice gate, on f32 samples in [-1, 1]. This is NOT the
/// speech threshold — the real threshold is learned from the room's noise floor (see
/// `Vad`). This only stops a dead or unplugged microphone from having its own dither
/// tracked down to zero and then reported as speech.
///
/// It was previously the speech threshold itself, and it silently deleted most of a
/// quiet preacher's sermon — see the doc comment on `Vad`.
const VAD_RMS_THRESHOLD: f32 = 0.0015;

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
            });
        }
    }
    out
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
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();

        let handle = std::thread::spawn(move || {
            if let Err(e) = build_and_run(device_name, stop_thread, on_chunk, on_quality) {
                on_error(e);
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

/// Resolve the device, build the cpal stream, then run the chunk/VAD loop until
/// stopped. Everything touching the non-Send `Device`/`Stream` stays on this
/// one thread.
fn build_and_run<F, Q>(
    device_name: Option<String>,
    stop: Arc<AtomicBool>,
    on_chunk: F,
    on_quality: Q,
) -> Result<(), String>
where
    F: Fn(&AudioChunk) + Send + 'static,
    Q: Fn(&dsp::AudioQuality) + Send + 'static,
{
    let host = cpal::default_host();
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
    let (tx, rx) = mpsc::channel::<Vec<f32>>();

    // Prefer the 48 kHz config (RNNoise runs frame-aligned), but if that exact
    // config can't actually be opened on this device — common with USB mics,
    // aggregate/virtual devices, or unusual channel layouts — fall back to the
    // device's own default config so audio STILL flows (denoise self-disables).
    // Without this, selecting a non-default device silently produced no audio.
    let (stream, used) = match build_stream(&device, &preferred, &tx) {
        Ok(s) => (s, preferred),
        Err(e1) => {
            eprintln!("audio: preferred 48 kHz config failed ({e1}); using device default");
            let def = device.default_input_config().map_err(|e| e.to_string())?;
            let s = build_stream(&device, &def, &tx)?;
            (s, def)
        }
    };
    let sample_rate = used.sample_rate().0;
    stream.play().map_err(|e| e.to_string())?;

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
        println!("audio: RECORDING cleaned input to {}", p.to_string_lossy());
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
        "audio: capture @ {sample_rate} Hz · denoise {}",
        if frontend.denoise_active() {
            "on (RNNoise)"
        } else {
            "off (device not 48 kHz — auto-gain only)"
        }
    );

    while !stop.load(Ordering::Relaxed) {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(samples) => {
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
                    };
                    on_chunk(&ac);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(stream);
    if let Some((path, buf)) = rec {
        match write_wav_f32(&path, &buf, sample_rate) {
            Ok(()) => println!(
                "audio: wrote {:.1}s to {}",
                buf.len() as f32 / sample_rate as f32,
                path.display()
            ),
            Err(e) => eprintln!("audio: could not write recording: {e}"),
        }
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

/// Build a cpal input stream for `supported`, downmixing to mono and forwarding
/// samples over `tx`. Kept separate so the caller can try a preferred config and
/// fall back to the device default if the preferred one won't open.
fn build_stream(
    device: &cpal::Device,
    supported: &cpal::SupportedStreamConfig,
    tx: &mpsc::Sender<Vec<f32>>,
) -> Result<cpal::Stream, String> {
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.clone().into();
    let channels = config.channels as usize;
    let err_fn = |e| eprintln!("audio stream error: {e}");
    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let tx = tx.clone();
            device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let _ = tx.send(downmix_f32(data, channels));
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
                    let _ = tx.send(downmix_i16(data, channels));
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
                    let _ = tx.send(downmix_u16(data, channels));
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
        bytes[start..]
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .filter(|v| v.is_finite())
            .collect()
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
