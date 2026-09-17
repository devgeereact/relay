# Consolidation — three roots, one branch, and the defects the waves left behind

**Date** 2026-09-17 · **Branch** `feat/consolidation` · **Target** `main`, one PR
**Operator decisions taken** 2026-09-17: scope includes the background layer; the
programme rail's past-zero behaviour is delegated to the implementer (§2).

---

## 0. Why this document exists

Five waves of work sit on 37 branches across 24 worktrees. None of it is on `main`,
which last moved on 2026-09-15. Three pull requests are open and none of their work
has shipped. The most current copy of the gap register exists on one laptop and had
never been pushed until this branch was cut.

This is not a feature specification. It is the contract for assembling that work
without losing any of it, then repairing what the assembly exposes. Every claim below
was measured on the trees named, not inferred from a document.

**The single most dangerous belief available right now** is that
`feat/waves-3-5-merge` is the newest branch and therefore contains everything. Its
head commit reads *"Merge feat/wave3-timers (catch-up)"*, and the merge base is
`956106d`. Thirteen commits of `feat/wave3-timers` are outside it, carrying RG-149,
RG-150, RG-152 and RG-153 as closed, the whole countdown warning chain, and
`src/lib/timers.js`. Anyone who takes that branch wholesale loses four closed
findings and a rendered control, and its register still shows all four as open.

---

## 1. The root set

Three heads. Their union is every commit that exists; no pair contains another.

| branch | head | ahead of `main` | carries |
|---|---|---|---|
| `feat/waves-3-5-merge` | `5890688` | 169 | waves 0, 1, 2, 5, 7 + wave 3 up to `956106d` |
| `feat/wave3-timers` | `27712c5` | 144 | waves 0, 1, 2 + all six wave-3 fix branches |
| `feat/wave4-stage-planner` | `41b91be` | 126 | waves 0, 1, 2 + wave 3 tracks A–F + wave 4 A–E |

Everything else is contained in one of the three. `rebrand/live` and
`rebrand/planner` are zero commits ahead of `main` and hold nothing; their worktrees
are locked and can be removed.

`feat/wave7-timers-templates-stage` is **not a wave 7**. There is no wave 6 or 7 plan
in `docs/superpowers/`. It is the original shared working-branch name from the wave 2
spec, and it is contained in `feat/wave2-integration`.

### 1.1 Baseline, measured before any merge

| tree | frontend | Rust | fmt | clippy | release build |
|---|---|---|---|---|---|
| `feat/waves-3-5-merge` | 165 files / 2512 tests | 839 | clean | clean | 1m07s, clean |
| `feat/wave4-stage-planner` | 152 files / 2351 tests | 817 | — | — | — |

Both sides are green. Any red after the merge is the merge's fault, which is the
property this baseline exists to establish.

---

## 2. The programme rail — the one place two waves built opposite behaviour

`src/Stage.svelte`'s `$: programme` block was rewritten by both waves. The conflict is
not textual, and `src/lib/timers.test.js` carries three pairs of assertions that
cannot both pass.

| behaviour | `feat/wave3-timers` | `feat/wave4-stage-planner` |
|---|---|---|
| past zero | counts up, `+4:37` | `0:00`, or the operator's done message |
| `countdown_done` on the rail | deliberately no reader | read, and sized as prose |
| warn with no chosen threshold | falls back to the shared rule | never warns |
| held rows | not modelled | `held`, frozen, never warned |
| row width (RG-147) | `progCh` → `--tch` from the widest rendered string | absent |
| rail capacity (RG-147) | absent | `MIN_TIMER_PX = 132`, `capacity`, `+N more` |

### 2.1 The ruling

**Wave 3's past-zero semantics win. Wave 4's `held` state and rail capacity are kept.
The done-message keeps no reader on the rail.**

