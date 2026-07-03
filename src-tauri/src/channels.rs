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

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::broadcast;

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
/// template id (looked up from the DB by the window) and a display name.
/// Pure — unit-tested.
pub fn output_url(template_id: i64, name: &str) -> String {
    format!(
        "output.html?template_id={}&name={}",
        template_id,
        urlencode(name)
    )
}

/// Open a native fullscreen output window rendering template `template_id`.
/// Borderless so it behaves as a projector/second-screen surface.
pub fn open_native_window(
    app: &tauri::AppHandle,
    label: &str,
    template_id: i64,
    name: &str,
) -> Result<(), String> {
    if app.get_webview_window(label).is_some() {
        return Err(format!("output window '{label}' already open"));
    }
    WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(output_url(template_id, name).into()),
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
/// renders — native windows (Tauri event) AND networked kiosk clients (WS).
pub fn broadcast_content(app: &tauri::AppHandle, content: OutputContent) {
    let json = serde_json::json!({
        "kind": "content",
        "reference": content.reference,
        "text": content.text,
        "translation": content.translation,
    })
    .to_string();
    let _ = app.emit("output://content", content);
    publish_kiosk(app, json);
}

/// Clear all output channels (operator "Clear all screens" / Esc).
pub fn clear(app: &tauri::AppHandle) {
    let _ = app.emit("output://clear", ());
    publish_kiosk(app, r#"{"kind":"clear"}"#.to_string());
}

fn publish_kiosk(app: &tauri::AppHandle, msg: String) {
    if let Some(hub) = app.try_state::<KioskHub>() {
        hub.publish(msg);
    }
}

// ===== network_client render target — kiosk clients over WebSocket =====

/// Fan-out hub for kiosk (networked browser) output channels. A LAN device
/// (e.g. a $50 Raspberry Pi in Chromium kiosk mode) hits the output page and
/// connects here; state is pushed to it over WebSocket — the offline, low-cost
/// output-hardware path (docs/DECISIONS.md). Same content, its own template.
pub struct KioskHub {
    tx: broadcast::Sender<String>,
}

impl Default for KioskHub {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(128);
        KioskHub { tx }
    }
}

impl KioskHub {
    pub fn publish(&self, msg: String) {
        let _ = self.tx.send(msg); // Err only means no subscribers — fine.
    }
    pub fn sender(&self) -> broadcast::Sender<String> {
        self.tx.clone()
    }
}

/// Run the kiosk WebSocket server: accept LAN clients and forward every
/// published message to each. Binds 0.0.0.0 so kiosks on the network can reach
/// it. Runs for the app's lifetime; a bind failure is logged, not fatal.
pub async fn run_kiosk_server(tx: broadcast::Sender<String>, port: u16) {
    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("kiosk: failed to bind WS server on :{port}: {e}");
            return;
        }
    };
    println!("kiosk: WebSocket server listening on :{port}");
    loop {
        let Ok((stream, _addr)) = listener.accept().await else {
            continue;
        };
        let mut rx = tx.subscribe();
        tokio::spawn(async move {
            let ws = match tokio_tungstenite::accept_async(stream).await {
                Ok(w) => w,
                Err(_) => return,
            };
            let (mut write, mut read) = ws.split();
            loop {
                tokio::select! {
                    msg = rx.recv() => match msg {
                        Ok(m) => {
                            if write
                                .send(tokio_tungstenite::tungstenite::Message::Text(m))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    },
                    incoming = read.next() => match incoming {
                        Some(Ok(_)) => {} // ignore client → server messages
                        _ => break,      // closed or errored
                    },
                }
            }
        });
    }
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
            output_url(1, "Main screen"),
            "output.html?template_id=1&name=Main%20screen"
        );
    }

    #[test]
    fn output_url_escapes_specials() {
        let u = output_url(2, "Stage/2");
        assert!(u.contains("name=Stage%2F2"), "got {u}");
    }

    /// End-to-end kiosk path (what OBS/vMix uses): a WS client connects, a fire
    /// is published, and the client receives it.
    #[tokio::test]
    async fn kiosk_ws_forwards_published_content() {
        let hub = KioskHub::default();
        let tx = hub.sender();
        tokio::spawn(run_kiosk_server(tx, 8199));
        // Give the listener a moment to bind.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let (ws, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:8199")
            .await
            .expect("connect");
        let (_write, mut read) = ws.split();
        // The client has completed the handshake (so it's subscribed); publish.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        hub.publish(r#"{"kind":"content","reference":"John 3:16"}"#.to_string());

        let msg = tokio::time::timeout(std::time::Duration::from_secs(2), read.next())
            .await
            .expect("no message within timeout")
            .expect("stream ended")
            .expect("ws error");
        assert!(msg.into_text().unwrap().contains("John 3:16"));
    }
}
