// THE SHARED CONTROLS — what each one guarantees, and the defect each guarantee
// was written against.
//
// `ui.test.js` beside this file holds the three state components. This file holds
// the five controls added to consolidate the console's button, menu, field and row
// vocabulary, and it is deliberately about BEHAVIOUR and CONTRACT rather than
// about appearance: jsdom computes no layout, so nothing here can tell you what a
// control measures on a screen. Where a claim is about a painted box it is made
// against `src/app.css` as text, and it says so.
//
// Two assertions check the STYLESHEET rather than a component. That is on purpose
// and it is the shape `buttonshapes.test.js` records: `.r-btn.xs` was used at three
// call sites and declared in no stylesheet anywhere, so three buttons rendered at
// the full height while the source read as a deliberate choice. A variant name that
// does nothing is worse than no name, because it stops the next person looking.
import { describe, it, expect, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { tick } from 'svelte';
import Button from './Button.svelte';
import IconButton from './IconButton.svelte';
import Menu from './Menu.svelte';
import MenuItem from './MenuItem.svelte';
import Field from './Field.svelte';
import Toolbar from './Toolbar.svelte';
import ListState from './ListState.svelte';

const CSS = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8').replace(
  /\/\*[\s\S]*?\*\//g,
  '',
);

let host;
function mount(Component, props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  return new Component({ target: host, props });
}
afterEach(() => {
  host?.remove();
  host = null;
});

describe('the kit asks for variants the stylesheet actually publishes', () => {
  it('and the reader of the stylesheet is reading the stylesheet', () => {
    // Guards the guard. A comment stripper that ate the code, or a path that
    // resolved to an empty file, would make every assertion below pass vacuously —
    // which is how two scanners in this repository were wrong while looking
    // exhaustive (`ipc.test.js`, twice, recorded in its own header).
    expect(CSS.length).toBeGreaterThan(10000);
    expect(CSS).toMatch(/\.r-btn\{[^}]*height:26px/);
    expect(CSS, 'comments are not being stripped').not.toContain('THE CONTROL METRICS');
  });

  it('every variant `Button` offers has a rule, and `.r-iconbtn.sm` now exists', () => {
    for (const v of ['primary', 'ghost', 'quiet', 'danger', 'amber']) {
      expect(CSS, `.r-btn.${v} is declared nowhere`).toMatch(
        new RegExp(`\\.r-btn\\.${v}[\\s,{:]`),
      );
    }
    // THE DEFECT THIS PASS EXISTS FOR. `.r-btn.sm` has been published for waves;
    // `.r-iconbtn` had ONE size, so every icon button in the product is 26px
    // including the ones sitting in rows of 22px small buttons. Earlier waves
    // correctly stripped the local overrides, which made the 4px step permanent:
    // it could not be closed at a call site without putting the override back.
    expect(CSS).toMatch(/\.r-btn\.sm\{[^}]*height:22px/);
    expect(CSS, 'the small icon step is missing again').toMatch(
      /\.r-iconbtn\.sm\{[^}]*width:22px[^}]*height:22px/,
    );
  });

  it('`.r-well` has a body and keeps the focus outline it was already promised', () => {
    // It was named in app.css's `:focus-within` group and NOWHERE else: no rule
    // body, zero call sites. So the product published a focus treatment for a
    // control that did not exist, while four search boxes each invented their own
    // after switching the input's own outline off.
    expect(CSS, '.r-well has no rule body again').toMatch(/\.r-well\{[^}]*height:26px/);
    const focus = CSS.slice(CSS.indexOf('.r-well:focus-within'));
    expect(focus.slice(0, 200)).toMatch(/outline:2px solid var\(--v-sel\)/);
  });

  it('`.r-menu` and `.r-menuitem` are published, so a menu is not six shapes', () => {
    expect(CSS).toMatch(/\.r-menu\{[^}]*border-radius:var\(--v-r-md\)/);
    expect(CSS).toMatch(/\.r-menuitem\{[^}]*padding:7px 9px/);
    // The destructive row draws the SAME red as the destructive button, from the
    // same token. Two reds under one meaning is the defect `.r-btn.danger` was
    // already fixed for once.
    expect(CSS).toMatch(/\.r-menuitem\.danger\{[^}]*color:var\(--v-red\)/);
  });
});

