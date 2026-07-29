# Live Production (§4) — design loop log

Reference for the surface itself: `relay-console-screen.png` (already matched —
see `.loop/live-log.md`). The nine remaining entries in §4 have **no reference
panel**; they were assessed against the design system and against what the run
surface already does.

**Compare method: PIXEL** — captured at **1280×800**, which is the case these
modes exist for: a 13" church-booth laptop, not the wide desk monitor the console
reference assumes. `live-{normal,compact,fullscreen}-2.png`.

Gate: `npm run build` clean, **195 frontend**, **264 Rust**, fmt + clippy clean.

---

## Built

### Compact Mode
A **density change, not a different screen**: same panels, same controls, tighter
spacing and type. Persisted in the session, because a booth's screen does not
change between Sundays.

**Nothing is removed in compact.** A run surface that hides a control at small
sizes hides it exactly when the operator is most cramped and most rushed.

The real win turned out not to be padding. `.con-top` is a fixed
`clamp(268px,33vh,364px)`, so on an 800px-tall laptop the preview/program row ate
a third of the window and pushed the transport and the panic controls below the
fold. Compact shrinks that row and gives the space back to the row the operator
actually touches. (Panels scroll internally via `.pane-body`, so nothing was ever
*unreachable* — it was just below the fold, which on a Sunday is close enough to
the same thing.)

### Full Screen Live Control
Hides the sidebar, top bar and footer so the run surface owns the window.

**Escape does not exit it, and that is deliberate.** Everywhere else in computing
Escape leaves full screen. In Relay, Escape CLEARS THE CONGREGATION'S SCREENS and
that meaning is not negotiable. So the way out is a **visible button, always on
screen** — never the key a muscle-memory reflex reaches for. Full screen also
applies *only* while Live is the active tab, so it can never strand an operator
on a navigation-less Settings page mid-service.

---

## Not built — already exists

| Listed | Where it already is |
|---|---|
| **Active Outputs Overview** | The **Output Status** panel in Live, already built to the console reference. Read-only on purpose: during a service the only question is "is it up?" |
| **Emergency Control Panel** | Panic already has **three** entry points: `Clear screens` / `Blackout` in Quick Controls, the global `Esc` / `B` keys (`shortcuts.js`), and `Emergency Stop` in the top bar. A fourth surface is not extra safety — it is a fourth code path to keep correct, on the one control that may never report a success it did not achieve (DECISIONS §20). One panic path, three ways to reach it, is the safe shape. |
| **Dual Monitor Mode**, **Multi Monitor Control** | Channels owns output targets and display assignment. Relay's model is that every output is a render target of one template engine — a second place to assign displays is a second source of truth for which screen the congregation sees. |

## Not built — Relay has no such concept

**Operator View** and **Producer View** are role-based variants, and **Relay has
no roles**. User Management is §21 and unbuilt; there is one operator at one
desk. Two named views of the same surface, with nothing to distinguish them,
is bloat that would have to be maintained forever.

## Not built — and this one is a hazard, not just a gap

**Confidence View** and **AI Confidence Timeline** are deferred to §5 (AI
Detection), where the Inspector belongs — but they need flagging now, because
their obvious implementation is forbidden by the product's own rules:

> **A paraphrase shows NO percentage, at any score.** A TF-IDF cosine is not a
> probability, and a number that lies is worse than no number because it looks
> like information and therefore gets acted on.
> — CLAUDE.md §18, DECISIONS §21, pinned by `detect.test.js` and
> `router.rs::semantic_can_never_auto_fire`

A "confidence timeline" is precisely the shape that manufactures that number: a
chart needs a y-value for every point, so a screen like this pressures you into
inventing a percentage for the exact case the codebase refuses to give one. When
§5 is built, it must plot **what kind of claim** was made (direct / semantic /
ambiguous) and the matched evidence — not a fabricated confidence curve.

---

## Still off / not verified

- Captured in a browser with no backend: preview and program are empty, no
  channels, no plan, no transcript. **The populated console at compact density
  has not been seen** — which is the state that would actually prove the density
  is right.
- Compact was tuned at 1280×800 only. Nothing verified below 1024px, where the
  responsive rules already re-stack the grid.
- No keyboard shortcut was added for either mode. `shortcuts.js` owns the global
  keydown path and its keys are load-bearing (`Esc`, `B`, `Space`); adding a
  binding there deserves its own pass rather than being smuggled in with a CSS
  density change.
