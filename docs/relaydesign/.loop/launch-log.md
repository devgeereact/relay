# Launch & Startup — design loop log

Section 1 of `docs/relaydesign/relayscreens.md`, everything after the Splash.

Reference: **there is no mockup PNG for any of these eight screens.** The only
source is `docs/relaydesign/relay-designsystem.png` v1.0 (tokens, type scale,
mode colours, cards, progress indicators, chips). So this was a build against the
DESIGN SYSTEM, not a pixel-match against a screen mockup — the loop prompt's
"compare against `<REF>`" step compared each capture against the token sheet and
against the already-shipped Splash, which is the chrome these inherit.

**Compare method: PIXEL**, for all eight. They render standalone (no backend),
so they were captured in headless Chromium at 1536×1024, DPR 1,
`reducedMotion: reduce`, against `vite` serving a temporary preview page —
`boot-preview.html` / `boot-preview.js` at the repo root, which mounted one
screen per query param with a fixture pushed into the `checks` store. **Both
files were deleted after the final capture**; nothing about them shipped.

This is the same exception the Splash log describes: the *console* still cannot
be captured from the Tauri webview on this machine, but a screen with no live
dependencies can.

Build gate, every iteration: `npm run build` clean + `npx vitest run`
**167/167** (was 140 — 26 new in `boot.test.js`, 1 new in `ipc.test.js`).

---

## What was built

| Screen | File | State |
|---|---|---|
| Boot Diagnostics | `src/lib/boot/BootDiagnostics.svelte` | built · pixel |
| Hardware Check | `src/lib/boot/HardwareCheck.svelte` | built · pixel |
| Plugin Loading | `src/lib/boot/PluginLoading.svelte` | built · pixel |
| Database Migration | `src/lib/boot/DatabaseMigration.svelte` | built · pixel |
| Recover Previous Session | `src/lib/boot/RecoverSession.svelte` | built · pixel |
| Safe Mode Startup | `src/lib/boot/SafeModeStartup.svelte` | built · pixel |
| Crash Report Recovery | `src/lib/boot/CrashReportRecovery.svelte` | built · pixel |
| Update Available | `src/lib/boot/UpdateAvailable.svelte` | built · pixel |
| License Verification | — | **DROPPED**, see below |

Supporting, not screens: `boot.js` (state machine + persisted boot record),
`probes.js` (the real Tauri calls), `BootShell.svelte` (shared chrome),
`CheckList.svelte` (the check rows), `BootSequence.svelte` (the sequencer).
Shared CSS is one `.b-*` block in `src/app.css` — eight screens, one copy.

---

## License Verification — deliberately not built

The screen list has it; `CLAUDE.md` says Relay is **MIT, free, open source**, and
`docs/DECISIONS.md` has no activation/seat decision in it. An activation screen
would be the first thing in the product to contradict the licence. Raised with
the human, who chose to **drop it and record the contradiction** rather than
build it or quietly invent a "licence acknowledgement" variant.

Note this also reconciles the screen-count table, which says **8** for Launch &
Startup while the bullet list has 9 entries.

---

## Iteration 1 — first render

Captured: `launch-{diagnostics,hardware,plugins,migration,crash,safemode,update,recover}-1.png`.
No console errors, no page errors, every screen mounted with a non-trivial DOM.

Read back against the token sheet. Three real defects:

1. **The "NOT PROBED" chip rendered as a full-width block.** `.b-check .txt span`
   is a *descendant* selector and was also matching the chip nested inside the
   `<b>`, so `display:block` won and every stub row's layout broke into three
   lines with a stretched pill. Fixed to a direct-child selector
   (`.b-check .txt > span`) and the label became a flex row.

2. **Boot warnings were CYAN — a mode-colour violation.** Cyan means exactly one
   thing in this app: *the AI is guessing* (a paraphrase match). The NDI
   "not available in this build" row was wearing it. Amber was already out (tally
   light). Repointed to **amethyst**, which is the launch sequence's own accent
   and whose app-wide meaning — "not reaching the screens" — is what these
   warnings actually say. **This is an inferred value**: the design sheet
   publishes no warning colour that is not already a mode colour.

