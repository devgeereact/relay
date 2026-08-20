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
    /// The content KIND — "scripture" | "song" | "media" | "announce" |
    /// "countdown". Rides to every output so a screen can decide whether to show
    /// it: an online wall shows all, a stage/confidence monitor might show only
    /// scripture, songs and the timer. Per-screen visibility is filtered on this
    /// in the output page against the template's `shows` set. None = unspecified
    /// (treated as always shown, for older paths).
    pub kind: Option<String>,
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
    /// True when `template_json` is a cue's DELIBERATE per-cue template choice
    /// (a Planner item picked that look), which overrides the screen's own
    /// template. False for a content-type DEFAULT (a "content look"), which
    /// DEFERS to the screen's own template — so an operator sees the template they
    /// assigned per screen, not one silently swapped in. See DECISIONS §29.
    pub template_pinned: bool,
    /// Operator's private note for this cue (e.g. "hold for prayer"). Rides with
    /// the slide but is confidence-monitor only — the stage remote shows it, the
    /// congregation output never does (no template region renders it).
    pub stage_note: Option<String>,
    /// The NEXT verse coming up, for a stage/confidence monitor's "up next" line.
    /// Reaches output like `stage_note` but no congregation template renders it —
    /// only a monitor template carrying a `next` / `next_reference` layer does.
    /// BOUNDED by the read range: reading John 3:16–17 shows no "next" once 3:17
    /// is up, rather than spilling into 3:18 (see `attach_next_verse`). None at a
    /// range/chapter end or when the following verse is not in the corpus.
    pub next_reference: Option<String>,
    pub next_text: Option<String>,
    /// Epoch (ms) the current service session STARTED, for a stage/confidence
    /// monitor's elapsed timer (counts UP, ticked locally in the renderer like the
    /// clock). None when nothing is being recorded. Like the other monitor fields
    /// it rides to output but no congregation template renders it — only a monitor
    /// template with an `elapsed` layer. It clears with the screens (the panic
    /// clear stays total — a monitor is not exempt), which is deliberate.
    pub service_started_at: Option<i64>,
    /// Planned service length (ms), for a monitor's REMAINING timer (target minus
    /// elapsed, ticked in the renderer). None when no length is configured. Rides
    /// with `service_started_at`; only a monitor template with a `remaining` layer
    /// renders it.
    pub service_target_ms: Option<i64>,
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
    // macOS knows the display's real name ("HP-532sf") and tao does not surface
    // it; everywhere else the OS string is the best available and the humaniser
    // handles it.
    #[cfg(target_os = "macos")]
    let real_names = macos_display_names(app);

    monitors
        .into_iter()
        .enumerate()
        .map(|(index, m)| {
            let pos = m.position();
            #[cfg(target_os = "macos")]
            let name = real_names
                .get(&(pos.x, pos.y))
                .cloned()
                .unwrap_or_else(|| humanize_monitor_name(m.name().map(|s| s.as_str()), index));
            #[cfg(not(target_os = "macos"))]
            let name = humanize_monitor_name(m.name().map(|s| s.as_str()), index);
            let size = m.size();
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

/// Real display names from macOS, keyed by the monitor position Tauri reports.
///
/// **Why this exists.** tao names a macOS monitor
/// `format!("Monitor #{}", CGDisplay::model_number())`, so Relay offered the
/// operator "Monitor #1234555" for a screen macOS itself calls "HP-532sf". On the
/// control that decides which physical screen the congregation sees, an
/// unrecognisable name is a mis-send waiting to happen.
///
/// The name macOS shows in System Settings › Displays is `NSScreen.localizedName`.
/// Matching it back to a Tauri monitor is done on POSITION, because Tauri drops
/// the native display id — it keeps only name/size/position/scale. tao derives
/// position as `CGDisplayBounds(id).origin * scale_factor`, so the same
/// computation here produces a key that matches exactly.
///
/// Runs on the main thread: `NSScreen` is AppKit and is not safe to touch from a
/// Tauri command's worker thread. Falls back to an empty map on any failure, and
/// the caller then uses the generic humaniser — a missing real name degrades to
/// the old behaviour rather than to no display list.
#[cfg(target_os = "macos")]
fn macos_display_names(app: &tauri::AppHandle) -> HashMap<(i32, i32), String> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    if app
        .run_on_main_thread(move || {
            let _ = tx.send(collect_macos_display_names());
        })
        .is_err()
    {
        return HashMap::new();
    }
    // Bounded: a hung main thread must not wedge the Channels screen.
    rx.recv_timeout(std::time::Duration::from_millis(500))
        .unwrap_or_default()
}

/// MAIN THREAD ONLY. See `macos_display_names`.
#[cfg(target_os = "macos")]
fn collect_macos_display_names() -> HashMap<(i32, i32), String> {
    use core_graphics::display::CGDisplayBounds;
    use objc2_app_kit::NSScreen;
    use objc2_foundation::{ns_string, MainThreadMarker};

    let mut out = HashMap::new();
    let Some(mtm) = MainThreadMarker::new() else {
        return out;
    };
    for screen in NSScreen::screens(mtm) {
        // The CGDirectDisplayID lives in the screen's device description.
        let desc = screen.deviceDescription();
        let Some(num) = desc.objectForKey(ns_string!("NSScreenNumber")) else {
            continue;
        };
        let Ok(num) = num.downcast::<objc2_foundation::NSNumber>() else {
            continue;
        };
        let display_id = num.as_u32();

        let name = screen.localizedName().to_string();
        if name.trim().is_empty() {
            continue;
        }

        // Mirror tao's own arithmetic so the key lines up with what Tauri reports.
        let scale = screen.backingScaleFactor();
        let bounds = unsafe { CGDisplayBounds(display_id) };
        let key = (
            (bounds.origin.x * scale).round() as i32,
            (bounds.origin.y * scale).round() as i32,
        );
        out.insert(key, name);
    }
    out
}

/// A display's name as a human should read it.
///
/// The OS name is not fit to show an operator as-is, and it differs wildly by
/// platform. Windows reports the GDI device path `\\.\DISPLAY1`. X11/Wayland
/// report the connector — `HDMI-1`, `DP-2`, `eDP-1`. macOS is the good case and
/// usually gives the real product name ("DELL U2720Q", "Built-in Retina
/// Display"), which must be passed through untouched.
///
/// A volunteer choosing which screen the congregation sees needs to recognise a
/// physical object in the room, so a real product name always wins; failing that,
/// the connector is at least something written on the back of the machine
/// ("HDMI 1"), which beats a device path. `Display N` is the last resort.
///
/// **What this cannot do**: turn `\\.\DISPLAY1` into the monitor's actual model.
/// That name lives in the EDID blob and needs a Windows-specific device-registry
/// lookup Tauri does not expose. So on Windows the operator gets "Display 1"
/// alongside its resolution and primary flag, which is enough to tell two screens
/// apart, and no claim is made about the make.
fn humanize_monitor_name(raw: Option<&str>, index: usize) -> String {
    let fallback = || format!("Display {}", index + 1);
    let Some(name) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return fallback();
    };

    // macOS via tao: `Monitor #1234555` — a raw EDID model number, which is what
    // sent the operator looking for a screen called "1234555". The real name is
    // fetched separately (`macos_display_names`); this is the safety net for when
    // that lookup finds nothing, and it must never let the model number through.
    if let Some(rest) = name.strip_prefix("Monitor #") {
        if rest.chars().all(|c| c.is_ascii_digit()) {
            return fallback();
        }
    }

    // Windows: `\\.\DISPLAY1` — a device path, never shown to a person.
    if let Some(rest) = name.strip_prefix(r"\\.\") {
        let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
        return match digits.parse::<usize>() {
            Ok(n) if rest.to_ascii_uppercase().starts_with("DISPLAY") => format!("Display {n}"),
            _ => fallback(),
        };
    }

    // Linux/BSD connector names. `eDP` is the internal panel — the laptop's own
    // screen — which is worth saying plainly, because it is the one display an
    // operator must usually NOT send the congregation's output to.
    let upper = name.to_ascii_uppercase();
    for (prefix, label) in [
        ("EDP", "Built-in display"),
        ("LVDS", "Built-in display"),
        ("HDMI", "HDMI"),
        ("DP", "DisplayPort"),
        ("DVI", "DVI"),
        ("VGA", "VGA"),
    ] {
        if let Some(rest) = upper.strip_prefix(prefix) {
            // Only treat it as a connector when what follows is punctuation and
            // digits ("HDMI-1", "DP-2"). A product name that merely starts with
            // these letters ("HDMI Splitter Pro") must survive intact.
            let tail = rest.trim_start_matches(['-', '_', ' ', 'A', '/']);
            if !rest.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) && !tail.is_empty() {
                if label == "Built-in display" {
                    return label.to_string();
                }
                return format!("{label} {tail}");
            }
        }
    }

    // A real product name. Leave it exactly as the OS gave it.
    name.to_string()
}

