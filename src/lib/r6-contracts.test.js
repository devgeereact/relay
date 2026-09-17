// R6 · Independent Auditor — layer-C contract tests, written before reading R1–R5.
//
// Precedent: `src/lib/ipc.test.js`. These parse source and assert relationships
// between files, which is the only instrument that can reach a guarantee's TWIN —
// the surface nobody listed. Three of this repo's real bugs are that shape.
//
// Tests marked RED ON PURPOSE are findings. Do not "fix" them by relaxing the
// assertion; the assertion is the audit.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

function walk(dir, out = []) {
  for (const e of fs.readdirSync(path.join(ROOT, dir), { withFileTypes: true })) {
    const rel = path.join(dir, e.name);
    if (e.isDirectory()) walk(rel, out);
    else if (rel.endsWith('.svelte')) out.push(rel);
  }
  return out;
}
const SVELTE = walk('src').filter((f) => !f.includes('__r6probe'));

// ─────────────────────────────────────────────────────────────────────────────
// R6-3 · The blackout does not reach the preacher's screen.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * RED ON PURPOSE — R6-3.
 *
 * `channels.rs` publishes four content-bearing message kinds to the kiosk hub:
 * `content`, `clear`, `black`, `stage_next`. Two browser clients consume that hub:
 *
 *   Output.svelte  — the projector / OBS browser source.  Handles black. ✓
 *   Stage.svelte   — the preacher's phone / stage monitor. Does NOT handle black.
 *
 * So when the operator hits `B`, or the Blank Screen tile, or `/api/black` from the
 * LAN, the congregation's wall goes black and the screen the preacher is READING FROM
 * keeps the verse. The console reports success, correctly — the message did leave the
 * machine. Nobody is told that one of the screens ignored it.
 *
 * This is the `stage_next` leak in mirror image: the same twin, the same door, the
 * opposite direction. `stage_next` sent something it should not have; `black` fails to
 * arrive somewhere it must.
 */
