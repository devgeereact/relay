// OUTPUT HEALTH — a screen reporting that it is still there, and what the console
// is allowed to say about the answer.
//
// ── The bug this exists to end ─────────────────────────────────────────────────
//
// Relay used to work out whether a screen was alive by asking itself: is the app
// still holding a window object, and is it still serving the URL? Both are true of
// a projector showing a frozen renderer, a browser source whose tab has been
// killed, and a display that went to sleep. So the console could read **On Air**
// over a screen showing nothing, and the operator's one glance-up check was a
// check that could not fail.
//
// The fix is not a cleverer inference. It is that the screen answers for itself,
// and stops answering when it stops.
//
// ── Rules, all of which are the same rule ──────────────────────────────────────
//
// 1. **NEVER THROW, EVER.** This ticks for the whole length of a service on the
//    page that is on the wall. A rejected promise here must not become an
//    unhandled rejection in the middle of a sermon. A health signal that can break
//    the thing it watches is worse than no health signal.
// 2. **Silence is the message.** When anything goes wrong — no bridge, a socket
//    mid-reconnect, a dead command — the correct behaviour is to send NOTHING and
//    let the beat go stale. Every failure mode has to fail towards "this screen is
//    not answering", never towards "all is well".
// 3. **State, never text.** The payload is one of three words. A kiosk beat
//    crosses an unauthenticated LAN (DECISIONS §35) and lands in the operator's
//    status pane; a free-text field there would be an injection surface into the
//    one UI that must never lie. Rust parses it against a closed enum and drops
//    anything else. The two numbers below obey the same rule: integers, clamped
//    at the door, never text.
//
// ── What the beat says about its own silence (RG-119) ─────────────────────────
//
// A beat that never arrives tells Relay nothing about WHY, and the two reasons
// want opposite fixes: the screen stopped painting, or Relay lost a heartbeat the
// screen did send. A service on 2026-09-06 lost the main output three times for
// 19.3 minutes in total and the record could not say which. Only the page knows,
// so it says, on the beat that ends its silence:
//
// * `since_ms` — its own clock since the previous tick. About one interval means
//   the page kept ticking and the beats were lost on the way. Minutes mean the
//   page was not running: the OS suspended or throttled it.
// * `hidden_ms` — how much of that was spent `document.hidden`. On macOS a window
//   covered by another window is hidden, which is the leading suspicion for that
//   service and is not the same fault as a frozen renderer.
//
// Both are omitted rather than sent as 0 when the page cannot know (the first beat
// of its life has no previous tick). Absent means "did not say"; zero would mean
// "said it never went quiet", and only one of those is true.
//
// The interval is Rust's `channels::BEAT_INTERVAL_MS`, and the staleness window it
// has to stay under is `channels::BEAT_STALE_MS`. They are coupled — three beats
// of grace — and `r6-contracts.test.js` fails if this file and that one drift.

/** How often a screen reports in. Must match `channels::BEAT_INTERVAL_MS`. */
export const BEAT_INTERVAL_MS = 2000;

/**
 * A clock for measuring a gap. `performance.now()` where it exists, because it is
 * monotonic and a wall clock that steps (a laptop waking, an NTP correction) would
 * report a silence that never happened into the one record RG-119 is trying to
 * make trustworthy.
 */
const now = () =>
  typeof performance !== 'undefined' && typeof performance.now === 'function'
    ? performance.now()
    : Date.now();

/** The three things a screen can be showing. Must match `channels::PaintState`. */
export const PAINT_STATES = ['content', 'clear', 'black'];

/**
 * What a page is showing right now, as the closed enum Rust expects.
 *
 * Blackout wins over content: a blacked-out screen showing a stale verse
 * underneath is black to the congregation, and reporting `content` would describe
 * the DOM rather than the room.
 */
export function paintState({ black, visible, content }) {
  if (black) return 'black';
  return visible && content ? 'content' : 'clear';
}

/**
 * Start reporting. Returns a stop function; call it on destroy.
 *
 * `getState` is called at each tick rather than captured, so the beat always
 * describes the screen as it is now and never as it was when the timer started.
 * `getWs` is likewise a getter: a kiosk socket is replaced on every reconnect, and
 * a captured reference would keep beating down a dead one.
 */