/// Prefix for programmatically-created output-window labels. Kept in sync with
/// the capability glob (`output-*`) in capabilities/default.json.
const OUTPUT_PREFIX: &str = "output-";

/// Prefix for a window opened FOR A CONFIGURED CHANNEL, as opposed to an ad-hoc
/// output window. Still inside `OUTPUT_PREFIX`, so `list_open` and the panic
/// paths keep treating it as an output window.
const CHANNEL_PREFIX: &str = "output-ch";

/// The window label for a channel's native output. Deterministic, so a label can
/// be mapped back to the channel that owns it.
///
/// It used to be minted from a monotonic counter (`output-1`, `output-2`, …),
/// which had two consequences. Opening the same channel twice produced a SECOND
/// fullscreen window for one channel, with nothing to notice the duplicate. And
/// no label could be traced to a channel, so the app could not answer "does this
/// channel have a window open?" — the question the Channels screen exists to
/// answer. With the id in the label, `open_native_window`'s existing
/// already-open check becomes the duplicate guard for free.
pub fn channel_label(channel_id: i64) -> String {
    format!("{CHANNEL_PREFIX}{channel_id}")
}

/// The channel id owning `label`, if it is a channel output window.
pub fn channel_id_of(label: &str) -> Option<i64> {
    label.strip_prefix(CHANNEL_PREFIX)?.parse().ok()
}

/// Channel ids that currently have a native output window open. This is a fact
/// about the running app, not a stored flag — `output_channels.status` is written
/// once at insert and never updated, so it has always read `offline`.
pub fn open_channel_ids(app: &tauri::AppHandle) -> Vec<i64> {
    app.webview_windows()
        .into_keys()
        .filter_map(|k| channel_id_of(&k))
        .collect()
}