This is decided on a measurable conflict rather than on which ruling came later.
Wave 3 sizes every column from the widest *rendered string*. Wave 4 places **prose**
into that same slot. An operator typing `WRAP UP NOW` sets `progCh` to 11, widening
every row on the rail — which is precisely the six-60px-columns failure that wave 4's
`MIN_TIMER_PX` exists to prevent. The two fixes actively fight each other the moment
words are allowed onto the rail, and neither author could see that, because neither
branch contained the other.

Both RG-147 answers are correct and they are different bugs: wave 3 fixes a
95-minute clock painting `1:30:1` inside an `overflow:hidden` box; wave 4 fixes six
timers collapsing into six 60px columns. **Losing either is a regression.** The merged
block carries `progCh`/`--tch` *and* `MIN_TIMER_PX`/`capacity`/`+N more`.

The product argument agrees with the technical one: the rail is one preacher's
bookkeeping, read mid-sermon, and what it is asked past zero is *how far over*.
`+4:37` escalates as the minutes pass; `WRAP UP` does not. The operator's words are
what a **congregation** countdown says when it lands, and they still say it there.

### 2.2 The warning rule

Wave 3's shared rule with a fallback wins, and not by preference. Wave 4's no-fallback
rule is argued from a premise its comment states explicitly: *"This page has no Tauri
bridge and does not import that module, so the default in force here is the SHIPPED
minute whatever the church set."* Wave 3's `warnchain` branch makes that false — it
adds `channels::CountdownWarnDefault` and ships `warn_default_ms` on the timer frame
and `countdown_warn_default_ms` on content frames, which `Stage.svelte` reads through
`applyWarnDefault`. **The merge deletes wave 4's reason.** That comment must be
rewritten or removed rather than left arguing from a premise the merge removed.

### 2.3 The tests

Three pairs in `src/lib/timers.test.js` assert opposite outcomes. A "keep both sides"
merge produces a suite that fails whichever implementation wins. The losing side's
cases are **deleted**, not merged, and the deletion is recorded in the commit.

---

## 3. Merge order and conflict dispositions

### Step 0 — before any merge

Push all three roots. Then strip the debris: 36 committed `.playwright-mcp/` artefacts
on two branches, plus `stage-before.json` and `stage-after.json` at the repo root
(raw computed-style dumps). `feat/wave4-stage-planner` carries none. `.gitignore`
covers none of it, so all of it lands on `main` unless removed. Add the ignore rule in
the same commit.

### Step 1 — `feat/consolidation` ← `feat/wave3-timers`

Four conflicts. Three are "keep both", because the two sides append to the same place
without competing:

| file | disposition |
|---|---|
| `src-tauri/src/main.rs` | both — two adjacent `setup` warm-up blocks, `kiosk.cache_channel_roles` and `CountdownWarnDefault` |
| `src/Output.svelte` | both — one side adds `acceptsStageMessage, roleOf`, the other `setCountdownWarnDefault` |
| `src/lib/stores/capture.js` | both — two paragraphs appended to one `startCountdown` doc comment |
| `docs/qa/RELAY_GAP.md` | hand-reconciled, §4 |

### Step 2 — ← `feat/wave4-stage-planner`

Six conflicts. The same set appears against either wave-3 root, which proves these are
wave-3-versus-wave-4 in origin and that step 1 does not change them.

| file | disposition |
|---|---|
| `src/Stage.svelte` | rewrite per §2 — neither side wholesale |
| `src/lib/timers.test.js` | both, minus the contradictory cases per §2.3 |
| `src-tauri/src/e2e.rs` | both — differently named tests at an adjacent insertion point |
| `docs/qa/RELAY_GAP.md` | hand-reconciled **and renumbered**, §4 |
| `CLAUDE.md` | the `waves-3-5-merge` side, then correct the audit count — neither side is right; verify with `ls docs/qa/audits \| wc -l` |
| `docs/qa/QA_HARNESS.md` | keep both dated blocks as history, add a freshly measured one |

Two files sit outside the conflict set and git will pick one silently. Both must be
checked by hand:

