# Relay — Product Audit

**Revision 3 · 2026-07-13 · verified against `cfa2aa5`**
Supersedes Revision 2 (2026-07-12) and Revision 1 (2026-07-05). Every claim was re-verified against the code that exists today; nothing is carried forward on trust. Line references are live.

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

**What that leaves is the honest part.** Nothing on the list below can be fixed by writing code, which is why it is still here.

---

## 0b. What changed since Revision 1

Revision 1 named three critical blockers and called them "one epic". **That epic shipped.** So did most of Phase 2. Retiring stale findings is as much a part of an audit's job as raising new ones, so, explicitly:

| Revision 1 said | Reality today |
|---|---|
| 🔴 C1 — no in-app model download; user must run `curl` | **SHIPPED.** `models.rs:140` — resumable, SHA-256 verified, atomic `.part` rename, progress bar, cancellable. Reachable from first run, Settings, *and* a Live banner. |
| 🔴 C2 — no code signing | **HALF-SHIPPED.** macOS signing + notarization are wired (`release.yml:114-163`). **Windows is not signed at all** — see 🔴 D1. |
| 🔴 C3 — no auto-updater | **SHIPPED — and inert.** Plugin, pubkey, launch-check and never-during-a-service gating all exist. It cannot currently deliver an update — see 🔴 D2. |
| First-run wizard missing | **SHIPPED.** `FirstRun.svelte` — screen → microphone → fire a verse. |
| Console + Planner must merge | **SHIPPED, properly.** `Console.svelte` is gone; `ServicePlanner` imports *zero* fire commands (`ServicePlanner.svelte:19-38`). Live is the run surface. Planner cannot reach a screen. |
| Rehearsal mode | **SHIPPED**, gated at the broadcast choke point (DECISIONS §18). |
| No in-app Help | **SHIPPED**, and it is now *better than the written user guide*. |
| "ON AIR" reports the microphone, not the screen | **FIXED.** `App.svelte:169-186` keys off `$live` (backend output state). Mic has its own quieter indicator. |
| Cheatsheet lists dead keys | **FIXED.** `shortcuts.js:93` derives it from what the mounted view actually registered. |
| `--v-faint` fails WCAG AA at 3.4:1 | **FIXED.** Now `#88888d` — 4.55–5.61:1 across every surface it sits on. All pass. |
| PRIVACY.md / SECURITY.md / AI disclosure missing | **SHIPPED, all three.** PRIVACY.md is the best document in the repo and it discloses the unauthenticated LAN broadcast honestly. |
| Five data-integrity gaps | **ALL FIVE FIXED.** `reimport_full_kjv` is transactional (`db/verses.rs:318`); `import_song` is transactional (`db/songs.rs:238`); `delete_media` cascades to plan cues (`db/library.rs:266`); `move_plan_item` finds neighbours by ordering, not `position ± 1` (`db/plans.rs:202`); the Lower-Third forward-fill is id-scoped (`db/mod.rs:115`). |
| No way to measure detection accuracy | **SHIPPED.** `eval.rs` + a 50-case labelled corpus, **scored through the real router**, CI-gated to fail the build above SPEC's 5% wrong-verse rate. |

That is an unusually complete execution of an audit. The scorecard moves accordingly.

**But the new findings are a different species, and a more dangerous one.** Revision 1's bugs were *visible*: a button that didn't work, text you couldn't read. The bugs found today are bugs of **false confidence** — a toast that says "Screens cleared" when the clear failed, a cheatsheet that teaches a panic key that doesn't work while typing, an updater that will silently never update, a signing pipeline that publishes an unsigned Windows installer and prints no warning. Software that lies to its operator is worse than software that fails in front of them, because the operator stops looking.

---

## 1. Executive Assessment

**The code is done. Relay is now blocked on four things, and not one of them is a commit.**

Revision 1's finding was *"the engineering is ahead of the product"* — the app could not be installed. Revision 2's was *"the product does not tell the operator the truth about itself"* — it reported successes it had not achieved. Both are now closed, and the second was the harder and more valuable of the two.

What that produced is a product with an unusual property for its stage: **its failure modes are visible.** A clear that fails says so. A `→` that cannot move says why it cannot. A paraphrase guess cannot masquerade as a heard reference, because it is rendered as a different kind of claim with no percentage attached. A release that would ship unsigned refuses to build. A migration that dies halfway can be retried. These are not features; they are the absence of a specific class of lie, and in software that fails **live, in front of five hundred people**, that class of lie is the whole danger.

The engine underneath is strong and now genuinely covered: **246 Rust + 138 frontend tests**, zero panic sites in any module that runs during a service, a detection benchmark that fails CI on regression, an end-to-end test that drives the real fire → nav → clear path, and a gate that makes "the AI put the wrong verse on the wall" structurally unrepresentable rather than merely unlikely.

