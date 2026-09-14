// The browser-source URL is a contract with OBS, and it was built twice — four
// lines apart in the same component. Only one copy was corrected when a screen
// gained the ability to have no look of its own, so Copy URL and the inspector's
// readout said different things about the same screen.
//
// What the tests hold is the rule, not the string: a screen that follows the
// content look says so by saying NOTHING about a template. Writing
// `template_id=1` for a follower starts its browser source on a built-in and
// leaves it there — the operator's window following correctly while the stream
// wears something else, which is the one divergence nobody at the desk sees.
import { describe, it, expect } from 'vitest';
import { outputUrl, OUTPUT_PORT } from './outputurl.js';

describe('outputUrl', () => {
  it('names the channel, which is what a screen IS', () => {
    // `channel` is what makes a live template swap work without a new URL
    // (DECISIONS §29), so it is never optional.
    expect(outputUrl('192.168.1.40', 3, 2, 'Main screen')).toContain('?channel=3');
  });

  it('carries a screen\'s own template', () => {
    expect(outputUrl('192.168.1.40', 3, 2)).toContain('&template_id=2');
  });

  it('says NOTHING about a template when the screen follows the content look', () => {
    for (const none of [null, undefined, '']) {
      expect(outputUrl('192.168.1.40', 3, none), String(none)).not.toContain('template_id');
    }
  });

  it('encodes a name, so a screen called "Foyer & Café" still parses', () => {
    expect(outputUrl('h', 1, 1, 'Foyer & Café')).toContain('&name=Foyer%20%26%20Caf%C3%A9');
  });

  it('leaves the name out when there is none, rather than an empty parameter', () => {
    expect(outputUrl('h', 1, 1)).not.toContain('name=');
  });

  it('is on the port the output pages are served from, not the Vite one', () => {
    // 5032 is Vite and exists only under `npm run tauri dev`; a browser source
    // pointed there shows a blank screen with nothing in any log (CLAUDE.md).
    expect(OUTPUT_PORT).toBe(8032);
    expect(outputUrl('h', 1, 1)).toContain(':8032/output.html');
  });
});
