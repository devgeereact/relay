# Stage, timers and the preacher's phone — the rehearsal script

**Status: NOT RUN.** This is phase 6 of
[`superpowers/plans/2026-09-19-stage-timers-mobile.md`](../superpowers/plans/2026-09-19-stage-timers-mobile.md).
Everything phases 0–5 changed was verified by unit tests, in-process WebSockets and a real
SQLite. **None of it has been seen by a phone, a projector or a packaged build.** This
document is what turns that into evidence.

Work through it in order — the early steps are prerequisites for the later ones. Record what
you see beside each check, including the ones that pass. A pass nobody wrote down is a pass
nobody can cite later.

---

## 0. Before anything: a build that contains the work

**This is the step most likely to waste your morning, so it is first.**
`network_addresses` is a command added during this work, and the wrapper that calls it is
written to throw. If you run against a binary that predates it, the Sharing pane offers **no
link and no QR at all** — which looks exactly like the bug you are trying to test. That is
plan finding S13.

```bash
cd /Users/mrgee/WebstormProjects/relay
git checkout stage-timers-mobile
npm install
npm run build
cd src-tauri && cargo build --release && cd ..
```

Then either `npm run tauri dev` (fastest) or `npm run tauri build` for the packaged app.
**Do the packaged build at least once** — §12 below is about the CSP, and `tauri dev` does
not exercise it (CLAUDE.md).

Record: which you ran, and the time you built.

---

## 0b. What has already been checked from this machine — and what that is worth

Run against the **packaged build** on 2026-09-19, over the LAN address
`192.168.1.144`, with no mocks anywhere. This is not a phone and does not
replace any check below; it means the parts a phone depends on were working
before you started.

| Checked | Result |
|---|---|
| `GET /stage.html` over the LAN address | HTTP 200 |
| `GET /output.html` over the LAN address | HTTP 200 |
| CSP header on the packaged build (§12b) | present, `default-src 'self'` … plus `X-Content-Type-Options: nosniff` |
| `GET /api/stage_zones` | `{"ok":true,"zones":{}}` — valid JSON |
| `GET /api/live`, `/api/search` | 200 |
| `GET /api/next` | 405, as it must be — mutators are POST only |
| WebSocket `hello` on channel 2 | replies `template, default_template, channel_roles, channel_looks, channel_shows, screen_state` |
| `beat` → `beat_ack` (§4) | answered, host clock within **1 ms** |
| Unparseable frames in the whole exchange | 0 |

**This found a real defect before you did.** `/api/stage_zones` was returning
`"zones":{}` — a fragment, not JSON. `Stage.svelte` would have thrown parsing
it, swallowed the error by design, and fallen back to the device's own zones —
so **§9b would have failed with no explanation anywhere**. Both test suites were
green: the frontend one mocks `fetch`, and the Rust route inventory only checked
that a route answered, not that the answer could be read. Fixed, and the
inventory now parses every route's body.

**What this does NOT tell you**, and why the rest of this document still stands:
nothing here rendered a pixel, decoded a QR with a camera, ran on iOS or
Android, survived a sleeping phone, or watched a timer for an hour. A socket
that answers correctly to a script is not a screen a preacher can read.

---

## 1. Record the room

Before touching Relay, write down:

- Computer: model, OS version.
- Phone/tablet: model, OS version, browser (Safari or Chrome — test both if you have both).
- Network: the wifi name, whether the computer is on wifi or ethernet, whether there is a
  guest network, whether the router has client isolation on.
- Relay build: `dev` or packaged, and the time from §0.

Most connection failures are the network, and none of this is diagnosable afterwards without
those five lines.

---

## 2. The address and the QR

**Outputs → Screens** → the stage screen → set **Role** to *Stage display*.
Then **Outputs → Sharing**.

| Check | Expected | If not |
|---|---|---|
| 2a | The address picker lists a real adapter (`Wi-Fi: 192.168.x.x`), not `localhost` | Press **Refresh addresses**. If it still says no address, record the whole pane |
| 2b | The link reads `http://<that address>:8032/stage.html?channel=<n>` | Record what it says instead |
| 2c | **Show QR** draws a code, about 240px, with a clear white border | Record whether the button did nothing, or an error appeared |
| 2d | **Scan it with the phone's camera.** It offers the same address | **This is the one thing no test can substitute.** If the camera will not read it, say so — it is a generation problem, not a network one |

Now open it. The page should load and say **live** in the header within a second or two.

Record: the address, and how long it took.

---

## 3. The console stops lying about the phone

With the phone connected, look at **Outputs → Screens** and at the Live desk.

| Check | Expected | Why |
|---|---|---|
| 3a | The stage screen reports as painting / attached — NOT "has never reported painting" | Plan S11. Before this work a correctly wired tablet always read as dead |
| 3b | Live's Stage Timer band does not warn that a Stage Timer needs the stage address | Same finding, seen from the run surface |

---

## 4. The clock

Start a **Screen Countdown** of 5 minutes from Quick tools.

| Check | Expected |
|---|---|
| 4a | The figure on the phone matches the console to within a second |
| 4b | Deliberately set the PHONE's clock a minute out (Settings → Date & Time, manual), reload the page, and compare again — it should still match | 

4b is the whole of plan S6. Before this the phone showed its own error. Put the phone's
clock back afterwards.

---

## 5. Losing the connection, and getting it back

| Check | Do this | Expected |
|---|---|---|
| 5a | Turn the phone's wifi off | Within ~6 seconds the header stops saying **live** and says **not answering · Ns**, with the seconds counting up. The verse stays on screen |
| 5b | Turn wifi back on | It returns to **live** on its own |
| 5c | Lock the phone for two minutes, then unlock | It reconnects **immediately** on unlock, not after a wait |
| 5d | Quit Relay, watch the phone, start Relay again | It reconnects and the current verse comes back |
| 5e | While disconnected, check the verse did not silently advance | Whatever was last on screen is still there |