export function startBeat({
  channelId,
  getState,
  getWs = () => null,
  invoke = null,
  // WHERE THIS SCREEN'S CLIP IS, or `null` when it is not playing one.
  //
  // It rides the beat rather than a channel of its own, because the beat is
  // already the one thing a screen says about itself and it already carries the
  // answer to "are you still painting". A clip's position is worth nothing
  // without that: a position from a screen that stopped answering a minute ago is
  // a countdown an operator would time the next cue against.
  //
  // The console must never compute this from its own preview. Its programme pane
  // renders through the same component, so it holds a second player of the same
  // file that buffers differently and carries on if the wall's copy stalls.
  getMedia = () => null,
  // WHAT THIS SCREEN COULD NOT LOAD, as a short sentence, or `null` (O-4). Sent
  // only when set, so an absent field means "nothing failed" and clears the
  // desk's last report — which is the honest reading of a beat that says nothing.
  getMediaError = () => null,
}) {
  // Channel 0 is a raw template preview with no channel behind it — there is no
  // screen for an operator to worry about, so there is nothing to report.
  if (!Number.isFinite(channelId) || channelId <= 0) return () => {};

  let stopped = false;

  // ── The page's account of its own silence. See the header note (RG-119) ──
  //
  // `lastTickAt` moves on every tick that RUNS, whether or not the send lands, so
  // a page that keeps ticking into a broken socket still reports a one-interval
  // gap and is not confused with a page the OS stopped running.
  let lastTickAt = null;
  let hiddenSince = null;
  let hiddenMs = 0;
  const doc = typeof document === 'undefined' ? null : document;

  const onVisibility = () => {
    try {
      if (doc.hidden) hiddenSince ??= now();
      else if (hiddenSince !== null) {
        hiddenMs += now() - hiddenSince;
        hiddenSince = null;
      }
    } catch {
      // Rule 1. A visibility listener may never take a live output page down.
    }
  };
  if (doc?.addEventListener) {
    doc.addEventListener('visibilitychange', onVisibility);
    if (doc.hidden) hiddenSince = now();
  }

  /** The two numbers for this tick, omitting what the page cannot know. */
  const gap = () => {
    const at = now();
    // A page suspended while hidden never runs this listener, so the time it
    // spent hidden has to be closed off here as well.
    if (hiddenSince !== null) {
      hiddenMs += at - hiddenSince;
      hiddenSince = doc?.hidden ? at : null;
    }
    const out = {};
    if (lastTickAt !== null) {
      out.since_ms = Math.max(0, Math.round(at - lastTickAt));
      out.hidden_ms = Math.max(0, Math.round(hiddenMs));
    }
    lastTickAt = at;
    hiddenMs = 0;
    return out;
  };

  /**
   * The clip fields for the wire, or nothing at all.
   *
   * A report with no duration is dropped whole rather than sent with a zero: zero
   * is not a clip that takes no time, it is a player that has not loaded one yet,
   * and "0:00 left" over a clip that has barely started is worse than saying
   * nothing. The engine drops it again on arrival for the same reason; this is the
   * near half of one rule, not a second one.
   */
  const mediaErrorField = () => {
    let e = null;
    try {
      e = getMediaError();
    } catch {
      e = null;
    }
    return typeof e === 'string' && e.trim() ? { media_error: e.trim().slice(0, 300) } : {};
  };
  const mediaFields = (m) => {
    const dur = Number(m?.dur_ms);
    const pos = Number(m?.pos_ms);
    if (!Number.isFinite(dur) || dur <= 0 || !Number.isFinite(pos) || pos < 0) return null;
    return {
      media_pos_ms: Math.round(Math.min(pos, dur)),
      media_dur_ms: Math.round(dur),
      media_paused: !!m.paused,
    };
  };

  const sendOverSocket = (ws, state, g, m) => {
    // OPEN only (readyState 1). A queued send on a reconnecting socket arrives
    // seconds later and would report a screen as healthy at a moment it demonstrably
    // was not — the beat would paper over the very gap it exists to expose.
    if (!ws || ws.readyState !== 1) return false;
    try {
      ws.send(JSON.stringify({ kind: 'beat', channel: channelId, state, ...g, ...(mediaFields(m) ?? {}), ...mediaErrorField() }));
      return true;
    } catch {
      return false;
    }
  };

  const sendOverBridge = async (state, g, m) => {
    try {
      const inv = invoke ?? (await import('@tauri-apps/api/core')).invoke;
      // camelCase across the bridge, snake_case on the wire: Tauri maps the
      // argument names and the WebSocket protocol does not.
      const mf = mediaFields(m);
      await inv('output_beat', {
        channelId,
        state,
        sinceMs: g.since_ms ?? null,
        hiddenMs: g.hidden_ms ?? null,
        // camelCase across the bridge, snake_case on the wire — the same mapping
        // the two lines above already make.
        mediaPosMs: mf?.media_pos_ms ?? null,
        mediaDurMs: mf?.media_dur_ms ?? null,
        mediaPaused: mf?.media_paused ?? null,
        // A PICTURE OR CLIP THIS SCREEN COULD NOT LOAD, or null (O-4).
        mediaError: mediaErrorField().media_error ?? null,
      });
    } catch {
      /* no backend, or the command is gone. Stay silent and go stale. */
    }
  };

  const tick = () => {
    if (stopped) return;
    let state;
    try {
      state = getState();
    } catch {
      // If the page cannot say what it is showing, it does not get to claim it is
      // fine. Skip the beat.
      return;
    }
    if (!PAINT_STATES.includes(state)) return;
    let ws = null;
    try {
      ws = getWs();
    } catch {
      ws = null;
    }
    const g = gap();
    // Read once per beat, not per frame. `timeupdate` fires several times a second
    // and none of those are worth a message; the beat's own interval is the rate
    // an operator can read anyway.
    let m = null;
    try {
      m = getMedia();
    } catch {
      m = null;
    }
    if (sendOverSocket(ws, state, g, m)) return;
    // A kiosk page has no bridge, so this is a no-op there and the beat correctly
    // goes stale while its socket is down.
    void sendOverBridge(state, g, m);
  };

  // Report at once, so a screen that has just opened is not shown as silent for
  // the first two seconds of its life — the moment an operator is most likely to
  // be looking at it.
  tick();
  const id = setInterval(tick, BEAT_INTERVAL_MS);

  return () => {
    stopped = true;
    clearInterval(id);
    if (doc?.removeEventListener) doc.removeEventListener('visibilitychange', onVisibility);
  };
}