**So the honest position is now a shopping list, not an engineering plan:**

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
| **Architecture** | **8 / 10** | ▲ +2 | The fire engine is now generic over `tauri::Runtime`, so the path that puts scripture on a wall can be driven **without a window** — which is the useful half of "split `main.rs`". Typed errors (`error.rs`) replace 88 × `Result<_, String>`. `main.rs` is still 2,922 lines / 101 commands, but it is no longer untestable, which was the actual problem. |
| **Performance** | **9 / 10** | — | Unchanged. Measure-before-optimising is practised here, not preached: the semantic scan stays a linear scan and beam search stays unused, both because measurement said so. |
| **Accessibility** | **8 / 10** | ▲ **+4** | Focus traps on all 5 dialogs, **with focus restore** (the half everyone forgets). A real heading structure. The AI suggestion feed, the transport and errors are all announced — the product's whole reason to exist used to arrive in total silence. Every text token passes WCAG AA. Not 10/10: ~150 lines of dead legacy CSS remain, deliberately (see §7). |
| **Security** | **8 / 10** | ▲ +1 | A tag name is no longer interpolated into a release shell (a real injection vector CodeRabbit caught). LAN bind is unauthenticated, broadcast-only, bounded, and honestly documented. Unsigned Windows remains the exposure — but the pipeline now *refuses* to produce it rather than doing so quietly. |
| **Privacy** | **9 / 10** | — | Unchanged, and still the strongest part of the product. Telemetry off by default, no DSN in OSS builds, free text *dropped* not sifted. |
| **Testing** | **9 / 10** | ▲ **+3** | **246 Rust + 138 frontend.** The gap was never the count — it was that `main.rs` had zero tests and no e2e existed, so the fire → nav → clear path was verified only by hand. `e2e.rs` now drives the real commands against a real DB. And the culture shifted: several fixes were **mutation-verified** — the test was checked to *fail* when the original bug was reintroduced. Two tests in this repo initially passed on broken code; both were caught that way. |
| **Developer experience** | **8 / 10** | ▲ +1 | CI, CodeRabbit, `clippy -D warnings`, an exemplary decision log now 25 rules deep. `scripts/version.mjs` makes releasing a one-liner. Still no CONTRIBUTING/CoC/templates (§14). |
| **AI readiness** | **6 / 10** | ▲ +1 | The operator can finally *see* what kind of claim the AI is making, and `related_scripture` — built, tested, and called by nothing for months — is surfaced. The gate remains excellent. But paraphrase is still TF-IDF, `verses.embedding` has still never been written to, and **the acoustic layer is still unmeasured**. Blocked on audio, not on code. |
| **Brand** | **4 / 10** | — | Unchanged and now the weakest column. Still no logo, no tagline, no positioning line. README still says *"Working name — rename freely."* |
| **Business model** | **N/A** | — | Deliberately free/MIT. Sustainability parked, not decided. |
| **Documentation** | **7 / 10** | ▲ +2 | CLAUDE.md, DECISIONS.md (§20–§25) and RELEASING.md are now current and unusually honest — each rule is a bug that reached, or would have reached, a congregation. **USER_GUIDE.md is still the weak point**: written for a developer, opens on `localhost:5032`, and never mentions the speech model. The in-app Help is better than the written guide. |
| **Legal compliance** | **8 / 10** | ▲ +2 | `LICENSE` names its holder. PRIVACY/SECURITY/AI_DISCLOSURE shipped and accurate. KJV-only with no import path for any other translation, so no exposure. WCAG now largely passes. Missing: CONTRIBUTING, CODE_OF_CONDUCT, CHANGELOG. |
| **Enterprise readiness** | **N/A** | — | Explicitly out of scope. See §13. |
| **Overall maturity** | **8 / 10** | ▲ **+1.5** | *Code-complete, and blocked on the world.* Every finding that a commit could close is closed. What remains needs a certificate, a billing page, a microphone in a real church, and people who speak Yorùbá. |

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

Revision 2 listed ten. Nine are fixed. What is left is real, and mostly cannot be fixed by typing.

1. **The moat is unmeasured, and that is now the single biggest weakness in the product.** No fine-tuned acoustic model ships, no native speaker has reviewed the 66×3 book aliases, Yorùbá numerals are not parsed, and **word error rate has never been measured in any language**. The tooling to measure it is *already built* (`stt.rs::bench` degrades audio to realistic church conditions and scores through the real detector) and it is dormant, because **there is no sermon audio in the repo**. Thirty minutes of tape is the cheapest, highest-leverage item in this entire document.
2. **Nobody has watched an update install.** The path is capable of it and the version can no longer drift — but "capable" is not "observed", and this is the mechanism by which every future fix reaches a church.
3. **Windows cannot ship.** By design: the gate refuses. It needs a ~$10/month certificate.
4. **`USER_GUIDE.md` is written for a developer.** It opens by explaining `localhost:5032`, still names a **Console** tab that no longer exists, and **never once mentions the speech model** — the first thing a new user must install. The in-app Help is better than it.
5. **The first-run wizard cannot be re-run.** An operator who skips it cannot get it back; everything in it lives in Settings, but they have to know that.
6. **~150 lines of dead legacy CSS remain**, including a colour that failed AA (now fixed in value, not removed). Deleting it needs eyes on a running app — Svelte does not scope a global stylesheet, and those rules use generic class names (`.tab`, `.dot`, `.live`) that live components still carry. See §7.
7. **`main.rs` is still 2,922 lines and 101 commands.** No longer *untestable* — the fire engine is runtime-generic and covered by `e2e.rs` — but still a single file holding both the IPC surface and the live engine.
8. **No CONTRIBUTING, no CODE_OF_CONDUCT, no CHANGELOG**, on a project whose docs actively solicit pull requests — and which now, with the i18n layer, has a genuinely low-friction way for non-programmers to contribute.
9. **Brand is untouched.** Still no logo, no tagline, no positioning line, and a README that says *"Working name — rename freely."*

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

