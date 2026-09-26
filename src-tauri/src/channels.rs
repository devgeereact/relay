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
    /// WHICH SCREENS THIS CUE IS FOR (RG-161). `None` is every screen and is the
    /// default — a cue that says nothing about screens behaves exactly as every
    /// cue did before this existed.
    ///
    /// **A screen this does not name is UNTOUCHED, not cleared.** Targeting
    /// means "these screens change and the others carry on": a notice aimed at
    /// the foyer TV leaves the wall on the verse and leaves the reading the
    /// preacher is halfway through. The alternative — untargeted screens go
    /// blank — gives a plan cue the reach of a panic control, and a cue built on
    /// a Tuesday with one screen ticked would blank every other screen on the
    /// Sunday in front of somebody who never saw it happen.
    ///
    /// An EMPTY list is not the same as `None` and is deliberately allowed: it
    /// is a cue that reaches no screen, which is what a disabled row in a plan
    /// should do rather than quietly reaching all of them.
    ///
    /// The panic controls never carry this. A `clear` that has to ask which
    /// screens it is talking to is a `clear` that can fail (rule 15,
    /// DECISIONS §20).
    pub channels: Option<Vec<i64>>,
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
    /// Epoch (ms) this countdown was aimed FROM, so `countdown_to - countdown_from`
    /// is the length it was aimed for. That span is the ONLY input the warning rule
    /// has (`layers.js::countdownWarning`: the last minute, or the last tenth of a
    /// countdown shorter than ten minutes) — without it the rule falls back to a
    /// flat last minute, which on a two-minute countdown is a colour lit for half
    /// its life.
    ///
    /// It was read by `TemplateRender` and written by NOBODY for as long as it
    /// existed, so the short-countdown half of §7's rule had never once fired in the
    /// product. A reader with no writer and a control with no reader are the same
    /// defect facing opposite ways (DECISIONS §69). Written by `start_countdown` and
    /// carried, unchanged, by every re-aim.
    pub countdown_from: Option<i64>,
    /// **A PAUSED COUNTDOWN IS NOT AN INSTANT, WHICH IS WHY THIS FIELD EXISTS.**
    ///
    /// `countdown_to` is an absolute instant: every output ticks against its own
    /// clock, which is what keeps a per-second timer off the network. Nudging one is
    /// just re-aiming it (`+1` moves the instant), but HOLDING one cannot be said in
    /// that language at all — there is no instant that means "not moving".
    ///
    /// So when this is `Some(n)`, the countdown is HELD with `n` ms left and every
    /// reader shows `n` instead of ticking. `countdown_to` is still set — it is where
    /// the countdown would land if it were resumed at the moment it was held, kept so
    /// the content still reads as a countdown to `pipeline::preflight`, to the retained
    /// screen frame and to the slide key. Nothing may compute a remaining time from
    /// it while this is `Some`.
    pub countdown_paused_ms: Option<i64>,
    /// Message shown in place of the timer when the countdown reaches zero.
    pub countdown_done: Option<String>,
    /// **THE THRESHOLD SOMEBODY CHOSE FOR THIS COUNTDOWN**, in ms before zero, or
    /// None when nobody chose one. Projected from `timers::Timer::warn_ms` in
    /// exactly one place (`timers::project_both`).
    ///
    /// It is not a second reading of when to worry. There is one rule and it is
    /// `layers.js::countdownWarning`; this is the figure it ranks FIRST, ahead of
    /// the configured default below and ahead of the tenth-of-span rule behind
    /// that. None is an absent figure, never a window of zero — which would be a
    /// warning colour that never comes on.
    pub countdown_warn_ms: Option<i64>,
    /// **THE CONFIGURED DEFAULT, DELIVERED RATHER THAN READ** — `Settings → General
    /// → Countdown warning`, in ms.
    ///
    /// It rides with the content because the two screens that need it cannot ask
    /// for it. `output.html` and `stage.html` are served to a browser source and a
    /// tablet with no Tauri bridge and no console state, so the setting reached the
    /// console alone and every congregation screen kept the shipped minute (RG-149).
    /// Stamped at the one door content leaves by (`main::broadcast_with_clock`,
    /// rule 36), so a content path added later carries it by construction; the same
    /// figure reaches the stage on the programme frame, which arrives before any
    /// content does.
    ///
    /// It is NOT resolved against `countdown_warn_ms` here. Ranking the two is the
    /// far side's job, once, or there would be two authorities on when a screen
    /// turns red and a way for them to disagree.
    pub countdown_warn_default_ms: Option<i64>,
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
    /// WHEN RELAY SENT THIS CLIP, on Relay's own clock (ms since the epoch).
    ///
    /// The baseline every screen corrects itself against (RG-220). The operator
    /// asked for *"all media in sync"* and chose the two things Relay can
    /// honestly do: every screen STARTS together and is pulled back when it
    /// drifts. Frame-exact playback across independent browsers on church wifi
    /// is not one of them.
    ///
    /// It is a fact about the SENDING, so it is stamped at the one door content
    /// leaves by (`main::broadcast_with_clock`, rule 36) rather than at each
    /// path that can put a picture up — a media path added next year carries it
    /// by construction. Every page already knows Relay's clock through
    /// `beat_ack`, so each one works out where the clip should be and seeks
    /// itself; nothing is elected, and no screen has to wait for a round trip.
    ///
    /// `None` for content with no clip, and for anything built before this
    /// existed: `mediasync::syncSeek` corrects nothing without it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_started_at: Option<i64>,
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
pub fn output_url(channel_id: i64, template_id: Option<i64>, name: &str) -> String {
    // `channel` lets the output live-swap its template when the screen is
    // reassigned (it filters a channel-retemplate broadcast by this id); `template_id`
    // is the first render before any push. `channel=0` = a channel-less preview.
    //
    // A screen with NO template of its own says so BY SAYING NOTHING (DECISIONS
    // §70). `outputurl.js` was taught that and this was not, so the two doors into
    // the same page disagreed: `Copy URL` omitted the parameter for a follower
    // while both native paths wrote `template_id=1` through an `unwrap_or(1)`.
    // The projector on HDMI and the OBS source in the same room were configured
    // identically and behaved differently — the projector wore built-in 1 for the
    // whole service and nothing corrected it, because `channel://retemplate` only
    // fires when the assignment CHANGES. Taking the Option here is what makes the
    // two callers unable to re-introduce it one at a time.
    let tpl = match template_id {
        Some(id) => format!("&template_id={id}"),
        None => String::new(),
    };
    format!(
        "output.html?channel={}{}&name={}",
        channel_id,
        tpl,
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
    template_id: Option<i64>,
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
    /// What the SCREEN'S OWN CLOCK said about the gap before this beat.
    gap: BeatGap,
    /// WHERE THE CLIP IS, according to the screen that is playing it.
    ///
    /// `None` is "this screen said nothing about media", which is true of every
    /// screen showing a verse and of every screen that has not been taught to
    /// report. It is never defaulted to zero: a defaulted position reads as "the
    /// clip is at the start", which is a claim, and the operator would be told a
    /// clip had 4:12 left when nothing was playing at all.
    media: Option<MediaBeat>,
}

/// A screen's account of the clip it is playing, carried on the beat it already
/// sends.
///
/// **The console must not time a clip from its own copy.** Its programme pane
/// renders through the same component, so it has a second video element playing
/// the same file — and that one is not the wall. It buffers differently, it starts
/// at a different instant, and if the wall's copy stalls the console's carries
/// happily on. An operator reading "0:12 left" off the console while the
/// congregation's screen is frozen at 2:30 is rule 35 exactly: a status line that
/// cannot detect its own failure.
///
/// So the numbers come from the screen that is painting, on the beat that already
/// says whether it is painting at all, and when no screen reports the console says
/// it does not know rather than doing the arithmetic itself.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct MediaBeat {
    pub pos_ms: u64,
    pub dur_ms: u64,
    pub paused: bool,
}

impl MediaBeat {
    /// Read a screen's media report off a JSON beat.
    ///
    /// The same discipline as `BeatGap::from_json` and for the same reason: a
    /// number that is not a sane non-negative integer is dropped, and a report
    /// with no duration is dropped whole. A duration of zero is not a clip that
    /// takes no time, it is a screen that has not finished loading one, and
    /// "0:00 left" over a clip that has barely started is worse than saying
    /// nothing.
    /// The same reading, from the native window's command arguments rather than
    /// from a JSON frame. One rule, two transports — the shape `BeatGap::clamped`
    /// already takes, so a window and a browser source cannot come to different
    /// conclusions about the same clip.
    pub fn clamped(pos_ms: Option<u64>, dur_ms: Option<u64>, paused: Option<bool>) -> Option<Self> {
        let dur_ms = dur_ms.filter(|d| *d > 0 && *d <= GAP_CLAMP_MS)?;
        let pos_ms = pos_ms.filter(|p| *p <= GAP_CLAMP_MS)?.min(dur_ms);
        Some(MediaBeat {
            pos_ms,
            dur_ms,
            paused: paused.unwrap_or(false),
        })
    }

    fn from_json(v: &serde_json::Value) -> Option<Self> {
        let num = |k: &str| {
            v.get(k)
                .and_then(|n| n.as_u64())
                .filter(|n| *n <= GAP_CLAMP_MS)
        };
        let dur_ms = num("media_dur_ms").filter(|d| *d > 0)?;
        let pos_ms = num("media_pos_ms")?.min(dur_ms);
        Some(MediaBeat {
            pos_ms,
            dur_ms,
            paused: v
                .get("media_paused")
                .and_then(|b| b.as_bool())
                .unwrap_or(false),
        })
    }
}

/// THE FRAMES A LAGGING SCREEN IS HANDED AGAIN (RG-195, 2026-09-21). The three
/// that decide what a screen shows — the global retained frame, this screen's
/// own targeted frame if it has one, and which screens are down — in the order
/// hello sends them, so a screen that fell behind ends up where a screen that
/// just joined would. Configuration frames (roles, looks, shows, templates) are
/// not re-sent: they rarely change mid-service and the next change republishes.
pub fn resync_frames(
    last_screen: &Mutex<Option<String>>,
    last_screen_by_channel: &Mutex<HashMap<i64, String>>,
    screens_down: &Mutex<String>,
    channel: Option<i64>,
) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(f) = last_screen.lock().ok().and_then(|g| g.clone()) {
        out.push(f);
    }
    if let Some(ch) = channel {
        if let Some(f) = last_screen_by_channel
            .lock()
            .ok()
            .and_then(|m| m.get(&ch).cloned())
        {
            out.push(f);
        }
    }
    if let Ok(g) = screens_down.lock() {
        out.push(g.clone());
    }
    out
}

/// A screen's account of a picture or clip that did not load, off a JSON beat.
/// Bounded, because a page on the LAN must not be able to push a novel into the
/// desk's status row; absent when the beat says nothing, which is the honest
/// reading of a screen whose media loaded (or that shows none).
pub fn media_error_from_json(v: &serde_json::Value) -> Option<String> {
    const MAX: usize = 300;
    let s = v.get("media_error")?.as_str()?.trim();
    if s.is_empty() {
        return None;
    }
    Some(s.chars().take(MAX).collect())
}

/// The screen's account of its own silence, carried on the beat that ends it.
///
/// **This exists to answer RG-119, and it is the only thing that can.** A service
/// on 2026-09-06 recorded the main output lost and recovered three times, 19.3
/// minutes of an 85.5 minute service, and the record could not say which of two
/// opposite failures it was: the screen really stopped painting, or `OutputHealth`
/// lost a heartbeat it should have kept. Relay's side of the beat cannot tell them
/// apart, because both look identical from here — no beat arrived.
///
/// The page knows, and only the page knows:
///
/// * `since_ms` — its own `Date.now()` gap since its previous tick. Roughly one
///   interval means the page kept ticking and the beats were lost in transport
///   (Relay's fault). A gap the size of the whole outage means the page was not
///   running at all: the OS suspended or throttled it, which is a screen that
///   genuinely was not painting.
/// * `hidden_ms` — how much of that gap the page spent `document.hidden`. On macOS
///   an occluded window is hidden, so this separates "covered by another window"
///   from "alive but silent", and those want opposite fixes.
///
/// **Both are ABSENT rather than zero when the page did not say** (`latency.rs`
/// learned that distinction the hard way), and both arrive over an unauthenticated
/// LAN socket, so they are clamped at the door and dropped if they are not
/// non-negative integers. A number here can only ever be evidence in a timeline
/// entry; nothing routes, gates or fires on it.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct BeatGap {
    pub since_ms: Option<u64>,
    pub hidden_ms: Option<u64>,
}

/// A day. Anything longer is a broken clock or a hostile client, and either way it
/// is not evidence about a service.
const GAP_CLAMP_MS: u64 = 24 * 60 * 60 * 1000;

impl BeatGap {
    /// Read the two numbers off a JSON beat. Anything that is not a non-negative
    /// integer within `GAP_CLAMP_MS` is dropped to `None` — an absent number reads
    /// as "the screen did not say", which is true, where a defaulted zero would
    /// read as "the screen said it never went quiet", which is a lie in the one
    /// record this field exists to make trustworthy.
    fn from_json(v: &serde_json::Value) -> Self {
        let field = |k: &str| {
            v.get(k)
                .and_then(|n| n.as_u64())
                .filter(|ms| *ms <= GAP_CLAMP_MS)
        };
        BeatGap {
            since_ms: field("since_ms"),
            hidden_ms: field("hidden_ms"),
        }
    }

    /// Same rule for the Tauri bridge, where the value arrives already typed.
    pub fn clamped(since_ms: Option<u64>, hidden_ms: Option<u64>) -> Self {
        BeatGap {
            since_ms: since_ms.filter(|ms| *ms <= GAP_CLAMP_MS),
            hidden_ms: hidden_ms.filter(|ms| *ms <= GAP_CLAMP_MS),
        }
    }

    /// One phrase for a timeline entry, or `None` when the screen said nothing.
    /// Content-free by construction: two durations and no text from anywhere.
    pub fn describe(&self) -> Option<String> {
        let since = self.since_ms?;
        let secs = |ms: u64| (ms as f64 / 1000.0).round() as u64;
        Some(match self.hidden_ms {
            Some(h) if h > 0 => format!(
                "screen's own clock: silent {}s, hidden {}s",
                secs(since),
                secs(h)
            ),
            _ => format!("screen's own clock: silent {}s, never hidden", secs(since)),
        })
    }
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
    /// WHAT A SCREEN SAID ABOUT A PICTURE OR CLIP IT COULD NOT LOAD (2026-09-21,
    /// O-4 / M-3). A 404, a codec the webview cannot decode and a CSP refusal
    /// were one observable event — nothing — while the beat still said `content`
    /// and the desk printed On Air in amber over a blank frame. The page now says
    /// so on the beat it already sends, and a beat that says nothing clears it.
    media_errors: Arc<Mutex<HashMap<i64, String>>>,
    /// HOW OFTEN A SCREEN FELL BEHIND AND WAS RE-SYNCED (RG-195, rule 33). A kiosk
    /// client that lagged past the broadcast buffer used to skip frames in
    /// silence — a `clear` among them was a panic control that never landed on
    /// that screen. Counted, and shown, like every other shed on the path.
    resyncs: Arc<Mutex<HashMap<i64, u32>>>,
    /// What was last REPORTED about each channel, so an edge can be detected and
    /// written to the service timeline exactly once.
    ///
    /// Separate from the beat itself because they answer different questions:
    /// `beats` is "what did the screen last say", this is "what have we already
    /// told the operator". Folding them together would log a screen as lost on
    /// every poll for as long as it stayed lost, which is how a timeline becomes
    /// something nobody reads.
    reported: Arc<Mutex<HashMap<i64, bool>>>,
    /// When each channel was first seen ATTACHED but not yet answering, so a page
    /// that is still loading is not written into the service record as a fault.
    /// Cleared the moment the channel detaches, so a window reopened later starts
    /// its grace again rather than inheriting one from an hour ago.
    first_seen: Arc<Mutex<HashMap<i64, std::time::Instant>>>,
}

