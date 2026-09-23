// THE BUILT-IN DISPLAY IS AN OUTPUT LIKE ANY OTHER (RG-274).
//
// The operator: *"this computer built in display can be used for output for
// monitoring purpose"*.
//
// **Checked, and it already can** — but nothing said so in one place, and the
// obvious way to get this wrong is to filter the primary display out of the
// picker on the argument that the console is already on it. That argument is
// wrong for the case the operator is describing: a laptop on a desk with the
// projector on HDMI is a second window on the same screen, which is exactly how
// an operator watches what a congregation is seeing without turning round.
//
// So this pins the absence of that filter, from both ends.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const read = (p) => readFileSync(resolve(p), 'utf8');

describe('every display the OS reports is offered', () => {
  it('the Rust side filters none of them out', () => {
    const src = read('src-tauri/src/channels.rs');
    const fn = src.slice(src.indexOf('let Ok(monitors) = app.available_monitors()'));
    const body = fn.slice(0, fn.indexOf('.collect()'));
    // A `.filter(` here is the only way the built-in display could be withheld.
    expect(body, 'a display is being withheld from the operator').not.toMatch(/\.filter\(/);
    // And the primary is MARKED rather than removed, so an operator can tell
    // which one they are already looking at.
    expect(body).toMatch(/primary:/);
  });

  it('the picker lists them all, and says which is primary', () => {
    const src = read('src/lib/views/Channels.svelte');
    expect(src).toMatch(/\{#each monitors as m \(m\.index\)\}/);
    expect(src).toMatch(/m\.primary \? ' \(primary\)' : ''/);
    // NOT hidden, NOT disabled: the built-in display is a legitimate monitoring
    // output, and a laptop beside a projector is the ordinary small-church rig.
    const each = src.slice(src.indexOf('{#each monitors as m (m.index)}'));
    const option = each.slice(0, each.indexOf('{/each}'));
    expect(option, 'the primary display is offered but not selectable').not.toMatch(/disabled/);
  });
});
