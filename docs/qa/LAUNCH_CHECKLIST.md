# Relay — Launch Checklist

**One gate list.** There used to be two — the retired product audit's §16 and
[RELAY_GAP.md](RELAY_GAP.md) §25 — and they disagreed. This is the only one.

**Every box either names the command that ticks it, or says plainly that it has never been
checked.** A ticked box with no instrument behind it is how this repository shipped a readiness
checklist claiming a macOS signing certificate that has never existed. Do not tick from memory.

`✅` verified by a command that was run · `⚠️` partially verified, with the limit stated ·
`⬜` **never checked** · `❌` checked and failing.

Last run: **2026-09-02**, against `0.1.0-4`. Findings live in
[RELAY_GAP.md](RELAY_GAP.md) §23; the reasoning is [RELAY_V1_AUDIT.md](../RELAY_V1_AUDIT.md).

---

## 1. The build

| | Gate | How |
|---|---|---|
| ✅ | Frontend suite passes | `npx vitest run` → 927 passing, 68 files |
| ✅ | Rust suite passes | `cd src-tauri && cargo test` → 629 passing, 17 ignored |
| ✅ | Formatting | `cargo fmt --all -- --check` |
| ✅ | Lints, warnings denied | `cargo clippy --all-targets -- -D warnings` — **this failed on 2026-09-02 and was fixed (RG-82). Run it; do not assume it** |
| ✅ | Frontend builds | `npm run build` |
| ✅ | The three version files agree | `npm run version:check` |
| ✅ | No dead commands, no orphan controls | `node scripts/qa-inventory.mjs` → 132/132, 1 intentional orphan |
| ✅ | Every citation resolves | `npx vitest run src/lib/crossrefs.test.js` |
| ⬜ | The packaged binary builds and launches | `npm run tauri build` — **CI's macOS job is compile-only and says so** |
| ⬜ | A clean machine installs it | never done |

## 2. The live path

| | Gate | How |
|---|---|---|
| ✅ | Only `Direct` can auto-fire, enforced before thresholds | `router::decide`; `router::semantic_can_never_auto_fire` |
| ✅ | A paraphrase cannot be promoted by any number | `router::corroboration_never_promotes_a_paraphrase` |
| ✅ | One window may put at most one verse on a wall | `main::rank_for_wall` |
| ✅ | Content leaves by one door, and the validator is on it | `channels::broadcast_content` has one caller |
| ✅ | Rehearsal cannot reach a congregation screen — including the stage monitor | `nothing_reaches_the_stage_monitor_during_a_rehearsal` |
| ✅ | Panic controls bypass the validator and can never report an unachieved success | `panic.test.js` |
| ✅ | `Esc` inside a dialog does not clear the screens | `shortcuts.test.js` |
| ✅ | Detection gate beats the SPEC target | `cargo test eval::tests::print_scorecard -- --nocapture` → 0.0% wrong-verse over 74 cases |
| ✅ | No `unwrap`/`expect` on the live path | grep, test-boundary aware, across the seven service modules |
| ⚠️ | The whole path, driven end to end | `cargo test e2e::` → 38 tests — real commands, real DB, **mock window** |
| ⬜ | The whole path, on a projector, in a room | one service on 2026-08-30; no second |

## 3. Speech

| | Gate | How |
|---|---|---|
| ✅ | Decode cost fits the cadence | `RELAY_BENCH_MODEL=… cargo test --release decode_cost -- --ignored` — base 56 ms, small 151 ms, turbo 594 ms at an 8 s window |
| ✅ | The cadence floor is a whole number of chunker hops | `stt::adaptive_cadence` |
| ✅ | A quiet preacher is still heard — levels are learned, never assumed | `cargo test audio::gate -- --ignored` |
| ⬜ | **Word error rate, any language** | needs 30 minutes of sermon audio · `bench/README.md` |
| ⬜ | First-partial and end-to-end p95 latency | `stt::realtime::live_transcript_latency`, same recording |
| ⬜ | The book aliases are right | needs a native speaker · `docs/CONTRIBUTING.md` |

## 4. Output and hardware

| | Gate | How |
|---|---|---|
| ✅ | A screen that stops answering cannot read On Air | `outputhealth.test.js` |
| ✅ | A template swap is live without re-copying a URL | channel-keyed output URLs, DECISIONS §29 |
| ⬜ | A real projector | never |
| ⬜ | A second monitor, OBS, a capture card | never |
| ⬜ | Any microphone other than this laptop's | never |

## 5. Release

| | Gate | How |
|---|---|---|
| ✅ | The signing gate is per-platform and fails loud on a real tag | `release.yml` — two certificates, two verdicts |
| ❌ | **A macOS code-signing certificate exists** | `gh secret list` → **zero of the six `APPLE_*`** |
| ❌ | **A Windows code-signing certificate exists** | `gh secret list` → **zero of the eight `AZURE_*`/`WINDOWS_*`** |
| ✅ | The updater manifest is signed | `TAURI_SIGNING_PRIVATE_KEY` is set |
| ❌ | **The updater endpoint resolves** | `curl -sI …/releases/latest/download/latest.json` → **HTTP/2 404** (RG-83) |
| ⬜ | An update has been watched installing, once | never |
| ⬜ | The microphone survives the first correctly-signed macOS build | `npm run tauri build && ./scripts/sign-local.sh` — **free, and it reproduces rule 17's trap without a certificate. Run it before buying anything** |
| ⬜ | The offline bundle onto a stick, carried to a machine with no internet | `node scripts/offline-bundle.mjs` |

## 6. Security and privacy

| | Gate | How |
|---|---|---|
| ✅ | No secrets in the tree | `git grep`; `.env` untracked, `.env.example` tracked |
| ✅ | No production dependency advisories | `npm audit --omit=dev` → 0 |
| ⚠️ | Dev-toolchain advisories | 10, all `vite`/`vitest`/`svelte-hmr`/`esbuild` — none shipped |
| ⬜ | Rust dependency advisories | `cargo audit` — not installed |
| ✅ | Mutating LAN routes require POST and are denied CORS even on success | `main::remote_mutates` |
| ⚠️ | The CSP | tight where it counts; grants `http:`/`ws:` to any host (RG-85). **`tauri dev` does not exercise it — verify against a packaged build** |
| ✅ | Nothing a preacher said reaches the timeline | `timeline_tests`, and `timeline.test.js` from the other side |
| ✅ | Crash reports drop free text wholesale rather than filtering it | `telemetry::scrub` |

## 7. The people

| | Gate | How |
|---|---|---|
| ⚠️ | A real service, start to finish | once, 2026-08-30, by the author |
| ⬜ | **A service run by somebody who did not write it** | never — and this is the gate that decides general release |
| ⬜ | A second and third service, watching for drift | never |

---

## The decision this checklist supports

**NOT READY for general release · READY WITH CONDITIONS for a supervised pilot.**
Five things block a general release and **two of them are not commits** — a certificate is a
purchase and a published release is an action. [RELAY_GAP.md](RELAY_GAP.md) §24 owns the
conditions; [RELAY_V1_AUDIT.md](../RELAY_V1_AUDIT.md) §1 owns the reasoning.
