// THE LONG TAIL OF THE BUTTON — wave 3, agent B2. docs/REBRAND.md §1.
//
// `workspacegrammar.test.js` holds the SHARED rules: `.r-btn` draws the house
// fill, one edge, one red, and every button is given a font. B1 fixed all four
// in `src/app.css`, and every one of them is about the instrument rather than
// about its use. That left the thing an operator actually sees, which B1
// measured and could not finish: **181 hand-rolled buttons across 30 files**,
// each drawing its own height, corner, fill, edge and type.
//
// This file covers the Templates and Library workspaces — the two worst files
// in that census (TemplateEditor at 31, TemplateGallery at 18) plus VerseDeck at
// 17 and their neighbours. It holds two claims, and they are deliberately
// different in kind:
//
//   1. A BUTTON IN A ROW OF BUTTONS USES THE SHARED CONTROL, and having used it
//      does not then redraw it. This is mechanical and is checked as such.
//   2. EVERYTHING ELSE IS NAMED AND SAYS WHAT IT IS. A tab, a chip, a list row,
//      a grid cell, an icon-only kebab, a card that happens to be a <button> —
//      these are not buttons and forcing them into `.r-btn` would change how the
//      surface reads. What this file requires of them is a CLASS and a
//      COMMENT, because the only thing that separates a deliberate shape from
//      drift is somebody having said which it is.
//
// (2) is the unusual assertion and it is the point. A shape census — the lead's
// browser probe grouping every button by computed height · radius · font-size ·
// weight · background · border · colour — cannot tell a chip from a button that
// lost its class. A name can. So the rule is not "look like the shared control";
// it is "either be the shared control, or be a named thing with a reason".
//
// WHAT THIS CANNOT DO, stated so nobody reads it as more: it reads source, not a
// rendered box. A converted button whose parent flexes it to 40px still passes
// here. Only the lead's browser can answer what a control measures.
//
// Each assertion below was watched to fail with its own defect reintroduced.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const read = (f) => readFileSync(resolve(__dirname, '../../..', f), 'utf8');

// The two workspaces this pass covers, in full. A file is in the list because
// its buttons were audited one at a time, not because a glob matched it —
// widening it is the work, quietly dropping one out to make a build green is
// not. The rest of the 30-file census is other agents' and other waves'.
const FILES = [
  'src/lib/views/Library.svelte',
  'src/lib/views/Templates.svelte',
  'src/lib/views/library/Announcements.svelte',
  'src/lib/views/library/Arrangements.svelte',
  'src/lib/views/library/Browse.svelte',
  'src/lib/views/library/Collections.svelte',
  'src/lib/views/library/History.svelte',
  'src/lib/views/library/ImportReview.svelte',
  'src/lib/views/library/Inspector.svelte',
  'src/lib/views/library/LiveOutputRail.svelte',
  'src/lib/views/library/LyricsPane.svelte',
  'src/lib/views/library/MediaLibrary.svelte',
  'src/lib/views/library/Scripture.svelte',
  'src/lib/views/library/VerseDeck.svelte',
  'src/lib/views/templates/DeskStrip.svelte',
  'src/lib/views/templates/TemplateEditor.svelte',
  'src/lib/views/templates/TemplateGallery.svelte',
  'src/lib/views/themes/ThemeEditor.svelte',
  'src/lib/views/themes/ThemeGallery.svelte',
];

// Shared instruments, all defined in `src/app.css`. A button wearing one of
// these has opted into the house shape and is not hand-rolled.
const SHARED = ['r-btn', 'r-iconbtn', 'r-cbtn'];
// Shared CONTAINERS whose members are styled by the container, not by a class
// of their own: `.r-seg` is app.css's segmented control and its buttons are
// deliberately classless. `.rw-item` is WorkspaceFrame's rail row.
const SHARED_GROUP = ['r-seg', 'rw-item'];

const styleOf = (src) => {
  const i = src.lastIndexOf('<style>');
  return i === -1 ? '' : src.slice(i);
};
const markupOf = (src) => {
  const i = src.indexOf('<style');
  return (i === -1 ? src : src.slice(0, i))
    // The `<script>` block is not markup. VerseDeck's JSDoc on the row handler
    // says *"the GRID card is a native `<button>`"* — a scanner that counted
    // that would report an anonymous button in a paragraph of prose, and the
    // fix for it would be to delete the explanation.
    .replace(/<script[\s\S]*?<\/script>/g, '')
    // Same for HTML comments. Four `<button>`s in this repository live inside a
    // comment EXPLAINING why a menu needs no keydown handler.
    .replace(/<!--[\s\S]*?-->/g, '');
};

