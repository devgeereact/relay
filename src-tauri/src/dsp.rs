//! Audio front-end intelligence: noise suppression, auto-gain, quality metrics.
//!
//! Single responsibility: clean the raw mono capture stream *before* it reaches
//! chunking / VAD / STT, and report a quality signal to the operator. This
//! module knows nothing about capture, transcription, or detection — it takes
//! samples in and hands cleaner samples out. See docs/SPEC.md §4 (the detection
//! pipeline only works as well as the audio feeding it).
//!
//! Pipeline order: `raw → denoise (C1) → auto-gain (C2) → quality (C3)`.
//!
//! RNNoise (via `nnnoiseless`) is the ML denoiser. It runs at a fixed 48 kHz on
//! 480-sample frames, so `audio.rs` prefers a 48 kHz capture config and this
//! front-end runs frame-aligned with no resampling. When the device cannot do
//! 48 kHz, denoise is disabled and only the (sample-rate-agnostic) auto-gain and
//! quality metering run — honest degradation, never a panic (CLAUDE.md: no
//! `unwrap()` on a running path).

use nnnoiseless::DenoiseState;
use serde::Serialize;

/// RNNoise's mandatory sample rate. `audio.rs` prefers a capture config at this
/// rate so the denoiser runs without any resampling.
pub const RNNOISE_RATE: u32 = 48_000;

/// Samples per RNNoise frame (480 @ 48 kHz = 10 ms).
const FRAME: usize = DenoiseState::FRAME_SIZE;

/// RNNoise expects f32 samples in 16-bit PCM range (`[-32768, 32767]`), not the
/// `[-1, 1]` the rest of the pipeline uses. Scale in, scale out.
const I16_SCALE: f32 = 32_768.0;

// --- auto-gain tuning (seed values, like the router thresholds — configuration,
//     not magic constants; tune against real corpus/room data later) ----------
/// Target RMS the auto-gain drives voiced speech toward (~ -18 dBFS).
const TARGET_RMS: f32 = 0.12;
/// Gain clamp: never boost more than +~16 dB (amplifying the noise floor is
/// worse than a quiet-but-clean signal) nor attenuate below -~12 dB.
const MAX_GAIN: f32 = 6.0;
const MIN_GAIN: f32 = 0.25;
/// Per-frame smoothing for the applied gain (one-pole). Small = gentle, no pump.
const GAIN_SMOOTH: f32 = 0.08;
/// Speech / noise level tracker coefficients (per 10 ms frame).
const SPEECH_ATTACK: f32 = 0.20; // level rises quickly onto speech
const NOISE_TRACK: f32 = 0.03; // noise floor adapts slowly
/// RNNoise VAD probability above/below which a frame counts as speech / noise.
const SPEECH_PROB: f32 = 0.55;
const NOISE_PROB: f32 = 0.35;

// --- quality thresholds ------------------------------------------------------
/// Fraction of near-full-scale samples above which we warn the operator the
/// input is clipping.
const CLIP_WARN: f32 = 0.02;
/// Speech level below which the mic is likely off / muted / far away.
const QUIET_LEVEL: f32 = 0.004;
/// Speech-to-noise ratio (dB) below which detection accuracy will suffer.
const SNR_WARN_DB: f32 = 6.0;

const EPS: f32 = 1e-6;

/// A quality warning surfaced to the operator as *data* — never a rendering
/// decision. The console may show it or ignore it; the pipeline keeps running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityWarning {
    /// Input is too hot — samples hitting full scale.
    Clipping,
    /// Almost no signal — mic likely off, muted, or too far.
    TooQuiet,
    /// Poor speech-to-noise ratio — detection will struggle.
    Noisy,
}

/// Per-block audio-quality snapshot. Additive: emitted on its own channel and
/// safe for the UI to ignore.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct AudioQuality {
    /// Pre-clean input level (RMS of the raw block).
    pub input_rms: f32,
    /// Fraction of raw samples at/near full scale.
    pub clip_ratio: f32,
    /// Smoothed speech probability (0..1).
    pub speech_prob: f32,
    /// Rough speech-vs-noise ratio in dB.
    pub snr_db: f32,
    /// Whether denoise is actually running (false = device not at 48 kHz).
    pub denoise: bool,
    /// The single most important thing wrong right now, if anything.
    pub warning: Option<QualityWarning>,
}

/// The audio front-end. Owns the RNNoise state and the auto-gain trackers, and
/// buffers input into 480-sample frames. Lives on the single capture/processing
/// thread — not `Send`-shared, no locking.
pub struct FrontEnd {
    denoise: Box<DenoiseState<'static>>,
    denoise_enabled: bool,
    /// Pending input samples not yet forming a full frame.
    in_buf: Vec<f32>,
    /// Smoothed trackers.
    speech_prob: f32,
    speech_level: f32,
    noise_level: f32,
    gain: f32,
}

