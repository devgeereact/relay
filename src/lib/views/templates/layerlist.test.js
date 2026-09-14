// THE LAYER LIST — the one surface that drives paint order.
//
// `layerops.test.js` holds the arithmetic. This holds what the arithmetic cannot:
// that the list an operator reads is the stack the wall paints, that a band's
// words are shown as being INSIDE their band rather than beside it, and that the
// two arrows on a row move the thing the row names.
//
// The defect this was written against: the list was the raw array, reversed, and
// the arrows swapped two adjacent entries of it. A band's words live in that
// array and are not drawn from it, so on `lowerBible` — a band and its two
// words, which is the whole template — pressing either arrow, on any row, did
// nothing at all. Every lower third the product ships has that shape.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

// The shape `layers.js::lowerBible` builds, plus one shape on top of it — a band,
// the two words it names, and something in the stack to move past them.
const TEMPLATE = {
  id: 11,
  name: 'Lower Third',
  layout: {
    layers: [
      { id: 'b1', type: 'band', name: 'Band', members: ['w1', 'w2'], x: 6, y: 74, w: 88, h: 26, top: 74, side: 6, pad: 3, lift: 3 },
      { id: 'w1', type: 'text', name: 'Verse', bind: 'verse', x: 9, y: 76, w: 82, h: 10, size: 2.6 },
      { id: 'w2', type: 'text', name: 'Reference', bind: 'reference', x: 9, y: 86, w: 82, h: 5, size: 1.5 },
      { id: 's1', type: 'shape', name: 'Plate', x: 0, y: 0, w: 100, h: 8 },
    ],
  },
  style: {},
};

const { default: TemplateEditor } = await import('./TemplateEditor.svelte');
const { templates } = await import('../../stores/capture.js');

const settle = () => new Promise((r) => setTimeout(r, 0));

let host;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  return new TemplateEditor({ target: host, props: { templateId: 11 } });
}

const rows = () => [...host.querySelectorAll('.te-layer')];
const names = () => rows().map((r) => r.querySelector('.te-lname').textContent.trim());
const rowFor = (name) => rows().find((r) => r.querySelector('.te-lname').textContent.trim() === name);
// BY WHAT IT SAYS IT DOES, not by where it sits. This was
// `querySelectorAll('.te-lmini')[0 | 1]`, and the row has five of those: two
// state toggles and three actions. Adding the visibility eye to the front of the
// row made "the up arrow" mean "hide this layer", and the suite reported a
// reorder bug that did not exist. An index into a row of icon buttons is a test
// asserting a layout; the aria-label is the contract.
const arrow = (name, which) => {
  const want = which === 'up' ? /^(Bring|Move) .* (forward|earlier in the band)$/ : /^(Send|Move) .* (back|later in the band)$/;
  const hit = [...rowFor(name).querySelectorAll('.te-lmini')].find((b) => want.test(b.getAttribute('aria-label') || ''));
  if (!hit) throw new Error(`no ${which} arrow on the row for ${name}`);
  return hit;
};

