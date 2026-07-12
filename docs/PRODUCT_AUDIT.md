# Relay — Product Audit (2026)

**Scope decided with the owner before writing:**
- **Strategy: unchanged.** Free, MIT, offline-first, no accounts, no server. The recorded decisions in [DECISIONS.md](DECISIONS.md) stand.
- **Optimise for: the first 10 churches.** Not enterprise scale. The bar is *a volunteer, in a dark booth, with no training and no second take.*

Phases of the transformation brief that assume a commercial multi-tenant SaaS — billing, RBAC/SSO, multi-tenancy, audit logs, growth/monetization, government/healthcare/finance readiness — are marked **NOT APPLICABLE**, each with reasoning. They are not oversights. Adopting them would destroy the product's actual moat.

---

## 1. Executive Assessment

**Relay is a genuinely good piece of engineering wrapped around a product that cannot currently be delivered to the people it was built for.**

The engine is real and the discipline is unusually high: a full offline pipeline (listen → transcribe → detect → gate → render), 164 Rust tests, a template engine that makes the operator's preview provably identical to the projector, and a decision log that is better than most funded startups keep. After this week's hardening pass it also survives its own worst failure modes — the AI can no longer put a wrong verse on the wall unasked, a crash no longer takes the console down mid-service, and the panic key works from every screen.

None of that reaches a church.

To make Relay's core feature work, a user must open a terminal and run `curl` to fetch a ~148 MB model file into a folder that **does not exist in the packaged app**. The download itself will be blocked by macOS Gatekeeper and Windows SmartScreen, because nothing is signed. And when we fix a bug — as we just fixed six — there is no mechanism to deliver the fix.

So the honest position is:

> **The engineering is ahead of the product. Relay is roughly one week of distribution work away from being usable, and that week is worth more than any feature on the roadmap.**

The competitive bet in DECISIONS.md — out-execute Pewbeam on independent multi-screen templating and African-language speech — is still sound, and the templating half is already won. But you cannot out-execute anyone on a product that a volunteer cannot install.

---

## 2. Product Scorecard

Scored against the stated bar (*first 10 churches*), not against Stripe.