/// Result of one `process` call: the cleaned samples ready for chunking, plus
/// the quality snapshot for that block.
pub struct ProcessOut {
    pub samples: Vec<f32>,
    pub quality: AudioQuality,
}

impl FrontEnd {
    /// Build a front-end for a stream at `sample_rate`. Denoise only runs when
    /// the rate is exactly 48 kHz (RNNoise's requirement); otherwise it is
    /// disabled and only auto-gain + metering run.
    pub fn new(sample_rate: u32) -> Self {
        FrontEnd {
            denoise: DenoiseState::new(),
            denoise_enabled: sample_rate == RNNOISE_RATE,
            in_buf: Vec::with_capacity(FRAME * 4),
            speech_prob: 0.0,
            speech_level: 0.0,
            noise_level: QUIET_LEVEL,
            gain: 1.0,
        }
    }

    /// True when the ML denoiser is active for this stream.
    pub fn denoise_active(&self) -> bool {
        self.denoise_enabled
    }

    /// Clean a block of mono samples. Returns as many cleaned samples as are
    /// ready (buffers up to one sub-480 remainder internally, so output tracks
    /// input 1:1 in steady state with ~10 ms of latency).
    pub fn process(&mut self, input: &[f32]) -> ProcessOut {
        let (input_rms, clip_ratio) = block_stats(input);
        self.in_buf.extend_from_slice(input);

        let mut out = Vec::with_capacity(self.in_buf.len());
        let mut din = [0f32; FRAME];
        let mut dout = [0f32; FRAME];

        while self.in_buf.len() >= FRAME {
            din.copy_from_slice(&self.in_buf[..FRAME]);

            let vad = if self.denoise_enabled {
                for x in din.iter_mut() {
                    *x *= I16_SCALE;
                }
                let p = self.denoise.process_frame(&mut dout, &din);
                for x in dout.iter_mut() {
                    *x /= I16_SCALE;
                }
                p
            } else {
                dout.copy_from_slice(&din);
                energy_prob(&din)
            };

            self.in_buf.drain(..FRAME);
            self.apply_agc(&mut dout, vad);
            out.extend_from_slice(&dout);
        }

        let snr_db = 20.0 * (self.speech_level.max(EPS) / self.noise_level.max(EPS)).log10();
        let quality = AudioQuality {
            input_rms,
            clip_ratio,
            speech_prob: self.speech_prob,
            snr_db,
            denoise: self.denoise_enabled,
            warning: self.warning(clip_ratio, snr_db),
        };
        ProcessOut {
            samples: out,
            quality,
        }
    }

    /// Update the level trackers from this frame and apply smoothed gain in
    /// place, clamping to avoid overflow. Gain is frozen during noise/pauses so
    /// the front-end never pumps up the noise floor between phrases.
    fn apply_agc(&mut self, frame: &mut [f32], vad: f32) {
        let frame_rms = rms(frame);
        self.speech_prob += 0.3 * (vad - self.speech_prob);

        if vad >= SPEECH_PROB {
            self.speech_level += SPEECH_ATTACK * (frame_rms - self.speech_level);
        } else if vad <= NOISE_PROB {
            self.noise_level += NOISE_TRACK * (frame_rms - self.noise_level);
        }

        let mut desired = if self.speech_level > EPS {
            (TARGET_RMS / self.speech_level).clamp(MIN_GAIN, MAX_GAIN)
        } else {
            self.gain
        };
        // Don't raise gain while we're not hearing speech — that would just
        // amplify room noise during the pastor's pauses.
        if vad < SPEECH_PROB && desired > self.gain {
            desired = self.gain;
        }
        self.gain += GAIN_SMOOTH * (desired - self.gain);

        for s in frame.iter_mut() {
            *s = (*s * self.gain).clamp(-1.0, 1.0);
        }
    }

    /// Pick the single most actionable warning, if any.
    fn warning(&self, clip_ratio: f32, snr_db: f32) -> Option<QualityWarning> {
        if clip_ratio > CLIP_WARN {
            Some(QualityWarning::Clipping)
        } else if self.speech_level < QUIET_LEVEL {
            Some(QualityWarning::TooQuiet)
        } else if snr_db < SNR_WARN_DB {
            Some(QualityWarning::Noisy)
        } else {
            None
        }
    }
}

/// RMS amplitude of a sample block. (Mirrors `audio::rms`; kept local so this
/// module stays independent of the capture module.)
fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// RMS plus the fraction of samples at/near full scale (clipping estimate).
fn block_stats(samples: &[f32]) -> (f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let mut sum_sq = 0.0f32;
    let mut clipped = 0usize;
    for &s in samples {
        sum_sq += s * s;
        if s.abs() >= 0.98 {
            clipped += 1;
        }
    }
    let n = samples.len() as f32;
    ((sum_sq / n).sqrt(), clipped as f32 / n)
}

