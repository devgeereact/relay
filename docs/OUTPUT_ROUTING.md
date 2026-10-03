# Relay — The Output Routing Map

**How do I get Relay onto my screens?**

Relay produces pictures in exactly two ways, and between them they reach every
setup a church actually has.

| You have | Use | What Relay gives you |
|---|---|---|
| A projector or a TV on a cable | **Native window** | A fullscreen picture on that display, over HDMI |
| An ATEM or another video switcher | **Native window**, plus the right cable | The same HDMI picture, into the switcher's input |
| OBS, vMix, or a smart TV / Raspberry Pi in the foyer | **Network client** | A web address you paste into a browser source |
| More screens than your laptop has ports | **Network client** | A screen that costs no video port at all |

Both are set up in the same place: **Outputs → Screens → ＋ Add Screen**, then
pick `Native window (HDMI / display)` or `Network client (OBS / kiosk)`.

Everything below is the detail behind those four rows, including the things that
do **not** work and are worth knowing before you buy anything.

---

## 1. A projector or a TV

This is the ordinary case and it needs no extra kit.

Plug the projector or TV into your laptop as a second display. In **Outputs →
Screens**, add a screen with the type **Native window**, set its **Display** to the
one the projector is on, and **press Open**. That third step is the one that puts
the picture up: Relay opens a borderless window inside that display's bounds and
fullscreens it, so the OS puts it on the monitor you chose rather than on
whichever one it feels like.

You press **Open** once. After that it restores itself at every launch, so on a
normal Sunday there is nothing to press at all (see below).

Three things follow from how this is built, and they are worth knowing:

- **It comes back by itself, from the next launch onwards.** The display
  assignment lives in Relay's local database, so after a restart, an update or a
  crash, the screen reopens on the same projector with no clicks. The restore runs
  at launch and only at launch, which is why the very first time you set a screen
  up you have to press **Open** yourself. On a one-monitor desk nothing auto-opens;
  plug the projector in and its screen restores itself.
- **It will never cover your console.** Relay refuses to auto-open an output onto
  the primary display. Covering the operator's own screen mid-service is worse
  than showing nothing.
- **The window backdrop is black, not white.** The page itself is transparent so
  a lower-third template keys out cleanly in OBS, but on a real projector a
  transparent page showed the webview's white background around the band. The
  native window paints black behind it; the browser-source version stays truly
  transparent.

**What Relay supplies:** a plain display signal, exactly like a slideshow.
**What you supply:** the cable, and the projector.

---

## 2. An ATEM (or any hardware switcher)

Relay feeds an ATEM the same way it feeds a projector: **a native output window on
a display, over HDMI.** There is nothing special to configure in Relay. If your
switcher takes HDMI, you are finished; if it takes SDI, you need one small
converter, and that is the whole story.

### Which ATEM takes what

| Family | Inputs | What you need |
|---|---|---|
| ATEM Mini, Mini Pro, Mini Extreme (all Mini models) | **HDMI only**, no SDI inputs | An HDMI cable. Nothing else |
| Rack-mount ATEMs (Television Studio HD8, Constellation, and the rest) | **SDI only**, no HDMI inputs | One HDMI-to-SDI converter |

A trap on the rack-mount models: the **ATEM Television Studio HD8 has a single
HDMI connector, and it is an output.** It is a monitoring port, not a way in.
Plugging Relay into it does nothing.

### The converter

A **Blackmagic Micro Converter HDMI to SDI 3G**, about $75, takes Relay's HDMI and
gives you SDI. It is bus-powered, it is the size of a matchbox, and it dissolves
the entire SDI question. This is why Relay's "no native SDI hardware" rule costs a
church nothing: Relay emits a normal HDMI display signal, and a dongle cheaper
than a microphone cable does the rest. See [SPEC.md](SPEC.md) §9.

### Three things people are told to try that do not work

- **No ATEM accepts NDI.** Not one model, not with a licence, not with a
  firmware update. Blackmagic's IP direction is SMPTE 2110, which is a different
  standard on different hardware. NDI would buy you OBS, vMix, TriCaster and
  ProPresenter, and none of those is what the ATEM question is about. (NDI is
  parked in Relay anyway; see the last section.)
- **The ATEM Media Player is not a live input.** It holds **stills**, twenty
  slots, uploaded at roughly three seconds per 1080p frame, RLE-compressed so
  dense text is slower still, and the pool locks while an upload runs. It is a
  pre-service graphics store. It cannot show a verse the preacher just named.
- **SuperSource is not a way in.** It composites sources that are **already**
  connected to the switcher. It adds no input; it rearranges the ones you have.

### And one that people expect to work

