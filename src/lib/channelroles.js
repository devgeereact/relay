/**
 * WHAT A SCREEN IS FOR — the one reader of `output_channels.role`.
 *
 * ## Why this module exists
 *
 * Live's programme pane used to answer "which screen am I previewing?" with a
 * heuristic written inline:
 *
 * ```js
 * channels.find((c) => c.render_target === 'native_window') ?? channels[0] ?? null
 * ```
 *
 * `render_target` is how a screen is WIRED, not what it is for. So renaming the
 * main screen changed nothing, deleting it silently promoted whatever happened to
 * be first in the list, and a church running its wall through OBS — every channel
 * a `network_client` — had the pane previewing its Streaming feed with no
 * indication that it had fallen through. One expression, three different
 * meanings, and the pane said the same thing in all three.
 *
 * `role` makes it a setting. This module is the only place that reads it, so Live,
 * the Outputs picker and the output page cannot come to different conclusions
 * about which screen is the main one.
 *
 * ## The fallback is kept, and it is never silent
 *
 * An install that has never opened Outputs still has to preview SOMETHING — a
 * blank programme pane would be a worse answer than an imperfect one. So the old
 * heuristic survives as the fallback, and `byRole` says which of the two answered.
 * A caller that renders the name without rendering that distinction has built the
 * status line rule 35 is about: one sentence over two different situations.
 *
 * `channels.rs`'s `MonitorInfo.primary` is unrelated and stays unrelated — that is
 * a property of a physical display, not of an output channel.
 */

/** The roles a screen may hold, with the words an operator reads. */
export const CHANNEL_ROLES = [
  { key: 'main', label: 'Main screen' },
  { key: 'stage', label: 'Stage display' },
];

/** The words for "this screen has no special job", used by the picker. */
export const NO_ROLE_LABEL = 'No special role';

/**
 * WHICH SCREEN THE PROGRAMME PANE IS A PREVIEW OF.
 *
 * @param {Array} channels rows as `list_output_channels` returns them
 * @returns {{ channel: object|null, byRole: boolean, label: string }}
 *   `byRole` is false when no screen holds `main` and the old render-target
 *   heuristic answered instead. `label` is the sentence for the pane, and it
 *   differs between the two cases on purpose.
 */
export function programmeScreen(channels) {
  const list = Array.isArray(channels) ? channels : [];
  const byRole = list.find((c) => c?.role === 'main') ?? null;
  if (byRole) return { channel: byRole, byRole: true, label: `as ${byRole.name}` };
  const guess = list.find((c) => c?.render_target === 'native_window') ?? list[0] ?? null;
  if (!guess) return { channel: null, byRole: false, label: '' };
  return { channel: guess, byRole: false, label: `as ${guess.name} · no main screen set` };
}

/**
 * MAY THIS SCREEN BE SHOWN A STAGE MESSAGE?
 *
 * The filter lives at the RECEIVER because the kiosk hub records nothing about
 * who connected and DECISIONS §35 is not being reversed: the hub broadcasts a
 * `stage_alert` to every client and cannot address one. What the output page has
 * is its own channel — the URL is channel-keyed (DECISIONS §29) — and the role
 * that channel holds.
 *
 * Only `stage` qualifies. No role is not a stage: a lobby TV and a streaming feed
 * both arrive here with `role == null`, and a message meant for the platform must
 * never be the thing a default answers yes to.
 */
export function acceptsStageMessage(role) {
  return role === 'stage';
}

/**
 * The role a channel id holds, given the map the backend publishes
 * (`{"1":"main","2":"stage"}`). Absent, unparseable or unknown is no role.
 *
 * Keys arrive as strings because JSON objects have string keys; the channel id a
 * page parses out of its own URL is a number. Comparing the two without this is
 * how a filter that looks right refuses everything.
 */
export function roleOf(roles, channelId) {
  if (!roles || typeof roles !== 'object') return null;
  const id = Number(channelId);
  if (!Number.isFinite(id) || id === 0) return null;
  const r = roles[String(id)];
  return typeof r === 'string' && r ? r : null;
}

