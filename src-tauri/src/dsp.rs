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
/// Gain clamp. Raised from 6.0 (+16 dB): a church laptop's built-in mic, or a
/// lightly-driven feed off the desk, can sit around RMS 0.005 — and +16 dB only
/// lifts that to 0.03, a quarter of the target, so the front-end simply gave up
/// short of the goal it exists to reach. The auto-gain is still frozen during
/// pauses (see `apply_agc`), so a bigger ceiling does not mean pumping up room
/// noise between phrases.
const MAX_GAIN: f32 = 24.0;
const MIN_GAIN: f32 = 0.25;
/// Per-frame smoothing for the applied gain (one-pole). Small = gentle, no pump.
const GAIN_SMOOTH: f32 = 0.08;
/// Speech / noise level tracker coefficients (per 10 ms frame).
const SPEECH_ATTACK: f32 = 0.20; // level rises quickly onto speech
/// Noise-floor tracking (per 10 ms frame). Asymmetric on purpose: it drops onto a
/// pause within a fraction of a second, and takes ~20 s to climb — so a long passage
/// of speech cannot teach it that the preacher is the background.
const NOISE_DOWN: f32 = 0.30;
const NOISE_UP: f32 = 0.0005;
/// Recent-peak tracker for the fallback (non-48 kHz) speech probability.
const PEAK_ATTACK: f32 = 0.35; // jump onto a loud frame at once
const PEAK_RELEASE: f32 = 0.002; // ~seconds to decay — outlives a sentence pause
/// How far the loudest recent audio must stand above the quietest before we believe
/// anyone is speaking at all. Below this there is no dynamic range in the signal —
/// it is a room, not a voice.
const SPEECH_CONTRAST: f32 = 3.0;
/// Speech probability above which a frame counts as speech. (There is no separate
/// "noise" threshold any more: the noise floor is a minimum statistic tracked from
/// every frame, and no longer needs to be told which frames are noise — see
/// `apply_agc`.)
const SPEECH_PROB: f32 = 0.55;

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
    /// Tracked noise floor and recent peak (diagnostics).
    pub noise_level: f32,
    pub peak_level: f32,
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
    /// Loudest recent frame. Fast to rise, slow to fall — the reference the fallback
    /// speech probability is measured against, so it is a level the mic in THIS room
    /// actually reaches rather than one the developer hoped for.
    peak_level: f32,
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
            // Unseeded. Taken from the first frame (see apply_agc) rather than assumed:
            // a fixed starting floor is a guess about a room, and if it starts ABOVE the
            // microphone's actual speech level the front-end concludes the room is empty
            // and never recovers.
            noise_level: -1.0,
            peak_level: 0.0,
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
                // Seed the floor from the first frame BEFORE anything reads it. The
                // sentinel used to leak into `energy_prob`, where a negative noise floor
                // made every input — including pure room tone — look like certain speech.
                if self.noise_level < 0.0 {
                    self.noise_level = rms(&din);
                }
                // Track the loudest recent frame BEFORE judging this one against it:
                // fast attack so real speech sets the reference immediately, slow
                // release so a pause between sentences does not drag it down to the
                // noise floor and make room tone look like speech.
                let r = rms(&din);
                if !r.is_finite() {
                    self.in_buf.drain(..FRAME);
                    continue;
                }
                let a = if r > self.peak_level {
                    PEAK_ATTACK
                } else {
                    PEAK_RELEASE
                };
                self.peak_level += a * (r - self.peak_level);
                energy_prob(&din, self.noise_level, self.peak_level)
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
            noise_level: self.noise_level,
            peak_level: self.peak_level,
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
        // One inf/NaN sample — a flaky driver, a device unplugged mid-callback — would
        // otherwise poison every tracker below IRREVERSIBLY: NaN compares false against
        // everything, so the gain, the floor and the peak all latch to NaN and the audio
        // path stays dead for the rest of the service, with no error anywhere.
        if !frame_rms.is_finite() {
            for s in frame.iter_mut() {
                if !s.is_finite() {
                    *s = 0.0;
                }
            }
            return;
        }
        // Also seeded here: the denoise-enabled (48 kHz) path gets its speech
        // probability from RNNoise and never calls energy_prob.
        if self.noise_level < 0.0 {
            self.noise_level = frame_rms;
        }
        self.speech_prob += 0.3 * (vad - self.speech_prob);

        if vad >= SPEECH_PROB {
            self.speech_level += SPEECH_ATTACK * (frame_rms - self.speech_level);
        }

        // The noise floor is a MINIMUM STATISTIC — falls fast, rises very slowly —
        // and is tracked from every frame, with no reference to whether we think this
        // one is speech.
        //
        // It used to rise only on frames the VAD had already called noise, and that
        // was a deadlock: on a quiet microphone the VAD called NOTHING speech, so the
        // preacher's own voice was folded into the noise estimate, which raised the
        // floor, which kept him below the speech threshold, which kept him out of the
        // estimate. The front-end talked itself into believing the room was empty.
        //
        // A floor that only falls quickly and rises over ~20 seconds cannot be dragged
        // up by speech, so it needs no help deciding what speech is — which is the
        // whole point, because deciding that is what it is FOR.
        let a = if frame_rms < self.noise_level {
            NOISE_DOWN
        } else {
            NOISE_UP
        };
        self.noise_level += a * (frame_rms - self.noise_level);

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

