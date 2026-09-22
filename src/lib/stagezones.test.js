// W5 · THE STAGE MONITOR — zones, the stacked rail clock, and the geometry that
// cannot overflow. docs/REBRAND.md §5.
//
// A stage monitor is not a congregation screen in other colours. Six zones, each
// switchable, and the figures either beside the reading or across the bottom.
//
// TWO INSTRUMENTS, and the split is deliberate. jsdom has no layout engine, so
// "nothing may leave the screen" cannot be measured here — a `getBoundingClientRect`
// in jsdom is zeroes, and a test that asserts on zeroes passes over a broken page.
// So the geometry is held as a CONTRACT ON THE STYLESHEET (the reading is the only
// `flex: 1 1 0`, the rows below are `flex-basis` and never `height`, and each is
// clipped), and the rendered measurement is a browser job recorded in the PR.
// Everything that is real in jsdom — what renders, what persists, what the figures
// say — is asserted against the mounted component.
//
//   npx vitest run src/lib/stagezones.test.js
// NOTE (2026-09-21, DECISIONS §116): a Stage Message is now a NOTE by default and
// an ALARM only when the operator asks. Every frame below carries `urgent: true`
// because these tests are about the alarm — the full-bleed panel, its sizing and
// what a panic control does to it. The quiet path has its own tests in
// `stagemessagenative.test.js`.

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { tick } from 'svelte';
import { DEFAULT_STAGE_ZONES } from './stagelayout.js';

const Stage = (await import('../Stage.svelte')).default;
const { formatCountdown } = await import('./layers.js');

const SRC = readFileSync(path.resolve(__dirname, '../Stage.svelte'), 'utf8');
const ZONE_KEY = 'relay.stage.zones';

// The page opens a kiosk socket on mount. A stub, so the component mounts exactly
// as it does in a browser and messages can be pushed through the real `apply`.
let socket;
class FakeSocket {
  constructor() {
    socket = this;
    this.sent = [];
  }
  send(m) {
    this.sent.push(m);
  }
  close() {
    this.closed = true;
  }
}

beforeEach(() => {
  socket = null;
  globalThis.WebSocket = FakeSocket;
  try {
    localStorage.removeItem(ZONE_KEY);
  } catch {
    /* the shim in test-setup.js hands us a real Storage */
  }
});
afterEach(() => {
  cleanup();
});

let host;
let app;

function cleanup() {
  if (app) app.$destroy();
  if (host) host.remove();
  app = null;
  host = null;
}

/**
 * Mount the stage page and deliver one hub frame through the real socket path.
 *
 * THE PAGE IS MOUNTED ON A CHANNEL, and the role map is delivered before
 * anything else. `stage.html` used to accept a Stage Message from anybody; it
 * now refuses one unless its own channel holds the `stage` role, exactly as
 * `output.html` does (`stagepageidentity.test.js`). Channel 2 is the screen a
 * fresh install seeds as `Stage display`, so this fixture is the stage monitor a
 * church actually has rather than an anonymous page that happens to work.
 */
const STAGE_ROLES = { 1: 'main', 2: 'stage' };
async function mount(frame) {
  window.history.replaceState({}, '', '/stage.html?channel=2');
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  socket.onmessage({ data: JSON.stringify({ kind: 'channel_roles', roles: STAGE_ROLES }) });
  if (frame) {
    socket.onmessage({ data: JSON.stringify(frame) });
  }
  await tick();
  await tick();
  return { container: host };
}

/** The button whose visible text is exactly this. */
function button(text) {
  const b = [...host.querySelectorAll('button')].find((n) => n.textContent.trim() === text);
  expect(b, `no control reads "${text}"`).toBeTruthy();
  return b;
}
async function click(text) {
  button(text).click();
  await tick();
}

/**
 * SET THE ZONES THE WAY AN OPERATOR NOW DOES (RG-241).
 *
 * The picker left the phone: it was one tap from the screen a preacher is
 * reading mid-sermon, and on an unassigned screen it let that person switch off
 * the clock they were relying on. Every case below used to reach the zones by
 * tapping it; they reach them the way the product does now, over the layout
 * frame the desk publishes.
 *
 * What each case ASSERTS is unchanged — this is the door, not the subject.
 */
/** Open the control panel the way RG-246 made it open: the search button. */
async function holdOpen() {
  host.querySelector('.sctl-find').click();
  await tick();
}

async function assign(over) {
  // The frame is a MAP, screen id → that screen's zones: the hub addresses no
  // client (DECISIONS §35), so every page is sent every screen's layout and
  // picks out its own. This fixture is channel 2.
  socket.onmessage({
    data: JSON.stringify({
      kind: 'stage_zones',
      zones: { 2: { ...DEFAULT_STAGE_ZONES, ...over } },
    }),
  });
  await tick();
  await tick();
}

const verse = {
  kind: 'content',
  reference: 'Romans 8:28',
  text: 'And we know that all things work together for good.',
  translation: 'KJV',
};