it('R6-3: every kiosk client has a DECISION about every kind the hub publishes', () => {
  const rust = read('src-tauri/src/channels.rs') + read('src-tauri/src/main.rs');

  // Derive the kinds from the Rust rather than hardcoding them. Hardcoding is what
  // made the original miss possible: a list of two, written by someone who had the
  // two in mind. This fails when a FIFTH kind is added and a client is forgotten.
  const all = [...new Set([...rust.matchAll(/"kind"\s*:\s*"([a-z_]+)"/g)].map((m) => m[1]))];

  // INBOUND kinds travel client → hub. A browser client having no branch for one
  // is correct, not an oversight, so they are excluded here — but they are named
  // explicitly rather than pattern-matched, because a list that silences a kind is
  // exactly where an OUTBOUND kind could hide and reproduce the original finding.
  // The assertion below is what stops that: each of these must appear on the
  // server's READ path, which is the only place an inbound message is handled.
  const INBOUND = ['hello', 'beat', 'rendered'];
  const server = read('src-tauri/src/channels.rs');
  for (const k of INBOUND) {
    expect(
      server,
      `"${k}" is excused from the per-client contract as an INBOUND message, so it ` +
        `must be handled on the hub's read path. If it is now published TO clients, ` +
        `take it out of INBOUND and give every client a verdict.`,
    ).toMatch(new RegExp(`Some\\("${k}"\\)`));
  }

  const published = all.filter((k) => !INBOUND.includes(k)).sort();

  // Every kind needs an explicit verdict PER CLIENT. "Not applicable" is a fine
  // answer; silence is not, because silence is indistinguishable from an oversight
  // and that is the whole finding.
  const EXPECTED = {
    'src/Output.svelte': {
      content: true,
      clear: true,
      black: true, // a panic control
      channel_template: true,
      template: true,
      stage_next: false, // monitor-only field; no congregation template renders it
      // THE STAGE MESSAGE. This was `false`, and the `false` WAS the
      // guarantee: docs/REBRAND.md §5's "no congregation screen can show it" held
      // because this page had no branch for the frame at all — an omission, which
      // is a fine guarantee right up until somebody has a reason to end it.
      //
      // The `stage_message` layer binding is that reason. A renderer reads the
      // value now, so the absence protects nothing and the refusal has to be a
      // decision the page takes out loud: it accepts the frame ONLY when its own
      // channel's role is `stage`. The filter is at the receiver because the hub
      // records nothing about who connected and DECISIONS §35 is not being
      // reversed; the page knows its own channel because the URL is channel-keyed
      // (DECISIONS §29).
      //
      // `true` here now means "has a branch", which on its own is weaker than
      // what the `false` used to mean. `src/lib/stagemessage.test.js` holds the
      // rest, by driving the real page and asserting on what it PAINTS — which is
      // the claim, and is a thing a source scan cannot reach.
      stage_alert: true,
      // WHAT EACH SCREEN IS FOR — the fact the filter above is taken on. Sent on
      // every hello and whenever the operator changes a role, so this page can
      // answer "am I the stage?" and stop being one the moment it is not.
      channel_roles: true,
      // The operator's transition override (DECISIONS §84). A congregation screen
      // is the whole point of it — a picker that moved the console preview and left
      // every OBS source cutting would be rule 35 on the one surface a congregation
      // is looking at. Held here, not inferred from the absence of a call.
      transition: true,
      // THE CONFIGURED DEFAULT — the resolver's final fallback link (wave 2, task 2).
      // A congregation screen with no template of its own and no content look either
      // must still end its chain somewhere other than the bundled Classic Serif.
      default_template: true,
      // THE PROGRAMME TIMERS — `false`, and this is the same guarantee as
      // `stage_next` and `stage_alert` above rather than a new one. A programme
      // timer is the operator's bookkeeping for one person on a platform: "Sermon ·
      // 4:12 left" behind a preacher is the running order in front of the whole
      // building. The congregation's own countdown is a different thing entirely
      // and reaches this page as ordinary `content` with the four `countdown_*`
      // fields on it, which is why nothing is lost by refusing this one.
      timer: false,
      // THE STANDING BACKGROUND — the church's own picture, behind everything.
      // This is a congregation screen and the backdrop is congregation furniture,
      // so of course it is handled here. What the branch does on `clear` and
      // `black` is the part worth naming: it drops the backdrop, on both doors,
      // because a panic control means everything and the picture is part of
      // everything. `src/lib/backdroppage.test.js` drives the real page and
      // asserts on what it PAINTS, which is the claim; this row only says a
      // branch exists.
      background: true,
      // WHICH SCREENS THE OPERATOR TOOK OUT OF THE WALL. This page IS a screen, so
      // it is the party that has to act on it — the hub broadcasts to everybody and
      // records nothing about who connected (DECISIONS §35), which is the same
      // argument as `channel_template` two rows up and `stage_alert` above it.
      screen_state: true,
    },
    'src/Stage.svelte': {
      content: true,
      clear: true,
      black: true, // WAS false, and that was the finding — see the note above
      stage_next: true,
      stage_alert: true, // the whole point of the message
      channel_template: false, // the stage page has one fixed look
      template: false,
      // Same reason as template/channel_template above: the stage page does not
      // render through TemplateRender, so a template-shaped fallback has nothing
      // to resolve for it.
      default_template: false,
      // DELIBERATELY NOT, and for the same reason the three above are not: the
      // stage page has one fixed look and does not render through
      // `TemplateRender`, so there is no slide for a transition to be of. It is
      // also the screen a preacher reads from mid-sermon, and motion there is a
      // distraction nobody asked for. A transition is about how a CONGREGATION
      // screen changes (DECISIONS §84).
      transition: false,
      // THIS VERDICT WAS `false`, AND THE REVERSAL IS THE POINT OF THE ROW.
      //
      // The reasoning recorded here was that `stage.html` is not an output
      // channel — "it has no `?channel=` in its URL, and it is a stage screen by
      // construction rather than by configuration". That was true of the page's
      // INTENT and false of its behaviour. Every copy of this page open anywhere
      // on the network was a stage screen by construction, including a lobby TV
      // somebody had pointed at the URL and a spare tablet in a back room, and
      // each of them was painted the Stage Message full-bleed. One of two doors
      // carried DECISIONS §89's guarantee and this was the other one.
      //
      // So the page now reads `?channel=`, refuses a Stage Message unless its own
      // channel holds the `stage` role, and says plainly when it has no channel
      // at all rather than refusing in silence (rule 35). It is not a security
      // boundary and is not claimed as one: the LAN is trusted by decision (§35,
      // docs/SECURITY.md T4) and anyone may type `?channel=2`. It closes the
      // accident, by the same mechanic and in the same frame as `output.html`.
      // `stagepageidentity.test.js` drives the real page and asserts what it
      // paints, which this file — which reads source text — cannot reach.
      channel_roles: true,
      // THE PROGRAMME TIMERS, and this page is the only client that may have them.
      // A `Stage`-scoped timer publishes no content frame at all — which is exactly
      // why it survives a verse — so this frame is its only way onto any screen,
      // and the screen it is for is this one.
      timer: true,
      // THE STANDING BACKGROUND — DELIBERATELY NOT, and for the same two reasons
      // `template`, `channel_template`, `default_template` and `transition` above
      // are not. This page has one fixed look and does not render through
      // `TemplateRender`, so there is no layer stack for a backdrop to take its
      // place in; a picture painted here would have to be a second, hand-rolled
      // way of drawing one, which is the WYSIWYG guarantee failing one level above
      // the renderer.
      //
      // And it is the right answer anyway. A backdrop is decoration chosen for a
      // CONGREGATION, and this is the screen a preacher reads from mid-sermon:
      // a photograph behind the words is precisely how scripture becomes
      // unreadable, and the one reader of this page cannot fix it from where they
      // are standing. Nothing is lost — the congregation's screens carry the
      // picture, this one carries the words.
      background: false,
      // AND THE PER-SCREEN CONTROL REACHES BOTH DOORS. A confidence monitor in a
      // foyer is a screen an operator may reasonably want down while the
      // platform's own stays up, and this page is the door this repository has
      // now missed four times — most recently for `black`, which left the verse
      // on the one screen the preacher reads from while the console correctly
      // reported success (DECISIONS §91).
      screen_state: true,
    },
  };

  const problems = [];
  for (const [file, expected] of Object.entries(EXPECTED)) {
    const src = read(file);
    for (const kind of published) {
      if (!(kind in expected)) {
        problems.push(
          `${file}: no decision recorded for the new hub message "${kind}" — add a row`,
        );
        continue;
      }
      const handled = new RegExp(`kind\\s*===\\s*['"]${kind}['"]`).test(src);
      if (expected[kind] && !handled) problems.push(`${file} ignores "${kind}"`);
      if (!expected[kind] && handled) {
        problems.push(`${file} now handles "${kind}" — update the expectation and say why`);
      }
    }
  }

  // AND THE OTHER DIRECTION, which this table did not have.
  //
  // The loop above walks the kinds the hub PUBLISHES and requires a verdict for
  // each. A verdict for a kind nobody publishes any more is invisible to it —
  // and that is not hypothetical: `themes` sat here with a verdict on both
  // clients after the hub frame was deleted (DECISIONS §87), reading as a
  // guarantee about a message that no longer exists. `channels.rs`'s
  // `FRAME_VERDICTS` has carried the reverse guard for exactly this reason since
  // it was written ("a verdict about nothing"); this is its client-side twin and
  // was missing it. Watched to fail by putting the `themes` rows back.
  for (const [file, expected] of Object.entries(EXPECTED)) {
    for (const kind of Object.keys(expected)) {
      if (!published.includes(kind) && !INBOUND.includes(kind)) {
        problems.push(
          `${file}: a verdict is recorded for "${kind}", which the hub no longer publishes`,
        );
      }
    }
  }

  expect(
    problems,
    'R6-3: a panic control that reaches three of four screens is not a panic control. ' +
      "`Stage.svelte` (the preacher's phone, stage.html) had no `black` branch, so a " +
      'blackout left the verse up on the one screen the preacher reads from while the ' +
      'console correctly reported success — the message HAD left the machine. This is ' +
      'the `stage_next` leak in mirror image: same twin, same door, opposite direction. ' +
      'Every kind now needs a per-client verdict, so the next one cannot be forgotten ' +
      'quietly.',
  ).toEqual([]);

  // The panic kinds specifically: both clients, always, no exceptions available.
  for (const kind of ['clear', 'black']) {
    expect(published, `the hub must publish ${kind}`).toContain(kind);
    for (const file of Object.keys(EXPECTED)) {
      expect(EXPECTED[file][kind], `${file} must honour the panic kind ${kind}`).toBe(true);
    }
  }
});