describe('the layer list', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return [TEMPLATE];
      if (cmd === 'get_setting') return '';
      return null;
    });
    templates.set([structuredClone(TEMPLATE)]);
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  it('is the stack front-first, with a band\'s words beneath the band that owns them', async () => {
    mount();
    await settle();
    // Front first: the shape is drawn last, so it is at the top. The band's two
    // words follow the band, in the order the band names them — reading order,
    // which is not reversed, because a band lays its words out rather than
    // stacking them.
    expect(names()).toEqual(['Plate', 'Band', 'Verse', 'Reference']);
  });

  it('marks a word as being inside its band rather than beside it', async () => {
    mount();
    await settle();
    expect(rowFor('Verse').classList.contains('inband')).toBe(true);
    expect(rowFor('Band').classList.contains('inband')).toBe(false);
    expect(rowFor('Plate').classList.contains('inband')).toBe(false);
  });

  it('moves a stack object PAST the whole band in one press', async () => {
    mount();
    await settle();
    arrow('Plate', 'down').click();
    await settle();
    // One press, one visible step: the plate is now behind the band. It used to
    // swap with `Reference` — a layer nothing draws from the stack — so the
    // wall was identical and the next two presses looked broken too.
    expect(names()).toEqual(['Band', 'Verse', 'Reference', 'Plate']);
  });

  it('moves a word within its band, and never out of it', async () => {
    mount();
    await settle();
    arrow('Reference', 'up').click();
    await settle();
    expect(names()).toEqual(['Plate', 'Band', 'Reference', 'Verse']);

    // Already first in the band: the arrow has nowhere to go, and it does not
    // get there by leaving the band.
    arrow('Reference', 'up').click();
    await settle();
    expect(names()).toEqual(['Plate', 'Band', 'Reference', 'Verse']);
  });

  it('names each kind with its own icon, from the one register', async () => {
    mount();
    await settle();
    const icon = (n) => rowFor(n).querySelector('.te-ltype').textContent.trim();
    // A band drew `T`, the text icon, in the one place an operator goes to tell
    // one kind from another.
    expect(icon('Band')).toBe('▬');
    expect(icon('Plate')).toBe('▢');
    expect(icon('Verse')).toBe('T');
  });

  it('lets an operator name an object, which the list then shows', async () => {
    mount();
    await settle();
    rowFor('Verse').click();
    await settle();
    const box = host.querySelector('#te-oname');
    expect(box, 'the selected object has a name field').toBeTruthy();
    box.value = 'Speaker';
    box.dispatchEvent(new Event('input', { bubbles: true }));
    await settle();
    expect(names()).toContain('Speaker');
  });
});

// ── THE DRAG ───────────────────────────────────────────────────────────────
//
// The move/up pair is listened for on `window`, which hears nothing that happens
// outside the window. Release the button over the desktop and `pointerup` is
// delivered somewhere else: the drag is still armed, and the object follows the
// pointer the next time it crosses the canvas with no button held. Capturing the
// pointer is what makes the gesture end where the operator ended it.
describe('direct manipulation on the canvas', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return [TEMPLATE];
      if (cmd === 'get_setting') return '';
      return null;
    });
    templates.set([structuredClone(TEMPLATE)]);
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  const down = (el) => {
    const e = new MouseEvent('pointerdown', { bubbles: true, cancelable: true, clientX: 100, clientY: 100 });
    Object.defineProperty(e, 'pointerId', { value: 7 });
    el.dispatchEvent(e);
    return e;
  };

  const plateBox = () =>
    [...host.querySelectorAll('.te-hbox')].find((b) => b.getAttribute('aria-label') === 'Plate');

  it('captures the pointer, so a release outside the window still ends the drag', async () => {
    mount();
    await settle();
    // The shape's handle box. A band and its words are PLACED, not dragged —
    // dragging would write x/y/w/h and nothing draws them from x/y/w/h — so they
    // are deliberately not draggable and would prove nothing here.
    const box = plateBox();
    expect(box, 'the stack object has a handle box on the canvas').toBeTruthy();
    const captured = [];
    box.setPointerCapture = (id) => captured.push(id);
    down(box);
    expect(captured, 'the gesture is captured for the pointer that started it').toEqual([7]);
  });

  it('reads back a share of the frame, never a pixel', async () => {
    mount();
    await settle();
    const box = plateBox();
    box.setPointerCapture = () => {};
    down(box);
    await settle();
    // The board has no layout under jsdom, so a real move cannot be measured
    // here. What CAN be measured is the unit the editor stores and shows: the
    // layer's own x/y/w/h, which the renderer sizes in cqw.
    expect(host.querySelector('.te-botchip').textContent.trim()).toBe('0,0 · 100×8');
  });
});

