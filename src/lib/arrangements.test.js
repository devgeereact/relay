// RG-21 / RG-22 — the running order, and the ground moving under it.
//
// Two claims, and only the second one can put wrong words on a wall:
//
//   RG-21  A person can build a named running order at all. Everything else in
//          the chain shipped months ago — the table, three commands, the store
//          wrapper, the plan-cue expander, the Planner's picker — and none of it
//          could ever run, because nothing could create the row it all read.
//
//   RG-22  An arrangement is a list of section INDICES. Fix a typo in verse two
//          and every arrangement still plays verse two — that is the whole reason
//          the schema stores indices rather than copied lyrics. But reorder,
//          insert, delete or rename a section and index 3 stops meaning what the
//          person who chose it meant, silently, on a Sunday.
//
// The Rust side of RG-22 is pinned in `db/mod.rs`
// (`a_lyric_edit_keeps_an_arrangement_and_a_structural_edit_flags_it` and
// `a_plan_cue_does_not_re_expand_through_a_drifted_arrangement`). This file pins
// what an OPERATOR is shown, which is the half a backend test cannot see.
//
//   npx vitest run src/lib/arrangements.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const Arrangements = (await import('./views/library/Arrangements.svelte')).default;
const { songCue } = await import('./cues.js');

const read = (p) => readFileSync(resolve(process.cwd(), p), 'utf8');

// Each mount is torn down. Leaving hosts in the body is not merely untidy: a
// second element with the same id in the document makes jsdom's `#id` selector
// return null inside the CURRENT host, and the test then fails as if the control
// were never rendered.
let host;
function mount(props) {
  host = document.createElement('div');
  document.body.appendChild(host);
  new Arrangements({ target: host, props });
  return host;
}

/**
 * Let a clicked handler finish.
 *
 * `tick()` alone is not enough: `capture.js` reaches the backend through a dynamic
 * `import('@tauri-apps/api/core')`, which resolves a turn later than Svelte's
 * scheduler — a test that only ticks sees zero calls and reads like a dead button.
 */
async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

const txt = () => host.textContent;
const typeName = async (v) => {
  const input = host.querySelector('[id="ar-name"]');
  input.value = v;
  input.dispatchEvent(new Event('input'));
  await settle();
};
const byText = (t) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === t);
const click = async (t) => {
  const b = byText(t);
  if (!b) throw new Error(`no button labelled "${t}" — found: ${
    [...host.querySelectorAll('button')].map((x) => x.textContent.trim()).join(' | ')}`);
  b.click();
  await settle();
};

const SONG = {
  id: 7,
  title: 'Great Are You Lord',
  sections: [
    { tag: 'V1', label: 'Verse 1', lyrics: 'you give life' },
    { tag: 'C', label: 'Chorus', lyrics: 'it is your breath' },
    { tag: 'V2', label: 'Verse 2', lyrics: 'you restore' },
  ],
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue([]);
});

afterEach(() => {
  host?.remove();
  host = null;
});

// ─────────────────────────────────────────────────────────────────────────────
// RG-21 · a person can build one
// ─────────────────────────────────────────────────────────────────────────────