**A virtual camera is not a signal.** Both the Windows and macOS virtual-camera
APIs register a device with the local operating system's camera stack. Nothing
reaches a cable, so nothing reaches a switcher in another rack. Syphon and Spout
are the same story: same-machine texture sharing. On macOS the virtual-camera
route additionally needs a restricted Apple entitlement, which sits behind a
code-signing certificate Relay does not yet have (see
[qa/RELAY_GAP.md](qa/RELAY_GAP.md) RG-73).

**What Relay supplies:** an HDMI display signal.
**What you supply:** a cable, and on an SDI switcher, one converter.

---

## 3. OBS, vMix, or a kiosk screen

This is the path most churches should look at first, and the reason is one
sentence: **a network screen costs zero video ports.** It is a web page served by
Relay over your own Wi-Fi or LAN, so it does not occupy an HDMI socket, a GPU
display pipe, or a dock.

In **Outputs → Screens**, add a screen with the type **Network client**. Then
press that screen's **Copy URL** and paste it into an OBS or vMix browser source,
or open it in a browser on the other machine. The address looks like this:

```
http://<this-computer>:8032/output.html?channel=<id>&template_id=<n>
```

Relay shows you the computer's own address in **Outputs → Sharing**, along with
the two ports it uses:

- **`:8032`** serves the output and stage pages over plain HTTP.
- **`:8031`** is the WebSocket that pushes live changes to those pages.

**Use Copy URL rather than typing an address.** The URL is keyed on `channel`,
and that is what makes a template swap live: change a screen's look in Relay and
the page repaints itself, with nobody re-copying anything. A hand-built address
carrying only `template_id` will render once and then never change again
([DECISIONS.md](DECISIONS.md) §29).

The same address and the same rule are in [USER_GUIDE.md](USER_GUIDE.md)'s
troubleshooting table, which is where an operator looks when a screen is blank.
**Change one and change the other**: this exact fact has drifted here before, when
both documents said `:5032` for months.

Two more things that matter on a Sunday:

- **The page is transparent.** In an OBS browser source it keys out, so a
  lower-third template sits over your camera with no black box behind it.
- **A screen that joins late is caught up.** An OBS source restarting, a kiosk
  page reloading, or a lobby TV dropping off the Wi-Fi comes back showing what is
  on the screens right now, not black until the next verse.

If Relay's kiosk page is hosted somewhere else, say on a Pi running its own
signage page, the handshake will be refused because the page did not come from
Relay. The refusal names the environment variable that allows it,
`RELAY_KIOSK_ANY_ORIGIN=1`, in the message it prints.

**What Relay supplies:** a web page, live, over your own network.
**What you supply:** a network both machines are on.

---

## 4. More screens than ports

You have a main screen, a stage monitor and a foyer TV, and a laptop with one
HDMI socket. This is the most common hardware problem in the building.

**The answer is section 3.** Make the extra screens network clients. A browser
source on the OBS machine, a smart TV pointed at the URL, a cheap Pi in the foyer:
none of them touches a video port on the operator's laptop, and all of them
update at the same instant as the main screen. Relay is at its best here because
each screen carries its own template, so the stage monitor can show plain white
text on black while the wall shows the full design.

**A monitor for the PREACHER is a different address, though, and this is the
setup mistake that costs a Sunday.** The `output.html` URL in section 3 renders
a stage-shaped template perfectly well and **cannot show a Stage Timer or be
sent a Stage Message** — those exist only on `stage.html`, which is handed out
from **Outputs → Sharing** once a screen's Role is *Stage display*. A confidence
monitor in the booth is happy on either; the platform wants the stage address.
[USER_GUIDE.md](USER_GUIDE.md) §"Screens for the platform and the booth" has the
whole distinction.

Where it matters, the hardware limits are these.

**On a Mac, the chip decides, and no adapter changes it.** Apple Silicon does not
support DisplayPort MST extended desktop on **any** chip, so a multi-display hub
will mirror rather than extend.

**Read the "lid" column before you buy anything.** On a base M3 the second
external display only works with the laptop closed, so it is two screens in
total, not three. A church that buys an M3 Air expecting its own screen plus a
projector plus a foyer TV gets a choice of two of the three. That is the purchase
this section exists to prevent.

**These are LAPTOP figures**, because the operator's machine is usually a laptop
and the lid is the whole catch. A desktop Mac is a different table; see below.

| Laptop chip | External displays | Lid |
|---|---|---|
| M1, M2 (base) | one | either |
| M3 (base) | two | **closed only.** Open the lid and the second external display goes dark |
| M4 (base) | two | either. Closing it adds nothing |
| Pro tiers | two or more, varies by model | check the model |
| Max tiers | up to four | check the model |