/// Build the output view URL for a channel: the shared output.html plus the
/// template id (looked up from the DB by the window) and a display name.
/// Pure — unit-tested.
pub fn output_url(channel_id: i64, template_id: i64, name: &str) -> String {
    // `channel` lets the output live-swap its template when the screen is
    // reassigned (it filters a channel-retemplate broadcast by this id); `template_id`
    // is the first render before any push. `channel=0` = a channel-less preview.
    format!(
        "output.html?channel={}&template_id={}&name={}",
        channel_id,
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
        // Derive the channel id from the window label (channel_label encodes it),
        // so a native output can live-swap its template like a kiosk does.
        WebviewUrl::App(output_url(channel_id_of(label).unwrap_or(0), template_id, name).into()),
    )
    .title(format!("Relay — {name}"))
    .decorations(false)
    // BLACK, not the webview default of WHITE. The output page is transparent so
    // it keys out in an OBS/ATEM browser source; but on a real projector a
    // transparent lower-third template used to show the webview's white
    // background around the band. A black window backdrop makes the band sit on
    // black on the wall while the :8032 browser source stays truly transparent.
    .background_color(tauri::window::Color(0, 0, 0, 255))
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
///
/// ## The choke point is FOUR functions, and it is worth naming them
///
/// "One function" was the intent and never the fact, and the gap cost a leak. Every
/// function below that publishes to the kiosk hub is a way out of the machine:
///
/// * `broadcast_content` — gated
/// * `clear` — gated
/// * `black` — gated
/// * `stage_next` — gated, and it was NOT. It leaked "up next" to a live stage
///   tablet mid-rehearsal, and it has no Tauri emit at all, so the e2e rehearsal
///   test — which counts wall events — saw nothing wrong.
///
/// `main.rs::set_channel_template` also publishes, and is DELIBERATELY not gated:
/// it carries a template, not content. Reassigning a screen's look is live by
/// design (DECISIONS §29), puts no scripture anywhere, and suppressing it would
/// leave a kiosk rendering a template the operator has already replaced.
///
/// Anything added here that carries what a person would READ belongs in the gated
/// list. Check `rehearsing(app)` first, and add an e2e case that watches the KIOSK
/// hub, not just the wall.
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
fn rehearsing<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    app.try_state::<Rehearsal>()
        .map(|r| r.on())
        .unwrap_or(false)
}

/// The WS wire form of a content broadcast (what a kiosk/OBS client receives).
///
/// A SEPARATE shape from the Tauri event because the kiosk protocol renames
/// `kind` to `content_kind` (the client uses `kind` for the message type). Every
/// field a monitor template can bind MUST appear here — this is the exact bug it
/// guards: `next_reference`/`next_text` reach native windows via the struct emit
/// but were dropped from THIS json, so a kiosk stage monitor never showed the
/// "up next" verse while a native one did. Kept as a pure function so a test can
/// assert the field set without a Tauri app handle.
fn kiosk_content_json(content: &OutputContent) -> String {
    serde_json::json!({
        "kind": "content",
        "content_kind": content.kind,
        "reference": content.reference,
        "text": content.text,
        "translation": content.translation,
        "media_url": content.media_url,
        "media_kind": content.media_kind,
        "template_id": content.template_id,
        "template_json": content.template_json,
        "template_pinned": content.template_pinned,
        "stage_note": content.stage_note,
        "next_reference": content.next_reference,
        "next_text": content.next_text,
        "service_started_at": content.service_started_at,
        "service_target_ms": content.service_target_ms,
        "countdown_to": content.countdown_to,
        "countdown_done": content.countdown_done,
    })
    .to_string()
}

/// Push content to every output channel. One broadcast, N independently-styled
/// renders — native windows (Tauri event) AND networked kiosk clients (WS).
///
/// In rehearsal this reaches the operator console and NOTHING else.
/// WHAT THE CONGREGATION IS ACTUALLY LOOKING AT.
///
/// The console has always known this — it derives it from the `output://` events —
/// and the backend did not, so anything asking the backend had to guess. The LAN
/// remote guessed with the Context passage ANCHOR, which deliberately survives a
/// clear (that is what makes `→` resume instead of restarting) and is therefore not
/// an answer to "what is on the wall". `/api/live` consequently named a verse over
/// cleared screens and over blacked-out ones.
///
/// Maintained at the three choke points below and nowhere else, so it cannot drift:
/// `broadcast_content` (and only on the path that really broadcasts — a rehearsal
/// returns before it), `clear`, and `black`.
#[derive(Default)]
pub struct WallState {
    on_air: std::sync::atomic::AtomicBool,
    black: std::sync::atomic::AtomicBool,
}

impl WallState {
    /// True when a congregation can currently see content.
    pub fn on_air(&self) -> bool {
        use std::sync::atomic::Ordering;
        self.on_air.load(Ordering::Relaxed) && !self.black.load(Ordering::Relaxed)
    }
    pub fn blacked(&self) -> bool {
        self.black.load(std::sync::atomic::Ordering::Relaxed)
    }
    fn set(&self, on_air: bool, black: bool) {
        use std::sync::atomic::Ordering;
        self.on_air.store(on_air, Ordering::Relaxed);
        self.black.store(black, Ordering::Relaxed);
    }
}

/// Record a change at one of the three choke points. A no-op when the state is not
/// managed (headless tests that do not care).
fn note_wall<R: tauri::Runtime>(app: &tauri::AppHandle<R>, on_air: bool, black: bool) {
    if let Some(w) = app.try_state::<WallState>() {
        w.set(on_air, black);
    }
}

pub fn broadcast_content<R: tauri::Runtime>(app: &tauri::AppHandle<R>, content: OutputContent) {
    let json = kiosk_content_json(&content);
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
    // AFTER the rehearsal early-return above, so this records the CONGREGATION
    // wall and not the operator's sandbox.
    note_wall(app, true, false);
}

/// Clear all output channels (operator "Clear all screens" / Esc). Clears to the
/// template background — transparent templates key out for OBS/ATEM.
///
/// Returns Err if the clear could not be delivered to the output webviews. It used
/// to `let _ =` the emit and return `()`, so a failed clear was indistinguishable
/// from a successful one all the way up the stack — and the console cheerfully told
/// the operator "Screens cleared" over a wall that still had scripture on it. A
/// panic control that reports a success it did not achieve is worse than one that
/// is missing: the operator stops looking at the screen and trusts the toast.
pub fn clear<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    if rehearsing(app) {
        return app
            .emit_to(CONSOLE, "output://clear", ())
            .map_err(|e| e.to_string());
    }
    app.emit("output://clear", ()).map_err(|e| e.to_string())?;
    publish_kiosk(app, r#"{"kind":"clear"}"#.to_string());
    note_wall(app, false, false);
    Ok(())
}

