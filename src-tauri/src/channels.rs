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
    /// The decode pass that produced this content (`latency::Trace`), when it came
    /// from speech. Rides to every output — the native window and every kiosk
    /// browser source — purely so the page can report back the instant it painted,
    /// which is the only way to measure the last leg of the chain (fire sent →
    /// pixels on a projector) rather than assuming it is small.
    ///
    /// `None` for anything a human fired: an operator's own action has no decode
    /// pass behind it, and inventing one would put manual fires into a percentile
    /// that is supposed to describe the AI's path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<u64>,
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
// Generic over the runtime, like everything else on this path (CLAUDE.md rule
// 24). It was welded to the concrete desktop runtime, which meant nothing that
// runs under `tauri::test::mock_builder` could ask whether an output window was
// open — and `refresh_wake` has to ask exactly that.
pub fn list_open<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Vec<String> {
    app.webview_windows()
        .into_keys()
        .filter(|k| k.starts_with(OUTPUT_PREFIX))
        .collect()
}

// ===== OUTPUT HEALTH — does the screen still paint? =====

/// How often an output page reports that it is alive. Every output — the native
/// window on the projector and every kiosk/OBS browser source — ticks at this
/// rate.
pub const BEAT_INTERVAL_MS: u64 = 2_000;

/// How long a screen may go silent before Relay stops claiming it is painting.
///
/// **Derived from the interval, not written next to it.** Three beats of grace plus
/// half a beat of slack: generous enough to survive a slow frame or a garbage
/// collection, short enough that an operator glancing up during a service finds out
/// before the congregation does. Two independently-reasonable numbers sitting side
/// by side is how they drift, and both directions of drift are silent — too tight
/// and every healthy screen flickers into NOT RESPONDING, which teaches an operator
/// to ignore the one colour that matters; too loose and a dead projector reads
/// healthy for most of a sermon.
pub const BEAT_STALE_MS: u64 = BEAT_INTERVAL_MS * 3 + BEAT_INTERVAL_MS / 4;

/// What a screen last reported it was showing. A closed enum on purpose: this
/// value arrives over the WebSocket from a LAN client Relay does not authenticate
/// (DECISIONS §35), and it is rendered in the operator's console. A free-text
/// field here would be an injection surface into the one UI that must never lie —
/// so the wire carries a state, never a caption, and anything unrecognised is
/// dropped rather than displayed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaintState {
    /// Content is on this screen.
    Content,
    /// The screen is intentionally empty.
    Clear,
    /// The screen is blacked out.
    Black,
}

impl PaintState {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "content" => Some(PaintState::Content),
            "clear" => Some(PaintState::Clear),
            "black" => Some(PaintState::Black),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            PaintState::Content => "content",
            PaintState::Clear => "clear",
            PaintState::Black => "black",
        }
    }
}

struct Beat {
    at: std::time::Instant,
    state: PaintState,
    transport: &'static str,
}

/// Liveness of every output, reported BY the output.
///
/// ## Why this exists, and what it is not
///
/// A screen's status used to be inferred: a native channel was "online" if the app
/// still held a window object, and a networked channel was "online" unconditionally
/// because Relay was serving its URL. Both facts are true of a screen that has
/// frozen, crashed its renderer, or been unplugged — so the Live tab could read
/// **On Air** over a projector showing nothing, which is the one thing an operator
/// glances at the status pane to rule out.
///
/// The fix is that the screen answers for itself. Every output page ticks every
/// `BEAT_INTERVAL_MS` over whatever transport it already has: the native window
/// through the Tauri bridge, a kiosk/OBS source over the WebSocket it is already
/// listening on. No new port, no new connection, no new permission.
///
/// ## This is NOT device identity, and must not become it
///
/// `channels.rs` has promised that "Relay does not record who connected, from what
/// address, or when", and that promise is load-bearing — DECISIONS §35 accepted an
/// unauthenticated LAN control plane partly BECAUSE nothing here is tracking
/// anybody. This narrows exactly one word of it, deliberately and no further:
///
/// * **who** — still nothing. No address, no user agent, no id the client chose,
///   no cookie, no fingerprint. A beat says "the screen for channel N painted",
///   not "device X painted".
/// * **when** — an in-memory `Instant` per CHANNEL, overwritten by the next beat.
///   Not a history, not a log, never written to the database, gone on quit.
///
/// So this is anonymous liveness, and it stays that way. If a future change wants
/// to know *which* device, that is the pairing proposal in `docs/qa/RELAY_GAP.md` §20,
/// and it needs a human first.
#[derive(Clone, Default)]
pub struct OutputHealth {
    beats: Arc<Mutex<HashMap<i64, Beat>>>,
    /// What was last REPORTED about each channel, so an edge can be detected and
    /// written to the service timeline exactly once.
    ///
    /// Separate from the beat itself because they answer different questions:
    /// `beats` is "what did the screen last say", this is "what have we already
    /// told the operator". Folding them together would log a screen as lost on
    /// every poll for as long as it stayed lost, which is how a timeline becomes
    /// something nobody reads.
    reported: Arc<Mutex<HashMap<i64, bool>>>,
}

impl OutputHealth {
    /// Record that the screen for `channel_id` is alive and painting `state`.
    /// A lock poisoned by a panicking reader must not take the wall's status with
    /// it: a lost beat degrades to "silent", which is the safe direction.
    pub fn beat(&self, channel_id: i64, state: PaintState, transport: &'static str) {
        if channel_id <= 0 {
            return;
        }
        if let Ok(mut m) = self.beats.lock() {
            m.insert(
                channel_id,
                Beat {
                    at: std::time::Instant::now(),
                    state,
                    transport,
                },
            );
        }
    }

    /// Age of the last beat in milliseconds, plus what it said. `None` means this
    /// channel has never reported — which is an ABSENCE, not a zero, and callers
    /// must render it as "no answer yet" rather than as a fresh beat.
    pub fn read(&self, channel_id: i64) -> Option<(u64, PaintState, &'static str)> {
        let m = self.beats.lock().ok()?;
        let b = m.get(&channel_id)?;
        Some((b.at.elapsed().as_millis() as u64, b.state, b.transport))
    }

    /// True only if this channel reported within `BEAT_STALE_MS`.
    pub fn painting(&self, channel_id: i64) -> bool {
        matches!(self.read(channel_id), Some((age, _, _)) if age <= BEAT_STALE_MS)
    }

    /// Drop a channel's beat — called when its window is deliberately closed, so a
    /// reopened screen starts from "no answer yet" instead of inheriting a stale
    /// one that would read as freshly silent.
    pub fn forget(&self, channel_id: i64) {
        if let Ok(mut m) = self.beats.lock() {
            m.remove(&channel_id);
        }
        self.forget_transition(channel_id);
    }

    /// Has this channel's answering state CHANGED since the last time anyone
    /// looked? Returns the new value on an edge, `None` otherwise.
    ///
    /// The status poll is what notices — there is no other regular tick on this
    /// path, and adding a timer to watch screens that are already being watched
    /// twice a second would be a second answer to one question. That means this
    /// mutates from inside what reads like a query, which is worth stating plainly
    /// rather than discovering: `channel_status` is the edge detector.
    pub fn transition(&self, channel_id: i64, painting: bool) -> Option<bool> {
        let mut m = self.reported.lock().ok()?;
        match m.insert(channel_id, painting) {
            Some(prev) if prev == painting => None,
            // First sighting of a HEALTHY screen is not an event — it is the
            // normal case, and a timeline that opens with "Main screen recovered"
            // for every screen every service is noise.
            None if painting => None,
            _ => Some(painting),
        }
    }