- `docs/superpowers/specs/2026-09-15-timers-templates-stage-design.md` — the 73-line
  §4.2 correction exists **only** on wave 4. **Take wave 4's copy.** Its own plan names
  the failure mode: *"Leaving that sentence in a spec with no correction beside it is
  how the next wave under-scopes it again."*
- `docs/REBRAND.md` — **take the `waves-3-5-merge` copy**, which carries the channel-role
  reason and the wave-numbering disambiguation.

### Step 3 — migrations

No migration is added twice with different content; `db/mod.rs` auto-merges in every
pair, and `docs/data/schema.sql` is edited in non-overlapping hunks.
`docs/data/schema-baseline.sql` is untouched on all three, which is correct — it must
never be edited.

**The residual risk is ordering, not content.** Wave 2's
`ensure_tables_retires_before_it_seeds` and wave 5's `ensure_template_seed_identity`
and `ensure_templates_name_real_families` all operate on the template seed, and
nothing tests the combined boot order. Verification is in §8.

---

## 4. The register — five doubly-allocated ids

**RG-145 … RG-149 name ten different findings.** Wave 3 and wave 4 both filed from
RG-145 upward, for entirely different work.

| id | wave 3 side | wave 4 side |
|---|---|---|
| RG-145 | ✅ the console says a word is on the preacher's monitor after a panic control took it off | ⏳ per-cue screen targeting does not exist |
| RG-146 | ✅ the Start control is not clickable at its centre | ⚠️ the rail's warning and finished message cannot be reached by any control |
| RG-147 | ✅ a programme timer past one hour is clipped | ✅ nothing removes a programme timer; the floor keeps the oldest |
| RG-148 | ✅ the stage row wears no warning at any threshold | ✅ the rail plus both stage panels cuts the verse |
| RG-149 | ✅ a warn threshold reaches neither a congregation screen nor a stage | ✅ `.alert.sm` cannot render |

**A union merge by id silently destroys five wave-4 findings, three of them closed.**

The precedent is already set and documented: wave 5 filed RG-143 … RG-147, met wave 3,
and renumbered to RG-156 … RG-160, keeping both id sets side by side in its audit.
Apply the same rule. **Wave 4's five become RG-161 … RG-165.** Thirty citations across
nine files follow. `docs/qa/audits/2026-09-17-WAVE4-STAGE-PLANNER.md` is frozen
evidence and gets a renumbering note at the top, never a rewrite.

**No instrument catches a stale citation here**, because RG-145 … RG-149 still exist
after the merge — they simply mean something else. `crossrefs.test.js` resolves them
happily. This is the failure CLAUDE.md warns about in another costume: *"a dead §16
looks like evidence."* The only instrument that fires is `relaygap.test.js`, on the
header counts, and only after they are recomputed.

Wave 4's RG-146 may close for free: it was flagged rather than closed because *"the
control that would write one belongs to wave 3 Track E"*, and wave 3's `fix-wayback`
and `fix-warnchain` branches supply exactly that control. **Re-check after step 2
rather than assuming either way.**

### 4.1 DECISIONS §89 means two different things

`main` ends at §85. `feat/waves-3-5-merge` already renumbered wave 3's §89 to §91 and
recorded it. Wave 4 adds no sections, so it introduces no new collision — but the four
wave-3 fix branches outside the merge were written against §89, and **§89 still exists
after the merge** as the channel-role decision. A stale citation therefore *resolves*
and points at the wrong decision. Grep the merged tree and check every hit by meaning.

---

## 5. Cross-surface state — one event, five doors

The operator reported that Live's sensitivity dial and Settings' "do not work
together". They are right, and the mechanism is narrower and worse than that.

**Rust is not at fault.** `apply_thresholds` is a genuinely single door that moves the
gate, the baseline and the stored profile together, and its doc comment explains why
doing one without the others leaves a profile describing a state the router was never
in.

**The frontend has no threshold event to subscribe to.** No `thresholds://` event
exists; `main.rs` emits ten events and none carries the gate. Consequences, all
measured:

- Live's dial is the **only setting in the shell held in a component-local `let`**
  (`Dock.svelte:465`), read once at `onMount`. The dock is mounted outside the
  workspace router, so unlike every view it is never rebuilt and never re-reads.
