//! Speech-to-text.
//!
//! Single responsibility: turn AudioChunks into a rolling transcript with
//! per-chunk language identification. Local-first (whisper.cpp-class model),
//! optional cloud fallback when online. Never assumes single-language input —
//! code-switching (English mixed with a local language mid-sentence) is the
//! normal case for the target market. See PROMPT.md Phase 4.
//!
//! TODO(phase 4): wire up local model, English only first, then extend to
//! Yoruba / Swahili / Hausa (see docs/DECISIONS.md).

pub struct TranscriptChunk {
    pub text: String,
    pub language: String, // ISO 639-1/3 code
    pub confidence: f32,
    pub is_final: bool,
}
