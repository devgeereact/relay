# Channels (§10 Output Management) — design loop log

Reference: `docs/relaydesign/relay-channels-screen.png` (hi-fi; supersedes the
panel-4 crop the loop prompt pointed at).

Gate: `npm run build` clean, **263 frontend**, **355 Rust** (+15), `cargo fmt` +
`clippy -D warnings` clean, detection scorecard unchanged (100% recall, 0 wrong
verses).

Compare: **pixel**, 3 iterations at 1536×1024 plus a native-channel and a
1280×800 variant. Captured from the Vite webview with a stubbed
`window.__TAURI_INTERNALS__.invoke`. **No app code was changed for the capture.**

Screens: `channels-baseline.png` (before) → `channels-3.png` (final), plus
`channels-native.png`, `channels-narrow.png`.

---

## The central finding: almost none of this screen's data existed

The reference shows, per channel: latency (24ms / 18ms / 16ms), resolution,
frame rate, uptime, bandwidth (2.4 Mbps), dropped frames (0.00%), "Last Seen",
priority, a Test Connection button, and a health verdict of "Excellent".

**None of it exists anywhere in Relay.** Nothing in the pipeline times a
delivery, counts a frame, or records a connect time. `telemetry.rs` is crash
reporting only. The nearest analogue to a dropped frame is
`Err(RecvError::Lagged(_)) => continue` in the WS task, which silently skips and
counts nothing.

So the choice on this screen was: invent plausible numbers, or say what is
actually known. On the one screen an operator uses to decide *whether the
projector is working*, invented numbers are the worst possible decoration.

## Three real bugs found on the way in

1. **`output_channels.status` was write-once dead data.** Both INSERTs hardcode
   `'offline'` and there is no `UPDATE … SET status` anywhere in the tree. Every
   channel reported offline forever — including one filling a projector. The old
   UI never read the column, so nothing lied on screen; but this screen exists to
   show exactly that, so it was a trap primed to go off.
2. **`open_channel_output` minted `output-{n}` from a counter.** Opening one
   channel twice produced a SECOND fullscreen window for it with nothing to
   notice, and no label could be traced back to a channel — so the app could not
   answer "does this channel have a window open?" at all.
3. **A channel's assigned display was silently ignored.** `seed_channels` wrote
   `display_target = "Display 1"`; `open_channel_output` parsed it with a plain
   `parse::<usize>()`, got `None`, and fell back to the primary display without
   reporting anything. On a two-screen booth that puts the congregation's verse
   on the operator's own monitor.

## Liveness, made real

Approved as full scope. The online light is now **computed from the running app**
and never read from the DB column:

- **Native channels** — the window label is now deterministic
  (`channels::channel_label` → `output-ch{id}`), so `open_channel_ids` answers
  which channels have a window open. This also makes `open_native_window`'s
  existing already-open check a duplicate guard for free, fixing bug 2.
- **Networked channels** — the WS hub already knew each client's `template_id`,
  but kept it on the task's own stack. It is now registered in a
  `ClientRegistry` (a count per template), so "3 clients showing this channel's
  template" is a real number. Registration is held by a **drop guard**, so a
  client is released however the task ends — break, error, or panic. Without
  that, a kiosk reconnecting across a service leaves a phantom counted on every
  previous connection and the channel reads LIVE with a dead screen.
- **NDI** — reports `supported: false` and renders **UNAVAILABLE**, a different
  claim from offline. NDI is parked, not broken.

`display_target` parsing now accepts both `"Display 1"` (1-based human label) and
a bare `"0"` (0-based index, what `set_channel_display` writes), and the seed
writes the canonical form. Bug 3 fixed.

A **count, not a client list**: Relay records no address, identity, or connect
time for a kiosk client, so a count is the most that can honestly be reported —
and it is enough to answer the only question being asked.

Liveness is **polled at 2s**, not pushed: nothing raises an event when a kiosk
connects or a window closes, so the honest options were polling or a status that
goes stale.

## Display names an operator can recognise

