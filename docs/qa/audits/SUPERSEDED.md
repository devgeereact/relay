# Superseded whole-product audits

**Two whole-product audits, merged into one file on 2026-09-21** and otherwise untouched.
Both are **frozen** and both are **superseded**: what is live is `../RELAY_GAP.md` (the register,
the release decision and the launch checklist) and `../QA_HARNESS.md` (the apparatus, the
scorecards and the brief disposition). They are kept because they are evidence of what was true
when they were written, and because rows in the register cite them. Nothing is edited except
citation paths.

**What is in this file, in order:**

- `PRODUCT-2026-07-13.md` — Relay — Product Audit (retired, 2026-09-02)
- `QA-2026-08-14.md` — Relay — QA Audit, 2026-08-14

---

<!-- ===== was docs/qa/audits/SUPERSEDED.md, merged 2026-09-21, verbatim ===== -->

# Relay — Product Audit (retired, 2026-09-02)

> ## Retirement note — this document is now frozen evidence
>
> **It was `docs/PRODUCT_AUDIT.md`. It is now a dated audit under `docs/qa/audits/`, and it may
> not be edited again.** That is what the other four documents in this directory are, and it is
> what this one always was: a scorecard produced on a date, against a commit hash, by a named
> method. Keeping it live invited exactly the drift it was meant to catch — four documents were
> carrying the same GO/NO-GO, the same readiness scores and the same blocked-on-the-world list,
> and three of the four had gone stale.
>
> **Where each part went:**
>
> | This document's | Now owned by |
> |---|---|
> | §2 Product Scorecard | [`RELAY_V1_AUDIT.md`](../../RELAY_V1_AUDIT.md) §15, reconciled with [`RELAY_GAP.md`](../RELAY_GAP.md) §22 |
> | §16 Production Readiness Checklist | [`LAUNCH_CHECKLIST.md`](../LAUNCH_CHECKLIST.md) |
> | §15 Prioritised Roadmap · §5 D1–D5 | [`KNOWN_ISSUES.md`](../../KNOWN_ISSUES.md) and the `RG-` register |
> | §6–§14, §17 | kept here — the strategy, the information architecture and the §13 NOT-APPLICABLE reasoning are the parts worth reading, and they are why this was retired rather than deleted |
>
> **Three things in the body below are wrong, and stay wrong**, because an audit that corrects
> its own findings stops being evidence. They are recorded here instead:
>
> 1. **§Closing: "the decision log is now twenty-five rules deep."** True on 2026-07-13. It is
>    **46** numbered decisions (§18–§63) plus 28 earlier table rows — this document's own banner
>    says 46, six hundred lines above the sentence that says twenty-five.
> 2. **§17: "100% on the 50-case corpus."** The corpus is **74** cases —
>    `jq '.cases|length' src-tauri/data/eval_corpus.json`. It was 50 when this was written.
> 3. **Twenty-six `file.rs:NNN` citations.** Every one of eight sampled now lands on a closing
>    brace, a `///`, or a blank line. This is the pattern RG-51 bans — *never a line number* —
>    and this document predates that rule. Do not follow them; search for the symbol instead.
>
> Four relative links in the body (`audits/SUPERSEDED.md`, `audits/FIELD.md`,
> `RELAY_GAP.md`, `DECISIONS.md`) were correct from `docs/` and are wrong from here. They resolve
> to [`QA-2026-08-14.md`](SUPERSEDED.md), [`FIELD-2026-08-30.md`](FIELD.md),
> [`RELAY_GAP.md`](../RELAY_GAP.md) and [`DECISIONS.md`](../../DECISIONS.md).

---

**Revision 3 · 2026-07-13 · verified against `cfa2aa5`**
Supersedes Revision 2 (2026-07-12) and Revision 1 (2026-07-05). Every claim was re-verified against the code that exists today; nothing is carried forward on trust. Line references are live.

> ## ⚠️ Status as of 2026-08-31 — read this before the scorecard
>
> **The current per-dimension scores are `RELAY_GAP.md` §22, not the table below.** That one
> was re-scored twice on 2026-08-31 and states plainly that **not one score moved on the second
> pass** — because a readiness score tracks evidence, not effort. This document's scorecard is a
> 2026-07-13 snapshot and is kept for its *shape* and its §13 reasoning.
>
> **This revision is seven weeks behind, predates the first full machine audit entirely, and
> predates the first time Relay ever ran in a real room.** The scorecard below is still the right *shape* — the strategy,
> the scope decisions and the §13 NOT-APPLICABLE reasoning all stand — but treat its numbers and
> its "what's left" list as a snapshot of 2026-07-13, not as today.
>
> What has happened since, and what it changes:
>
> | | Rev 3 said | Today |
> |---|---|---|
> | Tests | 250 Rust + 138 frontend | Run `cd src-tauri && cargo test` and `npx vitest run` — **a number written here was wrong within a week, three times** (RELAY_GAP §18) |
> | `main.rs` | 2,922 lines / 101 commands | `wc -l src-tauri/src/main.rs` · `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` — it has drifted past every number ever written here |
> | Decision log | 25 decisions | **46** numbered (§18–§63), plus 28 earlier table rows — `grep -cE '^## [0-9]' docs/DECISIONS.md` |
> | `Result<_, String>` in `main.rs` | replaced | **zero remain** — confirmed |
> | Release decision | not stated | **NO-GO for general release, GO for a supervised pilot** (2026-08-31) — RELAY_GAP §24 owns it, not this document |
>
> **The load-bearing change is the audit this revision could not have seen.**
> [`audits/SUPERSEDED.md`](audits/SUPERSEDED.md) was a six-agent full-scope run. It raised
> **one P0 and eleven P1s** — the P0 being that Relay would, unattended, put **Numbers 3:16** on
> the wall when a preacher said *"please turn to hymn number three sixteen"*. **Every one of the
> twelve is now closed** (that document's §0 fix log records each closure and the test that fails
> if it returns), and the repair for the P0 was structural — a `DetectionMethod::UncertainBook`
> that no confidence score, sensitivity dial or calibrator drift can undo (CLAUDE.md rule 10).
>
> **The release decision is now NO-GO for general release and GO for a supervised pilot** (RELAY_GAP §24), on the condition Rev 3 never scored:
> *roughly half of Relay, as a volunteer experiences it, has never been reached by any
> instrument here.*
>
> **Two of those zeros moved on 2026-08-30, and the verdict did not.** Relay ran a live
> sermon — 49.5 minutes, packaged build, `ggml-large-v3-turbo`
> ([`audits/FIELD.md`](audits/FIELD.md)):
>
> | | Rev 3 / the earlier banner | After the first real service |
> |---|---|---|
> | The packaged build | 0% | Built, installed, ran a service. Notarization still needs a certificate |
> | Audio in | 0% | A real preacher transcribed for 49.5 min. **Word error rate still unmeasured, in every language** |
> | Pixels out · hardware · a real congregation | 0% | Unchanged |
> | Stage F11 (does latency drift over a long service?) | never run | **Run. No drift** — p50 held 627–699 ms across 2,423 decodes |
>
> **And that service is the argument FOR this banner, not against it.** Fifty minutes in a room
> produced **seven findings** that months of reading source had not — a wrong verse on a
> congregation's wall, an end-to-end metric that was measuring how long the preacher had been
> talking, a service record pointing at the wrong sentence. One was withdrawn as wrong. Six are
> closed. That ratio is what "half of this product has never been reached by an instrument"
> means when somebody finally reaches it.
>
> The narrower sweeps of **2026-08-29 through 2026-08-31** re-measured the counts above, hunted
> contradictions across the doc set, and closed **56** register items;
> [RELAY_GAP.md](RELAY_GAP.md) is the record. None raised a new P0, and none moved the release
> decision.
>
> **The last of those sweeps was aimed at this document set rather than at the code**, and what
> it found is worth stating here because this banner is exactly the kind of thing it catches:
> `RELAY_GAP.md` §2 — the item-by-item status matrix — still said **MISSING** about thirty-one
> requirements that had shipped during the same week, *directly underneath a fix log saying they
> had shipped*. Four documents also quoted a line from `README.md` that had been deleted. Both
> are corrected, and the register's own summary counts are now asserted against its table by a
> test, because prose that counts something it sits beside is the cheapest thing in this
> repository to check and the most reliably wrong when it is not checked.
>
> So the honest one-line summary of this document, updated: **the code is done and genuinely
> hardened, and it has now been run once in a real room by the person who wrote it; what is
> left needs a certificate, a projector, a Yorùbá speaker, a second Sunday, and an operator who
> is not the author.** Nothing on that list is a commit — which is what Rev 3 said, and it was
> right.

> **Revision 3 in one line: every finding in this document that could be closed by writing code has been closed. What is left cannot be — it needs money, a certificate, a native speaker, and thirty minutes of a real preacher on tape.**

**Scope, decided with the owner and unchanged:**
- **Strategy: unchanged.** Free, MIT, offline-first, no accounts, no server. The decisions in [DECISIONS.md](DECISIONS.md) stand.
- **Optimise for: the first 10 churches.** Not enterprise scale. The bar is *a volunteer, in a dark booth, with no training and no second take.*

Phases of the transformation brief that assume a commercial multi-tenant SaaS — billing, RBAC/SSO, multi-tenancy, audit logs, growth/monetisation, government/healthcare/finance readiness — are marked **NOT APPLICABLE** in §13, each with reasoning. They are not oversights. Adopting them would destroy the product's actual moat.

---

## 0a. What changed since Revision 2 *(16 commits, one day)*

Revision 2 raised five criticals and a roadmap. All five are fixed, and so is every non-blocked item in Phases 2, 3 and 4 of that roadmap. Retiring findings is as much an audit's job as raising them, so, explicitly:

