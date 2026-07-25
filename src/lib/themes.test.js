import { describe, it, expect } from 'vitest';
import {
  BUILTIN_THEMES,
  THEME_STYLE_KEYS,
  THEME_TOKENS,
  isThemeToken,
  themeById,
  applyTheme,
  templateThemeRef,
  resolveThemed,
  parseThemes,
  serializeTheme,
  parseImportedTheme,
  THEME_FILE_MARKER,
  applyThemeToTemplate,
} from './themes.js';

describe('themeById', () => {
  it('resolves a builtin by id', () => {
    expect(themeById(-1)).toBe(BUILTIN_THEMES[0]);
  });
  it('resolves a custom theme by id', () => {
    const custom = [{ id: 7, name: 'Mine', style: {} }];
    expect(themeById(7, custom).name).toBe('Mine');
  });
  it('returns null for an unknown id', () => {
    expect(themeById(999)).toBe(null);
    expect(themeById(null)).toBe(null);
  });
});

describe('applyTheme — template overrides theme, key by key', () => {
  const theme = { id: -1, style: { verseColor: '#fff', accent: '#0ff', verseSize: '6' } };

  it('fills keys the template leaves unset', () => {
    const t = { layout: {}, style: { accent: '#f00' } };
    const out = applyTheme(t, theme);
    // template's accent wins; theme fills verseColor + verseSize
    expect(out.style.accent).toBe('#f00');
    expect(out.style.verseColor).toBe('#fff');
    expect(out.style.verseSize).toBe('6');
  });

  it('never mutates the input template or its style', () => {
    const t = { layout: {}, style: { accent: '#f00' } };
    const before = JSON.stringify(t);
    applyTheme(t, theme);
    expect(JSON.stringify(t)).toBe(before);
  });

  it('a null/garbage theme leaves the template look unchanged (never blanks)', () => {
    const t = { style: { verseColor: '#abc' } };
    expect(applyTheme(t, null).style).toEqual({ verseColor: '#abc' });
    expect(applyTheme(t, {}).style).toEqual({ verseColor: '#abc' });
    expect(applyTheme(t, 42).style).toEqual({ verseColor: '#abc' });
  });

  it('only applies whitelisted theme keys — a stray key is ignored', () => {
    const dirty = { style: { verseColor: '#fff', bgImage: 'data:evil', themeRef: 9 } };
    const out = applyTheme({ style: {} }, dirty);
    expect(out.style.verseColor).toBe('#fff');
    expect(out.style.bgImage).toBeUndefined(); // not a theme key
    expect(out.style.themeRef).toBeUndefined();
  });

  it('handles a template with no style at all', () => {
    const out = applyTheme({ layout: { align: 'left' } }, theme);
    expect(out.style.verseColor).toBe('#fff');
    expect(out.layout.align).toBe('left');
  });
});

describe('THEME_STYLE_KEYS is the applied whitelist', () => {
  it('applyTheme applies exactly the whitelisted keys present on the theme', () => {
    const theme = { style: Object.fromEntries(THEME_STYLE_KEYS.map((k) => [k, 'x'])) };
    const out = applyTheme({ style: {} }, theme);
    for (const k of THEME_STYLE_KEYS) expect(out.style[k]).toBe('x');
  });
});

describe('templateThemeRef / resolveThemed', () => {
  it('reads a numeric style.themeRef', () => {
    expect(templateThemeRef({ style: { themeRef: -4 } })).toBe(-4);
    expect(templateThemeRef({ style: {} })).toBe(null);
    expect(templateThemeRef({})).toBe(null);
  });

  it('resolves and applies a pinned builtin theme', () => {
    const t = { style: { themeRef: -4 } }; // Classic → accent #ffb000
    const out = resolveThemed(t);
    expect(out.style.accent).toBe('#ffb000');
  });

  it('an unknown themeRef degrades to the template unchanged', () => {
    const t = { style: { themeRef: 123456, verseColor: '#abc' } };
    const out = resolveThemed(t);
    expect(out.style.verseColor).toBe('#abc');
  });
});

describe('layer theme tokens', () => {
  const layered = (layers) => ({ layout: { layers }, style: {} });

  it('isThemeToken recognises tokens, rejects literals', () => {
    expect(isThemeToken('theme:accent')).toBe(true);
    expect(isThemeToken('#fff')).toBe(false);
    expect(isThemeToken('theme:nope')).toBe(false);
    expect(isThemeToken(null)).toBe(false);
    expect(THEME_TOKENS.length).toBeGreaterThan(0);
  });

  it('resolves a layer colour token to the theme colour', () => {
    const t = layered([{ id: 'a', type: 'text', color: 'theme:accent' }]);
    const out = applyTheme(t, { style: { accent: '#123456' } });
    expect(out.layout.layers[0].color).toBe('#123456');
  });

  it('resolves fill and font tokens too', () => {
    const t = layered([{ id: 'a', type: 'shape', fill: 'theme:background', font: 'theme:font' }]);
    const out = applyTheme(t, { style: { background: '#0a0a0a', font: 'var(--f-body)' } });
    expect(out.layout.layers[0].fill).toBe('#0a0a0a');
    expect(out.layout.layers[0].font).toBe('var(--f-body)');
  });

  it('a token resolves to a LITERAL fallback even with no theme (never emits invalid CSS)', () => {
    const t = layered([{ id: 'a', type: 'text', color: 'theme:verse' }]);
    const out = applyTheme(t, null);
    expect(out.layout.layers[0].color).toBe('#f4e4c8'); // verse fallback
    expect(isThemeToken(out.layout.layers[0].color)).toBe(false);
  });

  it('leaves literal layer colours untouched (fast path, same object identity)', () => {
    const layers = [{ id: 'a', type: 'text', color: '#abcdef' }];
    const out = applyTheme({ layout: { layers }, style: {} }, null);
    expect(out.layout.layers[0]).toBe(layers[0]); // untouched reference
    expect(out.layout.layers[0].color).toBe('#abcdef');
  });

  it('never mutates the input layer', () => {
    const layers = [{ id: 'a', type: 'text', color: 'theme:accent' }];
    applyTheme({ layout: { layers }, style: {} }, { style: { accent: '#111' } });
    expect(layers[0].color).toBe('theme:accent'); // original token intact
  });
});