    /// Stop tracking a channel's edges — it is no longer attached, so neither
    /// "lost" nor "recovered" would mean anything about it.
    pub fn forget_transition(&self, channel_id: i64) {
        if let Ok(mut m) = self.reported.lock() {
            m.remove(&channel_id);
        }
    }
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
        // Rides to every kiosk client purely so it can report back when it painted
        // — the last leg of the latency chain, over the real church network. See
        // `OutputContent::trace_id` and the `rendered` message the hub accepts.
        "trace_id": content.trace_id,
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
    /// A count, not a client list: Relay does not record WHO connected, or from
    /// what address — no identity, no address, nothing the client chose, nothing
    /// persisted. That part of the promise is unchanged and load-bearing
    /// (DECISIONS §35).
    ///
    /// The "or when" half was narrowed on purpose when outputs began reporting
    /// that they are still painting: `OutputHealth` holds one in-memory instant
    /// per CHANNEL, overwritten by the next beat and gone on quit. It answers
    /// "is that screen alive", never "who is watching". See `OutputHealth`.
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

/// A port nothing else on this machine is using.
///
/// These tests used to bind FIXED ports (8199–8205). On 2026-09-03 the whole Rust
/// suite failed on `kiosk_ws_forwards_published_content` with a response carrying
/// `x-powered-by: PHP/8.5.9` — an unrelated PHP dev server for a different project
/// already held `127.0.0.1:8199`. Relay's server bound `0.0.0.0:8199` and
/// *succeeded* (tokio sets `SO_REUSEADDR`), the more specific loopback binding won
/// the connection, and the test spent its life talking to somebody else's web
/// server.
///
/// That is worse than a port clash: nothing errored, the server printed that it was
/// listening, and the failure read as a regression in code that had not changed.
/// Asking the OS for a port removes the whole class, and the port it returns is
/// loopback-specific, so a foreign wildcard listener cannot shadow it either.
#[cfg(test)]
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = l.local_addr().expect("addr").port();
    drop(l);
    port
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
#[allow(clippy::too_many_arguments)]
pub async fn run_kiosk_server(
    on_error: ErrorSink,
    tx: broadcast::Sender<String>,
    templates: Arc<Mutex<HashMap<i64, String>>>,
    clients: ClientRegistry,
    themes: Arc<Mutex<String>>,
    health: OutputHealth,
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
    // THE SAME TWO BOUNDS THE HTTP SERVER HAS, ON THE PORT THAT DID NOT GET THEM.
    //
    // RG-90 gave `:8032` a read deadline and RG-97 gave it a connection cap; this
    // port had neither, and it is on the same church LAN. A socket that completed
    // the TCP handshake and then never sent a WebSocket upgrade held a task and a
    // descriptor for the life of the process — the RG-90 finding verbatim, on the
    // door RG-90 did not check. Nothing hostile is needed: a port scanner sweeping
    // the network produces them, silently, across a service.
    let in_flight = std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_KIOSK_CLIENTS));
    loop {
        let Ok((stream, _addr)) = listener.accept().await else {
            continue;
        };
        // Over the cap the socket is dropped rather than queued. A screen retries;
        // a queued socket is the resource this cap exists to bound.
        let Ok(permit) = in_flight.clone().try_acquire_owned() else {
            continue;
        };
        let mut rx = tx.subscribe();
        let templates = templates.clone();
        let clients = clients.clone();
        let themes = themes.clone();
        let health = health.clone();
        tokio::spawn(async move {
            let _permit = permit;
            // A kiosk `hello`, a `beat` and a `rendered` are a few hundred bytes
            // each. tungstenite's defaults are 64 MiB per message and 16 MiB per
            // frame, and every frame is buffered whole before `serde_json` sees
            // it — on an unauthenticated port, that is a memory bound set by
            // whoever is on the wifi rather than by Relay.
            let cfg = tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
                max_message_size: Some(64 * 1024),
                max_frame_size: Some(64 * 1024),
                ..Default::default()
            };
            let ws = match tokio::time::timeout(
                REQUEST_READ_TIMEOUT,
                tokio_tungstenite::accept_hdr_async_with_config(stream, origin_gate, Some(cfg)),
            )
            .await
            {
                Ok(Ok(w)) => w,
                // A handshake that never arrives, one that fails and one that is
                // refused are the same outcome here: nothing to serve, so let the
                // task end and give the slot back.
                Ok(Err(_)) | Err(_) => return,
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
                                // A kiosk page reporting that it has PAINTED a verse.
                                //
                                // This is the ONLY inbound message that is not a
                                // hello, and it is deliberately inert: it stamps a
                                // latency trace and touches nothing else. A kiosk
                                // client still cannot push to the screens (the
                                // read-only guarantee this server is built on) —
                                // the worst a hostile client on the LAN can do with
                                // it is make a diagnostic number wrong, and only for
                                // a trace id it managed to guess inside the ten
                                // seconds one stays open.
                                //
                                // It exists because the last leg — fire sent to
                                // pixels on the projector, over the real church
                                // network — is the one stage nothing else can see,
                                // and "the output path is probably fast" is not a
                                // measurement.
                                if v.get("kind").and_then(|k| k.as_str()) == Some("rendered") {
                                    if let (Some(id), Some(at)) = (
                                        v.get("trace_id").and_then(|i| i.as_u64()),
                                        v.get("at").and_then(|a| a.as_u64()),
                                    ) {
                                        crate::latency::frontend_mark(
                                            id,
                                            crate::latency::Stage::OutputRendered,
                                            at,
                                        );
                                    }
                                }
                                // A screen reporting that it is still painting.
                                //
                                // Same shape and the same guarantee as `rendered`
                                // above: inert, read-only, unable to push anything
                                // to any screen. The worst a hostile client on the
                                // LAN can do with it is claim a channel is alive —
                                // and a screen wrongly reported as HEALTHY is a
                                // real (if small) harm, so this deliberately
                                // carries no free text: `state` is parsed against a
                                // closed enum and anything else is dropped, and the
                                // channel is an integer that must already exist for
                                // the status view to show it at all.
                                if v.get("kind").and_then(|k| k.as_str()) == Some("beat") {
                                    if let (Some(ch), Some(st)) = (
                                        v.get("channel").and_then(|c| c.as_i64()),
                                        v.get("state")
                                            .and_then(|s| s.as_str())
                                            .and_then(PaintState::parse),
                                    ) {
                                        health.beat(ch, st, "kiosk");
                                    }
                                }
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

async fn serve_embedded<S>(request_path: &str, range: Option<&str>, stream: &mut S)
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
        serve_media_file(rest, range, stream).await;
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

/// What a `Range:` header asks for, once it has been read against a known length.
///
/// A player asks for a range for two reasons and both matter on a church LAN: to
/// SEEK (jump to 40 s of a five-minute clip) and to RESUME a connection that
/// dropped. Without an answer it can do neither — it starts the clip again from
/// zero, which is what an operator sees as "the video restarted itself".
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RangeAsk {
    /// No `Range:` header, or one this server does not implement (multiple
    /// ranges, a unit other than bytes). Answer 200 with the whole file — a
    /// client that asked for a range and got the whole thing still works.
    Whole,
    /// `start..=end`, inclusive, both already clamped inside the file.
    Slice { start: u64, end: u64 },
    /// A syntactically valid byte range that starts past the end of the file.
    /// RFC 9110 says 416, and says the reply must carry `Content-Range: bytes */len`
    /// so the client can correct itself.
    Unsatisfiable,
}

/// Parse one `Range:` header value against a known content length.
///
/// Deliberately narrow: a single byte range, which is what every browser
/// `<video>` sends. `bytes=0-`, `bytes=500-999` and `bytes=-500` (the last 500
/// bytes) are the three forms in practice; anything else falls back to `Whole`
/// rather than guessing, because serving the WRONG bytes with a 206 hands a
/// player a corrupt file, and serving the whole file merely costs bandwidth.
pub(crate) fn parse_range(value: &str, len: u64) -> RangeAsk {
    let spec = match value.trim().strip_prefix("bytes=") {
        Some(s) => s.trim(),
        None => return RangeAsk::Whole,
    };
    // One range only. `a-b,c-d` needs a multipart/byteranges body; we do not
    // build one, and a 200 is a correct answer to a range request.
    if spec.contains(',') {
        return RangeAsk::Whole;
    }
    let (first, last) = match spec.split_once('-') {
        Some(p) => p,
        None => return RangeAsk::Whole,
    };
    let (first, last) = (first.trim(), last.trim());
    if len == 0 {
        return RangeAsk::Unsatisfiable;
    }
    if first.is_empty() {
        // `bytes=-N` — the final N bytes.
        let n: u64 = match last.parse() {
            Ok(n) => n,
            Err(_) => return RangeAsk::Whole,
        };
        if n == 0 {
            return RangeAsk::Unsatisfiable;
        }
        let start = len.saturating_sub(n);
        return RangeAsk::Slice {
            start,
            end: len - 1,
        };
    }
    let start: u64 = match first.parse() {
        Ok(n) => n,
        Err(_) => return RangeAsk::Whole,
    };
    if start >= len {
        return RangeAsk::Unsatisfiable;
    }
    let end = if last.is_empty() {
        len - 1
    } else {
        match last.parse::<u64>() {
            // A player may ask for more than there is; the answer is what exists,
            // not an error.
            Ok(n) => n.min(len - 1),
            Err(_) => return RangeAsk::Whole,
        }
    };
    if end < start {
        return RangeAsk::Unsatisfiable;
    }
    RangeAsk::Slice { start, end }
}

/// The policy every imported file is served under.
///
/// **An SVG is a document, not a picture.** The Library imports one as an image
/// and `mime_for` answers `image/svg+xml`, which is correct — and a browser that
/// opens `http://<relay>:8032/media/7` TOP-LEVEL runs any script inside it, on
/// the same origin as `output.html` and the kiosk socket. `nosniff` does not help:
/// the type is right, it is the type that is executable. The file arrives by
/// import, so it is the operator's own file — but the operator's own file is
/// exactly how a graphic pack from the internet gets onto a church laptop.
///
/// `default-src 'none'` with inline styles allowed keeps a designed SVG looking
/// the way its author drew it while giving it nothing to run and nowhere to send.
/// **Applied to SVG replies only, deliberately.** A CSP on an image or a video
/// response is ignored by a browser rendering it as a subresource, and `sandbox`
/// on a subresource has a history of being honoured inconsistently — so putting
/// this header on a background video would risk a blank wall on a Sunday to
/// protect a case that does not exist. SVG is the one imported type a browser
/// will execute.
pub(crate) const MEDIA_CSP: &str = "default-src 'none'; img-src data:; style-src 'unsafe-inline'";

/// The header line for a media reply: present for SVG, absent for everything else.
fn media_csp_header(mime: &str) -> String {
    if mime.starts_with("image/svg") {
        format!("Content-Security-Policy: {MEDIA_CSP}\r\n")
    } else {
        String::new()
    }
}

/// Serve an imported media/document file by its DB id from `media_dir()`. Files
/// are stored as `{id}_{name}`; we take the leading digits of the request as the
/// id (so `../` and other traversal can't escape the media dir) and stream the
/// file, honouring a single-byte-range request.
async fn serve_media_file<S>(id_part: &str, range: Option<&str>, stream: &mut S)
where
    S: tokio::io::AsyncWrite + Unpin,
{
    serve_media_from_dir(&crate::db::media_dir(), id_part, range, stream).await
}

/// The same, with the directory as a parameter so a test can serve a real file
/// from a temporary directory without touching the machine's app-data path or
/// mutating `RELAY_DB_PATH` under every other test running in the same process.
pub(crate) async fn serve_media_from_dir<S>(
    dir: &std::path::Path,
    id_part: &str,
    range: Option<&str>,
    stream: &mut S,
) where
    S: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncSeekExt;
    use tokio::io::AsyncWriteExt;
    let id: String = id_part.chars().take_while(|c| c.is_ascii_digit()).collect();
    let found = if id.is_empty() {
        None
    } else {
        let prefix = format!("{id}_");
        std::fs::read_dir(dir).ok().and_then(|rd| {
            rd.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(&prefix))
                    .unwrap_or(false)
            })
        })
    };
    // Open and measure; do NOT read. This used to be `std::fs::read` — the whole
    // file into a `Vec<u8>` before a single byte went out — so a 400 MB background
    // loop cost 400 MB of resident memory PER REQUEST, and a wall, a stage screen
    // and an OBS machine asking for the same clip during a service cost three
    // copies of it on the laptop running the sermon. The bytes are the same; where
    // they live while they travel is not.
    let opened = match found {
        Some(p) => match tokio::fs::File::open(&p).await {
            Ok(f) => match f.metadata().await {
                Ok(m) => Some((p, f, m.len())),
                Err(_) => None,
            },
            Err(_) => None,
        },
        None => None,
    };
    match opened {
        Some((path, mut file, len)) => {
            let mime = mime_for(&path.to_string_lossy());
            let csp = media_csp_header(mime);
            let ask = match range {
                Some(v) => parse_range(v, len),
                None => RangeAsk::Whole,
            };
            // `Accept-Ranges` rides on EVERY reply, including the 200 and the 416.
            // It is the only way a player learns it may seek at all; without it a
            // browser disables the scrub bar however well the 206 works.
            let (start, end) = match ask {
                RangeAsk::Unsatisfiable => {
                    let msg = b"Range not satisfiable";
                    let header = format!(
                        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nAccept-Ranges: bytes\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
                        len,
                        msg.len()
                    );
                    let _ = stream.write_all(header.as_bytes()).await;
                    let _ = stream.write_all(msg).await;
                    return;
                }
                RangeAsk::Whole => (0, len.saturating_sub(1)),
                RangeAsk::Slice { start, end } => (start, end),
            };
            let partial = !matches!(ask, RangeAsk::Whole);
            let count = if len == 0 { 0 } else { end - start + 1 };
            if partial && file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                return;
            }
            let header = if partial {
                format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Type: {mime}\r\nContent-Length: {count}\r\nContent-Range: bytes {start}-{end}/{len}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nX-Content-Type-Options: nosniff\r\n{csp}Cache-Control: no-cache\r\nConnection: close\r\n\r\n"
                )
            } else {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {len}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nX-Content-Type-Options: nosniff\r\n{csp}Cache-Control: no-cache\r\nConnection: close\r\n\r\n"
                )
            };
            if stream.write_all(header.as_bytes()).await.is_err() {
                return;
            }
            let mut remaining = count;
            let mut buf = vec![0u8; 64 * 1024];
            while remaining > 0 {
                let want = remaining.min(buf.len() as u64) as usize;
                match file.read(&mut buf[..want]).await {
                    Ok(0) => break,
                    Ok(n) => {
                        // A screen that closed its tab mid-clip is an ordinary end,
                        // not an error to report anywhere.
                        if stream.write_all(&buf[..n]).await.is_err() {
                            return;
                        }
                        remaining -= n as u64;
                    }
                    // Truncated rather than wrong: the header already promised a
                    // length, so the client sees a short read and retries. Inventing
                    // padding would hand a player a corrupt file.
                    Err(_) => break,
                }
            }
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

