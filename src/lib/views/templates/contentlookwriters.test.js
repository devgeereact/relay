// WHO MAY SET A CONTENT LOOK, AND WHERE THE ONE ANSWER LIVES.
//
// A content look is a GLOBAL binding: tick Scripture against a template and
// every screen set to *Follow the content look* wears it when scripture fires
// (DECISIONS §25 and §70). `setContentTemplate` is the one writer of that map —
// three surfaces once held private copies of it and silently disagreed, which is
// why the store exists at all.
//
// One of the surfaces writing it was the template editor, immediately above
// `Content this template renders` — a per-template filter on `layout.shows`,
// which is a different fact entirely and was rendered as a visually identical
// five-chip row over the same five labels. An operator's first click on the
// filter made four chips go dark at once, reported as "clicking a content look
// activates all"; nothing was wrong with either handler, the render was telling
// them something false. Wave 2 gave the filter a different control shape. This
// removes the register it was being confused with, from the one surface where
// the two sat together.
//
// THE FEATURE IS NOT DELETED, ONLY ITS COPY IN THE EDITOR. `Channels.svelte`
// keeps the authoritative matrix. This file is the census that keeps that claim
// checkable, because "we removed the duplicate" is exactly the kind of statement
// that stops being true without anybody noticing.
import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve, join } from 'node:path';

const root = resolve(__dirname, '../../../..');
const read = (p) => readFileSync(resolve(root, p), 'utf8');

/** Every source file under `src/`, so the census cannot quietly narrow to a
 *  hand-written list — this repository has had a scanner do exactly that twice,
 *  and a scanner that narrows passes everything. */
function sources(dir = 'src', out = []) {
  for (const e of readdirSync(resolve(root, dir), { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) sources(p, out);
    else if (/\.(svelte|js)$/.test(e.name) && !/\.test\.js$/.test(e.name)) out.push(p);
  }
  return out;
}

/** A file that CALLS the writer, as opposed to one that merely names it in a
 *  comment. Both the editor and the gallery carry paragraphs about it. */
const callsTheWriter = (src) => /(?:await\s+|=\s*)setContentTemplate\s*\(/.test(src);

describe('the content look has one writer and a known set of callers', () => {
  it('the template editor is not one of them any more', () => {
    const src = read('src/lib/views/templates/TemplateEditor.svelte');
    expect(callsTheWriter(src)).toBe(false);
    expect(src).not.toMatch(/function\s+toggleUsedFor/);
    // The import went with it, so nothing here can reach the map by accident.
    expect(src).not.toMatch(/import\s*\{[^}]*setContentTemplate/);
  });

  it('keeps the per-template FILTER, which looks the same and is not the same fact', () => {
    // `layout.shows` is a per-screen allow-list read at runtime by
    // `Output.svelte` and `layers.js::templateShows`: untick Media and a stage
    // monitor wearing this template ignores a picture and holds the passage. It
    // has nothing to do with the global binding and it stays.
    const src = read('src/lib/views/templates/TemplateEditor.svelte');
    expect(src).toMatch(/function\s+toggleShows/);
    expect(src).toMatch(/Content this template renders/);
    expect(src).toMatch(/templateShows\(/);
  });

  it('names the surfaces that may write it, and fails when a new one appears', () => {
    // THE RULING ON THE GALLERY'S COPY, recorded here rather than in a commit
    // message nobody will find. It STAYS. The harm the editor's copy did was
    // specific to the editor: two identical chip rows, one above the other,
    // over the same five labels. The gallery inspector has no such twin beside
    // it — it is the panel an operator browses from, answering "what is this
    // template for", and it was deliberately built to replace a read-only
    // Usage tab that could only signpost the control it was describing. It
    // writes through the same one writer, so it cannot disagree with anything.
    //
    // Settings is the FOURTH caller, which the plan for this wave did not know
    // about when it counted three. It is a select per kind rather than a chip
    // grid, on a page with no lookalike beside it, and changing it is outside
    // this track — it is recorded here so the next reader starts from four.
    const callers = sources().filter((p) => callsTheWriter(read(p))).sort();
    expect(callers).toEqual([
      'src/lib/views/Channels.svelte',        // the authoritative matrix
      'src/lib/views/Settings.svelte',        // one select per kind
      'src/lib/views/templates/TemplateGallery.svelte', // the inspector's chips
    ]);
    // The scanner can still see a real instance — this repository has twice
    // shipped a scanner that quietly narrowed and therefore passed everything.
    expect(callsTheWriter(read('src/lib/views/Channels.svelte'))).toBe(true);
  });

  it('changes nothing in the engine', () => {
    // The design's own condition: `tpl_{kind}`, `set_content_template`,
    // `ContentTemplates`, `cue_or_content_tpl` and the content-look link inside
    // `resolveOutputTemplate` are untouched, and §29's resolution order stands.
    expect(read('src-tauri/src/main.rs')).toMatch(/fn set_content_template/);
    expect(read('src-tauri/src/pipeline.rs') + read('src-tauri/src/main.rs')).toMatch(/cue_or_content_tpl/);
    expect(read('src/lib/layers.js')).toMatch(/export function resolveOutputTemplate/);
  });
});
