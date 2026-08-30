// R3 · SURFACE INVENTORY — the evidence file.
//
// Layer B (mount real components in jsdom with a recording `invoke` mock) plus a
// little of layer C. Nothing here claims a backend call succeeded: the backend is a
// mock and the only assertions are about what the DOM says and which command string
// was dispatched.
//
// Every `it()` in this file is a FINDING, written so it FAILS if the defect is
// fixed — or, where the defect is a missing thing, asserts the absence explicitly
// and says in a comment what the fix should make it assert instead. Read the
// describe() headers as the finding titles.
//
// Precedent: liveoutputrail.test.js (including its settle() helper — tick() alone
// is not enough, because capture.js reaches the backend through a dynamic import
// that resolves a turn later than Svelte's scheduler).

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';

// ─────────────────────────────────────────────────────────────────────────────
// R3-00 · onMount IS A NO-OP IN THIS TEST SUITE
//
// Svelte 4's package exports map the `svelte` entry to `src/runtime/ssr.js` for
// every resolution condition EXCEPT `browser`:
//
//     "." : { "browser": { "default": "./src/runtime/index.js" },
//             "default": "./src/runtime/ssr.js" }
//
// and `ssr.js` line 14 is, verbatim, `export function onMount() {}`.
//
// FIXED 2026-08-14 (P1-11). `vitest.config.js` now sets
// `resolve: { conditions: ['browser'] }`, so the lifecycle is real and the
// assertions below that need it run under a plain `npm test`.
//
// The gate is LEFT IN PLACE and is deliberately self-detecting: `LIFECYCLE_LIVE`
// reads the runtime rather than trusting the config. If that one line is ever
// removed, these tests SKIP rather than pass vacuously, and the harness test
// immediately below says so out loud. A suite that quietly stops checking is the
// thing this whole file was written about.
// ─────────────────────────────────────────────────────────────────────────────
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

describe('R3-00 · the harness itself', () => {
  it('reports whether onMount is alive, so nothing below passes vacuously', () => {
    // Not an assertion about the product — an assertion about the instrument.
    // Under the repo config this logs `false`, which IS the finding.
    expect(typeof svelteRuntime.onMount).toBe('function');
  });
});

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { installShortcuts, registerContext, cheatsheet } = await import('./shortcuts.js');
const { live, screenBlack, rehearsing, panicError, capture, templates, readErrors } =
  await import('./stores/capture.js');
const { setSafeMode } = await import('./boot/boot.js');

let host;
let app;

function mountInto(Component, props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Component({ target: host, props });
  return host;
}

async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

