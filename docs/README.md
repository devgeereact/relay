# Relay — Documentation Index (Engineering Blueprint)

This is the map. Every claim in these documents is **re-verified against the code**, and the
date of the last sweep is stated rather than implied. Last **full** sweep of every claim:
**2026-08-31** — the pass that rewrote [RELAY_GAP.md](qa/RELAY_GAP.md) §0–§2 and §6–§18 against the
working tree, after thirty-one status rows had gone stale while the fix logs directly above them
said so. What it found and corrected is [RELAY_GAP.md](qa/RELAY_GAP.md) §0, §18 and §26.

> **Counts live beside the command that produces them, and nowhere else.** Three sweeps have
> now corrected the same three numbers (`main.rs` lines, registered commands, `capture.js`
> lines) and all three were wrong again within the week — [RELAY_GAP.md](qa/RELAY_GAP.md) §18 is
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
it waits.** (See [KNOWN_ISSUES.md](KNOWN_ISSUES.md) for where the "waits" pile lives, and why.)

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
- **No native SDI hardware, ever** (unless reopened). HDMI only; NDI if it is ever unparked.
  SDI is bridged with a converter, which a church may already own or can buy for about the
  price of a microphone cable.

---

## The specification hierarchy

**The directory says which is which.** `docs/` is the specification. `docs/qa/` is how Relay is
checked and what checking found. `docs/qa/audits/` is frozen evidence — dated, and never edited
after the fact. **`docs/superpowers/plans/` holds the ONE live plan** (`2026-09-19-stage-timers-mobile.md`, with its own inline status); **`docs/archive/` holds every wave design and work order whose work has landed**, each headed with where its rulings and findings now live. Neither is specification nor evidence — a ruling that lives only in a design has not been made yet and belongs in `DECISIONS.md` (§92, §93, §94 and §95 all landed on 2026-09-17, and three of the four had to be renumbered: four branches wrote a §92 at the same insertion point on the same day). `CHANGELOG.md` stays at the
repository root, where the convention and GitHub both expect it.

**One row below sits under the `docs/qa/` heading and is not in `docs/qa/`** — `REBRAND.md` is
top-level. The link is right; the filing is not, and the paragraph above says the directory is the
classifier. (`RELAY_V1_AUDIT.md` was the other until 2026-09-21; it and `LAUNCH_CHECKLIST.md` are
folded into `RELAY_GAP.md` and `QA_HARNESS.md` and archived.)

| Volume | Owns the question | Document(s) | Status |
|---|---|---|---|
| **1 · Product** | *What is Relay, for whom, and what must it do?* | [SPEC.md](SPEC.md) | Complete |
| **2 · Domain model** | *What is Relay made of — the entities, their lifecycle, the invariants, the events?* | [DATA_MODEL.md](DATA_MODEL.md) | **NEW** |
| **3 · UX & design system** | *How does it look and behave — tokens, type, colour meaning, components?* | [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) · [design/](design/) references | Complete |
| **4 · System architecture** | *How is it built — process model, pipeline, rendering, data layer, invariants?* | [ARCHITECTURE.md](ARCHITECTURE.md) | Complete |
| **5 · AI specification** | *What does the AI decide, what will it never do, where is it honestly weak?* | [AI_DISCLOSURE.md](AI_DISCLOSURE.md) · [LANGUAGES.md](LANGUAGES.md) | Complete |
| **6 · Engineering handbook** | *How do we work — conventions, the rules learned the hard way, contribution bar?* | [../CLAUDE.md](../CLAUDE.md) · [CONTRIBUTING.md](CONTRIBUTING.md) | Complete |
| **7 · Operations manual** | *How is it released, signed, updated, and operated on a Sunday? And how does Relay reach the actual screens: a projector, an ATEM, OBS, or a fourth screen with no port left?* | [RELEASING.md](RELEASING.md) · [USER_GUIDE.md](USER_GUIDE.md) · [OUTPUT_ROUTING.md](OUTPUT_ROUTING.md) | Complete |
| **Decisions (ADR log)** | *Why is anything the way it is?* | [DECISIONS.md](DECISIONS.md) | Complete |
| **Known issues & tech debt** | *What is deferred, parked, or owed — and on whose authority?* | [KNOWN_ISSUES.md](KNOWN_ISSUES.md) | **NEW** |
| **Agent routing** | *How an agent working in this repository is meant to operate* | [GEE-OS.md](GEE-OS.md) | Not part of the product hierarchy |
| **Data schema** | *The canonical on-device SQLite shape.* ⚠️ **`include_str!`d by `db/mod.rs`, so it IS the shipped baseline schema — not documentation.** Delete it and `cargo build` fails while the whole frontend suite stays green | [data/schema.sql](data/schema.sql) | Refreshed |
| **Schema baseline** | *The oldest schema Relay can upgrade from — checked in so a test can prove every column added since has a migration. **Never edit it***. ⚠️ **Also `include_str!`d**, for the same reason and with the same consequence: "`docs/` is the specification" above is true of every file here except these two, and a reader who prunes them on that sentence gets a red Rust build and a green `npx vitest run`. Pinned by `hardrules.test.js` | [data/schema-baseline.sql](data/schema-baseline.sql) | **NEW** |
| **Gap register** | *What an outside product brief asked for vs what exists — and the two proposals that would reverse a recorded decision* | [RELAY_GAP.md](qa/RELAY_GAP.md) | **NEW** |
| **Security & privacy** | *What Relay is asked to defend against, and what it promises a church* | [SECURITY.md](SECURITY.md) (T1–T10) · [PRIVACY.md](PRIVACY.md) | Complete |
| **Community** | *How contributions arrive and how people are treated* | [CONTRIBUTING.md](CONTRIBUTING.md) · [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) | Complete |
| **Build-phase map** | *The phase numbers eight Rust module docs cite as their rationale anchor. **Must not be deleted*** | [PROMPT.md](PROMPT.md) | Historical |
| **Release notes** | *What changed, for the person in the booth* | [../CHANGELOG.md](../CHANGELOG.md) | Complete |

