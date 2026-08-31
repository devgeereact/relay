# Relay — Documentation Index (Engineering Blueprint)

This is the map. Every claim in these documents is **re-verified against the code**, and the
date of the last sweep is stated rather than implied. Last **full** sweep of every claim:
**2026-08-31** — the pass that rewrote [RELAY_GAP.md](RELAY_GAP.md) §0–§2 and §6–§18 against the
working tree, after thirty-one status rows had gone stale while the fix logs directly above them
said so. What it found and corrected is [RELAY_GAP.md](RELAY_GAP.md) §0, §18 and §26.

> **Counts live beside the command that produces them, and nowhere else.** Three sweeps have
> now corrected the same three numbers (`main.rs` lines, registered commands, `capture.js`
> lines) and all three were wrong again within the week — [RELAY_GAP.md](RELAY_GAP.md) §18 is
> the record of that. A number in prose is wrong the moment somebody commits; re-correcting it
> is the same bill paid twice. If you need one, run the command.

That honesty is the point, not the paperwork.

This index does one job: lay the docs out as a specification hierarchy so a new engineer,
operator, or contributor can find any part of the spec without hunting, and see at a glance
which document owns which question.

> **Read this first if you are new.** Then follow the "Start here" path for whoever you are.

---

## The one sentence everything serves

> **Deliver the right visual content to the right screen at exactly the right moment with the
> least possible effort from the operator.**

Every design review, every feature, every line is measured against that. Its corollary is the
scoping rule this whole product runs on: **if a feature does not make Sunday morning smoother,
it waits.** (See [ROADMAP.md](ROADMAP.md) for where the "waits" pile lives, and why.)

## Non-negotiables (the load-bearing walls)

These do not move without a human explicitly reopening them. Full reasoning in
[DECISIONS.md](DECISIONS.md) and the root [CLAUDE.md](../CLAUDE.md).

- **Offline-first.** Every core feature (STT, detection, rendering) works with zero internet.
  Cloud is an optional fallback, never a requirement.
- **Operator authority is first-class.** Automation reduces workload; it never removes the
  human. A paraphrase never reaches a congregation without a person agreeing.
- **Output channels are render targets of one template engine.** No `if channel_type == …`
  in rendering logic — that is a template-configuration problem, not a code problem.
- **Local-first data.** Transcripts, verse text, templates, and history live in local SQLite.
  Nothing leaves the device without an explicit, visible reason.
- **No native SDI hardware, ever** (unless reopened). NDI + HDMI only; bridge to SDI with gear
  the church already owns.

---

## The specification hierarchy

Relay's docs map cleanly onto the volumes a mature product spec is expected to carry. Most
already existed under a working name; the two marked **NEW** were written to close the last
real gaps.

| Volume | Owns the question | Document(s) | Status |
|---|---|---|---|
| **1 · Product** | *What is Relay, for whom, and what must it do?* | [SPEC.md](SPEC.md) · [PRODUCT_AUDIT.md](PRODUCT_AUDIT.md) | Complete |
| **2 · Domain model** | *What is Relay made of — the entities, their lifecycle, the invariants, the events?* | [DOMAIN_MODEL.md](DOMAIN_MODEL.md) | **NEW** |
| **3 · UX & design system** | *How does it look and behave — tokens, type, colour meaning, components?* | [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) · [design/](design/) references | Complete |
| **4 · System architecture** | *How is it built — process model, pipeline, rendering, data layer, invariants?* | [ARCHITECTURE.md](ARCHITECTURE.md) | Complete |
| **5 · AI specification** | *What does the AI decide, what will it never do, where is it honestly weak?* | [AI_DISCLOSURE.md](AI_DISCLOSURE.md) · [LANGUAGES.md](LANGUAGES.md) | Complete |
| **6 · Engineering handbook** | *How do we work — conventions, the rules learned the hard way, contribution bar?* | [../CLAUDE.md](../CLAUDE.md) · [../CONTRIBUTING.md](../CONTRIBUTING.md) | Complete |
| **7 · Operations manual** | *How is it released, signed, updated, and operated on a Sunday?* | [RELEASING.md](RELEASING.md) · [USER_GUIDE.md](USER_GUIDE.md) | Complete |
| **Decisions (ADR log)** | *Why is anything the way it is?* | [DECISIONS.md](DECISIONS.md) | Complete |
| **Roadmap & tech debt** | *What is deferred, parked, or owed — and on whose authority?* | [ROADMAP.md](ROADMAP.md) | **NEW** |
| **Data schema** | *The canonical on-device SQLite shape* | [data/schema.sql](data/schema.sql) | Refreshed |
| **Schema baseline** | *The oldest schema Relay can upgrade from — checked in so a test can prove every column added since has a migration. **Never edit it***| [data/schema-baseline.sql](data/schema-baseline.sql) | **NEW** |
| **Gap register** | *What an outside product brief asked for vs what exists — and the two proposals that would reverse a recorded decision* | [RELAY_GAP.md](RELAY_GAP.md) | **NEW** |

