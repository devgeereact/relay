# Wave 2 — Templates ⊕ Themes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish DECISIONS §27 by folding themes into templates, make the configured default template actually reach a screen, reshape the seed into five coordinated families across all five content kinds, separate the two five-chip registers in the template editor, and close the three overflow sources that sit outside the fit loop.

**Architecture:** Six tracks, each landing as its own pull request off `feat/wave7-timers-templates-stage`. Tracks A (default resolver), B (overflow) and C (content-look UI) are independent of each other and of everything else. Track E (theme deletion and migration) runs before Track D (the twenty-five seed), because the seed's five families are what replaces the theme concept and seeding them first would mean migrating rows that were just written. Track F is the browser-driven verification pass the design requires for this wave, and it runs last against everything the five code tracks landed.

**Tech Stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest + jsdom, `cargo test`, Chrome via the browser audit harness.

**Spec:** `docs/superpowers/specs/2026-09-15-timers-templates-stage-design.md` (Wave 2)

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
- Create: `docs/qa/audits/DESIGN-2026-09-15-WAVE2.md` — frozen evidence, findings only. This task creates it; it does not exist yet.
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
