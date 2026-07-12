# Relay — Product Audit

**Revision 2 · 2026-07-12 · verified against `9f14d10`**
Supersedes Revision 1 (2026-07-05). Every claim below was re-verified against the code that exists today; nothing was carried forward on trust. Line references are live.

**Scope, decided with the owner and unchanged:**
- **Strategy: unchanged.** Free, MIT, offline-first, no accounts, no server. The decisions in [DECISIONS.md](DECISIONS.md) stand.
- **Optimise for: the first 10 churches.** Not enterprise scale. The bar is *a volunteer, in a dark booth, with no training and no second take.*

Phases of the transformation brief that assume a commercial multi-tenant SaaS — billing, RBAC/SSO, multi-tenancy, audit logs, growth/monetisation, government/healthcare/finance readiness — are marked **NOT APPLICABLE** in §13, each with reasoning. They are not oversights. Adopting them would destroy the product's actual moat.

---

## 0. What changed since Revision 1

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

**Relay is now installable. It is not yet trustworthy in the one dimension that matters most: it does not reliably tell the operator the truth about itself.**

Revision 1's position was *"the engineering is ahead of the product."* That is no longer true, and it is a real achievement. A volunteer can now download the app, run a wizard, fetch the speech model with one button, and get a verse on a projector without ever opening a terminal. The engine underneath is genuinely strong: 221 Rust tests, zero panic sites in any module that runs during a service, a detection benchmark that fails CI if accuracy regresses, and a gate that makes "the AI put the wrong verse on the wall" structurally unrepresentable rather than merely unlikely.

The problem has moved. Three things are now true at once:

1. **The distribution pipeline has two silent, unguarded failure paths.** Tag a real release today and it publishes an **unsigned Windows installer** with no warning (`release.yml:101` gates on Apple secrets only; there is no `bundle.windows` block anywhere). And because the version is hard-coded to `0.1.0` in two files that nothing in CI reads or checks, a forgotten bump means `latest.json` advertises the new build *as the version everyone already has* — every install compares equal and **never updates, forever**. The updater exists precisely to prevent being unable to ship a fix. In its current state it guarantees it.

2. **The safety architecture is invisible at the moment of decision.** The entire correctness story rests on one distinction — a *direct reference heard* versus a *TF-IDF paraphrase guess* — and the operator, live, cannot see which one they are looking at. `Live.svelte:478` renders both as `AI suggestion — 92% match`. The `method` field is already in the IPC payload (`pipeline.rs:155`). The `matched_text` — the actual words that triggered the match — is captured in Rust (`detection.rs:779`) and never crosses the bridge. We built a careful gate and then hid it from the only person who can override it.

3. **The panic path reports success it did not achieve.** `Live.svelte:221-231` — `clearAll()` calls `clearScreens()`, which swallows its own errors internally, and then flashes **"Screens cleared"** unconditionally. If the clear failed, the operator is told the wall is clean while the verse is still on it. And pressing `Esc` to dismiss the *help overlay* wipes the congregation's screens as a side-effect (`shortcuts.js:115-121`), because there is no "is the cheatsheet open?" guard.

So the honest position is:

> **Relay is one careful week from being genuinely shippable, and that week is now about honesty, not features: sign Windows, derive the version from the tag, stop lying on the panic path, and show the operator which kind of match they are being offered.**

The competitive bet in DECISIONS.md still holds. But the moat needs restating truthfully, because the repo's own `LANGUAGES.md` does so and the audit should not soften it: **Relay's African-language differentiator today is a hand-curated multilingual reference-parsing table sitting on top of stock Whisper base — not African-language speech recognition.** That table is real, tested, and more valuable than it sounds (`LANGUAGES.md:22`: *"The moat was blocked on a lookup table, not on machine learning"*). But no fine-tuned acoustic model ships, no native speaker has reviewed the book names, Yoruba numerals are not parsed, and word error rate has never been measured in any language, because no sermon audio exists.

---

## 2. Product Scorecard

