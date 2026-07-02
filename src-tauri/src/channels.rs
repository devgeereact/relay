//! Output channels: render targets for the shared template engine.
//!
//! Single responsibility: given a "show this content" event and a channel's
//! assigned template, render it to that channel's render target. Never
//! special-case behavior per channel type (main/stage/streaming/lobby) —
//! that's what templates are for. See docs/SPEC.md §5 and PROMPT.md Phase 7/10.
//!
//! TODO(phase 7): native_window render target first (simplest — a borderless
//!                fullscreen window pinned to a display).
//! TODO(phase 10): ndi_encode and network_client (WebSocket kiosk) targets.

pub enum RenderTarget {
    NativeWindow,
    NdiEncode,
    NetworkClient,
}