3. **Long notes ellipsised mid-sentence.** The NDI row read
   `not available in this build — needs the NDI S…`, truncating the only
   actionable half. Notes now wrap instead of ellipsising.

Also: content sat pinned to the top of a tall viewport. Changed `.b-inner` to
`margin:auto 0` rather than `justify-content:center` on the scroll container —
centring a flex child in an overflow container clips its top unreachably.

And: the crash card's opt-in checkbox rendered as the default **white square** on
a near-black card. `accent-color` only paints a *checked* box, and telemetry is
opt-in — so unchecked is the state literally everyone sees. Added `.b-check-box`.

## Iteration 2 — verify

Re-captured all eight (`*-2.png`). Read back: chip is inline and small, NDI row
is amethyst and wraps to full text, stage content is optically centred, the
checkbox is dark with an amethyst tick. No new fixable diffs against the token
sheet. **Stopping** — the remaining differences are all "there is no mockup to
differ from".

---

## Values inferred rather than read from the design system

| Value | Inferred as | Why |
|---|---|---|
| Boot warning colour | `--v-amethyst2` | The sheet's only warning-ish colours are amber and cyan; both are mode colours already spoken for. |
| Stage rail (1·2·3·4 breadcrumb) | amethyst ring on current, green tick on done | The sheet's STEPPER component, restyled to the boot palette. |
| `.b-check` row height (13px v-padding) | — | Sheet publishes cards and table rows, not a check row. Derived from the 8pt scale. |
| `.b-stub` chip | Small Chips, at 9px | Sheet's small-chip style, one step down. |
| Gate card width (600px) | — | Between the sheet's card examples and the existing `.cheat` dialog (520px). |
| Stage dwell 700ms / cap 6000ms | — | Not a design-system concern; chosen so a clean boot is not four clicks. |

## Iteration 3 — the stubs made real (backend written)

The human's instruction: *"make them real and live."* So the six `probe: 'stub'`
rows got a Rust backend. **There are now no stubs anywhere in the check table**,
and `boot.test.js` asserts that.

New: `src-tauri/src/sysprobe.rs` (+ `system_hardware`, `probe_integrations`,
`migration_status` commands; `db::schema_report` / `db::manual_status_report`).

| Was a stub | Now |
|---|---|
| Processor | `available_parallelism` — threads this **process** may use, not the physical count. Warns under 4: whisper on the CPU will lag the preacher. |
| Memory | `sysinfo`, at the moment of the call. Warns under ~1.5 GB free — that is where a church laptop starts swapping mid-sermon. |
| GPU | **A build fact, not a hardware one.** Reports the whisper backends compiled into this binary (`metal`/`cuda`/`vulkan`/`coreml` cargo features, all off by default). Never the card in the machine. |
| Disk | Free space on the volume holding **app-data**, not the boot volume. Warns under 2 GB. |
| OBS / ATEM | A 300 ms TCP connect to :4455 / :9910. |

Database Migration stopped asserting and started **asking SQLite**
(`sqlite_master` + `pragma_table_info`).

### Three real bugs this shook out

1. **The migration manifest named two objects that do not exist.** `media` is
   actually `media_assets`, and `template_active` is not a table at all — it is
   a COLUMN, `templates.console_active`. The old hard-coded screen would have
   green-ticked both forever. The very first run of the real query failed and
   said so. This is the entire argument for the change, demonstrated on itself.

2. **`sysinfo` 0.32 returns `available_memory() == 0` on macOS — silently.** The
   screen would have read *"0.0 GB free of 25.8 GB — close other apps before the
   service"* on a completely healthy laptop, on every boot. `free_memory()` is
   not a substitute (macOS excludes cached/inactive pages: it reported 674 MB on
   the same machine that had 12.3 GB genuinely available). Fixed by requiring
   **sysinfo ≥ 0.39**, with the reason written into `Cargo.toml` and a
   regression test (`available_memory_is_actually_read`).

3. **Memoising the resolved value instead of the promise.** `hw ??= await
   invoke(…)` yields before it assigns, so concurrent callers each fire their own
   request — four commands where one was intended. Caught by a test asserting
   one call; harmless in the serial sequencer, and would have bitten the first
   time two rows ran at once.

