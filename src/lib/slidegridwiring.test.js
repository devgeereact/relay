// THE SLIDE GRID, ON THE SURFACE THAT RENDERS IT.
//
// `slidegrid.test.js` pins the arbitration: a double press never fires both. That
// guarantee is worth nothing if the component reaches around it — and this
// repository's recurring bug is exactly that shape: a rule enforced on one door
// and skipped on its twin (CLAUDE.md, "A guarantee is only kept on the doors you
// checked"). So this file asks the questions the pure tests cannot:
//
//   · does anything render the grid at all? (a component nothing renders is not
//     covered, however green its tests)
//   · do BOTH press handlers go through the one arbiter?
//   · does anything else in the view call the fire path directly, behind it?
//   · do the panic controls and the teardown disarm a press that is already armed?
//
//   npx vitest run src/lib/slidegridwiring.test.js
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const src = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

describe('the grid is rendered, and its presses go through the arbiter', () => {
  it('Live renders a grid of cells from the staged source', () => {
    expect(src).toMatch(/<div class="sgrid">/);
    expect(src).toMatch(/\{#each grid\.cells as c \(c\.key\)\}/);
    expect(src).toMatch(/gridSource\(\{/);
  });

  it('a cell is a real button, so the keyboard and a screen reader can reach it', () => {
    const cell = src.match(/<button\s+class="sg-cell"[\s\S]*?>/);
    expect(cell).not.toBeNull();
  });

  // The whole point. A second click path — an `on:click` that called `fireCell`
  // or `fireSlide` straight — would put the slide on the wall on its way to a
  // preview, which is the accident the 190ms beat exists to prevent.
  it('BOTH handlers go through the one arbiter, and there is no second path', () => {
    expect(src).toMatch(/on:click=\{\(\) => gridPress\.press\(c\)\}/);
    expect(src).toMatch(/on:dblclick=\{\(\) => gridPress\.double\(c\)\}/);
    // `fireCell` is reachable from exactly two places: the arbiter's `send`, and
    // `take()` taking what the operator previewed. Nothing else may call it.
    const calls = [...src.matchAll(/fireCell\(/g)].length;
    expect(calls).toBe(2);
    expect(src).toMatch(/send: fireCell,/);
    expect(src).toMatch(/if \(gridPreview\) return fireCell\(gridPreview\);/);
  });

  it('the grid never builds output by hand — it reuses the existing fire paths', () => {
    // CLAUDE.md: never hand-roll an OutputContent. A plan cell is fireSlide; a
    // verse cell is the same manualFire the search box uses.
    const body = src.slice(src.indexOf('async function fireCell('), src.indexOf('const gridPress'));
    expect(body).toMatch(/return fireSlide\(item, cell\.slideIdx\);/);
    expect(body).toMatch(/await manualFire\(cell\.reference\);/);
    expect(body).not.toMatch(/fireContent\(/);
  });

  it('a fire that fails is reported — the arbiter is given somewhere to put it', () => {
    expect(src).toMatch(/onError: \(e\) => flash\(humanError\(e\)\),/);
  });

  // A press armed a beat ago, landing after a blackout, is a verse appearing on a
  // wall the operator just took down. Rule 15's family: a panic control that can
  // be undone 190ms later is not a panic control.
  //
  // Live no longer owns a Clear screens button — the dock does, one row below, on
  // every workspace. So the disarm cannot hang off a handler in this file: it
  // watches the STORE, which is where Esc, the dock button and a spoken clear all
  // land. Watching the button would have left three of the four ways a wall gets
  // cleared able to be undone 190ms later.
  it('a wall going clear disarms a pending press, however it was cleared', () => {
    const watch = src.slice(src.indexOf('unsubLive = live.subscribe('), src.indexOf('unsubLive = live.subscribe(') + 500);
    expect(watch).toMatch(/if \(wasLive && !now\) \{/);
    expect(watch).toMatch(/gridPress\.cancel\(\);/);
    // The preacher's "up next" must not outlive the content it was about, and
    // that one reports its own failure rather than going quiet.
    expect(watch).toMatch(/setStageNext\(null, null\)/);
    expect(watch).toMatch(/\.catch\(/);
    expect(src.slice(src.indexOf('onDestroy(('), src.indexOf('onDestroy((') + 700))
      .toMatch(/gridPress\.cancel\(\);/);
  });

  it('a cell reads ON AIR from the store, never from "we pressed the button"', () => {
    const rule = src.slice(src.indexOf('$: cellLive = '), src.indexOf('$: cellLive = ') + 400);
    // Plan cells: the store's own onAir + cursor. Verse cells: what the store
    // says is on screen, and not while the wall is black.
    expect(rule).toMatch(/planOnAir && c\.cueId === liveCueId && c\.slideIdx === liveSlide/);
    expect(rule).toMatch(/!\$screenBlack && \$liveContent\?\.reference === c\.reference/);
    // Nothing optimistic: the arbiter's cell is not consulted.
    expect(rule).not.toMatch(/gridPress|pressed/);
  });

  it('amber is ON AIR and steel blue is the preview — never the other way round', () => {
    expect(src).toMatch(/\.sg-cell\.islive \.sg-thumb\{border-color:var\(--v-amber\)/);
    expect(src).toMatch(/\.sg-cell\.cued \.sg-thumb\{border-color:var\(--v-sel\)/);
    // And the state is said in words as well as in colour.
    expect(src).toMatch(/class="sg-air">On Air</);
    expect(src).toMatch(/class="sg-prev">Preview</);
  });

  // A CELL IS THE WALL IN MINIATURE (docs/REBRAND.md §2). The grid drew a grey
  // box with the label in it, so a cell said "Romans 8:28-31" and nothing about
  // what a congregation would actually see. These four are the whole claim: it
  // renders, it renders through the ONE renderer, the box is a container query
  // so cqw resolves against the thumbnail, and the fit report stays the wall's.
  it('a cell RENDERS the slide, through the one renderer', () => {
    const cell = src.slice(src.indexOf('<span class="sg-thumb">'), src.indexOf('</span>\n                <span class="sg-meta">'));
    expect(cell).toMatch(/<TemplateRender template=\{cellTemplate\(c\) \?\? \{\}\} content=\{cellContent\(c\)\} \/>/);
    // No second renderer, and no second fit path.
    expect(src).not.toMatch(/SlideThumb|MiniRender|fitCell/);
  });

  it('the thumb is a CONTAINER, or every cell renders at the page width', () => {
    const thumb = src.slice(src.indexOf('.sg-thumb{'), src.indexOf('.sg-thumb{') + 400);
    expect(thumb).toMatch(/position:relative/);
    expect(thumb).toMatch(/aspect-ratio:16\/9/);
    expect(thumb).toMatch(/container-type:inline-size/);
  });

  // Rule 37's "this is rendering at 38% of its designed size" is the WALL's
  // measurement, and there must be exactly one of it. Twenty thumbnails
  // reporting their own fit would bury the one that matters.
  it('a thumbnail never reports a fit — `noteFit` has one caller, the Program pane', () => {
    expect([...src.matchAll(/onFit=\{noteFit\}/g)]).toHaveLength(1);
    const cell = src.slice(src.indexOf('<span class="sg-thumb">'), src.indexOf('</span>\n                <span class="sg-meta">'));
    expect(cell).not.toMatch(/onFit/);
  });

  // A lyric slide projects the lyric. `fire_content` suppresses a song's title
  // (rule 36 — one place decides), so a thumbnail that printed it would be
  // showing the operator something no congregation will ever see.
  it('a song thumbnail carries NO title — it shows what the wall shows', () => {
    const body = src.slice(src.indexOf('$: cellContent = '), src.indexOf('$: cellContent = ') + 300);
    expect(body).toMatch(/reference: c\.ctype === 'song' \? null : c\.label/);
  });

  // `planCells` draws a cue it could not expand rather than dropping it — a real
  // eight-cue plan rendered six cells and nothing said which two were missing.
  // Drawn, named, counted, and refused: there is genuinely nothing to send.
  it('a cue with nothing to show is drawn and DISABLED, not silently dropped', () => {
    expect(src).toMatch(/class:isempty=\{c\.empty\}/);
    expect(src).toMatch(/disabled=\{!\$capture\.available \|\| c\.empty\}/);
    expect(src).toMatch(/class="sg-void">Nothing to show</);
    // And it says why, rather than being a dead cell an operator keeps pressing.
    expect(src).toMatch(/has nothing to show — open it in the Planner/);
  });

  it('muted text never sits on --v-surf3 (tokencontrast.test.js fails the build for it)', () => {
    const sg = src.slice(src.indexOf('.sg-body{'), src.indexOf('.sg-cap{') + 200);
    const onSurf3 = [...sg.matchAll(/background:var\(--v-surf3\);\s*color:var\(--v-([a-z0-9-]+)\)/g)];
    for (const m of onSurf3) expect(m[1]).not.toBe('faint');
  });
});
