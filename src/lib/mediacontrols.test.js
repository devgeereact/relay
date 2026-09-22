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

// RG-254: the transport left the Controls card for a strip of its own in the
// shell. Still one set, still in the shell, still not on Live — which is the
// whole of RG-237's guarantee, and is why the move did not have to weaken it.
const DOCK = readFileSync(resolve(__dirname, 'ClipBar.svelte'), 'utf8');
const DOCKCARD = readFileSync(resolve(__dirname, 'Dock.svelte'), 'utf8');
const LIVE = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

describe('the controls are in the shell', () => {
  const dock = codeOnly(DOCK);

  it('the dock carries the transport, and calls the one wrapper', () => {
    expect(dock, 'the dock has no media transport').toMatch(/setMediaTransport/);
    expect(dock).toMatch(/mediaTransport/);
  });

  it('play, pause and replay are all there, named', () => {
    // THE WORDS LEFT WITH THE CARD (RG-254). The operator asked for *"just the
    // Icons like the play icon, pause icon and all"*, so the controls are
    // shapes now — which means the NAME has to be somewhere a screen reader
    // reaches, and an `aria-label` is that place. Asserting the visible word
    // would be asserting the thing the operator asked to remove.
    for (const label of ['Hold the clip', 'Let the clip run', 'again from the beginning'])
      expect(dock, `nothing is labelled "${label}"`).toContain(label);
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
    expect(codeOnly(DOCKCARD), 'the dead button is still taking the space').not.toMatch(/No service/);
  });

  it('and Clear screens is still the full-width control along the bottom', () => {
    // Rule 15, unchanged and asserted here because this card was rearranged:
    // the panic control may never be behind an overflow edge or out of order.
    // THE CARD, not the strip: `Clear screens` never moved, and this case is
    // about the card that was rearranged around it.
    expect(codeOnly(DOCKCARD)).toMatch(/Clear screens/);
    expect(codeOnly(DOCKCARD)).toMatch(/ctlbody/);
    // AND THE STRIP DOES NOT CARRY A PANIC CONTROL. It is rendered only while a
    // clip is up, so a copy of Clear screens on it would be a panic control
    // that comes and goes — which is rule 15 inverted.
    expect(codeOnly(DOCK), 'a panic control that disappears with the clip').not.toMatch(/Clear screens/);
  });
});
