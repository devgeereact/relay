# Relay — Launch Checklist

**One gate list.** There used to be two — the retired product audit's §16 and
[RELAY_GAP.md](RELAY_GAP.md) §25 — and they disagreed. This is the only one.

**Every box either names the command that ticks it, or says plainly that it has never been
checked.** A ticked box with no instrument behind it is how this repository shipped a readiness
checklist claiming a macOS signing certificate that has never existed. Do not tick from memory.

`✅` verified by a command that was run · `⚠️` partially verified, with the limit stated ·
`⬜` **never checked** · `❌` checked and failing.

Last run: **2026-09-05**, against `0.1.0-4`. Findings live in
[RELAY_GAP.md](RELAY_GAP.md) §23; the reasoning is [RELAY_V1_AUDIT.md](../RELAY_V1_AUDIT.md).

---

## 1. The build

| | Gate | How |
|---|---|---|
| ✅ | Frontend suite passes | `npx vitest run` — the counts live in [QA_HARNESS.md](QA_HARNESS.md) §0, which is the register; this row is about the gate, not the number |
| ✅ | Rust suite passes | `cd src-tauri && cargo test` — same: §0 owns the count. This row said *942 / 70* and *644* for three days after both moved |
| ✅ | Formatting | `cargo fmt --all -- --check` |
| ✅ | Lints, warnings denied | `cargo clippy --all-targets -- -D warnings` — **this failed on 2026-09-02 and was fixed (RG-82). Run it; do not assume it** |
| ✅ | Frontend builds | `npm run build` |
| ✅ | The three version files agree | `npm run version:check` |
| ✅ | No dead commands, no orphan controls | `node scripts/qa-inventory.mjs` → 133/133, 1 intentional orphan |
| ✅ | Every citation resolves | `npx vitest run src/lib/crossrefs.test.js` |
| ✅ | The packaged binary builds and launches | `npm run tauri build` → `.app` + `.dmg`, 0 warnings; launched 2026-09-03 with an isolated `RELAY_DB_PATH` and printed **exactly one** boot heartbeat. **CI's macOS job is still compile-only and says so** — this box is ticked by a human running the command |
| ✅ | The hardened runtime does not kill the microphone | `./scripts/sign-local.sh` → `flags=0x10002(adhoc,runtime)`, mic entitlement present, usage string present. **Rule 17's trap reproduced without a certificate** |
| ✅ | The LAN surface behaves in production, not just in tests | `curl` against the running bundle, re-run 2026-09-05 on a build carrying the ranged-media change: `output.html` 200 + kiosk CSP + `nosniff`; `GET /api/black` → **405, `Allow: POST`, no CORS wildcard**; traversal → 404; `/media/<id>` 200 + `Accept-Ranges`, `Range: bytes=500-599` → **206 + `Content-Range: bytes 500-599/1024`, byte-exact**, and a range past the end → **416** |
| ❌ | The update channel resolves | `npm run updater:check` — **HTTP 404 on both configured endpoints (RG-83)**, re-verified 2026-09-05. Every release so far is a draft AND a pre-release, and GitHub's `/releases/latest/` excludes both. Publish a full release, then run the **Update channel** workflow from the Actions tab and watch it go green |
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
| ⬜ | **A database from a released build survives the corpus repair** | RG-102 … RG-105. `sqlite3 "$HOME/Library/Application Support/com.relay.app/relay.db" "PRAGMA user_version; SELECT COUNT(*) FROM verses;"` before an upgrade and after. Held by tests; never run on a machine that has recorded real services |

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
| ⚠️ | Dev-toolchain advisories | 10, all `vite`/`vitest`/`svelte-hmr`/`esbuild` — none shipped, and **the two reachable ones needed the dev server to be on the LAN**, which it no longer is by default (RG-98, DECISIONS §65: `RELAY_DEV_LAN=1` opts in) |
| ✅ | Rust dependency advisories | `cargo audit` → **0 vulnerabilities** (RG-101). Run for the first time on 2026-09-04, when it found three; all three went by lockfile update, including the two 7.5-HIGH `quick-xml` ones this register had recorded as needing an upstream Tauri bump — `plist` 1.10.0 already depends on the fixed version. The 18 remaining rows are warnings: unmaintained GTK3 **Linux** bindings, which neither shipped platform builds. **A CI job now runs it on every push**, beside `npm audit --omit=dev` |
| ✅ | Mutating LAN routes require POST and are denied CORS even on success | `main::remote_mutates` |
| ⚠️ | The CSP | **Narrowed 2026-09-05 (RG-85)**: `img-src`/`media-src` are `http://*:8032` and `connect-src` is `ws://*:8031` — Relay's own ports, any host, because a LAN address cannot be named in a static policy. Held by `qa::kiosk_headers::the_console_policy_allows_relays_own_media_url_and_no_other_host`, and the packaged build boots and prints its one heartbeat. **Still ⚠️ because `tauri dev` does not exercise the CSP and nobody has watched a background video paint on a projector under it** |
| ✅ | Only a page Relay served may join the kiosk feed | **RG-108, DECISIONS §64.** Probed against the packaged binary: `101` for no origin, `:8032` on two hosts, `:5032` and `tauri://localhost`; **`403`** for `evil.example.com`, `null`, a LAN host on `:3000` and an `https://` origin. `RELAY_KIOSK_ANY_ORIGIN=1` is the escape hatch and the refusal names it. **What no instrument here could check: the `Origin` a real OBS browser source sends** |
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
