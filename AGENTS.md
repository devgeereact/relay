# AGENTS.md

Instructions for any AI coding agent working in this repository — Codex, Cursor, Copilot,
Gemini, Aider, Claude Code, or whatever comes next. It is tool-agnostic on purpose.

> **`CLAUDE.md` is operational law. This file never restates a fact that lives there.**
> Duplication is not redundancy here — it is drift. Six documents once said Relay had 114 Tauri
> commands; the code had 118, and the file every agent reads first was on the wrong side.
> If you need a number, get it from the command that produces it (below), not from prose.

---

## Read this order, every session

1. **`CLAUDE.md`** — the non-negotiable constraints, the tech stack, and the numbered
   **architecture rules learned the hard way**. Every one of those rules is a bug that reached
   real people. Do not regress them, and do not "improve" one without saying so first.
2. **`docs/README.md`** — the documentation index; it says which document owns which question.
3. **`docs/DECISIONS.md`** — *why*. **If a decision is not in there, it has not been made.
   Ask; do not assume.**
4. The volume you actually need: `ARCHITECTURE.md`, `DATA_MODEL.md`, `DESIGN_SYSTEM.md`,
   `LANGUAGES.md`, `RELEASING.md`, `QA_HARNESS.md`.

Auditing rather than building? **`docs/qa/QA_HARNESS.md` Part 4 first** — it is what the tests
already pin. An agent that skips it burns its run "finding" a bug that was fixed in July.

Wondering whether a feature is missing or refused? **`docs/qa/RELAY_GAP.md`** §2 and §17, and
**`docs/KNOWN_ISSUES.md`** §3. A great many obvious-looking gaps are recorded refusals.

---

## What this software is, and why that constrains you

Relay listens to a live sermon and puts scripture on a congregation's screens, offline, on a
volunteer's laptop, with no second take. **A bug here is not a bad user experience — it is the
wrong verse on a wall in front of a room of people, or a black screen mid-sermon.**

That single fact produces every rule below.

---

## The seven things you may not do

1. **Do not make the live path faster by making it less safe.** Thresholds, corroboration, and
   the auto-fire cap are not performance knobs. If a change touches audio, STT, detection,
   `router.rs`, `pipeline.rs`, `channels.rs`, the panic controls or the rehearsal gates, it needs
   an end-to-end test in `src-tauri/src/e2e.rs`, not just a unit test.
2. **Only a *heard* reference may reach a screen unattended.** A paraphrase is a suggestion at
   any score, and a cosine similarity is never displayed as a percentage. Raising a number is
   not a fix for a false positive; it is the same bug wearing a different hat.
3. **Never report a success that did not happen.** Panic controls return a result *and* raise a
   global error, because they fire from places that cannot `catch`. A green toast over a
   swallowed error is the worst line of code in this repository's history.
4. **Never build an `OutputContent` or a `DetectionEvent` by hand.** Go through
   `pipeline::Fire`. Five hand-rolled copies drifted, and two silently dropped the template.
5. **Nothing leaves the device without an explicit, visible reason.** Transcripts, verse text,
   lyrics, announcements and service names are the church's material, not diagnostics. Crash
   reporting is opt-in, off by default, and scrubbed — keep it that way.
6. **Do not add accounts, cloud sync, multi-tenancy, RBAC, SSO, billing, an analytics dashboard,
   a plugin marketplace, or native SDI.** Each is a *recorded refusal* with reasoning, not an
   oversight. To reopen one, write the reversal proposal (see below) and stop.
7. **Do not fork an existing surface.** There is one renderer, one error humaniser, one store,
   one test fixture, one readiness probe set, one keydown listener, and one error type. When you
   are tempted to write a second, you have found the thing to extend.

---

## The rule about guarantees, which is the one agents break most

**A guarantee is only kept on the doors you checked.**

This repository has had four separate bugs with one root cause: a rule enforced on one surface
and skipped on its twin. Rehearsal gated three of four kiosk publishers. The throw-vs-swallow
contract held for eight of nine wrappers. `NavResult` — built so nav could never again silently
do nothing — was thrown away by the LAN remote with `Ok(_)`. And the first-run wizard cleared its
"mic off" flag before awaiting the stop.

**When you fix something, enumerate every caller of the thing you fixed, and write the test on
the surface that was missed.**

---

## Writing tests here

- **Test the bug, not the fix.** Verify the new test *fails* if you reintroduce the original
  defect. One entitlement test in this repo initially passed against a broken file because it
  grepped a comment.