- It also goes stale with nobody touching a control, because the router self-calibrates
  through `record_feedback` on every confirm and dismiss.
- Settings' slider binds to `editing.sensitivity` on a voice-profile object.
  `update_voice_profile` compares that stale number against an already-updated row,
  concludes the dial moved, and re-derives the gate from it — **silently reverting a
  change made thirty seconds earlier on the same screen.**
- Switching preacher or applying a room moves what may auto-fire unattended, and no
  surface that displays the gate moves.

### 5.1 The fix

A new `detection://thresholds` event carrying `{auto_fire, suggest, sensitivity}`,
announced after **every** mutation of `Router.thresholds`. Five writers exist:
`apply_thresholds` (two callers), `apply_profile` (three callers — profile save,
profile select, room apply), and `record_feedback`. One store in `capture.js`; the
dock's local `let` and Settings' editor both derive from it.

Per rule 2 the emit happens after the router lock is released, never inside it.
`apply_thresholds` already releases before touching the database, so the shape exists.

A scanner in `hardrules.test.js` asserts that no writer can move the thresholds
without announcing them. Five doors onto one guarantee is the exact shape of this
repository's most repeated bug, and a guarantee kept on four of five doors is the
bug, not a mitigation.

### 5.2 What was investigated and found clean

Recorded so nobody re-audits it. Detection on/off, rehearsal, blackout and clear, safe
mode, service lock, content looks, the default template and per-channel templates are
all store-backed from a backend answer and cannot disagree. **Recognition language
persists correctly** — `set_stt_language` writes through `db::set_active_profile_language`
*before* touching the engine. RG-138 closed that on 2026-09-15 and **CLAUDE.md was
never updated**; the handbook paragraph saying it does not persist, and RG-116 naming
that control as its own fix, are both now wrong and are corrected by this branch.

Two latent instances of the same class, harmless only by accident: `activeTranslation`
is a local `let` in both Settings and Library with no store behind it, safe only
because both views are destroyed on a tab switch. Recorded, not fixed.

---

## 6. UI coherence

The complaint is accurate and already half-measured. `buttonshapes.test.js` records
*"181 hand-rolled buttons across 30 files"* and holds 16 of them.

Measured on the assembled surface: **357 buttons across 51 components, 122 of them
(34%) touching no shared class**, in 80 component-local classes, at 13 bespoke heights
against a published ladder of 22 / 26 / 34. Shared-class rate by workspace:
Outputs 83% · Settings 74% · Live 58% · Planner 50% · Templates 47% · Library 44% ·
**Stage 0%**.

`src/Stage.svelte` imports `app.css` and then uses none of it: five bespoke buttons, no
`:hover` on any of them, no `:focus-visible` on any of them, every edge a raw `rgba()`
rather than a line token.

### 6.1 The component layer

`src/lib/ui/` holds five components today. The missing pieces, in payoff order:

1. **`Button` / `IconButton`** — height, padding, radius and type from the ladder;
   `variant`, `size`, and a `disabledReason` that renders `title` and
   `aria-describedby`. **Publishing `.r-iconbtn.sm` at 22px is the literal cause** of
   the 26-in-a-row-of-22 misalignment in Library's action row and the template
   editor's toolbar: earlier waves correctly stripped local size overrides from every
   icon button, which made the 4px step uniform and permanent because app.css never
   published the small step.
2. **`Menu` / `MenuItem`** — six popover shells today, with five row paddings, four row
   font sizes, three hover inks, three radii and two shadows. Also the one place to put
   rule 44's `Esc` contract, which four files currently re-derive and three get wrong.
3. **`Field`** — `.r-well` is named in app.css's focus group with **no rule body and
   zero call sites**, while four search boxes each set `outline:none` and invent their
   own focus treatment, one of which is a 1px border change measuring 2.29:1 against
   its own field.