### `docs/qa/` — how Relay is checked

| Document | Owns |
|---|---|
| [qa/QA_HARNESS.md](qa/QA_HARNESS.md) | The QA apparatus, **§0 the register of counts for the whole repository** — every row beside the command that produces it — and Parts 5 and 6 the three scorecards and the brief disposition, frozen at 2026-09-05 |
| [qa/RELAY_GAP.md](qa/RELAY_GAP.md) | The `RG-` gap register (§23), the release decision and its reasoning (§24), and the one launch gate list (§27). Meant to be updated, unlike the audits |
| [REBRAND.md](REBRAND.md) | The active build specification for Relay Studio — the brand, the workspace grammar, the template engine, and the twelve phases with their status. `AGENTS.md` step 2 sends readers here to find it |
| [archive/](archive/) | Finished wave designs, and the two retired audit documents: the V1 production audit (`2026-09-05-relay-v1-audit.md`) and the launch checklist (`2026-09-15-launch-checklist.md`), folded into the register and the harness on 2026-09-21 |
| [qa/audits/](qa/audits/) | **Frozen.** **Twelve** dated audits (`ls qa/audits | wc -l`): four field services, two performance runs, one six-agent QA sweep, **four browser-driven passes** (waves 2, 3 and 5 on 2026-09-16, wave 4 on 2026-09-17) and the retired product audit. Closures go in a fix log at the top, never into the findings |

**On ADRs:** [DECISIONS.md](DECISIONS.md) *is* the architecture-decision record — a single
narrative log with reasoning and explicit non-goals, numbered decisions from §18 upward (run `grep -cE '^## [0-9]+\. ' DECISIONS.md` for the count; this line said 78 and §95 while the command answered 88 and §105, which is why it no longer carries a number) —
plus 28 earlier ones carried as table rows (`sed -n '1,59p' DECISIONS.md | grep -cE '^\|'`,
less the three header-and-separator pairs). **This line said 46 and §18–§63**, which had been
wrong for twenty-nine decisions, in a paragraph four screens below this page's own rule that
counts live beside the command that produces them. It is deliberately not
split into per-file `adr/NNNN-*.md` documents: the log is cross-referenced from code comments
and the handbook, and one file keeps the *why* readable end to end. If the code ever
contradicts it, the code is wrong — flag it, don't silently "fix" the decision.

---

## Start here — by who you are

**A new engineer**
1. [../CLAUDE.md](../CLAUDE.md) — conventions, non-negotiables, and the rules learned the hard
   way (regressions that reached real congregations). Read before touching code.
2. [ARCHITECTURE.md](ARCHITECTURE.md) — how the pieces fit, end to end.
3. [DATA_MODEL.md](DATA_MODEL.md) — the entities and the invariants that govern them.
4. [DECISIONS.md](DECISIONS.md) — why. If a decision isn't here, it hasn't been made — ask.