// ── THE RAIL, THE READABILITY PANEL AND THE ALIGNMENT STRIP ────────────────
// Wave 3, agent S1. The operator's words were "the template editor is too
// rough… canva style with layer… should look alike the propresenter 7 type",
// and the lead measured four things on a rendered console at 1600x1000:
//
//   1. layer names truncated at THREE layers (`Refere…`, `Backgr…`) — five 20px
//      buttons beside the name in a 222px column left it about 72px;
//   2. a seven-paragraph readability essay permanently below the layer list —
//      the biggest thing in the column and the least used;
//   3. no thumbnail and no drag affordance on a row;
//   4. the inspector opened on TEMPLATE facts, so selecting an object and then
//      editing it took a second decision.
//
// Two claims in the brief were NOT true of the code and are recorded here so
// nobody re-fixes them: the visibility toggle AND a real lock both already
// existed (`toggleLock`, and `startDrag`/`onCanvasKey` both refuse a locked
// layer). What was missing was the thumbnail, the grip, the drag, and the room
// for the name.
describe('S1 · the layer rail', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return [TEMPLATE];
      if (cmd === 'get_setting') return '';
      return null;
    });
    templates.set([structuredClone(TEMPLATE)]);
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  const mini = (name, label) =>
    [...rowFor(name).querySelectorAll('.te-lmini')].find((b) => new RegExp(label, 'i').test(b.getAttribute('aria-label') || ''));

  it('gives every row a grip, a thumbnail, an eye and a lock', async () => {
    mount();
    await settle();
    for (const n of ['Plate', 'Band', 'Verse', 'Reference']) {
      const row = rowFor(n);
      expect(row.querySelector('.te-lgrip'), `${n}: no drag grip`).toBeTruthy();
      expect(row.querySelector('.te-lthumb .te-lthumbbox'), `${n}: no thumbnail`).toBeTruthy();
      expect(mini(n, 'hide|show'), `${n}: no visibility toggle`).toBeTruthy();
      expect(mini(n, 'lock'), `${n}: no lock`).toBeTruthy();
    }
  });

  it("the thumbnail reads the SAME geometry the canvas handle does", async () => {
    // A band's words are placed BY THE BAND (`drawBoxes`), not by the x/y/w/h
    // they store. A thumbnail drawn from the stored box would put a lower
    // third's reference somewhere the reference is not — the WYSIWYG guarantee
    // failing one layer above the renderer, which is the defect the canvas
    // overlay already had and was fixed for.
    mount();
    await settle();
    for (const n of ['Verse', 'Reference', 'Band']) {
      const thumb = rowFor(n).querySelector('.te-lthumbbox').getAttribute('style');
      const canvas = [...host.querySelectorAll('.te-hbox')]
        .find((b) => b.getAttribute('aria-label') === n)
        .getAttribute('style');
      for (const k of ['left', 'top', 'width', 'height']) {
        const of = (s) => (s.match(new RegExp(`${k}:\\s*([^;]+)`)) || [])[1]?.trim();
        expect(of(thumb), `${n}: the rail and the canvas disagree about ${k}`).toBe(of(canvas));
      }
    }
    // ...and that geometry is NOT what the member stores, or the assertion
    // above would pass over two copies of the same wrong number.
    const stored = TEMPLATE.layout.layers.find((l) => l.id === 'w2');
    expect(rowFor('Reference').querySelector('.te-lthumbbox').getAttribute('style'))
      .not.toContain(`top:${stored.y}%`);
  });

  it('carries the full name, and says what kind of object it is', async () => {
    mount();
    await settle();
    // jsdom has no layout, so the WIDTH of the cell cannot be measured here —
    // that is the lead's browser pass. What is measurable is that the name is
    // no longer competing with five buttons for one line: the name and the kind
    // are separate cells, and the row's actions live on the second line.
    expect(names()).toEqual(['Plate', 'Band', 'Verse', 'Reference']);
    expect(rowFor('Band').querySelector('.te-lkindtxt').textContent.trim()).toBe('Band (lower third)');
    expect(rowFor('Verse').querySelector('.te-lkindtxt').textContent.trim()).toBe('in band');
  });

  it('a LOCKED row cannot be dragged, on this surface as well as on the canvas', async () => {
    mount();
    await settle();
    expect(rowFor('Plate').getAttribute('draggable')).toBe('true');
    mini('Plate', 'lock').click();
    await settle();
    expect(rowFor('Plate').getAttribute('draggable'), 'a locked row still offers the drag').toBe('false');
    // And the gesture itself is refused, not merely un-offered: a `dragstart`
    // fired at it anyway arms nothing, so the following drop moves nothing.
    drag('Plate', 'Reference');
    await settle();
    expect(names()).toEqual(['Plate', 'Band', 'Verse', 'Reference']);
  });
});