Raised mid-build, then corrected again after a real-hardware check.

**The first pass was wrong about macOS.** It assumed macOS reported real product
names and passed them through. It does not. tao names a macOS monitor:

```rust
// tao-0.35.3/src/platform_impl/macos/monitor.rs:205
let screen_num = CGDisplay::new(display_id).model_number();
Some(format!("Monitor #{}", screen_num))
```

— a raw EDID **model number**. So the display picker offered
**"Monitor #1234555"** for a screen macOS itself calls **"HP 532sf"**, and the
first humaniser passed it straight through because it matched none of its
patterns. On the one control that decides which physical screen a congregation
sees, that is a mis-send waiting to happen.

**The fix.** The name macOS shows in System Settings › Displays is
`NSScreen.localizedName` (10.15+). `list_monitors` now reads it and matches it
back to Tauri's monitor list **by position**, because Tauri drops the native
display id — its `Monitor` keeps only name/size/position/work_area/scale_factor.
tao computes position as `CGDisplayBounds(id).origin * scale_factor`, so
`collect_macos_display_names` performs the identical arithmetic and the keys line
up exactly. It runs via `run_on_main_thread` (AppKit is main-thread-only) with a
500 ms bound, and degrades to the generic humaniser on any failure rather than to
an empty display list.

**Verified on real hardware**, which is the only thing that could settle it —
`cargo run --example displays` on the reporting machine:

```
1 display(s) attached

[0] HP 532sf
     localizedName : HP 532sf   <- what Relay now shows
     CGDisplayID   : 6
     bounds        : 1920x1080 at (0, 0)  scale 1
     match key     : (0, 0)   <- must equal Tauri's monitor position
```

`src-tauri/examples/displays.rs` is kept as the standing probe: it prints the
resolved name, the CGDisplayID, the bounds and the match key, so a mismatch can
be diagnosed on the machine it happens on rather than argued about. It is a
standalone example and not a test because `NSScreen` must be read from the main
thread, which is where an example's `main` runs and a test's body does not.

Elsewhere `humanize_monitor_name`:

- passes a **real product name through untouched**;
- turns a Windows device path into `Display 1`, never showing the path;
- refuses to pass `Monitor #<digits>` through — the safety net for when the
  AppKit lookup finds nothing, so an EDID model number can never reach the
  operator by any route;
- turns a connector into the socket it names — `HDMI-1` → **HDMI 1**, `DP-2` →
  **DisplayPort 2**, `HDMI-A-1` → **HDMI 1**;
- says **Built-in display** for `eDP`/`LVDS` — the laptop panel, which is the one
  display the congregation's output usually must *not* go to;
- falls back to `Display N` only when there is nothing else.

A product name that merely starts with connector letters ("HDMI Splitter Pro")
survives intact — there is a test pinning that, because the naive rule eats it.

**What this still cannot do**, and the log should say so plainly:

- **Windows** has no equivalent fix here. `\\.\DISPLAY1` → the monitor's actual
  model needs an EDID/device-registry lookup Tauri does not expose, and no
  Windows machine was available to test one against. The operator gets
  "Display 1" *plus its resolution and primary flag*, which distinguishes two
  screens without claiming a make. **This is the remaining half of the same
  bug** — worth doing when a Windows box is at hand.
- **Multi-display matching is unproven.** The reporting machine has one screen at
  `(0, 0)`, so the position match was trivially satisfied. The arithmetic mirrors
  tao's line for line, but a second display — especially a Retina/non-Retina mix,
  where the scale factors differ — is the case that would expose an error. Run
  `cargo run --example displays` on a two-screen setup and check each `match key`
  against the picker before trusting it there.

## Departures from the reference

**Refused — no backing data, and inventing it would be worst here:**

- **CHANNEL HEALTH** (bandwidth · dropped frames · uptime · "Excellent") with
  sparklines. Replaced by a panel that states the limit in words: Relay reports
  whether a window is open and how many clients are connected, and does *not*
  measure latency, bandwidth, frame rate or dropped frames — so a channel reading
  LIVE means something is attached, not that the picture is good.
