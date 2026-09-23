// THE CLIP CARRIES ITS OWN CONTROLS, AND THEY LEAVE THE CONTROLS CARD (RG-254).
//
// The operator: *"I want to remove the Media functionality, like the play, loop
// and all from the control and look for a suitable place to fix it... I just want
// the Icons like the play icon, pause icon and all with the media slider... also
// the level and all i dont think its needed"*.
//
// **Why it left.** The Controls card is the one card in the product that may
// never scroll (rule 15), because `Clear screens` may never be behind an overflow
// edge. It was spending most of its room on a transport for a clip that is not
// playing for most of a service, and eight controls plus a scrub, a level slider
// and a clock were squeezed into a 178px card beside the panic controls.
//
// **Where it went, and why not the programme pane.** `mediacontrols.test.js`
// holds RG-237's guarantee — ONE set of controls, in the shell, over every
// screen — by asserting `Live.svelte` carries no transport row. The programme
// pane is inside `Live.svelte`, so putting the chrome there would have meant
// rewriting that guarantee to build a convenience. A strip in the SHELL keeps it
// as written and keeps the controls reachable from Templates and Outputs too,
// which is the half of RG-237 that made it a shell control in the first place.
//
// **It costs nothing when there is no clip**, which is the operator's other
// instruction — *"strategise on where to put the functionality that will not let
// the workspace busy or rough"*. The strip is not rendered at all unless a clip
// is on the screens.
import { describe, it, expect, afterEach } from 'vitest';
import { tick } from 'svelte';
import ClipBar from './ClipBar.svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const read = (p) => readFileSync(resolve(p), 'utf8');
const BAR = read('src/lib/ClipBar.svelte');
const DOCK = read('src/lib/Dock.svelte');
const APP = read('src/App.svelte');
const LIVE = read('src/lib/views/Live.svelte');