### And one design regression, caused by making things real

With real probes, the **normal** state of a church laptop produces three
warnings on the Integrations screen every boot (no OBS on :4455, no ATEM on
:9910, no NDI SDK). The sequencer held for a click on anything short of clean —
so every single launch would have stopped dead on a screen saying nothing was
wrong. That is the "clicked through blindly" failure: a gate that fires every
time stops being read, and then the one boot that mattered gets the same reflex.

Now **only a failure holds**. A warning lingers (2.6 s) and advances itself. That
required a run token in `BootSequence.svelte`, because the Continue button is on
screen during the dwell and clicking it would otherwise start a second loop
racing the first through the remaining screens.

### Wording that had to stay careful

Relay speaks neither the OBS WebSocket protocol nor ATEM's, so it may not report
"OBS is running" — only that *something answered on the port a default install
would use*, and that Relay cannot control it. Both the probe and a test enforce
that phrasing.

## Iteration 4 — verify, with real numbers

Re-captured all eight. The hardware and integration fixtures are now the
**verbatim output of `cargo test sysprobe::tests::print_hardware -- --ignored
--nocapture`** on this machine, pushed through the *same* probe functions the app
uses — so the values on the screenshots are values the real probes produced.

Only fix: the whisper-acceleration note wrapped and orphaned the word "in" on its
own right-aligned line. Shortened to one line. **Stopping.**

Gate after this iteration: `cargo fmt` + `clippy -D warnings` clean,
**264 Rust** (was 250) and **191 frontend** (was 140) tests passing,
`npm run build` clean.

## Honest gaps

- **No stub checks remain** — every row is a real read. The stub *machinery* is
  kept anyway (`probe: 'stub'` → renders `unknown` + a "not probed" chip, never a
  pass), because the rule outlives any particular gap and the next unprobeable
  thing must not be tempted into a green tick. `boot.test.js` covers both, and
  the guard test **fails** if the branch in `runStage()` is removed — verified by
  removing it.
- **Relay still cannot control OBS or ATEM.** The probes report port
  reachability, nothing more, and say so in those words. Making them real did not
  make them a control channel.
- **NDI remains parked** (proprietary SDK, per CLAUDE.md) and is reported as a
  warning naming the reason.
- **Database Migration is a verification screen, not a progress screen**, and
  says so. `run_migrations` executes once, synchronously, before the webview
  exists; there is nothing to stream — so it asks SQLite what exists instead.
- **The whole sequence was never seen inside the Tauri window.** It was captured
  standalone against fixtures (real ones, taken from the Rust probes, but
  fixtures). The wiring into `App.svelte` (splash → sequence → console) and the
  `system_hardware` / `migration_status` / `probe_integrations` commands running
  over real IPC are **code- and test-level only**, unverified on this machine.
  Worth a human launching the app once.
- **GPU acceleration is compiled out of every shipped build.** The cargo features
  exist and are wired (`--features metal` etc.) but nothing enables them yet, and
  no GPU build has been produced or benchmarked. The screen says "CPU only".
- These are built against tokens. If a mockup PNG for any of the eight turns up
  later, expect real layout diffs.

## Things this touched outside section 1

- `src/lib/crash.js` — now records the crash into the boot record so the *next*
  launch can offer Crash Report Recovery, and marks a clean exit on `beforeunload`.
- `src/App.svelte` — hosts the sequence; honours safe mode by disarming detection
  the moment the engine attaches; safe mode outranks rehearsal in the topbar and
  footer state.
- `src/lib/views/Settings.svelte` — **the way out of safe mode.** Both launch
  screens promise "turn it off in Settings"; without this control that promise is
  a lie told to someone already having a bad morning.
- `src/lib/ipc.test.js` — the contract test only scanned `capture.js`. The boot
  probes call Tauri directly (a probe wants the raw failure, not a wrapper that
  swallows it), which put them outside the contract entirely. Now covered, plus a
  guard-the-guard assertion so a refactor cannot silently empty the regex.
