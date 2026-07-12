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
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::broadcast;

/// How a channel's template is actually output.
///
/// Not yet constructed in code: channels currently resolve their target from the
/// DB string. Kept as the declared seam for the NDI / network-client targets
/// (PROMPT.md Phase 10) rather than deleted, so the shape of the extension point
/// stays visible — NDI is parked, not abandoned (docs/DECISIONS.md).
#[allow(dead_code)]
pub enum RenderTarget {
    NativeWindow,
    NdiEncode,
    NetworkClient,
}

/// The content pushed to every output channel. Templates bind these fields to
/// their regions; the pipeline never formats per channel.
#[derive(Debug, Clone, Default, Serialize)]
pub struct OutputContent {
    pub reference: String,
    pub text: Option<String>,
    pub translation: Option<String>,
    /// Absolute URL of a media asset to paint behind the text (image/video),
    /// served by the embedded HTTP server. None for text-only cues.
    pub media_url: Option<String>,
    /// "image" | "video" — how the output page renders the media layer.
    pub media_kind: Option<String>,
    /// Per-content-type template override (ProPresenter-style: lyrics use the
    /// lyric template, scripture the scripture template). When set, the output
    /// renders THIS content with `template_json` instead of the channel's own
    /// template; when None, the channel's assigned template is used (default).
    pub template_id: Option<i64>,
    pub template_json: Option<String>,
    /// Operator's private note for this cue (e.g. "hold for prayer"). Rides with
    /// the slide but is confidence-monitor only — the stage remote shows it, the
    /// congregation output never does (no template region renders it).
    pub stage_note: Option<String>,
    /// Pre-service countdown: the target epoch (ms) to count down TO. When set,
    /// the output renders a live MM:SS (ticked locally, so no per-second network
    /// traffic) styled by the template; `reference` is the label above it.
    pub countdown_to: Option<i64>,
    /// Message shown in place of the timer when the countdown reaches zero.
    pub countdown_done: Option<String>,
}

/// A connected physical display, shaped for the Channels UI. `index` is the
/// position in the OS monitor list and is what a channel stores as its
/// `display_target` for HDMI output.
#[derive(Debug, Clone, Serialize)]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub primary: bool,
}

