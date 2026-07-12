// A volunteer must never be shown a raw Rust error.
//
// Channels.svelte set `error = String(err)` in five places and rendered it in a
// MONOSPACE font — so a church volunteer got things like
//
//     failed to bind 0.0.0.0:8032: Address already in use (os error 48)
//
// which tells them nothing they can do about it, and reads like the app has crashed.
import { describe, it, expect } from 'vitest';
import { humanError } from './errors.js';

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