4. **`Toolbar`** — fixes one control height for its children, so a mixed row cannot step.
5. **`ListState`** — makes *empty ≠ loading ≠ error* structural rather than remembered.
   `Arrangements.svelte` currently renders four different states through one class and
   has no error branch at all.
6. **`Chip`**, **`Dialog`**, **`Surface`** — 74 hand-rolled chip-shaped elements against
   3 call sites of `.r-chip`; ten files importing `trapFocus` separately; five
   hand-rolled overlay shadows for one job.

A class in a global stylesheet can be ignored by omission. A component cannot. That is
why this is a component layer and not more CSS.

### 6.2 Two colour-law breaches

**Both are P1, because a colour that lies to an operator during a live service is the
same class of failure as a control that lies.**

`VerseDeck.svelte` paints an on-air row's border `rgba(255,176,0,…)` — `#ffb000` — and
that same row's 3px bar `var(--v-amber)` — `#ffa31a`. One row, two ambers. This is
verbatim the defect `DESIGN_SYSTEM.md` §1.3 already describes having fixed elsewhere.
Sixteen further off-palette `rgba()` literals exist, including a retired amethyst and
a retired Tailwind green, two of them self-contradicting inside a single declaration.
`workspacegrammar.test.js` states in its own header that **`rgba()` is not scanned at
all**.

**`app.css` contradicts itself in its own file.** Line 427 states amethyst is *"the
rehearsal colour, and nothing else uses it"*. The same file then spends amethyst
eleven times on the boot ladder — brand bars, current step, progress fill, spinner,
check states — plus the splash and a Settings network caution. Fourteen surfaces have
quietly adopted amethyst as progress, caution and brand. `DESIGN_SYSTEM.md` §1 says
*"a colour carrying a promise cannot be borrowed for a hunch"*, and
`colourlaw.test.js` polices neither.

This needs **one ruling recorded in DECISIONS.md**: either amethyst is reserved and
the boot and caution surfaces move to steel, or the doc gains the second meaning and
the test is widened to police the boundary. It cannot stay as it is, because today the
stylesheet and the design system each assert the other is wrong.

Stage additionally spends amber on *selected*, on *pressed*, and on a *focus ring* —
on the screen the preacher reads.

### 6.3 Enforcement

The components are half the work. Widen `buttonshapes.test.js` from 16 files to all
50, and `workspacegrammar.test.js`'s tier-3 sweep to `rgba(`. Both files already say
in their own headers that a scanner which quietly narrows passes everything, and both
are narrow in exactly the places this audit found defects.

---

## 7. Outputs

### 7.1 What Relay already does better than ProPresenter, and must not lose

Every output page reports whether it is still painting, every two seconds, with a
closed enum, and a disagreement between Relay's belief and the screen's own claim is
**printed rather than smoothed over**. And a screen joining mid-service is caught up to
what is on the screens in one frame, with a panic control that cannot be undone by the
replay. Those are the two things that actually went wrong in the field. Neither is
traded away for parity.

### 7.2 The background layer

**The largest real gap, and it is structural.** The entire layer stack sits inside
`{#if content}`, and `media_url` is written at exactly one site in the whole binary.
A verse and a picture are therefore mutually exclusive payloads: scripture over the
church's own background is impossible today. It is also the prerequisite for clear
groups and for an announcement that does not destroy the reading underneath it.

Design constraints, in the order they must be honoured:

- **A template with no background layer behaves byte-identically to today.** The change
  is opt-in by template design. There is no feature flag: a stored-but-unread
  preference was explicitly ruled out by wave 4, and a flag nobody reads is the same
  defect wearing a switch.
- Background becomes its own payload with **its own retained hub slot**. Four separate
  comments in `KioskHub` record that a shared slot erases the retained verse.
- **`clear` must still remove everything, background included.** This is the invariant
  to write the test against *first*, before any of the rendering work. The existing
  rule is stated at the top of the layer stack and it is load-bearing.
- The pre-air validator and rehearsal gating must cover the second payload kind at the
  **choke point**, not at its call sites. A validator added at five call sites is a
  validator that will be missing from the sixth, and four separate bugs in this
  repository have that exact shape.

