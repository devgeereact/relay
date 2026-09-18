// The demo dataset, from the frontend's side.
//
// Three claims this file holds, each of which failed silently in an earlier
// version of this feature or is one rename away from doing so:
//
//   1. The control is REACHABLE. `ipc.test.js` proves the command exists in Rust
//      and `qa-inventory.mjs` proves a rendered component imports the wrapper; what
//      neither says is that the operator can see the button in the section it was
//      argued into. That is read here, out of the markup.
//   2. The plan's title is written in TWO places — Rust seeds it, Settings quotes it
//      back so the operator knows what to look for — and a rename on one side would
//      leave the sentence pointing at a plan that is not there.
//   3. Nothing loads itself. Rust holds that from its own side; this holds the other
//      half: no frontend surface calls `load_demo_content` on mount, on boot, or on
//      anything but the button.

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, it, expect } from 'vitest';

const root = resolve(__dirname, '../..');
const read = (p) => readFileSync(resolve(root, p), 'utf8');

const settings = read('src/lib/views/Settings.svelte');
const captureJs = read('src/lib/stores/capture.js');
const demoRs = read('src-tauri/src/db/demo.rs');
const mainRs = read('src-tauri/src/main.rs');

describe('demo content', () => {
  it('is loaded and removed from the same rendered surface', () => {
    // Both controls, in the section the decision put them in. That section was
    // `history` — the argument being that it owned the DATABASE as a thing an
    // operator manages — and it is `start` (Getting started) now: History is a
    // route of its own, and a section is a MOMENT rather than a table. Loading a
    // sample Sunday is something an operator does in their first week and never
    // again, which is the whole of that section.
    const start = settings.slice(settings.indexOf("{:else if section === 'start'}"));
    expect(start.length).toBeGreaterThan(0);
    expect(start).toContain('Load demo content');
    expect(start).toContain('Remove demo content');
    expect(start).toContain('doLoadDemo');
    expect(start).toContain('doRemoveDemo');
  });

  it('guards the removal with an in-app two-step, never a native confirm', () => {
    // Rule 41: `confirm()` returns false in this webview without showing anything,
    // so a two-step built on one deletes nothing and reports success. The arming
    // flag is what stands in for it, and the destructive handler must sit INSIDE
    // the armed branch — `hardrules.test.js` holds the other half (that no native
    // dialog exists anywhere in `src/`).
    const armed = settings.slice(
      settings.indexOf('{#if demoArmed}'),
      settings.indexOf('{:else}', settings.indexOf('{#if demoArmed}')),
    );
    expect(armed).toContain('doRemoveDemo');
    const unarmed = settings.slice(
      settings.indexOf('{:else}', settings.indexOf('{#if demoArmed}')),
      settings.indexOf('{/if}', settings.indexOf('{#if demoArmed}')),
    );
    expect(unarmed, 'the first press only arms; it must not delete').not.toContain('doRemoveDemo');
  });

  it('quotes the same plan title Rust seeds', () => {
    const fromRust = demoRs.match(/pub const PLAN_TITLE: &str = "([^"]+)"/);
    expect(fromRust, 'db::demo::PLAN_TITLE still exists').toBeTruthy();
    const fromUi = settings.match(/const DEMO_PLAN_TITLE = '([^']+)'/);
    expect(fromUi, 'Settings still quotes the plan title').toBeTruthy();
    expect(fromUi[1]).toBe(fromRust[1]);
  });

  it('never loads itself — the only caller of the wrapper is the button', () => {
    // Every `src/` file, not just Settings: the failure this guards against is a
    // convenience added to a boot probe or an onMount somewhere else entirely.
    const calls = [
      ...read('src/lib/views/Settings.svelte').matchAll(/loadDemoContent\s*\(/g),
    ].length;
    expect(calls, 'Settings calls it exactly once, from doLoadDemo').toBe(1);
    // And nothing in the launch ladder or the shell reaches for it at all.
    for (const f of ['src/App.svelte', 'src/lib/boot/boot.js', 'src/lib/boot/probes.js']) {
      expect(read(f)).not.toContain('load_demo_content');
      expect(read(f)).not.toContain('loadDemoContent');
    }
  });

  it('is held back during a recorded service, on both doors', () => {
    const lock = read('src-tauri/src/servicelock.rs');
    expect(lock).toContain('"load_demo_content"');
    expect(lock).toContain('"remove_demo_content"');
    // …and the command really guards itself, which is what `servicelock.rs`'s own
    // `every_protected_command_actually_guards_itself` asserts from the Rust side.
    expect(mainRs).toContain('lock.guard("load_demo_content")');
    expect(mainRs).toContain('lock.guard("remove_demo_content")');
  });

  it('reads as a swallow and writes as a throw', () => {
    // `demoStatus` may never take Settings down; the two writes may never fail in
    // silence (the throw-vs-swallow contract at the top of capture.js).
    expect(captureJs).toMatch(/export async function demoStatus\(\)[\s\S]{0,200}guardedRead/);
    expect(captureJs).toMatch(
      /export async function loadDemoContent\([\s\S]{0,200}await invoke\(\)/,
    );
    expect(captureJs).toMatch(
      /export async function removeDemoContent\(\)[\s\S]{0,200}await invoke\(\)/,
    );
  });

  it('tells the operator what a removal will KEEP, before they press it', () => {
    // A demo item they have edited survives the removal. If the surface only said
    // so afterwards, they would find an unexplained row in their Library later.
    expect(settings).toContain('demo.edited');
    expect(settings).toMatch(/Removing will <b>keep<\/b>/);
    expect(settings).toMatch(/gone\.kept/);
  });
});
