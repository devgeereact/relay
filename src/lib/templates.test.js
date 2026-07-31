import { describe, it, expect } from 'vitest';
import {
  BUILTINS,
  serializeTemplate,
  parseImportedTemplate,
  TEMPLATE_FILE_MARKER,
  appendTemplateVersion,
  sameTemplateShape,
  parseTemplateVersions,
  TEMPLATE_VERSIONS_MAX,
} from './templates.js';

describe('template export / import', () => {
  it('round-trips name + layout + style, dropping id and active', () => {
    const t = {
      id: 9,
      active: true,
      name: 'Lower Third',
      layout: { regions: ['verse_text', 'reference'], lowerThird: true },
      style: { accent: '#8b5cf6', verseSize: '2.6' },
    };
    const back = parseImportedTemplate(serializeTemplate(t));
    expect(back).toEqual({
      name: 'Lower Third',
      layout: { regions: ['verse_text', 'reference'], lowerThird: true },
      style: { accent: '#8b5cf6', verseSize: '2.6' },
    });
    expect(back.id).toBeUndefined();
    expect(back.active).toBeUndefined();
  });

  it('round-trips a layered template', () => {
    const t = {
      name: 'Stage',
      layout: { layers: [{ id: 'a', type: 'text', bind: 'verse', x: 5, y: 5, w: 90, h: 40 }] },
      style: {},
    };
    const back = parseImportedTemplate(serializeTemplate(t));
    expect(back.layout.layers).toHaveLength(1);
    expect(back.layout.layers[0].bind).toBe('verse');
  });

  it('a builtin can be exported and re-imported', () => {
    const back = parseImportedTemplate(serializeTemplate(BUILTINS[0]));
    expect(back.name).toBe(BUILTINS[0].name);
    expect(back.layout).toEqual(BUILTINS[0].layout);
  });

  it('the file carries the marker', () => {
    expect(JSON.parse(serializeTemplate({ name: 'x', layout: {}, style: {} })).marker).toBe(
      TEMPLATE_FILE_MARKER,
    );
  });

  it('a missing style defaults to an empty object, not a throw', () => {
    const text = JSON.stringify({ marker: TEMPLATE_FILE_MARKER, name: 'x', layout: { regions: [] } });
    expect(parseImportedTemplate(text).style).toEqual({});
  });

  it('rejects a non-template file with a plain-language error', () => {
    expect(() => parseImportedTemplate('not json')).toThrow(/valid JSON/);
    expect(() => parseImportedTemplate('[]')).toThrow(/isn't a Relay template/);
    expect(() => parseImportedTemplate('{"name":"x","layout":{}}')).toThrow(/marker/); // no marker
    expect(() =>
      parseImportedTemplate(JSON.stringify({ marker: TEMPLATE_FILE_MARKER, name: 'x' })),
    ).toThrow(/no layout/); // missing layout
    expect(() =>
      parseImportedTemplate(JSON.stringify({ marker: TEMPLATE_FILE_MARKER, name: 'x', layout: [] })),
    ).toThrow(/no layout/); // layout must be an object, not an array
  });
});

describe('template version history', () => {
  const t = (style) => ({ id: 1, name: 'T', layout: { regions: ['verse_text'] }, style });

  it('prepends a new snapshot, newest first', () => {
    let list = appendTemplateVersion([], t({ a: 1 }), 100);
    list = appendTemplateVersion(list, t({ a: 2 }), 200);
    expect(list).toHaveLength(2);
    expect(list[0].ts).toBe(200);
    expect(list[0].style).toEqual({ a: 2 });
  });

  it('dedupes a save identical in shape to the newest (no version for a no-op)', () => {
    let list = appendTemplateVersion([], t({ a: 1 }), 100);
    list = appendTemplateVersion(list, t({ a: 1 }), 200); // same shape
    expect(list).toHaveLength(1);
    expect(list[0].ts).toBe(100); // unchanged
  });

  it('a rename alone is not a new version (name is not part of the shape)', () => {
    let list = appendTemplateVersion([], { ...t({ a: 1 }), name: 'A' }, 100);
    list = appendTemplateVersion(list, { ...t({ a: 1 }), name: 'B' }, 200);
    expect(list).toHaveLength(1);
  });

  it('trims to the max, dropping the oldest', () => {
    let list = [];
    for (let i = 0; i < TEMPLATE_VERSIONS_MAX + 5; i++) list = appendTemplateVersion(list, t({ a: i }), i);
    expect(list).toHaveLength(TEMPLATE_VERSIONS_MAX);
    expect(list[0].style.a).toBe(TEMPLATE_VERSIONS_MAX + 4); // newest kept
    expect(list.at(-1).style.a).toBe(5); // oldest 0..4 dropped
  });

  it('sameTemplateShape compares layout+style, not name', () => {
    expect(sameTemplateShape(t({ a: 1 }), { ...t({ a: 1 }), name: 'other' })).toBe(true);
    expect(sameTemplateShape(t({ a: 1 }), t({ a: 2 }))).toBe(false);
  });

  it('parseTemplateVersions rejects junk', () => {
    expect(parseTemplateVersions('nope')).toEqual([]);
    expect(parseTemplateVersions('{}')).toEqual([]);
    expect(parseTemplateVersions(JSON.stringify([{ layout: {} }, { nope: 1 }]))).toHaveLength(1);
  });
});