Scored against the stated bar (*first 10 churches*), not against Stripe. Δ is the move since Revision 1.

| Dimension | Score | Δ | Why |
|---|---|---|---|
| **Core engine** | **9 / 10** | — | Offline pipeline works end to end. Detection is DB-free and pure. **Zero `unwrap`/`expect`/`panic!` in any of the seven modules that run during a service** — verified against the real `#[cfg(test)]` boundaries, and the array indexing in `detection.rs` is bounds-guarded, not lucky. Lock discipline (Db before Session; never emit under a lock) holds everywhere it was checked. |
| **Distribution / install** | **6 / 10** | ▲ **+5** | Transformed. In-app model download, first-run wizard, macOS signing + notarization, updater plumbing. Held back from 8+ by two silent killers: unsigned Windows on a real tag, and a hard-coded version that strands every install. |
| **Onboarding / first-run** | **7 / 10** | ▲ +4 | Real wizard: screen → mic → *fire an actual verse as proof*. Ends with the user having seen the product work. Loses points because **the mic step's level meter is dead** (`FirstRun.svelte:152` — capture is never started, so the bar that "proves the microphone is hearing something" never moves), and the wizard cannot be re-run. |
| **UX (live operation)** | **6 / 10** | ▲ +1 | The merge is done and the transport is mode-aware and says so. But three separate controls now *lie*: the clear toast, the `B`-while-typing cheatsheet line, and `Esc`-dismisses-help-by-clearing-the-wall. In live software, a control that lies scores worse than a control that is missing. |
| **UI / design language** | **7 / 10** | — | Dark/amber broadcast language is correct and should not be "modernised". Amethyst-for-rehearsal is a genuinely good tally decision. Loses points for four competing empty-state classes and an English-only UI. |
| **Architecture** | **6 / 10** | ▼ −1 | The `db/` split by aggregate is good. But `main.rs` has *grown* to **2,807 lines and 101 commands**, holds the live-fire engine as well as the IPC surface, and has **zero tests**. There is no integration test anywhere. The one path that actually puts a verse on a wall is verified only by hand. |
| **Performance** | **9 / 10** | — | 26 MB install. Semantic scan measured at 2.6 ms/query and deliberately *not* optimised, because measurement said not to. Whisper decode measured at ~207 ms against a 1000 ms budget. Measure-before-optimising is practised here, not just preached. |
| **Accessibility** | **4 / 10** | ▲ +2 | `--v-faint` now passes AA. One real `aria-live` region exists (`App.svelte:105`) and it correctly keys off backend truth. But: **0 focus traps** across 3 dialogs, three `role="button"` divs that are focusable and **not operable** (no `on:keydown`), no `<h1>` anywhere in the shell, and the preacher's phone (`Stage.svelte`) never got the contrast fix — its standby text is **2.25:1**. |
| **Security** | **7 / 10** | — | CSP set and verified in a packaged build. LAN bind is unauthenticated, broadcast-only, bounded, *and now honestly documented in PRIVACY.md* — which is the correct handling of an accepted risk. Unsigned Windows binaries are the live exposure. |
| **Privacy** | **9 / 10** | ▲ +1 | PRIVACY.md now exists and is the best document in the repo. It discloses the LAN broadcast rather than hiding it. Telemetry is off by default, has **no DSN in the OSS build**, and *drops* free text rather than sifting it — with tests named `no_free_text_survives_at_all`. |
| **Testing** | **6 / 10** | ▼ −2 | 221 Rust + 59 vitest, both in CI on macOS *and* Windows, plus a CI-gated detection benchmark. Marked down because the count hid a hole: **`main.rs` has zero tests**, there is no `tests/` directory, no e2e, no driver. All 101 commands and the entire fire → nav → clear path are untested. |
| **Developer experience** | **7 / 10** | — | CI, CodeRabbit, `clippy -D warnings`, an exemplary decision log. Held back by the 2,807-line `main.rs` and by the total absence of contributor infrastructure (§14). |
| **AI readiness** | **5 / 10** | — | The *gate* is excellent and property-tested (`router.rs:397`: semantic can never auto-fire, at any score, at any sensitivity). Detection accuracy is now measurable and measured. But paraphrase is still TF-IDF, the `verses.embedding` column exists and has never been written to, and the African-language acoustic layer is unbuilt and unmeasured. |
| **Brand** | **4 / 10** | — | Still no logo, no tagline, no positioning line. README **still says "Working name — rename freely."** The in-app header still says **"Relay Console"** — a tab that no longer exists. |
| **Business model** | **N/A** | — | Deliberately free/MIT. Sustainability parked, not decided. Correct at this stage. |
| **Documentation** | **5 / 10** | — | ARCHITECTURE.md and DECISIONS.md remain excellent for engineers, and LANGUAGES.md is the most honest artifact in the repo. But the *operator* guide still opens by explaining `localhost:5032`, still says "the five screens" and lists six (there are seven), still names a **Console** tab that does not exist — and **never once mentions the speech model**. The in-app Help is now better than the written guide. |
| **Legal compliance** | **6 / 10** | ▲ +3 | PRIVACY, SECURITY, AI_DISCLOSURE all shipped and accurate. KJV-only, public domain, and there is **no import path for any other translation** — so there is no licensing exposure today. Two defects: **`LICENSE:3` still reads `Copyright (c) 2026 [Your name / organization]`**, and WCAG would still not pass. |
| **Enterprise readiness** | **N/A** | — | Explicitly out of scope. See §13. |
| **Overall maturity** | **6.5 / 10** | ▲ **+1.5** | *Shippable-pending-honesty.* The install problem is solved. The remaining work is making the product tell the truth about its own state. |

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

