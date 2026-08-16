# Working Agent — evidence baseline

Companion to [Working-Agent.md](Working-Agent.md). This is what the repository can already
prove, verified by reading the tests rather than trusting the count. It is the audit's starting
line: an agent that re-derives all of this burns a run rediscovering things that were pinned
deliberately, and — worse — may "find" a fixed bug and file it.

Everything below was checked against the working tree on **2026-08-14**. Rows marked
**unverified** are honest gaps in *this document*, not claims about the code, and each names
who resolves it.

---

## 1. The instruments that already exist

| Layer | Present today | Where |
|---|---|---|
| **A — Command E2E** | Yes. 14 tests driving the real commands against a real in-memory DB through the real router and pipeline | `src-tauri/src/e2e.rs` (643 lines) |
| **B — Component mount** | Yes, and used — but only twice | `src/lib/inspector.test.js` mounts `DetectionInspector`; `src/lib/layers.test.js` mounts `TemplateRender` |
| **C — Static contract** | Yes, one exemplar | `src/lib/ipc.test.js` — command names both directions, event listeners, and a `greet`-has-one-caller assertion |
| **D — Live app** | Exists as a surface, is not exercised by any test | `channels.rs` serves `:8032`; `main.rs::remote_api` handles `search / fire / next / prev / clear / black / live`. Kiosk hub on `:8031` |
| **E — Human** | The bench harness is built and pointed at nothing | `bench/README.md` says what to record; `bench/.gitignore` refuses to let sermon audio into the repo |

Totals: **33 frontend test files**, **47 `.svelte` files** (23 of them views), **~334
`<button>` occurrences**, **114 registered Tauri commands**, **18 tables** in
`docs/data/schema.sql`.

Layer B is the biggest under-used asset in the repo. The pattern works, it is proven twice, and
it is the only instrument that can see a control at all.

---

## 2. The six distinctions

| Distinction | Pinned? | Evidence |
|---|---|---|
| **Rehearsal ≠ Live** | **Yes, both doors** | `e2e.rs::nothing_reaches_the_congregation_during_a_rehearsal` and `::nothing_reaches_the_stage_monitor_during_a_rehearsal` — the second exists because the first watched Tauri events and was therefore blind to `channels::stage_next`, which publishes to the kiosk and emits nothing. Frontend side: `rehearsal.test.js` (off by default; throws rather than lying when the backend refuses; a nonsense answer counts as NOT rehearsing) |
| **Suggestion ≠ Auto-fire** | **Yes** | `router::decide` caps Semantic and Ambiguous at `Suggest` by construction; `e2e.rs` drives a paraphrase at maximum confidence and asserts it cannot reach the wall |
| **Paraphrase ≠ Direct** | **Yes** | `detect.test.js`: a spoken reference is HEARD, a paraphrase is not "however high its score", the three methods get three distinguishable keys, and *"a paraphrase NEVER shows a percentage — at any score"* |
| **Clear ≠ Blackout** | **Yes, as separate contracts** | `panic.test.js`: `clearScreens` returns FALSE on backend failure and the caller must not flash success; a failed clear raises the panic banner; *"blackout has the identical contract — it is a panic control too"*; a success clears a stale warning; no crying wolf with no backend at all |
| **Cued ≠ On Air** | **Yes** | `transport.test.js`: Esc/clear takes the plan off air but REMEMBERS the position; blackout the same; a FAILED hand-fire leaves the plan exactly as it was; clearing twice is idempotent and does not lose the position |
| **Preview ≠ Programme** | **Yes, now** | `src/lib/liveoutputrail.test.js` — 17 tests, none skipped: staging reaches nobody, TAKE hands the slide to the parent and fires nothing itself, TAKE is dead with nothing staged / in safe mode / mid-take, the monitor is honest in every state, amber never sits beside a staged slide, and the operator is still told when the wall is hot behind one |

**Preview ≠ Programme was the gap you flagged, and closing it found two things.**