| Dimension | Score | Why |
|---|---|---|
| **Core engine** | **9 / 10** | Offline pipeline works end to end. Detection is DB-free, pure, 44 tests. Router now gates by method, not by an uncalibrated number. Genuinely strong. |
| **Distribution / install** | **1 / 10** | **The critical failure.** No code signing, no notarization, no auto-updater, and the STT model must be fetched by hand via `curl` into a path that doesn't exist in the packaged app. |
| **Onboarding / first-run** | **3 / 10** | Credit where due: the DB seeds 4 templates (auto-activated) and 4 channels, so a verse *can* reach a primary display in 2 clicks. But there is **no wizard, no welcome, no tour** (zero hits for `onboard|first.?run|welcome|wizard|tutorial`), no model, and the projector path is blocked by a bug (§6). |
| **UX (live operation)** | **5 / 10** | Strong bones: global panic keys, blackout, arm→confirm, crash recovery that tells the truth, and genuinely excellent mic-quality copy. Undermined by: Console/Planner splitting one service in half, a cheatsheet that lists dead keys, and an **ON AIR** badge that reports the microphone rather than the screen. |
| **UI / design language** | **7 / 10** | The dark/amber broadcast language is *correct* for a booth — a real design decision, not a default. Tokens are unified. Loses points for inconsistent empty states and English-only copy. |
| **Architecture** | **7 / 10** | Now well-separated: `pipeline.rs`, `router.rs`, `db/` by aggregate. `main.rs` is still 2,600 lines of command surface, and `capture.js` is still one large module. |
| **Performance** | **9 / 10** | 26 MB install. Semantic scan measured at 2.6 ms/query — deliberately *not* optimised, because measurement said not to. Tauri over Electron was the right call. |
| **Accessibility** | **2 / 10** | `aria-live`: **zero**. No focus trap, no skip link, a11y warnings suppressed with `svelte-ignore` rather than fixed. And `--v-faint` (#6c6b71 ≈ **3.4:1**) fails WCAG AA — it is the colour used for **every empty state**, i.e. the text a new operator most needs. |
| **Security** | **7 / 10** | CSP now set and verified in a packaged build. LAN bind is unauthenticated but broadcast-only and now a *recorded* decision. Unsigned binaries are the real exposure. |
| **Privacy** | **8 / 10** | Genuinely local-first, and the one exception (crash reporting) is opt-in, off by default, and now actually scrubbed. **But there is no PRIVACY.md** — the biggest trust asset is undocumented. |
| **Testing** | **8 / 10** | 164 Rust + 28 frontend. CI on macOS *and* Windows incl. release build. The IPC contract test is a genuinely good idea. No end-to-end/UI test. |
| **Developer experience** | **7 / 10** | CI, CodeRabbit, clippy `-D warnings`, an excellent decision log. Held back by a 2,600-line `main.rs`. |
| **AI readiness** | **5 / 10** | Detection is solid for direct references. Paraphrase is TF-IDF standing in for an embedder. **African-language accuracy is weak** — the headline differentiator is the least finished part. |
| **Brand** | **4 / 10** | "Relay" is a decent working name but undifferentiated and unsearchable. No logo, no tagline, no positioning statement anywhere. README calls it a "working name — rename freely." |
| **Business model** | **N/A** | Deliberately free/MIT. Sustainability is *parked, not decided* (DECISIONS.md). Correct to leave parked at this stage. |
| **Documentation** | **5 / 10** | `ARCHITECTURE.md` and `DECISIONS.md` are excellent — for engineers. The *operator* guide opens by explaining why `localhost:5032` is a dead console. Wrong audience. |
| **Legal compliance** | **3 / 10** | LICENSE present. No privacy policy, no security policy, no AI-disclosure statement. KJV is public domain (fine); other translations would not be. |
| **Enterprise readiness** | **N/A** | Explicitly out of scope. See §13. |
| **Overall maturity** | **5 / 10** | *Pre-launch, engineering-led.* Excellent core, undeliverable package. |

---

## 3. Strengths — protect these

1. **Offline-first is the moat, not a constraint.** It works when the power flickers and the wifi dies. For the target market that is the entire product. Every "modernization" instinct that erodes it is wrong.
2. **One template engine, one renderer.** `TemplateRender.svelte` drives the editor preview, the console wall, *and* the real output. WYSIWYG is therefore true by construction, not by discipline. This is the best idea in the codebase.
3. **The unified cue model.** Scripture, song, media, announcement and countdown all reduce to one polymorphic cue. Adding a content type is a payload shape, not new plumbing.
4. **Operator override as a first-class control.** Not a fallback UI. It always wins, it bypasses the gate and the debounce, and it is now recorded as a human decision rather than an AI one.
5. **The decision log.** `DECISIONS.md` with reasoning and explicit non-goals is rarer and more valuable than the code.
6. **Honest seams.** NDI returns a clear error instead of pretending. That instinct is worth more than a dozen features.

---

## 4. Weaknesses

1. **The product cannot be installed by its user.** (See §5.)
2. **The differentiator is the least finished part.** Relay's stated edge is African-language speech. Base multilingual Whisper is weak on Yoruba and Hausa — the README says so. Today, Relay is an *English* tool with African-language ambitions.
3. **Relay understands Yoruba but does not speak it.** The operator UI is hardcoded English, with no i18n layer. It listens to the preacher in Yoruba and talks to the volunteer in English.
4. **Three places show the same live output** (Console wall, Planner monitors, Templates preview) — and the operator must switch tabs mid-service.
5. **No onboarding.** Nothing teaches the one non-obvious idea in the product (channels are render targets of a shared template engine).
6. **Accessibility is an afterthought**, and `aria-live: 0` is the specific proof.
7. **`main.rs` is a 2,600-line command surface** and `capture.js` is a 790-line god-module. Neither blocks a service; both slow every future change.
8. **~20 bare `catch {}` blocks in `capture.js`** swallow errors entirely — the operator presses a button and *nothing happens*, with no error and no log. Silent failure is the worst possible behaviour in live software.
9. **Raw Rust error strings are shown to volunteers.** `Channels.svelte:264` renders `String(err)` in a **monospace** font. The audio path has excellent plain-language copy; the rest of the app does not.

---

## 5. Critical Issues — the three that decide whether Relay ships

### 🔴 C1 — The AI is unreachable in the shipped app
To enable speech recognition, the user is instructed to:
```bash
mkdir -p models
curl -L -o models/ggml-base.bin https://huggingface.co/ggerganov/whisper.cpp/...
```
A church volunteer will not do this. Worse: **in a packaged app there is no repo `models/` folder** — that instruction only works if you cloned the repo with git. So for the actual target user, Relay's entire reason to exist silently does not work. (It now at least *says so*, with the correct per-OS path — that was this week's fix — but saying so is not solving it.)

