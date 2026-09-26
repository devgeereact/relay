// THE LARGEST IRREVERSIBLE ACTION IN THE PRODUCT, MADE REVERSIBLE AND VISIBLE.
//
// Opening the Templates tab converts every legacy region template to layers, in
// place. On a fresh install that is roughly 31 templates, and each write goes
// through `save_template`, which is not merely a persist: `main.rs` pushes the
// fresh JSON into `KioskHub::set_template` and emits `template://updated` to every
// native output window. So one click on a tab republished a church's whole shelf
// to every screen in the building.
//
// Three things were wrong with how it did that, and this file holds all three.
// Each was watched to fail by restoring the original line:
//
//   1 · NO WAY BACK — no `snapshotTemplateVersion` before the write, so the
//       version History menu offered nothing to restore. Restoring the old body
//       (no snapshot call) turns the first test RED.
//   2 · A SWALLOWED CATCH — `.catch(() => {})` per template, so a refusal on
//       template 7 of 31 was invisible and the operator saw a half-converted shelf
//       with no message. Restoring it turns the second and third tests RED.
//   3 · MID-SERVICE — `save_template` is not on `servicelock::PROTECTED`, so this
//       ran while a church was recording. Removing the `$serviceLock` branch turns
//       the fourth test RED.
//
// It still runs ON MOUNT rather than lazily, deliberately, and the fifth test
// holds that on purpose: ten files reason about the conversion having already
// happened — `templateKind.js` reads `layout.lowerThird` off the CONVERTED shape
// to keep six of the operator's eight lower thirds in the Quick tools picker, and
// `nameband.test.js` records the real database ids that depend on it. Going lazy
// is a separate change with its own evidence.

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';

const src = readFileSync(
  resolve(process.cwd(), 'src/lib/views/templates/TemplateGallery.svelte'),
  'utf8',
);

/** The function body, comments stripped — a claim in prose is not behaviour. */
const body = (() => {
  const from = src.indexOf('async function upgradeLegacyToLayers');
  const to = src.indexOf('\n  }', src.indexOf('finally', from));
  return codeOnly(src.slice(from, to));
})();

describe('converting the shelf is reversible, visible, and not done mid-service', () => {
  it('banks a version BEFORE it overwrites a template', () => {
    expect(body, 'the conversion must be recoverable from History').toContain(
      'snapshotTemplateVersion(t)',
    );
    // Order is the whole claim: a snapshot written after the overwrite protects
    // against nothing.
    expect(body.indexOf('snapshotTemplateVersion(t)')).toBeLessThan(
      body.indexOf('saveTemplateQuiet('),
    );
  });

  it('no longer swallows a failed conversion', () => {
    expect(body, 'the per-template swallow is the defect').not.toMatch(
      /saveTemplateQuiet\([^)]*\)[\s\S]{0,40}\.catch\(\s*\(\)\s*=>\s*\{\s*\}\s*\)/,
    );
    // It must collect what failed rather than merely not-swallowing.
    expect(body).toMatch(/failed\.push\(/);
  });

  it('reports the outcome, and names what could not be converted', () => {
    expect(body).toMatch(/upgradeNote\s*=/);
    expect(body, 'a partial conversion must say so').toMatch(/Converted \$\{done\} of/);
    expect(body, 'and name the templates it could not do').toMatch(/failed\.join\(/);
    // The two outcomes must not read alike — `bad` is what colours the failure.
    expect(body).toMatch(/upgradeBad\s*=\s*failed\.length\s*>\s*0/);
  });

  it('declines while a service is being recorded, and says so', () => {
    expect(body, 'a shelf rewrite must not run mid-service').toMatch(
      /\$serviceLock\?\.engaged/,
    );
    const guard = body.slice(body.indexOf('$serviceLock?.engaged'));
    expect(guard.slice(0, 260), 'and it must tell the operator, not fail silently').toMatch(
      /upgradeNote\s*=/,
    );
    // Before any write.
    expect(body.indexOf('$serviceLock?.engaged')).toBeLessThan(
      body.indexOf('saveTemplateQuiet('),
    );
  });

  it('the operator can actually read the outcome — it is rendered, not just set', () => {
    // A count nothing renders is the swallowed catch with extra steps.
    expect(src).toMatch(/\{#if upgradeNote\}/);
    expect(src).toMatch(/class:bad=\{upgradeBad\}/);
    expect(src, 'good news must not interrupt — status, never alert').toMatch(
      /tg-upgnote[^>]*role="status"/,
    );
    expect(src).toMatch(/\.tg-upgnote\.bad\{/);
  });

  it('still runs on mount, because ten files depend on that', () => {
    // Pinned so the "convert lazily" half cannot be done as a drive-by. It is a
    // real improvement and it falsifies `templateKind.js`'s lower-third path and
    // `nameband.test.js`'s recorded ids, so it needs its own evidence.
    const mount = src.slice(src.indexOf('onMount('), src.indexOf('onMount(') + 400);
    expect(mount).toContain('upgradeLegacyToLayers()');
  });
});