**The component that read like the safety model was not in the product.**
`src/lib/views/library/PreviewProgram.svelte` — 312 lines, two panes, and a header comment
stating the danger exactly (*"Relay used to fire on a single click. One slip of a trackpad put
the wrong scripture on a wall in front of a congregation, instantly, with no undo"*) — was
imported by **nothing**, and fourteen tests were written against it before
`scripts/qa-inventory.mjs` said so. It has been deleted; the single-pane rail is a deliberate
design evolution, not an unfinished migration. The surface that ships is
`LiveOutputRail.svelte`: one pane, time-multiplexed — staged content when something is staged,
live content otherwise.

**On that surface, amber lied.** With verse A live and verse B staged, the pane rendered verse
B while the badge — `onAir = !!$live && !$screenBlack`, which knew nothing about `preview` —
rendered amber, a pulsing dot, and "Live". The header said "· Preview" in small grey text
beside it; the louder indicator was the wrong one.

**Fixed.** The badge now describes the **pane** (staged → grey "Preview"), and a second smaller
chip carries the fact the badge no longer can: `.lo-behind`, reading "Wall live" in amber or
"Wall: rehearsal" in amethyst, present only when the wall is genuinely hot — absent on clear
screens and during a blackout, because a warning that fires in the ordinary case stops being
read. Verified to fail before the fix with `expected 'r-badge amber' not to match /amber/`.

---

## 3. Adjacent guarantees already pinned

Useful to know so no agent re-files them:

- **`NavResult` is four distinguishable outcomes** and each is explained to the operator —
  `nav.test.js` (eight tests, including "a successful step says nothing — the wall IS the
  feedback" and "an unknown outcome degrades to silence, never to a crash"). The remote surface
  was the door that discarded it with `Ok(_)`; that is fixed and covered by
  `e2e.rs::the_remote_says_which_outcome_its_nav_had_not_merely_ok`.
- **Suggestion lifetime** outlives the router's repeat cooldown so a human can read it, and an
  undated suggestion is treated as stale rather than immortal — `suggestions.test.js`.
- **`stopCapture` cannot swallow** — `micstop.test.js`, written because one bare `catch {}`
  around both the bridge import and the command printed "Start listening" over a live mic.
- **`greet` has exactly one caller** — `ipc.test.js` fails if any file other than `App.svelte`
  mentions it. The heartbeat's value is the count.
- **Fresh-install seeding** — `db/mod.rs::seeds_full_kjv` (>31,000 verses) and
  `::seeds_the_builtin_templates` (five built-ins plus presets, and the lyrics template by
  name).
- **Migration retryability** — `ensure_service_plans_is_retryable`,
  `ensure_voice_profiles_is_idempotent`, and the schema-report tests that guard the Database
  Migration screen against drawing green ticks from a hard-coded list.
- **macOS mic entitlement + usage string** — `models::config_boots`.

---

## 4. What a fresh install actually contains

From `db::init_fresh` — schema, then `seed`, then `ensure_tables`, then a stamped
`user_version`:

| Seeded | Why it is content, not demo data |
|---|---|
| 31,100 KJV verses + the translation row | Bundled at `src-tauri/data/kjv.json` via `include_str!`, required to build. A church with an empty verse table has a broken install |
| 5 built-in templates + presets | `templates.rs::seed_templates`. Includes "Worship Lyrics", added because every earlier built-in was scripture-shaped and put the song title where the words should be |
| Default output channels | `channels.rs::seed_channels` |
| One active voice profile | `ensure_tables` guarantees it even on a bare in-memory DB |

**Not seeded, therefore the real subject of the cold-start audit:** `service_plans`,
`plan_items`, `songs`, `song_sections`, `song_arrangements`, `saved_scripture`,
`announcements`, `media_assets`, `services`, `transcripts`, `detections`, `cues`,
`app_settings`.

**First pass, from `node scripts/qa-inventory.mjs`.** It traces
`INSERT → db fn → #[tauri::command] → capture wrapper → a component that imports it`, following
store-internal calls (so `startService`, which only `beginService` ever calls, resolves
correctly). Structure is reliable; intent is heuristic. **R1 verifies before filing, including
the rows this passes** — a tool that agrees with you is not evidence.

Every table above resolves to a create path except one:

- **`song_arrangements` — no create path.** `save_arrangement` is registered, `saveArrangement`
  exists in the store, and **no component imports it**. If that holds, a user cannot save a
  song arrangement at all, and CLAUDE.md's "every one of the 114 commands has a frontend
  caller" is true at the wrapper level and false at the level that matters. Filed as **F3** in
  [Working-Agent.md](Working-Agent.md) §9.

Still worth R1's attention even though the tool is content:

- **`translations`** — only KJV is seeded, and the tool marks it `seeded-only` correctly. The
  question it cannot answer is whether that is a gap: the Library and the planner both treat
  translation as a first-class concept, so is multi-translation effectively "the bundled one"?
- **`app_settings`** — writable through eight wrappers. Worth confirming nothing user-visible
  depends on a key only ever written by a code path that no longer runs.

---

## 5. The fixture trap, written down

`e2e.rs::app()` does one thing a fresh install does not:

```rust
// A fresh install seeds templates but does NOT assign a per-content-type
// override — `tpl_scripture` is only written when the operator picks one …
db::set_content_template(&conn, "scripture", Some(tpl))
```

That is correct there: without it the "every fire carries its template" assertion would be
vacuous. It is disqualifying for a cold-start audit — an audit that starts from it inherits the
convenience it exists to detect.

**Closed.** `src-tauri/src/qa.rs::bare_app()` is `init_fresh` and nothing else, and `e2e::app()`
is now that fixture plus its one documented difference, so the difference is visible in three
lines instead of buried in a fifty-line copy. The fixture is held honest by a test rather than
a comment: `qa::tests::the_bare_fixture_is_a_first_launch_and_nothing_more` asserts no
content-look is chosen except `tpl_song` — which **is** seeded, deliberately, because every
other built-in is scripture-shaped and a lyric rendered through one put the song title where
the words should be. Writing that test is how that fact was found; it had been assumed absent.

The second harness test, `the_kiosk_door_is_watchable_and_is_not_the_wall`, asserts that
`stage_next` reaches the kiosk and emits **no** Tauri event — so if the two doors ever merge,
the rehearsal-containment tests built on `qa::Kiosk` cannot start passing by seeing nothing.

---

## 6. What no instrument here can reach

This list is the audit's most valuable output, not its excuse. Each item is BLOCKED and needs a
person.

| Area | Why it is blocked | What a human must do |
|---|---|---|
| Anything visual | This machine cannot screenshot the app | Open the app; check layout, contrast, spacing, the dark palette, and that amber only ever appears when something is genuinely live |
| Window resize, multi-monitor, high DPI | No window | Resize the Live console to a small laptop screen; confirm no critical control disappears |
| Microphone, rooms, accents | No audio device, no room | `RELAY_RECORD_WAV`, `RELAY_AUDIO_RMS=1`, `RELAY_STT_TIMING=1`; then replay through `RELAY_BENCH_WAV` at church-laptop levels. Audio levels are LEARNED, never assumed — three individually reasonable thresholds once made Relay deaf to a quiet preacher, 94% voiced at studio level and 2% at a church laptop |
| Word error rate, any language | Never measured. The ruler is built (`stt::bench::wer`) and pointed at nothing | Thirty minutes of a real preacher on tape, per language. `bench/README.md` says what to record |
| Yoruba / Swahili / Hausa aliases | No native speaker has reviewed the 66×3 table | A native speaker, per language |
| OBS, ATEM, ProPresenter, Companion, Stream Deck | Hardware and software not present | Connect each; verify a failed connection shows a humanised message with a recovery action, never a raw socket error |
| NDI | Parked by decision — needs a proprietary SDK; `open_ndi_output` returns a clear error on purpose | Nothing. Confirm the error is still clear and still honest |
| The macOS microphone under a signed build | The mic dies on the **first correctly-signed build**: notarization forces the hardened runtime, under which opening an input device without `com.apple.security.device.audio-input` is TCC-killed, and without `NSMicrophoneUsageDescription` the app is terminated the instant it asks. `tauri dev` and unsigned pre-releases both work fine | `npm run tauri build && ./scripts/sign-local.sh`, then actually speak into it |
| CSP | `tauri dev` does not exercise it — Tauri loads the Vite `devUrl`, and `app.security.csp` only applies to bundled assets | `npm run tauri build`, then run the packaged binary |
| An actual congregation | — | A Sunday |

---

## 7. The first hour, spent

Not on an agent — and it produced three findings before one ran, which is the argument for
building instruments first.

1. ✅ **The bare cold-start fixture** (layer A) — `qa.rs`, §5. Found that `tpl_song` is
   deliberately seeded, a fact this document had asserted the opposite of.
2. ✅ **The Preview ≠ Programme test** (layer B) — found that the component it was written
   against is not in the product, and that amber lies on the one that is. §2.
3. ✅ **The create-path trace** (layer C) — `scripts/qa-inventory.mjs`. Found
   `song_arrangements` has no path from any rendered control.

**Still open, and the natural next step:** turn the create-path trace into an *assertion*.
Right now it is a report someone has to run and read. A test in the `ipc.test.js` style —
every table that is neither seeded nor runtime-only must terminate in a rendered control,
with today's one known gap listed explicitly so a **new** gap fails the build — would keep
holding after the audit is over, and runs in a second. It was deliberately not written yet:
an assertion whose expected-value list was never checked by a human is a test that pins
whatever happened to be true the day it was generated.

Then the agents, for the things a test cannot enumerate in advance.
