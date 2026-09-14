// W3 · THE PLANNER IS A DESK — the instrument for the rebrand pass on this
// workspace (docs/REBRAND.md §2, prototype "PLANNER").
//
// `plannerbuildonly.test.js` holds the thing this workspace must never do. This
// file holds the four things it must do, each of which was found by rendering the
// prototype and the app side by side at 1440px and reading the difference:
//
//   1. it OPENS ON A PLAN. The desk used to open as a page title, a paragraph and
//      three empty panes, the middle one saying "Pick a plan on the left to open
//      it" — a workspace whose first screen is an instruction for making it
//      useful.
//   2. the caveat that this workspace cannot reach an output is ALWAYS ON SCREEN.
//      It has now lived in three places: a toolbar note that `display:none`d
//      itself below 1240px, a page standfirst that cost two rows of the desk, and
//      now the running order's own footer. A caveat that can disappear is not a
//      caveat, so the test is about where it CANNOT be, not where it is.
//   3. a plan rail row never prints the word `undefined`. Driven against a bridge
//      whose plan summaries were missing `plan_date`/`cue_count`, the rail read
//      "No date · undefined cues" — the frontend leaking onto a screen an
//      operator reads.
//   4. a cue is dragged 1:1 and the order COMMITS ON RELEASE — nothing is
//      persisted while the row is still in hand.
//
// Written the way CLAUDE.md asks: every assertion below was watched to fail with
// the change it covers reverted. The reversion for each is named in its comment.

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { capture } = await import('../stores/capture.js');

// The same self-detecting gate as `plannerbuildonly.test.js`: without
// `resolve: { conditions: ['browser'] }` in `vitest.config.js`, `onMount` is a
// literal empty function and every one of these tests would pass over a Planner
// that never loaded a thing.
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const PLANS = [
  { id: 1, title: 'Sunday Morning', plan_date: '2026-09-14', cue_count: 3 },
  { id: 2, title: 'Evening Prayer', plan_date: '2026-09-14', cue_count: 1 },
];

const cue = (id, cue_type, label, section_title, duration_sec, payload = {}, template_id = null) => ({
  id,
  plan_id: 1,
  cue_type,
  label,
  payload_json: JSON.stringify(payload),
  template_id,
  section_title,
  duration_sec,
});

// Two of these repeat the section of the cue above them, because that is the
// shape the running order got wrong: every cue recording the section it is IN,
// rather than only the first cue of each section carrying the heading.
const CUES = [
  cue(11, 'announce', 'Welcome & notices', 'Gathering', 120, { body: 'Please keep the gate clear' }, 3),
  cue(12, 'song', 'Great Is Thy Faithfulness', 'Gathering', 300, {
    title: 'Great Is Thy Faithfulness',
    sections: [{ tag: 'V1', label: 'Verse 1', lyrics: 'Great is thy faithfulness' }],
    arrangement_name: 'Standard',
  }),
  cue(13, 'scripture', 'Romans 8:28', 'Word', 0, {
    reference: 'Romans 8:28',
    text: 'And we know that all things work together for good',
    verse: 28,
  }),
  cue(14, 'scripture', 'Psalms 119:105', 'Word', 0, {
    reference: 'Psalms 119:105',
    text: 'Thy word is a lamp unto my feet',
    verse: 105,
  }),
];

let host;
let app;
let planRows = PLANS;

function bridge(cmd) {
  switch (cmd) {
    case 'list_plans':
      return Promise.resolve(planRows);
    case 'plan_items':
      return Promise.resolve(CUES);
    case 'list_templates':
      return Promise.resolve([
        { id: 1, name: 'Classic Serif' },
        // The real builtin, verbatim from `templates.js`: transparent ground,
        // near-black ink, `lowerThird` — the template the empty preview was
        // rendered with.
        {
          id: 3,
          name: 'Lower Third',
          layout: { regions: ['verse_text', 'reference'], align: 'left', lowerThird: true, refFirst: false },
          style: {
            font: 'var(--f-body)',
            background: 'transparent',
            accent: '#8b5cf6',
            verseColor: '#1c1224',
            verseSize: '2.6',
            refSize: '1.7',
            italicRef: false,
          },
        },
      ]);
    case 'list_media':
    case 'list_announcements':
    case 'search_scripture':
    case 'search_songs':
    case 'list_arrangements':
      return Promise.resolve([]);
    default:
      return Promise.resolve(null);
  }
}