// S3 · THE SWITCHES REMOVE THINGS. THEY ARE NOT HOW THINGS ARRIVE.
//
// §5 said "clean by default … the rest is switched on", and `next`, `note` and
// `elapsed` shipped OFF. All three carry something an operator DELIBERATELY SENT
// TO THE PREACHER and to nobody else — a line typed against a cue, an Up Next
// published to the stage, the service clock. So the operator typed a word to the
// preacher, the console showed it had gone, and the preacher's screen showed
// nothing, because of a switch on a device the operator cannot see. No surface
// anywhere reports that: rule 35's shape on the one screen whose reader cannot
// glance at the console to find out what happened.
//
// "Clean by default" survives anyway, and that is the half worth testing, because
// it is the reason this is safe rather than a busier screen: four of the six
// zones render NOTHING unless the operator has made something for them to render.
describe('zones — everything the operator sent, and nothing they did not', () => {
  it('a fresh stage screen carries the note, the Up Next and the service clock the operator sent it', async () => {
    const note = 'Wrap at 11:40';
    const { container } = await mount({ ...verse, stage_note: note, service_started_at: Date.now() - 60_000 });
    socket.onmessage({ data: JSON.stringify({ kind: 'stage_next', label: 'Offering', text: 'Ushers come forward' }) });
    await tick();

    expect(container.querySelector('.reading')).toBeTruthy();
    expect(container.querySelector('.reading').textContent).toContain('Romans 8:28');
    // Nobody had to find a switch for any of these three.
    expect(container.querySelector('.noterow')?.textContent).toContain(note);
    expect(container.querySelector('.next')?.textContent).toContain('Offering');
    expect(container.textContent).toContain('Elapsed');
    // The clock is a figure rather than a fixed header item — which is what makes
    // it switchable at all.
    expect(container.querySelector('.figrow')).toBeTruthy();
    expect(container.textContent).toContain('Time');
  });

  it('and a screen nobody has sent anything to is still just the reading, the countdown and the clock', async () => {
    // The other half of the same change, and the one that keeps §5's sentence
    // true. Four zones are on and four render nothing, because a zone with no
    // content behind it is not a row — there is no note, no Up Next, and no
    // service recording in this frame.
    const { container } = await mount(verse);

    expect(container.querySelector('.reading')).toBeTruthy();
    expect(container.querySelector('.noterow')).toBeNull();
    expect(container.querySelector('.next')).toBeNull();
    expect(container.textContent).not.toContain('Elapsed');
    expect(container.textContent).toContain('Time');
  });

  it('switching a zone off hides it, and the choice survives a reload of this device', async () => {
    // The switches now go the other way, which is the direction they are actually
    // for: a lobby TV that should not carry the preacher's note switches it off,
    // once, on that device. Persisted per DEVICE — two stage screens in one
    // building are allowed to want different things, and the console must not have
    // to know about either.
    const note = 'Two minutes on the offering';
    const { container } = await mount({ ...verse, stage_note: note });
    expect(container.querySelector('.noterow')).toBeTruthy();

    await assign({ note: false });
    expect(container.querySelector('.noterow')).toBeNull();

    // AND IT IS THE OPERATOR'S, so it survives a reload of the phone because the
    // desk sends it again — not because the phone wrote it down. That is the
    // difference RG-241 made: the arrangement is a fact about the screen, held
    // where the person responsible for it can see it.
    cleanup();
    const again = await mount({ ...verse, stage_note: note });
    await assign({ note: false });
    expect(again.container.querySelector('.noterow')).toBeNull();
  });

  it('a device that cannot store anything still gets the default layout', async () => {
    const spy = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('site data blocked');
    });
    const { container } = await mount({ ...verse, stage_note: 'Wrap at 11:40' });
    expect(container.querySelector('.reading')).toBeTruthy();
    expect(container.querySelector('.figrow')).toBeTruthy();
    // …and the default layout is the one that shows the operator's note. A kiosk
    // with site data blocked is exactly the device nobody can go and configure.
    expect(container.querySelector('.noterow')).toBeTruthy();
    spy.mockRestore();
  });

  // ── WAVE 4 TRACK A · THE SEVENTH ZONE ───────────────────────────────────────
  //
  // `ZONES` had six keys and the programme rail was not one of them, so the one
  // region on this screen that carries the OPERATOR'S bookkeeping was the one
  // region with no switch behind it. A lobby TV running the stage page had no way
  // to stop showing the preacher's programme, and §5's whole premise is that the
  // switches remove things on the device that wants them removed.
  const programme = (at) => ({
    kind: 'timer',
    timers: [
      {
        id: 7,
        label: 'Offering',
        countdown_to: at + 300_000,
        countdown_from: at,
        countdown_paused_ms: null,
        countdown_done: '',
        warn_ms: null,
      },
    ],
  });

  it('the programme rail is a zone, it is on by default, and switching it off gives up its room', async () => {
    const { container } = await mount(programme(Date.now()));
    expect(
      container.querySelector('.progrow'),
      'every other zone ships on; this one must too',
    ).toBeTruthy();

    await assign({ programme: false });

    // Not hidden — GONE. `.progrow` is a `flex-basis: auto` row, so a switched-off
    // rail that still rendered would keep taking the room it needs for its content.
    expect(container.querySelector('.progrow')).toBeNull();

    // The phone writes nothing down any more (RG-241) — the arrangement is the
    // desk's and arrives again on the next connection, which is what makes it a
    // fact about the SCREEN rather than about whichever device was pointed at
    // it. So the reopened page is asked, and answers.
    cleanup();
    const again = await mount(programme(Date.now()));
    await assign({ programme: false });
    expect(again.container.querySelector('.progrow')).toBeNull();
  });

  it('a layout stored before this zone existed still loads, and gets the default', async () => {
    // `loadZones` reads key by key OFF THE DEFAULTS, which is what makes a zone
    // added in a later version arrive switched ON rather than `undefined` — and
    // `undefined` is not false in a `{#if}`, so this is the difference between the
    // rail appearing and the whole page reading a stale object. Written out as a
    // stored payload from the version before this one, not as a doctored object.
    localStorage.setItem(
      ZONE_KEY,
      JSON.stringify({
        reading: true,
        next: true,
        note: false,
        countdown: true,
        clock: true,
        elapsed: true,
        figures: 'beside',
      }),
    );

    const { container } = await mount(programme(Date.now()));
    expect(container.querySelector('.progrow'), 'the new zone did not get its default').toBeTruthy();

    // …and the six that WERE stored are untouched, `figures` included — a
    // migration that quietly resets a device's layout is a migration nobody asked
    // for. `figures` is the one of the seven that is not a boolean, so it is the
    // one a key-by-key loop is most likely to lose.
    // …and the six that WERE stored are untouched, `figures` included. There is
    // no panel to read them off any more (RG-241), so they are read where they
    // now show: the note is off because the stored layout says so, the rail is
    // on because the new zone got its default, and the figures are beside the
    // reading because `figures` is the one stored value that is not a boolean
    // and is the one a key-by-key loop is most likely to lose.
    expect(container.querySelector('.noterow'), 'a stored choice was reset').toBeNull();
    expect(container.querySelector('.progrow'), 'the new zone did not get its default').toBeTruthy();
    expect(container.querySelector('.rail'), 'the stored figures placement was lost').toBeTruthy();
  });
});