**On ADRs:** [DECISIONS.md](DECISIONS.md) *is* the architecture-decision record — a single
narrative log with reasoning and explicit non-goals, 46 numbered decisions (§18–§63) deep,
plus 28 earlier ones carried as table rows. It is deliberately not
split into per-file `adr/NNNN-*.md` documents: the log is cross-referenced from code comments
and the handbook, and one file keeps the *why* readable end to end. If the code ever
contradicts it, the code is wrong — flag it, don't silently "fix" the decision.

---

## Start here — by who you are

**A new engineer**
1. [../CLAUDE.md](../CLAUDE.md) — conventions, non-negotiables, and the rules learned the hard
   way (regressions that reached real congregations). Read before touching code.
2. [ARCHITECTURE.md](ARCHITECTURE.md) — how the pieces fit, end to end.
3. [DOMAIN_MODEL.md](DOMAIN_MODEL.md) — the entities and the invariants that govern them.
4. [DECISIONS.md](DECISIONS.md) — why. If a decision isn't here, it hasn't been made — ask.

**An operator / church volunteer**
1. [USER_GUIDE.md](USER_GUIDE.md) — 10-minute setup and the Sunday-morning path.
2. [AI_DISCLOSURE.md](AI_DISCLOSURE.md) — what the AI does, and what it will never do.
3. [../PRIVACY.md](../PRIVACY.md) — nothing you say, sing, or show leaves your computer.

**A contributor (code or language)**
1. [../CONTRIBUTING.md](../CONTRIBUTING.md) — the two contributions that need **no code**
   (book aliases, locale files) come first.
2. [LANGUAGES.md](LANGUAGES.md) — Yorùbá / Kiswahili / Hausa; fix a book name in a one-line PR.
3. [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) — before you touch any UI.

**A designer / UI contributor**
1. [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) — tokens, type, the load-bearing colour meanings.
2. [design/](design/) — thirteen rendered screen references (PNG, ~19 MB). A record of
   intent, not a spec: where a reference and `src/app.css` disagree, the stylesheet is
   what shipped. Six are cited by name from source comments and seven are not; **the whole
   set is kept deliberately**, because a half-set described as "the rendered screen
   references" is more misleading than a complete one that costs disk. **The directory is
   `design/`** — sixteen comments once cited a `relaydesign/` that has never existed here
   (RG-70), and `crossrefs.test.js` now fails on a citation to any `docs/…` file that is
   not there.

**An AI coding agent**
- [../CLAUDE.md](../CLAUDE.md) first, every session. Then this index, then the volume you need.
- Auditing rather than building? [QA_HARNESS.md](QA_HARNESS.md) — and read its **Part 4**
  before filing anything, so you don't re-discover a deliberate decision or "find" a bug that
  was fixed.

---

## How Relay gets audited

