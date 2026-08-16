# Relay — Documentation Index (Engineering Blueprint)

This is the map. Relay's documentation is already complete and, unusually, **re-verified
against the code on every revision** — that honesty is the point, not the paperwork. This
index does one job: lay the existing docs out as a professional specification hierarchy so a
new engineer, operator, or contributor can find any part of the spec without hunting, and see
at a glance which document owns which question.

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

Relay's docs map cleanly onto the six volumes a mature product spec is expected to carry. Most
already existed under a working name; the three marked **NEW** were written to close the only
real gaps.

| Volume | Owns the question | Document(s) | Status |
|---|---|---|---|
| **1 · Product** | *What is Relay, for whom, and what must it do?* | [SPEC.md](SPEC.md) · [PRODUCT_AUDIT.md](PRODUCT_AUDIT.md) | Complete |
| **2 · Domain model** | *What is Relay made of — the entities, their lifecycle, the invariants, the events?* | [DOMAIN_MODEL.md](DOMAIN_MODEL.md) | **NEW** |
| **3 · UX & design system** | *How does it look and behave — tokens, type, colour meaning, components?* | [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) · [design/](design/) mockups | **NEW** (+ existing mockups) |
| **4 · System architecture** | *How is it built — process model, pipeline, rendering, data layer, invariants?* | [ARCHITECTURE.md](ARCHITECTURE.md) | Complete |
| **5 · AI specification** | *What does the AI decide, what will it never do, where is it honestly weak?* | [AI_DISCLOSURE.md](AI_DISCLOSURE.md) · [LANGUAGES.md](LANGUAGES.md) | Complete |
| **6 · Engineering handbook** | *How do we work — conventions, the rules learned the hard way, contribution bar?* | [../CLAUDE.md](../CLAUDE.md) · [../CONTRIBUTING.md](../CONTRIBUTING.md) | Complete |
| **7 · Operations manual** | *How is it released, signed, updated, and operated on a Sunday?* | [RELEASING.md](RELEASING.md) · [USER_GUIDE.md](USER_GUIDE.md) | Complete |
| **Decisions (ADR log)** | *Why is anything the way it is?* | [DECISIONS.md](DECISIONS.md) | Complete |
| **Roadmap & tech debt** | *What is deferred, parked, or owed — and on whose authority?* | [ROADMAP.md](ROADMAP.md) | **NEW** |
| **Data schema** | *The canonical on-device SQLite shape* | [data/schema.sql](data/schema.sql) | Refreshed |

**On ADRs:** [DECISIONS.md](DECISIONS.md) *is* the architecture-decision record — a single
narrative log with reasoning and explicit non-goals, 25 decisions deep. It is deliberately not
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
2. [design/](design/) — open the `.html` mockups directly in a browser, no build step.

**An AI coding agent**
- [../CLAUDE.md](../CLAUDE.md) first, every session. Then this index, then the volume you need.
- Auditing rather than building? [Working-Agent.md](Working-Agent.md) — and read
  [Working-Agent-COVERAGE.md](Working-Agent-COVERAGE.md) before filing anything, so you don't
  re-discover a deliberate decision or "find" a bug that was fixed.

---

## How Relay gets audited

Three documents own the QA apparatus. They are **not** part of the specification hierarchy
above — SPEC, DECISIONS and PRODUCT_AUDIT own the product; these own how it is checked.

| Document | Owns |
|---|---|
| [Working-Agent.md](Working-Agent.md) | The design: five evidence layers, the six-agent roster, the hook, and why "click every button" cannot be executed literally on a desktop binary that this machine cannot see |
| [Working-Agent-PROMPT.md](Working-Agent-PROMPT.md) | The shared preamble every `relay-qa-*` agent inherits, plus the six mandates |
| [Working-Agent-COVERAGE.md](Working-Agent-COVERAGE.md) | The evidence baseline — what the existing tests already prove, and what no instrument here can reach |

Run it with **`/qa-audit`** (changed surface by default; `--full` before a release, `--live` to
drive the running app over `:8032`). The cheap half runs on every edit:
`.claude/hooks/relay-fast-gate.mjs`, path-filtered and report-only. Reports land in
`docs/audits/`.

---

## Product health, honestly

[PRODUCT_AUDIT.md](PRODUCT_AUDIT.md) is the current health check — a scorecard (overall
maturity **8.5/10**) that re-verifies every claim against a commit hash and retires findings as
they're fixed. It is the most candid document in the repo; read it before assuming anything is
missing. What remains open is captured, with reasoning, in [ROADMAP.md](ROADMAP.md) — and most
of it is blocked on the world (a certificate, a native speaker, thirty minutes of a real
sermon on tape), not on a commit.
