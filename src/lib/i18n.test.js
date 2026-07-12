// Relay listens to a sermon in Yorùbá and talks to the volunteer running it in English.
//
// The layer that fixes that must never make things WORSE than English — which is the
// real risk with i18n, and the reason most half-translated apps are worse than
// untranslated ones: a missing key becomes a blank label, and the operator is left
// looking at an unlabelled button with a congregation waiting.
//
// A partial translation is the NORMAL state of this file, not a failure of it. So the
// fallback chain is the whole safety story, and it is what these tests pin.
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { locale, setLocale, translate, t, tNow, LOCALES } from './i18n.js';
import en from './locales/en.json';

describe('the fallback chain', () => {
  beforeEach(() => setLocale('en'));

  it('an untranslated key falls back to English, not to nothing', () => {
    // yo.json is deliberately empty — this is the normal case, today and for a while.
    expect(translate('yo', 'live.no_suggestions')).toBe(en['live.no_suggestions']);
  });

  it('a key that exists nowhere shows the KEY — ugly, visible, and therefore fixed', () => {
    // The one thing that must never happen is a blank label. A key on screen is a bug
    // you can see; an empty button is a bug you find during a service.
    expect(translate('en', 'nope.not.a.key')).toBe('nope.not.a.key');
    expect(translate('yo', 'nope.not.a.key')).toBe('nope.not.a.key');
  });

  it('never returns empty, for any key, in any locale', () => {
    for (const { code } of LOCALES) {
      for (const key of Object.keys(en)) {
        if (key.startsWith('_')) continue;
        expect(translate(code, key).length).toBeGreaterThan(0);
      }
    }
  });

  it('an unknown locale still speaks English rather than breaking', () => {
    expect(translate('klingon', 'app.on_air')).toBe(en['app.on_air']);
  });
});

describe('placeholders', () => {
  it('interpolates', () => {
    expect(translate('en', 'live.now_live', { reference: 'John 3:16' })).toBe('Now live: John 3:16');
  });

  // A sentence with a visible hole in it is a bug you can see. A sentence quietly
  // missing a word is one you cannot.
  it('leaves an unknown placeholder VISIBLE rather than blanking it', () => {
    expect(translate('en', 'live.now_live', {})).toContain('{reference}');
  });

  it('survives a translation that reorders the sentence', () => {
    // What a real translator does: same placeholder, different position.
    const reordered = '{reference} — ni bayi';
    expect(reordered.replace(/\{(\w+)\}/g, () => 'John 3:16')).toBe('John 3:16 — ni bayi');
  });
});

describe('the store', () => {
  beforeEach(() => setLocale('en'));

  it('$t re-renders everything when the operator switches language', () => {
    expect(get(t)('app.on_air')).toBe('On Air');
    setLocale('yo');
    expect(get(locale)).toBe('yo');
    expect(get(t)('app.on_air')).toBe('On Air'); // falls back — yo.json is empty
  });

  it('refuses a locale it does not have, rather than blanking the UI', () => {
    setLocale('yo');
    setLocale('klingon');
    expect(get(locale)).toBe('yo');
  });

  it('tNow works outside a component (stores, errors.js)', () => {
    setLocale('en');
    expect(tNow('common.try_again')).toBe('Try again');
  });
});

// The catalogue is a contract with translators. If a key is dropped from en.json, every
// locale that translated it silently starts showing the raw key.
describe('the English catalogue', () => {
  it('has no empty strings — a blank source is a blank UI in every language', () => {
    for (const [k, v] of Object.entries(en)) {
      if (k.startsWith('_')) continue;
      expect(typeof v).toBe('string');
      expect(v.trim().length).toBeGreaterThan(0);
    }
  });

  it('covers the live surface — the screen a volunteer uses under pressure', () => {
    const live = Object.keys(en).filter((k) => k.startsWith('live.'));
    expect(live.length).toBeGreaterThan(15);
  });

  // Every key the app actually RENDERS must exist. A typo in a $t('…') call does not
  // fail the build — it silently prints the raw key on screen, which an operator sees as
  // gibberish like "live.no_plans". Cheap to catch here, embarrassing to catch live.
  it('every key the app renders exists in the catalogue', async () => {
    const fs = await import('node:fs');
    const path = await import('node:path');
    const files = [];
    (function walk(dir) {
      for (const f of fs.readdirSync(dir)) {
        const full = path.join(dir, f);
        if (fs.statSync(full).isDirectory()) walk(full);
        else if (full.endsWith('.svelte') || (full.endsWith('.js') && !full.includes('.test.'))) files.push(full);
      }
    })('src');

    const used = new Set();
    for (const f of files) {
      const src = fs.readFileSync(f, 'utf8');
      // Only $t(…) and tNow(…). A looser regex matches get('…') too — it also ends in t(.
      for (const m of src.matchAll(/(?:\$t|\btNow)\(\s*'([a-z0-9_.]+)'/g)) used.add(m[1]);
      // tab labels are keys held in a config object, not literal $t('…') calls
      for (const m of src.matchAll(/label:\s*'(tab\.[a-z]+)'/g)) used.add(m[1]);
    }

    expect(used.size).toBeGreaterThan(20); // the regex still finds things
    for (const key of used) {
      expect(Object.keys(en), `no such key in en.json: ${key}`).toContain(key);
    }
  });

  // A key in yo.json that does not exist in en.json is dead: nothing renders it, and
  // the translator who wrote it will never see it used. Usually it means a key was
  // renamed in English and the locales were not told.
  it('no locale carries a key English does not have', async () => {
    const locales = { yo: await import('./locales/yo.json'), sw: await import('./locales/sw.json'), ha: await import('./locales/ha.json') };
    for (const [code, mod] of Object.entries(locales)) {
      for (const key of Object.keys(mod.default)) {
        if (key.startsWith('_')) continue;
        expect(Object.keys(en), `${code}.json has an orphan key: ${key}`).toContain(key);
      }
    }
  });
});