One document owns the QA apparatus: **[QA_HARNESS.md](QA_HARNESS.md)**. It is **not** part of
the specification hierarchy above — SPEC, DECISIONS and PRODUCT_AUDIT own the product; this owns
how it is checked.

| Part | Owns |
|---|---|
| **0 · Inventory** | The current counts, each with the command that reproduces it |
| **1 · The design** | Five evidence layers, the hook, and why "click every button" cannot be executed literally on a desktop binary this machine cannot see |
| **2 · The shared preamble** | The text every `relay-qa-*` agent inherits verbatim, including what is already decided and is therefore not a finding |
| **3 · The roster** | The six agents and what each may not claim. Full mandates live in `.claude/agents/` |
| **4 · The evidence baseline** | What the existing tests already prove, and what no instrument here can reach |

Run it with **`/qa-audit`** (changed surface by default; `--full` before a release, `--live` to
drive the running app over `:8032`). The cheap half runs on every edit:
`.claude/hooks/relay-fast-gate.mjs`, path-filtered and report-only. Reports land in
`docs/audits/`.

---

## Product health, honestly

Two documents, at two altitudes, and they disagree on purpose:

- **[PRODUCT_AUDIT.md](PRODUCT_AUDIT.md)** — a human's scorecard, written from the outside.
  It re-verifies every claim against a commit hash and retires findings as they are fixed.
- **[audits/](audits/)** — dated machine audits, each frozen. They never rewrite their own
  findings; closures are recorded in the fix log at the top instead, because an audit that
  edits its own history stops being evidence. Four so far:
  [QA-2026-08-14.md](audits/QA-2026-08-14.md) (six-agent full scope; §16 is the human test
  script), [PERF-2026-08-24.md](audits/PERF-2026-08-24.md) (real-time latency, and §6 is
  what its numbers do **not** establish),
  **[FIELD-2026-08-30.md](audits/FIELD-2026-08-30.md)** — *the first real service*: a live
  sermon, 49.5 minutes, packaged build. Stage F11 answered (no drift), five of six
  auto-fires correct, **one wrong verse on a real wall**, and six findings of which one was
  later withdrawn as wrong — and
  **[PERF-MODELS-2026-08-30.md](audits/PERF-MODELS-2026-08-30.md)**, what each speech model
  costs in transcript updates per second, which is the number a church actually chooses
  between.
- **[RELAY_GAP.md](RELAY_GAP.md)** §19b — **decisions this report needs and cannot make.** One
  is open: whether Relay ships a second Bible translation, an import path for one, or neither.
- **[RELAY_GAP.md](RELAY_GAP.md)** — the third altitude, and unlike the audits it is meant to be
  updated: an outside product brief scored item by item against the code, with a gap register
  and the corrections this index's own counts needed.

**Relay has never shipped. As of 2026-08-31: NO-GO for general release, GO for a supervised
pilot** — two churches, named operators, every service watched. Every P0 and P1 from the last
full audit is closed and 56 register entries with them, but four things block a general
release and none is a commit: a **wrong verse reached a real congregation** on 2026-08-30,
**word error rate has never been measured in any language**, **neither platform has a code-signing certificate** (RG-73 — all four releases went out unsigned),
and nobody but the author has ever run a service. The decision, its conditions, and the five
things that convert it into a general release are [RELAY_GAP.md](RELAY_GAP.md) §24.

The packaged build **has** now been reached by an instrument — a live sermon, 49.5 minutes
([audits/FIELD-2026-08-30.md](audits/FIELD-2026-08-30.md)). Pixels out, hardware, and a
congregation still have not: [audits/QA-2026-08-14.md](audits/QA-2026-08-14.md) §16 is the
human test script and it has not been run.

What remains open is captured, with reasoning, in [ROADMAP.md](ROADMAP.md) — and most of it is
blocked on the world (a certificate, a native speaker, thirty minutes of a real sermon on tape),
not on a commit.
