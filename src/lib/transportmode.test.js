// The key an operator presses more than any other, and what it does next.
//
// Reported by the operator on 2026-09-20: *"the Next button should work with
// next verse, and next slide if not a verse on the screen, and allow the
// keyboard control work accurately."* Two defects sat behind that sentence and
// neither had an instrument, because the rule was one reactive line inside
// `Live.svelte` that could only be reached by mounting the whole run surface.
import { describe, it, expect } from 'vitest';
import { transportMode, verseIsOnAir, fallsThroughToPlan, SCRIPTURE } from './transportmode.js';

const verse = { kind: SCRIPTURE, reference: 'John 3:16' };
const song = { kind: 'song' };
const notice = { kind: 'announce' };
const plan = (n) => ({ live: null, screenBlack: false, planOnAir: false, planLength: n });

describe('what is actually on the wall', () => {
  it('a verse a person put up is a verse to walk', () => {
    expect(verseIsOnAir(verse, false, false)).toBe(true);
  });

  /// THE DEFECT. `live` means CONTENT IS ARMED, of any kind, and rule 38 has
  /// `ContextMemory::forget` clear the passage for every non-scripture kind. So
  /// over a song the bar said VERSE, the operator pressed Next, the engine
  /// answered NoPassage, and nothing moved.
  it('a song or a notice is NOT a verse, however armed it is', () => {
    expect(verseIsOnAir(song, false, false)).toBe(false);
    expect(verseIsOnAir(notice, false, false)).toBe(false);
    expect(verseIsOnAir({ kind: 'countdown' }, false, false)).toBe(false);
    expect(verseIsOnAir({ kind: 'media' }, false, false)).toBe(false);
    // A fire with no kind at all is not a verse either. Absence is not scripture.
    expect(verseIsOnAir({}, false, false)).toBe(false);
  });

  it('a blackout is not a verse in front of people', () => {
    // `$live` survives `B`. Reading VERSE after a blackout and SLIDE after Esc
    // meant one state drove two transports, and the next press undid the
    // emergency key.
    expect(verseIsOnAir(verse, true, false)).toBe(false);
  });

  it('a scripture slide the PLAN put up belongs to the plan', () => {
    expect(verseIsOnAir(verse, false, true)).toBe(false);
  });

  it('an empty wall is not a verse', () => {
    expect(verseIsOnAir(null, false, false)).toBe(false);
  });
});

describe('which mode the bar must say', () => {
  it('a verse on the wall takes the key, plan or no plan', () => {
    expect(transportMode({ ...plan(12), live: verse })).toBe('verse');
    expect(transportMode({ ...plan(0), live: verse })).toBe('verse');
  });

  /// THE OTHER DEFECT: the rule began `openPlan && items.length && …`, so with
  /// nothing on the wall and a plan loaded it still read VERSE.
  it('an empty wall with a plan loaded steps the PLAN', () => {
    expect(transportMode(plan(12))).toBe('slide');
  });

  it('a song on the wall with a plan loaded steps the PLAN', () => {
    // The case the operator hit. Nothing to walk, so the key goes to the plan
    // rather than to a refusal.
    expect(transportMode({ ...plan(12), live: song })).toBe('slide');
  });

  it('with no plan there is nothing else for the key to do', () => {
    // It stays VERSE rather than claiming a plan that is not loaded: `nav` then
    // says "no passage" in words, which is a true sentence, where a bar reading
    // SLIDE over no plan would be a false one.
    expect(transportMode(plan(0))).toBe('verse');
    expect(transportMode({ ...plan(0), live: song })).toBe('verse');
  });

  it('a blackout mid-plan hands the key back to the plan', () => {
    expect(transportMode({ ...plan(12), live: verse, screenBlack: true })).toBe('slide');
  });
});

