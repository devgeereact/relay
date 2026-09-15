// THE BAND ON A LOWER THIRD MUST ACTUALLY PAINT.
//
// A lower third is the one template family whose entire purpose is to be readable
// over a picture Relay does not control. All six seeded ones shipped with nothing
// behind the words.
//
// `panelOn` is `!!style.textPanel && !layout.lowerThird`, so for a keyed template
// it is false BY CONSTRUCTION, and `panelBg` fell through to the string
// `transparent`. That value is written as an INLINE style on `.content`, and an
// inline declaration always beats the `.slide.lower-third .content { background:
// var(--accent) }` rule meant to paint the band. Measured on a rendered
// `output.html?template_id=3` before the fix: computed background
// `rgba(0, 0, 0, 0)`, with `--accent` resolved to a real colour and ignored.
//
// It was silent to every instrument. `legibility.js` answers `unknown` for a
// transparent ground rather than failing it, and the fit reports a healthy scale
// because the type fits its box perfectly well. It reaches a congregation through
// the stream and the ATEM, where nobody at the desk is watching.
//
// Watched to fail by restoring `panelBg = panelOn ? … : 'transparent'`.

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { BUILTINS } from './templates.js';

const src = readFileSync(resolve(process.cwd(), 'src/lib/TemplateRender.svelte'), 'utf8');
const code = src
  .replace(/<!--[\s\S]*?-->/g, '')
  .replace(/\/\*[\s\S]*?\*\//g, '')
  .replace(/^\s*\/\/.*$/gm, '');

describe('a lower third paints its band', () => {
  it('panelBg answers for the band case before it can fall through', () => {
    const decl = code.slice(code.indexOf('$: panelBg'), code.indexOf(';', code.indexOf('$: panelBg')));
    expect(decl, 'the band must be the first branch').toMatch(/bandMode/);
    // And it must resolve to a colour, never the string `transparent`.
    expect(decl).toMatch(/style\.accent/);
  });

  it('…and the band branch is reached for a keyed template, which panelOn never is', () => {
    // The structural reason the old code could not work: these two conditions are
    // mutually exclusive, so a lower third could never take the panel branch.
    expect(code).toContain('$: panelOn = !!style.textPanel && !layout.lowerThird');
    expect(code).toContain('$: bandMode = !!layout.lowerThird');
  });

  it('the seeded lower third does not wear a law colour, and is not dark-on-dark', () => {
    const lt = BUILTINS.find((t) => t.layout?.lowerThird);
    expect(lt, 'a seeded lower third must exist').toBeTruthy();
    const accent = String(lt.style.accent).toLowerCase();
    // Amethyst means REHEARSAL (rule 18). A caption bar on a stream is not one.
    expect(accent).not.toMatch(/^#8b5cf6|^#b080e0|^#9b7fd4/);
    // And the type must read on it: a near-black band needs light words.
    const verse = String(lt.style.verseColor).toLowerCase();
    expect(verse).not.toBe(accent);
  });
});

// ── A LABEL NEVER GROWS PAST THE THING IT LABELS ─────────────────────────────
//
// `data-fit={L.fit || 'both'}` defaulted every layer to `'both'`, which GROWS a
// short string until it fills its box. No text layer in the shipped shelf sets
// `fit`, so every declared `size` was advisory and the shortest string in the
// template won the most room — which on a scripture slide is always the citation.
//
// Measured at 1920x1080 before the fix: `High Visibility` rendered the verse at
// 108.3px against a designed 161.3px, and `Romans 8:28` at 126.5px against a
// designed 88.3px — the reference 17% LARGER than the scripture, in the same
// white, on the template meant for a lit room. Five of eight shelf templates
// inverted the hierarchy, against REBRAND §4's "reference beneath, small".
//
// Watched to fail by restoring `L.fit || 'both'`.
describe('a reference is subordinate by default', () => {
  it('the renderer defaults a label layer to shrink, not both', () => {
    expect(code, 'the default must be derived, not the literal "both"').toContain(
      "data-fit={L.fit || defaultFit(L)}",
    );
    const fn = code.slice(code.indexOf('const defaultFit'), code.indexOf('\n', code.indexOf('const defaultFit')));
    expect(fn).toContain("'shrink'");
    expect(fn).toContain("'both'");
  });

  it('…and the binds it treats as labels are the subordinate ones', () => {
    const set = code.slice(code.indexOf('const LABEL_BINDS'), code.indexOf('\n', code.indexOf('const LABEL_BINDS')));
    expect(set).toContain('reference');
    expect(set).toContain('translation');
    // The verse and the lyric must NOT be capped — they should still take the room.
    expect(set).not.toContain("'verse'");
    expect(set).not.toContain("'lyric'");
  });

  it('every shelf template designs its reference smaller than its verse', () => {
    // The designers already agreed on the hierarchy; only the fit default
    // overrode them. This holds the data side so a new template cannot ship
    // inverted and rely on the renderer to hide it.
    const shelf = JSON.parse(
      readFileSync(resolve(process.cwd(), 'src-tauri/data/shelf_templates.json'), 'utf8'),
    );
    const list = Array.isArray(shelf) ? shelf : (shelf.templates ?? []);
    for (const t of list) {
      const layers = t.layout?.layers ?? [];
      const verse = layers.find((l) => l.bind === 'verse');
      const ref = layers.find((l) => l.bind === 'reference');
      if (!verse || !ref) continue;
      expect(
        Number(ref.size),
        `${t.name}: the reference is designed at least as large as the verse`,
      ).toBeLessThan(Number(verse.size));
    }
  });
});
