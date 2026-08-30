// THE PATH CHECK — say one verse, and find out whether the whole chain works.
//
// Single responsibility: watch the six stages between a microphone and a screen,
// and say which of them were reached.
//
// ── Why the launch checks are not enough ──────────────────────────────────────
//
// `boot/probes.js` asks twenty-one good questions and every one of them is about a
// PART: is there a microphone, is a model loaded, is the database there, is a window
// open. All twenty-one can pass on a machine where nothing works end to end — a
// microphone the operating system has muted, a model that loads and mishears
// everything, a gate calibrated to a room that has since filled with people, an
// output window on a display that is asleep.
//
// A church discovers that at 10:31. This is the thing that finds it at 10:05.
//
// ── The six stages, and why each is separate ─────────────────────────────────
//
// Each one is observed from a DIFFERENT event, and a stage that is not reached is
// reported as itself rather than blamed on the next one. "No transcript" and "no
// detection" have completely different fixes, and a check that collapsed them into
// "it didn't work" would send an operator to the wrong one.
//
// ── It runs in rehearsal, or it does not run ─────────────────────────────────
//
// The whole point is to fire a real verse through the real pipeline, and the whole
// danger is that this happens twenty minutes before a service with a congregation
// arriving. Rehearsal (DECISIONS §18) sandboxes the last hop at the broadcast, so
// the console sees the verse and no screen does. **If rehearsal cannot be turned on,
// the walk is abandoned** — running it live would be exactly the accident it exists
// to prevent.

/** The stages, in the order a spoken word passes through them. */
export const STAGES = [
  { id: 'microphone', label: 'Microphone opened' },
  { id: 'audio', label: 'Relay heard a voice' },
  { id: 'transcript', label: 'It turned that into words' },
  { id: 'detection', label: 'It recognised a reference' },
  { id: 'fire', label: 'The gate allowed it' },
  { id: 'output', label: 'It reached a screen' },
];

/** What the operator is asked to say. Short, unambiguous, and famous. */
export const PHRASE = 'John chapter three, verse sixteen';
export const EXPECT = 'John 3:16';

/** How long to listen before giving up. */
export const WALK_TIMEOUT_MS = 25_000;

export function newWalk() {
  return {
    started: false,
    // `reached[stage] = ms since start`. Absent means NOT REACHED — never 0, which
    // would read as "instantly".
    reached: {},
    /** What it actually detected, if anything. May not be what was asked for. */
    heard: null,
    detected: null,
    fired: null,
    error: null,
  };
}

const mark = (w, id, at) => {
  // FIRST sighting only. A stage that keeps happening (audio chunks arrive many
  // times a second) must report when it first worked, not when it last did.
  if (w.reached[id] === undefined) w.reached[id] = at;
};

export function onStarted(w, at = 0) {
  return { ...w, started: true, reached: { ...w.reached, microphone: at } };
}

/**
 * An audio chunk. Only a VOICED one counts.
 *
 * A level meter moving proves the microphone is connected; it does not prove Relay
 * can hear a person, and the difference between those two is the entire finding of
 * DECISIONS §19 — the gate is learned, and a room can be loud while no speech ever
 * opens it.
 */
export function onAudio(w, chunk, at) {
  if (!chunk?.isVoice) return w;
  const next = { ...w, reached: { ...w.reached } };
  mark(next, 'audio', at);
  return next;
}

export function onTranscript(w, t, at) {
  if (!t?.text || !t.text.trim()) return w;
  const next = { ...w, reached: { ...w.reached }, heard: t.text.trim() };
  mark(next, 'transcript', at);
  return next;
}

/**
 * A detection reached the console.
 *
 * ANY detection proves the stage. Whether it was the RIGHT verse is a separate
 * question with a separate answer, because "Relay recognised something" and "Relay
 * recognised what you said" fail for different reasons and are fixed differently —
 * one is a dead pipeline, the other is a mishearing.
 */
export function onDetection(w, d, at) {
  if (!d?.reference) return w;
  const next = { ...w, reached: { ...w.reached }, detected: d.reference };
  mark(next, 'detection', at);
  // The gate is a separate stage: a Direct hit that auto-fires passed it; a
  // suggestion did not, and that is a correct outcome to report rather than a
  // failure.
  if (d.status === 'auto' || d.status === 'manual') mark(next, 'fire', at);
  return next;
}

export function onOutput(w, content, at) {
  const next = { ...w, reached: { ...w.reached }, fired: content?.reference ?? null };
  mark(next, 'output', at);
  // Content reaching an output proves the gate allowed it, even if the detection
  // event arrived out of order.
  mark(next, 'fire', at);
  return next;
}

export function onError(w, message) {
  return { ...w, error: message };
}

/** Every stage reached, in order, plus the first one that was not. */
export function progress(w) {
  const rows = STAGES.map((s) => ({
    ...s,
    at: w.reached[s.id],
    // An ABSENCE, not a failure: a stage after the one that broke was never given
    // the chance, and calling it "failed" would send somebody debugging the wrong end.
    state: w.reached[s.id] !== undefined ? 'ok' : 'not reached',
  }));
  const firstMissing = rows.find((r) => r.state === 'not reached') ?? null;
  return { rows, firstMissing };
}

/** Is the walk finished — everything reached, or time up? */
export function isComplete(w) {
  return STAGES.every((s) => w.reached[s.id] !== undefined);
}

/**
 * One sentence, naming the FIRST stage that did not happen and what to do.
 *
 * Only the first: a check that lists five failures when one thing is broken has
 * told the operator nothing, because four of them are consequences.
 */
export function verdict(w, timedOut = false) {
  if (w.error) return { ok: false, sentence: w.error };
  if (isComplete(w)) {
    const right = w.detected === EXPECT || w.fired === EXPECT;
    return {
      ok: true,
      // The whole chain worked, and whether it heard the RIGHT verse is said
      // separately — a working pipeline that misheard is a different situation from
      // a broken one, and both are worth knowing before a service.
      sentence: right
        ? 'The whole path works: your voice reached a screen, and it got the right verse.'
        : `The whole path works — though it heard “${w.detected ?? w.fired}”, not ${EXPECT}. ` +
          'Try again, a little slower; if it keeps mishearing, the speech model or the room is the thing to look at.',
    };
  }
  if (!timedOut) return { ok: null, sentence: '' };

  const { firstMissing } = progress(w);
  const FIX = {
    microphone: 'Relay could not open the microphone. Check Settings → Audio.',
    audio:
      'Relay never heard a voice. Check the microphone is not muted, and that the level meter moves when you speak — Relay listens for speech, not for noise.',
    transcript:
      'Relay heard you but produced no words. That is the speech model: check one is loaded in Settings → Network.',
    detection: `Relay wrote down what you said but did not recognise a reference. What it heard was “${w.heard ?? ''}”. Try saying it as “${PHRASE}”.`,
    fire: `Relay recognised ${w.detected ?? 'a reference'} but did not put it up. That is usually correct — a paraphrase or an uncertain match is only ever offered, never fired. Try saying the reference directly.`,
    output:
      'Everything worked up to the last step, and nothing reached a screen. Check a screen is set up and open in the Outputs tab.',
  };
  return {
    ok: false,
    sentence: FIX[firstMissing?.id] ?? 'The check did not finish.',
  };
}