beforeEach(() => {
  planRows = PLANS;
  invoke.mockReset();
  invoke.mockImplementation(bridge);
  capture.update((c) => ({ ...c, available: true }));
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  document.body.innerHTML = '';
});

async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

async function until(predicate, what, tries = 50) {
  for (let i = 0; i < tries; i += 1) {
    if (predicate()) return;
    await settle();
  }
  throw new Error(`timed out waiting for: ${what}`);
}

async function mount() {
  const ServicePlanner = (await import('./ServicePlanner.svelte')).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new ServicePlanner({ target: host });
  return host;
}

const PLANNER = resolve(__dirname, 'ServicePlanner.svelte');
const src = readFileSync(PLANNER, 'utf8');

describe('§2 · the desk opens on a plan', () => {
  itMounted('draws the newest plan, its cues and its sections without a click', async () => {
    // Fails with the `open(plans[0])` in `onMount` removed: the running order is
    // the EmptyState and `.sp-row` never appears.
    await mount();
    await until(() => host.querySelector('.sp-row'), 'the running order to draw itself');

    expect(host.querySelectorAll('.sp-row').length).toBe(CUES.length);
    expect(host.textContent).toContain('Sunday Morning');
    expect(host.textContent).not.toContain('Pick a plan on the left to open it.');

    // …and the inspector is about something, rather than asking to be given
    // something: opening a plan selects its first cue.
    expect(host.textContent).not.toContain('Select a cue.');
    expect(host.querySelector('.sp-row.sel')).toBeTruthy();
  });

  itMounted('still opens whichever plan the operator picks instead', async () => {
    // Opening a plan for the operator is only acceptable if picking a different
    // one still works, so that is what is asserted. The `!openPlan` guard beside
    // the auto-open is NOT pinned and deliberately so: `refresh()` resolves and
    // the `.then` runs before the rail has rendered a single row, so there is no
    // instant at which a click could interleave, and a test written for it
    // passed with the guard removed. A test that cannot fail is a theory that
    // was never tested (CLAUDE.md rule 40); the guard stays as cheap defence
    // against a future `refresh()` that awaits more, and it stays unpinned.
    await mount();
    await until(() => host.querySelectorAll('.sp-railcard').length === 2, 'both plans in the rail');
    host.querySelectorAll('.sp-railcard')[1].click();
    await settle();
    await settle();
    const head = host.querySelector('.sp-plantitle');
    expect(head.textContent.trim()).toBe('Evening Prayer');
  });

  itMounted('draws ONE heading per section, not one per cue', async () => {
    // The defect, rendered at 1440×960: four cues in two sections drew four
    // headings — `GATHERING / Welcome & notices`, `GATHERING / Great Is Thy
    // Faithfulness`, `WORD / Romans 8:28`, `WORD / Psalms 119:105`. A heading
    // that repeats on every row is not a heading, it is a column.
    //
    // The count is the assertion. Fails against `sectionsOf`'s old rule (any cue
    // carrying a title begins a section), which is the state this fixture's
    // consecutive same-section cues reproduce.
    await mount();
    await until(() => host.querySelector('.sp-row'), 'the running order');

    const caps = [...host.querySelectorAll('.sp-seccap')].map((e) => e.textContent.trim());
    expect(host.querySelectorAll('.sp-row').length).toBe(4);
    expect(caps).toEqual(['Gathering', 'Word']);
    expect(caps.length).toBeLessThan(host.querySelectorAll('.sp-row').length);
    // The prototype's heading is a caption and a hairline, one of each.
    expect(host.querySelectorAll('.sp-sec .sp-secln').length).toBe(2);
  });

  itMounted('prints the kind of every cue as a word, never a truncation', async () => {
    await mount();
    await until(() => host.querySelector('.sp-row'), 'the running order');
    const chips = [...host.querySelectorAll('.sp-ck')].map((e) => e.textContent.trim());
    expect(chips).toEqual(['NOTE', 'SONG', 'WORD', 'WORD']);
  });
});