5c is the reason the retry backs off — the long waits are for a page nobody is looking at.

---

## 6. The control panel

Open **Control** on the phone.

| Check | Do this | Expected |
|---|---|---|
| 6a | Search a reference, tap a result | It appears on the wall |
| 6b | **Next** / **Prev** | The wall moves; the phone says nothing when it worked |
| 6c | Reach the end of a reading, tap **Next** | "End of the reading." |
| 6d | Put the phone in airplane mode and tap **Next** | Within 6 seconds: **"Relay did not answer. Look at the screen — it may or may not have moved."** The buttons come back |
| 6e | | It must NOT say "No next verse." |

6d/6e are plan S9. The old behaviour left the panel disabled for the rest of the service and
described a network failure in the words of a correct passage boundary.

---

## 7. The Stage Timers

From the Live desk, **Stage Timer** band.

| Check | Do this | Expected |
|---|---|---|
| 7a | Start a 1-minute timer named "Sermon" | Appears on the phone's rail and on the band |
| 7b | Let it run past zero | Both read `+0:05 over` and keep counting. The rail marks it |
| 7c | Press **Hold** while it is over | Both freeze at the same figure, and both say held. **This is RG-175 and it could not be done before** |
| 7d | Press **Resume** | It carries on from where it was held, not from zero |
| 7e | Press **+5** | Five minutes from now |
| 7f | Press **Reset** | Back to 1 minute, keeping its name |
| 7g | Press **Clear screens** | The reading goes from every screen. **The Stage Timer keeps running** — this is deliberate, and the user guide used to say the opposite |

---

## 8. Counting down to a time of day

| Check | Do this | Expected |
|---|---|---|
| 8a | Quick tools → countdown → type a clock time ~3 minutes ahead in **or at** → Start | The wall counts down to that time |
| 8b | Type a time that has already passed → Start | It starts **already over**, counting up — it does NOT jump to 23-something |
| 8c | Type nonsense ("half ten") | **Start** is disabled and the field is marked |
| 8d | Same three on the Live Stage Timer band's **or at** field | Same behaviour |

---

## 9. Stage layouts

**Outputs → Stage layouts.**

| Check | Do this | Expected |
|---|---|---|
| 9a | Before assigning anything, set some zones on the PHONE's own Zones panel | They apply, and the desk cannot see them |
| 9b | Assign **Timer focus** to the stage screen | The phone changes within a second. Its Zones panel is now disabled and says the desk set it |
| 9c | Set the screen back to **Whatever the device is set to** | The phone returns to **the zones you set in 9a** — not to defaults |
| 9d | Make a new layout, name it, pick zones, **Save** | Nothing changes on the phone until Save |
| 9e | Try to delete a layout the screen is wearing | Refused, naming the screen |
| 9f | Try to delete **Preacher** | Refused — it is a shipped starter and would come back |
| 9g | Restart Relay | The assignment survives |

9c is the one that matters most: it is the promise that this feature did not quietly destroy
an arrangement a church was already using.

---

## 10. Stage Messages

| Check | Expected |
|---|---|
| 10a | Quick tools → type a message → **Send to stage** → it appears full-bleed on the phone |
| 10b | It appears on **no** congregation screen |
| 10c | **Take down** removes it |
| 10d | Press **Clear screens** while one is up → it comes down |
| 10e | Reload the phone → the old message does NOT come back |

---

## 11. A second stage screen

Add a second screen, set its Role to *Stage display*, give it a **different** layout, and
open it on another device (or a second browser).

| Check | Expected |
|---|---|
| 11a | Sharing lets you pick which stage screen the link is for |
| 11b | Each device shows its own layout |
| 11c | A Stage Message reaches both |

---

## 11b. A cue that names its screens (RG-161)

In the Planner, select a cue and use its **Screens** row.

| Check | Do this | Expected |
|---|---|---|
| 11b-i | Leave a cue alone and fire it | Reaches every screen, exactly as before. This is the half that must not regress |
| 11b-ii | Point a cue at the main screen only, fire it | It appears on the main screen |
| 11b-iii | Look at the preacher's screen at the same moment | **It still shows whatever it had.** It must NOT go blank — targeting narrows what a cue reaches and can never clear a screen |
| 11b-iv | Untick every screen, fire it | Reaches nothing. The row says so before you fire |
| 11b-v | Tick every screen again | The row goes back to saying "Every screen" |
| 11b-vi | With a targeted cue live, reload a screen it did NOT name | It comes back to what IT was showing, not to the targeted cue |
| 11b-vii | With a targeted cue live, press **Clear all screens** | Every screen clears, targeted or not |

11b-iii and 11b-vii are the two that matter. The first is the product decision —
an unnamed screen carries on — and the second is the guarantee that a panic
control is never narrowed by any of this.

---

## 12. The packaged build

Only meaningful on `npm run tauri build`.

| Check | Expected |
|---|---|
| 12a | The stage page loads from the packaged app, not just from `tauri dev` |
| 12b | No CSP errors in the phone's browser console (Safari: Develop menu; Chrome: `chrome://inspect`) |
| 12c | The microphone still works (CLAUDE.md rule 17 — signing kills it, and this is the build that would show it) |

---

## What to send back

For anything that failed, the useful report is:

1. Which numbered check.
2. What you saw, in your words — including the exact words on screen.
3. The five lines from §1.
4. Whether it happened once or every time.

For anything where the behaviour was *right* but the wording was confusing, say so too. Half
the findings in this work were a control that did the correct thing and described it wrongly.
