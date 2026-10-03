// 2026-09-21 · Phase 1 audit E-4/E-5 (RG-196). The Help screen described a
// purple band across the top of the app and an "Open output" control on the
// Live tab. Neither exists: the wall state is the bottom status bar since the
// 2026-09-14 chrome clean-out, and a screen is turned on in Outputs → Screens.
// The page a volunteer opens under pressure must describe the build they have.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const help = readFileSync(resolve(process.cwd(), 'src/lib/views/Help.svelte'), 'utf8');

describe('Help describes the shell that ships', () => {
  it('sends the operator to Outputs → Screens to turn a screen on, not to a Live control that is not there', () => {
    expect(help).not.toMatch(/On the <b>Live<\/b> tab, click <b>Open output<\/b>/);
    expect(help).toMatch(/Outputs → Screens/);
    expect(help).toMatch(/Turn on/);
  });
  it('describes the rehearsal signal where it actually is', () => {
    expect(help).not.toMatch(/purple band sits across the top/);
    expect(help).toMatch(/status bar/);
  });
});