/// Blackout: paint every output opaque black (kills the screen entirely, unlike
/// a transparent clear). The next content/clear cancels it.
///
/// Returns Err for the same reason `clear` does — see above.
pub fn black<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(), String> {
    if rehearsing(app) {
        return app
            .emit_to(CONSOLE, "output://black", ())
            .map_err(|e| e.to_string());
    }
    app.emit("output://black", ()).map_err(|e| e.to_string())?;
    publish_kiosk(app, r#"{"kind":"black"}"#.to_string());
    note_wall(app, false, true);
    Ok(())
}

/// Push the "up next" preview to the stage/confidence monitor(s). Distinct from
/// live content — it only reaches the stage view, never the main output. None
/// clears the panel.
///
/// REHEARSAL APPLIES HERE TOO, and it did not.
///
/// This is the one content publisher that has no Tauri emit at all: a stage monitor
/// is always a network client (stage.html over :8032, state over the :8031 hub), so
/// it is reached only through `publish_kiosk`. That is exactly why it slipped the
/// gate — `broadcast_content`, `clear` and `black` were each checked, and the e2e
/// rehearsal test asserts on the WALL, which counts Tauri events. `stage_next`
/// emits none, so it was invisible to both the gate and the test that guards it.
///
/// So an operator rehearsing on the real desk pushed the real upcoming verse to
/// whatever stage tablet was still connected from the last service — to the
/// preacher's own screen, in the middle of a live one. Nothing on the congregation
/// wall would have moved, which is worse: the sandbox looked intact.
pub fn stage_next<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    label: Option<String>,
    text: Option<String>,
) {
    if rehearsing(app) {
        // Nothing to preview on the console — there is no console stage panel — so
        // this is a suppression, not a redirect. The stage monitor keeps showing
        // whatever it was showing, exactly as the projector does.
        println!("rehearsal: stage_next SUPPRESSED — nothing left the machine");
        return;
    }
    let json =
        serde_json::json!({ "kind": "stage_next", "label": label, "text": text }).to_string();
    publish_kiosk(app, json);
}

fn publish_kiosk<R: tauri::Runtime>(app: &tauri::AppHandle<R>, msg: String) {
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
    /// How many clients are currently connected, per template id.
    ///
    /// This is what makes a networked channel's "online" light REAL. Each WS task
    /// already knew which template its client asked for, but it kept that on its
    /// own stack, so the app could not answer "is anything actually showing this
    /// channel?" — and `output_channels.status` was a column nothing ever wrote,
    /// so every channel read `offline` forever, including one filling a projector.
    ///
    /// A count, not a client list: Relay does not record who connected, from what
    /// address, or when. A count is the most that can be honestly known here, and
    /// it is enough to answer the only question the operator is asking.
    clients: Arc<Mutex<HashMap<i64, usize>>>,
    /// The operator's CUSTOM themes, as a JSON array string (the `themes.custom`
    /// settings blob). Sent to every kiosk client on connect so a browser source
    /// (which has no DB) can resolve a template that pins a custom theme — builtin
    /// themes it already knows (bundled in the page). Always well-formed: only a
    /// validated JSON array is ever stored (see `set_themes`), so embedding it raw
    /// into a WS message can never corrupt the frame.
    themes: Arc<Mutex<String>>,
}

impl Default for KioskHub {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(128);
        KioskHub {
            tx,
            templates: Arc::new(Mutex::new(HashMap::new())),
            clients: Arc::new(Mutex::new(HashMap::new())),
            themes: Arc::new(Mutex::new("[]".to_string())),
        }
    }
}

/// Registry of connected kiosk clients, counted per template id.
///
/// Cloned into each WS task so it can register on `hello` and — critically —
/// deregister when the socket drops. Handed out as its own type so the drop-guard
/// below is the only way to hold a registration: a task that returned early, or
/// panicked, would otherwise leave a phantom client counted forever and a channel
/// showing `ONLINE` with nothing on the other end.
#[derive(Clone, Default)]
pub struct ClientRegistry(Arc<Mutex<HashMap<i64, usize>>>);

impl ClientRegistry {
    /// Register one client on `template_id`. The returned guard deregisters it
    /// when dropped, however the task ends.
    fn join(&self, template_id: i64) -> ClientGuard {
        if let Ok(mut m) = self.0.lock() {
            *m.entry(template_id).or_insert(0) += 1;
        }
        ClientGuard {
            reg: self.clone(),
            template_id,
        }
    }
    /// Clients currently connected and showing `template_id`.
    pub fn count(&self, template_id: i64) -> usize {
        self.0
            .lock()
            .ok()
            .and_then(|m| m.get(&template_id).copied())
            .unwrap_or(0)
    }
}

/// Deregisters its client when dropped — including on an early return or a panic
/// inside the WS task, which is the whole point of it being a guard.
struct ClientGuard {
    reg: ClientRegistry,
    template_id: i64,
}