/**
 * THE ADDRESS AN OPERATOR HANDS THE PREACHER.
 *
 * `stage.html` used to be a page rather than a screen: it said hello with no
 * identity and painted any Stage Message that reached it, so the address Outputs
 * printed was a bare `stage.html` and every copy of that page anywhere on the
 * network was a stage monitor. The receiving page now refuses a Stage Message
 * unless its own channel holds the `stage` role, which makes the bare address a
 * page that will never be handed one — working in every visible respect, and
 * silently missing the one thing it is set up for.
 *
 * So the desk hands out a SCREEN. A URL is produced only when a screen actually
 * holds the role, and `null` when none does: an address that half works is the
 * reassuring sentence over a broken thing that rule 35 is about, and the caller
 * has to say something instead of printing it.
 *
 * `others` names the stage screens this address is NOT, because several screens
 * may be stages (DECISIONS §89) — a confidence monitor and a preacher's tablet.
 * An operator setting up the second one needs to know the number changes rather
 * than concluding the link is broken.
 *
 * @param {string} host the LAN address of this machine (`local_ip`)
 * @param {Array} channels rows as `list_output_channels` returns them
 * @returns {{ url: string|null, channel: object|null, others: string[] }}
 */
export function stageRemoteUrl(host, channels) {
  const stages = (Array.isArray(channels) ? channels : []).filter((c) => c?.role === 'stage');
  if (!stages.length) return { url: null, channel: null, others: [] };
  const [first, ...rest] = stages;
  return {
    url: `http://${host}:8032/stage.html?channel=${first.id}`,
    channel: first,
    others: rest.map((c) => c.name),
  };
}

// ── RG-167 · WHAT THE TWO CLOCKS SAY WHEN THE THING BEHIND THEM IS BROKEN ────
//
// Rule 35: *a status badge that cannot detect its own failure is not a status
// badge*. Both timer controls were silent in exactly the way that rule is about.
//
// `Start timer` on Live succeeds when NOTHING holds the `stage` role, and it is
// right to — a timer is a registry fact and a screen may be opened a minute
// later (DECISIONS §91). What was wrong is that the band then read identically to
// a band with a stage tablet painting three feet from the preacher. The dock's
// countdown had the same shape: `on the screens` over a wall where every screen's
// template either hides the kind or is keyed.
//
// Both answers are computed here, pure, so that Live, the dock and anything added
// later cannot come to different conclusions about the same screens — the same
// reason `describeScreen` is pure, and the same reason this module is the only
// reader of `role`.
//
// THE WORDS ARE CHOSEN AGAINST A TEST THAT ALREADY EXISTED.
// `programmetimer.test.js` forbids *on stage*, *on air*, *on the screens* and
// *live* anywhere in that band, and it is right: `attached` is a claim about a
// socket. Whether the preacher's `programme` zone is switched on is `localStorage`
// on his own device and is not a fact available on this side of the room.
import { isKeyedTemplate, templateShows } from './layers.js';

/** `a, b and c` — the Oxford-free list an operator reads rather than a JSON array. */
function nameList(names) {
  if (names.length === 1) return names[0];
  if (names.length === 2) return `${names[0]} and ${names[1]}`;
  return `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`;
}

/** The screen's name, or a stand-in — never an id, which a volunteer cannot map. */
const nameOf = (c, fallback) => (c?.name || '').trim() || fallback;

/**
 * CAN A STAGE TIMER REACH ANYBODY?
 *
 * @param {Array<{c: object, s: {kind: string}, st: object|null}>} rows
 *   one per output channel: the channel row, the verdict `describeScreen` gave
 *   it, and the raw health row. The verdict is passed in rather than recomputed
 *   so there is exactly one rule about what a screen's beat means.
 * @param {{read?: boolean, error?: string}} list
 *   has the channel list been read at all, and why it failed if it did. A read
 *   that failed answers `[]` through `listOutputChannels` (group 2), which
 *   renders exactly like a church that has not set a stage display up — the
 *   reason lives in `readErrors`.
 * @returns {{ text: string, kind: string }}
 *   `kind` is for tests and for nothing on screen: this line wears no colour,
 *   because every colour that carries a promise is spoken for (rule 18) and the
 *   facts here could honestly wear none of them.
 */