// ── THE CONSOLE HALF ──────────────────────────────────────────────────────────
//
// `describeScreen` is pure, and it is pure on purpose: the rule about what the
// operator is told is the part that must never be wrong, and a rule buried in a
// component can only be tested by mounting one. Live and the Outputs inspector
// both call this, so they cannot disagree about the same screen.

/**
 * How long a just-attached screen may stay silent before silence becomes a
 * finding. A window that has only just opened has not had time to report, and
 * calling that a fault would teach an operator to ignore the one colour that
 * matters.
 */
export const BEAT_GRACE_MS = 8000;

/**
 * Is this screen attached, and is it answering? — the half of the verdict that has
 * nothing to do with what Relay is currently sending.
 *
 * Both surfaces that describe a screen call this, so they cannot reach different
 * conclusions about the same one. The Live pane and the Outputs table use
 * different words for an operator ("On Air" vs "LIVE"); they must not be allowed
 * to use different *facts*.
 *
 * Returns one of:
 * - `unknown`     — nothing has been polled yet. Not a claim.
 * - `unsupported` — Relay cannot drive this target at all (NDI is parked).
 * - `offline`     — nothing is attached: no window open.
 * - `never`       — attached, and has never once reported painting.
 * - `silent`      — it reported before, and has now stopped. The worst case, and
 *                   the one that used to be invisible.
 * - `ok`          — reported within the staleness window.
 */
export function screenFault(st) {
  if (!st) return 'unknown';
  if (!st.supported) return 'unsupported';
  if (!st.online) return 'offline';
  if (st.painting) return 'ok';
  return st.last_beat_ms === null || st.last_beat_ms === undefined ? 'never' : 'silent';
}