/// Fallback speech probability from raw energy, used only when the ML denoiser
/// is disabled (device not at 48 kHz). Rough — just enough to gate the auto-gain.
fn energy_prob(frame: &[f32]) -> f32 {
    (rms(frame) / TARGET_RMS).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sine at `freq` Hz, `amp` peak, `n` samples at 48 kHz.
    fn sine(freq: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| amp * (std::f32::consts::TAU * freq * i as f32 / RNNOISE_RATE as f32).sin())
            .collect()
    }

    #[test]
    fn buffers_sub_frame_input_then_emits() {
        let mut fe = FrontEnd::new(RNNOISE_RATE);
        // Fewer than one frame → nothing out yet, held in the buffer.
        let out = fe.process(&vec![0.1; FRAME - 1]);
        assert!(out.samples.is_empty());
        // One more sample completes exactly one frame.
        let out = fe.process(&[0.1]);
        assert_eq!(out.samples.len(), FRAME);
    }

    #[test]
    fn steady_state_output_tracks_input_one_to_one() {
        let mut fe = FrontEnd::new(RNNOISE_RATE);
        let mut total_in = 0usize;
        let mut total_out = 0usize;
        for _ in 0..20 {
            let block = sine(220.0, 0.2, 1000);
            total_in += block.len();
            total_out += fe.process(&block).samples.len();
        }
        // Output lags by at most one partial frame — never drops or duplicates.
        assert!(total_in - total_out < FRAME);
    }

    #[test]
    fn silence_stays_quiet_no_noise_pumping() {
        // Denoise off (16k) isolates the auto-gain: pure silence must not be
        // amplified into anything.
        let mut fe = FrontEnd::new(16_000);
        let out = fe.process(&vec![0.0; FRAME * 8]);
        assert!(out.samples.iter().all(|s| s.abs() < 1e-6));
    }

    #[test]
    fn autogain_boosts_below_target_speech_toward_target_without_clipping() {
        let mut fe = FrontEnd::new(16_000); // isolate AGC from RNNoise numerics
                                            // A tone loud enough to clear the (energy-only, fallback) speech gate but
                                            // below TARGET_RMS — the auto-gain should lift it toward the target.
                                            // amp 0.12 → rms ~0.085 < TARGET_RMS 0.12.
        let amp = 0.12;
        let in_rms = rms(&sine(300.0, amp, FRAME));
        assert!(
            in_rms < TARGET_RMS,
            "test setup: input must be below target"
        );
        let mut last = Vec::new();
        for _ in 0..80 {
            last = fe.process(&sine(300.0, amp, FRAME)).samples;
        }
        let out_rms = rms(&last);
        assert!(
            out_rms > in_rms * 1.15,
            "sub-target speech should be boosted: {in_rms} -> {out_rms}"
        );
        assert!(
            out_rms <= TARGET_RMS * 1.15,
            "should settle near target, not overshoot: {out_rms}"
        );
        assert!(
            last.iter().all(|s| s.abs() <= 1.0),
            "must never exceed full scale"
        );
    }

    #[test]
    fn autogain_does_not_boost_sub_speech_level_noise() {
        // A very quiet tone reads as noise to the energy-only fallback VAD, so the
        // anti-pump guard must leave it alone rather than amplify the floor.
        let mut fe = FrontEnd::new(16_000);
        let mut last = Vec::new();
        for _ in 0..80 {
            last = fe.process(&sine(300.0, 0.02, FRAME)).samples;
        }
        let out_rms = rms(&last);
        assert!(
            out_rms <= 0.03,
            "near-silence must not be pumped up: {out_rms}"
        );
    }

    #[test]
    fn flags_clipping_on_hot_input() {
        let mut fe = FrontEnd::new(16_000);
        let hot = vec![1.0f32; FRAME * 2]; // full-scale everywhere
        let q = fe.process(&hot).quality;
        assert_eq!(q.warning, Some(QualityWarning::Clipping));
        assert!(q.clip_ratio > CLIP_WARN);
    }

    #[test]
    fn flags_too_quiet_on_near_silence() {
        let mut fe = FrontEnd::new(16_000);
        let q = fe.process(&vec![0.0005; FRAME * 4]).quality;
        assert_eq!(q.warning, Some(QualityWarning::TooQuiet));
    }

    #[test]
    fn denoise_runs_at_48k_and_produces_finite_output() {
        let mut fe = FrontEnd::new(RNNOISE_RATE);
        assert!(fe.denoise_active());
        let out = fe.process(&sine(440.0, 0.3, FRAME * 4)).samples;
        assert_eq!(out.len(), FRAME * 4);
        assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    }

    #[test]
    fn denoise_disabled_below_48k() {
        assert!(!FrontEnd::new(16_000).denoise_active());
        assert!(!FrontEnd::new(44_100).denoise_active());
        assert!(FrontEnd::new(48_000).denoise_active());
    }
}
