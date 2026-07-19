// A volunteer must never be shown a raw Rust error.
//
// Channels.svelte set `error = String(err)` in five places and rendered it in a
// MONOSPACE font — so a church volunteer got things like
//
//     failed to bind 0.0.0.0:8032: Address already in use (os error 48)
//
// which tells them nothing they can do about it, and reads like the app has crashed.
import { describe, it, expect } from 'vitest';
import { humanError, isRetryable, isRefusal } from './errors.js';

describe('humanError', () => {
  it('a port clash becomes an instruction, not a diagnosis', () => {
    const msg = humanError('failed to bind 0.0.0.0:8032: Address already in use (os error 48)');
    expect(msg).toMatch(/second copy of Relay/i);
    expect(msg).not.toMatch(/0\.0\.0\.0|os error/); // no addresses, no errno
  });

  it('an unplugged projector says what to do about it', () => {
    expect(humanError('monitor index 2 not found')).toMatch(/not connected|plug it back/i);
  });

  it('a missing model points at the place that installs it', () => {
    expect(humanError('no such file: ggml-base.bin')).toMatch(/Settings/);
  });

  it('strips the Error: prefix', () => {
    expect(humanError('Error: something odd')).not.toMatch(/^Error:/);
  });

  // Hiding an error we do not recognise would be worse than showing it — but it is
  // framed as a sentence, so it does not read like a crash.
  it('an unrecognised error is still SHOWN, as a sentence', () => {
    const msg = humanError('flux capacitor desynchronised');
    expect(msg).toContain('flux capacitor desynchronised');
    expect(msg).toMatch(/^That didn't work:/);
  });

  it('never returns nothing at all', () => {
    for (const v of [undefined, null, '', '   ']) {
      expect(humanError(v).length).toBeGreaterThan(0);
    }
  });
});

// ── typed errors from the backend ────────────────────────────────────────────
//
// error.rs now sends `{ kind, message }` instead of a bare string, so the console can
// finally answer the one question a live operator actually has: is pressing this
// button again worth my time?
describe('typed errors', () => {
  it('a refusal keeps the sentence Rust already wrote for the volunteer', () => {
    const e = { kind: 'refused', message: 'A service is being recorded. End it before rehearsing.' };
    expect(humanError(e)).toBe(e.message);
  });

  it('a missing verse is shown as-is, not mangled by the pattern table', () => {
    const e = { kind: 'not_found', message: "John 3:99 isn't in the Bible text — check the reference" };
    expect(humanError(e)).toContain('check the reference');
  });

  it('a busy database tells the operator to try again', () => {
    const e = { kind: 'busy', message: 'Relay is busy saving. Try that again in a moment.' };
    expect(humanError(e)).toMatch(/try that again/i);
    expect(isRetryable(e)).toBe(true);
  });

  // The distinction the type exists for: retrying a locked DB works, retrying a full
  // disk never will. As strings, these were indistinguishable.
  it('knows what is worth retrying and what is not', () => {
    expect(isRetryable({ kind: 'busy', message: 'x' })).toBe(true);
    expect(isRetryable({ kind: 'io', message: 'No space left on device' })).toBe(false);
    expect(isRetryable({ kind: 'internal', message: 'x' })).toBe(false);
    expect(isRetryable('a plain string')).toBe(false);
  });

  it('a refusal is not a fault — nothing is broken', () => {
    expect(isRefusal({ kind: 'refused', message: 'x' })).toBe(true);
    expect(isRefusal({ kind: 'not_found', message: 'x' })).toBe(true);
    expect(isRefusal({ kind: 'internal', message: 'x' })).toBe(false);
  });

  // An `internal`/`io` error is unclassified as far as the operator is concerned, so
  // it still goes through the pattern table — which knows how to make a few of them
  // actionable.
  it('an unclassified error still gets humanised by the pattern table', () => {
    const e = { kind: 'io', message: 'failed to bind 0.0.0.0:8032: Address already in use' };
    expect(humanError(e)).toMatch(/second copy of Relay/i);
  });

  it('never renders [object Object]', () => {
    for (const e of [
      { kind: 'internal', message: 'boom' },
      { kind: 'io', message: 'boom' },
      { kind: 'busy', message: 'boom' },
    ]) {
      expect(humanError(e)).not.toContain('[object');
    }
  });
});

describe('no engine behind the page', () => {
  it('does not show a volunteer a raw JS TypeError from the Tauri bridge', () => {
    // The bug: this exact string reached the FIRST-RUN WIZARD — the first thirty
    // seconds a new operator ever spends in Relay — as
    // "That didn't work: TypeError: Cannot read properties of undefined (reading 'invoke')".
    const msg = humanError(
      new TypeError("Cannot read properties of undefined (reading 'invoke')"),
    );
    expect(msg).not.toMatch(/TypeError/);
    expect(msg).not.toMatch(/undefined/);
    expect(msg).toMatch(/engine is not running/i);
  });
});
