# Wave designs and work orders — the eleven that were built to

**Eleven design documents, merged into this file on 2026-09-21** and otherwise untouched.

Every one describes work that has since landed. They are kept as the record of what was intended
and why, and they are **not specification**: a ruling that lives only in a design has not been
made. The rulings are in `../DECISIONS.md`, the findings in `../qa/RELAY_GAP.md`, and where a
design and the shipped code disagree the code is what shipped.

Each document keeps its own status line, which describes the day it was written rather than
today. Nothing is edited except the paths of citations, retargeted so every citation still
resolves.

**What is in this file, in order:**

- `2026-09-15-wave0-prerequisites.md` — Wave 0 — Prerequisites Implementation Plan
- `2026-09-15-wave1-settings-splash-routing.md` — Wave 1 — Settings, Splash and Routing Implementation Plan
- `2026-09-15-wave2-templates-themes.md` — Wave 2 — Templates ⊕ Themes Implementation Plan
- `2026-09-15-timers-templates-stage-design.md` — Design — timers, templates, stage, settings, routing
- `2026-09-16-wave3-timers.md` — Wave 3 — Timers Implementation Plan
- `2026-09-16-wave4-stage-planner.md` — Wave 4 — Stage and Planner Implementation Plan
- `2026-09-16-wave5-shelf-names-seal.md` — Wave 5 — the shelf, the names and the seal: implementation plan
- `2026-09-16-shelf-names-and-seal-design.md` — Design — the shelf, the names and the seal (Wave 5)
- `2026-09-17-consolidation-design.md` — Consolidation — three roots, one branch, and the defects the waves left behind
- `2026-09-18-channel-looks-design.md` — Channel looks — a per-screen, per-kind template
- `2026-09-19-stage-planner-media-propresenter-design.md` — Stage reach, Planner craft, media transport and ProPresenter

---

<!-- ===== was docs/archive/WAVE-DESIGNS.md, merged 2026-09-21, verbatim ===== -->

> **Archived 2026-09-21.** Historical work order. Wave 0 landed 2026-09-15; see `docs/REBRAND.md` §Status. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Wave 0 — Prerequisites Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the three enforcement gaps that later waves rely on, so that the guarantees they depend on are real rather than documented.

**Architecture:** Three independent repairs. Add a `REHEARSAL_VERDICTS` enumeration test modelled on the existing `FRAME_VERDICTS` one, so a publisher with no rehearsal gate fails a test instead of nothing. Make `fire_media` generic over `tauri::Runtime` so the last un-driveable fire path becomes testable. Remove a dead command name from two hand-written enumerations and make both assert registration so they cannot rot again.

**Tech Stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest, `cargo test`.

**Spec:** `docs/archive/WAVE-DESIGNS.md` (Wave 0)

## Global Constraints

- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` must be clean before every commit. CI enforces both.
- No `unwrap()` / `expect()` in code that runs during a live service. Test code may use them.
- Run `npm run build` before `cargo test` on a fresh tree, or two `channels` tests fail on a bare 404 because `dist/` is gitignored (RG-127).
- Read test counts from **the runner's own summary line**, never from a grep. `docs/qa/QA_HARNESS.md` §0 is the only register that carries values.
- Every new test must be verified to FAIL when the defect it describes is reintroduced. Test the bug, not the fix.
- Commit message bodies are normal English prose, not shorthand.
- Never commit to `main`. This work is on `feat/wave7-timers-templates-stage`.

---

### Task 1: The rehearsal gate becomes an enumeration

**Why this exists:** Retention is enumerated — `channels.rs:3304` `FRAME_VERDICTS` plus `every_kind_this_module_publishes_has_an_explicit_verdict`, which reads the module's own source and fails on any published kind with no verdict. Rehearsal gating is not. The list of gated publishers lives only in a doc comment at `channels.rs:845-863`. Wave 3 adds a publisher; today a publisher with no `rehearsing()` check fails no test. The repository has already been bitten by exactly this — `channels.rs:1113-1118` records that `stage_next` shipped ungated and was invisible to `e2e.rs`'s `Wall` because it emits no Tauri event.

**Files:**
- Modify: `src-tauri/src/channels.rs` — add to the existing `mod tests`, beside `FRAME_VERDICTS` (around line 3304)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `REHEARSAL_VERDICTS: &[(&str, bool, &str)]` — `(function name, is_gated, reason)` — in `channels.rs`'s `mod tests`. Wave 3 adds a row to it for the timer publisher.

**The five gated publishers today**, each calling `publish_kiosk` and each already checking `rehearsing`:

| function | line | `publish_kiosk` at |
|---|---|---|
| `broadcast_content` | 1041 | 1056 |
| `clear` | 1071 | 1081 |
| `black` | 1090 | 1098 |
| `stage_next` | 1120 | 1134 |
| `stage_alert` | 1147 | 1153 |

**The three deliberately ungated**, which reach the hub through `KioskHub` methods rather than `publish_kiosk` and have no `AppHandle` to ask: `KioskHub::set_transition` (`:1388`), `KioskHub::set_themes` (`:1437`), `KioskHub::set_template` (`:1449`).

- [ ] **Step 1: Write the failing test**

Add to `src-tauri/src/channels.rs`, inside `mod tests`, immediately after `every_kind_this_module_publishes_has_an_explicit_verdict`:

```rust
    /// Every function in this module that can put something on a LAN device, and
    /// whether a rehearsal must stop it.
    ///
    /// `true` means the function checks `rehearsing(app)` and returns early.
    /// `false` means it deliberately does not, and the third column is why — the
    /// same reason that must be at the call site.
    const REHEARSAL_VERDICTS: &[(&str, bool, &str)] = &[
        ("broadcast_content", true, "it carries what a congregation reads"),
        ("clear", true, "a rehearsal must not take a real wall down"),
        ("black", true, "a rehearsal must not black a real wall"),
        (
            "stage_next",
            true,
            "it leaked 'up next' to a live stage tablet mid-rehearsal, and it has \
             no Tauri emit, so the e2e wall test saw nothing wrong",
        ),
        (
            "stage_alert",
            true,
            "a word to the preacher is for a person, and a rehearsal has no person \
             waiting for it",
        ),
        (
            "set_transition",
            false,
            "configuration, not content. A screen that receives it looks identical \
             afterwards; gating it would leave every screen armed with the \
             pre-rehearsal transition once the operator went live",
        ),
        (
            "set_themes",
            false,
            "a palette, not content. It puts nothing a person reads on a screen",
        ),
        (
            "set_template",
            false,
            "a template, not content. Reassigning a screen's look is live by \
             design (DECISIONS §29), and suppressing it would leave a kiosk \
             rendering a template the operator has already replaced",
        ),
    ];

    /// THE ENUMERATION MUST GROW WITH THE MODULE, OR IT IS NOT AN ENUMERATION.
    ///
    /// Retention has had a scanner since rule 43; rehearsal gating has had a doc
    /// comment. That comment is honest about why it exists — `stage_next` shipped
    /// ungated and leaked to a live stage tablet — and a doc comment is exactly
    /// what failed to catch it. A sixth publisher added to this module with no
    /// `rehearsing()` check currently fails nothing.
    ///
    /// This reads the module's own source, finds every function that reaches a LAN
    /// device, and requires a verdict for each. For a function whose verdict is
    /// `true` it goes further and requires the gate to actually be IN the
    /// function body — an enumeration that only counted names would pass on a
    /// publisher whose check had been deleted.
    #[test]
    fn every_publisher_in_this_module_has_an_explicit_rehearsal_verdict() {
        let src = include_str!("channels.rs");
        let body = src.split("mod tests").next().unwrap_or(src);

        // (function name, its body) for every fn in the module, in source order.
        let mut fns: Vec<(&str, String)> = Vec::new();
        let mut current: Option<&str> = None;
        let mut buf = String::new();
        for line in body.lines() {
            let t = line.trim_start();
            let decl = t
                .strip_prefix("pub fn ")
                .or_else(|| t.strip_prefix("fn "))
                .or_else(|| t.strip_prefix("pub async fn "))
                .or_else(|| t.strip_prefix("async fn "));
            if let Some(rest) = decl {
                if let Some(name) = rest.split(['(', '<', ' ']).next() {
                    if !name.is_empty() {
                        if let Some(prev) = current.take() {
                            fns.push((prev, std::mem::take(&mut buf)));
                        }
                        current = Some(name);
                        buf.clear();
                    }
                }
            }
            if current.is_some() {
                buf.push_str(line);
                buf.push('\n');
            }
        }
        if let Some(prev) = current.take() {
            fns.push((prev, buf));
        }

        assert!(
            fns.len() > 20,
            "the scanner found only {} functions in this module — it has stopped \
             reading it, and a scanner that quietly narrows passes everything",
            fns.len()
        );

        // A function reaches a LAN device if it hands the hub a message.
        let publishes = |b: &str| {
            b.contains("publish_kiosk(")
                || b.contains("self.publish(")
                || b.contains("hub.publish(")
        };

        let mut found: Vec<&str> = Vec::new();
        for (name, b) in &fns {
            // `publish_kiosk` and `publish` are the plumbing, not publishers.
            if *name == "publish_kiosk" || *name == "publish" {
                continue;
            }
            if publishes(b) && !found.contains(name) {
                found.push(name);
            }
        }

        assert!(
            !found.is_empty(),
            "the scanner found no publisher at all — it has stopped reading this \
             module, and a scanner that quietly narrows passes everything"
        );

        for name in &found {
            let Some((_, gated, _)) = REHEARSAL_VERDICTS.iter().find(|(n, _, _)| n == name)
            else {
                panic!(
                    "`{name}` publishes to the kiosk hub and no one has said whether \
                     a rehearsal must stop it. Add it to REHEARSAL_VERDICTS with a \
                     reason, and if it is gated, add an e2e case that watches \
                     `qa::Kiosk` rather than `qa::Wall`."
                );
            };
            if *gated {
                let b = &fns.iter().find(|(n, _)| n == name).expect("found above").1;
                assert!(
                    b.contains("rehearsing("),
                    "REHEARSAL_VERDICTS says `{name}` is gated, and its body does \
                     not call `rehearsing(`. A verdict is not a gate."
                );
            }
        }

        for (name, _, reason) in REHEARSAL_VERDICTS {
            assert!(
                found.contains(name),
                "REHEARSAL_VERDICTS names `{name}`, which this module no longer \
                 publishes — a verdict about nothing"
            );
            assert!(
                !reason.is_empty(),
                "`{name}` has a verdict and no reason. The reason is the half a \
                 future reader needs."
            );
        }
    }
```

- [ ] **Step 2: Run the test to verify it passes on the current tree**

```bash
cd /Users/mrgee/WebstormProjects/relay/src-tauri
cargo test channels::tests::every_publisher_in_this_module_has_an_explicit_rehearsal_verdict -- --nocapture
```

Expected: PASS. The current tree is correct; the test exists to hold it that way.

- [ ] **Step 3: Verify the test actually catches the bug — delete a gate**

Temporarily remove the `rehearsing` early-return from `stage_alert` (`src-tauri/src/channels.rs:1148-1151`) and re-run.

Expected: FAIL with `REHEARSAL_VERDICTS says stage_alert is gated, and its body does not call rehearsing(. A verdict is not a gate.`

- [ ] **Step 4: Verify the test catches the other bug — add an ungated publisher**

Restore `stage_alert`. Now temporarily add, just above `fn publish_kiosk`:

```rust
pub fn scratch_probe<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    publish_kiosk(app, r#"{"kind":"scratch"}"#.to_string());
}
```

Re-run.

Expected: FAIL with `scratch_probe publishes to the kiosk hub and no one has said whether a rehearsal must stop it.`

- [ ] **Step 5: Remove the scratch probe and confirm green**

```bash
cd /Users/mrgee/WebstormProjects/relay/src-tauri
cargo test channels:: 2>&1 | tail -5
cargo fmt --all && cargo clippy --all-targets -- -D warnings
```

Expected: PASS, clippy clean.

- [ ] **Step 6: Widen the retention scanner to match**

`every_kind_this_module_publishes_has_an_explicit_verdict` reads `include_str!("channels.rs")` only, so `channel_template` — published from `main.rs:5691` and `:5706` — has no retention verdict.

**Do not concatenate the two files and keep the existing split.** The test does `src.split("mod tests").next()`, which on concatenated text cuts at *channels.rs's own* first `mod tests` and discards main.rs entirely — a scanner that reads less than before while looking wider, which is the failure this test exists to prevent. And main.rs carries `#[cfg(test)]` from line 16, so no test-stripping split works on it at all.

Scan main.rs **whole**, skipping comment lines. Verified: main.rs holds exactly one distinct frame literal, `"kind":"channel_template"`, twice, and no others anywhere in the file including its tests. A frame literal added inside a main.rs test would demand a verdict row, which is stricter than the channels.rs half and correct.

Extract the per-line matching already in the test into a helper, and call it twice:

```rust
        /// Every `"kind":"…"` literal in `src`, in order, without duplicates.
        /// Comment lines talk ABOUT frames without publishing any.
        fn kinds_in(src: &str, out: &mut Vec<String>) {
            for line in src.lines() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                let mut rest = line;
                while let Some(i) = rest.find("\"kind\"") {
                    rest = &rest[i + "\"kind\"".len()..];
                    let Some(after) = rest.trim_start().strip_prefix(':') else {
                        continue;
                    };
                    let Some(after) = after.trim_start().strip_prefix('"') else {
                        continue;
                    };
                    let Some(end) = after.find('"') else { continue };
                    let kind = after[..end].to_string();
                    if !out.contains(&kind) {
                        out.push(kind);
                    }
                }
            }
        }

        let chan = include_str!("channels.rs");
        let mut found: Vec<String> = Vec::new();
        // channels.rs strips its own tests: its `mod tests` is full of example
        // frames that are not published by the module.
        kinds_in(chan.split("mod tests").next().unwrap_or(chan), &mut found);
        // main.rs is scanned WHOLE. It carries `#[cfg(test)]` from line 16, so no
        // split can separate its tests, and it holds exactly one frame literal.
        kinds_in(include_str!("main.rs"), &mut found);
```

Then replace the two loops below it to compare `&str` against `String` (`k == kind.as_str()`, `found.iter().any(|f| f == kind)`), and add to `FRAME_VERDICTS`:

```rust
        // A screen's own look, pushed from main.rs when the operator reassigns it.
        // Not retained HERE: `last_screen` holds one frame and the newest wins, so
        // retaining a template would replace the verse and the next screen to join
        // would be sent a look and a blank wall. The hub keeps templates in their
        // own per-id cache (`cache_template`) and replays them on hello from there.
        ("channel_template", false),
```

- [ ] **Step 7: Run the retention test and confirm it still passes**

```bash
cd /Users/mrgee/WebstormProjects/relay/src-tauri
cargo test channels::tests::every_kind_this_module_publishes_has_an_explicit_verdict -- --nocapture
```

Expected: PASS. If it fails naming a kind published only from `main.rs`, add that kind to `FRAME_VERDICTS` with a reason — do not narrow the scanner back.

- [ ] **Step 8: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src-tauri/src/channels.rs
git commit -F - <<'EOF'
test: enumerate the rehearsal gate, and widen the retention scanner to main.rs

Retention has had a scanner since rule 43. Rehearsal gating has had a doc
comment, and that comment is honest about why it exists: stage_next shipped
ungated, leaked "up next" to a live stage tablet mid-rehearsal, and was
invisible to the e2e rehearsal test because it emits no Tauri event and that
test counts wall events. A doc comment is exactly what failed to catch it, and
a sixth publisher added to this module with no rehearsing() check still failed
nothing.

The new test reads the module's own source, finds every function that hands the
hub a message, and requires a verdict with a reason for each. For a function
whose verdict is "gated" it goes further and requires the gate to be in the
body, because an enumeration that only matched names would pass on a publisher
whose check had been deleted. Both halves were verified by reintroducing the
defect: removing stage_alert's gate fails it, and adding an ungated publisher
fails it.

The retention scanner read channels.rs alone, so channel_template — published
from main.rs — had no verdict. It now reads both files and channel_template has
one.
EOF
```

---

### Task 2: `fire_media` becomes generic over `tauri::Runtime`

**Why this exists:** `src-tauri/src/main.rs:2972` takes a concrete `tauri::AppHandle`. It is a real fire path — it puts a picture on a congregation's wall — with no `e2e.rs` coverage, because `e2e.rs` drives a `tauri::test::mock_builder` runtime and a concrete handle cannot be handed one. Rule 24's enforcement (`src/lib/hardrules.test.js:114`) checks four names only, so it cannot see the omission.

**Files:**
- Modify: `src-tauri/src/main.rs:2972` — the `fire_media` signature and any concrete-handle uses in its body
- Modify: `src/lib/hardrules.test.js:114` — the name list
- Modify: `src-tauri/src/e2e.rs` — add a case

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `fn fire_media<R: tauri::Runtime>(app: tauri::AppHandle<R>, …) -> error::Result<…>` — same parameters and return type as today, only the handle is generic. Wave 4 does not depend on this; wave 3 does not either. It is here because it is cheap and it is the last hole in rule 24.

- [ ] **Step 1: Write the failing frontend test**

In `src/lib/hardrules.test.js`, find the rule-24 name list at line 114 and add `fire_media` to it, keeping the existing four:

```js
// Rule 24. The fire path is generic over `tauri::Runtime` — that is what makes
// `e2e.rs` possible at all. `fire_media` was NOT on this list and was NOT
// generic: a real path that puts a picture in front of a congregation, with no
// layer-A coverage, and an enforcement of four names that could not see it.
const GENERIC_FIRE_PATH = [
  'fire_manual',
  'handle_nav',
  'clear_or_report',
  'persist_cue',
  'fire_media',
];
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/hardrules.test.js -t "rule 24"
```

Expected: FAIL, naming `fire_media` as not generic.

- [ ] **Step 3: Make it generic**

In `src-tauri/src/main.rs`, change the `fire_media` signature at line 2972 from a concrete `app: tauri::AppHandle` to:

```rust
fn fire_media<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    // … the existing parameters, unchanged
) -> error::Result<()> {
```

Read the body and make every helper it calls generic too if the compiler asks. Change nothing else — no reordering, no new behaviour. If a helper cannot be made generic, stop and report rather than working around it: a concrete handle anywhere on this path re-welds it.

- [ ] **Step 4: Run the frontend test and the Rust build**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/hardrules.test.js -t "rule 24"
cd src-tauri && cargo build 2>&1 | tail -5
```

Expected: vitest PASS, `cargo build` clean.

- [ ] **Step 5: Write the e2e case**

In `src-tauri/src/e2e.rs`, add:

```rust
/// A PICTURE IS A FIRE PATH, AND IT HAD NO TEST.
///
/// `fire_media` puts an image on a congregation's wall through the same
/// `broadcast_with_clock` door as a verse, so the pre-air validator, the
/// passage disarm and the rehearsal gate all apply to it. None of that was
/// driven, because the function took a concrete `AppHandle` and `e2e.rs` runs
/// on a mock runtime — rule 24's own failure mode, in the one place rule 24's
/// four-name enforcement could not look.
#[test]
fn r0_a_picture_reaches_the_wall_and_disarms_the_passage() {
    let (h, _db) = qa::bare_app();
    let wall = qa::Wall::watch(&h);

    super::fire_manual(h.clone(), "John 3:16".into(), None).expect("fire the verse");
    qa::settle();
    assert!(
        super::nav(h.clone(), "next".into()).is_ok(),
        "a passage is armed after a verse"
    );

    super::fire_media(h.clone(), 1, None).expect("fire the picture");
    qa::settle();

    let sent = wall.content();
    assert!(
        sent.iter().any(|c| c.kind.as_deref() == Some("media")),
        "the picture reached the wall: {sent:?}"
    );

    // Rule 38: anything that is not scripture disarms the passage.
    let after = super::nav(h.clone(), "next".into()).expect("nav answers");
    assert!(
        format!("{after:?}").contains("NoPassage"),
        "a picture replaced the reading, so `next` must not walk it: {after:?}"
    );
}
```

Adjust `fire_media`'s argument list and `qa::Wall`'s accessor to match what the real signatures are — read them, do not guess. If `bare_app()` seeds no media asset with id 1, seed one in the test rather than changing the fixture: `qa::bare_app()` is a fresh install and nothing else, and a second fixture is how two suites start disagreeing about what a fresh install contains.

- [ ] **Step 6: Run the e2e case**

```bash
cd /Users/mrgee/WebstormProjects/relay/src-tauri
cargo test e2e::r0_a_picture_reaches_the_wall_and_disarms_the_passage -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Verify the test catches the bug**

Temporarily change `main.rs:632`'s rule-38 match to `k != "scripture" && k != "media"` and re-run.

Expected: FAIL on the `NoPassage` assertion. Restore.

- [ ] **Step 8: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src-tauri/src/main.rs src-tauri/src/e2e.rs src/lib/hardrules.test.js
git commit -F - <<'EOF'
fix: make fire_media generic over tauri::Runtime, and drive it from e2e

fire_media puts a picture on a congregation's wall through the same
broadcast_with_clock door as a verse, so the pre-air validator, the passage
disarm and the rehearsal gate all apply to it. None of that was ever driven,
because the function took a concrete AppHandle and e2e.rs runs on a mock
runtime. Rule 24 exists to prevent exactly this, and its enforcement checks
four names, none of which was this one — so the rule was kept on the doors
somebody had already thought of.

The new e2e case drives the real command and asserts both halves: the picture
reaches the wall, and the reading it replaced is no longer armed. It was
verified by exempting media from the rule 38 match and watching it fail.
EOF
```

---

### Task 3: Two enumerations stop naming a command that no longer exists

**Why this exists:** `push_announcement` appears in `servicelock.rs:217` and `transport.test.js:126`. It is not a registered command — the only references in the tree are those two lists. Neither assertion checks registration, so the staleness is silent. `transport.test.js`'s `SCREEN_COMMANDS` is the gate on "every wrapper that fires decides about `liveCue.onAir`", and it only checks the eight names it lists, so a wrapper added in a later wave is unchecked until somebody remembers to add it.

**Files:**
- Modify: `src-tauri/src/servicelock.rs:200-230` — the forbidden-to-lock list and its test
- Modify: `src/lib/transport.test.js:117-126` — `SCREEN_COMMANDS` and its test

**Interfaces:**
- Consumes: nothing.
- Produces: both lists assert that every name in them is registered in `generate_handler!`.

- [ ] **Step 1: Confirm the command really is gone**

```bash
cd /Users/mrgee/WebstormProjects/relay
grep -rn "push_announcement" src-tauri/src src | sort
```

Expected: exactly two hits, `servicelock.rs` and `transport.test.js`, both inside a list. If there is a third — a `#[tauri::command]` or a `generate_handler!` entry — **stop**: the name is live and this task is wrong.

- [ ] **Step 2: Write the failing test in Rust**

In `src-tauri/src/servicelock.rs`, inside the existing `the_lock_can_never_reach_the_live_path` test, add after the existing assertions:

```rust
        // A NAME IN A HAND-WRITTEN LIST IS NOT A COMMAND.
        //
        // This list held `push_announcement` for as long as it took somebody to
        // grep for it: the assertion checks that each name is NOT protected, and
        // a name that is not a command is trivially not protected, so a dead
        // entry passes for ever while looking like coverage.
        let main = include_str!("main.rs");
        let handler = main
            .split("generate_handler!")
            .nth(1)
            .and_then(|s| s.split(']').next())
            .expect("generate_handler! block");
        for name in LIVE_PATH {
            assert!(
                handler.contains(name),
                "`{name}` is named here as a command the lock may never reach, and \
                 it is not registered in generate_handler! — a guarantee about \
                 nothing"
            );
        }
```

Name the existing list `LIVE_PATH` if it is currently inline; if it already has a name, use that.

- [ ] **Step 3: Run it to verify it fails**

```bash
cd /Users/mrgee/WebstormProjects/relay/src-tauri
cargo test servicelock::the_lock_can_never_reach_the_live_path -- --nocapture
```

Expected: FAIL with `push_announcement is named here as a command the lock may never reach, and it is not registered in generate_handler!`

- [ ] **Step 4: Remove the dead name**

Delete `"push_announcement",` from the list in `src-tauri/src/servicelock.rs:217`.

- [ ] **Step 5: Run it to verify it passes**

```bash
cd /Users/mrgee/WebstormProjects/relay/src-tauri
cargo test servicelock:: -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Write the failing test in vitest**

In `src/lib/transport.test.js`, add a case beside the `SCREEN_COMMANDS` enumeration:

```js
  it('every name in SCREEN_COMMANDS is a command Rust actually registers', () => {
    // A name in a hand-written list is not a command. This list held
    // `push_announcement` after the command was deleted, and the assertion it
    // feeds — every wrapper that fires decides about liveCue.onAir — passed
    // anyway, because a wrapper that does not exist trivially has no wrapper
    // bug. A dead entry looks exactly like coverage.
    const main = readFileSync(
      new URL('../../src-tauri/src/main.rs', import.meta.url),
      'utf8',
    );
    const handler = main.split('generate_handler!')[1]?.split(']')[0] ?? '';
    expect(handler).not.toBe('');
    for (const name of SCREEN_COMMANDS) {
      expect(handler, `${name} is in SCREEN_COMMANDS and not registered`).toContain(name);
    }
  });
```

Add `import { readFileSync } from 'node:fs';` at the top of the file if it is not already there.

- [ ] **Step 7: Run it to verify it fails**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/transport.test.js -t "actually registers"
```

Expected: FAIL naming `push_announcement`.

- [ ] **Step 8: Remove the dead name and confirm green**

Delete `'push_announcement',` from `SCREEN_COMMANDS` at `src/lib/transport.test.js:126`.

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/transport.test.js
```

Expected: PASS, all cases in the file.

- [ ] **Step 9: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src-tauri/src/servicelock.rs src/lib/transport.test.js
git commit -F - <<'EOF'
test: two hand-written enumerations stop naming a deleted command

push_announcement was removed as a command and stayed in two lists, both of
which are gates. servicelock's list asserts that each name is NOT protected,
and a name that is not a command is trivially not protected, so the dead entry
passed for ever while looking like coverage. transport.test.js's
SCREEN_COMMANDS is the gate on "every wrapper that fires decides about
liveCue.onAir" and checks only the names it lists, so it under-checked by one
and would not have noticed a ninth wrapper either.

Both now assert that every name they hold is registered in generate_handler!,
so the next deletion fails a test instead of quietly shrinking the gate. Both
assertions were verified by running them before removing the name.
EOF
```

---

### Task 4: Wave 0 verification

**Files:** none modified.

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces: a green tree that waves 1–4 can build on.

- [ ] **Step 1: Build the frontend so the Rust suite has a `dist/`**

```bash
cd /Users/mrgee/WebstormProjects/relay
npm run build
```

Expected: a clean Vite build. This must happen before `cargo test`, or two `channels` tests fail on a bare 404 because `dist/` is gitignored (RG-127).

- [ ] **Step 2: Run the Rust suite**

```bash
cd /Users/mrgee/WebstormProjects/relay/src-tauri
cargo test 2>&1 | tail -20
```

Expected: 0 failed. Read the runner's own summary line. Record the numbers in `docs/qa/QA_HARNESS.md` §0 beside the command that produced them, and **nowhere else** — CLAUDE.md records that the same three counts were corrected five times in one week by being restated in a second place.

- [ ] **Step 3: Run fmt and clippy**

```bash
cd /Users/mrgee/WebstormProjects/relay/src-tauri
cargo fmt --all && cargo clippy --all-targets -- -D warnings
```

Expected: no output from fmt, no warnings from clippy. CI enforces both.

- [ ] **Step 4: Run the frontend suite**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run 2>&1 | tail -20
```

Expected: 0 failed. Read the runner's own summary line.

- [ ] **Step 5: Update the QA register and commit**

Update `docs/qa/QA_HARNESS.md` §0 with the two new counts beside their commands.

```bash
cd /Users/mrgee/WebstormProjects/relay
git add docs/qa/QA_HARNESS.md
git commit -F - <<'EOF'
docs: record the suite counts after wave 0

Both suites green. The numbers live here, beside the command that produces
them, and are not restated anywhere else.
EOF
```

---

<!-- ===== was docs/archive/WAVE-DESIGNS.md, merged 2026-09-21, verbatim ===== -->

> **Archived 2026-09-21.** Historical work order. Wave 1 landed 2026-09-15; see `docs/REBRAND.md` §Status and DECISIONS §77. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Wave 1 — Settings, Splash and Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every Settings control tell the truth about what it did, correct two false statements in CLAUDE.md, give the splash a design pass, and write down the output-routing map a church actually needs.

**Architecture:** Three independent tracks. Track A repairs ten Settings findings, one of which (safe mode) is a live-safety defect fixed at a choke point rather than at each caller. Track B is documentation: two CLAUDE.md corrections and the routing map, plus one small helper in Outputs. Track C is a visual pass on the existing splash. Nothing here changes the fire path, the router, or any threshold.

**Tech Stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest + jsdom, `cargo test`.

**Spec:** `docs/archive/WAVE-DESIGNS.md` (Wave 1)

**Depends on:** Wave 0 complete and green.

## Global Constraints

- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- Run `npm run build` before `cargo test` on a fresh tree (RG-127).
- Read test counts from the runner's own summary line, never a grep. Values live only in `docs/qa/QA_HARNESS.md` §0.
- Every new test verified to FAIL when its defect is reintroduced. Test the bug, not the fix.
- **Rule 41:** never `confirm()`, `alert()` or `prompt()`. Tauri's webview does not implement them — `confirm()` returns `false` without showing a dialog. Use an in-app arm/confirm or a mounted `[role="dialog"]`.
- **Rule 44:** any overlay must consume `Esc` for itself on the first press and must not paint over `Clear screens`. Pinned by `panicoverlay.test.js`, which enumerates the whole tree.
- **Rule 18 / DESIGN_SYSTEM §1.1:** amber means ON AIR and nothing else. Amethyst means rehearsal. Cyan means a guess. Grey means cued. Settings is never on air.
- **`src/lib/errors.js` is the ONE backend-error humaniser.** Never render a raw Rust `Err` string to a volunteer.
- Commit message bodies are normal English prose.
- Never commit to `main`. Work on `feat/wave7-timers-templates-stage`.

---

## Track A — Settings truth pass

### Task 1: Safe mode enforces its own promise (P1)

**Why this exists:** `src/lib/views/Settings.svelte:914` offers a switch whose row says *"Outputs will not open and detection is disarmed — nothing Relay does can reach a screen."* `setSafeMode` (`src/lib/boot/boot.js:115`) calls `patchRecord({ safeMode })` and nothing else. `$safeMode` is read in `src/App.svelte` at lines 419 and 493 **inside `onMount` only**; there is no reactive statement re-applying it. `src/lib/views/Live.svelte` references `safeMode` zero times, so the run surface's fire path is not gated. `grep -rn "safe_mode" src-tauri/src/` returns nothing.

So the switch flips a label, already-open windows stay open, and the detector stays armed until the next launch.

**The fix must be at a choke point.** CLAUDE.md records four separate bugs whose single root cause is a rule enforced on one surface and skipped on its twin. Adding a `$safeMode` check to each fire site would be the fifth.

**Files:**
- Modify: `src/lib/boot/boot.js:115` — `setSafeMode` stops being the public door
- Modify: `src/lib/stores/capture.js` — add `applySafeMode` and `safeModeError`
- Modify: `src/lib/views/Settings.svelte:914-920` — call the new door, render the error
- Modify: `src/App.svelte:493` — the mount path calls the same door
- Create: `src/lib/safemode.test.js`

**Interfaces:**
- Consumes: `setDetection` (`capture.js:559`), `listOutputChannels` (`:1978`), `closeChannelOutput` (`:2249`).
- Produces:
  - `export async function applySafeMode(on: boolean): Promise<boolean>` in `capture.js` — returns `true` when the promise was kept, `false` otherwise, **and** sets `safeModeError`. Both, for the same reason `panicRun` does both: the caller may be a view that has crashed.
  - `export const safeModeError` — a `writable(null)` holding a humanised reason.

- [ ] **Step 1: Write the failing test**

Create `src/lib/safemode.test.js`:

```js
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';

// SAFE MODE IS A PROMISE, NOT A LABEL.
//
// The switch's own row says "detection is disarmed — nothing Relay does can
// reach a screen". Before this test, setSafeMode patched a localStorage record
// and nothing else: App.svelte honoured it inside onMount only, Live.svelte
// never mentioned it, and Rust had no notion of it at all. The switch flipped,
// aria-checked flipped, and a live detector stayed armed over open projector
// windows until the next launch.
//
// A control that reports a success it did not achieve is rule 15's failure in
// a different costume, and this one makes a bigger promise than a panic key.

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

describe('safe mode keeps its own promise', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(true);
  });

  it('disarms detection and closes every open screen when turned ON', async () => {
    const { applySafeMode } = await import('./stores/capture.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_output_channels') {
        return Promise.resolve([
          { id: 1, name: 'Main screen', render_target: 'native_window' },
          { id: 2, name: 'Lobby', render_target: 'native_window' },
        ]);
      }
      return Promise.resolve(true);
    });

    const ok = await applySafeMode(true);

    expect(ok).toBe(true);
    const cmds = invoke.mock.calls.map((c) => c[0]);
    expect(cmds).toContain('set_detection');
    expect(
      invoke.mock.calls.find((c) => c[0] === 'set_detection')[1],
    ).toMatchObject({ enabled: false });
    expect(cmds.filter((c) => c === 'close_channel_output')).toHaveLength(2);
  });

  it('reports failure and does not claim a success it did not achieve', async () => {
    const { applySafeMode, safeModeError } = await import('./stores/capture.js');
    safeModeError.set(null);
    invoke.mockImplementation((cmd) => {
      if (cmd === 'set_detection') return Promise.reject(new Error('audio lock poisoned'));
      if (cmd === 'list_output_channels') return Promise.resolve([]);
      return Promise.resolve(true);
    });

    const ok = await applySafeMode(true);

    expect(ok).toBe(false);
    expect(get(safeModeError)).toBeTruthy();
  });

  it('turning safe mode OFF does not re-arm anything by itself', async () => {
    // Coming out of safe mode restores the operator's freedom to arm things; it
    // must not arm them FOR them. A detector that switches itself back on is a
    // different surprise from the one this control exists to prevent.
    const { applySafeMode } = await import('./stores/capture.js');
    invoke.mockImplementation((cmd) =>
      cmd === 'list_output_channels' ? Promise.resolve([]) : Promise.resolve(true),
    );

    await applySafeMode(false);

    const armed = invoke.mock.calls.filter(
      (c) => c[0] === 'set_detection' && c[1]?.enabled === true,
    );
    expect(armed).toHaveLength(0);
  });

  it('setSafeMode has exactly one caller, and it is applySafeMode', async () => {
    // THE CHOKE POINT IS WHERE THE CHECK GOES, NOT THE CALL SITES (rule 36).
    // The record write must not be reachable without the enforcement beside it,
    // or the next surface to flip safe mode reproduces the original defect.
    const { readFileSync, readdirSync, statSync } = await import('node:fs');
    const { join } = await import('node:path');
    const root = new URL('..', import.meta.url).pathname;

    const files = [];
    const walk = (d) => {
      for (const e of readdirSync(d)) {
        const p = join(d, e);
        if (statSync(p).isDirectory()) walk(p);
        else if (/\.(js|svelte)$/.test(e) && !e.endsWith('.test.js')) files.push(p);
      }
    };
    walk(root);

    const callers = files.filter((f) => {
      if (f.endsWith('boot/boot.js')) return false; // the definition
      return /\bsetSafeMode\s*\(/.test(readFileSync(f, 'utf8'));
    });

    expect(
      callers.map((f) => f.slice(root.length)),
      'setSafeMode must be reached only through applySafeMode',
    ).toEqual(['stores/capture.js']);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/safemode.test.js
```

Expected: FAIL — `applySafeMode` is not exported.

- [ ] **Step 3: Add the choke point**

In `src/lib/stores/capture.js`, near `panicError` (`:2322`), add:

```js
/**
 * The reason safe mode could not keep its promise, humanised — or null.
 *
 * Module scope, and set by `applySafeMode` itself rather than returned to the
 * caller, for the same reason `panicError` is: the control that flips safe mode
 * can be a view that has crashed, and a view that cannot `catch` cannot report.
 */
export const safeModeError = writable(null);

/**
 * TURN SAFE MODE ON OR OFF, AND MAKE THE PROMISE TRUE.
 *
 * Safe mode's row says "outputs will not open and detection is disarmed —
 * nothing Relay does can reach a screen". `setSafeMode` writes that into the
 * boot record; for as long as it was the only thing that happened, the sentence
 * was false until the next launch — App.svelte honoured it inside onMount only,
 * Live.svelte never mentioned it, and Rust has no notion of it.
 *
 * So the enforcement lives HERE, at the one door, and not as a `$safeMode` check
 * at each fire site. This repository has had four separate bugs whose single
 * root cause is a rule enforced on one surface and skipped on its twin; a check
 * per caller would be the fifth.
 *
 * Returns whether the promise was kept, AND sets `safeModeError`. Turning safe
 * mode OFF restores the operator's freedom to arm things and deliberately arms
 * nothing for them: a detector that switches itself back on is a different
 * surprise from the one this control prevents.
 */
export async function applySafeMode(on) {
  safeModeError.set(null);
  setSafeMode(on);
  if (!on) return true;

  const failures = [];

  try {
    await setDetection(false);
  } catch (e) {
    failures.push(humanError(e));
  }

  try {
    const chans = await listOutputChannels();
    for (const c of chans ?? []) {
      try {
        await closeChannelOutput(c.id);
      } catch (e) {
        failures.push(`${c.name ?? `screen ${c.id}`}: ${humanError(e)}`);
      }
    }
  } catch (e) {
    failures.push(humanError(e));
  }

  if (failures.length) {
    safeModeError.set(
      `Safe mode is recorded, and it could not be enforced: ${failures.join('; ')}. ` +
        'Something may still be able to reach a screen.',
    );
    return false;
  }
  return true;
}
```

Add `import { setSafeMode } from '../boot/boot.js';` to `capture.js`'s imports if it is not already present, and confirm `humanError` is already imported from `../errors.js`.

- [ ] **Step 4: Point the two callers at the new door**

In `src/lib/views/Settings.svelte`, change the switch handler at `:919` from `setSafeMode(!$safeMode)` to `applySafeMode(!$safeMode)`, update the import, and render `$safeModeError` beneath the row with `role="alert"`:

```svelte
{#if $safeModeError}<p class="rw-foot s-netbad" role="alert">{$safeModeError}</p>{/if}
```

Use `.s-netbad` (rose), never amber — the file's own comments say "Rose, never amber" three times and this page is never on air.

In `src/App.svelte:493`, replace the inline `setDetection(false)` block with a call to the same door, so the mount path and the switch cannot drift:

```js
    // SAFE MODE IS A PROMISE, NOT A LABEL. One door, so a console reopened in
    // safe mode and a switch flipped in Settings enforce exactly the same thing.
    if ($safeMode) {
      await applySafeMode(true);
    }
```

- [ ] **Step 5: Run the test to verify it passes**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/safemode.test.js
```

Expected: PASS, all four cases.

- [ ] **Step 6: Verify the tests catch the bug**

Temporarily change `applySafeMode` to `setSafeMode(on); return true;` and re-run.

Expected: the first two cases FAIL. Restore.

Then temporarily add a second `setSafeMode(` call in `src/lib/views/Settings.svelte` and re-run.

Expected: the fourth case FAILS naming the extra caller. Restore.

- [ ] **Step 7: Record the decision**

Append a section to `docs/DECISIONS.md` recording that safe mode's enforcement lives at one door in `capture.js`, that it reports failure like a panic control, and the open question the spec named: whether safe mode should also exist in Rust so the engine cannot be armed while the record says disarmed. Answer it in this commit and write the reason down. `crossrefs.test.js` resolves every `DECISIONS §N` against the real headings, so use the next free number.

- [ ] **Step 8: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src/lib/stores/capture.js src/lib/views/Settings.svelte src/App.svelte src/lib/safemode.test.js docs/DECISIONS.md
git commit -F - <<'EOF'
fix: safe mode enforces its own promise instead of flipping a label

The switch's own row says "outputs will not open and detection is disarmed —
nothing Relay does can reach a screen". setSafeMode patched a localStorage boot
record and nothing else. App.svelte honoured it inside onMount only, with no
reactive statement re-applying it; Live.svelte never mentioned it, so the run
surface's fire path was not gated at all; and there is no notion of safe mode
anywhere in Rust. Already-open projector windows stayed open and the detector
stayed armed until the next launch, while aria-checked said otherwise.

The enforcement now lives at one door, applySafeMode, which writes the record,
disarms detection, closes every open screen, and reports failure rather than
claiming a success it did not achieve — the same contract as a panic control,
because it makes a larger promise than one. A check per fire site was the
obvious alternative and is the shape of the four bugs this repository already
records, where a rule was kept on one surface and skipped on its twin.

Turning safe mode off deliberately arms nothing: it restores the operator's
freedom to arm things, and a detector that switched itself back on would be a
different surprise from the one this control prevents.

A test holds setSafeMode to exactly one caller, so the next surface to offer
safe mode cannot reproduce the original defect.
EOF
```

---

### Task 2: The setup walk-through is guarded during a recorded service (P2)

**Why this exists:** `src/lib/views/Settings.svelte:1438` carries no `disabled` expression in any state. One click sets `session.setupDone = false`, which mounts `FirstRun` full-screen over a live console (`App.svelte:564`). From inside it, `stopMicTest()` and `chooseDevice()` each call `stopCapture()` on the live microphone (`FirstRun.svelte:177`, `:201`), and "Try it" fires `manualFire('John 3:16')` to the congregation's screens (`:222`). The service lock cannot help, because `restartSetup` is a session write and `servicelock::guard` is never consulted. CLAUDE.md rule 44 already names this sentence; its `Esc` half is fixed and pinned, and this half is open.

**Files:**
- Modify: `src/lib/views/Settings.svelte:1438`
- Modify: `src/lib/settingssections.test.js`

**Interfaces:**
- Consumes: the `serviceLock` store already read at `Settings.svelte:1502` for "Unlock for this service".
- Produces: nothing later tasks depend on.

- [ ] **Step 1: Write the failing test**

Add to `src/lib/settingssections.test.js`:

```js
  it('the setup walk-through cannot be started over a recorded service', async () => {
    // It mounts FirstRun full-screen over a live console, and from inside it
    // stopMicTest() and chooseDevice() each stop the LIVE microphone while
    // "Try it" fires John 3:16 to the congregation. The service lock cannot
    // reach it, because restartSetup is a session write and never consults
    // servicelock::guard — so the guard has to be on the button.
    const { container } = render(Settings);
    serviceLock.set({ engaged: true, reason: 'a service is being recorded' });
    await tick();

    const btn = [...container.querySelectorAll('button')].find((b) =>
      /setup walk-through/i.test(b.textContent ?? ''),
    );
    expect(btn, 'the walk-through button is rendered').toBeTruthy();
    expect(btn.disabled).toBe(true);
    expect(
      btn.closest('.s-prose')?.textContent ?? '',
      'a disabled control must carry its reason',
    ).toMatch(/service is being recorded|while a service/i);
  });
```

Match the existing file's import style and render helper rather than inventing one.

- [ ] **Step 2: Run it to verify it fails**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/settingssections.test.js -t "walk-through"
```

Expected: FAIL — `expected false to be true`.

- [ ] **Step 3: Guard the button**

In `src/lib/views/Settings.svelte`, replace the button at `:1438` with:

```svelte
<button
  class="r-btn ghost sm"
  on:click={restartSetup}
  disabled={$serviceLock.engaged}>Run the setup walk-through</button>
{#if $serviceLock.engaged}
  <p class="rw-foot s-netwarn">Not while a service is being recorded — the walk-through stops the microphone and puts a verse on your screens. End the service first, or unlock it below.</p>
{/if}
```

Use `.s-netwarn` (amethyst), not amber.

- [ ] **Step 4: Run the test to verify it passes**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/settingssections.test.js
```

Expected: PASS.

- [ ] **Step 5: Verify the test catches the bug**

Remove the `disabled` attribute and re-run. Expected: FAIL. Restore.

- [ ] **Step 6: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src/lib/views/Settings.svelte src/lib/settingssections.test.js
git commit -F - <<'EOF'
fix: the setup walk-through is held back while a service is recorded

One click set session.setupDone = false, which mounts the first-run wizard
full-screen over a live console. From inside it, two paths stop the live
microphone and "Try it" fires John 3:16 to the congregation's screens, under a
label that says so. There was no arm, no confirm and no disabled state in any
condition.

The service lock could not help: restartSetup is a session write, so
servicelock::guard is never consulted, which means the guard has to be on the
button. It now carries the lock's own wording, and the unlock control is six
rows below it on the same page.

CLAUDE.md rule 44 already names this exact sentence. Its Escape half was fixed
and pinned; this is the other half.
EOF
```

---

### Task 3: "Check for Updates" stops reporting a check it never ran (P2)

**Why this exists:** `Settings.svelte:1580`. While a service is recording, `checkForUpdate()` (`src/lib/updater.js:92`) returns `null` **without calling `noteChannel`**, so `doCheckUpdates` falls through to `ch.state === 'ok'` and prints "You're on the latest version." The status row six pixels above goes through `describeChannel` and is honest. The two disagree and the louder one is wrong. This is rule 35 on the one path by which a fix reaches a church that already has Relay — the same category as RG-83 and RG-133.

**Files:**
- Modify: `src/lib/updater.js:92` — the idle refusal becomes a recorded outcome
- Modify: `src/lib/views/Settings.svelte:1580-1583`
- Modify: `src/lib/updatechannel.test.js`

**Interfaces:**
- Consumes: `noteChannel`, `describeChannel` (`updater.js`).
- Produces: a `'skipped'` state on `updateChannel`, described by `describeChannel`.

- [ ] **Step 1: Write the failing test**

Add to `src/lib/updatechannel.test.js`:

```js
  it('a check refused because a service is recording is its own outcome', () => {
    // RULE 35. The button printed "You're on the latest version." whenever the
    // refusal happened after a successful launch check, because the refusal
    // returned null without recording anything and the caller fell back to the
    // stale 'ok'. One reassuring sentence over two different situations, on the
    // one path by which a fix reaches a church that already has Relay.
    noteChannel({ state: 'ok', at: Date.now() });
    noteChannel({ state: 'skipped', reason: 'service', at: Date.now() });

    const said = describeChannel(get(updateChannel));
    expect(said.tone).not.toBe('ok');
    expect(said.text).toMatch(/service/i);
    expect(said.text).not.toMatch(/latest version/i);
  });
```

Match the file's existing import and helper style.

- [ ] **Step 2: Run it to verify it fails**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/updatechannel.test.js -t "refused"
```

Expected: FAIL — `describeChannel` has no `skipped` branch.

- [ ] **Step 3: Record the refusal and describe it**

In `src/lib/updater.js`, at the idle guard around `:92`, replace the bare `return null` with:

```js
  if (!idle()) {
    // A REFUSAL IS A THIRD OUTCOME, NOT AN ABSENCE. Returning null here let the
    // caller fall back to whatever the last successful check said, so a button
    // pressed mid-service printed "You're on the latest version." about a check
    // that never ran. Rule 35: if the line reads the same when the thing behind
    // it did not happen, it is not a status line.
    noteChannel({ state: 'skipped', reason: 'service', at: Date.now() });
    return null;
  }
```

Add the `skipped` branch to `describeChannel`:

```js
    case 'skipped':
      return {
        tone: 'warn',
        text: 'Not checked — Relay does not check for updates while a service is being recorded.',
      };
```

In `src/lib/views/Settings.svelte`, make `doCheckUpdates` render `describeChannel($updateChannel).text` rather than composing its own sentence from `ch.state === 'ok'`.

- [ ] **Step 4: Run the test to verify it passes**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/updatechannel.test.js
```

Expected: PASS.

- [ ] **Step 5: Verify the test catches the bug**

Remove the `noteChannel({ state: 'skipped' … })` line and re-run. Expected: FAIL. Restore.

- [ ] **Step 6: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src/lib/updater.js src/lib/views/Settings.svelte src/lib/updatechannel.test.js
git commit -F - <<'EOF'
fix: a refused update check is its own outcome, not a stale success

While a service is being recorded, checkForUpdate returns null without
recording anything, so the button fell through to whatever the last successful
check had said and printed "You're on the latest version." about a check that
never ran. The status row six pixels above it goes through describeChannel and
was honest the whole time, so the page disagreed with itself and the louder
half was wrong.

The refusal is now a third state, described in the one place that turns channel
state into words. This is the same failure RG-83 and RG-133 were about, on the
same path: how a fix reaches a church that already has Relay.
EOF
```

---

### Task 4: The five smaller Settings findings

Each is independent. Commit them together — a reviewer would accept or reject them as one pass.

**Files:**
- Modify: `src/lib/views/Settings.svelte` (`:1077`, `:1455`, `:1583`, `:1853`, `:1890`, `:186`)
- Modify: `src/lib/audioOutput.js:81`
- Modify: `src/lib/ModelSetup.svelte:54`

**Interfaces:**
- Consumes: `humanError` (`src/lib/errors.js`), `$contentTemplates` (`capture.js:274`).
- Produces: nothing later tasks depend on.

- [ ] **Step 1: F-3 — "Detect speakers" says what happened**

`audioOutput.js:81` `ensureDeviceAccess` returns `false` both when the permission was refused and when there was nothing to find, so `Settings.svelte:1077` re-renders identically in three different situations. Return which it was, and render it. Include the OS path to reverse a refusal.

- [ ] **Step 2: F-5 — the offline model install uses the one humaniser**

`ModelSetup.svelte:54` is `installMsg = e?.message ?? String(e);`, rendering a Rust `Err(String)` verbatim while its six siblings on the same surface go through `humanError`. Route it through `humanError`. The defect is latent — today's strings are volunteer-worded — and nothing constrains the next one, because `install_from_file` returns `Result<String, String>` rather than the typed `{ kind, message }`.

- [ ] **Step 3: F-8 — the Sentry DSN commits**

`setCrashReporting` is called from exactly one place, `toggleCrash` (`Settings.svelte:469`). With the switch already on, editing the DSN at `:1853` writes only the local object, and leaving the section re-reads `getCrashReporting` and overwrites it. The one control that decides where data leaves the machine can be edited and silently keep pointing at the old place. Commit on `change`/blur, or add a Save beside the field.

- [ ] **Step 4: F-9 — amber is not spent on a page that is never on air**

`Settings.svelte:1455` sets `style="color:var(--v-amber)"` inline on the demo-content edited count. The file's own comments say "Rose, never amber" at `:2032`, `:2040` and `:2056`, and define `.s-netbad` (rose) and `.s-netwarn` (amethyst) for exactly this. Use `.s-netwarn`. Leave `:1497` ("A service is being recorded.") alone — that one is arguably the on-air session.

- [ ] **Step 5: F-10 — two results are announced**

`updateMsg` (`:1583`) and `crashMsg` (`:1890`) have no live region, while six other message surfaces on the same page do — `micErr :1042`, `roomMsg :1110`, `profileErr :1171`, `demoErr/demoNote :1487`, `lockErr :1503`, `diagMsg :1642`. Add `role="status"` to both. Crash reporting is the one control that decides whether data leaves the machine.

- [ ] **Step 6: The private `ctMap`**

`Settings.svelte:186` holds a private content-look map that omits `countdown`, contradicting the "one store" comment at `capture.js:274`. Read `$contentTemplates` instead, like the other three surfaces do.

- [ ] **Step 7: Write the tests**

One case per finding, in `src/lib/settingssections.test.js` (or the file that already covers the surface). Each must be verified to fail with its defect reintroduced. The `role="status"` pair and the amber one are assertions about the rendered markup; the DSN one drives an edit and a re-read; the `ctMap` one asserts that Settings and the Templates gallery agree about all five kinds.

- [ ] **Step 8: Run the suite and commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run 2>&1 | tail -10
git add -A
git commit -F - <<'EOF'
fix: five Settings controls report what actually happened

Detect speakers was pixel-identical when the permission was refused, when it
worked, and when the machine genuinely has one output; audioOutput.js already
knew which and did not say. The offline model install rendered a Rust error
string verbatim, bypassing the one humaniser its six siblings on the same
surface use. The Sentry DSN had no commit path while
crash reporting was already on, so the one control that decides where data
leaves the machine could be edited and keep pointing at the old place. A demo
count was painted amber by an inline style on a page that is never on air, in a
file whose own comments say "Rose, never amber" three times. Two results — the
update check and the crash-reporting toggle — were announced to nobody while
six other message surfaces on the same page have live regions.

Settings also held a private content-look map that omitted the countdown kind,
contradicting the one-store comment it was meant to follow. It reads the store.
EOF
```

---

### Task 5: "End current service" reports a failure (P3)

**Why this exists:** `capture.js:605` `endService` swallows every failure in a bare `catch {}`, and `History.svelte:580` then calls `refresh()` and repaints the same list, so a refused `end_service` leaves the operator believing the record is closed. Same shape as rule 15 on a smaller control. This is its own task because it is not a cosmetic fix: it decides which of `capture.js`'s two documented wrapper groups this wrapper belongs to, and CLAUDE.md records that **a contract stated in a comment is not a contract** — `stopCapture` sat in the THROWS group, swallowing, for as long as its comment existed.

**Files:**
- Modify: `src/lib/stores/capture.js:605`
- Modify: `src/lib/views/library/History.svelte:245`, `:580`
- Create: `src/lib/endservice.test.js`

**Interfaces:**
- Consumes: `humanError` (`src/lib/errors.js`).
- Produces: `endService` in whichever group you place it, **and the test that holds it there**.

- [ ] **Step 1: Read the groups before deciding**

Read the throw-vs-swallow group comment at the top of `src/lib/stores/capture.js`, and read `src/lib/micstop.test.js`, which is the pattern to copy — it exists because `stopCapture` printed "Start listening" over a live microphone while sitting in the group that says it throws.

- [ ] **Step 2: Write the failing test**

Create `src/lib/endservice.test.js`:

```js
import { describe, it, expect, vi, beforeEach } from 'vitest';

// A CONTRACT STATED IN A COMMENT IS NOT A CONTRACT.
//
// endService swallowed every failure in a bare catch {}, and History then
// repainted the same list, so a refused end_service left the operator believing
// the record was closed. The service lock is re-read afterwards, so the truth
// was on the page — in a different section, in smaller type.

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

describe('ending a service cannot report a success it did not achieve', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('propagates a refused end_service to its caller', async () => {
    const { endService } = await import('./stores/capture.js');
    invoke.mockRejectedValue(new Error('the service lock is held'));

    await expect(endService()).rejects.toThrow();
  });

  it('resolves normally when the command succeeds', async () => {
    const { endService } = await import('./stores/capture.js');
    invoke.mockResolvedValue(true);

    await expect(endService()).resolves.not.toThrow();
  });
});
```

- [ ] **Step 3: Run it to verify it fails**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/endservice.test.js
```

Expected: FAIL on the first case — the bare `catch {}` resolves rather than rejecting.

- [ ] **Step 4: Move the wrapper and show the reason**

Remove the bare `catch {}` from `endService` so it throws, and in `src/lib/views/library/History.svelte`, catch it in `stopRecording` (`:245`) and render `humanError(e)` beside the button with `role="alert"`. Do not re-`refresh()` over a failure — repainting an unchanged list is how the original defect read as success.

- [ ] **Step 5: Run the test to verify it passes**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/endservice.test.js src/lib/transport.test.js
```

Expected: PASS. `transport.test.js` is in the list because it holds the wrapper-group enumeration Wave 0 Task 3 repaired.

- [ ] **Step 6: Verify the test catches the bug**

Restore the bare `catch {}` and re-run. Expected: the first case FAILS. Remove it again.

- [ ] **Step 7: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src/lib/stores/capture.js src/lib/views/library/History.svelte src/lib/endservice.test.js
git commit -F - <<'EOF'
fix: ending a service reports a refusal instead of repainting the list

endService swallowed every failure in a bare catch {}, and History then called
refresh() and repainted an unchanged list, so a refused end_service left the
operator believing the record was closed. The truth was on the page the whole
time — the service lock is re-read afterwards — in a different section.

The wrapper now throws, its caller shows the humanised reason beside the button,
and a test holds it in that group. capture.js documents its throw-vs-swallow
groups in a comment at the top of the file, and stopCapture sat in the throwing
group while swallowing for as long as that comment existed: a contract stated in
a comment is not a contract, so the test is the half that matters.
EOF
```

---

## Track B — documentation and the routing map

### Task 6: Two corrections to CLAUDE.md

**Why this exists:** CLAUDE.md is the first thing every agent and contributor reads. Two of its statements are false, and both are the kind that stop somebody asking a question they should ask.

**Files:**
- Modify: `CLAUDE.md` — the build-status block
- Modify: `docs/ARCHITECTURE.md` §6 — the event table

**Interfaces:** none.

- [ ] **Step 1: Correct the model-pinning claim**

CLAUDE.md states *"A pilot must pin the model, and nothing currently does."* Verify it is false before changing it:

```bash
cd /Users/mrgee/WebstormProjects/relay
grep -n "select_stt_model\|STT_MODEL_KEY\|stt_model_setting" src-tauri/src/main.rs
```

Expected: `select_stt_model` at `:4836` (service-lock guarded at `:4838`), `STT_MODEL_KEY = "stt.model"` at `:4817`, and `build_stt` reading it at `:4714`.

Replace the claim with what is true: pinning exists and works — "Use this one" in `ModelSetup.svelte:172` writes `app_settings['stt.model']`, `build_stt` reads it at every launch, and the operator's choice beats `MODEL_CANDIDATES` order in `stt::resolve_model` (`stt.rs:1120`). Then state what is genuinely missing, which is the sentence worth keeping: **a pinned recognition language**. The picker at `Settings.svelte:1277` defaults to "Auto-detect (code-switching)", and RG-116 records that automatic language election cost a whole service on `ggml-small` — 17 incoherent transcripts against 161 with the language fixed. Add that no pre-service surface states which model and language a service will run on: `Dashboard.svelte:267` says "Ready for a service." over `ggml-base` without naming it.

- [ ] **Step 2: Correct the event count**

CLAUDE.md and `docs/ARCHITECTURE.md` §6 both say nineteen events. Reproduce the real set:

```bash
cd /Users/mrgee/WebstormProjects/relay
grep -rhoE '"[a-z_]+://[a-z_]+"' src-tauri/src/*.rs | sort -u
```

That yields twenty-one, of which `tauri://localhost` is an origin string at `channels.rs:2278` and not an event — so **twenty**. `docs/ARCHITECTURE.md` §6's table lists eighteen, omitting `output://error` and `output://transition`, one of which is an error path.

Add the two missing rows to §6's table. In CLAUDE.md, state twenty and keep the existing instruction to reproduce the set rather than trust the number — that instruction is why this was findable.

- [ ] **Step 3: Verify the cross-references still resolve**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/crossrefs.test.js
```

Expected: PASS. A citation that resolves to nothing is worse than an uncited claim.

- [ ] **Step 4: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add CLAUDE.md docs/ARCHITECTURE.md
git commit -F - <<'EOF'
docs: correct two statements in the handbook that were false

CLAUDE.md said a pilot must pin the STT model and that nothing does. Pinning
exists and has since select_stt_model landed: "Use this one" writes
app_settings['stt.model'], build_stt reads it at every launch, and the
operator's choice beats MODEL_CANDIDATES order in resolve_model. What is
genuinely missing beside it is a pinned recognition LANGUAGE — the picker
defaults to auto-detect, and RG-116 records that automatic election cost a
whole service on ggml-small, 17 incoherent transcripts against 161 with the
language fixed — and a pre-service statement of which model and language a
service will run on, since the Dashboard says "Ready for a service." over
ggml-base without naming it.

The event count said nineteen in two places. The scanner's own set is twenty,
and ARCHITECTURE section 6's table listed eighteen, omitting output://error and
output://transition. One of those is an error path.

Both are the kind of statement that stops somebody asking a question they
should ask, which is why they are worth correcting rather than footnoting.
EOF
```

---

### Task 7: The output routing map

**Why this exists:** Relay already has both realistic routing paths and no document says so, so the question "how do I get this onto the ATEM" has no answer in the repository. The research behind this task is in the spec; this task writes it down where a church can find it.

**Files:**
- Create: `docs/OUTPUT_ROUTING.md` — this task creates it; it does not exist yet.
- Modify: `docs/README.md` — the index
- Modify: `docs/SPEC.md:25`
- Modify: `src/lib/views/Channels.svelte` — one helper

**Interfaces:**
- Consumes: `outputurl.js:29` (the one URL builder), `screenKind`/`screenTransport` (`outputHealth.js:473`).
- Produces: nothing later tasks depend on.

- [ ] **Step 1: Write `docs/OUTPUT_ROUTING.md`** — this task creates it.

Four sections, one per case a church actually has, each stating what Relay emits and what the church supplies:

1. **A projector or a TV.** A fullscreen Tauri webview on a physical monitor — `channels::open_native_window` (`channels.rs:419`) places a borderless webview inside the target monitor's bounds and fullscreens it; `auto_open_outputs` (`main.rs:5164`) opens only `native_window` channels, only onto a connected display, and never onto the operator's own monitor.
2. **An ATEM.** Every ATEM Mini has HDMI inputs and no SDI inputs; every rack-mount ATEM has SDI inputs and no HDMI inputs, and the single HDMI connector on an ATEM Television Studio HD8 is an **output**. A Blackmagic Micro Converter HDMI to SDI 3G, about $75, bridges Relay's HDMI into any SDI ATEM. Relay emits a plain HDMI display signal and needs nothing SDI-aware, which is why the "no native SDI" constraint costs nothing. State plainly that **no ATEM accepts NDI** — Blackmagic's IP direction is SMPTE 2110 — that the ATEM **Media Player is stills only** at roughly three seconds per 1080p frame with a pool lock and twenty slots, and that **SuperSource composites inputs already on the switcher** and is not an ingest path.
3. **OBS, vMix or a kiosk screen.** The `:8032` URL in a browser source, and the fact that matters most: it costs **zero GPU display pipes**. Give the canonical URL and say to use **Copy URL** in Outputs — a hand-built `template_id`-only URL will not live-swap, because the swap is keyed on `channel` (DECISIONS §29).
4. **More screens than ports.** Apple Silicon has no DisplayPort MST extended desktop at any chip: base M1/M2 drive one external display, M3/M4 base two, only Max tiers four. DisplayLink is the only workaround, and granting it the macOS Screen Recording permission **disables HDCP system-wide** — harmless for scripture, a trap for a church that also plays a licensed clip. Windows supports MST natively. The real answer for most churches is section 3, because a network screen costs no pipe at all.

Close with what Relay does **not** do and why: NDI is parked (`open_ndi_output` returns a clear error), and it is worth recording that it is reachable without shipping a proprietary byte — Vizrt documents `NDIlib_v5_load()` runtime loading as being "of value in Open-Source projects", with a redistributable-URL constant for the absent-runtime case — but that it buys nothing toward an ATEM, which is what churches ask about.

- [ ] **Step 2: Fix the SPEC overstatement**

`docs/SPEC.md:25` claims Relay "talks to OBS, ATEM, and ProPresenter over NDI, HDMI, and the local network." NDI is parked and `open_ndi_output` returns an error. `SPEC.md:151`, `:169`, `:171` and `Settings.svelte:1421` all state it correctly; line 25 does not. Correct it and point at `docs/OUTPUT_ROUTING.md`, which this task creates in Step 1.

- [ ] **Step 3: Add the helper in Outputs**

In `src/lib/views/Channels.svelte`, beside **Copy URL**, add a short "How do I reach this screen?" disclosure that names the case from the screen's own `render_target` — HDMI for `native_window`, a browser source for `network_client` — and links to the new document. No new command, no new state; it reads what the card already has.

- [ ] **Step 4: Index it and run the cross-reference test**

Add `OUTPUT_ROUTING` to `docs/README.md`'s index, then:

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/crossrefs.test.js src/lib/surface.test.js
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
# docs/OUTPUT_ROUTING.md does not exist yet — this task creates it in Step 1.
git add docs/OUTPUT_ROUTING.md docs/README.md docs/SPEC.md src/lib/views/Channels.svelte
git commit -F - <<'EOF'
docs: write down how Relay reaches a projector, an ATEM, OBS and a fourth screen

Relay already has both realistic routing paths — a fullscreen window on a
physical monitor, and a URL in a browser source — and nothing in the repository
said so, so "how do I get this onto the ATEM" had no answer here.

The short version is that a 75-dollar Blackmagic Micro Converter dissolves the
whole SDI question: every ATEM Mini is HDMI-only, every rack-mount ATEM is
SDI-only, Relay emits a plain HDMI display signal either way, and nothing
SDI-aware is needed in software. Three things churches are told elsewhere are
dead ends and are named as such: no ATEM accepts NDI, the ATEM Media Player is
a stills pool at roughly three seconds a frame, and SuperSource composites
inputs that are already on the switcher rather than accepting a new one.

For a laptop with one HDMI port, the answer is the browser source, because a
network screen costs zero GPU display pipes — which matters most on Apple
Silicon, where no chip supports MST extended desktop and the base parts drive
one or two external displays.

SPEC line 25 claimed Relay talks to ATEM over NDI. NDI is parked and
open_ndi_output returns an error; three other lines in the same document said so
correctly and that one did not.
EOF
```

---

## Track C — the splash

### Task 8: Splash design pass

**Why this exists:** `src/lib/Splash.svelte` exists and works — 399 lines, amethyst brand (never amber, stated at `:9`), held `BOOT_HOLD_MS` 900 and capped `BOOT_CAP_MS` 4000 (`App.svelte:393`). On a healthy machine the operator sees the splash and then the console: the four stage screens and four gates appear only when they have something to say (`BootSequence.svelte:23`). This is a visual quality pass, not a build.

**Files:**
- Modify: `src/lib/Splash.svelte`
- Modify: `src/lib/boot/BootShell.svelte` — only if the splash and the stage screens disagree visually

**Interfaces:**
- Consumes: `BrandMark.svelte`, the `--v-*` tokens in `src/app.css`.
- Produces: nothing later tasks depend on.

- [ ] **Step 1: Render it and look at it**

This machine cannot screenshot the Tauri window, so drive the splash in a browser against the mock bridge — the harness that found the two defects of the 2026-09-10 pass. Capture the splash at 1920×1080, 1366×768 and 1024×768.

- [ ] **Step 2: Make the changes**

Scope: the hero lockup, the wordmark and tagline, the stage and detail lines, the spinner, and the handover into the console. Keep amethyst; amber is ON AIR and the splash is not. Keep `prefers-reduced-motion` handling on the spinner and the pulse glyph.

**Change nothing about the boot ladder's logic.** `boot.js`'s "a stub can never render green" rule, the stage/gate ordering, and the `Esc`-skips-a-stage-never-a-gate behaviour are all out of scope.

- [ ] **Step 3: Confirm the timings still hold**

`BOOT_HOLD_MS` 900 and `BOOT_CAP_MS` 4000 exist so a fast machine does not flash the splash and a slow one is not held behind it. If a change makes either wrong, say so and change the constant deliberately rather than as a side effect.

- [ ] **Step 4: Run the suite**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/views/workspacegrammar.test.js src/lib/r2livepath.test.js
npx vitest run 2>&1 | tail -10
```

Expected: 0 failed.

- [ ] **Step 5: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src/lib/Splash.svelte src/lib/boot/BootShell.svelte
git commit -F - <<'EOF'
design: a pass over the splash screen

Visual only. The boot ladder's logic is untouched: the stages and gates still
appear only when they have something to say, a stub still cannot render green,
and Escape still skips a stage and never a gate. The hold and cap timings are
unchanged, so a fast machine still does not flash the splash and a slow one is
still not held behind it.

Rendered in a browser at three widths, because this machine cannot screenshot
the Tauri window.
EOF
```

---

### Task 9: Wave 1 verification

**Files:** `docs/qa/QA_HARNESS.md` §0 only.

- [ ] **Step 1: Build, then run both suites**

```bash
cd /Users/mrgee/WebstormProjects/relay
npm run build
cd src-tauri && cargo test 2>&1 | tail -20
cargo fmt --all && cargo clippy --all-targets -- -D warnings
cd .. && npx vitest run 2>&1 | tail -20
```

Expected: 0 failed on both, clippy clean.

- [ ] **Step 2: Drive the manual checks that no test can reach**

Three things in this wave are only true on a running app, and two of them are the findings themselves:

1. **Safe mode.** With a verse on a second screen and the mic live, turn safe mode on. The screen must go clear and the detector must stop, **now**, not at the next launch. Then relaunch with safe mode still on and confirm no screen reopens.
2. **The walk-through guard.** Start a service, go to Settings → History & Backup, and confirm the button is disabled and says why.
3. **The update refusal.** Online, let the launch check succeed, start recording a service, then press Check for Updates. It must name the service. Read the status row above the button at the same moment and confirm the two agree.

- [ ] **Step 3: Record the counts and commit**

Update `docs/qa/QA_HARNESS.md` §0 with both counts beside their commands, and nowhere else.

```bash
cd /Users/mrgee/WebstormProjects/relay
git add docs/qa/QA_HARNESS.md
git commit -m "docs: record the suite counts after wave 1"
```

---

<!-- ===== was docs/archive/WAVE-DESIGNS.md, merged 2026-09-21, verbatim ===== -->

> **Archived 2026-09-21.** Historical work order. Wave 2 landed 2026-09-16 and was audited in `docs/qa/audits/DESIGN.md`; see `docs/REBRAND.md` §Status and DECISIONS §87, §88. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Wave 2 — Templates ⊕ Themes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish DECISIONS §27 by folding themes into templates, make the configured default template actually reach a screen, reshape the seed into five coordinated families across all five content kinds, separate the two five-chip registers in the template editor, and close the three overflow sources that sit outside the fit loop.

**Architecture:** Six tracks, each landing as its own pull request off `feat/wave7-timers-templates-stage`. Tracks A (default resolver), B (overflow) and C (content-look UI) are independent of each other and of everything else. Track E (theme deletion and migration) runs before Track D (the twenty-five seed), because the seed's five families are what replaces the theme concept and seeding them first would mean migrating rows that were just written. Track F is the browser-driven verification pass the design requires for this wave, and it runs last against everything the five code tracks landed.

**Tech Stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest + jsdom, `cargo test`, Chrome via the browser audit harness.

**Spec:** `docs/archive/WAVE-DESIGNS.md` (Wave 2)

**Depends on:** Waves 0 and 1 complete and green.

## Global Constraints

- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- Run `npm run build` before `cargo test` on a fresh tree (RG-127) — two `channels` tests fail on a bare 404 otherwise, because `dist/` is gitignored.
- Read test counts from the runner's own summary line, never a grep. Values live only in `docs/qa/QA_HARNESS.md` §0.
- Every new test verified to FAIL when its defect is reintroduced. Test the bug, not the fix.
- **Rule 25:** a migration must be retryable. `DROP TABLE IF EXISTS` any scratch table first, roll back on failure, never leave a transaction open for the following `PRAGMA foreign_keys = ON` to no-op inside.
- **Rule 36:** a check goes at the choke point, not at the call sites. Four bugs in this repository have the shape "enforced on one surface, skipped on its twin".
- **Rule 37:** the 45% legibility floor is a RATIO of the template's own chosen size. `TemplateRender` keeps shrinking and showing rather than blanking — a blank screen is strictly worse for a congregation.
- **Rule 41:** never `confirm()`, `alert()` or `prompt()`. Tauri's webview does not implement them. Use an in-app arm/confirm or a mounted `[role="dialog"]`.
- **Rule 42:** the two-try refit bound stays. `fittedWithTheRealFont` is inert in the shipped product (Relay bundles no webfont) — do not delete it and do not count it as protection.
- **Rule 44:** any overlay must consume `Esc` for itself on the first press and must not paint over `Clear screens`. Pinned by `panicoverlay.test.js`.
- **DECISIONS §29 / §70:** `resolveOutputTemplate`'s precedence is transparency law first, then a pinned cue template, then the screen's own, then the content look. The configured default is a new FINAL fallback, never a new winner.
- **Seeds insert by NAME and only when absent.** Never repoint an id a channel, cue or content look holds.
- `src/lib/errors.js` is the ONE backend-error humaniser. Never render a raw Rust `Err` string to a volunteer.
- Commit message bodies and PR bodies are normal English prose, not caveman.
- Never commit to `main`. Each track is a PR off `feat/wave7-timers-templates-stage`.

## Corrections to the design, verified against the working tree

The design was written at the wave 0 fork point. Four of its statements have since drifted, and one was imprecise. Verified by reading the cited lines on 2026-09-15:

1. **§2.5's "Also" item is already done.** `Settings.svelte` holds no private `ctMap`; it reads `$contentTemplates` (`src/lib/views/Settings.svelte:256` carries the comment, `:1126` the binding), and `src/lib/settingssections.test.js:944` fails if a private `ctMap` returns. **No task implements it.**
2. **`preset_templates()` holds thirteen entries, not fourteen** (`src-tauri/src/db/templates.rs:271`). The names are Midnight Blue, Royal Amethyst, Deep Teal, Crimson Grace, Emerald Word, Indigo Night, Slate Minimal, Pure Contrast, Lyric Bold, Lyric Glow, Lower Third Night, Stage Confidence, Lobby Sunrise.
3. **Line numbers have moved.** `set_themes` is `channels.rs:1440` (not 1430); `preset_template_count` is `templates.rs:526` (not 524); the theme block in `capture.js` starts at `:1882` (not 1820). Cite what you find, not what this plan says, if they move again.
4. **"Stage · Large type" is not a `STARTERS` entry.** `src/lib/layers.js:729` has a `stage` starter (Stage Display) and a `supersource` starter; `Stage · Large type` exists only as a shelf row in `src-tauri/data/shelf_templates.json`. Task 11 decides its fate explicitly rather than assuming a starter already covers it.
5. **The current seed is 38 rows on a fresh install**: 4 Rust built-ins + 1 lyrics template + 13 presets + 12 theme-family rows + 8 shelf rows.

## Operator decisions taken for this wave

- **Upgrade behaviour: retire unreferenced old presets.** The migration deletes seeded preset rows that nothing points at (no channel, no plan cue, no content look, not the configured default) and that the operator has not edited. Anything referenced or edited is kept untouched. A fresh install and an upgraded install both land near twenty-five.
- **Delivery: one PR per track.** Six PRs off the wave branch, each green on both suites.

---

# Track A — the default template actually applying

**Spec:** §2.6. **Independent of every other track.** Lands as PR 1.

Today `default_template_id` is a settings-KV integer read by two of twelve render surfaces. Rust never reads it (`grep default_template_id src-tauri/src/` returns nothing). `src/Output.svelte:77` — the real wall, the kiosk and every OBS browser source — falls back to a hard-coded `DEFAULT_TEMPLATE`, which is `BUILTINS[0]`, Classic Serif. The default reaches a screen exactly once, at creation (`Channels.svelte:384`). `setDefaultTemplate` (`capture.js:277`) is a plain `set_setting` with no broadcast.

The fix mirrors the plumbing `set_channel_template` already has: the backend owns the fact, pushes it to native windows as an event and to kiosk clients as a hub frame, and caches it so a screen that connects later is told. The output page then ends its resolution chain at the configured default instead of at a hard-coded builtin.

### Task 1: The hub carries the configured default (P1)

**Why this exists:** A browser source has no database. Until the hub carries the default, the output page cannot know what it is, and no amount of frontend resolving will put it on a wall.

**Files:**
- Modify: `src-tauri/src/channels.rs` — add `default_tpl` to `KioskHub`, `cache_default_template`, `set_default_template`, the `hello` reply, and the `FRAME_VERDICTS` row
- Modify: `src-tauri/src/main.rs` — a `set_default_template` command; warm the cache in `setup`
- Test: `src-tauri/src/channels.rs` `mod tests`

**Interfaces:**
- Produces:
  - `KioskHub::cache_default_template(&self, template_json: &str)` — validate-then-store, no push. Mirrors `cache_themes`.
  - `KioskHub::set_default_template(&self, template_json: &str)` — cache and publish `{"kind":"default_template","template":…}`.
  - `#[tauri::command] set_default_template(app, db, kiosk, template_id: Option<i64>) -> error::Result<()>`.
- Consumes: `db::set_setting` / `db::get_setting` (`src-tauri/src/db/settings.rs`), `db::get_template`.

- [ ] **Step 1: Write the failing tests**

Add to `src-tauri/src/channels.rs`'s `mod tests`:

```rust
#[test]
fn the_configured_default_is_sent_to_a_screen_that_joins_later() {
    // A BROWSER SOURCE HAS NO DATABASE. The configured default template is a
    // settings row, so the only way a kiosk or OBS client can end its
    // resolution chain at the operator's default — rather than at the bundled
    // Classic Serif — is for the hub to carry it. Cached without a push at
    // startup, pushed when it changes, and replayed on hello, exactly like the
    // per-template cache beside it.
    let hub = KioskHub::new();
    hub.cache_default_template(r#"{"id":7,"name":"House Look"}"#);
    assert_eq!(hub.default_template_json(), r#"{"id":7,"name":"House Look"}"#);
}

#[test]
fn a_malformed_default_degrades_to_null_rather_than_breaking_the_frame() {
    // The blob is embedded RAW into a WS frame, so anything that is not valid
    // JSON would produce a frame no client can parse — and a client that fails
    // to parse one frame is a screen that stops applying every frame after it.
    let hub = KioskHub::new();
    hub.cache_default_template("{not json");
    assert_eq!(hub.default_template_json(), "null");
}

#[test]
fn the_default_template_frame_is_configuration_not_a_screen_frame() {
    // Rule: only `content`, `clear` and `black` decide what a screen is
    // SHOWING and are retained as the screen frame (rule 43). The default
    // template paints nothing on its own — it tells a screen what to wear when
    // nothing else answers — so retaining it would let it stand in for the
    // verse a late-joining screen is owed.
    assert!(!is_screen_frame(
        r#"{"kind":"default_template","template":null}"#
    ));
}
```

Add the row to `FRAME_VERDICTS` in the same module:

```rust
        ("default_template", false),
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test channels::tests::the_configured_default`
Expected: FAIL — `no method named 'cache_default_template'`.

- [ ] **Step 3: Implement the hub side**

In `src-tauri/src/channels.rs`, beside the `themes` field on `KioskHub`, add:

```rust
    /// THE CONFIGURED DEFAULT TEMPLATE, as JSON, or the literal `null`.
    ///
    /// The last link in every screen's resolution chain (DECISIONS §29: the
    /// transparency law, then a pinned cue template, then the screen's own, then
    /// the content look, then THIS). A browser source has no database, so the
    /// hub is the only way it can learn the operator's default; without it the
    /// output page ends at the bundled `Classic Serif` and the configured
    /// default reaches a screen exactly once, when the channel is created.
    default_tpl: Mutex<String>,
```

initialised to `Mutex::new("null".into())` in `KioskHub::new`, and:

```rust
    /// Validate + store the default template WITHOUT pushing (startup warm).
    /// Anything that is not valid JSON becomes the literal `null`, because the
    /// value is embedded raw into a WS frame and one unparseable frame stops a
    /// client applying every frame after it.
    pub fn cache_default_template(&self, template_json: &str) {
        let safe = match serde_json::from_str::<serde_json::Value>(template_json) {
            Ok(_) => template_json.to_string(),
            Err(_) => "null".to_string(),
        };
        if let Ok(mut t) = self.default_tpl.lock() {
            *t = safe;
        }
    }

    /// The cached default template JSON (`null` when none is configured).
    pub fn default_template_json(&self) -> String {
        self.default_tpl
            .lock()
            .map(|t| t.clone())
            .unwrap_or_else(|_| "null".into())
    }

    /// Update the default template AND push it live, so a screen following the
    /// content look re-resolves the instant the operator changes the default
    /// instead of at the next reload. Same validate-then-store rule as
    /// `cache_default_template`.
    pub fn set_default_template(&self, template_json: &str) {
        self.cache_default_template(template_json);
        let blob = self.default_template_json();
        self.publish(format!(
            r#"{{"kind":"default_template","template":{blob}}}"#
        ));
    }
```

In `run_kiosk_server`'s `hello` reply, immediately after the themes block and before the transition override, send it:

```rust
                                    // The configured default template, so this
                                    // client can end its resolution chain where
                                    // the operator said rather than at the
                                    // bundled builtin. Like the template and the
                                    // themes above it, this is configuration: on
                                    // its own it paints nothing.
                                    let dblob = default_tpl
                                        .lock()
                                        .map(|t| t.clone())
                                        .unwrap_or_else(|_| "null".into());
                                    let _ = write
                                        .send(tokio_tungstenite::tungstenite::Message::Text(
                                            format!(
                                                r#"{{"kind":"default_template","template":{dblob}}}"#
                                            ),
                                        ))
                                        .await;
```

cloning `default_tpl` into the task alongside the existing `themes` clone.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test channels::`
Expected: PASS, including `the_screen_frame_matcher_agrees_with_what_is_published`.

- [ ] **Step 5: Add the command and warm the cache**

In `src-tauri/src/main.rs`, beside `set_channel_template`:

```rust
/// Set (or clear, with `None`) the DEFAULT template — the last link in every
/// screen's resolution chain — and push the change live.
///
/// Changing the default used to be a bare `set_setting` from the frontend: it
/// was read at channel creation and by two console panes, and nothing else in
/// the building was told. A screen already following the content look kept the
/// old look until it was reopened, which is why the default "did not activate on
/// all screens". Native windows get `channel://retemplate` with a null channel
/// (meaning: this is not about one screen); kiosk/OBS clients get the hub frame.
#[tauri::command]
fn set_default_template<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: tauri::State<'_, Db>,
    kiosk: tauri::State<'_, channels::KioskHub>,
    template_id: Option<i64>,
) -> error::Result<()> {
    // DB write + resolve the JSON under one lock, release BEFORE emitting
    // (rule 2: never hold a Mutex across emit).
    let tjson = {
        let conn = db.0.lock()?;
        match template_id {
            Some(id) => {
                db::set_setting(&conn, "default_template_id", &id.to_string())?;
                db::get_template(&conn, id)?.and_then(|t| serde_json::to_string(&t).ok())
            }
            None => {
                db::set_setting(&conn, "default_template_id", "")?;
                None
            }
        }
    };
    let blob = tjson.unwrap_or_else(|| "null".into());
    kiosk.set_default_template(&blob);
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&blob) {
        let _ = app.emit(
            "output://default_template",
            serde_json::json!({ "template": v }),
        );
    }
    Ok(())
}
```

Register it in the `invoke_handler!` list, and warm the cache in `setup` where `cache_themes` is already called:

```rust
    // The configured default, warmed before any client can connect — a screen
    // that joins during launch must not be told the default is `null` and then
    // corrected.
    let dj = db::get_setting(&conn, "default_template_id")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(|id| db::get_template(&conn, id).ok().flatten())
        .and_then(|t| serde_json::to_string(&t).ok())
        .unwrap_or_else(|| "null".into());
    kiosk.cache_default_template(&dj);
```

- [ ] **Step 6: Verify the whole Rust suite and the contract**

Run: `npm run build && cd src-tauri && cargo test`
Expected: PASS. Then `cargo fmt --all && cargo clippy --all-targets -- -D warnings`.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/channels.rs src-tauri/src/main.rs
git commit -m "feat: the hub carries the configured default template"
```

### Task 2: The output page ends its chain at the configured default (P1)

**Why this exists:** `src/Output.svelte:77` falls back to `DEFAULT_TEMPLATE` — `BUILTINS[0]`, Classic Serif — so the real wall, the kiosk and every OBS browser source ignore the operator's default entirely. The design's rule is: never a bare `BUILTINS[0]`.

**Files:**
- Modify: `src/lib/layers.js:262` — `resolveOutputTemplate` gains a fourth argument
- Modify: `src/Output.svelte` — consume the new frame and the new event, pass the default into the resolver
- Test: `src/lib/defaulttemplate.test.js` (create)

**Interfaces:**
- Consumes: `KioskHub`'s `default_template` frame and `output://default_template` (Task 1).
- Produces: `resolveOutputTemplate(channelTpl, override, pinned = false, fallback = null)` — the fallback is the LAST link, applied only when nothing else answered. Precedence above it is unchanged (DECISIONS §29 / §70).

- [ ] **Step 1: Write the failing test**

Create `src/lib/defaulttemplate.test.js`:

```js
import { describe, it, expect } from 'vitest';
import { resolveOutputTemplate } from './layers.js';

// THE CONFIGURED DEFAULT IS THE LAST LINK, NEVER A NEW WINNER.
//
// DECISIONS §29 and §70 rank the authorities: the transparency law first, then
// a pinned cue template, then the screen's own template, then the content look.
// Before this test the chain simply ended, and every caller finished the
// sentence itself with `|| DEFAULT_TEMPLATE` — the bundled Classic Serif — so
// the operator's default reached a screen exactly once, when the channel was
// created. Adding it at the END is the whole change; adding it anywhere else
// would let a house look overrule a screen the operator styled by hand.

const keyed = { id: 1, name: 'Band', layout: { lowerThird: true } };
const opaque = { id: 2, name: 'Full', layout: { lowerThird: false } };
const look = { id: 3, name: 'Look', layout: { lowerThird: false } };
const fallback = { id: 9, name: 'House', layout: { lowerThird: false } };

describe('resolveOutputTemplate — the configured default', () => {
  it('answers when the screen has no template and no look applies', () => {
    expect(resolveOutputTemplate(null, null, false, fallback)).toBe(fallback);
  });

  it('never beats the screen own template', () => {
    expect(resolveOutputTemplate(opaque, null, false, fallback)).toBe(opaque);
  });

  it('never beats a pinned cue template', () => {
    expect(resolveOutputTemplate(null, look, true, fallback)).toBe(look);
  });

  it('never beats a content look', () => {
    expect(resolveOutputTemplate(null, look, false, fallback)).toBe(look);
  });

  it('does not break the transparency law on a keyed screen', () => {
    // An OPAQUE fallback on a KEYED channel would blot out the camera the band
    // exists to caption. The keyed screen keeps its own template.
    expect(resolveOutputTemplate(keyed, null, false, fallback)).toBe(keyed);
  });

  it('is optional — an absent fallback resolves exactly as before', () => {
    expect(resolveOutputTemplate(null, null, false)).toBe(null);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/lib/defaulttemplate.test.js`
Expected: FAIL — `expected null to be { id: 9, … }`.

- [ ] **Step 3: Implement the resolver change**

In `src/lib/layers.js`, change the signature and the final return:

```js
export function resolveOutputTemplate(channelTpl, override, pinned = false, fallback = null) {
```

and end the existing chain with `?? fallback ?? null` rather than `?? null`, leaving every branch above it untouched. Document it at the call site:

```js
 * `fallback` is the operator's CONFIGURED DEFAULT (`default_template_id`) and is
 * the last link: it answers only when nothing above it did. It is not part of
 * the ranking §29 and §70 describe — those decide between authorities that each
 * chose a look for this screen, and the default is what remains when none did.
```

- [ ] **Step 4: Run it to verify it passes**

Run: `npx vitest run src/lib/defaulttemplate.test.js`
Expected: PASS, 6 tests.

- [ ] **Step 5: Wire the output page**

In `src/Output.svelte`, add a `defaultTpl` variable, apply the new frame inside `applyMessage`:

```js
    } else if (m.kind === 'default_template') {
      // THE CONFIGURED DEFAULT, pushed by the hub on connect and whenever the
      // operator changes it. This screen follows the content look, so a change
      // here is a change to what it wears — it is applied live rather than at
      // the next reload, which is what "the default does not activate on all
      // screens" actually was.
      defaultTpl = m.template ?? null;
```

listen for the desktop event beside `channel://retemplate`:

```js
      unlisten.push(
        await listen('output://default_template', (e) => {
          defaultTpl = e.payload?.template ?? null;
        }),
      );
```

and pass it into the resolver:

```js
  $: activeTemplate =
    resolveOutputTemplate(t, override, !!content?.template_pinned, defaultTpl) || DEFAULT_TEMPLATE;
```

`DEFAULT_TEMPLATE` stays as the floor beneath the floor: a template must always be paintable, and a configured default that fails to resolve must not blank a wall.

- [ ] **Step 6: Verify the contract test and the suite**

Run: `npx vitest run`
Expected: PASS. `ipc.test.js` must accept `output://default_template` — it is emitted by Rust and listened for here, so it needs no `DELIBERATELY_UNHEARD` entry.

- [ ] **Step 7: Commit**

```bash
git add src/lib/layers.js src/Output.svelte src/lib/defaulttemplate.test.js
git commit -m "fix: the wall ends its template chain at the configured default"
```

### Task 3: The console surfaces stop writing the chain out by hand (P2)

**Why this exists:** `Channels.svelte:485` and `:547` and `Live.svelte:1360` each finish the sentence themselves, with slightly different words: two `|| DEFAULT_TEMPLATE`, one `|| $templates[0]`. Three hand-written chains are how the console pane and the wall came to disagree before (the comment at `Live.svelte:1347` records it). One resolver, four callers.

**Files:**
- Modify: `src/lib/views/Channels.svelte:485,547` — resolve through `resolveOutputTemplate`
- Modify: `src/lib/views/Live.svelte:1358-1362` — same chain, same order
- Modify: `src/lib/stores/capture.js:277` — `setDefaultTemplate` calls the new command
- Test: `src/lib/defaulttemplate.test.js` (extend), `src/lib/ipc.test.js` (already enforces the command exists)

**Interfaces:**
- Consumes: `resolveOutputTemplate(channelTpl, override, pinned, fallback)` (Task 2), `set_default_template` (Task 1).
- Produces: no new exports. `setDefaultTemplate(id)` keeps its signature and gains a broadcast.

- [ ] **Step 1: Write the failing test**

Append to `src/lib/defaulttemplate.test.js`:

```js
import { vi } from 'vitest';

// CHANGING THE DEFAULT IS NEWS. `setDefaultTemplate` was a bare `set_setting`,
// so the value changed in the database and no screen in the building was told.
// The command it calls now is the one that also pushes.
describe('setDefaultTemplate', () => {
  it('goes through the command that broadcasts, not through set_setting', async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    vi.doMock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
    const { setDefaultTemplate } = await import('./stores/capture.js');
    await setDefaultTemplate(7);
    const names = invoke.mock.calls.map((c) => c[0]);
    expect(names).toContain('set_default_template');
    expect(names).not.toContain('set_setting');
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/lib/defaulttemplate.test.js -t "broadcasts"`
Expected: FAIL — the call list contains `set_setting`.

- [ ] **Step 3: Implement**

In `src/lib/stores/capture.js`, replace the body of `setDefaultTemplate`:

```js
export async function setDefaultTemplate(id) {
  const call = await invoke();
  // The COMMAND, not the raw setting. Rust owns the fact now: it writes the
  // row, pushes `default_template` to every kiosk client and emits
  // `output://default_template` to native windows, so a screen following the
  // content look re-resolves at once instead of at its next reload.
  await call('set_default_template', { templateId: id ?? null });
  defaultTemplateId.set(id ?? null);
}
```

In `src/lib/views/Live.svelte`, replace the hand-written chain:

```js
  $: mainTpl =
    resolveOutputTemplate(
      (mainChannel && $templates.find((t) => t.id === mainChannel.template_id)) || null,
      null,
      false,
      $templates.find((t) => t.id === $defaultTemplateId) || null,
    ) || $templates[0] || null;
```

In `src/lib/views/Channels.svelte`, pass `$templates.find((t) => t.id === $defaultTemplateId) || null` as the fourth argument at both `:485` and `:547`, keeping `|| DEFAULT_TEMPLATE` as the floor.

- [ ] **Step 4: Run the suites**

Run: `npx vitest run`
Expected: PASS, including `ipc.test.js` (the new command exists in Rust from Task 1).

- [ ] **Step 5: Commit**

```bash
git add src/lib/stores/capture.js src/lib/views/Live.svelte src/lib/views/Channels.svelte src/lib/defaulttemplate.test.js
git commit -m "fix: one resolver answers what a screen wears, on all four surfaces"
```

### Task 4: The gallery preview and the console readout tell the same truth (P2)

**Why this exists:** `TemplateGallery.svelte`'s inspector preview is hard-bound to the selected row, so it never shows what a screen would actually paint. And `cue_or_content_tpl` (`main.rs:3029`) answers `None` for a kind with no content look, so the console readout says "no template" where the wall will in fact wear the default.

**Files:**
- Modify: `src-tauri/src/main.rs:3029-3055` — `cue_or_content_tpl` falls back to `default_template_id`
- Modify: `src/lib/views/templates/TemplateGallery.svelte` — preview through the resolver
- Test: `src-tauri/src/main.rs` (`mod tests` beside the existing template tests) or `src-tauri/src/e2e.rs` if a live fire is needed

**Interfaces:**
- Consumes: `db::content_template_id`, `db::get_setting`.
- Produces: no new exports; `cue_or_content_tpl` keeps its `(Option<i64>, Option<String>, bool)` shape. **It must keep returning `None` for the JSON** — shipping the default template's JSON on every fire is the megabyte regression the comment at `:3043` records.

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` in `src-tauri/src/main.rs`:

```rust
#[test]
fn a_kind_with_no_content_look_answers_with_the_configured_default() {
    // The console readout names the template a fire will wear. With no content
    // look set for this kind it said "none" — while the wall, since the default
    // now reaches it, wears the operator's default. Two surfaces, one fire, two
    // answers. The ID travels; the JSON deliberately does not (a default
    // carrying an embedded image has been 13 MB, and it used to be serialized
    // and broadcast on every single fire).
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    db::migrate(&conn, true).unwrap();
    db::set_setting(&conn, "default_template_id", "3").unwrap();
    let (id, json, pinned) = cue_or_content_tpl(&conn, None, "scripture");
    assert_eq!(id, Some(3));
    assert!(json.is_none(), "the default must never ship its JSON");
    assert!(!pinned, "a default is not a deliberate per-cue choice");
}

#[test]
fn a_content_look_still_beats_the_configured_default() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    db::migrate(&conn, true).unwrap();
    db::set_setting(&conn, "default_template_id", "3").unwrap();
    db::set_content_template(&conn, "scripture", Some(5)).unwrap();
    let (id, _, _) = cue_or_content_tpl(&conn, None, "scripture");
    assert_eq!(id, Some(5));
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd src-tauri && cargo test a_kind_with_no_content_look`
Expected: FAIL — `assertion failed: left: None, right: Some(3)`.

- [ ] **Step 3: Implement**

In `cue_or_content_tpl`, after the `content_template_id` lookup:

```rust
    let id = db::content_template_id(conn, kind).ok().flatten().or_else(|| {
        // NOTHING BOUND THIS KIND, so the screens following the content look
        // wear the configured default. The id travels so the console readout
        // names what the wall will actually paint; the JSON still does not.
        db::get_setting(conn, "default_template_id")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<i64>().ok())
    });
```

- [ ] **Step 4: Run to verify they pass**

Run: `cd src-tauri && cargo test cue_or_content && cargo test e2e`
Expected: PASS.

- [ ] **Step 5: Point the gallery preview at the resolver**

In `src/lib/views/templates/TemplateGallery.svelte`, the inspector preview renders the selected row. Render instead what a screen following the content look would wear, and say which authority answered:

```js
  // WHAT A SCREEN WOULD ACTUALLY PAINT. The preview used to render the selected
  // row unconditionally, which is right for "what does this template look like"
  // and wrong for the question an operator is asking in the inspector — "what
  // happens when this fires". The chain is the wall's own (DECISIONS §29 / §70).
  $: previewTpl =
    resolveOutputTemplate(selected, null, false, $templates.find((t) => t.id === $defaultTemplateId) || null) ||
    DEFAULT_TEMPLATE;
```

- [ ] **Step 6: Run both suites**

Run: `npm run build && npx vitest run && cd src-tauri && cargo test`
Expected: PASS on both summary lines.

- [ ] **Step 7: Commit and open the PR**

```bash
git add src-tauri/src/main.rs src/lib/views/templates/TemplateGallery.svelte
git commit -m "fix: the console readout and the gallery preview name the template the wall will wear"
# --fill takes the body from this track's commit messages, which are already normal English prose.
gh pr create --base feat/wave7-timers-templates-stage --title "Wave 2 track A — the default template actually applying" --fill
```

---

# Track B — the three overflow sources outside the fit loop

**Spec:** §2.7. **Independent of every other track.** Lands as PR 2.

Rule 37 says a fit loop with no notion of failure always succeeds. These three are worse: they are not in the loop at all, so they cannot even fail. `fitLayers` queries `.ltext` (`TemplateRender.svelte:1239`), `fitText` queries `.slide .content` (`:361`), and `fitBoxes()` (`:583`) queries one or the other — so the default countdown block, the ticker and the contrast panel are each invisible to the search, to `overflowing()` and to `onFit`. Live's "how small it went" readout therefore cannot report them either.

**Do not weaken rules 37 or 42.** Keep shrinking and showing rather than blanking; the 45% floor stays a ratio of the template's own size; the two-try refit bound stays.

### Task 5: The default countdown block joins the fit loop (P1)

**Why this exists:** `TemplateRender.svelte:1483-1497` emits `.cd-default` in layer mode, outside `{#each layerViews}`, carrying no `.ltext` / `.lfit` class. Its CSS (`:1649`) sets no `overflow` at all and it renders the digits at `verseSize * 2`. A countdown is the one thing on a pre-service wall, at the largest type Relay ever paints, on a screen nobody is watching until the room fills.

**Files:**
- Modify: `src/lib/TemplateRender.svelte:1483-1497` (markup) and `:1649` (CSS)
- Test: `src/lib/fitcoverage.test.js` (create)

**Interfaces:**
- Consumes: the existing `.ltext` / `.lfit` contract — a `.ltext` box with a single `.lfit` child carrying `data-base` (cqw) and `data-fit` (`both` | `shrink` | `none`). `fitLayers` reads exactly this.
- Produces: no new exports.

- [ ] **Step 1: Write the failing test**

Create `src/lib/fitcoverage.test.js`:

```js
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/svelte';
import TemplateRender from './TemplateRender.svelte';

// WHAT THE FIT LOOP CANNOT SEE, IT CANNOT SHRINK — AND CANNOT REPORT.
//
// `fitLayers` queries `.ltext`; `fitText` queries `.slide .content`. The default
// countdown block was emitted outside `{#each layerViews}` with neither class,
// so the largest type Relay ever paints (verseSize × 2) was outside the binary
// search, outside `overflowing()` and outside `onFit`. Rule 37's "a fit loop
// with no notion of failure always succeeds" one level up: this one had no
// notion of the box at all, and its CSS set no `overflow` either, so it did not
// even clip.
//
// jsdom does not lay out, so this test asserts the CONTRACT the fitter reads —
// the classes and the data attributes — not a pixel. The pixel half is Task 16's
// browser pass, against the real backend at 1920×1080.

const timerTemplate = {
  id: 1,
  name: 'Timer',
  layout: { layers: [{ id: 'bg', type: 'bg', fill: '#000' }] },
  style: { verseSize: '6' },
};

describe('the default countdown block is inside the fit loop', () => {
  it('emits the .ltext / .lfit contract fitLayers reads', () => {
    const { container } = render(TemplateRender, {
      props: {
        template: timerTemplate,
        content: { kind: 'countdown', reference: 'Service begins in', countdown_to: Date.now() + 60000 },
      },
    });
    const box = container.querySelector('.cd-default .ltext');
    expect(box, 'the digits must sit in a box the fitter queries').toBeTruthy();
    const fit = box.querySelector('.lfit');
    expect(fit, 'the box must hold exactly the .lfit element fitLayers sizes').toBeTruthy();
    expect(fit.dataset.base, 'the designed size must be declared, not only fitted').toBeTruthy();
    expect(fit.dataset.fit).toBe('shrink');
  });

  it('puts the label in the loop too', () => {
    const { container } = render(TemplateRender, {
      props: {
        template: timerTemplate,
        content: { kind: 'countdown', reference: 'Service begins in', countdown_to: Date.now() + 60000 },
      },
    });
    expect(container.querySelectorAll('.cd-default .ltext').length).toBe(2);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/lib/fitcoverage.test.js`
Expected: FAIL — `the digits must sit in a box the fitter queries: expected null to be truthy`.

- [ ] **Step 3: Implement the markup**

Replace the `.cd-default` body at `TemplateRender.svelte:1486-1495`:

```svelte
      <div class="cd-default">
        {#if content.reference && !countdownDone}
          <div class="ltext cd-line cd-ref" style="align-items:center;">
            <div class="lfit" data-base={refSize} data-fit="shrink" style="font-size:{refSize}cqw; {refStyle} text-align:center;">{content.reference}</div>
          </div>
        {/if}
        <div class="ltext cd-line cd-digits" style="align-items:center;">
          <!-- THE SIZE IS DECLARED AND THE BOX IS MEASURABLE. `data-base` is the
               designed size in cqw, so a countdown that has not been fitted yet
               paints at the size the template asked for rather than at the app's
               UI type (the same reasoning as the layer text path above). `shrink`
               caps growth at that size: a countdown must never grow to fill a
               box, because the digits change width every second and a growing
               clock jitters. -->
          <div class="lfit" data-base={verseSize * 2} data-fit="shrink" class:warn={countdownWarn}
            style="font-size:{verseSize * 2}cqw; margin-top:{refGap}cqw; color:{countdownWarn ? CD_WARN : verseColor}; text-align:center; text-shadow:{verseShadowCss};">
            {countdownDone ? (content.countdown_done || '0:00') : countdownText}
          </div>
        </div>
      </div>
```

The `.verse.countdown` class is dropped from the digits element on purpose: it existed to carry type styling that the `.lfit` element now carries, and `fitOne` explicitly skips its seed estimate for `.countdown` (`:306`) — a rule that belongs to the region path and does not apply here.

- [ ] **Step 4: Implement the CSS**

At `TemplateRender.svelte:1649`, add the clip and position the two boxes, since `.ltext` is absolutely positioned for layers and these are flex children:

```css
  .cd-default {
    position: absolute;
    inset: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 6% 7%;
    box-sizing: border-box;
    /* IT CLIPS. This set no `overflow` at all, so a countdown too big for its
       box did not slice — it painted straight over the template's own layers and
       off the edge of the screen. Clipping is what makes the fitter's verdict
       honest: `overflowing()` reads `scrollHeight > clientHeight`, which an
       unclipped box never reports. */
    overflow: hidden;
  }
  /* The two fit boxes inside it are flex children, not absolutely-positioned
     layers, so they override `.ltext`'s `position: absolute`. `min-height: 0`
     is what lets a flex child actually be shorter than its content — without it
     the box reports that everything fits, at any size. */
  .cd-default .cd-line {
    position: relative;
    display: flex;
    width: 100%;
    min-height: 0;
    overflow: hidden;
    justify-content: center;
  }
  .cd-default .cd-digits {
    flex: 1 1 auto;
  }
  .cd-default .cd-ref {
    flex: 0 0 auto;
    max-height: 25%;
  }
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `npx vitest run src/lib/fitcoverage.test.js`
Expected: PASS, 2 tests.

- [ ] **Step 6: Verify the defect reproduces when reverted**

Revert only the two `class="ltext …"` wrappers (keep the CSS), re-run the test, confirm FAIL, then restore. Test the bug, not the fix.

- [ ] **Step 7: Run the whole frontend suite**

Run: `npx vitest run`
Expected: PASS. Watch `templatefit.test.js`, `cardfit.test.js` and `countdown`-named tests in particular.

- [ ] **Step 8: Commit**

```bash
git add src/lib/TemplateRender.svelte src/lib/fitcoverage.test.js
git commit -m "fix: the default countdown block is measured, clipped and reported like every other box"
```

### Task 6: Ticker mode gets its own fit pass (P1)

**Why this exists:** `.ticker` / `.ticker-label` / `.ticker-run` (`TemplateRender.svelte:1528`) render **instead of** `.content`. `fitText` queries `.slide .content` (`:361`), so in ticker mode the loop finds zero boxes and `lastFitScale` stays 1 — a template reporting a perfect fit while `.ticker-label` (`white-space: nowrap`, raw `refSize`) pushes the scrolling body off its own band.

**Files:**
- Modify: `src/lib/TemplateRender.svelte` — `fitText`, `fitBoxes`
- Test: `src/lib/fitcoverage.test.js` (extend)

**Interfaces:**
- Consumes: `FIT_STEP`, `keepShrinking` (`src/lib/templatemodel.js`), `lastFitScale`, `onFit`.
- Produces: `function fitTicker(): number` — module-private; returns the scale it landed on, for `fitText` to fold into `worst`.

- [ ] **Step 1: Write the failing test**

Append to `src/lib/fitcoverage.test.js`:

```js
// A TICKER IS NOT EXEMPT FROM BEING MEASURED.
//
// The crawl renders INSTEAD OF `.content`, and the region fitter queries
// `.slide .content`. So an announcement in ticker mode found zero boxes, the
// loop ran zero times, and `lastFitScale` stayed at 1 — a template reporting a
// perfect fit with its fixed label shoving the body clean off the band. The
// label is `white-space: nowrap` at raw `refSize`, so it never wraps and never
// clips; it just takes the room.
describe('ticker mode is measured', () => {
  it('reports a fit for the ticker, not the silence of an empty query', () => {
    const scales = [];
    render(TemplateRender, {
      props: {
        template: {
          id: 2,
          name: 'Notice',
          layout: { regions: ['verse_text', 'reference'], align: 'center' },
          style: { scroll: true, refSize: '2', verseSize: '3' },
        },
        content: { kind: 'announce', reference: 'THIS SUNDAY', text: 'Church picnic after the second service.' },
        onFit: (info) => scales.push(info),
      },
    });
    expect(scales.length, 'ticker mode must report through the same reporter as every other mode').toBeGreaterThan(0);
  });

  it('queries the ticker label as a fit box', async () => {
    const { container } = render(TemplateRender, {
      props: {
        template: {
          id: 2,
          name: 'Notice',
          layout: { regions: ['verse_text', 'reference'], align: 'center' },
          style: { scroll: true, refSize: '2', verseSize: '3' },
        },
        content: { kind: 'announce', reference: 'THIS SUNDAY', text: 'Church picnic.' },
      },
    });
    expect(container.querySelector('.ticker-label')).toBeTruthy();
    expect(container.querySelector('.slide .content'), 'ticker renders instead of .content').toBeFalsy();
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/lib/fitcoverage.test.js -t "ticker"`
Expected: FAIL on the first case — `onFit` is never called because the fitter found nothing to measure.

- [ ] **Step 3: Implement the ticker fit**

In `src/lib/TemplateRender.svelte`, beside `fitText`:

```js
  /**
   * THE CRAWL HAS A BUDGET, AND THE LABEL SPENDS IT FIRST.
   *
   * A ticker is a band: a fixed label on the left, then the body scrolling
   * through whatever room is left. Neither element is `.content`, so the region
   * fitter never saw either — the loop found zero boxes and reported a scale of
   * 1, which is the "fit loop with no notion of failure" of rule 37 with the
   * loop removed entirely.
   *
   * The label is what is worth measuring: it is `nowrap`, so it never wraps and
   * never clips, it simply takes the width. Cap it at 45% of the band and shrink
   * it on the same 0.95 curve every other box uses, so one long label cannot
   * leave the body with nothing to scroll through. The body itself is
   * deliberately NOT shrunk: it scrolls, so its length is time, not overflow.
   */
  const TICKER_LABEL_SHARE = 0.45;
  function fitTicker() {
    if (!stageEl) return 1;
    const band = stageEl.querySelector('.ticker');
    const label = stageEl.querySelector('.ticker-label');
    if (!band || !label) return 1;
    const budget = (band.clientWidth || 0) * TICKER_LABEL_SHARE;
    if (budget <= 0) return 1;
    const base = parseFloat(getComputedStyle(label).fontSize) || 0;
    if (!base) return 1;
    let scale = 1;
    label.style.fontSize = `${base}px`;
    while (keepShrinking({ overflowing: label.scrollWidth > budget, scale })) {
      scale *= FIT_STEP;
      label.style.fontSize = `${base * scale}px`;
    }
    return scale;
  }
```

and call it from `fitText`, folding its answer into the same `worst`:

```js
    stageEl.querySelectorAll('.slide .content').forEach((box) => {
      worst = Math.min(worst, fitOne(box, stageEl));
    });
    // Ticker mode renders instead of `.content`, so the query above finds
    // nothing at all. Its own pass is the only measurement this mode gets.
    worst = Math.min(worst, fitTicker());
```

Finally add the label to `fitBoxes()` so `overflowing()` and the late re-look can see it:

```js
  function fitBoxes() {
    if (!stageEl) return [];
    return [
      ...(layered
        ? stageEl.querySelectorAll('.ltext')
        : stageEl.querySelectorAll('.slide .content, .slide .ticker-label')),
    ];
  }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run src/lib/fitcoverage.test.js`
Expected: PASS.

- [ ] **Step 5: Verify the defect reproduces when reverted**

Remove the `fitTicker()` line from `fitText`, confirm the first ticker case FAILS, restore.

- [ ] **Step 6: Commit**

```bash
git add src/lib/TemplateRender.svelte src/lib/fitcoverage.test.js
git commit -m "fix: ticker mode is measured instead of reporting the silence of an empty query"
```

### Task 7: The contrast panel clips (P2)

**Why this exists:** `.content.panel { overflow: visible }` (`TemplateRender.svelte:1729`) removes the clip `.content` otherwise provides (`:1719`, `overflow: hidden`, `max-height: 92%`). The panel is chosen for a bright background — the case where legibility is already hardest — and it is the one mode where overflow leaves the plate entirely rather than being sliced by it.

**Files:**
- Modify: `src/lib/TemplateRender.svelte:1729`
- Test: `src/lib/fitcoverage.test.js` (extend)

**Interfaces:** none new.

- [ ] **Step 1: Write the failing test**

Append to `src/lib/fitcoverage.test.js`:

```js
// THE PLATE IS A BOX, NOT A SUGGESTION.
//
// `.content` clips at `max-height: 92%`; `.content.panel` set `overflow:
// visible` and took the clip away. `overflowing()` reads `scrollHeight >
// clientHeight`, which an unclipped box does not report — so the one mode
// chosen for a hard-to-read background was also the one mode whose overflow the
// fitter could not detect. Asserted on the stylesheet the component ships,
// because jsdom does not lay out.
describe('the contrast panel clips', () => {
  it('does not set overflow: visible', async () => {
    const src = await import('node:fs').then((fs) =>
      fs.readFileSync('src/lib/TemplateRender.svelte', 'utf8'),
    );
    const block = src.slice(src.indexOf('.content.panel {'));
    const rule = block.slice(0, block.indexOf('}'));
    expect(rule, '.content.panel must not remove the clip .content provides').not.toMatch(
      /overflow:\s*visible/,
    );
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/lib/fitcoverage.test.js -t "contrast panel"`
Expected: FAIL — the rule matches `overflow: visible`.

- [ ] **Step 3: Implement**

```css
  .content.panel {
    padding: 3.5cqw 4.5cqw;
    max-width: 82%;
    /* IT CLIPS, LIKE `.content` DOES. This was `overflow: visible`, which took
       away the clip the unpanelled box has — and with it the only signal
       `overflowing()` reads. The panel is the mode an operator picks for a
       bright background, which is where legibility is already hardest, so it
       was the worst box in the component to have left unmeasured. The padding
       still gives the plate room; what it no longer does is let the words leave
       the plate. */
    overflow: hidden;
  }
```

- [ ] **Step 4: Run the suite**

Run: `npx vitest run`
Expected: PASS on the summary line.

- [ ] **Step 5: Commit and open the PR**

```bash
git add src/lib/TemplateRender.svelte src/lib/fitcoverage.test.js
git commit -m "fix: the contrast panel clips, so its overflow can be seen at all"
# --fill takes the body from this track's commit messages, which are already normal English prose.
gh pr create --base feat/wave7-timers-templates-stage --title "Wave 2 track B — the three overflow sources outside the fit loop" --fill
```

---

# Track C — the two registers stop looking like one fact printed twice

**Spec:** §2.5. **Independent of every other track.** Lands as PR 3.

The operator reported that clicking a content look "activates all". It does not — no handler writes more than one kind per click, and `toggleShows` (`TemplateEditor.svelte:539`) already materialises the implicit list correctly as all-kinds-minus-the-clicked-one. The defect is the render: two visually identical five-chip grids, both `.te-showgrid`, over the same five `CONTENT_KINDS` labels (`:1698` and `:1718`). The second starts all-ticked because `templateShows` returns `true` for every kind when `layout.shows` is absent (`layers.js:246`), so the first click looks like it wiped four.

Both rows are real features and neither is deleted. **"Used for"** is a global binding written by `setContentTemplate` (DECISIONS §70). **"Content this template renders"** is the per-template filter on `layout.shows`, read by `Output.svelte` and `templateShows`.

**Note:** §2.5's third item — `Settings.svelte`'s private `ctMap` — is already fixed and pinned. Do not re-implement it.

### Task 8: A switch list in its own section, not a second chip grid (P1)

**Files:**
- Modify: `src/lib/views/templates/TemplateEditor.svelte:1715-1727` (markup) and the `.te-showgrid` CSS block at `:2119`
- Test: `src/lib/inspectorobjects.test.js` (extend) or `src/lib/showsregister.test.js` (create — prefer create, the existing file is about objects on a slide)

**Interfaces:**
- Consumes: `CONTENT_KINDS`, `templateShows` (`src/lib/layers.js`), `toggleShows`, `toggleUsedFor` (unchanged).
- Produces: no new exports. The second register renders as `role="switch"` controls under an `<h3 class="te-sec">` heading of its own.

- [ ] **Step 1: Write the failing test**

Create `src/lib/showsregister.test.js`:

```js
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';

// TWO FACTS, TWO SHAPES.
//
// `Used for` is a GLOBAL binding: tick Scripture here and every screen set to
// "Follow the content look" wears this template when scripture fires. `Content
// this template renders` is a PER-TEMPLATE filter: untick Media and a stage
// monitor wearing this template ignores a picture and holds the passage.
//
// They were rendered as two identical `.te-showgrid` chip rows over the same
// five labels, one above the other, and the second starts all-ticked because an
// absent `layout.shows` means "shows everything". So the first click on the
// second row materialised the list as all-minus-one — four chips visibly going
// dark at once — which is exactly what an operator reported as "clicking a
// content look activates all". Nothing was wrong with the handler. The render
// was telling them something false about what they had just done.
//
// Asserted on the source because the distinction is a rendering decision: the
// point is that the two registers cannot be mistaken for each other, and a test
// that mounted the editor and read chip text would have passed throughout the
// life of the bug.

const src = readFileSync('src/lib/views/templates/TemplateEditor.svelte', 'utf8');

describe('the two content-kind registers are told apart', () => {
  it('uses one chip grid, not two', () => {
    const grids = src.match(/class="te-showgrid"/g) || [];
    expect(grids.length, '`Used for` keeps the chip grid; the filter must not wear the same shape').toBe(1);
  });

  it('renders the per-template filter as switches', () => {
    const block = src.slice(src.indexOf('Content this template renders'));
    expect(block.slice(0, 1200)).toMatch(/role="switch"/);
    expect(block.slice(0, 1200)).toMatch(/aria-checked/);
  });

  it('gives the filter its own section heading', () => {
    const block = src.slice(src.indexOf('Content this template renders') - 400, src.indexOf('Content this template renders'));
    expect(block, 'a heading is what makes the boundary between the two facts visible').toMatch(/te-sec/);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/lib/showsregister.test.js`
Expected: FAIL — two `.te-showgrid` matches, no `role="switch"`.

- [ ] **Step 3: Implement the markup**

Replace the second register at `TemplateEditor.svelte:1717-1727`:

```svelte
          <h3 class="te-sec te-showsec">Content this template renders</h3>
          <p class="te-fnote te-showintro">An unticked kind is not blanked — a screen wearing this template holds what it already had.</p>
          <div class="te-showlist">
            {#each CONTENT_KINDS as k}
              <button
                class="te-showrow"
                role="switch"
                aria-checked={templateShows(edit, k.key)}
                on:click={() => toggleShows(k.key)}
              >
                <span class="te-showname">{k.label}</span>
                <span class="te-showstate" aria-hidden="true">{templateShows(edit, k.key) ? 'Shows' : 'Ignores'}</span>
              </button>
            {/each}
          </div>
```

The heading and the `Shows` / `Ignores` word carry the difference: `Used for` answers "when this kind fires, who wears this template", and this one answers "when this kind fires, does a screen wearing this template act on it". A tick that means two different things on one panel is what produced the report.

- [ ] **Step 4: Implement the CSS**

Beside `.te-showgrid` at `:2119`:

```css
  .te-showlist { display: flex; flex-direction: column; gap: 2px; }
  .te-showrow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 7px 10px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--panel-2);
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .te-showrow[aria-checked='false'] { color: var(--text-dim); }
  /* NEVER A LAW COLOUR. Amber is ON AIR, amethyst is rehearsal, cyan is a guess
     (rule 18 · DESIGN_SYSTEM §1.1). This is a configuration switch in an editor:
     it says Shows or Ignores in words, and the state reads without colour. */
  .te-showstate { font-size: 0.85em; letter-spacing: 0.04em; text-transform: uppercase; }
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `npx vitest run src/lib/showsregister.test.js`
Expected: PASS, 3 tests.

- [ ] **Step 6: Check the surface inventory still sees every control**

Run: `node scripts/qa-inventory.mjs`
Expected: 0 handlerless buttons, 0 unnamed controls. A `role="switch"` with no `aria-checked` would be reported.

- [ ] **Step 7: Run the frontend suite**

Run: `npx vitest run`
Expected: PASS. `inspectorobjects.test.js` asserts `Used for` by name — it must still pass untouched, because that register is unchanged.

- [ ] **Step 8: Commit and open the PR**

```bash
git add src/lib/views/templates/TemplateEditor.svelte src/lib/showsregister.test.js
git commit -m "fix: the per-template content filter stops wearing the content look's shape"
# --fill takes the body from this track's commit messages, which are already normal English prose.
gh pr create --base feat/wave7-timers-templates-stage --title "Wave 2 track C — the two content-kind registers are told apart" --fill
```

---

# Track E — themes are folded into templates

**Spec:** §2.2, §2.3. **Runs before Track D.** Lands as PR 4.

A theme has no field a template does not already have: `THEME_STYLE_KEYS` (`src/lib/themes.js:29`) is a twenty-one-entry subset of the same flat `style` keys. There is no `themes` table, no Rust `Theme` struct — custom themes are one JSON array in `app_settings['themes.custom']`. `LAYER_THEME_KEYS` (`:552`) records that nine of the fourteen theme controls move nothing on a layer template. DECISIONS §27 called themes "a style layer beneath templates, not a parallel system"; this completes that rather than contradicting it.

**There is no active-theme concept.** `grep -rn "themes.active\|activeTheme\|theme_active" src src-tauri/src` returns nothing. A custom theme's only effect anywhere is through a template's `style.themeRef`, which is what makes inlining lossless.

**A deviation from the design, stated:** §2.3 says custom themes are migrated "one template per (theme × kind) the operator actually used". With no active-theme and no theme×kind binding in the code, "used" can only mean "referenced by a template's `themeRef`" — and those are already preserved exactly by the inline step. So Task 12 inlines every reference, and converts each custom theme that nothing references into **one** real template named after it, rather than five. Nothing an operator made is discarded, and no template is created that the operator never had.

### Task 9: Freeze the theme definitions the migration will read (P1)

**Why this exists:** The migration runs in Rust; the builtin theme styles live in JS. A migration must inline the values **as they were**, so it embeds a frozen snapshot rather than importing a table that will keep changing. This is the `shelf_templates.json` precedent: one file, `include_str!`d by Rust and readable by the frontend test that checks the two agree.

**Files:**
- Create: `src-tauri/data/legacy_themes.json`
- Create: `src/lib/legacythemes.test.js`

**Interfaces:**
- Produces: `src-tauri/data/legacy_themes.json` — `{"themes":[{"id":-1,"name":"Modern Dark","style":{…}}, …]}`, the nine builtins with ids −1 … −9 (Modern Dark, Minimal, Light, Classic, Youth, Conference, Wedding, Livestream, High Visibility).

- [ ] **Step 1: Generate the file from the live table**

```bash
node --input-type=module -e '
import { BUILTIN_THEMES } from "./src/lib/themes.js";
import { writeFileSync } from "node:fs";
const themes = BUILTIN_THEMES.map(({ id, name, style }) => ({ id, name, style }));
writeFileSync("src-tauri/data/legacy_themes.json", JSON.stringify({ themes }, null, 2) + "\n");
console.log(themes.length, "themes frozen");
'
```

Expected: `9 themes frozen`.

- [ ] **Step 2: Write the test that pins the snapshot**

Create `src/lib/legacythemes.test.js`:

```js
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { BUILTIN_THEMES } from './themes.js';

// A MIGRATION INLINES THE VALUES AS THEY WERE.
//
// The themes are being deleted, and the one thing that must outlive them is
// what every template pinning one actually looked like. Rust does the inlining
// (it runs before any window is shown), and the builtin styles live in JS — so
// the values are frozen into a file both sides read, exactly as
// `shelf_templates.json` already is.
//
// This test exists for the window in which BOTH still exist. Once `themes.js`
// no longer exports `BUILTIN_THEMES` (Task 13), delete this file: the snapshot
// is then the only copy, which is the point of a snapshot.
describe('the frozen theme snapshot', () => {
  it('matches the table it was taken from, key for key', () => {
    const frozen = JSON.parse(readFileSync('src-tauri/data/legacy_themes.json', 'utf8')).themes;
    expect(frozen.length).toBe(BUILTIN_THEMES.length);
    for (const t of BUILTIN_THEMES) {
      const f = frozen.find((x) => x.id === t.id);
      expect(f, `theme ${t.id} (${t.name}) is missing from the snapshot`).toBeTruthy();
      expect(f.style).toEqual(t.style);
    }
  });
});
```

- [ ] **Step 3: Run it**

Run: `npx vitest run src/lib/legacythemes.test.js`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/data/legacy_themes.json src/lib/legacythemes.test.js
git commit -m "chore: freeze the theme definitions the migration will inline"
```

### Task 10: The migration inlines every themed template (P1)

**Why this exists:** Deleting `resolveThemed` without inlining first would silently change the look of every template a church themed — the one class of change a congregation sees and nobody in the building can explain.

**Files:**
- Modify: `src-tauri/src/db/templates.rs` — add `ensure_themes_are_inlined`
- Modify: `src-tauri/src/db/mod.rs:356-360` — call it in `ensure_tables`, after `ensure_preset_templates`
- Test: `src-tauri/src/db/templates.rs` `mod tests`

**Interfaces:**
- Produces: `pub(super) fn ensure_themes_are_inlined(conn: &Connection) -> rusqlite::Result<()>` — idempotent, retryable, no scratch table.
- Consumes: `LEGACY_THEMES_JSON` (`include_str!("../../data/legacy_themes.json")`), `app_settings['themes.custom']`.

- [ ] **Step 1: Write the failing tests**

Add to `src-tauri/src/db/templates.rs`'s `mod tests`:

```rust
#[test]
fn a_themed_template_keeps_exactly_the_look_it_had() {
    // `{ ...theme.style, ...template.style }` is what `applyTheme` computed at
    // render time, so inlining it is the effective look BY CONSTRUCTION — the
    // template's own keys still win, and only the keys it left unset are filled.
    // Anything else here is a church's wall changing appearance on an update,
    // with nothing in the building able to say why.
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate(&conn, true).unwrap();
    conn.execute(
        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
        (
            "Themed",
            r#"{"regions":["verse_text","reference"]}"#,
            // Pins Modern Dark (-1) and overrides ONE of its keys.
            r#"{"themeRef":-1,"verseColor":"#ff0000"}"#,
        ),
    )
    .unwrap();
    ensure_themes_are_inlined(&conn).unwrap();
    let style: String = conn
        .query_row("SELECT style_json FROM templates WHERE name = 'Themed'", [], |r| r.get(0))
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&style).unwrap();
    assert!(v.get("themeRef").is_none(), "the ref must be gone");
    assert_eq!(v["verseColor"], "#ff0000", "the template's own key still wins");
    assert_eq!(v["accent"], "#22d3ee", "the theme's unset keys are inlined");
}

#[test]
fn an_unknown_theme_ref_is_dropped_without_touching_the_style() {
    // A dangling ref already rendered as the template's own look (`resolveThemed`
    // degrades rather than blanking). The migration must agree with what the
    // wall was doing, not invent a look for it.
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate(&conn, true).unwrap();
    conn.execute(
        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
        ("Dangling", r#"{"regions":["verse_text"]}"#, r#"{"themeRef":123456,"verseColor":"#abc"}"#),
    )
    .unwrap();
    ensure_themes_are_inlined(&conn).unwrap();
    let style: String = conn
        .query_row("SELECT style_json FROM templates WHERE name = 'Dangling'", [], |r| r.get(0))
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&style).unwrap();
    assert!(v.get("themeRef").is_none());
    assert_eq!(v["verseColor"], "#abc");
    assert_eq!(v.as_object().unwrap().len(), 1, "no keys invented");
}

#[test]
fn a_custom_theme_nothing_referenced_becomes_a_template_rather_than_being_lost() {
    // A custom theme's ONLY effect anywhere is through a template's themeRef
    // (there is no active-theme concept). One that nothing references therefore
    // changed no screen — but it is still a look the operator built, and the
    // key it lives in is about to be deleted. It becomes one real template,
    // named after the theme.
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate(&conn, true).unwrap();
    set_setting(
        &conn,
        "themes.custom",
        r#"[{"id":5,"name":"Harvest","style":{"accent":"#e08b2a","verseColor":"#fff5e6"}}]"#,
    )
    .unwrap();
    ensure_themes_are_inlined(&conn).unwrap();
    let style: String = conn
        .query_row("SELECT style_json FROM templates WHERE name = 'Harvest'", [], |r| r.get(0))
        .unwrap();
    assert!(style.contains("#e08b2a"));
    assert!(
        get_setting(&conn, "themes.custom").unwrap().is_none(),
        "the key is dropped once its contents are preserved"
    );
}

#[test]
fn inlining_is_retryable_and_idempotent() {
    // Rule 25. Run it three times: the second and third must be no-ops, not
    // errors, and must not stack a second copy of the theme's keys or a second
    // template per custom theme. A migration that fails every boot after a
    // half-run is a church whose app will not start.
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate(&conn, true).unwrap();
    conn.execute(
        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
        ("Themed", r#"{"regions":["verse_text"]}"#, r#"{"themeRef":-1}"#),
    )
    .unwrap();
    ensure_themes_are_inlined(&conn).unwrap();
    let after_one: String = conn
        .query_row("SELECT style_json FROM templates WHERE name = 'Themed'", [], |r| r.get(0))
        .unwrap();
    ensure_themes_are_inlined(&conn).unwrap();
    ensure_themes_are_inlined(&conn).unwrap();
    let after_three: String = conn
        .query_row("SELECT style_json FROM templates WHERE name = 'Themed'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(after_one, after_three);
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM templates WHERE name = 'Themed'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd src-tauri && cargo test templates::tests::a_themed_template_keeps`
Expected: FAIL — `cannot find function ensure_themes_are_inlined`.

- [ ] **Step 3: Implement**

In `src-tauri/src/db/templates.rs`:

```rust
/// The theme definitions as they stood when themes were folded into templates
/// (DECISIONS §27 completed, wave 2). A FROZEN SNAPSHOT on purpose: a migration
/// inlines the values a template actually rendered with, so it must not read a
/// table that keeps changing. Pinned against the live table by
/// `src/lib/legacythemes.test.js` for as long as both exist.
// `get_setting` / `set_setting` / `content_template_id` live in `db/settings.rs`
// and are re-exported by `db/mod.rs` (`pub use settings::*`), but `templates.rs`
// imports them only inside `mod tests` today. Add the import at the top of the
// file: `use super::settings::{content_template_id, get_setting, set_setting};`
// `Value`, `Connection` and `OptionalExtension` are already imported (`:7-10`).

const LEGACY_THEMES_JSON: &str = include_str!("../../data/legacy_themes.json");

/// INLINE EVERY THEME A TEMPLATE POINTED AT, THEN DROP THE THEMES.
///
/// A theme was `{ ...theme.style, ...template.style }` computed at render time
/// (`applyTheme`), so inlining that same merge leaves the effective look
/// unchanged by construction. A dangling ref is dropped and nothing is invented,
/// because a dangling ref already rendered as the template's own look.
///
/// Retryable (rule 25): no scratch table exists to strand, each template is
/// rewritten in one statement inside one transaction, and a second run finds no
/// `themeRef` to act on. A failure rolls the transaction back rather than
/// leaving it open for the `PRAGMA foreign_keys = ON` that follows to no-op
/// inside it.
pub(super) fn ensure_themes_are_inlined(conn: &Connection) -> rusqlite::Result<()> {
    #[derive(serde::Deserialize)]
    struct Frozen {
        themes: Vec<FrozenTheme>,
    }
    #[derive(serde::Deserialize)]
    struct FrozenTheme {
        id: i64,
        name: String,
        style: serde_json::Map<String, Value>,
    }

    let mut known: Vec<(i64, String, serde_json::Map<String, Value>)> =
        match serde_json::from_str::<Frozen>(LEGACY_THEMES_JSON) {
            Ok(f) => f.themes.into_iter().map(|t| (t.id, t.name, t.style)).collect(),
            // A malformed snapshot must not stop the app booting. Every themeRef
            // is then dropped without inlining, which is exactly what a dangling
            // ref already did on screen.
            Err(e) => {
                eprintln!("legacy_themes.json could not be read ({e}) — refs will be dropped");
                Vec::new()
            }
        };

    // The operator's own themes, from the key that is about to be deleted.
    let custom_raw = get_setting(conn, "themes.custom")?;
    if let Some(raw) = custom_raw.as_deref() {
        if let Ok(list) = serde_json::from_str::<Vec<Value>>(raw) {
            for t in list {
                let id = t.get("id").and_then(Value::as_i64);
                let name = t.get("name").and_then(Value::as_str).unwrap_or("Theme").to_string();
                let style = t
                    .get("style")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                if let Some(id) = id {
                    known.push((id, name, style));
                }
            }
        }
    }

    conn.execute_batch("BEGIN")?;
    let inlined = (|| -> rusqlite::Result<Vec<i64>> {
        let mut used: Vec<i64> = Vec::new();
        let rows: Vec<(i64, String)> = {
            let mut stmt = conn.prepare("SELECT id, style_json FROM templates")?;
            let it = stmt.query_map([], |r| Ok((r.get(0)?, r.get::<_, String>(1)?)))?;
            it.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (id, style_json) in rows {
            let Ok(Value::Object(mut style)) = serde_json::from_str::<Value>(&style_json) else {
                continue;
            };
            let Some(theme_ref) = style.get("themeRef").and_then(Value::as_i64) else {
                continue;
            };
            style.remove("themeRef");
            if let Some((_, _, tstyle)) = known.iter().find(|(tid, _, _)| *tid == theme_ref) {
                used.push(theme_ref);
                // The TEMPLATE's own keys win — the same precedence `applyTheme`
                // applied at render time. Only keys it left unset are filled.
                for (k, v) in tstyle {
                    style.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            conn.execute(
                "UPDATE templates SET style_json = ?1 WHERE id = ?2",
                (serde_json::to_string(&Value::Object(style)).unwrap_or_default(), id),
            )?;
        }
        Ok(used)
    })();
    let inlined = match inlined {
        Ok(v) => v,
        Err(e) => {
            // ROLL BACK. An open transaction would swallow the pragma that runs
            // after this (rule 25 — the exact failure `ensure_manual_detection_status`
            // once had).
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    };
    conn.execute_batch("COMMIT")?;

    // A CUSTOM THEME NOTHING REFERENCED IS STILL SOMEBODY'S WORK. It changed no
    // screen (there is no active-theme concept — a theme reached a wall only
    // through a template's themeRef), but the key it lives in is being deleted,
    // so it becomes one real template named after it. Added BY NAME and only
    // when absent, like every other seed, so a second run creates nothing.
    if let Some(raw) = custom_raw {
        if let Ok(list) = serde_json::from_str::<Vec<Value>>(&raw) {
            for t in list {
                let id = t.get("id").and_then(Value::as_i64).unwrap_or(0);
                if inlined.contains(&id) {
                    continue;
                }
                let name = t.get("name").and_then(Value::as_str).unwrap_or("Theme");
                let style = t.get("style").cloned().unwrap_or(Value::Object(Default::default()));
                let present: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM templates WHERE name = ?1",
                    [name],
                    |r| r.get(0),
                )?;
                if present == 0 {
                    eprintln!("themes: keeping {name:?} as a template (nothing referenced it)");
                    conn.execute(
                        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
                        (
                            name,
                            r#"{"regions":["verse_text","reference"],"align":"center","lowerThird":false,"refFirst":false}"#,
                            style.to_string(),
                        ),
                    )?;
                }
            }
        }
        conn.execute("DELETE FROM app_settings WHERE key = 'themes.custom'", [])?;
    }
    Ok(())
}
```

- [ ] **Step 4: Call it from the migration ladder**

In `src-tauri/src/db/mod.rs`, inside `ensure_tables`, immediately after `ensure_lower_third_band_is_not_a_law_colour(conn)?;`:

```rust
    ensure_themes_are_inlined(conn)?; // themes folded into templates — see templates.rs
```

and add it to the `use` list at `:46`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test templates::`
Expected: PASS, including the three existing preset tests.

- [ ] **Step 6: Verify the defect reproduces when reverted**

Comment out the `for (k, v) in tstyle` inlining loop, re-run `a_themed_template_keeps_exactly_the_look_it_had`, confirm it FAILS on `accent`, restore.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/db/templates.rs src-tauri/src/db/mod.rs
git commit -m "feat: inline every themed template before the themes are removed"
```

### Task 11: Delete the parallel system (P1)

**Why this exists:** With the data migrated, the theme surfaces, stores, hub frame and resolvers are a second way to say what a template already says. A theme desk that can only set a subset of the template's own keys, nine of which move nothing on a layer template, is a layer that has finished being useful.

**Files:**
- Delete: `src/lib/views/themes/ThemeGallery.svelte` (344 lines), `src/lib/views/themes/ThemeEditor.svelte` (403 lines), `src/lib/themes.test.js`, `src/lib/themerender.test.js`, `src/lib/legacythemes.test.js`
- Modify: `src/lib/views/Templates.svelte` — the two-way desk switch becomes the gallery
- Modify: `src/lib/themes.js` → rename to `src/lib/styletokens.js`, keeping `THEME_FONTS`, `fontLabel`, `THEME_TOKENS`, `isThemeToken`, `applyTheme` (now `resolveTokens`), `DEFAULT_THEME`'s role removed
- Modify: `src/lib/stores/capture.js:1882-2040` — delete `customThemes`, `loadThemes`, `saveTheme`, `deleteTheme`, `exportTheme`, `importThemeFromFile`
- Modify: `src/Output.svelte` — drop the `themes` frame branch and `resolveThemed`
- Modify: `src/lib/TemplateRender.svelte` — drop the `style.themeRef` resolution path
- Modify: `src/lib/views/templates/TemplateEditor.svelte:70-95` — drop the theme picker
- Modify: `src-tauri/src/channels.rs` — delete `themes`, `cache_themes`, `set_themes`, the `hello` themes reply, the `FRAME_VERDICTS` row
- Modify: `src-tauri/src/main.rs` — drop the `set_themes` / `cache_themes` call sites
- **Do NOT modify:** `src/lib/session.js:227` — `MOVED_TABS.themes = 'templates'` must stay. A stored session naming the old desk still has to land somewhere valid.

**Interfaces:**
- Produces: `src/lib/styletokens.js` exporting `THEME_FONTS`, `fontLabel`, `THEME_TOKENS`, `isThemeToken`, `resolveTokens(template)` — the token resolver with its theme argument removed, resolving `theme:accent` and friends against the template's **own** style. This already works: `applyTheme`'s `effective` is `{ ...themeStyle, ...template.style }`, and with no theme that is the template's style.
- Consumes: nothing new.

- [ ] **Step 1: Write the failing test**

Create `src/lib/thememerge.test.js`:

```js
import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { resolveTokens } from './styletokens.js';
import { MOVED_TABS } from './session.js';

// ONE MODEL, NOT TWO.
//
// A theme had no field a template does not have: THEME_STYLE_KEYS was a subset
// of the same flat `style` keys, there was no themes table and no Rust struct.
// What it did have was a second gallery, a second editor, a second store, a
// second export format and a hub frame of its own — five surfaces that could
// disagree with the template model about what a screen wears.
//
// A LAYER COLOUR BOUND TO A TOKEN STILL RESOLVES. That is the half of themes.js
// that was never about themes: `theme:accent` on a layer resolved against the
// merged style, and with no theme in the merge that is the template's own style.
// Deleting the resolver with the desk would have silently blanked every layer in
// the stage, confidence and countdown starters.
describe('themes are folded into templates', () => {
  it('leaves no theme desk behind', () => {
    expect(existsSync('src/lib/views/themes/ThemeGallery.svelte')).toBe(false);
    expect(existsSync('src/lib/views/themes/ThemeEditor.svelte')).toBe(false);
  });

  it('still sends a stored session naming the old desk somewhere valid', () => {
    // The desk is gone; a laptop that was left on it is not. Without this the
    // operator lands nowhere and concludes the whole workspace was deleted.
    expect(MOVED_TABS.themes).toBe('templates');
  });

  it('resolves a layer token against the template own style', () => {
    const t = {
      style: { accent: '#ffb000', verseColor: '#eee' },
      layout: { layers: [{ id: 'a', type: 'text', color: 'theme:accent', fill: 'theme:background' }] },
    };
    const out = resolveTokens(t);
    expect(out.layout.layers[0].color).toBe('#ffb000');
    expect(out.layout.layers[0].fill).toBe('transparent');
  });

  it('leaves no themeRef resolution in the render path', () => {
    const render = readFileSync('src/lib/TemplateRender.svelte', 'utf8');
    const output = readFileSync('src/Output.svelte', 'utf8');
    expect(render).not.toMatch(/themeRef/);
    expect(output).not.toMatch(/themeRef|resolveThemed/);
  });

  it('leaves no themes frame on the hub', () => {
    const ch = readFileSync('src-tauri/src/channels.rs', 'utf8');
    expect(ch).not.toMatch(/fn set_themes|"kind":"themes"/);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/lib/thememerge.test.js`
Expected: FAIL — the files still exist and `styletokens.js` does not.

- [ ] **Step 3: Rename and narrow the module**

```bash
git mv src/lib/themes.js src/lib/styletokens.js
```

In `src/lib/styletokens.js`: delete `THEME_STYLE_KEYS`, `BUILTIN_THEMES`, `DEFAULT_THEME`, `themeById`, `templateThemeRef`, `resolveThemed`, `applyThemeToTemplate`, `parseThemes`, `THEME_FILE_MARKER`, `serializeTheme`, `parseImportedTheme`, `THEME_PREVIEW_TEMPLATE`, `LAYER_THEME_KEYS`, `THEME_SAMPLE_CONTENT`. Rename `applyTheme(template, theme)` to `resolveTokens(template)` with the theme merge removed, keeping `TOKEN_RESOLVERS`, `isThemeToken`, `THEME_TOKENS`, `THEME_FONTS` and `fontLabel`. Head the file:

```js
/**
 * THE STYLE LAYER THAT OUTLIVED THEMES.
 *
 * Themes were folded into templates in wave 2 (DECISIONS §27 completed): a
 * theme's every field was already a template `style` key, so a layer beneath
 * templates whose only content is a subset of the template's own keys had
 * finished being useful.
 *
 * What is here was never about themes. A layer's colour, fill or font may be a
 * TOKEN (`theme:accent`) rather than a literal, so that a stage or confidence
 * starter follows whatever template it is dropped into. The token name keeps its
 * spelling: it is written into every saved layer in every install, and renaming
 * it would need a migration to buy a nicer word.
 */
```

- [ ] **Step 4: Update the importers**

`src/Output.svelte`, `src/lib/TemplateRender.svelte`, `src/lib/stores/capture.js`, `src/lib/views/Templates.svelte`, `src/lib/views/templates/TemplateEditor.svelte`, `src/lib/legibility.test.js`, `src/lib/templatedoors.test.js`, `src/lib/templatemigrate.test.js`, `src/lib/views/workspacegrammar.test.js`. In `Output.svelte` delete the `themes` frame branch and `$: themedTemplate = resolveThemed(...)`, rendering `activeTemplate` through `resolveTokens` instead.

- [ ] **Step 5: Delete the desk and the two galleries**

```bash
git rm src/lib/views/themes/ThemeGallery.svelte src/lib/views/themes/ThemeEditor.svelte src/lib/themes.test.js src/lib/themerender.test.js src/lib/legacythemes.test.js
```

In `src/lib/views/Templates.svelte`, remove the two-way switch and render the template gallery directly. Keep the workspace label and the route key exactly as they are.

- [ ] **Step 6: Delete the Rust half**

In `src-tauri/src/channels.rs`: remove the `themes` field, `cache_themes`, `set_themes`, the `hello` reply block and the `("themes", false)` row in `FRAME_VERDICTS` — that table's own test fails if a named kind is no longer emitted, which is the point of it. In `src-tauri/src/main.rs`: remove the `cache_themes` warm call and any `set_themes` call site.

- [ ] **Step 7: Run everything**

Run: `npm run build && npx vitest run && cd src-tauri && cargo test`
Expected: PASS on all three summary lines. `ipc.test.js` fails loudly if a deleted command still has a frontend caller — that is the instrument for this task.

Run: `node scripts/qa-inventory.mjs`
Expected: no orphaned components. A gallery nothing imports would be reported here, which is how `PreviewProgram.svelte` was found.

- [ ] **Step 8: Update the documents**

`docs/DECISIONS.md` gets a new numbered decision recording the merge and citing §27 and §69; `CLAUDE.md`'s "Frontend shape" paragraph loses the Themes-desk sentence; `docs/REBRAND.md` §2 likewise. Do not restate the same fact in a fifth place.

- [ ] **Step 9: Commit and open the PR**

```bash
git add -A
git commit -m "refactor: fold themes into templates and delete the parallel system"
# --fill takes the body from this track's commit messages, which are already normal English prose.
gh pr create --base feat/wave7-timers-templates-stage --title "Wave 2 track E — themes are folded into templates" --fill
```

---

# Track D — the seed becomes twenty-five

**Spec:** §2.4. **Runs after Track E.** Lands as PR 5.

Five style families × five content kinds: Scripture, Songs/Lyrics, Media, Announcements, Timer/Countdown. The pattern already exists — `theme_templates()` (`src-tauri/src/db/templates.rs:371`) seeds twelve as three families × four kinds, and `every_theme_is_a_complete_coordinated_family` (`:860`) already enforces completeness.

**The five families are Classic, Aurora, Ember, Lower Third and High Visibility.** A lower third is not a sixth content kind — it is a keyed variant, and the transparency law in `resolveOutputTemplate` (`src/lib/layers.js:262`) depends on keyed templates existing at all. Making it a family means every content kind gets a keyed option, which is strictly more capability than the two keyed presets shipping today. High Visibility is likewise a family rather than an extra: CLAUDE.md names it as an accessibility feature beside `legibility.js` and the distance preview, so it has to cover every kind to be worth having. **Nocturne is dropped** — its style keys are close enough to Ember and Classic that keeping it would cost a slot Lower Third and High Visibility each use better.

**SuperSource and Stage · Large type do not take seed slots.** The SuperSource starter is `src/lib/layers.js:729` (`supersource`) and the stage starter is `stage`; `composite.test.js`'s coverage of the `region` layer is untouched. A starter is a way to make a template; a seed row is a template a church did not ask for. **Note the design's imprecision:** `Stage · Large type` is a shelf row, not a starter. Task 12 keeps the `stage` starter as the way to build one and retires the shelf row with the rest.

Current fresh-install total is 38 rows: 4 Rust built-ins + 1 lyrics template + 13 presets + 12 theme-family rows + 8 shelf rows.

### Task 12: Five families, five kinds, and `layout.shows` written down (P1)

**Files:**
- Modify: `src-tauri/src/db/templates.rs:271` (`preset_templates` — emptied), `:371` (`theme_templates` — twenty-five entries), `:819` and `:860` (the two tests)
- Modify: `src-tauri/data/shelf_templates.json` — keep only what no family claims
- Test: `src-tauri/src/db/templates.rs` `mod tests`, `src/lib/shelf.test.js` (reads the same bytes)

**Interfaces:**
- Produces: `theme_templates()` returning twenty-five `(name, layout, style)` triples named `"{Family} · {Kind}"` where Kind ∈ {`Scripture`, `Lyrics`, `Media`, `Announcement`, `Timer`}.
- Consumes: `preset_template_count()` (`:526`) — the single source of the number; it must agree without being told.

- [ ] **Step 1: Write the failing test**

Replace `every_theme_is_a_complete_coordinated_family` in `src-tauri/src/db/templates.rs`:

```rust
    #[test]
    fn every_family_is_complete_across_every_content_kind() {
        // A FAMILY IS A SET. An operator picks a look and fires five kinds of
        // content at it over a morning; a family missing its Timer means the
        // pre-service countdown falls back to a mismatched default in front of
        // a filling room, which is worse than the family not existing.
        //
        // Five kinds, because CONTENT_KINDS has five (src/lib/layers.js:228) and
        // the seed had four — Media and Timer were the two nobody could pick a
        // coordinated look for.
        use std::collections::HashSet;
        let mut families: HashSet<&str> = HashSet::new();
        for (name, _, _) in theme_templates() {
            match name.split_once(" · ") {
                Some((f, _)) => {
                    families.insert(f);
                }
                None => panic!("seed template {name:?} is not named 'Family · Kind'"),
            }
        }
        let mut want: Vec<&str> = vec!["Classic", "Aurora", "Ember", "Lower Third", "High Visibility"];
        want.sort();
        let mut got: Vec<&str> = families.into_iter().collect();
        got.sort();
        assert_eq!(got, want, "the five families are fixed by DECISIONS, not by taste");
        for family in &want {
            for kind in ["Scripture", "Lyrics", "Media", "Announcement", "Timer"] {
                let name = format!("{family} · {kind}");
                assert!(
                    theme_templates().iter().any(|(n, _, _)| *n == name),
                    "family {family:?} is missing its {kind} template"
                );
            }
        }
        assert_eq!(theme_templates().len(), 25);
    }

    #[test]
    fn every_seeded_template_says_which_kinds_it_renders() {
        // `templateShows` returns true for EVERY kind when `layout.shows` is
        // absent (src/lib/layers.js:246), so a seeded template starts with all
        // five ticked in the editor and the operator's first click looks like it
        // wiped four. Writing the list down is what stops the render lying about
        // what the click did — and a Lower Third family that quietly claimed to
        // render full-screen media was never true either.
        for (name, layout, _) in theme_templates() {
            let v: serde_json::Value = serde_json::from_str(layout)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let shows = v["shows"].as_array().unwrap_or_else(|| panic!("{name} has no shows list"));
            assert!(!shows.is_empty(), "{name}: an empty list shows nothing at all");
            for k in shows {
                let k = k.as_str().unwrap_or("");
                assert!(
                    ["scripture", "song", "media", "announce", "countdown"].contains(&k),
                    "{name}: {k:?} is not a content kind"
                );
            }
        }
    }

    #[test]
    fn the_keyed_family_is_keyed_in_every_kind() {
        // The transparency law (src/lib/layers.js:262) needs keyed templates to
        // exist at all, and this is now the only family that supplies them.
        for (name, layout, _) in theme_templates() {
            if let Some(rest) = name.strip_prefix("Lower Third · ") {
                let v: serde_json::Value = serde_json::from_str(layout).unwrap();
                assert_eq!(
                    v["lowerThird"], true,
                    "Lower Third · {rest} is not keyed, so the family cannot caption a camera"
                );
            }
        }
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cd src-tauri && cargo test templates::tests::every_family_is_complete`
Expected: FAIL — `got` is `["Aurora", "Ember", "Nocturne"]`.

- [ ] **Step 3: Write the twenty-five**

In `theme_templates()`, define five layout constants and one style per (family, kind). Each layout carries its own `shows`:

```rust
    // FULL-SCREEN SCRIPTURE — reference and verse, centred. The common case.
    const SCRIPTURE: &str = r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":false,"refFirst":false,"shows":["scripture"]}"##;
    // LYRIC — the words alone. A room does not sing the title.
    const LYRIC: &str = r##"{"regions":["verse_text"],"align":"center","lowerThird":false,"refFirst":false,"shows":["song"]}"##;
    // MEDIA — a picture or video fills the frame; the caption rides beneath it.
    const MEDIA: &str = r##"{"regions":["reference"],"align":"center","lowerThird":false,"refFirst":true,"shows":["media"]}"##;
    // ANNOUNCEMENT — a title over a scrolling body line (the ticker).
    const ANNOUNCE: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":true,"shows":["announce"]}"##;
    // TIMER — the countdown, with its label above it.
    const TIMER: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":true,"shows":["countdown"]}"##;
    // The keyed family replaces each of the five with its banded twin.
    const BAND_SCRIPTURE: &str = r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":true,"refFirst":false,"shows":["scripture"]}"##;
```

```rust
    const BAND_LYRIC: &str = r##"{"regions":["verse_text"],"align":"center","lowerThird":true,"refFirst":false,"shows":["song"]}"##;
    const BAND_MEDIA: &str = r##"{"regions":["reference"],"align":"center","lowerThird":true,"refFirst":true,"shows":["media"]}"##;
    const BAND_ANNOUNCE: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":true,"refFirst":true,"shows":["announce"]}"##;
    const BAND_TIMER: &str = r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":true,"refFirst":true,"shows":["countdown"]}"##;
```

Ten layout constants, twenty-five entries: `Classic`, `Aurora`, `Ember` and `High Visibility` each use the five opaque layouts; `Lower Third` uses the five banded ones.

**Where each family's palette comes from — four of the five already exist in the tree, so this is carrying work forward, not new taste:**

| Family | Source of its style keys |
|---|---|
| Classic | `builtin_templates()`'s `Classic Serif` entry (`src-tauri/src/db/templates.rs:130`) |
| Aurora | the four existing `Aurora · …` entries (`:371`), verbatim |
| Ember | the four existing `Ember · …` entries (`:371`), verbatim |
| Lower Third | `builtin_templates()`'s `Lower Third`, with the band colour `#101319` that `ensure_lower_third_band_is_not_a_law_colour` already enforces — never a law colour (rule 18) |
| High Visibility | the shelf's `High Visibility` entry in `src-tauri/data/shelf_templates.json`, flattened from its layer stack to the region style keys |

The five new members each family needs are its existing palette applied to a kind it did not cover: `Media` (caption type, no verse box), `Timer` (the countdown label and digits) for all five families, plus `Scripture` / `Lyrics` / `Announcement` for Classic, Lower Third and High Visibility. Change the layout and the sizes; **do not invent a new colour for any of them** — a family is a set precisely because its colours do not vary across the kinds.

Each style must keep a light verse colour on a dark field (except the keyed band, where the text is dark on a solid accent) — `every_preset_is_valid_json_with_a_readable_verse_colour` (`:900`) enforces it and must keep passing untouched.

- [ ] **Step 4: Empty `preset_templates` and prune the shelf**

`preset_templates()` returns `&[]` with its doc comment rewritten to say the thirteen standalone presets were retired into the five families, naming them so the migration in Task 13 can be read against this list. In `src-tauri/data/shelf_templates.json`, keep only entries no family claims and that are genuinely a different SHAPE rather than a different look. Retire `High Visibility`, `Notice Board`, `Media Frame`, `Lower Third · Lyric`, `Lower Third · Scripture`, `Stage · Large type`, `SuperSource · Word right` and `SuperSource · Word left` — every one is now either a family member or a starter (`supersource`, `stage`).

Update `there_are_presets_across_every_screen_type` (`:819`) to assert the new shape rather than `12..=15` standalone presets: assert `preset_templates().is_empty()` and that `region_presets()` still covers `scripture`, `song` and `lower-third`.

- [ ] **Step 5: Run the Rust suite**

Run: `npm run build && cd src-tauri && cargo test`
Expected: PASS, including `the_bare_fixture_is_a_first_launch_and_nothing_more` in `qa.rs` — the tripwire that catches a seed drifting from what a fresh install contains. If it fails, the fixture's expectation is what to read first, not the seed.

- [ ] **Step 6: Run the frontend suite**

Run: `npx vitest run`
Expected: PASS. `src/lib/shelf.test.js` reads the same shelf bytes and renders every entry through the real `TemplateRender`; a pruned shelf must leave it green.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/db/templates.rs src-tauri/data/shelf_templates.json
git commit -m "feat: five coordinated families across all five content kinds"
```

### Task 13: Retiring a preset nobody chose, and only that (P1)

**Why this exists:** Seeds insert by name and only when absent, so an existing install keeps all thirty-three of its old rows and would also gain the twenty-five — fifty-eight templates in the gallery. **Operator decision: retire the unreferenced ones.** The rule has to be exact, because deleting a template a church styled by hand is the one mistake here that cannot be undone from inside Relay.

A row is retired only when **all** of these hold:
1. Its name is one of the retired seed names (the thirteen presets and the eight shelf rows, listed verbatim in the migration).
2. Its `region_config_json` and `style_json` still match the bytes that seed shipped — so anything the operator edited is left alone, without needing an `updated_at` column the schema does not have.
3. Nothing points at it: no `output_channels.template_id`, no `plan_items.template_id`, no `app_settings` key `tpl_scripture` / `tpl_song` / `tpl_media` / `tpl_announce` / `tpl_countdown`, and not `default_template_id`.

**Files:**
- Modify: `src-tauri/src/db/templates.rs` — add `ensure_retired_presets_are_gone`
- Modify: `src-tauri/src/db/mod.rs` — call it after `ensure_preset_templates`
- Create: `src-tauri/data/retired_presets.json` — the frozen name/layout/style triples being retired
- Test: `src-tauri/src/db/templates.rs` `mod tests`

**Interfaces:**
- Produces: `pub(super) fn ensure_retired_presets_are_gone(conn: &Connection) -> rusqlite::Result<()>`.

- [ ] **Step 1: Freeze what is being retired**

Do this **before** emptying the lists in Task 12, on a tree that still has them. Add a one-off `#[ignore]`d dumper to `src-tauri/src/db/templates.rs`'s `mod tests`:

```rust
    /// ONE-OFF. Prints the exact triples `ensure_preset_templates` would have
    /// inserted, so the retirement migration can match on the bytes the seed
    /// actually wrote. Run once, redirect into `data/retired_presets.json`,
    /// then delete this — the file is the record, not this function.
    #[test]
    #[ignore]
    fn dump_retiring_presets() {
        let names: Vec<&str> = vec![
            "Midnight Blue", "Royal Amethyst", "Deep Teal", "Crimson Grace", "Emerald Word",
            "Indigo Night", "Slate Minimal", "Pure Contrast", "Lyric Bold", "Lyric Glow",
            "Lower Third Night", "Stage Confidence", "Lobby Sunrise",
            "High Visibility", "Notice Board", "Media Frame", "Lower Third · Lyric",
            "Lower Third · Scripture", "SuperSource · Word right", "SuperSource · Word left",
            "Stage · Large type",
        ];
        let rows: Vec<serde_json::Value> = all_presets()
            .filter(|(n, _, _)| names.contains(n))
            .map(|(n, l, s)| serde_json::json!({ "name": n, "layout": l, "style": s }))
            .collect();
        assert_eq!(rows.len(), names.len(), "a name in the list is not in the seed");
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "templates": rows })).unwrap()
        );
    }
```

```bash
cd src-tauri && cargo test templates::tests::dump_retiring_presets -- --ignored --nocapture
```

Copy the printed JSON into `src-tauri/data/retired_presets.json`. **`layout` and `style` are strings there, not nested objects** — the migration matches on bytes, and a parse-and-re-serialise round trip sorts the keys (`serde_json`'s map is a BTreeMap), producing a string that equals nothing in the database. Delete the dumper once the file exists.

- [ ] **Step 2: Write the failing tests**

```rust
#[test]
fn an_untouched_unreferenced_preset_is_retired() {
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate(&conn, true).unwrap();
    // A row exactly as an older version seeded it.
    insert_retired_fixture(&conn, "Midnight Blue");
    ensure_retired_presets_are_gone(&conn).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM templates WHERE name = 'Midnight Blue'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
}

#[test]
fn a_preset_the_operator_edited_is_never_retired() {
    // No `updated_at` column exists, so "edited" is decided by comparing the
    // bytes with what the seed shipped. A single changed colour makes the row
    // somebody's work, and somebody's work is not a leftover.
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate(&conn, true).unwrap();
    insert_retired_fixture(&conn, "Midnight Blue");
    conn.execute(
        "UPDATE templates SET style_json = replace(style_json, '#f0b74a', '#00ff99') WHERE name = 'Midnight Blue'",
        [],
    )
    .unwrap();
    ensure_retired_presets_are_gone(&conn).unwrap();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM templates WHERE name = 'Midnight Blue'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1, "an edited preset is the operator's, not a leftover");
}

#[test]
fn a_referenced_preset_is_never_retired() {
    // Four doors point at a template: a channel, a plan cue, a content look and
    // the configured default. Checking three of them is this repository's
    // recurring bug — a guarantee kept on one surface and skipped on its twin —
    // so each is asserted separately rather than in one case that could pass on
    // the first door alone.
    for door in ["channel", "cue", "look", "default"] {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        insert_retired_fixture(&conn, "Midnight Blue");
        let id: i64 = conn
            .query_row("SELECT id FROM templates WHERE name = 'Midnight Blue'", [], |r| r.get(0))
            .unwrap();
        match door {
            "channel" => {
                conn.execute(
                    "INSERT INTO output_channels (name, render_target, template_id) VALUES ('Main','native_window',?1)",
                    [id],
                )
                .unwrap();
            }
            "cue" => {
                conn.execute("INSERT INTO service_plans (title) VALUES ('Sunday')", []).unwrap();
                conn.execute(
                    "INSERT INTO plan_items (plan_id, position, cue_type, label, template_id) VALUES (1,0,'scripture','Reading',?1)",
                    [id],
                )
                .unwrap();
            }
            "look" => set_content_template(&conn, "scripture", Some(id)).unwrap(),
            _ => set_setting(&conn, "default_template_id", &id.to_string()).unwrap(),
        }
        ensure_retired_presets_are_gone(&conn).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM templates WHERE id = ?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "a preset reachable through the {door} was retired");
    }
}

#[test]
fn retiring_is_retryable_and_a_fresh_install_is_unaffected() {
    // Rule 25, and the fresh-install case: a first launch has never seeded the
    // retired names, so this must find nothing and change nothing, three times
    // over.
    let conn = Connection::open_in_memory().unwrap();
    crate::db::migrate(&conn, true).unwrap();
    let before: i64 = conn.query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0)).unwrap();
    ensure_retired_presets_are_gone(&conn).unwrap();
    ensure_retired_presets_are_gone(&conn).unwrap();
    ensure_retired_presets_are_gone(&conn).unwrap();
    let after: i64 = conn.query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0)).unwrap();
    assert_eq!(before, after);
}
```

with the fixture helper in the same module:

```rust
    /// Insert a retired preset exactly as an older version seeded it, read from
    /// the frozen record so the test cannot drift from what the migration matches.
    fn insert_retired_fixture(conn: &Connection, name: &str) {
        let (n, l, s) = retired_presets()
            .into_iter()
            .find(|(n, _, _)| n == name)
            .unwrap_or_else(|| panic!("{name} is not in retired_presets.json"));
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (n, l, s),
        )
        .unwrap();
    }
```

- [ ] **Step 3: Run them to verify they fail**

Run: `cd src-tauri && cargo test templates::tests::an_untouched_unreferenced`
Expected: FAIL — `cannot find function ensure_retired_presets_are_gone`.

- [ ] **Step 4: Implement**

```rust
const RETIRED_PRESETS_JSON: &str = include_str!("../../data/retired_presets.json");

/// The presets retired when the seed became five families × five kinds, frozen
/// as the exact bytes they shipped with. Matching on BYTES is what separates a
/// leftover from somebody's work: the schema has no `updated_at`, and inventing
/// one to answer this question would be a column nothing else ever reads.
fn retired_presets() -> Vec<(String, String, String)> {
    #[derive(serde::Deserialize)]
    struct Retired {
        templates: Vec<RetiredEntry>,
    }
    #[derive(serde::Deserialize)]
    struct RetiredEntry {
        name: String,
        // RAW STRINGS, NOT PARSED JSON. The match below is on BYTES, and
        // `serde_json`'s map is a BTreeMap — parsing and re-serialising sorts
        // the keys, so a round trip would produce a string that never equals
        // what the seed actually wrote and the migration would silently retire
        // nothing. The same trap the `starts_with` frame matcher fell into
        // (rule 43): a check that looks exhaustive and matches nothing.
        layout: String,
        style: String,
    }
    // An unreadable record yields an EMPTY list, so the migration retires
    // nothing. That is the right way for this one to fail: a church keeps a few
    // templates it does not want, rather than losing ones it does.
    match serde_json::from_str::<Retired>(RETIRED_PRESETS_JSON) {
        Ok(r) => r
            .templates
            .into_iter()
            .map(|e| (e.name, e.layout, e.style))
            .collect(),
        Err(e) => {
            eprintln!("retired_presets.json could not be read ({e}) — retiring nothing");
            Vec::new()
        }
    }
}

/// REMOVE A PRESET NOBODY CHOSE — and nothing else.
///
/// Seeds insert by name and only when absent, so an install that predates the
/// five families keeps its thirteen standalone presets and eight shelf rows and
/// would also receive the twenty-five: fifty-eight templates in a gallery an
/// operator scrolls on a Sunday morning.
///
/// Four doors can point at a template — a channel, a plan cue, a content look,
/// and the configured default — and this checks all four. Three of four is the
/// bug this repository has had four times.
///
/// Retryable (rule 25): a DELETE that matches nothing is a no-op, there is no
/// scratch table, and a fresh install has none of these names at all.
pub(super) fn ensure_retired_presets_are_gone(conn: &Connection) -> rusqlite::Result<()> {
    let looks: Vec<i64> = ["scripture", "song", "media", "announce", "countdown"]
        .iter()
        .filter_map(|k| content_template_id(conn, k).ok().flatten())
        .collect();
    let default_id = get_setting(conn, "default_template_id")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok());
    for (name, layout, style) in retired_presets() {
        // The row exactly as the seed wrote it, or nothing. A name that matches
        // with different bytes is a template somebody edited.
        let id: Option<i64> = conn
            .query_row(
                "SELECT id FROM templates
                  WHERE name = ?1 AND region_config_json = ?2 AND style_json = ?3",
                (&name, &layout, &style),
                |r| r.get(0),
            )
            .optional()?;
        let Some(id) = id else { continue };
        // THE TWO DOORS THAT ARE SETTINGS ROWS, NOT FOREIGN KEYS. A content look
        // and the configured default point at a template through `app_settings`,
        // so no `NOT IN (SELECT …)` can reach them and they are checked here.
        if looks.contains(&id) || default_id == Some(id) {
            continue;
        }
        conn.execute(
            "DELETE FROM templates
              WHERE id = ?1
                AND id NOT IN (SELECT template_id FROM output_channels WHERE template_id IS NOT NULL)
                AND id NOT IN (SELECT template_id FROM plan_items WHERE template_id IS NOT NULL)",
            [id],
        )?;
    }
    Ok(())
}

- [ ] **Step 5: Call it from the ladder**

In `src-tauri/src/db/mod.rs`, immediately after `ensure_preset_templates(conn)?;`:

```rust
    ensure_retired_presets_are_gone(conn)?; // the seed became five families — see templates.rs
```

Order matters: the new families are inserted first, so an install is never momentarily left with neither.

- [ ] **Step 6: Run to verify they pass, then verify the bug**

Run: `cd src-tauri && cargo test templates::`
Expected: PASS. Then delete the `plan_items` clause from the statement, re-run `a_referenced_preset_is_never_retired`, confirm it FAILS on the `cue` door, restore.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/db/templates.rs src-tauri/src/db/mod.rs src-tauri/data/retired_presets.json
git commit -m "feat: retire the presets nobody chose, and only those"
```

### Task 14: The counts and the documents agree (P2)

**Why this exists:** `docs/qa/QA_HARNESS.md` §0 is the one register that carries counts, and four documents have disagreed about this repository's numbers before. The seed changed size; the register has to say so, and nothing else may restate it.

**Files:**
- Modify: `docs/qa/QA_HARNESS.md` §0
- Modify: `docs/DECISIONS.md` — the seed decision, beside Track E's merge decision
- Modify: `CLAUDE.md` — only where a statement is now false

- [ ] **Step 1: Read the real numbers off the runners**

```bash
npm run build && npx vitest run 2>&1 | tail -5
cd src-tauri && cargo test 2>&1 | tail -5
```

Record what the summary lines say. Never a grep.

- [ ] **Step 2: Update §0 and the decision records**

Write the new seed total beside the command that reproduces it. Add one decision recording the five families, the five kinds, the dropping of Nocturne, and the retirement rule — with the reason, not only the outcome.

- [ ] **Step 3: Verify the cross-references resolve**

Run: `npx vitest run src/lib/crossrefs.test.js src/lib/relaygap.test.js`
Expected: PASS. A `DECISIONS §N` that resolves to nothing is worse than an uncited claim.

- [ ] **Step 4: Commit and open the PR**

```bash
git add docs CLAUDE.md
git commit -m "docs: record the seed the five families ship"
# --fill takes the body from this track's commit messages, which are already normal English prose.
gh pr create --base feat/wave7-timers-templates-stage --title "Wave 2 track D — the seed becomes twenty-five" --fill
```

---

# Track F — the browser-driven pass

**Spec:** "Ordering and verification" — *waves 2 and 4 additionally need a browser-driven pass against the real backend, because this machine cannot screenshot the Tauri window and the defects that matter most in those waves are ones no static instrument has ever caught.* Lands as PR 6 (evidence and any fixes it finds).

The 2026-09-10 pass found two congregation-facing defects — a first verse painted with its top and bottom lines sliced (rule 42), and a screen that reconnected mid-service coming back blank (rule 43) — that `qa-inventory` was blind to, while correctly reporting 0 handlerless buttons throughout. Track B's three fixes are all about pixels this machine cannot see from a unit test.

### Task 15: Drive it and record what it does (P1)

**Files:**
- Create: `docs/qa/audits/DESIGN.md` — frozen evidence, findings only. (The plan named it `DESIGN-2026-09-15-WAVE2.md`; the pass ran on the 16th and the file carries that date. `crossrefs.test.js` let the wrong name through only because "this task creates it" is the forward-reference escape hatch, which is exactly how a permanently dead path survives in a file agentic workers read.)
- Modify: `docs/qa/RELAY_GAP.md` — any new `RG-` ids, filed there and nowhere else

- [ ] **Step 1: Bring the real backend up**

```bash
npm run tauri dev
```

with the console rendered in Chrome against the mock bridge and the output/stage pages against the real backend on `:8032`, per the browser audit harness.

- [ ] **Step 2: Walk the six checks this wave owns**

1. **The countdown at 1920×1080.** Open `output.html`, fire a countdown on a layered template with no timer layer, confirm the digits are inside their box and that `onFit` reported — Track B, Task 5. Repeat at 1280×720 and at a 2.4 m viewing distance through the distance preview.
2. **The ticker.** Fire an announcement with a long fixed label; confirm the label is capped and the body still scrolls — Task 6.
3. **The contrast panel.** Fire a long verse on a panelled template over a bright background; confirm it clips rather than leaving the plate — Task 7.
4. **The default template.** With a screen set to *Follow the content look* and no look bound, change the default in Templates and confirm the open screen re-resolves **without being reopened** — Track A. Then reconnect the browser source and confirm the hello reply carries it.
5. **The twenty-five.** Open a fresh profile, count the gallery, and render one member of each family through the inspector preview at each of the five kinds — Track D.
6. **The two registers.** Open a template in the editor and confirm the first click on the content filter no longer looks like it wiped four kinds — Track C.

- [ ] **Step 3: Write the audit**

One dated file in `docs/qa/audits/`, findings only — closures go in a fix log, never in the findings. Note what was measured against the real backend and what was measured in a browser against the mock bridge; they are different claims.

- [ ] **Step 4: File anything found**

New findings go in `docs/qa/RELAY_GAP.md` as `RG-` ids. Do not restate them in `RELAY_V1_AUDIT.md`, `QA_HARNESS.md` or `CLAUDE.md` — four documents disagreeing is how that happened last time.

- [ ] **Step 5: Commit and open the PR**

```bash
git add docs/qa
git commit -m "docs: the wave 2 browser-driven pass"
# --fill takes the body from this track's commit messages, which are already normal English prose.
gh pr create --base feat/wave7-timers-templates-stage --title "Wave 2 track F — the browser-driven pass" --fill
```

---

# Wave close

- [ ] Both suites green, read from the runners' own summary lines: `npm run build && npx vitest run` and `cd src-tauri && cargo test`.
- [ ] `cargo fmt --all && cargo clippy --all-targets -- -D warnings` clean.
- [ ] `npm run version:check` passes.
- [ ] `node scripts/qa-inventory.mjs` reports no orphaned components and no handlerless controls.
- [ ] `docs/qa/QA_HARNESS.md` §0 carries the new counts, and no other document restates them.
- [ ] **Nothing here moves the release decision.** Word error rate is still unmeasured in every language, neither platform has a code-signing certificate (RG-73), and nobody but the author has run a service. Detection, the router and every threshold are untouched — rules 10, 28, 30 and 34 are not in this wave's diff at all.

## Spec coverage

| Spec section | Task |
|---|---|
| §2.1 why the merge is correct | Track E preamble, Task 11's test comments |
| §2.2 what is deleted | Task 11 |
| §2.3 the migration | Tasks 9, 10 |
| §2.4 the seed becomes twenty-five | Tasks 12, 13, 14 |
| §2.5 content looks | Task 8 (the `ctMap` item is already done — see Corrections) |
| §2.6 the default template applying | Tasks 1, 2, 3, 4 |
| §2.7 overflow | Tasks 5, 6, 7 |
| Browser-driven pass | Task 15 |

---

<!-- ===== was docs/archive/WAVE-DESIGNS.md, merged 2026-09-21, verbatim ===== -->

> **Archived 2026-09-21.** Historical. The design waves 0 to 4 were built to; the work landed by 2026-09-17. Rulings are in `docs/DECISIONS.md` §71 to §95; findings are in `docs/qa/RELAY_GAP.md` RG-139 to RG-165. Its own status line ("approved, not yet implemented") describes the day it was written. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Design — timers, templates, stage, settings, routing

**Date:** 2026-09-15
**Status:** approved, not yet implemented
**Branch:** `feat/wave7-timers-templates-stage`

This document is the design for a seven-part change set. It is written for the
engineers and agents who will implement it. Every claim about current behaviour
in this document was verified against the working tree by reading the code at
the cited line, not inferred from the handbooks — several of the handbook
statements it corrects were wrong.

## What prompted this

An operator's review raised thirteen items: the timer, the splash screen, the
word to the preacher, multiple programme timers, the templates/themes
duplication, planner colour coding, content looks, the stage display, output
routing to OBS and ATEM, the Settings surface, template overflow, a slide-size
control, and colour in Quick tools.

Investigation found that **most of the substrate already exists.** The twelve-phase
work recorded in `docs/REBRAND.md` shipped a countdown with pause and a warning
rule, a word to the preacher as a stage-only hub message, a splash screen, a slide
grid, and a single template model. The work below is therefore mostly repair,
consolidation and completion, with exactly one genuinely new subsystem (the timer
registry).

Four things the review asked for were found to be already built and working, and
are **not** in scope except where a wave touches them for another reason:

- The word to the preacher already overrides the whole stage display and already
  flashes. `src/Stage.svelte:921-954` — `position: fixed; inset: 0; z-index: 50`,
  outside the zone layout on purpose, pulsing `#c8121c ↔ #7a0a11` under
  `prefers-reduced-motion: no-preference`. It reaches no congregation screen
  (`src/lib/r6-contracts.test.js:85`).
- The countdown warning already flashes on all three surfaces — wall, stage and
  dock (`src/lib/TemplateRender.svelte:1702`, `src/Stage.svelte:917`,
  `src/lib/Dock.svelte:1510`).
- Stage zones (`note`, `next`, `elapsed`) exist, persist per device under
  `relay.stage.zones`, and have defaulted **on** since 2026-09-14.
- STT model pinning exists and works: `select_stt_model`
  (`src-tauri/src/main.rs:4836`, service-lock guarded) writes
  `app_settings['stt.model']`, and `build_stt` (`main.rs:4714`) reads it at every
  launch, where the operator's choice beats `MODEL_CANDIDATES` order
  (`stt.rs:1120`).

### A correction this change set must make to CLAUDE.md

`CLAUDE.md` states, in the build-status block, that *"a pilot must pin the model,
and nothing currently does."* That is false as written and has been since
`select_stt_model` landed. Wave 1 corrects it. The genuinely missing control
beside it is a **pinned recognition language**: the picker at
`src/lib/views/Settings.svelte:1277` defaults to "Auto-detect (code-switching)",
and RG-116 records that automatic language election cost a whole service on
`ggml-small` — 17 incoherent transcripts against 161 with the language fixed.

## The four decisions this design rests on

These were put to the operator and answered. They are recorded here because three
of them change or supersede a written decision, and a later reader must be able to
see that they were deliberate.

1. **A stage timer survives a panic control; a congregation timer does not.**
   This reverses the refusal recorded in `docs/DECISIONS.md:538` (§27). See
   "Wave 3 — the §27 reversal" below for why this makes the code more honest
   rather than less.
2. **Themes are deleted as a concept.** The nine builtin themes become five named
   template families across the five content kinds.
3. **The seed becomes twenty-five templates** — five styles × five content kinds.
   Fourteen of the current thirty-nine are deleted.
4. **The Planner gains a screen choice per cue; no existing fire path is
   removed.** Removing them would break the operator override that CLAUDE.md
   names as a non-negotiable constraint.

## Constraints every wave must respect

These are the guarantees the fire path already carries. They were surveyed
specifically for this change set; each is cited so an implementer can re-read the
enforcement rather than trusting this list.

| Guarantee | Choke point | Pinned by |
|---|---|---|
| One caller of `channels::broadcast_content`, with the pre-air validator inside it | `main.rs:605` `broadcast_with_clock` | `hardrules.test.js:246`, `safescreen.test.js:124` |
| A passage may not outlive the content that replaced it | `main.rs:632`, a negated match on `kind` | `e2e.rs:1458` |
| Only `content`, `clear` and `black` are retained for a late-joining screen | `channels.rs:936` `is_screen_frame` | `channels.rs:3272`, `:3333`, `:3398`, `:3695` |
| The fire path is generic over `tauri::Runtime` | four names | `hardrules.test.js:114` |
| A panic control may never report a success it did not achieve | `main.rs:5755`, `:5765`, `:5779` | `panic.test.js` (14 tests) |
| Nothing leaves the machine during a rehearsal | `channels.rs:880` `rehearsing`, five call sites | `e2e.rs:1043`, `:1083`, `:1614`, `:2474`, `r6.rs:118` |
| An enumerated set of commands is held back while a service records | `servicelock.rs:88` `guard` | `servicelock.rs:200`, `:293` |
| Every command the frontend calls exists; every event is heard or excused | `ipc.test.js` | itself |

Three of these enforcements have gaps that this change set must close **before**
it relies on them. They are wave 0.

---

# Wave 0 — prerequisites

Small, and everything after it depends on the guarantees being real rather than
documented.

## 0.1 Rehearsal gating needs an enumeration test

Retention is enumerated: `channels.rs:3304` `FRAME_VERDICTS` plus
`every_kind_this_module_publishes_has_an_explicit_verdict`, which scans the
module source and fails on any published kind with no verdict. Rehearsal gating
is not. The list of four gated publishers lives only in a doc comment at
`channels.rs:849-856`.

Wave 3 adds a publisher. A publisher with no `rehearsing()` check currently fails
no test, and the repository has already been bitten by exactly this — the comment
at `channels.rs:1113-1118` records that `stage_next` shipped ungated and was
invisible to `e2e.rs`'s `Wall` because it emits no Tauri event.

**Build:** a test in the same shape as `FRAME_VERDICTS` — scan `channels.rs` for
every function that calls `publish_kiosk` or `emit`, and require each to appear in
an explicit `REHEARSAL_VERDICTS` table with a gated/ungated verdict. The four
deliberately ungated publishers (`transition`, `set_template`, `set_themes`, and
`main.rs::set_channel_template`) each carry a reason at their call site; the table
must carry the same reason, so a future reader does not have to guess.

**Watch for:** the retention scanner reads `include_str!("channels.rs")` only, so
`channel_template` — published from `main.rs:5691` and `:5706` — has no verdict in
`FRAME_VERDICTS` today. The new rehearsal scanner must read both files, and the
retention scanner should be widened to match in the same change.

## 0.2 `fire_media` must become generic over `tauri::Runtime`

`main.rs:2972` takes a concrete `tauri::AppHandle`. It is a real fire path with no
`e2e.rs` coverage, and rule 24's enforcement (`hardrules.test.js:114`) checks four
names only — `fire_manual`, `handle_nav`, `clear_or_report`, `persist_cue` — so it
cannot see the omission.

**Build:** make it `fn fire_media<R: tauri::Runtime>(app: tauri::AppHandle<R>, …)`,
add `fire_media` to the name list in `hardrules.test.js:114`, and add an `e2e.rs`
case that drives it.

## 0.3 Two enumerations name a command that no longer exists

`push_announcement` appears in `servicelock.rs:217` and `transport.test.js:126`.
It is not a registered command. Neither assertion checks registration, so the
staleness is silent, and `transport.test.js`'s `SCREEN_COMMANDS` is the gate on
"every wrapper that fires decides about `liveCue.onAir`" — it only checks the
eight names it lists.

**Build:** remove the dead name from both; make both lists assert that every name
in them is registered in `generate_handler!`, so the next one cannot rot quietly.

**Verification for wave 0:** `cd src-tauri && cargo test` and `npx vitest run`,
both green, and each new test verified to fail when the defect it describes is
reintroduced. Per CLAUDE.md: test the bug, not the fix.

---

# Wave 1 — Settings, splash, routing

Three independent tracks. None blocks another and none blocks a later wave.

## 1.1 Settings truth pass

Ten findings, ordered by severity. Full evidence is in the audit that produced
them; the essentials are below.

### F-2 — Safe mode disarms nothing (P1, live safety)

`src/lib/views/Settings.svelte:914`. The row's own words are *"Outputs will not
open and detection is disarmed — nothing Relay does can reach a screen."*

What actually happens: `setSafeMode` patches a `localStorage` boot record. `$safeMode`
is read in `src/App.svelte` at lines 419 and 493 only, both inside `onMount`.
There is no reactive statement re-applying it. `src/lib/views/Live.svelte`
references `safeMode` zero times, so the run surface's fire path is not gated at
all. There is no `safe_mode` anywhere in `src-tauri/src/`. Already-open windows
stay open and the detector stays armed until the next launch.

**Build:** the enforcement belongs where the promise is made. `setSafeMode`
becomes the choke point: on the transition it disarms detection and closes output
windows, and it **reports a failure rather than flipping the label** — the same
contract as a panic control, because it makes the same kind of promise.

**Do not** fix this by adding a `$safeMode` check to each fire site. That is the
shape of the four "guarantee kept on one door, skipped on its twin" bugs CLAUDE.md
already records.

**Open question for the implementer to settle and record:** whether safe mode
should also exist in Rust so the engine cannot be armed while the record says
disarmed. The frontend choke point is sufficient for the stated promise; a Rust
flag would be stronger. Decide, and write the reason down.

### F-6 — The setup walk-through is unguarded during a recorded service (P2)

`Settings.svelte:1438` carries no `disabled` expression in any state. One click
sets `session.setupDone = false`, which mounts `FirstRun` full-screen over a live
console. From inside it, `stopMicTest()` and `chooseDevice()` each call
`stopCapture()` on the live microphone (`FirstRun.svelte:177`, `:201`), and "Try
it" fires `manualFire('John 3:16')` to the congregation's screens (`:222`).

The service lock cannot help, because `restartSetup` is a session write and
`servicelock::guard` is never consulted.

CLAUDE.md rule 44 already names this sentence. The Esc half of that rule is fixed
and pinned (`FirstRun.svelte:271`, `:283`; `panicoverlay.test.js:121`). The
"over a recorded service" half is open and has no RG row.

**Build:** disable it while `$serviceLock.engaged`, using the lock's own wording —
the service-lock section is six rows below it on the same page.

### F-1 — "Check for Updates" reports a check it never ran (P2)

`Settings.svelte:1580`. While a service is recording, `checkForUpdate()`
(`src/lib/updater.js:92`) returns `null` **without calling `noteChannel`**, so
`doCheckUpdates` falls through to `ch.state === 'ok'` and prints "You're on the
latest version." The status row six pixels above it is honest, because it goes
through `describeChannel`. The two disagree and the louder one is wrong.

This is rule 35 on the one path by which a fix reaches a church that already has
Relay — the same category as RG-83 and RG-133.

**Build:** the refusal is a **third outcome**. Either `noteChannel('skipped')` and
let `describeChannel` own the sentence, or disable the button with the reason. The
button must not reuse a stale channel state as its own result.

### The remaining six

- **F-3** (P3) — "Detect speakers" (`Settings.svelte:1077`) is pixel-identical when
  the permission is refused and when the machine genuinely has one output.
  `audioOutput.js:81` already knows which happened; return it and say so, with the
  OS path to reverse a refusal.
- **F-5** (P4) — `ModelSetup.svelte:54` renders a Rust `Err(String)` verbatim,
  bypassing `errors.js`, the one humaniser. Latent today because those strings are
  volunteer-worded; nothing constrains the next one.
- **F-7** (P3) — `History.svelte:580` "End current service" swallows every failure
  in a bare `catch {}` (`capture.js:605`) and repaints the same list. Same shape as
  rule 15, on a smaller control.
- **F-8** (P3) — the Sentry DSN field (`Settings.svelte:1853`) has no commit path
  while crash reporting is already on: `setCrashReporting` is called only from
  `toggleCrash`. The one control that decides where data leaves the machine can be
  edited and silently keep pointing at the old place. Commit on blur, or add a Save.
- **F-9** (P3) — `Settings.svelte:1455` paints a demo-content count in
  `var(--v-amber)` by inline style, on a page that is never on air, in a file whose
  own comments say "Rose, never amber" three times. Use `.s-netwarn`.
- **F-10** (P3) — `updateMsg` (`:1583`) and `crashMsg` (`:1890`) have no live
  region, while six other message surfaces on the same page do. Add `role="status"`.

### 1.2 The CLAUDE.md corrections

Two, both in the build-status block:

1. *"a pilot must pin the model, and nothing currently does"* — false. Replace
   with what is true: pinning exists (`select_stt_model` → `app_settings['stt.model']`
   → `build_stt`), and what is missing is a pinned **language** and a pre-service
   confirmation of which model and language a service will run on.
2. The event count. CLAUDE.md and `docs/ARCHITECTURE.md` §6 both say nineteen.
   The scanner's own set is **twenty**; §6's table lists eighteen, omitting
   `output://error` and `output://transition`. Reproduce with
   `grep -rhoE '"[a-z_]+://[a-z_]+"' src-tauri/src/*.rs | sort -u`, which yields
   twenty-one including `tauri://localhost`, an origin string at `channels.rs:2278`
   and not an event.

Also add: a recognition-language pin, defaulting to something other than
auto-detect, and a pre-service statement of model + language. `Dashboard.svelte:267`
currently says "Ready for a service." over `ggml-base` without naming it.

## 1.3 Splash and boot

`src/lib/Splash.svelte` exists — 399 lines, amethyst brand (never amber, stated at
`:9`), `BOOT_HOLD_MS` 900 and `BOOT_CAP_MS` 4000 (`App.svelte:393`). A clean boot
collapses to the splash alone; the four stage screens and four gates appear only
when they have something to say (`BootSequence.svelte:23`).

This is a design-quality pass, not a build. Scope: the hero lockup, the stage and
detail lines, the spinner, and the transition into the console. Nothing about the
boot ladder's logic changes, and `boot.js`'s "a stub can never render green" rule
is untouched.

## 1.4 The output routing map

Research conclusion: **Relay already has both realistic paths. What is missing is
the map, not the technology.**

The two real paths:

1. **A fullscreen Tauri webview on a physical monitor** —
   `channels::open_native_window` (`channels.rs:419`) places a borderless webview
   inside the target monitor's bounds and fullscreens it; `auto_open_outputs`
   (`main.rs:5164`) opens only `native_window` channels, only onto a connected
   display, and never onto the operator's own monitor.
2. **The `:8032` URL in a browser source** — and this one costs **zero GPU display
   pipes**, which is the entire answer to "my laptop has one HDMI port".

Facts that settle the ATEM question, each confirmed against Blackmagic's own
tech-spec pages:

- Every ATEM Mini has HDMI inputs and no SDI inputs. Every rack-mount ATEM has SDI
  inputs and no HDMI inputs. The single HDMI connector on an ATEM Television
  Studio HD8 is an **output**.
- A **Blackmagic Micro Converter HDMI to SDI 3G, $75**, dissolves the whole SDI
  question. Relay emits a plain HDMI display signal — which it already does — and
  the dongle does the rest. Nothing SDI-aware is needed in software, which is why
  CLAUDE.md's "no native SDI hardware integration" constraint costs nothing.
- **No ATEM accepts NDI.** None of them. Blackmagic's IP direction is SMPTE 2110.
  NDI would buy OBS, vMix, TriCaster and ProPresenter — a different set of
  destinations from the one the ATEM question is about.
- The ATEM **Media Player is not a live input**: stills only, roughly three seconds
  per 1080p frame, RLE-compressed so dense text is slower, a pool lock, twenty
  slots. A pre-service graphics store, not a live text renderer.
- **SuperSource composites inputs already on the switcher.** It is not an ingest
  path.
- A **virtual camera is not a signal.** Both the Windows and macOS APIs register a
  device with the local OS camera stack; nothing reaches a cable. The macOS route
  additionally needs a restricted Apple entitlement, so it sits behind the
  code-signing certificate Relay does not have (RG-73). Syphon and Spout are the
  same story: same-machine texture sharing.
- **Apple Silicon has no DisplayPort MST extended desktop at any chip.** Base
  M1/M2 drive one external display; M3/M4 base drive two; only Max tiers reach
  four. DisplayLink is the only workaround, and granting it the macOS Screen
  Recording permission **disables HDCP system-wide** — harmless for scripture
  pages, a trap for a church that also plays a licensed clip.

**Build:** documentation, plus one helper in Outputs that answers "how do I reach
this screen?" for the four cases a church actually has — a projector, an ATEM, an
OBS machine, and a screen with nothing but wifi behind it.

**Also fix:** `docs/SPEC.md:25` claims Relay "talks to OBS, ATEM, and ProPresenter
over NDI, HDMI, and the local network." NDI is parked and `open_ndi_output`
returns an error. `SPEC.md:151`, `:169`, `:171` and `Settings.svelte:1421` all
state it correctly; line 25 does not.

**Not in scope, and worth stating:** NDI is genuinely reachable without shipping a
proprietary byte — Vizrt documents `NDIlib_v5_load()` runtime loading as being
"of value in Open-Source projects", with a redistributable-URL constant provided
for exactly the case where the runtime is absent. It stays parked because it buys
nothing toward ATEM, which is what was asked about.

---

# Wave 2 — Templates ⊕ Themes

## 2.1 Why the merge is correct rather than cosmetic

A theme has **no field a template does not already have**. `THEME_STYLE_KEYS`
(`src/lib/themes.js:29`) is a twenty-one-entry subset of the same flat `style`
keys a template carries. There is no `themes` table — custom themes are a single
JSON array in `app_settings['themes.custom']`. There is no Rust `Theme` struct.
The only theme-exclusive concepts are `builtin: true` and a negative id.

And `LAYER_THEME_KEYS` (`src/lib/themes.js:552`) records that **nine of the
fourteen theme controls move nothing on a layer template**, because `applyTheme`
resolves tokens on `color`, `fill` and `font` only (`:344-356`).

`docs/DECISIONS.md:501` (§27) established themes as "a style layer BENEATH
templates, not a parallel system". The merge does not contradict that; it
completes it. A layer beneath templates whose only content is a subset of the
template's own keys is a layer that has finished being useful.

## 2.2 What is deleted

- `src/lib/views/themes/ThemeGallery.svelte`, `src/lib/views/themes/ThemeEditor.svelte`
- The Themes desk in `src/lib/views/Templates.svelte`'s two-way switch
- `style.themeRef` and `templateThemeRef` / `resolveThemed` / `applyThemeToTemplate`
- `app_settings['themes.custom']`, `loadThemes` / `saveTheme` / `deleteTheme` /
  `exportTheme` / `importThemeFromFile` (`capture.js:1820-1946`)
- The `themes` hub frame and `KioskHub::set_themes` (`channels.rs:1430`), plus its
  `FRAME_VERDICTS` row and its `hello` reply (`channels.rs:1730`)
- `THEME_PREVIEW_TEMPLATE`, the shim canvas the two galleries painted through

`src/lib/themes.js` keeps `THEME_FONTS` / `fontLabel` and the token resolvers, which
are about fonts and layer tokens rather than about themes, and moves to a name
that says so.

**What must NOT be deleted:** `session.js:227` `MOVED_TABS.themes = 'templates'`.
A stored session naming the old desk must still land somewhere valid.

## 2.3 The migration

Every template carrying `style.themeRef` has its resolved style keys inlined once
and the ref removed. The effective look is unchanged by construction, because
`{ ...theme.style, ...template.style }` is what `applyTheme` already computes.

**It must be retryable** — CLAUDE.md rule 25. `DROP TABLE IF EXISTS` any scratch
table first, roll back on failure, and never leave a transaction open for the
following `PRAGMA foreign_keys = ON` to no-op inside. A migration that fails
mid-batch and then fails every subsequent boot, before the window is shown, is a
failure mode this repository has already had.

Custom themes in `themes.custom` are migrated to real templates before the key is
dropped, one template per (theme × kind) the operator actually used. A theme
nobody applied is dropped with its name recorded in the migration's log line.

## 2.4 The seed becomes twenty-five

Five style families × five content kinds — Scripture, Songs/Lyrics, Media,
Announcements, Timer/Countdown.

The pattern already exists: `theme_templates()` (`src-tauri/src/db/templates.rs:371`)
seeds twelve as three families × four kinds, and
`every_theme_is_a_complete_coordinated_family` (`:860`) already enforces
completeness. Extend it to five families × five kinds and delete the fourteen
leftovers — the fourteen ad-hoc presets at `:271` and the parts of the eight-item
shelf that no family claims.

`preset_template_count()` (`:524`) is the single source of the number and must
agree. `the_bare_fixture_is_a_first_launch_and_nothing_more` (`qa.rs`) is the
tripwire that will catch a seed that drifts from what a fresh install contains —
it is what caught `tpl_song` being seeded on purpose, and it will fail loudly here
if the count and the fixture disagree.

**The five families are Classic, Aurora, Ember, Lower Third and High Visibility.**

This choice needs stating, because "five styles" and "twenty-five templates" can
be read two ways and the wrong reading loses tested capability.

A lower third is not a sixth content kind — it is a *keyed variant*, and the
transparency law in `resolveOutputTemplate` (`src/lib/layers.js:262`) depends on
keyed templates existing at all. Making it one of the five families means every
content kind gets a keyed option, which is strictly more capability than the two
keyed presets that ship today. High Visibility is likewise a family rather than an
extra: CLAUDE.md names it as an accessibility feature beside `legibility.js` and
the distance preview, and it has to cover every kind to be worth having.

**SuperSource and Stage · Large type do not take seed slots.** Both are already
`STARTERS` entries in `src/lib/layers.js:729` — the SuperSource starter and its
inspector block landed in REBRAND phase 12 — so they stay reachable when creating
a template, and `composite.test.js`'s coverage of the `region` layer is untouched.
A starter is a way to make a template; a seed row is a template a church did not
ask for. Twenty-five seed rows, starters unchanged.

Nocturne is dropped. Its style keys are close enough to Ember and Classic that
keeping it would cost a family slot that Lower Third and High Visibility each use
better.

## 2.5 Content looks

The operator reported that clicking a content look "activates all". It does not —
no handler in the repository writes more than one kind per click, and every call
site of `setContentTemplate` passes exactly one key.

What actually happens: the template editor stacks **two visually identical
five-chip grids over the same five labels**, both `.te-showgrid`
(`TemplateEditor.svelte:1697` and `:1717`). The second, "Content this template
renders", starts **all five ticked**, because `templateShows` returns `true` for
every kind when `layout.shows` is absent (`src/lib/layers.js:246`). The first
click materialises the list as all-kinds-minus-one. The file's own comment at
`:1676` records that the two rows were already known to look alike.

Both rows are real features and neither is deleted:

- **"Used for"** binds a kind to this template — one row per kind in `app_settings`
  under `tpl_{kind}` (`src-tauri/src/db/settings.rs:36`). A `set_setting` upsert,
  so binding a kind to template B silently replaces A; no unset-the-previous-owner
  step exists and none is needed.
- **"Content this template renders"** is the per-screen allow-list — a stage or
  confidence monitor that should ignore a picture and hold what it had.

**Build:** give the second row a different control shape — a switch list in its own
section with its own heading, not a chip grid — and write `layout.shows`
explicitly on every seeded template so it never starts in the all-ticked state
that made the first click look destructive.

**Also:** `Settings.svelte:186` holds a **private `ctMap`** that omits `countdown`,
contradicting the "one store" comment at `capture.js:274`. It must read
`$contentTemplates` like the other three surfaces.

## 2.6 The default template actually applying

`default_template_id` is a settings-KV integer. It is read by **two of twelve
render surfaces**. Rust never reads it at all — `grep default_template_id
src-tauri/src/` returns nothing. `src/Output.svelte:77` — the real wall, the kiosk
and every OBS browser source — falls back to a hard-coded `DEFAULT_TEMPLATE`,
which is `BUILTINS[0]`, Classic Serif.

The default reaches a screen exactly once, at creation: `addChannel(name,
newTarget, $defaultTemplateId ?? 1)` (`Channels.svelte:348`). Afterwards, changing
the default broadcasts nothing — `setDefaultTemplate` is a plain `set_setting`
(`capture.js:264`) with no `channel://retemplate` and no hub push.

That is the whole explanation of both reported symptoms: the default does not
activate on all screens, and the preview does not show it.

**Build:**

1. One resolver that every surface calls, taking the default as the last link in
   the chain the way `Live.svelte:1349` already does: channel template → content
   look → configured default → bundled builtin. Never a bare `BUILTINS[0]`.
2. Rust reads it, so `cue_or_content_tpl` (`main.rs:3029`) can answer with it.
3. Changing the default broadcasts `channel://retemplate` to screens that follow
   the content look, so the change is visible without reopening anything.
4. The gallery inspector preview (`TemplateGallery.svelte:729`) renders through
   the same resolver instead of being hard-bound to the selected row.

**Do not** break `resolveOutputTemplate`'s existing precedence
(`src/lib/layers.js:262`) — transparency law first, then a pinned cue template,
then the screen's own, then the content look. DECISIONS §29 and §70. The default is
a new final fallback, not a new winner.

## 2.7 Overflow

Three concrete sources, all outside the fit loop:

1. **The default countdown block.** `TemplateRender.svelte:1483-1497` emits
   `.cd-default` in layer mode, outside the `{#each layerViews}`, carrying no
   `.ltext` / `.lfit` class. `fitLayers`'s `querySelectorAll('.ltext')` (`:1239`)
   never sees it, `fitBoxes()` (`:580`) never sees it, so it is invisible to
   `overflowing()` **and** to `onFit`. Its CSS (`:1649`) sets no `overflow` at all,
   and it renders at `verseSize * 2`.
2. **Ticker mode.** `.ticker` / `.ticker-label` / `.ticker-run` (`:1528`) render
   *instead of* `.content`, and `fitText` queries `.slide .content` (`:361`), so
   the loop finds zero boxes and `lastFitScale` stays 1. `.ticker-label` is
   `white-space: nowrap` at raw `refSize`.
3. **The contrast panel.** `.content.panel { overflow: visible }` (`:1729`)
   removes the clip that `.content` (`:1719`, `overflow: hidden`, `max-height: 92%`)
   otherwise provides.

**Build:** give `.cd-default` the `.ltext` / `.lfit` contract so the binary search
sees it; give ticker mode its own fit pass; clip `.content.panel`.

**Do not weaken rules 37 or 42.** The 45% legibility floor is a *ratio* of the
template's own chosen size, not a point size, and `TemplateRender` must keep
shrinking and showing rather than blanking — a blank screen is strictly worse for
a congregation. The two-try refit bound stays.

**Note for the implementer:** `templatefit.test.js` asserts `needsRefit` and the
pure `templatemodel.js` helpers, and asserts **nothing** about `fitLayers`, the
binary search, `FIT_FLOOR_CQW`, `MIN_LEGIBLE_SCALE`, `onFit`, `verifyFit` or
`document.fonts.check`. The new coverage has to be written, not extended. And
`fittedWithTheRealFont` (`:602`) is **inert in the shipped product** — Relay
bundles no webfont, so an empty `FontFaceSet` answers `true` for every family. Its
own comment says so. Do not delete it (rule 42 exists for a real measured bug) and
do not count it as protection.

---

# Wave 3 — Timers

## 3.1 What is wrong today

The countdown is **four fields riding on the one live `OutputContent`**
(`channels.rs:94`, `:107`, `:121`, `:123`), held in a single slot,
`CountdownState(Mutex<Option<OutputContent>>)` (`channels.rs:1017`). It has no
identifier. `grep timer_id` and `grep countdown_id` return nothing anywhere in
`src/` or `src-tauri/src/`.

It is a wall-clock deadline broadcast once and ticked locally by each renderer —
three `setInterval`s, at 250 ms on the wall, 500 ms in the dock and 1000 ms on the
stage page. Rust owns no tick. A paused countdown is not an instant, which is why
`countdown_paused_ms` exists as the one exception; `countdown.js:76` is the single
reader of how long is left, and every surface goes through it. That part of the
design is good and survives this wave unchanged.

What is wrong is its lifetime. `note_countdown` (`channels.rs:1029`) keeps state
only while the live content is a countdown:

```rust
let next = content
    .filter(|c| c.countdown_to.is_some() && c.kind.as_deref() == Some("countdown"))
    .cloned();
```

Fire a verse, a song or a notice and the countdown is forgotten. `adjust_countdown`
then answers `"Nothing is counting down."` (`main.rs:2875`), and there is no
resume path — `start_countdown` is the only place a countdown is created. Pinned
by `e2e::r7_a_cleared_countdown_cannot_be_brought_back_by_the_transport`.

That is precisely the reported defect, and it cannot be patched. It is a
consequence of where the state lives.

## 3.2 The design: one registry, two scopes

`TimerRegistry(Mutex<HashMap<TimerId, Timer>>)` replaces the single slot.

```
Timer {
    id:           TimerId,
    label:        String,
    done_msg:     String,
    target_ms:    i64,          // epoch; the deadline, as today
    from_ms:      i64,          // when it was aimed; the warning rule's span input
    paused_ms:    Option<i64>,  // held with n ms left, as today
    warn_ms:      Option<i64>,  // per-timer threshold; None = the default rule
    scope:        Scope,        // Both | Stage
    plan_item_id: Option<i64>,
}
```

**`start_countdown` becomes a thin wrapper.** It creates a `scope: Both` timer and
fires a content frame that projects it into the four existing `countdown_*` fields.
Everything downstream is unchanged: `Output.svelte`, `Stage.svelte`,
`TemplateRender`, `kiosk_content_json`, `countdown.js`, and all sixteen existing
countdown tests keep working against the same wire form. The registry becomes the
source of truth and the fields become its projection.

**Programme timers are `scope: Stage` and fire no content frame at all.** That is
exactly why they survive a verse: nothing about them rides on the live content, so
replacing the live content cannot forget them.

**One formatter, one reader.** `countdown.js::countdownRemainingMs` stays the only
arithmetic. Adding a second would reproduce the bug `docs/REBRAND.md` phase 7
records as already fixed once.

## 3.3 Publication and retention

Timers publish a `{"kind":"timer", …}` hub frame carrying the registry's
stage-visible entries.

**It gets its own retained slot — `last_timers` — and is never `last_screen`.**
The precedent is `last_transition` (`channels.rs:1281`), which has its own slot for
exactly this reason: `last_screen` holds one frame, newest wins, so retaining a
non-content frame there means the next screen to join is sent the extra and paints
no verse. That is rule 43's trap 1, and it is why `stage_next` is excluded.

On `hello`, the order becomes: template, themes *(removed by wave 2)*, transition,
**timers**, then the retained screen frame last (`channels.rs:1719-1763`). Timers
go before the screen frame so a late-joining stage tablet paints the reading last
and does not flash a timer over it.

`FRAME_VERDICTS` (`channels.rs:3304`) gains a `("timer", false)` row — not retained
into `last_screen` — and the new `REHEARSAL_VERDICTS` table from wave 0 gains a
gated row, because a rehearsal must reach no stage tablet.

**Trap 2, restated:** `is_screen_frame` is a `contains`, not a `starts_with`,
because `serde_json`'s map is a BTreeMap and a content frame begins
`{"content_kind":…`. The first version of that matcher matched nothing while
looking exactly like the bug it fixed. Any new matcher in this wave has the same
hazard.

## 3.4 The §27 reversal

`docs/DECISIONS.md:538` says:

> The panic "Clear all screens" stays TOTAL — a monitor is not exempt. A persistent
> service timer that survives the clear was considered and REFUSED: it would mean
> adding an "except monitors" branch to a life-critical control… Whether a stage
> monitor should ignore the congregation clear is a real product decision, left
> open rather than resolved by a quiet special-case.

The operator has taken that decision: **a `Stage`-scoped timer survives a panic
control; a `Both`-scoped timer does not.**

This is recorded as a new DECISIONS section that explicitly supersedes §27's
refusal, for three reasons worth writing down:

1. §27 itself left the question open and named it a real product decision rather
   than a settled one. It is being answered, not overruled.
2. **§27's "no exceptions" is already not literally true in shipped code.**
   `src/Stage.svelte:436` made the opposite call locally — the service elapsed
   clock deliberately survives, on the stated grounds that *"a cleared or blacked
   wall is not the end of a service."* And the word to the preacher survives both
   panic controls too: the `clear`/`black` branch at `Stage.svelte:406-438` resets
   `visible`, `note`, `cdTo`, `cdFrom`, `cdPaused` and `next`, and does **not**
   reset `alert`. That one appears to be undecided rather than decided — nothing
   in the tree records a reason.
3. The scope split keeps the congregation guarantee exactly as strong as it is
   today. Clear and Blackout still take back every congregation screen totally.

**The rule that must hold:** the split lives in **one place**, and it is a property
of the timer (`scope`), never a branch inside `channels::clear` or
`channels::black` asking which screen it is talking to. A panic control that has to
ask a question can fail to answer it.

While implementing, settle and write down whether `alert` surviving a panic on the
stage page is intended. It is currently a silent third answer to the same question.

## 3.5 The warning threshold becomes a setting

`COUNTDOWN_WARN_MS = 60_000` and `countdownWarning(remaining, total)` —
the last minute, or the last tenth of a countdown shorter than ten minutes
(`src/lib/layers.js:318-338`). The comment at `:325` says it is *"a rule rather
than a setting, deliberately: the control belongs in the Settings pass, and a
setting with nowhere to set it is worse than a sensible default."*

This is the Settings pass. `warn_ms` becomes per-timer, settable per cue in the
Planner and as a default in Settings, and `countdownWarning` takes the override
when present and falls back to today's rule when absent.

**`countdownTotalMs` returns null rather than a guess** when either end is missing
(`countdown.js:105`), because a made-up span puts the warning colour on at the
wrong moment. Keep that. A colour that is on at the wrong moment is worse than one
that is on a minute early.

The flash itself needs no new work — `cdwarn` keyframes already exist on all three
surfaces, with a `prefers-reduced-motion` glow fallback on each.

## 3.6 What a new content kind must satisfy

Eleven sites consume the kind string. This wave adds one, and the list is here so
none is missed — missing one produces this repository's signature bug shape.

| # | Site | If skipped |
|---|---|---|
| 1 | `main.rs:632` rule 38 match | anything not literally `"scripture"` disarms the passage; a timer is disarmed for free |
| 2 | `pipeline.rs:247` `is_countdown` | **see below — this is the one that bites** |
| 3 | `main.rs:3029` `cue_or_content_tpl` | no content look for the kind |
| 4 | `main.rs:3057` `ContentTemplates` (five hard-coded fields) | no row in the content-look UI |
| 5 | `channels.rs:895` `kiosk_content_json` | native windows get the field, kiosk and OBS do not |
| 6 | `src/lib/layers.js:228` `CONTENT_KINDS` | the kind cannot be toggled per screen |
| 7 | `src/lib/layers.js:246` `templateShows` | **a template with an explicit `layout.shows` hides an unknown kind silently** |
| 8 | `src/Output.svelte:180` and `:279` | the filter is applied at both doors |
| 9 | `src/Stage.svelte:406-445` | the stage page branches on message kind |
| 10 | `TemplateRender.svelte:1117` `slideKey` | a kind varying in none of the five keyed fields will not re-key, so no transition and no refit |
| 11 | `persist_cue` → `cues.type` | free-form TEXT, no CHECK; no migration needed |

**Site 2 is the trap.** `pipeline.rs:240`:

```rust
let is_countdown = content.countdown_to.is_some()
    || content.countdown_paused_ms.is_some()
    || content.kind.as_deref() == Some("countdown");
if !is_countdown && !has_text && !has_media && !has_reference { return Err(Unsafe::Nothing); }
```

A new timer kind with its own field name and no text would be **refused by the
pre-air validator** as `Unsafe::Nothing`. The design avoids this by keeping the
`Both`-scoped timer's wire form as the existing `countdown_*` fields; if that
changes, `is_countdown` changes with it, in the same commit.

Site 7 is the second trap: a new kind is visible or invisible depending on a
per-template field nobody will think to update. Wave 2.5 writes `layout.shows`
explicitly on every seeded template, which is what makes this safe.

Finally, `main.rs:632` uses `is_some_and`, which is false for `None` — a payload
built with `..Default::default()` and no `kind` does **not** disarm the passage.
Every current caller sets it; nothing enforces it. Add the assertion.

---

# Wave 4 — Stage and Planner

## 4.1 Stage display

Zones exist (`reading`, `next`, `note`, `countdown`, `clock`, `elapsed`), persist
per device under `relay.stage.zones`, and default on. The alert already takes the
whole screen. The transport, the verse search and tap-to-fire already work over
`:8032/api`.

This is a layout-quality pass plus the timer rail from wave 3. It must be driven
in a browser against the real backend rather than reviewed by reading, because the
two worst defects the 2026-09-10 pass found — a first verse fitted in the wrong
typeface, and a reconnecting screen coming back blank — were invisible to every
static instrument, and `qa-inventory` reported zero problems throughout and was
right both times.

`Stage.svelte` renders **no template at all** (`:481`) and is unaffected by wave 2.

## 4.2 Planner

**Screen choice per cue.** `plan_items` has `template_id` but no channel column.
The Planner can already assign a template per cue (`ServicePlanner.svelte:986` →
`set_plan_template` → `plan_items.template_id`), and that template becomes
`template_pinned: true` on the wire, which is what lets a cue's look beat a
screen's own. The missing half is which screens. Add it, defaulting to all.

Nothing is removed. Ten-plus surfaces can fire or set a template today, and Live's
manual box and slide grid are the Sunday-morning override that CLAUDE.md names as
a non-negotiable constraint: *"Operator override is a first-class control, never a
fallback UI."* The Planner becomes the place where the **plan** decides, not the
only place anything can reach a screen.

### Correction — "add it, defaulting to all" is four pieces, and none of them exists

*Written 2026-09-16, verified by reading every line cited here against the working
tree on `feat/wave4-stage-planner`. The paragraph above is not wrong about what the
Planner should eventually do. It is wrong about what it costs, and the sentence
"add it, defaulting to all" reads like a column plus a select.*

**There is no channel dimension on the wire at all.** Per-cue screen targeting is
not an addition to an existing facility; it is the facility. Four pieces are
missing, and each one is missing in a place that already has a guarantee attached
to it.

1. **The content carries no channel.** `OutputContent` (`channels.rs:39-135`) has
   nineteen fields — `kind`, `reference`, `text`, the two media fields, the two
   template fields, `template_pinned`, the four monitor-only fields, the five
   countdown fields and `trace_id` — and not one of them names a screen. Every
   per-screen decision about CONTENT that exists today is taken by the RECEIVER
   out of `kind`, against its own template's `shows` set: a standing per-screen
   preference, not a choice the plan makes cue by cue.
2. **The publish is unrouted, on both doors.** `broadcast_content`
   (`channels.rs:1075`) ends in exactly two lines: `channels.rs:1089` emits
   `output://content` app-wide to every webview, and `channels.rs:1090` hands one
   string to `publish_kiosk`, which reaches the hub's single broadcast sender
   (`channels.rs:1513`) and therefore every connected kiosk client. Routing means
   changing the choke point rule 36 exists to protect, which is the same function
   `pipeline::preflight` is called from and the same function wave 3's remaining
   tracks are still editing.
3. **No receiver compares its own channel.** `src/Output.svelte:293` listens to
   `output://content` and filters on `kind` alone; the kiosk half does the same at
   `Output.svelte:190-195`. The page knows which channel it is — `channelId` is
   parsed from the URL at `Output.svelte:26` — and never asks the question about
   content. **The precedent that it can is `channel://retemplate`**, emitted
   app-wide with a `channel` key (`main.rs:6067` and `main.rs:6083`) and filtered
   client-side at `Output.svelte:330` on the bridge and `Output.svelte:237` on the
   socket. Addressing is possible, it is already done for the template, and it has
   never once been done for content.
4. **Retention is one global slot, and rule 43 lives in it.** `KioskHub` retains
   the last screen frame (`channels.rs:1393`) and, separately, the last timer frame
   (`channels.rs:1424`), and replays them to a client on `hello`. Rule 43 says a
   screen that joins mid-service is shown what is on the screens. **With targeting,
   "the screens" stops being one answer**: a lobby TV that reconnects must be sent
   what is on the lobby TV, so the retained frame becomes per-channel, and the
   struct's own comment already records that one slot per kind is what stops a
   clock erasing a retained verse. Getting this half wrong is congregation-facing
   and silent — it is RG-129 again, with more slots to get wrong.

**What must not move.** The panic controls stay global and stay unrouted. A `clear`
that has to work out which screens it is talking to is a `clear` that can fail to
answer, and rule 15 and DECISIONS §20 do not allow one of those; `channels.rs`
states the same thing in its own words at `:1061-1064`, about the timer scope split.
Whatever shape targeting takes, `clear` and `black` reach every screen, and the
retained frame a panic control writes reaches every screen that joins after it.

**And one question must be answered before any code is written.** `template_pinned`
means a cue's deliberate look overrides the screen's own (`main.rs:3325`,
DECISIONS §29). Nobody has said what it means on a screen the cue does not target.
The three candidate answers — the screen is untouched and keeps what it had, the
screen is cleared, the screen shows the cue but in its own template — are three
different Sunday mornings, and the wire form cannot be designed until one is chosen.

**Descoped by an operator decision on 2026-09-16.** Wave 4 specifies this and
builds none of it, for two reasons stated plainly: it is a wire change at the one
choke point wave 3's remaining tracks are still editing, and it makes rule 43's
retention per-channel, which is congregation-facing. It is filed as **RG-161**.

**Storing the choice without wiring it was considered and refused.** A
`plan_items` channel column with a Planner select in front of it is one afternoon's
work and would look like progress. It is precisely the defect the 2026-09-10 design
pass closed on seven Settings controls — a preference an operator sets, that the
application never reads, with nothing saying so (RG-132, DECISIONS §69). A column
nothing routes on is worse than no column, because the operator who fills it in on
a Tuesday believes the screens will do what they were told on the Sunday.

**Multi-timer cues** bind to the wave-3 registry through `plan_items.duration_sec`,
which exists (`db/plans.rs:72`) and currently drives no running clock.

**Colour coding.** Cue colour is deliberately zero today —
`TAXONOMY_INK = 'var(--v-faint)'` and every `TYPE[*].color` is that same token
(`src/lib/plan.js:49`, `:63-75`). The rationale at `:14-47` records which hues were
spent and withdrawn, and states that the only unclaimed hues on the wheel are
magenta ~310° and lime ~80°, for which no tokens exist.

Five kinds need five colours, so hue-per-kind cannot be invented cleanly. Use the
`--v-col-*` family (`src/app.css:222-225`), which is already kind-mapped and
already used for exactly this in the Library, and add `--v-col-timer`.

**This reuses reserved hues and is acceptable only because the Planner, like the
Library, is never on air.** `--v-col-scripture` aliases amber, which means ON AIR
anywhere a congregation's state is being reported. The Planner cannot fire to an
output (`plannerbuildonly.test.js`), which is what makes this safe — and that test
is what must keep it safe. If the Planner ever gains a fire path, this decision
must be revisited in the same change.

**Slide sizer.** `.sgrid` is fixed at `repeat(auto-fill, minmax(158px, 1fr))`
(`Live.svelte:2502`). Add a 2×2 / 3×3 / 4×4 control, following the `ZOOMS` stepper
pattern in `TemplateEditor.svelte:772` — the only zoom idiom in the app.

Note that a `Normal | Compact` density segment was built here and **deliberately
deleted**, handler, session key and CSS (`Live.svelte:1762`). This is not that: it
is a size rather than a density, it persists to session, and it exists to make a
slide readable before it is selected.

**Quick tools colour.** The card is deliberately colourless — no amber and no
amethyst anywhere, stated at `Dock.svelte:1009`, `:1447`, `:1081` and `:1520`. Its
only colour is the stage-alert red and two steel-blue primaries.

Colour-code by **tool group** using the neutral ramp and the red already present.
It cannot borrow amber (ON AIR), amethyst (rehearsal), cyan (a guess) or grey
(cued). The Controls card next door is where the four-colour treatment lives and
must stay the only place it does.

---

# Ordering and verification

Waves run in order. 0 → 1 → 2 → 3 → 4. Wave 1's three tracks are independent of
each other; wave 4's two are independent of each other.

Every wave ends green on both suites, using **the runner's own summary line** and
never a grep:

```
cd src-tauri && cargo test
cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings
npx vitest run
npm run build
```

Note `npm run build` must run before the Rust suite on a fresh tree, or two
`channels` tests fail on a bare 404 because `dist/` is gitignored (RG-127).

Every new test is verified to **fail when the defect it describes is
reintroduced**. CLAUDE.md: test the bug, not the fix. This repository has had a
test that passed against a broken file because it grepped a comment, and a
regression test written against a diagnosis that turned out to be wrong, which
passed with the supposed fix reverted.

Waves 2 and 4 additionally need a browser-driven pass against the real backend,
because this machine cannot screenshot the Tauri window and the defects that
matter most in those waves are ones no static instrument has ever caught.

## What this change set does not claim

- It does not move the release decision. Word error rate remains unmeasured in
  every language, neither platform has a code-signing certificate (RG-73), and
  nobody but the author has run a service.
- It does not touch detection, the router, or any threshold. Rules 10, 28, 30 and
  34 are untouched, and nothing here makes the fire path faster by making it less
  safe.
- It does not unpark NDI, and it adds no SDI integration.

---

<!-- ===== was docs/archive/WAVE-DESIGNS.md, merged 2026-09-21, verbatim ===== -->

> **Archived 2026-09-21.** Historical work order. Wave 3 landed 2026-09-16 and was audited in `docs/qa/audits/DESIGN.md`; findings RG-146 to RG-155; rulings DECISIONS §91 to §93. Its 63 checkboxes were never ticked; the audit is the record. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Wave 3 — Timers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This repository has never checked a box in a landed plan — completion is evidenced by commits and green suites, not by the boxes.

**Goal:** Give a timer an identity and a lifetime of its own. Today the countdown is four fields riding on the one live `OutputContent`, held in a single slot, and any other fire forgets it — so a re-aim after a verse answers *"Nothing is counting down."* and there is no way back. Wave 3 replaces the slot with a registry of identified timers in two scopes: a congregation timer (`Both`) whose wire form stays exactly the four `countdown_*` fields it is today, and a programme timer (`Stage`) that publishes to the stage tablet only, survives any other content, and survives a panic control — the reversal of `DECISIONS.md` §27 the operator has taken.

**Architecture:** Five code tracks and one verification pass, each landing as its own pull request off `feat/wave3-timers`, which branches from `feat/wave5-shelf-names-seal` (waves 0, 1 and 2 plus the wave 5 design doc). **Track A is the base and lands first** — it creates `timers.rs` and the registry every other track reads. Tracks B (publication and retention), C (the §27 reversal), D (the warning threshold) and E (the eleventh content kind and the operator's surface) run in parallel off A. Track F is the browser-driven pass and runs last against everything the five landed.

**Tech Stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest + jsdom, `cargo test`, Chrome via the browser audit harness.

**Spec:** `docs/archive/WAVE-DESIGNS.md` §3 (lines 533–726).

**Depends on:** Waves 0, 1 and 2, all verified landed in this tree on 2026-09-16 (`REHEARSAL_VERDICTS` at `channels.rs:3504`; `fire_media` generic at `main.rs:2979`; the dead `push_announcement` gone from both enumerations).

## Global Constraints

- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- Run `npm run build` before `cargo test` on a fresh tree (RG-127) — two `channels` tests fail on a bare 404 otherwise, because `dist/` is gitignored.
- Read test counts from the runner's own summary line, never a grep. Values live only in `docs/qa/QA_HARNESS.md` §0. The baseline measured 2026-09-16: Rust 784 passed / 16 ignored, frontend 2254 passed across 145 files, `e2e.rs` 66, 139 registered commands. Re-measure; do not restate.
- Every new test verified to FAIL when its defect is reintroduced. Test the bug, not the fix.
- **Rule 2:** never hold a `Mutex` across `emit` or `broadcast_content`. The registry is a `Mutex`; every reader clones and releases before it publishes. `CountdownState`'s doc comment states this discipline already — inherit it verbatim.
- **Rule 6:** lock order `Db` before `Session`. A new registry lock is innermost of all: take it last, release it first.
- **Rule 10 / 28 / 30 / 34:** no threshold moves in this wave. Nothing here touches detection or the router.
- **Rule 15:** a panic control may never report a success it did not achieve, and may never be made able to fail. Track C adds no question to `clear` or `black`.
- **Rule 24:** every new fire-path function is generic over `tauri::Runtime`. A concrete `AppHandle` re-welds the path and takes it out of `e2e.rs`'s reach.
- **Rule 33:** nothing new runs on the decoder thread and no queue is unbounded.
- **Rule 36:** the check goes at the choke point. `broadcast_with_clock` (`main.rs:612`) is the one caller of `channels::broadcast_content`; `note_countdown` is called from `broadcast_content`, `clear` and `black` — three doors, and all three stay.
- **Rule 38:** `main.rs:639` disarms the passage for any kind that is not `"scripture"`, using `is_some_and`, which is false for `None`. Track E adds the assertion this rule has never had.
- **Rule 43:** a retained frame decides what a screen is SHOWING. `is_screen_frame` (`channels.rs:938`) is a `contains`, not a `starts_with`, because `serde_json`'s map is a BTreeMap. Any new matcher has the same hazard, and its first version looking correct is the failure mode.
- **Rule 41 / 44:** no native `confirm()`/`alert()`/`prompt()`; any new overlay consumes `Esc` itself and never paints over `Clear screens`.
- `src/lib/errors.js` is the ONE backend-error humaniser. Never render a raw Rust `Err` string to a volunteer.
- `countdown.js::countdownRemainingMs` stays the ONLY arithmetic for how long is left, on every surface. A second reader is the bug `docs/REBRAND.md` phase 7 records as fixed once already.
- Commit message bodies and PR bodies are normal English prose.
- Never commit to `main`. Each track is a PR off `feat/wave3-timers`.

## Corrections to the design, verified against the working tree on 2026-09-16

Every structural claim in §3 was checked by reading the cited code. None was substantively wrong. The citations have drifted, one points at unrelated code, and three facts the design did not know have appeared since it was written.

1. **Line numbers moved.** Use these: `CountdownState` `channels.rs:1019`; `note_countdown` `:1031`; `kiosk_content_json` `:896`; `is_screen_frame` `:938`; `FRAME_VERDICTS` `:3389`; `rehearsing` `:881` with five call sites (`:1048, :1077, :1094, :1127, :1150`); `last_transition` `:1285`; the `hello` block `:1703–1791`. In `main.rs`: `broadcast_with_clock` `:612`; the rule-38 match `:639`; `start_countdown` `:2800`; the label payload `:2826`; `adjust_countdown` `:2877` with its refusal string at `:2883`; `cue_or_content_tpl` `:3036`; `ContentTemplates` `:3108`. In `pipeline.rs`: `is_countdown` `:247–249`, and the `Unsafe::Nothing` return it guards is at `:260`, eleven lines later, not adjacent as the spec's fused quote implies. Cite what you find if they move again.
2. **§3.6 site 8 is FALSE as written.** `src/Output.svelte:180` and `:279` are `applyTemplateUpdate` and a kiosk-reconnect `catch`. The two real kind filters are `Output.svelte:195` (`m.content_kind`, the kiosk door) and `:296` (`e.payload?.kind`, the Tauri door). Both still need the sweep; the line numbers in the spec do not point at them.
3. **Wave 0 landed, so `REHEARSAL_VERDICTS` already exists** (`channels.rs:3504`, a three-column table with a reason, completeness-checked by `every_publisher_in_this_module_has_an_explicit_rehearsal_verdict` at `:3580`). Wave 3's job there is one additive row, not a new table.
4. **`plan_items.duration_sec` is already taken** (`db/plans.rs:45`, migrated at `:71`) and means the Planner's running-time estimate, not a timer span. Do not reuse the name for anything a timer counts.
5. **Numbering:** the last DECISIONS section is 88, so the §27 reversal takes the next number after it. The highest register id filed is RG-142, so anything this wave files starts at the next free id. Neither number is written as a citation in this plan on purpose: `crossrefs.test.js` resolves every `DECISIONS §N` and every `RG-` id against the real documents, and a plan that cites a section it is about to create turns the suite red before a line of code is written. Write the citation in the same commit that writes the section.
6. **`servicelock.rs` needs nothing by default.** `guard()` returns `Ok(())` for any command absent from `PROTECTED`, and `start_countdown`/`adjust_countdown` are already named in `LIVE_PATH`. Track A adds the new commands to `LIVE_PATH` so the "the lock can never reach the live path" assertion keeps covering them — it does not add them to `PROTECTED`.
7. **Existing coverage to keep green, counted from the files themselves:** `e2e::r7_*` seven tests (`e2e.rs:3105, :3163, :3197, :3240, :3278, :3339, :3385`), `pipeline::a_countdown_is_not_an_empty_screen` (`:600`), `channels::the_kiosk_wire_form_carries_every_monitor_bindable_field` (`:4401`), and on the frontend `countdown.test.js` (9 describes), `countdownwiring.test.js` (6), plus countdown blocks in `layers.test.js:292, :315`, `templatestyle.test.js:228`, `fitcoverage.test.js:45`, `rendercontent.test.js:249`, `lowerthird.test.js:109`, `quicktools.test.js:350, :485, :588`. **None of these may change shape, with exactly one stated exception — item 8.** If any other one has to, that is a signal the wire form moved and the plan is being broken, not followed.

8. **One existing test asserts the defect this wave reverses, and it is the only test that changes.** `e2e::r7_the_transport_can_never_start_a_countdown` (`e2e.rs:3197`) has a second half at `:3216–3230` that starts a countdown, fires `John 3:16`, and asserts `adjust_countdown` returns `Err`. That is this wave's own scenario with the opposite verdict, and no implementation satisfies both. Its first half stands unedited: **the transport can never create a countdown from nothing**, which is what the test's name claims and what the wave leaves exactly as strong as it is today. The second half fused two claims, and only one of them is being reversed:

   - *"the transport refuses once a verse has replaced the countdown"* — **reversed.** The refusal was a consequence of where the state lived, not a decision anybody took.
   - *"nothing reached a screen and the verse must still be up"* — **kept, and made load-bearing.** Those two assertions survive verbatim into the replacement test.

   **The ruling, which is a product decision and not a test edit:** re-aiming, holding or releasing a `Both` timer that is **not currently on the screens** succeeds, changes the registry, and **paints nothing**. Putting a timer back in front of a congregation is an explicit action with its own control — never a side effect of `+1`. A transport press that repaints a countdown over a sermon is the same class of failure `adjust_countdown`'s own doc comment already forbids for templates, where it says a rebuilt fire "would silently re-skin every screen in the building". The operator override stays first-class: the way back is one action, and it says what it does.

## Operator decisions carried into this wave

- **A `Stage`-scoped timer survives a panic control; a `Both`-scoped timer does not.** Recorded as a new DECISIONS section, superseding §27's refusal.
- **The split is a property of the timer, never a question inside `clear` or `black`.** A panic control that has to ask which screen it is talking to can fail to answer.
- **`label` stays a field on the timer.** Wave 5 Track G removes the dock's hard-coded supply of one; this wave does not pre-empt it, and the two orders are compatible either way.
- **The `Both` timer's wire form does not change.** It stays the four `countdown_*` fields, which is what keeps the `is_countdown` guard in `pipeline.rs:247` from refusing a timer as `Unsafe::Nothing`.
- **Delivery: one PR per track**, six PRs off `feat/wave3-timers`, each green on both suites.

---

# Track A — the registry

**Spec:** §3.2. **The base. Lands first; every other track branches from it.**

Today `CountdownState(Mutex<Option<OutputContent>>)` (`channels.rs:1019`) holds the one live countdown, and `note_countdown` (`:1031`) forgets it the moment the live content is anything else. That is the reported defect and it cannot be patched where it lives.

`timers.rs` is a new module with one responsibility: **hold the timers and answer questions about them.** It is DB-free and IO-free and does not know what a Tauri handle is, in the same discipline as `detection.rs` and `router.rs`, so the arithmetic and the lifetime rules are unit-testable without a window.

### Task 1: `timers.rs` — the registry, pure (P1)

**Why this exists:** Putting the map inside `channels.rs` would make every lifetime rule reachable only through a broadcast, which is how the current bug got its test coverage shape — `e2e::r7_a_cleared_countdown_cannot_be_brought_back_by_the_transport` pins the defect end to end because there was nowhere smaller to pin it.

**Files:**
- Create: `src-tauri/src/timers.rs` with a module doc comment stating the single responsibility
- Modify: `src-tauri/src/main.rs` — `mod timers;`

**Interfaces:**
- Produces:
  - `pub type TimerId = i64;`
  - `pub enum Scope { Both, Stage }` — `Both` reaches every screen through the content frame; `Stage` reaches the stage tablet only.
  - `pub struct Timer { id, label, done_msg, target_ms, from_ms, paused_ms, warn_ms, scope, plan_item_id }` exactly as §3.2 names them. `warn_ms: Option<i64>` is Track D's; carry the field now so Track D is a reader and a writer, never a migration.
  - `pub struct TimerRegistry(Mutex<HashMap<TimerId, Timer>>)` with: `start(&self, Timer) -> TimerId`, `get`, `stop(&self, TimerId) -> bool`, `stop_scope(&self, Scope) -> usize`, `adjust(&self, TimerId, remaining_ms: Option<i64>, paused: Option<bool>, now_ms: i64) -> Result<Timer, TimerError>`, `snapshot(&self) -> Vec<Timer>`, `snapshot_scope(&self, Scope) -> Vec<Timer>`.
  - `pub fn remaining_ms(t: &Timer, now_ms: i64) -> i64` — the held figure when held, otherwise `target_ms - now_ms`, clamped at 0. **The same rule as `countdown.js::countdownRemainingMs` and `adjust_countdown`'s inline `current`.** One rule, stated twice across the bridge and pinned on both sides; never three times on one side.
- Consumes: nothing. No `tauri::`, no `rusqlite::`, no clock — `now_ms` is passed in, which is what makes the tests deterministic.

- [ ] **Step 1: Write the failing tests** in `timers.rs`'s own `mod tests`, each naming the field failure it prevents:
  - `a_timer_outlives_content_that_is_not_a_timer` — start a `Both` timer, then prove the registry still answers for it after anything else has been broadcast. (The registry has no broadcast; this test asserts the registry does not itself forget, and Task 3 is where the door stops forgetting.)
  - `a_held_timer_reports_the_figure_it_was_held_at` — `paused_ms` wins over the instant, at any `now_ms`.
  - `a_re_aim_does_not_release_a_hold` — `adjust(id, Some(x), None, now)` on a held timer leaves it held with `x` left. This is the `+1`-restarts-a-held-countdown failure `adjust_countdown`'s doc comment records.
  - `a_resume_re_aims_the_instant_from_the_figure_it_was_holding` — `adjust(id, None, Some(false), now)` sets `target_ms = now + held`, so releasing a hold does not teleport the deadline.
  - `adjusting_a_timer_that_is_not_there_is_refused_rather_than_creating_one` — `TimerError::NoSuchTimer`. Start is the only creator; there must be exactly one of those.
  - `a_timer_shorter_than_a_second_is_refused` — the `next < 1000` rule `adjust_countdown:2883`-onward already keeps, moved into the pure layer with its reason.
  - `two_timers_have_two_identities` — the whole point of the wave: ids are distinct and `stop` takes one without touching the other.
  - `stopping_a_scope_takes_every_timer_in_it_and_no_other` — Track C depends on this being a property of the registry rather than a branch in a panic control.
- [ ] **Step 2: Implement** until green. Ids monotonic from 1 per process; never reuse an id inside a run — a reused id is a re-aim landing on the wrong clock.
- [ ] **Step 3: Verify each test fails when its defect is reintroduced.** Revert the hold-precedence line and watch two of them fail; if only one does, one of them is not testing what it says.
- [ ] **Step 4:** `cargo fmt --all && cargo clippy --all-targets -- -D warnings`, then the Rust suite.

### Task 2: `start_countdown` becomes a thin wrapper over a `Both` timer (P1)

**Why this exists:** The registry is only the source of truth if the shipped path writes to it. The projection direction matters: the registry owns the facts and the four `countdown_*` fields become its projection, not a second copy that can drift.

**Files:**
- Modify: `src-tauri/src/main.rs` — `start_countdown` (`:2800`), `adjust_countdown` (`:2877`)
- Modify: `src-tauri/src/channels.rs` — `CountdownState` and `note_countdown` become registry-backed (see Task 3)
- Modify: `src-tauri/src/timers.rs` — `pub fn project_both(t: &Timer) -> (i64, i64, Option<i64>, String, String)` or an explicit struct; one function, so the four fields are filled in exactly one place
- Test: `src-tauri/src/e2e.rs`

- [ ] **Step 1: Write the failing e2e test** `r7_a_countdown_survives_a_verse_and_can_still_be_re_aimed_without_taking_the_wall` — start a countdown, fire `John 3:16`, then `adjust_countdown`. Today that returns `Err("Nothing is counting down.")`. After this task it returns `Ok`, **the wall count does not move, and `John 3:16` is still the last thing on it**; the re-aimed figure is then proved by an explicit put-back, which paints the countdown with the adjusted time left rather than the original one. **This is the reported defect and it is the test the wave exists to pass.** Take the two surviving assertions verbatim from `e2e.rs:3226–3230`, which is where they are today.
- [ ] **Step 1b: Edit the one test that contradicts it**, and only its second half. `r7_the_transport_can_never_start_a_countdown` (`:3197`) keeps its first half unedited; its second half's `expect_err` becomes the new expectation — succeeds, paints nothing, verse still up — and gains a comment naming the correction in item 8 above. **Do not delete the second half**: its wall assertions are the guarantee that a transport press cannot take a congregation screen, and that guarantee is now carried by fewer places, not more.
- [ ] **Step 2:** `start_countdown` creates the timer in the registry (scope `Both`, `from_ms` = now, `label`, `done_msg`, `warn_ms: None`, `plan_item_id: None`) and broadcasts a content frame projected from it. Every existing field keeps its current value and meaning, including `template_id`/`template_json`/`template_pinned` resolution through `cue_or_content_tpl` — **do not re-resolve the template on a re-aim**, which is the silent re-skin `adjust_countdown`'s doc comment forbids.
- [ ] **Step 3:** `adjust_countdown` reads the registry rather than `live_countdown` and applies through `TimerRegistry::adjust`. **It re-broadcasts only when that timer is what is on the screens right now**, carrying the ORIGINAL content verbatim apart from the four fields; when it is not, it changes the registry and publishes nothing (item 8). "What is on the screens right now" is a fact the hub already keeps — read it, do not invent a second answer to it. Its refusal string stays exactly `"Nothing is counting down."` when the registry holds no `Both` timer at all, which an operator reads and which is still true in the only case that can now produce it.
- [ ] **Step 4:** Keep all seven `r7_*` tests green **unchanged**. `r7_a_cleared_countdown_cannot_be_brought_back_by_the_transport` must still pass: a cleared `Both` timer is stopped, and the transport still cannot create one.
- [ ] **Step 5:** Verify the new test fails with Task 2 reverted, and that it fails for the right reason (the refusal string, not a panic).

### Task 3: the three doors stop forgetting, and `CountdownState` retires (P1)

**Why this exists:** `note_countdown` is called at three doors — `broadcast_content` (`:1042`), `clear` (`:1077`-ish) and `black`. Its filter is the lifetime bug. Removing the filter without removing the slot leaves two answers to one question.

**Files:**
- Modify: `src-tauri/src/channels.rs` — `CountdownState`, `live_countdown`, `note_countdown`
- Modify: `src-tauri/src/main.rs` — `setup` manages `TimerRegistry`
- Modify: `src-tauri/src/qa.rs` — `bare_app()` manages `TimerRegistry` too

**`qa::bare_app()` is not optional.** A fixture without the registry is an app in a state no church could be in, and a command reading it would panic rather than fail readably. `the_bare_fixture_is_a_first_launch_and_nothing_more` is the tripwire and it must keep passing.

- [ ] **Step 1: Write the failing tests** in `channels.rs`'s `mod tests`: `firing_a_verse_does_not_forget_the_congregation_timer`, and `a_clear_takes_the_congregation_timer_and_leaves_the_programme_timer` (the second one goes green in Track C; write it here as `#[ignore]` **only if** Track C is a separate PR, and Track C's first act is to un-ignore it — an ignored test with no owner is how `e2e.rs` carried two defects for months).
- [ ] **Step 2:** `broadcast_content` stops filtering. A `Both` timer is stopped by `clear`/`black` and by a new `Both` timer replacing it, and by nothing else.
- [ ] **Step 3:** `live_countdown` becomes a registry read that projects the newest `Both` timer, or is deleted in favour of a registry call at its two call sites. Prefer deletion: a compatibility shim over a new source of truth is how five hand-rolled `OutputContent` copies drifted apart.
- [ ] **Step 4:** `setup` and `qa::bare_app()` both `manage(TimerRegistry::default())`. Grep every `mock_builder` fixture in the tree for a third one; the repo's signature bug is a guarantee kept on one door and skipped on its twin.
- [ ] **Step 5:** Full Rust suite green; `cargo test e2e` green.

### Task 4: the commands, and the lock (P2)

**Files:**
- Modify: `src-tauri/src/main.rs` — new `#[tauri::command]`s, all generic over `R: tauri::Runtime`, all registered in `generate_handler!`
- Modify: `src-tauri/src/servicelock.rs` — `LIVE_PATH`
- Modify: `src/lib/stores/capture.js` — wrappers in the documented throw group
- Test: `src/lib/ipc.test.js` (contract, both directions)

**Interfaces:**
- `start_timer(app, db, minutes: f64, label: String, done_msg: String, scope: String, warn_ms: Option<i64>, plan_item_id: Option<i64>, template_id: Option<i64>) -> error::Result<i64>` — returns the id.
- `adjust_timer(app, timer_id: i64, remaining_ms: Option<i64>, paused: Option<bool>) -> error::Result<()>`
- `stop_timer(app, timer_id: i64) -> error::Result<()>`
- `list_timers(app) -> error::Result<Vec<TimerView>>` — a serialisable view, so the console can render a list without a second arithmetic.
- `show_timer(app, db, timer_id: i64, template_id: Option<i64>) -> error::Result<()>` — **the explicit way back onto a congregation screen** for a `Both` timer the registry still holds (item 8). It broadcasts the projection through `broadcast_with_clock` like `start_countdown` does, and it is the ONLY thing besides `start_countdown` that may put a timer in front of people. A `Stage`-scoped timer is refused here, in words: it has no congregation wire form.

- [ ] **Step 1:** Write `ipc.test.js`'s side first: the four commands must exist in Rust and be called from `capture.js`. `ipc.test.js` fails in both directions, which is the point.
- [ ] **Step 2:** Implement, `#[tauri::command]` each, register each, and add each to `servicelock.rs`'s `LIVE_PATH` with the reason: a timer control is a live control and the lock may never reach it.
- [ ] **Step 3:** `capture.js` wrappers go in **Group 1 (THROWS)** beside `startCountdown` and `adjustCountdown`, with the group named in each docstring. A wrapper placed in a group without the test that holds it there is the gap `micstop.test.js` exists for — add the test.
- [ ] **Step 4:** Both suites green. Re-measure the registered-command count; do not restate 139.

**PR 1 ends here.** Title: *the timer gets an identity and stops being forgotten*.

---

# Track B — publication and retention

**Spec:** §3.3. **Off Track A.** Lands as PR 2.

A `Stage` timer publishes no content frame — that is exactly why it survives a verse. It needs its own hub frame and its own retained slot.

### Task 5: the `timer` hub frame and the `last_timers` slot (P1)

**Why this exists:** A stage tablet that reloads mid-service must come back to the programme timer it was showing. `last_screen` holds one frame and the newest wins, so retaining a timer there would replace the verse and the next screen to join would be sent a clock and a blank wall. That is rule 43's trap 1 and the reason `stage_next` is excluded.

**Files:**
- Modify: `src-tauri/src/channels.rs` — `KioskHub`, `publish_timers`, `last_timers`, the `hello` reply (`:1703–1791`), `FRAME_VERDICTS` (`:3389`), `REHEARSAL_VERDICTS` (`:3504`)
- Test: `src-tauri/src/channels.rs` `mod tests`

- [ ] **Step 1: Write the failing tests:**
  - `a_stage_tablet_that_joins_mid_service_is_sent_the_programme_timers` — hello replays `last_timers`.
  - `a_timer_frame_is_never_retained_as_a_screen_frame` — `is_screen_frame(timer_json) == false`, and a published timer does not overwrite `last_screen`. **Write the json with `serde_json` and not by hand**: a BTreeMap serialises `{"kind":…}` in key order, and the first version of `is_screen_frame` matched nothing while looking exactly like the bug it fixed.
  - `a_timer_published_during_a_rehearsal_reaches_no_stage_tablet`, watching the hub itself and not `e2e::Wall` — `stage_next` was gated-in-theory and ungated-in-fact precisely because the Wall sees only Tauri events.
  - `the_hello_order_puts_the_screen_frame_last` — template, default_template, transition, **timers**, then the retained screen frame. A late stage tablet must paint the reading last and not flash a clock over it.
- [ ] **Step 2:** Implement `publish_timers` as a gated publisher: `rehearsing(app)` early-return, then `publish_kiosk`. Clone out of the registry and drop the lock before publishing (rule 2).
- [ ] **Step 3:** Add `("timer", false)` to `FRAME_VERDICTS` with the same shape of comment its neighbours carry, and `("publish_timers", true, "a rehearsal has no stage tablet waiting for a programme clock")` to `REHEARSAL_VERDICTS`. Both completeness tests must pass without being edited — if either needs editing to accept the new row, the row is in the wrong table.
- [ ] **Step 4:** Verify each test fails against the pre-fix code.

### Task 6: the stage page renders the programme timers (P1)

**Files:**
- Modify: `src/Stage.svelte` — a `timer` branch beside `stage_next` and `stage_alert` (`:406–445`)
- Modify: `src/lib/countdown.js` — nothing. **It already answers how long is left and it stays the only thing that does.**
- Test: `src/lib/` — a new `timers.test.js`, plus the existing `countdownwiring.test.js` unchanged

- [ ] **Step 1: Write the failing test:** a `{"kind":"timer", timers:[…]}` message paints one row per stage timer, each through `countdownRemainingMs`, with the existing 1000 ms tick (`Stage.svelte:521`) and no second interval.
- [ ] **Step 2:** Implement. Reuse `formatCountdown`; add no formatter.
- [ ] **Step 3:** A timer with no label renders digits alone without a collapsed or clipped box — wave 5 Track G makes label-less the dock's default and this page must already survive it.
- [ ] **Step 4:** Frontend suite green.

### Task 7: `ARCHITECTURE.md` §6 and the event register (P2)

- [ ] **Step 1:** If Track B adds a Tauri event (it should not — the stage path is a hub frame), it goes in `docs/ARCHITECTURE.md` §6 and `ipc.test.js` catches it both ways. Reproduce the set with the command CLAUDE.md gives rather than trusting any number in prose.
- [ ] **Step 2:** Record the new hub frame kind wherever `docs/ARCHITECTURE.md` enumerates them.

**PR 2 ends here.** Title: *a programme timer publishes to the stage and is replayed to a screen that joins late*.

---

# Track C — the §27 reversal

**Spec:** §3.4. **Off Track A.** Lands as PR 3.

### Task 8: scope decides what a panic control takes (P1)

**Why this exists:** The congregation guarantee must stay exactly as strong as it is today. It does, because the split is a property of the timer and the panic control asks nothing.

**Files:**
- Modify: `src-tauri/src/channels.rs` — `clear`, `black`
- Test: `src-tauri/src/e2e.rs`, `src-tauri/src/channels.rs`

- [ ] **Step 1:** Un-ignore (or write) `a_clear_takes_the_congregation_timer_and_leaves_the_programme_timer`, and add `a_blackout_answers_the_same_way_as_a_clear`. **Both controls, deliberately, in both branches** — `Stage.svelte:406` carries a comment saying in as many words that if Relay ever lets the stage survive a panic, it must survive both, not by one of them being forgotten. This is that day.
- [ ] **Step 2:** `clear` and `black` call `registry.stop_scope(Scope::Both)` and nothing else. **No branch asks which screen it is talking to.** A panic control that has to ask a question can fail to answer it.
- [ ] **Step 3:** Prove the congregation guarantee is unchanged: every existing panic test stays green, unedited, and `panic.test.js`'s fourteen stay green.
- [ ] **Step 4:** Verify the new tests fail if `stop_scope` is replaced by a full stop.

### Task 9: the new DECISIONS section, and the `alert` question settled (P1)

**Files:**
- Modify: `docs/DECISIONS.md` — a new section, the next number after 88, that explicitly supersedes §27's refusal
- Modify: `src/Stage.svelte` — the `alert` branch, with its answer written down
- Test: `src/lib/crossrefs.test.js` resolves the new citation

- [ ] **Step 1:** Write the new section with the three reasons §3.4 gives: §27 left the question open and named it a real product decision; §27's "no exceptions" was already not literally true in shipped code (`Stage.svelte`'s `svcStart` survives, on a stated ground); and the congregation guarantee is untouched. Cite §27 by number so `crossrefs.test.js` can resolve it.
- [ ] **Step 2: Settle `alert`.** The clear/black branch resets `visible, note, cdTo, cdFrom, cdPaused, next` and does **not** reset `alert`, and nothing in the tree records why. Decide it in the new section and make the code say so either way, because the current state is a silent third answer to the question that section exists to answer. Whichever way it goes, pin it with a test named after the decision.
- [ ] **Step 3:** `crossrefs.test.js` green: a citation that resolves to nothing is worse than an uncited claim.

### Task 10: the registers stop disagreeing (P2)

- [ ] **Step 1:** `docs/REBRAND.md` phase 7 says *"The button landed in wave 3"* about the rebrand's own wave 3, which is a different numbering from this spec's. Add the distinction where it is ambiguous, or leave it and say why. **Do not restate the wave 3 scope in a fifth place** — `RELAY_V1_AUDIT.md`, `RELAY_GAP.md`, `QA_HARNESS.md` §0 and the audit files are the four that already exist.
- [ ] **Step 2:** File anything this track found but did not fix in `docs/qa/RELAY_GAP.md`, taking the next free id, and keep `relaygap.test.js` green.

**PR 3 ends here.** Title: *a stage timer survives a panic control, and the congregation guarantee does not move*.

---

# Track D — the warning threshold becomes a setting

**Spec:** §3.5. **Off Track A** (it reads `Timer::warn_ms`, which Track A carries). Lands as PR 4.

`COUNTDOWN_WARN_MS = 60_000` is at `src/lib/layers.js:323` and `countdownWarning` at `:335–343`; the comment at `:332–333` says the rule is deliberately not a setting because *"the control belongs in the Settings pass, and a setting with nowhere to set it is worse than a sensible default."* This is the Settings pass.

### Task 11: `countdownWarning` takes an override (P1)

**Files:**
- Modify: `src/lib/layers.js`
- Test: `src/lib/layers.test.js:315`

- [ ] **Step 1: Write the failing tests:** an explicit `warnMs` wins; `null`/`undefined` falls back to today's rule exactly (the last minute, or the last tenth of a countdown shorter than ten minutes); and **`countdownTotalMs` returning null still yields no warning colour rather than a guessed span** (`countdown.js:105`, whose body already refuses a guess — keep it). A colour on at the wrong moment is worse than one on a minute early.
- [ ] **Step 2:** Implement as a third argument with a default, so every existing call site keeps working unedited.
- [ ] **Step 3:** Update the `:332` comment to say the setting now exists and where. A comment that describes a decision the code has since reversed is the failure `docs/` has been corrected for four times this month.

### Task 12: the default in Settings, the override per cue (P1)

**Files:**
- Modify: `src/lib/views/Settings.svelte` — a warning-threshold default under the countdown/timer grouping
- Modify: `src/lib/views/ServicePlanner.svelte` — `addCountdownCue` (`:287`, payload at `:289`) gains the per-cue field
- Modify: `src/lib/stores/capture.js` — carry `warn_ms` through `startTimer`/`startCountdown`
- Test: `src/lib/settingssections.test.js`, `src/lib/plan.test.js` or the planner's own suite

- [ ] **Step 1: Write the failing tests:** the Settings control **saves a preference something reads** — seven controls that saved a preference nothing read were closed on 2026-09-10 and that is the defect shape to avoid; and a cue carrying `warn_ms` fires a timer that warns at that figure.
- [ ] **Step 2:** Implement. The Settings default is the fallback when a timer carries no `warn_ms`; the per-cue value wins. Two authorities, ranked in one place, and the ranking reported — the same shape as `resolveTransition`.
- [ ] **Step 3:** No native `confirm`/`prompt` anywhere near it (rule 41).
- [ ] **Step 4:** Frontend suite green; `qa-inventory` reports no handlerless control.

### Task 13: the flash itself (P2)

- [ ] **Step 1:** Nothing to build — `cdwarn` keyframes exist on all three surfaces with a `prefers-reduced-motion` glow fallback each (`TemplateRender.svelte:1790–1796`, `Stage.svelte:842–843, :917`, `Dock.svelte:1531–1534`). **Assert that, do not rebuild it.** Add a test only if none pins the reduced-motion fallback.

**PR 4 ends here.** Title: *the warning threshold becomes a setting, per timer and per cue*.

---

# Track E — the eleventh site sweep, and the operator's surface

**Spec:** §3.6. **Off Track A.** Lands as PR 5.

Eleven sites consume the kind string. Missing one produces this repository's signature bug shape. The list, with the corrected citations:

| # | Site | Now at | If skipped |
|---|---|---|---|
| 1 | rule 38 match | `main.rs:639` | a kind that is not literally `"scripture"` disarms the passage; a timer is disarmed for free |
| 2 | `is_countdown` | `pipeline.rs:247–249`, guarding `:260` | **the pre-air validator refuses the frame as `Unsafe::Nothing`** |
| 3 | `cue_or_content_tpl` | `main.rs:3036` | no content look for the kind |
| 4 | `ContentTemplates` | `main.rs:3108` (five fields) | no row in the content-look UI |
| 5 | `kiosk_content_json` | `channels.rs:896` | native windows get the field, kiosk and OBS do not |
| 6 | `CONTENT_KINDS` | `layers.js:228` | the kind cannot be toggled per screen |
| 7 | `templateShows` | `layers.js:246` | **a template with an explicit `layout.shows` hides the kind silently** |
| 8 | the two kind filters | `Output.svelte:195` and `:296` (**not** `:180`/`:279`) | the filter is applied at one door and not its twin |
| 9 | the stage branch | `Stage.svelte:406–445` | the stage page ignores the message |
| 10 | `slideKey` | `TemplateRender.svelte:1161` | a kind varying in none of the five keyed fields does not re-key: no transition, no refit |
| 11 | `cues.type` / `cue_type` | free-form TEXT, `schema.sql:219`; the comment at `:89` enumerates five kinds | no migration needed; the comment is stale the moment a sixth is written |

### Task 14: the sweep, and the assertion rule 38 never had (P1)

- [ ] **Step 1: Write the failing test first:** a payload built with `..Default::default()` and **no** `kind` must not reach `broadcast_with_clock` — `is_some_and` is false for `None`, so such a payload does not disarm the passage, every current caller sets the field, and nothing enforces it. Add the assertion and the test that fails without it.
- [ ] **Step 2:** Walk all eleven sites. For each, either make the change or write down at the site why the kind needs nothing there. **A site skipped silently is indistinguishable from a site nobody looked at.**
- [ ] **Step 3:** Site 2 is the trap and the design's answer is to keep the `Both` wire form as the existing `countdown_*` fields. If anything in this wave changes that, `is_countdown` changes **in the same commit**.
- [ ] **Step 4:** Site 7 is the second trap: wave 2 writes `layout.shows` explicitly on every seeded template, which is what makes a new kind safe. Verify that is still true of the seed as it stands today rather than assuming it.
- [ ] **Step 5:** Update `schema.sql:89`'s comment if a sixth `cue_type` is written.

### Task 15: the operator can start and see a programme timer (P1)

**Why this exists:** A registry nothing can reach from a control is a command nobody calls, which CLAUDE.md counts as attack surface rather than a feature.

**Files:**
- Modify: `src/lib/Dock.svelte` — Quick tools / the transport row
- Modify: `src/lib/views/Live.svelte` — if the list belongs on the run surface, it goes where an operator already looks
- Test: `src/lib/timers.test.js`, and `scripts/qa-inventory.mjs` must report the control as reachable

- [ ] **Step 1: Write the failing test:** a rendered control starts a `Stage` timer, the list shows it, and stopping it takes that one and no other.
- [ ] **Step 2:** Implement. **The Controls card never scrolls** — an operator may never have to scroll to reach Clear screens. If the list cannot fit without scrolling that card, it goes somewhere else.
- [ ] **Step 3:** Colour semantics: amber means ON AIR and is never allowed to lie; cyan means a guess; amethyst means rehearsal. A programme timer is none of those — pick from the existing token set and say why in the component.
- [ ] **Step 4:** `node scripts/qa-inventory.mjs` — zero handlerless buttons, zero unnamed controls, and the new command traced to a rendered control.

### Task 16: the status line question (P2)

- [ ] **Step 1:** If this track adds any status text about a timer, ask what it says when the thing behind it is broken (rule 35). If the answer is the same as when everything is fine, it is not a status line.

**PR 5 ends here.** Title: *the eleventh content kind, swept at every door, with a control an operator can reach*.

---

# Track F — the browser-driven pass

**Runs last, against everything the five tracks landed.** Lands as PR 6.

Static instruments in this repository reported zero problems throughout the two passes that found the worst defects, and were right both times. A timer is a moving render on three surfaces; nothing else here can see it.

### Task 17: drive it (P1)

- [ ] **Step 1:** `npm run tauri dev`. Console in Chrome against the mock bridge; **the output and stage pages against the real backend on `:8032`** (`http://localhost:8032/output.html?channel=<id>&template_id=<n>`, and the stage page from the same server). `5032` is Vite and exists only in dev — never point a browser source at it.
- [ ] **Step 2:** The scenarios, each stated as what a church would see:
  - A congregation countdown runs; a verse is fired; the countdown is re-aimed. **The defect this wave exists to fix.**
  - A programme timer runs while verses, songs and notices come and go on the wall. It does not blink.
  - `Esc`, then `B`. The congregation screens go; the programme timer stays; the congregation timer does not come back.
  - A stage tablet reloads mid-service and comes back to the timer AND the reading, in that order, with no flash of a clock over the verse.
  - A timer crosses its warning threshold on all three surfaces, at the default and at an override, with `prefers-reduced-motion` on and off.
  - A rehearsal: nothing reaches the stage tablet.
- [ ] **Step 3:** Record what was measured against the real backend and what was measured against the mock bridge. **They are different claims.**

### Task 18: the evidence file (P1)

- [ ] **Step 1:** Write `docs/qa/audits/<DATE>-WAVE3-TIMERS.md` — frozen evidence. Closures go in a fix log, never in the findings.
- [ ] **Step 2:** File every finding in `docs/qa/RELAY_GAP.md`, taking the next free ids; keep `relaygap.test.js` and `crossrefs.test.js` green.
- [ ] **Step 3:** Re-measure `docs/qa/QA_HARNESS.md` §0 from the runners' own summary lines.
- [ ] **Step 4:** Update `CLAUDE.md` only where this wave made an existing sentence false. **Do not add a wave 3 narrative to it.**

**PR 6 ends here.** Title: *the wave 3 browser-driven pass*.

---

## What this wave does not claim

- It does not move the release decision. Word error rate is unmeasured in every language, neither platform has a code-signing certificate (RG-73), and nobody but the author has run a service.
- It does not touch detection, the router, or any threshold.
- It does not change the `Both` timer's wire form, so nothing downstream of `countdown_*` is re-tested by this wave rather than trusted.
- It does not close RG-139, RG-140 or RG-141 — the region-model rendering rows are wave 5's, and a timer rendered through a region-model template is still the same defect afterwards.
- It does not pre-empt wave 5 Track G: `label` stays a field, and whether the dock supplies one is that track's decision.

---

# Operator decisions taken on 2026-09-17, after the browser pass

The browser-driven pass (Track F) filed ten findings. Three of them the audit
refused to answer, correctly, because each is a product decision rather than a
bug: what a rehearsal's timers are, what a timer past zero says, and where the
way back onto a congregation screen lives. They were put to the operator and
answered. They are written here so the answer outlives the conversation that
produced it, and each names the row it closes.

## RG-150 — a rehearsal's timers belong to the rehearsal

**Ending a rehearsal stops every timer started inside it.** The registry is
emptied of them at the exit and the stage tablet is published the real set, which
is what it should have been seeing all along.

The alternative was to republish at the exit on the grounds that the timers were
real all along. That is the smaller change and it is the wrong product: it means
an operator who practises a twenty-minute sermon clock at ten o'clock finds it on
the preacher's tablet when the service starts, twenty minutes in, counting down
to a moment that has passed. A rehearsal is a sandbox in every other respect —
nothing it publishes reaches a screen — and a clock it started is not an
exception to that.

**Implementation note, not a licence to redesign:** the stop belongs at the
rehearsal exit and must be a property of the timer (when it was started), never a
question asked of a screen. `set_rehearsal(false)` stops them and publishes, in
that order, so the tablet is never shown a set that is about to change.

## RG-153 — a timer past zero counts up, in the warning colour

**A programme timer that reaches zero keeps counting, upward, wearing the warning
colour.** `+4:37` means the preacher is four and a half minutes over, which is
the question a stage monitor is actually asked. `formatRemaining`'s negative
branch already exists for this.

`done_msg` keeps having no reader on the stage rail, and that is now a decision
rather than an oversight: the words an operator typed are what a CONGREGATION
countdown says when it lands, and a programme timer is a different instrument. If
that is ever revisited it is a new row, not this one.

Not to be built while implementing this: a second arithmetic. `countdownRemainingMs`
stays the one reader of how long is left on every surface, and the upward figure
is its negative, formatted — not a new subtraction.

## RG-152 — the way back lives in the dock's Countdown block

**`show_timer` gets its control in the dock's Countdown block**, beside Start,
Pause, Reset, ±1 and Clear. When a `Both` timer exists and is not what is on the
screens, the block says so and offers to put it back.

Quick tools staying at three blocks is upheld. Track E tried a fourth and
`quicktools.test.js` refused it on a pinned operator instruction; that pin stands,
and this is the reason it did not need overturning. Live's programme band was the
other candidate and loses for one reason: it exists only on the Live workspace,
and the way back onto a congregation screen must be reachable from anywhere the
dock is, which is everywhere.

**What the control may not do.** It may not lie about what is on the screens —
the block's own state has to come from the same fact `adjust_countdown` uses to
decide whether to repaint, never from a second answer (rule 35). It may not
create a timer: Start is the one control that puts a countdown in front of people
for the first time, and `show_timer` refuses a `Stage`-scoped timer in words.

---

<!-- ===== was docs/archive/WAVE-DESIGNS.md, merged 2026-09-21, verbatim ===== -->

> **Archived 2026-09-21.** Historical work order. Wave 4 landed 2026-09-17 and was audited in `docs/qa/audits/DESIGN.md`; findings RG-161 to RG-165. Its 41 checkboxes were never ticked; the audit is the record. Task 8 ("wave 4 does NOT build screen targeting") was superseded when RG-161 shipped. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Wave 4 — Stage and Planner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This repository has never checked a box in a landed plan — completion is evidenced by commits and green suites, not by the boxes.

**Goal:** Finish the two surfaces wave 3 left half-answered. The stage page already carries a programme timer rail, and that rail cannot say a timer is nearly out, is finished, or is being held — so the one surface a preacher watches is the one surface where a clock can run out without changing colour. The Planner can already choose a cue's look and estimate a service's length, and it cannot bind a cue to a timer or make a slide readable before it is picked. Wave 4 closes those, drives both surfaces in a browser against the real backend, and writes down the one thing it deliberately does not build.

**Architecture:** Three code tracks and one verification pass, each landing as its own pull request off `feat/wave4-stage-planner`, which branches from `feat/wave3-timers` at `d9dcf51` — wave 3's tracks A, B and D merged. Tracks A (the stage) and B (the Planner) are independent of each other and run in parallel. Track C is documentation and files the descoped work. Track D is the browser-driven pass and runs last, against everything the other three landed.

**Tech Stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest + jsdom, `cargo test`, Chrome via the browser driver named in Track D.

**Spec:** `docs/archive/WAVE-DESIGNS.md` §4 (lines 726–801).

**Depends on:** Waves 0, 1 and 2, plus wave 3 tracks A, B and D, all verified present in this tree on 2026-09-16 — `src-tauri/src/timers.rs` (the registry, 11 unit tests), `channels.rs:1212` `timer_frame_json`, `channels.rs:1424` the timer frame's own retained slot, and the countdown warning threshold as a setting.

## Baseline, measured on this branch on 2026-09-16

Taken from each runner's own summary line, on `feat/wave4-stage-planner` at `d9dcf51`, with `npm run build` run first per RG-127.

| Count | Value | Command |
|---|---|---|
| Rust tests | **810 passed / 0 failed / 16 ignored** | `cd src-tauri && cargo test` |
| Frontend tests | **2301 passed, 149 files** | `npx vitest run` |
| Registered commands | **144** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |

Re-measure at the end of the wave; do not restate these figures anywhere but `docs/qa/QA_HARNESS.md` §0.

## Global Constraints

- **Do not touch any other worktree, branch or lock.** Wave 3's tracks C and E are in flight in their own worktrees (`feat/wave3-track-c`, `feat/wave3-track-e`) and are somebody else's work. Wave 4 branches from the wave 3 integration tip and reconciles at merge time, never by editing their trees.
- **Never commit to `main`.** Each track is a PR off `feat/wave4-stage-planner`.
- Each track works in its own worktree under `.claude/worktrees/`. Set `CARGO_TARGET_DIR=/Users/mrgee/WebstormProjects/relay/src-tauri/target` so a track reuses the built dependency graph rather than compiling whisper.cpp from source again; cargo takes a file lock, so parallel runs serialise rather than corrupt.
- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- Run `npm run build` before `cargo test` on a fresh tree (RG-127) — two `channels` tests fail on a bare 404 otherwise, because `dist/` is gitignored.
- Read test counts from the runner's own summary line, never a grep.
- Every new test verified to FAIL when its defect is reintroduced. Test the bug, not the fix.
- **Rule 2:** never hold a `Mutex` across `emit` or `broadcast_content`.
- **Rule 15 / 44:** no panic control gains a question it can fail to answer, and no overlay this wave adds may paint over `Clear screens` or take `Esc` without consuming it.
- **Rule 18 and the colour law:** amber means ON AIR, amethyst means rehearsal, cyan means a guess, grey means CUED. `src/lib/colourlaw.test.js` enforces it and this wave does not amend it. See the operator decision below.
- **Rule 24:** any new fire-path function is generic over `tauri::Runtime`.
- **Rule 35:** a status line that reads the same when the thing behind it is broken is not a status line. Every new figure on the stage rail must be able to say it has no answer.
- **Rule 36:** the check goes at the choke point, not at the call sites.
- **Rule 43:** a retained frame decides what a screen is showing, and `is_screen_frame` / `is_timer_frame` are substring matches because `serde_json`'s map is a BTreeMap. Nothing in this wave adds a matcher; if that changes, the first version looking correct is the failure mode.
- `countdown.js::countdownRemainingMs` stays the ONLY arithmetic for how long is left, on every surface.
- `src/lib/errors.js` is the ONE backend-error humaniser.
- Commit message bodies and PR bodies are normal English prose.

## Corrections to the design, verified against this tree on 2026-09-16

Every structural claim in §4 was checked by reading the cited code. The substance held in most places; the citations have drifted, three claims are false as written, and two facts the design did not know have appeared since.

1. **`docs/REBRAND.md` has its own wave numbering and it is not this one.** REBRAND's "fourth wave · T2" is the template shelf, landed 2026-09-14. Where a sentence could be read either way, say which numbering it means.

2. **The stage timer rail is already built.** The spec reads as though wave 4 builds it. Wave 3 Track B landed it: the frame at `channels.rs:1212-1227`, its own retained slot at `channels.rs:1424` kept out of `last_screen` so a clock cannot erase a retained verse, the hello replay at `channels.rs:1936-1953` ordered before the retained screen frame, the derivation at `Stage.svelte:260-263` and the markup at `Stage.svelte:658-668`, with seven tests in `src/lib/timers.test.js`. **Do not rebuild it.**

3. **`Stage.svelte:481` is not where "renders no template at all" lives.** That line is inside the wave-3 `timer` branch of the socket handler. The real statements are `Stage.svelte:413`, `:525-528` and `:805-806`, and the claim is independently pinned by `src/lib/r6-contracts.test.js:115`.

4. **The stage zone key holds a seventh value the spec does not mention.** `relay.stage.zones` also carries `figures: 'bottom' | 'beside'` (`Stage.svelte:168`, written `:186`, restored `:179`). A layout pass that moves the figures is changing persisted state, not only CSS.

5. **`Stage.svelte:114-134` records that REBRAND §5 asked for three zones to ship OFF and that they were deliberately flipped ON.** A layout pass that re-reads §5 will find a sentence the code has already overruled on purpose.

6. **There is no channel dimension on the wire, so §4.2's "screen choice per cue" is not a column plus a UI.** `OutputContent` carries no channel field; `channels.rs:1089` emits `output://content` app-wide and `:1090` publishes one string to every kiosk client; `src/Output.svelte:293` never compares its own channel. The only channel-addressed message that exists is `channel://retemplate` (`main.rs:6067`, filtered at `Output.svelte:330`), which proves addressing is possible and has never been done for content. See the operator decision below — this wave specifies it and does not build it.

7. **Cue colour is not merely unset; it is test-locked to one shared ink.** `src/lib/colourlaw.test.js:96-100` asserts every `TYPE[*].color` is `TAXONOMY_INK` by identity, so any per-kind ramp fails CI until that assertion is deliberately rewritten. `--v-col-scripture` **is** `--v-amber` (`app.css:222`, `:169`), which means ON AIR. `src/lib/plan.js:13-48` records the measurement that produced the rule and says in as many words not to reach for a promise colour because it is the one that reads well.

8. **The `--v-col-*` exemption cannot extend to the Dock.** The spec grants it to the Planner because the Planner is never on air. Quick tools lives in `Dock.svelte`, which renders on every workspace including Live (`App.svelte:756`), so the same argument does not transfer. `src/lib/quicktools.test.js:370` already asserts the block spends none of the colour law, with its own slice boundaries asserted at `:376-378` so it cannot pass vacuously.

9. **`ZOOMS` is half the pattern the spec asks for.** `TemplateEditor.svelte:759-761` (not `:772`) is a component-local `let` with no session key, no localStorage and no keyboard binding. The markup idiom at `:993-997` is worth copying; the persistence must be invented, and the idiom for that is `liveFullscreen` — declared in `EMPTY` with a comment (`session.js:56`), read as a reactive derivation (`Live.svelte:1389`), written through a named one-liner (`Live.svelte:1390`).

10. **`.sgrid` is at `Live.svelte:2512`, not `:2502`**, and the cell is inlined in `Live.svelte:1787-1826` rather than living in a component. `.sg-thumb` carries `container-type: inline-size` (`:2534-2540`) and is the container-query root `TemplateRender`'s `cqw` sizes resolve against — a size control must move the grid track floor, never the thumb.

11. **The deleted density segment is recorded in four places, not one** — `Live.svelte:1372-1388`, `:1771-1777`, `:2187-2191`, `:2237-2244` — and one of them is a live constraint: the row once wanted 135px inside 114px and clipped `Compact` to `Compa`. A size control landing in the same rail can clip the same way.

12. **The Quick tools colour claim is stated at six places, not four**: `Dock.svelte:1032-1033`, `:1084-1085`, `:1105-1107`, `:1446-1447`, `:1524-1526`, `:1552-1555`.

13. **`plan_items` has `section_title` and `duration_sec` and `docs/data/schema.sql:85-94` does not show them.** The live shape is `db/plans.rs:59-72`. Any schema work in this wave corrects the doc in the same commit.

14. **`duration_sec` is the running-time estimate and nothing else reads it.** `db/plans.rs:32-34`, written at `:266-274`, read only by `plan.js:256-289` for the estimate. No timer code mentions it. Do not overload it.

15. **`plan_item_id` on a `Timer` is write-only today.** It exists at `timers.rs:76`, is set only by `start_timer` (`main.rs:3054`), rides out through `TimerView`'s flatten, and nothing reads or filters on it. Binding a cue to a timer means giving it a reader.

16. **`plannerbuildonly.test.js` has a list that must not rot.** `TAKES_A_SCREEN` (`:62-73`) names ten commands and does not include `start_timer`, `show_timer`, `adjust_timer` or `stop_timer`. `show_timer` is a second door onto a wall (`main.rs:3152-3178`). Any wave-4 work that gives the Planner a timer control adds the right names to that list in the same commit, or the guarantee has a hole in it.

17. **Numbering.** The last DECISIONS section is 88 and the highest register id filed is RG-144, and wave 3's own tracks may take the next of each before this wave lands. `crossrefs.test.js` resolves every `DECISIONS §N` and every `RG-` id against the real documents, so a plan that cites a section it has not written turns the suite red. Write the citation in the same commit that writes the section, and take the next free id at that moment rather than reserving one here.

## Operator decisions carried into this wave

Taken 2026-09-16, after the three verification passes above.

- **Per-cue screen targeting is descoped to its own wave.** It is a wire change at the one choke point wave 3's tracks C and E are still editing, and it forces rule 43's retained frame to become per-channel, which is congregation-facing. Wave 4 writes the specification and files the gap; it builds none of it. **A stored-but-unread preference is explicitly not an acceptable middle** — that is the defect the 2026-09-10 pass closed on seven Settings controls.
- **The colour law stands and cue colour is not built.** `TAXONOMY_INK` stays the one ink, the kind stays carried by the words already printed beside every cue, and `colourlaw.test.js` is not amended. `--v-col-timer` is not created.
- **Quick tools gains no colour.** Grouping, if it is done at all, is carried by the caption and the neutral surface ramp the card already uses.
- **The stage half is Stage.svelte's own states plus a layout pass.** Wave 3 Track E owns the control that starts a programme timer and is in progress elsewhere; this wave does not build one, and touches neither `Live.svelte`'s timer surface nor `layers.js`.

---

# Track A — the stage says what its clocks are doing

**Spec:** §4.1. **Files are `src/Stage.svelte` and its tests only.** That file is not touched by wave 3 tracks C or E, which is what makes this track safe to run in parallel with them.

Today a programme row renders a label and digits and nothing else. `warn_ms` is published per row (`channels.rs:1224`) and explicitly not read (`Stage.svelte:256-259`). `countdown_done` is published (`channels.rs:1223`) and never rendered, so a finished programme timer shows `0:00`. A held row freezes, correctly, and says nothing about being held. The congregation figure row has a warning state (`.fig.warn`, `Stage.svelte:640`, CSS `:498-500`) and the preacher's own rail does not.

### Task 1: a programme row that is nearly out says so (P1)

**Why this exists:** The rail is the one surface in the building whose whole job is to tell a preacher how long is left, and it is the only timer surface with no warning state. Rule 35 in its general form: a figure that looks the same at four minutes and at ten seconds is not telling anybody anything.

**Files:**
- Modify: `src/Stage.svelte` — the `programme` derivation (`:260-263`) and the `.progrow` markup (`:658-668`) and CSS (`:923-950`)
- Test: `src/lib/timers.test.js`

- [ ] **Step 1: Write the failing tests.** A row whose `warn_ms` is crossed carries the warning class; a row with `warn_ms: null` never does, at any figure; the threshold is read from the frame and never re-derived on the page; a row that is held does not flash. Name each after the field failure it prevents.
- [ ] **Step 2: Implement.** Reuse the congregation warning treatment rather than inventing a second one — `.fig.warn` and its CSS are the precedent, and `countdownWarning` already exists as the shared rule. The colour is the one already used for a warning figure; no new token.
- [ ] **Step 3: Reduced motion is a cut**, exactly as the existing warning motion is. `countdownwarnmotion.test.js` is the file that already holds that guarantee for the other surfaces.
- [ ] **Step 4: Verify each test fails with the change reverted.**

### Task 2: a finished programme timer says its message, and a held one says it is held (P1)

**Why this exists:** `countdown_done` crosses the wire per row and is dropped on the floor. The congregation countdown substitutes `cdDone` at zero (`Stage.svelte:639`); the rail prints `0:00`, which reads as a clock that is still running and has just arrived. And `countdownIsPaused` is already imported into this file (`Stage.svelte:3`) and used only for the congregation figure at `:214`.

**Files:**
- Modify: `src/Stage.svelte`
- Test: `src/lib/timers.test.js`

- [ ] **Step 1: Write the failing tests.** A row at zero with a done message shows the message; a row at zero with an empty message shows `0:00` and not an empty box; a held row is marked held and keeps its frozen figure; releasing a hold takes the mark away.
- [ ] **Step 2: Implement** through the existing readers. `countdownRemainingMs` stays the only arithmetic and `countdownIsPaused` the only held test — a second copy of either is the defect `docs/REBRAND.md` phase 7 records as fixed once already.
- [ ] **Step 3:** The held mark is not a colour from the law. Held is a state the operator caused deliberately, and `Dock.svelte:1524-1526` already records which treatment that gets.

### Task 3: the rail is a zone, and it has a floor (P2)

**Why this exists:** `ZONES` (`Stage.svelte:150-157`) has six keys and the programme rail is not one of them, so a lobby screen running the stage page cannot switch the preacher's bookkeeping off. And `.tmr { flex: 1 1 0 }` inside a `max-height: 20%` row with `overflow: hidden` divides the width by the row count with no floor — the rail has never been rendered above about three rows.

**Files:**
- Modify: `src/Stage.svelte` — `ZONES`, `DEFAULT_ZONES`, `loadZones`, the zone panel (`:673-692`), `.progrow` CSS
- Test: `src/lib/stagezones.test.js`, `src/lib/timers.test.js`

- [ ] **Step 1: Write the failing tests.** The new zone appears in the panel and toggles; it defaults ON, like every other zone; a stored `relay.stage.zones` written before this key existed still loads and gets the default rather than `undefined`; a switched-off rail gives up its room, which is the guarantee `stagezones.test.js:627` already holds for the others; `figures` survives the migration untouched.
- [ ] **Step 2: Implement.** The zone key is content-free. Persisting is `saveZones` as it stands — one key, whole object, per device, `try/catch` both ways.
- [ ] **Step 3: Give the row a floor.** Decide a minimum legible width per timer and what happens past it — wrap, scroll or a count. Whichever is chosen, a row that cannot show every timer must say so rather than shrinking them all below reading size. **Measure it in Task 10 rather than asserting it here.**
- [ ] **Step 4:** Keep every existing stage-layout invariant green: nothing leaves the screen (`stagezones.test.js:220`), the reading takes what is left (`:231`), rows below are `flex-basis` and not `height` (`:236`), each row is its own container (`:263`).

### Task 4: the layout-quality pass, read before it is driven (P2)

- [ ] **Step 1:** Read `Stage.svelte`'s nine top-level regions against each other at 1024×768, 1280×800 and a phone in portrait, and list what a preacher cannot read. Do not change anything yet — Track D drives it, and a change made from reading and then confirmed by driving is a different claim from one made from driving.
- [ ] **Step 2:** Anything fixed here must keep `stagezones.test.js`'s size table (`:403`) green, or change it deliberately with its reason.

**PR 1 ends here.** Title: *the stage rail says what its clocks are doing*.

---

# Track B — the Planner binds a cue to a clock, and a slide can be read before it is picked

**Spec:** §4.2, minus the two descoped halves. **Runs in parallel with Track A.**

### Task 5: a cue can carry a timer, and Live is what starts it (P1)

**Why this exists:** `plan_item_id` exists on every `Timer` and nothing reads it. `duration_sec` is the running-time estimate and must not be overloaded. The Planner cannot fire to an output and that guarantee is load-bearing (`plannerbuildonly.test.js`), so the Planner stores the binding and the run surface acts on it.

**Files:**
- Modify: `src-tauri/src/db/plans.rs` — one additive column through the existing idempotent `add_plan_item_column`
- Modify: `docs/data/schema.sql` — correct it for this column **and** for `section_title` and `duration_sec`, which it already omits (correction 13)
- Modify: `src-tauri/src/main.rs` — the command that writes it; `src-tauri/src/timers.rs` — a reader keyed on `plan_item_id`
- Modify: `src/lib/views/ServicePlanner.svelte` (the inspector), `src/lib/stores/capture.js` (the wrapper)
- Test: `src-tauri/src/db/plans.rs`, `src/lib/views/plannerbuildonly.test.js`, a new frontend test file

- [ ] **Step 1: Write the failing tests first.** The migration is retryable and runs twice without error (rule 25). A cue with no binding is untouched by the migration and reads as unbound, not as zero. The Planner writes the binding and invokes nothing from `TAKES_A_SCREEN`. The registry can answer "which timer belongs to this cue" and returns nothing rather than a wrong timer when none does.
- [ ] **Step 2: The column.** Name it for what it is — a timer the cue asks for — so it can never be confused with `duration_sec`. State in the doc comment, beside the column, what each of the two means and why there are two.
- [ ] **Step 3: The Planner control** goes in the cue inspector beside the template select, and stores. It must not be able to start anything: `plannerbuildonly.test.js`'s behavioural sweep (`:239-269`) clicks every control in the component and asserts none of them reaches a screen-taking command.
- [ ] **Step 4: The list that must not rot.** Add `start_timer`, `show_timer`, `adjust_timer` and `stop_timer` to `TAKES_A_SCREEN` in the same commit, with the reason. `show_timer` is a second door onto a wall and the test's own anti-rot assertion (`:195-207`) will hold the names honest.
- [ ] **Step 5: Live starts it, at one call site.** When a bound cue goes on air, Live calls `startTimer({ …, planItemId })`. One call site, in the cue fire path, so wave 3 Track E's Live surface composes with it rather than racing it. If Track E has landed by the time this task runs, read its surface first and add nothing that duplicates it.
- [ ] **Step 6: Verify each test fails with the change reverted**, including the migration's retryability — drop the column and run it twice.

### Task 6: a slide can be read before it is picked (P2)

**Why this exists:** `.sgrid` is fixed at `minmax(158px, 1fr)` and `runsurface.test.js` names 158px in its own describe title as the size at which a cell must be unmistakable. An operator cannot make a slide bigger to read it, and the thing they are choosing between is the words on it.

**Files:**
- Modify: `src/lib/views/Live.svelte` — the view-controls wrapper (`:1771-1777`), the `.sgrid` rule (`:2512`)
- Modify: `src/lib/session.js` — one field in `EMPTY`
- Test: `src/lib/slidegridwiring.test.js` or a new file, `src/lib/session.test.js`

- [ ] **Step 1: Write the failing tests.** The control steps through its sizes and stops at each end; the chosen size survives a remount through the session; a size nobody has chosen is the current 158px behaviour exactly; the control is a real button with an accessible name.
- [ ] **Step 2: Implement the stepper** on the `ZOOMS` markup idiom (`TemplateEditor.svelte:993-997`) — two `r-iconbtn` steppers and a readout, each end disabled at its end — and the `liveFullscreen` persistence idiom (`session.js:56`, `Live.svelte:1389-1390`). The value is a number or an enum, never text.
- [ ] **Step 3: Move the grid track, never the thumb.** `.sg-thumb` is the container-query root; changing it resolves every `cqw` in the cell against the wrong box.
- [ ] **Step 4: Watch the rail it lands in.** `Live.svelte:2283` records a row that wanted 135px inside 114px and clipped a label to `Compa`. Measure the control's own width in the rail, at the narrowest supported window, in Track D.
- [ ] **Step 5: If the control is ever removed**, its session key goes in `migrateSession`'s drop list in the same commit (`session.js:66-82`). Write that instruction beside the field now.
- [ ] **Step 6:** Keep `slidegridwiring.test.js` and `runsurface.test.js` green. Both locate CSS declarations **by literal string match**, so reformatting a rule or inserting a class between two of them breaks them by spelling rather than by meaning.

### Task 7: the Planner's own legibility, and nothing that paints a promise (P2)

- [ ] **Step 1:** The kind stays carried by the words. If a cue is hard to scan, fix it with type, spacing, rule weight or the heading it already sits under.
- [ ] **Step 2:** `colourlaw.test.js` is not amended by this wave. If a change here makes it fail, the change is wrong.

**PR 2 ends here.** Title: *a cue can carry a clock, and a slide can be read before it is picked*.

---

# Track C — what wave 4 does not build, written down where it will be found

**Documentation only. No code.** Runs in parallel with A and B.

### Task 8: specify per-cue screen targeting (P1)

**Why this exists:** The spec says "add it, defaulting to all" about a facility that has no wire, no receiver filter and no per-channel retention. Leaving that sentence in a spec with no correction beside it is how the next wave under-scopes it again.

**Files:**
- Modify: `docs/archive/WAVE-DESIGNS.md` — a correction beside §4.2's paragraph, stating what it costs
- Modify: `docs/qa/RELAY_GAP.md` — file it at the next free id

- [ ] **Step 1: State the four pieces** it actually needs, each with the citation that proves it is missing: a channel set on the content (`channels.rs` `OutputContent`), a routed publish at the choke point (`channels.rs:1089-1090`), a receiver-side filter (`src/Output.svelte:293`, which today never compares its own channel — the precedent that it can is `channel://retemplate`, filtered at `:330`), and per-channel retention so a screen that joins mid-service is shown what is on **its** screen (rule 43, `channels.rs:1424`).
- [ ] **Step 2: State what must not move.** The panic controls stay global — a `clear` that has to ask which screens it is talking to is a `clear` that can fail (rule 15). `template_pinned`'s meaning on a screen a cue does not target must be answered before any code is written.
- [ ] **Step 3: File the gap** with its priority and the reason it was descoped, and keep `relaygap.test.js` and `crossrefs.test.js` green. Take the next free id at the moment of writing.
- [ ] **Step 4: Do not restate the scope in a fifth place.** `RELAY_V1_AUDIT.md`, `RELAY_GAP.md`, `QA_HARNESS.md` §0 and the audit files are the four registers that already exist.

### Task 9: record the two decisions this wave took (P2)

- [ ] **Step 1:** The colour law was re-examined against §4.2's proposal and kept. Record it where the law lives — a short note beside `plan.js:44-47`'s escape hatch is enough, and it must say that the proposal was considered and refused, not that nobody asked.
- [ ] **Step 2:** If wave 3 Track C has landed by now, check whether the scope-split panic control has a DECISIONS section yet. The behaviour is live at `channels.rs:1067-1072` and was undocumented at the time this plan was written. **Do not write that section for another track** — say so in the PR body if it is still missing, and leave it with its owner.

**PR 3 ends here.** Title: *the targeting wave 4 does not build, specified where the next wave will find it*.

---

# Track D — the browser-driven pass

**Runs last, against everything the three tracks landed.** Lands as PR 4.

Static instruments in this repository reported zero problems throughout the two passes that found the worst defects, and were right both times. Both of wave 4's surfaces are moving renders; nothing else here can see them.

### Task 10: drive it (P1)

**The harness is not in this tree and that is a real cost to budget for.** `docs/qa/QA_HARNESS.md:187` says there is no browser driver, no Playwright and no `@testing-library/svelte`, and `src/lib/__auditbridge.js` — the console bridge the wave 2 pass used — was deliberately never committed (`docs/qa/audits/DESIGN.md:52-62`). **The stage page needs none of it**: `:8032` is served by Relay itself and joins the real hub on `:8031`, so it can be driven with the browser driver alone. Only the console needs the bridge rebuilt from that audit's prose.

- [ ] **Step 1:** `npm run tauri dev`. The stage page and the output page against the real backend on `:8032`. `5032` is Vite and exists only in dev — never point a browser source at it.
- [ ] **Step 2: The scenarios, each stated as what a room would see.**
  - A programme timer crosses its warning threshold on the stage rail, with `prefers-reduced-motion` on and off.
  - A programme timer reaches zero with a done message, and with none.
  - A programme timer is held and released; the rail says which it is.
  - The programme rail is switched off and the reading takes the room back.
  - Six programme timers at once, at 1024×768 and on a phone in portrait: measure whether the row is still readable and what the floor did.
  - A stage tablet reloads mid-service and comes back to the timers and the reading, in that order, with no flash of a clock over the verse.
  - The slide sizer at each step, at the narrowest supported window, with its own width measured in the rail that once clipped `Compact` to `Compa`.
  - A bound cue goes on air and its timer appears on the preacher's rail; the same cue in a rehearsal reaches the stage tablet not at all.
- [ ] **Step 3: Measure, do not eyeball.** Geometry from `getBoundingClientRect`; clipping from a `Range` over the text node against the nearest ancestor whose computed `overflow` is not `visible`; an overlay proven with `elementFromPoint` over the control's centre.
- [ ] **Step 4: Record which claims were measured against the real backend and which against a mock.** They are different claims and the audit says which is which.

### Task 11: the evidence file (P1)

- [ ] **Step 1:** Write `docs/qa/audits/<DATE>-WAVE4-STAGE-PLANNER.md` — frozen evidence and findings only. Closures go in a fix log, never in the findings.
- [ ] **Step 2:** File every finding in `docs/qa/RELAY_GAP.md` at the next free ids; keep `relaygap.test.js` and `crossrefs.test.js` green.
- [ ] **Step 3:** Re-measure `docs/qa/QA_HARNESS.md` §0 from the runners' own summary lines.
- [ ] **Step 4:** Update `CLAUDE.md` only where this wave made an existing sentence false. **Do not add a wave 4 narrative to it.**

**PR 4 ends here.** Title: *the wave 4 browser-driven pass*.

---

## What this wave does not claim

- It does not move the release decision. Word error rate is unmeasured in every language, neither platform has a code-signing certificate (RG-73), and nobody but the author has run a service.
- It does not touch detection, the router, or any threshold.
- It does not build per-cue screen targeting, and says so in the spec and the register rather than in a commit message.
- It does not colour a taxonomy, in the Planner or in the Dock.
- It does not build the control that starts a programme timer. That is wave 3 Track E's, and it was in progress elsewhere while this plan was written.
- It does not change the congregation countdown's wire form, so nothing downstream of `countdown_*` is re-tested by this wave rather than trusted.

---

<!-- ===== was docs/archive/WAVE-DESIGNS.md, merged 2026-09-21, verbatim ===== -->

> **Archived 2026-09-21.** Historical work order. Wave 5 landed 2026-09-16 and was audited in `docs/qa/audits/DESIGN.md`; findings RG-156 to RG-160. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Wave 5 — the shelf, the names and the seal: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` or `superpowers:executing-plans` to work this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking. This repository has never checked a box in a landed plan — completion is evidenced by commits and green suites, not by boxes.

**Goal:** answer the four questions the design opens with. What a church finds on the shelf becomes forty layer-model templates with a stable identity the legacy conversion cannot destroy; one concept carries one name everywhere; the operator console's own skin stops at the glass; and four controls that lie about what they do stop lying.

**Spec:** `docs/archive/WAVE-DESIGNS.md`, approved by the operator on 2026-09-16 before any code was written.

**Architecture:** seven code tracks and one browser-driven verification pass. Each lands as its own branch merged into `feat/wave5-impl`, which branches from `feat/wave5-shelf-names-seal` (waves 0, 1 and 2 plus the wave 5 design doc). Wave 3 is in flight on `feat/wave3-timers` in other worktrees and **is not touched by this wave**: wave 3 branches from the same base, keeps `countdown_*` as the wire form, and every template this wave seeds survives it unchanged.

**Tech stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest + jsdom, `cargo test`, Chrome through the browser audit harness.

## Track map

| Track | Branch | Scope | Depends on |
|---|---|---|---|
| A | `feat/wave5-track-a` | the shelf becomes forty; `seed_key` + `edited_at`; retirement that survives conversion | — |
| B+F+H | `feat/wave5-track-b` | Create New Template stops creating one; the three editor layout defects; the "Used for" block leaves the editor | — |
| C | `feat/wave5-track-c` | `output_channels.role`; Live stops guessing the main screen; `stage_message` binding filtered at the receiver | — |
| D | `feat/wave5-track-d` | the name register (Stage Message · Stage Note · Up Next) and `names.test.js` | — |
| E | `feat/wave5-track-e` | the seal: template data, renderer fallback, and `app.css` shipping to the congregation | — |
| G | `feat/wave5-track-g` | the timer runs bare; the Planner gains the fields it never had | — |
| I | `feat/wave5-track-i` | starter content on a fresh install; the tripwire asserts identity instead of zero | **A** |
| J | `feat/wave5-track-j` | the browser-driven pass against the real backend | all of the above |

Tracks A, B+F+H, C, D, E and G are independent of each other and run in parallel. Track I lands after A, because its example plan pins a template by `seed_key`. Track J runs last.

## Global constraints

Every one of these is a rule this repository learned the hard way. A track that appears to need an exception has found a design problem, not a rule problem.

- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before every commit; both suites green before a track is offered for merge.
- Run `npm run build` before `cargo test` on a fresh tree (RG-127): two `channels` tests fail on a bare 404 otherwise, because `dist/` is gitignored.
- Read test counts from **the runner's own summary line**, never a grep. Values live only in `docs/qa/QA_HARNESS.md` §0. Re-measure; do not restate.
- Every new test is verified to **fail when its defect is reintroduced**. Test the bug, not the fix.
- **Rule 10, 28, 30, 34:** no threshold moves in this wave; nothing here touches `detection.rs` or `router.rs`.
- **Rule 15 and 44:** nothing added here may paint over `Clear screens`, and any overlay that disarms `Esc` consumes it and gives it back on the second press.
- **Rule 41:** no native `confirm()` / `alert()` / `prompt()`. The unsaved-draft guard in Track B is an in-app two-step.
- **Rule 36:** `pipeline::Fire` stays the only way to build output; `broadcast_with_clock` stays the only caller of `channels::broadcast_content`; the pre-air validator stays where it is.
- **Rule 37 and 42:** the 45% legibility floor stays a ratio of the designer's own size, `TemplateRender` keeps shrinking rather than blanking, and the two-try refit bound stays.
- **Rule 43:** no frame introduced here is retained into `last_screen`, and `is_screen_frame` is a `contains`, not a `starts_with`.
- **Rule 25:** every migration is retryable and idempotent — a second run is a no-op and a third is identical to the second.
- **Rule 18:** a congregation screen never inherits the console's ON AIR amber.
- **DECISIONS §29, §35, §70, §78 are not reversed.** A template's role stays derived from what it renders; the kiosk hub still records nothing about who connected.
- `src/lib/errors.js` is the ONE backend-error humaniser. Never render a raw Rust `Err` string to a volunteer.
- Commit messages, PR bodies and documentation are normal English prose, never caveman.
- Never commit to `main`. Each track is a branch merged into `feat/wave5-impl`.

## Corrections to the design, verified against the working tree on 2026-09-16

Every structural claim cited below was checked by reading the code in this tree. None was substantively wrong; three citations have drifted and one is incomplete in a way that changes a track's scope.

1. **The template components moved.** They are `src/lib/views/templates/TemplateGallery.svelte` and `src/lib/views/templates/TemplateEditor.svelte`, not `src/lib/views/`. `newFrom` is `TemplateGallery.svelte:324` (the save is at `:329`). `armedDelete` renders at `TemplateEditor.svelte:1140-1144`.
2. **"Used for" has THREE writers, not two.** `toggleUsedFor` exists at `TemplateEditor.svelte:535` *and* at `TemplateGallery.svelte:399` (rendered at `:780`), beside the authoritative matrix in `Channels.svelte`. Track H's design text names only the editor. **Decide the gallery's copy explicitly and say which way you went in the commit body** — do not delete it silently, and do not leave it unexamined. The editor's block goes, per the design; the gallery's is a finding this plan is filing, not a licence to widen scope by reflex.
3. **Live's guess is where the design says it is:** `Live.svelte:1357`, `channels.find((c) => c.render_target === 'native_window') ?? channels[0] ?? null`.
4. **The dock's hard-coded label is `Dock.svelte:625`**, and the defaults behind it are `capture.js:1431-1432` (`label = 'Service begins in'`, `doneMsg = 'Welcome'`). The Planner's copy is `ServicePlanner.svelte:289`, inside `addCountdownCue`, which already seeds `setPlanDuration(added.id, m * 60)` — leave that alone.
5. **The seed tripwire is real and load-bearing:** `db/mod.rs:662` carries the "NOTHING SEEDS DEMO CONTENT HERE" comment, and `preset_template_count()` is `db/templates.rs:708`, asserted at `db/mod.rs:1020` and `:1053` and `db/templates.rs:1473`. Track I reverses the comment's rule and Track A moves the count; both update every assertion in the same commit that changes the thing it asserts, never afterwards.
6. **Numbering:** the last `DECISIONS.md` section is 88 and the highest register id filed is RG-142. The starter-content reversal and the channel-role decision each take the next free section number; anything filed takes the next free RG id. **Write the citation in the same commit that writes the section** — `crossrefs.test.js` resolves every `DECISIONS §N` and every `RG-` id against the real documents, so a citation written ahead of its section turns the suite red.
7. **RG-139 is not closed by this wave and must not be claimed as closed.** Track A seeds six templates with a `countdown`-bound layer, which widens that row's reach. Track A says so in the register when it lands.

## Track A — the shelf becomes forty

- [ ] **Step 1:** add `templates.seed_key TEXT` and `templates.edited_at TEXT` in one retryable migration that back-fills `seed_key` by name against the OLD seed lists, declining on a name collision.
- [ ] **Step 2:** `upsert_template` never rewrites `seed_key`, and stamps `edited_at` on any save that does not come from the seeder.
- [ ] **Step 3:** retirement becomes `seed_key` in the retired set AND `edited_at IS NULL` AND unreferenced through the four doors `ensure_retired_presets_are_gone` already checks. `retired_presets.json` and its byte comparison stay, additively, for installs that never opened Templates.
- [ ] **Step 4:** seed the forty — eight roles × five, every one layer-model, every one declaring `layout.shows` explicitly, every one clearing 7:1 body contrast against its own ground, every name unique and prefixed `Role · Name`.
- [ ] **Step 5:** stop seeding the region-model rows. A test asserts **no seeded row is region-model**, which is what keeps RG-141 closed against a later tidy-up.
- [ ] **Step 6:** evidence — `preset_template_count()` returns 40 with the assertion beside it; `the_bare_fixture_is_a_first_launch_and_nothing_more` updated in the same commit; `templateKind.test.js` names the derived role of all forty; the contrast instrument walks all forty; a retirement fixture that **converts before it retires**.
- [ ] **Step 7:** file RG-139's widened reach in `docs/qa/RELAY_GAP.md`.

## Track B+F+H — the editor stops lying

- [ ] **Step 1 (B):** `newFrom` builds the starter in memory and dispatches a draft `{ id: null, name, layout, style }`. The editor renders a draft as it renders a saved row, with an unsaved header, a Save that inserts, and a Discard that writes nothing.
- [ ] **Step 2 (B):** leaving a dirty draft raises an in-app two-step, never `confirm()`. Duplicate and import keep today's behaviour.
- [ ] **Step 3 (B):** a test mounts the gallery, chooses a starter, closes without saving and asserts `save_template` was never called — written to fail against today's code first.
- [ ] **Step 4 (F):** the armed layer delete gets styling and room. `Sure?` must not land in a 20px box inside a clipped, horizontally scrolling list.
- [ ] **Step 5 (F):** `.te-objtabs` stops growing unbounded inside `.te-pane{overflow:hidden}`; the add-layer menu's absolute positioning is measured and, if it clips, moved to `position:fixed` the way `TemplateGallery` already did.
- [ ] **Step 6 (H):** delete `toggleUsedFor` and the "Used for" block from the editor. **Keep** the row beneath it — `layout.shows`, a per-screen allow-list, is a different feature. Rule on the gallery's third copy explicitly (correction 2).
- [ ] **Step 7:** nothing in the engine changes: `tpl_{kind}`, `set_content_template`, `ContentTemplates`, `cue_or_content_tpl` and `resolveOutputTemplate` are untouched.

## Track C — channel roles and the door Stage Message needs

- [ ] **Step 1:** `output_channels.role TEXT CHECK (role IS NULL OR role IN ('main','stage'))`, seeded `Main screen` → `main`, `Stage display` → `stage`, others NULL. At most one `main`, enforced in the helper so the error is readable.
- [ ] **Step 2:** Live reads the role instead of guessing, and the programme pane names the screen it is rendering. Outputs gains the picker.
- [ ] **Step 3:** a `stage_message` binding in `layers.js`, labelled **Stage Message**.
- [ ] **Step 4:** `Output.svelte` accepts a `stage_alert` frame **only when its own channel's role is `stage`**. The value never rides on `OutputContent`; it stays its own frame kind in renderer state. `FRAME_VERDICTS` keeps `("stage_alert", false)`. The rehearsal gate is unchanged.
- [ ] **Step 5:** `e2e::r5_a_word_to_the_preacher_reaches_no_congregation_channel` gains a sibling driving a channel whose role is **not** stage, asserting the frame paints nothing.
- [ ] **Step 6:** settle and write down whether `alert` surviving a panic control on the stage page is intended (`Stage.svelte:406-438` resets six fields and not that one). The behaviour need not change; it must stop being an accident.

## Track D — the names

- [ ] **Step 1:** one name per concept — **Stage Message** (`stage_alert`), **Stage Note** (`stage_note`), **Up Next** (`stage_next` / `next_*`) — applied to the dock card heading and aria, the binding labels, the Planner field, the stage page zones and any status line.
- [ ] **Step 2:** `names.test.js` holds, per concept, the single user-visible string and the files allowed to contain it, failing when a second label for the same concept appears anywhere in `src/`.
- [ ] **Step 3:** the scanner reads every `src/` file rather than a hand-written list, **plus a test that the scanner can still see a known instance** — a scanner that quietly narrows passes everything, and this repository has had that exact failure twice.
- [ ] **Step 4:** neither of the first two concepts becomes the other. One is persisted per cue, one is a live instruction.

## Track E — the seal

- [ ] **Step 1:** seeded `style_json` stops storing `var(--f-serif)` / `var(--f-display)` / `var(--f-body)`. Templates name real families. (Coordinate with Track A, which authors the forty: A owns the new rows, E owns any legacy row and the rule that keeps it true.)
- [ ] **Step 2:** `TemplateRender`'s `--accent` fallback stops being `var(--v-amber)` and becomes a template-side default.
- [ ] **Step 3:** the legacy unscoped rules in `app.css` (`.prev-main .verse`, `.prev-stage .verse`, `.prev-stream .lower-third`, `.prev-lobby .verse`, `.tmpl-row.active`, `.toggle.on`) stop reaching `output.html` and `stage.html`. The token block and the self-hosted fonts stay shared — the seal is against rules and template data, not against the palette.
- [ ] **Step 4:** a test that fails when a console-only rule or an app-chrome token reappears in template data.

## Track G — the timer runs bare

- [ ] **Step 1:** the dock's Start passes no label and no done-message. Digits alone.
- [ ] **Step 2:** `capture.js`'s defaults become empty strings, so a caller that omits them gets a bare timer rather than a surprise.
- [ ] **Step 3:** the Planner keeps both fields and gains the interface it never had — a cue that wants words beside the clock says so, and fires them through Live exactly as today.
- [ ] **Step 4:** `countdown.js::countdownRemainingMs` stays the only arithmetic; `countdown_from` keeps being written; `adjust_countdown` still cannot create a countdown; none of the sixteen countdown tests changes shape.

## Track I — starter content on a fresh install (after A)

- [ ] **Step 1:** reverse `db/mod.rs:662` deliberately, with a numbered `DECISIONS.md` section of its own, written in the same commit as its citation.
- [ ] **Step 2:** seed announcements with the operator-facing title separated from the words that reach the room (REBRAND §10).
- [ ] **Step 3:** the 34 files at `src/backgrounds/` become `media_assets` rows, reachable from the Library and from a template's media layer.
- [ ] **Step 4:** one example service plan carrying a countdown cue with its own label and duration.
- [ ] **Step 5:** `the_bare_fixture_is_a_first_launch_and_nothing_more` keeps its name and its job, and asserts the starter set **by identity** — not zero, not a range. `demo.rs` is untouched.

## Track J — the browser-driven pass

- [ ] **Step 1:** render the console against the mock bridge and the output and stage pages against the real backend, per `docs/qa/QA_HARNESS.md`.
- [ ] **Step 2:** drive all forty templates at 1920×1080 and at a phone width, watching for the rule 42 first-verse defect and for anything a static instrument cannot see.
- [ ] **Step 3:** confirm the three editor layout fixes in a real layout engine, not in argument.
- [ ] **Step 4:** file what is found in `docs/qa/RELAY_GAP.md` with fresh RG ids, and write the pass up as a frozen audit under `docs/qa/audits/`.

## What this wave does not claim

- It does not move the release decision. Word error rate remains unmeasured in every language, neither platform has a code-signing certificate (RG-73), and nobody but the author has run a service.
- It does not touch detection, the router, or any threshold.
- It does not close RG-139.
- It does not unpark NDI and it adds no SDI integration.
- It does not reverse DECISIONS §29, §35, §70 or §78.

---

<!-- ===== was docs/archive/WAVE-DESIGNS.md, merged 2026-09-21, verbatim ===== -->

> **Archived 2026-09-21.** Historical. Wave 5 (shelf, names, seal) landed on 2026-09-16 and was audited in `docs/qa/audits/DESIGN.md`; rulings are in `docs/DECISIONS.md` §87 to §90; the name register is `src/lib/names.test.js`. Its statement that waves 3 and 4 had not landed was true that day and is not now. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Design — the shelf, the names and the seal (Wave 5)

**Status:** approved by the operator on 2026-09-16, before any code was written.
**Predecessor:** `docs/archive/WAVE-DESIGNS.md`,
whose waves 0, 1 and 2 have landed and whose waves 3 (Timers) and 4 (Stage and
Planner) have not.

**This wave runs BEFORE waves 3 and 4, not after.** Nothing in it needs the timer
registry, and wave 3 keeps `countdown_*` as the wire form, so the templates this
wave seeds survive it unchanged. Where this design amends the predecessor, the
amendment is written down in §0.3 rather than left to be discovered.

---

## What prompted this

Ten requests from the operator, taken in one sitting after wave 2 was assembled
and driven in a browser. They are not ten unrelated items; they are four
questions wearing ten coats.

1. **What does a church actually find on the shelf?** Thirty-seven seeded
   templates across two rendering models, six of which have "High Visibility" in
   the name, and three open register rows (RG-139, RG-140, RG-141) saying that
   what a fresh install renders is not what the designer drew.
2. **Is a thing called the same thing everywhere?** One concept carries six
   labels today. `stage_alert` is "Word to the preacher"; `stage_note` is "Stage
   note" in the Planner, "Operator note (monitors only)" in the template editor
   and "Note" on the stage page.
3. **Does the app's own skin stop at the glass?** It does not. Seeded template
   styles store `var(--f-serif)`; `TemplateRender` falls back to the console's
   amber; `app.css` ships its legacy unscoped rules into `output.html`.
4. **Does a control do what it says?** "Create new template" creates one before
   you have saved anything. Arming a layer delete paints the word `Sure?` into a
   fixed 20px box. Running a timer silently attaches the words "Service begins
   in", which no interface can change.

---

## 0.1 The four decisions this design rests on

Each was put to the operator as a question with its cost stated, and answered.

**One — the shelf is rebuilt whole, at forty.** All thirty-seven seeded rows go.
Forty arrive: eight roles × five. Scripture · Song · Media · Announcement · Timer
· Scrolling Lower Third · SuperSource · Stage. Every one is layer-model, unique,
and clears one contrast floor rather than concentrating legibility in a single
family.

**Two — a seeded row gains an identity the conversion cannot destroy.** RG-142
measured the previous retirement at **0 of 21** against a copy of a real
database, because it matched on bytes and `TemplateGallery.upgradeLegacyToLayers`
had already rewritten them. "Delete all thirty-seven" cannot be built that way.
Two columns are added: `templates.seed_key` and `templates.edited_at`.

**Three — an output channel gains an explicit role.** "Main screen" is currently
a frontend guess (`Live.svelte:1357`, the first channel whose `render_target` is
`native_window`). Rename or delete that channel and Live's programme pane falls
through to `channels[0]` in silence. A role column makes it a setting — and it is
also the door that makes Stage Message safe in a template, which §3 explains.

**Four — a fresh install ships starter content.** This reverses
`db/mod.rs:662`'s *"NOTHING SEEDS DEMO CONTENT HERE, and nothing ever may."* The
reversal is deliberate, is the operator's to take, and gets a numbered DECISIONS
section of its own when it lands.

## 0.2 Constraints every track respects

These are not restated per track. A track that appears to need an exception has
found a design problem, not a rule problem.

- **`pipeline::Fire` stays the only way to build output**, `broadcast_with_clock`
  stays the only caller of `channels::broadcast_content` (rule 36), and the
  pre-air validator stays where it is.
- **Rule 10 is untouched.** Nothing here moves a threshold, adds an auto-fire
  route, or touches `detection.rs` or `router.rs`.
- **Rules 37 and 42 are untouched.** The 45% legibility floor stays a ratio of the
  designer's own size; `TemplateRender` keeps shrinking and showing rather than
  blanking; the two-try refit bound stays.
- **Rule 41**: no native `confirm()` / `alert()` / `prompt()`. The unsaved-template
  guard in §2 is an in-app two-step, not a browser dialog.
- **Rule 43**: a screen joining mid-service is shown what is on the screens. No
  new frame introduced here may be retained into `last_screen`, and
  `is_screen_frame` is a `contains`, not a `starts_with`.
- **Rule 15 and rule 44**: nothing added here may paint over `Clear screens` or
  take `Esc` without giving it back.
- **DECISIONS §29 and §70 are not reversed.** The resolution order — transparency
  law, pinned cue template, the screen's own, the content look, the configured
  default — is unchanged by every track in this wave.
- **DECISIONS §35 is not reversed.** The kiosk hub still records nothing about who
  connected. §3's per-channel filter therefore lives at the receiver.
- **DECISIONS §78 is not reversed.** A template's role stays derived from what it
  renders. The forty names carry a kind prefix for findability; nothing reads the
  prefix to decide a role.

## 0.3 Where this amends the predecessor spec

Three statements in `2026-09-15-timers-templates-stage-design.md` no longer
describe what is being built. They are amended here and the predecessor is left
as the record of what was true when it was written.

| Predecessor | Said | Now |
|---|---|---|
| §2.4 | "The seed becomes twenty-five", five families × five kinds | Forty, eight roles × five. The family idea survives only as a visual through-line, not as a seeding rule |
| §2.5 | "Both rows are real features and neither is deleted" | The **"Used for"** row is deleted from the template editor. The per-screen `layout.shows` switch list stays. `Channels.svelte` keeps the one authoritative content-look matrix, so the feature is not deleted — its second writer is |
| §3 (Wave 3) | The timer design keeps `label` as a field on the timer | Still true, and the DOCK stops supplying one. §7 below |

---

# Track A — the shelf becomes forty

## A.1 Why the count is not the point

A fresh install currently holds thirty-seven templates from three sources:
`builtin_templates()` (5, region model), `theme_templates()` (25, four of them
layer model), and `shelf_templates.json` (7, layer model). Thirty of the
thirty-seven are region-model, which is the fact underneath all three open
rendering rows:

- **RG-141** — a fresh install's countdown takes the region branch and paints a
  `312 × 43` blob: 30.3px digits against 192px designed.
- **RG-140** — `Classic · Announcement` paints a footer ticker before the
  Templates workspace is opened and a mid-screen crawl after, switched by a
  silent one-way conversion.
- **RG-139** — a running countdown makes every other text layer paint its
  declared size instead of its fitted one, clipping 91px of a label on
  `Stage · Large type`.

RG-140 and RG-141 both name the same repair and call it a decision rather than a
patch: **decide whether a seeded row's region form or its converted form is the
product.** This track answers that. The converted form is the product, and the
region form stops shipping. That closes RG-141 and RG-140 at the source — not by
fixing the region fitter, but by no longer seeding anything that reaches it.

RG-139 is **not** closed by this track and must not be claimed as closed. It is a
fit-loop defect on the layer path, reproducible on any template with a
`countdown`-bound layer, and this track seeds five of those on purpose (`Timer ·
*`) plus one more (`Stage · Rail`). **Track A therefore widens RG-139's blast
radius and must say so in the register when it lands.** Either RG-139 is fixed
first, or the Timer group ships with the mitigation the row already names. This
is called out here rather than discovered during implementation.

## A.2 The identity problem, and the two columns

`ensure_retired_presets_are_gone` compares `region_config_json` and `style_json`
against frozen bytes. Two independent rewrites defeat it, either fatal alone:
`upgradeLegacyToLayers` converts and **saves** on mount, with layer ids from
`newId()` (which folds in `performance.now()`); and `style_json` returns through
`serde_json`'s BTreeMap alphabetically ordered against the seed's hand-written
key order. Measured result, against a real pre-wave database: 31 in, 57 out,
**0 of 21 retired**.

RG-142's own closing sentence is the design constraint: *"Anything surviving the
conversion needs an identity the conversion preserves, not a byte comparison."*

**Two columns on `templates`:**

```sql
seed_key   TEXT,     -- set by the seeder, never by an operator. NULL = hand-made.
edited_at  TEXT      -- stamped when a save does not come from the seeder.
```

- `seed_key` is a stable slug (`scripture.dayspring`, `timer.monolith`). It is
  written once, at insert, and `upsert_template` never changes it — an operator
  editing a seeded row keeps the key and gains an `edited_at`.
- The migration that installs it back-fills by name against the OLD seed lists,
  which is the one moment name-matching is correct: it runs before anything in
  this wave has renamed a row, and a name collision at that instant is exactly
  the case RG-142 says the migration should decline.
- **Retirement becomes**: delete rows whose `seed_key` is in the retired set AND
  whose `edited_at IS NULL` AND which nothing points at through the four doors
  `ensure_retired_presets_are_gone` already checks (content look ×5, configured
  default, `output_channels.template_id`, `plan_items.template_id`).
- A row the operator edited survives, which was always the right half of the old
  rule. A row they never touched goes, which is the half that never worked.

**Idempotence and retryability, rule 25.** The migration adds columns and
back-fills in one `unchecked_transaction`; the scratch-table hazard does not
arise because nothing is rebuilt, but the rollback path is still explicit and the
test asserts a second run is a no-op and a third is identical to the second.

**`retired_presets.json` is superseded, not deleted.** It stays as the frozen
record of the twenty-one, and its byte comparison stays for installs that never
opened Templates — where it is the only thing that works, and where it is
correct. The `seed_key` path is additive.

## A.3 The forty

Eight roles, five each. Names are `Role · Name`, unique across all forty. The
prefix is for finding, not for deciding: **`templateKind` still derives the role
from what the template renders**, DECISIONS §78 unchanged, and
`templateKind.test.js` names the role each of the forty lands on so a seed that
drifts is caught by a failing test rather than quietly becoming Custom.

Every one of the forty:

- is **layer model**;
- declares **`layout.shows` explicitly** (the predecessor spec's §3.6 trap 7 — a
  template with a narrow `shows` list drops content at `Output.svelte:195`
  *before* `resolveOutputTemplate` ever runs);
- clears **7:1 body-text contrast against its own ground**, measured by the same
  helper `legibility.test.js` uses today. This is the "high visibility as a
  floor, not a family" decision: the accessibility family becomes the accessibility
  *standard*, and `Contrast` variants exist where a church needs the 21:1 maximum.
  **What "its own ground" means for a keyed or translucent template**, because the
  floor is otherwise unmeasurable: the ground is the band's declared plate colour
  **at full alpha**, never the camera behind it. Relay cannot know what a camera is
  pointed at, and a contrast figure that pretends otherwise would be a number that
  lies — which is worse than no number (rule 18). A translucent band is therefore
  held to its plate, and `Scroll · Glass`'s alpha is bounded so that plate still
  clears the floor at the alpha it ships with;
- names a **real font family**, never `var(--f-serif)` — see Track E;
- carries **no law colour** as a band or ground: not amber (ON AIR), not amethyst
  (rehearsal), not cyan (a guess), per rule 18 and DECISIONS §21.

### Scripture — verse and reference

| Name | `seed_key` | Design |
|---|---|---|
| Scripture · Dayspring | `scripture.dayspring` | Warm ivory serif on deep charcoal; reference beneath, small, tracked, right-aligned |
| Scripture · Meridian | `scripture.meridian` | White display sans on pure black, centred. The 21:1 maximum |
| Scripture · Vellum | `scripture.vellum` | Dark ink on a cream ground — the one light-ground scripture look, for daylight rooms |
| Scripture · Indigo | `scripture.indigo` | White sans on deep indigo with a vertical gradient; reference in the same white, not a tint |
| Scripture · Column | `scripture.column` | Left-aligned with a hairline rule; reference above the verse, not below |

### Song — words only, no reference at all

Per `docs/REBRAND.md` §4: a congregation is not singing the title. None of the
five binds `reference`.

| Name | `seed_key` | Design |
|---|---|---|
| Song · Anthem | `song.anthem` | Large centred display sans, white on black |
| Song · Chorus | `song.chorus` | Ivory on deep plum, generous leading |
| Song · Open | `song.open` | White on dark teal, wide measure, lightest weight that still clears 7:1 |
| Song · Plain | `song.plain` | Pure black, tightest margins, the largest type in the set |
| Song · Ember | `song.ember` | Warm white on near-black brown, serif |

### Media — a `media` layer, and as few words as the kind allows

| Name | `seed_key` | Design |
|---|---|---|
| Media · Full | `media.full` | The picture fills the frame. Nothing else |
| Media · Caption | `media.caption` | Picture full-frame, one caption line on a solid plate at the foot |
| Media · Frame | `media.frame` | Picture inset on a ground plate with a hairline outline |
| Media · Split | `media.split` | Picture one half, words the other, via a `region` at real aspect |
| Media · Wash | `media.wash` | Picture full-frame under a scrim so type over it stays legible |

### Announcement — a full-screen notice

Not scrolling. Scrolling is its own role below, which is what keeps
`templateKind`'s announcement rule (a text layer that SCROLLS) from claiming
these.

| Name | `seed_key` | Design |
|---|---|---|
| Announce · Board | `announce.board` | Title over body, left aligned, strong hierarchy |
| Announce · Card | `announce.card` | Centred card on a ground, generous padding |
| Announce · Bold | `announce.bold` | One line, as large as the frame allows |
| Announce · List | `announce.list` | Title over stacked lines |
| Announce · Contrast | `announce.contrast` | White on black, the 21:1 maximum |

### Timer — a placed timer layer

Each carries a real layer bound to `countdown`, so none takes the `.cd-default`
fallback and none takes the region branch. `Timer · Monolith` is the one the dock
fires at by default: digits and nothing else, which is §7's whole point.

| Name | `seed_key` | Design |
|---|---|---|
| Timer · Monolith | `timer.monolith` | Digits alone, full frame. No label layer at all |
| Timer · Rail | `timer.rail` | Digits with the clock beneath |
| Timer · Titled | `timer.titled` | A `reference`-bound label above the digits — the one a plan cue's label lands in |
| Timer · Panel | `timer.panel` | Digits on a panel over a ground |
| Timer · Contrast | `timer.contrast` | White on black, the largest digits in the set |

### Scrolling Lower Third — announcements while the camera is live

A `band` layer with declared `members`, plus `scroll` on the text. These are the
five the operator asked for: notices that can run during a live service and are
equally correct on a stream, a kiosk page or a lobby TV.

| Name | `seed_key` | Design |
|---|---|---|
| Scroll · Banner | `scroll.banner` | Solid band, one crawling line |
| Scroll · Ribbon | `scroll.ribbon` | Thin band, a fixed label chip at the left and the crawl beside it |
| Scroll · Glass | `scroll.glass` | Translucent band at a declared alpha — the body sits at exactly the set value, per REBRAND §4 |
| Scroll · Bold | `scroll.bold` | Tall band, large type, for a room that reads from the back |
| Scroll · Clear | `scroll.clear` | Transparent ground, band only. Keys cleanly for OBS and for an ATEM fed over HDMI |

**The transparency law applies to all five** (`resolveOutputTemplate`, keyed
templates): a keyed template is never replaced by an unkeyed one, or a lower third
becomes a full-screen slide over a live camera.

**A crawl and reduced motion.** `prefers-reduced-motion` turns the animation off
and leaves `text-overflow: ellipsis` over `white-space: nowrap`, which **silently
truncates a notice**. That is exactly why the old `High Visibility · Announcement`
carried no `scroll`. So each of the five declares a reduced-motion fallback that
**pages** rather than truncates: the text steps through in full-width pages on a
timer rather than crawling. A notice that cannot be read in full is not an
accessible notice, and an ellipsis is a silent failure of the kind rule 35 exists
to refuse.

### SuperSource — a `region` layer whose fill is a real template

Per DECISIONS §74: a composite may not be another composite's fill, a region may
name only a built-in, and the region is its own container so the inner template
scales to the region exactly as it would to a screen of that width.

| Name | `seed_key` | Design |
|---|---|---|
| Source · Word right | `source.wordright` | Camera left, words right |
| Source · Word left | `source.wordleft` | Words left, camera right |
| Source · Tall | `source.tall` | Camera above, words below |
| Source · Inset | `source.inset` | A small camera inset on a word slide |
| Source · Even | `source.even` | Equal halves |

### Stage — monitor-only bindings

These render on a screen whose channel role is `stage`. Four of the five carry
bindings no congregation template renders (`next`, `note`, `elapsed`,
`remaining`), and one carries the new `stage_message` binding from §3.

| Name | `seed_key` | Design |
|---|---|---|
| Stage · Reading | `stage.reading` | The reading takes what is left; clock beneath |
| Stage · Next | `stage.next` | Reading with the next reference under it |
| Stage · Message | `stage.message` | Reading with a Stage Message zone that fills the screen when one is live |
| Stage · Rail | `stage.rail` | Reading with the figures beside it in a rail of their own |
| Stage · Plain | `stage.plain` | The reading alone, at the largest size the frame allows |

## A.4 Evidence this track owes

- `preset_template_count()` returns 40 and the assertion beside it says so.
- `the_bare_fixture_is_a_first_launch_and_nothing_more` (`qa.rs`) is updated to
  the new contents in the same commit that changes them, never after.
- `templateKind.test.js` names the derived role of every one of the forty.
- A contrast test walks all forty and fails below 7:1 — the instrument
  `legibility.test.js` already has, pointed at the whole shelf instead of one
  family.
- A test asserts **no seeded row is region-model**, which is what keeps RG-141
  closed against a later "tidy-up".
- A test asserts **every seeded row declares `layout.shows`**.
- Retirement: a fixture that **converts before it retires**, which RG-142 names as
  the missing instrument — every existing test in that module inserts the frozen
  bytes verbatim and so has never asked the question this wave is answering.

---

# Track B — Create New Template stops creating one

`TemplateGallery.svelte:322`:

```js
async function newFrom(starter) {
    const t = starter.make();
    const id = await saveTemplate({ name: starter.label, layout: t.layout, style: t.style });
    selId = id;
    dispatch('edit', { id });
}
```

The row exists before the editor opens. Abandon the editor and it stays — and
because the seed inserts by name, a gallery can accumulate several
`Countdown Timer` rows that a church never asked for.

**Build.** `newFrom` builds the starter in memory and dispatches a **draft** —
`{ id: null, name, layout, style }`. The editor renders a draft exactly as it
renders a saved row, with three differences:

1. the header says the template is unsaved;
2. **Save** inserts (and from then on the editor holds a real id);
3. **Discard** closes without writing anything.

Leaving a dirty draft raises an in-app two-step, never `confirm()` — Tauri's
webview returns `false` from `confirm()` without showing anything, which is rule
41 and the reason a two-step delete once deleted nothing while reporting success.

**Duplicate and import keep their current behaviour.** Both are acts on content
that already exists; a duplicate the operator asked for is not a draft.

**Evidence.** A test that mounts the gallery, chooses a starter, closes the editor
without saving, and asserts `save_template` was never called — written to fail
against today's code before the fix lands.

---

# Track C — channel roles, Main Screen, and the door Stage Message needs

## C.1 The guess

```js
$: mainChannel = channels.find((c) => c.render_target === 'native_window') ?? channels[0] ?? null;
```

`Live.svelte:1357`. `output_channels` has no primary column;
`channels.rs:149`'s `pub primary: bool` is a **monitor** property and unrelated.
So the programme pane's idea of the main screen is a render-target heuristic over
a seeded name, and it degrades silently.

## C.2 The role column

```sql
role TEXT CHECK (role IS NULL OR role IN ('main','stage'))
```

Seeded: `Main screen` → `main`, `Stage display` → `stage`. `Streaming` and
`Lobby screen` stay `NULL`. At most one row may hold `main` — enforced in the
helper, not by a partial index, because the enforcement has to produce a readable
error rather than a constraint violation. Several rows may hold `stage`: a church
may have a confidence monitor and a preacher's tablet.

Live reads the role instead of guessing, and the programme pane names the screen
it is rendering. `Outputs` gains the picker.

## C.3 Stage Message in a template, safely

Today `channels::stage_alert` publishes to **every** kiosk client through
`publish_kiosk` (`channels.rs:1149`). It is stage-only because `Output.svelte`
has no handler for `stage_alert` — a guarantee kept by omission. Adding a
`stage_message` binding to the template model means a renderer would start
reading it, and the omission stops protecting anything.

**Build, and the order matters:**

1. A new binding `stage_message` in `layers.js`, labelled **Stage Message**.
2. `Output.svelte` accepts a `stage_alert` frame **only when its own channel's
   role is `stage`**. The filter is at the receiver because the hub records no
   client identity and DECISIONS §35 is not being reversed; the output page already
   knows its own channel, because the URL is channel-keyed (DECISIONS §29).
3. The value never rides on `OutputContent`. It stays its own frame kind, held in
   renderer state, so it cannot travel on a content frame to a congregation
   screen.
4. `FRAME_VERDICTS` keeps `("stage_alert", false)` — never retained, so a private
   message cannot replay to a lobby TV joining late (rule 43).
5. The rehearsal gate is unchanged: `stage_alert` is suppressed in a rehearsal
   like every other publisher beside it.

**Evidence.** `e2e::r5_a_word_to_the_preacher_reaches_no_congregation_channel`
gains a sibling that drives a channel whose role is **not** `stage` and asserts
the frame paints nothing — the surface that was missed, per the repository's own
"a guarantee is only kept on the doors you checked".

**Settle and write down** what the predecessor spec §3.4 left open: whether
`alert` surviving a panic control on the stage page is intended. It currently
survives because the `clear`/`black` branch at `Stage.svelte:406-438` resets six
fields and not that one, and nothing in the tree records a reason. It is a silent
third answer to a question §27 asked and wave 3 answers. This wave does not need
to change the behaviour; it needs to stop it being an accident.

---

# Track D — the names

## D.1 The register

| Concept | One name | Where it must read that way |
|---|---|---|
| `stage_alert` — typed live, stage only, not saved | **Stage Message** | Dock card heading and aria, the `stage_message` binding, the stage page, any status line |
| `stage_note` — saved on a cue | **Stage Note** | Planner field, the `note` binding label (today "Operator note (monitors only)"), the stage page zone |
| `stage_next` + `next_*` | **Up Next** | Live's control, the stage zone, the binding labels |

Neither of the first two becomes the other. They behave differently — one is
persisted per cue and one is a live instruction — and one name over two behaviours
is the defect this track exists to remove, not a tidier version of it.

## D.2 The instrument

A `names.test.js` that holds, for each concept, the single user-visible string and
the files allowed to contain it — failing if a second label for the same concept
appears anywhere in `src/`. The same shape as `ipc.test.js`: a scanner that reads
every `src/` file rather than a hand-written list, plus a test that the scanner
can still see a known instance, because **a scanner that quietly narrows passes
everything** and this repository has had that exact failure twice.

## D.3 Template names

The forty `seed_key`s are stable and machine-facing; the forty display names are
unique and human-facing. The gallery rail, the gallery card, the inspector, the
Planner's cue template picker, Outputs' per-screen picker and Live's programme
pane all render the same string — which is already true, because all of them read
`templates` from one store. What this track adds is the test that keeps it true.

---

# Track E — the seal

Three leaks, both directions, all measured.

1. **App chrome reaching the wall through the template's own data.** Seeded
   `style_json` stores `"font":"var(--f-serif)"` (`db/templates.rs:136`),
   `var(--f-display)` (`:141`) and `var(--f-body)` (`:152`). Those are declared
   only in `src/app.css:54-55`, and `--f-display` has already been re-aliased once
   from Space Grotesk to Inter — an app-chrome edit that silently changed the
   typeface of every template naming it. **The forty name real families.**
2. **App chrome reaching the wall through the renderer.**
   `TemplateRender.svelte:1358` falls back to `var(--v-amber)` for `--accent`. A
   congregation screen inheriting the operator console's ON AIR amber is rule 18
   in the one place it is least visible. **The fallback becomes a template-side
   default.**
3. **The console stylesheet shipping to the congregation.** `src/output.js` and
   `src/stage.js` both `import './app.css'`, and Svelte does not scope a global
   stylesheet, so legacy unscoped rules (`.prev-main .verse`, `.prev-stage
   .verse`, `.prev-stream .lower-third`, `.prev-lobby .verse`, `.tmpl-row.active`,
   `.toggle.on`) are live on `output.html`. `app.css:35-37` records that the
   removal was blocked on *"eyes on a running app, and this machine cannot
   screenshot one"* — which Track I's browser pass supplies. **The tokens stay
   shared; the rules go.**

What is **not** being done: the design tokens are not forked. Output and stage
keep importing the token block and the self-hosted fonts. The seal is against
rules and against template data, not against the palette.

---

# Track F — the editor stops painting text over text

## F.1 The armed delete, reproduced in source

`TemplateEditor.svelte:1144` renders `{armedDelete === L.id ? 'Sure?' : '✕'}` into
`.te-lmini`, which is `width:20px; height:20px` with no `overflow`
(`:1937`). The only `.armed` rule in the file is `.te-objacts .armed`
(`:1826`) — scoped to the **inspector** row at `:1363`. So the layer row's armed
button gets neither the red confirm styling nor any extra width, and five
characters land in a 20px box inside `.te-layerlist{overflow-y:auto}` (`:1859`),
which computes `overflow-x: auto`. The CSS comment at `:1933-1936` claims this
state is covered. It is not.

## F.2 The other two

- **The object tab strip is unbounded inside a clipped pane.** `.te-objtabs`
  (`:1813`) wraps with no `flex`, no `max-height` and no `overflow`, inside
  `.te-pane{overflow:hidden}` (`:1809`). Only `.te-designbody` can shrink
  (`:2024`), so on a template with many layers the strip grows and the properties
  body collapses toward zero.
- **The add-layer menu is absolutely positioned inside that same clipped pane**
  (`.te-addmenu:1847` within `aside.te-pane.te-layers:1050`). `TemplateGallery`
  already abandoned this construction for `position:fixed` after measuring it
  (`TemplateGallery.svelte:183-186`, `:914`). Reported as an asymmetry; the
  browser pass in Track I decides whether it clips in practice.

## F.3 Evidence

Measured in a real layout engine, not argued — the same method the 2026-09-10 and
2026-09-16 passes used, because both of the worst defects those passes found were
invisible to every static instrument while `qa-inventory` reported zero problems
and was right.

---

# Track G — the timer runs bare

`Dock.svelte:625`:

```js
? startCountdown(r.broadcastMs / 60_000, 'Service begins in', 'Welcome')
```

Hard-coded, and there is no interface anywhere to type a different one. The label
is **payload**, not template: it rides in `content.reference` (`main.rs:2825`), and
a template's `reference`-bound layer is what draws it.

**Build.**

- The dock's Start passes **no label and no done-message**. Digits alone, wherever
  a timer object sits. `Timer · Monolith` has no label layer at all, so it is
  unchanged either way; the other four simply render an empty label box, which the
  fit loop already handles.
- **The Planner keeps both fields**, and gains the interface it never had:
  `addCountdownCue` currently writes the same two constants
  (`ServicePlanner.svelte:289`) with nothing to edit them. A cue that wants words
  beside the clock says so, and fires them through `Live.svelte:600-607` exactly as
  today.
- `capture.js:1429`'s defaults become empty strings rather than the two constants,
  so a caller that omits them gets a bare timer rather than a surprise.

**What does not change.** `countdown.js::countdownRemainingMs` stays the only
arithmetic. `countdown_from` keeps being written, so the warning rule keeps firing.
`adjust_countdown` still cannot create a countdown. None of the sixteen countdown
tests changes shape.

---

# Track H — the Content Looks section leaves the template editor

`toggleUsedFor` (`TemplateEditor.svelte:530`) and the **"Used for"** block
(`:1670-1690`) are deleted. That block is the editor's second writer of the
content look; `Channels.svelte:854` is the authoritative one and keeps the matrix.

**What stays, and why the distinction matters.** The row immediately beneath it —
"Content this template renders" — is `layout.shows`, a **per-screen allow-list**,
not a content look. They were two visually identical five-chip grids over the same
five labels, which is how the first click on one came to look like it had
activated all five on the other. Wave 2 gave the second one a different control
shape. This wave removes the first.

**Nothing in the engine changes.** `tpl_{kind}`, `set_content_template`,
`ContentTemplates`, `cue_or_content_tpl` and the content-look link inside
`resolveOutputTemplate` are all untouched. DECISIONS §25, §29 and §70 stand.

---

# Track I — starter content on a fresh install

## I.1 The reversal, stated plainly

`db/mod.rs:662-664` says *"NOTHING SEEDS DEMO CONTENT HERE, and nothing ever
may."* `qa.rs:257-274` enforces it, asserting `COUNT(*) == 0` for
`service_plans`, `songs`, `announcements`, `saved_scripture` and `media_assets`.

The operator has taken the opposite decision, and it gets a numbered DECISIONS
section of its own when it lands. The reasons worth recording:

1. An empty install is not neutral. A church opening Relay for the first time has
   no announcement to fire, no background to choose and no plan to run, so the
   surfaces that exist to be operated cannot be operated at all — which is how a
   volunteer decides a workspace is broken.
2. The instruments this repository trusts are all about **drift**, not about
   emptiness. The tripwire's value is that it fails when the seed changes without
   anybody saying so. That value is kept by asserting the starter set **exactly**,
   not by asserting zero.
3. Starter content is not demo content. `demo.rs` stays exactly as it is, behind
   its Settings button, writing no services, transcripts, detections, cues,
   `service_events` or `perf_samples`.

## I.2 What ships

- **Announcements** — a small set a church can fire or edit on the first Sunday,
  with the operator-facing title separated from the words that reach the room
  (REBRAND §10).
- **Backgrounds as real media** — the 34 files already bundled at
  `src/backgrounds/` become `media_assets` rows, so a background is reachable from
  the Library and from a template's media layer rather than only from the editor's
  build-time picker. This closes the gap the template-editor background picker has
  today: it offers only `BACKGROUNDS` and cannot reach `media_assets` at all.
- **One example service plan**, carrying a countdown cue with its own label and
  duration — which is also the first thing that exercises Track G's Planner fields
  and wave 3's `plan_items.duration_sec`.

## I.3 What the tripwire becomes

`the_bare_fixture_is_a_first_launch_and_nothing_more` keeps its name and its job.
It stops asserting zero and starts asserting the starter set by identity: this
many announcements with these keys, this many media rows, one plan with this
shape. A seed that drifts still fails it. **It is not deleted, and it is not
weakened to a range.**

---

# Ordering and verification

Tracks A, B, F, G and H are independent of each other and of C, D, E and I.
Track C must land before the `stage_message` binding is offered anywhere. Track A
must land before Track I's example plan pins a template by `seed_key`.

Every track ends green on both suites, using **the runner's own summary line** and
never a grep:

```
npm run build
cd src-tauri && cargo test
cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings
npx vitest run
```

`npm run build` runs before the Rust suite on a fresh tree, or two `channels`
tests fail on a bare 404 because `dist/` is gitignored (RG-127).

Every new test is verified to **fail when the defect it describes is
reintroduced**. This repository has had a test that passed against a broken file
because it grepped a comment, and a regression test written against a diagnosis
that turned out to be wrong, which passed with the supposed fix reverted.

**This wave needs a browser-driven pass against the real backend**, for the same
reason waves 2 and 4 do: forty new templates, a new band-with-crawl rendering, a
reduced-motion paging fallback and three editor-layout fixes are all things no
static instrument in this repository has ever been able to see. `qa-inventory`
reported zero problems throughout the two passes that found the worst defects, and
was right both times.

## What this change set does not claim

- It does not move the release decision. Word error rate remains unmeasured in
  every language, neither platform has a code-signing certificate (RG-73), and
  nobody but the author has run a service.
- It does not touch detection, the router, or any threshold. Rules 10, 28, 30 and
  34 are untouched.
- It does not close RG-139. Track A widens that row's reach by seeding six
  templates with a `countdown`-bound layer, and the register must say so.
- It does not unpark NDI, and it adds no SDI integration.
- It does not reverse DECISIONS §29, §35, §70 or §78.

---

<!-- ===== was docs/archive/WAVE-DESIGNS.md, merged 2026-09-21, verbatim ===== -->

> **Archived 2026-09-21.** Historical. The consolidation it describes merged to `main` as PR #87 on 2026-09-17; the measured baselines it carries are in `docs/qa/QA_HARNESS.md` Part 0 (the 2026-09-17 block). It never had a status section; this line is it. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

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

- `docs/archive/WAVE-DESIGNS.md` — the 73-line
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
nine files follow. `docs/qa/audits/DESIGN.md` is frozen
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
   harness is described in `audits/DESIGN.md` §0 and is
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

---

<!-- ===== was docs/archive/WAVE-DESIGNS.md, merged 2026-09-21, verbatim ===== -->

> **Archived 2026-09-21.** Historical. Built the same day: `docs/DECISIONS.md` §97 and §98, register rows RG-171 to RG-174 (closed 2026-09-18). Its status line ("specified, not built") and its claim that per-cue targeting "stays closed" are both false now (RG-161, RG-177 closed). Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Channel looks — a per-screen, per-kind template

**Date** 2026-09-18 · **Status** specified, not built · **Prerequisite** landed

---

## Why this is written down before it is built

The operator's report was that *"each screen should be able to have their own
template set to what they want"*. Investigating it found something underneath that
had to be fixed first, and was: **"Follow the content look" had never worked.** The
per-kind look was stored, the id crossed both doors, and nothing read it. A
following screen wore `default_template_id`, always. Worse, the Outputs tile
resolved the look correctly while idle, so the one surface an operator uses to
check the setup showed it working over a wall wearing something else.

That is fixed. This document is the feature that sits on top of it, and the reason
it is separate is that building it on a rung nobody had ever seen work would have
been building on sand.

---

## The gap that remains

`output_channels.template_id` gives a screen ONE template for ALL content kinds.
`app_settings['tpl_<kind>']` gives a per-kind default that is GLOBAL to every
screen. So an operator can have **per-kind globally**, or **per-screen uniformly**,
and never *"on the main screen scripture looks like this and songs look like that,
while the lobby TV uses something else for both."*

---

## The data model

A row per (screen, kind), not a column per kind. `CONTENT_KINDS` is already
mirrored by hand in three places with no test linking them; a column would make DDL
the fourth mirror and the least editable of them.

```sql
CREATE TABLE channel_looks (
    channel_id  INTEGER NOT NULL REFERENCES output_channels(id) ON DELETE CASCADE,
    kind        TEXT    NOT NULL,
    template_id INTEGER NOT NULL REFERENCES templates(id),
    PRIMARY KEY (channel_id, kind)
);
```

`template_id` is `NOT NULL` deliberately: **no row is the only way to say "this kind
inherits"**. An absent row and a NULL row would have to mean the same thing at every
reader, and two spellings of one fact is exactly what the role map already refuses.

`kind` carries no `CHECK`. SQLite cannot `ALTER` a `CHECK`, so a sixth content kind
would mean the table rebuild rule 25 is the scar of. A kind nobody reads is an inert
row; a rebuild before the window is shown is not.

### The migration, and the back-fill it refuses

`ensure_channel_looks` is one `CREATE TABLE IF NOT EXISTS`. Retryable by having
nothing to retry: no scratch table, no rebuild, no transaction opened, so rule 25's
failure mode has no way to arise.

**There is no back-fill, and refusing one is the whole of the migration.** An
install with four channels each carrying a `template_id` ends with zero rows, every
kind falls through to the screen's own template, and first launch is identical **by
construction** rather than by comparison.

The tempting back-fill — write each screen's current template into all five kinds so
the shape is explicit — changes nothing on day one and everything on day two: the
operator then changes that screen's template and four kinds silently keep the old
one, with no control having been touched.

### The fifth retirement door

`ensure_retired_presets_are_gone` deletes a retired preset only when nothing points
at it, checking four doors. `channel_looks` is a **fifth** and must be added in the
same commit, asserted separately from the other four. Missing it deletes a template
a screen is wearing for one kind, on a Sunday, silently. Two guards: the migration
goes on the ladder BEFORE the retirement, and a failed read of the fifth door
retires nothing.

---

## The resolution chain

A per-kind look **is not an override**. It is the screen's own template, for one
kind — so it joins one notch above the screen's blanket template, not at the
override level. That is what leaves DECISIONS §29 and the transparency law intact.

0. `templateShows`, unchanged, consulted on the SCREEN'S own template and never on
   the resolved per-kind look — otherwise choosing a look for a kind could turn that
   kind off, silently.
1. **The transparency law** — a keyed screen never accepts an opaque pinned override.
2. **A pinned cue template** — a deliberate choice about *this item* outranks a
   standing preference about *this screen*.
3. **The screen's look for this kind** — new.
4. **The screen's own template** — §29, unchanged.
5. **The global content look for this kind** — §70.
6. **The configured default.**
7. **`DEFAULT_TEMPLATE`**, the bundled floor.

The law is evaluated ONCE, against the pinned claimant only, after rungs 3 and 4
have produced "the screen's template":

```
screenLook = channel_looks[kind] ?? channel.template_id
if (screenLook == null) return pinned ?? contentLook ?? default ?? DEFAULT_TEMPLATE
if (pinned == null)     return screenLook
if (isKeyedTemplate(screenLook) && !isKeyedTemplate(pinned)) return screenLook
return pinned
```

The null branch comes first because `isKeyedTemplate(null)` is true, so a following
screen would otherwise "keep its keyed template", which is nothing.

**The law must not run between rungs 3 and 4.** The naive implementation calls the
existing resolver twice, screen-template as channel and per-kind look as override;
that applies the law to the per-kind look and silently discards an opaque
Announcement look on a lower-third screen — which is §29's original complaint
verbatim.

---

## What crosses the wire

**The content frame does not change at all.** Per-kind looks are configuration, and
configuration already has a working pattern in this hub: the role map. Copy it.

A new retained slot and a `channel_looks` frame carrying ids only, channels and
kinds with no row omitted, sent on every hello **including `{}`** — `{}` is an
answer, and a page that cannot tell "nobody has told me" from "I have no per-kind
look" paints the wrong template for one frame.

Hello ordering: with the configuration, before the retained screen frame. A look
must be in hand before the frame it dresses.

Published on change on **both doors** — a Tauri emit for the native window, which
has the bridge and no socket, and the hub frame for every browser source, which has
the socket and no backend.

The JSON behind the ids reuses what the content-look fix already built: the hello
reply sends a `template` frame for exactly the ids this install's map names, once
per connect, never per fire. **A default template still rides as an ID only** — the
13 MB PERF rule is untouched.

---

## The UI

**The screen card does not change.** A card is 232px of picture and two words;
twenty choices do not go there. Its `<select>` keeps its meaning and becomes the
screen's look **for everything else**.

One new control, in the inspector only: a disclosure called **"Per kind…"**.
Collapsed by default, summary reading **"Same look for every kind"**, zero rows.
Four screens times five kinds is twenty choices only for a church that asks for
twenty; for everyone else it is four choices and a closed line of text.

Each row's inherit option **names its destination, resolved live** — `Same as this
screen · Classic · Scripture`, or `Follow the content look · Aurora · Scripture`, or
`Follow the configured default · Classic Serif`. An inherit option that reads
identically whether a content look is set or not is not a line.

The summary names the kinds, never a count. The card gains one text term on its meta
line, no new lamp and no new colour.

`set_channel_look(channel_id, kind, template_id: Option<i64>)` — `None` deletes the
row. **Not service-lock protected**, for the reason `rename_channel` states: it is
reversible, and the moment an operator most wants it is when a look turns out wrong,
which is during a service.

---

## What this deliberately does not do

**Per-cue-per-screen targeting (RG-161) stays closed and this does not open it.** The
content frame is unchanged, the publish stays global and unrouted, and retention is
one configuration slot replayed to everyone. **It decides a LOOK at the receiver and
never a DESTINATION at the sender.**

The line to hold, and it will be pushed: **a per-kind look changes what a screen
wears, never whether it paints.** The moment somebody proposes "a look of NONE means
this screen skips this kind", that is RG-161 arriving through the back door with
none of its pieces and none of its tests. `layout.shows` is the standing answer to
that question and it lives on the template.

No per-kind background, transition or role. No back-fill. No `CHECK` on `kind`. No
look ids in the browser-source URL. No second resolver and no resolution in Rust —
the client resolves, because the client is the screen.

---

## Tests, in the order they must be watched to fail

1. `ensure_channel_looks_is_retryable` — three calls, one connection.
2. `an_upgraded_install_behaves_identically_on_first_launch` — fails the moment
   anyone adds the back-fill.
3. `a_template_a_screen_wears_for_one_kind_is_not_retired` — the fifth door,
   asserted separately.
4. `every_kind_this_module_publishes_has_an_explicit_verdict` — already exists, must
   be seen red before the new verdict is added.
5. `the_look_map_is_not_a_screen_frame`.
6. `a_client_that_connects_mid_service_is_sent_the_look_map_before_the_verse`.
7. `a_screen_wears_a_different_look_for_each_kind`, asserted on **both** doors.
8. `a_panic_control_takes_every_screen_whatever_look_it_was_wearing` — no channel and
   no kind anywhere on the clear/black path.
9. The pure resolver, rungs enumerated — **the per-kind-beating-blanket case must be
   watched to fail against the naive two-call implementation**, which applies the
   transparency law between rungs 3 and 4.
10. `templateShows` still reads the SCREEN'S template, never the resolved look.
11. The inspector and card previews resolve through the same call as the wall.

---

<!-- ===== was docs/archive/WAVE-DESIGNS.md, merged 2026-09-21, verbatim ===== -->

> **Archived 2026-09-21.** Historical. Eleven of its thirteen items shipped on 2026-09-20 (RG-161 to RG-177, DECISIONS §104, §105); its open questions now live at the tail of `docs/superpowers/plans/2026-09-19-stage-timers-mobile.md`. Its "Verification status" block describes the evening it was written and is false about the code as it stands. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Stage reach, Planner craft, media transport and ProPresenter

Date: 2026-09-19. Status when written: design, approved in outline, no code written.

> **Status on 2026-09-21: largely SHIPPED, and this document is now history.** Thirty commits
> dated 2026-09-20 delivered SP1, 2b, 2c, 2d, 3a, 3b, 4a, 4b, 4c, 4d, 4f, 5a, 5b and 5d
> (see the register RG-161 to RG-177 and DECISIONS §104, §105). Still not built: **4e**
> (click semantics, contradicted by DECISIONS §81 on the run surface), **5c** (playlists into
> plans), **5e** (media import, BLOCKED on the church's media folder) and the elapsed timer.
> The "Verification status" block at the end describes the evening this was written and is
> false about the code as it stands; it is kept because a frozen design should not be edited
> to agree with what happened after it. Rulings that only lived here are now in
> `docs/DECISIONS.md`.

The operator chose "spec everything first, build nothing yet" on the evening before a
service test.

Companion document: [`../superpowers/plans/2026-09-19-stage-timers-mobile.md`](../superpowers/plans/2026-09-19-stage-timers-mobile.md).
That plan owns the connection journey, timer state and saved stage layouts, and
several of its phases are already delivered. **This document does not restate it.**
Where the two meet, this one says which phase it depends on and stops.

## What this is

Thirteen requests, arriving together, spanning five subsystems. They are too many
for one implementation plan, so this document decomposes them into five
sub-projects with a dependency order. Each sub-project gets its own implementation
plan afterwards. The decomposition is the deliverable; the per-item design below is
deep enough that a plan can be written from it without re-investigating the code.

Every file:line reference in this document was read on branch `stage-timers-mobile`
at `3956d26`. Four of the load-bearing ones were then verified a second time by
hand, because a previous session recorded that three of eleven agent findings were
false. Those four are marked **(verified)**.

## The decisions taken before writing this

Recorded here because each one closes off an alternative that a later reader would
otherwise reopen.

| Question | Decision | Consequence |
|---|---|---|
| Stage TV timer | Fix the address now; **role-gate** the refusal later | `timer: false` becomes `timer: role === 'stage'`, not `timer: true` |
| Screen Countdown | Leave Quick tools; become a Planner cue aimed at the streaming screen | `quicktools.test.js` drops from three blocks to two, on the operator's instruction |
| Planner colour | **Sections only**, in hues nothing else owns | `plan.js:13-77`'s refusal of per-kind colour stands |
| Planner firing | **No.** The Planner still cannot reach an output | The Tuesday/Sunday separation in `CLAUDE.md` survives untouched |
| Click semantics | Single click previews, double click previews large, a deliberate action goes live | No single click anywhere can reach a congregation screen |
| ProPresenter | Songs, then playlists into plans, stage layouts, discovery, media | Needs real protobuf decoding, which does not exist today |

## The corrections this work rests on

Three claims that were believed at the start of the session and are false.

**The stage TV timer is not a defect. (verified)** `src/Output.svelte` handles
thirteen frame kinds at `:647-759` and contains the string `'timer'` zero times.
`src/lib/r6-contracts.test.js:165` pins `timer: false` with a stated reason: a
programme timer is one person's bookkeeping, and painting "Sermon · 4:12 left"
behind a preacher puts the running order in front of the whole building. There are
two stage pages, not one:

| Page | Address built by | Stage Timer |
|---|---|---|
| `stage.html?channel=N` | `channelroles.js:146`, surfaced in Outputs → **Sharing** | Yes, `Stage.svelte:952` |
| `output.html?channel=N` | `outputurl.js:27`, surfaced as Screens → **Copy URL** | Never |

`src/lib/stagetimerreach.test.js:94` already names this trap in prose. A TV wired
from Copy URL is online, is attached, reports healthy, and will never show a rail.

**Per-cue screen targeting lies for two of five cue kinds. (verified)** The Screens
row at `ServicePlanner.svelte:1118` is not gated by cue type. Scripture
(`Live.svelte:917`), song (`:951`) and announce (`:953`) pass `cueChannels`. Media
(`:923`) and countdown (`:930`) do not, and neither `fire_media`
(`main.rs:3867`) nor `startCountdown` (`capture.js:1646`) accepts such an argument.
`OutputContent.channels` therefore stays `None` through `..Default::default()`, and
the cue reaches every screen under a hint reading "Other screens keep what they are
showing". **Not yet filed in the register**, and it should be: it is the fourth instance of the shape `CLAUDE.md`
records under "a guarantee is only kept on the doors you checked".

**The Planner's neutral palette is a refusal, not an oversight.** `plan.js:13-77`
records declining per-kind colour twice, most recently 2026-09-16, with the
measurements. `--v-col-scripture` **is** `--v-amber` and `--v-col-media` **is**
`--v-amethyst` (`tokens.css:194-197`), so colouring cue kinds from the existing set
puts ON AIR colour on a row that is not on air and rehearsal colour on one that is
not a rehearsal. `plan.js:35` records magenta (~310°) and lime (~80°) as the only
free hues.

## The source material

`~/Downloads/rrr.zip`, 62 MB, is a ProPresenter 7 library and configuration export
from `/Users/hop-media-001/Documents/ProPresenter/`.

- 726 `.pro` files under `rrr/SONGS/`. ProPresenter 7 protobuf, not XML.
- Root configuration files, all protobuf: `Library` (playlists), `Media`, `Theme`,
  `Stage`, `Timers`, `Props`, `Workspace`, `Macros`, `CCLI`, `Groups`, `Labels`.
- **The media is not in the archive.** One `.mp4` and one `.png`. `rrr/Media` holds
  paths under `/Users/hop-media-001/` that do not exist on this machine. Importing
  it yields broken references. The church's media folder is needed separately, and
  this is a blocker on one request rather than a design problem.
- `rrr/Timers` names the operator's own two concepts: `PRE SERVICE COUNTDOWN` and
  `SEGMENT COUNT DOWN`.
- `rrr/Stage` holds their layouts: `Current + Next Text`, `Seg Timer`,
  `Video Timer`, `Video Countdown`, `Segment Countdown`, built on a `${timer}` token.
- `rrr/Workspace` names their screens: `ONLINE SCREEN` (Samsung, 1080p60),
  `STAGE SCREEN` (LF24T35), `ONLINE + STAGE SCREEN`.

**The existing importer already reads the songs.** `proimport.rs` does not parse the
protobuf at all (`proimport.rs:5-7`); it scans raw bytes for `{\rtf1 … }` blocks
(`:204-240`) and strips RTF to text (`:25-100`). Measured against the real archive:
all 726 files contain RTF, 14,364 blocks in total, zero files with none.
`JESUS.pro` carries 332 blocks and about forty files carry exactly one, which is
the set worth eyeballing after an import rather than a reason to doubt the method.

ProPresenter is **not installed on this machine**. No `/Applications/ProPresenter.app`,
no `~/Documents/ProPresenter`. Discovery must therefore be designed against a
folder the operator points at, with Spotlight as an accelerator, not against an
application bundle.

---

# Sub-project 1: screen targeting truth

**The smallest piece of work here, and the only one that is a defect.**
It is first because a control that reports a success it did not achieve is the
failure this repository has recorded four times, and because sub-project 3 needs
`start_countdown` to carry screens before a countdown can be aimed at one.

## Design

Give `fire_media` and `start_countdown` the argument their neighbours already have,
and pass it at the two call sites.

```
fire_media(app, db, id, template_id)
  -> fire_media(app, db, id, template_id, channels: Option<Vec<i64>>)

start_countdown(..., warn_ms, until_ms)
  -> start_countdown(..., warn_ms, until_ms, channels: Option<Vec<i64>>)
```

Both already end at `broadcast_with_clock`, which is the choke point
(`main.rs:785`), so the field rides the existing route and no receiver changes.
`Live.svelte:923` and `:930` pass `cueChannels`, which is already computed three
lines above them at `:909`.

`show_background` is deliberately **not** given the argument. A standing backdrop is
room furniture rather than a targeted cue, and `channels` on it would be a second
answer to a settled question. Record that at the call site so the next reader does
not read the omission as the same bug.

## What this must not break

- `null` means every screen and `[]` means no screen (`plan.js:487`). Neither
  meaning may change; a cue written before targeting existed still reaches
  everything.
- Rule 36. The check stays at `broadcast_with_clock`, never at the call sites.
- A screen a cue does not name keeps what it was showing. It is never cleared.

## Tests

- Extend the existing per-cue targeting tests rather than writing a parallel set.
- The test that matters is the one on the surface that was missed: fire a **media**
  cue naming one of three screens and assert the other two are untouched. Write the
  same for **countdown**. Watch both fail against the current code first.
- An `e2e.rs` case, because this is the fire path.

## Acceptance

A media cue and a countdown cue each naming one screen of three reach that screen
and no other, asserted at the kiosk hub and at the Tauri door.

---

# Sub-project 2: stage reach

Everything that decides what the preacher can see. Depends on nothing; can start
immediately.

## 2a. The address, and the affordance that stops the trap recurring

No code is required to fix tonight's TV: point it at the Sharing address. The
product change is that **Screens → Copy URL must stop being able to mislead**.

Design: in the Screens inspector, when a channel holds `role === 'stage'`, the
address row offers the **stage address** as well as the output address, labelled for
what each one does, with the Copy URL affordance saying plainly that it produces a
congregation-shaped page. `channelroles.js:146` already builds the stage URL and
`Channels.svelte:1770-1812` already owns the address row.

`describeStageReach` (`channelroles.js:214`) already produces the right sentence
when a stage screen has never reported painting. It is currently only read by Live
(`:520`). Read it in the Outputs inspector too, which is where an operator is
standing when they wire a screen.

## 2b. Stage-shaped templates carry the programme timer

The reversal, gated on role so the recorded safety reason survives.

```
r6-contracts.test.js:165   timer: false
                        -> timer: 'stage-role only'
```

- `Output.svelte` gains a `timer` branch in the ladder at `:647-759`, guarded by
  `roleOf(roles, channelId) === 'stage'`. Roles already ride the wire
  (`channel_roles`, read at `Output.svelte:740`) and are already retained and
  replayed on hello (`channels.rs:3033`), so no new plumbing is needed.
- `TemplateRender.svelte` gains a `programme` bind beside the existing monitor-only
  binds (`layers.js:51,53,62,63`). It renders the timer **set**, which is what makes
  it different from `bind:'countdown'`: `Stage.svelte:530-544` already derives the
  set, and `timers.js:35` already filters by scope. Extract that derivation into
  `timers.js` so the two surfaces cannot disagree, exactly as `outputHealth.js`
  serves Live and Outputs from one helper.
- `layers.js` stage starters (`stageDisplay` `:940`, `confidenceMonitor` `:962`,
  the preacher view `:982`) gain a programme layer.
- The congregation refusal is unchanged and must be re-asserted by a test: a screen
  with `role === 'main'` wearing a stage template still shows no rail.

**The overflow rule comes with it.** `Stage.svelte:605-618` collapses to a "+N more"
cell below `MIN_TIMER_PX`, driven by `innerWidth`. A TV webview reporting a small
width is a recorded suspect (plan finding S10). Whatever `TemplateRender` does here
must be measured against its container, not the window, because a template renders
inside a region.

## 2c. A stage alert the preacher cannot miss

`@keyframes stagealert` exists only on the phone (`Stage.svelte:1907-1913`): a
1.4 s background pulse between `#c8121c` and `#7a0a11`, behind
`prefers-reduced-motion: no-preference`. `TemplateRender` has no equivalent; its
only pulses are `cdwarn` (`:2159`) and the ticker crawl (`:2125`).

Design: one shared alert presentation, defined once and used by both pages. The
operator asked for "very strong flashing", so the stage form is deliberately louder
than anything a congregation screen is allowed to do: full-bleed, high-contrast,
and a pulse that does not stop until it is taken down.

Three constraints that are not negotiable:

- **Reduced motion still gets an answer.** The phone's existing rule swaps the pulse
  for a static treatment. Keep that. A preacher with vestibular sensitivity must
  still know a message arrived.
- **A panic control takes it down.** `Stage.svelte:908` already clears the alert on
  `clear` and `black` (DECISIONS §91). The template path must do the same.
- **`stage_alert` is not retained** (`channels.rs:5537`) and must stay that way. A
  phone that reconnects an hour later must not be flashed a message from before the
  sermon.

## 2d. Media and images on the stage display, with scripture overriding

A new zone in the shared vocabulary at `stagelayout.js:15-26`, alongside `reading`,
`next`, `note`, `countdown`, `clock`, `elapsed`, `programme`.

The precedence rule is the whole design, and the operator stated it: **scripture
overrides everything**. Written out:

1. A reading on the stage display always wins the primary area.
2. Stage media occupies the primary area only while no reading holds it.
3. When a reading arrives, stage media is displaced, not destroyed. When the reading
   is cleared, it returns.
4. A panic control takes both down. No exception, per rule 15.

This needs a retained frame of its own, in its own slot, so a device joining
mid-service is shown what is on the stage (the same reasoning as rule 43). It must
**not** share the single screen slot with `content`, or a stage image would stand in
for the reading that displaced it.

Note honestly: this is the request with the least existing scaffolding in this
sub-project. It needs a new frame kind, a new zone, a retention slot, a precedence
rule and receiver handling on one page. It is not a small addition to 2b.

## Tests

`stageprogrow.test.js`, `stagezones.test.js`, `stagealertpanic.test.js` and
`r6-contracts.test.js` are the four that already hold this ground. Extend them.
`r6-contracts.test.js` in particular must keep enumerating the whole frame ladder,
because a test that checked only the kinds we remembered would have passed on the
night all four overlays were broken (rule 44's lesson).

## Acceptance

A stage-role screen on `output.html` shows the programme rail; a main-role screen
wearing the same template does not. An alert flashes on both stage surfaces, stands
down under reduced motion, and is taken by both panic keys. Stage media yields to a
reading and returns when it is cleared. A device joining mid-service sees what is on
the stage.

---

# Sub-project 3: timer surfaces

## 3a. Screen Countdown leaves Quick tools

Quick tools is pinned at three blocks by `quicktools.test.js:319` and `:427`, on the
operator's 2026-09-14 instruction. **This change is that instruction being revised,
not a test being worked around.** Record it as such.

Design:

- Remove the countdown block (`Dock.svelte:1181-1329`). The card becomes two
  blocks: Live transcript's neighbour tools and the Stage Message. Update
  `quicktools.test.js` to assert **two**, with the reason in the file.
- A countdown is started from a Planner `countdown` cue, which already exists
  (`ServicePlanner.svelte:316`, fired at `Live.svelte:924`).
- The cue carries screens, which sub-project 1 makes real. Aimed at the streaming
  screen, a countdown reaches the stream and no congregation screen, which is what
  "only when needed for the live service that goes online" means in code.
- The transport (pause, resume, reset, ±1) has to remain reachable during a service.
  It moves to the Live run surface beside the existing Stage Timer band
  (`Live.svelte:2298-2345`), not into Settings and not into Outputs.

**Open question for the operator, to settle before the plan is written:** with the
dock block gone, how is a countdown started when there is no plan? A pre-service
countdown on a Sunday with an empty plan is a real case. Options are a cue added to
an ad-hoc plan, or a small starter on the Live surface itself. This document does
not choose.

## 3b. Timer typography

Scope: the congregation-facing countdown only. The operator asked that the stage one
"stay inline for readability", so `Stage.svelte`'s four-step discrete sizing
(`:786`, `:1601-1611`) is untouched.

Today, `.countdown` (`TemplateRender.svelte:2262-2271`) is `tabular-nums`, weight
700, `line-height: 1.05`, `letter-spacing: 0.01em`, `nowrap`, at `verseSize * 2`.
The work is craft rather than architecture: optical weight at very large sizes,
letter-spacing that suits digits rather than prose, the treatment of the separator,
and how the figure settles when it changes.

Four existing guards constrain it, and all four are load-bearing:

- **Rule 37.** The fit floor is a ratio, 45% of the designer's cqw
  (`MIN_LEGIBLE_SCALE`, `:324`), reported through `onFit`, never a blank screen.
- **Rule 42.** A fit measured in the fallback face must re-fit, bounded to two
  retries (`needsRefit`, `:23`).
- **RG-141.** The countdown is fitted against a definite rectangle,
  `.content.cdbox` (`:2218`), because a `nowrap` line makes a shrink-to-fit box
  scale with its own type.
- **The tick gate.** `fitSig()` ends `…|${countdownTo ? 1 : 0}` (`:590`), so a
  per-second tick does not retrigger a fit. Any animation added to the digits must
  not defeat this, or a 4 Hz reflow storm returns.

One honest gap worth closing while here: `legibility.js` reviews the template's
declared verse size and does not know the countdown paints at `verseSize * 2`
(`:1919`). Its contrast and distance verdicts are therefore silent about the
largest thing on the screen.

---

# Sub-project 4: Planner craft and media transport

The largest sub-project, and the one that should be split again when its plan is
written. Its first four items are craft; its last is architecture.

## 4a. Section colour

Colour the section bands, never the cue kinds. `sectionsOf` (`plan.js:285`) already
derives sections and `ServicePlanner.svelte:812-818` already renders the caption and
hairline, so the structure exists and only its presentation changes.

Constraints:

- Hues come from magenta (~310°) and lime (~80°), the two `plan.js:35` records as
  unclaimed, plus neutrals. A section band may never wear amber, amethyst, cyan,
  grey, rose or emerald, because each already means something exact
  (`tokens.css:136-178`).
- The cue kind chip `.sp-ck` stays neutral. `TAXONOMY_INK` (`plan.js:78`) does not
  move.
- `colourlaw.test.js` must be strengthened while this is done: its `TYPE` sweep is a
  substring match today, which `plan.js:67-76` already notes would let
  `var(--v-col-…)` slip past.
- Colour is never the only signal. A section must read correctly in greyscale, for
  the same reason a paraphrase shows no percentage.

## 4b. Adding and deleting a cue

The operator asked for adding and deleting to be clean and straightforward.
Measured against the code:

- **Adding** is already one click per result row (`ServicePlanner.svelte:923-948`),
  behind a mode toggle at `:781`. The friction is the toggle and the search, not the
  commit.
- **Deleting** is the real gap. A cue can only be removed from the **inspector**
  (`:1175`); there is no affordance in the running order at all. Deleting three cues
  means three select-then-travel-to-inspector round trips.
- **Duplicate** has a defect worth fixing in passing: `duplicateCue()` (`:594`)
  appends at the end despite a comment claiming it copies "a cue in place".

Design: a per-row delete in the running order, and multi-select so a run of cues can
go at once. No native `confirm()` (rule 41: Tauri's webview returns `false` without
showing anything, so a guarded delete deletes nothing and reports success). Use the
in-app two-step arm and confirm that `TemplateGallery` and the plan rail
(`:186`) already use.

`ServicePlanner.svelte` is 1607 lines and this work will grow it. Two blocks are
already self-contained and should move out first: the add and search block
(`:243-345`) and the drag block (`:376-460`).

## 4c. Media preview on a Planner slide

The exact cause, traced end to end:

1. `plan.js:169` returns `[{ tag: 'BG', label: item.label, text: '' }]` for a media
   cue. **No `media_id`, no URL, empty text.**
2. `ServicePlanner.svelte:636-643` builds `previewContent` without `media_url` or
   `media_kind`, the two fields `TemplateRender` actually reads.
3. `previewState` (`plan.js:449`) sees empty text, takes the media branch at `:453`
   and returns `plate: false`.
4. `ServicePlanner.svelte:1004` therefore never enters the `TemplateRender` branch
   at `:1021`, and paints the sentence at `:1037` instead.

`TemplateRender` would have rendered it correctly all along:
`TemplateRender.svelte:1786-1793` and `:1925-1933` branch on
`content.media_kind === 'video'`.

The fix already has a model in the repo. `mediaUrl(host, asset)`
(`bundledbackgrounds.js:87`) is the shared builder and mirrors Rust's `media_url`
(`main.rs:4036`) including the bundled case; `MediaLibrary.svelte:161` uses it for
thumbnails. The Planner already holds `allMedia` (`:126`, loaded `:230`). The only
missing piece is the host, obtained by `localIp()` as `MediaLibrary.svelte:142`
does. A video thumbnail uses `preload="metadata" muted playsinline`, as the Library
already does.

## 4d. Drag a media file into the Planner

**No file-drop exists anywhere in `src/`.** Zero hits for `dataTransfer.files`, and
zero for `tauri://drag-drop`, `dragDropEnabled` or `onDragDropEvent` in `src-tauri/`.
The only HTML5 drag in the product is the Template editor's layer reorder
(`TemplateEditor.svelte:572-580`). The Planner's own reorder is pointer-based and
was deliberately migrated away from HTML5 drag (`ServicePlanner.svelte:376-386`).

Two candidate mechanisms:

- **Webview drop.** A `drop` handler reading `dataTransfer.files`, then the existing
  base64 path (`fileToBase64` → `importMedia`). No new Rust, no new permission. The
  file is read into the webview, so the 256 MB `MAX_IMPORT_BYTES` cap
  (`main.rs:2807`) applies and a 4K background is a real memory event.
- **Tauri's native drag-drop event**, which hands over a **path** rather than bytes.
  Far better for large video, needs `dragDropEnabled` and a Rust command that reads
  from a path, which is a new capability decision.

Recommendation: the webview drop first, because it reuses the whole existing import
path and adds no permission surface. Revisit the native event when a real 4K file
proves the cap is a problem.

Two rules apply to the drop itself: it must not collide with the pointer-based
reorder, and dropping onto a running order must show where the cue will land before
the mouse is released.

## 4e. Click semantics

The operator's model, as decided: **single click previews, double click previews
large, going live is a deliberate separate action.** No single click anywhere may
reach a congregation screen, and the Planner still cannot reach an output at all.

| Surface | Single click | Double click | Live |
|---|---|---|---|
| Library | load into preview | large preview | existing fire control |
| Planner | select and preview | large preview | **not available** |
| Live | select into the preview pane | large preview | Enter, or the GO control |

This is a change to a shared interaction vocabulary and must be applied everywhere
at once, or the same gesture means two things on two surfaces, which is exactly the
failure the transport's mode label exists to prevent.

## 4f. Media transport and remaining time

**The largest single item in this document, and the one with no existing wire.**

What exists: nothing. The `<video>` is mounted `autoplay loop` with no `controls`
(`TemplateRender.svelte:1790`, `:1887`, `:1931`). A repo-wide search over `src/`
excluding tests returns **zero** hits for `currentTime`, `timeupdate`, `.duration`,
`.pause()` and `playbackRate`. The only `play()` calls are the two inside
`routeAudio` (`:1045-1058`). There is no `media://` event; media rides
`output://content` as two fields. `loop` is a hard-coded attribute, not a setting.

Five things must be built, in this order:

1. **A player that can be asked.** `TemplateRender` exports duration on
   `loadedmetadata` and position on `timeupdate`, and accepts `paused`, `loop` and
   `seek`. The `{#key slideKey}` remount at `:1447` destroys playback state on any
   content change, so the media URL must be isolated from the text fields in that
   key or a caption edit restarts the clip.
2. **Fields on the one frame, following the countdown precedent exactly.**
   `OutputContent` already carries `countdown_to`, `countdown_from` and
   `countdown_paused_ms`: an absolute deadline plus a frozen figure, so the console
   subtracts locally and never polls. The media equivalents are `media_started_at`,
   `media_paused_at` and `media_loop`, threaded through `broadcast_content` at the
   choke point exactly as `channels` was.
3. **A duration the console can read.** This is where the countdown analogy runs
   out: a countdown's length is chosen and a clip's is a property of the file.
   Either `media_assets` (`db/library.rs:203`) grows a `duration_ms` column
   populated at import, which needs a probe the crate does not have, or the screen
   reports it.
4. **A reverse channel, which does not exist.** The hub's only client-to-server
   messages are `hello` (`channels.rs:2870`) and `beat` (`:2860`). **This is the
   decisive constraint.** Without a screen reporting its real playback state,
   remaining time on Live is computed from `now - media_started_at` against a stored
   duration, and a screen that buffered, stalled or failed to autoplay makes the
   console's clock a confident lie. That is rule 35: a status line that cannot
   detect its own failure is not a status line. **Design the reverse report first,
   or do not ship the readout.**
5. **Commands and receivers.** `pause_media`, `replay_media`, `seek_media`,
   `set_media_loop`; none exist (`main.rs:512-519` registers only `list_media`,
   `import_media`, `delete_media`, `fire_media`). All three doors need handling:
   `output.html`'s kiosk branch, its Tauri branch, and `stage.html`.

Colour, because it is already decided elsewhere: the transport may not use amber for
"playing", since amber means ON AIR and nothing else. Grey (CUED) and steel blue
(selection) are the only honest candidates in the existing set.

**Recommendation:** split 4f out of this sub-project when its plan is written. It is
a wire change, a schema change and a new client-to-server direction, and it should
not travel with thumbnail work.

---

# Sub-project 5: Library and ProPresenter

## 5a. Library scripture numbering

The operator asked that Library chapters show numbers as Live does. The
investigation changes the request, so it needs restating before it is built:

- **Neither surface shows verse numbers.** Both show a deck ordinal. Library prints
  `{v.slideNo}` bare (`VerseDeck.svelte:260`, `:450`); Live prints
  `String(c.n).padStart(2, '0')` (`Live.svelte:2569`). **(verified)**
- Library's ordinal is deliberately decoupled from the verse number. The comment at
  `Browse.svelte:341-345` records why: a verse number drifts the moment anything is
  sorted or filtered, and then two slides on screen wear the same number.
- Library re-derives the reference locally (`Browse.svelte:313`) while Live consumes
  the backend string (`slidegrid.js:123`). Same output today
  (`db/verses.rs:131` uses the identical format), but two copies of one rule.
- Live keeps a real verse number in `tag` holding `v` plus the verse number (`slidegrid.js:126`) and
  never renders it.

Three separable pieces of work, and the operator should choose which they meant:

1. **Padding.** Make Library's ordinal `01`-shaped like Live's. One line each at two
   sites, and the consistency asked for.
2. **Real verse numbers.** Render `v.verse` on both surfaces, beside or instead of
   the ordinal. Genuinely more useful for finding a verse, and it must not
   reintroduce the drift the comment warns about, so it is an addition to the
   ordinal rather than a replacement.
3. **One source for the reference string.** Point Library at `v.reference`. Not
   visible to anyone, and it removes a duplicated formatting rule.

There is no shared card component: `VerseDeck.svelte` and Live's inline `.sg-cell`
grid are two implementations. Only `TemplateRender` is shared. Extracting a shared
card is the right fix and is larger than the request.

## 5b. Bulk song import

Works today, needs only volume. `parse_import` (`main.rs:3007`) and
`save_reviewed_songs` (`:3068`) are the one path, fed from
`Library.svelte:389`, byte-capped at 256 MB (`main.rs:2807`, mirrored
`capture.js:2226`).

What 726 files need that one file does not: progress, cancellation, a review step
that does not require reading 726 entries one at a time, and a report at the end
naming what came in thin. About forty files yield a single RTF block, and those are
where a silent failure would hide.

Titles come from the file stem (`proimport.rs:313`) and every slide is renamed
`Slide N` (`main.rs:3036`), so section structure is lost. That matters for 5c,
because an arrangement is a list of indices into sections (rule 39).

## 5c. Playlists into plans, and stage layouts

**Both need real protobuf decoding, which does not exist.** `proimport.rs:5-7`
states it outright. ProPresenter 7's `.proto` schemas are not published by
RenewedVision, so this is reverse-engineering from the wire format.

`rrr/Library` holds the playlists, including their references
(`Libraries/SONGS/30 Billion.pro`, `Libraries/SCRIPTURES/Announcement.pro`).
`rrr/Stage` holds the layouts. Mapped onto Relay:

| ProPresenter | Relay | Note |
|---|---|---|
| Playlist | a plan (`plans`, `plan_items`) | order preserved; each entry becomes a cue |
| Playlist entry → `.pro` | a song cue | needs the song imported first, so 5b gates this |
| Stage layout | a stage layout (`stage_layouts`, `db/stage.rs:96`) | zone vocabulary differs; `${timer}` maps to `programme` |
| `Timers` | nothing yet | `PRE SERVICE COUNTDOWN` and `SEGMENT COUNT DOWN` map onto `Scope::Both` and `Scope::Stage` respectively |

Sequencing: 5b before 5c, because a playlist entry that names a song Relay does not
have is a cue that cannot be built. An entry whose song is missing must be imported
as a visibly incomplete cue rather than silently dropped.

**Honest scoping note.** This is the item most likely to overrun. A defensible
smaller first delivery is playlist **order and titles** only, matched against songs
already imported by name, with everything unmatched listed for the operator. That
needs far less of the schema than full decoding and delivers most of the value.

## 5d. Discovery

Not possible today, and the reasons are specific:

- The frontend has **no filesystem API at all**. `capabilities/default.json` grants
  only `core:default`, `updater:default` and `process:allow-restart`. Neither
  `tauri-plugin-fs` nor `tauri-plugin-dialog` is in `package.json` or `Cargo.toml`.
- Rust is unsandboxed (`relay.entitlements` says so explicitly), so a Rust-side
  `read_dir` needs **no capability change**.
- But `~/Documents` is TCC-gated on macOS 10.15+, and `Info.plist` carries only
  `NSMicrophoneUsageDescription`. A scan there needs
  `NSDocumentsFolderUsageDescription` added. **Rule 17 applies directly**: this is
  exactly the class of failure that is invisible until the one signed build handed
  to a church. `scripts/sign-local.sh` reproduces it without a certificate and must
  be run before this is called done.

Design: a new Rust command that scans a folder the operator points at, defaulting to
`~/Documents/ProPresenter` and its known siblings, reporting what it found without
importing anything. Spotlight (`mdfind`) is an accelerator for the default case and
must never be the only mechanism, because it is disabled on some machines and
returns nothing on an external drive.

**A scan and not a file picker needs its reason recorded**, the way
`models.rs:595` already records the same decision for the STT model directory.

## 5e. Media import and preview

Design is ordinary and mostly exists: `import_media` (`main.rs:2874`),
`media_assets` (`db/library.rs:203`), `/media/<id>` (`channels.rs:3324`, range
requests supported), thumbnails in `MediaLibrary.svelte:161`.

**The blocker is data, not code.** The archive holds one `.mp4` and one `.png`.
`rrr/Media` references files under `/Users/hop-media-001/` that are not present. An
importer pointed at this archive will produce a media library of broken references.
The church's media folder has to be supplied separately, and until it is, this item
is `BLOCKED` rather than pending.

Note also: there is **no audio kind**. `image`, `video` and `document` only
(`Library.svelte:307-310`), and documents are refused as output backgrounds
(`main.rs:3903`, `:4007`).

---

# Order of work

```
SP1 targeting truth  ──────────────┐
  (smallest, and a real defect)    │
                                   ├──► SP3a countdown scoping
SP2 stage reach  ──────────────────┤
  2a address affordance            │
  2b role-gated programme timer    │
  2c alert flash                   │
  2d stage media + override        │
                                   │
SP3 timer surfaces  ───────────────┘
  3b typography (independent)

SP4 planner craft         SP5 library + propresenter
  4a section colour         5a numbering (independent, tiny)
  4b add/delete             5b bulk song import
  4c media thumbnails       5c playlists + stage layouts (needs 5b)
  4d drag-drop              5d discovery (needs Info.plist, rule 17)
  4e click semantics        5e media import (BLOCKED on the media folder)
  4f media transport  ◄──── split this out; it is a wire change
```

SP1 first because it is a defect and because SP3a needs it. SP2 and SP5 can run in
parallel with each other. SP4's 4f should become its own plan.

# What this document deliberately does not decide

- How a countdown is started when there is no plan (3a).
- Which of the three numbering changes the operator meant (5a).
- Whether media transport ships without a reverse channel. The recommendation is
  that it does not, and that is a recommendation rather than a decision.
- Anything covered by [`../superpowers/plans/2026-09-19-stage-timers-mobile.md`](../superpowers/plans/2026-09-19-stage-timers-mobile.md),
  whose phases 1, 2, 4, 5 and 6 remain open. Phase 6 in particular is a written
  rehearsal script that has never been run, and no claim here about a physical
  stage TV, a phone or a projector can be made until it is.

# Verification status

- **NOT TESTED:** every item in this document. No code has been written.
- **PASS (measured this session):** 726 of 726 `.pro` files contain RTF, 14,364
  blocks, zero empty. Command recorded in the session log.
- **PASS (verified by hand):** `Output.svelte` contains no `timer` branch;
  `fire_media` and `startCountdown` take no `channels` argument while three sibling
  call sites pass one; the Planner Screens row is not gated by cue type; Library
  prints a bare ordinal where Live pads to two digits.
- **BLOCKED:** 5e, on the church's media folder, which is not in the archive.
- **NOT APPLICABLE:** any claim about a physical stage TV, a phone, a projector or a
  packaged build. This machine cannot screenshot the Tauri window and no device has
  been driven.

---