**A Mac mini is not the same machine, and it is often the better console.** A
church buying a dedicated box rather than using somebody's laptop gets more
outputs, not fewer, and there is no lid to argue with:

| Mac mini | External displays |
|---|---|
| M2 (2023) | two |
| M4 (2024) | three |
| M1 and other older models | Apple's page does not state it; check that model's tech specs |

Checked against Apple's own support pages in September 2026:
[Use dual monitors with M3 MacBook Air and MacBook Pro](https://support.apple.com/en-us/117373)
for the M3 lid-closed procedure,
[How many displays can be connected to MacBook Air](https://support.apple.com/en-us/122212)
for the M4, and
[How many displays can be connected to Mac mini](https://support.apple.com/en-us/102194)
for the two mini rows. The Pro and Max laptop rows are not qualified here because
they vary by model and year; check the specific machine.

**DisplayLink is the only real workaround, and it has a cost worth knowing.**
A DisplayLink dock adds displays in software rather than through the GPU's
pipes. On macOS it needs the Screen Recording permission, and granting it
**disables HDCP system-wide**. For scripture pages that is harmless. For a church
that also plays a licensed clip from a streaming service, it is a trap: the clip
will refuse to play, on every display, and nothing will explain why.

**On Windows, MST works natively.** An MST hub genuinely extends the desktop, so
the port count is less often the wall it is on a Mac.

**What Relay supplies:** as many independently-templated screens as you want.
**What you supply:** the network, or, if you insist on cables, the ports.

---

## What Relay does not do, and why

**NDI is parked.** `open_ndi_output` returns a clear error rather than pretending,
and it names the two things that do work in its place. NDI needs Vizrt's
proprietary SDK, and there is no pure-Rust crate for it.

It is worth recording that this is not an impossible licensing wall. Vizrt
documents `NDIlib_v5_load()` runtime loading as being "of value in Open-Source
projects", and provides a redistributable-URL constant for the case where the
runtime is absent on a user's machine, so an MIT-licensed application can support
NDI without shipping a proprietary byte.

It stays parked because of what it would and would not buy. It would reach OBS,
vMix, TriCaster and ProPresenter, all of which already work through section 3 at
no cost. It would not reach an ATEM, because no ATEM accepts NDI, and the ATEM is
what churches ask about. See [KNOWN_ISSUES.md](KNOWN_ISSUES.md) §2.

**SDI is out of scope permanently**, unless a human explicitly reopens it. Real
SDI input and output needs dedicated hardware and a C++ SDK: high cost, narrow
reach, and the gap is closed by a converter, which a church may already own or
can buy for about the price of a microphone cable. This sentence used to say the
church "already owns" one, which is the assumption that made the original ATEM
claim false and cannot be true and cost $75 in the same breath — §2 of this file
says plainly that a rack-mount ATEM needs one you go and buy. Section 2 is the
whole answer.

**Relay does not measure your video path.** It knows whether a screen is
answering and what it says it last painted. It does not time a delivery, count a
dropped frame, or record a connect time, and it will not show you a bandwidth
figure it did not measure.

---

## Where the hardware figures came from, and when

Everything in this document about **Relay** was read out of this repository's own
source and is checked by its tests.

Everything about **other people's hardware** was not, and it is worth being exact
about where it came from.

The ATEM model lists, the Media Player's upload speed and slot count, and the $75
converter price were read out of
[superpowers/specs/2026-09-15-timers-templates-stage-design.md](superpowers/specs/2026-09-15-timers-templates-stage-design.md)
§1.4, which records them as confirmed against Blackmagic's own tech-spec pages in
**September 2026**. They were not re-checked against those pages while this
document was written. Blackmagic's product pages are the place to check them:
[ATEM switchers](https://www.blackmagicdesign.com/products/atem) and
[Micro Converters](https://www.blackmagicdesign.com/products/microconverters).
The two Apple Silicon rows in §4 were checked directly, and cite the support
pages they came from.

All of it is a snapshot, not a standing truth: a price moves, a model list grows,
and Apple ships a new chip every year. The **shapes** are the durable part and are
what this document is really claiming, because they follow from how the products
are built rather than from a spec sheet:

- HDMI on the small ATEMs, SDI on the rack-mount ones, and an HDMI-to-SDI
  converter bridging between them for the price of a microphone cable.
- No ATEM ingesting NDI, because Blackmagic's IP direction is a different
  standard.
- The Media Player being a pre-service stills pool rather than a live input.
- A network screen costing no video port, which is why it is the answer whenever
  ports run out.

Before spending money on a specific model or a specific Mac, check that model's
current page. If you find one of these facts has changed, correct it here rather
than working around it somewhere else.