1. **The product tells the operator things that are not true.** (Panic toast, cheatsheet, `Esc`, the updater's silence.) This is now the defining weakness and §5 is entirely about it.
2. **The safety architecture is invisible.** The direct-vs-paraphrase distinction is the whole correctness story and it is not rendered live. Both are `AI suggestion — 92% match`.
3. **The moat is a lookup table, and the acoustic layer is unmeasured.** No fine-tune, no native-speaker review of the book names, no Yoruba numerals, no WER in any language, and **no sermon audio exists** to measure with. Thirty minutes of real recorded audio is the cheapest unblock in this document.
4. **`main.rs` is 2,807 lines, 101 commands, and 0 tests** — and it holds the live-fire engine, not just the IPC surface. The one path that puts a verse on a wall is hand-verified.
5. **14 commands return no `Result` at all** — and they are the *live* ones: `nav`, `clear_screens`, `blackout`, `set_stage_next`. `handle_nav` has three silent bail-outs and discards `fire_manual`'s `bool`. The operator presses **Next** mid-sermon, nothing changes, and there is no error, no toast, no log.
6. **39 error-swallowing `catch` sites in `capture.js` alone; exactly one `throw` in all of `src/`.** A contract is stated in a comment and applied ad hoc — and it is not applied to the panic path.
7. **Relay understands Yoruba but does not speak it.** No i18n layer of any kind. It listens to the preacher in Yoruba and talks to the volunteer in English.
8. **The preacher's phone was left behind.** `Stage.svelte` hardcodes hexes instead of tokens, including the exact pre-fix value the CSS comment documents as removed for failing AA. Its default resting state is **2.25:1**.
9. **Raw Rust error strings still reach volunteers.** `Channels.svelte:264` renders `String(err)` in a **monospace** font from five separate call sites. `Live` has a `humanError()` layer; nothing else does.
10. **A fully-built feature is wired to nothing.** `related_scripture` (19 themes, keyword-scored, a Tauri command, registered at `main.rs:330`) has **zero frontend callers**.

---

## 5. Critical Issues — the five that decide whether Relay survives contact with a church

### 🔴 D1 — A real release tag publishes an unsigned Windows installer, silently

The pre-flight gate (`release.yml:101`) sets `signed=true` on the presence of **`APPLE_CERTIFICATE` alone**. There is **no `bundle.windows` block in `tauri.conf.json`**, no `certificateThumbprint`, no `signCommand`, and no Windows cert-import step. The two `WINDOWS_CERTIFICATE*` env vars at `release.yml:169-170` are consumed by nothing.

Tag `v0.2.0` with all six Apple secrets set → the gate passes → macOS is signed and notarized → **the Windows `.msi` ships unsigned**, and the ⚠️ unsigned-build banner in the release notes (`release.yml:185`) is keyed on `signed == 'false'`, which is now `true`. Nothing tells the maintainer. Nothing tells the church. Windows is the target market's dominant platform on cost grounds (DECISIONS.md).

**Fix:** make the gate platform-aware and fail loud on a non-prerelease tag with no Windows certificate. Then buy the certificate (Azure Trusted Signing, ~$10/mo, no HSM needed).

### 🔴 D2 — The updater cannot deliver an update, and will not say so

Two independent faults, either one sufficient:

- **The version is hard-coded.** `tauri.conf.json:4` and `package.json:4` both say `0.1.0`. Nothing in `release.yml` derives the version from `github.ref_name`, and no CI check compares them. Tag `v0.2.0` without hand-editing both files and `latest.json` advertises the new artifacts *under version 0.1.0* — every installed client compares equal and **never updates**. Silently. Forever. `RELEASING.md:106` says "bump the version first" — in a comment, in a code block, enforced by nothing.
- **The endpoint cannot serve the builds we can currently make.** The updater points at `.../releases/latest/download/latest.json`, and GitHub's `/releases/latest/` resolves only to **non-draft, non-prerelease** releases. `release.yml:186` forces `releaseDraft: true` and `:187` forces unsigned builds to `prerelease: true`. So `RELEASING.md:234-236`'s claim that the updater "can be tested end to end today" is false — the artifacts are produced and the endpoint 404s.

**This is the exact failure the updater exists to prevent.** We fixed six screen-facing bugs and built the mechanism to ship them; the mechanism is currently a no-op.

**Fix:** derive the version from the tag in CI and assert it matches both config files. Publish (not draft) the release. Then actually perform an end-to-end update from an installed build before calling it done.

### 🔴 D3 — The panic path tells the operator it worked when it didn't

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

### 🔴 D4 — The safety architecture is invisible at the moment of decision

`pipeline.rs:155` already ships `method` (`"direct"` / `"semantic"`) across the IPC bridge. `Live.svelte:478-482` throws it away and renders every candidate identically:

> **AI suggestion** · John 3:16 · **92% match**

A 92% *heard reference* and a 92% *TF-IDF cosine against a bag of words* are not the same claim, are not on the same scale, and — per `detection.rs:29-31` and DECISIONS.md — **the second one is not a probability at all**. The operator is shown a number that means one thing for one kind of match and nothing for the other, with no way to tell them apart. The only place `method` is ever rendered is `History.svelte:119` — *after* the service.

And `matched_text` — the actual words that triggered the match, the clearest possible explanation of an AI decision — is captured at `detection.rs:779`, marked `#[allow(dead_code)]`, and **never leaves Rust**. It isn't even a field on `DetectionEvent`.

**Fix (cheapest high-value change in this document):** add `matched_text` to `DetectionEvent`, render the method as a distinct badge (amethyst for paraphrase — the rehearsal precedent is already set), show the matched words, and show confidence as a bar for `direct` and *no number at all* for `semantic`, because the number is not meaningful. Trust, not magic.

### 🔴 D5 — The model download hangs forever on a church's flaky wifi, and Cancel does nothing

`models.rs:184-187` builds a `reqwest::Client` with **no `timeout` and no `read_timeout`**, and the cancel flag is checked only *after* `stream.next().await` yields (`models.rs:219-222`). A half-open TCP connection — a dropped wifi, the single most likely real-world church-network event — means `stream.next()` never returns. Progress freezes at N%. No error is emitted. **Cancel is inert.** And `running` is never cleared (`models.rs:152` is unreachable), so every subsequent attempt returns *"A model download is already running"* **until the app is restarted**.

Adjacent: a `.part` file that is exactly `model.bytes` long (crashed on the final chunk) sends `Range: bytes=<len>-`, the server answers **416**, `models.rs:198` hard-errors, and the `.part` is never deleted — **permanently bricked** until the user finds and deletes a file they don't know exists.

**Fix:** set a read timeout, select on the cancel flag, clear `running` in a guard, and delete the `.part` on a 416.

---

### 🟠 One more, held just below the line because it cannot be seen until it happens

**There is no macOS microphone entitlement.** No `.entitlements`, no `Info.plist`, no `NSMicrophoneUsageDescription` anywhere under `src-tauri/`. Notarization *requires* the hardened runtime, and under the hardened runtime `cpal` opening the input device is TCC-killed without that entitlement. This will not reproduce in `tauri dev` and will not reproduce in an ad-hoc-signed pre-release. **The first correctly-signed, notarized macOS build — the one built specifically to hand to a church — is the first one where the microphone is dead.**

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
| **macOS mic entitlement** | **ADD — P0** | Invisible until the first notarized build. |
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
2. **Split `main.rs` (2,807 lines, 101 commands, 0 tests)** into `commands/{live,library,plans,output,settings}.rs`, and lift the fire engine (`resolve_fire`, `fire_manual`, `emit_detections`, `handle_nav`) out of the IPC surface entirely so it can be tested without a Tauri app handle. **The split is a means; the test is the point.**
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
| LICENSE (MIT) | ⚠️ **Present but defective.** `LICENSE:3` still reads `Copyright (c) 2026 [Your name / organization]`. An MIT grant with no named licensor. **One-line fix; do it today.** |
| PRIVACY.md | ✅ **Shipped, and excellent.** Accurate against `telemetry.rs` and `channels.rs`. Crucially, it **discloses the unauthenticated LAN broadcast** (`PRIVACY.md:74-89`) rather than hiding it, and flags the café-wifi media-serving risk. |
| SECURITY.md | ✅ Shipped. Private reporting, 72h SLA, threat model ranked by content leakage first. |
| AI transparency | ✅ `docs/AI_DISCLOSURE.md` — plain-language, states its own weaknesses. Rare. |
| **CONTRIBUTING.md** | ❌ **Missing** — and `LANGUAGES.md:53` actively solicits PRs (*"Edit, open a pull request, done"*). |
| **CODE_OF_CONDUCT.md** | ❌ Missing. |
| **Issue / PR templates** | ❌ Missing. `.github/` contains only the two workflows. |
| **CHANGELOG.md** | ❌ Missing — and now load-bearing, because an updater without release notes is an unexplained download. |
| Bible translation licensing | ✅ **Clean.** KJV only, recorded as `license_type = "public domain"` (`db/verses.rs:173`), bundled via `include_str!`. **There is no import path for any other translation** — no `import_translation` command exists. Zero exposure today. Keep it that way, or licence properly. |
| GDPR / UK GDPR | ✅ Effectively N/A by architecture — no personal data leaves the device. **PRIVACY.md now says so. This is a selling point, and it is finally written down.** |
| Accessibility (WCAG) | ❌ Would not pass. 0 focus traps; 3 focusable-but-inoperable controls; no `<h1>`; `Stage.svelte` at 2.25:1. |

---

## 15. Prioritised Roadmap

### Phase 1 — **Stop lying** *(this week — the only thing that matters)*
1. **D1** — Windows signing + a platform-aware release gate that fails loud
2. **D2** — tag-derived version, CI assertion, non-draft release; then *actually perform an update* from an installed build
3. **D3** — panic path tells the truth: await + surface failures; `Esc` guards on the cheatsheet; fix the `B`-while-typing line
4. **D5** — model download: read timeout, real cancel, clear `running`, delete a bricked `.part`
5. **macOS mic entitlement** — before the first notarized build, not after a church reports a dead mic
6. **`LICENSE:3`** — put a name in it

**Exit criterion: a volunteer installs Relay on Windows *and* macOS, the OS does not warn, the microphone works, they get a verse on a projector, and when we ship a fix next week their machine actually receives it.** Until that is true, nothing else ships.

### Phase 2 — **Be honest about the AI** *(the trust layer)*
7. **D4** — method badge + `matched_text` + confidence-as-a-bar-for-direct-only
8. First-run mic meter actually moves
9. Live's flash-of-false-empty-state; mobile bottom nav; `Stage.svelte` contrast; the three inoperable `role="button"` divs
10. Surface or delete `related_scripture`
11. In-app error humanising beyond Live; kill the raw `String(err)` monospace

### Phase 3 — **Make the code survivable** *(pay the debt while it's cheap)*
12. Live commands return `Result`; `handle_nav` stops silently no-op'ing
13. Split `main.rs`; lift the fire engine out of the IPC surface
14. One e2e test: fire → nav → clear
15. Typed errors; normalise the swallow contract
16. Fix the migration ladder (`ROLLBACK`, real versioned rungs)

### Phase 4 — **Win the bet** *(the moat)*
17. **Record 30 minutes of real sermon audio.** Everything below is blocked on this.
18. Native-speaker review of the 66×3 book aliases — free, and it *is* the moat
19. Yoruba numerals
20. Service plan → `initial_prompt` (after `prompt_sweep` settles the shape question)
21. Measure WER. Then, and only then, evaluate a fine-tune.
22. Neural paraphrase embedder + populate `verses.embedding`
23. Operator UI localisation (yo/sw/ha)

### Phase 5 — **Grow**
24. Rename + brand + tagline + landing page
25. ProPresenter import as the marketed adoption wedge
26. CONTRIBUTING / CoC / templates / CHANGELOG
27. NDI (only if a real church asks)

---

## 16. Production Readiness Checklist

**Blocking a first church:**
- [x] In-app model download — *shipped, needs D5*
- [x] Signed + notarized macOS build
- [ ] **macOS microphone entitlement** *(without it, the notarized build has a dead mic)*
- [ ] **Signed Windows build**
- [x] Auto-updater — *shipped, needs D2 to function*
- [ ] **An update actually delivered end-to-end to an installed build**
- [x] First-run wizard — *shipped; its mic meter is dead*
- [x] PRIVACY.md + SECURITY.md + AI_DISCLOSURE.md
- [ ] **The panic path cannot report a success it did not achieve**
- [ ] An operator guide written for a volunteer *(the in-app Help already is; the written guide is not)*
- [ ] **A real service run end-to-end by someone who is not the author**

**Before public release:**
- [ ] Method + `matched_text` visible live
- [ ] `LICENSE` names a copyright holder
- [ ] WCAG: focus traps, operable controls, `<h1>`, `Stage.svelte` contrast
- [ ] Rename decided
- [ ] CONTRIBUTING + CODE_OF_CONDUCT + CHANGELOG
- [ ] Docs reconciled with the code (no more "Console" tab)
- [x] Crash reporting verified opt-in
- [x] Bible translation licensing confirmed (KJV only, no import path)

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

Revision 1 said Relay needed to become *installable*. It did that, and did it thoroughly — the whole epic, plus most of the next phase, plus five data-integrity bugs nobody was going to notice until a corpus went missing mid-service.

Revision 2's finding is subtler and, in live software, more dangerous. **Relay now needs to become honest.** An unsigned Windows installer that reports itself as signed. An updater that will silently never update. A toast that says the screens are clear when they are not. A help overlay whose dismiss key wipes the wall. A confidence score that means a probability for one kind of match and an arbitrary cosine for another, rendered identically. None of these will show up in a test. All of them will show up on a Sunday.

The engine is good. The gate is excellent. The design language is right, the decision log is exemplary, and the discipline around measurement is better than most funded teams manage. What is left is the unglamorous work of making the product's own reports about itself true — and then getting thirty minutes of a real preacher on tape, so that the moat can stop being a claim and start being a number.