### 7.3 The repairs, in priority order

1. **Per-screen clear and blackout.** `clear_screens` and `blackout` take no channel
   argument. "Take the lobby TV down but leave the wall live" is an ordinary request
   and is impossible. The existing total controls stay **exactly** as they are, first
   and largest, per rule 15 and DECISIONS §20. The split goes in the *call*; a panic
   control that must work out which screen it is addressing is a panic control that
   can fail.
2. **`stage.html` identity.** It sends `hello` with no channel and no role, and accepts
   any `stage_alert` unconditionally, while `output.html` role-filters correctly.
   Anyone on the LAN who opens the stage URL is shown the word meant for the preacher.
   This is the "guarantee kept on one of two doors" mistake on the surface carrying
   private words about a service.
3. **A message overlay** that sits over a reading rather than replacing it, with one
   owner, cleared by the panic controls, published on both doors, retained in its own
   slot. Exactly one overlay — not ProPresenter's props system.
4. **Rename a screen.** No command exists.
5. **Display targeting by stable identity** rather than by OS monitor index, which
   reassigns the projector when a dock is unplugged.
6. **Media transport.** A fired video is `autoplay loop muted` with no pause, no seek,
   no play-once and no end action.

### 7.4 Deliberately never built

SDI and hardware fill/key (constitutional; a converter closes the gap for the price of
a microphone cable). NDI (reaches no ATEM at any tier, and everything it would reach
already works over `:8032`). Live video input and capture (an explicit SPEC non-goal,
and it pulls toward the SDI wall). Masks, alpha mattes and blend modes (a mask is
precisely the object that can make a verse invisible in a way no test here would
catch). Edge blending, warp and corner pin (breaks the one-renderer WYSIWYG invariant
and makes the auto-fit measure a shape it is not painting). Per-layer and per-screen
transition matrices (more authorities is the defect §69 and §71 are the scars from).
Genuinely independent content per audience screen (reverses "one AI decision fanned
out" and doubles every safety surface).

---

## 8. Verification

No claim of completion without the evidence beside it.

1. `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`.
2. `cargo test` and `npx vitest run`, reading **the runner's own summary line**. The
   same three counts were corrected five times in one week; `QA_HARNESS.md` §0 is the
   one register that carries values, each beside the command that produces it.
3. `npm run build` **before** `cargo test`, per RG-127 — two `channels` tests need
   `dist/` and do not say so.
4. `npm run version:check`, `node scripts/qa-inventory.mjs`, `npm run updater:check`.
5. **Boot once against a copy of a real `relay.db`.** RG-142 is on the register
   precisely because a migration that passed every test retired 0 of 21 rows on a used
   install, and every test in that module inserts the frozen fixture verbatim.
6. **A browser-driven pass against the real backend.** Every defect that reached a
   congregation was invisible to every static instrument in this repository, and
   `qa-inventory` reported zero problems throughout and was right every time. The
   harness is described in `audits/2026-09-17-WAVE4-STAGE-PLANNER.md` §0 and is
   temporary — it belongs in no commit.
7. `npm run tauri build`, then `scripts/sign-local.sh`, which reproduces rule 17's
   hardened-runtime conditions without a certificate.

Re-measure `QA_HARNESS.md` §0 on the assembled tree. None of the three existing
figure sets survives the merge.

---

## 9. What this branch does not do

- It does not move the release decision. NO-GO for general release, GO for a
  supervised pilot, stands.
- It does not touch detection, the router's decision logic, or any threshold value.
  Rules 10, 28, 30 and 34 are not in this diff. The threshold work in §5 changes who
  is *told* about a gate, never where the gate sits.
- It does not measure word error rate, which remains unmeasured in every language
  against roughly 196 minutes of church audio.
- It does not buy a code-signing certificate. Neither platform has one; that is a
  purchase, not a commit.
- It does not close RG-32, which is open on purpose and wants a second and third
  Sunday.