/// Fallback speech probability from raw energy, used when the ML denoiser is off
/// (device not at 48 kHz — which includes every 44.1 kHz interface, so this path is
/// common, not exotic).
///
/// RELATIVE to the loudest audio recently seen, never to an absolute level.
///
/// It used to be `rms / TARGET_RMS`, i.e. "how close is this frame to 0.12?", and
/// with `SPEECH_PROB = 0.55` that means a frame had to reach RMS 0.066 before it
/// counted as speech at all. A quiet microphone sitting at 0.005 therefore never
/// produced a single "speech" frame — so `speech_level` never updated, so the
/// auto-gain stayed frozen (`desired = self.gain`) and never lifted it.
///
/// A perfect deadlock: to be granted gain you had to already be loud enough not to
/// need it. The quieter the input, the more certain the front-end was that there was
/// nothing there — the exact opposite of what it is for.
fn energy_prob(frame: &[f32], noise: f32, peak: f32) -> f32 {
    // Speech is CONTRAST, not volume. If the loudest recent audio is barely above
    // the quietest, then whatever this room is doing, nobody is talking — and that
    // holds however loud the room happens to be. Without this, a steady hiss sits at
    // its own recent peak, reads as 100% speech, and the auto-gain amplifies it.
    if peak < noise * SPEECH_CONTRAST {
        return 0.0;
    }
    let r = rms(frame);
    ((r - noise) / (peak - noise).max(EPS)).clamp(0.0, 1.0)
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

    /// A voice is a signal that STARTS AND STOPS. Steady tone plus silence.
    fn speech_like(amp: f32, cycles: usize) -> Vec<f32> {
        let mut v = Vec::new();
        for _ in 0..cycles {
            v.extend(sine(300.0, amp, FRAME * 6)); // ~60ms of "word"
            v.extend(vec![0.0; FRAME * 3]); // ~30ms gap
        }
        v
    }

    #[test]
    fn autogain_lifts_a_quiet_voice_toward_target() {
        // THE BUG, as a test. A church laptop mic sitting around RMS 0.0035 was never
        // recognised as speech at all (the fallback probability was `rms / 0.12`, and
        // needed 0.066 to register), so `speech_level` never updated and the gain was
        // frozen at 1.0. The front-end whose entire job is to lift a quiet feed left it
        // exactly as quiet as it found it — and the transcript downstream turned to
        // nonsense with no error anywhere.
        let mut fe = FrontEnd::new(16_000);
        let quiet = speech_like(0.005, 1);
        let in_rms = rms(&sine(300.0, 0.005, FRAME));
        let mut last = Vec::new();
        for _ in 0..60 {
            last = fe.process(&quiet).samples;
        }
        // Measure the loud part, not the gaps.
        let out_rms = rms(&last[..FRAME * 4]);
        assert!(
            out_rms > in_rms * 4.0,
            "a quiet voice must be lifted, not left where it was: {in_rms} -> {out_rms}"
        );
        assert!(
            out_rms <= TARGET_RMS * 1.2,
            "…and lifted toward the target, not past it: {out_rms}"
        );
    }

    #[test]
    fn autogain_boosts_below_target_speech_toward_target_without_clipping() {
        let mut fe = FrontEnd::new(16_000); // isolate AGC from RNNoise numerics
        let amp = 0.03; // below TARGET_RMS once RMS'd
        let in_rms = rms(&sine(300.0, amp, FRAME));
        assert!(
            in_rms < TARGET_RMS,
            "test setup: input must be below target"
        );
        let mut last = Vec::new();
        for _ in 0..60 {
            last = fe.process(&speech_like(amp, 1)).samples;
        }
        let out_rms = rms(&last[..FRAME * 4]);
        assert!(
            out_rms > in_rms * 1.15,
            "sub-target speech should be boosted: {in_rms} -> {out_rms}"
        );
        assert!(
            out_rms <= TARGET_RMS * 1.2,
            "should settle near target, not overshoot: {out_rms}"
        );
    }

    #[test]
    fn autogain_does_not_pump_steady_room_noise() {
        // Room tone has no dynamic range — it never starts and never stops. It must not
        // be amplified NO MATTER HOW LOUD IT IS, which is why the test is run at a level
        // well above the old fixed "this is speech" line: loudness is not the signal.
        for amp in [0.02f32, 0.08, 0.2] {
            let mut fe = FrontEnd::new(16_000);
            let mut last = Vec::new();
            for _ in 0..80 {
                last = fe.process(&sine(300.0, amp, FRAME)).samples;
            }
            let in_rms = rms(&sine(300.0, amp, FRAME));
            let out_rms = rms(&last);
            assert!(
                out_rms <= in_rms * 1.2,
                "steady room tone at {amp} must not be pumped: {in_rms} -> {out_rms}"
            );
        }
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
