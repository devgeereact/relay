// RG-167 — `on the screens` WAS TRUE OF THE STORE AND FALSE OF THE ROOM.
//
// The dock's countdown caption reads `on the screens` whenever `$live` carries a
// `countdown_to`. That is a fact about what Relay BROADCAST, and two kinds of
// screen silently do not show it:
//
//   · a template whose explicit `shows` allow-list omits `countdown` drops the
//     fire before any renderer sees it, and the screen holds whatever it had.
//     `layers.js` says so in as many words at that site: *"This is the site that
//     hides a kind SILENTLY … Nothing anywhere reports that."*
//   · a KEYED template refuses it (RG-166), because painting a clock over a live
//     camera takes the preacher off the stream.
//
// Both are properties of the template a screen resolves, so the answer needs no
// screen to say anything — which is why the line can stand under the transport
// before a countdown has ever been fired, at the moment an operator is setting
// one up and can still fix it.
//
// Rule 35: *if the answer is the same when the thing behind it is broken as when
// everything is fine, it is not a status line.*
//
//   npx vitest run src/lib/countdownreach.test.js

import { describe, it, expect } from 'vitest';
import { describeCountdownReach } from './channelroles.js';
import { resolveOutputTemplate } from './layers.js';

// ── THE FIXTURES, EACH A REAL TEMPLATE SHAPE ────────────────────────────────

/** An ordinary opaque wall template: paints its own frame, shows everything. */
const PLAIN = { id: 1, name: 'Main look', layout: { regions: ['reference', 'verse_text'] }, style: { background: '#120d08' } };
/** A screen told to show scripture and songs and nothing else. */
const NO_COUNTDOWN = { id: 2, name: 'Lobby look', layout: { regions: ['verse_text'], shows: ['scripture', 'song'] }, style: { background: '#000' } };
/** A layer-model lower third — keyed, and carrying no `lowerThird` flag (RG-166). */
const KEYED = {
  id: 3,
  name: 'Stream look',
  layout: {
    shows: ['scripture', 'song', 'countdown'],
    layers: [
      { id: 'b', type: 'shape', visible: true, x: 4, y: 76, w: 92, h: 20, fill: '#0b0f16', opacity: 1 },
      { id: 't', type: 'text', visible: true, bind: 'verse', x: 6, y: 79, w: 88, h: 12, size: 3 },
    ],
  },
  style: {},
};

const TEMPLATES = [PLAIN, NO_COUNTDOWN, KEYED];
const chan = (id, name, templateId) => ({ id, name, template_id: templateId, role: null, render_target: 'native_window' });

/** The resolver the dock hands in — the real one, so this cannot disagree with the wall. */
const resolve = (c) =>
  resolveOutputTemplate(TEMPLATES.find((t) => t.id === c.template_id) ?? null, null, false, PLAIN);