describe('RG-21 · building a running order', () => {
  it('offers the song’s sections, and a repeat is a normal thing to want', async () => {
    mount({ song: SONG });
    await settle();
    await click('New arrangement');

    // Every section is offered, by the name the operator gave it.
    for (const tag of ['V1', 'C', 'V2']) expect(byText(tag)).toBeTruthy();

    // V1 C V2 C — the chorus twice, which is how the song is actually sung and
    // the reason a play order exists at all.
    await click('V1');
    await click('C');
    await click('V2');
    await click('C');
    expect(host.querySelectorAll('.ar-step').length).toBe(4);
  });

  it('refuses an unnamed order, and says so before the round trip', async () => {
    mount({ song: SONG });
    await settle();
    await click('New arrangement');
    await click('V1');
    await click('Save arrangement');

    expect(invoke).not.toHaveBeenCalledWith('save_arrangement', expect.anything());
    expect(txt()).toMatch(/Give the arrangement a name/);
  });

  it('refuses an empty order rather than saving a play order with nothing in it', async () => {
    mount({ song: SONG });
    await settle();
    await click('New arrangement');
    await typeName('Sunday');
    await click('Save arrangement');

    expect(invoke).not.toHaveBeenCalledWith('save_arrangement', expect.anything());
    expect(txt()).toMatch(/needs at least one section/);
  });

  it('saves the sequence as indices, with the repeat intact', async () => {
    mount({ song: SONG });
    await settle();
    await click('New arrangement');
    await typeName('Sunday');
    await click('V1');
    await click('C');
    await click('V2');
    await click('C');
    await click('Save arrangement');

    const call = invoke.mock.calls.find(([c]) => c === 'save_arrangement');
    expect(call).toBeTruthy();
    expect(call[1]).toMatchObject({ songId: 7, id: null, name: 'Sunday', sequence: [0, 1, 2, 1] });
  });

  it('a delete is armed, not immediate — and a refusal is a sentence, not a Rust string', async () => {
    invoke.mockResolvedValue([{ id: 3, name: 'Short', sequence: [0, 1], stale: false }]);
    mount({ song: SONG });
    await settle();
    expect(txt()).toMatch(/Short/);

    await click('Delete');
    expect(invoke).not.toHaveBeenCalledWith('delete_arrangement', expect.anything());

    // Second press. Service Lock refuses it — that refusal is worth reading.
    invoke.mockRejectedValueOnce({ kind: 'refused', message: 'a service is recording' });
    await click('Delete — sure?');
    expect(invoke).toHaveBeenCalledWith('delete_arrangement', { id: 3 });
    expect(txt()).toMatch(/recording/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// RG-22 · the ground moving under it
// ─────────────────────────────────────────────────────────────────────────────

describe('RG-22 · an order whose sections moved', () => {
  it('is named as needing checking, and says why in words', async () => {
    invoke.mockResolvedValue([{ id: 3, name: 'Short', sequence: [0, 1], stale: true }]);
    mount({ song: SONG });
    await settle();
    expect(txt()).toMatch(/NEEDS CHECKING/);
    expect(txt()).toMatch(/sections changed since this was built/);
  });

  it('keeps what the operator chose instead of rewriting it', async () => {
    // The repair is a person looking at both, not Relay guessing which section
    // was meant — so the stored order is what opens for editing, unchanged.
    invoke.mockResolvedValue([{ id: 3, name: 'Short', sequence: [0, 2], stale: true }]);
    mount({ song: SONG });
    await settle();
    await click('Edit');
    const steps = [...host.querySelectorAll('.ar-steptag')].map((n) => n.textContent.trim());
    expect(steps).toEqual(['V1', 'V2']);
  });

  it('names a step whose section is gone entirely, rather than printing “?”', async () => {
    invoke.mockResolvedValue([{ id: 3, name: 'Short', sequence: [0, 9], stale: true }]);
    mount({ song: SONG });
    await settle();
    await click('Edit');
    expect(txt()).toMatch(/section that no longer exists/);
  });

  it('is rose, never amber — amber means ON AIR and is never allowed to lie', () => {
    const f = read('src/lib/views/library/Arrangements.svelte');
    expect(f).toMatch(/--r-rose/);
    expect(f).not.toMatch(/--r-amber|#f5a524/i);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// The doors this guarantee has to hold on
// ─────────────────────────────────────────────────────────────────────────────

describe('RG-22 · every door, not just the editor', () => {
  it('a plan cue records the structure it was built against', () => {
    const arr = { id: 3, name: 'Short', sequence: [0, 1], built_shape: '[["V1","Verse 1"]]' };
    const { payload } = songCue(SONG, arr);
    expect(payload.arrangement_seq).toEqual([0, 1]);
    expect(payload.arrangement_shape).toBe('[["V1","Verse 1"]]');
    // Without this the backend cannot tell a drifted cue from a fresh one, and
    // `sync_song_in_plans` re-expands through indices that moved.
    expect(payload.arrangement_stale).toBe(false);
  });

  it('Standard is still the absence of an arrangement, and carries no shape', () => {
    const { payload } = songCue(SONG, null);
    expect(payload.arrangement_name).toBe('Standard');
    expect(payload.arrangement_seq).toBeNull();
    expect(payload.arrangement_shape).toBeNull();
  });

  it('the Planner offers a stale arrangement but will not let it into a plan', () => {
    const f = read('src/lib/views/ServicePlanner.svelte');
    // Offered — hiding it would leave an operator hunting for one they know they
    // made. Disabled — adding it would put the wrong words in the plan.
    expect(f).toMatch(/disabled=\{a\.stale\}/);
    expect(f).toMatch(/sections changed since this was built/);
  });

  it('the Planner’s cue badge does not claim an arrangement that was dropped', () => {
    const f = read('src/lib/views/ServicePlanner.svelte');
    expect(f).toMatch(/arrangement_stale/);
    expect(f).toMatch(/PLAYING IN THE SONG’S OWN ORDER/);
  });
});