/** Every `<button>` tag in a component's markup, with its class attribute. */
function buttons(src) {
  const markup = markupOf(src);
  const spans = sharedGroupSpans(markup);
  return [...markup.matchAll(/<button\b[^>]*?>/gs)].map((m) => {
    const tag = m[0];
    const cls = (tag.match(/class="([^"]*)"/) || [, ''])[1];
    return {
      tag: tag.replace(/\s+/g, ' ').slice(0, 110),
      // Only the STATIC half. `class:on={…}` is a state, never a shape.
      classes: cls.split(/\s+/).filter(Boolean),
      // Is this button INSIDE a shared group container? Per button, not per
      // file — see the comment on `sharedGroupSpans`.
      inGroup: spans.some(([a, b]) => m.index > a && m.index < b),
    };
  });
}

/**
 * The character ranges covered by a shared group container (`.r-seg`, a rail).
 *
 * WHY THIS IS NESTING-AWARE rather than "does the file mention `.r-seg`". The
 * first version of this asked the FILE, and that is not an exemption, it is an
 * amnesty: `TemplateGallery.svelte` renders one `.r-seg` tab strip, so every
 * anonymous button anywhere in its 700 lines was excused — including the four
 * row-menu items this pass exists to have named. Watched: stripping `class`
 * from a `.tg-menu` item and running the suite gave a green pass.
 *
 * So it walks the element. Open and close tags of the same name are counted
 * from the container's own tag until the depth returns to zero, which is what
 * an actual parser would do and what "inside" actually means.
 */
function sharedGroupSpans(markup) {
  const spans = [];
  const open = new RegExp(`<([a-z]+)\\b[^>]*class="[^"]*\\b(?:${SHARED_GROUP.join('|')})\\b[^"]*"[^>]*>`, 'gs');
  for (const m of [...markup.matchAll(open)]) {
    const el = m[1];
    if (m[0].endsWith('/>')) continue;
    const step = new RegExp(`<${el}\\b|</${el}>`, 'gs');
    step.lastIndex = m.index + m[0].length;
    let depth = 1;
    let hit;
    while (depth > 0 && (hit = step.exec(markup))) {
      depth += hit[0][1] === '/' ? -1 : 1;
    }
    spans.push([m.index, depth === 0 && hit ? hit.index : markup.length]);
  }
  return spans;
}

describe('B2 · the scanner can still see what it scans for', () => {
  // Both of this repository's other source scanners quietly narrowed and passed
  // everything (ipc.test.js, twice). So prove each half of this one before
  // trusting a green run out of it.
  it('finds buttons, reads their classes, and ignores the ones in comments', () => {
    const src = '<div><button class="r-btn ghost">Go</button>\n<!-- a <button> in prose --></div>';
    const got = buttons(src);
    expect(got).toHaveLength(1);
    expect(got[0].classes).toEqual(['r-btn', 'ghost']);
    // ...and a `<button>` named in a script's own prose is not markup either.
    expect(buttons('<script>/** a <button> in a docstring */</script><div></div>')).toHaveLength(0);
  });

  it('and knows which buttons are INSIDE a shared group, not merely in the same file', () => {
    // The amnesty this replaces: the first version asked whether the FILE
    // mentioned `.r-seg`, so one tab strip excused every anonymous button in the
    // component. Both halves are asserted — a member is in, a sibling after the
    // container closes is out — because a span function that always returned
    // `true` and one that always returned `false` would each pass half of this.
    const src =
      '<div class="r-seg"><div><button>in</button></div></div><button>out</button>';
    const got = buttons(src);
    expect(got.map((b) => b.inGroup)).toEqual([true, false]);
    // ...and it really does see the whole of a real file.
    expect(buttons(read('src/lib/views/templates/TemplateEditor.svelte')).length).toBeGreaterThan(30);
    expect(styleOf(read('src/lib/views/library/VerseDeck.svelte')).length).toBeGreaterThan(1000);
  });
});