describe('which screens would show a Screen Countdown', () => {
  it('says so plainly when every screen would', () => {
    const rows = [chan(1, 'Main screen', 1), chan(2, 'Overflow', 1), chan(3, 'Foyer', 1), chan(4, 'Creche', 1)];
    expect(describeCountdownReach(rows, resolve, { read: true }).text).toBe('Goes to all 4 screens');
  });

  it('names the screen that is set not to show it', () => {
    // BY NAME, never a count alone. "3 of 4" tells an operator that something is
    // wrong and not which screen to go and look at, mid-service, with a
    // congregation waiting. `degraded.js` learned this the hard way — its banner
    // shipped with ids in it for months, because the producer sent ids and the
    // documentation said names.
    const rows = [chan(1, 'Main screen', 1), chan(2, 'Overflow', 1), chan(3, 'Lobby screen', 2), chan(4, 'Creche', 1)];
    expect(describeCountdownReach(rows, resolve, { read: true }).text).toBe(
      'Goes to 3 of 4 screens · Lobby screen ignores it',
    );
  });

  it('names the keyed screen, and says why rather than only that', () => {
    // "Streaming ignores it" would be true and useless. The reason is the whole
    // value: a keyed channel is composited over a camera, and an operator who
    // does not know that will go and tick a box that is not the problem.
    const rows = [chan(1, 'Main screen', 1), chan(2, 'Overflow', 1), chan(3, 'Streaming', 3), chan(4, 'Creche', 1)];
    expect(describeCountdownReach(rows, resolve, { read: true }).text).toBe(
      'Goes to 3 of 4 screens · Streaming is keyed — the countdown will not go there',
    );
  });

  it('names BOTH exceptions, by screen name, when both happen at once', () => {
    const rows = [chan(1, 'Main screen', 1), chan(2, 'Lobby screen', 2), chan(3, 'Streaming', 3), chan(4, 'Creche', 1)];
    expect(describeCountdownReach(rows, resolve, { read: true }).text).toBe(
      'Goes to 2 of 4 screens · Lobby screen ignores it · Streaming is keyed — the countdown will not go there',
    );
  });

  it('says when it would reach nobody at all', () => {
    const rows = [chan(1, 'Lobby screen', 2), chan(2, 'Streaming', 3)];
    expect(describeCountdownReach(rows, resolve, { read: true }).text).toBe(
      'No screen would show it · Lobby screen ignores it · Streaming is keyed — the countdown will not go there',
    );
  });

  it('and says so when the read failed, rather than reporting a quiet church', () => {
    // THE HALF RULE 35 IS ACTUALLY ABOUT. `listOutputChannels` is a GROUP 2 read:
    // it swallows and answers `[]`, which renders exactly like a church with no
    // screens set up. The reason lives in `readErrors`, and the two must not
    // produce one sentence.
    expect(describeCountdownReach([], resolve, { read: true, error: 'the engine is not answering' }).text).toBe(
      'Cannot tell which screens would show it',
    );
    expect(describeCountdownReach([], resolve, { read: false }).text).toBe(
      'Cannot tell which screens would show it',
    );
    // …and genuinely having no screens is its own answer, not that one.
    expect(describeCountdownReach([], resolve, { read: true }).text).toBe(
      'No screens are set up — nothing would show it',
    );
  });

  it('a resolver that threw is an unread answer, not an empty one', () => {
    const boom = () => {
      throw new Error('no');
    };
    expect(describeCountdownReach([chan(1, 'Main screen', 1)], boom, { read: true }).text).toBe(
      'Cannot tell which screens would show it',
    );
  });
});

describe('the wording holds up at the edges', () => {
  it('a single screen is not "all 1 screens"', () => {
    expect(describeCountdownReach([chan(1, 'Main screen', 1)], resolve, { read: true }).text).toBe(
      'Goes to the one screen',
    );
  });

  it('two excluded screens read as a list, not as two sentences', () => {
    const rows = [chan(1, 'Main screen', 1), chan(2, 'Lobby', 2), chan(3, 'Creche', 2)];
    expect(describeCountdownReach(rows, resolve, { read: true }).text).toBe(
      'Goes to 1 of 3 screens · Lobby and Creche ignore it',
    );
  });

  it('a screen with no name gets one rather than an id in prose', () => {
    const rows = [chan(1, '', 2), chan(2, 'Main screen', 1)];
    expect(describeCountdownReach(rows, resolve, { read: true }).text).toBe(
      'Goes to 1 of 2 screens · Screen 1 ignores it',
    );
  });
});

describe('the two exclusions are the real rules, not a copy of them', () => {
  it('the keyed verdict comes from `isKeyedTemplate`, so RG-166 and this line agree', () => {
    // Give the keyed template a full-frame background layer and it stops being
    // keyed — the renderer would paint the countdown, and so this line must stop
    // naming it. A hand-rolled `lowerThird` check here would answer the same for
    // both, which is the defect RG-166 fixed one layer down.
    const opaque = {
      ...KEYED,
      layout: {
        ...KEYED.layout,
        layers: [{ id: 'bg', type: 'background', visible: true, fill: '#000', opacity: 1 }, ...KEYED.layout.layers],
      },
    };
    const tpls = [PLAIN, opaque];
    const r = (c) => resolveOutputTemplate(tpls.find((t) => t.id === c.template_id) ?? null, null, false, PLAIN);
    expect(describeCountdownReach([chan(1, 'Main screen', 1), chan(2, 'Streaming', 3)], r, { read: true }).text).toBe(
      'Goes to all 2 screens',
    );
  });

  it('a screen following the content look is judged on what it would actually resolve', () => {
    // `template_id: null` means "follow the default" (§70). Judging it on its own
    // absent template would ask `isKeyedTemplate(null)`, which answers TRUE, and
    // every following screen would be reported as keyed.
    const rows = [chan(1, 'Main screen', null), chan(2, 'Overflow', null)];
    expect(describeCountdownReach(rows, resolve, { read: true }).text).toBe('Goes to all 2 screens');
  });
});