It is worse than "undocumented": `docs/USER_GUIDE.md` — the **operator** guide — **never mentions the model at all**. And Settings tells a church volunteer, in the product, to go and read the developer README: *"no model — audio-only (see README dev setup)"* (`Settings.svelte:259`).

**Fix:** in-app model download. First run detects no model, shows one button — *"Download the speech model (~148 MB)"* — with a progress bar, a checksum, and a resumable fetch. This is 1–2 days and it is the highest-value work available anywhere in this document.

### 🔴 C2 — The download is blocked by the operating system
No code signing, no notarization, no Windows signing. macOS will report the app as damaged or from an unidentified developer; Windows SmartScreen will warn. A volunteer bounces at this screen, every time.

**Fix:** Apple Developer ID ($99/yr) + notarization; an Azure Trusted Signing or equivalent certificate for Windows. Wire both into the release workflow. This is the price of admission and there is no way around it.

### 🔴 C3 — There is no way to ship a fix
No auto-updater. We fixed six screen-facing bugs this week; there is no mechanism to get them to a church that already installed Relay. For software that fails **live, in front of 500 people**, an update path is not a nice-to-have.

**Fix:** `tauri-plugin-updater` + signed release manifests on GitHub Releases. Update checks must never run during a service — check on launch, apply on next launch.

> **These three are one epic: "Relay can be installed, trusted, and updated by a non-technical volunteer." Nothing else on this roadmap matters until it's done.**

---

## 6. UX Redesign Recommendations

### Credit first: the cold-start defaults are good
A fresh DB seeds **4 templates (auto-activated)** and **4 channels** (`db/templates.rs:116-122`, `db/channels.rs:84-99`). So on a primary display the path really is: **Open output → type `John 3:16` → Enter.** Two clicks. That is genuinely well done and should not be "improved" away.

### But the projector path is broken, and it's a one-line bug
`openMainOutput()` calls `openOutput(id, 'Main screen')` with **two** arguments, while the signature is `openOutput(templateId, name, monitorIndex)` (`Console.svelte:142`, `capture.js:731`). `monitor_index` therefore arrives as `None`.

**The Console's "Open output" button can only ever open on the primary display.** A projector is, by definition, the *second* display. So the one button an operator will press first cannot do the one thing they need — and the workaround (go to Channels, set the display, press a *different* Open button) is undiscoverable.

It also hardcodes the "Classic Serif" template (`Console.svelte:136`), ignoring the channel's own assignment. Two buttons named "open the output" with different behaviour.

**Fix: pass the monitor index; make Console's button open the *channel*, not an ad-hoc window.** This is the cheapest high-impact fix in the product.