/**
 * What to say about one screen.
 *
 * Returns `{ kind, label, note }`. `kind` chooses the colour, and it obeys the
 * colour law (DECISIONS §22): **amber is spent only on a screen that is both
 * genuinely on air and answering.** A screen that is not answering can never be
 * amber, and neither can one that IS answering and says it is showing nothing —
 * that is the entire point of this function existing, because "Relay believes it
 * sent content" and "the projector is showing it" are different claims and only
 * the first was ever checked.
 *
 * A screen the operator has TAKEN DOWN on its own (`st.down`) is answered before
 * either claim is compared, because the two disagree on purpose there and the
 * disagreement is not news.
 *
 * @param st       the channel's `ChannelLiveness` row, or null before the first poll
 * @param wall     `{ rehearsing, live, black }` — what Relay believes it is sending
 * @param waitedMs how long this screen has been attached without answering
 */
export function describeScreen(st, wall, waitedMs = 0) {
  const fault = screenFault(st);
  if (fault === 'unknown') return { kind: 'unknown', label: 'Checking…', note: '' };
  if (fault === 'unsupported')
    return { kind: 'idle', label: 'Unavailable', note: st.detail ?? '' };
  if (fault === 'offline') return { kind: 'idle', label: 'No window', note: st.detail ?? '' };

  if (fault !== 'ok') {
    // Never answered AND still inside the grace window: say so plainly rather
    // than accusing a screen that is still starting up.
    if (fault === 'never' && waitedMs < BEAT_GRACE_MS)
      return { kind: 'idle', label: 'Waiting…', note: 'the screen has not reported yet' };
    return {
      kind: 'down',
      label: 'Not responding',
      note:
        fault === 'never'
          ? 'this screen has never reported painting'
          : `last answered ${Math.round(st.last_beat_ms / 1000)}s ago`,
    };
  }

  // ── THE LAST PIECE OF THE SAME BUG ──────────────────────────────────────────
  //
  // Everything above this line asks the screen whether it is ANSWERING. That was
  // the whole of RG-01, and it left one half of the badge still derived from what
  // Relay believes it sent: a screen that answers punctually and says **clear**
  // read `On Air`, in amber, because `wall.live` was true. That is the shape of
  // RG-129 — an output page that reconnected mid-service and came back blank
  // while every instrument said On Air — and of a blackout that did not land, and
  // of a `clear_screens` that returned `Ok` over a screen still showing the
  // previous verse. In each of them the screen was saying so, on the beat, and
  // nothing read the answer.
  //
  // So the screen's own last word decides the badge, and Relay's belief is only
  // what that word is checked AGAINST:
  //
  //   they agree      → say it: On Air · Blackout · Ready
  //   they disagree   → say THAT, and print both claims
  //
  // **Amber is now spent only on a screen that has itself said it is painting
  // content** (rule 18: amber is never allowed to lie).
  //
  // ── What this costs, stated rather than hidden ──────────────────────────────
  //
  // A beat is `BEAT_INTERVAL_MS` apart and the console polls on its own 2s timer,
  // so for a few seconds after every fire the freshest word a screen has said is
  // still the one from before it. During that window this reports `Not confirmed`
  // rather than `On Air` — grey, calm, and TRUE: Relay genuinely cannot yet
  // confirm. It resolves to amber on the next beat. The alternative is to keep
  // claiming amber from Relay's own belief, which is the defect, and the honest
  // direction to lag in is the cautious one. It is deliberately NOT rose: an
  // alarm that fires on every fire is an alarm an operator learns to ignore, and
  // a screen that is genuinely blank is then the one card that STAYS grey while
  // the others go amber.
  //
  // `null` when the row carries no `paint_state` is treated as "has not said" for
  // the same reason. Rust reads the state and the age from one beat, so a
  // `painting` row always carries one; a row without it is a caller's fixture,
  // and a fixture must not be able to earn amber that a screen would not.
  const says = PAINT_STATES.includes(st.paint_state) ? st.paint_state : null;
  const seen = says ? `screen: ${says}` : '';
  if (wall?.rehearsing) return { kind: 'rehearsal', label: 'Rehearsal', note: seen };

  // ── THE OPERATOR TOOK THIS SCREEN OUT OF THE WALL ──────────────────────────
  //
  // `st.down` is `clear` or `black` when the operator used the per-screen control
  // on this screen, and absent for every screen that is following the wall.
  //
  // It has to be read HERE, above the agreement check, and reading it is not
  // optional. Below it, Relay's belief (`content` is on the wall) would be
  // compared against this screen's own beat (`clear`, because it was told to
  // clear) and the badge would read **Not confirmed** for the rest of the
  // service: a standing alarm about a screen doing exactly what it was told. An
  // alarm that fires on a correct state is an alarm an operator learns to ignore,
  // and the genuinely broken screen beside it is then the one nobody looks at.
  //
  // It is NOT amber. Amber means live and is never allowed to lie (rule 18), and
  // this screen is showing nothing on purpose. It is not rose either: nothing has
  // failed. Grey, with the reason in words, which is what `ready` already means
  // here for **Blackout** and **Ready**.
  //
  // The note names the way back, because a durable state an operator can forget
  // is a state that has to say how to undo it.
  if (st.down === 'clear' || st.down === 'black') {
    return {
      kind: 'ready',
      label: st.down === 'black' ? 'Taken down · black' : 'Taken down',
      note: seen ? `you took this screen down · ${seen}` : 'you took this screen down',
    };
  }

  // What Relay believes it is sending this screen, in the screen's own vocabulary.
  // A SCREEN THAT SAYS ITS PICTURE DID NOT LOAD IS NOT ON AIR (O-4, 2026-09-21).
  // Its DOM has a slide, so `paint_state` honestly reads `content`; the picture
  // inside it is blank. Ranked with `down`, because a congregation looking at a
  // blank frame under an amber badge is the failure rule 35 exists to stop.
  if (typeof st.media_error === 'string' && st.media_error.trim()) {
    return { kind: 'down', label: 'Not painting the picture', note: st.media_error.trim() };
  }

  const sending = wall?.live && !wall?.black ? 'content' : wall?.black ? 'black' : 'clear';
  // `clear` and `black` both mean "nothing of ours is on that screen", which is
  // the claim a Blackout or a Ready badge makes. Only `content` vs not-content is
  // a difference a congregation can see, so only that is checked.
  const agrees = says !== null && (sending === 'content' ? says === 'content' : says !== 'content');

  if (!agrees) {
    return {
      kind: 'ready',
      label: 'Not confirmed',
      note:
        says === null
          ? 'the screen has not said what it is showing'
          : `${RELAY_CLAIM[sending]} · the screen says ${says}`,
    };
  }
  if (sending === 'content') return { kind: 'onair', label: 'On Air', note: seen };
  return { kind: 'ready', label: sending === 'black' ? 'Blackout' : 'Ready', note: seen };
}