| Revision 2 said | Reality today |
|---|---|
| 🔴 **D1** — a real tag ships an unsigned Windows installer, silently | **FIXED.** The gate is per-platform (two certificates, two verdicts) and refuses a real tag that is not covered on **both**. Windows signing is wired for Azure Trusted Signing *or* a `.pfx`. There is deliberately no combined `signed` flag any more — one global boolean standing in for two certificates *was* the bug. |
| 🔴 **D2** — the updater can never deliver an update | **FIXED.** The version lived in **three** files (not two — `Cargo.toml` as well). `scripts/version.mjs` owns all three; CI asserts they agree on every PR and the release gate asserts they equal the tag. |
| 🔴 **D3** — the panic path reports a success it did not achieve | **FIXED, from the Rust up.** A failed clear was *unrepresentable*: `channels::clear` discarded the emit error, `clear_screens` returned `()`, the JS swallowed the rest. Now `Result` all the way, and the frontend returns a boolean **and** raises a global banner (the panic controls fire from places that cannot `catch`). |
| 🔴 **D4** — the safety architecture is invisible | **FIXED.** `method` + `matched_text` cross the bridge and are rendered. A paraphrase shows **no percentage at all** — a cosine is not a probability. Cyan, never amethyst (that means rehearsal). |
| 🔴 **D5** — the model download hangs forever; Cancel is inert | **FIXED.** Own stall deadline (not reqwest's), cancel checked on a tick so it works when the network is *dead*, `running` cleared by a `Drop` guard, and the 416 brick gone (a full-size `.part` is settled by checksum, never by a `Range` request). |
| 🟠 macOS microphone entitlement | **FIXED.** Notarization forces the hardened runtime, under which the mic is TCC-killed without the entitlement — so the *first correctly-signed build* would have been the first one that could not hear the preacher, and no build we can make locally would have shown it. Pinned by `models::config_boots`. |
| `nav` silently does nothing | **FIXED.** `NavResult` — Fired / EndOfPassage / NoPassage / NotInLibrary. Not every outcome is a failure; the end of a passage is a correct boundary, and the operator is entitled to know *which*. A bool would have been the wrong repair. |
| `main.rs` has zero tests; no e2e anywhere | **FIXED.** `e2e.rs` drives the **real** commands against a real in-memory DB, through the real router and pipeline. To make that possible the fire engine is now generic over `tauri::Runtime` — which is the useful half of "split main.rs": the point was never the line count, it was that the engine could not be driven without a window. |
| 88 × `Result<_, String>`, no typed error | **FIXED.** `error.rs`: `{ kind, message }` — refused / not_found / busy / io / internal. SQLite carries the one distinction that matters live: *is pressing it again worth my time?* |
| ~34 `catch {}`, one `throw`, no contract | **FIXED.** One question — *can the congregation see the difference?* — and three groups (throws / swallows / reports-via-store), written at the top of `capture.js`. Applying it caught two more silent liars. |
| The detections migration | **FIXED.** It had no `ROLLBACK`, so a failure left the transaction open, the `PRAGMA foreign_keys = ON` became a no-op *inside* it, and the leftover scratch table made **every subsequent boot** fail with "table already exists". Forever. Before the window is even shown. |
| Accessibility 4/10: 0 focus traps, no `<h1>`, the AI announces nothing | **FIXED.** Focus traps on all 5 dialogs (with restore), a real heading structure, and the suggestion feed / transport / errors all announced. Every text token in the app now passes WCAG AA. |
| Empty ≠ Loading ≠ Error (Live said "No plans yet" before the DB answered) | **FIXED.** Three shared components. `ErrorState` only offers *Try again* when the backend says the fault is transient — which is the first place typed errors earn their keep. |
| `related_scripture`: built, registered, **zero callers** | **SURFACED.** In the Intelligence Feed, as the quietest thing in it: no tally colour, no confidence, and it says out loud that nobody spoke these references. |
| No i18n layer at all | **BUILT.** 60 lines, no dependency. Translation is now a *data* contribution (one JSON file, no code). The locale files for yo/sw/ha ship **empty on purpose** — see §11. |
| `USER_GUIDE.md` is written for a developer | **REWRITTEN for a volunteer.** The speech model first — the one step nobody expects, and without which Relay cannot hear a thing. A real troubleshooting table for what actually goes wrong in a booth. No tab that does not exist. |
| No CONTRIBUTING / CoC / CHANGELOG / templates | **ALL SHIP.** CONTRIBUTING leads with the two contributions that need **no code at all**. The CHANGELOG is written for the *operator* — it is what a volunteer reads in the update banner twenty minutes before a service, and an update with no explanation is asking them to gamble. The issue forms include a **language** form; the PR checklist is the project's real rules, not a generic one. |
| **The moat has never been measured** | **The ruler is now built.** `stt::bench::wer` — Levenshtein over words, folding punctuation exactly the way the detector does, and deliberately **not clamped at 1.0** so a hallucinating decoder scores *worse* than a silent one. Unit-tested, runs in CI **today**, with no recording in existence. `bench/README.md` says what to record; `bench/.gitignore` **refuses to let sermon audio into the repository** (PRIVACY.md's promise is not conditional on whose device it is). **The instrument is calibrated and pointed at nothing.** |

**What that leaves is the honest part.** Nothing on the list below can be fixed by writing code, which is why it is still here.

---

## 0b. What changed since Revision 1

Revision 1 named three critical blockers and called them "one epic". **That epic shipped.** So did most of Phase 2. Retiring stale findings is as much a part of an audit's job as raising new ones, so, explicitly:

| Revision 1 said | Reality today |
|---|---|
| 🔴 C1 — no in-app model download; user must run `curl` | **SHIPPED.** `models.rs:140` — resumable, SHA-256 verified, atomic `.part` rename, progress bar, cancellable. Reachable from first run, Settings, *and* a Live banner. |
| 🔴 C2 — no code signing | **HALF-SHIPPED.** macOS signing + notarization are wired (`release.yml:114-163`). **Windows is not signed at all** — see 🔴 D1. |
| 🔴 C3 — no auto-updater | **SHIPPED.** Plugin, pubkey, launch-check and never-during-a-service gating all exist. *(This row read "and inert — it cannot currently deliver an update" for a week after 🔴 D2 was fixed in the table above it: the three version files now agree, asserted in CI and against the tag. Since then RG-06 added preflight, a database snapshot before the update, verification on the next launch, and a restore that runs before the database is opened.)* |
| First-run wizard missing | **SHIPPED.** `FirstRun.svelte` — screen → microphone → fire a verse. |
| Console + Planner must merge | **SHIPPED, properly.** `Console.svelte` is gone; `ServicePlanner` imports *zero* fire commands (`ServicePlanner.svelte:19-38`). Live is the run surface. Planner cannot reach a screen. |
| Rehearsal mode | **SHIPPED**, gated at the broadcast choke point (DECISIONS §18). |
| No in-app Help | **SHIPPED**, and it is now *better than the written user guide*. |
| "ON AIR" reports the microphone, not the screen | **FIXED.** `App.svelte:169-186` keys off `$live` (backend output state). Mic has its own quieter indicator. |
| Cheatsheet lists dead keys | **FIXED.** `shortcuts.js:93` derives it from what the mounted view actually registered. |
| `--v-faint` fails WCAG AA at 3.4:1 | **FIXED.** Now `#88888d` — 4.55–5.61:1 across every surface it sits on. All pass. |
| PRIVACY.md / SECURITY.md / AI disclosure missing | **SHIPPED, all three.** PRIVACY.md is the best document in the repo and it discloses the unauthenticated LAN broadcast honestly. |
| Five data-integrity gaps | **ALL FIVE FIXED.** `reimport_full_kjv` is transactional (`db/verses.rs:318`); `import_song` is transactional (`db/songs.rs:238`); `delete_media` cascades to plan cues (`db/library.rs:266`); `move_plan_item` finds neighbours by ordering, not `position ± 1` (`db/plans.rs:202`); the Lower-Third forward-fill is id-scoped (`db/mod.rs:115`). |
| No way to measure detection accuracy | **SHIPPED.** `eval.rs` + a labelled corpus (`eval_corpus.json`), **scored through the real router**, CI-gated to fail the build above SPEC's 5% wrong-verse rate. |

That is an unusually complete execution of an audit. The scorecard moves accordingly.

**But the new findings are a different species, and a more dangerous one.** Revision 1's bugs were *visible*: a button that didn't work, text you couldn't read. The bugs found today are bugs of **false confidence** — a toast that says "Screens cleared" when the clear failed, a cheatsheet that teaches a panic key that doesn't work while typing, an updater that will silently never update, a signing pipeline that publishes an unsigned Windows installer and prints no warning. Software that lies to its operator is worse than software that fails in front of them, because the operator stops looking.

---

## 1. Executive Assessment

**The code is done. Relay is now blocked on four things, and not one of them is a commit.**

Revision 1's finding was *"the engineering is ahead of the product"* — the app could not be installed. Revision 2's was *"the product does not tell the operator the truth about itself"* — it reported successes it had not achieved. Both are now closed, and the second was the harder and more valuable of the two.

What that produced is a product with an unusual property for its stage: **its failure modes are visible.** A clear that fails says so. A `→` that cannot move says why it cannot. A paraphrase guess cannot masquerade as a heard reference, because it is rendered as a different kind of claim with no percentage attached. A release that would ship unsigned refuses to build. A migration that dies halfway can be retried. These are not features; they are the absence of a specific class of lie, and in software that fails **live, in front of five hundred people**, that class of lie is the whole danger.

The engine underneath is strong and now genuinely covered (**250 Rust + 138 frontend tests** when this was written; **478 + 581** today), zero panic sites in any module that runs during a service, a detection benchmark that fails CI on regression, an end-to-end test that drives the real fire → nav → clear path, and a gate that makes "the AI put the wrong verse on the wall" structurally unrepresentable rather than merely unlikely.

**So the honest position is now a shopping list, not an engineering plan:**

> **Updated 2026-08-31.** Item 3 is no longer a purchase — it is one environment variable.
> `RELAY_RECORD_WAV=/path/x.wav` writes the cleaned audio stream a service actually heard, and
> the release decision (RELAY_GAP §24) now permits supervised pilot services in which to set it.
> **The tape is the single highest-leverage thing on this page and it is now free to obtain.**

1. **~$10/month for a Windows code-signing certificate.** The gate now *refuses* to ship unsigned rather than doing it quietly — so until this is bought, Windows cannot ship at all. Windows is the platform most of the target market is on, for cost reasons.
2. **GitHub Actions billing.** The repo is private, so every macOS runner minute bills at ×10. Relay is MIT and open-source *by recorded decision* — making the repo public makes this problem disappear permanently and costs nothing that was being kept.
3. **Thirty minutes of a real preacher on tape.** This is the single highest-leverage item in the entire document, and it is not a coding task. It unblocks word error rate (never measured, in any language), the dormant STT bench (already built, already scores through the real detector), the fine-tune evaluation, and the unresolved question of whether the decoder-bias prompt is helping or hurting. **Every claim about the moat is currently an assertion.**
4. **Native speakers.** For the 66×3 book aliases (unreviewed), the Yorùbá numerals (unparsed), and the three locale files (which ship empty *on purpose* — see §11).

And one thing no amount of engineering substitutes for: **a real service, run by an operator who is not the author.**

The competitive bet in DECISIONS.md still holds, but the moat must be restated truthfully — the repo's own `LANGUAGES.md` does so, and this audit will not soften it: **Relay's African-language differentiator today is a hand-curated multilingual reference-parsing table on top of stock Whisper base — not African-language speech recognition.** That table is real, tested, and more valuable than it sounds (`LANGUAGES.md:22`: *"The moat was blocked on a lookup table, not on machine learning"*). But no fine-tune ships, no native speaker has read the book names, Yorùbá numerals are not parsed, and WER has never been measured. Item 3 above is what turns that from a claim into a number.

---

## 2. Product Scorecard

Scored against the stated bar (*first 10 churches*), not against Stripe. Δ is the move since **Revision 2**.

| Dimension | Score | Δ | Why |
|---|---|---|---|
| **Core engine** | **9 / 10** | — | Offline pipeline works end to end. Detection is DB-free and pure. **Zero `unwrap`/`expect`/`panic!` in any of the seven modules that run during a service.** Lock discipline (Db before Session; never emit under a lock) holds. Unchanged, and it did not need to change. |
| **Distribution / install** | **8 / 10** | ▲ +2 | Per-platform signing gate that fails loud, tag-derived version enforced in CI *and* at release, a model download that cannot hang or brick, and the macOS mic entitlement that would have killed the first signed build. **Not 10/10 for one reason only: nobody has watched an update actually install, and Windows has no certificate.** Both need money, not code. |
| **Onboarding / first-run** | **8 / 10** | ▲ +1 | The wizard now *proves* the microphone works — its meter was dead, so the one step whose entire purpose was proof proved nothing. Still cannot be re-run once skipped. |
| **UX (live operation)** | **8 / 10** | ▲ +2 | Every control that lied has been fixed: the clear toast, the `B`-while-typing cheatsheet line, `Esc`-wipes-the-wall-from-inside-a-modal (which turned out to affect the arrangement pickers too, not just the cheatsheet), and the `nav` key that silently did nothing. The transport now follows **what is on the wall**, not what the operator intended. |
| **UI / design language** | **8 / 10** | ▲ +1 | The dark/amber broadcast language remains correct and unmodernised. One `EmptyState`/`Loading`/`ErrorState` trio replaces four competing classes. The colour discipline held under pressure: a paraphrase got cyan, *not* amethyst, because amethyst already means rehearsal — a colour carrying a promise cannot be borrowed for a hunch. |
| **Architecture** | **8 / 10** | ▲ +2 | The fire engine is now generic over `tauri::Runtime`, so the path that puts scripture on a wall can be driven **without a window** — which is the useful half of "split `main.rs`". Typed errors (`error.rs`) replace 88 × `Result<_, String>`, and **zero remain**. `main.rs` was 2,922 lines / 101 commands here and is several thousand lines larger — **the current pair is deliberately not written down**, because it has been wrong within a week every time it has been, and twice within one day (RELAY_GAP §18). Reproduce it: `wc -l src-tauri/src/main.rs` and `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` — but it is no longer untestable, which was the actual problem. |
| **Performance** | **9 / 10** | — | Unchanged. Measure-before-optimising is practised here, not preached: the semantic scan stays a linear scan and beam search stays unused, both because measurement said so. |
| **Accessibility** | **8 / 10** | ▲ **+4** | Focus traps on all 5 dialogs, **with focus restore** (the half everyone forgets). A real heading structure. The AI suggestion feed, the transport and errors are all announced — the product's whole reason to exist used to arrive in total silence. Every text token passes WCAG AA. Not 10/10: ~150 lines of dead legacy CSS remain, deliberately (see §7). |
| **Security** | **8 / 10** | ▲ +1 | A tag name is no longer interpolated into a release shell (a real injection vector CodeRabbit caught). LAN bind is unauthenticated, broadcast-only, bounded, and honestly documented. Unsigned Windows remains the exposure — but the pipeline now *refuses* to produce it rather than doing so quietly. |
| **Privacy** | **9 / 10** | — | Unchanged, and still the strongest part of the product. Telemetry off by default, no DSN in OSS builds, free text *dropped* not sifted. |
| **Testing** | **9 / 10** | ▲ **+3** | **250 Rust + 138 frontend** at the time of writing; **478 + 581** today. The gap was never the count — it was that `main.rs` had zero tests and no e2e existed, so the fire → nav → clear path was verified only by hand. `e2e.rs` now drives the real commands against a real DB. And the culture shifted: several fixes were **mutation-verified** — the test was checked to *fail* when the original bug was reintroduced. Two tests in this repo initially passed on broken code; both were caught that way. |
| **Developer experience** | **9 / 10** | ▲ +2 | CI, CodeRabbit, `clippy -D warnings`, a decision log 25 rules deep (**46 numbered decisions** and **40** hard-way rules today — `grep -cE '^## [0-9]' docs/DECISIONS.md`, `grep -cE '^[0-9]+\. \*\*' CLAUDE.md; the '35' this cell carried was itself stale within a week`). `scripts/version.mjs` makes releasing a one-liner. CONTRIBUTING / CoC / CHANGELOG / issue forms / PR template all ship — and the PR checklist is the project's *real* rules (no `unwrap()` in a live path, no borrowed tally colours, *reintroduce the bug and check your test fails*), not a generic one. |
| **AI readiness** | **6 / 10** | ▲ +1 | The operator can finally *see* what kind of claim the AI is making, and `related_scripture` — built, tested, and called by nothing for months — is surfaced. The gate remains excellent. But paraphrase is still TF-IDF, `verses.embedding` has still never been written to, and **the acoustic layer is still unmeasured**. Blocked on audio, not on code. |
| **Brand** | **4 / 10** | — | Unchanged and now the weakest column. Still no logo, no tagline, no positioning line, and the name is still undecided (`docs/SPEC.md`). **The old *"Working name — rename freely"* line is no longer in `README.md`** — this row quoted it for a week after it was removed |
| **Business model** | **N/A** | — | Deliberately free/MIT. Sustainability parked, not decided. |
| **Documentation** | **9 / 10** | ▲ **+4** | CLAUDE.md, DECISIONS.md (§20–§25) and RELEASING.md are current and unusually honest — each rule is a bug that reached, or would have reached, a congregation. **USER_GUIDE.md is rewritten for a volunteer**: the speech model first, a real troubleshooting table, and no tab that does not exist. CHANGELOG is written for the *operator*, because that is who reads it in the update banner. `bench/README.md` tells someone exactly how to measure the moat. The only doc still missing is the one that cannot be written yet — a real number for word error rate. |
| **Legal compliance** | **9 / 10** | ▲ +3 | `LICENSE` names its holder. PRIVACY / SECURITY / AI_DISCLOSURE / CONTRIBUTING / CODE_OF_CONDUCT / CHANGELOG all ship and are accurate. KJV-only with no import path for any other translation, so no exposure. WCAG now largely passes. Complete. |
| **Enterprise readiness** | **N/A** | — | Explicitly out of scope. See §13. |
| **Overall maturity** | **8.5 / 10** | ▲ **+2** | *Code-complete, documented, and blocked on the world.* Every finding a commit could close is closed — including the measuring instrument for the one thing still unknown. What remains needs a certificate, a billing page, **a microphone in a real church**, and people who speak Yorùbá. |

---

## 3. Strengths — protect these

1. **Offline-first is the moat, not a constraint.** It works when the power flickers and the wifi dies. Every "modernisation" instinct that erodes it is wrong.
2. **One template engine, one renderer.** `TemplateRender.svelte` drives the editor preview, the console wall, *and* the real output. WYSIWYG is true by construction, not by discipline. Still the best idea in the codebase.
3. **The gate is structural, not numeric.** `router.rs:203` refuses auto-fire for any non-`Direct` method *before* it consults a threshold, and a property test sweeps every sensitivity × every confidence to prove it. "The AI put the wrong verse on the wall" is not unlikely here; it is close to unrepresentable. This is the single best engineering decision in the product.
4. **Rehearsal gates at the broadcast, not at the caller.** Seven fire sites and rising; gating at the one choke point makes every *future* caller sandboxed by construction. It also fails open, which is the correct direction.
5. **Measurement culture.** The semantic scan was benchmarked and left as a linear scan. Beam search was benchmarked and deliberately not used. The STT bench scores through the *detector*, not by grepping the transcript, because a grep-scorer once rated a hallucination a success. This instinct is rarer than any feature.
6. **Honest seams.** NDI returns a clear error instead of pretending. `LANGUAGES.md` states plainly that no fine-tune ships and why. Do not let a rewrite sand this off.
7. **The decision log.** `DECISIONS.md` with reasoning *and* explicit non-goals is worth more than the code it describes.

---

## 4. Weaknesses

Revision 2 listed ten. **Seven are fixed.** What is left is real, and — with two exceptions — cannot be fixed by typing.

1. **The moat is unmeasured, and that is now the single biggest weakness in the product.** No fine-tuned acoustic model ships, no native speaker has reviewed the 66×3 book aliases, Yorùbá numerals are not parsed, and **word error rate has never been measured in any language**.
   **The ruler now exists.** `stt::bench::wer` is written, unit-tested and runs in CI *today* — Levenshtein over words, folding punctuation the same way the detector does, and deliberately **not** clamped at 1.0 so a hallucinating decoder scores *worse* than a silent one. `bench/README.md` says exactly what to record and how to run it, and `bench/.gitignore` refuses to let sermon audio into the repository at all (PRIVACY.md's promise is not conditional on the device belonging to a church).
   So the blocker is now precisely one thing, and it is not a keyboard: **thirty minutes of a real preacher on tape.** The instrument is calibrated and pointed at nothing.
2. **Nobody has watched an update install.** The path is capable of it and the version can no longer drift — but "capable" is not "observed", and this is the mechanism by which every future fix reaches a church.
3. **Windows cannot ship.** By design: the gate refuses. It needs a ~$10/month certificate.
4. **The first-run wizard cannot be re-run.** An operator who skips it cannot get it back; everything in it lives in Settings, but they have to know that.
5. **~150 lines of dead legacy CSS remain**, including a colour that failed AA (now fixed in value, not removed). Deleting it needs eyes on a running app — Svelte does not scope a global stylesheet, and those rules use generic class names (`.tab`, `.dot`, `.live`) that live components still carry. See §7.
6. **`main.rs` is still 2,922 lines and 101 commands.** No longer *untestable* — the fire engine is runtime-generic and covered by `e2e.rs` — but still a single file holding both the IPC surface and the live engine.
7. **Brand is untouched.** Still no logo, no tagline, no positioning line, and a name that is still undecided — stated in `docs/SPEC.md`, no longer in `README.md`.

---

## 5. Critical Issues — the five that decide whether Relay survives contact with a church

> **Status, 2026-07-12: all five are fixed in code.** Each is kept below with its
> original diagnosis intact — a fixed bug whose reasoning is deleted is a bug that gets
> rewritten. Two things still need a **human**, not a commit:
>
> 1. **Buy a Windows code-signing certificate** (Azure Trusted Signing, ~$10/mo). The gate
>    now *refuses* to ship an unsigned Windows build on a real tag rather than doing it
>    silently — but it cannot buy the certificate for you.
> 2. **Watch an update actually install**, once, on a real machine. The path is capable
>    of it now; nobody has seen it happen.
>
> The **macOS microphone entitlement** — the sixth issue, below — is also fixed, as is
> `LICENSE`, which now names its copyright holder. Phase 1 is complete in code.

### ✅ D1 — A real release tag publishes an unsigned Windows installer, silently
*Fixed 2026-07-12. The gate is now per-platform and the Windows secrets are actually consumed. Ships as soon as a certificate is bought — see the note at the end of this section.*

The pre-flight gate (`release.yml:101`) sets `signed=true` on the presence of **`APPLE_CERTIFICATE` alone**. There is **no `bundle.windows` block in `tauri.conf.json`**, no `certificateThumbprint`, no `signCommand`, and no Windows cert-import step. The two `WINDOWS_CERTIFICATE*` env vars at `release.yml:169-170` are consumed by nothing.

Tag `v0.2.0` with all six Apple secrets set → the gate passes → macOS is signed and notarized → **the Windows `.msi` ships unsigned**, and the ⚠️ unsigned-build banner in the release notes (`release.yml:185`) is keyed on `signed == 'false'`, which is now `true`. Nothing tells the maintainer. Nothing tells the church. Windows is the target market's dominant platform on cost grounds (DECISIONS.md).

**Fixed.** Three changes, in `release.yml`:
- **The gate is per-platform.** macOS and Windows are two certificates and now get two independent verdicts. A real tag requires both and fails before it builds anything, naming the exact missing secrets. macOS additionally requires the *notarization* credentials, not just the certificate — a signed-but-un-notarized app is still blocked by Gatekeeper, so from a church's point of view it is unsigned.
- **The Windows secrets are now consumed by something.** Two schemes, chosen by which secrets are set: Azure Trusted Signing (via a `signCommand` calling `trusted-signing-cli`) or a classic OV/EV `.pfx` (imported to the runner's store, found by thumbprint). The signing config is *generated per build* and merged over `tauri.conf.json` with a second `--config` — it cannot be committed, because a thumbprint in the base config would break `tauri build` for every contributor on Windows who doesn't hold the certificate.
- **There is deliberately no combined `signed` flag any more.** One global "is it signed?" boolean, standing in for two independent certificates, *is* the bug. Every consumer now has to ask about a specific platform. The release notes carry a separate per-platform warning.

Verified by executing the gate against all nine secret combinations: a real tag with only the Apple secrets — the exact shipped bug — now refuses with `Windows is UNSIGNED — missing: AZURE_* (recommended) or WINDOWS_CERTIFICATE`. A pre-release with the Apple secrets still signs macOS.

**Still required from a human:** buy the certificate. Azure Trusted Signing, ~$10/mo, no HSM. Until then, every Windows release must be a pre-release tag — which the gate now enforces rather than assumes. And the signing step itself (PowerShell, needs a real certificate and a Windows runner) has **not** been executed — it cannot be, locally. The first real tag is its first run.

### ✅ D2 — The updater cannot deliver an update, and will not say so
*Fixed 2026-07-12.*

Two independent faults, either one sufficient:

- **The version is hard-coded.** `tauri.conf.json:4` and `package.json:4` both say `0.1.0`. Nothing in `release.yml` derives the version from `github.ref_name`, and no CI check compares them. Tag `v0.2.0` without hand-editing both files and `latest.json` advertises the new artifacts *under version 0.1.0* — every installed client compares equal and **never updates**. Silently. Forever. `RELEASING.md:106` says "bump the version first" — in a comment, in a code block, enforced by nothing.
- **The endpoint cannot serve the builds we can currently make.** The updater points at `.../releases/latest/download/latest.json`, and GitHub's `/releases/latest/` resolves only to **non-draft, non-prerelease** releases. `release.yml:186` forces `releaseDraft: true` and `:187` forces unsigned builds to `prerelease: true`. So `RELEASING.md:234-236`'s claim that the updater "can be tested end to end today" is false — the artifacts are produced and the endpoint 404s.

**This is the exact failure the updater exists to prevent.** We fixed six screen-facing bugs and built the mechanism to ship them; the mechanism is currently a no-op.

**Fixed.**

*The version.* It turned out to live in **three** files, not two — `src-tauri/Cargo.toml` carries it as well. `scripts/version.mjs` now owns all three (`npm run version:set -- 0.2.0`), CI asserts they agree on **every PR**, and the release gate asserts they also equal the tag **before it builds anything**. A `v0.2.0` tag against a repo that still says `0.1.0` now refuses with the reason, not a green build. It also rejects a version Tauri cannot parse as semver — an unparseable version is a version no church ever updates past — and refuses a `workflow_dispatch` fired from a branch, which would otherwise stamp a release `main`.

*The endpoint.* Re-examined, and the original finding was **half wrong** — worth recording, because the half that is right is the dangerous half. The *production* path is fine: a plain tag builds a non-prerelease draft, and the moment you publish that draft `/releases/latest/download/latest.json` resolves and every install offers the update. Draft-by-default is deliberate and stays. What is genuinely broken is the *testing* path, and `RELEASING.md` asserted the opposite (*"the auto-updater can be tested end to end today"*): `/releases/latest/` skips prereleases by design, and a pre-release tag is the **only** kind you can cut before you own certificates. So the updater could not be exercised until the day it mattered. `RELEASING.md` now says so, and gives a real recipe — build a local app stamped `0.0.1` pointed at a specific RC tag's manifest by exact URL, and watch it offer the update.

**Still required from a human:** perform that end-to-end update once, on a real machine. The code path is now capable of it; nobody has watched it happen.

### ✅ D3 — The panic path tells the operator it worked when it didn't
*Fixed 2026-07-12.*

```js
// src/lib/views/Live.svelte:221-231
async function clearAll() {
  try { await clearScreens(); }
  catch { /* backend absent */ }     // ← dead code: clearScreens() swallows internally
  setStageNext(null, null);
  flash('Screens cleared');          // ← unconditional
}
```

`clearScreens()` (`capture.js:897`) already swallows its own errors, so the `catch` here can never fire. If the Rust `clear_screens` command fails, the operator sees **"Screens cleared"** while the verse is still on the wall. `Live.svelte:649` is worse — `blackScreen()` isn't even awaited before `flash('Blackout')`. And `shortcuts.js:117` / `App.svelte:193` (the Escape key and the Emergency Stop button) call `clearScreens()` fire-and-forget with no error path at all.

Two related lies, same family:

- **`Esc` clears the congregation's screens as a side-effect of closing the help overlay.** `shortcuts.js:115-121` runs `clearScreens()` unconditionally, then also sets `cheatsheet.set(false)`. There is no "is the cheatsheet open?" guard. An operator presses `?` mid-service to check a binding, presses `Esc` to put it away, and **wipes the wall**.
- **The cheatsheet teaches a false fact about a panic key.** `App.svelte:255` says *"`Esc` and `B` work on every tab, even while typing."* But `shortcuts.js:124` — `if (typing) return;` — sits **above** the `B` handler. `B` does nothing while the cursor is in an input. This is the precise failure mode `shortcuts.js`'s own header comment warns against.

Partial credit, and it matters: the `aria-live` region and the ON AIR badge both key off `$live` (backend truth), so they stay honest. **It is the visual toast that lies — and the toast is what the operator is looking at.**

**Fixed, from the Rust up.** The root cause was that a failed clear was *unrepresentable*: `channels::clear` discarded the emit error with `let _ =`, `clear_screens` returned `()`, and `clearScreens()` swallowed whatever was left. Nothing in the stack could express "this did not work", so the toast had nothing to check.

- **Rust:** `channels::clear` / `black` now return `Result`, and `clear_screens` / `blackout` are `Result` commands. The debounce is forgotten and the cue recorded **only on success** — if the screens did not clear, the verse *is* still up, and "forget what's on screen" would have been a lie told to the router as well as to the operator.
- **The two fire-and-forget paths got a voice.** The spoken *"clear the screen"* and the exit from rehearsal (which hands the wall back to the congregation) have no caller to return an error to, and both used to `let _ =` it. They now raise `output://panic_failed`. The **spoken** clear is a panic control too, and it was as silent as the keyed one.
- **Frontend:** `clearScreens()` / `blackScreen()` return a boolean **and** set a global `panicError` store. Both, deliberately — the panic controls are fired from a global keydown handler and from a shell button that must work even when the current view has crashed, and a `throw` in those places is an unhandled rejection, which is to say silence. A silent failure is now unrepresentable, whichever way the control is triggered.
- **The banner is unlike every other message in Relay:** top of screen, `role="alert"` / `aria-live="assertive"` (the one message allowed to interrupt a screen reader), rose — never amber, because amber is a tally light and is never allowed to lie — and it **does not auto-dismiss**. A toast that fades after 2.6 seconds is precisely how the operator misses it.
- **`Esc` no longer wipes the wall to close the help overlay.** It closes the overlay and does nothing else. With no overlay open it is still the panic key, unchanged.
- **The cheatsheet stops teaching a false fact about a panic key.** `B` cannot fire from inside a text field — an operator typing "Habakkuk" into the reference box must not black out the room on the `b` — so the *behaviour* was right and the *promise* was wrong. The footer now says what is actually true.

**Verified, and the tests were checked against the bug rather than the fix.** 10 new tests (`panic.test.js`, plus three in `shortcuts.test.js`). Reintroducing each original bug was confirmed to fail them: removing the `Esc` guard fails 1, restoring the error-swallowing `clearScreens` fails 3. Full suite: 214 Rust + 69 frontend, `clippy -D warnings` clean.

### ✅ D4 — The safety architecture is invisible at the moment of decision
*Fixed 2026-07-12.*

`pipeline.rs:155` already ships `method` (`"direct"` / `"semantic"`) across the IPC bridge. `Live.svelte:478-482` throws it away and renders every candidate identically:

> **AI suggestion** · John 3:16 · **92% match**

A 92% *heard reference* and a 92% *TF-IDF cosine against a bag of words* are not the same claim, are not on the same scale, and — per `detection.rs:29-31` and DECISIONS.md — **the second one is not a probability at all**. The operator is shown a number that means one thing for one kind of match and nothing for the other, with no way to tell them apart. The only place `method` is ever rendered is `History.svelte:119` — *after* the service.

And `matched_text` — the actual words that triggered the match, the clearest possible explanation of an AI decision — is captured at `detection.rs:779`, marked `#[allow(dead_code)]`, and **never leaves Rust**. It isn't even a field on `DetectionEvent`.

**Fixed.** The operator can now see both things they are being asked to judge.

- **`matched_text` crosses the bridge.** It rides `Cand` → `Fire` → `DetectionEvent`, through the one pipeline a verse already takes, so a sixth fire path gets it by construction. The console shows the words: *Heard — "john three sixteen"*. An operator can tell at a glance whether Relay heard the reference or misheard "gone free sixty".
- **A paraphrase can now explain itself,** which turned out to be the real work. A TF-IDF match has no transcript span — its evidence is *which rare words overlapped*. `SemanticIndex::top_k_explained` returns the terms that actually produced the cosine (ranked by their contribution to it, so it is the true reason and not a plausible-looking one). The card reads *Matched on — "shepherd · lord"*, which a human can agree or disagree with. `0.61` is not.
- **The card is visibly a different kind of claim.** Heard: gold, a confidence bar, a percentage. Guess: cyan, no glow, **no number at all** — printing "61%" beside a cosine invites the operator to read it as "61% likely to be right", which is exactly what it is not. A number that lies is worse than no number, because it looks like information and gets acted on.
- **Not amethyst, contrary to this audit's own recommendation.** Amethyst already means REHEARSAL (DECISIONS §18). A colour that means *"nothing is reaching the congregation"* cannot also mean *"this guess is shaky"* — on the day both are true, the operator reads the wrong one. Cyan instead.
- **The presentation rule is now pure and tested** (`src/lib/detect.js` + 8 tests), not buried in a `.svelte` file where it could not be pinned. The key test asserts a paraphrase never shows a percentage *at any score* — the frontend mirror of `router.rs`'s property test that a paraphrase never auto-fires at any score.

**Also found while wiring it:** a `#[test]` I wrote asserting "the rarest shared word leads the explanation" failed — correctly. In a 3-verse fixture "lord" and "shepherd" each appear once, so their IDF is identical and the ranking ties. The claim is only true at corpus scale. The test now builds a corpus where "lord" is actually common. The assumption was wrong, not the code.

220 Rust + 77 frontend tests, `clippy -D warnings` clean.

### ✅ D5 — The model download hangs forever on a church's flaky wifi, and Cancel does nothing
*Fixed 2026-07-12.*

`models.rs:184-187` builds a `reqwest::Client` with **no `timeout` and no `read_timeout`**, and the cancel flag is checked only *after* `stream.next().await` yields (`models.rs:219-222`). A half-open TCP connection — a dropped wifi, the single most likely real-world church-network event — means `stream.next()` never returns. Progress freezes at N%. No error is emitted. **Cancel is inert.** And `running` is never cleared (`models.rs:152` is unreachable), so every subsequent attempt returns *"A model download is already running"* **until the app is restarted**.

Adjacent: a `.part` file that is exactly `model.bytes` long (crashed on the final chunk) sends `Range: bytes=<len>-`, the server answers **416**, `models.rs:198` hard-errors, and the `.part` is never deleted — **permanently bricked** until the user finds and deletes a file they don't know exists.

**Fixed.** The failure this module has to survive is not "the download fails" — it is *"the download neither succeeds nor fails, forever, and the operator cannot get out of it"*: a volunteer, an hour before the service, with no terminal.

- **The stall is now owned by us, not by the HTTP client.** The read loop waits on `tokio::time::timeout(CANCEL_POLL, stream.next())` and gives up only after `STALL_TIMEOUT` (45s) with no byte at all. Deliberately **not** a whole-request `reqwest .timeout()` — that would abort a legitimately slow 148 MB download on exactly the connections this feature exists for. A stall is measured as *the gap between bytes*, not the length of the download.
- **Cancel works when the network is dead**, which is the only time it matters. It is checked on every 400 ms tick, not only after a chunk arrives. A cancelled download **keeps its `.part`** — cancelling means "stop", not "throw away my 90 MB".
- **Cancel is no longer an error.** It emits `model://cancelled`, not `model://error`. Stopping your own download used to paint a red failure box — one with no dismiss, so it sat there until the component remounted, directly above a working *Try again* button. The error box is now dismissable too.
- **`running` clears via a `Drop` guard**, so it releases however we leave — including a panic or a dropped future. The old bare `store(false)` after the await was never reached by the infinite hang, so the flag stayed set for the life of the process and every retry — *even after the wifi came back* — was refused with "A model download is already running" until Relay was quit and reopened. A recoverable blip became a dead feature.
- **The 416 brick is gone.** A `.part` of *exactly* `model.bytes` is now settled by **checksum**, never by asking the server to resume from the end of it. The guard was `> model.bytes`, so an exactly-full part file sent `Range: bytes=147951465-`, got **416**, hard-errored, and *did not delete the file* — so every retry hit the same 416, forever. If the checksum passes we rename it into place (the download was actually complete); if not, we delete it and start clean. A 416 from the server also now deletes the `.part` rather than leaving it to poison future attempts.

**Verified.** The resume decision was extracted into a pure `resume_plan()` precisely so the bug that bricked it is testable without a network, and reintroducing the original `>` makes `a_full_size_part_file_is_verified_never_resumed` fail. 6 new tests (226 Rust total), `clippy -D warnings` clean.

**Not fixed, and deliberately:** there is still no free-disk-space precheck before starting a 148 MB fetch. A write failure is surfaced and the `.part` is kept for resume, which is the right behaviour; a precheck needs a new crate for a marginal gain.

---

### ✅ One more, held just below the line because it cannot be seen until it happens
*Fixed 2026-07-12.*

**There is no macOS microphone entitlement.** No `.entitlements`, no `Info.plist`, no `NSMicrophoneUsageDescription` anywhere under `src-tauri/`. Notarization *requires* the hardened runtime, and under the hardened runtime `cpal` opening the input device is TCC-killed without that entitlement. This will not reproduce in `tauri dev` and will not reproduce in an ad-hoc-signed pre-release. **The first correctly-signed, notarized macOS build — the one built specifically to hand to a church — is the first one where the microphone is dead.**

**Fixed.** `src-tauri/relay.entitlements` grants `com.apple.security.device.audio-input` (and nothing else — Relay is not sandboxed, and library validation stays on because whisper.cpp is statically linked; we must not weaken the hardening to pretend otherwise). `src-tauri/Info.plist` carries `NSMicrophoneUsageDescription`. Both are wired in `tauri.conf.json` under `bundle.macOS`.

Worth being precise about the failure, because it is not "permission denied": without the usage string macOS does not show a dialog the user declines — **the process is terminated the instant it asks**. And that string *is* the dialog, so it is the only explanation a church ever gets for why this software wants to listen to their service. It is written for them, and it answers the question they are actually asking: the audio is transcribed on this computer and is never sent anywhere. That claim matches PRIVACY.md, and if it ever stops being true, that string must change first.

**Pinned by two tests in `models::config_boots`** — the module that exists precisely for invariants a compile cannot catch. They assert the entitlement is present *and* `<true/>` (present-but-`false` is worse than absent: it reads as deliberate), that the config points at both files, and that the usage string is a real sentence that says where the audio goes.

**A note on the tests, because it is the whole lesson of this fix.** My first version of them passed on a *broken* file. Both plists explain themselves at length, and those comments naturally quote the very keys being asserted on — so a grep of the raw text matched the **prose** and would have happily green-lit an empty `<dict>`. They now strip XML comments first, and that is mutation-verified: emptying the `<dict>` while leaving the comment intact fails the test. A config bug that only appears on the one build you cannot test locally deserves an assertion that cannot pass by accident.

---

## 6. UX Redesign Recommendations

### Credit first, and loudly

The merge was executed properly. Not "Live gained some Planner features" — `ServicePlanner`'s import block contains **zero** fire commands, so Build and Run are separated *by construction*, not by discipline. The transport bar prints whether `→` steps a **SLIDE** or a **VERSE**. `liveCue` separates position from on-air-ness, so a panic key can clear the wall without restarting the plan at cue 1. These are the decisions of someone who has been burned live, and they are right.

### Fix the three lies (see D3). Then:

> **Status, 2026-08-31 — every recommendation in this section is actioned except one, which is
> recorded as deliberate debt.** They are kept in place, unedited, because a scorecard that
> quietly loses what it used to claim cannot be checked — the same rule `docs/audits/` runs on.
> **Read the paragraphs below in the past tense.**
>
> Closed: the first-run mic meter · the mobile bottom nav · the false empty state · the
> `EmptyState` / `Loading` / `ErrorState` trio (`src/lib/ui/`) · `aria-live` beyond one region ·
> the unoperable `role="button"` divs · focus trap and restore on every dialog (`focus.js`) · an
> `<h1>` in the shell and on every view · the `Stage.svelte` contrast · a humanised error layer
> everywhere (`errors.js`, the ONE humaniser).
>
> **Still open, on purpose:** *"delete the dead legacy token set"*. The **contrast failure is
> fixed** — `--text-faint` now aliases `--v-faint` — but the ~150 lines stay, because they are
> global unscoped rules with generic class names that live components may still carry, and
> deleting them safely needs eyes on a running app this machine cannot see. `ROADMAP.md` §4 owns
> it as accepted debt.
>
> Two paragraphs in particular read as present-tense defects and are not:
>
> | Was | Now |
> |---|---|
> | *"The first-run mic meter is dead"* | Fixed, and it turned out to hide a second fault: the wizard cleared its own `micOn` flag **before** awaiting `stopCapture`, so a failed stop printed "off" over a live microphone and the wizard opened a second capture on top of it. Pinned by `firstrunmic.test.js` |
> | *"The mobile bottom nav is broken"* | Fixed. The bottom nav calls `go()`, like the desktop sidebar, and `App.svelte` carries the comment saying why — `active` is a derivation of `$session`, so assigning to it is discarded by the next `setSession()`, which Live fires on every slide |


**Show the operator which kind of match they are being offered (D4).** This is a UX fix, not an AI fix. The data is already in the payload.

**The first-run mic meter is dead.** `FirstRun.svelte:152-156` promises *"a moving bar proves the microphone is actually hearing something"* — and the bar is fed by an `audio://chunk` listener that only registers inside `startCapture()`, which FirstRun never calls (`capture.js:313`). The one step whose entire purpose is proof, proves nothing. It silently falls through to *"You can test this from the Live tab."*

**The mobile bottom nav is broken.** `App.svelte:264` assigns to `active` — which is a *reactive derivation* (`App.svelte:41`) — instead of calling `go()` as the desktop sidebar correctly does at `:138`. So a tab tap is never persisted, and the next `setSession()` from anywhere (Live fires one on every slide) **snaps the operator back to the previous tab**.

**Flash of false empty state on Live.** `Live.svelte:132` awaits `listPlans()` in `onMount` while `:569` renders `{#if !plans.length}` immediately. On every mount, an operator with a full plan library is told *"No service plans yet"* before the query resolves. Same shape at `:554`.

**Empty states: four competing classes, no component.** `.r-empty` (shared), `.empty` (view-scoped — used by **Live**, the most important screen, which opts out of the shared class), `.chan-empty`, `.cat-empty`. And only **two views in the entire app have a loading state at all**.

**Accessibility, concretely and cheaply:**
- Extend `aria-live` beyond the one region: the suggestion feed, `flash()` messages, and errors are all currently silent to a screen reader.
- **Fix the three `role="button"` divs** (`ServicePlanner.svelte:341,428`; `Lyrics.svelte:198`) — they have no `on:keydown`, so they are focusable and **not operable**, which is strictly worse than a plain div.
- Focus trap + restore on all three dialogs (0 of 3 today; only the crash overlay even takes initial focus).
- Add an `<h1>`. There is none anywhere in the shell, and the headings run `h3 → h2` on Live.
- **Port the contrast fix to `Stage.svelte`** (2.25:1 standby text, on a phone, at arm's length, in a lit auditorium).

**Delete the dead legacy token set.** `app.css:13-162` — ~150 lines, `--text-faint` at **2.82:1**, referenced by ~12 classes that no `.svelte` file uses. It's a loaded gun: the next view that reaches for `.data-table` silently inherits failing text.

---

## 7. UI Modernisation Plan

**Do not modernise the visual language. It is already right.** Dark, near-black, amber-as-tally-light is a deliberate and correct choice for a person in an unlit booth behind a congregation. A light-mode "modern SaaS" refresh would actively harm the user. Amethyst-for-rehearsal (amber means ON AIR and is never allowed to lie) is exactly the kind of thinking this product needs more of, not less.

What genuinely needs work:
1. **A trust signal for AI decisions (D4).** Method badge + `matched_text` highlight. Paraphrase must *look* less certain, because it is.
2. **Confidence as a bar for `direct`, and as nothing at all for `semantic`.** `0.92` means nothing to a volunteer, and for a cosine it means nothing to anyone.
3. **One `<EmptyState>` / `<Loading>` / `<ErrorState>` component trio.** Four classes and two loading states is not a design system.
4. **A human error layer everywhere, not just on Live.** `humanError()` (`Live.svelte:266`) is the right idea, applied to one view.

---

## 8. Feature Matrix

| Feature | Verdict | Reasoning |
|---|---|---|
| Scripture detection (direct) | **KEEP** | The core. Works, benchmarked, CI-gated. |
| The method gate (`router.rs:203`) | **KEEP — PROTECT** | The best decision in the product. Do not "fix" it by raising a number. |
| Semantic/paraphrase detection | **IMPROVE** | TF-IDF standing where an embedder belongs. `verses.embedding` exists and has never been written to. |
| Template engine + one renderer | **KEEP** | Best asset. Don't touch. |
| Output channels (HDMI/OBS/kiosk/stage) | **KEEP** | The differentiator vs. Pewbeam. |
| Live (merged run surface) | **KEEP** | The merge worked. |
| Service Planner | **KEEP** | Correctly cannot reach an output. |
| Rehearsal mode | **KEEP** | Gated at the choke point. Correct by construction. |
| In-app model download | **FIX (D5)** | Good design, two hangs and a brick. |
| Auto-updater | **FIX (D2)** | Present and inert. |
| First-run wizard | **FIX** | Ships; its proof step proves nothing. |
| Channels tab | **MERGE** → Settings/Design | A volunteer configures this once, not weekly. Doesn't deserve top-level nav. |
| Templates tab | **KEEP** | Behind "Design", out of the live path. |
| Library (scripture/songs/media/announce/history) | **KEEP** | Real value; ProPresenter parity. |
| ProPresenter import | **KEEP + MARKET** | Excellent adoption wedge — *"bring your existing songs"*. Still under-marketed. |
| `related_scripture` (19 themes) | **SURFACE or DELETE** | Fully built, a registered command, **zero callers**. Either put it in the Intelligence Feed or delete it — dead built code rots. |
| `eval.rs` benchmark | **KEEP + EXTEND** | Genuinely excellent. Extend it to construct a `Semantic` candidate — today it never does, so `no_paraphrase_ever_auto_fires` isn't testing what its name claims (the real guarantee lives in `router.rs:397`). |
| Voice profiles / self-calibration | **SIMPLIFY** | Powerful and now correct (one baseline, no ratchet). Still too many concepts for a volunteer. Expose **one** dial; keep the learning invisible. |
| Crash reporting | **KEEP** | Opt-in, no DSN in OSS builds, drops free text. Exemplary. |
| NDI | **DEFER** | Honestly parked. Leave it parked. |
| **Windows code signing** | **ADD — P0** | See D1. |
| **Tag-derived version + CI assertion** | **ADD — P0** | See D2. |
| **macOS mic entitlement** | ✅ **DONE** | Invisible until the first notarized build — which is exactly why it needed an assertion, not a test run. |
| **Method badge + `matched_text`** | **ADD — P0** | See D4. Cheapest high-value change available. |
| **Service plan → STT `initial_prompt`** | **ADD — P1** | The plan names the passages; the decoder is never told. Cheap accuracy win from data we already have. |
| **Sermon audio corpus (30 min)** | **ADD — P1** | Unblocks WER, unblocks the dormant STT bench, unblocks the entire moat. |
| **UI localisation (yo/sw/ha)** | **ADD — P1** | Relay detects these languages and cannot speak them to its own operator. |
| **e2e test (fire → nav → clear)** | **ADD — P1** | The path that puts a verse on a wall has zero automated coverage. |
| Multi-tenancy / accounts / billing / RBAC | **REMOVE (never build)** | Contradicts the offline-first moat. Not a gap — a decision. |

---

## 9. Information Architecture

**Now (7 tabs):** Live · Channels · Templates · Library · Planner · Settings · Help

The Console/Planner merge is done, so the *live* IA problem is solved. What remains is that **Channels is a once-ever configuration screen occupying a top-level slot in a live tool.**

**Proposed (5):**

```
LIVE          ← the only tab that exists during a service
                (plan cues + AI suggestions + output wall + transport)
PLANNER       ← BUILD a plan. Cannot reach an output. A Tuesday job.
LIBRARY       ← scripture · songs · media · announcements · history
DESIGN        ← templates + the channel↔display assignment that lives in Channels today
SETTINGS      ← audio · speech + model · sensitivity · voice profiles · privacy
HELP          ← already good; now make the written guide match it
```

**During a service the operator never leaves LIVE.** That is the test, and it now passes.

---

## 10. Technical Modernisation

Ranked by value, not fashion.

> **All five are closed as of 2026-08-31, and each is struck through with its original
> wording kept beneath.** Item 2 was marked done long ago; the other four had been finished
> for weeks and were still written in the present tense as outstanding work — in the one
> section a reader opens to decide what to build next. That is the RG-54 / RG-68 class
> (a document asserting a defect that is fixed), in the most expensive possible place.
> Closed findings are inverted, never deleted.

1. ~~**Make the live commands able to fail.**~~ ✅ **Done.** `nav` returns `error::Result<NavResult>` — four distinguishable outcomes, each explained to the operator — and `clear_screens`/`blackout` return `Result` all the way, with the frontend returning a boolean **and** raising `panicError`, because panic controls fire from places that cannot `catch` (DECISIONS §20). The original finding, for the record: 14 commands return no `Result` — `nav`, `clear_screens`, `blackout`, `set_stage_next`. `handle_nav` (`main.rs:588`) has three silent bail-outs and *discards* `fire_manual`'s return `bool` at `:600`. An operator presses **Next**, the wall doesn't change, and there is no error, no toast, no log. This is the same silent-no-op class we just fixed in `move_plan_item`, now living in the live nav path.
2. ~~**Split `main.rs`**~~ ✅ **Done differently, and better.** The stated goal was to lift the fire engine out of the IPC surface *so that it could be tested without a Tauri app handle* — and the split was only ever the means. The engine is now **generic over `tauri::Runtime`**, which achieves exactly that: `e2e.rs` drives the real commands headlessly against a real database. `main.rs` is still 2,922 lines, and that is now a readability complaint rather than a correctness one. **The split was a means; the test was the point, and the test exists.**
3. ~~**Add one e2e test that drives a real service**~~ ✅ **Done, and it grew into the suite.** `src-tauri/src/e2e.rs` drives the real commands against a real in-memory database through the real router and pipeline, with nothing mocked but the window — **38 tests, none ignored** (`cd src-tauri && cargo test e2e::`). The original finding: fire, nav, clear against a headless build; there is no `tests/` dir, no driver, no integration test anywhere.
4. ~~**Introduce a typed error.**~~ ✅ **Done — zero remain.** `src-tauri/src/error.rs` is the ONE typed error across the bridge (`{ kind, message }`), and `src/lib/errors.js` is the ONE humaniser on the other side. Verify: `grep -c 'Result<[^>]*String>' src-tauri/src/main.rs` returns only `error::Result<String>` — a String *value*, never a String *error*. The original finding: 88 × `Result<_, String>` in `main.rs`; the frontend cannot distinguish *not found* from *DB locked* from *disk full*, which is exactly why it renders `String(err)` in monospace.
5. ~~**Normalise the throw-vs-swallow contract in `capture.js`**~~ ✅ **Done, and pinned — which was the harder half.** The groups are stated at the top of `capture.js` **and held there by tests**: `micstop.test.js` exists because `stopCapture` sat in the THROWS group while swallowing, so a failed stop printed "Start listening" over a live microphone. `qa-r5-groups.test.js` covers the rest. **A contract stated in a comment is not a contract** (CLAUDE.md). The original finding: 39 swallow sites; 1 `throw` in all of `src/`; stated in a comment and applied ad hoc, and not applied to the panic path.
6. **Fix the migration ladder.** `SCHEMA_VERSION = 1` and `run_migrations` is **empty** — all schema evolution happens via idempotent `ensure_*` helpers that run on *every* boot, so `user_version` never advances and gates nothing. And `ensure_manual_detection_status` (`db/mod.rs:234-266`) turns FKs off, runs a `BEGIN…COMMIT` batch with **no `ROLLBACK` on failure**, then re-enables FKs *inside the still-open transaction*, where the pragma is a documented no-op — and the `Err` propagates to an `expect()` that panics at startup. Data survives; the app doesn't.
7. **Don't hold the Audio mutex across device init.** `start_capture` (`main.rs:1581-1636`) holds the lock through a blocking `AudioEngine::start`, and the chunk callback emits from the cpal thread. It's the one place the "compute under lock, release, then emit" rule isn't enforced by construction.
8. **Replace TF-IDF with a real embedder — but cost it honestly.** The interface swap is half a day (`SemanticIndex::top_k`, three call sites). The real work is *recalibration* — `SEMANTIC_FLOOR = 0.30` (`main.rs:340`) and the router's `suggest = 0.35` are tuned to a TF-IDF cosine, and a neural cosine's baseline for *unrelated* text sits far higher, so 0.30 would admit everything — plus an offline embedding pipeline to populate `verses.embedding` (the column is already there, waiting). Call it a week.
9. **Do NOT rewrite the stack.** Rust + Tauri + Svelte + SQLite is correct for this product and would be chosen again.

---

## 11. AI Enhancement Strategy

Only where it earns its place.

| Opportunity | Verdict |
|---|---|
| **Show the method + `matched_text` (D4)** | **DO THIS FIRST.** Not an AI feature — an *honesty* feature. The data is already in the payload. Highest value per hour in this document. |
| **Feed the service plan into `initial_prompt`** | **YES, cheap.** The plumbing exists (`stt.rs:143` `set_prompt`, `stt.rs:565` `scripture_bias_prompt`) and is fed only from `VoiceProfile.bias_terms`. The plan knows the sermon's passages (`plan_items`) and **none of it reaches the decoder**. ⚠️ But resolve the contradiction first: `stt.rs:757-761` argues that `initial_prompt` is *prior context, not a vocabulary list*, and that "a dump of 66 book names drags the decoder toward emitting nouns" — while the shipped prompt **is** the 66-book dump (`stt.rs:566`). The sweep that would settle it (`prompt_sweep`) cannot run: there is no audio. |
| **Record 30 minutes of real sermon audio** | **THE UNBLOCK.** It activates the dormant STT bench (`stt.rs:695` — already built, already scores through the real detector, already degrades audio to church conditions), makes WER measurable for the first time in any language, and settles the prompt question above. Everything in the moat is currently an assertion. |
| **African-language STT fine-tunes** | **The stated moat, and still unbuilt** — correctly so, per `LANGUAGES.md:164`: *"Relay ships no fine-tune today, because none has been verified against real sermon audio."* You cannot evaluate a fine-tune without the corpus above. Do the corpus. |
| **Native-speaker review of the 66×3 book aliases** | **Yes, and it's free.** Marked ❌ for all three languages (`LANGUAGES.md:73-75`). This is the actual moat and no native speaker has read it. |
| **Yoruba numerals** | **Yes.** Swahili and Hausa parse in-language; Yoruba does not (subtractive: 16 = *ẹrìndínlógún*). Yoruba is the largest addressable church market in the tier-1 list. |
| **Neural paraphrase embedder** | **Yes, eventually.** Would let paraphrase *earn* the right to auto-fire — which today it is (correctly) forbidden from doing. See §10.8 for the honest cost. |
| **Surface `related_scripture`** | **Yes or delete.** Built, registered, zero callers. |
| **Post-service summary** | **Maybe.** History already stores everything; today's export is a raw markdown dump, not a summary. |
| **AI chat assistant** | **NO.** DECISIONS.md: *"Not a general AI assistant. Scope discipline."* Still right. |

---

## 12. Brand Refresh

Genuinely weak, and cheap to fix.

- **The name.** `README.md` no longer carries the *"Working name — rename freely"* line; `docs/SPEC.md` states the position correctly — *"Relay" was a placeholder product name. It is still undecided.* "Relay" is generic, unsearchable, and already taken across broadcast and networking. Decide **before** the first church installs it, not after.
- **No logo, no tagline, no positioning line** exists anywhere in the repo. The icon set is complete but generic — there is no mark behind it.
- **The in-app header says "Relay Console"** (`App.svelte:133`) — a tab that no longer exists.
- **The positioning is strong and unstated.** Suggested: *"It hears the verse. It puts it on screen. Even when the internet doesn't."* Offline-first and African-language-first are the two things no competitor is saying.
- **Under-marketed wedge:** ProPresenter import. *"Bring your songs, keep your workflow."*
- **There is no landing page, website, or distribution surface of any kind.** GitHub Releases is it.

---

## 13. Enterprise Readiness — NOT APPLICABLE (and that is correct)

| Asked for | Verdict |
|---|---|
| Multi-tenancy | **No.** One church, one machine, no server. |
| RBAC / SSO / audit logs | **No.** There is no login. There is one operator, standing in the room. |
| Compliance (SOC2/HIPAA/gov) | **No.** No data leaves the device. There is nothing to certify. |
| Multi-region / global deploy | **No.** There is no deployment. It's a desktop app. |
| API ecosystem | **Partial, already true.** OBS/kiosk over WebSocket + LAN HTTP. That *is* the integration story, and it's the right one. |
| Internationalisation | **YES — a real gap.** But for the *operator's* language (Yoruba/Swahili/Hausa), not for enterprise localisation. |

**These are not gaps. They are the shape of the product.** A church of 80 people in Ibadan does not need SSO. It needs the verse on the screen when the power comes back.

---

## 14. Legal & Compliance Review

| Item | Status |
|---|---|
| LICENSE (MIT) | ✅ **Fixed 2026-07-12.** It read `Copyright (c) 2026 [Your name / organization]` — an MIT grant with no named licensor, and the one outright legal defect in the repo. It now names one. |
| PRIVACY.md | ✅ **Shipped, and excellent.** Accurate against `telemetry.rs` and `channels.rs`. Crucially, it **discloses the unauthenticated LAN broadcast** (`PRIVACY.md:74-89`) rather than hiding it, and flags the café-wifi media-serving risk. |
| SECURITY.md | ✅ Shipped. Private reporting, 72h SLA, threat model ranked by content leakage first. |
| AI transparency | ✅ `docs/AI_DISCLOSURE.md` — plain-language, states its own weaknesses. Rare. |
| **CONTRIBUTING.md** | ✅ **Shipped.** Leads with the two highest-value contributions, both of which need no code: a native speaker (locales, book aliases, Yorùbá numerals) and sermon audio. |
| **CODE_OF_CONDUCT.md** | ✅ Contributor Covenant 2.1, unmodified — the standard text on purpose, because a code of conduct someone invented themselves is one nobody has read. |
| **Issue / PR templates** | ✅ Shipped. Three issue forms — including a **language** form, because the highest-value contribution to this project needs no code — plus a PR template whose checklist is the project's real rules (no `unwrap()` in a live path, no swallowed errors, no borrowing a tally colour, and *reintroduce the bug and check your test fails*). |
| **CHANGELOG.md** | ✅ Shipped, and written for the **operator**: it is what a volunteer reads in the update banner before deciding whether to restart the app twenty minutes before a service. |
| Bible translation licensing | ✅ **Clean.** KJV only, recorded as `license_type = "public domain"` (`db/verses.rs:173`), bundled via `include_str!`. **There is no import path for any other translation** — no `import_translation` command exists. Zero exposure today. Keep it that way, or licence properly. |
| GDPR / UK GDPR | ✅ Effectively N/A by architecture — no personal data leaves the device. **PRIVACY.md now says so. This is a selling point, and it is finally written down.** |
| Accessibility (WCAG) | ✅ **Largely passes now.** Focus traps + restore on all 5 dialogs; every control operable by keyboard; a real heading structure; every text token at AA (the preacher's phone was at **2.25:1**). Remaining: ~150 lines of dead legacy CSS whose deletion needs eyes on a running app. |

---

## 15. Prioritised Roadmap

### Phase 1 — **Stop lying** *(this week — the only thing that matters)*
1. ~~**D1** — Windows signing + a platform-aware release gate that fails loud~~ ✅ **done** *(code side; still needs a certificate bought)*
2. ~~**D2** — tag-derived version, CI assertion~~ ✅ **done** — still owed: *actually perform an update* from an installed build, once
3. ~~**D3** — panic path tells the truth: await + surface failures; `Esc` guards on the cheatsheet; fix the `B`-while-typing line~~ ✅ **done**
4. ~~**D5** — model download: read timeout, real cancel, clear `running`, delete a bricked `.part`~~ ✅ **done**
5. ~~**macOS mic entitlement** — before the first notarized build, not after a church reports a dead mic~~ ✅ **done**
6. ~~**`LICENSE:3`** — put a name in it~~ ✅ **done**

**Exit criterion: a volunteer installs Relay on Windows *and* macOS, the OS does not warn, the microphone works, they get a verse on a projector, and when we ship a fix next week their machine actually receives it.** Until that is true, nothing else ships.

### Phase 2 — **Be honest about the AI** *(the trust layer)* — ✅ **COMPLETE**
7. ~~**D4** — method badge + `matched_text` + confidence-as-a-bar-for-direct-only~~ ✅
8. ~~First-run mic meter actually moves~~ ✅ *(it was dead — the one step whose purpose was proof proved nothing)*
9. ~~Live's flash-of-false-empty-state; mobile bottom nav; `Stage.svelte` contrast; the inoperable `role="button"` divs~~ ✅
10. ~~Surface or delete `related_scripture`~~ ✅ *surfaced*
11. ~~In-app error humanising beyond Live; kill the raw `String(err)` monospace~~ ✅ *(`lib/errors.js`)*

### Phase 3 — **Make the code survivable** — ✅ **COMPLETE**
12. ~~Live commands return `Result`; `handle_nav` stops silently no-op'ing~~ ✅ *(`NavResult`)*
13. ~~Split `main.rs`; lift the fire engine out of the IPC surface~~ ✅ — **done differently, and better.** The engine is now generic over `tauri::Runtime`, so it can be driven without a window. The line count was never the problem; untestability was.
14. ~~One e2e test: fire → nav → clear~~ ✅ *(`e2e.rs`, 7 tests, against a real DB)*
15. ~~Typed errors; normalise the swallow contract~~ ✅ *(`error.rs`; the contract is written at the top of `capture.js`)*
16. ~~Fix the migration ladder (`ROLLBACK`)~~ ✅ *(it could brick every future boot)*

### Phase 3b — **Accessibility & polish** — ✅ **COMPLETE** *(added since Rev 2)*
17. ~~Focus traps + restore on every dialog; `<h1>` and a real heading order~~ ✅
18. ~~`aria-live` on the suggestion feed, the transport and errors~~ ✅ *(the AI used to announce itself in total silence)*
19. ~~One `EmptyState` / `Loading` / `ErrorState` trio~~ ✅
20. ~~The i18n layer~~ ✅ *(60 lines, no dependency; translation is now a data contribution)*

### Phase 4 — **Win the bet** *(the moat)* — ⛔ **BLOCKED ON A MICROPHONE, NOT ON CODE**
21. **Record 30 minutes of real sermon audio.** *Everything below is blocked on this, and the tooling to use it is already written and dormant.*
22. Native-speaker review of the 66×3 book aliases — free, and it **is** the moat
23. Yorùbá numerals — subtractive/vigesimal; a real parsing problem, and a great first contribution for a Yorùbá speaker (deliberately **not** hand-authored by an AI: a wrong numeral silently shows a *different verse*)
24. Fill `locales/{yo,sw,ha}.json` — the layer is built and the files ship empty, on purpose
25. Service plan → `initial_prompt` *(after `prompt_sweep` settles whether the 66-book dump helps or hurts — which needs audio)*
26. Measure WER. Then, and only then, evaluate a fine-tune.
27. Neural paraphrase embedder + populate `verses.embedding` *(the column exists and has never been written to)*

### Phase 5 — **Grow**
28. Rename + brand + tagline + landing page — *now the weakest column on the scorecard*
29. ProPresenter import as the marketed adoption wedge
30. ~~CONTRIBUTING / CoC / CHANGELOG / issue + PR templates~~ ✅ **complete**
31. NDI (only if a real church asks)

---

## 16. Production Readiness Checklist

Everything that a commit can tick is ticked. The unticked boxes in the first list are the entire remaining distance between this repo and a church, and **none of them is code.**

> **One box was ticked and should not have been** — a signed + notarized macOS build. Corrected 2026-08-31 (RG-73). It is the only wrong tick found in this checklist, and it mattered: it made a *release-blocking* absence look closed on the page a release decision would be read from.

**Blocking a first church:**
- [x] In-app model download *(resumable, checksummed, cancellable — and it can no longer hang or brick)*
- [ ] 🍎 **A signed + notarized macOS build** — *this was ticked and should not have been. The chain is **wired and locally reproducible** (`relay.entitlements`, `Info.plist`, `scripts/sign-local.sh`, pinned by `models::config_boots`), and the release gate refuses a real tag that is not covered — but **no Apple certificate exists**. `gh secret list` holds only the two `TAURI_SIGNING_PRIVATE_KEY*` updater keys; the gate wants six `APPLE_*` secrets and finds none, so every release so far went out on the unsigned pre-release path. **CLAUDE.md rule 17's trap is therefore still ahead of this project, not behind it.***
- [x] **macOS microphone entitlement** *(without it, the first correctly-signed build is the first one that cannot hear the preacher)*
- [ ] 💳 **A Windows code-signing certificate** — *~$10/mo (Azure Trusted Signing). The gate refuses to ship a real tag without it.* **Neither platform has a certificate**, which the line above used to hide by being ticked.
- [x] Auto-updater *(version enforced against the tag in CI and at release)*
- [ ] 👁 **One update actually installed, end to end, on a real machine** — *the path is capable of it; nobody has watched it happen*
- [x] First-run wizard *(and its microphone meter now actually moves — it was dead)*
- [x] PRIVACY.md + SECURITY.md + AI_DISCLOSURE.md
- [x] The panic path cannot report a success it did not achieve
- [x] The transport cannot silently do nothing
- [x] 📖 **An operator guide written for a volunteer** *(rewritten: setup, the Sunday path, and a troubleshooting table for what actually goes wrong)*
- [ ] ⛪ **A real service run end-to-end by someone who is not the author** — *the only test that actually counts*

**Before public release:**
- [x] Method + `matched_text` visible live
- [x] `LICENSE` names a copyright holder
- [x] WCAG: focus traps + restore, operable controls, `<h1>`, every text token at AA *(**and it is now measured rather than asserted** — `tokencontrast.test.js`, RG-74. When it was first measured it was false: `--v-faint` sat at 4.38:1 on `--v-surf2` with five rules using that pairing. Token moved two steps to `#8c8c8c`.)*
- [x] Crash reporting verified opt-in
- [x] Bible translation licensing confirmed (KJV only, no import path)
- [x] Typed errors, an e2e test, and a migration that can be retried
- [ ] Rename decided *(still undecided — `docs/SPEC.md`, not README, is where that is now stated)*
- [x] CONTRIBUTING.md *(leads with the two contributions that need no code)*
- [x] CODE_OF_CONDUCT.md *(Contributor Covenant 2.1, unmodified)*
- [x] CHANGELOG.md *(written for the operator — it is what the update banner shows)*
- [x] `USER_GUIDE.md` reconciled with the code

**The moat — blocked on a microphone, not on a keyboard:**
- [ ] 🎙 **30 minutes of real sermon audio** — *the bench AND the WER scorer are now built, tested, and pointed at nothing. See `bench/README.md`.*
- [ ] Word error rate measured, in any language, for the first time
- [ ] Native-speaker review of the 66×3 book aliases
- [ ] `locales/{yo,sw,ha}.json` filled in *(the layer is built; the files ship empty on purpose)*

---

## 17. Success Metrics

Vanity metrics are wrong for this product. Measure **services survived**, not users acquired.

| Metric | Target | Status today |
|---|---|---|
| **Install → first verse on screen** | **< 10 min, zero terminal** | *Achievable for the first time.* Measure it on a real volunteer. |
| **Time from bug report → church running the fix** | < 7 days | **Currently ∞** — the updater is inert (D2). This is the metric that D2 exists to move. |
| **Services completed without operator panic** (no Emergency Stop, no crash) | > 95% | The real definition of "it works". Unmeasured. |
| **Wrong-verse rate** (auto-fired, then dismissed) | **< 5%** | **Now CI-enforced** on the eval corpus (`eval.rs:283`). Not yet measured on a *live* service. |
| **Detection recall** on a real sermon | > 80% of spoken references caught | 100% on the 50-case corpus — but that corpus is mostly clean hand-written text, not ASR output. Recall on *easy* input. |
| **Yoruba/Swahili/Hausa word error rate** | Baseline it, then beat it | **Never measured, in any language.** The moat is an assertion. Blocked on 30 minutes of audio. |
| **Crash-free sessions** | > 99% | Opt-in telemetry can now tell you. |
| **Churches running a 2nd service** | The only retention metric that matters | One service is a trial. Two is a product. |

---

## Closing

Revision 1 said Relay needed to become **installable**. It did that.

Revision 2 said it needed to become **honest** — an unsigned Windows installer reporting itself as signed, an updater that would silently never update, a toast saying the screens were clear when they were not, a help overlay whose dismiss key wiped the wall, a confidence score meaning a probability for one kind of match and an arbitrary cosine for another, rendered identically. None of those would ever have shown up in a test. All of them would have shown up on a Sunday.

It did that too, and the doing of it turned up more of the same species than the audit had found: a spoken "next" that failed as silently as a keyed one; `Esc` wiping the wall from inside an arrangement picker, not just the cheatsheet; a first-run step whose entire purpose was to *prove* the microphone worked, and which proved nothing; a migration that could brick every future boot before the window was even shown; a wizard that could auto-fire a verse onto a projector while the operator said "testing, testing" into the mic. Several of the tests written to catch these bugs **initially passed on the broken code** — a focus trap whose visibility check reported every element hidden under jsdom, an entitlement test that grepped a comment instead of the config — and were only caught by deliberately reintroducing the bug and checking that the test failed. That habit is worth more than any single fix in this document.

**So Revision 3's finding is short: there is nothing left to fix by typing.**

The engine is good. The gate is excellent — a paraphrase cannot reach a congregation's wall unattended, at any score, and that is enforced structurally rather than hoped for. The design language is right and survived the pressure to dilute it: a hunch was not allowed to borrow a tally colour. The decision log is now twenty-five rules deep and every one of them is a bug that reached, or would have reached, a congregation.

What stands between this repo and a church in Lagos is a **certificate**, a **billing page**, **thirty minutes of a real preacher on tape**, and **people who speak Yorùbá**. The last two are the moat, and the moat is still a claim rather than a number — not because the work is hard, but because nobody has yet held a microphone in front of a sermon and pressed record.

That is a good problem to have. It is the first time in three revisions that the honest answer to *"what is blocking Relay?"* is not a line of code.

---

<!-- ===== was docs/qa/audits/SUPERSEDED.md, merged 2026-09-21, verbatim ===== -->

# Relay — QA Audit, 2026-08-14

**R6 · Independent Auditor.** Six-agent full-scope run (`/qa-audit --full`). **Layer D was
disabled for every agent**, including this one: no `npm run tauri dev`, no driving `:8032` or
`:8031` over a socket, no packaged build.

> **Nobody in this audit clicked anything, saw anything, or heard anything.** Every claim below
> names the instrument that produced it and the command that reproduces it. Where no instrument
> could reach, the finding is **BLOCKED** and appears in §16 as a step in a human test script —
> not as a pass.

This document does not modify `docs/PRODUCT_AUDIT.md`, which belongs to a human.

> ### Standing, as of 2026-08-20 — read before §1 or §20
>
> **Every finding in this audit is closed.** One P0 and eleven P1s were raised; §0 records each
> closure and the test that fails if the defect returns. §1 and §20 still read **NO-GO** because
> **this document does not rewrite its own findings** — an audit that edits its history stops
> being evidence. That is deliberate, not neglect.
>
> **The NO-GO nonetheless stands**, on its second condition rather than its first: *roughly half
> of this product, as a volunteer experiences it, was unreachable by every instrument in this
> run.* **§16 — the human test script — has not been run.** Until it has, no GO is available at
> any test count.

---

## 0. Fix log

Findings are **not** rewritten as they are closed — an audit that edits its own history stops
being evidence. Closures are recorded here, each with the test that now fails if the defect
comes back. **Corrections to this document's own citations go here too, for the same reason.**

### ⚠️ Correction — 2026-08-31: one citation in §16 F8 does not resolve

F8 names two pins. The first, `e2e::two_references_in_one_window_put_one_verse_on_the_wall`,
exists and holds. **The second, `detection::r4_07`, has never existed** — the R4 tests run
`r4_01` … `r4_06` and there is no seventh. The guarantee itself is pinned; only that citation
was wrong, and it was wrong when written rather than made wrong by a later rename.

F8's text is left exactly as it was. Found by `crossrefs.test.js`, which resolves every
`module::item` citation in the repository into the Rust tree — **and which excludes this
directory precisely because these documents may not be edited**, so a dangling citation inside
a frozen audit can only ever be reported here.

### ✅ P0-1 — closed 2026-08-14

**The repair is structural, not a threshold** (CLAUDE.md rule 10 forbids the other kind). A new
`DetectionMethod::UncertainBook` joins `Semantic` and `Ambiguous` on the wrong side of
`may_auto_fire()`, so the cap cannot be undone by a confidence score, by the operator's
sensitivity dial, or by the calibrator drifting. Two routes reach it, and they are deliberately
governed by different rules because they are different facts:

| Route | Example | Rescued by a chapter/verse keyword? |
|---|---|---|
| `fuzzy_book` edit-distance repair | "room two twelve" → Romans | **No, ever.** The repair is a guess about the acoustics, and surrounding grammar does not make the guessed word likelier to be the spoken one |
| Ordinary English word that is also a one-token alias (`ORDINARY_WORD_ALIASES` = `song`, `job`) | "we will sing song two twelve" | **Yes.** The word really was heard; only its meaning was in doubt, and "Song of Solomon chapter two verse twelve" removes the doubt |

`psalm` is deliberately excluded from that list. R6's 118-noun sweep included it and flagged four
**true positives**; "Psalm three sixteen" is how preachers name the book and must keep firing.
That was a fix to the ruler, not to the thing measured.

**The cap did nothing until P1-10 was built. Recorded because the sequence is the lesson.**
The `UncertainBook` variant, its unit tests, `r6_11`/`r6_12` and the whole router-level story
above all went green while the product was **still putting Numbers 3:16 on the wall** — because
`emit_detections` built its candidate with a hardcoded `method: DetectionMethod::Direct`,
discarding the parser's verdict before the router ever saw it. The fix was declared closed on
that evidence and it was not closed. What caught it was the very next item in §17: the first
e2e test of the auto-fire path, `e2e::ordinary_church_announcements_reach_nobody`, written
minutes later. R6's argument for ordering P1-10 second — *"it is the instrument that would have
caught P0-1"* — was correct in a stronger sense than intended: it caught the incomplete **fix**
for P0-1.

**Three instrument bugs found while closing it, all the same shape — a hardcoded
`DetectionMethod::Direct` standing in for the real one — and between them they made the P0
invisible AND its first repair inert:**

| Site | Effect |
|---|---|
| `main.rs`'s `emit_detections` candidate | **Production.** Every parsed candidate was relabelled as heard before routing, so no method-based cap could ever apply |
| `eval.rs:79` | The CI benchmark routed as though everything was heard, so it could not measure the cap either |
| `detection.rs`'s test harness | Same, in the unit tests |


- `eval.rs:79` and `detection.rs`'s harness both passed a hard-coded `DetectionMethod::Direct`
  to `router.decide` instead of `m.method`. The benchmark therefore routed every candidate as
  though the parser had heard it exactly — so it could not have measured the new cap either.
  A gate that assumes the answer is not a gate.
- The corpus had the Yorùbá twin of this trap (`neg-yo-everyday-song`) and not the English one.
  Five English negatives and **two controls** are now in `data/eval_corpus.json`: the controls
  (`pos-en-psalm-singular`, `pos-en-song-with-keyword`) exist so that a future over-correction
  fails the build too. A safety fix that breaks them has overshot.

**Verified:**

```
cd src-tauri
cargo test e2e                                         # 18/18 — incl. 4 new auto-fire-path tests
cargo test r6_                                         # 14/14 — every P0 reproduction green
cargo test eval::tests::print_scorecard -- --nocapture # 57 cases, 100% recall, 0 wrong verses
cargo test                                             # 469 pass; fmt + clippy -D warnings clean
npx vitest run src/lib/detect.test.js                  # 12, incl. 4 new on the frontend half
```

Mutation-tested rather than assumed: reverting the one-line cap makes
`eval::tests::negative_cases_put_nothing_on_the_screen` and `wrong_verse_rate_beats_the_spec_target`
fail with `[neg-en-hymn-number] auto-fired ["Numbers 3:16"] — a wrong verse would have gone on
the wall`.

The frontend half is closed too: `uncertain_book` gets its own `methodKey`, so it can never
describe itself as a reference Relay heard, and `showsConfidence` is false at every score. Its
number is a *genuine* parse confidence about the wrong question, which makes it the most
dangerous value in the product to print.

### ✅ P1-10 — closed 2026-08-14

`emit_detections` and `confirm_detection` are now generic over `R: tauri::Runtime`, so the two
paths where **the AI decides** obey architecture rule 24 like every human path already did. Four
tests in `e2e.rs` drive the auto-fire path end to end for the first time:

| Test | Claim |
|---|---|
| `a_spoken_reference_auto_fires_all_the_way_to_the_wall` | The positive control. Without it the three below could pass by the path being broken outright — the failure mode a suppression test cannot distinguish from success |
| `ordinary_church_announcements_reach_nobody` | The P0's seven phrasings, asserted at the wall rather than at the router. **This is the test that caught the incomplete fix** |
| `a_paraphrase_never_auto_fires_through_the_real_transcript_path` | The cap survives everything between `decide` and the projector, not just `decide` |
| `nothing_the_ai_decides_escapes_a_rehearsal` | Rehearsal contains the **machine**, on both doors. Every previous rehearsal test drove a human path |

### ✅ P1-11 — closed 2026-08-14

One line in `vitest.config.js`: `resolve: { conditions: ['browser'] }`. Svelte 4 maps its `.`
export to `src/runtime/ssr.js` under every condition except `browser`, and `environment: 'jsdom'`
does not imply it — so `onMount`, `beforeUpdate` and `afterUpdate` were literal empty functions
in every frontend test this repo has ever had.

**Re-running the layer-B findings, which was the other half of the item:**

| | Before | After |
|---|---|---|
| Frontend tests | 551 pass · **8 skipped** · 10 fail | **559 pass** · 0 skipped · 10 fail |
| Tests newly executing | — | R3's 8 lifecycle-gated assertions, all green |
| `npm run build` | — | unaffected; `vite.config.js` is a separate config |

Nothing was re-graded down: every layer-B finding in this document survives with a live
lifecycle. Three characterisation tests were inverted rather than deleted, because each was
written about an assumption nobody had stated and is now the guard against it returning:

- `r6-lifecycle-probe.test.js` — rewritten to assert real bodies and a full
  `['mount','afterUpdate','destroy']` ordering. Its original closing comment predicted exactly
  this: *"If this ever reads ['mount','afterUpdate','destroy'], the config was fixed."*
- `r2livepath.test.js`'s R2-G block — its deepest test now asserts that mounting the run column
  **does** reach `list_output_channels`. That is the one that fails first if the config line is
  removed, and it fails in terms of the application rather than the runtime.
- `surface.test.js` — the `LIFECYCLE_LIVE` gate is **kept**, deliberately. It reads the runtime
  rather than trusting the config, so removing the line makes those tests skip loudly instead of
  passing vacuously.

`__vitest.browsercond.mjs`, R3's scratch config, is deleted.

### ✅ P1-3 + P1-4 — closed 2026-08-14 (Escape, both directions)

**P1-3, seven doors.** `shortcuts.js` probed for `[role="dialog"]` alone. The DOM-probe design
is deliberate — *"this must not depend on anybody remembering"* — but the bug was never "this
one role was missed", it was **"the list was a list"**. The guard now recognises the whole
overlay class: `dialog`, `alertdialog`, `menu`, `listbox`.

Two layers, because a panic key deserves two:

1. **The guard** — an overlay that declares a recognised role cannot clear the wall, even if it
   forgets everything else.
2. **The menus** — each now consumes Escape itself, so the operator gets the outcome they asked
   for rather than merely the absence of the one they did not. Four already carried
   `role="menu"`; the two that carried nothing were the Countdown picker **on the run rail** and
   the VerseDeck kebab — the two used during a service.

**P1-4 is the mirror image.** `Announcements.svelte`'s editor claimed `role="dialog"` with no
scrim, no `aria-modal`, no focus trap and nothing bound to Escape. Because the guard *reads* that
role, a wrong one does not mislabel a box — it **disarms the panic key**, and `Esc` is the only
panic key that survives a focused text field, in a panel that is nothing but text fields. Now
`role="group"`.

Held by `shortcuts.test.js` (table-driven, one row per overlay kind) and by a static contract in
`r6-contracts.test.js`: **every floating menu must declare a role the probe can see.** A popup
with no role is invisible to the guard, and that is how both of these were built.

### ✅ P1-1 — closed 2026-08-14 (the blackout that missed a screen)

`Stage.svelte` handled `content`, `clear` and `stage_next` and **not `black`** — so `B` blacked
the congregation's wall and left the verse on the screen the preacher reads from, while the
console correctly reported success, because the message *had* left the machine. The
`stage_next` leak in mirror image: same twin, same door, opposite direction.

The ambiguity R2 raised is real and is now written into the code rather than left implicit: a
stage monitor faces the *preacher*, so one could argue a blackout should leave it alone. The
conservative reading wins — **the harsher control must never do less than the milder one**, and
`clear` already blanks this page. An operator who has just hit the emergency key cannot be asked
to remember it reaches three screens out of four. If Relay ever decides the stage monitor should
survive a panic, it must survive *both* controls, deliberately, in both branches.

R6-3 was rewritten rather than merely satisfied. It now **derives the kind list from the Rust**
and requires an explicit per-client verdict for each — `false` with a reason is a fine answer,
silence is not, because silence is indistinguishable from an oversight. Hardcoding a list of two
is what made the original miss possible. Mutation-tested: removing the `black` branch fails it.

### ✅ P1-6 — closed 2026-08-14 (amber that lied, on five paths and one event)

`leavePlan()` had three callers against nine paths that put something on a congregation screen.
Fixed in three parts, because the finding had three:

1. **The five wrappers.** `fireContent`, `fireMedia` and `startCountdown` are *also* the plan's
   own take path, so they take `manualFire`'s existing `keepPlan` flag rather than an
   unconditional `leavePlan()` — `Live.svelte::fireSlide` passes `true`, everything else gets
   the safe default. `pushAnnouncement` never opts out: the emergency notice is never a plan
   cue and covers every screen. `navVerse` clears only on a `fired` outcome — `NavResult` exists
   precisely because `EndOfPassage` and `NotInLibrary` leave the screens exactly as they were,
   and taking the plan off air because a key did nothing would be a new lie for an old one.
2. **The half no wrapper can reach.** `/api/clear` from the preacher's phone, the spoken "clear
   the screen", and the exit from a rehearsal all reach `channels::clear` directly, so the
   `output://clear` / `output://black` listeners are the console's only report of them. They set
   `live` and `screenBlack` and nothing else. They now `leavePlan()` too.
3. **The transport mode, which was the dangerous half.** `mode` read `$live` — "content is
   armed" — where it meant "a congregation is looking at it", the identical confusion the run
   rail's amber badge had. So `Esc` gave SLIDE mode and `B` gave VERSE mode from the same
   conceptual state, and after a blackout mid-plan the next `→` fired a verse from an earlier
   passage **and cancelled the blackout** — the emergency key undone by the key an operator
   presses most. Adding `!$screenBlack` makes the derivation match the sentence already written
   above it.

**The test is the point.** `transport.test.js` pinned exactly the four wrappers that behaved and
enumerated none of their twins — which is how five more shipped. It now asserts the rule over
the **whole set, derived from the source**: any exported wrapper that calls a screen-changing
command must either `leavePlan()`, accept `keepPlan`, or route through `panicRun`. Add a tenth
path and the test names it until somebody decides. Mutation-tested by removing `fireMedia`'s
clause: `expected [ 'fireMedia' ] to deeply equal []`.

This was the third eight-of-nine in this repo's history — rehearsal gated three of four kiosk
publishers, throw-vs-swallow held for eight of nine wrappers, `NavResult` was honoured by the
console and discarded by the remote. The contract is now written where the count was.

### ✅ P1-7 — closed 2026-08-14 (`confirm_detection`'s two silent successes)

Both paths now return a typed `not_found` instead of `Ok(thresholds)`: the parse that finds
nothing, and — the one that was genuinely invisible — `fire_manual` returning `false`, whose
bool was **discarded with no binding and no `if`**.

The second is reachable in ordinary use, which is why it is a P1 and not a tidy-up.
`emit_detections` deliberately demotes a parsed-but-absent verse to a suggestion rather than
dropping it ("heard-but-unresolvable must degrade to a suggestion, never to silence") and emits
it with `in_library: false` — and **no frontend file reads `in_library`**. So a garbled
"Psalms 23:99" rendered as an ordinary card with Accept enabled, the backend answered `Ok`,
`capture.js` ran `leavePlan()` and removed the card, and `Live.svelte` flashed *"Now live:
Psalms 23:99"* while the previous verse was still on the wall. Exactly the bug the comment above
`acceptTop` says was fixed: the caller was hardened, the callee was not.

It also fed the calibrator — `record_feedback(true, …)` ran on the `Ok` path whether or not
anything had fired.

The refusal reuses `manual_fire`'s sentence verbatim. It is the same failure with the same
engine underneath, and a volunteer should not have to learn two sentences for one problem
depending on which control they pressed.

Pinned by `e2e::accepting_a_suggestion_that_cannot_fire_says_so` — three cases plus a positive
control, asserting both the error and that **the previous verse is untouched**, since the flash
was over a wall that had never changed. Only possible because P1-10 made the command generic.
Mutation-tested by restoring the discarded bool.

### ✅ P1-8 — closed 2026-08-14 (three documents promising a guarantee the code broke)

**Documentation-first, as R5 recommended: the code is unchanged.** What changed is that the
product now says what it does.

- **`docs/DECISIONS.md` §35** — written. It records that the HTTP API is an *unauthenticated
  control plane on the local network, deliberately*, and why: a password on a device shared
  between a preacher, a tech volunteer and a stand-in every Sunday is a password on a sticky
  note behind the desk. It also separates the two guarantees the old text conflated — the
  **WebSocket hub really is still broadcast-only**; its sibling HTTP port never was.
- **`PRIVACY.md`** — said people on your network "**cannot** push content to your screens".
  Now says they can, names the URL, and carries a dated note saying the file previously said
  the opposite, in case a church made a network decision on the strength of that sentence.
  A privacy document that undersells the exposure is worse than one that says nothing.
- **`SECURITY.md`** — the report-this list now tells researchers not to spend their time on
  the unauthenticated control plane, and says what *is* wanted on that surface: a route that
  reaches something other than the outputs, a path escaping `media_dir()`, a way to read
  transcripts, or a way in from outside the LAN.

**The phantom citation is gone.** `channels.rs`, `main.rs` and this file's own cross-reference
all cited "DECISIONS §47"; the document's sections end at §34 and `47` was a *line number*.
All three now point at §35.

§35 records the drive-by (R5-6) as the widest reading of the same decision, and names the one
cheap change that would close it without touching the product: **require `POST` on the mutating
routes and drop the wildcard CORS header from them.** That removes the
`<img src=".../api/black">` vector entirely while the preacher's phone keeps working. It is
flagged as "if we do only one thing, do this" rather than done, because it is a code change to
a shipped protocol and belongs with the P2 cluster, not in a documentation fix.

The section closes with the rule this cost: **cite a section number that exists, and when a
feature crosses a line an existing decision drew, the new decision is part of the feature — not
follow-up work.**

### ✅ P1-5 — closed 2026-08-14 (`Space` on a VerseDeck list row)

**The obvious fix was wrong and the tests said so.** Adding `preventDefault` + `stopPropagation`
so the row fires and the transport does not closes the double-action and leaves the two layouts
of the same deck still disagreeing — which is the actual finding. Rule 11 is *"`Space` means
advance, app-wide, and nothing else"*, and the GRID card already obeys it: it is a native
`<button>`, and `shortcuts.js` calls `preventDefault` on Space globally, so Space advances and
does not activate.

So the row now answers **Enter** and lets **Space** fall through to the transport, exactly as
the grid card does. The ARIA authoring practices want a `role="button"` to answer both; this app
deliberately overrides Space everywhere including native buttons, and an operator who has
learned "Space advances" must not meet an exception on a live surface. Both layouts are pinned,
plus a new test that Enter acts on the row and does *not* also advance.

### ✅ P1-9 — closed 2026-08-14 (imported templates, and the missing CSP)

**Validated at the import boundary**, as R5 recommended, not in the renderer: `TemplateRender`
has five call sites and will grow a sixth, and it runs on the hot fire path where this repo
already has a hard rule about template-JSON cost. `parseImportedTemplate` checked the *shape* —
a marker, a layout object, a style object — and not one value inside either. It now walks every
value, arrays included, and strips anything that reaches the network or escapes its own
declaration.

Two things worth recording because both were mistakes caught by tests rather than by review:

- **The first version skipped arrays**, which meant it sanitised nothing that mattered — a
  template's values live in `layout.layers[]`, so the one shape a hostile file actually uses was
  the one shape that passed through untouched.
- **The second version deleted every embedded background.** A `data:` URI legitimately contains
  a semicolon (`image/png;base64,…`), so the blunt "no semicolons" guard ran before the
  allow-list and ate the most common thing a shared template carries. The control test —
  *"keeps the values a template legitimately needs"* — exists precisely for this, and a safety
  fix that breaks ordinary templates would have shipped looking like a success.

**The CSP half was R6's addition to R5's finding, and it was the larger hole.** The packaged app
has a policy in `tauri.conf.json`; the `:8032` server that hands `output.html` to OBS, kiosk
screens and phones sent **none**. Those are ordinary browsers rendering a look assembled from
template JSON that may have arrived in an email — the half of the audience that needed it most.
`KIOSK_CSP` is deliberately *tighter* than the packaged policy in the one way that matters: no
`http:` in `img-src`/`media-src`. The desktop app allows it for operator-chosen local sources; a
LAN page has no such need, and Relay renders offline or it does not render. Pinned by
`qa::kiosk_headers`, which asserts the header is sent and that those two directives never regain
the network.

**Still open from §17:** P1-2 — a product decision, see below — then the P2 clusters.

### ✅ P1-2 — decided and closed 2026-08-15: **remove the preview half.**

**Both suites are now green: Rust 474/474, frontend 567/567.**

The question was posed as "stage or fire", and the code answered it. `Library.svelte` line 78
read *"PREVIEW holds AI SUGGESTIONS ONLY. Browsing FIRES"*, and `stage(d)` took a **detection**
whose `_fire` was `confirmDetection`. So the intended producer was never the content panes — it
was the **Heard** panel, and that panel calls `confirmDetection` directly. The real question was
therefore *"should accepting an AI suggestion fire immediately, or stage for review?"*

**Decision: it stays one press, and the preview half is deleted rather than wired.** Four
reasons, in order of weight:

1. **`Live.svelte` already implements Preview ≠ Programme, for real, on the plan path.** A second
   staging model on the most time-critical surface is a second mental model — and two
   implementations of one safety distinction is the exact shape that produced most of this
   audit's findings.
2. **The queue already is the switcher.** "Up Next" is a staging area that holds N items instead
   of one, it is reachable, and `goLive()` already fires from it. Staging was a one-item
   duplicate of a mechanism that ships.
3. **The card already carries the decision information** — reference, text, method, evidence.
   What a Preview monitor adds is how it will *look*, which is a Templates-tab question, not a
   Sunday one.
4. **"The least possible effort from the operator"** is the product's own sentence, and an extra
   press on the AI path costs it exactly when the preacher has moved on.

The trackpad-slip risk the preview pane was drawn to answer is real, and it belongs to the
suggestion **feed**, not to a monitor: the mitigation is that a live, reordering list must not
move under the pointer. That is a separate and much cheaper fix, and it is recorded as open.

Removed: `stage()`, `staged`, `take()`, `taking`, the `select` stub, the `preview`/`onTake`/`busy`
props, the preview render branch, and the permanently-disabled "Take to screen →" button.
`Go Live` now fires the top of the queue and is disabled when the queue is empty. Seventeen tests
over the unreachable prop are gone; seven remain on what the component actually does.

**The audit's own instruction is worth quoting, because it was aimed at this session:** *"decide
what Preview is in the Library, and either wire it or remove the whole column. Do not fix the
badge again."* The badge had been fixed here, against a prop nothing could supply.

### ⏸ Superseded — the original framing of P1-2

`stage()` in `Library.svelte` has zero callers, so `preview` is permanently null and the run
column's whole Preview/Take half is unreachable. R6's instruction was *"decide what Preview is
in the Library, and either wire it or remove the whole column. **Do not fix the badge again.**"*
That instruction is aimed at this session: the badge was fixed here, against a prop nothing can
supply, and 17 tests were written to hold it.

The choice is about how operators work, not about code:

| | If the Library **stages** | If the Library **fires** |
|---|---|---|
| Clicking a verse | fills Preview; a second, deliberate press puts it up | goes straight to the wall |
| Matches | the switcher model every video desk uses, and `PreviewProgram.svelte`'s deleted rationale | what `LiveOutputRail`'s single time-multiplexed pane and its always-disabled Take button imply today |
| Work | wire `stage()` to `onSelect`, keep the 17 tests | delete the preview half of the run column and those tests |

Recommendation: **stage.** The safety model already names Preview ≠ Programme as one of the six
distinctions, `Live.svelte` implements it for real, and the Library is where content is chosen
mid-service — the dangerous half. But this is the one finding in the audit where the right answer
depends on how a specific church's operators actually run a Sunday.

---

### ✅ The error-handling P2 cluster — closed 2026-08-14 (R6-5, R3-07, R3-08)

R6 grouped these as sharing one root: *"the error was captured and then thrown away or shown
raw."* Three of the four are closed; the fourth (R3-04) is below.

- **R6-5 — six sites rendering a raw rejection**, two of them in monospace, and worse than the
  audit's headline suggests: because `error.rs` sends a typed **object**, `String(e)` produced
  the literal text **`[object Object]`**. `humanError` already handled typed errors correctly —
  the sites simply never called it. All six now do, and the test was generalised from "these two
  files stringify" to "no view stringifies", so the next one is caught by class.
- **R3-07 — History captured the reason and rendered "empty"**. `detail.error` was set and
  referenced nowhere in the template, so a service whose detail query failed was reported as
  *"No transcript recorded"* and *"No verses fired"* — telling an operator their Sunday was never
  captured, with the reason one property away. That is the kind of wrong that gets acted on. It
  now renders *"Could not open this service — …"*, announced. The export line lost its `r-mono`
  too: monospace is what made the old raw dumps read like a crash.
- **R3-08 — seven error surfaces nobody could hear.** All seven now carry `role="alert"`. The
  worst was `lo-err` on the **run rail** — the most dangerous surface in the app, whose line
  saying a Take or a Countdown *failed* was silent to a screen-reader operator.

### ✅ R5-4, R5-8, R5-3 — closed 2026-08-14

Three P2s that each turned out to be about a *rule* rather than a line.

- **R5-4 — the sensitivity dial reported a position it did not reach.** `catch { return
  sensitivity; }` handed back the value the operator asked for, as if it had landed, under a doc
  comment promising "the landed dial position". Now GROUP 1 (throws), and `Live.svelte` re-reads
  the real position from the backend and says *"Sensitivity stayed at 50 — …"*. This is the
  **third** wrapper repaired for the rule behind DECISIONS §20, after `clearScreens` and
  `stopCapture`, and it sits on the one control governing what the AI may put on a wall without
  asking.
- **R5-8 — `setStageNext` swallowed a failed clear**, and the fix is a correction to the GROUP 2
  *rule*, not just to the wrapper. GROUP 2's test is *"can the congregation see the
  difference?"*; here the honest answer is **"no, but the preacher can, and he is the one acting
  on it"** — the stage monitor is a real screen on a stand in front of a person, and
  `setStageNext(null, null)` is how its "up next" panel comes down. Moved to GROUP 1 so each call
  site states its choice: the push after a fire shrugs deliberately and says why; the clear
  surfaces.
- **R5-3 — `Esc` during boot left a cue marked ON AIR across a process death.** The repair is not
  "clear it in the third exit too": **a flag that must die on every exit belongs at the exit.**
  All three routes run through `finish()`, so it is cleared there, and a fourth exit added later
  inherits the guarantee instead of needing to remember it.

**R2-J is NOT closed by R5-3**, and R2 predicted that when it filed it. `liveCue` is what the
amber badge is drawn from; `Live.svelte` copies `saved.liveOnAir` into it at mount; and the shell
renders *outside* the boot guards, so the copy is taken before the gate is answered. Clearing the
session on every exit still leaves the store holding `onAir: true`. Two facts, one owner needed —
its test now points at `finish()` and still asserts the gap.

### ✅ R5-5, R2-A, R2-B — closed 2026-08-15. **The Rust suite is green: 474 / 0 failed.**

- **R5-5 — `telemetry::scrub` was documented as an allow-list and implemented as a blocklist.**
  It enumerated the carriers it emptied and shipped every other field of `sentry::protocol::Event`
  verbatim, so `logentry`, `tags` and `threads[].stacktrace.frames[].vars` survived — the *same*
  carrier the exception path was careful to clear, on the other stacktrace field. Rewritten as
  what its own doc comment describes: **destructure the event and build a fresh one from the
  allowed fields.** `..Default::default()` is what makes it hold — a field added by a future SDK
  version arrives empty rather than arriving populated. `Frame.abs_path` (the build machine's
  source paths) goes too. Not a live leak when found, which was precisely the argument for
  closing it: one `set_tag` away, in a module whose promise in PRIVACY.md is unconditional.
- **R2-A / R2-B — `/api/live` answered from the wrong source of truth.** It read
  `Context::current()` — the passage anchor, which deliberately survives a clear because that is
  what makes `→` resume rather than restart — and published it as `live`. So the preacher's phone
  named a verse over cleared screens, over blacked-out ones, and during a rehearsal, byte-identically
  to a real fire. Containment held on all four *push* doors; the HTTP control plane is a **pull**,
  which is exactly why every enumeration missed it.

  The repair adds `channels::WallState`, maintained at the three choke points that change what a
  congregation sees — `broadcast_content` (after the rehearsal early-return, so it records the
  congregation wall and not the sandbox), `clear`, and `black` — and nowhere else. The anchor
  still rides, under the honest name **`cued`**, because it is genuinely useful to the remote:
  it is what Next/Prev will step. The answer also carries `rehearsing` and `blacked`, so a blank
  phone can say *why* it is blank.

**And the fixture drift the audit warned about happened inside the audit.** `r6.rs` hand-rolled
its own copy of `qa::bare_app()` rather than calling it — written independently, and by the next
day already out of date: it did not manage `WallState`, so `/api/live` answered "clear" for every
test built on it. `first_launch()` is now one line delegating to the shared fixture. One fixture,
or two fixtures that disagree.

### ✅ R3-04 — closed 2026-08-15 (the structural one)

**GROUP 2's written rationale was the bug.** *"A list that fails to load costs the operator
nothing they cannot see for themselves — the list is visibly empty."* A fresh install ships
**five** built-in templates, so *"No templates yet — create one to start"* was never something
the Templates tab could truthfully say about an empty list; it could only ever mean the read had
failed. An operator told their five templates do not exist is about to make five more.

The wrappers still return `[]` — that is what keeps every caller working and what stops a broken
read taking a view down. What changed is that **the reason is no longer discarded**: `readErrors`
records it, keyed by wrapper name, exactly as `panicError` records a failed panic control and for
the same reason — the caller is not in a position to act on it, and the person who is looks at a
screen. Cleared on the next successful read of the same key, so a transient failure does not
leave a permanent banner.

**All 22 reads are routed, not the ones that mattered.** `loadRehearsal` and `loadServiceTarget`
feed a boolean and a number rather than a list, so neither was part of the finding; they are
routed anyway, because a rule covering 20 of 22 is the exact shape this repo keeps shipping bugs
in, and *"which reads are guarded?"* should not be a question anyone has to look up. Pinned by a
contract test that derives the wrapper list from the source: a new `list*`/`load*`/`search*` with
a bare `catch { return [] }` fails it by name.

Two things the mechanical conversion nearly cost, both caught before they landed:

- `guardedRead` takes an `onFail` callback because **two of these reads mirror into a store and
  their old catch blocks reset it**. A fallback *value* cannot carry a side effect, and dropping
  the reset would have left a failed read displaying the last good data — a quieter version of
  the very bug being fixed.
- The first transform's regex spanned a function boundary and silently merged four wrappers.
  Caught because the module stopped parsing; worth recording because a subtler version would not
  have.

`TemplateGallery` is converted as the reference view — three facts, not two. The remaining views
still show two; each is a small edit against a mechanism that now exists, and the empty sentence
survives for the case where it is actually true (a filter matching nothing). Every read wrapper in
`capture.js` is GROUP 2 and returns `[]` on failure, so **no list in Relay can distinguish "the
query failed" from "there is nothing here"** — the Templates tab prints *"No templates yet"* on an
install that ships five built-ins. Fixing it means changing what GROUP 2 *means*, which is a
deliberate contract with a written rationale, so it wants its own pass rather than being tacked
onto this one.

---

## 1. Executive summary and score

### Release decision: **NO-GO**

> Relay's engine is genuinely good, and it currently puts the wrong Bible verse on the
> congregation's wall, by itself, when somebody says "please turn to hymn number three sixteen".

That is one open **P0** and eleven open **P1**s. The rule is not negotiable: never GO with an
open P0 or P1.

### Score: **46 / 100 — on the part of the product an instrument could reach**

| What the score is made of | Weight | Verdict |
|---|---|---|
| Core engine correctness (detection, router, pipeline, DB, locks) | 30 | **17** — the cap holds, locks are clean, migrations are retryable; the auto-fire *decision* is wrong on ordinary speech |
| Live-safety distinctions (the six) | 25 | **9** — five are pinned; **Preview ≠ Programme does not exist**; Cued ≠ On Air is violated on three surfaces |
| Panic controls | 15 | **8** — honest where they were checked, and Blackout silently misses one screen while Escape *causes* a clear from seven overlays |
| CRUD reachability from a fresh install | 10 | **6** — 14 of 18 tables reachable; three New-Item entries and the whole arrangement write side are not |
| Error legibility & accessibility | 10 | **3** — six raw-error sites, seven unannounced error surfaces, no heading structure in six views |
| Offline-first | 5 | **4** — genuinely honoured; one imported-template hole |
| Security & data honesty | 5 | **2** — the shipped docs describe a threat model the code no longer has |

### The half of the product no instrument here could reach

State this plainly, because the score above is **not** a score for Relay:

- **Audio in — 0% covered.** Whisper, the VAD, the gain gate, the chunker, WER in any language.
  Every agent scored detection over *text*. The product's input half was never exercised.
- **Pixels out — 0% covered.** `TemplateRender` was read, never rendered to a screen anyone saw.
  Legibility at booth distance, colour under a projector, multi-monitor, DPI: all unmeasured.
- **Every mount-time code path — 0% covered.** See P1-11: `onMount`, `beforeUpdate` and
  `afterUpdate` are silent no-ops in all 557 frontend tests.
- **The packaged build — 0% covered.** The CSP only applies to bundled assets. The macOS
  microphone entitlement trap only bites a signed build. The updater was never run. Windows was
  never run.
- **All hardware — 0% covered.** OBS, ATEM, ProPresenter, NDI, a projector, a second monitor, a
  phone on the church wifi.

**Roughly half of Relay, as a volunteer experiences it, is unscored — not passed.** The honest
output for that half is §16.

---

## 2. Coverage — what each instrument actually saw

| Layer | Used by | What it proved | Reproduce |
|---|---|---|---|
| **A** Command E2E | R1, R2, R4, R5, R6 | Real commands, real in-memory DB, real router and pipeline, assertions on events that left the machine | `cd src-tauri && cargo test` |
| **B** Component mount | R2, R3, R5, R6 | A component renders and its handlers reach a command **that exists**. Nothing about mount. | `npx vitest run` |
| **C** Static contract | all six | Relationships between files — the only instrument that reaches a guarantee's *twin* | `npx vitest run src/lib/r6-contracts.test.js` |
| **D** Live app | **nobody — disabled** | — | — |
| **E** Human | nobody — impossible here | — | §16 |

### Suite state, after the audit

| Suite | Pass | Fail | Ignored |
|---|---|---|---|
| Rust (`cargo test`) | 462 | **6** | 26 (incl. R2's 4 and R4's 10, red on purpose) |
| Frontend (`npm test`) | 547 | **10** | 8 |

**CI is red.** `.github/workflows/ci.yml` runs `cargo test --all-targets` (macOS *and* Windows)
and `npm test`. Sixteen tests are red on purpose — they are findings, deliberately left in the
tree — so **no PR can merge until a human triages them**. That is intentional and it is stated
here so nobody mistakes it for breakage.

### The cap on everything at layer B

Four agents independently found it; I confirmed it a fifth time and sharpened it. See **P1-11**.
Its effect on this audit's confidence:

- Every layer-B **PASS** in this run means "the component renders and its click handlers are
  wired", never "the screen works".
- Every layer-B claim about *loading*, *initial state*, *subscriptions*, *focus on open* or
  *scroll on update* is **vacuous**, including the ones phrased confidently.
- Because `onDestroy` **is** real while `onMount` is not, a teardown assertion can pass over a
  setup that never happened. That asymmetry is new in this audit and is why I do not treat the
  cap as merely "some tests are weaker".

---

## 3. Bug summary by severity

| Severity | Count | Meaning here |
|---|---|---|
| **P0** | **1** | Wrong content reaches a congregation |
| **P1** | **11** | A core Sunday workflow is broken or dangerous, or a safety distinction is not legible |
| **P2** | 23 | Important functionality broken; a workaround exists |
| **P3** | 18 | Real, bounded |
| **P4** | 8 | Nits, latent hazards, documentation debt |
| **BLOCKED** | 9 areas | No instrument in this run could reach them — §16 |

**Total filed: 61**, plus nine BLOCKED areas.

Merged across six agents. Where two agents found the same defect with different instruments I
count it **once** and say so — independent confirmation raises confidence, not the count.

---

## 4. Critical findings

### P0-1 · Ordinary church speech puts the wrong verse on the wall, by itself

| | |
|---|---|
| **Severity** | **P0** — wrong content reaches a congregation |
| **Category** | correctness / live-safety |
| **Layer** | **A** — scored through the real `Router`, never by reading the transcript |
| **Surface** | `src-tauri/src/detection.rs:772` (`NEVER_FUZZY`), `:803` (`fuzzy_book`), `:365` (`split_run_into_chapter_verse`), `:424–515` (the typing-abbreviation table) |
| **Frequency** | **always**, for the phrasings below |
| **Evidence** | `cd src-tauri && cargo test r6_11 r6_12 -- --nocapture` · `cargo test r6_13 r6_14 -- --nocapture` · `cargo test detection::r4_audit::r4_01 -- --ignored` |

**Precondition.** A fresh install. Sensitivity at the shipped default (50). The microphone
listening — i.e. a normal service.

**Actual, measured through the router at dial 50 (`auto_fire = 0.50`):**

| What somebody says | What Relay puts on the wall | Confidence |
|---|---|---|
| "please turn to **hymn number three sixteen**" | **Numbers 3:16** | 0.840 |
| "we will sing **hymn number one one**" | Numbers 1:1 | 0.840 |
| "the youth meet in **room two twelve** after the service" | **Romans 2:12** | 0.840 |
| "the crèche is in **room one one** for under fives" | Romans 1:1 | 0.840 |
| "**row three sixteen**" | Romans 3:16 | 0.840 |
| "**van three sixteen**" / "**day three sixteen**" | Daniel 3:16 | 0.840 |
| "**song two twelve**" | Song of Solomon 2:12 | 0.900 |
| "Nehemiah, **fifty two days**" (R4-B) | Nehemiah 5:2 | 0.770 |
| "**Mary, twenty two** years of age" (R4-B) | Mark 2:2 | 0.770 |
| "Psalms **two three one**" (R4-C) | Psalms 2:3 | 0.900 |

`cargo test r6_11` sweeps 118 ordinary English nouns × 5 spoken numbers and finds **37**
phrases that auto-fire.

**Root cause, and it is two things, not one.**

1. **`fuzzy_book` repairs ASR text against a table built for typing.** The abbreviation list at
   `detection.rs:424` was added "for fast manual-override typing" — `num`, `rom`, `dan`, `mic`,
   `lam`, `ps`, `song`, `acts`. `fuzzy_book` then runs a Levenshtein repair against that same
   table with a budget of **1 edit for a token ≤5 characters**. One edit from a three-letter
   typing shortcut is an enormous set of ordinary English words: `rom` → *room, row*; `dan` →
   *day, van, can, ban*; `num` → *number* (6 chars, budget 2, distance 1 to `numbers`). The only
   guard is "the next token must be a number" — which is precisely what an announcement is.

2. **The documented mitigation is worth 0.06 and the margin is 0.34.** `fuzzy_book`'s own doc
   comment says a repaired reference "is still marked FUZZY, which costs confidence downstream,
   so a repaired reference needs to be otherwise strong to reach the auto-fire line." Measured:
   a clean `john three sixteen` is 0.900; a repaired `room two twelve` is **0.840**. The bar is
   0.50. The penalty is not a mitigation. *A contract stated in a comment is not a contract.*

**Why it was never caught.** Three reasons, and they compound:

- `eval.rs`'s corpus has 50 cases and **10 negatives**. One of them —
  `neg-yo-everyday-song`, *"Ẹ jẹ́ ká kọ orin 3"* ("let's sing song 3") — is exactly this trap,
  **in Yorùbá**. The English equivalent was never written. The corpus author saw the shape and
  tested it in the wrong language.
- No negative case contains an ordinary English noun immediately followed by a spoken number.
- **The auto-fire path has no end-to-end test at all** (P1-10). Nothing has ever driven a
  transcript through `emit_detections` to a wall.

**Impact on a volunteer.** The announcements happen before the sermon, when the operator is
looking at their notes and not at the wall. The congregation looks up from "hymn number 316" and
sees Numbers 3:16 — *"And these are the names of the sons of Aaron…"* — with no explanation and
no obvious cause. Nothing the operator did produced it, so nothing they know how to undo will
prevent the next one.

**Recommendation (direction, not a fix).** The typing table and the ASR-repair table are two
different tables serving two different users, and they must stop being one table. Separately:
`fuzzy_book`'s minimum alias length is the real knob — a 1-edit repair against a 3-letter alias
cannot be made safe by any confidence penalty. And whatever is done, the acceptance criterion is
a *negative* corpus of real non-sermon church speech, scored through the router, in CI.

**I am upgrading R4-B from P1 to P0, deliberately.** R4 found the same class via proper nouns in
sermon speech and graded it P1. I found the common-noun variant, which is higher-frequency by an
order of magnitude — every service has announcements — and reaches the wall with **no human in
the loop**. The calibration in the brief is explicit: P0 is "wrong content can reach a
congregation". It does, weekly.

---

## 5. The eleven P1s, in full

### P1-1 · Blackout never reaches the preacher's screen

**Layer C** · `src/Stage.svelte:103–119` vs `src/Output.svelte:130–137` · **always** ·
`npx vitest run src/lib/r6-contracts.test.js -t "R6-3"`

The kiosk hub publishes `content`, `clear`, `black` and `stage_next`. Two browser clients consume
it. `Output.svelte` (the projector / OBS source) handles `black`. **`Stage.svelte` — the
preacher's phone and the stage monitor — has no `black` branch at all.** It handles `content`,
`clear` and `stage_next` and silently drops the rest.

So the operator presses `B`, or the Blank Screen tile, or `/api/black` from the LAN. The
congregation's wall goes black. The screen the preacher is reading from keeps the verse. The
console reports success — **correctly**, because the message did leave the machine — and nothing
anywhere says a screen ignored it.

This is the `stage_next` leak in mirror image: the same twin, the same door, the opposite
direction. `stage_next` sent something it should not have; `black` fails to arrive somewhere it
must. The milder control (`clear`) is honoured and the harsher one is dropped, which is not a
decision anybody made.

> **Disagreement, recorded.** R2 filed this **P3** (R2-I). I file it **P1**. The calibration
> says a panic control that fails silently is P0-grade; this one fails on one of four render
> targets, so P1 is the floor, not the ceiling. A reasonable person would file P0. The reason
> for P1 rather than P0 is only that the congregation's wall *does* go black.

**Fix direction.** Not "add a branch to `Stage.svelte`". Add the contract test first — the hub's
message kinds are a published protocol with two consumers, and nothing enumerated them.

---

### P1-2 · Preview ≠ Programme is not implemented

**Layer C** · `src/lib/views/Library.svelte:81, 87, 110, 115, 476` · **always** ·
`npx vitest run src/lib/r6-contracts.test.js -t "R6-6"` · `npx vitest run src/lib/r2livepath.test.js -t "can never stage anything"`

The sixth safety distinction has no producer. `LiveOutputRail.svelte` renders a Preview monitor,
a Preview badge and a "Take to screen →" button, all keyed off its `preview` prop. `Library.svelte`
is the component's only renderer and binds `preview={staged}`. `staged` is written by exactly one
function, `stage(d)`, and **`stage()` has zero callers** — not a button, not a keyboard path, not
an event, not a store subscription. Both content panes receive `onSelect={select}` where
`const select = () => {}`.

Therefore, for the life of the process:

- the Preview monitor can never show anything;
- **"Take to screen →" is `disabled={!preview || …}` and is permanently disabled**;
- `goLive()`'s `if (preview) return onTake()` branch is dead;
- the badge can never read "Preview".

Independently found by R2 (R2-E) and by me, with different instruments. Agreed exactly.

> **This is the finding that indicts the audit process, and it belongs here.** During this run the
> orchestrator shipped a product-code fix for the amber-while-staging bug (F1) — the
> `preview ? 'grey'` badge and the `.lo-behind` "Wall live" chip now in
> `LiveOutputRail.svelte:167–187` — plus 17 tests in `liveoutputrail.test.js`. **Every one of
> those 17 tests mounts the component with a `preview` prop supplied by the test.** Nothing in
> the shipping app can supply it. The fix is unreachable code and the tests are green over a
> feature that does not exist. F1's severity should be revised to **not-a-live-bug**: the amber
> badge could never have lied, because the branch could never be taken.
>
> The lesson generalises and is the single most useful thing in this document: **verifying that
> something renders a component is not verifying that anything can supply its input.** Three
> findings in this audit are that shape at three depths — F2 (a component with no renderer),
> P1-2 (a prop with no producer), P1-10 (a fire path with no test).

---

### P1-3 · Escape clears the congregation's screens from seven overlays, and does not close them

**Layer C** · `src/lib/shortcuts.js:143` + six menus + the crash panel · **always** ·
`npx vitest run src/lib/r6-contracts.test.js -t "R6-4"`

`shortcuts.js` suppresses the Escape → clear-the-wall panic action only when a `[role="dialog"]`
is mounted (plus a special case for the cheatsheet). That guard exists because of architecture
rule 16: *"Dismissing a help overlay or an arrangement picker is not a live action — it used to
wipe the wall as a side-effect."*

Seven surfaces are outside the guard:

| Surface | Role it carries | Consumes Escape? |
|---|---|---|
| `LiveOutputRail.svelte:363` — Countdown minute picker, **on the run rail** | none | no |
| `VerseDeck.svelte:191` — per-verse kebab, **in the live verse deck** | none | no |
| `TemplateGallery.svelte:266` — New Template menu | `role="menu"` | no |
| `TemplateGallery.svelte:457` — per-template More menu | `role="menu"` | no |
| `TemplateEditor.svelte:531` — History menu | `role="menu"` | no |
| `TemplateEditor.svelte:572` — Add-layer menu | `role="menu"` | no |
| `CrashReportRecovery.svelte` — the crash panel | **`role="alertdialog"`** | no |

`Library.svelte:350/363` gets it right (`menuEsc` → `stopPropagation`, with `use:trapFocus` so
the handler can fire at all) and is the control case.

Open the Countdown picker mid-service, change your mind, press Escape — the universal "close
this" key. The menu stays open. The wall goes blank.

The crash-panel case (R2-F) is the sharpest: the panel's own copy reads *"Your output screens are
still live … Recovering reloads only this control panel. It will not blank the screens."* It
binds nothing to Escape. `role="alertdialog"` is not `role="dialog"`. The operator hits the reflex
key at the one moment the wall is guaranteed hot, and the panel's promise is broken by the app
itself.

Independently found by R3 (R3-01, six menus), R2 (R2-F, the crash panel) and me (the same six
menus, via a different query). Counted once.

**Fix direction.** Not seven patches. `shortcuts.js` should read the DOM for `[role="menu"]` and
`[role="alertdialog"]` as well — the guard was written to *"read from the DOM rather than a
registry, because the whole point is that this must not depend on anybody remembering"*, and then
the query it reads with is a registry of one role.

---

### P1-4 · The Announcements editor silently disarms the panic key

**Layer B+C** · `src/lib/views/library/Announcements.svelte:183` · **always** ·
`npx vitest run src/lib/surface.test.js -t "Announcements"`

The inverse of P1-3, and worse. The Announcements editor is an ordinary in-flow panel — no scrim,
no `position:fixed`, no `aria-modal`, no focus trap, no Escape handler — and it carries
`role="dialog"`. The global guard sees a dialog and stands down.

`Esc` is the **only** panic key that survives a focused text field (`B` is deliberately suppressed
while typing, so an operator typing "Habakkuk" does not black out the church). The Announcements
panel is nothing but text fields. **While it is open, the operator has no panic key at all**, and
nothing indicates that.

Filed by R3 (R3-02). I confirmed the markup independently while auditing dialog accessibility and
initially graded it a11y-only; **R3 is right and I was wrong** — the live-safety consequence is
the finding, and the missing `aria-modal` is a symptom of it.

---

### P1-5 · `Space` puts scripture on the wall from a list row

**Layer B** · `src/lib/views/library/VerseDeck.svelte:57–64` · **always** ·
`npx vitest run src/lib/surface.test.js -t "Space"`

`Space` means *advance*, app-wide, and nothing else (architecture rule 11). The VerseDeck grid
card is a native `<button>`, so `shortcuts.js`'s `preventDefault` suppresses the browser's default
activation. The **list row** is a `role="button"` `<div>` whose own keydown calls `onFire(v)` and
neither prevents default nor stops propagation, so it runs first.

Tab to a verse in the list, press Space to scroll or to advance — scripture goes live.

R3 verified the scope rather than assuming it: only `Live.svelte` calls `registerContext`, so the
transport does not *also* move. Six views render `VerseDeck`.

---

### P1-6 · Amber "On Air" survives five of the nine paths that take the wall

**Layer A+B** · `src/lib/stores/capture.js:212` (`leavePlan`), `:327–329`; `src/lib/views/Live.svelte:1191`, `:124` ·
`npx vitest run src/lib/r2livepath.test.js -t "who takes the plan off air"`

`capture.js` states the rule in its own comment — *"EVERY path that takes plan content off the
screen has to clear `onAir`"* — and `leavePlan()` has **three** call sites against nine content
wrappers. Verified independently: `leavePlan` is called at `capture.js:615`, `:658`, `:1621`.

Clears `onAir`: `manualFire`, `confirmDetection`, `clearScreens`, `blackScreen`.
Does not: `fireContent`, `fireMedia`, `startCountdown`, `pushAnnouncement`, `navVerse`.

Amber means live and is never allowed to lie. Here it lies in five ways.

The sharpest case no wrapper can fix: `/api/clear` from the preacher's phone and the spoken
"clear" reach `channels::clear` directly. The console learns via `output://clear`, whose listener
sets `live` and `screenBlack` and nothing else.

And the backend half compounds it: `Context` is written only by scripture fires, so a passage
stays armed under a song forever — `nav next` fires **John 3:17** onto a wall showing a hymn and
reports `Fired`. That becomes a mode bug at `Live.svelte:124`: `Esc` nulls `$live` → transport
reads **SLIDE**; `B` leaves `$live` set → transport reads **VERSE**. Same state, two panic keys,
opposite transports, and only the `Esc` behaviour is documented. After a blackout mid-plan the
next `→` fires a verse from an earlier passage *and cancels the blackout*.

It survived because `transport.test.js` pins exactly the four wrappers that behave.

---

### P1-7 · `confirm_detection` reports a success it did not achieve — twice

**Layer A** · `src-tauri/src/main.rs:2426`, discarded bool at `:2452` ·
`cargo test detection::r4_audit -- --ignored`

Two paths through `confirm_detection` return `Ok(())` having put nothing on any screen:
`detect_direct` returning nothing falls straight through the `if let Some(m)`, and `fire_manual`
returning `false` has its boolean **discarded — no binding, no `if`**. Its twin `manual_fire`
reports both.

It is reachable by design: `emit_detections` deliberately demotes a parsed-but-absent verse
(garbled speech readily yields `Psalms 23:99`) to `Suggested` and emits it with
`in_library: false` — and **no frontend file reads `in_library`**. So the suggestion card looks
identical to a real one, Accept is enabled, the backend returns `Ok`, `capture.js` runs
`leavePlan()` and removes the card, and `Live.svelte:372` flashes *"Now live: Psalms 23:99"* over
the previous verse, which is still up.

It also calls `record_feedback(true, …)` on that path, teaching the self-calibrating gate that a
fire which never happened was correct.

This is the exact bug the comment above `acceptTop` says was fixed.

---

### P1-8 · Three shipped documents promise a guarantee the LAN remote broke

**Layer A+C** · `src-tauri/src/main.rs:1161–1241`; `docs/DECISIONS.md:47`; `PRIVACY.md:82`; `SECURITY.md:65` ·
`cd src-tauri && cargo test qa_r5::the_lan_remote_answers_exactly_seven_routes_and_refuses_the_rest`

The decision record says the hub is *"broadcast-only … a stranger on the network can read the live
content feed but can **never push to the screens**"*. `PRIVACY.md` says *"They **cannot** push
content to your screens."* `SECURITY.md` says *"broadcast-only (no screen takeover)"*.

`remote_api` serves unauthenticated `fire / next / prev / clear / black` on `0.0.0.0:8032`, and
`Stage.svelte:26–61` ships a touch UI for exactly that at `http://<host>:8032/stage.html`.

The code cites "DECISIONS §47" in three places. **`DECISIONS.md`'s sections end at §34.** 47 is
the *line number* of the old broadcast-only row. The expansion from read-only to control was
never written down anywhere.

> **Orchestrator error, audited as a finding.** R5's brief asserted the LAN no-auth control plane
> was a decided position and told R5 not to report it. R5 checked anyway — correctly — and found
> the record says the opposite. **R5 was right to check and the brief was wrong.** The shared
> preamble's own instruction was *"your job is not to report it — it is to check the decision
> still holds"*, and R5 executed that instruction to the letter and produced the most important
> documentation finding in the run. An auditor told "this is settled" should still read the
> settlement.

**Recommendation.** Documentation-first, and before any code change. A church deciding whether to
put Relay on its network is reading PRIVACY.md, and PRIVACY.md is currently wrong.

---

### P1-9 · An imported template can beacon to the internet, and go blank offline

**Layer B** · `src/lib/TemplateRender.svelte:87, 613, 642`; `src/lib/templates.js:201–219` ·
`npx vitest run src/lib/qa-r5-template-injection.test.js` *(red on purpose)*

`parseImportedTemplate` validates **shape only** — a marker, a `layout` object, a `style` object —
and not one value inside either. `bgPaint` interpolates raw into a style attribute:

```
background:url("http://tracker.example/beacon.png") center / cover no-repeat
```

observed in the rendered DOM. So a template pack shared between churches — which is what
export/import exists for — is a per-fire beacon, and **a blank background the first Sunday the
wifi is out**, on a product whose first constraint is offline-first.

R5 also proved and pinned the good news: **no `{@html}` anywhere in `TemplateRender`**, and a
verse containing `<img src=x onerror=…>` renders as inert text. I confirmed this independently
across all 46 components — the three `{@html}` sites that exist (`App.svelte`, `Settings.svelte`,
`Help.svelte`) all interpolate hardcoded module constants and are now allow-listed by a test
(`R6-G2`).

> **I am adding to R5-2, and it makes it worse.** R5 wrote that "the shipped CSP permits `http:`".
> That is true of the console. **The LAN-served output page has no CSP at all.** `serve_embedded`
> and `serve_json` in `channels.rs:1024–1046` send `Access-Control-Allow-Origin` and
> `Cache-Control` and no `Content-Security-Policy` header on any response — verified:
> `grep -c "Content-Security-Policy" src-tauri/src/channels.rs` → **0**. So on the OBS browser
> source and every kiosk screen, which is where most churches will actually render, nothing
> constrains an imported template's CSS at all. R5's SUSPECTED half (`color: "red; position:fixed;
> inset:0; background:#000"` as a full-frame blackout from a file) has no CSP standing behind it
> on those surfaces either.

**Recommendation.** Validate at the import boundary, not in the renderer — there are five call
sites and there will be a sixth. And send a CSP header from `:8032`.

---

### P1-10 · The auto-fire path — the only thing Relay does that nothing else does — has no end-to-end test

**Layer C** · `src-tauri/src/main.rs:689` (`emit_detections`), `:2426` (`confirm_detection`)

```rust
fn emit_detections(handle: &tauri::AppHandle, text: &str, now_ms: u64, is_final: bool)
```

`tauri::AppHandle` with no `<R>`. Architecture rule 24 exists for exactly this: *"The fire engine
is generic over `tauri::Runtime` … That is what makes `e2e.rs` possible: welded to the concrete
desktop runtime, the one path that puts scripture on a wall could not be driven without a window,
and so was never tested. **Keep new fire-path code generic — a concrete `AppHandle` quietly
re-welds it.**"*

`main.rs` has 16 functions generic over `R`. Every human fire path is one of them, and `e2e.rs`
drives them all: `manual_fire`, `nav`, `clear_screens`, `blackout`, `set_rehearsal`.
`emit_detections` and `confirm_detection` are not, and cannot be driven at all.

**So every e2e test in this repo exercises a path with a human in the loop, and the path with no
human in the loop has never been exercised end to end.** That is why P0-1 and P1-7 both had to be
proved by mechanism rather than by driving them, and it is the structural reason P0-1 shipped.

Independently found by R4 (R4-I) and me. Agreed exactly, including the diagnosis.

---

### P1-11 · No frontend test in this repo can observe a component mounting

**Layer B** · `vitest.config.js` · **always** ·
`npx vitest run src/lib/r6-lifecycle-probe.test.js`

Svelte 4's `package.json` maps the bare `svelte` specifier to `./src/runtime/ssr.js` under every
export condition except `browser`. That file contains, literally:

```js
export function onMount() {}
export function beforeUpdate() {}
export function afterUpdate() {}
```

`vitest.config.js` sets `environment: 'jsdom'` and **no `resolve.conditions`**, so vite-node
resolves through the default condition. Components still render correctly, because
`svelte/internal` has only a `default` condition and is always the real client build.

**The precise statement, which is sharper than "onMount is a no-op":**

| API | In this suite |
|---|---|
| `onMount` | dead |
| `beforeUpdate` | dead |
| `afterUpdate` | dead |
| `onDestroy` | **real** |
| `tick`, `setContext`, `createEventDispatcher` | **real** |

Two consequences nobody stated:

1. **`afterUpdate` is dead too**, and architecture rule 1 names `afterUpdate` as the *safe*
   alternative to calling `tick()` in a reactive block. The rule the repo learned from a hard
   freeze cannot be regression-tested.
2. **The deadness is asymmetric.** `onDestroy` runs. So a test that asserts cleanup happened can
   pass over a setup that never ran, and read as coverage.

Demonstrated three ways in the probe: by inspecting the resolved functions, by asserting the
asymmetry, and by mounting a real component through the same plugin pipeline and observing that
`seen` contains `['destroy']` and nothing else.

R2 added the concrete cost (R2-G): mounting `LiveOutputRail` and waiting 20 ms produces **zero**
`invoke` calls though its `onMount` awaits `listOutputChannels()` — so `channels` stays `[]` and
`monitorTemplate`, the thing deciding *which screen* the run column shows, is entirely
unexercised. `Live.svelte` cannot be mounted meaningfully at all: no plan loads, the playhead is
never restored, `registerContext` never runs.

Found independently by R3, R2 and me. **Counted once, and weighted heavily** — three instruments,
three agents, one conclusion.

R3 measured the remedy: with `resolve.conditions: ['browser']` added, 474 of 475 tests still pass
— *which is the point*. Almost nothing was asserting on lifecycle, because nothing could.

**Recommendation.** One line in `vitest.config.js`. Then re-run this entire audit's layer-B
findings, because their confidence changes. R3 correctly did not apply it; changing the harness
mid-audit would have invalidated the baseline.

---

## 6. CRUD completeness

18 tables in `docs/data/schema.sql`. R1 traced every one from `INSERT` → `#[tauri::command]` →
`capture.js` wrapper → component → routed view, starting from `db::init_fresh` and nothing else.
I re-derived the table list and the create/insert map independently and agree with R1's verdicts.

**Reachable from a rendered control:** 14 of 18.

| Table | Reachable? | Breaking link |
|---|---|---|
| `verses` | seeder only | correct by design — 31,100 KJV verses are product content |
| `translations` | **no** | **R1-05** — no command, no importer, not even a documented file drop. The Library's translation `<select>` carries `disabled={translations.length < 2}` and is therefore **permanently disabled on every install that will ever exist** |
| `songs`, `song_sections` | **file picker only** | **R1-01** — `newPasteSong()` sets `lyricAction`, which is passed to no child and declared as no prop. A church whose lyrics arrive by email cannot add a song. `save_song` is an UPDATE and refuses a non-existent id |
| `saved_scripture` | **file picker only** | **R1-01** — same shape, `scriptureAction` |
| `song_arrangements` | **no** | **R1-03 / R3-05 / F3** — the whole write side is orphaned (`saveArrangement` *and* `deleteArrangement`), while the reader is wired. `ServicePlanner.svelte:253` therefore always takes the empty branch, and **the arrangement picker at `:801`, with its focus trap and Escape handling, is a dialog no operator can reach** |
| the other 13 | **yes** | driven end to end by `cargo test --quiet cold_start::every_table_a_control_can_reach_is_filled_by_driving_the_real_commands` |

**Two stated caveats on R1's matrix, and they are the right caveats to state:** `save_template`
takes a concrete `AppHandle`, so templates were driven one layer down at `db::upsert_template`;
`import_media` writes to `db::media_dir()`, so `media_assets` was driven at `db::insert_media`.
Both are weaker claims and R1 labelled them so.

**Dead commands — adjudicated.** R1-07 says five, R3-05 says eight. **R3 is right.** I traced all
eight wrappers to their importers:

```
saveArrangement · deleteArrangement · createTemplate · importSong
importProFile   · openOutput        · listOutputWindows · activeVoiceProfile
```

Every one exists in `capture.js`; **no component imports any of them.** Reproduce:
`for f in saveArrangement deleteArrangement createTemplate importSong importProFile openOutput listOutputWindows activeVoiceProfile; do grep -rl "\b$f\b" src/ | grep -v 'capture.js\|test'; done` → empty.

CLAUDE.md's claim that *"every one of the 114 registered commands has a frontend caller"* is
**false**, and it is a claim with a date on it. `open_ndi_output` is correctly excluded as
parked-by-design.

> **Correction to R3-05's framing, from my own check.** The eight are not equivalent. Three
> (`open_output_window`, `list_output_windows`, `active_voice_profile`) are dead **because they
> were superseded** — `open_channel_output` at `main.rs:3201` is registered and is called by the
> "Open" button at `Channels.svelte:403`, so the native-output feature is fully reachable. Two
> (`import_song`, `import_pro`) are superseded by `parse_import`, though ProPresenter import is
> listed as *Shipping* in CLAUDE.md while its command is dead. **Three are dead because a feature
> is missing**: `save_arrangement`, `delete_arrangement`, `create_template`. Those three are the
> ones that matter, and lumping all eight together hides which.

**Referential integrity — R1's passes, each with a command.** Deleting a pinned template degrades
the cue to the content default rather than broadcasting a dangling id · deleting a song takes its
sections and arrangements but leaves the plan cue **with its snapshotted lyrics** · deleting a
media asset takes exactly its cue.

> **Latent, and R1 was right not to file it but right to note it.** `delete_song` removes sections
> explicitly and leaves arrangements to the FK cascade — which only fires while
> `PRAGMA foreign_keys` is ON, and that pragma is turned **off and back on** inside
> `ensure_manual_detection_status`. One reordering away from an orphan.

---

## 7. Seed audit

From `db::init_fresh`, verified by R1 and re-derived by me from `docs/data/schema.sql`:

| Table | Rows | Note |
|---|---|---|
| `verses` | 31,100 | full KJV, `include_str!` from `src-tauri/data/kjv.json` |
| `verses_fts` | 31,100 | derived |
| `translations` | 1 | and there can never be a second — R1-05 |
| `templates` | **31** | 4 built-ins + Worship Lyrics + 26 presets |
| `output_channels` | 4 | each with a non-null `template_id` |
| `voice_profiles` | 1 | active, sensitivity 50, verified equal to `Thresholds::default()` |
| `app_settings` | 1 | key `tpl_song` |
| everything else | 0 | |

**Nothing seeded is demo data.** No fake plan, no sample song, no example service. That is the
right call and it is worth saying so.

**The fixture trap R1 was sent to find does not exist.** A genuine first launch fires John 3:16
with `template_id: null` and it still renders, because a screen's own template wins (DECISIONS §29)
and all four seeded screens carry one. Cold-start counterpart added:
`cargo test --quiet cold_start::a_first_launch_puts_a_verse_on_the_wall_with_no_setup_at_all`.
What `e2e::app()`'s convenience actually hides is narrower and R1 named it precisely: its assertion
*"the scripture template was dropped on the way to the wall"* is a claim about an
already-configured system.

**Migrations.** All ten `ensure_*` rungs are retryable over ten consecutive boots.
`ensure_manual_detection_status` is the only table rebuild and already does
`DROP TABLE IF EXISTS` first and `ROLLBACK` on failure. I re-verified this independently:
`grep -rn "_new" src-tauri/src/db/*.rs` finds exactly one rebuild, with the correct shape. The
class of bug that once bricked every subsequent boot is closed.

One real defect remains (R1-08, P3): the rebuild's column list is hard-coded at **seven** columns
and `heard_text` is an eighth, added by the v2 rung. Safe today only because the rebuild runs on
the v0 baseline path and `run_migrations` runs after — **an ordering asserted by nothing**.
`heard_text` exists because a real service put forty wrong verses on a wall and the log could not
say what any of them heard.

---

## 8. Screen by screen

| Surface | Routed | Verdict | Layer | Key findings |
|---|---|---|---|---|
| **Live** (`Live.svelte`) | yes | **the good one** — real Preview/Program pair, real transport mode, aria-live on the transport and messages | A/B/C | R2-D, R2-H, R3-12 (the only major view that never says the engine is missing, though 18 of its controls disable on it) |
| **Library** | yes | a **second run surface** with a full mic/panic/rehearsal/queue column, and no keyboard transport | B/C | **P1-2**, P1-3, P1-5, R1-01, R3-08 |
| **Outputs** (`Channels.svelte`) | yes | solid; the humanised-error regression is genuinely fixed | B/C | *see the note below* |
| **Templates** | yes | works | B/C | P1-3 ×2, R3-04, R3-13 (no heading at all) |
| **Themes** | yes | works | B/C | R6-5 raw errors ×4, R3-13 (no heading) |
| **Planner** | yes | works | A/B | R3-05 (the arrangement picker is unreachable), R2-H |
| **Settings** | yes | works | A/B | R4-F (the second baseline), R1-12 |
| **Help** | yes | genuinely good — offline, and the cheatsheet cannot advertise a dead key | C | — |
| **Output page** (`Output.svelte`, `output.html`) | n/a | handles all six message kinds correctly | C | **no CSP when served from `:8032`** (P1-9) |
| **Stage page** (`Stage.svelte`, `stage.html`) | n/a | **the least-audited surface in the product** | C | **P1-1**, and it has no rehearsal indicator at all |
| Boot gates (9 components) | conditional | | C | P1-3 (crash panel), R5-3, R2-J, R3-10 |

> **A methodological caution, and I am recording it because I nearly filed it as a finding.**
> `Channels.svelte` contains **zero** calls to `humanError` — and it is **correct**. It stores the
> typed error and renders it through `ui/ErrorState.svelte`, which humanises. Any audit that greps
> for `humanError` and calls Channels a regression is wrong. Pinned as a note in
> `src/lib/r6-contracts.test.js` so the next auditor does not repeat my mistake.

> **`Stage.svelte` deserves its own line.** It is a full second console — it can search the whole
> Bible, fire any verse to the wall, and walk a passage — and it is served over the LAN with no
> authentication to any device that scans a QR code. It has: no rehearsal indicator (so a preacher
> firing during a rehearsal gets `{"ok":true}` and a wall that does not move, with no explanation),
> no `black` handling (P1-1), no panic control of its own, and it is unreachable by layers A and B
> alike. It is the surface with the most authority and the least coverage in the product.

---

## 9. End-to-end workflows

| Workflow | Verdict | Layer | Command |
|---|---|---|---|
| Fresh install → fire a verse → it reaches the wall with its text and template | **PASS** | A | `cargo test --quiet cold_start::a_first_launch_puts_a_verse_on_the_wall_with_no_setup_at_all` |
| Fire → `→` walks the passage → reports `EndOfPassage` at the boundary | **PASS** | A | `cargo test e2e` |
| Clear / blackout → the wall goes blank **and the command says it did** | **PASS on the console** | A | `cargo test e2e` |
| The same, on the preacher's screen | **FAIL** | C | **P1-1** |
| Panic → cue position survives → next `→` resumes where it was | **PASS** | A | `cargo test e2e` |
| Rehearsal → nothing reaches the wall, the kiosk, or the stage tablet | **PASS on four push doors** | A | `cargo test e2e` |
| Rehearsal → nothing reaches the **LAN pull door** | **FAIL** | A | `cargo test r6_2` |
| Preacher's phone: search → fire → next → the outcome rides in the JSON | **PASS** | A | `cargo test e2e` |
| Preacher's phone: "what is live" matches the wall | **FAIL** | A | `cargo test r6_1` |
| Build a plan in Planner → run it in Live | **PASS** until the last slide | A/B | R2-H |
| **Listen → transcribe → detect → gate → wall** | **BLOCKED at every audio step**, and **FAIL** at the gate for ordinary speech | A (text only) | **P0-1** |
| Import a ProPresenter file → review → save | **PASS** at the parser | A | `cargo test proimport` — and see R5-7 (no cap; a 42 KB zip bomb is an OOM kill) |
| Add a song by pasting lyrics | **FAIL — no path** | C | R1-01 |
| Crash → recover → nothing stale returns to air | **PASS**, except the amber badge | A/C | R5-3, R2-J |

---

## 10. Live-production safety — answered in words a volunteer would use

### Could a volunteer safely run a service with this build?

**No.** Not because it would crash — it is more robust than most software of its age — but
because of one specific thing: **during the notices, when somebody says "please turn to hymn
number three sixteen" or "the youth are meeting in room two twelve", Relay will put a Bible verse
on the screen on its own.** Nobody pressed anything. Nothing in the app explains it. The verse
that appears is real scripture, correctly formatted, and completely unrelated to what is
happening in the room.

There is a workaround and it is not a good one: turn the microphone off until the sermon starts,
and turn it on again. Nothing in the app tells you to do that.

### Can they tell Preview from Programme, and Cued from On Air, at a glance?

**Cued vs On Air: mostly yes, and it lies in five specific ways.** Grey means "this is where `→`
will resume, and it is not on screen". Amber means "the congregation can see this right now".
Amethyst means "rehearsal". Those are right, and the console honours them. But after a picture, a
video, a countdown, an announcement, or a `→` that walked a verse, the amber light **stays on
after the thing it was describing has gone** (P1-6). Amber is the one indicator that is never
allowed to lie, and it does.

**Preview vs Programme: no — there is no Preview.** The Library's run column shows a Preview
monitor and a "Take to screen →" button, and nothing in the app can ever put anything into them.
The button is permanently greyed out. That is the whole distinction, and it is not there (P1-2).

*(Two things in the room here are unmeasurable from this machine: whether amber and amethyst are
distinguishable on a dim booth monitor, and whether they are distinguishable to a colour-blind
volunteer. §16 step 12.)*

### Can they clear the screens and black out immediately, and would they be told if it failed?

**Clear: yes, and yes.** `Esc` works from every tab, including one whose view has crashed, and it
works with your cursor in a text box. If it fails, the console says so and does not claim success
— that has been fixed properly and is pinned by tests.

**Blackout: mostly, with two holes.**

1. `B` does **not** work while you are typing. That is deliberate and correct — otherwise typing
   "Habakkuk" would black out the church — but it means that in a text box `Esc` is your only
   panic key.
2. **The blackout does not reach the preacher's screen** (P1-1). The wall goes black; the stage
   monitor and the preacher's phone keep the verse. Nothing tells you.

**And a third thing that is not a hole in the panic keys but is worse in practice:** on seven
different pop-up menus and on the crash-recovery panel, pressing `Esc` **causes** a clear you did
not ask for, and does not close the menu (P1-3, P1-4). The most likely way a volunteer will
accidentally blank the congregation's screens with this build is by pressing Escape to close a
dropdown.

### Can they override the AI?

**Yes, and this part is genuinely well built.** Manual fire always wins — it bypasses every
threshold and every debounce. The AI can only ever *suggest* a paraphrase, at any confidence, and
four separate agents attacked that cap from four directions and could not get past it. A
paraphrase never shows a percentage, because a cosine is not a probability, and that rule holds on
all four surfaces that render a claim.

The gap is not the override, it is the *undo*: accepting a suggestion for a verse that is not in
the Bible text tells you "Now live: Psalms 23:99" while the previous verse is still on the wall
(P1-7).

### Can the app recover from an interruption without putting something stale back on air?

**Almost. One thing comes back wrong.** On restart, nothing is re-broadcast — the kiosk handshake
replays the template and the themes and never any content, and `live` starts empty. That is right,
and it was verified.

But the **amber "On Air" light comes back on** for a cue that died with the process, if the
operator dismissed a boot gate with `Esc` (R5-3). So the console says the congregation is looking
at John 3:16 and the projector window no longer exists. R2 found a second, adjacent path (R2-J):
`Live.svelte` reads its copy of `liveOnAir` *before* the recovery gate has been answered, so the
obvious fix for R5-3 does not close it.

There is also a reliability question nobody could settle from here — see §12.

---

## 11. Offline

**PASS**, and this is one of the strongest parts of the product. R5 enumerated every network call
and I re-derived the list independently; we agree completely.

| Call | Classification |
|---|---|
| Whisper model download (`models.rs:118–170`) | optional, explicit, friendly error, resumable |
| Updater manifest (`updater.js:47–60`) | optional; catch returns null; **refuses to check while the mic is live** |
| Sentry | opt-in, no DSN in OSS builds, dev DSN compiled out of release |
| `local_ip` (`main.rs:2244`) | UDP `connect` to 8.8.8.8:80 **sends no packet**; offline ⇒ boot reports *"offline — every core feature still works"* |
| OBS / ATEM probes | loopback, 300 ms, off-thread |
| Kiosk WS + HTTP servers | LAN **servers**, not clients |
| Fonts | **none** — `app.css:10` says so and it is true |

Frontend: verified there is no `fetch` or `WebSocket` anywhere except `Output.svelte` and
`Stage.svelte`, and both talk only to Relay's own LAN ports. Pinned:
`npx vitest run src/lib/r6-contracts.test.js -t "R6-G1"`.

**No hidden dependency.** The one hole is P1-9 — a template can import one.

---

## 12. Recovery, and one disagreement worth the human's attention

R5 filed as a **PASS**: *"`run_kiosk_server`'s `hello` replays template and themes only, never
content."* I read the identical line of code (`channels.rs:897–928`) as a **defect**.

Both readings are correct and they cannot both be acted on:

- **R5's reading (safety).** Nothing stale can ever come back on air by itself. A kiosk that
  reconnects after the operator has cleared the wall does not resurrect a verse. This is right,
  and it is the more important property.
- **My reading (reliability).** A projector on church wifi that drops its socket for two seconds
  mid-verse reconnects to a hub that sends it a template and no content. **The wall stays blank
  until the operator fires something else**, and the console shows the verse still up, because
  the console's `$live` is unaffected. The Channels tab's online light will blink off and back
  on; nothing else indicates what happened.

I file this as **R6-I, P2 SUSPECTED**, because layer D was disabled and I could not drop a socket
and watch. It is §16 step 8, and it is the single test where a human will learn something neither
of us could.

The right resolution is probably neither: a `hello` that replays the *current* content only when
the content is genuinely live gets both properties. That is a design question, not a bug fix.

---

## 13. Security

| Finding | Sev | Layer | Note |
|---|---|---|---|
| **P1-8** three docs promise a broken guarantee | P1 | A/C | §5 |
| **P1-9** imported template beacons; no CSP on `:8032` | P1 | B/C | §5 |
| **R5-6 → P2** any web page can drive the wall | **P2** *(upgraded from R5's P3)* | C | below |
| **R5-5** telemetry scrub is a blocklist documented as an allow-list | P2 latent | A | below |
| Route surface has not grown | **PASS** | A | `cargo test r6_3` — exactly seven; 12 plausible new names refused |
| `clear`/`black` from the LAN reach the same engine as the console panic keys | **PASS** | A | `cargo test qa_r5` |
| SQL parameterised throughout | **PASS** | A | the single `format!` into SQL (`db/profiles.rs:154`) interpolates a `const` column list |
| Media path traversal | **PASS** | A | `serve_media_file` takes leading ASCII digits only — I re-verified |
| Static path traversal | **PASS** | C | `..` rejected before the dev disk read; the embedded bundle is traversal-safe |
| Hostile input round-trips | **PASS** | A | Yorùbá/Swahili/Hausa diacritics, quotes, emoji, 20,000 chars, `<script>`, an RTL override, a SQL-injection string — all byte-identical |
| No `{@html}` on any path that reaches a wall | **PASS** | C | `npx vitest run src/lib/r6-contracts.test.js -t "R6-G2"` |

**R5-6, upgraded to P2.** Three properties compose: the HTTP request line is parsed
**verb-agnostically** (`channels.rs:1146–1150` takes `split_whitespace().nth(1)` and never checks
the method), every action is a **side-effecting GET**, and every response carries
`Access-Control-Allow-Origin: *`. So `<img src="http://<relay-ip>:8032/api/black">` on **any**
website, loaded by **anyone** on the church wifi, blacks out the wall — no preflight, no LAN
foothold beyond a victim's browser, and with `ACAO: *` the response of `/api/live` is readable
cross-origin too. R5 graded this P3. I grade it P2: the recorded threat model is "someone already
on the church network", and the actual exposure is "someone who can get any phone on the guest
wifi to load a web page", which includes an ad. It is the second reason P1-8's record needs
rewriting.

**R5-5.** `telemetry.rs:47–88`'s `scrub` is documented as an allow-list — *"a blocklist fails
open, and the cost of failing open here is publishing somebody's sermon"* — and implemented as a
blocklist. It enumerates the carriers it empties and ships every other field of
`sentry::protocol::Event` verbatim: `logentry.message`, `logentry.params`, `tags`, and
`threads[].stacktrace.frames[].vars` — the *same* carrier the exception path is careful to clear,
on the other stacktrace field. **Not a live leak today**: only the `panic` integration is enabled
and nothing calls `capture_message`, `add_breadcrumb` or `set_tag`. `qa_r5::nothing_a_sentry_event_can_carry_survives_scrub`
is red on purpose.

**Zero `unwrap()`/`expect()`** in all ten service modules, verified independently:
`for f in audio dsp stt detection router pipeline channels models proimport telemetry; do ...` →
0 each. `main.rs` has five, all inside `main()`/`setup`, all startup-only. The rule holds.

---

## 14. Performance

Layer A and C only; nothing was profiled under load and nothing was measured on a real machine
with a real projector.

- **R5-7 (P3).** `import_media` takes the global `Db` mutex at `main.rs:1667` and holds it through
  `std::fs::write` at `:1681`. For the duration, every command touching SQLite blocks — including
  `emit_detections`, `fire_manual` and `persist_cue`. The clear itself still reaches the wall
  (`channels::clear` runs before `persist_cue`), so this is a stall and not a lost panic, but
  **the AI goes deaf and mute while a volunteer imports a sermon video.** No size cap at any layer;
  the file is held in RAM three times over.
- **R5-7 (P3).** `parse_proplaylist` (`proimport.rs:243`) `read_to_end`s every ZIP entry with no
  entry count and no cap. A 42 KB zip bomb is an OOM kill. Path traversal *is* defended.
- **Lock discipline — R5's priority-1 audit is clean, and it is the most thorough single piece of
  work in this run.** All 125 `.lock()` sites in `main.rs`, 10 in `channels.rs`, 8 in `stt.rs`, 3
  in `telemetry.rs`, plus a scripted scope-scan for a live guard enclosing `emit` / `emit_to` /
  `broadcast_content` / `publish_kiosk`. **No mutex is held across an emit or a broadcast.**
  `Db` before `Session` everywhere. The one apparent inversion, `verse_repeat_count`, is safe only
  because the guard is a statement temporary — one refactor from being real.
- The deliberate linear semantic scan (2.6 ms/query at ~1 query/sec) remains the right call and
  was not re-litigated.
- **Not measured, by anyone:** startup time, memory over a two-hour service, template render cost
  with an embedded `data:` image, WS fan-out to N kiosks, SQLite growth over a year of services.

---

## 15. Accessibility and UX

| Finding | Sev | Note |
|---|---|---|
| **R3-08** seven error surfaces are never announced | P2 | `LiveOutputRail:377`, `MediaLibrary:255`, `LyricsPane:422`, `Scripture:227`, `Browse:424`, `Dashboard:290`, `ImportReview:83`. **The run rail is the worst**: the line saying a Take or a Countdown *failed* is silent to a screen reader. I confirmed `LiveOutputRail` independently — no `role`, no `aria-live`, no ancestor carrying either, while `Live.svelte` has both |
| **R3-04** Empty ≠ Loading ≠ Error is not honoured on most lists | P2 | A fresh install ships **31 templates** and the first thing a new operator reads on the Templates tab is *"No templates yet — create one to start."* The Error half is **structural**: every read wrapper in `capture.js` is GROUP 2 and returns `[]` on failure. I re-verified `listPlans`, `loadTemplates`, `listOutputChannels`. **No list in Relay can distinguish "the query failed" from "there is nothing here."** `ErrorState.svelte` is imported by exactly one view |
| **R3-06 / R6-5** raw backend errors reach volunteers | P2 | **Six sites**, not three: `ImportReview.svelte:72→:83` (`r-mono`), `History.svelte:14→:130` (`r-mono`), `ThemeEditor.svelte:65→:93`, and **`ThemeGallery.svelte:67, 83, 97`** which R3 missed. And a mechanism R3 did not state: `error.rs` serialises as `{kind, message}`, so `String(e)` renders the literal text **`[object Object]`** — in monospace, on two of them. `npx vitest run src/lib/r6-contracts.test.js -t "R6-5"` |
| **R3-09** amethyst means two things, six lines apart | P2 | `VerseDeck.svelte:166` on-air badge is `{rehearsing ? 'amethyst' : 'amber'}`; `:172` is `class="r-badge amethyst"` for **"Edited"**. In a rehearsal the deck shows two identical amethyst pills, one of them the safety signal |
| **R3-10** modals do not trap or restore focus | P3 | `TemplatePreviewOverlay.svelte:24` claims `aria-modal="true"` with no trap — after opening, `document.activeElement` is still the opener and Tab walks out into the app behind. Four boot gates never move focus in. I confirmed the same set from a different query |
| **R3-13 / R6-M** no heading structure | P3 | No heading **at all** in `Library`, `Templates`, `Themes`, `ThemeGallery`, `ThemeEditor` and every `library/*` sub-view. Only `Settings` has an `<h1>`. **Not one of `EmptyState`'s ~15 call sites passes an action**, though the component styles a `:global(.r-btn)` in its slot because it was built expecting one |
| **R3-11** safe mode explains itself on two of four fire controls | P3 | The VerseDeck list row's label still promises *"Put John 3:16 on the screens"*, which it cannot |
| **R3-12** Live is the only major view that never says the engine is missing | P3 | Eighteen controls disable on `!$capture.available`; Channels, Planner and History each show a rose *"Backend not attached"* badge; `Live.svelte` shows nothing and `App.svelte` has no global banner |
| **R6-Q** two run surfaces, one keyboard | P3 | **New.** `registerContext` is called by `Live.svelte` and nothing else. The Library run rail has the microphone, both panic tiles, rehearsal, the queue and Go Live — and **no `→`, no `←`, no `A`, no `D`**. The cheatsheet correctly hides them, so the app is honest; but the surfaces an operator can run a service from were never enumerated as a set, and only one of them got the transport. Related P4: `registerContext` is a **single slot, not a stack** — its unregister closure unconditionally sets `ctx = {}`, so if a second view ever registers, the first one's teardown will silently disarm it |
| **R3-13** `.r-badge.pulse` animates forever | P4 | `app.css:320`, no `prefers-reduced-motion` rule |

**Passes worth keeping:** `ErrorState` refuses *Try again* for `kind:'io'` and offers it for
`kind:'busy'` · `Loading` announces and `EmptyState` deliberately does not · the paraphrase
no-percentage rule holds on all four claim-rendering surfaces · every `role="button"` div answers
both Enter and Space with `preventDefault` **except** the VerseDeck list row (P1-5) · amber is
never borrowed by an error or a rehearsal surface (`npx vitest run src/lib/r6-contracts.test.js -t "R6-G3"`).

---

## 16. BLOCKED — the human test script

**This is the real output of auditing a desktop app from a machine that cannot see or hear it.**
Four agents each supplied a list. This is one ordered script, merged and sequenced by *what you
have to physically set up*, because you plug the ATEM in once.

Bring: the laptop a church would actually use (not a dev machine), a projector or second monitor
with an HDMI cable, a phone on the same wifi, a USB microphone, a pair of headphones, thirty
minutes of recorded preaching if you have it, and — for §D — an ATEM or OBS.

---

### Stage A — before you plug anything in (30 min)

**A1. Build the thing a church installs.**
`npm run tauri build`. Then `./scripts/sign-local.sh`.
*Expect:* the script passes. If it fails on the entitlement or the usage string, stop — the
microphone will be TCC-killed on the first correctly-signed build and nothing else in this script
will work. This is architecture rule 17 and **no instrument in this audit exercised it.**

**A2. Confirm the CSP, which only exists in a packaged build.**
Launch the packaged binary. Open the webview inspector. Fire a verse.
*Expect:* no CSP violations in the console. `tauri dev` does not exercise the CSP at all, so this
is the only time anyone will find out.

**A3. Delete the app-data directory and launch cold.**
`rm -rf ~/Library/Application\ Support/com.relay.app` then launch.
*Expect:* the first-run wizard, six steps, and a working install at the end. Then: does the
Templates tab say *"No templates yet"* before the query lands, over 31 seeded templates? (R3-04.)

**A4. Corrupt the session and relaunch.**
Write garbage into the session key in Web Storage. Relaunch.
*Expect (R1-04):* the six-step wizard returns on **every** launch from now on, and `planId`,
`liveCueId` and `serviceId` are gone. The fallback is written back over the bad bytes, so it never
self-heals.

**A5. Break the STT model and try to recover.**
Truncate the `.bin` in `app-data/models/` to 1 KB. Launch.
*Expect (R1-02):* the UI offers "Use this one", `select_stt_model` returns false, and re-download
is an instant no-op. There is no delete, no re-verify, no replace. **Confirm you cannot get out of
this from inside the app.**

**A6. The model download's three headline claims (R1-13).**
Start a fresh download. (i) Kill the network at 40% and resume — does it send a `Range` header,
append rather than truncate, and does the resumed file hash correctly? (ii) Corrupt the partial
file and resume — is the mismatch detected, is the file deleted, is the operator told? (iii) Press
Cancel while the TCP connection is half-open — **this is the case the whole design exists for and
it is tested nowhere.** (iv) Kill the network and watch whether the error message appears at all
(R1-09 — the event and the rejection race, and the rejection is discarded).

---

### Stage B — plug in the projector (45 min). **This is the most valuable half-hour in the script.**

**B1. Confirm the P0 yourself.** Start listening. Say, in a normal voice, at a normal distance:

> "Good morning. Please turn to hymn number three sixteen."
> "The youth are meeting in room two twelve after the service."

*Expect, and this is the finding:* **Numbers 3:16 and Romans 2:12 appear on the projector.** No
key was pressed. If they do not appear, the acoustic path is losing something and that is *also*
worth knowing — tell the team, because it changes the severity.

**B2. Then say a real reference** — "let us read from John chapter three verse sixteen" — and
confirm John 3:16 reaches the wall, correctly templated, at the right size, legible from the back
of the room. Nobody in this audit has seen a rendered verse.

**B3. Blackout, on every screen at once.** With John 3:16 on the wall *and* the stage page open on
your phone (`http://<host>:8032/stage.html`), press `B`.
*Expect (P1-1):* the projector goes black. **The phone keeps the verse.** The console says
"Blacked out."

**B4. Escape from a dropdown.** With a verse on the wall, open the **Countdown** menu in the
Library run rail. Press `Esc`.
*Expect (P1-3):* the wall clears and the menu is still open.

**B5. Escape from the Announcements editor.** Open it, put the cursor in a text field, press `Esc`.
*Expect (P1-4):* nothing happens at all. You have no panic key.

**B6. Space from the verse deck.** Tab to a verse in the Library **list** view (not grid). Press
Space.
*Expect (P1-5):* the verse goes live.

**B7. Amber that lies.** Fire a verse. Then fire a picture, or start a countdown, or push an
announcement.
*Expect (P1-6):* the amber "On Air" pill still describes the verse.

**B8. Pull the network cable from the kiosk screen.** Point OBS or a second browser at
`http://<host>:8032/output.html?channel=<id>`. Fire a verse. Disconnect the network for five
seconds. Reconnect.
*Expect (R6-I, the disagreement in §12):* the screen reconnects and stays **blank** until you fire
something else, while the console still shows the verse as live. Confirm which of R5 and R6 is
describing the more important property.

**B9. Plug the projector in *after* Relay is running.** Then look for a way to open an output
window on it.
*Expect:* the "Open" button on the Outputs tab works. (I checked this statically and it does —
`open_channel_output` is wired. Verify it, because `auto_open_outputs` runs only at launch and
only in `onMount`, which **no test in this repo can observe**.)

**B10. Multi-monitor, resize, DPI (R3, section E).** Drag the console between a retina and a
non-retina display. Resize to a 13" laptop width. Turn on full-screen Live control and confirm the
exit button is always reachable — `Esc` deliberately does not exit it.

---

### Stage C — the microphone and the languages (60 min, and it needs a person)

**C1. WER has never been measured, in any language.** Record thirty minutes of real preaching.
This is the single most valuable contribution anyone can make to this product.

**C2. The quiet-preacher case.** `RELAY_BENCH_WAV=… cargo test audio::gate stt::bench -- --ignored --nocapture`
at several `RELAY_BENCH_SCALE` values. Architecture rule 12 exists because three individually
reasonable thresholds made Relay 94% voiced at studio level and **2% at a church-laptop level**.

**C3. Code-switching, out loud.** A Yorùbá, a Swahili and a Hausa speaker, each reading a
reference in their own language mid-English-sentence. Detection over *text* works — I confirmed
`Yohana 3:16`, `Zaburi 23:1`, `Ìwé Jóhánù 3:16`, `Yahaya 3:16`, `Mwanzo 1:1` all reach the wall
(`cargo test r6_8 -- --nocapture`). **Whether Whisper produces those strings from those voices is
completely unknown.**

**C4. Yorùbá numerals are not parsed** — a reference spoken *entirely* in Yorùbá produces nothing
at all. Confirm this out loud with a native speaker, so the team hears what it sounds like.

**C5. The spoken panic command does not exist in any tier-1 language** (R4-G). Say "clear the
screen" in Swahili and confirm nothing happens.

**C6. No native speaker has reviewed the alias table.** 66 books × 3 languages, 71 Swahili / 133
Yorùbá / 72 Hausa aliases. Get three people to read them.

**C7. Two references in one breath.** Say "turn with me to John three sixteen and also Romans
eight twenty eight."
*Expect (R4-E):* both fire; **which one is left on the wall is decided by hash order** and changes
between runs.

---

### Stage D — the hardware nobody could touch (as long as it takes)

**D1. OBS.** Add a browser source at `http://<host>:8032/output.html?channel=<id>`. Confirm a
transparent template keys out. Change the channel's template in Relay and confirm the source
live-swaps without re-copying the URL (DECISIONS §29).

**D2. ATEM.** Bridge via HDMI. Confirm the tally and the key behave.

**D3. ProPresenter.** Import a real `.pro` playlist, not a synthetic one. Then import a malformed
one and a large one — there is **no size cap and no entry count** anywhere (R5-7).

**D4. NDI.** Confirm `open_ndi_output` returns a clear error naming the SDK. **BLOCKED BY DESIGN,
not broken.** Do not file it.

**D5. The updater.** Tag a build, publish `latest.json`, and confirm an existing install actually
updates. Nobody has ever run this. Architecture rule 19 is that a version drift makes *nothing
ever update, silently*.

**D6. Windows.** Everything above, on Windows. Architecture rule 9 exists because a macOS-only
`$HOME` path meant packaged Windows never found the STT model and ran with **speech recognition
silently dead**. CI compiles on Windows; this audit ran only on macOS.

---

### Stage E — the things you can only see with your eyes

**E1. Colour at booth distance, in the dark.** Amber vs amethyst vs grey vs cyan on the actual
console screen, from where the operator actually sits.

**E2. The same, through a deuteranopia filter.** Amber-on-air and red-error are the pair that
matters. A red/green booth light is unreadable to a colour-blind volunteer in the dark.

**E3. A screen reader through one whole service.** Start at the transport bar, which is the only
place with `aria-live` that matters, and then try the Library run rail, which has none (R3-08).

**E4. The imported-template beacon (P1-9).** Craft a template whose layer background is
`url("http://<a-box-you-control>/beacon.png")`, import it, fire a verse, and watch your web server
log. Then take the machine offline and fire it again — confirm the background is blank.

**E5. The CSRF (R5-6, P2).** From a phone on the church guest wifi, load a page containing
`<img src="http://<relay-ip>:8032/api/black">`. Confirm the wall blacks out.

### Stage F — real-time speech and detection latency (added 2026-08-24)

> **Added, not substituted.** Nothing above is withdrawn or rewritten. This stage exists because
> the complaint it answers — "the transcript arrives after the words" — was made *after* this
> audit, and was invisible to every instrument in it. The engineering behind it is
> `docs/DECISIONS.md` §38; what was and was not measured on a development machine is
> `docs/audits/PERF.md`. **Everything here still needs a room, a person and the
> packaged build**, which is why it is in §16 and not in §14.
>
> **Read the numbers off the app, not off a stopwatch.** Settings → Diagnostics → Live latency
> reports every span below, measured on the machine in the room, including the legs a stopwatch
> cannot see (the IPC bridge, the LAN, the projector's own paint). Press **Start a fresh
> measurement** at the top of each test. A stopwatch is still worth holding for F1 and F14 as a
> sanity check on the instrument — if the two disagree, the instrument is the finding.

**F1. Immediate transcript response — UNKNOWN.** Natural conversational speech, at least 20
samples. Expect the transcript to begin appearing while the sentence is still being spoken.
Report median, P95 and worst audio-to-visible, updates per second, and whether it grew.
*Target: P50 ≤ 300 ms, P95 ≤ 700 ms. Met by `ggml-base` on a development machine; unverified on
church hardware and unverified with a webview in the path.*

**F2. Continuous dictation — UNKNOWN.** Five minutes of speech with no deliberate pauses. The
per-minute series in Diagnostics is the answer; a rising line is the finding whatever the median
says.

**F3. Partial transcript correction — UNKNOWN.** Speak phrases the decoder will revise. Expect no
duplicated phrases, no permanent fragments, no repeated blocks, no flicker from rebuilding the
whole panel. *This is a claim about the RENDERER, and it has no automated coverage: nothing in the
suite watches a partial being replaced on screen.*

**F4. Scripture detection during speech — UNKNOWN.** "Let us read from John chapter three verse
sixteen", no deliberate pause. Expect recognition to begin mid-sentence and the fire to follow one
corroborating pass later — **not** the end of the utterance.

**F5. Scripture latency measurement — UNKNOWN.** 20 references, different books. Read the four
spans separately from Diagnostics. **A single total is not an acceptable report** — the previous
field test produced one healthy number and drew the wrong conclusion from it.

**F6. Ordinary speech safety — FIXED, REGRESSION TEST.** "Please turn to hymn number three
sixteen." "The youth are meeting in room two twelve." Nothing may reach the wall.
*Pinned: `e2e::ordinary_church_announcements_reach_nobody`.*

**F7. Parser regression — FIXED, REGRESSION TEST.** "1 Corinthians chapter 9 and verse 24" must
be `9:24`, never `9:1`. *Pinned in `e2e.rs`.*

**F8. Two references in one window — FIXED, REGRESSION TEST.** One verse reaches the wall, the
rest are offered. *Pinned: `e2e::two_references_in_one_window_put_one_verse_on_the_wall` and
`detection::r4_07`.*

**F9. Quiet speaker — UNKNOWN.** Falling volume. Report transcript latency *and* voice-gate
behaviour; the gate is learned, not absolute (§19), and this is the test that proved it before.

**F10. Church background noise — UNKNOWN.** Congregation movement, low conversation, room tone.
Responsive without false transcript activity or unsafe detections.

**F11. Long-service stability — UNKNOWN, and the highest-value item in this stage.** A full
service. Latency must not grow; memory, queues and the shed-partial counter must stay flat.
*The cadence change in §38 means a model slower than one chunker hop now runs the decoder
continuously for the whole service. Thermal throttling would show up here, as a rising per-minute
line, and nothing short of a real service can produce it.*

**F12. CPU, GPU and memory — UNKNOWN.** Sample during continuous transcription. Record queue depth
and shed partials from Diagnostics alongside Activity Monitor.

**F13. Network and output latency — UNKNOWN.** `fire sent → output painted`, on the real LAN, to a
real browser source. Diagnostics reports it directly (a kiosk page marks its own paint back over
the hub). Expect a small, predictable share of the total.

**F14. Complete end-to-end — UNKNOWN. This is the acceptance test.** Packaged build, real
microphone, real laptop, real church network, projector, preaching distance, room acoustics. 20
samples of spoken word → transcript visible, and spoken reference → scripture visible.
*Target: ≤ 1 s typical, P95 < 1.5 s.* If the hardware cannot hold it, **record the measured limit
and the stage that owns it** — do not restate the target as met.

---

## 17. Priority order

**Before anything else — the release blockers:**

1. **P0-1** — ordinary church speech auto-fires the wrong verse. Nothing ships past this.
2. **P1-10** — make `emit_detections` and `confirm_detection` generic over `R`, and write the
   first e2e test of the auto-fire path. **Do this second, not later**, because it is the
   instrument that would have caught P0-1 and is the only thing that will catch the next one.
3. **P1-11** — one line in `vitest.config.js`. Then re-run every layer-B finding in this document.
4. **P1-3 + P1-4** — Escape. One change in `shortcuts.js` covers seven of the eight doors; the
   Announcements `role="dialog"` is the eighth and is a separate, one-line, opposite fix.
5. **P1-1** — the blackout that misses a screen. Write the message-kind contract test first.
6. **P1-6** — amber that lies, on five paths.
7. **P1-7** — `confirm_detection`'s two silent successes.
8. **P1-8** — rewrite `DECISIONS.md`, `PRIVACY.md` and `SECURITY.md`. This is a documentation
   change and it is faster than everything above it.

**Then, and they are cheap:**

9. **P1-2** — decide what Preview *is* in the Library, and either wire it or remove the whole
   column. Do not fix the badge again.
10. **P1-5** — one `preventDefault` in `VerseDeck`.
11. **P1-9** — validate at the import boundary, and send a CSP header from `:8032`.

**Then the P2 cluster that shares one root:** R3-04, R3-06/R6-5, R3-07, R3-08 are all "the error
was captured and then thrown away or shown raw". Fixing `capture.js`'s GROUP 2 wrappers to
distinguish failure from emptiness closes most of it at once.

**Then P2 detection:** R4-C, R4-D, R4-E, R4-F, R4-H — and note R4-H is genuinely surprising: the
confirm arm of the self-calibrating gate **never fires**, because `confirm_detection` re-parses a
canonical `"Book C:V"` string which always scores 0.96, above the 0.50–0.90 window where a
correction happens. Every confirm in the product is pure decay toward baseline.

**Then everything else, in severity order.**

---

## 18. Completeness — which modality was never run, and where nobody looked

This section is the next round of work.

### Never run at all

| Modality | Consequence |
|---|---|
| **Layer D** (a running app, `:8032`, `:8031` over a socket) | Disabled by the run configuration. Everything about the LAN surface is reasoned, not driven. R2 called `remote_api` in-process, which is the closest anyone got |
| **Layer E** (a human) | Impossible here. §16 |
| **Audio, at any point** | The input half of the product |
| **A packaged build** | CSP, notarization, entitlements, updater, installer |
| **Windows** | CI compiles it; nobody ran it |
| **`npm run tauri build`** | Not run once in this audit, by anyone. R5 reasons about the CSP; no instrument exercised it |
| **`./scripts/sign-local.sh`** | The macOS microphone trap (rule 17) is untested this run |

### Claims that were never verified

- **CLAUDE.md: "every one of the 114 registered commands has a frontend caller."** False — eight
  do not (§6). Adjudicated in R3's favour.
- **CLAUDE.md: "Shipping: … ProPresenter import."** `import_pro` reaches no component.
- **`fuzzy_book`'s doc comment: "costs confidence downstream."** Measured: 0.06 against a 0.34
  margin. Not a mitigation.
- **`channels.rs:458–477`: "the choke point is FOUR functions."** R2 counted three ungated kiosk
  publishers, not one (`set_channel_template`, `KioskHub::set_template`, `KioskHub::set_themes`),
  and its justification — *"it carries a template, not content"* — is false in premise, because
  `layers.js::boundValue` has `case 'static': return layer.text`, so a template can carry literal
  text a congregation reads. I add a **fifth door**: `/api/live` is a *pull*, not a publisher, and
  nobody enumerated pulls at all (`cargo test r6_2`).
- **`telemetry.rs`: "an allow-list."** It is a blocklist (R5-5).
- **`plan.js:117`: a null `cueId` "is the case after the panic keys have cleared the screens."**
  It is not; `leavePlan` preserves `cueId`.
- **`session.test.js:28–36`** is titled *"a corrupt session must not strand the operator in a
  permanent wizard"* and asserts `setupDone === false`, which **is** the strand. The test's name
  states a guarantee its assertion contradicts (R1-04).

### Twins nobody checked

| Guarantee | Door that was checked | Door that was not |
|---|---|---|
| Rehearsal containment | `broadcast_content`, `clear`, `black`, `stage_next` — four **push** doors | **`/api/live`, a pull door.** `cargo test r6_2` |
| "What is live" | the console | the preacher's phone. `cargo test r6_1` |
| Panic messages honoured | `Output.svelte` | **`Stage.svelte`.** P1-1 |
| Escape suppressed by a modal | `role="dialog"` | `role="menu"`, `role="alertdialog"`, and a non-modal *claiming* `role="dialog"`. P1-3, P1-4 |
| The fire path is generic over `R` | every human path | **the auto path.** P1-10 |
| The transport is keyboard-reachable | `Live.svelte` | the Library run rail. R6-Q |
| A template's CSS is constrained by a CSP | the console | **the LAN output page has no CSP header at all.** P1-9 |
| Errors are humanised | Channels (fixed, correctly) | Themes ×4, ImportReview, History. R6-5 |

### Nobody looked here at all

1. **Non-sermon church speech.** R4 attacked the *sermon*. Nobody tested the welcome, the notices,
   the offering, the children's talk, the banns, the prayer list — which is most of a service, is
   when the microphone is on, and is when nobody is watching the wall. **This is where the P0
   lives**, and it was found by asking "what else does a person say in this room?"
2. **The abbreviation table as an ASR surface.** `song`, `acts`, `mic`, `num`, `rom`, `dan`, `lam`
   were added for *typing* and are used for *listening*. Nobody separated the two audiences.
3. **The asymmetry of the SSR stub.** Four agents found `onMount`. Nobody found that `onDestroy`
   is real, which is what makes a teardown assertion able to false-pass.
4. **A kiosk reconnecting.** Read as a PASS by R5 and as a defect by me; never driven by anyone
   (§12).
5. **The set of run surfaces.** There are two. One has a keyboard (R6-Q).
6. **`registerContext` as a single slot.** Latent today, because only one caller exists.
7. **Whether CI is green.** It is not, deliberately — 16 red tests. Somebody has to triage them
   before any PR can merge.
8. **`docs/PRODUCT_AUDIT.md` against this run.** Out of scope for an agent to edit, and its claims
   now differ from this document in at least the "114 commands" line and the ProPresenter line.

### The orchestrator's two errors, audited

**(a) R5 was told the LAN no-auth control plane was a decided position; the record says
otherwise.** The brief was wrong and R5 was right to check. But the *instruction* was right: the
preamble says "your job is not to report it — it is to check the decision still holds", and that
is precisely the instruction that produced P1-8. **This is not a process failure; it is the
process working.** An agent told "this is settled" should still read the settlement, and R5 did.
The only correction needed is to the brief's confidence, not to its structure.

**(b) The orchestrator shipped a product-code fix and 17 tests for the Preview/Take surface,
having verified that something renders the component but not that anything can supply its input.**
This one *is* a process failure and it is the most instructive event in the run.
`liveoutputrail.test.js` has 17 green tests. All 17 supply the `preview` prop themselves. The
shipping app cannot. So a real bug (F1, amber-while-staging) was diagnosed, fixed, tested and
merged in a branch of code no operator can ever reach — and the green suite made it *look* more
verified than the unfixed code was.

Three things follow, and they are the standing recommendations of this audit:

- **F1's severity should be revised to "not a live defect."** The amber badge could not have lied,
  because `preview` is never truthy.
- **A mount test that supplies a prop must be accompanied by a test that something in the app
  supplies it.** That is a one-line static assertion and it would have caught F2, P1-2 and P1-10.
- **Fixing during an audit is how an auditor stops auditing.** The preamble forbids it for
  exactly this reason, and the orchestrator is subject to the same rule as the agents.

---

## 19. What R6 left in the tree

| File | What it is | Command |
|---|---|---|
| `src-tauri/src/r6.rs` | 14 layer-A tests. **5 red on purpose**: `r6_1`, `r6_2` (the LAN pull door), `r6_9`, `r6_11`, `r6_12` (P0-1). The rest are green evidence: the seven-route surface, hostile query handling, the auto-fire cap under NaN and infinity, the single baseline, the code-switching record, the fuzzy-penalty measurement | `cd src-tauri && cargo test r6::` |
| `src/lib/r6-contracts.test.js` | 9 layer-C contract tests. **4 red on purpose**: R6-3 (blackout twin), R6-4 (Escape/menus), R6-5 (raw errors), R6-6 (Preview has no producer). Plus three green ones (offline surface, `{@html}` allow-list, amber reserved) and one note pinning the Channels false-positive | `npx vitest run src/lib/r6-contracts.test.js` |
| `src/lib/r6-lifecycle-probe.test.js` + `src/lib/__r6probe.svelte` | Instrument calibration for P1-11. **Green today; will go red the moment somebody fixes `vitest.config.js`**, which is the signal to re-grade every layer-B finding here | `npx vitest run src/lib/r6-lifecycle-probe.test.js` |
| `src-tauri/src/main.rs` | One `#[cfg(test)] mod r6;` declaration. **No product code was changed by R6.** | `git diff src-tauri/src/main.rs` |

`cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` are clean.

---

## 20. Release decision

> ## NO-GO
>
> **Relay's engine is genuinely good, and it currently puts the wrong Bible verse on the
> congregation's wall, by itself, when somebody says "please turn to hymn number three sixteen".**

One open P0. Eleven open P1s. And — the condition that would block a GO even with all twelve
closed — **roughly half of this product, as a volunteer experiences it, was unreachable by every
instrument in this run.** No audio was heard, no pixel was seen, no hardware was attached, no
packaged build was made, and no component's `onMount` ever ran.

A GO on this build would rest on the sentence the brief forbids: *we read the source and it
looked right.*

**The path to a GO:** close P0-1 and the eleven P1s in the order at §17, then run §16 — all five
stages, on real hardware, with a real person and a real microphone — and re-audit against what
that finds. §16 is not a formality after the fixes; **it is the half of the audit that has not
happened yet.**

---

*R6 · Independent Auditor. Layers A, B and C. Layer D disabled; layer E impossible.
No product code was changed. `docs/PRODUCT_AUDIT.md` was not touched.*

---
