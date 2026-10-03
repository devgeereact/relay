// ── THE MICROPHONE MAY BE CHANGED WHILE RELAY IS LISTENING (RG-291) ─────────
//
// The operator: *"when audio input switch, continue transcript once audio is
// dected..."*
//
// Both pickers were disabled during capture, and that was HONEST rather than
// broken: `setInputDevice` only wrote the store and the setting, so the device
// was chosen when capture opened and at no other time. Disabling the control was
// the right answer to a capability that did not exist — the Dock's own title
// said so out loud.
//
// It exists now. `setInputDevice` stops the running capture and opens the new
// device, and a stop that did not stop ends there rather than opening a second
// capture on one device (`micstop.test.js`'s bug, one level up). So the control
// is handed back, and this file is what stops it being taken away again by a
// tidy-up that does not know why it was disabled.
//
// WHY THIS MATTERS ON A SUNDAY: the case is not somebody browsing a menu. It is
// a desk feed dying mid-sermon with a handheld already plugged in, and
// `servicelock.rs` deliberately does not hold this back — §40, "the operator
// outranks it, always".
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';

const read = (p) => codeOnly(readFileSync(resolve(p), 'utf8'));
const DOCK = read('src/lib/Dock.svelte');
const SETTINGS = read('src/lib/views/Settings.svelte');

/** The whole `<select>` tag for the microphone picker in a file. */
function picker(src, file) {
  const at = src.indexOf('aria-label="Microphone input device"');
  expect(at, `no microphone picker in ${file}`).toBeGreaterThan(-1);
  const open = src.lastIndexOf('<select', at);
  return src.slice(open, src.indexOf('>', at) + 1);
}

describe('both doors onto the microphone picker', () => {
  it('the scanner finds a real picker in each file', () => {
    // Guards the guard: a slice that found nothing would make every assertion
    // below pass over an empty string.
    expect(picker(DOCK, 'Dock.svelte')).toMatch(/<select/);
    expect(picker(SETTINGS, 'Settings.svelte')).toMatch(/<select/);
  });

  it('neither is disabled merely because Relay is listening', () => {
    for (const [src, file] of [[DOCK, 'Dock.svelte'], [SETTINGS, 'Settings.svelte']]) {
      const tag = picker(src, file);
      // THE `disabled` ATTRIBUTE ALONE. `capturing` is still read in the TITLE,
      // legitimately — the tooltip says a different thing while a capture is
      // running — so a blunt scan of the whole tag would forbid the fix as well
      // as the defect.
      const dis = /disabled=\{([^}]*)\}/.exec(tag);
      expect(dis, `${file} has no disabled attribute at all`).toBeTruthy();
      expect(dis[1], `${file} still disables the picker during capture`).not.toMatch(/capturing/);
      // The bridge test stays: with no engine there is nothing to open.
      expect(dis[1], `${file} lets the picker be used with no engine`).toMatch(/!\$capture\.available/);
    }
  });

  it('and neither still tells the operator to stop listening first', () => {
    // A title that survives its own rule is worse than no title: it is an
    // instruction to do something unnecessary, from the product itself.
    for (const [src, file] of [[DOCK, 'Dock.svelte'], [SETTINGS, 'Settings.svelte']]) {
      expect(picker(src, file).toLowerCase(), `${file} still says to stop listening first`)
        .not.toMatch(/stop listening to change/);
    }
  });

  it('the capability they depend on is real, and lives in one place', () => {
    // The control is only safe to enable because `setInputDevice` actually moves
    // a running capture. If that is ever reverted, these pickers become the lie
    // they used to be honest about — so this asserts the mechanism, not the
    // markup, and `inputdevice.test.js` drives it for real.
    const CAP = codeOnly(readFileSync(resolve('src/lib/stores/capture.js'), 'utf8'));
    const at = CAP.indexOf('export async function setInputDevice');
    expect(at, 'setInputDevice is gone').toBeGreaterThan(-1);
    // To the next top-level `}` — this function's own closing brace sits at
    // column 0, and so does the next declaration's, so a slice to the first one
    // is exactly its body.
    const body = CAP.slice(at, CAP.indexOf('\n/**', at));
    expect(body, 'setInputDevice no longer moves a running capture').toMatch(/moveRunningCapture/);
    // And only when one was running. Reopening a capture nobody started would
    // turn choosing a microphone into switching the microphone ON.
    expect(body, 'it moves a capture that was never running').toMatch(/wasCapturing/);
    // The move itself must do both halves, in that order — a stop that did not
    // stop must never be followed by a start (`micstop.test.js`, one level up).
    const mAt = CAP.indexOf('async function moveRunningCapture');
    expect(mAt, 'the move is gone').toBeGreaterThan(-1);
    // TO THE NEXT DECLARATION, not to the first `\n}` — this file writes function
    // bodies un-indented, so a brace at column 0 is as likely to be a `catch`'s
    // as the function's own. A slice that stopped there read three lines and
    // reported that `startCapture` was missing from a function that calls it.
    const nextDecl = CAP.indexOf('\nasync function', mAt + 1);
    const move = CAP.slice(mAt, nextDecl > -1 ? nextDecl : CAP.length);
    expect(move).toMatch(/stopCapture/);
    expect(move).toMatch(/startCapture/);
    expect(move.indexOf('stopCapture'), 'it starts before it stops').toBeLessThan(move.indexOf('startCapture'));
  });
});