describe('falling through to the plan when there was no passage after all', () => {
  it('carries on into the plan when the engine has nothing to walk', () => {
    // A passage forgotten under unrelated content (rule 38), or a verse fired
    // before a restart. The operator pressed the transport; something should
    // move.
    expect(fallsThroughToPlan({ kind: 'no_passage' }, 12)).toBe(true);
    expect(fallsThroughToPlan({ kind: 'not_in_library', reference: 'Jude 30' }, 12)).toBe(true);
  });

  /// THE OPERATOR'S OWN DECISION, asked and answered on 2026-09-20.
  ///
  /// Reaching the last verse of a reading is a correct boundary, not a failure.
  /// Carrying on into the plan would put a slide in front of a congregation on a
  /// press that was meant to do nothing — and the ends of a plan are already
  /// hard stops that never wrap, so this is the same rule facing the other way.
  it('STOPS at the end of a passage and never walks into the plan', () => {
    expect(fallsThroughToPlan({ kind: 'end_of_passage' }, 12)).toBe(false);
  });

  it('does not fall through on a press that worked', () => {
    expect(fallsThroughToPlan({ kind: 'fired' }, 12)).toBe(false);
  });

  it('has nowhere to fall through to without a plan', () => {
    expect(fallsThroughToPlan({ kind: 'no_passage' }, 0)).toBe(false);
    expect(fallsThroughToPlan({ kind: 'no_passage' }, undefined)).toBe(false);
  });

  it('treats a missing answer as no fall-through', () => {
    // A rejected command is handled by its own catch; an absent outcome must not
    // quietly fire a plan slide.
    expect(fallsThroughToPlan(null, 12)).toBe(false);
    expect(fallsThroughToPlan(undefined, 12)).toBe(false);
  });
});

describe('the run surface uses this module and holds no second copy', () => {
  it('Live imports the rule rather than restating it', async () => {
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const src = readFileSync(resolve(process.cwd(), 'src/lib/views/Live.svelte'), 'utf8');
    const code = src
      .replace(/<!--[\s\S]*?-->/g, '')
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .replace(/^[ \t]*\/\/.*$/gm, '');
    expect(code).toContain("from '../transportmode.js'");
    // Two surfaces answering "which mode is the transport in" is how they come
    // to disagree — the same argument `outputHealth.js` and `countdown.js` are
    // already here for.
    expect(code).not.toMatch(/\$: mode =.*\?\s*'slide'\s*:\s*'verse'/);
    expect(code).not.toContain("=== 'no_passage'");
  });
});

// P-1's matcher. The `output://content` listener leaves the plan for scripture it
// was not told to expect; a plan fire announces its reference first. Only
// scripture can arrive unattended (the AI, the phone, spoken nav), so only
// scripture is judged — a song or a picture only ever comes from a wrapper that
// already knows whether it was a plan fire.
import { contentIsExpectedPlanFire } from './transportmode.js';

describe('contentIsExpectedPlanFire', () => {
  it('matches the plan scripture it was told about, loosely spelled', () => {
    expect(contentIsExpectedPlanFire({ kind: 'scripture', reference: 'Psalms 23:5' }, { reference: 'psalms  23:5' })).toBe(true);
  });
  it('does not match a different verse', () => {
    expect(contentIsExpectedPlanFire({ kind: 'scripture', reference: 'Hosea 6:1' }, { reference: 'Psalms 23:5' })).toBe(false);
  });
  it('nothing expected means nothing matches', () => {
    expect(contentIsExpectedPlanFire({ kind: 'scripture', reference: 'Hosea 6:1' }, null)).toBe(false);
  });
  it('non-scripture content is never judged here', () => {
    expect(contentIsExpectedPlanFire({ kind: 'song', text: 'x' }, null)).toBe(true);
    expect(contentIsExpectedPlanFire({ kind: 'media', media_url: 'u' }, null)).toBe(true);
  });
  it('a payload with no kind is scripture (the older shape)', () => {
    expect(contentIsExpectedPlanFire({ reference: 'John 3:16' }, null)).toBe(false);
  });
});
