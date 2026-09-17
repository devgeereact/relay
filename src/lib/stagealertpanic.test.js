// WHAT `Esc` AND `B` DO TO A STAGE MESSAGE — on BOTH stage surfaces.
//
// ── This file used to assert the opposite, and that is the point of it ────────
//
// `Stage.svelte`'s `clear`/`black` branch resets six fields and leaves one
// standing, `svcStart`, which says at the line why it survives: a cleared wall is
// not the end of a service and the elapsed zone is the preacher's own clock. For
// the whole life of the page it left a second one standing too, `alert`, and
// nothing recorded why — a silent third answer to a question nobody had asked out
// loud, indistinguishable from a field somebody forgot.
//
// Two waves answered it in the same week and answered it OPPOSITE ways. Wave 5's
// DECISIONS §89 ratified the survivor; wave 3's DECISIONS §91 cleared it. **The
// operator ruled for §91: the alert comes down with the screens.** The reason is
// that `.alert` is `position: fixed; inset: 0` — the page's own comment says "this
// panel IS the screen" — so a survivor meant an operator pressed `B`, whose whole
// meaning is *every output goes opaque black*, and the preacher's tablet stayed a
// full-bleed pulsing red panel: the brightest thing in the room, under a control
// the console had just reported succeeding. Clearing it also makes the live path
// agree with the reconnect path, because `stage_alert` is never a retained frame
// (rule 43, `FRAME_VERDICTS`), so a tablet that reloaded came back with no alert
// while the one beside it kept the panel.
//
// This file is not a duplicate of `stagepanic.test.js`, which holds §91's own two
// halves against `Stage.svelte` — the alert going and the clocks staying. **This
// one holds the claim the MERGE created and neither wave tested: that the two
// stage surfaces now agree about a panic control.** `Stage.svelte` is the
// preacher's phone; `output.html` with a `stage`-role channel and a
// `stage_message` layer is the other supported route to the same screen
// (DECISIONS §89), and before the ruling they behaved differently.
//
// ── What is deliberately NOT pinned here ─────────────────────────────────────
//
// They agree about the panic; they still disagree about what happens next.
// `Output.svelte` keeps the text in renderer state (`stageMessage`) and no
// `clear`/`black` branch resets it — the layer stack merely goes away with
// `content` — so the next verse fired paints the message again, unbidden.
// `Stage.svelte` needs a new Stage Message to say it again. That is **RG-156**,
// and it is left unpinned on purpose: a test asserting the reappearance would have
// to be deleted by whoever fixes it, which is how a register row becomes a
// requirement.
//
// ── Verified by reintroducing both defects ───────────────────────────────────
//
// Restoring the survivor (deleting `alert = ''` from `Stage.svelte`'s branch) fails
// the two `Stage.svelte` cases — the behaviour exactly as it shipped, and exactly
// what this file used to demand. Making `Output.svelte` pass `content={content}`
// instead of `content={visible ? content : null}` fails the `output.html` `clear`
// case, which is that page's whole mechanism for taking the message off. Deleting
// the `{#if black && !isBand}` overlay fails the `black` case. The note case fails
// if `note = ''` is removed, so the file cannot pass by asserting nothing.
//
//   npx vitest run src/lib/stagealertpanic.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

// The kiosk path for BOTH pages: the Tauri bridge must be absent, exactly as it is
// in a browser source, so `onMount`'s import throws and the WS client runs.
vi.mock('@tauri-apps/api/core', () => {
  throw new Error('no tauri bridge in a browser source');
});
vi.mock('@tauri-apps/api/event', () => {
  throw new Error('no tauri bridge in a browser source');
});

const Stage = (await import('../Stage.svelte')).default;
const Output = (await import('../Output.svelte')).default;

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

const MESSAGE = 'Wrap up — 5 minutes';
const NOTE = 'hold for prayer';
const VERSE_TEXT = 'For God so loved the world';

/** A stage-shaped template for `output.html`: the verse, and a Stage Message layer. */
const STAGE_TPL = {
  id: 7,
  name: 'Stage',
  layout: {
    layers: [
      { id: 'bg', type: 'background', fill: '#000', x: 0, y: 0, w: 100, h: 100 },
      { id: 'v', type: 'text', bind: 'verse', x: 5, y: 10, w: 90, h: 40, size: 4, color: '#fff' },
      {
        id: 'msg',
        type: 'text',
        bind: 'stage_message',
        x: 5,
        y: 60,
        w: 90,
        h: 20,
        size: 5,
        color: '#ff5555',
      },
    ],
  },
  style: { background: '#000' },
};

let host;
let app;

async function settle() {
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
}

function send(frame) {
  socket.onmessage({ data: JSON.stringify(frame) });
}

const painted = () => host.textContent;

