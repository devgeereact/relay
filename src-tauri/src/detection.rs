//! Verse/content detection: direct match, semantic match, context memory.
//!
//! Single responsibility: given a rolling transcript window, return zero or
//! more candidate verse detections with a confidence score and method
//! ("direct" or "semantic"). Does NOT decide what to do with a detection —
//! that's router.rs. See docs/SPEC.md §4 and PROMPT.md Phase 5/9.
//!
//! TODO(phase 5): direct pattern match (regex + book-name alias tables).
//! TODO(phase 9): semantic match (embeddings + vector search) and
//!                context-memory ("current passage" state).

pub enum DetectionMethod {
    Direct,
    Semantic,
}

pub struct Detection {
    pub verse_id: i64,
    pub confidence: f32,
    pub method: DetectionMethod,
}
