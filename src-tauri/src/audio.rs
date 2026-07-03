//! Audio capture + voice-activity-detection gate.
//!
//! Single responsibility: turn a raw microphone/mixer input into a stream of
//! 200-500ms overlapping audio chunks, with silence already filtered out by
//! VAD. This module knows nothing about transcription or detection — it only
//! hands clean audio chunks upstream. See PROMPT.md Phase 3.
//!
//! Design: one dedicated capture thread owns the (non-Send) cpal stream. The
//! stream's realtime callback does the minimum — downmix to mono, forward
//! samples over a channel — and the same thread runs chunking + VAD off the
//! realtime path. Nothing here `unwrap()`s on a running path (CLAUDE.md); a
//! device failure surfaces to the caller as an error string.

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
/// Seed RMS gate for voiced audio on f32 samples in [-1, 1]. Deliberately a
/// plain energy gate for Phase 3 — a real VAD (webrtc/silero) slots in behind
/// the same `Vad` seam later without touching capture or chunking.
const VAD_RMS_THRESHOLD: f32 = 0.008;

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

/// Energy-based voice-activity gate.
#[derive(Debug, Clone, Copy)]
pub struct Vad {
    pub threshold_rms: f32,
}
impl Vad {
    pub fn is_voice(&self, rms: f32) -> bool {
        rms >= self.threshold_rms
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
    pub fn start<F, E>(device_name: Option<String>, on_chunk: F, on_error: E) -> Self
    where
        F: Fn(&AudioChunk) + Send + 'static,
        E: Fn(String) + Send + 'static,
    {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();

        let handle = std::thread::spawn(move || {
            if let Err(e) = build_and_run(device_name, stop_thread, on_chunk) {
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
fn build_and_run<F>(
    device_name: Option<String>,
    stop: Arc<AtomicBool>,
    on_chunk: F,
) -> Result<(), String>
where
    F: Fn(&AudioChunk) + Send + 'static,
{
    eprintln!("audio: build_and_run start");
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
    eprintln!("audio: device = {:?}", device.name());

    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let channels = config.channels as usize;
    let sample_rate = config.sample_rate.0;
    eprintln!("audio: config sr={sample_rate} ch={channels} fmt={sample_format:?}");

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let err_fn = |e| eprintln!("audio stream error: {e}");

    // The realtime callback stays cheap: downmix to mono, forward. No DSP here.
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
                    let mono = downmix_i16(data, channels);
                    let _ = tx.send(mono);
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
                    let mono = downmix_u16(data, channels);
                    let _ = tx.send(mono);
                },
                err_fn,
                None,
            )
        }
        other => return Err(format!("unsupported sample format: {other:?}")),
    }
    .map_err(|e| e.to_string())?;
    eprintln!("audio: stream built");

    stream.play().map_err(|e| e.to_string())?;
    eprintln!("audio: stream playing");

    let vad = Vad {
        threshold_rms: VAD_RMS_THRESHOLD,
    };
    let mut chunker = Chunker::new(sample_rate, CHUNK_MS, HOP_MS);

    while !stop.load(Ordering::Relaxed) {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(samples) => {
                for (chunk, ts_ms) in chunker.push(&samples) {
                    let level = rms(&chunk);
                    let ac = AudioChunk {
                        is_voice: vad.is_voice(level),
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
    Ok(())
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

    #[test]
    fn vad_gates_on_threshold() {
        let vad = Vad {
            threshold_rms: 0.01,
        };
        assert!(!vad.is_voice(0.0));
        assert!(!vad.is_voice(0.009));
        assert!(vad.is_voice(0.01));
        assert!(vad.is_voice(0.5));
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
            |e| eprintln!("audio error: {e}"),
        );
        std::thread::sleep(std::time::Duration::from_secs(3));
        engine.stop();
        eprintln!("chunks captured: {}", count.load(Ordering::Relaxed));
    }
}