// ─────────────────────────────────────────────────────────────────────────────
// R6-4 · Escape closes a dropdown everywhere in computing. Here it clears the wall.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * R6-4 — CLOSED 2026-08-14 (P1-3). Inverted rather than deleted: the defect class,
 * not the individual menu, is what has to stay dead.
 *
 * `shortcuts.js` suppressed the Escape → clear-the-screens panic action only for a
 * mounted `[role="dialog"]`. That guard exists because "dismissing a help overlay or
 * an arrangement picker is not a live action — it used to wipe the wall as a
 * side-effect" (architecture rule 16). Six popup menus and the console crash panel
 * were then built outside the one role it knew, so Escape in any of them cleared the
 * congregation's screens AND left the overlay open. Two were on surfaces used DURING
 * a service: the Countdown picker in the run column, and the VerseDeck kebab.
 *
 * The repair is two-layer, because the guard must not depend on anybody remembering:
 *
 *   1. `shortcuts.js` recognises the whole overlay class — dialog, alertdialog,
 *      menu, listbox. An overlay that forgets everything else still cannot clear
 *      the wall.
 *   2. Each menu consumes Escape itself, so the operator gets the outcome they
 *      actually asked for and not merely the absence of the one they did not.
 *
 * This test holds layer 1 and the precondition layer 1 depends on: that every
 * transient menu DECLARES a role the DOM probe can see. A popup with no role is
 * invisible to the guard, and that is how two of these got built.
 */