describe('B2 · a button in a row of buttons uses the shared control', () => {
  it('and having used it, does not redraw it', () => {
    // THE DEFECT, four times over: `class="r-btn ghost sm tg-del"` with a local
    // rule repainting the fill, the edge and the colour. A variant that is
    // overridden the moment it is used is not a variant. What a local class on
    // a shared control may legitimately carry is POSITION — width, flex, margin
    // — and a state the variant has no opinion about, such as rule 41's armed
    // second press.
    //
    // The box properties, checked against the rule bodies of every class that
    // appears on an `.r-btn`/`.r-iconbtn` in these files.
    const BOX = ['height', 'border-radius', 'background', 'border', 'font-family', 'font-size'];
    // ONE named exception, with its reason, in the shape B1's own allowance has
    // (`Dock.svelte` relaxing `.r-cbtn`). `Go live` is the single control in the
    // Library that puts something in front of a congregation, it sits alone in a
    // pane foot, and it is deliberately given the CONTROL BUTTON's 34px rather
    // than the ordinary 26px — the same height `Dock.svelte` gives Clear screens
    // and Blackout, for the same reason. It is exempt because somebody argued
    // for it here, which is the whole difference between an exception and drift.
    const ALLOWED = new Map([['src/lib/views/library/LiveOutputRail.svelte', { 'lo-golive': ['height'] }]]);
    const offenders = [];
    for (const f of FILES) {
      const src = read(f);
      const style = styleOf(src).replace(/\/\*[\s\S]*?\*\//g, '');
      // Classes that ride alongside a shared instrument in this file.
      const riders = new Set();
      for (const b of buttons(src)) {
        if (!b.classes.some((c) => SHARED.includes(c))) continue;
        for (const c of b.classes) if (!SHARED.includes(c)) riders.add(c);
      }
      for (const m of style.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
        const sel = m[1].trim();
        // Only a rule about the control ITSELF, in its resting state. A rule
        // for a state (`.arm`, `.on`, `:disabled`, `:hover`) is allowed to
        // paint — that is what a state is for — and a DESCENDANT rule
        // (`.tg-del svg`) is about something inside the button.
        const bare = sel.match(/^\.([a-zA-Z0-9_-]+)$/);
        if (!bare || !riders.has(bare[1])) continue;
        const ok = ALLOWED.get(f)?.[bare[1]] ?? [];
        for (const p of BOX) {
          if (ok.includes(p)) continue;
          if (new RegExp(`(^|[;{\\s])${p}\\s*:`).test(m[2])) {
            offenders.push(`${f}: ${sel} redraws ${p}`);
          }
        }
      }
    }
    expect(offenders, 'delete the local box — the shared control owns it').toEqual([]);
  });

  it('and it asks for a variant that exists', () => {
    // THE DEFECT: three buttons in Arrangements wore `class="r-btn ghost xs"`.
    // `.xs` is declared in NO stylesheet in this repository — app.css publishes
    // `sm` and nothing else — so they rendered at the full 26px beside a Save
    // and a Cancel at 22px, while the source read as a deliberate choice. A
    // variant name that does nothing is worse than no name: it stops the next
    // person looking.
    //
    // The vocabulary is READ OUT of app.css rather than restated here, so this
    // cannot drift from the thing it is about.
    const css = read('src/app.css').replace(/\/\*[\s\S]*?\*\//g, '');
    const known = new Set();
    for (const m of css.matchAll(/\.r-btn((?:\.[a-zA-Z0-9_-]+)+)[\s,{:]/g)) {
      for (const v of m[1].split('.').filter(Boolean)) known.add(v);
    }
    expect(known.has('sm'), 'no .r-btn variants found in app.css').toBe(true);
    expect(known.has('xs'), 'app.css grew an xs — update the comment above').toBe(false);

    const offenders = [];
    for (const f of FILES) {
      for (const b of buttons(read(f))) {
        if (!b.classes.includes('r-btn')) continue;
        for (const c of b.classes) {
          // A variant is a bare lowercase word; a component's own class is
          // prefixed (`tg-del`, `lo-golive`) or is a shared utility.
          if (c === 'r-btn' || c.includes('-') || known.has(c)) continue;
          offenders.push(`${f}: .r-btn.${c} is declared nowhere`);
        }
      }
    }
    expect(offenders, 'use a variant app.css actually publishes').toEqual([]);
  });

  it('and Delete wears the one destructive variant, on every surface', () => {
    // THE DEFECT: four separate copies of `class="r-btn ghost sm X-del"` with a
    // local rule painting the text rose — Templates' Delete, Themes' Delete,
    // History's Erase service, and the template editor's own Delete, which was
    // the ONE that used `.r-btn sm danger`. So the same word drew a ghost's
    // `--v-500` hairline on three surfaces and a red one on the fourth, and in
    // Templates the two were a single press apart.
    //
    // B1's own assertion could not see this: it holds the app.css rule, and all
    // four of these were correct app.css rules being overridden in a component.
    for (const [f, sel] of [
      ['src/lib/views/templates/TemplateGallery.svelte', 'tg-del'],
      ['src/lib/views/themes/ThemeGallery.svelte', 'th-del'],
      ['src/lib/views/library/History.svelte', 'lib-del'],
    ]) {
      const hit = buttons(read(f)).find((b) => b.classes.includes(sel));
      expect(hit, `${f}: no ${sel} button`).toBeTruthy();
      expect(hit.classes, `${f}: ${sel} is not the danger variant`).toContain('danger');
      expect(hit.classes, `${f}: ${sel} is still a repainted ghost`).not.toContain('ghost');
    }
  });
});

describe('B2 · and everything that is NOT a button is named, and says what it is', () => {
  it('no button in these two workspaces is anonymous', () => {
    // THE DEFECT: fourteen buttons styled through a DESCENDANT selector —
    // `.te-addmenu button`, `.te-seg button`, `.tg-menu button`,
    // `.tg-newmenu button`, `.tg-viewtog button` — so the element carried no
    // class at all. To a shape census (and to the next person grepping for a
    // shape) those are indistinguishable from a button somebody forgot to
    // class, which is exactly what a census is trying to find.
    //
    // A button may be anonymous in ONE case: it is a member of a shared group
    // whose container styles it, which is app.css's `.r-seg` and the rail row.
    const offenders = [];
    for (const f of FILES) {
      for (const b of buttons(read(f))) {
        if (b.classes.length) continue;
        if (b.inGroup) continue; // a `.r-seg` / rail member — styled by its container
        offenders.push(`${f}: ${b.tag}`);
      }
    }
    expect(offenders, 'give it a class, or put it in a shared group').toEqual([]);
  });

  it('and every kept shape says, in words, what it is instead', () => {
    // THE DEFECT this replaces is not a rendering bug; it is the reason the
    // rendering bugs kept coming back. A hand-rolled shape and a deliberate one
    // look identical in a stylesheet, so each pass re-litigated the same
    // twenty controls and each pass reached a different answer.
    //
    // So: a class that shapes a button and is NOT the shared control must carry
    // a comment immediately above its rule. The scanner asks only that the
    // comment exists and names the shape — "not a button" is the phrase the
    // next census will grep for.
    const SHAPE = /(^|[;{\s])(height|border-radius|background|border|font-family|font-size)\s*:/;
    const offenders = [];
    for (const f of FILES) {
      const src = read(f);
      const style = styleOf(src);
      // Classes that appear on a button and never alongside a shared control.
      const own = new Set();
      for (const b of buttons(src)) {
        if (b.classes.some((c) => SHARED.includes(c) || SHARED_GROUP.includes(c))) continue;
        for (const c of b.classes) if (c !== 'r-focus') own.add(c);
      }
      // Walk the rules WITH their preceding text, so a comment above one is
      // visible. Matching `([^{}]*)\{` captures everything since the last brace,
      // comments included, which is exactly the span a reader would see.
      for (const m of style.matchAll(/\}([^{}]*)\{([^{}]*)\}/g)) {
        const lead = m[1];
        const sel = lead.replace(/\/\*[\s\S]*?\*\//g, '').trim();
        const bare = sel.match(/^\.([a-zA-Z0-9_-]+)$/);
        if (!bare || !own.has(bare[1])) continue;
        if (!SHAPE.test(m[2].replace(/\/\*[\s\S]*?\*\//g, ''))) continue;
        if (/\/\*/.test(lead)) continue; // it explains itself
        offenders.push(`${f}: .${bare[1]} draws a shape and does not say what it is`);
      }
    }
    expect(
      offenders,
      'name the shape in a comment — a chip, a tab, a list row, a grid cell, a card',
    ).toEqual([]);
  });

  it('the deck of slides is the case the rule exists for', () => {
    // VerseDeck was 17 of the census's 181 and NONE of them converted, which is
    // the honest answer and the one a forced conversion would have destroyed:
    // a slide card, a favourite star over artwork, a kebab, an invisible scrim
    // and nine menu rows. Pinned by name so a later pass cannot quietly decide
    // they were buttons after all without editing this list.
    const src = read('src/lib/views/library/VerseDeck.svelte');
    for (const c of ['vd-shot', 'vd-star', 'vd-kebab', 'vd-scrim', 'vd-mi', 'vd-ic']) {
      expect(buttons(src).some((b) => b.classes.includes(c)), `no .${c} button`).toBe(true);
      expect(styleOf(src), `.${c} does not say what it is`).toMatch(
        new RegExp(`not a button[\\s\\S]{0,700}\\.${c}\\s*\\{`),
      );
    }
    // ...and not one of them wears the shared control.
    for (const b of buttons(src)) {
      expect(b.classes, `VerseDeck: ${b.tag}`).not.toContain('r-btn');
    }
  });
});
