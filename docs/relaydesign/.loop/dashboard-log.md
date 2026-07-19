# Dashboard — design loop log

**No reference PNG exists for this section.** The contact sheet has no dashboard
panel, so this was built against `relay-designsystem.png` v1.0 (tokens, cards,
type scale) the same way §1 was.

**Compare method: PIXEL** — `dashboard-3.png`, captured in headless Chromium at
1440×900, DPR 1, `reducedMotion: reduce`.

Gate: `npm run build` clean, **195 frontend** tests (was 192), **264 Rust**,
`cargo fmt --check` + `clippy -D warnings` clean.

---

## Why this screen is allowed to exist

A dashboard is the easiest screen in any product to fill with duplicates of other
screens, and the §3 list reads like a generic admin template. This one earns its
place by answering a question no other tab does:

> It is 10:20 on a Sunday. The service is at 11:00. **Is this machine going to
> work?**

Boot Diagnostics answers exactly that — and then vanishes. It runs once, at
launch, and the operator who arrives forty minutes later never sees it.

**So System Health here is not a second health check. It is the same check.**
Same list, same probes, same severity rules, re-run on demand — `runChecks()` was
extracted from `runStage()` for precisely this. Two independently-written health
panels would eventually disagree, and then the app would be arguing with itself
about whether it works.

Everything else is a SHORTCUT to a real surface, never a copy of one. Nothing on
this screen is editable.

## The rule this screen is built around

**Nothing on the Dashboard can put anything on a screen.** The quick actions open
an output window (blank), arm the microphone, or toggle rehearsal. Firing content
stays in Live. A "go live" button on a summary screen is how the wrong thing
reaches a congregation.

## Screens covered

| Listed | Where it went |
|---|---|
| Home Dashboard | the screen itself |
| System Health | the boot checks, re-run — same source |
| Quick Actions | four actions, none of which can fire content |
| Recent Services | real `list_services` rows: date, length, verses, overrides |
| ~~Recent Projects~~ | **DROPPED.** Relay has no concept of a "project" — it has service plans and service history, and both are already here. Inventing a third noun for the same thing would be admin-template vocabulary, not Relay's. This also reconciles the count table, which says **4** for this module while the list has 5. |

## Landing behaviour, and a distinction the tests forced

Dashboard is the **first tab** but is not where a returning operator lands: the
active tab is persisted, so someone who ran a service yesterday comes back to
Live. Only a genuinely fresh install starts here.

Changing `EMPTY.activeTab` to `dashboard` immediately failed
`session.test.js` — *"a corrupt session falls back to a safe default"*, which
expected `live`. The test was right, and it exposed a distinction the loader did
not make:

- **Nothing saved** = a fresh install. Nobody has run a service on this machine.
  Land on the Dashboard.
- **A corrupt payload** = there WAS a session, possibly mid-service thirty
  seconds ago, and it is simply unreadable. Land on the **run surface**. If this
  happened during a service the operator needs the console, not a readiness
  report about a service that is already happening.

`load()` now separates the two, with tests for both.

## A bug found by looking at the render

**Failed checks printed raw JS errors.** `runChecks` did
`String(e?.message ?? e)`, so the health panel showed
`Cannot read properties of undefined (reading 'invoke')` — **six rows of it**.
CLAUDE.md is explicit that `errors.js` is the one humaniser. Fixed in `boot.js`,
which fixes Boot Diagnostics at the same time, and pinned by a test.

**Then the fix looked worse than the bug:** six rows of the same humanised
sentence is a wall of red that must be read six times to discover it says the
same thing six times. So when every failure shares one note, the Dashboard now
states the cause **once** in the hero and the rows read `not answered`. Notes are
also line-clamped to three lines so no single long error can explode a row.

## Still off / not verified

- **The healthy state has never been seen.** Captured in a browser, where there
  is no backend, so every probe fails — which is exactly the state the
  screenshots show. The green "Ready for a service" hero, populated recent
  services and a real plan list are **unrendered**.
- `makeProbes()` is constructed inside the component, so the Dashboard's health
  panel cannot be driven from a test with fake answers the way `probes.test.js`
  drives the probes themselves. Worth injecting if this screen grows.
- Quick actions were not clicked against a real backend — `openChannelOutput`,
  `startCapture` and `setRehearsal` are wired but unexercised from here.