describe('the media transport has left the Controls card', () => {
  it('the dock renders no transport and no clip clock', () => {
    // The whole block, by every name it went under.
    for (const gone of [
      'clipbar',
      'clipscrub',
      'cliplevel',
      'cliptime',
      'clipvol',
      'Scrub the clip',
      'Clip volume on the screens',
    ]) {
      expect(DOCK, `the dock still carries \`${gone}\``).not.toContain(gone);
    }
  });

  it('and the Controls card gets End service back for good', () => {
    // It was the `{:else}` of `{#if clipLive}` — inert most of a service and
    // replaced by a transport the rest of it. With the transport gone it is
    // simply the control it always was.
    expect(DOCK).toMatch(/endsvc/);
    expect(DOCK, 'the card still branches on a clip').not.toMatch(/\{#if clipLive\}/);
  });

  it('nothing anywhere offers a clip LEVEL any more', () => {
    // The operator's own words. Nothing outside the dock ever read it: the wire
    // field and its clamp stay, so a template or a cue could set one later, but
    // there is no longer a control on the desk for a decision nobody makes from
    // the desk — the sound comes off the desk, not off the screens.
    for (const src of [BAR, DOCK, LIVE]) {
      expect(src).not.toContain('Clip volume on the screens');
    }
  });
});

describe('the strip is in the shell, and only while a clip is live', () => {
  it('the programme pane mounts it, and nothing else does (RG-259)', () => {
    // THIS CASE REVERSED, and the reversal is the record. It shipped as a shell
    // strip because `mediacontrols.test.js` asserts `Live.svelte` carries no
    // transport ROW — and it still does, because what Live mounts is this one
    // shared component rather than a second set of markup. RG-237's guarantee
    // was ever "one set of controls", and one component in one place is the
    // strictest form of it.
    //
    // The drawing's reason is the better one: a scrub bar means something on
    // the frame it refers to and almost nothing on a strip across the shell.
    expect(LIVE).toMatch(/<ClipBar\b/);
    expect(APP, 'a second set of controls').not.toMatch(/<ClipBar\b/);
    expect(DOCK, 'a second set of controls').not.toMatch(/<ClipBar\b/);
    // AND IT IS THE SAME COMPONENT, over the picture rather than beside it.
    expect(LIVE).toMatch(/<ClipBar over \/>/);
  });

  it('renders nothing at all when no clip is on the screens', () => {
    // NOT a disabled bar and not an empty rail: absent. A strip that is always
    // there is exactly the "busy or rough" workspace the operator asked to
    // avoid, and a disabled transport over no clip is a control that owes a
    // reason it cannot give.
    expect(BAR).toMatch(/\{#if\s+clipLive\}/);
  });

  it('is icons and nothing else, each named for somebody who cannot see it', () => {
    // Four controls, four labels. An icon-only button with no `aria-label` is a
    // button that does not exist for a screen reader, and this one fires to
    // every screen in the building.
    // The LABELS, not the attribute spelling: Play/Pause names itself from the
    // state it is in, so its label is a ternary rather than a literal
    // attribute. Asserting the quoting would be asserting a spelling.
    for (const label of [
      'Hold the clip on every screen',
      'Let the clip run on every screen',
      'Start the clip again from the beginning',
      'Repeat the clip when it ends',
      "Put this on the preacher's screen as well",
    ]) {
      expect(BAR, `no control is labelled "${label}"`).toContain(label);
    }
    // And every one of them IS an aria-label, rather than a title or a tooltip
    // that a screen reader never reaches.
    expect((BAR.match(/aria-label/g) ?? []).length).toBeGreaterThanOrEqual(5);
  });

  it('spends no law colour on a clip', () => {
    // Amber means ON AIR, cyan means the AI is guessing, amethyst means
    // rehearsal. A transport is none of those — it is a control, and the ON AIR
    // claim beside it belongs to the screens, not to this bar.
    const css = BAR.slice(BAR.lastIndexOf('<style>'));
    expect(css).not.toMatch(/--v-amber|--v-cyan|--v-amethyst/);
  });
});

describe('the scrub bar says only what it knows', () => {
  it('asks clipPosition, rather than drawing the polled figure raw', () => {
    expect(BAR).toContain("from './clipposition.js'");
    expect(BAR).toMatch(/clipPosition\(/);
  });

  it('sends the seek on the DROP, never on every pixel of the drag', () => {
    // Each frame reaches every screen in the building. A drag across a two
    // minute clip would be hundreds of broadcasts and a wall that stutters
    // while the handle moves. This was RG-221's rule and it survives the move.
    //
    // ASSERTED ON WHAT `on:input` DOES, not on whether it exists. The first
    // version of this case sliced from `<input` to the first `>` and required
    // no `on:input` at all — and the first `>` in that tag is the one inside
    // `=>`, so the slice ended before the handler and the case passed over a
    // tag that had one. A scanner that quietly narrows passes everything.
    //
    // There IS an `on:input` and there has to be: it is what makes the handle
    // follow the operator's finger. What matters is that it touches a local
    // variable and nothing else — `seek` is the only thing that reaches a
    // screen, and it is bound to `change`, which fires on the drop.
    expect(BAR).toMatch(/on:input=\{\(e\) => \(dragMs = Number\(e\.target\.value\)\)\}/);
    expect(BAR).toMatch(/on:change=\{\(e\) => seek\(/);
    const onInput = BAR.slice(BAR.indexOf('on:input='), BAR.indexOf('on:change='));
    for (const reaches of ['send(', 'setMediaTransport', 'seek(']) {
      expect(onInput, `the drag reaches the screens through ${reaches}`).not.toContain(reaches);
    }
    // And `seek` itself is the one door to the wire from this control.
    expect(BAR).toMatch(/const seek = \(ms\) => \{[\s\S]*?send\(\{ seekMs:/);
  });

  it('the handle belongs to the operator while they are holding it', () => {
    // The old control re-applied `value=` on every 2s poll, so a poll landing
    // mid-drag snapped the handle back to where the screen last said it was.
    expect(BAR).toMatch(/dragging/);
    expect(BAR).toMatch(/on:pointerdown/);
  });

  it('and the bar is absent, not zero, when no screen is reporting', () => {
    // A zero-length scrub bar looks usable and can move nothing.
    expect(BAR).toMatch(/\{#if\s+posMs\s*!==\s*null\}/);
  });
});

// ── AND SOMETHING ACTUALLY RENDERS IT ───────────────────────────────────────
//
// Every case above reads the component as TEXT, which is the right instrument
// for "is the control still in the shell" and the wrong one for "does it work".
// It cost exactly what this repository says it costs: `formatCountdown` was
// imported from `countdown.js`, which does not export it — it is `layers.js`'s —
// and the whole file above stayed green while `npm run build` failed. A
// component nothing renders is not covered, however green its tests.
describe('mounted, with a clip on the screens', () => {
  let app;
  let host;

  afterEach(() => {
    app?.$destroy();
    host?.remove();
    app = null;
    host = null;
  });

  async function mount() {
    const { live, channelHealth, mediaTransport } = await import('./stores/capture.js');
    live.set({ media_url: 'http://host:8032/media/7', kind: 'media' });
    mediaTransport.set({ paused: false, loop: false, volume: 1 });
    // ONE SCREEN, PAINTING, HALFWAY THROUGH A TWO MINUTE CLIP.
    channelHealth.set({
      1: {
        online: true,
        painting: true,
        supported: true,
        last_beat_ms: 0,
        name: 'Main screen',
        media: { pos_ms: 60_000, dur_ms: 120_000, paused: false },
      },
    });
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new ClipBar({ target: host });
    await tick();
    await tick();
    return host;
  }

  it('renders the transport, the figures and a usable scrub', async () => {
    await mount();
    expect(host.querySelector('.clipbar'), 'nothing rendered at all').toBeTruthy();
    const scrub = host.querySelector('input[type="range"]');
    expect(scrub, 'no scrub').toBeTruthy();
    expect(Number(scrub.max)).toBe(120_000);
    // THE FIGURES ARE REAL, which is the half a source scan cannot see: a
    // `formatCountdown` that resolved to `undefined` renders as empty text and
    // every assertion about the markup still passes.
    expect(host.textContent).toMatch(/\d+:\d\d/);
    expect(host.textContent).toContain('1:00');
  });

  it('renders nothing when no clip is on the screens', async () => {
    const { live } = await import('./stores/capture.js');
    await mount();
    live.set({});
    await tick();
    expect(host.querySelector('.clipbar'), 'the strip outlived the clip').toBeNull();
  });
});

// ── A DRAG THAT NEVER ENDS FREEZES THE BAR (RG-271) ─────────────────────────
//
// `dragging` is set on `pointerdown` and cleared only in `seek`, which hangs off
// `change`. A press on the thumb that releases without moving it fires no
// `change` at all, so `dragging` stayed true for the rest of the service and
// `shownMs` went on reporting `dragMs` — a bar frozen where a finger last
// touched it, over a clip that was still running. That is half of *"operator
// still see something different from whats on the output screens"*.
describe('the handle is given back when the finger leaves', () => {
  const BAR = readFileSync(resolve('src/lib/ClipBar.svelte'), 'utf8');

  it('releases on pointerup and on cancel, not only on change', () => {
    expect(BAR, 'a press that does not move the handle never lets go').toMatch(/on:pointerup=/);
    expect(BAR, 'a cancelled gesture keeps the bar').toMatch(/on:pointercancel=/);
  });

  it('and the release is the same one door, so it cannot drift', () => {
    // `endDrag` rather than three copies of `dragging = false`.
    expect(BAR).toMatch(/const endDrag = \(\) =>/);
  });
});

// ── AND THE BAR IS BLANK UNTIL A READING ABOUT THIS CLIP ARRIVES (RG-271) ───
describe('a new clip does not inherit the last one’s bar', () => {
  const SRC = readFileSync(resolve('src/lib/ClipBar.svelte'), 'utf8');

  it('the strip notices when the clip on the screens changed', () => {
    expect(SRC).toMatch(/clipChangedAt/);
    // THE URL, not the id: a bundled picture has no id, and comparing ids would
    // read as "unchanged" forever.
    expect(SRC).toMatch(/\$live\?\.media_url \?\? null/);
  });

  it('and refuses a reading older than that, through the one rule', () => {
    expect(SRC).toContain("from './clipbaseline.js'");
    expect(SRC).toMatch(/readingIsAboutThisClip\(seenAt, clipChangedAt\)/);
  });
});
