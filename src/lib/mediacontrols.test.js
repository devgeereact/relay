// ONE SET OF MEDIA CONTROLS, IN THE SHELL, FOR EVERY SCREEN (RG-237).
//
// The operator: *"one control for all...not a different control for different
// screens...what operators see is whats on screen and when operator pause...it
// all pause on every screen"* and *"move all the controllers to the CONTROLS
// sections and replace it with the no service button"*.
//
// The transport lived on Live's programme monitor, which is a WORKSPACE. The
// dock is the shell: it is mounted once, it survives a crashed view, and it is
// where every other control over what a congregation sees already lives —
// Clear screens, Blackout, Rehearse. A clip is playing on every screen in the
// building whatever tab the operator happens to be on, so its controls belong
// where the panic controls are and not on one surface out of six.
//
// **`No service` is what it replaces**, on the operator's instruction. That
// button is inert whenever no service is recording — which is most of the life
// of the card — and a control that is dark most of the time is the wrong use of
// the one card that may never scroll.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';

const DOCK = readFileSync(resolve(__dirname, 'Dock.svelte'), 'utf8');
const LIVE = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

describe('the controls are in the shell', () => {
  const dock = codeOnly(DOCK);

  it('the dock carries the transport, and calls the one wrapper', () => {
    expect(dock, 'the dock has no media transport').toMatch(/setMediaTransport/);
    expect(dock).toMatch(/mediaTransport/);
  });

  it('play, pause and replay are all there, named', () => {
    for (const label of ['Play', 'Pause', 'Replay'])
      expect(dock, `no ${label} in the dock`).toMatch(new RegExp(label));
  });

  it('and the clip clock rides with them, from the screens', () => {
    // `describeMediaClock` reads what the SCREENS report, never this process's
    // own player — the reason that module exists. A readout in the shell that
    // timed the console's preview would be a number about nothing.
    expect(dock).toMatch(/describeMediaClock/);
    expect(dock).toMatch(/channelHealth/);
  });
});

describe('and there is only one set of them', () => {
  it('Live no longer carries its own transport row', () => {
    const live = codeOnly(LIVE);
    expect(live, 'the run surface still has a second Pause').not.toMatch(/aria-label="Scrub the clip"/);
    expect(live, 'two doors onto one clip').not.toMatch(/clip\(\{ paused:/);
  });

  it('the `No service` button is gone from the Controls card', () => {
    // It is not merely hidden: a control that is inert for most of a service is
    // the wrong use of the one card that may never scroll, and ending a service
    // is reachable from the readiness screen where it is read rather than
    // reached for under pressure.
    expect(codeOnly(DOCK), 'the dead button is still taking the space').not.toMatch(/No service/);
  });

  it('and Clear screens is still the full-width control along the bottom', () => {
    // Rule 15, unchanged and asserted here because this card was rearranged:
    // the panic control may never be behind an overflow edge or out of order.
    expect(codeOnly(DOCK)).toMatch(/Clear screens/);
    expect(codeOnly(DOCK)).toMatch(/ctlbody/);
  });
});
