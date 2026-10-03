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