### The keyboard cheatsheet lies
`registerContext()` **overwrites** the whole handler table (`shortcuts.js:41-49`). Console registers `accept/dismiss/next/prev/search`; Planner registers **only** `next/prev`. So on the Planner tab, **`A`, `D` and `/` are dead keys** — while the cheatsheet (`shortcuts.js:61-65`) still tells the operator they work.

Worse, `←`/`→`/`Space` mean **different things on different tabs**: verse navigation on Console, plan-slide advance on Planner. This is the exact hazard the shortcut registry was built to eliminate for `Space` — and it survived in the arrow keys.

**Fix:** context handlers must *merge*, not replace; and the cheatsheet must render only the keys actually live on the current surface.

### The "ON AIR" indicator is telling the operator the wrong thing
There are **six** live indicators across five places, and the two loudest — the pulsing topbar badge and the footer (`App.svelte:111-115`, `134-137`) — key off `$capturing` (**the microphone is on**), not `$live` (**something is on the projector**).

So Relay can be shouting **ON AIR** at an operator whose screens are blank, and whispering the truth in a 10px monospace footer inside a monitor tile (`Console.svelte:292`).

**Fix:** the biggest, loudest thing on the screen must answer *"what is the congregation looking at right now?"* Microphone state is secondary.

### Console and Planner must merge
The two tabs own **disjoint halves of the same service**. AI suggestions, the live transcript, the mic toggle and manual fire exist **only** on Console. Song/media/announcement cues, the slide flow, and the stage-monitor "Up next" exist **only** on Planner.

A real service needs both at once: you run songs from the plan, and the preacher goes off-script and quotes a verse. Today that means switching tabs mid-sermon.

There is a concrete casualty: **`setStageNext` is never called from Console** (`ServicePlanner.svelte:365-368`). An operator running from the Console leaves the preacher's phone with no stage note and no "up next" — two of the three reasons the stage monitor exists.

**Redesign:** one **Live** surface — plan cues left, AI suggestions right, output wall top, one transport bar.

### Two buttons, same name, ~200px apart
`Push to stage` fires the **AI's guess** (`Console.svelte:248`); `Push to stage` fires **what you typed** (`Console.svelte:330`). Identical gold styling. One is a machine decision, the other is a human one, and they look the same.

### Empty states: the least readable text in the app
`.r-empty` uses `--v-faint: #6c6b71` (`app.css:319`) — roughly **3.4:1** on `--v-surf`, below the **4.5:1 WCAG AA** floor. Every "No plans yet…", every placeholder, is the hardest text in the product to read — and it is precisely the text a *new operator* most needs.

Also: History's empty state says *"start listening in **Settings**"* (`History.svelte:202`). Listening is on the **Console**. The onboarding copy points the wrong way.

### Accessibility (concrete, cheap)
- **`aria-live="polite"` on the "now live" region.** The single most important a11y fix — announce what went to the screen. Currently **zero** `aria-live` in the entire app.
- Raise `--v-faint` to pass 4.5:1, or stop using it for body text.
- Focus trap + restore on dialogs; real `<button>`s instead of `role="button" tabindex="0"` divs (`ServicePlanner.svelte:529`, `620`).
- Fix heading order (`<h3>` above `<h2>` in `Console.svelte:212`/`276`).
- Never encode cue type in colour alone.

---

## 7. UI Modernisation Plan

**Do not modernise the visual language. It is already right.** Dark, near-black, amber-as-tally-light is a *deliberate, correct* choice for a person sitting in an unlit booth behind a congregation. A light-mode "modern SaaS" refresh would actively harm the user.

What genuinely needs work:
1. **A trust signal for AI decisions.** The operator must see *why* a verse fired — direct reference vs. paraphrase — at a glance, and paraphrase must *look* less certain. (Amethyst-for-paraphrase is specified in the design prompt and not yet built.)
2. **`matched_text` is captured and never shown.** Highlighting *the words that triggered the match* is the clearest possible explanation of an AI decision. The data is already there.
3. **Confidence as a bar, not a number.** `0.92` means nothing to a volunteer.
4. **Consistent empty/loading/error states** — currently ad-hoc per view.