/// How long a connection has to send its request line before Relay drops it.
///
/// Far longer than a LAN request takes, far shorter than a service lasts.
pub(crate) const REQUEST_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// The largest request head this server will read before giving up on a client.
///
/// A LAN request from a browser, a phone or OBS is a few hundred bytes. Anything
/// approaching this is either broken or trying to make the server hold memory.
pub(crate) const MAX_HEAD_BYTES: usize = 8 * 1024;

/// How many requests may be served at once. See the comment at the accept loop.
pub(crate) const MAX_CONCURRENT_REQUESTS: usize = 64;

/// The handshake callback: refuse a client whose `Origin` is not one of Relay's.
///
/// WHO IS ALLOWED TO LISTEN (RG-108). The hub subscribes a client to the content
/// feed at ACCEPT — before and without a `hello` — and WebSockets are exempt from
/// CORS, so any plain-`http:` page a congregant opened on the church wifi could
/// `new WebSocket('ws://<relay>:8031')` and receive the service. What travels is
/// not only the verse on the projector: `kiosk_content_json` carries `stage_note`,
/// `next_reference` and `next_text` — the preacher's own monitor. DECISIONS §35
/// accepts that somebody in the room can SEE the wall; this is content leaving the
/// building, which SECURITY.md ranks above everything else.
///
/// The check has to be here rather than after `hello`, because a check after the
/// subscription is a check after the leak.
#[allow(
    clippy::result_large_err,
    reason = "the handshake callback's signature takes and returns http::Response by value"
)]
fn origin_gate(
    req: &tokio_tungstenite::tungstenite::handshake::server::Request,
    resp: tokio_tungstenite::tungstenite::handshake::server::Response,
) -> Result<
    tokio_tungstenite::tungstenite::handshake::server::Response,
    tokio_tungstenite::tungstenite::handshake::server::ErrorResponse,
