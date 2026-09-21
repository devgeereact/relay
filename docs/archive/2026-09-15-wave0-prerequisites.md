> **Archived 2026-09-21.** Historical work order. Wave 0 landed 2026-09-15; see `docs/REBRAND.md` §Status. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Wave 0 — Prerequisites Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the three enforcement gaps that later waves rely on, so that the guarantees they depend on are real rather than documented.

**Architecture:** Three independent repairs. Add a `REHEARSAL_VERDICTS` enumeration test modelled on the existing `FRAME_VERDICTS` one, so a publisher with no rehearsal gate fails a test instead of nothing. Make `fire_media` generic over `tauri::Runtime` so the last un-driveable fire path becomes testable. Remove a dead command name from two hand-written enumerations and make both assert registration so they cannot rot again.

**Tech Stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest, `cargo test`.

**Spec:** `docs/archive/2026-09-15-timers-templates-stage-design.md` (Wave 0)

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