// ── AND THE DOCK ACTUALLY RENDERS IT ────────────────────────────────────────
//
// The rule above is pure so the dock and Live cannot disagree about the same
// screens. That is worth nothing if the transport renders none of it. This
// repository has shipped fourteen passing tests against a component nothing
// imported; a helper nobody calls is the same shape one level down.

import { beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const cap = await import('./stores/capture.js');
const Dock = (await import('./Dock.svelte')).default;

let host;
let app;

function bridge({ channels = [], templates = [] } = {}) {
  invoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_output_channels') return channels;
    if (cmd === 'list_templates') return templates;
    if (cmd === 'list_timers') return [];
    if (cmd === 'get_sensitivity') return 50;
    if (cmd === 'get_default_template') return null;
    return null;
  });
}

/**
 * MOUNT AND LET THE POLL RUN, for the reason `wayback.test.js` records at the
 * same point: the dock's mount-time reads do not all reach the mocked bridge in
 * this environment, so it is the two-second poll these tests watch.
 */
async function mountAndRead() {
  host = document.createElement('div');
  document.body.appendChild(host);
  vi.useFakeTimers({ shouldAdvanceTime: true });
  app = new Dock({ target: host, props: {} });
  await vi.advanceTimersByTimeAsync(2100);
  await tick();
  vi.useRealTimers();
  await new Promise((r) => setTimeout(r, 10));
  await tick();
  // …AND THEN WAIT FOR THE ANSWER, RATHER THAN FOR A FIXED NUMBER OF TICKS.
  //
  // The reads behind this line are two awaited commands and a derivation, and a
  // fixed settle is a bet on how many microtask turns a runtime takes to get
  // through them. CI runs Node 20, 22 and 24 precisely because that bet is not
  // portable, and it lost: this helper settled on 22 and 24 and did not on 20,
  // where the line still read `Cannot tell which screens would show it` — which is
  // the rule-35 fallback answering CORRECTLY about a read that had not landed.
  //
  // Bounded, so a line that never resolves still fails the test rather than
  // hanging it, and the assertion that follows is then about the answer rather
  // than about the scheduler.
  for (let i = 0; i < 60; i += 1) {
    const t = host.querySelector('.cdreach')?.textContent ?? '';
    if (t && !/^\s*Cannot tell/.test(t)) break;
    await new Promise((r) => setTimeout(r, 5));
    await tick();
  }
  return host;
}

const line = () => host.querySelector('.cdreach');

beforeEach(() => {
  invoke.mockReset();
  bridge();
  cap.live.set(null);
  cap.templates.set([]);
  cap.readErrors.set({});
  cap.capture.update((s) => ({ ...s, available: true }));
});
afterEach(() => {
  vi.useRealTimers();
  app?.$destroy();
  host?.remove();
  app = host = null;
  cap.readErrors.set({});
});

describe('the transport renders the answer', () => {
  it('names the screen that ignores it, on the rendered control', async () => {
    cap.templates.set(TEMPLATES);
    bridge({ channels: [chan(1, 'Main screen', 1), chan(2, 'Lobby screen', 2)], templates: TEMPLATES });
    await mountAndRead();
    expect(line(), 'the countdown block renders no reach line at all').toBeTruthy();
    expect(line().textContent).toContain('Lobby screen ignores it');
  });

  it('and says something different when every screen would show it', async () => {
    cap.templates.set(TEMPLATES);
    bridge({ channels: [chan(1, 'Main screen', 1), chan(2, 'Overflow', 1)], templates: TEMPLATES });
    await mountAndRead();
    expect(line().textContent.trim()).toBe('Goes to all 2 screens');
  });

  it('wears no law colour, whatever it says', async () => {
    // Amber is ON AIR and is never allowed to lie, cyan is a guess, amethyst is
    // rehearsal (rule 18). "which screens would show this" is none of the three.
    cap.templates.set(TEMPLATES);
    bridge({ channels: [chan(1, 'Lobby screen', 2)], templates: TEMPLATES });
    await mountAndRead();
    for (const cls of ['onair', 'inreh', 'guess', 'amber']) {
      expect(line().classList.contains(cls), `the line wears .${cls}`).toBe(false);
    }
  });
});