it('R6-4: the Escape guard covers the whole overlay class, and every menu declares itself', () => {
  const shortcuts = read('src/lib/shortcuts.js');
  for (const role of ['dialog', 'alertdialog', 'menu', 'listbox']) {
    expect(
      shortcuts.includes(`[role="${role}"]`),
      `the global Escape guard must recognise role="${role}" — an overlay kind it ` +
        'cannot see is an overlay that clears the congregation\'s screens',
    ).toBe(true);
  }

  // Every floating menu must carry a role the guard recognises. `role="menu"` here
  // is load-bearing markup, not decoration.
  const roleless = [];
  for (const f of SVELTE) {
    const src = read(f);
    for (const m of src.matchAll(/<div class="[a-z-]*menu"([^>]*)>/g)) {
      if (!/role=/.test(m[1])) roleless.push(`${f} → ${m[0].trim()}`);
    }
  }
  expect(
    roleless,
    'R6-4: these floating menus declare no ARIA role, so `shortcuts.js`\'s DOM probe ' +
      'cannot see them and Escape falls through to the panic handler — the menu stays ' +
      'open AND the wall goes blank mid-reading.',
  ).toEqual([]);
});

// ─────────────────────────────────────────────────────────────────────────────
// R6-5 · `String(e)` on a typed error is "[object Object]".
// ─────────────────────────────────────────────────────────────────────────────

