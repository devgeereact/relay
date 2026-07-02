//! Output channels: render targets for the shared template engine.
//!
//! Single responsibility: given a "show this content" event and a channel's
//! assigned template, render it to that channel's render target. Never
//! special-case behavior per channel type (main/stage/streaming/lobby) — that's
//! what templates are for. See docs/SPEC.md §5 and PROMPT.md Phase 7/10.
//!
//! Phase 7: native_window render target — a borderless fullscreen webview
//! pinned to a display, loading the shared output view (output.html) with a
//! template id in the query. Content is pushed to every open channel via one
//! `output://content` broadcast; each window renders it through its own
//! template. The render target and template are configuration, not branches.
//! ndi_encode and network_client targets come in Phase 10.

use serde::Serialize;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// How a channel's template is actually output.
pub enum RenderTarget {
    NativeWindow,
    NdiEncode,
    NetworkClient,
}

/// The content pushed to every output channel. Templates bind these fields to
/// their regions; the pipeline never formats per channel.
#[derive(Debug, Clone, Serialize)]
pub struct OutputContent {
    pub reference: String,
    pub text: Option<String>,
    pub translation: Option<String>,
}

/// Prefix for programmatically-created output-window labels. Kept in sync with
/// the capability glob (`output-*`) in capabilities/default.json.
const OUTPUT_PREFIX: &str = "output-";

/// Build the output view URL for a channel: the shared output.html plus the
/// template id and a display name. Pure — unit-tested.
pub fn output_url(template: &str, name: &str) -> String {
    format!(
        "output.html?template={}&name={}",
        urlencode(template),
        urlencode(name)
    )
}

/// Open a native fullscreen output window rendering `template`. Borderless and
/// always-on-top-free so it behaves as a projector/second-screen surface.
pub fn open_native_window(
    app: &tauri::AppHandle,
    label: &str,
    template: &str,
    name: &str,
) -> Result<(), String> {
    if app.get_webview_window(label).is_some() {
        return Err(format!("output window '{label}' already open"));
    }
    WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(output_url(template, name).into()),
    )
    .title(format!("Relay — {name}"))
    .inner_size(1280.0, 720.0)
    .decorations(false)
    .fullscreen(true)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Close an output window by label. Idempotent.
pub fn close_window(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(label) {
        w.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Labels of all currently-open output windows.
pub fn list_open(app: &tauri::AppHandle) -> Vec<String> {
    app.webview_windows()
        .into_keys()
        .filter(|k| k.starts_with(OUTPUT_PREFIX))
        .collect()
}

/// Push content to every output channel. One broadcast, N independently-styled
/// renders (each window applies its own template).
pub fn broadcast_content(app: &tauri::AppHandle, content: OutputContent) {
    let _ = app.emit("output://content", content);
}

/// Clear all output channels (operator "Clear all screens" / Esc).
pub fn clear(app: &tauri::AppHandle) {
    let _ = app.emit("output://clear", ());
}

/// Minimal query-safe encoder for the two values we put in the output URL.
fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c.to_string(),
            ' ' => "%20".to_string(),
            other => format!("%{:02X}", other as u32),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_url_carries_template_and_name() {
        assert_eq!(
            output_url("main", "Main screen"),
            "output.html?template=main&name=Main%20screen"
        );
    }

    #[test]
    fn output_url_escapes_specials() {
        let u = output_url("stage", "Stage/2");
        assert!(u.contains("name=Stage%2F2"), "got {u}");
    }
}