// The drag is driven through the real handlers rather than through a real
// pointer: jsdom implements neither HTML5 drag-and-drop nor a `DataTransfer`.
// The handlers guard every `dataTransfer` touch for exactly that reason, so what
// runs here is the code that runs in the webview minus the browser's own
// plumbing.
function fire(el, type) {
  const e = new Event(type, { bubbles: true, cancelable: true });
  el.dispatchEvent(e);
  return e;
}
function drag(from, to) {
  fire(rowFor(from), 'dragstart');
  fire(rowFor(to), 'dragover');
  fire(rowFor(to), 'drop');
}

describe('S1 · drag to reorder, driven through the arrows that already work', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return [TEMPLATE];
      if (cmd === 'get_setting') return '';
      return null;
    });
    templates.set([structuredClone(TEMPLATE)]);
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  it('drops a stack object past the whole band, in one gesture', async () => {
    mount();
    await settle();
    drag('Plate', 'Band');
    await settle();
    // ONE drop, ONE visible step. A splice of the raw array would have put the
    // plate between two of the band's words, where nothing draws it from, and
    // the wall would have been identical — the exact bug `moveLayer` exists to
    // prevent, one surface along.
    expect(names()).toEqual(['Band', 'Verse', 'Reference', 'Plate']);
  });

  it("and a drop on a band's WORD means the band, not a dead zone", async () => {
    // A band's words are indented under it. Refusing a drop there would leave
    // most of the rail inert on every lower third the product ships — a band
    // and its two words IS the whole template on `lowerBible`.
    mount();
    await settle();
    drag('Plate', 'Reference');
    await settle();
    expect(names()).toEqual(['Band', 'Verse', 'Reference', 'Plate']);
  });

  it('drops a word within its band, and refuses a drop that would take it out', async () => {
    mount();
    await settle();
    drag('Reference', 'Verse');
    await settle();
    expect(names()).toEqual(['Plate', 'Band', 'Reference', 'Verse']);

    // A word and a shape are in two different orders. Dropping one on the other
    // is not a move that exists — and half-doing it (shuffling the word inside
    // its band and stopping) is worse than refusing, because it looks like the
    // drop worked.
    drag('Reference', 'Plate');
    await settle();
    expect(names()).toEqual(['Plate', 'Band', 'Reference', 'Verse']);
  });

  it('a drop onto the row being dragged changes nothing', async () => {
    mount();
    await settle();
    drag('Plate', 'Plate');
    await settle();
    expect(names()).toEqual(['Plate', 'Band', 'Verse', 'Reference']);
  });
});

describe('S1 · the readability panel folds, and its one visible line is a real status line', () => {
  const withStyle = (style) => ({ ...structuredClone(TEMPLATE), style });

  const mountStyled = (style) => {
    const t = withStyle(style);
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return [t];
      if (cmd === 'get_setting') return '';
      return null;
    });
    templates.set([t]);
    return mount();
  };

  afterEach(() => {
    host?.remove();
    host = null;
  });

  it('starts folded — the essay is one row until it is asked for', async () => {
    mountStyled({});
    await settle();
    const toggle = host.querySelector('.te-legtoggle');
    expect(toggle, 'no readability heading').toBeTruthy();
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(host.querySelector('.te-leg'), 'the verdict rows are still rendered while folded').toBeFalsy();
    expect(host.querySelector('#te-scr'), 'the room fields are still rendered while folded').toBeFalsy();
  });

  it('and opens to exactly what was there before, caveat included', async () => {
    mountStyled({});
    await settle();
    host.querySelector('.te-legtoggle').click();
    await settle();
    expect(host.querySelector('.te-legtoggle').getAttribute('aria-expanded')).toBe('true');
    expect([...host.querySelectorAll('.te-legrow')].length).toBe(3);
    expect(host.querySelector('#te-scr')).toBeTruthy();
    expect(host.querySelector('#te-back')).toBeTruthy();
    expect(host.textContent).toMatch(/Neither has been checked against a projector/i);
  });

  it("does not say the same thing over a template with a problem as over one without", async () => {
    // RULE 35, on a line that is now the only thing an operator sees until they
    // open the panel. A folded panel headed "Readability" and nothing else would
    // read identically whether the verse was legible or invisible, which is
    // precisely the defect the rule names.
    mountStyled({ background: '#000000', verseColor: '#0a0a0a', refColor: '#0b0b0b' });
    await settle();
    const bad = host.querySelector('.te-legsum');
    expect(bad.textContent.trim()).toMatch(/to look at/);
    expect(bad.classList.contains('bad')).toBe(true);

    host.remove();
    mountStyled({ background: '#000000', verseColor: '#ffffff', refColor: '#ffffff' });
    await settle();
    const ok = host.querySelector('.te-legsum');
    expect(ok.textContent.trim()).not.toMatch(/to look at/);
    expect(ok.classList.contains('bad')).toBe(false);
  });

  it('folding it also puts the distance strip away', async () => {
    // The only switch for that strip lives inside the panel. Left on over a
    // folded panel it would paint four extra renders across the canvas with
    // nothing on screen able to turn them off.
    mountStyled({});
    await settle();
    host.querySelector('.te-legtoggle').click();
    await settle();
    [...host.querySelectorAll('.te-legbtn')][0].click();
    await settle();
    expect(host.querySelector('.te-dists'), 'the distance strip did not open').toBeTruthy();
    host.querySelector('.te-legtoggle').click();
    await settle();
    expect(host.querySelector('.te-dists')).toBeFalsy();
  });
});

