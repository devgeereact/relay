//! Audio capture + voice-activity-detection gate.
//!
//! Single responsibility: turn a raw microphone/mixer input into a stream of
//! 200-500ms overlapping audio chunks, with silence already filtered out by
//! VAD. This module knows nothing about transcription or detection — it only
//! hands clean audio chunks upstream. See PROMPT.md Phase 3.
//!
//! TODO(phase 3): list input devices, capture stream, implement chunking + VAD.

pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub timestamp_ms: u64,
}

pub fn list_input_devices() -> Vec<String> {
    // TODO: enumerate real devices via cpal or similar.
    vec![]
}