export function describeStageReach(rows, list = {}) {
  const reason = list.error ? String(list.error) : '';
  if (reason) return { kind: 'unknown', text: `Cannot tell whether a Stage display is open — ${reason}` };
  if (list.read === false)
    return { kind: 'unknown', text: 'Cannot tell whether a Stage display is open — reading the screens' };

  const all = Array.isArray(rows) ? rows : [];
  const stages = all.filter((r) => r?.c?.role === 'stage');
  if (!stages.length)
    return { kind: 'norole', text: 'No screen is set as a Stage display — Outputs → Screens → Role' };

  // SEVERAL SCREENS MAY HOLD THE ROLE (§89) — a confidence monitor and a
  // preacher's tablet. The line reports the best of them, because the question it
  // answers is "will this reach anybody", and names the one it is talking about.
  const rank = { onair: 0, ready: 0, rehearsal: 0, idle: 2, down: 3, unknown: 4 };
  const best = [...stages].sort((a, b) => (rank[a.s?.kind] ?? 9) - (rank[b.s?.kind] ?? 9))[0];
  const name = nameOf(best.c, 'Stage display');
  const kind = best.s?.kind ?? 'unknown';

  if (!best.st || kind === 'unknown')
    return { kind: 'unknown', text: `Cannot tell whether a Stage display is open — ${name} has not been checked yet` };

  // NEVER REPORTED PAINTING, AND THE TRAP THAT USUALLY MEANS.
  // `Outputs → Screens`'s `Copy URL` hands out `output.html`, which has no
  // `timer` branch at all — only `stage.html` renders the rail, and only
  // `stageRemoteUrl` above produces that address. A screen wired from the wrong
  // copied link is attached, is answering, and will never show a Stage Timer.
  if (best.st.last_beat_ms == null && kind !== 'idle')
    return {
      kind: 'down',
      text: `${name} has never reported painting — a Stage Timer needs the stage address (Outputs → Sharing)`,
    };

  if (kind === 'idle') return { kind: 'idle', text: `${name}: no window` };
  if (kind === 'down') return { kind: 'down', text: `${name}: not responding` };
  return { kind: 'ok', text: `${name} attached` };
}

/**
 * WHICH SCREENS WOULD SHOW A SCREEN COUNTDOWN?
 *
 * Answerable without asking any screen anything, which is why it can sit on the
 * transport rather than waiting for a fire: both exclusions are properties of the
 * template each screen resolves.
 *
 *   · `templateShows(tpl, 'countdown')` — an explicit `shows` allow-list that
 *     omits the kind drops the fire before a renderer ever sees it, silently,
 *     and the screen holds whatever it had.
 *   · `isKeyedTemplate(tpl)` — a keyed screen is composited over a live camera,
 *     and RG-166 is why it now refuses the clock rather than painting it over
 *     the preacher.
 *
 * @param {Array} channels rows as `list_output_channels` returns them
 * @param {(c: object) => object|null} resolve the caller's own template resolver
 *   (`resolveOutputTemplate`), passed in so this cannot disagree with the wall
 * @param {{read?: boolean, error?: string}} list as above
 */
export function describeCountdownReach(channels, resolve, list = {}) {
  if (list.error || list.read === false)
    return { kind: 'unknown', text: 'Cannot tell which screens would show it' };

  const rows = Array.isArray(channels) ? channels : [];
  if (!rows.length) return { kind: 'none', text: 'No screens are set up — nothing would show it' };

  const ignoring = [];
  const keyed = [];
  for (const c of rows) {
    let tpl = null;
    try {
      tpl = resolve(c);
    } catch {
      // A resolver that threw is an unread answer, not an empty one — the same
      // distinction the two branches above draw.
      return { kind: 'unknown', text: 'Cannot tell which screens would show it' };
    }
    if (!templateShows(tpl, 'countdown')) ignoring.push(nameOf(c, `Screen ${c?.id}`));
    else if (tpl && isKeyedTemplate(tpl)) keyed.push(nameOf(c, `Screen ${c?.id}`));
  }

  const total = rows.length;
  const reaching = total - ignoring.length - keyed.length;
  const base =
    reaching === 0
      ? 'No screen would show it'
      : reaching === total
        ? total === 1
          ? 'Goes to the one screen'
          : `Goes to all ${total} screens`
        : `Goes to ${reaching} of ${total} screens`;

  const parts = [base];
  if (ignoring.length)
    parts.push(`${nameList(ignoring)} ${ignoring.length === 1 ? 'ignores' : 'ignore'} it`);
  if (keyed.length)
    parts.push(
      `${nameList(keyed)} ${keyed.length === 1 ? 'is' : 'are'} keyed — the countdown will not go there`,
    );
  return { kind: reaching === total ? 'all' : reaching === 0 ? 'none' : 'some', text: parts.join(' · ') };
}