---

## 8. Feature Matrix

| Feature | Verdict | Reasoning |
|---|---|---|
| Scripture detection (direct) | **KEEP** | The core. Works. |
| Semantic/paraphrase detection | **IMPROVE** | TF-IDF is a placeholder standing where an embedder belongs. Cannot auto-fire (correctly). Replace the `SemanticIndex::top_k` seam with a small local embedding model. |
| Template engine | **KEEP** | Best asset. Don't touch. |
| Output channels (HDMI/OBS/kiosk/stage) | **KEEP** | The differentiator vs. Pewbeam. |
| Console | **MERGE** → Live | Merge with Planner. |
| Service Planner | **MERGE** → Live | See above. |
| Channels tab | **MERGE** → Settings/Output | A volunteer configures this once, not weekly. It doesn't deserve top-level nav. |
| Templates tab | **KEEP** | But move behind "Design", not in the live path. |
| Library (scripture/songs/media/announce/history) | **KEEP** | Real value; ProPresenter parity. |
| Song arrangements | **KEEP** | Genuinely well-modelled. |
| Countdown | **KEEP** | Cheap, high-utility. |
| ProPresenter import | **KEEP** | Excellent adoption wedge — *"bring your existing songs"*. Under-marketed. |
| Voice profiles / self-calibration | **SIMPLIFY** | Powerful, but two sliders + a dial + learned thresholds is too many concepts. Expose **one** sensitivity dial; keep the learning invisible. |
| Crash reporting | **KEEP** | Opt-in, scrubbed, off by default. Correct. |
| NDI | **DEFER** | Honestly parked. Leave it parked. |
| **In-app model download** | **ADD — P0** | See C1. |
| **Code signing + notarization** | **ADD — P0** | See C2. |
| **Auto-updater** | **ADD — P0** | See C3. |
| **First-run setup wizard** | **ADD — P0** | See §6. |
| **Rehearsal mode** | **ADD — P1** | Practise the service with outputs going to a preview only. The rehearsal we ran by hand this week found three bugs; give operators that safety. |
| **UI localisation (yo/sw/ha)** | **ADD — P1** | Relay detects these languages but cannot speak them to its own operator. |
| Multi-tenancy / accounts / billing / RBAC | **REMOVE (never build)** | Contradicts the offline-first moat. Not a gap — a decision. |

---

## 9. Information Architecture

**Now:** Console · Channels · Templates · Library · Planner · Settings (6 tabs, 2 of which are live surfaces, 3 of which show the same preview)

**Proposed:**

```
LIVE          ← the only tab that exists during a service
                (plan cues + AI suggestions + output wall + transport)
LIBRARY       ← scripture · songs · media · announcements · history
DESIGN        ← templates (+ the output-channel assignment that lives in Channels today)
SETTINGS      ← audio · speech + model · sensitivity · voice profiles · privacy
HELP          ← ← NEW. There is currently no in-app help at all.
```

Four working tabs plus Help. **During a service the operator never leaves LIVE.** That is the test.

---

## 10. Technical Modernisation

Ranked by value, not by fashion:

1. **Split `main.rs` (2,600 lines, 95 commands)** into `commands/{live,library,plans,output,settings}.rs`. Mechanical, low-risk, unblocks contributors.
2. **Split `capture.js` (790 lines, 77 exports)** by domain and normalise the throw-vs-swallow contract — half the wrappers swallow errors and return `[]`, half throw, and callers cannot tell which.
3. **Replace TF-IDF with a real embedder.** The `SemanticIndex::top_k` seam already exists. This is the AI half of the moat.
4. **Fix the five known data-integrity gaps** (from CodeRabbit, all pre-existing, none screen-facing): `reimport_full_kjv` isn't atomic (an interrupted run can empty the corpus — the nastiest); `import_song` isn't transactional; `delete_media` orphans plan cues; `move_plan_item` no-ops after a delete leaves a gap; the Lower-Third forward-fill isn't id-scoped.
5. **Add an end-to-end test** that drives a real service (fire, nav, clear) against a headless build.
6. **Do NOT** rewrite the stack. Rust + Tauri + Svelte + SQLite is correct for this product and would be chosen again.