> {
    let origin = req
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    if kiosk_origin_allowed(origin.as_deref()) {
        return Ok(resp);
    }
    // Printed, because the failure mode this creates is a screen that stays blank
    // with no explanation, on a Sunday. The refusal names the origin and the way out.
    println!(
        "kiosk: refused a connection from origin {:?} — Relay serves its pages on \
         :{}, so that is the origin it trusts. A kiosk page hosted elsewhere needs \
         RELAY_KIOSK_ANY_ORIGIN=1.",
        origin.unwrap_or_default(),
        crate::sysprobe::HTTP_PORT
    );
    // 403, not the default. `ErrorResponse::new` builds a 200, and tungstenite
    // refuses to write a SUCCESSFUL refusal — so the client got a bare close with
    // no status at all, which is indistinguishable from Relay being down. A page
    // that is refused should be able to say why in its own console.
    let mut refusal = tokio_tungstenite::tungstenite::handshake::server::ErrorResponse::new(Some(
        "origin not allowed".into(),
    ));
    *refusal.status_mut() = tokio_tungstenite::tungstenite::http::StatusCode::FORBIDDEN;
    Err(refusal)
}

/// May a browser at this `Origin` join the kiosk feed? (RG-108)
///
/// ## The rule, and why each half of it
///
/// **No `Origin` at all → allowed.** A browser always sends one on a WebSocket
/// handshake; a native client, a diagnostic tool and this repository's own tests
/// do not. Refusing them would break the things that are not the threat.
///
/// **An origin on Relay's own ports → allowed.** The pages that legitimately open
/// this socket are `output.html` and `stage.html`, and Relay serves them itself:
/// on `:8032` in a packaged build, on the Vite dev port in development. The HOST
/// cannot be checked, because it is whatever LAN address the church laptop has
/// that morning — but the PORT is Relay's own, and a page on it came from Relay.
/// An OBS browser source pointed at `http://<ip>:8032/output.html` sends exactly
/// that origin, which is the client this must not break.
///
/// **Anything else → refused**, including `null` (a page opened from a file, and
/// also what a sandboxed frame sends).
///
/// ## The escape hatch, and why it exists
///
/// A church that hosts its own kiosk page somewhere else — a Raspberry Pi, an
/// existing signage box — is a real setup, and discovering on a Sunday that the
/// screen no longer connects is the worst possible time. `RELAY_KIOSK_ANY_ORIGIN=1`
/// restores the previous behaviour, and the refusal message above names it. It is
/// an env var rather than a setting on purpose: it is a decision about the church's
/// network, not about the service.
pub(crate) fn kiosk_origin_allowed(origin: Option<&str>) -> bool {
    let Some(origin) = origin else {
        return true;
    };
    if std::env::var("RELAY_KIOSK_ANY_ORIGIN").is_ok_and(|v| v == "1") {
        return true;
    }
    // Relay's OWN webview. A bundled page has no LAN port in its origin — Tauri
    // serves it from `tauri://localhost` (macOS) or `http://tauri.localhost`
    // (Windows). `Output.svelte` only reaches for the socket when the Tauri event
    // bridge is missing, which should not happen in a packaged build — but a
    // fallback that is refused is a blank wall, and this is Relay talking to
    // itself.
    if origin == "tauri://localhost" || origin == "http://tauri.localhost" {
        return true;
    }
    let Some(rest) = origin.strip_prefix("http://") else {
        // `https://` cannot be Relay (the LAN server is plain HTTP), and `null`
        // is not a host.
        return false;
    };
    let Some((_host, port)) = rest.rsplit_once(':') else {
        // No port means :80, which Relay never serves on.
        return false;
    };
    port.parse::<u16>()
        .is_ok_and(|p| p == crate::sysprobe::HTTP_PORT || p == DEV_CONSOLE_PORT)
}

/// The Vite dev server's port. `output.html` is served from it under
/// `npm run tauri dev`, and from nowhere in a packaged build.
pub(crate) const DEV_CONSOLE_PORT: u16 = 5032;

/// How many kiosk WebSocket clients may be connected at once.
///
/// A church runs a wall, a stage screen, an OBS machine and perhaps a phone. This
/// is an order of magnitude above that and far below the point where the laptop
/// running the sermon notices.
pub(crate) const MAX_KIOSK_CLIENTS: usize = 32;

/// Read the request head — everything up to the blank line — under ONE deadline.
///
/// `None` means the client said nothing, said too much, or ran out of time; the
/// caller drops the connection. Reading to the terminator rather than taking the
/// first packet is what makes a header AFTER the request line (`Range:`) reliable:
/// TCP may deliver a request in any number of segments, and a browser seeking in a
/// video is exactly the client most likely to be on a slow wifi link.
async fn read_request_head<S>(stream: &mut S, deadline: std::time::Duration) -> Option<String>
where
    S: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;
    let mut head: Vec<u8> = Vec::with_capacity(1024);
    let mut buf = [0u8; 2048];
    let read_all = async {
        loop {
            let n = stream.read(&mut buf).await.ok()?;
            if n == 0 {
                // A client that closed before finishing its head has no request.
                return None;
            }
            head.extend_from_slice(&buf[..n]);
            if head.windows(4).any(|w| w == b"\r\n\r\n") || head.windows(2).any(|w| w == b"\n\n") {
                return Some(String::from_utf8_lossy(&head).into_owned());
            }
            if head.len() >= MAX_HEAD_BYTES {
                return None;
            }
        }
    };
    tokio::time::timeout(deadline, read_all)
        .await
        .unwrap_or_default()
}

/// One header's value out of a request head, matched case-insensitively.
///
/// HTTP field names are case-insensitive and clients differ: Safari sends
/// `Range`, some players send `range`. Matching one spelling is a seek that works
/// on one browser and not another.
fn header_value(head: &str, name: &str) -> Option<String> {
    head.lines()
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            if k.trim().eq_ignore_ascii_case(name) {
                Some(v.trim().to_string())
            } else {
                None
            }
        })
}

pub async fn run_output_http_server(on_error: ErrorSink, api: ApiSink, port: u16) {
    run_output_http_server_with_timeout(on_error, api, port, REQUEST_READ_TIMEOUT).await
}

