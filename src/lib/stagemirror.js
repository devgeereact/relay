/**
 * WHAT THE CONSOLE MAY SAY THE PREACHER'S SCREEN IS SHOWING.
 *
 * ## Why this is a projection and can never be a readback
 *
 * There is no mirror of the preacher's screen anywhere in the console, and
 * `TemplateRender` cannot be one: it renders an output LOOK — a verse, a
 * countdown, a lower third — and has no zone model, no next/note rail and no
 * Stage-Timer column. What the preacher's page actually draws is `Stage.svelte`,
 * which owns its own zone layout and is never imported by the console.
 *
 * Nor can the console ASK. The kiosk hub records nothing about who connected
 * (DECISIONS §35): liveness is a count per template id, a screen's beat says only
 * that it is still painting, and no frame carries back what a page decided to
 * draw. Whether the preacher's `programme` zone is switched on is `localStorage`
 * on his own device, deliberately, so two stage screens in one building may want
 * different things.
 *
 * So this module answers the question the console CAN answer — *what did Relay
 * tell that screen to show* — and is built so that every answer it cannot give
 * comes back as `unknown` rather than as a plausible guess. That is rule 35 on
 * the one panel an operator opens precisely because they cannot see the device:
 * a status line that cannot detect its own failure is not a status line, and a
 * mirror that paints a confident picture of a screen it has never heard from is
 * the same defect with a picture in it.
 *
 * `Channels.svelte:1655` already says this in the Screens inspector — *"Relay
 * cannot see what they chose"* — and a new panel that quietly contradicted it
 * would be one desk with two answers about one screen.
 *
 * Pure, like `channelroles.js` beside it and for the same reason: Live, the
 * Screens inspector and the Preacher's screen section must not be able to reach
 * different conclusions about the same tablet.
 */
import { STAGE_ZONES, readStageZones } from './stagelayout.js';

/** Every zone, with nothing claimed about any of them. */
const allUnknown = () =>
  STAGE_ZONES.map((z) => ({ key: z.key, label: z.label, state: 'unknown' }));

/** The screen's name, or a stand-in — never an id, which a volunteer cannot map. */
const nameOf = (c) => ((c?.name ?? '') + '').trim() || 'Stage display';

/**
 * WHICH ZONES THIS SCREEN IS DRAWING.
 *
 * Four answers, and they are four different situations rather than one sentence
 * over four:
 *
 *   `unknown`  — the screen list has not been read, or the read failed. An empty
 *                list from a failed read renders exactly like a church with no
 *                stage screen, which is why the reason is a parameter.
 *   `norole`   — nothing holds the `stage` role, so there is no screen to mirror.
 *   `device`   — a stage screen with no layout assigned. The device's own Zones
 *                panel decides, in its own `localStorage`, and this side of the
 *                room genuinely cannot see it.
 *   `assigned` — an operator gave this screen a layout, and an assigned layout
 *                WINS on the device (`Stage.svelte`: `zones = assignedZones ??
 *                deviceZones`). So the zone states are real, and the note beside
 *                them still says what they are: what Relay told the screen.
 *
 * A LAYOUT THAT NAMES NO ZONE IS NOT A LAYOUT, and this is the half of that rule
 * the desk has to keep. `readStageZones` answers `null` for `{}` and the
 * receiving page falls through to the device's own zones — so treating an empty
 * layout as "all defaults" here would paint seven confident rows over a screen
 * still being set by hand at the other end.
 *
 * @param {object} o
 * @param {object|null} o.channel the channel row holding the `stage` role
 * @param {object|null} o.layout  the `stage_layouts` row that channel is assigned
 * @param {{read?: boolean, error?: string}} o.list has the channel list been read
 * @returns {{kind: string, headline: string, note: string, zones: Array<{key: string, label: string, state: string}>}}
 */