/** Relay's half of the note, in words — one per thing Relay can be sending. */
const RELAY_CLAIM = {
  content: 'Relay is sending content',
  black: 'Relay has blacked this screen out',
  clear: 'nothing is on the programme',
};

/**
 * Can the operator switch this screen on or off from here, and what does the
 * control say?
 *
 * ── Why this belongs on the run surface ──────────────────────────────────────
 *
 * The Output Status pane was read-only on the argument that "during a service the
 * only question is: is it up?". That argument is half right — it IS the only
 * question — and it left the pane at a dead end, because the pane's whole purpose
 * is to report a screen that is down and it offered no way to bring one back. An
 * operator who reads **Not responding** has to leave the run surface, find the
 * Outputs tab and hunt for the row, mid-service, with a congregation waiting.
 *
 * Switching a screen on or off is not configuration — it is the repair for the
 * state this pane exists to report. Changing a screen's DISPLAY or its template
 * is configuration, and that stays in the Outputs tab.
 *
 * ── The three answers, and why "no control" is one of them ───────────────────
 *
 * A browser source (OBS, a kiosk tab, a phone) cannot be opened from this
 * machine: it is a page on someone else's device, and the honest control is a
 * sentence telling the operator where to go, not a button that would do nothing.
 * This repository has shipped a handlerless button before; a disabled control
 * that says why is the version that does not waste a service.
 *
 * `st` is the channel's health row, `channel` its record. Pure — the rule lives
 * here so Live and the Outputs tab cannot disagree about the same screen.
 */