/// The same server, with the idle-connection deadline as a parameter.
///
/// The parameter exists so a test can assert the BEHAVIOUR in milliseconds instead
/// of seconds. The first attempt used tokio's paused clock, and it was flaky: with
/// two tasks holding timers, auto-advance can reach the client's deadline before
/// the server task has registered its own, so the test failed about one run in ten
/// on code that was correct. A flaky test on a timeout is worse than no test — it
/// teaches whoever sees it red to run it again.
///
/// `run_output_http_server` above is the only production caller and it passes
/// `REQUEST_READ_TIMEOUT`, so the constant is still the shipped value and this
/// indirection cannot drift away from it.
pub async fn run_output_http_server_with_timeout(
    on_error: ErrorSink,
    api: ApiSink,
    port: u16,
    read_timeout: std::time::Duration,
) {
    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
        Ok(l) => l,
        Err(e) => {
            on_error(bind_failure_message("The LAN output page server", port, &e));
            return;
        }
    };
    println!("output http: serving output/stage pages on :{port}");
    // HOW MANY REQUESTS MAY BE IN FLIGHT AT ONCE.
    //
    // Every accepted socket used to spawn a task with no ceiling, and `/api/search`
    // runs a semantic scan and an FTS query per request. Nothing hostile is needed
    // to hurt: a kiosk page with a reload loop, or an OBS source retrying a clip,
    // can put the laptop running the sermon under load with nothing on any screen
    // to say why. The cap is far above what a church uses — a wall, a stage screen,
    // an OBS machine and a phone are four clients, and a browser opens at most a
    // handful of connections per page — and far below the point where the console
    // stops answering. Over the cap a connection is answered `503` and closed at
    // once rather than queued: a client that is told no retries, a client left
    // waiting holds a socket.
    let in_flight = std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS));
    loop {
        let Ok((mut stream, _addr)) = listener.accept().await else {
            continue;
        };
        let api = api.clone();
        let Ok(permit) = in_flight.clone().try_acquire_owned() else {
            tokio::spawn(async move {
                use tokio::io::AsyncWriteExt;
                let msg = b"Too many requests in flight";
                let header = format!(
                    "HTTP/1.1 503 Service Unavailable\r\nRetry-After: 1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    msg.len()
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(msg).await;
            });
            continue;
        };
        tokio::spawn(async move {
            // Held for the life of the reply, dropped with the task — so a client
            // that hangs up mid-stream returns its slot like any other.
            let _permit = permit;
            // A CONNECTION THAT NEVER SPEAKS MUST NOT BE HELD FOREVER.
            //
            // This awaited the first read with no deadline, so a socket that
            // connected and then said nothing kept a task, an 8 KiB buffer and a
            // file descriptor for the length of the process. Nothing malicious is
            // required to produce one: a browser that opens a speculative
            // connection, a kiosk that sleeps between the TCP handshake and the
            // request, a port scanner sweeping the church LAN. They accumulate
            // silently across a service and are freed only by quitting Relay.
            //
            // Five seconds is far longer than a LAN request line takes and far
            // shorter than a service — and it now bounds the WHOLE header block,
            // not the first packet of it. A client that sends its request line and
            // then dribbles headers one byte at a time is the same idle connection
            // wearing a hat.
            let Some(head) = read_request_head(&mut stream, read_timeout).await else {
                return;
            };
            // Parse "GET /path HTTP/1.1". The VERB is read, not discarded: it is
            // half of what stops a drive-by from driving the wall (DECISIONS §35).
            let mut first = head.lines().next().unwrap_or("").split_whitespace();
            let method = first.next().unwrap_or("GET");
            let path = first.next().unwrap_or("/");
            let range = header_value(&head, "range");
            if let Some(rest) = path.strip_prefix("/api/") {
                let reply = api(method, rest).unwrap_or_else(|| ApiReply {
                    status: 500,
                    body: "{\"ok\":false}".to_string(),
                    cors: false,
                });
                serve_json(&reply, &mut stream).await;
            } else {
                serve_embedded(path, range.as_deref(), &mut stream).await;
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
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nX-Content-Type-Options: nosniff\r\n{}{}Cache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
        let port = free_port();
        tokio::spawn(run_output_http_server(log_only(), no_api, port));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
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

        let mut s2 = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        s2.write_all(b"GET /nope.xyz HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let n2 = s2.read(&mut buf).await.unwrap();
        assert!(String::from_utf8_lossy(&buf[..n2]).starts_with("HTTP/1.1 404"));
    }

    /// A CONNECTION THAT NEVER SPEAKS MUST NOT BE HELD FOREVER.
    ///
    /// The first read had no deadline, so a socket that connected and then said
    /// nothing kept a task, an 8 KiB buffer and a file descriptor for the whole
    /// life of the process. Nothing hostile is needed to make one — a browser
    /// opening a speculative connection, a kiosk sleeping between the handshake
    /// and the request, a port scanner on the church LAN — and they accumulate
    /// silently across a service, freed only by quitting Relay.
    ///
    /// Driven with a 200 ms deadline instead of the shipped five seconds, so the
    /// assertion costs no wall time and — unlike the first attempt, which used
    /// tokio's paused clock — cannot race. With two tasks holding timers,
    /// auto-advance can reach the CLIENT's deadline before the server has
    /// registered its own, and that version failed about one run in ten on correct
    /// code. A flaky test about a timeout is worse than no test: it teaches whoever
    /// sees it red to run it again rather than read it.
    #[tokio::test]
    async fn a_connection_that_never_speaks_is_dropped() {
        use tokio::io::AsyncReadExt;
        let no_api: ApiSink = std::sync::Arc::new(|_: &str, _: &str| None);
        let port = free_port();
        tokio::spawn(run_output_http_server_with_timeout(
            log_only(),
            no_api,
            port,
            std::time::Duration::from_millis(200),
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        // Say nothing at all, then wait for the server to give up on us.
        //
        // The client's own deadline exists so that a REGRESSION is a failing test
        // rather than a hung one: without the server-side timeout this read never
        // returns, and a test that hangs forever in CI is worse than no test.
        let mut buf = [0u8; 64];
        let n = tokio::time::timeout(std::time::Duration::from_secs(10), s.read(&mut buf))
            .await
            .expect("the server never dropped an idle connection")
            .expect("read");
        assert_eq!(n, 0, "the server should have closed the connection");

        // And the SHIPPED deadline is the constant, not this test's 200 ms. The
        // production entry point takes no timeout argument precisely so there is
        // one value; this asserts it is still a real one.
        assert_eq!(REQUEST_READ_TIMEOUT, std::time::Duration::from_secs(5));
    }

    /// A JSON reply must carry `nosniff` like every other reply this server sends.
    ///
    /// The static path set it and the control plane did not, so the one surface
    /// that answers with attacker-influenceable strings (a search query echoes
    /// through `json_str`) was the one a browser was still free to sniff a content
    /// type for. The header costs nothing and closes the asymmetry.
    #[tokio::test]
    async fn a_json_reply_is_never_content_sniffed() {
        let mut sink: Vec<u8> = Vec::new();
        serve_json(
            &ApiReply {
                status: 200,
                body: "{\"ok\":true}".into(),
                cors: true,
            },
            &mut sink,
        )
        .await;
        let resp = String::from_utf8_lossy(&sink);
        assert!(resp.contains("X-Content-Type-Options: nosniff"), "{resp}");
        assert!(resp.contains("Access-Control-Allow-Origin: *"), "{resp}");
        assert!(resp.contains("Cache-Control: no-store"), "{resp}");
    }

    /// A mutating route still withholds the wildcard, nosniff or not. Guards the
    /// header addition against having widened what a cross-origin caller can read.
    #[tokio::test]
    async fn a_mutating_reply_still_withholds_the_cors_wildcard() {
        let mut sink: Vec<u8> = Vec::new();
        serve_json(
            &ApiReply {
                status: 405,
                body: "{\"ok\":false}".into(),
                cors: false,
            },
            &mut sink,
        )
        .await;
        let resp = String::from_utf8_lossy(&sink);
        assert!(
            resp.starts_with("HTTP/1.1 405 Method Not Allowed"),
            "{resp}"
        );
        assert!(resp.contains("Allow: POST"), "{resp}");
        assert!(!resp.contains("Access-Control-Allow-Origin"), "{resp}");
        assert!(resp.contains("X-Content-Type-Options: nosniff"), "{resp}");
    }

    // ── Ranged media (RG-96) ───────────────────────────────────────────────
    //
    // A `<video>` on a kiosk screen or an OBS source cannot SEEK and cannot
    // RESUME a dropped connection without a ranged reply; it starts the clip
    // again from zero, which an operator reads as "the video restarted itself".

    /// The three forms a browser actually sends, plus the two ways a range can be
    /// wrong. `Whole` is the deliberate answer to anything unsupported: a 200 with
    /// the whole file is always CORRECT, where a 206 carrying the wrong bytes
    /// hands a player a corrupt file.
    #[test]
    fn a_range_header_is_read_against_the_real_length() {
        assert_eq!(
            parse_range("bytes=0-", 1000),
            RangeAsk::Slice { start: 0, end: 999 }
        );
        assert_eq!(
            parse_range("bytes=500-999", 1000),
            RangeAsk::Slice {
                start: 500,
                end: 999
            }
        );
        // The final 100 bytes — how a player reads an MP4's moov atom when it is
        // at the end of the file.
        assert_eq!(
            parse_range("bytes=-100", 1000),
            RangeAsk::Slice {
                start: 900,
                end: 999
            }
        );
        // Asking for more than exists is answered with what exists.
        assert_eq!(
            parse_range("bytes=900-5000", 1000),
            RangeAsk::Slice {
                start: 900,
                end: 999
            }
        );
        // Case and whitespace are the client's business, not ours.
        assert_eq!(
            parse_range("  bytes=0-9  ", 1000),
            RangeAsk::Slice { start: 0, end: 9 }
        );
        // Past the end, backwards, and an empty file: 416, never a wrong slice.
        assert_eq!(parse_range("bytes=1000-", 1000), RangeAsk::Unsatisfiable);
        assert_eq!(parse_range("bytes=800-700", 1000), RangeAsk::Unsatisfiable);
        assert_eq!(parse_range("bytes=0-", 0), RangeAsk::Unsatisfiable);
        // Unsupported forms fall back to the whole file rather than guessing.
        assert_eq!(parse_range("bytes=0-9,20-29", 1000), RangeAsk::Whole);
        assert_eq!(parse_range("items=0-9", 1000), RangeAsk::Whole);
        assert_eq!(parse_range("bytes=abc-def", 1000), RangeAsk::Whole);
    }

    /// End to end over the real serving path: the slice is the right BYTES, the
    /// headers say so, and a whole-file GET still answers 200 while advertising
    /// that it could have done better.
    #[tokio::test]
    async fn a_ranged_media_request_is_answered_with_the_bytes_it_asked_for() {
        let dir = std::env::temp_dir().join(format!("relay-media-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let body: Vec<u8> = (0u8..=255).cycle().take(1000).collect();
        std::fs::write(dir.join("12_clip.mp4"), &body).unwrap();

        // Whole file: 200, and `Accept-Ranges` so a player enables its scrub bar.
        let mut sink: Vec<u8> = Vec::new();
        serve_media_from_dir(&dir, "12", None, &mut sink).await;
        let split = find_body(&sink);
        let head = String::from_utf8_lossy(&sink[..split]).to_string();
        assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
        assert!(head.contains("Accept-Ranges: bytes"), "{head}");
        assert!(head.contains("Content-Length: 1000"), "{head}");
        assert_eq!(&sink[split..], &body[..]);

        // A seek: 206, the exact slice, and a Content-Range naming the whole.
        let mut sink: Vec<u8> = Vec::new();
        serve_media_from_dir(&dir, "12", Some("bytes=500-599"), &mut sink).await;
        let split = find_body(&sink);
        let head = String::from_utf8_lossy(&sink[..split]).to_string();
        assert!(head.starts_with("HTTP/1.1 206 Partial Content"), "{head}");
        assert!(head.contains("Content-Range: bytes 500-599/1000"), "{head}");
        assert!(head.contains("Content-Length: 100"), "{head}");
        assert_eq!(&sink[split..], &body[500..600], "the wrong bytes were sent");

        // Past the end: 416 with the length, so the client can correct itself —
        // never a 200 pretending the request made sense.
        let mut sink: Vec<u8> = Vec::new();
        serve_media_from_dir(&dir, "12", Some("bytes=4000-"), &mut sink).await;
        let head = String::from_utf8_lossy(&sink).to_string();
        assert!(head.starts_with("HTTP/1.1 416"), "{head}");
        assert!(head.contains("Content-Range: bytes */1000"), "{head}");

        // The id is still the leading digits and nothing else: traversal cannot
        // reach outside the media directory (this is why the id is parsed, not
        // joined).
        let mut sink: Vec<u8> = Vec::new();
        serve_media_from_dir(&dir, "../../etc/passwd", None, &mut sink).await;
        assert!(
            String::from_utf8_lossy(&sink).starts_with("HTTP/1.1 404"),
            "traversal must not resolve"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn find_body(resp: &[u8]) -> usize {
        resp.windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|i| i + 4)
            .expect("no header terminator")
    }

    /// The head is read to its terminator, so a header sent in a SECOND packet is
    /// still seen. TCP may split a request anywhere, and the client most likely to
    /// be on a slow link is exactly the one seeking in a video.
    #[tokio::test]
    async fn a_header_that_arrives_in_a_second_packet_is_still_read() {
        let no_api: ApiSink = std::sync::Arc::new(|_: &str, _: &str| None);
        let port = free_port();
        tokio::spawn(run_output_http_server(log_only(), no_api, port));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        s.write_all(b"GET /stage.html HTTP/1.1\r\n").await.unwrap();
        s.flush().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        s.write_all(b"Range: bytes=0-9\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let mut buf = vec![0u8; 4096];
        let n = tokio::time::timeout(std::time::Duration::from_secs(5), s.read(&mut buf))
            .await
            .expect("the server never answered a split request")
            .unwrap();
        assert!(
            String::from_utf8_lossy(&buf[..n]).starts_with("HTTP/1.1 200"),
            "a request split across packets must still be served"
        );
    }

    /// A HEAD BLOCK THAT NEVER ENDS IS AN IDLE CONNECTION WEARING A HAT.
    ///
    /// Dribbling headers forever used to be outside the deadline, because only the
    /// FIRST read was timed. The deadline now covers the whole head.
    #[tokio::test]
    async fn a_client_that_dribbles_headers_forever_is_still_dropped() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let no_api: ApiSink = std::sync::Arc::new(|_: &str, _: &str| None);
        let port = free_port();
        tokio::spawn(run_output_http_server_with_timeout(
            log_only(),
            no_api,
            port,
            std::time::Duration::from_millis(300),
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        s.write_all(b"GET / HTTP/1.1\r\n").await.unwrap();
        let writer = tokio::spawn(async move {
            // Never send the blank line.
            for _ in 0..50 {
                if s.write_all(b"X-Pad: x\r\n").await.is_err() {
                    return s;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            s
        });
        let mut s = tokio::time::timeout(std::time::Duration::from_secs(10), writer)
            .await
            .expect("the writer never finished")
            .unwrap();
        let mut buf = [0u8; 64];
        // Either shape counts as dropped — see `assert_dropped`. What must NOT
        // happen is the read hanging, which is what it did before the deadline
        // covered the whole head.
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(5), s.read(&mut buf))
            .await
            .expect("the server never dropped a dribbling client");
        assert_dropped(outcome);
    }

    /// A DROPPED CONNECTION HAS TWO NAMES, AND THEY ARE PLATFORM-SPECIFIC.
    ///
    /// A read on a socket the peer has closed comes back as a clean EOF, or — if
    /// we kept writing to it first — as an error. macOS calls that error
    /// `ConnectionReset` (ECONNRESET) and **Windows calls it `ConnectionAborted`**
    /// (WSAECONNABORTED, 10053). Asserting only the first passed on this machine
    /// and failed on the Windows runner, on code that was behaving correctly —
    /// which is the shape of every Windows bug in this repository's history, for
    /// once caught by CI rather than by a church.
    fn assert_dropped(outcome: std::io::Result<usize>) {
        match outcome {
            Ok(n) => assert_eq!(n, 0, "the server should have closed the connection"),
            Err(e) => assert!(
                matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
                ),
                "unexpected error from a dropped connection: {e} ({:?})",
                e.kind()
            ),
        }
    }

    /// AN IMPORTED SVG IS A DOCUMENT, AND THIS PORT SERVES IT ON THE SAME ORIGIN
    /// AS THE OUTPUT PAGE.
    ///
    /// A video gets no policy: a CSP on a subresource is ignored by the browser
    /// painting it, and one that WAS honoured would risk a blank wall to protect a
    /// case that does not exist.
    #[tokio::test]
    async fn only_an_svg_is_served_under_a_content_policy() {
        let dir = std::env::temp_dir().join(format!("relay-media-csp-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("1_logo.svg"),
            b"<svg xmlns='http://www.w3.org/2000/svg'/>",
        )
        .unwrap();
        std::fs::write(dir.join("2_loop.mp4"), b"not really an mp4").unwrap();

        let mut svg: Vec<u8> = Vec::new();
        serve_media_from_dir(&dir, "1", None, &mut svg).await;
        let svg = String::from_utf8_lossy(&svg).to_string();
        assert!(svg.contains("image/svg+xml"), "{svg}");
        assert!(
            svg.contains("Content-Security-Policy: default-src 'none'"),
            "an SVG must be served with nothing it can run: {svg}"
        );

        let mut mp4: Vec<u8> = Vec::new();
        serve_media_from_dir(&dir, "2", None, &mut mp4).await;
        let mp4 = String::from_utf8_lossy(&mp4).to_string();
        assert!(mp4.contains("video/mp4"), "{mp4}");
        assert!(
            !mp4.contains("Content-Security-Policy"),
            "a video must not carry a policy that could stop it painting: {mp4}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RG-108 · WHO MAY LISTEN TO THE SERVICE.
    ///
    /// The rule in one table, because each row is a real client: no origin is a
    /// native tool, Relay's own ports are the pages Relay serves, and everything
    /// else is a page somebody opened on the church wifi.
    #[test]
    fn only_a_page_relay_served_may_join_the_kiosk_feed() {
        // A native client, a diagnostic tool, this file's other tests.
        assert!(kiosk_origin_allowed(None));
        // What an OBS browser source and a kiosk screen actually send — any LAN
        // host, because the address is whatever the laptop has that morning.
        assert!(kiosk_origin_allowed(Some("http://192.168.1.9:8032")));
        assert!(kiosk_origin_allowed(Some("http://relay-laptop.local:8032")));
        assert!(kiosk_origin_allowed(Some("http://localhost:8032")));
        // Development: the same page, served by Vite.
        assert!(kiosk_origin_allowed(Some("http://localhost:5032")));
        // Relay's own webview, which has no LAN port in its origin at all.
        assert!(kiosk_origin_allowed(Some("tauri://localhost")));
        assert!(kiosk_origin_allowed(Some("http://tauri.localhost")));

        // The finding itself: a page a congregant opened on the church wifi.
        assert!(!kiosk_origin_allowed(Some("http://evil.example.com")));
        assert!(!kiosk_origin_allowed(Some("http://192.168.1.55:3000")));
        // A file opened from disk, and a sandboxed frame, both say this.
        assert!(!kiosk_origin_allowed(Some("null")));
        // Relay's LAN server is plain HTTP, so an https origin is not Relay.
        assert!(!kiosk_origin_allowed(Some("https://192.168.1.9:8032")));
        // No port is :80, which Relay never serves on.
        assert!(!kiosk_origin_allowed(Some("http://192.168.1.9")));
    }

    /// And the refusal happens at the HANDSHAKE, before a single broadcast is
    /// forwarded — the hub subscribes at accept, so a check after `hello` would be
    /// a check after the leak.
    #[tokio::test]
    async fn a_page_from_another_origin_is_refused_the_socket() {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.themes_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let mut req = format!("ws://127.0.0.1:{port}")
            .into_client_request()
            .unwrap();
        req.headers_mut()
            .insert("origin", "http://evil.example.com".parse().unwrap());
        let refused = tokio_tungstenite::connect_async(req).await;
        let err = refused.expect_err("a page on another origin was handed the service feed");
        // A REAL 403, not a bare close. `ErrorResponse::new` builds a 200 and
        // tungstenite will not write a successful refusal, so the first version of
        // this dropped the connection with no status — indistinguishable, from the
        // kiosk's side, from Relay being switched off.
        match err {
            tokio_tungstenite::tungstenite::Error::Http(r) => assert_eq!(
                r.status(),
                tokio_tungstenite::tungstenite::http::StatusCode::FORBIDDEN,
                "the refusal has to say what it is"
            ),
            other => panic!("expected an HTTP refusal, got {other:?}"),
        }

        // …and a page Relay served is still let in, which is the half that breaks a
        // church if it is got wrong.
        let mut ok = format!("ws://127.0.0.1:{port}")
            .into_client_request()
            .unwrap();
        ok.headers_mut().insert(
            "origin",
            format!("http://127.0.0.1:{}", crate::sysprobe::HTTP_PORT)
                .parse()
                .unwrap(),
        );
        assert!(
            tokio_tungstenite::connect_async(ok).await.is_ok(),
            "a page served by Relay was refused its own hub"
        );
    }

    /// The kiosk port gets the same deadline the HTTP port has.
    ///
    /// A socket that connects to `:8031` and never sends a WebSocket upgrade used
    /// to hold a task and a descriptor for the life of the process — RG-90's
    /// finding on the door RG-90 did not check.
    #[tokio::test]
    async fn a_kiosk_socket_that_never_upgrades_is_dropped() {
        use tokio::io::AsyncReadExt;
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.themes_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        // Say nothing. The server has REQUEST_READ_TIMEOUT to give up; the client
        // deadline is longer, so a regression is a failing test rather than a hang.
        let mut buf = [0u8; 64];
        let outcome = tokio::time::timeout(
            REQUEST_READ_TIMEOUT + std::time::Duration::from_secs(4),
            s.read(&mut buf),
        )
        .await
        .expect("the kiosk server never dropped an idle connection");
        assert_dropped(outcome);
    }

    /// THE CONNECTION CAP (RG-97). Over the ceiling a client is told `503` at once
    /// rather than queued — a client that is told no retries; a client left waiting
    /// holds a socket on the laptop running the sermon.
    #[tokio::test]
    async fn a_flood_of_connections_is_refused_rather_than_queued() {
        use tokio::io::AsyncReadExt;
        let no_api: ApiSink = std::sync::Arc::new(|_: &str, _: &str| None);
        let port = free_port();
        tokio::spawn(run_output_http_server_with_timeout(
            log_only(),
            no_api,
            port,
            std::time::Duration::from_secs(3),
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Fill every slot with sockets that connect and say nothing: each holds a
        // permit until its read deadline.
        let mut held = Vec::new();
        for _ in 0..MAX_CONCURRENT_REQUESTS {
            held.push(
                tokio::net::TcpStream::connect(("127.0.0.1", port))
                    .await
                    .expect("connect"),
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        let mut buf = vec![0u8; 256];
        let n = tokio::time::timeout(std::time::Duration::from_secs(2), s.read(&mut buf))
            .await
            .expect("the server neither refused nor answered over the cap")
            .unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]).to_string();
        assert!(
            resp.starts_with("HTTP/1.1 503"),
            "over the cap the answer must be an immediate 503, got: {resp}"
        );
        assert!(resp.contains("Retry-After"), "{resp}");
        drop(held);
    }

    /// The #1 fix: a browser client (OBS/kiosk) says hello and gets back the REAL
    /// cached template, so it renders exactly what the editor shows.
    #[tokio::test]
    async fn kiosk_hello_returns_cached_template() {
        let port = free_port();
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
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
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
        let port = free_port();
        let hub = KioskHub::default();
        hub.cache_themes(r##"[{"id":3,"name":"Sanctuary","style":{"accent":"#abc"}}]"##);
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.themes_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
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
        let port = free_port();
        let hub = KioskHub::default();
        let tx = hub.sender();
        tokio::spawn(run_kiosk_server(
            log_only(),
            tx,
            hub.templates_handle(),
            hub.clients_handle(),
            hub.themes_handle(),
            OutputHealth::default(),
            port,
        ));
        // Give the listener a moment to bind.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
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
        let port = free_port();
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
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        assert_eq!(clients.count(4), 0, "nothing connected yet");

        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
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

    /// A KIOSK SCREEN REPORTS OVER THE SOCKET IT ALREADY HAS — and a malformed
    /// report is dropped, not defaulted.
    ///
    /// This is the OBS/browser-source half of output health. Its twin is the
    /// native window's `output_beat` command, and the two exist together on
    /// purpose: a guarantee kept on one door and skipped on the other is the
    /// single most repeated bug in this repository.
    ///
    /// The malformed half is the part worth testing. A beat arrives from a LAN
    /// client Relay does not authenticate, so a junk `state` that fell through to
    /// a default would let a hostile — or merely broken — client hold a dead
    /// screen's light green for a whole service.
    #[tokio::test]
    async fn a_kiosk_screen_reports_that_it_is_painting_and_junk_is_ignored() {
        let port = free_port();
        let hub = KioskHub::default();
        let health = OutputHealth::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.themes_handle(),
            health.clone(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");
        let (mut write, _read) = ws.split();

        assert!(health.read(5).is_none(), "nothing has reported yet");

        macro_rules! send {
            ($m:expr) => {
                write
                    .send(tokio_tungstenite::tungstenite::Message::Text(
                        $m.to_string(),
                    ))
                    .await
                    .expect("send")
            };
        }

        // A real beat lands.
        send!(r#"{"kind":"beat","channel":5,"state":"content"}"#);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert_eq!(
            health.read(5).map(|(_, st, tr)| (st, tr)),
            Some((PaintState::Content, "kiosk"))
        );

        // Junk in the state, a missing state, and a missing channel are each
        // dropped — none of them may overwrite what the screen last really said.
        for junk in [
            r#"{"kind":"beat","channel":5,"state":"ON AIR"}"#,
            r#"{"kind":"beat","channel":5}"#,
            r#"{"kind":"beat","state":"black"}"#,
            r#"{"kind":"beat","channel":"5","state":"black"}"#,
        ] {
            send!(junk);
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert_eq!(
            health.read(5).map(|(_, st, _)| st),
            Some(PaintState::Content),
            "a malformed beat must not change what a screen is reported to be showing"
        );

        // And a beat still cannot push anything to any screen: the hub's inbound
        // surface is unchanged in kind. Nothing was published.
        assert_eq!(
            hub.sender().receiver_count(),
            1,
            "only the server's own task"
        );
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

    // ── OUTPUT HEALTH ────────────────────────────────────────────────────

    /// THE BEAT MUST BE COMFORTABLY FASTER THAN THE STALENESS WINDOW.
    ///
    /// These two constants are one decision in two numbers, and getting the
    /// relationship wrong is silent in both directions: too tight and every
    /// healthy screen flickers into NOT RESPONDING on a slow frame, which teaches
    /// an operator to ignore the one colour that matters; too loose and a dead
    /// projector reads healthy for most of a sermon. Three beats of grace.
    #[test]
    fn the_beat_has_three_beats_of_grace_before_a_screen_is_called_silent() {
        // Read through locals so clippy sees a runtime comparison rather than a
        // const one — the assertion is the point, and `assertions_on_constants`
        // would have us delete it.
        let (interval, stale) = (BEAT_INTERVAL_MS, BEAT_STALE_MS);
        assert!(
            stale >= interval * 3,
            "a screen must be allowed to miss two beats: interval {interval}ms, stale {stale}ms"
        );
        // And not so loose that a screen can be dead for most of a reading.
        assert!(stale <= 10_000);
    }

    /// A SCREEN THAT HAS NEVER ANSWERED IS AN ABSENCE, NOT A ZERO.
    ///
    /// `latency.rs` learned this and it is the same mistake here: reporting an
    /// unknown screen as "0 ms since its last beat" would render as the freshest
    /// possible health, which is the exact inversion of the truth.
    #[test]
    fn a_screen_that_never_answered_is_absent_not_fresh() {
        let h = OutputHealth::default();
        assert!(h.read(7).is_none());
        assert!(!h.painting(7), "silence must never read as painting");
    }

    #[test]
    fn a_beat_makes_a_screen_painting_and_carries_what_it_said() {
        let h = OutputHealth::default();
        h.beat(7, PaintState::Content, "window");
        assert!(h.painting(7));
        let (age, state, transport) = h.read(7).expect("just beat");
        assert!(age < 1_000);
        assert_eq!(state, PaintState::Content);
        assert_eq!(transport, "window");
    }

    /// CHANNEL 0 IS A TEMPLATE PREVIEW, NOT A SCREEN.
    ///
    /// `output.html` defaults `?channel=` to 0 when it is opened as a raw preview.
    /// Recording a beat for it would invent a screen nobody configured, and it
    /// would then appear in a status view as an output going silent.
    #[test]
    fn a_preview_with_no_channel_reports_nothing() {
        let h = OutputHealth::default();
        h.beat(0, PaintState::Content, "window");
        h.beat(-1, PaintState::Content, "window");
        assert!(h.read(0).is_none());
        assert!(h.read(-1).is_none());
    }

    /// CLOSING A SCREEN ON PURPOSE MUST NOT LOOK LIKE ONE FAILING.
    ///
    /// Without this, reopening a channel inherits the beat of the window the
    /// operator deliberately closed, so the row reads "NOT RESPONDING for 40s"
    /// immediately after a completely normal action — and a status light that
    /// cries wolf is worse than none.
    #[test]
    fn forgetting_a_channel_resets_it_to_no_answer_yet() {
        let h = OutputHealth::default();
        h.beat(3, PaintState::Black, "kiosk");
        assert!(h.painting(3));
        h.forget(3);
        assert!(h.read(3).is_none());
    }

    /// THE WIRE CARRIES A STATE, NEVER A CAPTION.
    ///
    /// A kiosk beat crosses an unauthenticated LAN (DECISIONS §35) and lands in
    /// the operator's status pane. Anything outside the closed set is dropped at
    /// the door rather than defaulted — a malformed beat must not be able to keep
    /// a dead screen looking alive, and free text must never reach that pane.
    #[test]
    fn only_the_three_paint_states_parse() {
        assert_eq!(PaintState::parse("content"), Some(PaintState::Content));
        assert_eq!(PaintState::parse("clear"), Some(PaintState::Clear));
        assert_eq!(PaintState::parse("black"), Some(PaintState::Black));
        for junk in [
            "",
            "CONTENT",
            "on air",
            "<script>alert(1)</script>",
            "content ",
            "1",
        ] {
            assert_eq!(PaintState::parse(junk), None, "{junk:?} must not parse");
        }
        // Round-trips, so the console renders the same word Rust matched on.
        for st in [PaintState::Content, PaintState::Clear, PaintState::Black] {
            assert_eq!(PaintState::parse(st.as_str()), Some(st));
        }
    }

    /// A LATER BEAT REPLACES AN EARLIER ONE — a screen has one current state.
    #[test]
    fn the_latest_beat_wins() {
        let h = OutputHealth::default();
        h.beat(2, PaintState::Content, "window");
        h.beat(2, PaintState::Black, "kiosk");
        let (_, state, transport) = h.read(2).expect("beat");
        assert_eq!(state, PaintState::Black);
        assert_eq!(transport, "kiosk");
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