- **Latency / Frame Rate / Last Seen** rows in CHANNEL INFO — nothing times or
  timestamps anything.
- **Test Connection** — there is no ping, health, or reachability probe for an
  output channel anywhere. (`sysprobe::probe_integrations` TCP-probes *loopback*
  4455/9910 for OBS/ATEM and never touches 8031/8032.) A button that always
  reported success would be worse than no button.
- **Resolution / FPS per row** — shown **only** for a native channel with a
  display assigned, because that is the one case Relay knows one: it is the
  monitor's size, read from the OS. A networked channel's resolution is a
  property of the browser source at the far end, which Relay has never been told.

**Not built (decided):**

- **PRIORITY column with per-channel 1–7 reorder.** Priority implies channels are
  ordered or preempt each other. They are not: every channel renders the same
  broadcast simultaneously. A priority control that changes nothing is a lie with
  a dropdown on it.

**Substituted:**

- **Tabs "Network" + "Browser Sources" → one "Network".** A browser source *is* a
  network client in Relay's model; splitting them would imply a distinction the
  engine does not make. Tabs are the real taxonomy: All · Network · Native
  Windows · NDI.
- **Inspector tabs Overview / Settings / Preview / Advanced → one scroll.**
  "Settings" and "Advanced" have no content that exists; Preview is always shown
  rather than hidden behind a tab.
- **STATUS reads LIVE / IDLE / UNAVAILABLE**, not ONLINE/OFFLINE — "idle" is
  accurate for a correctly-configured channel with nothing attached yet, where
  "offline" sounds like a fault.
- Status is **green**, not amber. Green is "confirmed / connected" in the design
  sheet's own usage guide; amber means something is on the wall, and a channel
  being live does not put it there. Selection is amethyst for the same reason.

**Kept, though not in the reference:** the preacher's stage-remote card. It is
real and is the one output a church sets up by hand every week.

## A fourth bug, found by the pixels

The stage remote printed **`http://localhost:8032/stage.html`** and told the
operator to open it on a phone — where `localhost` is the phone. `{stageUrl()}`
was a function call in the markup, and Svelte tracks the *identifiers* in a
template expression: that one names `stageUrl`, not `lanIp`, so it rendered once
before `local_ip` resolved and never again. The QR was generated on click and so
was correct; only the address a human would actually type was wrong. Now a
reactive `$:` that names `lanIp`. The inspector's Address had the same latent
shape and was correct only by luck of ordering — also made reactive (`selAddr`).

## 15 new Rust tests

Registry: a client counted while connected and **not after** (verified by
disabling the drop guard — fails with `left: 1, right: 0`), re-hello moving a
client rather than double-counting, no underflow. Labels: round-trip, and an
ad-hoc `output-1` never mistaken for a channel. Naming: six cases covering
product names, Windows paths, connectors, the built-in panel, the
name-that-looks-like-a-connector, and blanks. Display targets: four cases
covering the human label, the bare index, unreadable input, and `Display 0`
underflow.

## Left as is

- **The dead `status` column stays in the schema.** Dropping it needs a table
  rebuild to remove the CHECK constraint — the exact path CLAUDE.md §25 warns
  about, and the one that once bricked every subsequent boot. It is now unread by
  anything, and both the column and the code that supersedes it say so.
- The reference's per-row overflow (⋮) menu — its actions are all present as
  buttons in the inspector.
- `Scripture Center v3` truncates in the TEMPLATE column on two rows; the
  inspector shows it in full.

## Not verified here

Vite webview under a stubbed IPC layer. **Not exercised**: a real kiosk client
connecting over the real WS hub (the registry is covered by a Rust integration
test against a real socket, but not end-to-end from a browser); real multi-monitor
enumeration, so `humanize_monitor_name` is proven only against the strings each
platform is documented to return, not against this machine's actual second
screen; and the packaged-build CSP.