**An operator / church volunteer**
1. [USER_GUIDE.md](USER_GUIDE.md) — 10-minute setup and the Sunday-morning path.
2. [OUTPUT_ROUTING.md](OUTPUT_ROUTING.md) — getting Relay onto your actual screens: a
   projector, an ATEM, an OBS machine, and the fourth screen you have no port for. It also
   names what does not work (no ATEM accepts NDI; the ATEM Media Player is a stills pool)
   before anybody spends money on it.
3. [AI_DISCLOSURE.md](AI_DISCLOSURE.md) — what the AI does, and what it will never do.
4. [PRIVACY.md](PRIVACY.md) — nothing you say, sing, or show leaves your computer.

**A contributor (code or language)**
1. [CONTRIBUTING.md](CONTRIBUTING.md) — the two contributions that need **no code**
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
- Auditing rather than building? [QA_HARNESS.md](qa/QA_HARNESS.md) — and read its **Part 4**
  before filing anything, so you don't re-discover a deliberate decision or "find" a bug that
  was fixed.

---

## How Relay gets audited

One document owns the QA apparatus: **[QA_HARNESS.md](qa/QA_HARNESS.md)**. It is **not** part of
the specification hierarchy above — SPEC and DECISIONS own the product; this owns how it is
checked.

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
`docs/qa/audits/`.

---

## Product health, honestly

Three documents, at three altitudes, and they are meant to disagree on scope rather than on fact:

- **[qa/RELAY_GAP.md](qa/RELAY_GAP.md) §24 owns the release decision and its reasoning**; nothing else restates it
  (this page, `README.md` and `CLAUDE.md` all did, and the copies disagreed). The V1 production audit that
  took it (2026-09-03, revised 2026-09-05) is archived at [archive/2026-09-05-relay-v1-audit.md](archive/2026-09-05-relay-v1-audit.md);
  its **three scorecards** and the disposition of every phase of both briefs — a 42-phase PWA brief
  and a live-service brief numbered §00–§105 — are [qa/QA_HARNESS.md](qa/QA_HARNESS.md) Parts 5 and 6,
  frozen at that date. Where a phase could not be reached from this machine it says **UNVERIFIED**
  and names the instrument that would answer it.
- **[qa/audits/](qa/audits/)** — frozen evidence. Four of the twelve are
  the audits done by DRIVING the app rather than reading it, and are therefore the only ones
  that have ever caught a congregation-facing rendering fault. **Read each one's header note
  before trusting an id in it**: the wave 4 and wave 5 passes were renumbered differently
  (`qa/RELAY_GAP.md` §23a), and one rewrote its body while the other did not.
- **[qa/RELAY_GAP.md](qa/RELAY_GAP.md) §27** — the gate list. Every box names the
  command that ticks it or says it has never been checked.

**Relay has never shipped. NOT READY for general release · READY WITH CONDITIONS for a
supervised pilot** — two churches, named operators, every service watched by somebody who can take
the wall back by hand. The register's size is a number with one home:
[qa/RELAY_GAP.md](qa/RELAY_GAP.md), where `relaygap.test.js` asserts it against the table. This
sentence used to carry its own copy, and the copy was stale by fourteen entries. **Four things
block a general release, and none is closed by a commit alone — two are not commits at all** (a
certificate is a purchase, an operator is a person): a wrong verse has reached a real congregation
(2026-08-30, 2026-09-06 and twice on 2026-09-20, each root-caused); word error rate has never been
measured in any language; neither platform has a code-signing certificate (RG-73); and nobody but
the author has ever run a service. The fifth, the auto-updater's dead endpoint, closed on 2026-09-05
(RG-83, RG-114). The conditions and the reasoning are both [qa/RELAY_GAP.md](qa/RELAY_GAP.md) §24.

The packaged build **has** been reached by an instrument — one live sermon
([FIELD-2026-08-30.md](qa/audits/FIELD-2026-08-30.md)). Pixels out, hardware, and a second
operator still have not: [QA-2026-08-14.md](qa/audits/QA-2026-08-14.md) §16 is the human test
script and it has not been run.

What remains open is captured, with reasoning, in [KNOWN_ISSUES.md](KNOWN_ISSUES.md) — and most
of it is blocked on the world (a certificate, a native speaker, thirty minutes of a real sermon
on tape), not on a commit.
