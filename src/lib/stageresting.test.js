// AT REST, THE TIMER OWNS THE SCREEN (RG-244).
//
// The last half of *"rebuild the Mobile template for the Pastor's screen
// properly... Timer is [the main thing]"*, and the state the drawing shows: a
// phone with nothing fired, the clock filling the room the reading is not using.
//
// RG-243 made the wall clock secondary to the countdown WITHIN the figure row.
// This is the row itself. Today it takes 15% of the screen and gives the rest to
// a reading area holding "— standby —", except in one case: a running COUNTDOWN
// takes 58%. So a church using the Stage Timer — the Stage Timer, the one a
// preacher is actually working to — got a large empty region above a small
// figure, for the whole of a sermon.
//
// **A reading still wins.** The moment anything is fired the room goes back,
// because a reading is what the screen is for; this is about the room nobody
// else is using.
import { describe, it, expect } from 'vitest';
import { restingLayout } from './stageresting.js';

const at = (over = {}) =>
  restingLayout({ reading: false, slide: false, countdown: false, programme: false, ...over });

describe('restingLayout — who takes the room nobody else is using', () => {
  it('a running Stage Timer takes it, which is the case that was missing', () => {
    expect(at({ programme: true })).toBe('programme');
  });

  it('a countdown takes it, as it always did', () => {
    expect(at({ countdown: true })).toBe('figures');
  });

  it('a countdown wins over the Stage Timer — it is the one with a deadline', () => {
    // Both are clocks and only one of them is about a moment the whole room is
    // waiting for. A pre-service countdown is the reason anybody is looking at
    // this page before a service starts.
    expect(at({ countdown: true, programme: true })).toBe('figures');
  });

  it('a reading takes it back, whatever is running', () => {
    for (const over of [{ countdown: true }, { programme: true }, { countdown: true, programme: true }])
      expect(restingLayout({ ...over, reading: true, slide: false }), JSON.stringify(over)).toBe('reading');
  });

  it('and so does a slide — the operator put that there to be looked at', () => {
    expect(at({ slide: true, programme: true })).toBe('reading');
  });

  it('with nothing running at all, nothing claims it', () => {
    // "— standby —" is a real state and it is the reading area's. A clock that
    // is not running must not take a screen's whole height to say nothing.
    expect(at()).toBe('reading');
  });
});