beforeEach(() => {
  socket = null;
  globalThis.WebSocket = FakeSocket;
  try {
    localStorage.removeItem('relay.stage.zones');
  } catch {
    /* the shim in test-setup.js hands us a real Storage */
  }
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

/**
 * The preacher's phone, mid-service: a verse, its private note, and the message.
 *
 * ON CHANNEL 2, with the role map delivered first, for the same reason the
 * `output.html` fixture below carries both: `stage.html` now refuses a Stage
 * Message unless its own channel holds the `stage` role
 * (`stagepageidentity.test.js`). The two fixtures in this file therefore differ
 * only in which page they mount, which is what makes the comparison below —
 * "the two stage surfaces agree about a panic control" — a real one.
 */
async function thePreachersPhone() {
  window.history.replaceState({}, '', '/stage.html?channel=2');
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  send({ kind: 'channel_roles', roles: { 1: 'main', 2: 'stage' } });
  await tick();
  send({
    kind: 'content',
    reference: 'John 3:16',
    text: VERSE_TEXT,
    stage_note: NOTE,
  });
  send({ kind: 'stage_alert', text: MESSAGE });
  await tick();
  expect(painted(), 'the fixture never got the message onto the page').toContain(MESSAGE);
  expect(painted()).toContain(NOTE);
}

/** `output.html` on channel 2, which the fresh install names the stage. */
async function aStageDisplayServedByOutputHtml() {
  window.history.replaceState({}, '', '/output.html?channel=2&template_id=7');
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Output({ target: host });
  await settle();
  send({ kind: 'template', id: 7, template: STAGE_TPL });
  send({ kind: 'channel_roles', roles: { 1: 'main', 2: 'stage' } });
  // The roles have to LAND before the alert arrives: `stage_alert` is refused
  // synchronously against `myRole`, which is reactive, so a message sent in the
  // same batch as the role map is refused by a page that is about to be a stage.
  await settle();
  send({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: VERSE_TEXT });
  send({ kind: 'stage_alert', text: MESSAGE });
  await settle();
  expect(painted(), 'the fixture never got the message onto the page').toContain(MESSAGE);
}

describe('DECISIONS §91 · a Stage Message comes down with the screens', () => {
  for (const kind of ['clear', 'black']) {
    // Both controls, named separately. The branch handles the two kinds together,
    // so they can only differ if somebody splits it — and the branch's own comment
    // says that split must never happen by accident: "the harsher control must
    // never do less than the milder one."
    it(`\`${kind}\` takes the Stage Message off the preacher's phone`, async () => {
      await thePreachersPhone();
      send({ kind });
      await tick();
      expect(
        painted(),
        'the alert is the whole screen — a panic control that leaves it has not blanked anything',
      ).not.toContain(MESSAGE);
    });
  }

  it('takes the Stage Note and the reading with it, so the ruling is not "stopped clearing things"', async () => {
    // The distinction the reversed ruling still rests on: a Stage Note describes
    // the slide and a cleared slide makes it wrong. If the Stage Message had
    // survived while the Stage Note went, the difference would have been the
    // ruling; now that both go, this case is what stops the file passing by
    // asserting nothing.
    await thePreachersPhone();
    send({ kind: 'clear' });
    await tick();
    expect(painted()).not.toContain(NOTE);
    expect(painted()).not.toContain(VERSE_TEXT);
  });

  it('and the operator can say it again in one action afterwards', async () => {
    // What makes taking it down defensible: the way back is the control that is
    // already there. A panic that also disabled the message would be a different
    // and much worse decision.
    await thePreachersPhone();
    send({ kind: 'black' });
    await tick();
    expect(painted()).not.toContain(MESSAGE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await tick();
    expect(painted()).toContain(MESSAGE);
  });
});

describe('the two stage surfaces agree about a panic control (the merge claim)', () => {
  it('`clear` takes it off a stage display served by output.html too', async () => {
    // Before the ruling these two pages disagreed: this one blanked the message
    // with the layer stack and the phone kept it. Now both take it off the glass,
    // which is what a church with one of each is entitled to assume.
    await aStageDisplayServedByOutputHtml();
    send({ kind: 'clear' });
    await settle();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('`black` covers it on output.html, by a different mechanism, and the difference is stated', async () => {
    // MEASURED, NOT ASSUMED — the first version of this case asserted the same
    // thing as the one above and went red.
    //
    // `Output.svelte` answers the two controls differently. `clear` sets
    // `visible = false`, so `content` becomes null and `TemplateRender`'s whole
    // layer stack — the Stage Message layer with it — leaves the DOM. `black`
    // leaves `visible` alone and paints `.blackout`: `position: fixed; inset: 0;
    // background: #000; z-index: 9999`. Nothing is visible either way, which is
    // the guarantee that matters, but the text is still THERE underneath the
    // cover, so a `.not.toContain` is the wrong instrument for this control and
    // would be asserting the wrong thing if it happened to pass.
    //
    // That difference belongs to RG-156's family rather than to this claim: the
    // message on this page is never reset, only hidden, whichever control hid it.
    await aStageDisplayServedByOutputHtml();
    expect(host.querySelector('.blackout'), 'the fixture started blacked out').toBeNull();
    send({ kind: 'black' });
    await settle();
    // What this test can and cannot reach: jsdom does not apply the component's
    // scoped stylesheet, so `getComputedStyle` reads `static` for an element the
    // browser paints `position: fixed; inset: 0; background: #000; z-index: 9999`.
    // The element's PRESENCE is the falsifiable half and is what is asserted; the
    // rule that makes it opaque lives in `Output.svelte`'s own `<style>` and was
    // read rather than measured here.
    expect(
      host.querySelector('.blackout'),
      '`black` painted no blackout at all on a stage display',
    ).toBeTruthy();
  });
});