describe('the stacked rail clock', () => {
  it('beside the reading, the countdown is three stacked pairs and the rail counts its own rows', async () => {
    const remaining = 3_725_000; // 1:02:05
    const { container } = await mount({
      ...verse,
      text: null,
      countdown_to: Date.now() + remaining,
    });

    await assign({ figures: 'beside' });

    const rail = container.querySelector('.rail');
    expect(rail, 'the figures did not move beside the reading').toBeTruthy();

    // S4 · the VALUE span, not the row's text — every rail row now carries its
    // own label as well, so `textContent` would read "Hrs 01".
    const rows = [...rail.querySelectorAll('.railrow .figv')].map((n) => n.textContent.trim());
    // HH / MM / SS, each its own row, plus the clock row (on by default).
    const [hh, mm, ss] = formatCountdown(remaining, 'hms').split(':');
    expect(rows.slice(0, 3)).toEqual([hh.padStart(2, '0'), mm, ss]);

    // `--rows` is what keeps the stack inside the rail's own height. It must count
    // the rows that are actually there, or the figure is sized for a layout nobody
    // is looking at.
    expect(rows).toHaveLength(4); // HH · MM · SS · the clock
    expect(rail.style.getPropertyValue('--rows').trim()).toBe(String(rows.length));
  });

  it('the pairs are the ONE formatter split, not a second piece of arithmetic', () => {
    // The rail and the figure beneath the reading must not be able to disagree, and
    // the way that is guaranteed is that one is literally the other's output.
    expect(SRC).toMatch(/formatCountdown\(cdRemain, 'hms'\)\s*\n?\s*\.split\(':'\)/);
    expect(SRC).not.toMatch(/Math\.floor\([^)]*3600/);
  });
});