describe('applyThemeToTemplate — makes a template follow a theme', () => {
  const theme = { id: 5, name: 'X', style: { accent: '#0ff' } };
  const template = {
    name: 'T',
    layout: {
      layers: [
        { id: 'bg', type: 'background', fill: '#000000' },
        { id: 'v', type: 'text', bind: 'verse', color: '#ffffff', font: 'var(--f-serif)' },
        { id: 'r', type: 'text', bind: 'reference', color: '#abcdef' },
        { id: 'l', type: 'text', bind: 'static', color: '#123456' },
        { id: 's', type: 'shape', fill: '#101319' },
      ],
    },
    style: {},
  };

  it('pins the theme and re-tokenises layer colours by role', () => {
    const out = applyThemeToTemplate(template, theme);
    expect(out.style.themeRef).toBe(5);
    const byId = Object.fromEntries(out.layout.layers.map((L) => [L.id, L]));
    expect(byId.bg.fill).toBe('theme:background');
    expect(byId.v.color).toBe('theme:verse');
    expect(byId.v.font).toBe('theme:font');
    expect(byId.r.color).toBe('theme:reference');
    expect(byId.l.color).toBe('theme:accent'); // a fixed label → accent
  });

  it('leaves structural layers (shapes, media) alone', () => {
    const out = applyThemeToTemplate(template, theme);
    expect(out.layout.layers.find((L) => L.id === 's').fill).toBe('#101319');
  });

  it('never mutates the input template', () => {
    const before = JSON.stringify(template);
    applyThemeToTemplate(template, theme);
    expect(JSON.stringify(template)).toBe(before);
  });

  it('is a no-op with no theme', () => {
    expect(applyThemeToTemplate(template, null)).toBe(template);
  });
});

describe('theme export / import', () => {
  it('round-trips name + style, dropping id and builtin', () => {
    const theme = { id: 5, builtin: false, name: 'Sanctuary', style: { accent: '#abc', verseSize: '6' } };
    const back = parseImportedTheme(serializeTheme(theme));
    expect(back).toEqual({ name: 'Sanctuary', style: { accent: '#abc', verseSize: '6' } });
    expect(back.id).toBeUndefined();
    expect(back.builtin).toBeUndefined();
  });

  it('a builtin can be exported and re-imported as a plain custom theme', () => {
    const back = parseImportedTheme(serializeTheme(BUILTIN_THEMES[0]));
    expect(back.name).toBe(BUILTIN_THEMES[0].name);
    expect(back.style.accent).toBe(BUILTIN_THEMES[0].style.accent);
  });

  it('the file carries the marker', () => {
    expect(JSON.parse(serializeTheme({ name: 'x', style: {} })).marker).toBe(THEME_FILE_MARKER);
  });

  it('rejects a non-theme file with a plain-language error', () => {
    expect(() => parseImportedTheme('not json')).toThrow(/valid JSON/);
    expect(() => parseImportedTheme('[]')).toThrow(/isn't a Relay theme/);
    expect(() => parseImportedTheme('{"name":"x","style":{}}')).toThrow(/marker/); // no marker
    expect(() => parseImportedTheme(JSON.stringify({ marker: THEME_FILE_MARKER, name: 'x' }))).toThrow(/no style/);
  });
});

describe('parseThemes — corrupt blob falls back to []', () => {
  it('parses a valid array', () => {
    const blob = JSON.stringify([{ id: 1, name: 'A', style: {} }]);
    expect(parseThemes(blob)).toHaveLength(1);
  });
  it('drops entries missing a numeric id or style object', () => {
    const blob = JSON.stringify([{ id: 1, style: {} }, { name: 'no id' }, { id: 'x', style: {} }, 5]);
    expect(parseThemes(blob)).toHaveLength(1);
  });
  it('returns [] for junk / non-array / empty', () => {
    expect(parseThemes('not json')).toEqual([]);
    expect(parseThemes('42')).toEqual([]);
    expect(parseThemes('null')).toEqual([]);
    expect(parseThemes('')).toEqual([]);
    expect(parseThemes(null)).toEqual([]);
  });
});