- **A test's assertion surface is part of its claim.** A test that listens for desktop events
  cannot see what leaves over the WebSocket. A rehearsal guarantee was green, and false for the
  preacher's tablet, for exactly this reason.
- **A component nothing renders is not covered, however green its tests.** Check something
  imports it before writing the test.
- **A contract stated in a comment is not a contract.** If you place a function in a behavioural
  group, add the test that holds it there.
- `cargo test` and `npx vitest` must both be green, and `cargo fmt --all` plus
  `cargo clippy --all-targets -- -D warnings` are enforced by CI.

---

## Commands you will actually need

Exact invocations, model paths, environment variables and the audio-debugging switches are in
**`CLAUDE.md` § Commands**. The four that matter most often:

```bash
npm test                                  # frontend suite
cd src-tauri && cargo test                # Rust suite (needs cmake on PATH)
node scripts/qa-inventory.mjs             # controls · orphans · command map · create paths
npm run version:check                     # the three version files must agree
```

**Counts come from those commands, never from a document.** If you need to quote a number in a
commit message, a report or a doc, run the command and put it beside the number.

---

## When you disagree with a recorded decision

You may. Some of them should be reopened eventually. But do it in the open:

> **Existing decision · original reasoning · proposed reversal · benefit · cost · new risks ·
> evidence required · why the current approach is insufficient.**

Write that, put it in front of a human, and stop. **Do not silently reverse a decision, and do
not implement one "to show what it would look like".** Two worked examples of the format live in
`docs/qa/RELAY_GAP.md` §20.

---

## Honesty rules for anything you write down

Relay's documentation is unusually candid about what has never been tested, and that is
load-bearing, not modesty. Preserve it.

- **"Not measured" is a valid answer and the correct one when nothing measured it.** Word error
  rate has never been measured in any language. No native speaker has reviewed the book aliases.
  Do not soften either sentence.
- **BLOCKED is a real outcome**, and it is a deliverable, not an excuse. Much of this product —
  **pixels out, hardware, a congregation** — has never been reached by any instrument in this
  repository. You cannot claim it from source.
  **Two of those blockers fell on 2026-08-30** and it is worth knowing how: the packaged build
  and audio-in were reached by *running a real service*, not by writing more tests. It produced
  seven findings in fifty minutes that months of source-reading had not.
- **"Blocked" is a claim, and it gets the same scrutiny as any other.** Three "blocked" entries
  in `RELAY_GAP.md` did not survive it: an old schema the repo supposedly did not keep (git had
  it), five commands documented as acceptably dead (they were deletable), and a licensing
  reason that was simply wrong (DECISIONS §32.4 says the opposite). **A wrong reason in a
  register parks the work forever**, because nobody re-examines something already explained.
- **A STATUS goes stale exactly like a count, and it is more dangerous when it does.** On
  2026-08-31 `RELAY_GAP.md` §2 still said **MISSING** about thirty-one requirements that had
  shipped that same week — directly underneath a fix log saying they had shipped. A wrong number
  reads as a number; a wrong status reads as a *decision*. If you are editing a document that
  scores the product, re-derive the rows you touch from the code, and cite a **file and a
  symbol** rather than a line number, because line numbers rot faster and rot invisibly.
- **Resolving something by deciding not to build it is a result, and it gets written down like a
  build** — the verdict, the reason, and the condition that would reverse it. "Not now" is how a
  decision decays back into a gap nobody re-argues and everybody re-files. See DECISIONS §62.
- **Audit the instruments, not only the code — it is the most productive seam in this
  repository and it is not close.** In one pass: a contract test that scanned one Rust file
  while claiming to cover the repository, its other half reading four frontend files out of
  nine, a CI job on a single Node version so the fix protecting every other one was never
  exercised, an agent brief describing a deleted component and a closed defect, and four
  citations pointing at a section that does not exist. Eleven of thirteen findings in the
  accessibility pass were the scanner's own bugs. **When a check says everything is fine, ask
  what it actually reads.** A scanner that quietly narrows passes everything.
- **Never imply you ran, clicked, saw or heard something you did not.**
- A count you cannot reproduce is a rumour.

---

## Repository conventions

- **Every change goes through a pull request.** Do not commit to `main`, infrastructure and
  one-line fixes included.
- Commit messages say what changed **for the person in the booth**, not which function moved.
  `CHANGELOG.md` is read by operators deciding whether to restart the app twenty minutes before
  a service.
- Pre-release versions must be **numeric** (`0.1.0-1`, never `0.1.0-rc1`) — the Windows MSI
  bundler rejects a named identifier fifteen minutes into a release.
- The version lives in **three** files and `npm run version:set` is the only thing that may
  change it.