/**
 * RED ON PURPOSE — R6-5.
 *
 * `error.rs` serialises every command failure as `{ kind, message }` (serde tag).
 * Tauri rejects `invoke` with that OBJECT, not a string. So `String(e)` produces the
 * literal text `[object Object]` — and three views render exactly that to a volunteer,
 * two of them in `r-mono`, which is the monospace-raw-Rust-error presentation the
 * repo already fixed once on Channels.
 */
it('R6-5: a typed backend error stringifies to "[object Object]"', () => {
  const typed = { kind: 'io', message: 'No space left on device' };
  expect(String(typed)).toBe('[object Object]');
  expect(`Export failed: ${typed}`).toBe('Export failed: [object Object]');
});

it('R6-5: no view renders a caught backend error without the one humaniser', () => {
  const offenders = [];
  for (const f of SVELTE) {
    const src = read(f);
    const lines = src.split('\n');
    lines.forEach((l, i) => {
      const m = l.match(/^\s*(?:let\s+)?(\w+)\s*=\s*(?:String\((\w+)\)|`[^`]*\$\{(\w+)\}[^`]*`)\s*;?\s*$/);
      if (!m) return;
      const target = m[1];
      const errVar = m[2] || m[3];
      // Only interested when the right-hand side is the variable a catch just bound.
      const inCatch = lines
        .slice(Math.max(0, i - 4), i)
        .some((p) => new RegExp(`catch\\s*\\(\\s*${errVar}\\s*\\)`).test(p));
      if (!inCatch) return;
      // …and only when that variable is actually rendered.
      if (!new RegExp(`\\{${target}\\}`).test(src)) return;
      offenders.push(`${f}:${i + 1}  ${l.trim()}`);
    });
  }
  expect(
    offenders,
    'R6-5: src/lib/errors.js is the ONE humaniser (CLAUDE.md). These sites bypass it ' +
      'and render the raw rejection. Because error.rs sends a typed OBJECT, what the ' +
      'volunteer actually reads is "[object Object]" — in monospace, on two of them.',
  ).toEqual([]);
});

// ─────────────────────────────────────────────────────────────────────────────
// R6-6 · The Preview half of Preview/Programme has no input.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * RED ON PURPOSE — R6-6.
 *
 * `Preview ≠ Programme` is the sixth live-safety distinction. `LiveOutputRail.svelte`
 * renders the Preview pane, its badge and its "Take to screen →" button, all keyed off
 * the `preview` prop. `Library.svelte` — the ONLY renderer of that component — binds
 * `preview={staged}`, and `staged` is assigned in exactly one place: `stage(d)`.
 *
 * `stage()` has no callers. Not a button, not a keyboard path, not an event, not a
 * store subscription. So `staged` is `null` for the life of the process:
 *
 *   • the Preview monitor can never show anything
 *   • "Take to screen →" is `disabled={!preview || …}` — permanently disabled
 *   • the Preview badge, and the "Wall live" chip shipped during this audit, are both
 *     unreachable branches
 *
 * The distinction is not mis-signalled. It does not exist.
 */