1. **Make the live commands able to fail.** 14 commands return no `Result` — `nav`, `clear_screens`, `blackout`, `set_stage_next`. `handle_nav` (`main.rs:588`) has three silent bail-outs and *discards* `fire_manual`'s return `bool` at `:600`. An operator presses **Next**, the wall doesn't change, and there is no error, no toast, no log. This is the same silent-no-op class we just fixed in `move_plan_item`, now living in the live nav path.
2. ~~**Split `main.rs`**~~ ✅ **Done differently, and better.** The stated goal was to lift the fire engine out of the IPC surface *so that it could be tested without a Tauri app handle* — and the split was only ever the means. The engine is now **generic over `tauri::Runtime`**, which achieves exactly that: `e2e.rs` drives the real commands headlessly against a real database. `main.rs` is still 2,922 lines, and that is now a readability complaint rather than a correctness one. **The split was a means; the test was the point, and the test exists.**
3. **Add one e2e test that drives a real service** — fire, nav, clear — against a headless build. There is no `tests/` dir, no driver, no integration test anywhere.
4. **Introduce a typed error.** 88 × `Result<_, String>` in `main.rs`. The frontend cannot distinguish *not found* from *DB locked* from *disk full*, which is exactly why it renders `String(err)` in monospace.
5. **Normalise the throw-vs-swallow contract in `capture.js`** (39 swallow sites; 1 `throw` in all of `src/`). The contract is *stated in a comment* and applied ad hoc — and it is not applied to the panic path.
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

- **The name.** README **still says** *"Working name — rename freely."* SPEC still says *"'Relay' is a placeholder product name."* "Relay" is generic, unsearchable, and already taken across broadcast and networking. Decide **before** the first church installs it, not after.
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
| **CONTRIBUTING.md** | ❌ **Missing** — and `LANGUAGES.md:53` actively solicits PRs (*"Edit, open a pull request, done"*). |
| **CODE_OF_CONDUCT.md** | ❌ Missing. |
| **Issue / PR templates** | ❌ Missing. `.github/` contains only the two workflows. |
| **CHANGELOG.md** | ❌ Missing — and now load-bearing, because an updater without release notes is an unexplained download. |
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
30. CONTRIBUTING / CoC / templates / CHANGELOG
31. NDI (only if a real church asks)

---

## 16. Production Readiness Checklist

Everything that a commit can tick is ticked. The four unticked boxes in the first list are the entire remaining distance between this repo and a church, and **none of them is code.**

**Blocking a first church:**
- [x] In-app model download *(resumable, checksummed, cancellable — and it can no longer hang or brick)*
- [x] Signed + notarized macOS build
- [x] **macOS microphone entitlement** *(without it, the first correctly-signed build is the first one that cannot hear the preacher)*
- [ ] 💳 **A Windows code-signing certificate** — *~$10/mo (Azure Trusted Signing). The gate now refuses to ship without it, so Windows cannot release at all until this is bought.*
- [x] Auto-updater *(version enforced against the tag in CI and at release)*
- [ ] 👁 **One update actually installed, end to end, on a real machine** — *the path is capable of it; nobody has watched it happen*
- [x] First-run wizard *(and its microphone meter now actually moves — it was dead)*
- [x] PRIVACY.md + SECURITY.md + AI_DISCLOSURE.md
- [x] The panic path cannot report a success it did not achieve
- [x] The transport cannot silently do nothing
- [ ] 📖 **An operator guide written for a volunteer** — *the in-app Help is; `USER_GUIDE.md` still opens on `localhost:5032` and never mentions the speech model*
- [ ] ⛪ **A real service run end-to-end by someone who is not the author** — *the only test that actually counts*

**Before public release:**
- [x] Method + `matched_text` visible live
- [x] `LICENSE` names a copyright holder
- [x] WCAG: focus traps + restore, operable controls, `<h1>`, every text token at AA
- [x] Crash reporting verified opt-in
- [x] Bible translation licensing confirmed (KJV only, no import path)
- [x] Typed errors, an e2e test, and a migration that can be retried
- [ ] Rename decided *(README still says "working name — rename freely")*
- [ ] CONTRIBUTING + CODE_OF_CONDUCT + CHANGELOG
- [ ] `USER_GUIDE.md` reconciled with the code (it still names a **Console** tab that does not exist)

**The moat — blocked on a microphone, not on a keyboard:**
- [ ] 🎙 **30 minutes of real sermon audio** — *the bench that consumes it is already built and dormant*
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
