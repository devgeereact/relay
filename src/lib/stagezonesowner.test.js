// THE OPERATOR CHOOSES WHAT THE PREACHER'S SCREEN SHOWS (RG-241).
//
// The operator: *"Allow only operators to select zones for preacher..... remove
// it from the preachers phone"*.
//
// The page carried a **Zones** button in its header, one tap from the screen a
// preacher is reading mid-sermon. It already refused to edit an operator's
// assigned layout — but on an unassigned screen it wrote `localStorage`, so the
// person the screen exists for could switch off the clock they were relying on,
// and nobody at the desk would know.
//
// **What is NOT done here, deliberately.** A device's stored arrangement is
// still READ. A church running a tablet configured by hand last month keeps
// exactly what it had; what it loses is the ability to change it from the
// tablet. There is no silent migration, exactly as the assigned-layout work
// decided when it left that arrangement alone.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';

const STAGE = codeOnly(readFileSync(resolve(__dirname, '../Stage.svelte'), 'utf8'));
const DESK = codeOnly(readFileSync(resolve(__dirname, 'views/Channels.svelte'), 'utf8'));

describe('the phone has no zone picker', () => {
  it('no Zones button in the header', () => {
    expect(STAGE, 'the picker is still one tap from a sermon').not.toMatch(/Choose what this screen shows/);
    expect(STAGE).not.toMatch(/showZones/);
  });

  it('and no panel behind it', () => {
    expect(STAGE).not.toMatch(/class="zonebtn"/);
  });

  it('but a device\u2019s stored arrangement is still READ, so nothing was taken away', () => {
    // The distinction that makes this safe to ship: a tablet configured by hand
    // keeps what it had. It simply cannot be changed from the tablet any more.
    expect(STAGE).toMatch(/relay\.stage\.zones/);
    expect(STAGE).toMatch(/loadZones/);
  });
});

describe('and the desk offers the same switches, plus the size', () => {
  it('Outputs carries every zone the page can draw', () => {
    expect(DESK).toMatch(/STAGE_ZONES/);
  });

  it('and the timer size beside them, on the one delivery path', () => {
    expect(DESK, 'the operator cannot set the timer size').toMatch(/TIMER_SIZES/);
    expect(DESK).toMatch(/timer_size/);
  });
});