/// Enumerate connected displays for screen assignment. HDMI output in Relay is
/// simply a borderless fullscreen window pinned to one of these (docs/SPEC.md §9
/// — no capture-card SDK). Returns an empty list rather than erroring.
pub fn list_monitors(app: &tauri::AppHandle) -> Vec<MonitorInfo> {
    let primary_name = app
        .primary_monitor()
        .ok()
        .flatten()
        .and_then(|m| m.name().cloned());
    let Ok(monitors) = app.available_monitors() else {
        return Vec::new();
    };
    monitors
        .into_iter()
        .enumerate()
        .map(|(index, m)| {
            let name = m
                .name()
                .cloned()
                .unwrap_or_else(|| format!("Display {}", index + 1));
            let size = m.size();
            let pos = m.position();
            MonitorInfo {
                primary: primary_name.as_ref() == m.name(),
                index,
                name,
                width: size.width,
                height: size.height,
                x: pos.x,
                y: pos.y,
                scale: m.scale_factor(),
            }
        })
        .collect()
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

/// Open a native fullscreen output window rendering template `template_id`,
/// pinned to the display at `monitor_index` when given (HDMI output). Borderless
/// so it behaves as a projector/second-screen surface.
///
/// Targeting works by placing the window inside the chosen monitor's bounds
/// first, then going fullscreen — the OS fullscreens on whichever monitor the
/// window sits on. Falls back to the primary display if the index is stale.
pub fn open_native_window(
    app: &tauri::AppHandle,
    label: &str,
    template_id: i64,
    name: &str,
    monitor_index: Option<usize>,
) -> Result<(), String> {
    if app.get_webview_window(label).is_some() {
        return Err(format!("output window '{label}' already open"));
    }
    let mut builder = WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(output_url(template_id, name).into()),
    )
    .title(format!("Relay — {name}"))
    .decorations(false)
    .inner_size(1280.0, 720.0);

    // Position within the target monitor (logical coords) before fullscreen.
    if let Some(idx) = monitor_index {
        if let Some(m) = app
            .available_monitors()
            .ok()
            .and_then(|ms| ms.into_iter().nth(idx))
        {
            let scale = m.scale_factor().max(0.1);
            let pos = m.position();
            let size = m.size();
            builder = builder
                .position(pos.x as f64 / scale, pos.y as f64 / scale)
                .inner_size(size.width as f64 / scale, size.height as f64 / scale);
        }
    }

    let win = builder.build().map_err(|e| e.to_string())?;
    // Fullscreen after placement so it lands on the targeted monitor.
    let _ = win.set_fullscreen(true);
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

/// The label of the operator console window. Tauri gives the window declared in
/// `tauri.conf.json` the default label "main".
const CONSOLE: &str = "main";

/// REHEARSAL MODE — practise a whole service with nothing reaching the congregation.
///
/// A volunteer has to be able to learn this software, and the only realistic place
/// to practise is the room it runs in: the real projector, the real sound desk, the
/// real plan. Which is also a room where a stray verse on the wall in the middle of
/// the 9am service is exactly the thing we cannot allow.
///
/// So rehearsal is gated HERE, in the one function content leaves the machine
/// through, rather than in each of the (currently seven) call sites that fire.
/// Everything upstream — detection, the router, the pipeline, the plan transport —
/// runs completely unchanged, because a rehearsal that behaves differently from a
/// service is not a rehearsal. Only the last hop is cut:
///
///   real:      emit to every window  +  publish to kiosk/OBS/LAN clients
///   rehearsal: emit to the CONSOLE only. Nothing else. No window, no socket.
///
/// The operator sees the output wall preview exactly as they would live. The
/// projector on the wall behind them keeps showing whatever it was showing.
///
/// Gating at the choke point, not at the callers, is also what makes it honest: a
/// new fire path added tomorrow is sandboxed by construction and cannot forget.
#[derive(Default)]
pub struct Rehearsal(pub AtomicBool);

impl Rehearsal {
    pub fn on(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    pub fn set(&self, v: bool) {
        self.0.store(v, Ordering::Relaxed);
    }
}

/// Is the app currently rehearsing? Reads managed state, so it is false anywhere
/// the state has not been registered (tests, early boot) — failing OPEN to a real
/// broadcast. That is the correct default: the dangerous mistake is silently
/// swallowing content the operator believes is live, not the reverse.
fn rehearsing(app: &tauri::AppHandle) -> bool {
    app.try_state::<Rehearsal>()
        .map(|r| r.on())
        .unwrap_or(false)
}

/// Push content to every output channel. One broadcast, N independently-styled
/// renders — native windows (Tauri event) AND networked kiosk clients (WS).
///
/// In rehearsal this reaches the operator console and NOTHING else.
pub fn broadcast_content(app: &tauri::AppHandle, content: OutputContent) {
    let json = serde_json::json!({
        "kind": "content",
        "reference": content.reference,
        "text": content.text,
        "translation": content.translation,
        "media_url": content.media_url,
        "media_kind": content.media_kind,
        "template_id": content.template_id,
        "template_json": content.template_json,
        "stage_note": content.stage_note,
        "countdown_to": content.countdown_to,
        "countdown_done": content.countdown_done,
    })
    .to_string();
    if rehearsing(app) {
        // Content-free by design: the reference is congregation/sermon data and this
        // log is written to disk. What matters operationally is only that the
        // broadcast was suppressed, which is what an operator (or a bug report)
        // needs to know.
        println!("rehearsal: broadcast SUPPRESSED — nothing left the machine");
        let _ = app.emit_to(CONSOLE, "output://content", content);
        return; // no output window, no kiosk, no LAN.
    }
    let _ = app.emit("output://content", content);
    publish_kiosk(app, json);
}

/// Clear all output channels (operator "Clear all screens" / Esc). Clears to the
/// template background — transparent templates key out for OBS/ATEM.
pub fn clear(app: &tauri::AppHandle) {
    if rehearsing(app) {
        let _ = app.emit_to(CONSOLE, "output://clear", ());
        return;
    }
    let _ = app.emit("output://clear", ());
    publish_kiosk(app, r#"{"kind":"clear"}"#.to_string());
}

/// Blackout: paint every output opaque black (kills the screen entirely, unlike
/// a transparent clear). The next content/clear cancels it.
pub fn black(app: &tauri::AppHandle) {
    if rehearsing(app) {
        let _ = app.emit_to(CONSOLE, "output://black", ());
        return;
    }
    let _ = app.emit("output://black", ());
    publish_kiosk(app, r#"{"kind":"black"}"#.to_string());
}

/// Push the "up next" preview to the stage/confidence monitor(s). Distinct from
/// live content — it only reaches the stage view, never the main output. None
/// clears the panel.
pub fn stage_next(app: &tauri::AppHandle, label: Option<String>, text: Option<String>) {
    let json =
        serde_json::json!({ "kind": "stage_next", "label": label, "text": text }).to_string();
    publish_kiosk(app, json);
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
    /// Cache of template JSON by id, so a browser client (OBS/kiosk — no Tauri
    /// runtime, can't call `get_template`) gets the REAL saved template, not a
    /// built-in fallback. This is what makes OBS match the editor preview.
    templates: Arc<Mutex<HashMap<i64, String>>>,
}

impl Default for KioskHub {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(128);
        KioskHub {
            tx,
            templates: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl KioskHub {
    pub fn publish(&self, msg: String) {
        let _ = self.tx.send(msg); // Err only means no subscribers — fine.
    }
    pub fn sender(&self) -> broadcast::Sender<String> {
        self.tx.clone()
    }
    /// Shared handle to the template cache, for the WS server task.
    pub fn templates_handle(&self) -> Arc<Mutex<HashMap<i64, String>>> {
        self.templates.clone()
    }
    /// Cache a template's JSON (no push). Used to warm the cache at startup.
    pub fn cache_template(&self, id: i64, template_json: &str) {
        if let Ok(mut m) = self.templates.lock() {
            m.insert(id, template_json.to_string());
        }
    }
    /// Update a template and push it live to any connected client showing it, so
    /// an edit in the console re-renders OBS/kiosk in real time (WYSIWYG).
    pub fn set_template(&self, id: i64, template_json: &str) {
        self.cache_template(id, template_json);
        self.publish(format!(
            r#"{{"kind":"template","id":{id},"template":{template_json}}}"#
        ));
    }
}

/// The template id targeted by a `{"kind":"template","id":N,…}` message, if it is
/// one. `None` for non-template messages (content/clear, forwarded to everyone).
fn template_msg_id(msg: &str) -> Option<i64> {
    let v: serde_json::Value = serde_json::from_str(msg).ok()?;
    if v.get("kind").and_then(|k| k.as_str()) == Some("template") {
        v.get("id").and_then(|i| i.as_i64())
    } else {
        None
    }
}

/// How a LAN server reports a fatal bind failure.
///
/// A bind failure means every networked output — OBS browser sources, kiosk
/// screens, the preacher's stage monitor — is dead. It used to be `eprintln!`'d
/// and swallowed, so the operator's only clue was that the screens they set up
/// last week simply never came up. That is exactly the kind of silent failure
/// this app cannot afford.
///
/// Taken as a closure rather than an `AppHandle` so the servers stay unit-testable
/// (tests can't build a real Tauri app handle); `report_to` wires it to the UI.
pub type ErrorSink = Box<dyn Fn(String) + Send + Sync + 'static>;

/// The production sink: surface the failure to the operator's console.
pub fn report_to(handle: &tauri::AppHandle) -> ErrorSink {
    let handle = handle.clone();
    Box::new(move |msg: String| {
        eprintln!("channels: {msg}");
        let _ = handle.emit("output://error", &msg);
    })
}

/// A sink that only logs — for tests, where there is no UI to tell.
#[cfg(test)]
fn log_only() -> ErrorSink {
    Box::new(|msg: String| eprintln!("channels: {msg}"))
}

fn bind_failure_message(what: &str, port: u16, e: &std::io::Error) -> String {
    format!(
        "{what} could not start on port {port} ({e}). \
         Networked outputs (OBS, kiosk screens, the stage monitor) will not work. \
         Another program is probably already using that port."
    )
}

/// Run the kiosk WebSocket server: accept LAN clients and forward published
/// messages. On connect a client sends `{"kind":"hello","template_id":N}` and
/// gets back its real template (`{"kind":"template",…}`); template updates are
/// forwarded only to clients showing that template. Content/clear go to all.
///
/// Binds `0.0.0.0` — every interface. This is a RECORDED tradeoff, not an
/// oversight (docs/DECISIONS.md): kiosk screens, OBS machines and the preacher's
/// phone are all other devices on the church LAN, so a loopback bind would defeat
/// the entire feature. The hub is broadcast-only — the sole inbound message it
/// honours is `hello` — so a stranger on the network can *read* the live content
/// feed but can never push to the screens. Accepted for a LAN appliance;
/// revisit if Relay ever runs somewhere the network isn't trusted.
pub async fn run_kiosk_server(
    on_error: ErrorSink,
    tx: broadcast::Sender<String>,
    templates: Arc<Mutex<HashMap<i64, String>>>,
    port: u16,
) {
    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
        Ok(l) => l,
        Err(e) => {
            on_error(bind_failure_message("The kiosk output server", port, &e));
            return;
        }
    };
    println!("kiosk: WebSocket server listening on :{port}");
    loop {
        let Ok((stream, _addr)) = listener.accept().await else {
            continue;
        };
        let mut rx = tx.subscribe();
        let templates = templates.clone();
        tokio::spawn(async move {
            let ws = match tokio_tungstenite::accept_async(stream).await {
                Ok(w) => w,
                Err(_) => return,
            };
            let (mut write, mut read) = ws.split();
            let mut my_template: Option<i64> = None;
            loop {
                tokio::select! {
                    msg = rx.recv() => match msg {
                        Ok(m) => {
                            // Template updates only go to the client showing them.
                            if let Some(id) = template_msg_id(&m) {
                                if Some(id) != my_template {
                                    continue;
                                }
                            }
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
                        Some(Ok(tokio_tungstenite::tungstenite::Message::Text(txt))) => {
                            // Client hello → remember its template + send it now.
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                                if v.get("kind").and_then(|k| k.as_str()) == Some("hello") {
                                    if let Some(id) = v.get("template_id").and_then(|i| i.as_i64()) {
                                        my_template = Some(id);
                                        let cached = templates.lock().ok().and_then(|m| m.get(&id).cloned());
                                        if let Some(tpl) = cached {
                                            let out = format!(
                                                r#"{{"kind":"template","id":{id},"template":{tpl}}}"#
                                            );
                                            let _ = write
                                                .send(tokio_tungstenite::tungstenite::Message::Text(out))
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                        Some(Ok(_)) => {} // other frames ignored
                        _ => break,       // closed or errored
                    },
                }
            }
        });
    }
}

// ===== Embedded LAN HTTP server for output/stage pages =====

/// The built frontend, embedded into the binary at compile time. This lets a
/// PACKAGED app serve output.html / stage.html (and their assets/fonts) to LAN
/// devices — OBS on another machine, kiosk screens, the preacher's phone — with
/// no dev server running. Requires `dist/` to exist at build time (the Tauri
/// build runs `npm run build` first).
static DIST: include_dir::Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/../dist");

fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ico" => "image/x-icon",
        "webmanifest" => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

async fn serve_embedded<S>(request_path: &str, stream: &mut S)
where
    S: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::AsyncWriteExt;
    // Strip the query string, normalise, default to output.html.
    let clean = request_path
        .split('?')
        .next()
        .unwrap_or("/")
        .trim_start_matches('/');
    let clean = if clean.is_empty() {
        "output.html"
    } else {
        clean
    };
    // Media assets live on disk (imported files), not in the embedded bundle.
    if let Some(rest) = clean.strip_prefix("media/") {
        serve_media_file(rest, stream).await;
        return;
    }
    match DIST.get_file(clean) {
        Some(f) => {
            let body = f.contents();
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                mime_for(clean),
                body.len()
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(body).await;
        }
        None => {
            let msg = b"Not found";
            let header = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                msg.len()
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(msg).await;
        }
    }
}

/// Serve an imported media/document file by its DB id from `media_dir()`. Files
/// are stored as `{id}_{name}`; we take the leading digits of the request as the
/// id (so `../` and other traversal can't escape the media dir) and stream the
/// first matching file. Whole-file read — fine for images/short clips; large
/// videos would want ranged streaming later.
async fn serve_media_file<S>(id_part: &str, stream: &mut S)
where
    S: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::AsyncWriteExt;
    let id: String = id_part.chars().take_while(|c| c.is_ascii_digit()).collect();
    let found = if id.is_empty() {
        None
    } else {
        let dir = crate::db::media_dir();
        let prefix = format!("{id}_");
        std::fs::read_dir(&dir).ok().and_then(|rd| {
            rd.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(&prefix))
                    .unwrap_or(false)
            })
        })
    };
    match found.and_then(|p| std::fs::read(&p).ok().map(|b| (p, b))) {
        Some((path, body)) => {
            let mime = mime_for(&path.to_string_lossy());
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                mime,
                body.len()
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(&body).await;
        }
        None => {
            let msg = b"Media not found";
            let header = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                msg.len()
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(msg).await;
        }
    }
}

/// Serve the embedded output/stage pages over HTTP on the LAN so other devices
/// can load them in a packaged app (not just `tauri dev`). GET-only, one response
/// per connection. Binds `0.0.0.0` for the same recorded reason as the kiosk WS
/// server above — see that doc comment and docs/DECISIONS.md.
pub async fn run_output_http_server(on_error: ErrorSink, port: u16) {
    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
        Ok(l) => l,
        Err(e) => {
            on_error(bind_failure_message("The LAN output page server", port, &e));
            return;
        }
    };
    println!("output http: serving output/stage pages on :{port}");
    loop {
        let Ok((mut stream, _addr)) = listener.accept().await else {
            continue;
        };
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = [0u8; 8192];
            let Ok(n) = stream.read(&mut buf).await else {
                return;
            };
            if n == 0 {
                return;
            }
            // Parse "GET /path HTTP/1.1" — a browser GET fits in one read.
            let head = String::from_utf8_lossy(&buf[..n]);
            let path = head
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("/");
            serve_embedded(path, &mut stream).await;
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

    /// The embedded LAN server serves the output/stage pages (200 + html) and
    /// 404s the unknown — this is what makes a packaged app reachable by OBS/
    /// kiosk/phone with no dev server.
    #[tokio::test]
    async fn output_http_serves_embedded_pages() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        tokio::spawn(run_output_http_server(log_only(), 8201));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let mut s = tokio::net::TcpStream::connect("127.0.0.1:8201")
            .await
            .expect("connect");
        s.write_all(b"GET /stage.html HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let mut buf = vec![0u8; 4096];
        let n = s.read(&mut buf).await.unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(
            resp.starts_with("HTTP/1.1 200"),
            "got {}",
            &resp[..resp.len().min(60)]
        );
        assert!(resp.contains("text/html"));

        let mut s2 = tokio::net::TcpStream::connect("127.0.0.1:8201")
            .await
            .expect("connect");
        s2.write_all(b"GET /nope.xyz HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let n2 = s2.read(&mut buf).await.unwrap();
        assert!(String::from_utf8_lossy(&buf[..n2]).starts_with("HTTP/1.1 404"));
    }

    /// The #1 fix: a browser client (OBS/kiosk) says hello and gets back the REAL
    /// cached template, so it renders exactly what the editor shows.
    #[tokio::test]
    async fn kiosk_hello_returns_cached_template() {
        let hub = KioskHub::default();
        hub.cache_template(
            7,
            r##"{"id":7,"name":"Custom","style":{"accent":"#f5a623"}}"##,
        );
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            8200,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let (ws, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:8200")
            .await
            .expect("connect");
        let (mut write, mut read) = ws.split();
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"kind":"hello","template_id":7}"#.to_string(),
            ))
            .await
            .expect("send hello");

        let msg = tokio::time::timeout(std::time::Duration::from_secs(2), read.next())
            .await
            .expect("no message within timeout")
            .expect("stream ended")
            .expect("ws error");
        let text = msg.into_text().unwrap();
        assert!(text.contains(r#""kind":"template""#), "got {text}");
        assert!(
            text.contains(r#""id":7"#) && text.contains(r#""name":"Custom""#),
            "got {text}"
        );
    }

    /// End-to-end kiosk path (what OBS/vMix uses): a WS client connects, a fire
    /// is published, and the client receives it.
    #[tokio::test]
    async fn kiosk_ws_forwards_published_content() {
        let hub = KioskHub::default();
        let tx = hub.sender();
        tokio::spawn(run_kiosk_server(
            log_only(),
            tx,
            hub.templates_handle(),
            8199,
        ));
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

#[cfg(test)]
mod rehearsal_tests {
    use super::*;

    #[test]
    fn defaults_to_off() {
        // A brand-new install must broadcast for real. If the default were ON, an
        // operator's very first service would show nothing on the projector and
        // there would be no obvious reason why.
        assert!(!Rehearsal::default().on());
    }

    #[test]
    fn toggles() {
        let r = Rehearsal::default();
        r.set(true);
        assert!(r.on());
        r.set(false);
        assert!(!r.on());
    }

    #[test]
    fn console_label_is_the_tauri_default() {
        // The whole sandbox rests on emit_to(CONSOLE, ..) reaching the operator
        // window and nothing else. Tauri labels the window declared in
        // tauri.conf.json "main"; if that ever changes, rehearsal silently stops
        // showing the operator ANY preview and looks completely broken.
        assert_eq!(CONSOLE, "main");
        // And the console must never collide with an output window's label, or a
        // rehearsal would emit straight onto a projector.
        assert!(!CONSOLE.starts_with(OUTPUT_PREFIX));
    }
}