impl OutputHealth {
    /// Record that the screen for `channel_id` is alive and painting `state`.
    /// A lock poisoned by a panicking reader must not take the wall's status with
    /// it: a lost beat degrades to "silent", which is the safe direction.
    pub fn beat(
        &self,
        channel_id: i64,
        state: PaintState,
        transport: &'static str,
        gap: BeatGap,
        media: Option<MediaBeat>,
    ) {
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
                    gap,
                    media,
                },
            );
        }
    }

    /// What the screen's own clock said about the silence before its last beat.
    /// `None` for a channel that has never beaten, which is an absence and not a
    /// zero gap. See `BeatGap`, and RG-119 for why it is recorded at all.
    pub fn last_gap(&self, channel_id: i64) -> Option<BeatGap> {
        let m = self.beats.lock().ok()?;
        Some(m.get(&channel_id)?.gap)
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
    /// WHERE THIS SCREEN SAYS ITS CLIP IS, or `None`.
    ///
    /// Only from a beat that is still fresh. A stale beat's media report is the
    /// same lie as a stale paint state: an operator would be shown a clip counting
    /// down on a screen that stopped answering a minute ago, and the countdown is
    /// the one thing they are timing the next cue against.
    /// A screen lagged and was handed the retained frames again. Counted per
    /// channel; never reset for the life of the process, because a number that
    /// goes back to zero hides the service it happened in.
    pub fn note_resync(&self, channel_id: i64) {
        if channel_id <= 0 {
            return;
        }
        if let Ok(mut m) = self.resyncs.lock() {
            *m.entry(channel_id).or_insert(0) += 1;
        }
    }

    pub fn resyncs_of(&self, channel_id: i64) -> u32 {
        self.resyncs
            .lock()
            .ok()
            .and_then(|m| m.get(&channel_id).copied())
            .unwrap_or(0)
    }

    /// Record, or clear, the media failure a screen reported on its latest beat.
    pub fn note_media_error(&self, channel_id: i64, error: Option<String>) {
        if channel_id <= 0 {
            return;
        }
        if let Ok(mut m) = self.media_errors.lock() {
            match error {
                Some(e) => {
                    m.insert(channel_id, e);
                }
                None => {
                    m.remove(&channel_id);
                }
            }
        }
    }

    /// The media failure a still-answering screen last reported, if any. A stale
    /// screen's report is dropped with the rest of its beat: "not responding" is
    /// the truer thing to say about it.
    pub fn media_error_of(&self, channel_id: i64) -> Option<String> {
        if !self.painting(channel_id) {
            return None;
        }
        self.media_errors.lock().ok()?.get(&channel_id).cloned()
    }

    pub fn media_of(&self, channel_id: i64) -> Option<MediaBeat> {
        let m = self.beats.lock().ok()?;
        let b = m.get(&channel_id)?;
        if b.at.elapsed().as_millis() as u64 > BEAT_STALE_MS {
            return None;
        }
        b.media
    }

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

    /// True once this channel has answered at least once in this run.
    fn ever_beaten(&self, channel_id: i64) -> bool {
        self.beats
            .lock()
            .map(|m| m.contains_key(&channel_id))
            .unwrap_or(false)
    }

    /// Has this channel's answering state CHANGED since the last time anyone
    /// looked? Returns the new value on an edge, `None` otherwise.
    ///
    /// The status poll is what notices — there is no other regular tick on this
    /// path, and adding a timer to watch screens that are already being watched
    /// twice a second would be a second answer to one question. That means this
    /// mutates from inside what reads like a query, which is worth stating plainly
    /// rather than discovering: `channel_status` is the edge detector.
    pub fn transition(&self, channel_id: i64) -> Option<bool> {
        // Read the beat side first and finish with it. Nothing in this type holds
        // two of these locks at once, and this is the one place that would be
        // tempted to.
        let painting = self.painting(channel_id);
        let ever = self.ever_beaten(channel_id);

        // ── A SCREEN THAT HAS NEVER ANSWERED YET IS NOT A FAULT YET ──
        //
        // A window is attached the instant it is created, and its page has to load
        // before it can beat. Without this, every service opened with
        // `output_lost` followed by `output_recovered` a fraction of a second
        // later: 2026-09-06 recorded that pair at 4.9 s and 5.0 s, and the service
        // before it at 1457.7 s and 1459.7 s. Both are in the permanent record,
        // both render in History as "Screen stopped responding", and the Sunday
        // report counts them. A fault that appears every single time is one an
        // operator learns to scroll past, which costs exactly the real one.
        //
        // The grace is bounded and it does NOT swallow the failure it looks like:
        // a page that never loads at all still reports lost, once
        // `BEAT_STALE_MS` has passed since it was first seen attached. The
        // difference is between "has not answered yet" and "is not answering".
        if !ever && !painting {
            let first = {
                let mut seen = self.first_seen.lock().ok()?;
                *seen
                    .entry(channel_id)
                    .or_insert_with(std::time::Instant::now)
            };
            if first.elapsed().as_millis() as u64 <= BEAT_STALE_MS {
                return None;
            }
        }

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

    /// Test-only: backdate the moment this channel was first seen attached, so the
    /// grace window inside `transition` has expired without a test sleeping through
    /// `BEAT_STALE_MS`. The unit tests in this file reach into `first_seen`
    /// directly; tests in other modules cannot, and needed the same thing.
    #[cfg(test)]
    pub(crate) fn expire_grace(&self, channel_id: i64) {
        if let Ok(mut seen) = self.first_seen.lock() {
            seen.insert(
                channel_id,
                std::time::Instant::now() - std::time::Duration::from_millis(BEAT_STALE_MS * 2),
            );
        }
    }

    /// Stop tracking a channel's edges — it is no longer attached, so neither
    /// "lost" nor "recovered" would mean anything about it.
    pub fn forget_transition(&self, channel_id: i64) {
        if let Ok(mut m) = self.reported.lock() {
            m.remove(&channel_id);
        }
        if let Ok(mut m) = self.first_seen.lock() {
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
/// ## The register is `REHEARSAL_VERDICTS`, not this paragraph
///
/// "One function" was the intent and never the fact, and the gap cost a leak:
/// `stage_next` shipped gated in name only, leaked "up next" to a live stage
/// tablet mid-rehearsal, and had no Tauri emit at all, so the e2e rehearsal
/// test — which counts wall events — saw nothing wrong. A prose list here was
/// exactly what failed to catch it, so the full, current list of publishers in
/// this module and their verdicts now lives in `REHEARSAL_VERDICTS`, in `mod
/// tests`, checked against the module's own source by
/// `every_publisher_in_this_module_has_an_explicit_rehearsal_verdict` — not
/// restated here, where it can drift the way it already once did.
///
/// `main.rs` used to publish a screen's template from outside this module, where
/// that scanner could not see it; it goes through `KioskHub::set_channel_template`
/// now, because the frame gained a retained slot and the publish and the retention
/// have to be one call. It is deliberately not gated either way: it carries a
/// template, not content. Reassigning a screen's look is live by design
/// (DECISIONS §29), puts no scripture anywhere, and suppressing it would leave a
/// kiosk rendering a template the operator has already replaced. The scanner reads
/// `main.rs` as well as this file, so a publisher moved between them keeps the
/// same verdict under the same name.
///
/// Anything added here that carries what a person would READ belongs in
/// `REHEARSAL_VERDICTS` as gated. Check `rehearsing(app)` first, and add an e2e
/// case that watches the KIOSK hub, not just the wall.
#[derive(Default)]
pub struct Rehearsal(pub AtomicBool);

/// **THE CONFIGURED COUNTDOWN WARNING WINDOW, AS MACHINE STATE** — `Settings →
/// General → Countdown warning`, in ms, with `0` meaning "never set, use the
/// shipped minute".
///
/// It is a mirror of one settings row, held here because the two publishers that
/// have to stamp it run on the path between a press and a projector, and a SQLite
/// lock is not a thing to take there for a number that changes twice a year. The
/// row in `app_settings` remains the truth; this is warmed from it at launch and
/// kept in step by `set_setting`, which is the one writer of that row (rule 36 —
/// the choke point is where the check goes).
///
/// Lock-free on purpose, for the same reason `Rehearsal` is: it is read inside
/// `publish_timers` and inside the one content door, and neither may ever block.
#[derive(Default)]
pub struct CountdownWarnDefault(pub std::sync::atomic::AtomicI64);

impl CountdownWarnDefault {
    /// The figure in force, or None when nothing is configured. Zero and anything
    /// negative read as ABSENT rather than as a window of zero — a warning colour
    /// that never comes on is the failure this whole chain exists to prevent, and
    /// the far side makes the identical judgement in `setCountdownWarnDefault`.
    pub fn get(&self) -> Option<i64> {
        let n = self.0.load(Ordering::Relaxed);
        (n > 0).then_some(n)
    }
    pub fn set(&self, ms: Option<i64>) {
        self.0
            .store(ms.filter(|n| *n > 0).unwrap_or(0), Ordering::Relaxed);
    }
}

/// WHAT THE CLIP ON THE SCREENS HAS BEEN ASKED TO DO.
///
/// Held here rather than derived from the retained frame, for the reason
/// `adjust_countdown` keeps its own state: the operator changes ONE of these at a
/// time. Pause must not un-loop, and Loop must not un-pause. A caller says nothing
/// about the fields it is not touching, which needs somewhere to read the others
/// back from.
///
/// It is reset when a clip is fired, and by the panic controls, in the same place
/// the retained frame is emptied — one door, so the state and the wire cannot come
/// to different conclusions about whether the clip is held.
#[derive(Default)]
pub struct MediaTransport {
    inner: Mutex<MediaTransportState>,
}

#[derive(Clone, Copy)]
struct MediaTransportState {
    paused: bool,
    looping: bool,
    /// Only ever goes up. See `media_transport_frame_json`: replay is an event,
    /// and an event expressed as a boolean cannot be sent twice.
    replay_epoch: u64,
    /// The same shape as `replay_epoch`, and counted APART from it (RG-221). One
    /// counter for both would swallow a scrub made immediately after a replay,
    /// which is exactly the pair of presses an operator makes when a clip started
    /// in the wrong place.
    seek_epoch: u64,
    /// Where the last scrub put it, in milliseconds.
    seek_ms: i64,
    /// THE ROOM'S LEVEL, 0.0–1.0, and deliberately NOT cleared by `reset`: an
    /// operator who turned a clip down for a quiet room did not mean "for this
    /// clip only", where `paused` and `looping` genuinely are per clip.
    volume: f64,
}

impl Default for MediaTransportState {
    /// Full volume, because a clip that arrives silent with nothing to say why is
    /// the worse of the two defaults. `f64::default()` is 0.0, which is why this
    /// is written out rather than derived.
    fn default() -> Self {
        Self {
            paused: false,
            looping: false,
            replay_epoch: 0,
            seek_epoch: 0,
            seek_ms: 0,
            volume: 1.0,
        }
    }
}

/// What every screen is told about the clip it is playing.
///
/// `Serialize` because the console is handed this same frame back by
/// `set_media_transport` (RG-260). It used to rebuild its own copy out of the
/// arguments it had passed in — `{paused, loop, volume}` — so its preview was
/// handed an object with no epochs at all and could act on neither a replay nor
/// a scrub. Two shapes for one instruction is two things that can disagree, and
/// they did, on the surface an operator watches to decide what the room is
/// seeing.
///
/// **It is still not an outcome.** A frame is the INSTRUCTION; the beat
/// (`MediaBeat`) is what says whether a screen obeyed, and Live reads the effect
/// from there. `serde(rename_all = "camelCase")` so the console receives the
/// field names its own player rule already reads.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportFrame {
    pub paused: bool,
    pub looping: bool,
    pub replay_epoch: u64,
    pub seek_epoch: u64,
    pub seek_ms: i64,
    pub volume: f64,
    /// THE BASELINE A SCRUB IMPLIES (RG-220, RG-221), or `None` when this frame
    /// carries no scrub. Without it the sync corrector would undo the operator's
    /// own drag within two seconds: they move the handle, the picture jumps back,
    /// and the app looks broken.
    pub started_at: Option<i64>,
}

impl MediaTransport {
    /// Apply what the caller actually said, leave the rest, and hand back the whole
    /// state to publish. `replay` bumps the counter.
    pub fn apply(
        &self,
        paused: Option<bool>,
        looping: Option<bool>,
        replay: bool,
        seek_ms: Option<i64>,
        volume: Option<f64>,
    ) -> TransportFrame {
        let mut g = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(p) = paused {
            g.paused = p;
        }
        if let Some(l) = looping {
            g.looping = l;
        }
        if let Some(v) = volume {
            // Clamped here as well as at the receiver. A value outside 0–1 throws
            // on a real media element, and a frame nobody can apply is worse than
            // a level slightly off what was asked for.
            g.volume = v.clamp(0.0, 1.0);
        }
        if replay {
            g.replay_epoch = g.replay_epoch.saturating_add(1);
            // STARTING IT AGAIN MEANS IT IS RUNNING. An operator who presses
            // Replay on a held clip means "play it from the top", not "seek to the
            // top and stay stopped" — and the second reading leaves a frozen first
            // frame on a wall with the transport saying it was actioned.
            g.paused = false;
        }
        let mut started_at = None;
        if let Some(at) = seek_ms {
            g.seek_epoch = g.seek_epoch.saturating_add(1);
            g.seek_ms = at.max(0);
            // AND THE SYNC BASELINE MOVES WITH IT. RG-220 pulls every screen onto
            // the instant Relay sent the clip; a scrub that did not restate that
            // instant would be corrected away within the tolerance, so the
            // operator's own drag would visibly undo itself.
            started_at = Some(now_epoch_ms() - g.seek_ms);
        }
        TransportFrame {
            paused: g.paused,
            looping: g.looping,
            replay_epoch: g.replay_epoch,
            seek_epoch: g.seek_epoch,
            seek_ms: g.seek_ms,
            volume: g.volume,
            started_at,
        }
    }

    /// Back to a clip that has just been fired: playing, not looping. The epoch is
    /// deliberately NOT reset — it is a monotonic instruction counter, and winding
    /// it back would let a later replay publish a number a screen has already seen.
    pub fn reset(&self) {
        let mut g = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        g.paused = false;
        g.looping = false;
        // The VOLUME survives, and that is the one difference between the three.
        // See the field's own note: the sound level is a fact about the room.
    }
}

/// The configured warning window, for a publisher that has an app handle. None
/// when nothing is configured OR when the state is not managed (headless tests,
/// early boot) — an absence, which the far side reads as "keep the shipped minute"
/// rather than as a figure.
pub fn countdown_warn_default<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<i64> {
    app.try_state::<CountdownWarnDefault>()
        .and_then(|s| s.get())
}

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
///
/// **Public for ONE reason beyond this module**: a timer records the mode it was
/// started in (`timers::Timer::started_in_rehearsal`, RG-150), and the two creators
/// live in `main.rs`. It is the same question the publishers here ask, read from the
/// same state, so a timer's stamp and a broadcast's suppression can never disagree
/// about the same instant. It is NOT a licence for a publisher outside this module:
/// `REHEARSAL_VERDICTS` can only see the ones in here.
pub fn rehearsing<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
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
pub(crate) fn kiosk_content_json(content: &OutputContent) -> String {
    serde_json::json!({
        "kind": "content",
        "content_kind": content.kind,
        // null = every screen. Receivers filter on it; the hub cannot address
        // one client (DECISIONS §35), so the routing is the receiver's, exactly
        // as `channel_template` and the Stage Message already are.
        "channels": content.channels,
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
        // BOTH halves of the countdown model, or a kiosk screen is the one surface
        // that disagrees with the wall — the exact bug `next_reference` caused here.
        // `countdown_from` is what makes the warning rule's short-countdown case
        // answerable; `countdown_paused_ms` is the difference between a held timer
        // and one that carries on counting on a browser source nobody is watching.
        "countdown_from": content.countdown_from,
        "countdown_paused_ms": content.countdown_paused_ms,
        "countdown_done": content.countdown_done,
        // WHEN TO WORRY, both facts, for the same reason the two above are here: a
        // kiosk screen missing one of them turns red at a different moment from the
        // native window beside it. `countdown_warn_ms` is the threshold chosen for
        // this countdown; `countdown_warn_default_ms` is the configured one, which
        // a page with no bridge has no other way of learning (RG-149).
        "countdown_warn_ms": content.countdown_warn_ms,
        "countdown_warn_default_ms": content.countdown_warn_default_ms,
        // Rides to every kiosk client purely so it can report back when it painted
        // — the last leg of the latency chain, over the real church network. See
        // `OutputContent::trace_id` and the `rendered` message the hub accepts.
        "trace_id": content.trace_id,
        // WHEN RELAY SENT THE CLIP (RG-220). Every screen corrects itself
        // against this and Relay's clock, which `beat_ack` already gives them.
        "media_started_at": content.media_started_at,
    })
    .to_string()
}

/// Is this frame one of the three that decide what a screen is SHOWING?
///
/// Deliberately a substring match on frames this module builds itself, not a JSON
/// parse: it runs inside `publish`, which is on the path between a fire and the
/// projector. (This said "prefix" directly above a body comment explaining why a
/// prefix check matched nothing — see below.) `template` and `stage_next` are excluded — the first is already
/// sent on hello and the second is a monitor-only extra that must not stand in
/// for the content it accompanies.
fn is_screen_frame(msg: &str) -> bool {
    // CONTAINS, not `starts_with`. `serde_json`'s default map is a BTreeMap, so
    // `kiosk_content_json` emits its keys in ALPHABETICAL order and a content frame
    // begins `{"content_kind":…` — a prefix check silently matched nothing and the
    // retained frame stayed empty, which looks exactly like the bug it fixes.
    // `"kind":"content"` cannot occur inside a JSON string value (the quotes would
    // be escaped), so this cannot be triggered by a verse.
    msg.contains(r#""kind":"content""#)
        || msg.contains(r#""kind":"clear""#)
        || msg.contains(r#""kind":"black""#)
}

/// Is this frame a panic control — the two that take a congregation screen back
/// TOTALLY rather than replacing what is on it?
///
/// Pulled out of `is_screen_frame` rather than spelled again, because the
/// background layer needs the same two names and a second copy of them is a
/// second list that can drift from the first. Same `contains` discipline and for
/// the same reason (`serde_json`'s map is a BTreeMap, so no key is first).
fn is_wipe_frame(msg: &str) -> bool {
    msg.contains(r#""kind":"clear""#) || msg.contains(r#""kind":"black""#)
}

// ─────────────────────────────────────────────────────────────────────────────
// THE BACKGROUND — a picture that outlives the words painted on top of it
// ─────────────────────────────────────────────────────────────────────────────

/// A PICTURE BEHIND EVERYTHING, WITH A LIFETIME OF ITS OWN.
///
/// Until this existed a verse and a picture were mutually exclusive payloads:
/// the entire layer stack renders inside `{#if content}` and `media_url` is a
/// field on `OutputContent`, so firing the church's backdrop REPLACED the
/// reading and firing the reading replaced the backdrop. Scripture over a
/// church's own background could not be expressed at all.
///
/// So it is a second payload kind rather than a field, and the difference is the
/// whole point: it is not what a screen is SHOWING, it is what the screen is
/// showing it ON. It survives every content change, it is retained in its own
/// hub slot and replayed to a screen that joins late (rule 43), and it is taken
/// down by the two controls that take everything down.
///
/// Deliberately NOT a template, a fit or an opacity. Where the picture sits, how
/// it is cropped and what is dimmed over it are the TEMPLATE's business — a
/// `backdrop` layer decides all three, per screen, which is what makes this
/// opt-in by template design and byte-identical on a template without one. A
/// second authority on how a background is drawn is the defect DECISIONS §69 and
/// §71 are the scars from.
#[derive(Clone, Debug, Serialize)]
pub struct Background {
    /// Where every screen loads it from — the app's own HTTP server on :8032, so
    /// a native window and a browser source in OBS fetch the identical bytes.
    pub media_url: String,
    /// `image` or `video`. The renderer needs to know which element to paint it
    /// with, and guessing from the extension is a second rule about a fact the
    /// database already holds.
    pub media_kind: String,
}

/// The wire form of the background, for every kiosk client.
///
/// `None` is how a background is TAKEN DOWN, and it is sent as an explicit null
/// rather than as an absent frame for the same reason `transition_json` does it:
/// an absent frame cannot say "there is none now", and a screen that missed the
/// take-down would carry the picture for the rest of the service.
///
/// Pure, like `kiosk_content_json`, `timer_frame_json` and `transition_json`, so
/// the frame can be asserted against without a Tauri app handle.
fn background_json(bg: Option<&Background>) -> String {
    serde_json::json!({
        "kind": "background",
        "media_url": bg.map(|b| b.media_url.as_str()),
        "media_kind": bg.map(|b| b.media_kind.as_str()),
    })
    .to_string()
}

/// Is this frame about the background at all?
///
/// True for BOTH the frame that puts one up and the frame that takes it down —
/// they are one message kind, because a screen has to act on both and a
/// take-down expressed as silence is not a message.
fn is_background_frame(msg: &str) -> bool {
    msg.contains(r#""kind":"background""#)
}

/// WHAT A PUBLISHED FRAME DOES TO THE RETAINED BACKGROUND.
///
/// Three answers, and the three-state return is the honest shape:
///
/// * `None` — this frame is not about the background. A verse, a clock, a
///   template, an alert: the backdrop stays exactly where it is, which is the
///   entire reason this payload exists.
/// * `Some(None)` — take it down. A panic control does this, and so does a
///   background frame that names no picture.
/// * `Some(Some(frame))` — this frame IS the background now.
///
/// **The panic arm is why this is decided here.** `publish` is the one door every
/// frame in this module goes through, so a `clear` or a `black` drops the
/// backdrop by construction — no publisher has to remember, and no SECOND frame
/// has to be sent to finish the job. A panic control that needed two frames is a
/// panic control that can half succeed, and rule 15 does not allow one.
///
/// The `"media_url":null` test is a substring match on a frame this module builds
/// itself, exactly like `is_screen_frame` and `is_timer_frame`, and it is pinned
/// against the real `background_json` output by
/// `the_background_retention_rule_agrees_with_what_is_published` — because a
/// matcher that quietly stops matching looks precisely like the bug it fixes.
fn background_retention(msg: &str) -> Option<Option<String>> {
    if is_wipe_frame(msg) {
        return Some(None);
    }
    if !is_background_frame(msg) {
        return None;
    }
    Some((!msg.contains(r#""media_url":null"#)).then(|| msg.to_string()))
}

/// SOMETHING FOR THE PREACHER TO LOOK AT, ON THE STAGE SCREEN ONLY.
///
/// An announcement slide, or the preacher's own deck, put where only they can see
/// it. `media_url: null` takes it down, the same shape `background` uses and for
/// the same reason: one door for up and down means the two cannot disagree about
/// which is in force.
///
/// **It is not a congregation background and must never be confused with one.**
/// `background` is the church's picture behind the words on every screen; this is
/// one person's reference material on one screen, and the only reason it is
/// retained at all is that a stage tablet reconnecting mid-sermon would otherwise
/// come back blank (rule 43).
fn stage_media_frame_json(url: Option<&str>, kind: Option<&str>) -> String {
    serde_json::json!({
        "kind": "stage_media",
        "media_url": url,
        "media_kind": kind,
        // WHEN IT STARTED, on Relay's clock, so the preacher's copy is corrected
        // against the same instant as the congregation's (RG-220). Without it
        // the stage countdown added in RG-213 is about a different moment from
        // the one everybody else is watching, which is the third of the three
        // things "all media in sync" could mean and the one the operator said
        // mattered most.
        //
        // `null` when the slide is being taken DOWN: there is nothing to be in
        // sync with, and a stamp there would be a fact about an absence.
        "started_at": url.map(|_| now_epoch_ms()),
    })
    .to_string()
}

/// Relay's own clock, in epoch milliseconds — the baseline a screen corrects
/// against. `0` before the UNIX epoch, which never happens, so no caller has to
/// handle an error.
fn now_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Is this the frame that decides what MEDIA a stage screen is holding?
///
/// `contains`, not `starts_with`, for the reason recorded on `is_screen_frame`:
/// `serde_json` orders map keys alphabetically, so a prefix check here would break
/// the moment a field sorting before `kind` is added.
fn is_stage_media_frame(msg: &str) -> bool {
    msg.contains(r#""kind":"stage_media""#)
}

/// Leave the stage media alone, take it down, or become it.
///
/// The same three answers as `background_retention`, and the take-down arm is here
/// for the same reason: `clear` and `black` are published through the one door, so
/// a panic control takes the preacher's slide with the wall by construction rather
/// than by a second message somebody has to remember to send.
///
/// **A READING DOES NOT APPEAR HERE, and that is the precedence rule.** Scripture
/// overrides stage media on the screen; it does not destroy it. The device paints
/// the reading over the media while it has one and paints the media again when the
/// reading is cleared. Taking the media down when a verse arrived would make the
/// operator push it again after every reading, which is not what "overrides" means.
fn stage_media_retention(msg: &str) -> Option<Option<String>> {
    if is_wipe_frame(msg) {
        return Some(None);
    }
    if !is_stage_media_frame(msg) {
        return None;
    }
    Some((!msg.contains(r#""media_url":null"#)).then(|| msg.to_string()))
}

/// WHAT THE OPERATOR HAS ASKED THE CLIP TO DO — held, looping, or started again.
///
/// **`replay_epoch` is a counter and not a flag**, and that is the whole of why
/// this works. "Start it again" is not a state a screen can be in; it is an event,
/// and an event expressed as a boolean cannot be sent twice. An operator pressing
/// Replay a second time on a clip already at its start would publish a frame
/// identical to the retained one, and a screen that had acted on the first would
/// do nothing at all. A number that only goes up is an instruction every time.
fn media_transport_frame_json(f: TransportFrame) -> String {
    serde_json::json!({
        "kind": "media_transport",
        "paused": f.paused,
        "loop": f.looping,
        "replay_epoch": f.replay_epoch,
        // A SCRUB IS AN EVENT, exactly as a replay is, and counted apart from it
        // (RG-221). `seek_ms` is where the handle was dropped; a screen acts on
        // an epoch it has not seen and ignores one it has, so a retained frame
        // cannot drag a screen that joins an hour later back to the start.
        "seek_epoch": f.seek_epoch,
        "seek_ms": f.seek_ms,
        // The room's level, 0.0–1.0.
        "volume": f.volume,
        // The baseline the scrub implies, when this frame carries one (RG-220).
        "started_at": f.started_at,
    })
    .to_string()
}

/// Is this the frame that says what the clip on the screens is doing?
///
/// `contains`, not `starts_with`, for the reason recorded on `is_screen_frame`.
fn is_media_transport_frame(msg: &str) -> bool {
    msg.contains(r#""kind":"media_transport""#)
}

/// Leave the transport alone, take it away, or become it.
///
/// Three answers again, and a fourth trigger the other retained frames do not
/// have: **a new CONTENT frame clears it**. A clip is started at its beginning and
/// playing, always, so a transport retained across a fire would hand the next
/// video a church put up already held, because somebody paused a different one
/// twenty minutes earlier. Nothing in the product would say why.
///
/// `clear` and `black` empty it too, with everything else they take.
fn media_transport_retention(msg: &str) -> Option<Option<String>> {
    if is_screen_frame(msg) {
        // Content, clear and black alike: the transport belongs to the clip that
        // was showing, and all three of those replace it.
        return Some(None);
    }
    if !is_media_transport_frame(msg) {
        return None;
    }
    Some(Some(msg.to_string()))
}

/// The frame a hub sends when every screen is following the wall.
pub(crate) const SCREENS_ALL_UP: &str = r#"{"kind":"screen_state","screens":{}}"#;

/// Does this frame say which screens the OPERATOR has taken out of the wall?
///
/// Disjoint from `is_screen_frame` and `is_timer_frame` by construction: a frame
/// is `content`/`clear`/`black`, or `timer`, or this, and no frame this module
/// builds is two of them. `contains`, not `starts_with`, for the reason recorded
/// on `is_screen_frame` — `serde_json` orders map keys alphabetically and a prefix
/// check there silently matched nothing while looking exactly right.
fn is_screen_state_frame(msg: &str) -> bool {
    msg.contains(r#""kind":"screen_state""#)
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

/// WHAT IS IN FRONT OF THE OPERATOR RIGHT NOW — the live content itself, whatever
/// kind it is, or None over a cleared or blacked wall.
///
/// **This used to be `CountdownState`, and the difference is the whole of wave 3.**
/// It held an `Option<OutputContent>` filtered down to countdowns, so the countdown
/// had no existence apart from being the live content: fire a verse, a song or a
/// notice and it was forgotten, `adjust_countdown` answered "Nothing is counting
/// down.", and there was no way back. That was never a decision anybody took — it
/// was a consequence of where the state lived. A timer's facts live in
/// `timers::TimerRegistry` now and have a lifetime of their own; what remains here
/// is the smaller, honest question this slot can actually answer: **is that timer
/// what the screens are showing at this moment?**
///
/// Reset and ±1 used to be assembled in the console out of `$live` — the label, the
/// done message and the template all read back off the event and handed to
/// `start_countdown` again. That works exactly as long as every caller remembers
/// every field, and a paused countdown adds one more thing to forget: a `+1` that
/// dropped `countdown_paused_ms` would quietly restart a held timer in front of a
/// congregation. The engine carries it instead, and the transport asks it to change
/// one thing about it.
///
/// Maintained at the SAME three doors as [`WallState`] — `broadcast_content`, `clear`
/// and `black` — so it cannot drift (rule 36), with one deliberate difference: it is
/// noted BEFORE the rehearsal branch, not after. `WallState` answers "what can a
/// congregation see", so a rehearsal must not touch it. This answers "what is the
/// operator looking at", and in a rehearsal that is the console's own copy — which
/// the console already mirrors, and on which ±1 works today. Noting it after the
/// branch would take the transport away in rehearsal, which is the one place an
/// operator is meant to be practising with it.
///
/// **Lock discipline:** innermost, and never held across an emit (rule 2). Every
/// reader clones and releases before it broadcasts.
#[derive(Default)]
pub struct LiveContent(pub std::sync::Mutex<Option<OutputContent>>);

/// The content currently in front of the operator, or None over a cleared wall.
/// A clone, taken under the lock and returned with the lock released.
pub fn live_content<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<OutputContent> {
    app.try_state::<LiveContent>()
        .and_then(|s| s.0.lock().ok().and_then(|g| g.clone()))
}

/// Record what is on the screens at one of the three doors.
///
/// **There is no filter here, and its absence is the fix.** The old version kept the
/// content only while it was a countdown, which made "is there a countdown" and "is
/// a countdown on the screens" the same question — so the answer to the first was
/// lost every time the answer to the second changed.
fn note_live_content<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    content: Option<&OutputContent>,
) {
    let Some(state) = app.try_state::<LiveContent>() else {
        return;
    };
    let next = content.cloned();
    if let Ok(mut g) = state.0.lock() {
        *g = next;
    };
}

/// A PANIC CONTROL TAKES EVERY CONGREGATION TIMER, AND ASKS NOTHING.
///
/// `clear` and `black` take back every congregation screen totally, and a timer
/// projected onto those screens goes with them — the guarantee DECISIONS §27 states,
/// unchanged. The split between congregation and programme timers is a property of
/// the timer (`timers::Scope`), never a question asked here: a panic control that
/// has to work out which screen it is talking to is a panic control that can fail to
/// answer, and rule 15 does not allow one of those.
///
/// A no-op when the registry is not managed, exactly like `note_wall`.
fn stop_congregation_timers<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(reg) = app.try_state::<crate::timers::TimerRegistry>() {
        // Takes and releases its own lock, so nothing is held across the emits
        // below (rule 2).
        reg.stop_scope(crate::timers::Scope::Both);
    }
}

pub fn broadcast_content<R: tauri::Runtime>(app: &tauri::AppHandle<R>, content: OutputContent) {
    let json = kiosk_content_json(&content);
    // BEFORE the rehearsal branch, deliberately — see `LiveContent`. The lock is
    // taken and released here, never held across the emit below (rule 2).
    note_live_content(app, Some(&content));
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
    // The content left the operator's screen either way, rehearsal or not — so it
    // is forgotten on both paths here, exactly as it is remembered on both above.
    note_live_content(app, None);
    stop_congregation_timers(app);
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
    note_live_content(app, None);
    stop_congregation_timers(app);
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

/// PUT A PICTURE BEHIND EVERYTHING — or take it away (`None`).
///
/// Both doors, because the wall is two kinds of screen: a native output window
/// with the Tauri bridge and no socket, and a kiosk/OBS browser source with the
/// socket and no bridge. A background wired to one of the two is the "guarantee
/// kept on one door" mistake this repository has now made four times, on the two
/// screens most often in the same room.
///
/// REHEARSAL-GATED, and not on a technicality: this is the one publisher here
/// besides `broadcast_content` that puts an IMAGE in front of a congregation.
/// The suppression mirrors `broadcast_content`'s exactly — the console still
/// receives it, so an operator rehearsing sees their own backdrop in the program
/// pane, and nothing leaves the machine. `stage_next` is the standing reminder of
/// what a publisher that shipped gated in name only costs.
///
/// **It deliberately does NOT touch `WallState`, `LiveContent` or the passage.**
/// Each of those answers a question about the CONTENT a congregation is reading,
/// and a backdrop is not content: it carries no reference and no words, so
/// `/api/live` naming it would be naming a verse that is not there, and disarming
/// the passage (rule 38) would make `next` answer `NoPassage` in the middle of a
/// reading the preacher is still in. A background does not REPLACE the reading,
/// which is the entire reason the payload exists.
///
/// Retention, and its removal by a panic control, both happen inside
/// `KioskHub::publish` — see `background_retention` for why that is the door and
/// not this function.
// GENERIC OVER THE RUNTIME (rule 24) — `e2e.rs` has to be able to drive it.
pub fn set_background<R: tauri::Runtime>(app: &tauri::AppHandle<R>, bg: Option<Background>) {
    if rehearsing(app) {
        // Content-free, like every other line on this path: what an operator (or a
        // bug report) needs to know is that nothing left the machine.
        println!("rehearsal: background SUPPRESSED — nothing left the machine");
        let _ = app.emit_to(CONSOLE, "output://background", bg);
        return;
    }
    let json = background_json(bg.as_ref());
    let _ = app.emit("output://background", bg);
    publish_kiosk(app, json);
}

/// ── PER-SCREEN CLEAR AND BLACKOUT ──────────────────────────────────────────
///
/// "Take the lobby TV down but leave the wall live" is an ordinary request and
/// was impossible: `clear` and `black` take no channel and never will.
///
/// **THE SPLIT IS IN THE CALL, NEVER INSIDE THE PANIC CONTROL.** `clear` and
/// `black` above are untouched, byte for byte. A panic control that has to work
/// out which screen it is addressing is a panic control that can fail to answer,
/// and rule 15 does not allow one of those — the same sentence
/// `stop_congregation_timers` already carries, for the same reason. What follows
/// is a SEPARATE control, reached from the Outputs desk, never from `Esc` or `B`.
///
/// **It is durable, not a one-shot frame.** A screen taken down stays down until
/// it is brought back, across every fire in between. A one-shot would be undone
/// by the next verse, which during a service is within a minute, which is to say
/// useless for the thing it is for. That durability is the risk as well: an
/// operator can forget. So the state is in MEMORY and not in the database — a
/// relaunch brings every screen back, because a screen nobody can see is a worse
/// thing to persist than a preference nobody set — and every surface that
/// describes a screen says which screens are down, through the one helper that is
/// allowed to turn a screen into words (`outputHealth.js::describeScreen`, rule
/// 35).
///
/// **The whole set, every frame.** Like the programme timers, and for the same
/// reason: a client that missed one delta would be wrong about itself for the
/// rest of the service with no way to find out. An empty map is how the last
/// screen comes back up.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ScreenState {
    /// Nothing of Relay's on this screen; the template background shows through,
    /// so a keyed channel still keys out for OBS.
    Clear,
    /// Opaque black on this screen alone.
    Black,
    /// The screen follows the wall again. Never stored — it is the absence.
    Live,
}

impl ScreenState {
    pub fn as_str(self) -> &'static str {
        match self {
            ScreenState::Clear => "clear",
            ScreenState::Black => "black",
            ScreenState::Live => "live",
        }
    }
    // NO `parse`, deliberately. Nothing on any door hands this a string: the three
    // commands each name one variant, so free text never becomes a state a screen
    // renders and there is nothing to parse it from. A parser nothing calls is a
    // parser nobody is checking.
}

/// The screens the operator has taken down on their own, and what they were told
/// to do. A channel that is live is ABSENT rather than present-and-`Live`: one
/// representation of one fact, so nothing can read "down" off an entry that says
/// it is up.
#[derive(Default)]
pub struct ScreensDown(pub std::sync::Mutex<HashMap<i64, ScreenState>>);

impl ScreensDown {
    /// What this screen has been told, or `None` when it follows the wall.
    pub fn get(&self, channel_id: i64) -> Option<ScreenState> {
        self.0.lock().ok().and_then(|m| m.get(&channel_id).copied())
    }

    /// The whole set as the wire form: `{"4":"clear"}`, and `{}` when every
    /// screen is following the wall.
    pub fn as_json(&self) -> String {
        let Ok(m) = self.0.lock() else {
            return "{}".into();
        };
        let map: serde_json::Map<String, serde_json::Value> = m
            .iter()
            .map(|(id, st)| (id.to_string(), serde_json::Value::from(st.as_str())))
            .collect();
        serde_json::Value::Object(map).to_string()
    }
}

/// TAKE ONE SCREEN DOWN, OR BRING IT BACK. The one place this is decided.
///
/// Rule 36: the choke point is where the check goes. Three commands call this and
/// none of them publishes anything itself, so a fourth way of taking a screen
/// down cannot arrive with its own idea of what that means.
///
/// **It does not touch anything the wall is made of.** Not `LiveContent`, not the
/// debounce, not the congregation timers, not `WallState`. Taking the lobby TV
/// down does not mean the verse has gone — it is still in front of the
/// congregation on every other screen — and a control that forgot the live
/// content would make the next spoken "next verse" answer `NoPassage` over a
/// verse people can see.
///
/// Returns Err when the Tauri door refuses, for the same reason `clear` does: a
/// control that cannot reach a screen must not report that it did.
pub fn set_screen_state<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel_id: i64,
    state: ScreenState,
) -> Result<(), String> {
    // ── REFUSED IN A REHEARSAL, AND THAT IS A DECISION ─────────────────────
    //
    // The gate is HERE rather than in the three commands, per rule 36: a fourth
    // way of taking a screen down must not be able to arrive with its own answer.
    //
    // Every other publisher in this module SUPPRESSES during a rehearsal and
    // returns success, because what it is suppressing is content and the
    // rehearsal's promise is that no content leaves. This one is refused instead.
    // Suppressing it would leave the operator's own belief and the screens
    // disagreeing with nothing to say so — the desk would show the lobby TV down
    // while the lobby TV showed the last thing it was sent — and that is rule 35's
    // shape on a control the operator pressed deliberately. It is not a panic
    // control, so it is allowed to refuse; `clear` and `black` are and are not,
    // which is why the split is in the call.
    if rehearsing(app) {
        return Err(
            "Relay is rehearsing, so no screen was changed. Leave rehearsal to take a screen down."
                .into(),
        );
    }
    let Some(down) = app.try_state::<ScreensDown>() else {
        // A headless Relay manages no registry. Saying so beats pretending a
        // screen was changed.
        return Err("this build has no screen registry".into());
    };
    {
        // Taken and released before any emit — rule 2, and this one is on the path
        // of a control an operator presses during a service.
        let Ok(mut m) = down.0.lock() else {
            return Err("the screen registry is unavailable".into());
        };
        match state {
            ScreenState::Live => m.remove(&channel_id),
            other => m.insert(channel_id, other),
        };
    }
    let blob = down.as_json();
    let json = format!(r#"{{"kind":"screen_state","screens":{blob}}}"#);
    app.emit(
        "output://screen_state",
        serde_json::json!({ "channel": channel_id, "state": state.as_str() }),
    )
    .map_err(|e| e.to_string())?;
    publish_kiosk(app, json);
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

/// A WORD TO THE PREACHER — the whole stage screen, and no other screen at all.
///
/// `text: None` clears it. Deliberately NOT retained by the hub: rule 43 retains
/// the frames that decide what a screen is SHOWING (`content`, `clear`, `black`),
/// and an alert is an instruction to a person rather than a state of the wall. A
/// tablet reconnecting ten minutes later must not be handed a message meant for a
/// moment that has passed.
///
/// Suppressed in a rehearsal, like every other publisher here — see `stage_next`
/// for what that cost the one time it was missed.
pub fn stage_alert<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    text: Option<String>,
    urgent: bool,
) {
    if rehearsing(app) {
        println!("rehearsal: stage_alert SUPPRESSED — nothing left the machine");
        return;
    }
    // ── QUIET OR URGENT, AND THE DIFFERENCE IS THE WHOLE POINT ───────────────
    //
    // There was one rendering: a full-bleed flashing red panel. So "wrap up in
    // five" arrived as the same emergency as "stop, there is a medical
    // incident", and an alarm spent on ordinary business stops being an alarm.
    // `urgent` is false for a note, which lands as fixed text where the
    // template puts it, and true for the panel (DECISIONS §116).
    let json =
        serde_json::json!({ "kind": "stage_alert", "text": &text, "urgent": urgent }).to_string();
    publish_kiosk(app, json);
    // ── THE SECOND DOOR, AND WHY IT IS NOT A WIDENING (RG-156) ───────────────
    //
    // This published to the kiosk hub and nothing else, so a screen wired as a
    // `native_window` and given the `stage` role received NOTHING. The seeded
    // `Stage display` is a `network_client`, so it took a church configuring a
    // confidence monitor on HDMI to meet it — and then the failure is silence:
    // the console reports a Stage Message sent, and the preacher is never told
    // something the operator believes they have been told. That is rule 35 from
    // the engine end rather than the badge end.
    //
    // **The guarantee does not move.** It never rested on this door being shut —
    // it rests on `channelroles::acceptsStageMessage`, which `Output.svelte` asks
    // before painting, at the kiosk door and now at this one, from ONE function.
    // It could not rest on the door: the hub cannot address a client either
    // (DECISIONS §35), so every congregation browser source has always been sent
    // this frame and has always refused it. A Tauri emit reaches every webview on
    // exactly the same terms.
    //
    // Nothing a congregation renderer binds rides on it: the payload is the text
    // and nothing else, `OutputContent` has no stage-message field, and
    // `e2e::r5_a_word_to_the_preacher_reaches_no_congregation_channel` asserts
    // both of those about both doors.
    let _ = app.emit(
        "output://stage_alert",
        serde_json::json!({ "text": text, "urgent": urgent }),
    );
}

/// PUT SOMETHING ON THE PREACHER'S SCREEN, or take it off (`None`).
///
/// Suppressed in a rehearsal like every other publisher here: a rehearsal reaches
/// no screen, and a slide appearing on a platform during one is the exact defect
/// `stage_next` shipped with.
pub fn stage_media<R: tauri::Runtime>(app: &tauri::AppHandle<R>, media: Option<(String, String)>) {
    if rehearsing(app) {
        println!("rehearsal: stage_media SUPPRESSED — nothing left the machine");
        return;
    }
    let json = match &media {
        Some((url, kind)) => stage_media_frame_json(Some(url), Some(kind)),
        None => stage_media_frame_json(None, None),
    };
    publish_kiosk(app, json);
}

/// TELL THE SCREENS WHAT TO DO WITH THE CLIP THEY ARE ALREADY PLAYING.
///
/// Not rehearsal-gated, and deliberately rather than by omission. Every other
/// publisher here PUTS something in front of somebody; this one changes what is
/// already there. In a rehearsal nothing is on a real screen to change, so the
/// frame reaches nobody — and gating it would only mean an operator rehearsing the
/// transport found the buttons dead with nothing to say why.
pub fn media_transport<R: tauri::Runtime>(app: &tauri::AppHandle<R>, frame: TransportFrame) {
    publish_kiosk(app, media_transport_frame_json(frame));
}

/// THE WIRE FORM OF THE PROGRAMME TIMERS — the whole stage-visible set, every time.
///
/// **A set, not a delta.** A tablet that missed one frame would otherwise be wrong
/// about the programme for the rest of the service, and there is no way for it to
/// find that out. Sending the set whole also makes the empty case expressible:
/// stopping the last timer publishes `timers: []`, which is how a clock comes OFF a
/// preacher's screen. An absent frame cannot say "there are none now".
///
/// **Why these key names.** `countdown.js::countdownRemainingMs` is the ONE reader
/// of how long is left on the frontend, and it reads `countdown_paused_ms` ahead of
/// `countdown_to`. Naming the fields anything else would mean the stage page doing
/// its own subtraction — a second copy of a rule that now has an exception
/// (docs/REBRAND.md §7), on the surface a preacher reads from mid-sermon.
///
/// Deliberately NOT `timers::project_both`: that is the congregation wire form and
/// carries a `reference` rather than a `label`, and no `id`. A stage entry is a row
/// in a rail and has to be identifiable; a congregation entry is a slide.
///
/// Pure, so the frame can be asserted against without a Tauri app handle — the same
/// reason `kiosk_content_json` and `transition_json` are pure.
fn timer_frame_json(timers: &[crate::timers::Timer], warn_default_ms: Option<i64>) -> String {
    let rows: Vec<serde_json::Value> = timers
        .iter()
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "label": t.label,
                "countdown_to": t.target_ms,
                "countdown_from": t.from_ms,
                "countdown_paused_ms": t.paused_ms,
                "countdown_done": t.done_msg,
                "warn_ms": t.warn_ms,
            })
        })
        .collect();
    // `warn_default_ms` sits BESIDE the rows, not on them. It is a fact about the
    // machine — `Settings → General → Countdown warning` — and this page cannot read
    // a setting, so it has to be delivered (RG-149). A copy per row would be a copy
    // that can disagree with itself inside one frame. A row's own `warn_ms` is the
    // separate fact: the threshold somebody chose for that timer, which beats this.
    serde_json::json!({ "kind": "timer", "timers": rows, "warn_default_ms": warn_default_ms })
        .to_string()
}

/// Is this the frame that decides what PROGRAMME TIMERS a stage tablet is showing?
///
/// A `contains`, and for exactly the reason `is_screen_frame` is one: `serde_json`'s
/// default map is a BTreeMap, so the keys come out in alphabetical order and not the
/// order anybody wrote. A `starts_with` would happen to work here (`kind` sorts
/// before `timers`) and would break the moment a field sorting before `kind` is
/// added — which is how the first version of `is_screen_frame` matched nothing while
/// looking exactly like the bug it fixed.
///
/// `"kind":"timer"` cannot occur inside a JSON string value — an operator's label
/// would have its quotes escaped — so a label cannot forge one.
fn is_timer_frame(msg: &str) -> bool {
    msg.contains(r#""kind":"timer""#)
}
/// WHICH SCREENS A FRAME NAMES, or `None` for every screen (RG-161).
///
/// Reads the wire rather than the struct, because retention happens at
/// `publish`, which is handed a finished string — the same reason
/// `is_screen_frame` parses rather than matching on a type.
///
/// `None` covers three cases that must behave identically: the key is absent
/// (every frame built before this existed), it is `null` (a cue that names no
/// screens), and it is not an array at all (a malformed frame, where reaching
/// every screen is the safe direction because the alternative is content that
/// silently reaches none).
///
/// An EMPTY array is `Some(vec![])` and is NOT `None`: a cue that reaches no
/// screen is a real thing to ask for, and turning it into "every screen" would
/// be the worst possible reading of it.
fn frame_channels(msg: &str) -> Option<Vec<i64>> {
    let v: serde_json::Value = serde_json::from_str(msg).ok()?;
    let arr = v.get("channels")?.as_array()?;
    Some(arr.iter().filter_map(|n| n.as_i64()).collect())
}

/// WHICH LAYOUT EACH STAGE SCREEN WEARS — `{"2":{"reading":true,…}}`.
///
/// The frame lives HERE rather than beside its one caller in `main.rs`, and
/// that is not tidiness. `every_kind_this_module_publishes_has_an_explicit_verdict`
/// and `r6-contracts.test.js` both find published kinds by reading THIS
/// module's source for `"kind":"…"` literals, so a frame published from
/// anywhere else is invisible to both — it would ship with no retention verdict
/// and no per-client verdict, which is the enumeration failing silently rather
/// than catching anything.
///
/// A screen with no layout is OMITTED by the query that builds `blob`, never
/// sent an empty object: absent means "use the device's own zones" and empty
/// would mean "show nothing", and those are a working screen and a blank one.
pub fn stage_zones_frame(blob: &str) -> String {
    format!(r#"{{"kind":"stage_zones","zones":{blob}}}"#)
}

/// THE PROGRAMME TIMERS, TO THE STAGE TABLET AND NOWHERE ELSE.
///
/// A `Stage` timer publishes no content frame at all — that is precisely why it
/// survives a verse, a song and a notice: nothing about it rides on the live
/// content, so replacing the live content cannot forget it. It needs a door of its
/// own, and this is it.
///
/// REHEARSAL-GATED, like every publisher here that carries something a person
/// READS. `stage_next` is the standing reminder of what that costs when it is
/// missed: it shipped gated in name only, leaked to a live stage tablet mid-
/// rehearsal, and had no Tauri emit, so the e2e rehearsal test — which counts wall
/// events — saw nothing wrong. A programme clock is the same shape of leak on the
/// same screen, so its test watches the HUB (`qa::Kiosk`), not the wall.
///
/// The registry is read and DROPPED before anything is published (rule 2):
/// `snapshot_scope` returns owned values and there is no way to still be holding
/// its lock on the far side of this call.
// GENERIC OVER THE RUNTIME (rule 24) — `e2e.rs` has to be able to drive it.
pub fn publish_timers<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if rehearsing(app) {
        // A suppression, not a redirect: there is no console stage panel to preview
        // it on, so the stage monitor keeps showing whatever it was showing —
        // exactly as the projector does. Same wording as `stage_next` on purpose.
        println!("rehearsal: publish_timers SUPPRESSED — nothing left the machine");
        return;
    }
    let Some(reg) = app.try_state::<crate::timers::TimerRegistry>() else {
        // No registry managed (headless tests, early boot). There is nothing to say
        // about timers, and saying "none" would be a claim from an absence.
        return;
    };
    let stage = reg.snapshot_scope(crate::timers::Scope::Stage);
    // The configured warning window rides with the set. This is the frame that
    // reaches a stage tablet FIRST — a programme timer can be running before
    // anything has been fired — so it is the only delivery that does not leave the
    // preacher's page on the shipped minute for the start of a service (RG-149(c)).
    let warn_default = countdown_warn_default(app);
    publish_kiosk(app, timer_frame_json(&stage, warn_default));
}

/// WHAT AN OVERRIDE IS, once: a mode and an optional duration, or nothing at all.
///
/// Named rather than spelled out at six signatures — clippy asks for this, and it
/// is also the honest shape: `None` here means "follow the template", which is a
/// third state and not an empty string.
pub type TransitionOverride = Option<(String, Option<u32>)>;
/// The hub's retained slot for it, shared with the WS task that answers `hello`.
pub type TransitionSlot = Arc<Mutex<TransitionOverride>>;

/// The wire form of the operator's transition override. `None` means "follow the
/// template" and is sent as an explicit null rather than omitted, so a screen can
/// tell a cleared override from a message it did not understand.
fn transition_json(t: Option<&(String, Option<u32>)>) -> String {
    let (mode, ms) = match t {
        Some((m, ms)) => (Some(m.as_str()), *ms),
        None => (None, None),
    };
    serde_json::json!({ "kind": "transition", "mode": mode, "ms": ms }).to_string()
}

/// HOW THE NEXT THING APPEARS — every screen, at once (docs/REBRAND.md §8,
/// DECISIONS §84).
///
/// Both doors, because the wall is two kinds of screen: a native output window
/// (Tauri event) and a kiosk/OBS browser source (the WS hub, which has no backend
/// at all). A control wired to one of the two is the "guarantee kept on one door"
/// mistake this repository has now made four times.
///
/// NOT REHEARSAL-GATED, and that is a deliberate difference from every publisher
/// above it. Those carry CONTENT; this carries configuration and paints nothing —
/// exactly like `set_template`, which is not gated either. A
/// screen that receives this looks identical afterwards; it only changes how the
/// NEXT change to it is drawn. So there is nothing of a rehearsal to leak, and
/// gating it would instead leave every screen still armed with the transition from
/// before the rehearsal once the operator went live.
///
/// It touches neither `WallState` nor `last_screen`, so it can neither report nor
/// alter what is on the screens — and the panic controls do not consult it at all.
pub fn transition<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    mode: Option<String>,
    ms: Option<u32>,
) {
    let mode = mode.filter(|m| !m.is_empty() && m.len() <= 32);
    let payload = serde_json::json!({ "mode": mode.clone(), "ms": ms });
    let _ = app.emit("output://transition", payload);
    if let Some(hub) = app.try_state::<KioskHub>() {
        hub.set_transition(mode, ms);
    }
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
    /// THE CONFIGURED DEFAULT TEMPLATE, as JSON, or the literal `null`.
    ///
    /// The last link in every screen's resolution chain (DECISIONS §29: the
    /// transparency law, then a pinned cue template, then the screen's own, then
    /// the content look, then THIS). A browser source has no database, so the
    /// hub is the only way it can learn the operator's default; without it the
    /// output page ends at the bundled `Classic Serif` and the configured
    /// default reaches a screen exactly once, when the channel is created.
    default_tpl: Arc<Mutex<String>>,
    /// THE LAST FRAME THAT DECIDED WHAT IS ON THE SCREENS — content, clear or
    /// black — kept so a client that joins LATE is shown it.
    ///
    /// Without this a browser source that reconnects mid-service comes back
    /// BLANK and stays blank until the operator happens to fire the next thing.
    /// That is not a rare event: an OBS source restarting, a kiosk page
    /// reloading, a Wi-Fi blip on the lobby TV, or this hub's own 1.5 s reconnect
    /// loop all produce it, and RG-119 records the main output going away three
    /// times in one 85.5 minute service. Reproduced by opening `output.html`
    /// while a verse was live: the page connected, was sent its template, and
    /// painted nothing.
    ///
    /// It retains the last frame of those three kinds and NOTHING else, so it can
    /// never resurrect a screen the operator cleared: `clear` and `black` are
    /// themselves published here and become the retained frame in their turn. A
    /// rehearsal publishes nothing to this hub at all (the gate is at the
    /// publishers), so nothing a rehearsal did can be replayed either.
    last_screen: Arc<Mutex<Option<String>>>,
    /// WHAT IS ON ONE SCREEN, when that screen was told something the others
    /// were not (RG-161).
    ///
    /// Rule 43 says a screen joining mid-service is shown what is on the
    /// screens — and the moment a cue can name screens, *the screens* stops
    /// being one answer. `last_screen` alone would replay a targeted frame to
    /// every late joiner, which is the targeting failing in exactly the way it
    /// was added to prevent.
    ///
    /// The two slots are kept consistent by one rule, in `publish`: an
    /// UNTARGETED frame supersedes everything, so it writes `last_screen` and
    /// EMPTIES this map. A targeted frame writes only the channels it names.
    /// So a present entry is always newer than `last_screen`, and `hello` needs
    /// no timestamps to decide between them — it prefers this map and falls
    /// back to the global slot.
    ///
    /// `clear` and `black` are untargeted by construction (the panic controls
    /// never carry a channel set), so they empty this map too and a cleared
    /// wall stays cleared for every screen that joins after it.
    last_screen_by_channel: Arc<Mutex<HashMap<i64, String>>>,
    /// THE OPERATOR'S LIVE TRANSITION OVERRIDE — `(mode, ms)`, or `None` to follow
    /// each template's own choice (DECISIONS §84).
    ///
    /// ITS OWN SLOT, NOT `last_screen`. Putting it there would have been the bug
    /// rule 43 exists to fix, wearing the fix's clothes: one slot means the newest
    /// frame wins, so an operator changing the transition would have REPLACED the
    /// retained verse, and the next screen to join would have been sent a
    /// preference and no content.
    ///
    /// Retained because a screen that joins mid-service must not be the one screen
    /// still cutting while the rest crossfade — the same argument as the template,
    /// which is cached and replayed here for the same reason. It is
    /// configuration, never content: it paints nothing on its own.
    last_transition: TransitionSlot,
    /// WHAT EACH SCREEN IS FOR — `{"1":"main","2":"stage"}`, or `{}`.
    ///
    /// A browser source has no database, so this is the only way it can learn
    /// its own channel's role — and it needs to, because a Stage Message is
    /// filtered at the RECEIVER. The hub broadcasts `stage_alert` to every
    /// client and cannot address one: it records nothing about who connected,
    /// and DECISIONS §35 is not being reversed to let it. So the message is
    /// refused where the refusal is possible, and the fact it is refused on has
    /// to reach the page.
    ///
    /// Its own slot, for the same reason `last_transition` has one: `last_screen`
    /// holds ONE frame and the newest wins, so retaining configuration there
    /// would replace the verse a late-joining screen is owed (rule 43).
    ///
    /// Ids and roles only — no names, no addresses, nothing a client chose.
    channel_roles: Arc<Mutex<String>>,
    /// WHAT EACH SCREEN WEARS FOR EACH KIND OF CONTENT — `{"1":{"scripture":9}}`
    /// (DECISIONS §97).
    ///
    /// **CONFIGURATION, COPIED FROM THE ROLE MAP ABOVE IN EVERY RESPECT THAT
    /// MATTERS**, because the role map is the pattern in this hub for "a small map
    /// of ids that every client filters for itself", and a second pattern for the
    /// same shape is how two maps come to be replayed at two different points in
    /// one hello reply.
    ///
    /// Its own slot, for the fourth time in this struct: `last_screen` holds ONE
    /// frame and the newest wins, so retaining a look map there would replace the
    /// verse a late-joining screen is owed (rule 43).
    ///
    /// IDS ONLY, never JSON. A per-kind look reaches a screen as a template id and
    /// the bytes ride separately, exactly as a content look does — the 13 MB
    /// reason is recorded at `main::cue_or_content_tpl` and this map does not
    /// weaken it. `look_ids` below is what gets the bytes to the receiver first.
    ///
    /// Channel ids against kind → template id and nothing else: no names, no
    /// addresses, nothing about who is connected. A screen learning what ANOTHER
    /// screen wears tells it nothing it could not read off the Outputs desk, which
    /// is the same argument `channel_roles_json` makes and the same reason
    /// DECISIONS §35 is untouched by it.
    channel_looks: Arc<Mutex<String>>,
    /// THE TEMPLATE EACH SCREEN WEARS FOR EVERYTHING ELSE — one retained
    /// `channel_template` frame per channel id.
    ///
    /// **THIS SLOT IS RULE 43'S HOLE, ONE FACT OVER.** The retained SCREEN frame
    /// answers "what is on the screens"; nothing answered "what does this screen
    /// LOOK like", and the hello reply's template branch only ever ran for a
    /// client that named a `template_id`. A channel-keyed browser source — the URL
    /// `Copy URL` produces, the one CLAUDE.md tells operators to use, and the only
    /// shape that follows a template swap — sends `template_id: null`, so it got
    /// nothing, resolved against a null channel template, and painted the content
    /// look or the configured default no matter what the operator had assigned.
    /// Every screen in the building wearing one template, measured on a real
    /// install, and it looked fine in testing because a screen connected at the
    /// moment of a `set_channel_template` broadcast does get it.
    ///
    /// **A MAP OF FRAMES, AND ONE FRAME IS SENT.** `channel_roles` and
    /// `channel_looks` go out whole because they are ids; this is BYTES — a
    /// template carrying an embedded `data:` image has been 13 MB
    /// (`main::cue_or_content_tpl`) — so a client is sent its own channel's frame
    /// and no other. The map is small (one string per configured screen, four on a
    /// seeded install) and it is the hub's, not a client's: no client can ask for
    /// another channel's look by naming it, because the lookup key is the channel
    /// that client said it was, and that is a number the operator configured.
    ///
    /// Frames rather than raw JSON, so the thing retained is exactly the thing
    /// published — the discipline `last_screen` already keeps, and the reason
    /// `the_screen_frame_matcher_agrees_with_what_is_published` exists.
    channel_tpls: Arc<Mutex<HashMap<i64, String>>>,
    /// WHICH KINDS EACH SCREEN SHOWS AT ALL — `{"1":["scripture","song"]}`
    /// (DECISIONS §98).
    ///
    /// The third small map in this struct and shaped exactly like the other two,
    /// deliberately: a second pattern for "a map of ids every client filters for
    /// itself" is how two maps come to be replayed at two different points in one
    /// hello reply.
    ///
    /// **A screen with NO OPINION is OMITTED, and `{}` is still sent.** Those are
    /// two different absences and both have to survive the wire: an omitted screen
    /// follows its template, which is where this decision lived before the column
    /// existed; an empty MAP means no screen anywhere has an opinion, and a page
    /// that could not tell that from "the reply has not arrived yet" would resolve
    /// the next fire against an opinion it has not been given.
    ///
    /// **It NARROWS content and nothing else.** `clear` and `black` do not pass
    /// through it, are not published from it, and never will be — a screen an
    /// operator can configure out of a panic control is rule 15's exact failure.
    channel_shows: Arc<Mutex<String>>,
    /// THE PROGRAMME TIMERS A STAGE TABLET IS SHOWING — the last `timer` frame.
    ///
    /// ITS OWN SLOT, NOT `last_screen`, and this is the fourth time that sentence
    /// has had to be written in this struct. One slot means the newest frame wins,
    /// so retaining a timer beside `content` would ERASE the retained verse: the
    /// next screen to join mid-reading would be handed a clock over a blank wall —
    /// rule 43's own failure, delivered by rule 43's own mechanism. It is also why
    /// `stage_next` is excluded from retention entirely.
    ///
    /// Retained, unlike `stage_alert`, because a programme timer is a STATE and not
    /// a moment: it is still running when the tablet comes back. Without this, a
    /// phone that locked its screen mid-sermon comes back with no clock until the
    /// operator next touches a timer, which during a sermon is never.
    ///
    /// A rehearsal publishes nothing to this hub at all — the gate is at
    /// `publish_timers` — so there is nothing of a rehearsal to replay here either.
    last_timers: Arc<Mutex<Option<String>>>,
    /// THE PICTURE EVERY SCREEN IS PAINTING ON — the last `background` frame.
    ///
    /// ITS OWN SLOT, NOT `last_screen`, and this is the FIFTH time that sentence
    /// has had to be written in this struct. One slot means the newest frame wins,
    /// so retaining a backdrop beside `content` would ERASE the retained verse:
    /// the next screen to join mid-reading would be handed wallpaper and no words
    /// — rule 43's own failure, delivered by rule 43's own mechanism.
    ///
    /// Retained, because a background is the most STATE-like thing this hub
    /// carries: it is put up once at the top of a service and is still up an hour
    /// later. A screen that reconnected without it comes back with the words
    /// floating on black while every other screen in the building carries the
    /// church's picture, and nothing anywhere says so.
    ///
    /// **Emptied by a panic control, at `publish`.** `clear` and `black` go
    /// through the same door, so the drop is by construction rather than by a
    /// publisher remembering — and the replay can no more resurrect a background
    /// than it can resurrect a verse. A rehearsal publishes nothing to this hub at
    /// all (the gate is at `set_background`), so there is nothing of a rehearsal
    /// to replay here either.
    last_background: Arc<Mutex<Option<String>>>,
    /// WHAT THE PREACHER'S OWN SCREEN IS HOLDING — see `stage_media_retention`.
    ///
    /// Its own slot for the same reason as the background's: it is neither a screen
    /// frame nor a timer, so retaining it in `last_screen` would replace the verse
    /// and hand the next screen to join a picture over a blank wall.
    last_stage_media: Arc<Mutex<Option<String>>>,
    /// WHAT THE CLIP ON THE SCREENS IS DOING — held, looping, and which replay.
    ///
    /// Its own slot, and the only one a CONTENT frame reaches into: a clip is
    /// always started at its beginning and playing, so a transport that outlived
    /// the clip it belonged to would hand the next video to a church already held.
    last_media_transport: Arc<Mutex<Option<String>>>,
    /// THE SCREENS THE OPERATOR HAS TAKEN OUT OF THE WALL — `{"4":"clear"}`.
    ///
    /// ITS OWN SLOT, NOT `last_screen`, and this is the fifth time that sentence
    /// has had to be written in this struct. One slot means the newest frame wins,
    /// so retaining this beside `content` would erase the verse a late-joining
    /// screen is owed (rule 43).
    ///
    /// Retained because it is a STATE and not a moment: a lobby TV the operator
    /// took down at the start of the sermon is still meant to be down when its
    /// browser source restarts twenty minutes later. Replayed LAST on hello,
    /// AFTER the retained screen frame, because it OVERRIDES what is on the
    /// screens — sent before it, the verse would paint over the operator's
    /// decision and the screen would bring itself back up.
    ///
    /// Ids and states only. Nothing a client chose, and nothing a preacher said.
    screens_down: Arc<Mutex<String>>,
    /// THE TEMPLATE IDS THIS INSTALL'S CONTENT LOOKS NAME — at most one per
    /// content kind, so five.
    ///
    /// NOT a cache of templates: `templates` above already holds every template's
    /// JSON. This is the far smaller question of WHICH of them a screen that has
    /// no look of its own can be asked to wear, and it exists because the answer
    /// has to be BOUNDED. A content look crosses the wire to an output as an id
    /// and no JSON (the 13 MB reason is at `main::cue_or_content_tpl`), so the
    /// receiver can only resolve an id it already holds the bytes for — and a
    /// browser source has no database to look them up in. The hub therefore sends
    /// them, in the hello reply, once per connect and never per fire.
    ///
    /// Sending `templates` wholesale instead would have been one line shorter and
    /// wrong in the way this product cannot afford: an install with a dozen
    /// templates, one of them carrying an embedded `data:` image, would push
    /// megabytes at every OBS source that reconnected — which is the very cost
    /// `cue_or_content_tpl` refuses to pay on the fire path, moved onto the
    /// connect path and paid every time the wifi blips.
    ///
    /// Warmed at startup and re-warmed whenever the operator changes the map, both
    /// from `main.rs`, which is the only place that knows what a content look is.
    look_ids: Arc<Mutex<Vec<i64>>>,
}

impl Default for KioskHub {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(128);
        KioskHub {
            tx,
            templates: Arc::new(Mutex::new(HashMap::new())),
            clients: Arc::new(Mutex::new(HashMap::new())),
            default_tpl: Arc::new(Mutex::new("null".to_string())),
            last_screen: Arc::new(Mutex::new(None)),
            last_screen_by_channel: Arc::new(Mutex::new(HashMap::new())),
            last_transition: Arc::new(Mutex::new(None)),
            channel_roles: Arc::new(Mutex::new("{}".to_string())),
            channel_looks: Arc::new(Mutex::new("{}".to_string())),
            channel_tpls: Arc::new(Mutex::new(HashMap::new())),
            channel_shows: Arc::new(Mutex::new("{}".to_string())),
            last_timers: Arc::new(Mutex::new(None)),
            last_background: Arc::new(Mutex::new(None)),
            last_stage_media: Arc::new(Mutex::new(None)),
            last_media_transport: Arc::new(Mutex::new(None)),
            // THE EMPTY FRAME, not an empty map. This slot holds a frame ready to
            // send, so seeding it with `{}` would put a bare object on the wire on
            // every hello before anything was ever taken down — a message with no
            // `kind`, which every client drops in silence.
            screens_down: Arc::new(Mutex::new(SCREENS_ALL_UP.to_string())),
            look_ids: Arc::new(Mutex::new(Vec::new())),
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
        if is_screen_frame(&msg) {
            // TARGETED OR NOT, and the two write different slots. See
            // `last_screen_by_channel`: an untargeted frame supersedes
            // everything and empties the map, so a present per-channel entry is
            // always the newer of the two and `hello` needs no clock to choose.
            match frame_channels(&msg) {
                Some(ids) => {
                    if let Ok(mut per) = self.last_screen_by_channel.lock() {
                        for id in ids {
                            per.insert(id, msg.clone());
                        }
                    }
                }
                None => {
                    if let Ok(mut last) = self.last_screen.lock() {
                        *last = Some(msg.clone());
                    }
                    if let Ok(mut per) = self.last_screen_by_channel.lock() {
                        per.clear();
                    }
                }
            }
        }
        // A SECOND SLOT, TESTED SEPARATELY. The two matchers are disjoint by
        // construction — a frame is `content`/`clear`/`black`, or it is `timer`, and
        // no frame this module builds is both — so a programme clock can never
        // become what a late-joining screen is shown, and a verse can never become
        // the programme.
        if is_timer_frame(&msg) {
            if let Ok(mut last) = self.last_timers.lock() {
                *last = Some(msg.clone());
            }
        }
        // A THIRD SLOT, AND THE ONLY ONE A PANIC CONTROL REACHES INTO.
        //
        // `background_retention` answers for every frame in the module at once:
        // leave it alone, take it down, or become it. The take-down arm is why the
        // decision is here rather than at `set_background` — `clear` and `black`
        // are published through this same door, so the church's backdrop goes with
        // the verse by construction, and nothing new had to be sent to achieve it.
        if let Some(next) = background_retention(&msg) {
            if let Ok(mut last) = self.last_background.lock() {
                *last = next;
            }
        }
        // A FIFTH SLOT, on the background's argument exactly: its own slot because
        // it is neither a screen frame nor a timer, and reached by the panic
        // controls because `clear` and `black` come through this door too.
        if let Some(next) = stage_media_retention(&msg) {
            if let Ok(mut last) = self.last_stage_media.lock() {
                *last = next;
            }
        }
        // A SIXTH SLOT, and the only one a CONTENT frame empties. Every other
        // retention here is cleared by a panic control alone; this one goes with
        // the next thing fired as well, because the transport belongs to the clip
        // rather than to the service.
        if let Some(next) = media_transport_retention(&msg) {
            if let Ok(mut last) = self.last_media_transport.lock() {
                *last = next;
            }
        }
        // A FOURTH SLOT, and the same disjointness argument. A `screen_state` frame
        // is neither a screen frame nor a timer — it says which screens the
        // operator has taken OUT of the wall, which is a fact about screens rather
        // than a thing to paint on one. The whole set rides every time, so the
        // newest frame winning is correct here rather than lossy.
        if is_screen_state_frame(&msg) {
            if let Ok(mut last) = self.screens_down.lock() {
                *last = msg.clone();
            }
        }
        let _ = self.tx.send(msg); // Err only means no subscribers — fine.
    }
    /// Shared handle to the retained screen frame, for the WS task to send on hello.
    /// Shared handle to the per-channel screen frames, for the WS task's `hello`.
    pub fn last_screen_by_channel_handle(&self) -> Arc<Mutex<HashMap<i64, String>>> {
        self.last_screen_by_channel.clone()
    }
    pub fn last_screen_handle(&self) -> Arc<Mutex<Option<String>>> {
        self.last_screen.clone()
    }
    /// Shared handle to the retained programme timers, for the WS task's hello.
    pub fn last_timers_handle(&self) -> Arc<Mutex<Option<String>>> {
        self.last_timers.clone()
    }
    /// Shared handle to the retained background, for the WS task's hello.
    pub fn last_media_transport_handle(&self) -> Arc<Mutex<Option<String>>> {
        self.last_media_transport.clone()
    }

    pub fn last_stage_media_handle(&self) -> Arc<Mutex<Option<String>>> {
        self.last_stage_media.clone()
    }

    pub fn last_background_handle(&self) -> Arc<Mutex<Option<String>>> {
        self.last_background.clone()
    }
    /// WHAT PICTURE IS BEHIND EVERYTHING RIGHT NOW, for a screen with no socket.
    ///
    /// The same argument `current_transition` makes: the kiosk hub replays the
    /// retained frame on `hello`, and a NATIVE output window has the Tauri bridge
    /// and no socket. Without a read-back, a projector opened mid-service through
    /// `open_channel_output` would be the one screen in the building with no
    /// backdrop, until the operator happened to change it.
    ///
    /// Parsed rather than kept as a second field, deliberately: one slot is one
    /// truth, and a struct beside the frame is a second copy that can disagree
    /// with what the LAN screens were actually sent. The parse is off the fire
    /// path — it runs once, when an output window mounts.
    pub fn current_background(&self) -> Option<(String, String)> {
        let frame = self.last_background.lock().ok()?.clone()?;
        let v: serde_json::Value = serde_json::from_str(&frame).ok()?;
        let url = v.get("media_url")?.as_str()?.to_string();
        let kind = v
            .get("media_kind")
            .and_then(|k| k.as_str())
            .unwrap_or("image")
            .to_string();
        Some((url, kind))
    }
    /// Shared handle to the screens the operator has taken down, for the WS task's
    /// hello.
    pub fn screens_down_handle(&self) -> Arc<Mutex<String>> {
        self.screens_down.clone()
    }
    /// Shared handle to the retained transition override, for the WS task's hello.
    pub fn last_transition_handle(&self) -> TransitionSlot {
        self.last_transition.clone()
    }
    /// Remember the operator's override and push it to every connected client.
    ///
    /// `mode: None` clears it — every screen goes back to following its template.
    ///
    /// The mode is NOT validated against a list of the seven here, on purpose.
    /// `src/lib/transitions.js` is the one register of what a transition is
    /// (DECISIONS §71), an unknown mode there is already a cut, and a second copy
    /// of the seven names in Rust is a second register that can drift from the
    /// first. What this DOES enforce is frame integrity: the value is serialised
    /// through `serde_json`, so no string an operator or a bad caller could supply
    /// can break out of the JSON, and anything implausibly long is dropped rather
    /// than retained and replayed to every screen that joins for the rest of the
    /// service.
    pub fn set_transition(&self, mode: Option<String>, ms: Option<u32>) {
        let mode = mode.filter(|m| !m.is_empty() && m.len() <= 32);
        let next = mode.map(|m| (m, ms));
        if let Ok(mut t) = self.last_transition.lock() {
            *t = next.clone();
        }
        self.publish(transition_json(next.as_ref()));
    }
    /// What is in force right now, for the console to read back on mount.
    ///
    /// Without this the console could reload mid-service, show "Follow template",
    /// and be wrong about every screen in the building — a control reading the same
    /// when it is in force as when it is not, which is rule 35.
    pub fn current_transition(&self) -> TransitionOverride {
        self.last_transition.lock().ok().and_then(|t| t.clone())
    }
    /// The raw broadcast sender — for the WebSocket server task to subscribe to,
    /// and for tests to listen on.
    ///
    /// `pub(crate)`, deliberately. It is a door out of this module that bypasses
    /// `publish`: `hub.sender().send(json)` reaches every connected kiosk client
    /// and contains neither `publish_kiosk(` nor `.publish(`, so the rehearsal
    /// enumeration cannot see it. Keeping it inside the crate keeps that door
    /// somewhere `every_publisher_in_this_module_has_an_explicit_rehearsal_verdict`
    /// can reach, and that test forbids its use anywhere in this module.
    pub(crate) fn sender(&self) -> broadcast::Sender<String> {
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
    /// Shared handle to the default-template blob, for the WS server task to read
    /// and send to each client on `hello`.
    pub fn default_template_handle(&self) -> Arc<Mutex<String>> {
        self.default_tpl.clone()
    }
    /// Validate + store the default template WITHOUT pushing (startup warm).
    /// Anything that is not valid JSON becomes the literal `null`, because the
    /// value is embedded raw into a WS frame and one unparseable frame stops a
    /// client applying every frame after it.
    pub fn cache_default_template(&self, template_json: &str) {
        let safe = match serde_json::from_str::<serde_json::Value>(template_json) {
            Ok(_) => template_json.to_string(),
            Err(_) => "null".to_string(),
        };
        if let Ok(mut t) = self.default_tpl.lock() {
            *t = safe;
        }
    }
    /// The cached default template JSON (`null` when none is configured).
    pub fn default_template_json(&self) -> String {
        self.default_tpl
            .lock()
            .map(|t| t.clone())
            .unwrap_or_else(|_| "null".into())
    }
    /// Update the default template AND push it live, so a screen following the
    /// content look re-resolves the instant the operator changes the default
    /// instead of at the next reload. Same validate-then-store rule as
    /// `cache_default_template`.
    pub fn set_default_template(&self, template_json: &str) {
        self.cache_default_template(template_json);
        let blob = self.default_template_json();
        self.publish(format!(
            r#"{{"kind":"default_template","template":{blob}}}"#
        ));
    }
    /// Shared handle to the role map, for the WS server task to send on `hello`.
    pub fn channel_roles_handle(&self) -> Arc<Mutex<String>> {
        self.channel_roles.clone()
    }
    /// Validate + store the role map WITHOUT pushing (startup warm).
    ///
    /// Same validate-then-store rule as `cache_default_template`, and for the same
    /// reason: the value is embedded RAW into a WS frame, and one unparseable
    /// frame stops a client applying every frame after it. Anything that is not a
    /// JSON object becomes `{}` — which is "no screen has a role", the safe
    /// reading, because every filter downstream asks whether a role IS `stage`.
    pub fn cache_channel_roles(&self, roles_json: &str) {
        let safe = match serde_json::from_str::<serde_json::Value>(roles_json) {
            Ok(v) if v.is_object() => roles_json.to_string(),
            _ => "{}".to_string(),
        };
        if let Ok(mut r) = self.channel_roles.lock() {
            *r = safe;
        }
    }
    /// The cached role map (`{}` when no screen has a role).
    pub fn channel_roles_json(&self) -> String {
        self.channel_roles
            .lock()
            .map(|r| r.clone())
            .unwrap_or_else(|_| "{}".into())
    }
    /// Update the role map AND push it live, so a screen already open learns it
    /// has become — or stopped being — the stage without waiting for a reload.
    ///
    /// It paints nothing on its own. What it changes is whether the NEXT Stage
    /// Message is accepted, which is the half of this that must not wait: an
    /// operator who has just made a tablet the stage display is about to type a
    /// message to the person holding it.
    pub fn set_channel_roles(&self, roles_json: &str) {
        self.cache_channel_roles(roles_json);
        let blob = self.channel_roles_json();
        self.publish(format!(r#"{{"kind":"channel_roles","roles":{blob}}}"#));
    }
    /// Shared handle to the per-kind look map, for the WS server task's `hello`.
    pub fn channel_looks_handle(&self) -> Arc<Mutex<String>> {
        self.channel_looks.clone()
    }
    /// Validate + store the per-kind look map WITHOUT pushing (startup warm).
    ///
    /// Same validate-then-store rule as `cache_channel_roles`, and for the same
    /// reason: the value is embedded RAW into a WS frame, and one unparseable
    /// frame stops a client applying every frame after it — including the retained
    /// verse that arrives later in the same hello reply. Anything that is not a
    /// JSON object becomes `{}`, which is "no screen has a per-kind look": the safe
    /// reading, because every reader downstream asks whether a look IS set and
    /// falls through to the screen's own template when it is not.
    pub fn cache_channel_looks(&self, looks_json: &str) {
        let safe = match serde_json::from_str::<serde_json::Value>(looks_json) {
            Ok(v) if v.is_object() => looks_json.to_string(),
            _ => "{}".to_string(),
        };
        if let Ok(mut l) = self.channel_looks.lock() {
            *l = safe;
        }
    }
    /// The cached per-kind look map (`{}` when no screen has one).
    pub fn channel_looks_json(&self) -> String {
        self.channel_looks
            .lock()
            .map(|l| l.clone())
            .unwrap_or_else(|_| "{}".into())
    }
    /// Update the per-kind look map AND push it live, so a screen already open
    /// learns what it now wears without waiting for a reload.
    ///
    /// It paints nothing on its own — like `set_channel_roles` and
    /// `set_default_template`, it is configuration and a screen that receives it
    /// looks identical until the next fire. What must not wait is the OTHER half:
    /// an operator who has just set the lobby TV's Announcements look is about to
    /// fire an announcement, and a map that arrived one frame later would put the
    /// old look on the wall once, in front of a congregation.
    pub fn set_channel_looks(&self, looks_json: &str) {
        self.cache_channel_looks(looks_json);
        let blob = self.channel_looks_json();
        self.publish(format!(r#"{{"kind":"channel_looks","looks":{blob}}}"#));
    }
    /// Shared handle to the per-screen `shows` map, for the WS task's `hello`.
    pub fn channel_shows_handle(&self) -> Arc<Mutex<String>> {
        self.channel_shows.clone()
    }
    /// Validate + store the `shows` map WITHOUT pushing (startup warm).
    ///
    /// Same validate-then-store rule as `cache_channel_roles` and
    /// `cache_channel_looks`, and for the same reason: the value goes RAW into a
    /// WS frame, and one unparseable frame stops a client applying every frame
    /// after it — including the retained verse later in the same hello reply.
    /// Anything that is not a JSON object becomes `{}`, which is "no screen has an
    /// opinion": every screen follows its template, which is where this decision
    /// lived before the column existed and is the only safe direction to fail in.
    /// The unsafe direction is an empty LIST, which is a congregation screen that
    /// paints nothing for the rest of a service.
    pub fn cache_channel_shows(&self, shows_json: &str) {
        let safe = match serde_json::from_str::<serde_json::Value>(shows_json) {
            Ok(v) if v.is_object() => shows_json.to_string(),
            _ => "{}".to_string(),
        };
        if let Ok(mut m) = self.channel_shows.lock() {
            *m = safe;
        }
    }
    /// The cached `shows` map (`{}` when no screen has an opinion).
    pub fn channel_shows_json(&self) -> String {
        self.channel_shows
            .lock()
            .map(|m| m.clone())
            .unwrap_or_else(|_| "{}".into())
    }
    /// Update the `shows` map AND push it live.
    ///
    /// It paints nothing on arrival and it can never blank a screen that is
    /// already showing something: it narrows what the NEXT fire of a kind reaches,
    /// which is what makes it configuration rather than a control. What must not
    /// wait is the other half — an operator who has just taken the countdown off
    /// the wall is about to start one.
    pub fn set_channel_shows(&self, shows_json: &str) {
        self.cache_channel_shows(shows_json);
        let blob = self.channel_shows_json();
        self.publish(format!(r#"{{"kind":"channel_shows","shows":{blob}}}"#));
    }
    /// Shared handle to the per-channel template frames, for the WS task's `hello`.
    pub fn channel_templates_handle(&self) -> Arc<Mutex<HashMap<i64, String>>> {
        self.channel_tpls.clone()
    }
    /// Retain what ONE screen wears, WITHOUT pushing (startup warm).
    ///
    /// `template_json` is the template's own JSON, or the literal `"null"` for a
    /// screen that has no look of its own and follows the content look
    /// (DECISIONS §70). **`"null"` is stored and sent rather than omitted**, on the
    /// same argument as `{}` in the two maps above: it is how a page learns it is a
    /// FOLLOWER, and a page that cannot tell that from "the reply has not arrived
    /// yet" keeps whatever its URL's `template_id` gave it — which for the shipped
    /// URL is nothing, and for a hand-built one is a look the operator has already
    /// replaced.
    pub fn cache_channel_template(&self, channel_id: i64, template_json: &str) {
        let safe = match serde_json::from_str::<serde_json::Value>(template_json) {
            Ok(_) => template_json.to_string(),
            // Unparseable bytes would go RAW into a WS frame and stop a client
            // applying every frame after it, including the retained verse later in
            // the same hello reply. `null` is the safe reading: the screen follows
            // the content look, which is where it was before this existed.
            Err(_) => "null".to_string(),
        };
        if let Ok(mut m) = self.channel_tpls.lock() {
            m.insert(
                channel_id,
                format!(
                    r#"{{"kind":"channel_template","channel":{channel_id},"template":{safe}}}"#
                ),
            );
        }
    }
    /// A screen has been deleted: forget what it wore.
    ///
    /// Without this the hub would answer for a channel no page can be, for the
    /// life of the process — harmless in itself, and exactly the kind of stale
    /// entry that makes a later reader believe the map is authoritative.
    pub fn forget_channel_template(&self, channel_id: i64) {
        if let Ok(mut m) = self.channel_tpls.lock() {
            m.remove(&channel_id);
        }
    }
    /// Reassign what ONE screen wears AND push it live.
    ///
    /// The retention and the publish are one call, deliberately: they were two
    /// (a `kiosk.publish` in `main.rs` and nothing retaining it) and that IS the
    /// defect — the frame reached whoever happened to be connected and nothing
    /// answered for the screen that connected a minute later. Rule 36: the choke
    /// point is where the check goes.
    pub fn set_channel_template(&self, channel_id: i64, template_json: &str) {
        self.cache_channel_template(channel_id, template_json);
        let frame = self
            .channel_tpls
            .lock()
            .ok()
            .and_then(|m| m.get(&channel_id).cloned());
        if let Some(frame) = frame {
            self.publish(frame);
        }
    }
    /// Shared handle to the content-look id list, for the WS task's `hello`.
    pub fn look_ids_handle(&self) -> Arc<Mutex<Vec<i64>>> {
        self.look_ids.clone()
    }
    /// Record which template ids this install's content looks name.
    ///
    /// Deduplicated and CAPPED. The cap is not defensiveness about a caller that
    /// cannot currently misbehave: it is the property the whole design rests on.
    /// Every id here becomes a `template` frame in every hello reply, so an
    /// unbounded list turns a reconnect into a bulk template download over a
    /// church's wifi — the cost `cue_or_content_tpl` refuses on the fire path,
    /// moved somewhere less visible.
    ///
    /// The list is now TWO sources, not one: the five global content looks, and a
    /// per-kind look per (screen, kind) (DECISIONS §97). The second has no exact
    /// bound, so `MAX_LOOK_IDS` is a judgement rather than an arithmetic fact, and
    /// truncating **says so**. Silent shedding is how a pipeline reaches "fine"
    /// while missing half its work (rule 33), and what is shed here is the bytes
    /// behind a look some screen is wearing — so the screen falls back to its
    /// blanket template and looks like a screen nobody configured. This runs when
    /// the operator changes a look and at startup, never on the fire path, so a
    /// line of output costs nothing that matters.
    pub fn cache_look_ids(&self, ids: &[i64]) {
        let mut seen: Vec<i64> = Vec::new();
        for id in ids {
            if !seen.contains(id) {
                seen.push(*id);
            }
        }
        if seen.len() > MAX_LOOK_IDS {
            println!(
                "kiosk: {} distinct look templates is over the {MAX_LOOK_IDS} a hello \
                 reply will carry — the ones past that are not sent, and any screen \
                 wearing one will fall back to its own template",
                seen.len()
            );
            seen.truncate(MAX_LOOK_IDS);
        }
        if let Ok(mut l) = self.look_ids.lock() {
            *l = seen;
        }
    }
    /// The content-look ids currently cached.
    ///
    /// Test-only: production reads the list through `look_ids_handle`, inside the
    /// WS task, because the hello reply is the only thing that needs it. A public
    /// getter nothing calls is a getter nobody is watching.
    #[cfg(test)]
    pub fn look_ids(&self) -> Vec<i64> {
        self.look_ids.lock().map(|l| l.clone()).unwrap_or_default()
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
    default_tpl: Arc<Mutex<String>>,
    channel_roles: Arc<Mutex<String>>,
    channel_looks: Arc<Mutex<String>>,
    channel_tpls: Arc<Mutex<HashMap<i64, String>>>,
    channel_shows: Arc<Mutex<String>>,
    last_screen: Arc<Mutex<Option<String>>>,
    last_screen_by_channel: Arc<Mutex<HashMap<i64, String>>>,
    last_transition: TransitionSlot,
    last_timers: Arc<Mutex<Option<String>>>,
    last_background: Arc<Mutex<Option<String>>>,
    last_stage_media: Arc<Mutex<Option<String>>>,
    last_media_transport: Arc<Mutex<Option<String>>>,
    screens_down: Arc<Mutex<String>>,
    look_ids: Arc<Mutex<Vec<i64>>>,
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
        let default_tpl = default_tpl.clone();
        let channel_roles = channel_roles.clone();
        let channel_looks = channel_looks.clone();
        let channel_tpls = channel_tpls.clone();
        let channel_shows = channel_shows.clone();
        let last_screen = last_screen.clone();
        let last_screen_by_channel = last_screen_by_channel.clone();
        let last_transition = last_transition.clone();
        let last_timers = last_timers.clone();
        let last_background = last_background.clone();
        let last_stage_media = last_stage_media.clone();
        let last_media_transport = last_media_transport.clone();
        let screens_down = screens_down.clone();
        let look_ids = look_ids.clone();
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
            // WHICH SCREEN THIS SOCKET IS, from its hello, so a lagging client can be
            // handed its own retained frame again (RG-195).
            let mut hello_channel: Option<i64> = None;
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
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // RE-SYNC, NOT SKIP (RG-195). The frames this client
                            // missed may include the one that decides what it
                            // shows — a `clear` is a panic control, and a panic
                            // control that did not land on one screen with nothing
                            // saying so is rule 15's exact failure. Hand it what a
                            // screen that just joined would get, and count it.
                            if let Some(ch) = hello_channel {
                                health.note_resync(ch);
                            }
                            for f in resync_frames(
                                &last_screen,
                                &last_screen_by_channel,
                                &screens_down,
                                hello_channel,
                            ) {
                                if write
                                    .send(tokio_tungstenite::tungstenite::Message::Text(f))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            continue;
                        }
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
                                        health.beat(
                                            ch,
                                            st,
                                            "kiosk",
                                            BeatGap::from_json(&v),
                                            MediaBeat::from_json(&v),
                                        );
                                        health.note_media_error(ch, media_error_from_json(&v));
                                        // ── AND THE SCREEN IS TOLD THE TIME ──
                                        //
                                        // Answered here, INSIDE the parse, so an
                                        // unparseable beat draws no reply: an ack
                                        // that arrived for anything would let a
                                        // client tell a good frame from a bad one
                                        // by whether the server spoke, which is a
                                        // probe this read-only server does not owe
                                        // anybody.
                                        //
                                        // Two findings wanted this one frame. The
                                        // page cannot detect a half-open socket
                                        // from silence, because the hub only
                                        // publishes when something CHANGES and
                                        // silence is the normal state of a quiet
                                        // service — so the thing it is already
                                        // sending every two seconds gets an
                                        // answer, and three unanswered beats mean
                                        // stale. And `countdown_to` is an absolute
                                        // epoch produced on THIS machine while the
                                        // stage page subtracts its own
                                        // `Date.now()`, so a tablet a minute out
                                        // showed a minute of error to the person
                                        // preaching; the host clock rides the ack,
                                        // and the round trip that carried it is
                                        // what bounds how well the offset can be
                                        // known.
                                        //
                                        // To the ONE client that beat, exactly as
                                        // the hello reply is, rather than
                                        // broadcast: a tick to every browser
                                        // source and lobby TV in the building
                                        // would be traffic bought for one page.
                                        // It carries a number this server already
                                        // knows and nothing any client said.
                                        let out = format!(
                                            r#"{{"kind":"beat_ack","at":{}}}"#,
                                            crate::now_epoch_ms()
                                        );
                                        let _ = write
                                            .send(tokio_tungstenite::tungstenite::Message::Text(
                                                out,
                                            ))
                                            .await;
                                    }
                                }
                                if v.get("kind").and_then(|k| k.as_str()) == Some("hello") {
                                    // THE WHOLE REPLY USED TO SIT INSIDE
                                    // `if let Some(id) = template_id`, AND THAT IS
                                    // RULE 43 WITH A HOLE IN IT.
                                    //
                                    // `Output.svelte` sends `template_id: null`
                                    // whenever the URL is CHANNEL-keyed — which is
                                    // the URL `Copy URL` produces and the one
                                    // CLAUDE.md tells operators to use, because a
                                    // channel-keyed source follows a template swap
                                    // and a `template_id`-keyed one does not. So
                                    // the recommended URL was the one shape that
                                    // got NO RETAINED FRAME:
                                    // an OBS source restarting mid-reading came
                                    // back black and stayed black until the next
                                    // fire, which is the exact failure rule 43 and
                                    // DECISIONS §68 exist to prevent. Measured on a
                                    // running build: `?channel=1` joined blank,
                                    // `?channel=1&template_id=1` joined with the
                                    // verse.
                                    //
                                    // Registration and the template reply still
                                    // need an id — a client with no template id has
                                    // no template to be counted against, and the
                                    // liveness count is per template id. The
                                    // retained frame needs nothing: it is about
                                    // what is ON THE SCREENS, not about which
                                    // look this screen wears.
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
                                    }
                                    // THIS SCREEN'S OWN TEMPLATE — the rung above
                                    // every look, and the one the hello reply used
                                    // to answer for nobody.
                                    //
                                    // The block above needs a `template_id`,
                                    // because the liveness count is per template
                                    // id and a client with no id has nothing to be
                                    // counted against. A screen's LOOK is a
                                    // different question with a different key, and
                                    // welding the two meant the URL `Copy URL`
                                    // produces — channel-keyed, `template_id:
                                    // null`, the only shape that follows a template
                                    // swap — was the one shape that received no
                                    // template at all. Measured on a real install:
                                    // four screens carrying four different
                                    // templates, all four painting the same one.
                                    //
                                    // ONE CHANNEL'S FRAME. A template is bytes and
                                    // one in the field was 13 MB, so this is keyed
                                    // by the channel the client said it was rather
                                    // than sent as a map like the two above it.
                                    // `channel: 0` is a raw preview that belongs to
                                    // no screen and is answered for by nobody.
                                    //
                                    // BEFORE the retained frame, with the
                                    // configuration, for the reason every other
                                    // line in this block gives: a template sent
                                    // after the verse repaints a reading a
                                    // congregation is already looking at.
                                    if let Some(ch) =
                                        v.get("channel").and_then(|c| c.as_i64()).filter(|c| *c > 0)
                                    {
                                        hello_channel = Some(ch);
                                        let mine = channel_tpls
                                            .lock()
                                            .ok()
                                            .and_then(|m| m.get(&ch).cloned());
                                        if let Some(frame) = mine {
                                            let _ = write
                                                .send(tokio_tungstenite::tungstenite::Message::Text(
                                                    frame,
                                                ))
                                                .await;
                                        }
                                    }
                                    // THE CONTENT LOOKS, BY VALUE, BECAUSE THE
                                    // FIRE PATH ONLY EVER SENDS THE NUMBER.
                                    //
                                    // A per-kind content look reaches a screen as
                                    // `content.template_id` and as nothing else —
                                    // deliberately, and it is a hard performance
                                    // rule recorded at `cue_or_content_tpl`: one
                                    // look carrying an embedded `data:` image was
                                    // 13 MB, and serialising that onto every fire
                                    // made verses take seconds to leave the
                                    // machine. So the id rides and the bytes do
                                    // not, which means the bytes have to already
                                    // be at the receiver when the id arrives.
                                    //
                                    // A native output window fetches them over the
                                    // Tauri bridge; a browser source has no bridge
                                    // and no database, so this is the only way it
                                    // can ever learn what template 9 IS. Without
                                    // it the id crossed the wire and died at the
                                    // receiver, and a screen set to follow the
                                    // content look wore the configured default for
                                    // the life of the product.
                                    //
                                    // HERE, in the configuration block, and not
                                    // after the retained frame: rule 43 replays
                                    // what is on the screens LAST precisely so the
                                    // look it needs has already arrived, and a
                                    // template sent afterwards would repaint a
                                    // verse a congregation is already reading.
                                    //
                                    // Bounded at `MAX_LOOK_IDS` where the
                                    // list is written, so a reconnect can never
                                    // become a bulk template download.
                                    let looks = look_ids
                                        .lock()
                                        .map(|l| l.clone())
                                        .unwrap_or_default();
                                    for lid in looks {
                                        let cached =
                                            templates.lock().ok().and_then(|m| m.get(&lid).cloned());
                                        if let Some(tpl) = cached {
                                            let out = format!(
                                                r#"{{"kind":"template","id":{lid},"template":{tpl}}}"#
                                            );
                                            let _ = write
                                                .send(tokio_tungstenite::tungstenite::Message::Text(out))
                                                .await;
                                        }
                                    }
                                    // The configured default template, so this
                                    // client can end its resolution chain where
                                    // the operator said rather than at the
                                    // bundled builtin. Like the template above
                                    // it, this is configuration: on its own it
                                    // paints nothing.
                                    let dblob = default_tpl
                                        .lock()
                                        .map(|t| t.clone())
                                        .unwrap_or_else(|_| "null".into());
                                    let _ = write
                                        .send(tokio_tungstenite::tungstenite::Message::Text(
                                            format!(
                                                r#"{{"kind":"default_template","template":{dblob}}}"#
                                            ),
                                        ))
                                        .await;
                                    // WHAT EACH SCREEN IS FOR. Sent on every
                                    // hello, not only when something has a role,
                                    // because `{}` is an answer: it is how a page
                                    // learns it is NOT the stage. Withholding it
                                    // would leave a screen unable to tell "no role
                                    // is set" from "the reply has not arrived yet",
                                    // and the only thing downstream of it is
                                    // whether a private message may be painted.
                                    let rblob = channel_roles
                                        .lock()
                                        .map(|r| r.clone())
                                        .unwrap_or_else(|_| "{}".into());
                                    let _ = write
                                        .send(tokio_tungstenite::tungstenite::Message::Text(
                                            format!(
                                                r#"{{"kind":"channel_roles","roles":{rblob}}}"#
                                            ),
                                        ))
                                        .await;
                                    // WHAT EACH SCREEN WEARS FOR EACH KIND
                                    // (DECISIONS §97). Ids only — the bytes went
                                    // up with the `template` frames above, which
                                    // is why this sits AFTER them and not before.
                                    //
                                    // Sent on EVERY hello, `{}` included, on the
                                    // identical argument to the role map one line
                                    // up: `{}` is an answer. It is how a page
                                    // learns it has NO per-kind look, and a page
                                    // that cannot tell that from "the reply has
                                    // not arrived yet" resolves the wrong template
                                    // for one frame — which is the first frame
                                    // after an OBS source restarts mid-reading.
                                    //
                                    // And BEFORE the retained screen frame, which
                                    // is the whole of the ordering rule (rule 43):
                                    // a look must be in hand before the frame it
                                    // dresses, or the verse replayed to a screen
                                    // that joined late is painted once in the
                                    // wrong template and then corrected in front
                                    // of a congregation.
                                    let lblob = channel_looks
                                        .lock()
                                        .map(|l| l.clone())
                                        .unwrap_or_else(|_| "{}".into());
                                    let _ = write
                                        .send(tokio_tungstenite::tungstenite::Message::Text(
                                            format!(
                                                r#"{{"kind":"channel_looks","looks":{lblob}}}"#
                                            ),
                                        ))
                                        .await;
                                    // AND WHICH KINDS EACH SCREEN SHOWS AT ALL
                                    // (DECISIONS §98). Sent on EVERY hello, `{}`
                                    // included, on the identical argument to the
                                    // two maps above: an empty answer is an answer.
                                    // A page that could not tell "nobody has told
                                    // me" from "no screen has an opinion" would
                                    // resolve the first fire after a reconnect
                                    // against something it has not been given.
                                    //
                                    // With the configuration and before the
                                    // retained frame, because it decides whether
                                    // that frame paints here at all — sent after
                                    // it, a screen the operator has taken the
                                    // countdown off would paint one and then drop
                                    // it, in front of a congregation.
                                    let sblob = channel_shows
                                        .lock()
                                        .map(|m| m.clone())
                                        .unwrap_or_else(|_| "{}".into());
                                    let _ = write
                                        .send(tokio_tungstenite::tungstenite::Message::Text(
                                            format!(
                                                r#"{{"kind":"channel_shows","shows":{sblob}}}"#
                                            ),
                                        ))
                                        .await;
                                    // The operator's transition override, if one is
                                    // in force (DECISIONS §84). Sent BEFORE the
                                    // retained frame, so a screen that joins late
                                    // is not the only one in the building still
                                    // cutting while the rest crossfade. Like the
                                    // template above it, this is configuration:
                                    // on its own it paints nothing.
                                    let x = last_transition.lock().ok().and_then(|t| t.clone());
                                    if x.is_some() {
                                        let _ = write
                                            .send(tokio_tungstenite::tungstenite::Message::Text(
                                                transition_json(x.as_ref()),
                                            ))
                                            .await;
                                    }
                                    // THE PROGRAMME TIMERS, if any are running.
                                    // BEFORE the retained frame and after the
                                    // configuration, which is the whole of the
                                    // ordering rule: a tablet sent the timers after
                                    // the reading paints the reading and then a
                                    // clock over it — a flash at exactly the moment
                                    // a preacher looks down. A timer is state, not
                                    // a moment (that is `stage_alert`, which is
                                    // deliberately not retained at all), so it is
                                    // replayed; and it is kept in its own slot, so
                                    // it cannot have replaced the verse below.
                                    let timers =
                                        last_timers.lock().ok().and_then(|t| t.clone());
                                    if let Some(frame) = timers {
                                        let _ = write
                                            .send(tokio_tungstenite::tungstenite::Message::Text(frame))
                                            .await;
                                    }
                                    // THE PICTURE EVERYTHING IS PAINTED ON.
                                    // Before the reading and after the
                                    // configuration, on the same ordering rule as
                                    // the timers above: a screen sent the backdrop
                                    // AFTER the verse paints the verse and then
                                    // slides a picture in behind it, which on a
                                    // congregation wall reads as a fault. Its own
                                    // slot, so it cannot have replaced the verse
                                    // below; emptied by `clear` and `black` at the
                                    // retention door, so there is nothing here to
                                    // replay after a panic control.
                                    let backdrop =
                                        last_background.lock().ok().and_then(|b| b.clone());
                                    if let Some(frame) = backdrop {
                                        let _ = write
                                            .send(tokio_tungstenite::tungstenite::Message::Text(frame))
                                            .await;
                                    }
                                    // AND THE PREACHER'S OWN SLIDE, if one is up.
                                    //
                                    // Rule 43, for the one screen in the building
                                    // that is not a congregation screen: a stage
                                    // tablet whose wifi dropped mid-sermon came back
                                    // with the reading and no slide, and stayed that
                                    // way until the operator happened to push it
                                    // again. Sent BEFORE the content below, on the
                                    // same ordering rule as the backdrop: the device
                                    // paints a reading over the media, so the media
                                    // has to be there first for the reading to be
                                    // over anything.
                                    //
                                    // It can never undo a panic control, because
                                    // `clear` and `black` empty this slot at the
                                    // retention door rather than being filtered out
                                    // here.
                                    let stage_media_frame =
                                        last_stage_media.lock().ok().and_then(|m| m.clone());
                                    if let Some(frame) = stage_media_frame {
                                        let _ = write
                                            .send(tokio_tungstenite::tungstenite::Message::Text(frame))
                                            .await;
                                    }
                                    // AND WHAT THE CLIP IS DOING, if one is held or
                                    // looping. A screen that rejoined mid-service
                                    // would otherwise start the clip playing while
                                    // every other screen sat held — and the operator
                                    // who pressed Pause would be watching one screen
                                    // disobey with nothing to say why.
                                    //
                                    // Emptied by the next fire as well as by a panic
                                    // control, so this can only ever describe the
                                    // clip that is actually up.
                                    let transport_frame =
                                        last_media_transport.lock().ok().and_then(|t| t.clone());
                                    if let Some(frame) = transport_frame {
                                        let _ = write
                                            .send(tokio_tungstenite::tungstenite::Message::Text(frame))
                                            .await;
                                    }
                                    // AND WHAT IS ON THE SCREENS RIGHT NOW.
                                    // Sent LAST so the template it needs to render
                                    // with has already arrived. `clear` and `black`
                                    // are retained the same way, so this can never
                                    // undo a panic control.
                                    //
                                    // AND WHAT IS ON *THIS* SCREEN, once a cue
                                    // can name screens (RG-161). `the screens`
                                    // stopped being one answer, so the per-
                                    // channel slot is preferred and the global
                                    // one is the fallback. No timestamps are
                                    // needed to choose between them: an
                                    // untargeted frame empties the map when it
                                    // is published, so a present entry is
                                    // always the newer of the two — which is
                                    // also why a `clear` still reaches a screen
                                    // that joins afterwards.
                                    let mine = v
                                        .get("channel")
                                        .and_then(|c| c.as_i64())
                                        .filter(|c| *c > 0)
                                        .and_then(|ch| {
                                            last_screen_by_channel
                                                .lock()
                                                .ok()
                                                .and_then(|m| m.get(&ch).cloned())
                                        });
                                    let retained = mine
                                        .or_else(|| last_screen.lock().ok().and_then(|l| l.clone()));
                                    if let Some(frame) = retained {
                                        let _ = write
                                            .send(tokio_tungstenite::tungstenite::Message::Text(frame))
                                            .await;
                                    }
                                    // AND WHICH SCREENS THE OPERATOR HAS TAKEN OUT
                                    // OF THE WALL.
                                    //
                                    // LAST, and the order is the whole of it. This
                                    // frame OVERRIDES what is on the screens for
                                    // the screens it names, so sent before the
                                    // retained verse the verse would paint over the
                                    // operator's decision and a lobby TV whose
                                    // browser source restarted would bring itself
                                    // back up mid-sermon — rule 43's own mechanism
                                    // undoing an operator's own control.
                                    //
                                    // Sent on EVERY hello, including when nothing
                                    // is down, because `{}` is an answer: it is how
                                    // a page learns it is NOT down, and a page that
                                    // cannot tell "nobody has told me" from "I am
                                    // up" is the distinction rule 35 is about.
                                    let sd = screens_down
                                        .lock()
                                        .map(|d| d.clone())
                                        .unwrap_or_else(|_| SCREENS_ALL_UP.into());
                                    let _ = write
                                        .send(tokio_tungstenite::tungstenite::Message::Text(sd))
                                        .await;
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

/// Where the pictures Relay ships live inside the bundle.
///
/// The frontend half of this constant is `BUNDLED_BACKGROUND_DIR` in
/// `src/lib/bundledbackgrounds.js`, which is what puts them there: the build
/// emits `src/backgrounds/*` at a stable, unhashed path precisely so the seed
/// can name one and this server can serve it (DECISIONS §90).
pub(crate) const BUNDLED_BACKGROUND_DIR: &str = "backgrounds";

/// The bundled pictures, by served file name, in a stable order.
///
/// Read out of the embedded bundle rather than from a list written down twice.
/// Vite sanitises the served name (`02 Emerald …jpg` becomes
/// `02_Emerald_…jpg`) and nothing in Rust needs to know that rule as long as it
/// reads the answer instead of re-deriving it.
///
/// Empty when `dist/` was built without the folder — which is the same
/// condition RG-127 already describes, and `db::starter`'s own tests say so in
/// words rather than seeding nothing in silence.
pub(crate) fn bundled_backgrounds() -> Vec<&'static str> {
    let Some(dir) = DIST.get_dir(BUNDLED_BACKGROUND_DIR) else {
        return Vec::new();
    };
    let mut names: Vec<&'static str> = dir
        .files()
        .filter_map(|f| f.path().file_name().and_then(|n| n.to_str()))
        .collect();
    names.sort_unstable();
    names
}

/// Does the bundle hold this path? The one honest way to check that a seeded
/// row points at something a screen can actually load.
///
/// Test-only: the serving path asks `DIST` for the file it was actually sent,
/// and a second production caller asking the same question a moment earlier
/// would be a check that can disagree with the answer.
#[cfg(test)]
pub(crate) fn bundle_holds(path: &str) -> bool {
    DIST.get_file(path).is_some()
}

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

/// How many LOOK templates in total the hub will hand a client on connect.
///
/// `MAX_CONTENT_LOOKS` is the bound on the GLOBAL per-kind map, and it is exact:
/// five kinds, one template each. Per-kind looks (DECISIONS §97) add a second
/// source of the same kind of id — one per (screen, kind) — and that product has
/// no exact bound, so this one is a JUDGEMENT and is written down as one.
///
/// Eight screens times five kinds is forty, and forty is the ceiling below. In
/// practice the number is far smaller, because the whole point of the feature is
/// that a church picks two or three looks and reuses them: the ids are
/// deduplicated before they are counted, so three looks across four screens is
/// three.
///
/// The bound is not defensiveness about a caller that cannot misbehave — it is
/// the property the design rests on. Every id here becomes a `template` frame in
/// every hello reply, and a template can carry an embedded `data:` image; one in
/// the field was 13 MB. A list that grew without limit would turn every OBS
/// reconnect into a bulk download over a church's wifi, which is exactly the cost
/// `main::cue_or_content_tpl` refuses to pay on the fire path, moved onto the
/// connect path where nobody is watching for it.
pub(crate) const MAX_LOOK_IDS: usize = 40;

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
            output_url(3, Some(1), "Main screen"),
            "output.html?channel=3&template_id=1&name=Main%20screen"
        );
    }

    /// DECISIONS §70: a screen with no look of its own says so BY SAYING NOTHING.
    ///
    /// `outputurl.js` was taught this and the native path was not, so `Copy URL`
    /// omitted the parameter for a follower while `open_output_window` and
    /// `auto_open_outputs` both wrote `template_id=1` through an `unwrap_or(1)`.
    /// The projector on HDMI and the OBS browser source in the same room were
    /// configured identically and behaved differently: the projector wore built-in
    /// 1 for the whole service, and nothing corrected it, because
    /// `channel://retemplate` only fires when the assignment CHANGES.
    ///
    /// Watched to fail by restoring `unwrap_or(1)` at either call site.
    #[test]
    fn a_screen_that_follows_carries_no_template_id() {
        let u = output_url(3, None, "Stream");
        assert!(
            !u.contains("template_id"),
            "a follower must not pin a look: {u}"
        );
        assert!(
            u.contains("channel=3"),
            "but it is still channel-keyed: {u}"
        );
    }

    #[test]
    fn output_url_escapes_specials() {
        let u = output_url(0, Some(2), "Stage/2");
        assert!(u.contains("name=Stage%2F2"), "got {u}");
    }

    /// RG-127. These two tests serve the real `dist/`, which is gitignored, so on a
    /// fresh clone they fail with `got HTTP/1.1 404 Not Found` — a message that says
    /// nothing about the frontend never having been built, and costs whoever reads
    /// it an hour in the HTTP server. The fix is the sentence, not the test: the
    /// pages genuinely are the built frontend and mocking that away would delete the
    /// thing being asserted.
    const NO_DIST: &str = "the page was not served. If this is a fresh clone or a \
        new worktree, `dist/` is gitignored and these two tests serve the REAL \
        built frontend: run `npm install && npm run build` first (RG-127).";

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
            "{NO_DIST} — got {}",
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
            "a request split across packets must still be served — {NO_DIST}"
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
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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

    /// The sibling of the cached-template test above, for the hub frame this task
    /// adds. `hello`'s reply sends several frames in sequence (template if cached,
    /// the configured default, the transition override, the retained screen
    /// frame); a bug in the new block's key, its position relative to the
    /// `.await` above it, or a forgotten `write.send` would leave every OTHER
    /// frame still arriving while this one silently never does — exactly the
    /// class of bug the cache/validate/classify unit tests below cannot see,
    /// because none of them opens a socket.
    #[tokio::test]
    async fn a_kiosk_client_receives_the_configured_default_on_hello() {
        let port = free_port();
        let hub = KioskHub::default();
        hub.cache_default_template(r#"{"id":7,"name":"House Look"}"#);
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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

        // Read frames until the default_template frame arrives (no template is
        // cached under id 7, so it is effectively first; the loop bound is
        // generous rather than exact, matching the cached-template test above).
        let mut got = false;
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await
            else {
                break;
            };
            let text = msg.into_text().unwrap();
            if text.contains(r#""kind":"default_template""#) {
                assert!(text.contains("House Look"), "got {text}");
                got = true;
                break;
            }
        }
        assert!(
            got,
            "the client never received the configured default template"
        );
    }

    /// THE CONTENT LOOKS ARE IN HAND BEFORE THE FRAME THEY DRESS.
    ///
    /// A content look reaches an output as `content.template_id` and NO JSON —
    /// deliberately, and it is a hard performance rule (`cue_or_content_tpl`: one
    /// look was 13 MB and serialising it per fire made verses take seconds). So
    /// the receiver can only resolve the number if it already holds the bytes,
    /// and a browser source has no database to find them in. Until this frame
    /// existed the id crossed the wire on both doors and died at both receivers:
    /// a screen set to "follow the content look" wore the configured default for
    /// the life of the product.
    ///
    /// TWO claims, and the second is the one a unit test on `cache_look_ids`
    /// could not make. The look must arrive, AND it must arrive BEFORE the
    /// retained screen frame — rule 43 sends what is on the screens last for
    /// exactly this reason, and a look sent after it would repaint a verse a
    /// congregation is already reading. So this opens a real socket, with a real
    /// retained verse behind it, and asserts on the ORDER the frames arrive in.
    #[tokio::test]
    async fn a_joining_screen_is_sent_the_content_looks_before_what_is_on_the_screens() {
        let port = free_port();
        let hub = KioskHub::default();
        hub.cache_template(9, r#"{"id":9,"name":"Scripture Look"}"#);
        hub.cache_template(11, r#"{"id":11,"name":"Lyric Look"}"#);
        hub.cache_look_ids(&[9, 11]);
        // A verse already on the wall, retained exactly as a real fire retains it.
        hub.publish(kiosk_content_json(&OutputContent {
            kind: Some("scripture".into()),
            reference: "Romans 8:28".into(),
            text: Some("And we know".into()),
            template_id: Some(9),
            ..Default::default()
        }));
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");
        let (mut write, mut read) = ws.split();
        // A FOLLOWER's hello: channel-keyed, no `template_id`, which is the URL
        // `Copy URL` produces for a screen with no look of its own (§70) and the
        // only configuration in which a content look applies to anything.
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"kind":"hello","channel":1,"template_id":null}"#.to_string(),
            ))
            .await
            .expect("send hello");

        let mut order: Vec<String> = Vec::new();
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_millis(600), read.next()).await
            else {
                break;
            };
            order.push(msg.into_text().unwrap());
        }
        let look_at = order
            .iter()
            .position(|m| m.contains(r#""kind":"template""#) && m.contains("Scripture Look"));
        let verse_at = order.iter().position(|m| m.contains(r#""kind":"content""#));
        assert!(
            look_at.is_some(),
            "a screen that follows the content look was never sent the look: {order:?}"
        );
        assert!(
            order
                .iter()
                .any(|m| m.contains(r#""kind":"template""#) && m.contains("Lyric Look")),
            "only one of the two looks arrived: {order:?}"
        );
        assert!(
            verse_at.is_some(),
            "the retained verse never arrived, so the ordering claim below would be vacuous: {order:?}"
        );
        assert!(
            look_at < verse_at,
            "the look arrived AFTER the verse it dresses — a screen joining \
             mid-reading paints the wrong template and then repaints: {order:?}"
        );
    }

    /// A CHANNEL-KEYED CLIENT IS SENT ITS OWN SCREEN'S TEMPLATE ON HELLO.
    ///
    /// **THE HOLE RULE 43 LEFT, ONE FACT OVER, IN THE SAME HANDSHAKE.** The hello
    /// reply's whole template branch used to sit inside
    /// `if let Some(id) = template_id`, and `Output.svelte` sends
    /// `template_id: null` whenever the URL is CHANNEL-keyed — which is the URL
    /// `Copy URL` produces and the one CLAUDE.md tells operators to use, because
    /// only a channel-keyed source follows a template swap. So the recommended URL
    /// was the one shape that received no screen template at all: `channelTpl`
    /// stayed null on the client, `resolveOutputTemplate(null, …)` returned the
    /// content look or the configured default, and every screen in the building
    /// painted the same template however carefully the operator had assigned them.
    ///
    /// Measured against the running backend before this was written: `?channel=3`
    /// painted the verse at 32px Fraunces centred, `?channel=3&template_id=68`
    /// painted the same verse at 15.97px Inter left, on the same screen with the
    /// same content. A screen only ever learned its look from a live
    /// `set_channel_template` broadcast while it happened to be connected, which
    /// is why it looked fine in testing and was wrong on every reconnect and every
    /// cold start.
    ///
    /// **ONE CHANNEL'S TEMPLATE, NEVER A MAP OF ALL OF THEM.** The role map and
    /// the look map ship whole because they are ids; a template is bytes, and one
    /// in the field was 13 MB (`main::cue_or_content_tpl`). The hub retains the
    /// frame per channel and sends the one this client asked to be.
    #[tokio::test]
    async fn a_channel_keyed_client_is_sent_its_own_screen_template_on_hello() {
        let port = free_port();
        let hub = KioskHub::default();
        // Three screens with three different looks, as a real install has.
        hub.cache_channel_template(3, r#"{"id":68,"name":"Source · Word right"}"#);
        hub.cache_channel_template(4, r#"{"id":44,"name":"Song · Chorus"}"#);
        hub.cache_channel_template(5, r#"{"id":39,"name":"Scripture · Meridian"}"#);
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");
        let (mut write, mut read) = ws.split();
        // EXACTLY WHAT `Copy URL` PRODUCES: a channel and a null template id.
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"kind":"hello","channel":3,"template_id":null}"#.to_string(),
            ))
            .await
            .expect("send hello");

        let mut order: Vec<String> = Vec::new();
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_millis(600), read.next()).await
            else {
                break;
            };
            order.push(msg.into_text().unwrap());
        }
        let mine = order
            .iter()
            .find(|m| m.contains(r#""kind":"channel_template""#))
            .unwrap_or_else(|| {
                panic!(
                    "the URL every operator is told to use was sent no screen \
                     template at all, so this screen paints the content look or the \
                     configured default whatever the operator assigned it: {order:?}"
                )
            });
        assert!(
            mine.contains("Source · Word right") && mine.contains(r#""channel":3"#),
            "got {mine}"
        );
        assert!(
            !order
                .iter()
                .any(|m| m.contains("Song · Chorus") || m.contains("Scripture · Meridian")),
            "every screen's template was sent to one client — a template is BYTES, \
             and one in the field was 13 MB: {order:?}"
        );
    }

    /// A CHANNEL-KEYED CLIENT IS SENT ITS OWN SCREEN'S `shows` SET ON HELLO.
    ///
    /// DECISIONS §98, at the level this file has now missed twice: the map is
    /// sent on EVERY hello including `{}`, and it is sent BEFORE the retained
    /// screen frame it can suppress. A screen sent the frame first paints a
    /// countdown the operator took off it and then drops it, in front of a
    /// congregation.
    ///
    /// The whole map goes, unlike the per-channel template beside it, because
    /// this is a handful of short strings rather than bytes — the same reasoning
    /// as `channel_roles` and `channel_looks`, and each client filters for its own
    /// id at the receiver because the hub cannot address one (§35).
    #[tokio::test]
    async fn a_client_that_connects_is_told_which_kinds_its_screen_shows() {
        let port = free_port();
        let hub = KioskHub::default();
        hub.cache_channel_shows(r#"{"1":["scripture","song"]}"#);
        hub.publish(kiosk_content_json(&OutputContent {
            kind: Some("scripture".into()),
            reference: "Romans 8:28".into(),
            text: Some("And we know".into()),
            ..Default::default()
        }));
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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
                r#"{"kind":"hello","channel":1,"template_id":null}"#.to_string(),
            ))
            .await
            .expect("send hello");

        let mut order: Vec<String> = Vec::new();
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_millis(600), read.next()).await
            else {
                break;
            };
            order.push(msg.into_text().unwrap());
        }
        let at = order
            .iter()
            .position(|m| m.contains(r#""kind":"channel_shows""#))
            .unwrap_or_else(|| {
                panic!(
                    "a screen that joined was never told which kinds it shows, so it \
                     paints every kind its template allows — which is the state that \
                     put a countdown on every screen in the building: {order:?}"
                )
            });
        assert!(
            order[at].contains(r#""1":["scripture","song"]"#),
            "the set arrived empty over a hub that has one: {}",
            order[at]
        );
        let verse_at = order
            .iter()
            .position(|m| m.contains(r#""kind":"content""#))
            .expect("the retained verse never arrived, so the ordering claim is vacuous");
        assert!(
            at < verse_at,
            "the set arrived AFTER the frame it can suppress — a screen joining \
             mid-service paints a kind it was configured out of, then drops it: \
             {order:?}"
        );
    }

    /// AN UNPARSEABLE `shows` MAP BECOMES `{}` — NO OPINION, NEVER AN EMPTY SET.
    ///
    /// The asymmetry is the whole point and it runs one way only. `{}` means no
    /// screen has an opinion, so every screen follows its template, which is where
    /// this decision lived before the column existed. The other direction — an
    /// empty LIST arrived at by accident — is a congregation screen that paints
    /// nothing for the rest of a service with nothing on it able to say why.
    #[test]
    fn a_shows_map_that_is_not_an_object_becomes_no_opinion() {
        let hub = KioskHub::default();
        hub.cache_channel_shows("{not json");
        assert_eq!(hub.channel_shows_json(), "{}");
        hub.cache_channel_shows(r#"["scripture"]"#);
        assert_eq!(hub.channel_shows_json(), "{}");
        hub.cache_channel_shows(r#"{"1":["scripture"]}"#);
        assert_eq!(hub.channel_shows_json(), r#"{"1":["scripture"]}"#);
    }

    /// THE LOOK MAP IS NOT A SCREEN FRAME.
    ///
    /// Asserted against the slot rather than only against the matcher, because
    /// the matcher test one screen up can only see the string it is handed and
    /// this is a claim about what `publish` DID with it. `last_screen` holds ONE
    /// frame and the newest wins, so a configuration frame retained there does
    /// not sit beside the verse — it REPLACES it, and the next screen to join
    /// mid-reading is handed a map of looks over a blank wall. That is rule 43's
    /// own failure delivered by rule 43's own mechanism, and it is the fourth
    /// time in this module that a configuration frame has had to be kept out of
    /// this slot.
    #[test]
    fn the_look_map_is_not_a_screen_frame() {
        let hub = KioskHub::default();
        hub.publish(kiosk_content_json(&OutputContent {
            kind: Some("scripture".into()),
            reference: "Romans 8:28".into(),
            text: Some("And we know".into()),
            ..Default::default()
        }));
        hub.set_channel_looks(r#"{"1":{"scripture":9}}"#);
        let retained = hub
            .last_screen_handle()
            .lock()
            .ok()
            .and_then(|l| l.clone())
            .expect("the verse must still be the retained screen frame");
        assert!(
            retained.contains(r#""kind":"content""#),
            "setting a per-kind look replaced what is on the screens: {retained}"
        );
        // …and it IS retained, in its own slot, or a screen that joins after it
        // would never learn what it wears.
        assert_eq!(hub.channel_looks_json(), r#"{"1":{"scripture":9}}"#);
    }

    /// AN UNPARSEABLE LOOK MAP BECOMES `{}` RATHER THAN A BROKEN FRAME.
    ///
    /// The value is embedded RAW into a WS frame, and one unparseable frame stops
    /// a client applying every frame after it — including the retained verse that
    /// arrives later in the same hello reply. `{}` is the safe reading: every
    /// screen falls through to its own template, which is where it was before this
    /// feature existed. Same rule and same reason as `cache_channel_roles`.
    #[test]
    fn a_look_map_that_is_not_an_object_becomes_the_empty_one() {
        let hub = KioskHub::default();
        hub.cache_channel_looks("{not json");
        assert_eq!(hub.channel_looks_json(), "{}");
        hub.cache_channel_looks(r#"[{"scripture":9}]"#);
        assert_eq!(hub.channel_looks_json(), "{}");
        hub.cache_channel_looks(r#"{"1":{"scripture":9}}"#);
        assert_eq!(hub.channel_looks_json(), r#"{"1":{"scripture":9}}"#);
    }

    /// A CLIENT THAT CONNECTS MID-SERVICE IS SENT THE LOOK MAP BEFORE THE VERSE.
    ///
    /// The ordering rule of rule 43, on the third thing a screen needs in hand
    /// before the frame it dresses. A per-kind look reaches a screen as an ID; the
    /// bytes ride in the `template` frames and the CHOICE rides in this map, so a
    /// screen sent the retained verse first resolves it against a map it has not
    /// been given, paints its blanket template, and then repaints a moment later
    /// in front of a congregation.
    ///
    /// Sent unconditionally, `{}` included — which is why the assertion is on a
    /// hub whose map IS set: a test that only proved `{}` arrives would pass on a
    /// hub that had stopped reading the slot at all.
    #[tokio::test]
    async fn a_client_that_connects_mid_service_is_sent_the_look_map_before_the_verse() {
        let port = free_port();
        let hub = KioskHub::default();
        hub.cache_channel_looks(r#"{"1":{"scripture":9}}"#);
        hub.cache_template(9, r#"{"id":9,"name":"Scripture Look"}"#);
        hub.cache_look_ids(&[9]);
        hub.publish(kiosk_content_json(&OutputContent {
            kind: Some("scripture".into()),
            reference: "Romans 8:28".into(),
            text: Some("And we know".into()),
            template_id: Some(9),
            ..Default::default()
        }));
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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
                r#"{"kind":"hello","channel":1,"template_id":null}"#.to_string(),
            ))
            .await
            .expect("send hello");

        let mut order: Vec<String> = Vec::new();
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_millis(600), read.next()).await
            else {
                break;
            };
            order.push(msg.into_text().unwrap());
        }
        let looks_at = order
            .iter()
            .position(|m| m.contains(r#""kind":"channel_looks""#));
        let bytes_at = order
            .iter()
            .position(|m| m.contains(r#""kind":"template""#) && m.contains("Scripture Look"));
        let verse_at = order.iter().position(|m| m.contains(r#""kind":"content""#));
        let at = looks_at.unwrap_or_else(|| {
            panic!(
                "a screen that joined was never told what it wears for each kind, \
                 so it can only ever paint its blanket template: {order:?}"
            )
        });
        assert!(
            order[at].contains(r#""1":{"scripture":9}"#),
            "the map arrived empty over a hub that has one: {}",
            order[at]
        );
        let verse_at = verse_at.expect(
            "the retained verse never arrived, so the ordering claim below would be vacuous",
        );
        let bytes_at =
            bytes_at.expect("the look's bytes never arrived, so the id could resolve to nothing");
        assert!(
            at < verse_at && bytes_at < verse_at,
            "the look map or its bytes arrived AFTER the verse they dress — a \
             screen joining mid-reading paints its blanket template and then \
             repaints in front of a congregation: {order:?}"
        );
    }

    /// A SLIDE ON THE PREACHER'S SCREEN CARRIES THE INSTANT IT WAS SENT (RG-220).
    ///
    /// The operator asked for *"all media in sync"* and chose the two things
    /// Relay can honestly do: every screen starts together and is corrected when
    /// it drifts. The correction is each screen's own and needs one fact from
    /// here — when the clip started, on Relay's clock. The page cannot infer it:
    /// it knows when the FRAME arrived, which is a fact about the network.
    ///
    /// Taking the slide DOWN carries no instant, because there is nothing to be
    /// in sync with and a stamp on an absence is a claim about nothing.
    #[test]
    fn a_stage_slide_says_when_relay_sent_it_and_a_removal_does_not() {
        let up: serde_json::Value = serde_json::from_str(&stage_media_frame_json(
            Some("http://x/media/7"),
            Some("video"),
        ))
        .unwrap();
        let at = up["started_at"]
            .as_i64()
            .expect("no instant on a stage slide");
        // A real clock, not a zero: `now_epoch_ms` falls back to 0 only before
        // the UNIX epoch, and a 0 here would sync every screen to 1970.
        assert!(
            at > 1_600_000_000_000,
            "started_at is not a wall clock: {at}"
        );

        let down: serde_json::Value =
            serde_json::from_str(&stage_media_frame_json(None, None)).unwrap();
        assert!(
            down["started_at"].is_null(),
            "taking a slide down claimed an instant: {down}"
        );
    }

    /// AND SO DOES A CLIP ON THE CONGREGATION'S SCREENS.
    ///
    /// The same fact by the other door. `kiosk_content_json` is the wire form
    /// every browser source receives, and a field missing from it is the exact
    /// bug this function's own doc comment records about `next_reference`.
    #[test]
    fn a_kiosk_content_frame_carries_the_instant_the_clip_started() {
        let content = OutputContent {
            kind: Some("media".into()),
            media_url: Some("http://x/media/7".into()),
            media_kind: Some("video".into()),
            media_started_at: Some(1_700_000_000_000),
            ..Default::default()
        };
        let v: serde_json::Value = serde_json::from_str(&kiosk_content_json(&content)).unwrap();
        assert_eq!(v["media_started_at"], 1_700_000_000_000i64);

        // A verse has no clip, so it claims no instant — `syncSeek` corrects
        // nothing without one, and a stamp here would be a fact about nothing.
        let verse = OutputContent {
            kind: Some("scripture".into()),
            reference: "John 3:16".into(),
            ..Default::default()
        };
        let v: serde_json::Value = serde_json::from_str(&kiosk_content_json(&verse)).unwrap();
        assert!(
            v["media_started_at"].is_null(),
            "a verse claimed a clip start"
        );
    }

    /// THE LIST IS A BOUND, NOT A CACHE.
    ///
    /// Every id here becomes a `template` frame in every hello reply, and a
    /// template can carry an embedded `data:` image. An unbounded list turns each
    /// reconnect — an OBS source restarting, a lobby TV losing the wifi — into a
    /// bulk download over a church's network, which is the cost the fire path
    /// refuses to pay, moved somewhere nobody is watching.
    #[test]
    fn the_content_look_list_is_deduplicated_and_capped() {
        let hub = KioskHub::default();
        // Two kinds pointing at one template is ordinary configuration, not a bug:
        // scripture and announcements often wear the same look.
        hub.cache_look_ids(&[9, 11, 9]);
        assert_eq!(hub.look_ids(), vec![9, 11]);
        let too_many: Vec<i64> = (1..=(MAX_LOOK_IDS as i64 + 7)).collect();
        hub.cache_look_ids(&too_many);
        assert_eq!(
            hub.look_ids().len(),
            MAX_LOOK_IDS,
            "a list longer than a hello reply will carry was sent whole"
        );
    }

    /// WHAT THIS SCREEN IS FOR, ON EVERY HELLO — including when the answer is
    /// "nothing".
    ///
    /// A browser source has no database, so this is the only way `output.html`
    /// can learn its own channel's role, and the only thing downstream of that is
    /// whether a Stage Message may be painted. The filter has to live at the
    /// receiver because the hub cannot address one client: it records nothing
    /// about who connected and DECISIONS §35 is not being reversed.
    ///
    /// Sent unconditionally, `{}` included. A page that never receives this
    /// cannot tell "no screen has a role" from "the reply has not come yet", and
    /// the two have to differ: only one of them will ever accept a message.
    #[tokio::test]
    async fn a_kiosk_client_is_told_what_every_screen_is_for_on_hello() {
        let port = free_port();
        let hub = KioskHub::default();
        hub.cache_channel_roles(r#"{"1":"main","2":"stage"}"#);
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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

        let mut got = None;
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await
            else {
                break;
            };
            let text = msg.into_text().unwrap();
            if text.contains(r#""kind":"channel_roles""#) {
                got = Some(text);
                break;
            }
        }
        let text = got.expect(
            "a client that joins is never told what its own screen is for, so it can \
             never accept a stage message",
        );
        assert!(
            text.contains(r#""2":"stage""#) && text.contains(r#""1":"main""#),
            "got {text}"
        );
    }

    #[test]
    fn the_role_map_carries_ids_and_roles_and_nothing_else() {
        // DECISIONS §35 is not being reversed. This frame goes to every client on
        // the LAN, so what it may contain is exactly what the Outputs desk already
        // shows: which channel holds which role. No names, no addresses, nothing a
        // client chose, nothing about who is connected.
        let hub = KioskHub::default();
        hub.cache_channel_roles(r#"{"1":"main","2":"stage"}"#);
        assert_eq!(hub.channel_roles_json(), r#"{"1":"main","2":"stage"}"#);
        let mut rx = hub.sender().subscribe();
        hub.set_channel_roles(r#"{"2":"stage"}"#);
        let frame = rx.try_recv().expect("the change is published");
        assert_eq!(frame, r#"{"kind":"channel_roles","roles":{"2":"stage"}}"#);
    }

    #[test]
    fn a_malformed_role_map_degrades_to_no_roles_rather_than_breaking_the_frame() {
        // The blob is embedded RAW into a WS frame, exactly like the default
        // template — one unparseable frame stops a client applying every frame
        // after it. `{}` is the safe reading, because every filter downstream asks
        // whether a role IS `stage`, so degrading refuses rather than admits.
        let hub = KioskHub::default();
        hub.cache_channel_roles("{not json");
        assert_eq!(hub.channel_roles_json(), "{}");
        // A valid JSON value that is not an object is the same failure wearing
        // better clothes: `roles[id]` on an array or a string is not a role.
        hub.cache_channel_roles(r#"["stage"]"#);
        assert_eq!(hub.channel_roles_json(), "{}");
    }

    #[test]
    fn the_configured_default_is_sent_to_a_screen_that_joins_later() {
        // A BROWSER SOURCE HAS NO DATABASE. The configured default template is a
        // settings row, so the only way a kiosk or OBS client can end its
        // resolution chain at the operator's default — rather than at the bundled
        // Classic Serif — is for the hub to carry it. Cached without a push at
        // startup, pushed when it changes, and replayed on hello, exactly like the
        // per-template cache beside it.
        let hub = KioskHub::default();
        hub.cache_default_template(r#"{"id":7,"name":"House Look"}"#);
        assert_eq!(
            hub.default_template_json(),
            r#"{"id":7,"name":"House Look"}"#
        );
    }

    #[test]
    fn a_malformed_default_degrades_to_null_rather_than_breaking_the_frame() {
        // The blob is embedded RAW into a WS frame, so anything that is not valid
        // JSON would produce a frame no client can parse — and a client that fails
        // to parse one frame is a screen that stops applying every frame after it.
        let hub = KioskHub::default();
        hub.cache_default_template("{not json");
        assert_eq!(hub.default_template_json(), "null");
    }

    #[test]
    fn the_default_template_frame_is_configuration_not_a_screen_frame() {
        // Rule: only `content`, `clear` and `black` decide what a screen is
        // SHOWING and are retained as the screen frame (rule 43). The default
        // template paints nothing on its own — it tells a screen what to wear when
        // nothing else answers — so retaining it would let it stand in for the
        // verse a late-joining screen is owed.
        assert!(!is_screen_frame(
            r#"{"kind":"default_template","template":null}"#
        ));
    }

    /// The retained-frame test above can only be trusted if the matcher agrees
    /// with what this module actually SERIALISES. It did not: `serde_json`'s map is
    /// a BTreeMap, so a content frame starts `{"content_kind":…` and the first
    /// version of `is_screen_frame` (a `starts_with`) matched none of them — the
    /// retained frame stayed `None` and a joining screen was still blank.
    #[test]
    fn the_screen_frame_matcher_agrees_with_what_is_published() {
        let content = kiosk_content_json(&OutputContent {
            kind: Some("scripture".into()),
            reference: "Romans 8:28".into(),
            text: Some("And we know that all things".into()),
            ..Default::default()
        });
        assert!(
            is_screen_frame(&content),
            "a real content frame is not recognised: {content}"
        );
        assert!(is_screen_frame(r#"{"kind":"clear"}"#));
        assert!(is_screen_frame(r#"{"kind":"black"}"#));
        // Not a frame that decides what is on the screen.
        assert!(!is_screen_frame(
            r#"{"kind":"stage_next","label":"John 3:17"}"#
        ));
        // An instruction to a person, about a moment. A tablet that rejoins ten
        // minutes later must not be handed it — and it must not stand in for the
        // content the screen is actually showing either.
        assert!(!is_screen_frame(
            r#"{"kind":"stage_alert","text":"two minutes"}"#
        ));
        assert!(!is_screen_frame(
            r#"{"kind":"template","id":1,"template":{}}"#
        ));
        // Configuration and a look. Both are retained, each in its own slot and
        // replayed on hello from there; neither may be retained as THE screen
        // frame, because that slot holds one message and the newest wins — a
        // preference or a template would replace the verse and the next screen to
        // join would be sent it over a blank wall. Both were in FRAME_VERDICTS
        // and neither was asserted here, which is the half of the pair that
        // checks the matcher rather than the list.
        assert!(!is_screen_frame(r#"{"kind":"transition","mode":"cut"}"#));
        assert!(!is_screen_frame(
            r#"{"kind":"channel_template","channel_id":1,"template":{}}"#
        ));
        // And the third of that family. A screen that joins late is owed the
        // verse, not a map of what every screen is for.
        assert!(!is_screen_frame(
            r#"{"kind":"channel_roles","roles":{"1":"main","2":"stage"}}"#
        ));
        // …and the fourth. A per-kind look map is what a screen WEARS, never what
        // it is SHOWING, so retaining it in the one screen slot would replace the
        // verse and hand the next screen to join a map of looks over a blank wall.
        assert!(!is_screen_frame(
            r#"{"kind":"channel_looks","looks":{"1":{"scripture":9}}}"#
        ));
        // …and the fifth. A screen's `shows` set decides whether a frame paints
        // here; it is not itself a frame that paints.
        assert!(!is_screen_frame(
            r#"{"kind":"channel_shows","shows":{"1":["scripture"]}}"#
        ));
    }
    /// A CLIP'S POSITION IS ONLY WORTH WHAT THE BEAT CARRYING IT IS WORTH.
    ///
    /// Requirement 11. The console must never time a clip off its own preview —
    /// its programme pane holds a second player of the same file, and if the
    /// wall's copy stalls the console's carries on. So the figure comes off the
    /// beat, and a stale beat's figure is refused with the rest of it.
    #[test]
    fn a_stale_beat_says_nothing_about_a_clip() {
        let h = OutputHealth::default();
        h.beat(
            4,
            PaintState::Content,
            "kiosk",
            BeatGap::default(),
            Some(MediaBeat {
                pos_ms: 12_000,
                dur_ms: 240_000,
                paused: false,
            }),
        );
        assert_eq!(h.media_of(4).map(|m| m.pos_ms), Some(12_000));
        // And a channel nobody has reported for says nothing rather than zero.
        assert_eq!(h.media_of(99), None);
    }

    /// ZERO IS NOT A CLIP THAT TAKES NO TIME.
    ///
    /// It is a player that has not loaded one yet. "0:00 left" over a clip that has
    /// barely started is worse than saying nothing, because the operator acts on
    /// it — that figure is what the next cue is timed against.
    #[test]
    fn a_report_with_no_duration_is_dropped_whole() {
        let j = |s: &str| serde_json::from_str::<serde_json::Value>(s).expect("json");
        assert_eq!(
            MediaBeat::from_json(&j(r#"{"media_pos_ms":0,"media_dur_ms":0}"#)),
            None
        );
        assert_eq!(MediaBeat::from_json(&j(r#"{"media_pos_ms":10}"#)), None);
        assert_eq!(MediaBeat::from_json(&j(r#"{}"#)), None);
        // And the same rule through the other transport, because a window and a
        // browser source must not disagree about the same clip.
        assert_eq!(MediaBeat::clamped(Some(0), Some(0), None), None);
        assert_eq!(MediaBeat::clamped(Some(10), None, None), None);
    }

    /// A NUMBER OFF THE LAN IS STILL UNTRUSTED INPUT — and a position past the end
    /// of its own clip is the shape that reaches an operator as a negative
    /// countdown.
    #[test]
    fn a_position_past_the_end_is_clamped_to_the_end() {
        let j = |s: &str| serde_json::from_str::<serde_json::Value>(s).expect("json");
        let m = MediaBeat::from_json(&j(r#"{"media_pos_ms":999999,"media_dur_ms":5000}"#))
            .expect("a clip with a real duration");
        assert_eq!(m.pos_ms, 5_000);
        assert_eq!(
            MediaBeat::clamped(Some(999_999), Some(5_000), None).map(|m| m.pos_ms),
            Some(5_000)
        );
        // Beyond a day is a broken clock or a hostile client, and neither is
        // evidence about a service. The same clamp `BeatGap` already applies.
        assert_eq!(
            MediaBeat::clamped(Some(0), Some(GAP_CLAMP_MS + 1), None),
            None
        );
    }

    /// Every `kind` this module publishes, and whether it decides what a screen
    /// is SHOWING. `true` here means the hub retains it and replays it to a
    /// client that joins late (rule 43).
    /// HOW MANY FRAMES A TEST READS OFF A HELLO REPLY BEFORE IT GIVES UP.
    ///
    /// **ONE NUMBER, BECAUSE SIXTEEN HAND-WRITTEN ONES IS SIXTEEN THINGS THAT GO
    /// STALE TOGETHER.** Every reader here loops until its own budget runs out OR
    /// the socket goes quiet, and the quiet is what normally ends it — so a
    /// generous budget costs nothing and a tight one is a time bomb. The bomb went
    /// off twice in one afternoon: `channel_looks` and then `channel_shows` joined
    /// the configuration block, and four tests that had nothing to do with either
    /// began reporting *"a screen that reconnected mid-reading was left blank"* —
    /// rule 43's own failure message, over a hub that was replaying the frame
    /// correctly and a reader that stopped one frame short of it.
    ///
    /// It must stay comfortably above the size of the whole hello reply, which is
    /// the configuration block plus everything retained. Raise it when that grows;
    /// never lower it to make a test "tighter", because a tight budget here does
    /// not assert anything — it only changes which bug the failure message names.
    const HELLO_FRAMES: usize = 24;

    const FRAME_VERDICTS: &[(&str, bool)] = &[
        ("content", true),
        ("clear", true),
        ("black", true),
        ("stage_next", false),
        ("stage_alert", false),
        // The preacher's own slide. Not a screen frame: it has its own slot, so
        // retaining it here would replace the verse and hand the next screen to
        // join a picture over a blank wall. Scripture overrides it on the DEVICE,
        // which is a rule about painting and not about retention.
        ("stage_media", false),
        // What the clip is DOING. Not a screen frame: it has its own slot, and
        // retaining it here would replace the verse with a pause instruction and
        // hand the next screen to join a blank wall.
        ("media_transport", false),
        ("template", false),
        // Configuration, not content. It is retained — in its OWN slot, and
        // replayed on hello from there — because a screen that joins late must not
        // be the only one still cutting. It must never be retained HERE: one slot
        // means the newest frame wins, so a transition would replace the verse and
        // the next screen to join would be sent a preference and a blank wall.
        ("transition", false),
        // A screen's own look, pushed from main.rs when the operator reassigns it.
        // Not retained HERE: `last_screen` holds one frame and the newest wins, so
        // retaining a template would replace the verse and the next screen to join
        // would be sent a look and a blank wall. The hub keeps templates in their
        // own per-id cache (`cache_template`) and replays them on hello from there.
        ("channel_template", false),
        // The operator's configured default. Not retained HERE, for the same
        // reason as `transition` and `channel_template`: `last_screen` holds one
        // frame and the newest wins, so retaining it would replace the verse and
        // the next screen to join would be sent a look and a blank wall. The hub
        // keeps it in its own slot (`default_tpl`) and replays it on hello from
        // there.
        ("default_template", false),
        // WHAT EACH SCREEN IS FOR. Configuration again, and not retained HERE for
        // the third time for the third identical reason: `last_screen` holds one
        // frame and the newest wins. It has its own slot (`channel_roles`) and is
        // replayed on hello from there, on EVERY hello — a page that does not know
        // its role cannot tell "I am not the stage" from "nobody has told me yet",
        // and what hangs off that distinction is whether a word meant for the
        // platform gets painted.
        ("channel_roles", false),
        // WHAT EACH SCREEN WEARS FOR EACH KIND. Configuration for the fourth time
        // and not retained HERE for the fourth identical reason: `last_screen`
        // holds one frame and the newest wins, so retaining it would replace the
        // verse and the next screen to join would be handed a map of looks over a
        // blank wall. It has its own slot (`channel_looks`) and is replayed on
        // hello from there, on EVERY hello — `{}` is an answer, and a page that
        // cannot tell "nobody has told me" from "I have no per-kind look" resolves
        // the wrong template for the one frame that matters, which is the first
        // one after it reconnects mid-service. It is sent with the configuration
        // and BEFORE the retained screen frame: a look must be in hand before the
        // frame it dresses.
        ("channel_looks", false),
        // WHICH KINDS EACH SCREEN SHOWS AT ALL (DECISIONS §98). Configuration for
        // the fifth time and not retained HERE for the fifth identical reason:
        // `last_screen` holds one frame and the newest wins. It has its own slot
        // (`channel_shows`) and is replayed on hello from there, on EVERY hello and
        // BEFORE the retained screen frame — because it decides whether that frame
        // paints on this screen at all, and sent after it a screen the operator has
        // taken the countdown off would paint one and then drop it.
        //
        // It is the one frame in this list that can stop a fire reaching a screen,
        // which is why it is worth saying what it can NOT stop: it narrows CONTENT,
        // it is never consulted for `clear` or `black`, and a screen an operator
        // could configure out of a panic control is rule 15's exact failure.
        ("channel_shows", false),
        // The programme timers a stage tablet is showing. Not retained HERE, for
        // the same reason as the three above: `last_screen` holds one frame and the
        // newest wins, so retaining a clock would replace the verse and the next
        // screen to join would be sent the programme over a blank wall. It IS
        // retained — in its own slot (`last_timers`), replayed on hello from there,
        // and sent BEFORE the screen frame so the reading is painted last.
        ("timer", false),
        // THE PICTURE EVERYTHING IS PAINTED ON. Not retained HERE, for the fifth
        // time for the fifth identical reason: `last_screen` holds one frame and
        // the newest wins, so a backdrop retained beside the verse would replace
        // it and the next screen to join mid-reading would be handed wallpaper and
        // no words. It IS retained — in its own slot (`last_background`), replayed
        // on hello from there, and sent BEFORE the screen frame so the reading is
        // painted last. **It is the one slot a panic control reaches into**: a
        // `clear` or a `black` empties it at this same door, so the replay can no
        // more resurrect a background than it can resurrect a verse.
        ("background", false),
        // WHICH SCREENS THE OPERATOR HAS TAKEN OUT OF THE WALL. Not retained HERE,
        // for the fifth time for the fifth identical reason: `last_screen` holds
        // one frame and the newest wins, so retaining this would replace the verse
        // and the next screen to join would be sent a map of what is down over a
        // blank wall. It IS retained — in its own slot (`screens_down`), replayed
        // on hello from there, and sent AFTER the screen frame rather than before
        // it, because it overrides what is on the screens rather than being what
        // is on them.
        ("screen_state", false),
        // NOT A PUBLISHED FRAME AT ALL, AND THAT IS THE VERDICT.
        //
        // Every other row here answers "should a screen that joins late be
        // shown this?". `beat_ack` does not reach the hub: it is written
        // straight to the one socket whose `beat` prompted it, the same way the
        // hello reply is, so there is nothing to retain and nobody to replay it
        // to. Retaining it would be meaningless twice over — it carries a host
        // timestamp that was true when a DIFFERENT client reported, and a late
        // joiner gets its own within two seconds by beating itself.
        //
        // It is listed rather than excused because the scanner reads source
        // literals, and a kind with no row is the finding this test exists for.
        ("beat_ack", false),
        // WHICH LAYOUT EACH STAGE SCREEN WEARS. Not retained, and this one is
        // the exception that has to justify itself, because unlike `beat_ack`
        // it IS durable configuration and rule 43 would ordinarily replay it.
        //
        // `stage.html` is its only consumer and the only page with an HTTP
        // control plane, so it READS this map from `GET /api/stage_zones` when
        // it connects and this frame exists only to carry a LIVE change to a
        // screen already open. Retaining it as well would be a second source of
        // one fact, and the one that a reconnecting page does not consult.
        //
        // The cost of that trade is real: a screen whose HTTP read fails falls
        // back to the device's own zones rather than to the assigned layout.
        // That is the safe direction — a working screen, not a blank one.
        ("stage_zones", false),
    ];

    /// THE ENUMERATION MUST GROW WITH THE MODULE, OR IT IS NOT AN ENUMERATION.
    ///
    /// The test above lists the kinds it knows about, and a list of examples
    /// cannot notice a kind nobody added to it. This one reads the module's own
    /// source and fails on any published `kind` with no verdict — which is the
    /// case that actually arose: `stage_alert` was added on `new_look_refresh`
    /// while `is_screen_frame` was being written on `audit/field-2026-09-13`, and
    /// the two met for the first time in a merge. The matcher happened to be
    /// right about it; nothing was checking.
    ///
    /// Same shape as `r6-contracts.test.js` on the client side: a new hub message
    /// that nobody has answered for is the finding.
    #[test]
    fn every_kind_this_module_publishes_has_an_explicit_verdict() {
        /// Every `"kind":"…"` literal in `src`, in order, without duplicates.
        /// Comment lines talk ABOUT frames without publishing any.
        ///
        /// It finds SOURCE LITERALS only. A frame built from a type carrying
        /// `#[serde(tag = "kind")]` produces no `"kind"` literal anywhere in this
        /// file, so it would need no verdict and none of this would notice — the
        /// scanner would stay green while the enumeration stopped being one.
        fn kinds_in(src: &str, out: &mut Vec<String>) {
            for line in src.lines() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                let mut rest = line;
                while let Some(i) = rest.find("\"kind\"") {
                    rest = &rest[i + "\"kind\"".len()..];
                    let Some(after) = rest.trim_start().strip_prefix(':') else {
                        continue;
                    };
                    let Some(after) = after.trim_start().strip_prefix('"') else {
                        continue;
                    };
                    let Some(end) = after.find('"') else { continue };
                    let kind = after[..end].to_string();
                    if !out.contains(&kind) {
                        out.push(kind);
                    }
                }
            }
        }

        let chan = include_str!("channels.rs");
        let mut found: Vec<String> = Vec::new();
        // channels.rs strips its own tests: its `mod tests` is full of example
        // frames that are not published by the module.
        kinds_in(chan.split("mod tests").next().unwrap_or(chan), &mut found);
        // main.rs is scanned WHOLE. It carries `#[cfg(test)]` from line 16, so no
        // split can separate its tests, and it holds exactly one frame literal.
        kinds_in(include_str!("main.rs"), &mut found);

        assert!(
            !found.is_empty(),
            "the scanner found no published kind at all — it has stopped reading \
             this module, and a scanner that quietly narrows passes everything"
        );
        for kind in &found {
            assert!(
                FRAME_VERDICTS.iter().any(|(k, _)| *k == kind.as_str()),
                "`{kind}` is published to the kiosk hub and no one has said whether \
                 a screen that joins late should be shown it. Add it to \
                 FRAME_VERDICTS with a reason, and assert it in the matcher test."
            );
        }
        for (kind, retained) in FRAME_VERDICTS {
            assert!(
                found.iter().any(|f| f == kind),
                "FRAME_VERDICTS names `{kind}`, which this module no longer \
                 publishes — a verdict about nothing"
            );
            let frame = format!(r#"{{"kind":"{kind}","x":1}}"#);
            assert_eq!(
                is_screen_frame(&frame),
                *retained,
                "the matcher disagrees with the verdict for `{kind}`"
            );
        }
    }

    /// Every function that can put something on a LAN device, and whether a
    /// rehearsal must stop it.
    ///
    /// `true` means the function checks `rehearsing(app)` and returns early.
    /// `false` means it deliberately does not, and the third column is why — the
    /// same reason that must be at the call site.
    const REHEARSAL_VERDICTS: &[(&str, bool, &str)] = &[
        (
            "broadcast_content",
            true,
            "it carries what a congregation reads",
        ),
        ("clear", true, "a rehearsal must not take a real wall down"),
        ("black", true, "a rehearsal must not black a real wall"),
        (
            "stage_next",
            true,
            "it leaked 'up next' to a live stage tablet mid-rehearsal, and it has \
             no Tauri emit, so the e2e wall test saw nothing wrong",
        ),
        (
            "stage_alert",
            true,
            "a word to the preacher is for a person, and a rehearsal has no person \
             waiting for it",
        ),
        (
            "media_transport",
            false,
            "it changes what is ALREADY on a screen rather than putting something \
             there, so in a rehearsal it reaches nobody by construction — and \
             gating it would only mean an operator rehearsing the transport found \
             the buttons dead with nothing to say why",
        ),
        (
            "stage_media",
            true,
            "the preacher's own slide is for a person on a platform, and a slide \
             appearing there during a rehearsal is `stage_next`'s defect again",
        ),
        (
            "publish_timers",
            true,
            "a rehearsal has no stage tablet waiting for a programme clock. Same \
             shape of leak as `stage_next` and on the same screen — it publishes to \
             the hub and emits nothing, so the e2e wall test could not see it, and \
             its rehearsal case watches `qa::Kiosk` instead",
        ),
        (
            "set_background",
            true,
            "it puts an IMAGE in front of a congregation, which is the same claim \
             `broadcast_content` makes and gets the same answer. A rehearsing \
             operator's backdrop reaches their own console preview and nothing \
             else — and because it publishes to the hub as well as emitting, its \
             rehearsal case watches `qa::Kiosk`, not `qa::Wall`",
        ),
        (
            "set_transition",
            false,
            "configuration, not content. A screen that receives it looks identical \
             afterwards; gating it would leave every screen armed with the \
             pre-rehearsal transition once the operator went live",
        ),
        (
            "set_template",
            false,
            "a template, not content. Reassigning a screen's look is live by \
             design (DECISIONS §29), and suppressing it would leave a kiosk \
             rendering a template the operator has already replaced",
        ),
        (
            "set_channel_template",
            false,
            "the operator has reassigned a screen's look: live by design \
             (DECISIONS §29), and a look is not something a person reads. It was \
             main.rs's own publisher when this entry was written, and it moved \
             INTO this module when the frame gained a retained slot — the publish \
             and the retention had to become one call, because two calls is \
             exactly how the frame came to reach whoever was connected at that \
             instant and nobody who connected a minute later. The name and the \
             verdict are unchanged, which is the point: the scanner keys on the \
             name and reads both files",
        ),
        (
            "set_default_template",
            false,
            "configuration, not content — the operator's chosen fallback look. \
             Gating it would leave a screen already following the content look \
             wearing the pre-rehearsal default once the operator went live, the \
             same reasoning as `set_template` and `set_channel_template`",
        ),
        (
            "set_screen_state",
            true,
            "it takes a real screen out of a real wall. Every other gated publisher \
             here SUPPRESSES and returns success, because what it is suppressing is \
             content; this one REFUSES, because suppressing it would leave the \
             Outputs desk showing the lobby TV down while the lobby TV showed the \
             last thing it was sent, with nothing to say so — rule 35's shape on a \
             control the operator pressed deliberately. It is not a panic control, \
             so it is allowed to refuse; `clear` and `black` are and are not, which \
             is why the split is in the call",
        ),
        (
            "publish_stage_zones",
            false,
            "WHICH LAYOUT a stage screen wears, which is configuration and not \
             content — the same verdict as `set_channel_roles`, \
             `set_channel_shows` and `set_channel_looks` below, for the same \
             reason. It paints nothing on arrival: it decides which ZONES the \
             next reading, note or clock appears in, and each of those is gated \
             on its own. Gating this instead would leave a screen still wearing \
             the pre-rehearsal layout once the operator went live, which is the \
             failure the other three rows already name — and a rehearsal is \
             exactly when an operator sets a stage up, so a change that appeared \
             to do nothing would be the worst possible moment for it",
        ),
        (
            "set_channel_roles",
            false,
            "what each screen is FOR. It paints nothing and carries nothing a \
             person reads; what it decides is whether the NEXT stage message is \
             accepted, and the alert itself is gated one line above. Gating this \
             too would leave a screen that changed role during a rehearsal \
             refusing real messages once the operator went live — the failure \
             `set_default_template` describes, on a surface where the cost is a \
             preacher not being told something",
        ),
        (
            "set_channel_shows",
            false,
            "configuration, and the same verdict as `set_channel_looks` beside it \
             for the same reason: it paints nothing on arrival and cannot blank a \
             screen that is already showing something — it narrows what the NEXT \
             fire of a kind reaches. Gating it would leave every screen still \
             obeying the pre-rehearsal set once the operator went live, which is \
             `set_default_template`'s failure on the one control whose whole point \
             is that a congregation screen does NOT show something. And a \
             rehearsal is exactly when an operator checks whether the countdown is \
             off the wall",
        ),
        (
            "set_channel_looks",
            false,
            "a map of LOOKS, not content — the same verdict as `set_template`, \
             `set_channel_template` and `set_default_template` for the same \
             reason, and it is the third rung of one ladder so a different answer \
             here would be a rehearsal in which two of a screen's three template \
             authorities were live and the third was not. It paints nothing on \
             arrival: a screen that receives it looks identical until something \
             fires. Gating it would leave every screen still wearing the \
             pre-rehearsal per-kind looks once the operator went live, which is \
             exactly the failure `set_default_template` records, on the control \
             an operator is most likely to be adjusting DURING a rehearsal — \
             because seeing a look on a real screen is what a rehearsal is for",
        ),
    ];

    /// THE ENUMERATION MUST GROW WITH THE MODULE, OR IT IS NOT AN ENUMERATION.
    ///
    /// Retention has had a scanner since rule 43; rehearsal gating has had a doc
    /// comment. That comment is honest about why it exists — `stage_next` shipped
    /// ungated and leaked to a live stage tablet — and a doc comment is exactly
    /// what failed to catch it. A sixth publisher added to this module with no
    /// `rehearsing()` check currently fails nothing.
    ///
    /// This reads the source, finds every function that reaches a LAN device, and
    /// requires a verdict for each. For a function whose verdict is `true` it goes
    /// further and requires the gate to actually be IN the function body — an
    /// enumeration that only counted names would pass on a publisher whose check
    /// had been deleted.
    ///
    /// **It reads `channels.rs` AND `main.rs`, the same two files the retention
    /// scanner reads.** For a while it read only `channels.rs`, and `main.rs`'s
    /// `set_channel_template` — a real publisher, holding a real `kiosk.publish(`
    /// — was answered for in prose in this doc comment instead. That is the
    /// mechanism this test replaces, so it cannot be the mechanism this test
    /// leans on. `main.rs` is scanned WHOLE: it carries `#[cfg(test)]` from line
    /// 16, so no split can separate its tests, exactly as the retention scanner
    /// already records.
    #[test]
    fn every_publisher_in_this_module_has_an_explicit_rehearsal_verdict() {
        /// The name the `fn` declaration on this line declares, whatever it is
        /// qualified with — `pub`, `pub(crate)`, `pub(super)`, `pub(in path)`,
        /// `async`, `const`, `unsafe`, `extern "C"`, in any order.
        ///
        /// The first version recognised four spellings (`fn`, `pub fn`,
        /// `async fn`, `pub async fn`) and this module already held three
        /// `pub(crate) fn` declarations. **An unrecognised declaration is not
        /// skipped**: a body runs until the NEXT declaration the scanner
        /// recognises, so the missed function's lines are appended to the
        /// PREVIOUS function's buffer and its `.publish(` call is credited to
        /// whichever publisher came before it. An ungated `pub(crate) fn`
        /// placed immediately after `stage_alert` — which is exactly where a
        /// timer publisher would land — passed green, with no new name found and
        /// no panic. The publish tripwire below is the other half of the fix: it
        /// is what notices the loss when a spelling gets past this function.
        fn declared_fn_name(line: &str) -> Option<&str> {
            let mut rest = line.trim_start();
            loop {
                if let Some(after) = rest.strip_prefix("fn ") {
                    let name = after.trim_start().split(['(', '<', ' ', ':']).next()?;
                    return if name.is_empty() { None } else { Some(name) };
                }
                // One qualifier at a time, in whatever order they were written.
                let next = if let Some(a) = rest.strip_prefix("pub(") {
                    &a[a.find(')')? + 1..]
                } else if let Some(a) = rest.strip_prefix("pub ") {
                    a
                } else if let Some(a) = rest.strip_prefix("async ") {
                    a
                } else if let Some(a) = rest.strip_prefix("const ") {
                    a
                } else if let Some(a) = rest.strip_prefix("unsafe ") {
                    a
                } else {
                    let a = rest.strip_prefix("extern ")?.trim_start();
                    match a.strip_prefix('"') {
                        Some(q) => &q[q.find('"')? + 1..],
                        None => a,
                    }
                };
                rest = next.trim_start();
            }
        }

        /// A function reaches a LAN device if it hands the hub a message. Matched
        /// generically on `.publish(` — any receiver, not a hardcoded list of
        /// variable names — because a publisher can be written against any local
        /// (`kiosk.publish(...)`, as `main.rs` already does). `publish_kiosk(` is
        /// matched separately because it is a free function call, not a method
        /// call on a receiver, so it never contains `.publish(`.
        fn publishes_in(text: &str) -> usize {
            text.matches(".publish(").count() + text.matches("publish_kiosk(").count()
        }

        /// Deliberately broader than `declared_fn_name`: `fn` as a word, followed
        /// by an identifier, with nothing before it on the line but characters a
        /// qualifier could be made of. It accepts spellings `declared_fn_name`
        /// does not — including ones nobody has written yet — which is the whole
        /// point. The two must agree exactly on every buffer, and where they stop
        /// agreeing the narrow one has gone blind.
        fn looks_like_a_declaration(line: &str) -> bool {
            let t = line.trim_start();
            let Some(i) = t.find("fn ") else { return false };
            if !t[..i]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "(): \"\t".contains(c))
            {
                return false;
            }
            t[i + 3..]
                .trim_start()
                .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        }

        let chan = include_str!("channels.rs");
        // channels.rs strips its own tests: its `mod tests` is full of example
        // publishers that the module does not ship.
        let chan_body = chan.split("mod tests").next().unwrap_or(chan);
        let main_rs = include_str!("main.rs");

        // (file, function name, its body) for every fn in both sources, in order.
        //
        // Keyed on POSITION, never on name: `set` and `transition` are each
        // declared twice in this module, so `find(|(n, _)| n == name)` answers
        // about the first one and can be answering about the wrong function
        // entirely.
        let mut fns: Vec<(&str, &str, String)> = Vec::new();
        let mut scanned_publishes = 0usize;
        for (file, body) in [("channels.rs", chan_body), ("main.rs", main_rs)] {
            let mut current: Option<&str> = None;
            let mut buf = String::new();
            for line in body.lines() {
                let t = line.trim_start();
                // Comment lines talk ABOUT a gate or a publish call without being
                // one — `kinds_in` above already knows this, and this scanner has
                // to know it too, or a doc comment that merely mentions
                // `rehearsing(` or `.publish(` reads as the real thing. Skipped
                // before the count as well as before the buffer, so the tripwire
                // compares like with like.
                if t.starts_with("//") {
                    continue;
                }
                if let Some(name) = declared_fn_name(t) {
                    if let Some(prev) = current.take() {
                        fns.push((file, prev, std::mem::take(&mut buf)));
                    }
                    current = Some(name);
                }
                scanned_publishes += publishes_in(line);
                if current.is_some() {
                    buf.push_str(line);
                    buf.push('\n');
                }
            }
            if let Some(prev) = current.take() {
                fns.push((file, prev, buf));
            }
        }

        assert!(
            fns.len() > 20,
            "the scanner found only {} functions — it has stopped reading these \
             sources, and a scanner that quietly narrows passes everything",
            fns.len()
        );

        // THE TRIPWIRE, IN TWO HALVES, BECAUSE THE FAILURE HAS TWO SHAPES.
        //
        // First: every publish call in the sources is attributed to some function.
        // A call counted in the source and absent from every buffer fell outside
        // all of them — it is gone from this test entirely.
        let attributed: usize = fns.iter().map(|(_, _, b)| publishes_in(b)).sum();
        assert_eq!(
            attributed, scanned_publishes,
            "{attributed} publish calls were attributed to functions and the \
             sources contain {scanned_publishes}. Some call fell outside every \
             function this scanner can see."
        );

        // Second, and this is the half that catches what actually happened: a
        // function this scanner cannot read is not SKIPPED, it is ABSORBED. A body
        // runs until the next declaration `declared_fn_name` recognises, so an
        // unreadable declaration and everything under it are appended to the
        // PREVIOUS function's buffer — its publish call credited to whichever
        // publisher came before it, its gate assertion satisfied by that
        // neighbour's gate. The totals do not move, so the first half sees
        // nothing. An ungated `pub(crate) fn push_timer` placed immediately after
        // `stage_alert` passed green that way, which is where a timer publisher
        // would naturally land.
        //
        // So: every buffer must hold exactly ONE thing that looks like a
        // declaration — its own. A second one means a function was absorbed.
        for (file, name, b) in &fns {
            let decls = b.lines().filter(|l| looks_like_a_declaration(l)).count();
            assert_eq!(
                decls, 1,
                "the body this scanner attributed to `{name}` ({file}) contains \
                 {decls} function declarations. A declaration `declared_fn_name` \
                 cannot read has been absorbed into it, along with whatever that \
                 function publishes — which is then credited to `{name}` and \
                 covered by `{name}`'s gate. Teach `declared_fn_name` the spelling."
            );
        }

        let publishes = |b: &str| publishes_in(b) > 0;

        let mut found: Vec<(&str, &str, &String)> = Vec::new();
        for (file, name, b) in &fns {
            // `publish_kiosk` and `publish` are the plumbing, not publishers.
            if *name == "publish_kiosk" || *name == "publish" {
                continue;
            }
            if publishes(b) {
                found.push((file, name, b));
            }
        }

        assert!(
            !found.is_empty(),
            "the scanner found no publisher at all — it has stopped reading these \
             sources, and a scanner that quietly narrows passes everything"
        );

        for (file, name, b) in &found {
            let Some((_, gated, _)) = REHEARSAL_VERDICTS.iter().find(|(n, _, _)| n == name) else {
                panic!(
                    "`{name}` ({file}) publishes to the kiosk hub and no one has said \
                     whether a rehearsal must stop it. Add it to REHEARSAL_VERDICTS \
                     with a reason, and if it is gated, add an e2e case that watches \
                     `qa::Kiosk` rather than `qa::Wall`."
                );
            };
            if *gated {
                // This entry's OWN body, not a lookup by name.
                assert!(
                    b.contains("rehearsing("),
                    "REHEARSAL_VERDICTS says `{name}` ({file}) is gated, and its body \
                     does not call `rehearsing(`. A verdict is not a gate."
                );
            }
        }

        for (name, _, reason) in REHEARSAL_VERDICTS {
            assert!(
                found.iter().any(|(_, n, _)| n == name),
                "REHEARSAL_VERDICTS names `{name}`, which nothing here publishes any \
                 more — a verdict about nothing"
            );
            assert!(
                !reason.is_empty(),
                "`{name}` has a verdict and no reason. The reason is the half a \
                 future reader needs."
            );
        }

        // THE ACCESSOR IS A DOOR TOO.
        //
        // `KioskHub::sender()` hands out the raw `broadcast::Sender`, so
        // `hub.sender().send(json)` reaches every connected kiosk client while
        // containing neither `publish_kiosk(` nor `.publish(` — a publisher every
        // assertion above is blind to. It is `pub(crate)`, not `pub`, so nothing
        // outside this crate can take that route at all; the in-crate callers that
        // remain take a sender to SUBSCRIBE (`qa.rs`'s Kiosk door, this module's
        // tests) or to hand to the WebSocket server task (`main.rs`), never to
        // publish. Inside this module nothing but the accessor itself may touch it.
        //
        // Residual, stated rather than hidden: `let tx = hub.sender();` followed by
        // `tx.send(…)` on a later line is still invisible here. The `pub(crate)`
        // is what keeps that inside a crate where this test can be extended.
        for (file, name, b) in &fns {
            if *file != "channels.rs" || *name == "sender" {
                continue;
            }
            assert!(
                !b.contains(".sender()"),
                "`{name}` reaches `KioskHub::sender()`, which hands out the raw \
                 broadcast sender. `hub.sender().send(json)` publishes to every \
                 connected kiosk and matches neither `publish_kiosk(` nor \
                 `.publish(`, so no verdict would ever be required of it. Publish \
                 through `KioskHub::publish` or `publish_kiosk`."
            );
        }
    }

    /// A SCREEN THAT JOINS LATE IS SHOWN WHAT IS ON THE SCREENS.
    ///
    /// Reproduced against the real backend: with a verse live, opening
    /// `output.html` connected, was sent its template, and painted
    /// NOTHING — a black rectangle in front of a congregation until the operator
    /// happened to fire the next thing. An OBS source restarting, a kiosk page
    /// reloading, a Wi-Fi blip on the lobby TV and this hub's own reconnect loop
    /// all produce exactly that, and RG-119 records the main output going away
    /// three times in one 85.5 minute service.
    #[tokio::test]
    async fn a_client_that_connects_mid_service_is_sent_what_is_on_the_screens() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // The verse went up BEFORE this client existed.
        hub.publish(
            r#"{"kind":"content","reference":"Romans 8:28","text":"And we know"}"#.to_string(),
        );

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

        let mut got = None;
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await
            else {
                break;
            };
            let text = msg.into_text().unwrap();
            if text.contains(r#""kind":"content""#) {
                got = Some(text);
                break;
            }
        }
        assert!(
            got.as_deref().unwrap_or("").contains("Romans 8:28"),
            "a screen that reconnected mid-reading was left blank: {got:?}"
        );
    }

    /// A TRANSITION MAY NEVER STAND IN FOR WHAT IS ON THE SCREENS.
    ///
    /// This is rule 43's own trap, one slot along. `last_screen` holds ONE frame
    /// and the newest wins, so the obvious place to put a retained transition — in
    /// there, beside `content`, `clear` and `black` — would mean an operator
    /// changing the transition ERASED the retained verse. The next screen to join
    /// mid-reading would then have been sent a preference and a blank wall: the
    /// exact failure DECISIONS §68 exists to prevent, delivered by the mechanism
    /// that prevents it.
    ///
    /// Watched to fail by adding `transition` to `is_screen_frame`.
    #[tokio::test]
    async fn a_transition_never_becomes_the_frame_a_late_screen_is_shown() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // The verse went up, and THEN the operator changed the transition.
        hub.publish(
            r#"{"kind":"content","reference":"Romans 8:28","text":"And we know"}"#.to_string(),
        );
        hub.set_transition(Some("crossfade".into()), Some(320));

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

        // BOTH, and the content must still arrive — that is the whole assertion.
        let mut saw_content = false;
        let mut saw_transition = false;
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await
            else {
                break;
            };
            let text = msg.into_text().unwrap_or_default();
            if text.contains(r#""kind":"content""#) && text.contains("Romans 8:28") {
                saw_content = true;
            }
            if text.contains(r#""kind":"transition""#) && text.contains("crossfade") {
                saw_transition = true;
            }
            if saw_content && saw_transition {
                break;
            }
        }
        assert!(
            saw_content,
            "changing the transition erased the verse a late screen is shown — \
             rule 43's failure delivered by rule 43's own mechanism"
        );
        assert!(
            saw_transition,
            "a screen that joined mid-service was left cutting while every other \
             screen in the building crossfaded"
        );
    }

    /// A TRANSITION MAY NOT DELAY, GATE OR SURVIVE A PANIC CONTROL.
    ///
    /// Three separate claims, and all three are the same rule (rule 15,
    /// DECISIONS §20): a panic control does what it says, at once, unconditionally.
    ///
    ///   · `clear` publishes its frame with an override in force, unchanged and
    ///     with nothing consulted — it does not read `last_transition` at all.
    ///   · The cleared wall is what a late screen is shown, not the verse.
    ///   · The override OUTLIVES the clear, which is correct and is the reason it
    ///     is kept in its own slot: it is configuration, so clearing the screens
    ///     must not quietly re-arm every template's own transition behind the
    ///     operator's back.
    #[test]
    fn a_panic_control_is_neither_delayed_nor_undone_by_a_transition() {
        let hub = KioskHub::default();
        hub.publish(r#"{"kind":"content","reference":"Psalms 23:1"}"#.to_string());
        // THE ORDER IS THE OPERATOR'S REAL ONE: hit Clear screens, then reach for
        // the picker. With one shared slot the second act would erase the first,
        // and a screen joining a moment later would still be showing the verse the
        // operator had just taken down.
        hub.publish(r#"{"kind":"clear"}"#.to_string());
        hub.set_transition(Some("fadeblack".into()), Some(800));

        let retained = hub
            .last_screen
            .lock()
            .ok()
            .and_then(|l| l.clone())
            .unwrap_or_default();
        assert_eq!(
            retained, r#"{"kind":"clear"}"#,
            "a wall the operator cleared was not what a late screen would be shown"
        );

        // …AND THE OTHER ORDER. A panic control pressed while an override is in
        // force takes the screens down and leaves the override alone: it is
        // configuration, so a blackout must not quietly re-arm every template's own
        // transition behind the operator's back, to be discovered on the next fire.
        hub.publish(r#"{"kind":"black"}"#.to_string());
        let retained = hub
            .last_screen
            .lock()
            .ok()
            .and_then(|l| l.clone())
            .unwrap_or_default();
        assert_eq!(
            retained, r#"{"kind":"black"}"#,
            "a blacked-out wall was not what a late screen would be shown"
        );
        assert_eq!(
            hub.current_transition(),
            Some(("fadeblack".to_string(), Some(800))),
            "a panic control silently threw away the operator's transition"
        );
    }

    /// AN IMPLAUSIBLE MODE IS NOT RETAINED AND REPLAYED FOR THE REST OF A SERVICE.
    ///
    /// The seven live in `src/lib/transitions.js` and are deliberately NOT copied
    /// here — a second register drifts from the first, and an unknown mode is
    /// already a cut over there (§71). What this end owes is frame integrity: the
    /// value is serialised by `serde_json`, so nothing can break out of the JSON,
    /// and a wildly long string is dropped rather than kept in the hub and sent to
    /// every screen that joins for the next hour.
    #[test]
    fn the_transition_frame_cannot_be_broken_out_of_or_stuffed() {
        let hub = KioskHub::default();
        hub.set_transition(Some("\",\"kind\":\"content\",\"text\":\"x".into()), None);
        // Long enough to be nonsense, short enough that the cap is what stops it.
        hub.set_transition(Some("c".repeat(64)), None);
        assert_eq!(
            hub.current_transition(),
            None,
            "a 64-character mode was kept"
        );

        hub.set_transition(Some("crossfade".into()), Some(320));
        let frame = transition_json(hub.current_transition().as_ref());
        let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");
        assert_eq!(v["kind"], "transition");
        assert_eq!(v["mode"], "crossfade");
        assert!(
            !is_screen_frame(&frame),
            "the real frame this module publishes matched the screen-frame matcher"
        );

        hub.set_transition(None, None);
        assert_eq!(hub.current_transition(), None);
        let cleared = transition_json(None);
        assert!(
            cleared.contains(r#""mode":null"#),
            "a cleared override must be an explicit null, not an absent key: {cleared}"
        );
    }

    /// One `Stage` timer, five minutes out, for the tests below.
    fn stage_timer(id: i64, label: &str) -> crate::timers::Timer {
        crate::timers::Timer {
            id,
            label: label.into(),
            done_msg: String::new(),
            target_ms: 1_700_000_000_000 + 5 * 60_000,
            from_ms: 1_700_000_000_000,
            paused_ms: None,
            warn_ms: None,
            scope: crate::timers::Scope::Stage,
            // A fixture states no configured length; `start` fills it from the span.
            configured_ms: 0,
            until_ms: None,
            plan_item_id: None,
            started_in_rehearsal: false,
            channels: None,
        }
    }

    /// A PROGRAMME TIMER IS NOT WHAT A SCREEN IS SHOWING.
    ///
    /// Rule 43's trap 1, one slot further along, and the fourth time this module has
    /// walked up to it: `last_screen` holds ONE frame and the newest wins, so a
    /// timer retained there would ERASE the retained verse — and the next screen to
    /// join mid-reading would be handed a clock over a blank wall. That is the
    /// failure DECISIONS §68 exists to prevent, delivered by its own mechanism, for
    /// the same reason `transition`, `channel_template` and `default_template` each
    /// have a slot of their own.
    ///
    /// **The frame is built by the module's own serialiser, never by hand.** A
    /// `serde_json` map is a BTreeMap, so the key order is alphabetical and not the
    /// order anybody wrote — the first version of `is_screen_frame` was a
    /// `starts_with` that matched nothing while looking exactly like the bug it
    /// fixed. A hand-written literal here would reproduce that: it would assert
    /// about a string this module never emits.
    /// RG-167 — THE WIRE KIND IS `"timer"`, AND A RENAME MUST NOT REACH IT.
    ///
    /// The two clocks were renamed on 2026-09-18: the congregation's is a
    /// **Screen Countdown** and the preacher's is a **Stage Timer**, because six
    /// labels between two concepts told an operator nothing about which of them
    /// reached a room and which reached one person (DECISIONS §92 addendum).
    ///
    /// **A label is what a person reads; a frame kind is what two programs agree
    /// on.** `stage.html` is served over the LAN and a church may have it open on
    /// a tablet that has not been reloaded since the last release, or pinned in a
    /// kiosk that reloads on a schedule of its own. Renaming this string to match
    /// the label would leave every such page matching nothing in its `apply`
    /// branch — no error, no console anybody can read, and the preacher's rail
    /// simply gone for the service.
    ///
    /// Asserted against the SERIALISED frame rather than a literal, for the same
    /// reason the test below it is: `serde_json`'s map is a BTreeMap, so the only
    /// honest question is what this module actually emits. Watched to fail by
    /// renaming the kind to `stage_timer` in `timer_frame_json`.
    #[test]
    fn the_wire_kind_stays_timer_however_the_control_is_labelled() {
        let frame = timer_frame_json(&[stage_timer(1, "Sermon")], None);
        let parsed: serde_json::Value =
            serde_json::from_str(&frame).expect("a timer frame is JSON");
        assert_eq!(
            parsed["kind"], "timer",
            "the programme rail's wire kind moved. `Stage.svelte` matches              `m.kind === 'timer'` and so does `is_timer_frame`; a church running an              older `stage.html` would lose its rail with nothing said: {frame}"
        );
        assert!(
            is_timer_frame(&frame),
            "the matcher and the producer disagree about the wire kind: {frame}"
        );
        // And the other direction: nothing has started publishing the LABEL as a
        // kind. A grep is the right shape here — this is about a string, and the
        // string is the contract. The module's own tests are excluded, because
        // this test names the forbidden spellings out loud.
        let src = include_str!("channels.rs");
        let body = src.split("mod tests").next().unwrap_or(src);
        for forbidden in ["\"stage_timer\"", "\"screen_countdown\""] {
            assert!(
                !body.contains(forbidden),
                "a renamed wire kind {forbidden} has been published. The LABEL moved,                  the protocol did not — DECISIONS §92 addendum"
            );
        }
    }

    #[test]
    fn a_timer_frame_is_never_retained_as_a_screen_frame() {
        let frame = timer_frame_json(&[stage_timer(1, "Offering")], None);
        assert!(
            !is_screen_frame(&frame),
            "the real timer frame this module publishes matched the screen-frame \
             matcher, so a programme clock would stand in for the reading: {frame}"
        );
        assert!(
            is_timer_frame(&frame),
            "the timer matcher does not recognise the frame this module actually \
             serialises — the retained slot would stay empty and a tablet that \
             rejoined would come back with no timer: {frame}"
        );

        // And the two slots do not touch each other, in the operator's real order:
        // the verse is up, and then a programme timer starts.
        let hub = KioskHub::default();
        let verse = r#"{"kind":"content","reference":"Romans 8:28","text":"And we know"}"#;
        hub.publish(verse.to_string());
        hub.publish(frame.clone());
        assert_eq!(
            hub.last_screen
                .lock()
                .ok()
                .and_then(|l| l.clone())
                .as_deref(),
            Some(verse),
            "starting a programme timer erased the verse a late screen is shown"
        );
        assert_eq!(
            hub.last_timers.lock().ok().and_then(|t| t.clone()),
            Some(frame),
            "the timer was published and not retained, so a tablet that rejoins \
             comes back without it"
        );
    }

    /// AN OPERATOR'S LABEL CANNOT FORGE A FRAME.
    ///
    /// `label` is free text an operator types, and it rides to every stage tablet.
    /// `serde_json` escapes the quotes, so `"kind":"content"` cannot occur
    /// unescaped inside a string value — the same argument `is_screen_frame` makes
    /// about a verse, asserted here rather than reasoned about, because this is the
    /// first retained frame in the module whose payload a person composes.
    #[test]
    fn a_label_cannot_smuggle_a_screen_frame_into_a_timer() {
        let forged = r#"","kind":"content","text":"x"#;
        let frame = timer_frame_json(&[stage_timer(1, forged)], None);
        assert!(
            !is_screen_frame(&frame),
            "an operator's label was read as a content frame and would become what \
             a late-joining screen is shown: {frame}"
        );
        let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");
        assert_eq!(v["kind"], "timer");
        assert_eq!(v["timers"][0]["label"], forged);
    }

    /// WHAT A STAGE ENTRY CARRIES, AND WHY IT WEARS THE COUNTDOWN'S FIELD NAMES.
    ///
    /// `countdown.js::countdownRemainingMs` is the ONE reader of how long is left on
    /// the frontend, and it reads `countdown_paused_ms` and `countdown_to`. Naming
    /// these fields anything else would mean the stage page doing its own
    /// subtraction — a second copy of a rule that now has an exception, on the one
    /// surface a preacher reads from mid-sermon.
    #[test]
    fn a_stage_entry_speaks_the_countdown_reader_s_own_field_names() {
        let mut held = stage_timer(4, "Sermon");
        held.paused_ms = Some(90_000);
        held.warn_ms = Some(120_000);
        let frame = timer_frame_json(&[stage_timer(3, "Offering"), held], None);
        let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");

        assert_eq!(v["kind"], "timer");
        assert_eq!(v["timers"][0]["id"], 3);
        assert_eq!(v["timers"][0]["label"], "Offering");
        assert_eq!(
            v["timers"][0]["countdown_to"],
            1_700_000_000_000i64 + 5 * 60_000
        );
        assert_eq!(v["timers"][0]["countdown_from"], 1_700_000_000_000i64);
        assert_eq!(
            v["timers"][0]["countdown_paused_ms"],
            serde_json::Value::Null
        );
        assert_eq!(
            v["timers"][1]["countdown_paused_ms"], 90_000,
            "a held timer must carry the figure it is held at, or the tablet counts \
             down through a hold the operator applied"
        );
        assert_eq!(v["timers"][1]["warn_ms"], 120_000);
        // The order is the registry's own, oldest first — a rail whose rows swap
        // places between frames is a rail nobody can read.
        assert_eq!(v["timers"][1]["id"], 4);
    }

    /// THE CONFIGURED WARNING WINDOW RIDES WITH THE PROGRAMME — RG-149(c), the
    /// stage half.
    ///
    /// `Settings → General → Countdown warning` is console state: its only writer is
    /// `stores/capture.js`, and `stage.html` does not and cannot import that module
    /// — it has no Tauri bridge. So the figure has to be DELIVERED, and this frame
    /// is the one that reaches the preacher's page first: a programme timer can be
    /// running before anything at all has been fired, and waiting for a content
    /// frame would leave the surface whose whole purpose is the clock on the shipped
    /// minute for that entire time.
    ///
    /// It sits beside `timers` rather than on each row on purpose. It is a fact
    /// about the MACHINE, not about a timer; a copy per row is a copy that can
    /// disagree with itself in one frame.
    #[test]
    fn the_programme_frame_carries_the_configured_warning_window() {
        let frame = timer_frame_json(&[stage_timer(3, "Offering")], Some(150_000));
        let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");
        assert_eq!(
            v["warn_default_ms"], 150_000,
            "the stage page has no way to learn the configured window"
        );
        // An empty set still carries it: that frame is how the last clock comes OFF
        // the screen, and it is also the first frame a tablet may ever receive.
        let none = timer_frame_json(&[], Some(150_000));
        let v: serde_json::Value = serde_json::from_str(&none).expect("valid JSON");
        assert_eq!(v["warn_default_ms"], 150_000);
        assert_eq!(v["timers"].as_array().map(|a| a.len()), Some(0));
        // Unset is an explicit null, never a zero — the page then keeps the shipped
        // minute rather than a window that never opens.
        let bare = timer_frame_json(&[], None);
        let v: serde_json::Value = serde_json::from_str(&bare).expect("valid JSON");
        assert_eq!(v["warn_default_ms"], serde_json::Value::Null);
        // And the frame is still recognised as the programme frame, so it is still
        // the one retained and replayed to a tablet that joins late (rule 43).
        assert!(is_timer_frame(&bare));
    }

    /// AN EMPTY SET IS A FRAME, NOT A SILENCE.
    ///
    /// Stopping the last programme timer has to reach the tablet, or the clock stays
    /// on the preacher's screen for the rest of the service. An absent frame cannot
    /// say "there are none now".
    #[test]
    fn stopping_the_last_timer_publishes_an_empty_set_rather_than_nothing() {
        let frame = timer_frame_json(&[], None);
        let v: serde_json::Value = serde_json::from_str(&frame).expect("valid JSON");
        assert_eq!(v["kind"], "timer");
        assert_eq!(
            v["timers"].as_array().map(|a| a.len()),
            Some(0),
            "an empty set must still be a timer frame: {frame}"
        );
        assert!(is_timer_frame(&frame));
    }

    // ════════════════════════════════════════════════════════════════════════
    //  THE BACKGROUND LAYER — a picture that outlives the words painted on it
    //
    //  Relay had no persistent background. The whole layer stack sits inside
    //  `{#if content}` and `media_url` was written at exactly one site in the
    //  binary, so a verse and a picture were MUTUALLY EXCLUSIVE payloads:
    //  scripture over the church's own background could not be expressed at all.
    //  A background is therefore a second payload kind with a lifetime of its
    //  own, and the first thing that had to be true of it is the thing that can
    //  hurt a congregation — a clear must still remove EVERYTHING.
    //
    //  These four were written BEFORE any of the rendering work, watched to fail,
    //  and each was watched to fail again with its guard reverted.
    // ════════════════════════════════════════════════════════════════════════

    /// A PANIC CONTROL TAKES THE BACKGROUND WITH IT.
    ///
    /// The invariant the whole layer was built against, at the retention door
    /// rather than at a publisher: a `clear` and a `black` are published through
    /// the same `publish` every other frame goes through, so the drop happens by
    /// construction and a content kind added next year cannot forget it. Nothing
    /// new is sent to do this — a panic control that needed a SECOND frame to
    /// finish its job is a panic control that can half succeed, and rule 15 does
    /// not allow one of those.
    #[test]
    fn a_panic_control_takes_the_retained_background_with_it() {
        for wipe in [r#"{"kind":"clear"}"#, r#"{"kind":"black"}"#] {
            let hub = KioskHub::default();
            hub.publish(background_json(Some(&Background {
                media_url: "http://10.0.0.5:8032/media/3".into(),
                media_kind: "image".into(),
            })));
            assert!(
                retained_background(&hub).is_some(),
                "the hub did not retain a background it was handed"
            );
            hub.publish(wipe.to_string());
            assert!(
                retained_background(&hub).is_none(),
                "{wipe} left a picture retained, so the next screen to join would \
                 have been painted a background over a wall the operator took down"
            );
        }
    }

    /// AND A BACKGROUND CAN NEVER BECOME WHAT A SCREEN IS SHOWING.
    ///
    /// Its own slot, for the fifth time in this module: `last_screen` holds ONE
    /// frame and the newest wins, so retaining a picture beside the verse would
    /// ERASE the verse and the next screen to join mid-reading would be handed
    /// wallpaper and no words — rule 43's own failure delivered by rule 43's own
    /// mechanism.
    #[test]
    fn a_background_is_not_what_a_screen_is_showing() {
        let hub = KioskHub::default();
        hub.publish(
            r#"{"kind":"content","reference":"Romans 8:28","text":"And we know"}"#.to_string(),
        );
        hub.publish(background_json(Some(&Background {
            media_url: "http://10.0.0.5:8032/media/3".into(),
            media_kind: "image".into(),
        })));
        let screen = hub.last_screen_handle().lock().unwrap().clone();
        assert!(
            screen.as_deref().unwrap_or("").contains("Romans 8:28"),
            "a background replaced the retained verse: {screen:?}"
        );
        assert!(retained_background(&hub).is_some());
    }

    /// THE RETENTION RULE AGREES WITH WHAT IS PUBLISHED.
    ///
    /// The other half of every matcher in this module, and the half that caught
    /// `is_screen_frame` matching nothing while looking exactly like the bug it
    /// fixed. `background_json(None)` is how a background is TAKEN DOWN, and a
    /// hub that retained that frame would hold a message whose only effect is to
    /// say "nothing" — two spellings of an absence, which is how two spellings of
    /// an absence come to disagree.
    #[test]
    fn the_background_retention_rule_agrees_with_what_is_published() {
        let up = background_json(Some(&Background {
            media_url: "http://10.0.0.5:8032/media/3".into(),
            media_kind: "image".into(),
        }));
        let down = background_json(None);
        assert!(is_background_frame(&up), "a real background frame: {up}");
        assert!(is_background_frame(&down), "so is the one that clears it");
        assert_eq!(
            background_retention(&up),
            Some(Some(up.clone())),
            "a frame naming a picture IS the background now"
        );
        assert_eq!(
            background_retention(&down),
            Some(None),
            "a frame naming no picture takes it down: {down}"
        );
        assert_eq!(
            background_retention(r#"{"kind":"clear"}"#),
            Some(None),
            "a panic control takes it down"
        );
        assert_eq!(
            background_retention(r#"{"kind":"black"}"#),
            Some(None),
            "and so does the other one"
        );
        // Everything else leaves it exactly as it is. A verse must not take the
        // church's backdrop down, which is the entire point of the payload.
        for other in [
            r#"{"kind":"content","reference":"Romans 8:28"}"#,
            r#"{"kind":"timer","timers":[]}"#,
            r#"{"kind":"stage_alert","text":"two minutes"}"#,
            r#"{"kind":"transition","mode":"cut"}"#,
        ] {
            assert_eq!(
                background_retention(other),
                None,
                "{other} moved the background, and it is not about the background"
            );
        }
        // And it is never a screen frame — the assertion that keeps the two slots
        // disjoint by construction.
        assert!(!is_screen_frame(&up));
        assert!(!is_screen_frame(&down));
    }

    /// A SCREEN THAT JOINS MID-SERVICE IS PAINTED THE BACKGROUND TOO.
    ///
    /// Rule 43 for the second payload kind. Without this an OBS source that
    /// restarted came back with the verse and no backdrop — the words floating on
    /// black while every other screen in the building carried the church's own
    /// picture, and nothing anywhere saying so.
    #[tokio::test]
    async fn a_screen_that_joins_mid_service_is_sent_the_background() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // The background went up before this screen existed.
        hub.publish(background_json(Some(&Background {
            media_url: "http://10.0.0.5:8032/media/3".into(),
            media_kind: "image".into(),
        })));

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

        let mut got = None;
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await
            else {
                break;
            };
            let text = msg.into_text().unwrap_or_default();
            if is_background_frame(&text) {
                got = Some(text);
                break;
            }
        }
        assert!(
            got.as_deref().unwrap_or("").contains("/media/3"),
            "a screen that joined mid-service was sent no background: {got:?}"
        );
    }

    /// …AND A SCREEN THAT JOINS AFTER A CLEAR IS SENT NOTHING AT ALL.
    ///
    /// The panic half of the test above, and the one that matters. The retained
    /// slot is emptied by the clear itself, so there is nothing left to replay —
    /// the replay can no more resurrect a background than it can resurrect a
    /// verse.
    #[tokio::test]
    async fn a_screen_that_joins_after_a_clear_is_sent_no_background() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        hub.publish(background_json(Some(&Background {
            media_url: "http://10.0.0.5:8032/media/3".into(),
            media_kind: "image".into(),
        })));
        hub.publish(r#"{"kind":"clear"}"#.to_string());

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

        let mut frames: Vec<String> = Vec::new();
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_millis(600), read.next()).await
            else {
                break;
            };
            frames.push(msg.into_text().unwrap_or_default());
        }
        assert!(
            !frames.iter().any(|f| is_background_frame(f)),
            "a screen that joined after a clear was painted the background back: \
             {frames:?}"
        );
        assert!(
            frames.iter().any(|f| f.contains(r#""kind":"clear""#)),
            "and it must still have been told the wall is clear: {frames:?}"
        );
    }

    /// The retained background, read the way the WS task reads it.
    fn retained_background(hub: &KioskHub) -> Option<String> {
        hub.last_background_handle().lock().unwrap().clone()
    }

    /// A STAGE TABLET THAT JOINS MID-SERVICE IS SENT THE PROGRAMME TIMERS.
    ///
    /// The same failure as rule 43's, on the screen most likely to produce it: a
    /// phone locking, a tablet reloading, or a walk out of wifi range. Without this
    /// the programme clock comes back only when the operator next touches a timer,
    /// which during a sermon is never.
    #[tokio::test]
    async fn a_stage_tablet_that_joins_mid_service_is_sent_the_programme_timers() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // The programme timer started BEFORE this tablet existed.
        hub.publish(timer_frame_json(&[stage_timer(1, "Offering")], None));

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

        let mut got = None;
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await
            else {
                break;
            };
            let text = msg.into_text().unwrap_or_default();
            if is_timer_frame(&text) {
                got = Some(text);
                break;
            }
        }
        assert!(
            got.as_deref().unwrap_or("").contains("Offering"),
            "a stage tablet that rejoined mid-service came back with no programme \
             timer: {got:?}"
        );
    }

    /// A CUE THAT NAMES A SCREEN IS RETAINED FOR THAT SCREEN ONLY (RG-161).
    ///
    /// Rule 43 says a screen joining mid-service is shown what is on the
    /// screens. The moment a cue can name screens, *the screens* stops being
    /// one answer — and a single retained slot would replay a targeted frame to
    /// every late joiner, which is the targeting failing in precisely the way
    /// it exists to prevent.
    #[test]
    fn a_targeted_frame_is_replayed_only_to_the_screen_it_named() {
        let hub = KioskHub::default();
        hub.publish(r#"{"kind":"content","channels":[2],"reference":"Notices"}"#.to_string());

        let per = hub.last_screen_by_channel.lock().expect("slot").clone();
        assert!(
            per.contains_key(&2),
            "the named screen has nothing retained"
        );
        assert!(
            !per.contains_key(&1),
            "an unnamed screen was given the frame"
        );
        assert!(
            hub.last_screen.lock().expect("slot").is_none(),
            "a targeted frame became what EVERY late joiner is shown"
        );
    }

    /// AN UNTARGETED FRAME SUPERSEDES EVERYTHING, which is what lets `hello`
    /// choose between the two slots without a clock.
    #[test]
    fn a_frame_for_every_screen_clears_what_one_screen_was_told() {
        let hub = KioskHub::default();
        hub.publish(r#"{"kind":"content","channels":[2],"reference":"Notices"}"#.to_string());
        hub.publish(r#"{"kind":"content","reference":"John 3:16"}"#.to_string());

        assert!(
            hub.last_screen_by_channel.lock().expect("slot").is_empty(),
            "a screen would rejoin to a stale targeted frame that a later \
             all-screens cue had already replaced"
        );
        assert!(hub.last_screen.lock().expect("slot").is_some());
    }

    /// AND A PANIC CONTROL STILL REACHES EVERY SCREEN.
    ///
    /// `clear` and `black` never carry a channel set — a control that has to
    /// ask which screens it is talking to is one that can fail (rule 15,
    /// DECISIONS §20) — so they take the untargeted path and empty the map with
    /// it. A screen that joins after a clear finds a cleared wall, targeted cue
    /// or not.
    #[test]
    fn a_clear_still_takes_down_what_one_screen_alone_was_shown() {
        let hub = KioskHub::default();
        hub.publish(r#"{"kind":"content","channels":[2],"reference":"Notices"}"#.to_string());
        hub.publish(r#"{"kind":"clear"}"#.to_string());

        assert!(
            hub.last_screen_by_channel.lock().expect("slot").is_empty(),
            "a targeted cue survived a panic control for a screen joining later"
        );
        let global = hub.last_screen.lock().expect("slot").clone();
        assert!(global
            .as_deref()
            .unwrap_or("")
            .contains(r#""kind":"clear""#));
    }

    /// THE THREE READINGS OF AN ABSENT CHANNEL SET, which must not diverge.
    #[test]
    fn a_frame_that_names_no_screens_is_every_screen_and_an_empty_list_is_none() {
        // Absent — every cue built before targeting existed.
        assert_eq!(frame_channels(r#"{"kind":"content"}"#), None);
        // Explicitly null.
        assert_eq!(
            frame_channels(r#"{"kind":"content","channels":null}"#),
            None
        );
        // Malformed. Reaching every screen is the safe direction: content that
        // silently reaches nothing is worse than content that reaches more.
        assert_eq!(frame_channels(r#"{"kind":"content","channels":7}"#), None);
        // Empty is NOT none-of-the-above: a cue that reaches no screen is a
        // real thing to ask for.
        assert_eq!(
            frame_channels(r#"{"kind":"content","channels":[]}"#),
            Some(vec![])
        );
        assert_eq!(
            frame_channels(r#"{"kind":"content","channels":[2,5]}"#),
            Some(vec![2, 5])
        );
    }

    /// AN EMPTY TARGET REACHES NO SCREEN AND OVERWRITES NOTHING.
    #[test]
    fn a_cue_that_names_no_screen_leaves_every_screen_as_it_was() {
        let hub = KioskHub::default();
        hub.publish(r#"{"kind":"content","reference":"John 3:16"}"#.to_string());
        hub.publish(r#"{"kind":"content","channels":[],"reference":"Nobody"}"#.to_string());

        let global = hub.last_screen.lock().expect("slot").clone();
        assert!(
            global.as_deref().unwrap_or("").contains("John 3:16"),
            "a cue aimed at nothing replaced what every screen was showing"
        );
        assert!(hub.last_screen_by_channel.lock().expect("slot").is_empty());
    }

    /// A SCREEN THAT REPORTS IS TOLD THE TIME, AND THAT IT WAS HEARD.
    ///
    /// Two open findings wanted the same frame, so they get one (plan S5, S6).
    ///
    /// **S5 — a socket is not a screen.** `Stage.svelte` set `connected` on
    /// `onopen` and never re-evaluated it, so a phone that slept, roamed, or sat
    /// behind a NAT that had timed out kept a green `live` pip over frozen
    /// content. Absence of frames cannot detect that: the hub only publishes
    /// when something changes, so silence is the normal state of a quiet
    /// service. Something has to answer on a schedule, and the page is already
    /// sending a `beat` every two seconds.
    ///
    /// **S6 — the countdown was computed against the phone's clock.**
    /// `countdown_to` is an absolute epoch produced on the HOST, and the stage
    /// page subtracts its own `Date.now()`. A tablet a minute out showed a
    /// minute of error to the person preaching. The ack carries the host's
    /// epoch, so the offset comes from the round trip that was already
    /// happening — and the round trip's own duration is what bounds the
    /// correction's accuracy.
    ///
    /// It is sent to the ONE client that beat, exactly as the hello reply is,
    /// rather than broadcast: a tick to every browser source and lobby TV in
    /// the building would be traffic bought for one page's benefit. It stays
    /// inert and read-only — it carries a number this server already knows and
    /// nothing a client said.
    #[tokio::test]
    async fn a_screen_that_beats_is_answered_with_the_host_clock() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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
                r#"{"kind":"beat","channel":2,"state":"content"}"#.to_string(),
            ))
            .await
            .expect("send beat");

        let before = crate::now_epoch_ms();
        let mut ack = None;
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await
            else {
                break;
            };
            let text = msg.into_text().unwrap_or_default();
            if text.contains(r#""kind":"beat_ack""#) {
                ack = Some(text);
                break;
            }
        }
        let after = crate::now_epoch_ms();

        let ack = ack.expect("a screen reported and was told nothing back");
        let v: serde_json::Value = serde_json::from_str(&ack).expect("beat_ack is not JSON");
        let at = v
            .get("at")
            .and_then(|a| a.as_i64())
            .expect("no host clock on the ack");
        assert!(
            at >= before && at <= after,
            "the ack carried {at}, which is not a host time taken between \
             {before} and {after} — a stage page correcting its clock against \
             this would be corrected to the wrong one"
        );
    }

    /// AND A MALFORMED BEAT IS STILL DROPPED, RATHER THAN ANSWERED.
    ///
    /// The ack must not become a way to make the server talk. `state` is parsed
    /// against a closed enum and a beat that fails it touches no health and now
    /// must also draw no reply — otherwise an unparseable beat would be
    /// distinguishable from a parseable one by whether an answer came back,
    /// which is a probe this read-only server does not owe anybody.
    /// 2026-09-21 · RG-182, through the real hub: a beat naming a picture the
    /// screen could not load reaches `OutputHealth`, and the next clean beat
    /// clears it. The pure reader is tested above; this is the wiring.
    #[tokio::test]
    async fn a_beat_naming_a_media_failure_reaches_the_desk_through_the_real_hub() {
        use futures_util::{SinkExt, StreamExt};
        let port = free_port();
        let hub = KioskHub::default();
        let health = OutputHealth::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            health.clone(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");
        let (mut write, mut read) = ws.split();
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"kind":"beat","channel":2,"state":"content","media_error":"image not loading · http://10.0.0.5:8032/media/3"}"#
                    .to_string(),
            ))
            .await
            .expect("send beat");
        // Wait for the ack: that is the hub saying it has read the beat.
        let mut acked = false;
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await
            else {
                break;
            };
            if msg
                .into_text()
                .unwrap_or_default()
                .contains(r#""kind":"beat_ack""#)
            {
                acked = true;
                break;
            }
        }
        assert!(acked, "the beat was never acknowledged");
        assert_eq!(
            health.media_error_of(2).as_deref(),
            Some("image not loading · http://10.0.0.5:8032/media/3"),
            "the desk never learned what the screen said about its picture"
        );
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"kind":"beat","channel":2,"state":"content"}"#.to_string(),
            ))
            .await
            .expect("send clean beat");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(health.media_error_of(2), None, "a clean beat must clear it");
    }

    #[tokio::test]
    async fn a_beat_that_does_not_parse_is_not_answered() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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
                r#"{"kind":"beat","channel":2,"state":"sideways"}"#.to_string(),
            ))
            .await
            .expect("send beat");

        let answered =
            tokio::time::timeout(std::time::Duration::from_millis(400), read.next()).await;
        assert!(
            answered.is_err(),
            "an unparseable beat drew a reply: {answered:?}"
        );
    }

    /// AND THE READING IS PAINTED LAST.
    ///
    /// Order, not merely presence. A tablet sent the timers AFTER the retained
    /// content frame paints the reading and then a clock over it — the flash a
    /// preacher sees at exactly the moment they look down. So: template,
    /// default_template, channel_roles, transition, timers, and WHAT IS ON THE
    /// SCREENS last, which is why the retained frame has always been last.
    ///
    /// **THE LIST IS EXHAUSTIVE ON PURPOSE, AND THAT IS WHY IT WENT RED IN A
    /// MERGE.** Wave 3 added the `timer` slot and wave 5 added `channel_roles`,
    /// on branches that never met until they were merged; both are configuration,
    /// both belong before the screen frame, and the compiler cannot see either
    /// omission. This assertion is the only instrument that noticed, and it
    /// noticed by failing rather than by being right — which is what it is for.
    /// A new slot goes in the middle of this list, never after `content`.
    ///
    /// **`screen_state` IS THE ONE EXCEPTION, AND IT IS AFTER `content`.** It went
    /// red in exactly the way the paragraph above describes and the answer was the
    /// other one: it is not configuration and it is not something to paint — it
    /// says which screens the operator has taken OUT of the wall, which overrides
    /// what is on them. Sent before the retained verse, the verse would paint over
    /// the operator's decision and a lobby TV whose browser source restarted would
    /// bring itself back up mid-sermon: rule 43's own mechanism undoing an
    /// operator's own control. So the rule is not "content is last" but "the thing
    /// that decides what shows is last", and content was last for as long as
    /// nothing could override it.
    #[tokio::test]
    async fn the_hello_order_puts_the_screen_frame_last() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Everything a tablet could be owed, all in force at once.
        hub.cache_template(7, r#"{"name":"Stage"}"#);
        hub.cache_default_template(r#"{"name":"House"}"#);
        hub.set_transition(Some("crossfade".into()), Some(320));
        hub.publish(timer_frame_json(&[stage_timer(1, "Offering")], None));
        hub.publish(background_json(Some(&Background {
            media_url: "http://10.0.0.5:8032/media/3".into(),
            media_kind: "image".into(),
        })));
        hub.publish(
            r#"{"kind":"content","reference":"Romans 8:28","text":"And we know"}"#.to_string(),
        );

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

        // READ UNTIL THE REPLY STOPS, NEVER UNTIL THE CONTENT ARRIVES. Stopping at
        // the content frame would make a timer sent AFTER it — which is the exact
        // defect this test exists to catch — look like a timer that never arrived,
        // and the failure message would then send the next reader after the wrong
        // bug. The loop ends on the read timeout, so a wrong order is reported as a
        // wrong order.
        let mut order: Vec<String> = Vec::new();
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_millis(600), read.next()).await
            else {
                break;
            };
            let text = msg.into_text().unwrap_or_default();
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if let Some(k) = v.get("kind").and_then(|k| k.as_str()) {
                order.push(k.to_string());
            }
        }
        assert_eq!(
            order,
            vec![
                "template",
                "default_template",
                "channel_roles",
                // WHAT EACH SCREEN WEARS FOR EACH KIND (DECISIONS §97) — with the
                // configuration, before anything that paints, on the same rule as
                // the role map above it.
                "channel_looks",
                // WHICH KINDS EACH SCREEN SHOWS AT ALL (DECISIONS §98) — with the
                // configuration and BEFORE anything that paints, because it can
                // suppress what paints.
                "channel_shows",
                "transition",
                "timer",
                "background",
                "content",
                "screen_state"
            ],
            "the hello reply reached this tablet in the wrong order — the reading \
             must be painted last of the things that paint, after the clock and the \
             backdrop that accompany it, and the screens the operator took down must \
             be named AFTER the reading or the reading would paint over them"
        );
    }

    /// A SCREEN THE OPERATOR TOOK DOWN COMES BACK DOWN.
    ///
    /// Rule 43 in the other direction, and the reason the per-screen state is
    /// retained at all. The operator takes the lobby TV down for the sermon; its
    /// browser source restarts twenty minutes later, says hello, and is handed the
    /// retained verse. Without this frame it would paint that verse and bring
    /// itself back up — rule 43's own mechanism undoing an operator's own control,
    /// in front of the room the control was used to spare.
    ///
    /// Two claims, and the second is the one that is easy to get wrong: the frame
    /// must ARRIVE, and it must arrive AFTER the content. Sent before it, the verse
    /// paints over it and the screen is up again with nothing to say why.
    ///
    /// Both watched to fail: deleting the send (the first assertion), and moving it
    /// above the retained screen frame (the second, which reported the real order).
    #[tokio::test]
    async fn a_screen_the_operator_took_down_rejoins_still_down() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // A verse on the wall, and one screen taken out of it.
        hub.publish(
            r#"{"kind":"content","reference":"Psalms 23:1","text":"The LORD is my shepherd"}"#
                .to_string(),
        );
        hub.publish(r#"{"kind":"screen_state","screens":{"4":"clear"}}"#.to_string());

        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");
        let (mut write, mut read) = ws.split();
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"kind":"hello","channel":4,"template_id":null}"#.to_string(),
            ))
            .await
            .expect("send hello");

        let mut order: Vec<String> = Vec::new();
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_millis(600), read.next()).await
            else {
                break;
            };
            order.push(msg.into_text().unwrap_or_default());
        }
        let down = order
            .iter()
            .position(|m| m.contains(r#""kind":"screen_state""#));
        assert!(
            down.is_some_and(|i| order[i].contains(r#""4":"clear""#)),
            "a lobby TV whose browser source restarted came back UP mid-sermon: {order:?}"
        );
        let content = order
            .iter()
            .position(|m| m.contains(r#""kind":"content""#))
            .expect("the retained verse should still be replayed");
        assert!(
            down.expect("checked above") > content,
            "the operator's decision was sent BEFORE the verse, so the verse painted \
             over it: {order:?}"
        );
    }

    /// A WHOLE-WALL CLEAR DOES NOT PUT A SCREEN BACK UP.
    ///
    /// The tempting simplification, written down so nobody reaches for it: "a panic
    /// control takes everything, so it should reset the per-screen states too". It
    /// must not. A panic control is about what is ON the screens; which screens are
    /// IN the wall is a separate decision the operator took deliberately, and an
    /// `Esc` that silently re-armed the lobby TV would put the next verse in front
    /// of exactly the room the operator had taken it out of. The way back is a
    /// control (`restore_screen`), which is findable, and the Outputs desk says
    /// which screens are down.
    #[test]
    fn a_whole_wall_clear_leaves_the_per_screen_states_alone() {
        let hub = KioskHub::default();
        hub.publish(r#"{"kind":"screen_state","screens":{"4":"black"}}"#.to_string());
        hub.publish(r#"{"kind":"clear"}"#.to_string());

        let retained = hub.screens_down.lock().expect("slot").clone();
        assert!(
            retained.contains(r#""4":"black""#),
            "a whole-wall clear wiped the operator's per-screen decision: {retained}"
        );
        // And the reverse, which is the half that would break rule 43: taking one
        // screen down must never become the frame a late-joining screen is shown
        // INSTEAD of the wall.
        let screen = hub.last_screen.lock().expect("slot").clone();
        assert_eq!(
            screen.as_deref(),
            Some(r#"{"kind":"clear"}"#),
            "a per-screen state became the retained screen frame"
        );
    }

    /// …AND A CHANNEL-KEYED SCREEN IS ONE OF THEM.
    ///
    /// The test above says hello with a `template_id`, and for a long time that was
    /// the only hello the hub answered at all — the whole reply sat inside
    /// `if let Some(id) = template_id`. `Output.svelte` sends `template_id: null`
    /// whenever the URL is CHANNEL-keyed, which is what `Copy URL` writes and what
    /// CLAUDE.md tells operators to use, because only a channel-keyed source follows
    /// a template swap. So the recommended URL was the one shape that joined blank:
    /// measured on a running build, `output.html?channel=1` came back with nothing
    /// after a fire while `?channel=1&template_id=1` came back with the verse.
    ///
    /// That is rule 43's own failure — an OBS source restarting mid-reading showing
    /// a congregation black — reached through the door the documentation points at.
    #[tokio::test]
    async fn a_screen_that_joins_without_a_template_id_is_still_sent_what_is_on_the_screens() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        hub.publish(
            r#"{"kind":"content","reference":"Psalms 23:1","text":"The LORD is my shepherd"}"#
                .to_string(),
        );

        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");
        let (mut write, mut read) = ws.split();
        // EXACTLY what a channel-keyed output page sends.
        write
            .send(tokio_tungstenite::tungstenite::Message::Text(
                r#"{"kind":"hello","template_id":null}"#.to_string(),
            ))
            .await
            .expect("send hello");

        let mut got = None;
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_secs(2), read.next()).await
            else {
                break;
            };
            let text = msg.into_text().unwrap();
            if text.contains(r#""kind":"content""#) {
                got = Some(text);
                break;
            }
        }
        assert!(
            got.as_deref().unwrap_or("").contains("Psalms 23:1"),
            "a channel-keyed screen that joined mid-reading was left blank: {got:?}"
        );
    }

    /// …AND IT CAN NEVER UNDO A PANIC CONTROL.
    ///
    /// The retained frame is whatever was published LAST, and `clear` and `black`
    /// are published through the same door. A screen that joins after the operator
    /// cleared the wall must join a cleared wall — a retained verse that outlived
    /// the control that removed it would be strictly worse than the blank screen
    /// this whole mechanism exists to fix (CLAUDE.md rule 15).
    #[tokio::test]
    async fn a_cleared_wall_stays_cleared_for_a_screen_that_joins_after_it() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        hub.publish(r#"{"kind":"content","reference":"Romans 8:28","text":"x"}"#.to_string());
        hub.publish(r#"{"kind":"clear"}"#.to_string());
        // A monitor-only extra must not stand in for the frame that decides the
        // screen, or the clear would be forgotten by the next "up next" push.
        hub.publish(r#"{"kind":"stage_next","label":"John 3:17"}"#.to_string());

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

        let mut frames = Vec::new();
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_millis(900), read.next()).await
            else {
                break;
            };
            frames.push(msg.into_text().unwrap());
        }
        assert!(
            frames.iter().any(|f| f.contains(r#""kind":"clear""#)),
            "the wall was cleared and the joining screen was not told: {frames:?}"
        );
        assert!(
            !frames.iter().any(|f| f.contains("Romans 8:28")),
            "a verse outlived the control that removed it: {frames:?}"
        );
    }

    /// A WORD TO THE PREACHER MUST NOT STAND IN FOR THE READING — at the SERVER.
    ///
    /// `is_screen_frame` says an alert is not retained and `FRAME_VERDICTS` agrees,
    /// but both are statements about a matcher. This drives the real hello path:
    /// verse, then alert, then a client connects. Two ways it could go wrong and
    /// only one of them is a matcher bug —
    ///
    ///   - the alert becomes the retained frame, and the screen that rejoined
    ///     mid-reading is handed a message meant for a moment that has passed
    ///     instead of the verse it should be painting (rule 43's own failure, with
    ///     the alert in the role of `stage_next`);
    ///   - the alert is retained ALONGSIDE the verse and replayed on hello, which
    ///     would put a red full-screen instruction on a congregation output ten
    ///     minutes after the operator sent it, on a page whose only defence is that
    ///     the message never arrives.
    ///
    /// The second is the congregation-facing one and no matcher test can see it:
    /// it is a property of what `hello` sends.
    #[tokio::test]
    async fn a_word_to_the_preacher_is_not_replayed_to_a_screen_that_joins_after_it() {
        let port = free_port();
        let hub = KioskHub::default();
        tokio::spawn(run_kiosk_server(
            log_only(),
            hub.sender(),
            hub.templates_handle(),
            hub.clients_handle(),
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
            OutputHealth::default(),
            port,
        ));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        hub.publish(
            r#"{"kind":"content","reference":"Romans 8:28","text":"And we know"}"#.to_string(),
        );
        hub.publish(r#"{"kind":"stage_alert","text":"Wrap up — 5 minutes"}"#.to_string());

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

        let mut frames = Vec::new();
        for _ in 0..HELLO_FRAMES {
            let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_millis(900), read.next()).await
            else {
                break;
            };
            frames.push(msg.into_text().unwrap());
        }
        assert!(
            frames.iter().any(|f| f.contains("Romans 8:28")),
            "the alert stood in for the reading and the joining screen was left \
             without it: {frames:?}"
        );
        assert!(
            !frames.iter().any(|f| f.contains("stage_alert")),
            "a word to the preacher was replayed to a screen that joined later — on \
             a congregation output that is a red screen nobody sent: {frames:?}"
        );
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
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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

        // BOTH HALVES OF THE COUNTDOWN, for the same reason `next_*` are above: a
        // kiosk/OBS screen that gets only the instant goes on counting down through a
        // countdown the operator has HELD, and a projector counting against a console
        // that says 4:00 is worse than a blank one — nothing about it looks wrong.
        // `countdown_from` rides too, or a kiosk screen turns the figure red at a
        // different moment from the native window beside it.
        let held = OutputContent {
            kind: Some("countdown".into()),
            reference: "Service begins in".into(),
            countdown_to: Some(1_700_000_300_000),
            countdown_from: Some(1_700_000_000_000),
            countdown_paused_ms: Some(240_000),
            countdown_done: Some("Welcome".into()),
            ..Default::default()
        };
        let v: serde_json::Value = serde_json::from_str(&kiosk_content_json(&held)).unwrap();
        assert_eq!(v["countdown_to"], 1_700_000_300_000_i64);
        assert_eq!(v["countdown_from"], 1_700_000_000_000_i64);
        assert_eq!(v["countdown_paused_ms"], 240_000);
        assert_eq!(v["countdown_done"], "Welcome");
    }

    /// WHEN TO WORRY CROSSES THE WIRE TOO — RG-149, the kiosk half.
    ///
    /// Two separate facts and they are deliberately two fields. `countdown_warn_ms`
    /// is a threshold somebody CHOSE for this countdown; `countdown_warn_default_ms`
    /// is the figure in `Settings → General → Countdown warning`, which a browser
    /// source cannot read because it has no bridge and no console state. Ranking
    /// them stays `layers.js::countdownWarning`'s job on the far side — chosen, else
    /// configured, else the tenth-of-span rule — so neither of these is a second
    /// reading of when to worry.
    ///
    /// Same reasoning as `next_*` and `countdown_from` above: a field dropped from
    /// THIS json and present on the Tauri struct means the browser source in OBS and
    /// the projector on HDMI turn red at different moments, in the same room.
    #[test]
    fn the_warning_window_reaches_a_kiosk_screen_as_well_as_a_native_one() {
        let c = OutputContent {
            kind: Some("countdown".into()),
            reference: "Service begins in".into(),
            countdown_to: Some(1_700_000_300_000),
            countdown_from: Some(1_700_000_000_000),
            countdown_warn_ms: Some(120_000),
            countdown_warn_default_ms: Some(150_000),
            ..Default::default()
        };
        let v: serde_json::Value = serde_json::from_str(&kiosk_content_json(&c)).unwrap();
        assert_eq!(
            v["countdown_warn_ms"], 120_000,
            "a threshold chosen for this countdown never left the machine"
        );
        assert_eq!(
            v["countdown_warn_default_ms"], 150_000,
            "the configured default never left the machine, so every screen \
             without a bridge keeps the shipped minute"
        );

        // AND IT IS THE RETAINED FRAME, so a screen that joins mid-service is sent
        // the figure with the content rather than warning at the wrong moment until
        // the next fire (rule 43). The replay itself is held by
        // `a_client_that_connects_mid_service_is_sent_what_is_on_the_screens`; what
        // this asserts is that the carrier is the frame that replay retains.
        assert!(is_screen_frame(&kiosk_content_json(&c)));

        // Absent is absent. A zero would be a warning window that never opens.
        let bare = OutputContent {
            kind: Some("countdown".into()),
            countdown_to: Some(1_700_000_300_000),
            ..Default::default()
        };
        let v: serde_json::Value = serde_json::from_str(&kiosk_content_json(&bare)).unwrap();
        assert_eq!(v["countdown_warn_ms"], serde_json::Value::Null);
        assert_eq!(v["countdown_warn_default_ms"], serde_json::Value::Null);
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
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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
            hub.default_template_handle(),
            hub.channel_roles_handle(),
            hub.channel_looks_handle(),
            hub.channel_templates_handle(),
            hub.channel_shows_handle(),
            hub.last_screen_handle(),
            hub.last_screen_by_channel_handle(),
            hub.last_transition_handle(),
            hub.last_timers_handle(),
            hub.last_background_handle(),
            hub.last_stage_media_handle(),
            hub.last_media_transport_handle(),
            hub.screens_down_handle(),
            hub.look_ids_handle(),
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
        h.beat(7, PaintState::Content, "window", BeatGap::default(), None);
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
    /// 2026-09-21 · O-4 / M-3. A screen whose picture did not load must not read
    /// On Air. The page says so on the beat; the desk must be able to read it back,
    /// and a later beat that says nothing must clear it (the failure healed, or a
    /// new clip replaced it).
    #[test]
    fn a_beat_carries_a_media_failure_and_a_clean_beat_clears_it() {
        let h = OutputHealth::default();
        h.beat(7, PaintState::Content, "window", BeatGap::default(), None);
        h.note_media_error(7, Some("video not loading · http://x:8032/media/1".into()));
        assert_eq!(
            h.media_error_of(7).as_deref(),
            Some("video not loading · http://x:8032/media/1")
        );
        h.beat(7, PaintState::Content, "window", BeatGap::default(), None);
        h.note_media_error(7, None);
        assert_eq!(h.media_error_of(7), None);
    }

    /// …and the kiosk door reads the same field off the JSON frame.
    #[test]
    fn a_kiosk_beat_names_its_media_failure() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"kind":"beat","channel":3,"state":"content","media_error":"image not loading · http://x:8032/media/3"}"#,
        )
        .unwrap();
        assert_eq!(
            media_error_from_json(&v).as_deref(),
            Some("image not loading · http://x:8032/media/3")
        );
        let clean: serde_json::Value =
            serde_json::from_str(r#"{"kind":"beat","channel":3,"state":"content"}"#).unwrap();
        assert_eq!(media_error_from_json(&clean), None);
        // Bounded: a page cannot push a novel through the desk's status row.
        let long = format!(
            r#"{{"kind":"beat","channel":3,"state":"content","media_error":"{}"}}"#,
            "x".repeat(5000)
        );
        let long: serde_json::Value = serde_json::from_str(&long).unwrap();
        assert!(media_error_from_json(&long).unwrap().len() <= 300);
    }

    /// 2026-09-21 · O-3 (RG-195). A kiosk client that fell more than the
    /// broadcast buffer behind hit `RecvError::Lagged` and silently skipped
    /// frames — a `clear` among them is a panic control that did not land on one
    /// screen. Rule 33: every queue on the path counts what it sheds. The count
    /// rides the status row so the desk can say "this screen was re-synced".
    #[test]
    fn a_lagging_screen_is_counted_and_the_count_survives_a_beat() {
        let h = OutputHealth::default();
        assert_eq!(h.resyncs_of(7), 0);
        h.note_resync(7);
        h.note_resync(7);
        h.beat(7, PaintState::Content, "kiosk", BeatGap::default(), None);
        assert_eq!(h.resyncs_of(7), 2);
        assert_eq!(h.resyncs_of(8), 0, "another screen is not blamed");
    }

    #[test]
    fn a_preview_with_no_channel_reports_nothing() {
        let h = OutputHealth::default();
        h.beat(0, PaintState::Content, "window", BeatGap::default(), None);
        h.beat(-1, PaintState::Content, "window", BeatGap::default(), None);
        assert!(h.read(0).is_none());
        assert!(h.read(-1).is_none());
    }

    /// A WINDOW STILL LOADING IS NOT A FAULT, AND ONE THAT NEVER LOADS STILL IS.
    ///
    /// RG-119. A window is attached the instant it is created and its page has to
    /// load before it can beat, so the first status poll used to write
    /// `output_lost` into the permanent service record, followed by
    /// `output_recovered` a fraction of a second later. Both real services on
    /// 2026-09-06 opened with that pair (4.9 s to 5.0 s, and 1457.7 s to 1459.7 s
    /// the service before), History renders it as "Screen stopped responding", and
    /// the Sunday report counts it. A fault that appears every single time is one
    /// an operator learns to scroll past.
    ///
    /// The grace may not swallow the failure it resembles, so this asserts both
    /// halves: silent inside the window, and reported once the window has passed.
    #[test]
    fn a_screen_that_has_not_answered_yet_is_not_a_fault_until_it_has_had_time() {
        let h = OutputHealth::default();
        assert_eq!(h.transition(9), None, "still loading is not a fault");
        assert_eq!(
            h.transition(9),
            None,
            "and it does not become one by polling"
        );

        // Backdate the moment it was first seen, which is the only thing that
        // separates "has not answered yet" from "is not answering". Reaching in
        // rather than sleeping through `BEAT_STALE_MS` keeps this a unit test.
        {
            let mut seen = h.first_seen.lock().expect("lock");
            let old =
                std::time::Instant::now() - std::time::Duration::from_millis(BEAT_STALE_MS * 2);
            seen.insert(9, old);
        }
        assert_eq!(
            h.transition(9),
            Some(false),
            "a page that never loads at all must still be reported"
        );
        assert_eq!(h.transition(9), None, "and reported exactly once");
    }

    /// A SCREEN THAT ANSWERED AND THEN STOPPED IS THE CASE THIS WAS BUILT FOR,
    /// and the grace above must not have bought it any silence.
    #[test]
    fn a_screen_that_answered_and_then_went_quiet_is_reported_at_once() {
        let h = OutputHealth::default();
        h.beat(9, PaintState::Content, "window", BeatGap::default(), None);
        assert_eq!(
            h.transition(9),
            None,
            "a healthy first sighting is not news"
        );

        // Age the beat past the staleness window without waiting for it.
        {
            let mut m = h.beats.lock().expect("lock");
            let b = m.get_mut(&9).expect("beat");
            b.at = std::time::Instant::now() - std::time::Duration::from_millis(BEAT_STALE_MS * 2);
        }
        assert_eq!(h.transition(9), Some(false));
        h.beat(9, PaintState::Content, "window", BeatGap::default(), None);
        assert_eq!(h.transition(9), Some(true));
    }

    /// THE SCREEN'S OWN ACCOUNT OF ITS SILENCE SURVIVES BOTH DOORS (RG-119).
    ///
    /// The distinction this carries is the whole point: about one interval means
    /// the page kept ticking and the beats were lost on the way, which is Relay's
    /// fault; minutes mean the page was not running at all, which is a screen that
    /// genuinely was not painting. A defaulted zero would erase exactly that.
    #[test]
    fn a_beat_carries_what_the_screen_said_about_its_own_silence() {
        let h = OutputHealth::default();
        h.beat(9, PaintState::Content, "window", BeatGap::default(), None);
        assert_eq!(h.last_gap(9), Some(BeatGap::default()));
        assert_eq!(
            h.last_gap(9).and_then(|g| g.describe()),
            None,
            "a screen that said nothing must not be quoted as saying zero"
        );

        h.beat(
            9,
            PaintState::Content,
            "window",
            BeatGap::clamped(Some(641_000), Some(641_000)),
            None,
        );
        assert_eq!(
            h.last_gap(9).and_then(|g| g.describe()).as_deref(),
            Some("screen's own clock: silent 641s, hidden 641s")
        );

        h.beat(
            9,
            PaintState::Content,
            "window",
            BeatGap::clamped(Some(2_000), Some(0)),
            None,
        );
        assert_eq!(
            h.last_gap(9).and_then(|g| g.describe()).as_deref(),
            Some("screen's own clock: silent 2s, never hidden")
        );
    }

    /// A NUMBER OFF THE LAN IS STILL UNTRUSTED INPUT.
    ///
    /// It only ever becomes a phrase in a timeline entry, but a client can send
    /// anything, and a year of milliseconds in a service record is not evidence.
    #[test]
    fn a_nonsense_gap_is_dropped_rather_than_believed() {
        for bad in [
            serde_json::json!({"since_ms": -1}),
            serde_json::json!({"since_ms": "641000"}),
            serde_json::json!({"since_ms": GAP_CLAMP_MS + 1}),
            serde_json::json!({"since_ms": 1.5}),
            serde_json::json!({}),
        ] {
            assert_eq!(
                BeatGap::from_json(&bad),
                BeatGap::default(),
                "not evidence: {bad}"
            );
        }
        assert_eq!(
            BeatGap::from_json(&serde_json::json!({"since_ms": 4000, "hidden_ms": 4000})),
            BeatGap {
                since_ms: Some(4_000),
                hidden_ms: Some(4_000)
            }
        );
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
        h.beat(3, PaintState::Black, "kiosk", BeatGap::default(), None);
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
        h.beat(2, PaintState::Content, "window", BeatGap::default(), None);
        h.beat(2, PaintState::Black, "kiosk", BeatGap::default(), None);
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
