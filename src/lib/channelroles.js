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