describe('S1 · the inspector opens on the selected object, not on the template', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return [TEMPLATE];
      if (cmd === 'get_setting') return '';
      return null;
    });
    templates.set([structuredClone(TEMPLATE)]);
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  const sections = () => [...host.querySelectorAll('.te-designbody .te-sec')].map((h) => h.textContent.trim());

  it('puts the object\'s own groups first and the template\'s facts last', async () => {
    mount();
    await settle();
    rowFor('Plate').click();
    await settle();
    const s = sections();
    expect(s[0], 'the panel still opens on the template').not.toBe('Template');
    expect(s[s.length - 1], 'the template section is not at the foot').toBe('Template');
    expect(s).toContain('Position');
  });

  it('and the template controls are all still there, in one section', async () => {
    mount();
    await settle();
    expect(host.querySelector('#te-name'), 'the template name field went missing').toBeTruthy();
    // Two registers of five chips: the global binding, then the per-template
    // filter. Neither is inert (agent S2 checked both readers), and the second
    // no longer calls a template a screen.
    const labels = [...host.querySelectorAll('.te-showlbl')].map((l) => l.textContent.trim());
    expect(labels).toEqual(['Used for', 'Content this template renders']);
    expect(host.textContent).not.toMatch(/Shows on this screen/);
  });
});

describe('S1 · the alignment strip writes percentages, and only where they mean something', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return [TEMPLATE];
      if (cmd === 'get_setting') return '';
      return null;
    });
    templates.set([structuredClone(TEMPLATE)]);
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  const alignBtn = (label) =>
    [...host.querySelectorAll('.te-alignbar button')].find((b) => b.getAttribute('aria-label') === label);
  const chip = () => host.querySelector('.te-botchip').textContent.trim();

  it('aligns the selected object without letting it leave the frame', async () => {
    mount();
    await settle();
    rowFor('Plate').click();
    await settle();
    expect(chip()).toBe('0,0 · 100×8');
    alignBtn('Align bottom').click();
    await settle();
    // 100 - h, not 100. The Position grid would otherwise read a plausible 100
    // over a shape that is entirely off the canvas.
    expect(chip()).toBe('0,92 · 100×8');
  });

  it('is disabled for the kinds x/y/w/h do not place', async () => {
    mount();
    await settle();
    rowFor('Band').click();
    await settle();
    expect(alignBtn('Align left').disabled, 'a band is placed by top/side, not by x').toBe(true);
    rowFor('Verse').click();
    await settle();
    expect(alignBtn('Align left').disabled, 'a word is placed by its band').toBe(true);
    rowFor('Plate').click();
    await settle();
    expect(alignBtn('Align left').disabled).toBe(false);
  });

  it('and Space evenly refuses rather than pretending, below three movable objects', async () => {
    mount();
    await settle();
    // This template has ONE object the canvas can move. "Distribute" over one
    // object cannot mean anything, and the tooltip says which three it needs.
    const space = [...host.querySelectorAll('.te-alignbar button')]
      .find((b) => /Space across/.test(b.textContent));
    expect(space.disabled).toBe(true);
    expect(space.getAttribute('title')).toMatch(/three objects/i);
  });
});