---

## 11. AI Enhancement Strategy

Only where it earns its place.

| Opportunity | Verdict |
|---|---|
| **African-language STT fine-tunes** | **THE priority.** This is the stated moat and it is the least-built part. Community Whisper fine-tunes for Yoruba/Swahili/Hausa exist; evaluate, bundle, and let the operator pick per-profile. |
| **Neural paraphrase embedder** | **Yes.** Replaces TF-IDF. Would let paraphrase matches *earn* the right to auto-fire — which today they are (correctly) forbidden from doing. |
| **Explain the match** (`matched_text`) | **Yes, cheap.** Show the words that triggered it. Trust, not magic. |
| **Sermon-aware biasing** | **Yes.** The preacher's plan already names likely passages — bias the decoder toward them. Cheap accuracy win, uses data you already have. |
| **Auto-build a plan from a sermon outline** | **Maybe.** Paste an outline → draft cue list. Real time-saver for a volunteer. |
| **Post-service summary** | **Maybe.** "Verses referenced today" — history already stores it. |
| **AI chat assistant** | **NO.** DECISIONS.md: *"Not a general AI assistant. Scope discipline."* Still right. |

---

## 12. Brand Refresh

Genuinely weak, and cheap to fix.

- **Name.** "Relay" is generic, unsearchable, and already used across broadcast/networking. The README literally says *"Working name — rename freely."* A rename is warranted **before** the first church installs it, not after.
- **No logo, no tagline, no positioning line** exists anywhere in the repo.
- **The positioning is strong and unstated.** Suggested: *"It hears the verse. It puts it on screen. Even when the internet doesn't."* — offline-first and African-language-first are the two things no competitor is saying.
- **Under-marketed wedge:** ProPresenter import. *"Bring your songs, keep your workflow."*

---

## 13. Enterprise Readiness — NOT APPLICABLE (and that is correct)

| Asked for | Verdict |
|---|---|
| Multi-tenancy | **No.** One church, one machine, no server. |
| RBAC / SSO / audit logs | **No.** There is no login. There is one operator, standing in the room. |
| Compliance (SOC2/HIPAA/gov) | **No.** No data leaves the device. There is nothing to certify. |
| Multi-region / global deploy | **No.** There is no deployment. It's a desktop app. |
| API ecosystem | **Partial, already true.** OBS/kiosk over WebSocket + LAN HTTP. That *is* the integration story, and it's the right one. |
| Internationalisation | **YES — and it's a real gap.** But for the *operator's* language (Yoruba/Swahili/Hausa), not for enterprise localisation. |

**These are not gaps. They are the shape of the product.** A church of 80 people in Ibadan does not need SSO. It needs the verse on the screen when the power comes back.

---

## 14. Legal & Compliance Review

| Item | Status |
|---|---|
| LICENSE (MIT) | ✅ Present |
| **PRIVACY.md** | ❌ **Missing — and it is your single biggest trust asset.** Relay records sermons. Say plainly: nothing leaves the device; crash reporting is off by default and never sends transcripts or verse text. |
| **SECURITY.md** | ❌ Missing. Needs a disclosure contact. |
| **AI transparency statement** | ❌ Missing. Say that verses are AI-detected, that the operator can always override, and that paraphrase matches never auto-fire. |
| CONTRIBUTING / CODE_OF_CONDUCT | ❌ Missing (needed for an OSS project inviting contributors) |
| Bible translation licensing | ⚠️ **KJV is public domain — fine.** NIV/ESV/NLT are **not**, and adding them requires a licence. Get this right *before* someone ships a build with a copyrighted translation. |
| GDPR / UK GDPR | ✅ Effectively N/A by architecture — no personal data leaves the device. **This is a selling point. Write it down.** |
| Accessibility (WCAG) | ❌ Would not pass. `aria-live: 0`. |

