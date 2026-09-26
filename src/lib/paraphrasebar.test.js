// ── THE PARAPHRASE BAR · THE CHURCH'S SECOND SWITCH (DECISIONS §125, RG-311) ──
//
// The operator, 2026-09-25: *"The preacher paraphrases a lot so I want you to catch
// that and use the style to work on how the app respond."* Measured over eleven of
// their own services, 74% of what the paraphrase detector offers on speech naming no
// scripture is noise — and no threshold cuts it, because the noise and the real
// citations share a score distribution and invert at the tails. What does cut it is a
// contiguous run of the verse's own words, in order.
//
// **It costs recall on exactly the case the product's claim rests on** — a story
// retold in modern words shares no run — so it is the operator's trade and it
// defaults OFF. This file holds the console's half of that: the switch exists, it
// says what it costs, and it cannot show a position the engine does not hold.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { get } from 'svelte/store';
import { codeOnly } from './codeonly.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const { capture, setParaphraseNeedsRun } = await import('./stores/capture.js');

const ROOT = path.resolve(__dirname, '../..');
/** Comments are not the surface — this repository's own scanning rule. */
const SETTINGS = codeOnly(
  fs.readFileSync(path.join(ROOT, 'src/lib/views/Settings.svelte'), 'utf8'),
);

describe('the switch exists where the gate is explained', () => {
  const section = () => {
    const from = SETTINGS.indexOf('Detection sensitivity');
    const to = SETTINGS.indexOf('Voice profiles', from);
    expect(from).toBeGreaterThan(-1);
    expect(to).toBeGreaterThan(from);
    return SETTINGS.slice(from, to);
  };

  it('is rendered beside the dial and the reader switch, not on a screen of its own', () => {
    // All three answer one question — what the AI may do with what it thinks it
    // heard — and splitting them is how an operator comes to believe they are
    // unrelated. Same reasoning `followreader.test.js` records for its own switch.
    expect(section()).toMatch(/Paraphrase must echo the verse/);
    // THE HOUSE SWITCH, not a hand-rolled one (REBRAND §12, RG-168).
    expect(section()).toMatch(/<Switch[\s\S]{0,400}toggleParaphraseBar/);
    // …and the chain reaches the engine rather than a local flag. A switch wired to
    // a handler that sets nothing is the shape of the seven dead Settings controls
    // DECISIONS §69 removed.
    expect(SETTINGS).toMatch(
      /function toggleParaphraseBar\(\)[\s\S]{0,400}setParaphraseNeedsRun\(/,
    );
    // A FAILURE HAS TO BE READ, and in its OWN line: two switches sharing one error
    // paragraph means the second one's failure erases the first one's explanation.
    expect(SETTINGS).toMatch(/paraErr = humanError\(e\)/);
    expect(section()).toMatch(/\{#if paraErr\}/);
    expect(SETTINGS).toMatch(/let paraErr = ''/);
    expect(SETTINGS).toMatch(/let followErr = ''/);
    // And the WORD beside it comes from the same store the switch writes, so the
    // control cannot show a state the engine does not hold (rule 35).
    expect(section()).toMatch(/\$capture\.paraphraseNeedsRun \? 'on' : 'off'/);
  });

  // **THE SETTING MUST SAY WHAT IT COSTS.** This is not a wording nit: the bar
  // removes three quarters of the paraphrase list and, with it, the retellings in
  // modern words — the case the operator asked for by name. A switch that advertised
  // only "fewer wrong suggestions" would be selling a trade as an improvement, and
  // the person paying for it would not know they had.
  it('names the price in the note, in words rather than in percentages', () => {
    const note = section().match(/rw-nvnote">([^<]*)</g) ?? [];
    const mine = note.find((n) => /paraphrase/i.test(n));
    expect(mine, 'the paraphrase switch has no note at all').toBeTruthy();
    expect(mine).toMatch(/miss/i);
    expect(mine).toMatch(/modern/i);
    // NO NUMBERS. "73.6% of offers" is not something a volunteer can act on in a
    // dark booth; the measurements live in `detection::PARAPHRASE_RUN_WORDS`.
    expect(mine).not.toMatch(/\d+\s*%/);
  });

  // Rule 10 is untouched and the note says so, because an operator reading "fewer
  // wrong suggestions" could otherwise reasonably infer that the ones that survive
  // have been promoted.
  it('says plainly that it changes nothing about what reaches a screen', () => {
    const note = (section().match(/rw-nvnote">([^<]*)</g) ?? []).find((n) =>
      /paraphrase/i.test(n),
    );
    expect(note).toMatch(/never changes what reaches a screen/i);
  });
});

describe('setParaphraseNeedsRun', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('applies what the backend landed on, not what was asked for', async () => {
    invoke.mockImplementation(async (_cmd, args) => args.on);
    expect(await setParaphraseNeedsRun(true)).toBe(true);
    expect(get(capture).paraphraseNeedsRun).toBe(true);
    expect(await setParaphraseNeedsRun(false)).toBe(false);
    expect(get(capture).paraphraseNeedsRun).toBe(false);
    expect(invoke).toHaveBeenCalledWith('set_paraphrase_needs_a_run', { on: false });
  });

  // **A BACKEND THAT ANSWERS ANYTHING BUT `true` LEAVES THE BAR OFF**, and the
  // asymmetry is the point. `followsReader` defaults ON and reads `!== false`; this
  // defaults OFF and reads `=== true`, because in both cases the unknown answer has
  // to land on the behaviour the church is actually running. An older build with no
  // such command resolves `null`, and `null` must not read as "the noise has stopped".
  it('an answer that is not yes leaves it off', async () => {
    capture.update((s) => ({ ...s, paraphraseNeedsRun: false }));
    invoke.mockImplementation(async () => null);
    expect(await setParaphraseNeedsRun(true)).toBe(false);
    expect(get(capture).paraphraseNeedsRun).toBe(false);
  });

  // GROUP 1 — IT THROWS. A failure must reach the caller rather than leaving a
  // switch that looks like it moved (rule 15).
  it('throws rather than leaving the switch showing a position it did not reach', async () => {
    invoke.mockImplementation(async () => {
      throw new Error('the bridge is gone');
    });
    capture.update((s) => ({ ...s, paraphraseNeedsRun: true }));
    await expect(setParaphraseNeedsRun(false)).rejects.toThrow(/bridge/);
    expect(get(capture).paraphraseNeedsRun).toBe(true);
  });
});

describe('the console opens on what the engine is actually doing', () => {
  const CAPTURE = fs.readFileSync(path.join(ROOT, 'src/lib/stores/capture.js'), 'utf8');

  // The shipped default, in the store's own seed. A console that guessed ON would
  // print "on" over a detector running the firehose.
  it('the store seeds the bar OFF, which is what every install runs', () => {
    expect(get(capture).paraphraseNeedsRun).toBeTypeOf('boolean');
    expect(CAPTURE).toMatch(/paraphraseNeedsRun: false,/);
  });

  // ASKED AT LAUNCH, from the state the detection path reads — not from the row, and
  // not left to a listener. There is no event for this, so a console that only
  // listened would open showing `false` over an engine with the bar on.
  it('asks the backend at launch and defaults to OFF if the answer never comes', () => {
    expect(CAPTURE).toMatch(/call\('get_paraphrase_needs_a_run'\)\.catch\(\(\) => false\)/);
    expect(CAPTURE).toMatch(/paraphraseNeedsRun: needsRun === true,/);
  });
});