describe('Button', () => {
  const btn = () => host.querySelector('button');

  it('draws the shared control and the variant it was asked for', () => {
    mount(Button, { variant: 'ghost', size: 'sm' });
    expect(btn().classList.contains('r-btn')).toBe(true);
    expect(btn().classList.contains('ghost')).toBe(true);
    expect(btn().classList.contains('sm')).toBe(true);
  });

  it('a disabled control says why, on BOTH channels', () => {
    // THE DEFECT: 16 of the 27 buttons gated on `!$capture.available`,
    // `$safeMode` or `$serviceLock` said nothing at all, one of them
    // `Clear screens`. Both channels, because neither reaches everybody: `title`
    // is invisible to a keyboard or screen-reader operator and
    // `aria-describedby` is invisible to a mouse.
    mount(Button, { disabled: true, disabledReason: 'The engine is not answering.' });
    expect(btn().getAttribute('title')).toBe('The engine is not answering.');
    const id = btn().getAttribute('aria-describedby');
    expect(id, 'no aria-describedby').toBeTruthy();
    // And it points at something that EXISTS and carries the words. An attribute
    // pointing at nothing is the same failure as a dead cross-reference: it looks
    // like evidence and resolves to silence.
    const described = host.querySelector(`#${id}`);
    expect(described, 'aria-describedby points at nothing').toBeTruthy();
    expect(described.textContent).toContain('The engine is not answering.');
  });

  it("and a caller's own title cannot outrank that reason", () => {
    // THE SPREAD-ORDER BUG, watched to fail with `{...$$restProps}` moved to the
    // end of the attribute list. Svelte applies a spread in source order, so with
    // it written last a caller's `title` silently replaced the one attribute this
    // component exists to guarantee — and it would have replaced it on exactly the
    // buttons most likely to carry a keyboard hint.
    mount(Button, { disabled: true, disabledReason: 'Because of the lock.', title: 'Save (Cmd+S)' });
    expect(btn().getAttribute('title')).toBe('Because of the lock.');
  });

  it('but a working control is not given a tooltip about a problem it does not have', () => {
    mount(Button, { disabled: false, disabledReason: 'The engine is not answering.' });
    expect(btn().getAttribute('title')).toBe(null);
    expect(btn().getAttribute('aria-describedby')).toBe(null);
    expect(host.querySelector('.sr-only')).toBe(null);
  });
});

describe('IconButton', () => {
  it('is named, because an unnamed icon button is a dead control', () => {
    mount(IconButton, { label: 'More actions' });
    const b = host.querySelector('button');
    expect(b.getAttribute('aria-label')).toBe('More actions');
    // The name doubles as the tooltip, so a volunteer who does not recognise a
    // glyph can hover it.
    expect(b.getAttribute('title')).toBe('More actions');
  });

  it('asks for the small step by name rather than by a local override', () => {
    mount(IconButton, { label: 'Undo', size: 'sm' });
    const b = host.querySelector('button');
    expect(b.classList.contains('r-iconbtn')).toBe(true);
    expect(b.classList.contains('sm')).toBe(true);
  });

  it('a keyboard hint is a better tooltip than the name, and still loses to a reason', () => {
    mount(IconButton, { label: 'Undo', title: 'Undo (Ctrl/⌘+Z)' });
    expect(host.querySelector('button').getAttribute('title')).toBe('Undo (Ctrl/⌘+Z)');
    host.remove();
    mount(IconButton, { label: 'Undo', title: 'Undo (Ctrl/⌘+Z)', disabled: true, disabledReason: 'Nothing to undo.' });
    expect(host.querySelector('button').getAttribute('title')).toBe('Nothing to undo.');
  });
});

