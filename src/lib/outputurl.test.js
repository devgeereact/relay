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

// 2026-09-21 · M-2 (RG-187). LAN pages are served under `media-src 'self'` while
// the media URL is built from `local_ip()`. An OBS source on this laptop loads
// `http://localhost:8032/output.html` and is handed
// `http://192.168.1.42:8032/media/1`: a different origin, refused by the policy,
// black picture, nothing in any log. The page rewrites Relay's own media host to
// the host it was loaded from; anything else is left alone.
import { sameHostMediaUrl } from './outputurl.js';

describe('sameHostMediaUrl', () => {
  it('rewrites Relay\'s media host to the page\'s own', () => {
    expect(sameHostMediaUrl('http://192.168.1.42:8032/media/1', 'localhost')).toBe('http://localhost:8032/media/1');
    expect(sameHostMediaUrl('http://192.168.1.42:8032/backgrounds/a.png', '10.0.0.7')).toBe('http://10.0.0.7:8032/backgrounds/a.png');
  });
  it('leaves a URL alone when the host already matches, is not port 8032, or is not http', () => {
    expect(sameHostMediaUrl('http://localhost:8032/media/1', 'localhost')).toBe('http://localhost:8032/media/1');
    expect(sameHostMediaUrl('https://cdn.example/x.mp4', 'localhost')).toBe('https://cdn.example/x.mp4');
    expect(sameHostMediaUrl('data:image/png;base64,AAAA', 'localhost')).toBe('data:image/png;base64,AAAA');
  });
  it('does nothing for a page with no host, such as the native window', () => {
    expect(sameHostMediaUrl('http://192.168.1.42:8032/media/1', '')).toBe('http://192.168.1.42:8032/media/1');
    expect(sameHostMediaUrl(null, 'localhost')).toBe(null);
  });
});
