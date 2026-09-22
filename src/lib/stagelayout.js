// WHAT A PREACHER'S SCREEN CAN SHOW — the one list of zones.
//
// These were `const ZONES` inside `Stage.svelte`, which was right while that
// page was the only thing that knew about them. The operator can now assign a
// layout from Outputs, so the desk has to offer the same switches the screen
// renders — and a second copy of this list is a desk that offers a zone no page
// draws, or misses one it does. `names.test.js` exists because this repository
// keeps making that mistake with words; this is the same mistake with keys.
//
// The ORDER is the order they appear in the Zones panel, and the labels are the
// operator's words rather than the field names: somebody choosing what a screen
// shows is choosing "Stage Timer", not `programme`.

/** Every zone a stage screen can show, in the order the panel lists them. */
export const STAGE_ZONES = [
  { key: 'reading', label: 'Reading' },
  // SOMETHING THE OPERATOR PUT THERE FOR THIS PERSON — an announcement to read
  // out, or the preacher's own deck. It shares the reading's area and LOSES it:
  // scripture overrides the slide, and the slide comes back when the reading is
  // cleared rather than having to be pushed again.
  //
  // A layout saved before this key existed has no opinion about it, and an absent
  // key reads as OFF rather than as the default's ON. That is the conservative
  // direction: a screen the operator configured last month does not start
  // carrying something new without being asked.
  { key: 'media', label: 'Slide' },
  { key: 'next', label: 'Next' },
  { key: 'note', label: 'Stage Note' },
  { key: 'countdown', label: 'Screen Countdown' },
  { key: 'clock', label: 'Clock' },
  { key: 'elapsed', label: 'Service elapsed' },
  // The preacher's bookkeeping. A lobby TV running this page has no business
  // carrying it, and until this key existed there was no way to take it off —
  // the rail was the one region on the screen with no switch behind it.
  { key: 'programme', label: 'Stage Timer' },
];

/**
 * Everything on.
 *
 * The default is DELIBERATELY the noisiest arrangement, not the quietest: a
 * screen that shows too much is a screen somebody turns things off on, and a
 * screen that shows too little is one where a preacher never learns the thing
 * was available at all.
 */
export const DEFAULT_STAGE_ZONES = Object.freeze(
  Object.fromEntries(STAGE_ZONES.map((z) => [z.key, true])),
);

/**
 * Read a stored or delivered zone set, key by key, off the defaults.
 *
 * One reader for both sources — the device's own `localStorage` and the
 * operator's layout — so a zone added in a later version arrives on its own
 * default rather than absent, and a corrupt value cannot delete one.
 *
 * Answers `null` when the input names no zone at all. That is the difference
 * between "no layout" and "a layout that shows nothing", and it is a working
 * screen against a blank one: the key-by-key merge would otherwise turn `{}`
 * into a full set of defaults and look exactly like the fallback it replaced.
 */
export function readStageZones(raw) {
  if (!raw || typeof raw !== 'object') return null;
  const out = { ...DEFAULT_STAGE_ZONES };
  let said = 0;
  for (const z of STAGE_ZONES) {
    if (typeof raw[z.key] === 'boolean') {
      out[z.key] = raw[z.key];
      said += 1;
    }
  }
  return said > 0 ? out : null;
}

/**
 * HOW BIG THE PREACHER'S CLOCK IS (RG-240).
 *
 * A platform monitor across a room and a phone on a lectern want different
 * figures, and until this existed the page sized itself from its own box with
 * nobody able to say otherwise.
 *
 * The OPERATOR chooses, because RG-241 takes the zone picker off the phone and
 * the same argument applies here: a preacher who taps something mid-sermon must
 * not be able to lose, or shrink, the one figure they are relying on.
 *
 * It travels in the zone blob rather than in a second column or a second frame.
 * `readStageZones` reads only the keys it knows and ignores the rest, so an
 * extra key is already safe on that wire, and one delivery path cannot disagree
 * with itself about which screen it is describing.
 */
export const TIMER_SIZES = [
  { key: 'normal', label: 'Normal' },
  { key: 'large', label: 'Large' },
  { key: 'huge', label: 'Huge' },
];

/** The multiplier each size applies to the figure the page would otherwise draw. */
const TIMER_SCALE = { normal: 1, large: 1.35, huge: 1.8 };

/**
 * The size a layout asked for.
 *
 * Always answers a size, unlike `readStageZones` — a figure has to be drawn at
 * something, and "no opinion" and "shows nothing" are not different states for a
 * scale the way they are for a zone. A layout saved before this existed, or a
 * value nobody recognises, is `normal`: the size the page drew before anybody
 * could choose.
 */
export function readTimerSize(raw) {
  const v = raw && typeof raw === 'object' ? raw.timer_size : null;
  return TIMER_SIZES.some((s) => s.key === v) ? v : 'normal';
}

/** The multiplier for a size, or 1 for anything unrecognised. */
export function timerScale(key) {
  return TIMER_SCALE[key] ?? 1;
}

/**
 * WHERE THE FIGURES SIT — across the foot, or beside the reading.
 *
 * It was a device setting inside the Zones panel, and it went where the panel
 * went (RG-241): it is a layout choice like every other, and the person
 * responsible for the screen is the person who should make it.
 *
 * `null` when the layout says nothing, so a caller can fall back to the device's
 * stored preference rather than being handed a default that would overwrite an
 * arrangement a church already has. That is the same distinction
 * `readStageZones` draws, and for the same reason.
 */
export function readStageFigures(raw) {
  const v = raw && typeof raw === 'object' ? raw.figures : null;
  return v === 'beside' || v === 'bottom' ? v : null;
}