impl Drop for ClientGuard {
    fn drop(&mut self) {
        if let Ok(mut m) = self.reg.0.lock() {
            if let Some(n) = m.get_mut(&self.template_id) {
                // Saturating: an underflow here would wrap to usize::MAX and
                // report a channel as wildly online forever.
                *n = n.saturating_sub(1);
                if *n == 0 {
                    m.remove(&self.template_id);
                }
            }
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
    /// Shared handle to the connected-client registry — for the WS server task to
    /// write, and for `channel_status` to read.
    pub fn clients_handle(&self) -> ClientRegistry {
        ClientRegistry(self.clients.clone())
    }
    /// Shared handle to the custom-themes blob, for the WS server task to read and
    /// send to each client on `hello`.
    pub fn themes_handle(&self) -> Arc<Mutex<String>> {
        self.themes.clone()
    }
    /// Validate + store the custom-themes blob WITHOUT pushing (startup warm).
    /// Only a well-formed JSON ARRAY is kept — anything else falls back to `[]`,
    /// so the value embedded raw into a WS frame is always valid JSON.
    pub fn cache_themes(&self, themes_json: &str) {
        let safe = match serde_json::from_str::<serde_json::Value>(themes_json) {
            Ok(v) if v.is_array() => themes_json.to_string(),
            _ => "[]".to_string(),
        };
        if let Ok(mut t) = self.themes.lock() {
            *t = safe;
        }
    }
    /// Update the custom themes AND push them live to every connected client, so a
    /// kiosk re-resolves a custom-themed template the instant the operator saves a
    /// theme. Same validate-then-store rule as `cache_themes`.
    pub fn set_themes(&self, themes_json: &str) {
        self.cache_themes(themes_json);
        let blob = self
            .themes
            .lock()
            .map(|t| t.clone())
            .unwrap_or_else(|_| "[]".into());
        self.publish(format!(r#"{{"kind":"themes","themes":{blob}}}"#));
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
    let hint = if cfg!(target_os = "windows") {
        "Another program may already be using that port, or Windows Defender Firewall \
         blocked it — if Windows asked whether to allow Relay on your network and you \
         chose Cancel, allow it in Windows Firewall settings."
    } else {
        "Another program is probably already using that port."
    };
    format!(
        "{what} could not start on port {port} ({e}). \
         Networked outputs (OBS, kiosk screens, the stage monitor) will not work. {hint}"
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
    clients: ClientRegistry,
    themes: Arc<Mutex<String>>,
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
        let clients = clients.clone();
        let themes = themes.clone();
        tokio::spawn(async move {
            let ws = match tokio_tungstenite::accept_async(stream).await {
                Ok(w) => w,
                Err(_) => return,
            };
            let (mut write, mut read) = ws.split();
            // Dropped when this task ends by ANY route — break, error, or panic —
            // which is what keeps the online count from drifting upward over a
            // service as kiosk screens reconnect.
            let mut _registration: Option<ClientGuard> = None;
            loop {
                tokio::select! {
                    msg = rx.recv() => match msg {
                        Ok(m) => {
                            // EVERY template edit goes to EVERY client. A client
                            // applies it only where it's actually showing that
                            // template — its channel template OR the content-type/
                            // cue OVERRIDE on the verse currently on screen (see
                            // Output.svelte::applyTemplateUpdate). Filtering by the
                            // client's channel template here would drop exactly the
                            // override case, so an edit to the scripture template
                            // never reached a live verse using it. The client-side
                            // guard makes the extra fan-out a no-op where irrelevant.
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
                                        // Replace, don't add: a client that says
                                        // hello twice (a kiosk page reloading onto
                                        // a different template) must not be
                                        // counted on both. Assigning drops the old
                                        // guard first.
                                        _registration = Some(clients.join(id));
                                        let cached = templates.lock().ok().and_then(|m| m.get(&id).cloned());
                                        if let Some(tpl) = cached {
                                            let out = format!(
                                                r#"{{"kind":"template","id":{id},"template":{tpl}}}"#
                                            );
                                            let _ = write
                                                .send(tokio_tungstenite::tungstenite::Message::Text(out))
                                                .await;
                                        }
                                        // Send the custom themes too, so this client
                                        // can resolve a template pinning a custom
                                        // theme (builtins it already knows). Always
                                        // a valid JSON array (see set_themes).
                                        let blob = themes.lock().map(|t| t.clone()).unwrap_or_else(|_| "[]".into());
                                        let _ = write
                                            .send(tokio_tungstenite::tungstenite::Message::Text(
                                                format!(r#"{{"kind":"themes","themes":{blob}}}"#),
                                            ))
                                            .await;
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
        "bmp" => "image/bmp",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        // The Library's importer accepts these, so this table has to know them.
        // Served as `application/octet-stream`, a browser will not PLAY a video
        // or DRAW an image — it offers to download it. The importer's accepted
        // extensions and this list are one thing in two places; when one grows,
        // the other has to. `mime_covers_every_imported_kind` pins that.
        "mkv" => "video/x-matroska",
        "ogv" => "video/ogg",
        "pdf" => "application/pdf",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
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
    // DEV ONLY: serve the LIVE on-disk `dist/` first. `DIST` is embedded at Rust
    // COMPILE time, so under `tauri dev` a frontend change (`npm run build`, or a
    // vite rebuild) updates `dist/` on disk but the running binary keeps serving
    // the stale bundle it was compiled with — which meant every OBS/LAN output on
    // :8032 silently ran old code (a fixed bug still "not fixed" on the very
    // screens a church uses). Reading disk here makes those outputs current
    // without a full recompile. Release builds (debug_assertions off) always use
    // the embedded bundle — there is no `dist/` next to a shipped binary.
    #[cfg(debug_assertions)]
    // Reject path traversal before touching disk. `clean` comes straight from the
    // request line, and this server binds 0.0.0.0 on a church LAN — without this,
    // `GET /../../../../etc/passwd` escapes dist/ and streams any readable file to
    // anyone on the network. The embedded DIST fallback below is traversal-safe,
    // so on a `..` request we simply skip the disk read and fall through to it.
    if !clean.contains("..") {
        let disk = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dist")
            .join(clean);
        if let Ok(body) = std::fs::read(&disk) {
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nContent-Security-Policy: {}\r\nX-Content-Type-Options: nosniff\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                mime_for(clean),
                body.len(),
                KIOSK_CSP
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(&body).await;
            return;
        }
    }
    match DIST.get_file(clean) {
        Some(f) => {
            let body = f.contents();
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nContent-Security-Policy: {}\r\nX-Content-Type-Options: nosniff\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                mime_for(clean),
                body.len(),
                KIOSK_CSP
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
/// The preacher's-remote control plane. Given a request path+query beginning with
/// `/api/` (e.g. `/api/search?q=john+3`, `/api/next`, `/api/fire?ref=John%203:16`),
/// it performs the action against the running app and returns a JSON body. `None`
/// for a non-api path (falls through to the static file server).
///
/// SECURITY: this accepts CONTROL over the LAN with no authentication — a
/// deliberate, recorded expansion of the previously broadcast-only exposure
/// (docs/DECISIONS.md §35). It exists so the preacher's phone can search and push
/// scripture. The threat model is unchanged in kind: anyone already on the church
/// wifi. Do NOT expose this port to an untrusted network.
/// What the control plane answers with: a body, the status it deserves, and
/// whether this particular route may be read cross-origin.
///
/// The last field is not decoration. Every response used to carry
/// `Access-Control-Allow-Origin: *`, every action was a side-effecting `GET`, and
/// the request line was parsed verb-agnostically — three individually reasonable
/// choices whose composition let `<img src="http://<relay>:8032/api/black">`, on any
/// page anyone on the church network happened to open, black out the congregation's
/// wall (DECISIONS §35). The mutating routes now refuse anything but `POST` and
/// answer without the wildcard, which removes that vector without touching the
/// preacher's phone.
pub struct ApiReply {
    pub status: u16,
    pub body: String,
    /// `true` only for the read-only routes. A mutating route never sends the
    /// wildcard, so a cross-origin caller cannot read what it did.
    pub cors: bool,
}

pub type ApiSink = std::sync::Arc<dyn Fn(&str, &str) -> Option<ApiReply> + Send + Sync>;

pub async fn run_output_http_server(on_error: ErrorSink, api: ApiSink, port: u16) {
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
        let api = api.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = [0u8; 8192];
            let Ok(n) = stream.read(&mut buf).await else {
                return;
            };
            if n == 0 {
                return;
            }
            // Parse "GET /path HTTP/1.1". The VERB is read, not discarded: it is
            // half of what stops a drive-by from driving the wall (DECISIONS §35).
            let head = String::from_utf8_lossy(&buf[..n]);
            let mut first = head.lines().next().unwrap_or("").split_whitespace();
            let method = first.next().unwrap_or("GET");
            let path = first.next().unwrap_or("/");
            if let Some(rest) = path.strip_prefix("/api/") {
                let reply = api(method, rest).unwrap_or_else(|| ApiReply {
                    status: 500,
                    body: "{\"ok\":false}".to_string(),
                    cors: false,
                });
                serve_json(&reply, &mut stream).await;
            } else {
                serve_embedded(path, &mut stream).await;
            }
        });
    }
}

/// The Content-Security-Policy the LAN pages are served with.
///
/// The packaged app has one (`tauri.conf.json`) and this server had **none** — so
/// `output.html` in the packaged webview was constrained and the *same page* served
/// to an OBS browser source, a kiosk screen or a phone was not. That is the half of
/// the audience this policy most needs to cover: those clients are ordinary browsers
/// on a church network, running a page whose look is assembled from template JSON
/// that may have arrived in an email.
///
/// Deliberately TIGHTER than the packaged policy in the one way that matters:
/// **no `http:` in `img-src` or `media-src`.** The desktop app allows it for
/// operator-chosen local sources; a page on the LAN has no such need, and Relay
/// renders offline or it does not render. `connect-src 'self' ws:` keeps the kiosk
/// socket working and nothing else.
pub(crate) const KIOSK_CSP: &str = "default-src 'self'; script-src 'self'; \
style-src 'self' 'unsafe-inline'; font-src 'self' data:; \
img-src 'self' data: blob:; media-src 'self' data: blob:; \
connect-src 'self' ws: wss:; object-src 'none'; frame-src 'none'; \
base-uri 'self'; form-action 'none'";

/// Write a JSON reply. `no-store` always; the CORS wildcard **only** where the
/// reply says it may go (read-only routes — see `ApiReply`).
async fn serve_json<S>(reply: &ApiReply, stream: &mut S)
where
    S: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::AsyncWriteExt;
    let phrase = match reply.status {
        200 => "OK",
        405 => "Method Not Allowed",
        _ => "Internal Server Error",
    };
    // `Allow` is part of a correct 405, and it is also the honest answer to a
    // developer wondering why their GET stopped working.
    let extra = if reply.status == 405 {
        "Allow: POST\r\n"
    } else {
        ""
    };
    let cors = if reply.cors {
        "Access-Control-Allow-Origin: *\r\n"
    } else {
        ""
    };
    let resp = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\n{}{}Cache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        reply.status,
        phrase,
        cors,
        extra,
        reply.body.len(),
        reply.body
    );
    let _ = stream.write_all(resp.as_bytes()).await;
    let _ = stream.flush().await;
}

/// Percent-decode a query value (`John%203%3A16` → `John 3:16`, `+` → space).
pub fn urldecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let h = |c: u8| (c as char).to_digit(16);
                if let (Some(a), Some(b)) = (h(bytes[i + 1]), h(bytes[i + 2])) {
                    out.push((a * 16 + b) as u8);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
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
    fn output_url_carries_channel_template_and_name() {
        assert_eq!(
            output_url(3, 1, "Main screen"),
            "output.html?channel=3&template_id=1&name=Main%20screen"
        );
    }

    #[test]
    fn output_url_escapes_specials() {
        let u = output_url(0, 2, "Stage/2");
        assert!(u.contains("name=Stage%2F2"), "got {u}");
    }

    /// The embedded LAN server serves the output/stage pages (200 + html) and
    /// 404s the unknown — this is what makes a packaged app reachable by OBS/
    /// kiosk/phone with no dev server.
    #[tokio::test]
    async fn output_http_serves_embedded_pages() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let no_api: ApiSink = std::sync::Arc::new(|_: &str, _: &str| None);
        tokio::spawn(run_output_http_server(log_only(), no_api, 8201));
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
            hub.clients_handle(),
            hub.themes_handle(),
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

    /// A kiosk gets the operator's custom themes on connect, so a browser source
    /// can resolve a template that pins a custom theme (builtins it bundles).
    #[tokio::test]
    async fn a_kiosk_client_receives_the_custom_themes_on_hello() {
        let hub = KioskHub::default();
        hub.cache_themes(r##"[{"id":3,"name":"Sanctuary","style":{"accent":"#abc"}}]"##);
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.themes_handle(),
            8203,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let (ws, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:8203")
            .await
            .expect("connect");
        let (mut write, mut read) = ws.split();
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"kind":"hello","template_id":7}"#.to_string(),
            ))
            .await
            .expect("send hello");

        // Read frames until the themes frame arrives (the template frame may come
        // first when a template is cached; here none is, so themes is first).
        let mut got = false;
        for _ in 0..3 {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await
            else {
                break;
            };
            let text = msg.into_text().unwrap();
            if text.contains(r#""kind":"themes""#) {
                assert!(text.contains("Sanctuary"), "got {text}");
                got = true;
                break;
            }
        }
        assert!(got, "the client never received the custom themes");
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
            hub.clients_handle(),
            hub.themes_handle(),
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

    // ── Channel liveness ──────────────────────────────────────────────────
    //
    // These back the Channels screen's online light. Before them the only
    // "status" was a DB column written once at insert and never updated, so
    // every channel read `offline` forever — including one on a projector.

    // ── Display names an operator can recognise ───────────────────────────
    //
    // The operator picking "which screen does the congregation see" is choosing
    // a physical object in the room. A device path or a bare connector id does
    // not identify one.

    /// THE BUG THE OPERATOR HIT.
    ///
    /// tao names a macOS monitor `Monitor #{CGDisplay::model_number()}`, so the
    /// display picker offered "Monitor #1234555" for a screen macOS itself calls
    /// "HP-532sf". The real name now comes from `NSScreen.localizedName`; this
    /// pins the fallback, so even when that lookup finds nothing the operator is
    /// never shown an EDID model number and asked to recognise it.
    #[test]
    fn a_macos_edid_model_number_never_reaches_the_operator() {
        for raw in ["Monitor #1234555", "Monitor #0", "Monitor #42"] {
            let got = humanize_monitor_name(Some(raw), 1);
            assert_eq!(got, "Display 2", "{raw} must not be shown as-is");
            assert!(!got.contains('#'));
        }
    }

    #[test]
    fn a_display_actually_called_monitor_something_survives() {
        // The rule keys on "Monitor #" followed by digits ONLY, so a real product
        // name is not swallowed by it.
        assert_eq!(
            humanize_monitor_name(Some("Monitor #2 Pro"), 0),
            "Monitor #2 Pro"
        );
        assert_eq!(
            humanize_monitor_name(Some("Monitor Wall A"), 0),
            "Monitor Wall A"
        );
    }

    #[test]
    fn a_real_product_name_is_never_touched() {
        // macOS gives these, and they are exactly what we want.
        assert_eq!(humanize_monitor_name(Some("DELL U2720Q"), 0), "DELL U2720Q");
        assert_eq!(
            humanize_monitor_name(Some("Built-in Retina Display"), 0),
            "Built-in Retina Display"
        );
        assert_eq!(humanize_monitor_name(Some("BenQ TK850"), 1), "BenQ TK850");
    }

    #[test]
    fn a_windows_device_path_is_not_shown_to_a_person() {
        assert_eq!(humanize_monitor_name(Some(r"\\.\DISPLAY1"), 0), "Display 1");
        assert_eq!(humanize_monitor_name(Some(r"\\.\DISPLAY2"), 1), "Display 2");
        // Unrecognised device path → positional fallback, never the raw path.
        let odd = humanize_monitor_name(Some(r"\\.\WEIRD"), 2);
        assert_eq!(odd, "Display 3");
        assert!(!odd.contains('\\'));
    }

    #[test]
    fn a_linux_connector_becomes_the_socket_it_is_plugged_into() {
        assert_eq!(humanize_monitor_name(Some("HDMI-1"), 0), "HDMI 1");
        assert_eq!(humanize_monitor_name(Some("DP-2"), 1), "DisplayPort 2");
        assert_eq!(humanize_monitor_name(Some("HDMI-A-1"), 0), "HDMI 1");
        assert_eq!(humanize_monitor_name(Some("VGA-1"), 0), "VGA 1");
    }

    #[test]
    fn the_laptops_own_panel_says_so() {
        // The one display the congregation's output usually must NOT go to.
        assert_eq!(humanize_monitor_name(Some("eDP-1"), 0), "Built-in display");
        assert_eq!(humanize_monitor_name(Some("LVDS-1"), 0), "Built-in display");
    }

    #[test]
    fn a_product_name_starting_with_connector_letters_survives() {
        // The connector rule must not eat a real name.
        assert_eq!(
            humanize_monitor_name(Some("HDMI Splitter Pro"), 0),
            "HDMI Splitter Pro"
        );
        assert_eq!(
            humanize_monitor_name(Some("DPI Vision 4K"), 0),
            "DPI Vision 4K"
        );
    }

    #[test]
    fn a_missing_or_blank_name_falls_back_to_its_position() {
        assert_eq!(humanize_monitor_name(None, 0), "Display 1");
        assert_eq!(humanize_monitor_name(Some(""), 1), "Display 2");
        assert_eq!(humanize_monitor_name(Some("   "), 2), "Display 3");
    }

    #[test]
    fn a_channel_label_round_trips_to_its_id() {
        assert_eq!(channel_label(7), "output-ch7");
        assert_eq!(channel_id_of("output-ch7"), Some(7));
        // Two channels never collide, and the label stays an output window so the
        // panic paths keep treating it as one.
        assert_ne!(channel_label(7), channel_label(8));
        assert!(channel_label(7).starts_with(OUTPUT_PREFIX));
    }

    #[test]
    fn the_kiosk_wire_form_carries_every_monitor_bindable_field() {
        // The regression: a stage/confidence monitor over OBS/kiosk binds `next`
        // and `note`, but the WS json dropped `next_reference`/`next_text`, so the
        // "up next" line was blank on a kiosk while a native window showed it.
        let content = OutputContent {
            kind: Some("scripture".into()),
            reference: "John 3:16".into(),
            text: Some("For God so loved...".into()),
            stage_note: Some("hold for prayer".into()),
            next_reference: Some("John 3:17".into()),
            next_text: Some("For God sent not...".into()),
            service_started_at: Some(1_700_000_000_000),
            service_target_ms: Some(1_800_000),
            ..Default::default()
        };
        let v: serde_json::Value = serde_json::from_str(&kiosk_content_json(&content)).unwrap();
        assert_eq!(v["kind"], "content");
        assert_eq!(v["content_kind"], "scripture");
        assert_eq!(v["reference"], "John 3:16");
        assert_eq!(v["stage_note"], "hold for prayer");
        assert_eq!(v["next_reference"], "John 3:17");
        assert_eq!(v["next_text"], "For God sent not...");
        assert_eq!(v["service_started_at"], 1_700_000_000_000_i64);
        assert_eq!(v["service_target_ms"], 1_800_000);
    }

    #[test]
    fn the_themes_blob_is_only_stored_when_it_is_a_valid_json_array() {
        let hub = KioskHub::default();
        // A well-formed array is kept verbatim.
        hub.cache_themes(r#"[{"id":1,"name":"Mine","style":{}}]"#);
        assert!(hub.themes_handle().lock().unwrap().contains("Mine"));
        // Junk, a non-array, or an object all fall back to "[]" so the value can
        // never corrupt the WS frame it is embedded raw into.
        for bad in [r#"not json"#, r#"{"id":1}"#, r#"42"#, r#"null"#] {
            hub.cache_themes(bad);
            assert_eq!(
                hub.themes_handle().lock().unwrap().as_str(),
                "[]",
                "bad blob {bad}"
            );
        }
    }

    #[test]
    fn an_ad_hoc_output_window_is_not_mistaken_for_a_channel() {
        // `open_output_window` still mints counter labels. Reading one of those as
        // a channel id would light up an unrelated channel.
        assert_eq!(channel_id_of("output-1"), None);
        assert_eq!(channel_id_of("main"), None);
        assert_eq!(channel_id_of("output-chX"), None);
    }

    #[tokio::test]
    async fn a_kiosk_client_is_counted_while_connected_and_not_after() {
        // The leak this guards: without the drop-guard, a kiosk screen that
        // reconnects across a service leaves a phantom client counted on every
        // previous connection, and the channel reads ONLINE with a dead screen.
        let hub = KioskHub::default();
        let clients = hub.clients_handle();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.themes_handle(),
            8202,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        assert_eq!(clients.count(4), 0, "nothing connected yet");

        let (ws, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:8202")
            .await
            .expect("connect");
        let (mut write, _read) = ws.split();
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"kind":"hello","template_id":4}"#.to_string(),
            ))
            .await
            .expect("send hello");
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert_eq!(clients.count(4), 1, "hello should register the client");
        assert_eq!(clients.count(9), 0, "only on the template it asked for");

        // Drop the socket — the server task must notice and deregister.
        drop(write);
        drop(_read);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert_eq!(clients.count(4), 0, "disconnect must not leak a client");
    }

    #[test]
    fn re_saying_hello_moves_a_client_rather_than_double_counting_it() {
        // A kiosk page reloading onto a different template says hello twice on one
        // socket. Counting it on both would show two channels online for one screen.
        let reg = ClientRegistry::default();
        let mut held = Some(reg.join(1));
        assert!(held.is_some());
        assert_eq!(reg.count(1), 1);

        held = Some(reg.join(2)); // assignment drops the previous guard
        assert_eq!(reg.count(2), 1);
        assert_eq!(reg.count(1), 0, "the old template must be released");

        drop(held.take());
        assert_eq!(reg.count(2), 0);
    }

    #[test]
    fn the_client_count_never_underflows() {
        // usize wrapping here would report a channel as online with ~1.8e19
        // clients, forever.
        let reg = ClientRegistry::default();
        drop(reg.join(3));
        drop(reg.join(3));
        assert_eq!(reg.count(3), 0);
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

    /// EVERY EXTENSION THE LIBRARY IMPORTS MUST HAVE A REAL MIME TYPE.
    ///
    /// `application/octet-stream` is not a rendering failure the operator can
    /// see coming: a browser source shows nothing, or offers a download, and the
    /// screen stays black. The importer's accepted list and `mime_for` are one
    /// decision kept in two files, so this pins them together.
    #[test]
    fn mime_covers_every_imported_kind() {
        // Mirrors IMG / VID / DOC in src/lib/views/Library.svelte.
        let imported = [
            "png", "jpg", "jpeg", "gif", "webp", "bmp", "avif", "svg", // image
            "mp4", "mov", "webm", "mkv", "m4v", // video
            "pdf", "pptx", "ppt", // document
        ];
        for ext in imported {
            let mime = mime_for(&format!("x.{ext}"));
            assert_ne!(
                mime, "application/octet-stream",
                "the Library imports .{ext} but mime_for does not know it"
            );
        }
    }
}
