# AI Detection (§5) — design loop log

Reference: `relay-production-interface.png` **panel 8 — AI Detection Detail /
Inspector**. Built as `src/lib/DetectionInspector.svelte`, opened from Live's
claim panel ("Why this match?").

**Compare method: PIXEL** — `inspector-{semantic,direct,ambiguous}-1.png`,
captured at 1100×820 by mounting the component against fixtures. Plus **11 new
component tests** (`inspector.test.js`), which is the stronger check here: the
rule this screen must obey is not a layout, it is a number that must not appear.

Gate: build clean, **206 frontend** tests (was 195), **264 Rust**, fmt + clippy clean.

---

## The reference is wrong in two places, and following it faithfully would have shipped the bug

This is the one section where the mockup and the codebase's own law disagree.

### 1. It draws a percentage on every claim

Panel 8's chip reads **`DIRECT 92%`** — fine. But the same chip shape applied to a
paraphrase would print a percentage beside a **TF-IDF cosine**, which is a
distance in an arbitrary vector space, not a probability. Printed as "61%" an
operator reads "61% likely to be right", in front of a congregation.

> A paraphrase shows NO number, at any score.
> — CLAUDE.md §18 · DECISIONS §21 · `router.rs::semantic_can_never_auto_fire`

The Inspector uses `detect.js::showsConfidence` — the same tested helper Live
uses — rather than re-deriving the rule. **Verified the test catches it:**
replacing the guard with an unconditional `{pct}%` (i.e. building the reference
literally) fails two tests; restoring it passes.

Because an absent number is itself confusing, the screen says *why* it is absent
instead of silently omitting it.

### 2. Its "Why this match?" bullets describe an algorithm that does not exist

The mockup lists: *"Order and structure align closely"*, *"Minimal words added or
skipped"*, *"Confidence computed from semantic similarity"*.

**Relay computes none of those things.** It has no word-order comparison, no
insertion/deletion metric, and for a direct match nothing semantic is involved at
all. Rendering those bullets would be fabricated reasoning — a screen explaining
a decision by describing machinery that is not there, which is worse than no
explanation because it is checkable and wrong.

The real evidence was already crossing the IPC bridge:

- **Heard reference** → the span of transcript the parser actually read.
- **Paraphrase** → the shared rare words that drove the cosine, from
  `SemanticIndex::top_k_explained`, joined into `matched_text` by `main.rs`.

Those are shown, as term chips, with the honest footnote: *"Relay does not compare
grammar, word order, or meaning — only which uncommon words overlap."* A test
asserts the mockup's three phrases never appear.

### 3. No sensitivity sliders (a third, quieter departure)

Panel 8 puts Sensitivity, Auto-fire and Minimum Confidence in this panel. Settings
already owns them, and thresholds have exactly ONE baseline (`router.rs`). A
second set of sliders is a second source of truth for the gate that decides what a
congregation sees. Current values are shown **read-only**, with a link to where
they live.

## What this screen adds that nothing else did

**The learning loop was invisible.** `confirm_detection` and `dismiss_detection`
both call `router.record_feedback` — every accept and every dismiss moves the
gate, and no screen had ever said so. That is invisible training: the operator
changes the product's behaviour without being told they are doing it. The footer
now states it plainly, including that it does not learn during a rehearsal.

## Screens covered

| Listed | Where it went |
|---|---|
| **Inspector** | **✅ built** |
| **Verse Match Comparison** | **✅ folded in** — an ambiguous reference's other candidates, shown side by side. That is the only moment the comparison is useful; as a standalone screen there is nothing to compare |
| **Confidence Tuning** | **already in Settings.** Shown read-only here with a link — not duplicated |
| **Detection History** | **already in Library → History**, which lists each service's fired detections |
| **Recognition Logs** | duplicate of §19 Logs, and the live view of it is Live's transcript panel. Deferred to §19 so it is built once |
| **False Positive Review** · **AI Learning Feedback** | **partly real already, and now surfaced.** Dismissing IS the false-positive signal and it already retunes the gate. What does **not** exist is a review queue of past mistakes — that needs a backend concept (a per-detection verdict, persisted) which does not exist today. Not faked; see below |

## Not built, and why

A **False Positive Review queue** would need detections to carry a persisted
operator verdict that can be revisited after the service. Today the only record is
`detections.status` (`auto` / `manual` / …) — enough for the router to
self-calibrate, not enough to reconstruct "the AI was wrong about this one".
Building the screen without that backend would mean inventing a review list from
data that cannot support it.

**AI Confidence Timeline** (listed in §4, deferred here) is still not built, for
the reason recorded in the §4 log: a chart needs a y-value per point, which
pressures you into inventing exactly the number this section refuses to show.

## Still off / not verified

- Captured against **fixtures**, not a live detection. The component was mounted
  directly with hand-written props; nothing has flowed from real audio through
  the real router into this screen.
- The transcript card shows the **most recent final line** and is labelled that
  way, because a detection carries no transcript id. It is context, not a proven
  link — worth wiring properly if detections ever carry one.
- The "Why this match?" copy for a paraphrase assumes `matched_text` is the
  `" · "`-joined term list `main.rs` produces. If that join format changes, the
  chips silently become one long chip.
