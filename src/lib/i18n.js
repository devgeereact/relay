// The operator's language.
//
// ## Why this exists
//
// Relay listens to a sermon in Yorùbá, detects the verse, and then talks to the
// volunteer running it — in English. It understands three African languages and cannot
// speak a word of any of them to its own operator. `docs/LANGUAGES.md` is honest about
// the acoustic gap; this was the gap nobody had written down.
//
// ## Why it is 60 lines and not a dependency
//
// The whole feature is a lookup table and a store. Every i18n library worth the name
// brings a bundler plugin, a message-format parser and an ICU runtime — more code than
// the thing it does, in an app whose first commitment is that it works offline on a
// donated laptop.
//
// ## Translation is a DATA contribution, not a code one
//
// Same rule as `book_aliases.json` and `numerals.json`, and for the same reason: the
// people who can do it are not Rust or Svelte programmers. A Yorùbá speaker adds
// `src/lib/locales/yo.json`, opens a pull request, and never opens a `.svelte` file.
//
// ## Missing keys fall back to English, silently and forever
//
// A half-translated UI is the NORMAL state of this file, not a failure of it. A locale
// ships the day it has one useful string in it, and grows. The alternative — hiding a
// language until it is "complete" — means it is never shipped at all, because a live
// product is never complete.
//
// The one thing that must never happen is a BLANK label. A missing key resolves to
// English; a key missing from English resolves to the key itself, which is ugly and
// visible and therefore gets fixed. It is never empty, and it never throws.

import { derived, writable, get } from 'svelte/store';
import en from './locales/en.json';
import yo from './locales/yo.json';
import sw from './locales/sw.json';
import ha from './locales/ha.json';

/** The languages Relay speaks to its operator. Tier-1 = the languages it LISTENS to. */
export const LOCALES = [
  { code: 'en', label: 'English' },
  { code: 'yo', label: 'Yorùbá' },
  { code: 'sw', label: 'Kiswahili' },
  { code: 'ha', label: 'Hausa' },
];

const CATALOGUE = { en, yo, sw, ha };

const KEY = 'relay.locale.v1';

function initial() {
  try {
    const saved = localStorage.getItem(KEY);
    if (saved && CATALOGUE[saved]) return saved;
  } catch {
    /* no localStorage (SSR, locked-down webview) — English is a fine answer */
  }
  return 'en';
}

/**
 * The current UI language.
 *
 * Stored in localStorage, NOT in the SQLite database — deliberately. This is a property
 * of the person sitting at the machine, not of the church's service data. It must also
 * be readable before the backend is up (the console renders while Tauri is still
 * starting), and it must not be one more thing that can fail to load.
 */
export const locale = writable(initial());

locale.subscribe((v) => {
  try {
    localStorage.setItem(KEY, v);
  } catch {
    /* nothing to do, and nothing worth telling the operator about */
  }
  if (typeof document !== 'undefined') {
    // Screen readers switch voice on this. A Yorùbá UI announced in an English voice is
    // worse than an English UI.
    document.documentElement.lang = v;
  }
});

export function setLocale(code) {
  if (CATALOGUE[code]) locale.set(code);
}

/**
 * Resolve `key` in `code`, falling back to English, then to the key itself.
 *
 * `vars` interpolates `{name}` placeholders:
 *
 *     t('live.now_live', { reference: 'John 3:16' })   // "Now live: John 3:16"
 */
export function translate(code, key, vars) {
  const raw = CATALOGUE[code]?.[key] ?? CATALOGUE.en[key] ?? key;
  if (!vars) return raw;
  return raw.replace(/\{(\w+)\}/g, (m, name) =>
    // An unknown placeholder is left visible rather than blanked. A sentence with a
    // hole in it is a bug you can see; a sentence missing a word silently is not.
    Object.prototype.hasOwnProperty.call(vars, name) ? String(vars[name]) : m,
  );
}

/**
 * The translator, as a store: `$t('app.on_air')` in any component, and every string in
 * the app re-renders the moment the operator changes language.
 */
export const t = derived(locale, ($locale) => (key, vars) => translate($locale, key, vars));

/** For non-component code (stores, errors.js) that needs a string right now. */
export const tNow = (key, vars) => translate(get(locale), key, vars);