export function stageMirrorZones({ channel = null, layout = null, list = {} } = {}) {
  const reason = list?.error ? String(list.error) : '';
  if (reason)
    return {
      kind: 'unknown',
      headline: `Cannot tell what the preacher's screen is showing — ${reason}`,
      note: '',
      zones: allUnknown(),
    };
  if (list?.read === false)
    return {
      kind: 'unknown',
      headline: "Cannot tell what the preacher's screen is showing — reading the screens",
      note: '',
      zones: allUnknown(),
    };
  if (!channel)
    return {
      kind: 'norole',
      headline: 'No screen is set as a Stage display — Outputs → Screens → Role',
      note: '',
      zones: allUnknown(),
    };

  const name = nameOf(channel);
  const set = layout ? readStageZones(layout.zones) : null;
  if (!set)
    return {
      kind: 'device',
      headline: `${name} is set from the device itself. Relay cannot see what they chose.`,
      note: 'Assign a layout below to decide from here instead.',
      zones: allUnknown(),
    };

  return {
    kind: 'assigned',
    headline: `${name} is set to the ${((layout.name ?? '') + '').trim() || 'assigned'} layout.`,
    // NEVER "is showing". The strongest honest verb is about what was SENT: a
    // screen that has not fetched the map, or is not on the network at all, is
    // drawing whatever it had. The health line beside this panel is the only
    // thing that speaks to whether anybody is listening, and it says `attached`
    // for the same reason.
    note: 'This is what Relay told the screen to show. Relay cannot ask a screen what it painted.',
    zones: STAGE_ZONES.map((z) => ({
      key: z.key,
      label: z.label,
      state: set[z.key] ? 'on' : 'off',
    })),
  };
}

/**
 * WHAT IS ON THE READING ZONE.
 *
 * The reading is the one zone whose CONTENT the console legitimately knows: the
 * hub publishes one frame to every client and retains the last `content`,
 * `clear` or `black` for a screen that joins late (rule 43), so what Relay last
 * sent is what a stage screen holds.
 *
 * Three things break that chain and each gets its own answer rather than being
 * folded into the happy one:
 *
 *   · A REHEARSAL publishes nothing to the kiosk hub at all. `$live` is then the
 *     rehearsal's content and the stage screen is still holding whatever went out
 *     before it — a thing the console does not keep. So: unknown.
 *   · THIS SCREEN TAKEN DOWN (`clear`/`black`, per screen) blanks it whatever is
 *     on the programme, and that is knowable and certain.
 *   · THE READING ZONE SWITCHED OFF draws none of it. When the zone state is
 *     itself unknown — a device-chosen screen — the caveat says so rather than
 *     the panel implying the verse is on the tablet.
 *
 * @param {object} o
 * @param {'on'|'off'|'unknown'} o.zone the reading zone's state from `stageMirrorZones`
 * @param {object|null} o.live what is on the programme (`$live`)
 * @param {boolean} o.black a global blackout
 * @param {string|null} o.down this screen's own down state — `'clear'` or `'black'`
 * @param {boolean} o.rehearsing
 * @returns {{kind: string, headline: string, caveat: string}}
 */
export function stageMirrorReading({
  zone = 'unknown',
  live = null,
  black = false,
  down = null,
  rehearsing = false,
} = {}) {
  // Only said where it changes the reading of the line above it. Under a screen
  // that is definitely blank it would be noise about a zone nobody is drawing.
  const caveat =
    zone === 'unknown'
      ? 'Whether this is drawn at all is the device’s own choice.'
      : '';

  if (zone === 'off')
    return { kind: 'off', headline: 'The Reading zone is switched off on this screen.', caveat: '' };
  if (rehearsing)
    return {
      kind: 'unknown',
      headline:
        'A rehearsal reaches no screen, so Relay cannot say what this one is still holding.',
      caveat,
    };
  if (down === 'clear' || down === 'black')
    return {
      kind: 'blank',
      headline: `You took this screen down${down === 'black' ? ' (black)' : ''}, so it is showing nothing.`,
      caveat: '',
    };
  if (black)
    return { kind: 'blank', headline: 'Blackout — every screen is showing nothing.', caveat: '' };
  if (!live)
    return {
      kind: 'blank',
      headline: 'The screens are clear — nothing has been sent to the reading zone.',
      caveat,
    };
  return { kind: 'content', headline: 'Relay last sent this to every screen.', caveat };
}