describe('§2 · the caveat cannot disappear', () => {
  it('is not a standfirst any more, and there is only one of it', () => {
    // Two copies of a safety sentence is how one of them gets shortened until it
    // says something else — which is exactly what happened to the toolbar note
    // ("never reaches …"). Fails if the standfirst is restored alongside it.
    expect(src).not.toMatch(/standfirst=/);
    expect([...src.matchAll(/nothing here\s*\n?\s*reaches an output/g)].length).toBe(1);
  });

  it('carries no media query that could hide it', () => {
    // The defect verbatim: `@media (max-width:1240px){ .sp-toolnote{ display:none } }`
    // — the sentence saying this workspace cannot reach a congregation went first
    // on the smallest screens.
    const style = src.slice(src.lastIndexOf('<style>'));
    const rules = [...style.matchAll(/@media[^{]*\{([\s\S]*?)\n  \}/g)].map((m) => m[1]);
    for (const r of rules) expect(r).not.toContain('sp-caveat');
    expect(style).not.toMatch(/\.sp-caveat[^{]*\{[^}]*display:\s*none/);
  });

  itMounted('is on screen with a plan open, in both modes, and with none open', async () => {
    const says = () => host.querySelector('.sp-caveat')?.textContent ?? '';
    planRows = [];
    await mount();
    await until(() => !host.querySelector('.r-empty, .es') === false, 'the empty desk');
    expect(says()).toContain('nothing here');

    planRows = PLANS;
    app.$destroy();
    host.remove();
    await mount();
    await until(() => host.querySelector('.sp-row'), 'the running order');
    expect(says()).toContain('Drag');
    expect(says()).toContain('nothing here');

    // …and in the add-cue panel, where "drag to reorder" is not true but the
    // caveat still is.
    const add = [...host.querySelectorAll('button')].find((b) => /Add cue/i.test(b.textContent));
    add.click();
    await settle();
    expect(says()).not.toContain('Drag');
    expect(says()).toContain('nothing here');
  });

  itMounted('sits outside the scroller, so a long plan cannot push it off', async () => {
    await mount();
    await until(() => host.querySelector('.sp-caveat'), 'the caveat');
    expect(host.querySelector('.sp-tablewrap .sp-caveat')).toBeNull();
    expect(host.querySelector('.sp-addpanel .sp-caveat')).toBeNull();
  });
});

describe('§2 · a plan rail row never leaks a frontend word', () => {
  itMounted('says what it does not know, rather than printing undefined', async () => {
    // Driven against a summary whose shape and the frontend's have come apart —
    // which is the one build in which that string can reach a screen. Fails
    // against `{p.plan_date || 'No date'}` + `{p.cue_count} cues`, which is what
    // rendered "No date · undefined cues".
    planRows = [{ id: 9, title: 'A plan from somewhere else' }];
    await mount();
    await until(() => host.querySelector('.sp-railcard'), 'the rail');

    const row = host.querySelector('.sp-railcard').textContent;
    expect(row).not.toContain('undefined');
    expect(row).toContain('No date');
    expect(row).toContain('Cue count unknown');
  });

  itMounted('prints a real summary exactly as the backend sent it', async () => {
    await mount();
    await until(() => host.querySelector('.sp-railcard'), 'the rail');
    const row = host.querySelector('.sp-railcard').textContent;
    expect(row).toContain('2026-09-14');
    expect(row).toContain('3 cues');
  });
});

describe('§2 · drag reorders on release, and only on release', () => {
  /** A pointer event jsdom will actually construct. */
  function pointer(el, type, clientY) {
    el.dispatchEvent(
      new MouseEvent(type, { bubbles: true, cancelable: true, clientY, button: 0 }),
    );
  }

  itMounted('commits the new order once, on pointerup', async () => {
    await mount();
    await until(() => host.querySelector('.sp-row'), 'the running order');

    const grips = host.querySelectorAll('.sp-grip');
    expect(grips.length).toBe(CUES.length);

    invoke.mockClear();
    pointer(grips[0], 'pointerdown', 0);
    // Two moves, a whole row each: nothing may be persisted while the cue is
    // still in the operator's hand. Fails against a handler that reorders on
    // move rather than on release.
    pointer(window, 'pointermove', 20);
    pointer(window, 'pointermove', 40);
    await settle();
    expect(invoke.mock.calls.map(([c]) => c)).not.toContain('reorder_plan');

    pointer(window, 'pointerup', 40);
    await settle();

    const reorders = invoke.mock.calls.filter(([c]) => c === 'reorder_plan');
    expect(reorders.length).toBe(1);
    // Row 1 moved down one place: the ids that were [11, 12, 13] are now
    // [12, 11, 13]. Fails against an off-by-one in `dropIndex`.
    expect(reorders[0][1].ids).toEqual([12, 11, 13, 14]);
  });

  itMounted('persists nothing when a drag goes nowhere', async () => {
    await mount();
    await until(() => host.querySelector('.sp-row'), 'the running order');
    const grip = host.querySelector('.sp-grip');

    invoke.mockClear();
    pointer(grip, 'pointerdown', 0);
    pointer(window, 'pointermove', 3);
    pointer(window, 'pointerup', 3);
    await settle();
    expect(invoke.mock.calls.map(([c]) => c)).not.toContain('reorder_plan');
  });

  itMounted('leaves no row stuck to the cursor when a drag is cancelled', async () => {
    // A touch drag interrupted by the OS never sends `pointerup`. Fails if
    // `pointercancel` is not wired: the row keeps its inline transform and the
    // next render paints the list shifted by a row.
    await mount();
    await until(() => host.querySelector('.sp-row'), 'the running order');
    const grip = host.querySelector('.sp-grip');

    pointer(grip, 'pointerdown', 0);
    pointer(window, 'pointermove', 40);
    expect(host.querySelector('.sp-row').style.transform).toContain('translateY');
    pointer(window, 'pointercancel', 40);
    await settle();
    for (const r of host.querySelectorAll('.sp-row')) expect(r.style.transform).toBe('');
  });

  it('drags with pointer events, not with HTML5 drag-and-drop', () => {
    // `dragstart`/`drop` never fire for a pen or a finger, and the browser draws
    // its own ghost instead of moving the row. Fails if either is reintroduced.
    expect(src).not.toMatch(/on:dragstart|on:dragover|on:drop\b|draggable=/);
    expect(src).toMatch(/on:pointerdown=\{\(e\) => onGripDown\(/);
  });

  itMounted('lets a keyboard reorder a plan at all', async () => {
    // The grip was an `aria-hidden` span with a `cursor:grab`, so the running
    // order could only be reordered with a mouse. Fails against that span.
    await mount();
    await until(() => host.querySelector('.sp-row'), 'the running order');
    const grip = host.querySelectorAll('.sp-grip')[0];
    expect(grip.tagName).toBe('BUTTON');
    expect(grip.getAttribute('aria-label')).toMatch(/reorder/i);

    invoke.mockClear();
    grip.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    await settle();
    const moves = invoke.mock.calls.filter(([c]) => c === 'move_plan_item');
    expect(moves.length).toBe(1);
    expect(moves[0][1]).toMatchObject({ id: 11, direction: 1 });
  });
});

describe('§2 · the cue inspector answers the question it is asked', () => {
  itMounted('shows the rendered preview BEFORE the fields, not below them', async () => {
    // "What will this put on the wall" is the only question this panel is asked
    // on a Tuesday, and the preview was the fifth thing in it — below the fold on
    // a short window. Fails against the old order (type badge, name, subtitle,
    // Section, Template, Duration, then the preview).
    await mount();
    await until(() => host.querySelector('.sp-preview'), 'the inspector preview');

    const body = host.querySelector('.sp-inspbody');
    const order = [...body.querySelectorAll('.sp-preview, .sp-flbl, .sp-kv, .sp-actions')];
    const at = (sel) => order.findIndex((e) => e.matches(sel));
    expect(at('.sp-preview')).toBe(0);
    expect(at('.sp-preview')).toBeLessThan(at('.sp-kv'));
    expect(at('.sp-kv')).toBeLessThan(at('.sp-actions'));

    const labels = [...body.querySelectorAll('.sp-flbl')].map((e) => e.textContent.trim());
    expect(labels).toEqual(['Label', 'Section', 'Duration', 'Actions']);
  });

  itMounted('puts a notice\'s words through the renderer, not an empty frame', async () => {
    // The behavioural half. Rendered at 1440\u00d7960 the notice preview was an empty
    // dark box; the probe that settled it showed the words WERE reaching
    // `TemplateRender` and being painted `#1c1224` on `transparent` \u2014 near-black
    // ink on a near-black ground. So this asserts the half a stylesheet cannot
    // lie about: the cue's body is in the DOM, inside the preview, through the
    // one renderer, with the real `Lower Third` builtin resolved onto it. Fails
    // if `previewContent` stops carrying an announce body (`slidesOf` \u2192
    // `p.body || p.text`), and fails if the inspector stops resolving the cue's
    // own template.
    await mount();
    await until(() => host.querySelector('.sp-preview'), 'the inspector preview');

    // The component's own `loadTemplates()` in `onMount` races the mock bridge's
    // dynamic `import('@tauri-apps/api/core')` under vitest and can lose \u2014 a
    // harness artefact, not the app: the same call awaited here resolves the full
    // list every time, and a real webview imports from one bundled chunk. Awaiting
    // it makes the store deterministic without changing what the component reads.
    const { loadTemplates } = await import('../stores/capture.js');
    await loadTemplates();
    await until(
      () => host.querySelector('.sp-preview').textContent.includes('gate'),
      'the notice body to reach the renderer',
    );

    expect(host.querySelector('.sp-preview').textContent).toContain('Please keep the gate clear');
    expect(host.querySelector('.sp-preview .sp-nopreview')).toBeNull();
    // …and it is the CUE'S template doing the rendering, which is the one whose
    // ink made the frame look empty.
    expect(host.querySelector('.sp-kv').textContent).toContain('Lower Third');

    // …and the operator is told the chequer is not something they chose. A plate
    // nobody explains is a background an operator thinks is part of the design.
    //
    // Asserted on the RENDERED caption, not on the source: the first version of
    // this grepped the file, and the file explains the plate in a CSS comment, so
    // it passed with the caption deleted. A scanner that can match a comment is
    // not checking the screen (CLAUDE.md — one entitlement test did this).
    const caption = host.querySelector('.sp-preview + .sp-fhelp').textContent;
    expect(caption).toMatch(/chequer is not part of the design/);
  });

  it('gives the preview a plate, so a KEYED template is not painted on the void', () => {
    // The other half, and it has to be a source assertion: jsdom computes no
    // `color-mix` and no gradient, so nothing mounted can see this. The plate is
    // unconditional on purpose — an opaque template covers it — because a
    // heuristic for "is this template keyed?" has a false negative, and the false
    // negative IS the defect. Fails against `background:var(--v-void)`.
    const preview = src.slice(src.indexOf('.sp-preview{'));
    const rule = preview.slice(0, preview.indexOf('}') + 1);
    expect(rule, 'the preview paints a flat ground again').not.toMatch(
      /background:\s*var\(--v-void\)/,
    );
    expect(rule, 'no chequer behind the render').toMatch(/background-image:[\s\S]*linear-gradient/);

  });

  itMounted('states kind, template and fires as name/value rows', async () => {
    await mount();
    await until(() => host.querySelector('.sp-kv'), 'the inspector facts');
    const keys = [...host.querySelectorAll('.sp-kv .rw-nvk')].map((e) => e.textContent.trim());
    expect(keys).toEqual(['Kind', 'Template', 'Fires']);
    // The first cue is an announcement: it may not claim the auto-detect that
    // only scripture has (`typeOf`, the one door).
    expect(host.querySelector('.sp-kv').textContent).toContain('NOTICE');
    expect(host.querySelector('.sp-kv').textContent).not.toContain('AUTO-DETECT');
  });

  itMounted('offers Move up, Move down and Delete', async () => {
    // "Move down" did not exist. With the running order's per-row buttons gone,
    // its absence meant a cue could be walked up a plan and never back down it
    // without a mouse. Fails against the two-button row.
    await mount();
    await until(() => host.querySelector('.sp-actions'), 'the inspector actions');
    const names = [...host.querySelectorAll('.sp-actions button')].map((b) => b.textContent.trim());
    expect(names).toEqual(['Duplicate', 'Move up', 'Move down', 'Delete']);

    // The first cue is at the top, so Move up is off and Move down is live.
    const [, up, down] = host.querySelectorAll('.sp-actions button');
    expect(up.disabled).toBe(true);
    expect(down.disabled).toBe(false);

    invoke.mockClear();
    down.click();
    await settle();
    const moves = invoke.mock.calls.filter(([c]) => c === 'move_plan_item');
    expect(moves.length).toBe(1);
    expect(moves[0][1]).toMatchObject({ id: 11, direction: 1 });
  });

  itMounted('shows a cue label it cannot save as a value, not as a dead input', async () => {
    // There is no `set_plan_label` on the bridge. A box an operator can type into
    // that silently discards what they typed is a control reporting a success it
    // did not achieve (rule 15's family). Fails against an `<input>` bound to the
    // label with no command behind it.
    await mount();
    await until(() => host.querySelector('.sp-fval'), 'the label row');
    expect(host.querySelector('.sp-fval').textContent.trim()).toBe('Welcome & notices');
    const labelled = [...host.querySelectorAll('.sp-inspbody input, .sp-inspbody textarea')].map(
      (e) => e.value,
    );
    expect(labelled).not.toContain('Welcome & notices');
  });
});

describe('rule 39 · arrangement staleness is untouched', () => {
  itMounted('still names a stale arrangement as needing checking, on the cue', async () => {
    // The rebrand may not quietly drop the one thing that stands between a
    // reordered song and the wrong words in a plan (CLAUDE.md rule 39,
    // DECISIONS §55).
    await mount();
    await until(() => host.querySelectorAll('.sp-row').length === CUES.length, 'the running order');
    host.querySelectorAll('.sp-row')[1].click(); // the song
    await settle();
    const slides = [...host.querySelectorAll('.sp-insptabs button')].find((b) =>
      /Slides/i.test(b.textContent),
    );
    slides.click();
    await settle();
    expect(host.querySelector('.sp-slidemeta').textContent).toContain('ARRANGEMENT: STANDARD');
  });

  it('still refuses a stale arrangement entry to a plan', () => {
    // The picker is the door; this asserts the door is still shut, in the file.
    expect(src).toMatch(/disabled=\{a\.stale\}/);
    expect(src).toContain('sections changed since this was built');
  });
});