describe('Menu — rule 44, in one place instead of four', () => {
  function press(el, key) {
    const e = new window.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
    el.dispatchEvent(e);
    return e;
  }

  it('declares itself, so `shortcuts.js` can see it and stand down', () => {
    mount(Menu, { label: 'New item' });
    const m = host.querySelector('.r-menu');
    expect(m.getAttribute('role')).toBe('menu');
    expect(m.getAttribute('aria-label')).toBe('New item');
  });

  // Propagation is observed at the SHELL rather than read off the event, because
  // that is where it matters and because the event's own `cancelBubble` is not a
  // reliable witness in jsdom. `shortcuts.js` listens on `window` in the bubble
  // phase, so a listener there is the actual thing being protected.
  function atShell(fn) {
    const seen = [];
    const h = (e) => seen.push(e.key);
    window.addEventListener('keydown', h);
    try {
      fn();
    } finally {
      window.removeEventListener('keydown', h);
    }
    return seen;
  }

  it('Escape closes THIS menu and goes no further', () => {
    // Both halves, because each without the other is a real bug that has shipped
    // in this repository: an Escape that clears the wall on a menu dismissal
    // (rule 16), and an Escape that is swallowed and does nothing at all (the four
    // template menus, whose comments claimed it was "handled globally").
    let closed = 0;
    let e;
    const reached = atShell(() => {
      mount(Menu, { close: () => (closed += 1) });
      e = press(host.querySelector('.r-menu'), 'Escape');
    });
    expect(closed).toBe(1);
    expect(e.defaultPrevented, 'the key was not consumed').toBe(true);
    expect(reached, 'the key reached the shell as well').toEqual([]);
  });

  it('and every other key is returned untouched, so Space still advances', () => {
    // The stated reason the four template menus bound nothing was that a handler
    // "would have to stopPropagation, which would swallow Space". It does not
    // follow, and the cost of believing it was four menus with no outcome at all.
    let closed = 0;
    const keys = [' ', 'ArrowRight', 'Enter', 'b'];
    const reached = atShell(() => {
      mount(Menu, { close: () => (closed += 1) });
      for (const key of keys) {
        const e = press(host.querySelector('.r-menu'), key);
        expect(e.defaultPrevented, `${key} was consumed`).toBe(false);
      }
    });
    expect(reached, 'a key the menu has no opinion about was swallowed').toEqual(keys);
    expect(closed).toBe(0);
  });
});

describe('MenuItem', () => {
  it('is a menuitem, not a button that happens to be in a menu', () => {
    // A menu whose children are plain buttons is a menu to the eye and a group of
    // buttons to a screen reader.
    mount(MenuItem, { variant: 'danger' });
    const b = host.querySelector('button');
    expect(b.getAttribute('role')).toBe('menuitem');
    expect(b.classList.contains('r-menuitem')).toBe(true);
    expect(b.classList.contains('danger')).toBe(true);
    // And it is NOT the shared button: `buttonshapes.test.js` already rules that a
    // menu row is not a button, and forcing one into `.r-btn` would centre its
    // label and draw a hairline round every row.
    expect(b.classList.contains('r-btn')).toBe(false);
  });
});

describe('Field', () => {
  it('renders the well, so the input inside it stops drawing its own box', () => {
    mount(Field, { class: 'tg-search' });
    const w = host.querySelector('.r-well');
    expect(w).toBeTruthy();
    expect(w.classList.contains('tg-search')).toBe(true);
  });

  it('a label is a real label, because a placeholder is not one', () => {
    // A placeholder disappears the moment somebody types, which is exactly when a
    // volunteer looks up to check what they are filling in.
    mount(Field, { label: 'Search', id: 'f1' });
    const l = host.querySelector('label');
    expect(l.textContent).toContain('Search');
    expect(l.getAttribute('for')).toBe('f1');
  });
});

describe('Toolbar', () => {
  it('centres its members, because `stretch` is what let one tall member deform a row', () => {
    mount(Toolbar, { size: 'sm', gap: 4 });
    const t = host.querySelector('div');
    // jsdom computes no layout, so this reads the declared value rather than a
    // measured one. Stated plainly: nothing here is a claim about a painted pixel.
    expect(t.style.getPropertyValue('--tb-h')).toBe('22px');
    expect(t.style.getPropertyValue('--tb-gap')).toBe('4px');
  });
});

describe('ListState — the precedence, fixed in one place', () => {
  it('an error outranks loading, and loading outranks empty', async () => {
    // THE DEFECT, in `views/library/Arrangements.svelte`: on a failed
    // `listArrangements`, `list` is still `[]`, so the panel rendered "No
    // arrangements yet" AND the error underneath it. An operator who built three
    // arrangements on Tuesday is told first that they have none.
    mount(ListState, { error: 'nope', loading: true, items: [], empty: 'None yet.' });
    await tick();
    expect(host.textContent).not.toContain('None yet.');
    expect(host.querySelector('[role="alert"]')).toBeTruthy();
  });

  it('and a list nobody has asked for yet is LOADING, never empty', async () => {
    // `items` defaults to null, not `[]`, because "no rows" and "no answer" are
    // the two facts this component exists to keep apart. Live told an operator
    // with a full plan library "No service plans yet" on every visit for exactly
    // this reason.
    mount(ListState, { items: null, empty: 'None yet.', what: 'arrangements' });
    await tick();
    expect(host.textContent).not.toContain('None yet.');
    expect(host.querySelector('[role="status"]')).toBeTruthy();
    expect(host.textContent).toContain('arrangements');
  });

  it('an answered, genuinely empty list says so', async () => {
    mount(ListState, { items: [], empty: 'None yet.' });
    await tick();
    expect(host.textContent).toContain('None yet.');
  });
});
