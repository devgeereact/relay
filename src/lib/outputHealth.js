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
//    anything else.
//
// The interval is Rust's `channels::BEAT_INTERVAL_MS`, and the staleness window it
// has to stay under is `channels::BEAT_STALE_MS`. They are coupled — three beats
// of grace — and `r6-contracts.test.js` fails if this file and that one drift.

/** How often a screen reports in. Must match `channels::BEAT_INTERVAL_MS`. */
export const BEAT_INTERVAL_MS = 2000;

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
export function startBeat({ channelId, getState, getWs = () => null, invoke = null }) {
  // Channel 0 is a raw template preview with no channel behind it — there is no
  // screen for an operator to worry about, so there is nothing to report.
  if (!Number.isFinite(channelId) || channelId <= 0) return () => {};

  let stopped = false;

  const sendOverSocket = (ws, state) => {
    // OPEN only (readyState 1). A queued send on a reconnecting socket arrives
    // seconds later and would report a screen as healthy at a moment it demonstrably
    // was not — the beat would paper over the very gap it exists to expose.
    if (!ws || ws.readyState !== 1) return false;
    try {
      ws.send(JSON.stringify({ kind: 'beat', channel: channelId, state }));
      return true;
    } catch {
      return false;
    }
  };

  const sendOverBridge = async (state) => {
    try {
      const inv = invoke ?? (await import('@tauri-apps/api/core')).invoke;
      await inv('output_beat', { channelId, state });
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
    if (sendOverSocket(ws, state)) return;
    // A kiosk page has no bridge, so this is a no-op there and the beat correctly
    // goes stale while its socket is down.
    void sendOverBridge(state);
  };

  // Report at once, so a screen that has just opened is not shown as silent for
  // the first two seconds of its life — the moment an operator is most likely to
  // be looking at it.
  tick();
  const id = setInterval(tick, BEAT_INTERVAL_MS);

  return () => {
    stopped = true;
    clearInterval(id);
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
 * amber — that is the entire point of this function existing, because "Relay
 * believes it sent content" and "the projector is showing it" are different
 * claims and only the first was ever checked.
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

  const seen = st.paint_state ? `screen: ${st.paint_state}` : '';
  if (wall?.rehearsing) return { kind: 'rehearsal', label: 'Rehearsal', note: seen };
  if (wall?.live && !wall?.black) return { kind: 'onair', label: 'On Air', note: seen };
  return { kind: 'ready', label: wall?.black ? 'Blackout' : 'Ready', note: seen };
}

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