/** A keydown that behaves like a real one: dispatched at the focused element. */
function press(key, target = document.body) {
  const e = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  target.dispatchEvent(e);
  return e;
}

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  document.body.innerHTML = '';
});

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue([]);
  live.set(null);
  screenBlack.set(false);
  rehearsing.set(false);
  panicError.set(null);
  cheatsheet.set(false);
  registerContext({});
  setSafeMode(false);
  capture.update((s) => ({ ...s, available: true }));
  // `readErrors` is app state that outlives a test, and it was the one store in this
  // file that was never reset. That is not tidiness — it is a real isolation gap,
  // and it fired on CI (a machine roughly 13x slower than this one) while passing
  // here five runs in a row:
  //
  //     expected 'All Templates0 … Relay's engine is not running…'
  //       to match /No templates yet — create one to start/
  //
  // TemplateGallery rendered ErrorState because `readErrors.loadTemplates` was
  // already set when the test began — a failure belonging to an earlier test, which
  // on a fast machine settles inside its own test and on a slow one does not.
  //
  // **The exact producer has not been pinned down**, and the fix does not depend on
  // it: a store that survives `beforeEach` will eventually carry something from the
  // test before, whichever call put it there. The two candidates, recorded so the
  // next person does not start from nothing — `loadTemplates` does
  // `templates.set(list)`, and a Svelte store propagates a THROWING SUBSCRIBER out
  // of `.set`, so a component still subscribed from a previous test can turn a
  // successful read into a recorded error; and `invoke.mockReset()` here makes the
  // mock return `undefined`, which a late-resolving read then works with.
  readErrors.set({});
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-01 · A POPUP MENU IS NOT A DIALOG, SO ESCAPE WIPED THE WALL BEHIND IT
//
// FIXED 2026-08-14 (P1-3). These tests now assert the repair; they are kept
// because the defect class — not the individual menu — is what has to stay dead.
//
// shortcuts.js reads the DOM to decide whether Escape belongs to an overlay or to
// the panic key, deliberately, so nobody has to remember to register the next
// popup. It probed for `[role="dialog"]` ALONE, and six popup menus were then
// built with `role="menu"` or no role at all, none consuming Escape. So the
// operator opened the Countdown picker on the RUN RAIL, changed their mind,
// pressed Escape — and the congregation's screens went clear while the menu
// stayed open. CLAUDE.md rule 16, verbatim, on the doors nobody enumerated.
//
// The repair is two-layer, because a panic key deserves two: the guard now covers
// `dialog`/`alertdialog`/`menu`/`listbox` (so an overlay that forgets everything
// still cannot clear the wall), AND each menu consumes Escape itself (so the
// operator gets the outcome they actually asked for).
// ─────────────────────────────────────────────────────────────────────────────
describe('R3-01 · Escape belongs to the popup menu, not to the panic key', () => {
  let clearScreens, teardown;

  beforeEach(() => {
    clearScreens = vi.fn();
    teardown = installShortcuts({ clearScreens, blackScreen: vi.fn() });
  });
  afterEach(() => teardown?.());

  it('LiveOutputRail — the Countdown menu, on the run surface mid-service', async () => {
    const LiveOutputRail = (await import('./views/library/LiveOutputRail.svelte')).default;
    const el = mountInto(LiveOutputRail, { queue: [] });
    await settle();

    const countdown = [...el.querySelectorAll('.lo-tile')].find((b) =>
      b.textContent.includes('Countdown'),
    );
    countdown.click();
    await tick();

    // The menu is open, and it now declares itself so `shortcuts.js` can see it.
    const menu = el.querySelector('.lo-menu');
    expect(menu).toBeTruthy();
    expect(menu.getAttribute('role')).toBe('menu');

    press('Escape', menu.querySelector('.lo-mi'));
    await tick();

    // FIXED 2026-08-14 (P1-3). Both halves: the wall is untouched, and the menu
    // actually closed. Before, the operator got the one outcome they did not ask
    // for and none of the one they did — mid-service, on the run rail.
    expect(clearScreens).not.toHaveBeenCalled();
    expect(el.querySelector('.lo-menu')).toBe(null);
  });

  it('VerseDeck — the per-slide kebab menu, reachable from every Library tab', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const el = mountInto(VerseDeck, {
      items: [{ reference: 'John 3:16', text: 'For God so loved…', slideNo: 1 }],
      layout: 'grid',
    });
    await tick();

    el.querySelector('.vd-kebab').click();
    await tick();

    const menu = el.querySelector('.vd-menu');
    expect(menu).toBeTruthy();
    expect(menu.getAttribute('role')).toBe('menu');

    press('Escape', menu.querySelector('.vd-mi'));
    await tick();

    // FIXED 2026-08-14 (P1-3), same repair as the run rail above. Six menus had
    // this shape; the guard now covers the whole class rather than these two.
    expect(clearScreens).not.toHaveBeenCalled();
    expect(el.querySelector('.vd-menu')).toBe(null);
  });

  // The control: the Library dropdown DOES get this right, by stopping the event
  // before it reaches window. Included so the finding above cannot be read as
  // "Escape is supposed to clear from everywhere".
  it('CONTROL — the Library New menu stops the event, and the wall survives', async () => {
    const menu = document.createElement('div');
    menu.setAttribute('role', 'menu');
    menu.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') e.stopPropagation();
    });
    document.body.appendChild(menu);
    press('Escape', menu);
    expect(clearScreens).not.toHaveBeenCalled();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-02 · AN INLINE PANEL WEARING role="dialog" DISARMS THE PANIC KEY
//
// The mirror image of R3-01. `Announcements.svelte`'s editor is an ordinary
// in-flow panel — no scrim, no `position:fixed`, no `aria-modal`, no focus trap —
// and it carries `role="dialog"`. shortcuts.js's DOM probe cannot tell that from a
// real modal, so for as long as an operator has a notice open for editing, Escape
// does nothing at all: it neither closes the panel (there is no Esc handler) nor
// clears the screens.
//
// Escape is the ONE panic key that survives a focused text field, and this panel
// is nothing but text fields.
// ─────────────────────────────────────────────────────────────────────────────
describe('R3-02 · the Announcements editor no longer disarms Escape', () => {
  let clearScreens, teardown;

  beforeEach(() => {
    clearScreens = vi.fn();
    teardown = installShortcuts({ clearScreens, blackScreen: vi.fn() });
  });
  afterEach(() => teardown?.());

  itMounted('is a labelled panel, not a modal, and no longer claims to be one', async () => {
    const Announcements = (await import('./views/library/Announcements.svelte')).default;
    const el = mountInto(Announcements, {});
    for (let i = 0; i < 6; i++) await settle();
    // The operator's own path: Library → Announcements → New notice.
    [...el.querySelectorAll('button')].find((b) => /New notice/.test(b.textContent))?.click();
    await tick();

    const editor = el.querySelector('.an-editor');
    expect(editor).toBeTruthy();

    // FIXED 2026-08-14 (P1-4). It carries none of the three markers a real modal
    // in this app carries — no aria-modal, no scrim, no focus trap — so it must
    // not carry the role either. `shortcuts.js` reads that role to decide who owns
    // Escape, which makes a wrong one DISARM the panic key rather than merely
    // mislabel a box.
    expect(editor.getAttribute('role')).not.toBe('dialog');
    expect(editor.getAttribute('role')).toBe('group');
    expect(editor.getAttribute('aria-modal')).toBe(null);
    expect(el.querySelector('.an-scrim')).toBe(null);
  });

  itMounted('so Escape is the panic key again, even from inside the body field', async () => {
    const Announcements = (await import('./views/library/Announcements.svelte')).default;
    const el = mountInto(Announcements, {});
    for (let i = 0; i < 6; i++) await settle();
    [...el.querySelectorAll('button')].find((b) => /New notice/.test(b.textContent))?.click();
    await tick();

    const body = el.querySelector('.an-text'); // the operator is typing here
    expect(body).toBeTruthy();
    press('Escape', body);
    await tick();

    // This is the whole point of the fix. `Esc` is the ONLY panic key that
    // survives a focused text field — `B` is suppressed while typing, or
    // "Habakkuk" would black out the wall on the second keystroke — and this panel
    // is nothing but text fields. While the fake role was there, Escape did
    // nothing at all: it neither closed the editor nor cleared the screens.
    expect(clearScreens).toHaveBeenCalledOnce();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-03 · SPACE PUTS SCRIPTURE ON THE WALL FROM A VERSEDECK LIST ROW
//
// CLAUDE.md rule 11: "`Space` means *advance*, app-wide, and nothing else."
//
// VerseDeck's GRID card is a native `<button>`, so shortcuts.js's `preventDefault`
// on keydown suppresses its Space activation and only the transport moves —
// correct. The LIST row is a `role="button"` div with its own `on:keydown` that
// calls `onFire` and neither preventDefaults nor stopPropagates, so it runs FIRST.
// Same deck, same content, two layouts, one key, two meanings — and the extra
// meaning is "put this in front of the congregation".
//
// Scope note, verified rather than assumed: `App.svelte` mounts one view at a time
// (`<svelte:component this={current} />`) and only `Live.svelte` calls
// `registerContext`, so on the Library tab `ctx.next` is normally unset and the
// transport does not also move. The second `expect` below therefore documents what
// happens IF a surface hosting a deck ever registers `next` — the shape to watch —
// while the FIRST one is the finding as it ships: Space fires content.
//
// Six views render VerseDeck: Scripture, Browse, LyricsPane, MediaLibrary,
// Announcements (and Live reaches them through Library). Every one inherits this.
// ─────────────────────────────────────────────────────────────────────────────
describe('R3-03 · Space means advance — except on a VerseDeck list row', () => {
  let next, teardown;

  beforeEach(() => {
    next = vi.fn();
    teardown = installShortcuts({ clearScreens: vi.fn(), blackScreen: vi.fn() });
    registerContext({ next }); // as Live.svelte does while a service is running
  });
  afterEach(() => {
    registerContext({});
    teardown?.();
  });

  it('LIST layout — Space advances and does NOT fire, exactly like the grid', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const onFire = vi.fn();
    const el = mountInto(VerseDeck, {
      items: [{ reference: 'John 3:16', text: 'For God so loved…', slideNo: 1 }],
      layout: 'list',
      onFire,
    });
    await tick();

    press(' ', el.querySelector('.vd-row'));

    // FIXED 2026-08-14 (P1-5). Space put scripture on the wall from here — and,
    // because the row neither preventDefaulted nor stopPropagated, ALSO stepped the
    // transport: two live actions from one press. The repair is not merely to stop
    // the double-action; it is that Space must mean the same thing in both layouts
    // of the same deck. It now falls through to the transport, as on the grid card.
    expect(onFire).not.toHaveBeenCalled();
    expect(next).toHaveBeenCalledOnce();
  });

  it('LIST layout — ENTER is the key that acts on the focused row', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const onFire = vi.fn();
    const el = mountInto(VerseDeck, {
      items: [{ reference: 'John 3:16', text: 'For God so loved…', slideNo: 1 }],
      layout: 'list',
      onFire,
    });
    await tick();

    press('Enter', el.querySelector('.vd-row'));

    // …and it does not ALSO advance the service.
    expect(onFire).toHaveBeenCalledOnce();
    expect(next).not.toHaveBeenCalled();
  });

  it('GRID layout — the same key on the same content only advances', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const onFire = vi.fn();
    const el = mountInto(VerseDeck, {
      items: [{ reference: 'John 3:16', text: 'For God so loved…', slideNo: 1 }],
      layout: 'grid',
      onFire,
    });
    await tick();

    press(' ', el.querySelector('.vd-card') ?? el.querySelector('button'));

    expect(onFire).not.toHaveBeenCalled();
    expect(next).toHaveBeenCalledOnce();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-04 · EMPTY IS NOT LOADING, AND IT IS NOT AN ERROR — ON SURFACES THAT
//         STILL CONFLATE ALL THREE
//
// `Loading.svelte` exists precisely because Live told an operator with a full plan
// library "No service plans yet" for the frames before the DB answered. Two views
// were fixed. These were not — and one of them is asserting emptiness about a
// table a fresh install SHIPS FILLED (five built-in templates).
//
// The ERROR half is structural: every read wrapper in capture.js is GROUP 2 and
// returns `[]` on failure, so no list in the app CAN distinguish "the query failed"
// from "there is nothing here". `ErrorState.svelte` is imported by exactly one view.
// ─────────────────────────────────────────────────────────────────────────────
describe('R3-04 · lists that say Empty before they know', () => {
  itMounted('TemplateGallery claims "No templates yet" before list_templates answers', async () => {
    templates.set([]);
    let release;
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_templates') return new Promise((r) => (release = () => r([{ id: 1, name: 'Scripture' }])));
      return Promise.resolve([]);
    });

    const TemplateGallery = (await import('./views/templates/TemplateGallery.svelte')).default;
    const el = mountInto(TemplateGallery);
    await tick();

    // FINDING: a fresh install ships five templates and this is the first thing a
    // new operator reads on the Templates tab.
    expect(el.textContent).toMatch(/No templates yet — create one to start/);
    expect(el.textContent).not.toMatch(/Loading/);
    release?.();
  });

  itMounted('…but says the REASON, not that sentence, when list_templates FAILS', async () => {
    templates.set([]);
    invoke.mockImplementation((cmd) =>
      cmd === 'list_templates' ? Promise.reject('database is locked') : Promise.resolve([]),
    );

    const TemplateGallery = (await import('./views/templates/TemplateGallery.svelte')).default;
    const el = mountInto(TemplateGallery);
    await settle();
    await settle();

    // FIXED 2026-08-15 (R3-04). The operator used to be told to create a template
    // while their five sat there and the query had failed — `loadTemplates`
    // swallowed the reason to `[]` under GROUP 2's rationale. `readErrors` keeps it,
    // and `ErrorState` is assertive, so a screen reader hears it too.
    expect(el.textContent).not.toMatch(/No templates yet — create one to start/);
    expect(el.querySelector('[role="alert"]')).toBeTruthy();
    expect(el.textContent).toMatch(/database is locked|didn't work|try/i);
  });

  itMounted('History claims "No services yet" before list_services answers', async () => {
    invoke.mockImplementation(() => new Promise(() => {})); // never resolves
    const History = (await import('./views/library/History.svelte')).default;
    const el = mountInto(History);
    await tick();

    expect(el.textContent).toMatch(/No services yet/);
    expect(el.textContent).not.toMatch(/Loading/);
  });

  itMounted('…and the identical sentence when list_services FAILS', async () => {
    invoke.mockRejectedValue('disk I/O error');
    const History = (await import('./views/library/History.svelte')).default;
    const el = mountInto(History);
    await settle();
    await settle();

    expect(el.textContent).toMatch(/No services yet/);
    expect(el.querySelector('[role="alert"]')).toBe(null);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-05 · EVERY EmptyState IS PROSE. NONE OF THEM OFFERS THE ACTION.
//
// `EmptyState.svelte` styles a `:global(.r-btn)` in its slot — the component was
// built expecting an action. Not one of its ~15 call sites passes one. The empty
// state tells a volunteer to "import or paste one with the Import button" and then
// makes them find it.
// ─────────────────────────────────────────────────────────────────────────────
describe('R3-05 · empty states have no way out', () => {
  itMounted('the Templates empty state renders a sentence and no control', async () => {
    templates.set([]);
    invoke.mockResolvedValue([]);
    const TemplateGallery = (await import('./views/templates/TemplateGallery.svelte')).default;
    const el = mountInto(TemplateGallery);
    await settle();

    const empty = el.querySelector('.r-empty');
    expect(empty).toBeTruthy();
    expect(empty.textContent).toMatch(/create one to start/);
    // FINDING: no button. Fixing this makes the next line fail, which is the point.
    expect(empty.querySelector('button')).toBe(null);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-06 · RAW RUST ERROR STRINGS STILL REACH A VOLUNTEER — IN MONOSPACE
//
// errors.js is the ONE humaniser and it exists because Channels rendered five raw
// `Err(...)` strings in a monospace font. Three surfaces never got the memo, and
// two of them use `r-mono` — the same font, the same shape, the same bug.
// ─────────────────────────────────────────────────────────────────────────────
// FIXED 2026-08-14 (R6-5 / R3-06). All six sites now go through `errors.js`.
// Because `error.rs` sends a typed OBJECT, `String(e)` on two of them rendered the
// literal text "[object Object]" — in monospace — to a volunteer.
describe('R3-06 · every surface goes through the ONE humaniser', () => {
  it('ImportReview shows a sentence, not a SQLite constraint name', async () => {
    const RUST = 'UNIQUE constraint failed: songs.title (code 2067)';
    invoke.mockRejectedValue(RUST);

    const ImportReview = (await import('./views/library/ImportReview.svelte')).default;
    const el = mountInto(ImportReview, {
      songs: [{ title: 'Amazing Grace', sections: [{ tag: '1', label: 'V1', lyrics: 'Amazing grace' }] }],
    });
    await tick();

    [...el.querySelectorAll('button')].find((b) => /Save \d+ to Library/.test(b.textContent))?.click();
    await settle();
    await settle();

    const msg = el.querySelector('.ir-msg');
    expect(msg).toBeTruthy();
    // FINDING: verbatim, and `r-mono` is the monospace class.
    // A sentence with a lead-in, not the constraint name — and crucially not
    // "[object Object]", which is what `String(e)` gives for a typed error.
    expect(msg.textContent).not.toBe(RUST);
    expect(msg.textContent).not.toContain('[object Object]');
    expect(msg.textContent).toMatch(/didn't work/i);
    // …and it is announced now, which is the other half of R3-08.
    expect(msg.getAttribute('role')).toBe('alert');
  });

  itMounted('ThemeEditor shows a sentence too', async () => {
    // Was `Err("…")` verbatim — a shape a volunteer has no way to read.
    const RUST = 'no such column: style_json (code 1)';
    invoke.mockImplementation((cmd) => {
      if (cmd === 'set_setting') return Promise.reject(RUST); // saveTheme persists via app_settings
      if (cmd === 'save_theme') return Promise.reject(RUST);
      if (cmd === 'get_setting')
        return Promise.resolve(JSON.stringify([{ id: 7, name: 'Mine', style: {} }]));
      return Promise.resolve([]);
    });

    const ThemeEditor = (await import('./views/themes/ThemeEditor.svelte')).default;
    const el = mountInto(ThemeEditor, { themeId: 7 });
    for (let i = 0; i < 6; i++) await settle();

    // Save is disabled until the draft is dirty, so edit the name first — which is
    // also the only way an operator ever reaches this button.
    const name = el.querySelector('.te-name');
    expect(name).toBeTruthy();
    name.value = 'Mine, edited';
    name.dispatchEvent(new Event('input', { bubbles: true }));
    await tick();

    const save = [...el.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Save');
    expect(save).toBeTruthy();
    save.click();
    for (let i = 0; i < 6; i++) await settle();

    const err = el.querySelector('.te-err');
    expect(err).toBeTruthy();
    expect(err.textContent.trim()).not.toBe(RUST);
    expect(err.textContent).not.toContain('[object Object]');
    expect(err.textContent).toMatch(/didn't work/i);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-07 · THE RUN RAIL'S ERROR LINE IS NOT ANNOUNCED
//
// LiveOutputRail is the most dangerous component in the app. Its `.lo-err` — the
// line that says a Take or a Countdown failed — has no `role="alert"` and no
// `aria-live`, so a screen-reader operator is told nothing. Six sibling views in
// `library/` share the same markup shape.
// ─────────────────────────────────────────────────────────────────────────────
describe('R3-07 · the run rail says a Take failed, out loud', () => {
  it('.lo-err carries no live-region role', async () => {
    const LiveOutputRail = (await import('./views/library/LiveOutputRail.svelte')).default;
    invoke.mockImplementation((cmd) =>
      cmd === 'start_countdown' ? Promise.reject('no output channel') : Promise.resolve([]),
    );
    const el = mountInto(LiveOutputRail, { queue: [] });
    await settle();

    [...el.querySelectorAll('.lo-tile')].find((b) => b.textContent.includes('Countdown'))?.click();
    await tick();
    el.querySelector('.lo-mi')?.click();
    await settle();
    await settle();

    const err = el.querySelector('.lo-err');
    expect(err).toBeTruthy();
    // FIXED 2026-08-14 (R3-08). This was the worst of the seven: the run rail is
    // the most dangerous surface in the app, and the line saying a Take or a
    // Countdown FAILED was silent to a screen-reader operator.
    expect(err.getAttribute('role')).toBe('alert');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-08 · AMETHYST MEANS REHEARSAL, AND VERSEDECK SPENDS IT ON "EDITED"
//
// Six lines apart in the same file: the tally badge is amethyst when rehearsing,
// and an edited slide is amethyst always. In a rehearsal the deck shows two
// identical amethyst pills meaning two unrelated things, one of which is the
// safety signal.
// ─────────────────────────────────────────────────────────────────────────────
describe('R3-08 · a second meaning for the rehearsal colour', () => {
  it('an EDITED slide wears the rehearsal badge colour while not rehearsing', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const el = mountInto(VerseDeck, {
      items: [{ reference: 'John 3:16', text: 'For God so loved…', slideNo: 1, edited: true }],
      layout: 'grid',
      rehearsing: false,
    });
    await tick();

    const tally = el.querySelector('.vd-tally');
    expect(tally.textContent.trim()).toBe('Edited');
    // FINDING: amethyst, with rehearsal off.
    expect(tally.className).toMatch(/amethyst/);
  });

  it('…the same colour the same component uses to say "nobody is looking"', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const el = mountInto(VerseDeck, {
      items: [{ reference: 'John 3:16', text: 'For God so loved…', slideNo: 1 }],
      layout: 'grid',
      rehearsing: true,
      liveRef: 'John 3:16',
    });
    await tick();

    const tally = el.querySelector('.vd-tally');
    expect(tally.textContent).toMatch(/Rehearsal/);
    expect(tally.className).toMatch(/amethyst/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-09 · A MODAL WITHOUT A FOCUS TRAP
//
// focus.js is the ONE implementation and it is one attribute to opt in.
// `TemplatePreviewOverlay` is a real full-screen modal (`position:fixed`,
// `aria-modal="true"`) and does not use it, so Tab walks straight out of the
// preview and into the app behind — and focus is not restored on close.
// ─────────────────────────────────────────────────────────────────────────────
describe('R3-09 · TemplatePreviewOverlay is aria-modal with no focus trap', () => {
  it('does not move focus into itself, and Tab is unguarded', async () => {
    const opener = document.createElement('button');
    document.body.appendChild(opener);
    opener.focus();

    const Overlay = (await import('./TemplatePreviewOverlay.svelte')).default;
    const el = mountInto(Overlay, { template: {}, onClose: () => {} });
    await tick();

    const dlg = el.querySelector('[role="dialog"]');
    expect(dlg.getAttribute('aria-modal')).toBe('true');
    // FINDING: focus never entered the modal. trapFocus would have moved it to the
    // first control (`.tpv-scrimbtn`).
    expect(document.activeElement).toBe(opener);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-10 · CONTROL GROUP — the laws that DO hold, pinned so a fix cannot break them
// ─────────────────────────────────────────────────────────────────────────────
describe('R3-10 · what is already right', () => {
  it('ErrorState refuses "Try again" for a non-transient typed error', async () => {
    const ErrorState = (await import('./ui/ErrorState.svelte')).default;
    const el = mountInto(ErrorState, {
      error: { kind: 'io', message: 'No space left on device' },
      onRetry: () => {},
    });
    await tick();
    expect(el.querySelector('button')).toBe(null);
  });

  it('…and offers it when the backend says the fault is transient', async () => {
    const ErrorState = (await import('./ui/ErrorState.svelte')).default;
    const el = mountInto(ErrorState, {
      error: { kind: 'busy', message: 'Relay is busy — try that again in a moment.' },
      onRetry: () => {},
    });
    await tick();
    expect(el.querySelector('button').textContent).toMatch(/Try again/);
    expect(el.querySelector('[role="alert"]')).toBeTruthy();
  });

  it('Loading announces itself; EmptyState deliberately does not', async () => {
    const Loading = (await import('./ui/Loading.svelte')).default;
    const el = mountInto(Loading, { what: 'plans' });
    await tick();
    expect(el.querySelector('[role="status"]').getAttribute('aria-live')).toBe('polite');
    expect(el.textContent).toMatch(/Loading plans/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-11 · SAFE MODE EXPLAINS ITSELF ON TWO OF VERSEDECK'S FOUR DOORS
//
// LiveOutputRail's Take relabels itself "Safe mode" when disabled — the repo's own
// stated rule ("a disabled button with no explanation reads as a bug"). VerseDeck
// has four fire controls. The GRID card carries the reason in its aria-label. The
// LIST row's `→`, and the kebab's "Take to screen", do not — and the list row's
// label still promises "Put John 3:16 on the screens", which it cannot do.
// ─────────────────────────────────────────────────────────────────────────────
describe('R3-11 · a disabled control that does not say why', () => {
  const ITEM = { reference: 'John 3:16', text: 'For God so loved…', slideNo: 1 };

  it('GRID card — disabled AND explained', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    setSafeMode(true);
    const el = mountInto(VerseDeck, { items: [ITEM], layout: 'grid' });
    await tick();
    const shot = el.querySelector('.vd-shot');
    expect(shot.disabled).toBe(true);
    expect(shot.getAttribute('aria-label')).toMatch(/Safe mode/);
    setSafeMode(false);
  });

  it('LIST row — disabled, unexplained, and the label now lies', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    setSafeMode(true);
    const el = mountInto(VerseDeck, { items: [ITEM], layout: 'list' });
    await tick();
    const go = [...el.querySelectorAll('.vd-ic')].find((b) =>
      /Put John 3:16 on the screens/.test(b.getAttribute('aria-label') ?? ''),
    );
    expect(go.disabled).toBe(true);
    // FINDING: no reason anywhere on the control.
    expect(go.getAttribute('aria-label')).not.toMatch(/Safe mode/);
    expect(go.getAttribute('title')).toBe(null);
    setSafeMode(false);
  });

  it('KEBAB "Take to screen" — disabled, unexplained', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    setSafeMode(true);
    const el = mountInto(VerseDeck, { items: [ITEM], layout: 'grid' });
    await tick();
    el.querySelector('.vd-kebab').click();
    await tick();
    const take = [...el.querySelectorAll('.vd-mi')].find((b) =>
      /Take to screen/.test(b.textContent),
    );
    expect(take.disabled).toBe(true);
    expect(take.textContent.trim()).toBe('Take to screen'); // not "Safe mode"
    expect(take.getAttribute('title')).toBe(null);
    setSafeMode(false);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-12 · LAYER C — STATIC CONTRACTS
//
// No mounting, no lifecycle, so these run identically under either config.
// ─────────────────────────────────────────────────────────────────────────────
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { resolve as rpath } from 'node:path';
const src = (p) => readFileSync(rpath(process.cwd(), p), 'utf8');

/** Every non-test frontend file, as [path, text]. Read once. */
const FRONTEND = (() => {
  const out = [];
  const walk = (d) => {
    for (const n of readdirSync(d)) {
      if (n.startsWith('.') || n === 'node_modules') continue;
      const f = `${d}/${n}`;
      if (statSync(f).isDirectory()) walk(f);
      else if (/\.(js|svelte)$/.test(n) && !n.endsWith('.test.js'))
        out.push([f, readFileSync(f, 'utf8')]);
    }
  };
  walk(rpath(process.cwd(), 'src'));
  return out;
})();

describe('R3-12 · seven registered commands have no UI path', () => {
  // CLAUDE.md: "No dead-but-built commands. Every registered #[tauri::command] has
  // a frontend caller." A store WRAPPER is not a caller in the sense that matters —
  // `saveArrangement` has had one and no UI for months.
  //
  // The chain that counts:
  //   #[tauri::command] → call('…') in capture.js → a wrapper some component imports
  //
  // CLOSED, and recorded rather than silently dropped from the list:
  //   `active_voice_profile` — reached from 2026-08-29 by Settings → Audio → Rooms,
  //   which reads the active profile in order to remember it with the room (RG-10).
  //   It was on this list for months; it is a caller now, and the test would fail
  //   if it were still here.
  const DEAD = {
    create_template: 'createTemplate',
    delete_arrangement: 'deleteArrangement',
    import_pro: 'importProFile',
    import_song: 'importSong',
    list_output_windows: 'listOutputWindows',
    open_output_window: 'openOutput',
    save_arrangement: 'saveArrangement',
  };

  const capture = src('src/lib/stores/capture.js');
  const mainRs = src('src-tauri/src/main.rs');
  const registered = new Set(
    mainRs
      .match(/generate_handler!\[([\s\S]*?)\]/)[1]
      .split(',')
      .map((s) => s.trim()),
  );

  for (const [cmd, wrapper] of Object.entries(DEAD)) {
    it(`${cmd} is registered, wrapped, and reached by nothing`, () => {
      expect(registered.has(cmd)).toBe(true);
      expect(capture).toMatch(new RegExp(`export async function ${wrapper}\\b`));
      // Nobody imports the wrapper. Checked against the actual import statements
      // rather than a bare name grep, so a comment mentioning it does not count.
      const importers = FRONTEND.filter(
        ([f, text]) =>
          !f.endsWith('capture.js') &&
          [...text.matchAll(/import\s*\{([^}]+)\}\s*from\s*['"][^'"]*capture\.js['"]/g)].some((m) =>
            m[1].split(',').some((n) => n.trim().split(/\s+as\s+/)[0] === wrapper),
          ),
      ).map(([f]) => f);
      expect(importers).toEqual([]);
    });
  }
});

describe('R3-12 · Live is the only major view that does not say the engine is missing', () => {
  // 18 controls on Live — Clear screens, Blackout, Fire, Rehearse, the mic — are
  // `disabled={!$capture.available}`. Channels, ServicePlanner and History each
  // render a rose "Backend not attached" badge in that state. Live renders nothing,
  // so the run surface simply appears broken.
  it('Channels, ServicePlanner and History explain it; Live does not', () => {
    for (const f of [
      'src/lib/views/Channels.svelte',
      'src/lib/views/ServicePlanner.svelte',
      'src/lib/views/library/History.svelte',
    ]) {
      expect(src(f)).toMatch(/Backend not attached/);
    }
    const live = src('src/lib/views/Live.svelte');
    expect(live).toMatch(/disabled=\{!\$capture\.available\}/); // it does disable
    expect(live).not.toMatch(/Backend not attached/); // FINDING: and says nothing
  });
});

describe('R3-12 · errors.js is the one humaniser, on every surface', () => {
  it('History humanises its export error and announces it', () => {
    const f = src('src/lib/views/library/History.svelte');
    expect(f).toMatch(/humanError\(e\)/);
    // `r-mono` is gone: monospace is what made a raw dump read like a crash.
    expect(f).not.toMatch(/class="lib-exportmsg r-mono"/);
    expect(f).toMatch(/role="status">\{exportMsg\}/);
  });

  it('History RENDERS the reason a service failed to open, instead of "empty"', () => {
    // It captured `error` and referenced it nowhere in the template, so a failed
    // query was reported as "No transcript recorded" / "No verses fired" — telling
    // an operator their Sunday was never captured, with the reason one property
    // away. That is the kind of wrong that gets acted on.
    const f = src('src/lib/views/library/History.svelte');
    expect(f).toMatch(/error: humanError\(e\)/);
    expect(f).toMatch(/detail\?\.error/);
    expect(f).toMatch(/Could not open this service/);
    // …and it is announced, because a screen reader got silence before.
    expect(f).toMatch(/role="alert"/);
  });

  it('ImportReview and ThemeEditor route through the humaniser, not String(e)', () => {
    for (const f of [
      'src/lib/views/library/ImportReview.svelte',
      'src/lib/views/themes/ThemeEditor.svelte',
      'src/lib/views/themes/ThemeGallery.svelte',
    ]) {
      const t = src(f);
      expect(t, `${f} still stringifies a typed error`).not.toMatch(/=\s*String\(e\)/);
      expect(t, `${f} does not import errors.js`).toMatch(/humanError/);
    }
  });
});

// FIXED 2026-08-14 (R3-08). `errors.js` gave every view the same sentence and
// nothing gave them the same live region — `ErrorState` has `role="alert"`, these
// hand-rolled twins had nothing. The worst was `lo-err` on the RUN RAIL: the line
// that says a Take or a Countdown FAILED was silent to a screen-reader operator,
// on the most dangerous surface in the app.
describe('R3-12 · every hand-rolled error line is announced', () => {
  const UNANNOUNCED = [
    ['src/lib/views/library/LiveOutputRail.svelte', 'lo-err'],
    ['src/lib/views/library/MediaLibrary.svelte', 'ml-err'],
    ['src/lib/views/library/LyricsPane.svelte', 'ly-err'],
    ['src/lib/views/library/Scripture.svelte', 'sv-err'],
    ['src/lib/views/library/Browse.svelte', 'br-err'],
    ['src/lib/views/Dashboard.svelte', 'd-err'],
    ['src/lib/views/library/ImportReview.svelte', 'ir-msg'],
  ];
  for (const [file, cls] of UNANNOUNCED) {
    it(`${cls} carries role="alert"`, () => {
      const line = src(file)
        .split('\n')
        .find((l) => l.includes(`class="${cls}`) || l.includes(`class="${cls} `));
      expect(line).toBeTruthy();
      expect(line, `${file}: ${cls} renders an error nobody hears`).toMatch(
        /role="alert"|aria-live/,
      );
    });
  }
});

describe('R3-12 · dialogs and the panic-key guard', () => {
  it('four boot gates are aria-modal with no focus trap', () => {
    for (const f of [
      'src/lib/boot/RecoverSession.svelte',
      'src/lib/boot/UpdateAvailable.svelte',
      'src/lib/boot/SafeModeStartup.svelte',
      'src/lib/boot/CrashReportRecovery.svelte',
    ]) {
      const t = src(f);
      expect(t).toMatch(/aria-modal="true"/);
      expect(t).not.toMatch(/use:trapFocus/); // FINDING
    }
  });

  it('TemplatePreviewOverlay is aria-modal with no focus trap', () => {
    const t = src('src/lib/TemplatePreviewOverlay.svelte');
    expect(t).toMatch(/aria-modal="true"/);
    expect(t).not.toMatch(/use:trapFocus/);
  });

  it('the Announcements editor no longer claims a role it cannot honour', () => {
    // FIXED 2026-08-14 (P1-4). Kept as a static assertion as well as a mounted one
    // because this is the cheap direction: `role="dialog"` on a panel with no
    // aria-modal, no focus trap and no Escape handler is always wrong, and grep
    // catches it the moment somebody adds it back.
    const t = src('src/lib/views/library/Announcements.svelte');
    expect(t).not.toMatch(/class="an-editor" role="dialog"/);
    expect(t).toMatch(/class="an-editor" role="group"/);
    // Still not a modal, and that is fine — it is not pretending to be one now.
    // Match the ATTRIBUTE, not the word: the file explains in prose why it carries
    // neither, and a bare-word grep failed on the explanation itself.
    expect(t).not.toMatch(/use:trapFocus/);
    expect(t).not.toMatch(/aria-modal=/);
  });
});

describe('R3-12 · six views have no heading at all', () => {
  // A screen-reader operator navigates by heading. The whole Library tab — five
  // sub-views and the deck — has none, and neither does Themes.
  for (const f of [
    'src/lib/views/Library.svelte',
    'src/lib/views/Themes.svelte',
    'src/lib/views/Templates.svelte',
    'src/lib/views/themes/ThemeGallery.svelte',
    'src/lib/views/themes/ThemeEditor.svelte',
    'src/lib/views/library/Scripture.svelte',
  ]) {
    it(`${f.split('/').pop()} renders no <h1>–<h6>`, () => {
      expect(src(f)).not.toMatch(/<h[1-6][\s>]/);
    });
  }
});

describe('R3-12 · the two genuinely unlabelled controls — CLOSED 2026-08-30', () => {
  // The inventory reported nine. Seven were the scanner's own blind spots —
  // `aria-label={expr}`, a wrapping `<label>`, a real for/id pair — and the scanner
  // was taught about all three (`inventory.test.js`). These two were real, and both
  // are fixed the NATIVE way rather than with an aria-label, so the visible text and
  // the accessible name are the same string and cannot drift apart.
  //
  // Kept as regression tests rather than deleted: the assertions are inverted, so
  // reintroducing either defect turns them red.
  it('the Planner stage-note textarea is named by a real <label for>', () => {
    const t = src('src/lib/views/ServicePlanner.svelte');
    expect(t).toMatch(/<label class="r-lbl sp-flbl" for="sp-stage-note">Stage note<\/label>/);
    expect(t).toMatch(/<textarea id="sp-stage-note"/);
    // …and NOT with an aria-label, which would be a second copy of the same words.
    expect(t).not.toMatch(/sp-note[^>]*aria-label/);
  });

  it('the ImportReview lyric textareas say WHICH slide they are', () => {
    // One of many identical boxes. "Slide text" announced eleven times over tells
    // somebody using a screen reader nothing about where they are.
    const t = src('src/lib/views/library/ImportReview.svelte');
    expect(t).toMatch(/aria-label="\{song\.title \|\| 'Song'\} — slide \{j \+ 1\} text"/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R3-04 · EMPTY ≠ ERROR — the structural half.
//
// Every read wrapper in capture.js is GROUP 2: it swallows and returns `[]`. The
// rationale written at the top of that file — "a list that fails to load costs the
// operator nothing they cannot see for themselves — the list is visibly empty" — is
// the sentence that produced the lie. A fresh install ships FIVE built-in
// templates, so "No templates yet" was never something the Templates tab could
// truthfully say; it could only ever mean the read had failed.
//
// The wrappers still return `[]` — that is what keeps every caller working and what
// stops a broken read taking a view down. What changed is that the REASON is no
// longer discarded: `readErrors` records it, keyed by wrapper name, the way
// `panicError` records a failed panic control and for the same reason.
// ─────────────────────────────────────────────────────────────────────────────
describe('R3-04 · a failed read is distinguishable from an empty one', () => {
  it('every list/load/search wrapper records WHY it returned nothing', () => {
    const store = src('src/lib/stores/capture.js');
    const heads = [...store.matchAll(/export async function (list|load|search)(\w+)\s*\(/g)];
    expect(heads.length).toBeGreaterThan(15); // the regex still works

    const undecided = [];
    heads.forEach((h, i) => {
      const next = heads[i + 1]?.index ?? store.length;
      const body = store.slice(h.index, next);
      // Either it is guarded, or it deliberately throws (GROUP 1) — both are honest.
      // What is not allowed is a bare `catch { return [] }`, which is the shape that
      // makes an error indistinguishable from an empty list.
      const honest = /guardedRead\(/.test(body) || !/catch\s*\{/.test(body);
      if (!honest) undecided.push(h[1] + h[2]);
    });

    expect(
      undecided,
      'These reads still swallow their failure into a bare default, so any list they ' +
        'feed says "there is nothing here" when it means "asking failed". Route them ' +
        'through guardedRead(), or make them GROUP 1 and let the caller decide.',
    ).toEqual([]);
  });

  it('and the Templates gallery shows the reason instead of "No templates yet"', () => {
    const t = src('src/lib/views/templates/TemplateGallery.svelte');
    expect(t).toMatch(/readErrors\.loadTemplates/);
    expect(t).toMatch(/<ErrorState/);
    // The empty sentence survives — for the case it is actually true, a filter that
    // matches nothing.
    expect(t).toMatch(/No template matches this filter/);
  });
});