---

## 15. Prioritised Roadmap

### Phase 0 — Ship the fix that exists *(this week)*
Merge PR #1. Six screen-facing bugs, CI on macOS + Windows, first working release build.

### Phase 1 — **Make it installable** *(the only thing that matters)*
1. In-app model download with progress + checksum **(C1)**
2. Code signing + notarization, macOS and Windows **(C2)**
3. Auto-updater **(C3)**
4. First-run wizard: projector → microphone → done
5. PRIVACY.md, SECURITY.md, AI-disclosure statement

**Exit criterion: a volunteer who has never seen a terminal installs Relay and gets a verse on a projector in under 10 minutes.** Until that is true, nothing else ships.

### Phase 2 — Make it survivable
6. Merge Console + Planner into one **Live** surface
7. Rehearsal mode
8. `aria-live` + focus traps + real buttons
9. In-app Help
10. Fix the five data-integrity gaps

### Phase 3 — Make it *ours* (the moat)
11. Yoruba / Swahili / Hausa STT fine-tunes — **the differentiator**
12. Neural paraphrase embedder (replaces TF-IDF)
13. Operator UI localisation
14. Show `matched_text` — explain the AI

### Phase 4 — Grow
15. Rename + brand + landing page
16. ProPresenter import as the adoption wedge
17. NDI (only if a real church asks)

---

## 16. Production Readiness Checklist

**Blocking a first church:**
- [ ] In-app model download
- [ ] Signed + notarized macOS build
- [ ] Signed Windows build
- [ ] Auto-updater
- [ ] First-run wizard
- [ ] PRIVACY.md + SECURITY.md
- [ ] An operator guide written for a volunteer, not a developer
- [ ] **A real service run end-to-end by someone who is not the author**

**Before public release:**
- [ ] `aria-live` on the live region
- [ ] In-app Help
- [ ] Crash reporting verified opt-in (done)
- [ ] Bible translation licensing confirmed (KJV only, or licensed)
- [ ] Rename decided

---

## 17. Success Metrics

Vanity metrics are wrong for this product. Measure **services survived**, not users acquired.

| Metric | Target | Why |
|---|---|---|
| **Install → first verse on screen** | **< 10 min**, zero terminal | The single number that decides adoption. Currently: *impossible*. |
| **Services completed without operator panic** (no Emergency Stop, no crash) | **> 95%** | The real definition of "it works". |
| **Wrong-verse rate** (auto-fired, then dismissed) | **< 5%** | Already in SPEC §2. Now measurable — `detections.status` distinguishes AI from human. |
| **Detection recall** on a real sermon | > 80% of spoken references caught | Needs a labelled sermon corpus. You have none. Build one. |
| **Yoruba/Swahili/Hausa word error rate** | Baseline it, then beat it | The moat is unmeasured today. |
| **Crash-free sessions** | > 99% | Sentry (opt-in) can now tell you. |
| **Time from bug report → church running the fix** | < 7 days | Currently **∞** — no updater. |
| **Churches running a 2nd service** | The only retention metric that matters | One service is a trial. Two is a product. |

---

## Closing

Relay's problem is not that it needs to become more like Linear or Stripe. It needs to become **installable**.

The engine is good. The design language is right. The architecture is sound and the decision log is exemplary. What is missing is the unglamorous week of work — signing, notarizing, an in-app model download, an updater, and a setup wizard — that stands between a strong piece of engineering and a church in Lagos actually using it on Sunday.

Do that week. Then go win the African-language bet, which is the only thing here that no competitor can copy quickly.