describe('nothing may leave the screen', () => {
  // The stylesheet IS the guarantee here, so the stylesheet is what is asserted.
  // A row given a `height` is a floor a long passage pushes past — which is how a
  // clock leaves the top of a monitor nobody is standing next to.
  const style = SRC.slice(SRC.indexOf('<style>'));
  const rule = (sel) => {
    const i = style.indexOf(`${sel} {`);
    expect(i, `no rule for ${sel}`).toBeGreaterThan(-1);
    return style.slice(i, style.indexOf('}', i));
  };

  it('the reading takes what is left', () => {
    expect(rule('.reading')).toContain('flex: 1 1 0');
    expect(rule('.reading')).toContain('min-height: 0');
  });

  it('the rows below are the only fixed sizes, and they are a BASIS', () => {
    for (const sel of ['.rail', '.figrow']) {
      const r = rule(sel);
      expect(r, `${sel} must be a flex basis`).toMatch(/flex: 0 0 /);
      expect(r, `${sel} must not be given a height`).not.toMatch(/[^-]height:\s*\d/);
      expect(r, `${sel} must clip`).toContain('overflow: hidden');
    }
    expect(rule('.noterow')).toContain('overflow: hidden');
  });

  // S3 · AND THE TWO ROWS THAT ARE NOW ON BY DEFAULT.
  //
  // `.next` had no ceiling at all — nothing bounded it but a `-webkit-line-clamp`,
  // a vendor property doing load-bearing layout work, and no `overflow` behind it
  // if that property is not honoured. It got away with it for as long as the zone
  // was off by default and nobody's screen had the row on it. Switching a zone on
  // is what makes its bound necessary, so the two land together.
  it('the Up Next and the note are bounded and clipped, like every other row', () => {
    for (const sel of ['.next', '.noterow']) {
      const r = rule(sel);
      expect(r, `${sel} needs a ceiling, not a line-clamp`).toMatch(/max-height:\s*\d/);
      expect(r, `${sel} must clip`).toContain('overflow: hidden');
      // A BASIS, never a height — the same trap the rail and the figure row name.
      expect(r, `${sel} must not be given a height`).not.toMatch(/[^-]height:\s*\d/);
    }
  });

  it('the rail is its own container, so the figure is a share of the rail', () => {
    expect(rule('.rail')).toContain('container-type: size');
    // …and the figure is sized in CONTAINER units, never viewport ones — a figure
    // sized against the frame looked right at one rail width and was clipped at
    // every other.
    const row = rule('.railrow');
    expect(row).toMatch(/cqw/);
    expect(row).toMatch(/cqh/);
    expect(row).not.toMatch(/\d(vw|vh)/);
    // AND IT COUNTS ITS CHARACTERS. `62cqw` fills a rail with a two-character pair
    // and put an eight-character clock at 205px in a 333px rail — clipped, silently,
    // because the row is `overflow: hidden`. Measured in a browser at 1280×800.
    expect(row, 'the rail figure must be sized by how many characters it has').toMatch(
      /var\(--ch/,
    );
    expect(rule('.fig .figv'), 'the bottom row needs the same rule').toMatch(/var\(--ch/);
  });
});

// S3 · THE READING FILLS THE ROOM IT HAS.
//
// `clamp(26px, 7vw, 64px)` is a ceiling, and on the screen this page exists for it
// was the binding one: a 1920×1080 platform monitor gave the verse 64px of type in
// an 800px-tall reading area — about a third of the height available — while
// ProPresenter's stage display fills it. A ceiling cannot know how much text it was
// given, so it is set for the longest passage and is then wrong for every ordinary
// one, and an ordinary one is what a stage monitor shows for almost all of a
// service.
//
// jsdom has no layout engine, so the SIZE is a browser measurement recorded in the
// PR. What is real here is the CONSTRUCTION: the reading is its own container, the
// verse is sized from its own character count in container units, and the old
// ceiling is gone.
describe('the reading is sized to the room and to the passage', () => {
  const style = SRC.slice(SRC.indexOf('<style>'));
  const rule = (sel) => {
    const i = style.indexOf(`${sel} {`);
    expect(i, `no rule for ${sel}`).toBeGreaterThan(-1);
    return style.slice(i, style.indexOf('}', i));
  };

  it('the reading is its own container, so the verse is a share of the reading', () => {
    // Not of the FRAME. With the figures across the bottom the reading is the frame
    // minus a fifth; beside them it is the frame minus a quarter of its width. A
    // verse sized against the frame is right in one layout and wrong in the other,
    // and the zones are switchable, so both happen on the same device.
    expect(rule('.reading')).toContain('container-type: size');
  });

  it('the verse is sized from how much text it has, in container units, with no fixed ceiling', () => {
    const v = rule('.verse');
    expect(v, 'the verse must read its own length').toMatch(/var\(--vn/);
    expect(v, 'both bounds come off the same measure').toMatch(/var\(--vcpl/);
    expect(v).toMatch(/cqw/);
    expect(v).toMatch(/cqh/);
    // The viewport is the wrong ruler here — that is what the reading being a
    // container is for.
    expect(v, 'a share of the reading, never of the viewport').not.toMatch(/\d(vw|vh)\b/);
    // And the floor survives: below it the fit has decided a passage cannot be
    // shown whole and §5's recorded deviation takes over — the reading SCROLLS
    // rather than clipping, because a preacher reading aloud must not lose the end
    // of a passage.
    expect(v).toMatch(/max\(26px/);
    expect(rule('.reading')).toContain('overflow: auto');
  });

  it('and the character count actually reaches the stylesheet', () => {
    // Guards the guard: the two assertions above are about a variable, and a
    // variable nothing sets is a default nothing can move off. `--vn` is the one
    // number CSS cannot count for itself.
    expect(SRC).toMatch(/--vn:\{verseChars\}/);
  });

  it('and the measure is set per passage, not once for every passage', () => {
    // The same guard for the other half. A `--vcpl` only the stylesheet sets is a
    // measure that cannot answer to the passage in front of it.
    expect(SRC).toMatch(/--vcpl:\{verseCpl\}/);
  });

  it('a long passage is given a wider column than a short one', async () => {
    // THE DEFECT. The fit takes the smaller of a width bound and a height bound,
    // and the measure sets both: narrow the column and the line gets short (raising
    // the width bound) while the passage gets tall (lowering the height bound). One
    // fixed measure is wrong at one end, and 22 was wrong at the long end — Esther
    // 8:9 rendered at the 26px floor on a 1920×1080 platform monitor with the time
    // of day beneath it at 84px. Scripture a third the size of the clock, on the
    // screen the preacher reads from.
    const cpl = async (n) => {
      const { container } = await mount({ ...verse, text: 'x'.repeat(n) });
      return Number(container.querySelector('.verse').style.getPropertyValue('--vcpl'));
    };
    const short = await cpl(64); // Psalms 23:1
    const long = await cpl(530); // Esther 8:9
    expect(long, 'a long passage must not be held to a short one’s measure').toBeGreaterThan(
      short,
    );
    // And both inside a measure a person can actually read a line of.
    for (const v of [short, long]) {
      expect(v).toBeGreaterThanOrEqual(16);
      expect(v).toBeLessThanOrEqual(64);
    }
    // Monotonic: there is no length at which asking for more text narrows the
    // column, which is the shape of an arithmetic slip in the square root.
    let prev = 0;
    for (const n of [20, 64, 140, 300, 530, 900]) {
      const v = await cpl(n);
      expect(v, `measure went backwards at ${n} characters`).toBeGreaterThanOrEqual(prev);
      prev = v;
    }
  });

  it('the verse really does get its own length', async () => {
    const text = 'And we know that all things work together for good to them that love God.';
    const { container } = await mount({ ...verse, text });
    expect(container.querySelector('.verse').style.getPropertyValue('--vn').trim()).toBe(
      String(text.length),
    );
  });
});

describe('the Stage Message', () => {
  // S3 · A MESSAGE THAT DOES NOT FIT IS A MESSAGE NOBODY READ.
  //
  // §5 fixes the type at 8.5cqw and the panel at `overflow: hidden`, which is the
  // right pair for the message §5 describes ("Wrap up — 5 minutes") and the wrong
  // pair for the one an operator types when something has actually gone wrong.
  // Those ran past the bottom of the box and were clipped, silently, on the one
  // surface in the product whose whole purpose is that a person reads every word
  // of it while facing a congregation.
  const longer = [
    ['Wrap up — 5 minutes', 'xl'],
    ['Wrap up in five minutes please — the band is waiting', 'lg'],
    [
      'Wrap up in five minutes please. The band is already on the platform and we still have the offering and the announcements to get through.',
      'md',
    ],
    // THE LONGEST MESSAGE THAT CAN ARRIVE, and the step that has to take it.
    // This row read `['x'.repeat(240), 'sm']` and was green over a defect: a
    // 240-character message cannot reach this page at all, because
    // `main::send_stage_alert` takes the first 140 characters — so the `sm` step,
    // which only began above 150, was unreachable and `.alert.sm` could not
    // render (RG-165). A test that asserts on a state nothing can produce is the
    // same mistake as the branch it was asserting on.
    ['x'.repeat(140), 'md'],
  ];

  for (const [text, step] of longer) {
    it(`a ${text.length}-character message is sized "${step}", not clipped`, async () => {
      const { container } = await mount(verse);
      socket.onmessage({ data: JSON.stringify({ kind: 'stage_alert', text, urgent: true }) });
      await tick();
      const el = container.querySelector('.alert');
      expect(el, 'the message did not render at all').toBeTruthy();
      // `classList`, not a substring of `className` — the Svelte scope hash is in
      // there too, and a two-letter needle in a haystack that changes every build
      // is a test that passes for the wrong reason one day.
      expect(el.classList.contains(step), `got "${el.className}"`).toBe(true);
      expect(el.textContent).toContain(text);
    });
  }

  // ── WAVE 4 TRACK E · THE STEPS AND THE CAP ARE ONE FIGURE (RG-165) ────────
  //
  // Read out of BOTH files in one assertion, which is the only shape that holds a
  // claim living in two places: the last step's boundary and `send_stage_alert`'s
  // cap must be the same number, so neither can move without the other and a dead
  // step cannot come back. `crossrefs.test.js` uses the same shape for citations.
  it('the last sizing step is exactly the length the backend will deliver', () => {
    const step = SRC.match(/const ALERT_MAX = (\d+);/);
    expect(step, '`ALERT_MAX` is how the page states its longest step').toBeTruthy();

    const rust = readFileSync(
      path.resolve(__dirname, '../../src-tauri/src/main.rs'),
      'utf8',
    );
    const body = rust.slice(rust.indexOf('fn send_stage_alert'));
    const cap = body.match(/const MAX: usize = (\d+);/);
    expect(cap, 'the cap moved or was renamed').toBeTruthy();
    expect(
      Number(step[1]),
      'a step above the cap is a rule no message can reach',
    ).toBe(Number(cap[1]));
  });

  it('there is no sizing step the stylesheet cannot draw', () => {
    // The other half. Deleting `.alert.sm` while leaving a step that yields `sm`
    // would be the same defect wearing the opposite coat: a class with no rule.
    const steps = [...SRC.matchAll(/size: '(\w+)'/g)].map((m) => m[1]);
    expect(steps.length, 'the steps table moved').toBeGreaterThan(0);
    const style = SRC.slice(SRC.indexOf('<style>'));
    for (const s of steps) expect(style, `no rule for .alert.${s}`).toContain(`.alert.${s} {`);
    expect(style, 'the unreachable step is still in the stylesheet').not.toContain('.alert.sm {');
  });

  it('and the short message is still §5’s own figure, unchanged', () => {
    const style = SRC.slice(SRC.indexOf('<style>'));
    expect(style).toMatch(/\.alert\.xl \{ font-size: 8\.5cqw; \}/);
  });

  it('takes the whole screen even when every zone is switched off', async () => {
    const { container } = await mount(verse);

    await assign({ reading: false, countdown: false, clock: false });
    expect(container.querySelector('.reading')).toBeNull();

    socket.onmessage({
      data: JSON.stringify({ kind: 'stage_alert', text: 'Wrap up — 5 minutes', urgent: true }),
    });
    await tick();
    const alert = container.querySelector('.alert');
    expect(alert, 'an instruction a switched-off zone could hide is not an instruction').toBeTruthy();
    expect(alert.textContent).toContain('Wrap up — 5 minutes');
  });
});


// AMBER MEANS ON AIR, AND A CLOCK IS NOT ON AIR.
//
// Found by driving the real backend: `http://127.0.0.1:8032/stage.html` rendered
// "— standby —" with the clock beside it in `--v-amber` — the ON AIR colour, at
// the largest type on the page, over a page with nothing on air. It was not a
// template's saved default and no operator chose it: this page renders no
// template at all (it imports three pure formatters from `layers.js` and paints
// its own chrome), so the colour was a stylesheet literal.
//
// The prototype's stage rail is `--stg-mc: #4CC9F0`, which is `--v-cyan` — and
// cyan on this console promises A GUESS (rule 18, DECISIONS §21). Swapping one
// promise for another is the same defect in a different hue, so the figures take
// the page's own ink instead. `--v-red` stays on the countdown's warning state,
// which is the one figure here that is a warning.
//
// HOW THIS WAS CHECKED: watched to go RED by restoring `color: var(--v-amber)`
// on `.railrow` — the test names the rule and the promise it broke.
describe('the colour law reaches the stage monitor', () => {
  const style = SRC.slice(SRC.indexOf('<style>'));
  const rule = (sel) => {
    const i = style.indexOf(`${sel} {`);
    expect(i, `no rule for ${sel}`).toBeGreaterThan(-1);
    return style.slice(i, style.indexOf('}', i));
  };

  /** Every token that carries a promise, and what it promises. Same law as
      `colourlaw.test.js`, applied to the one page that file cannot see. */
  const PROMISE = {
    '--v-amber': 'ON AIR',
    '--v-cyan': 'a guess',
    '--v-amethyst': 'rehearsal',
  };

  it('a figure — clock, elapsed or countdown — wears no promise colour', () => {
    for (const sel of ['.railrow', '.fig .figv']) {
      const r = rule(sel);
      for (const [token, means] of Object.entries(PROMISE)) {
        expect(r, `${sel} may not be ${token} — that means ${means}`).not.toContain(token);
      }
      expect(r, `${sel} takes the page's own ink`).toContain('color: var(--v-txt)');
    }
  });

  it('and the guard can still see a promise colour when there is one', () => {
    // Guards the guard: a scanner that matched nothing would pass vacuously,
    // which is how two scanners in this repo were wrong while looking exhaustive.
    expect(rule('.ref'), 'the reference on screen IS on air, and says so').toContain('--v-amber');
  });

  it('the warning state is still red, because that one IS a warning', () => {
    expect(rule('.railrow.warn')).toContain('var(--v-red)');
    expect(rule('.fig.warn .figv')).toContain('var(--v-red)');
  });
});

// ═══ S4 · THE PROPRESENTER-7 READING OF THIS SCREEN ═══════════════════════════
//
// ProPresenter's stage display is a set of DISCRETE, LABELLED REGIONS on black:
// the current slide dominates, everything else is visibly subordinate, and every
// region carries the same small quiet upper-case label. Relay already had every
// element PP7 has. What it did not have was a hierarchy or a label system, and
// all four defects below were found by rendering the page in a browser at
// 1920×1080, 1024×768 and 1080×1920 and looking at the picture.
describe('S4 · the stage reads as one instrument', () => {
  const style = SRC.slice(SRC.indexOf('<style>'));
  const rule = (sel) => {
    const i = style.indexOf(`${sel} {`);
    expect(i, `no rule for ${sel}`).toBeGreaterThan(-1);
    return style.slice(i, style.indexOf('}', i));
  };

  // A figure nobody can name is a figure nobody can use. The row ACROSS THE
  // BOTTOM said COUNTDOWN · TIME · ELAPSED; the rail BESIDE THE READING said
  // nothing at all, so with a countdown running it read
  //     00 · 03 · 42 · 12:01 AM · 45:00
  // — five rows of identical white mono, and the only way to tell the service
  // clock from the countdown was to watch which way it moved. Same two facts, two
  // layouts, labelled in one and bare in the other: the twin-door shape.
  it('every figure on the rail says what it is', async () => {
    const { container } = await mount({
      ...verse,
      text: null,
      countdown_to: Date.now() + 3_725_000,
      service_started_at: Date.now() - 60_000,
    });
    await assign({ figures: 'beside' });

    const rows = [...container.querySelectorAll('.rail .railrow')];
    expect(rows.length, 'the rail did not render').toBeGreaterThan(1);
    for (const row of rows) {
      const k = row.querySelector('.figk');
      expect(k, `a rail row with no label: "${row.textContent.trim()}"`).toBeTruthy();
      expect(k.textContent.trim().length, 'a blank label is not a label').toBeGreaterThan(0);
    }
    expect(rows.map((r) => r.querySelector('.figk').textContent.trim())).toEqual([
      'Hrs',
      'Min',
      'Sec',
      'Time',
      'Elapsed',
    ]);
  });

  // The labels were console pixels on a platform monitor: `.figk` 9px, `.note-lbl`
  // 9px, `.next-lbl` a hardcoded 10px — three treatments, none of which scaled, on
  // a page where the reference, the verse, the note, the Up Next and every figure
  // are all sized to the room. Photographed at 1920×1080 they were hairlines.
  it('every region label is ONE label, and it is sized to the room', () => {
    const r = rule('.figk, .note-lbl, .next-lbl');
    expect(r, 'the label must grow with the screen, not sit at a console size').toMatch(/vmin/);
    expect(r, 'and it must still floor at the size the phone was designed at').toContain(
      'var(--v-fs-fig)',
    );
    // One declaration, so the three cannot drift apart again. Each keeps only its
    // own ink and flex behaviour.
    // Matched at a line start, so this reads the STANDALONE rule and not the
    // grouped selector it also appears in — `indexOf('.next-lbl {')` finds the
    // group first, which would make this assertion pass for the wrong reason.
    for (const sel of ['note-lbl', 'next-lbl']) {
      const own = style.match(new RegExp(`\\n  \\.${sel} \\{([^}]*)\\}`));
      expect(own, `no standalone rule for .${sel}`).toBeTruthy();
      expect(own[1], `.${sel} must not carry its own font-size any more`).not.toMatch(
        /font-size/,
      );
    }
    // The old hardcoded one, by value, so restoring it fails here rather than
    // being caught only by an eye.
    expect(style).not.toMatch(/\.next-lbl[^}]*font-size:\s*10px/);
  });

  // `.figrow.tall` gives the figures 58% of the screen, and its own comment says
  // why: "a pre-service countdown is the whole reason anyone is looking at this
  // page". It was keyed on the reading having no BODY, which is also true of
  // STANDBY — so a page with nothing fired at all gave the wall clock 58% of a
  // platform monitor at 361px while "— standby —" sat above it at 34px.
  it('a wall clock does not get the room a countdown asked for', async () => {
    const { container } = await mount(verse);
    // Standby: content cleared, no countdown anywhere.
    socket.onmessage({ data: JSON.stringify({ kind: 'clear' }) });
    await tick();
    const row = container.querySelector('.figrow');
    expect(row, 'the clock zone is on by default').toBeTruthy();
    expect(
      row.classList.contains('tall'),
      'the time of day took the room a countdown asked for',
    ).toBe(false);

    // …and the exception still fires for the case it was written for.
    cleanup();
    const cd = await mount({ ...verse, text: null, countdown_to: Date.now() + 120_000 });
    expect(
      cd.container.querySelector('.figrow').classList.contains('tall'),
      'a pre-service countdown IS the reason anyone is looking at this page',
    ).toBe(true);
  });

  // The same exception facing sideways, which the rail did not have at all: a
  // countdown beside a bodiless reading left 74% of a platform monitor black and
  // squeezed the figures the room is watching into a quarter of the width.
  it('and the rail takes the room too, for the same reason', async () => {
    const { container } = await mount({
      ...verse,
      text: null,
      countdown_to: Date.now() + 120_000,
    });
    await assign({ figures: 'beside' });

    const rail = container.querySelector('.rail');
    expect(rail).toBeTruthy();
    expect(rail.classList.contains('wide')).toBe(true);
    expect(rule('.rail.wide'), 'a BASIS, never a height').toMatch(/flex-basis/);
    expect(rule('.rail.wide')).not.toMatch(/[^-]height:\s*\d/);

    // A reading with words in it keeps its room.
    socket.onmessage({ data: JSON.stringify(verse) });
    await tick();
    expect(container.querySelector('.rail').classList.contains('wide')).toBe(false);
  });

  // `main.stage` holds the reading and the rail, and the rail already requires the
  // reading zone — so with Reading switched OFF it was an empty `flex: 1 1 0`
  // competing with `.figrow.only`, which is the same. They split the screen, and a
  // stage monitor showing only a clock gave half of itself to a region with
  // nothing in it: 480px of black above the figures at 1920×1080.
  it('a zone that is switched off gives up its room', async () => {
    const { container } = await mount({ ...verse, service_started_at: Date.now() - 60_000 });
    expect(container.querySelector('main.stage')).toBeTruthy();

    await assign({ reading: false });

    expect(container.querySelector('.reading'), 'the reading is off').toBeNull();
    expect(
      container.querySelector('main.stage'),
      'an empty region must not keep holding half the screen',
    ).toBeNull();
    // …and the figures take what it gave up.
    expect(container.querySelector('.figrow').classList.contains('only')).toBe(true);
  });

  // Side by side on one baseline, each figure was sized to its OWN character
  // count, so the row was two or three type sizes pretending to be a row. It only
  // shows where the height bound stops binding — measured on a 1080×1920 portrait
  // panel, TIME at 94px beside ELAPSED at 150px.
  it('every figure across the bottom is one size, set by the longest of them', async () => {
    const { container } = await mount({
      ...verse,
      text: null,
      countdown_to: Date.now() + 120_000,
      service_started_at: Date.now() - 60_000,
    });
    const row = container.querySelector('.figrow');
    const figs = [...row.querySelectorAll('.fig')];
    expect(figs.length, 'countdown · time · elapsed').toBe(3);
    // The size now lives ONCE, on the row.
    expect(row.style.getPropertyValue('--ch').trim()).not.toBe('');
    for (const f of figs) {
      expect(f.style.getPropertyValue('--ch'), 'a per-figure size is the defect').toBe('');
    }
    // And it is the LONGEST value, or the longest figure is the one that clips.
    const longest = Math.max(
      ...[...row.querySelectorAll('.figv')].map((n) => n.textContent.trim().length),
    );
    expect(Number(row.style.getPropertyValue('--ch'))).toBe(longest);
  });
});

// ── WAVE 4 TRACK E · AN OPEN PANEL YIELDS THE RAIL, NOT THE READING ──────────
//
// RG-164. `Zones` and `Control` are two taps in the header, and both of them put a
// panel into the same flex column the reading is in. Measured in a browser at
// 1024×768 with the rail on and both panels open, the reading collapsed to 74.2px,
// the verse was already on its 26px floor with nothing left for the fit to shrink,
// and 38.9px of ink was cut off the bottom of the passage the preacher was reading
// aloud — on a page that is `overflow: hidden` by design, so there was nothing to
// scroll and nothing saying the verse was incomplete. With the Programme zone
// switched off the same two panels left the verse whole
// (`docs/qa/audits/DESIGN.md` §2.3).
//
// jsdom has no layout engine, so the 38.9px is not reproducible here and this file
// already records why (the split at the top). What IS reproducible is the cause:
// whether the rail is in the column at all while a panel is open. That is the fact
// the measurement was a consequence of, and asserting it would have caught the
// finding as the browser pass found it.
describe('an open panel yields the programme rail', () => {
  const clock = (at) => ({
    kind: 'timer',
    timers: [
      {
        id: 7,
        label: 'Sermon',
        countdown_to: at + 1_500_000,
        countdown_from: at,
        countdown_paused_ms: null,
        countdown_done: '',
        warn_ms: null,
      },
    ],
  });

  /** A verse on the screen and a clock on the rail — the state §2.3 measured. */
  async function reading() {
    const r = await mount(verse);
    socket.onmessage({ data: JSON.stringify(clock(Date.now())) });
    await tick();
    await tick();
    expect(r.container.querySelector('.progrow'), 'no rail to yield').toBeTruthy();
    expect(r.container.querySelector('.verse'), 'no verse to protect').toBeTruthy();
    return r.container;
  }

  // THE ZONES PANEL'S TWO CASES WENT WITH THE PANEL (RG-241). The guarantee they
  // held — an open panel yields the RAIL's room and never the verse's — is the
  // Control panel's alone now, and the case below is the whole of it. Deleting
  // them rather than pointing them at the surviving panel keeps this file from
  // asserting the same thing twice under two names.
  it('the Control panel does the same, because it is the same column', async () => {
    // The second door. The measurement in §2.3 opened BOTH, and a fix that knew
    // about one of them would be this repository's recurring bug once more.
    const container = await reading();
    await holdOpen();
    expect(container.querySelector('.ctl'), 'the panel did not open').toBeTruthy();
    expect(container.querySelector('.progrow')).toBeNull();
    expect(container.querySelector('.verse')).toBeTruthy();

    // The same button shuts it. The panel's own `Done` went with the header
    // controls (RG-246): the transport bar is where opening and closing both
    // live now, so there is one control rather than two that must agree.
    host.querySelector('.sctl-find').click();
    await tick();
    expect(container.querySelector('.progrow')).toBeTruthy();
  });

});