it('R6-6: the Library run column has no unreachable preview half', () => {
  // CLOSED 2026-08-15 (audit P1-2) by REMOVAL. R6's instruction was "decide what
  // Preview IS in the Library, and either wire it or remove the whole column — do
  // not fix the badge again", and it was aimed at this session, which had fixed the
  // badge against a prop nothing could supply.
  //
  // The decision: remove. `stage()` was built for AI suggestions ("browsing FIRES"),
  // its `_fire` was `confirmDetection`, and the Heard panel fires on one press. Two
  // implementations of Preview ≠ Programme — one real on `Live.svelte`, one
  // unreachable here — is the shape that produced most of this audit.
  // Strip line comments before matching. The file EXPLAINS the removal in prose,
  // and a grep that a comment can trip is a grep that will trip on the next one —
  // the same lesson the `aria-modal` assertion learned an hour earlier.
  const code = (p) => read(p).replace(/^\s*\/\/.*$/gm, '');
  const lib = code('src/lib/views/Library.svelte');
  const rail = code('src/lib/views/library/LiveOutputRail.svelte');

  expect(lib.match(/\bstage\s*\(/g) ?? [], 'stage() is gone, not orphaned again').toEqual([]);
  expect(rail).not.toMatch(/export let preview/);

  // …and the control that WOULD have crossed the line from here is gone with it.
  // Going live from the Library is the queue.
  expect(rail).not.toMatch(/class="lo-take"/);
  expect(rail).toMatch(/take\(queue\)/);
});

// ─────────────────────────────────────────────────────────────────────────────
// GREEN — things I checked and found holding. Each carries its command.
// ─────────────────────────────────────────────────────────────────────────────

it('R6-G1: the app makes no outbound network call except the model download and the updater', () => {
  const net = [];
  for (const f of [...SVELTE, 'src/lib/updater.js', 'src/lib/stores/capture.js']) {
    const src = read(f);
    if (/\bfetch\s*\(/.test(src) || /new WebSocket\(/.test(src)) net.push(f);
  }
  // Both remaining users talk to Relay's own LAN ports, not the internet.
  expect(net.sort()).toEqual(['src/Output.svelte', 'src/Stage.svelte']);
  for (const f of net) {
    const src = read(f);
    expect(/https:\/\//.test(src.replace(/^\s*\/\/.*$/gm, ''))).toBe(false);
  }
});

it('R6-G2: {@html} never receives backend, verse or template data', () => {
  // `{@html}` IS used, in three places, and all three interpolate a hardcoded
  // module-level constant (an SVG icon map, or a literal help table). None of them can
  // be reached by a verse, a transcript, a template file or anything from Rust — so
  // the injection surface is closed, but by inspection rather than by construction.
  // This allow-list is the construction: a new {@html} anywhere else fails here.
  const ALLOWED = new Set([
    'src/App.svelte:icons[tab.key]',
    'src/lib/views/Settings.svelte:ICONS[s.icon]',
    'src/lib/views/Help.svelte:step',
    'src/lib/views/Help.svelte:detail',
  ]);
  const found = [];
  for (const f of SVELTE) {
    for (const m of read(f).matchAll(/\{@html\s+([^}]+)\}/g)) found.push(`${f}:${m[1].trim()}`);
  }
  expect(found.filter((x) => !ALLOWED.has(x))).toEqual([]);
  // And the renderer that actually reaches a wall must have none at all.
  expect(/\{@html/.test(read('src/lib/TemplateRender.svelte'))).toBe(false);
  expect(/\{@html/.test(read('src/Output.svelte'))).toBe(false);
  expect(/\{@html/.test(read('src/Stage.svelte'))).toBe(false);
});

it('R6-G3: amber is reserved — no error or rehearsal surface borrows the tally colour', () => {
  // Amber means ON AIR and is never allowed to lie. An error styled amber is a tally
  // light that lies; a rehearsal styled amber is worse. (`!$rehearsing ? 'amber'` is
  // correct and must not match — the negation is the whole point.)
  const bad = [];
  for (const f of SVELTE) {
    const src = read(f);
    for (const m of src.matchAll(/class="[^"]*\b(err|error)\b[^"]*amber[^"]*"/g)) bad.push(`${f}: ${m[0]}`);
    for (const m of src.matchAll(/(?<![!])\$rehears\w*\s*\?\s*'amber'/g)) bad.push(`${f}: ${m[0]}`);
  }
  expect(bad).toEqual([]);
});

describe('R6 · instrument notes', () => {
  it('records that "no humanError in the file" is NOT evidence of a raw error', () => {
    // Channels.svelte has zero `humanError` calls and is CORRECT: it stores the typed
    // error and renders it through `ui/ErrorState.svelte`, which humanises. Any audit
    // finding that greps for `humanError` and calls Channels a regression is wrong.
    const ch = read('src/lib/views/Channels.svelte');
    expect(ch.includes('humanError')).toBe(false);
    expect(ch.includes('<ErrorState')).toBe(true);
    expect(read('src/lib/ui/ErrorState.svelte').includes('humanError')).toBe(true);
  });
});