export function screenSwitch(st, channel) {
  if (channel?.render_target !== 'native_window') {
    return {
      action: null,
      label: 'Browser source',
      why: 'Open or close this one where it runs — OBS, the kiosk tab, or the phone.',
    };
  }
  const fault = screenFault(st);
  // `unknown` is "we have not asked yet", not "it is off". Offering "Turn on" for
  // a screen that may already be on would be a guess printed as a control.
  if (fault === 'unknown') return { action: null, label: 'Checking…', why: '' };
  if (fault === 'unsupported')
    return { action: null, label: 'Unavailable', why: st?.detail ?? '' };
  if (fault === 'offline')
    return { action: 'on', label: 'Turn on', why: 'No window is open on this display.' };
  return { action: 'off', label: 'Turn off', why: '' };
}

/** Badge class per kind. Rose is "a failure the operator must act on". */
export const SCREEN_BADGE = {
  unknown: 'grey',
  idle: 'grey',
  ready: 'grey',
  rehearsal: 'amethyst',
  onair: 'amber',
  down: 'rose',
};

/** The Outputs table's word for each fault. Same facts, its own vocabulary. */
export const FAULT_WORD = {
  unknown: '—',
  unsupported: 'UNAVAILABLE',
  offline: 'IDLE',
  never: 'NO ANSWER',
  silent: 'NOT RESPONDING',
  ok: 'LIVE',
};

/**
 * REPORTING — does this screen still answer for itself, and when did it last?
 *
 * The Outputs inspector's own row (docs/REBRAND.md §5: `Type · Transport · Output
 * · URL · Reporting`). It lives HERE, beside `screenFault` and `describeScreen`,
 * for the reason rule 35 keeps giving: a word about a screen's health composed
 * inside a component is a word no other surface can be held to, and the inspector
 * already carried two of them written out by hand — a ternary chain for the
 * header badge and a second one for "Screen says", neither of which any test
 * could reach without mounting the view.
 *
 * `word` answers "is it reporting?" and `note` is the evidence — Relay's own
 * claim and the screen's own claim side by side, because when the two disagree
 * that disagreement IS the finding.
 *
 * `never` is not `no`. One is "attached, and has never once said it was
 * painting"; the other is "nothing is attached to ask". They want different
 * repairs, and collapsing them into one reassuring word is rule 35 exactly.
 *
 * @param st the channel's `ChannelLiveness` row, or null before the first poll
 */
export function screenReporting(st) {
  const fault = screenFault(st);
  if (fault === 'unknown') return { word: '—', note: '' };
  if (fault === 'unsupported') return { word: '—', note: st.detail ?? '' };
  if (fault === 'offline') return { word: 'no', note: st.detail ?? 'nothing is attached' };
  if (fault === 'never')
    return { word: 'never', note: 'attached, and has never reported painting' };
  if (fault === 'silent')
    return { word: 'stopped', note: `last answered ${Math.round(st.last_beat_ms / 1000)}s ago` };
  return {
    word: 'yes',
    note: st.paint_state
      ? `screen: ${st.paint_state} · ${Math.round((st.last_beat_ms ?? 0) / 1000)}s ago`
      : '',
  };
}

/**
 * The plain-language word for a screen's render target.
 *
 * ONE definition, because two surfaces name the same thing: the Outputs table's
 * TYPE column and Live's Output Status pane. Live had no word at all — its status
 * line fell back to printing the raw column value (`native_window`) at a volunteer
 * mid-service, which is the same defect as rendering a raw `Err` string.
 */
export function screenKind(renderTarget) {
  if (renderTarget === 'native_window') return 'Native window';
  if (renderTarget === 'ndi_encode') return 'NDI';
  return 'Network client';
}

/** How the pixels leave the machine. Pairs with `screenKind`. */
export function screenTransport(renderTarget) {
  if (renderTarget === 'native_window') return 'HDMI / display';
  if (renderTarget === 'ndi_encode') return 'unavailable';
  return 'WebSocket';
}
