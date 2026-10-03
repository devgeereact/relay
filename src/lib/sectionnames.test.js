import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const read = (p) => readFileSync(resolve(root, p), 'utf8');

/**
 * **AN INSTRUCTION THAT NAMES A SECTION THE APP DOES NOT HAVE.**
 *
 * Found 2026-09-29 while actually running `scripts/offline-bundle.mjs` for the §27
 * launch-gate row. Its `READ ME FIRST.txt` — the one document a church with NO
 * INTERNET has — said *"Go to Settings, then Network"*, and there has never been a
 * Network section. `stt.rs`'s lag warning said *"Settings -> Speech"*, which does not
 * exist either, and I had just propagated that string into a rewrite of that warning
 * without checking it. Both meant `Before the service`.
 *
 * Nothing could have caught it: the strings live in a Rust log line, a doc comment and
 * a generated text file, none of which any test read. A volunteer following the
 * offline README would have hunted for a tab that is not there, which is the worst
 * place in the product for it to happen — there is no internet to look anything up.
 */
describe('an instruction may only name a Settings section that exists', () => {
  const settings = read('src/lib/views/Settings.svelte');
  // The section labels, from the one place that defines them.
  const labels = [...settings.matchAll(/label: '([^']+)'/g)].map((m) => m[1]);

  it('the section list is readable and did not move', () => {
    expect(labels).toContain('Before the service');
    expect(labels.length).toBeGreaterThanOrEqual(8);
  });

  it('nothing tells an operator to open a section that is not there', () => {
    const sources = [
      'src-tauri/src/stt.rs',
      'src-tauri/src/main.rs',
      'scripts/offline-bundle.mjs',
    ];
    const named = [];
    for (const src of sources) {
      const body = read(src);
      // "Settings -> X", "Settings → X", 'Settings, then "X"' — the three shapes the
      // product actually used, two of which were wrong.
      for (const m of body.matchAll(/Settings(?:\s*(?:->|→)\s*|,\s*then\s*)"?([A-Z][A-Za-z ]{2,24}?)"?[.,;`\n]/g)) {
        named.push([src, m[1].trim()]);
      }
    }
    expect(named.length, 'the scanner found no instructions at all — it has stopped seeing them').toBeGreaterThan(0);
    const wrong = named.filter(([, name]) => !labels.includes(name));
    expect(wrong, `these name a Settings section that does not exist: ${JSON.stringify(wrong)}`).toEqual([]);
  });
});
