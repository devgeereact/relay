// WHY A CONTROL IS OFF — the one place the sentences are written.
//
// This is `errors.js` and `settingvalue.js` again, for the third absence in the
// product that reads the same whether or not anything is wrong. A greyed control
// says only "not now"; it never says why, and an operator halfway through a
// service has no way to tell "the engine is not attached" from "you are in safe
// mode" from "Relay has crashed". Measured on this tree: 27 buttons are gated on
// `!$capture.available`, `$safeMode` or `$serviceLock` and 16 of them say nothing.
//
// The reasons live here rather than at the call sites for the reason DESIGN_SYSTEM
// section 5 gives about a list row's missing value: the wording belongs to whoever
// knows what the absence MEANS, and that is the gate, not the button. Sixteen
// buttons each writing their own sentence about `!$capture.available` is sixteen
// descriptions of one fact, and they would not agree.
//
// WHAT THESE SENTENCES MAY NOT DO. They may not overstate the damage. A dropped
// Tauri bridge does not take down a screen that is already lit -- the output window
// and the kiosk clients keep painting the last frame they were sent -- so the
// honest sentence says the control cannot reach the screens FROM HERE and that
// whatever is up is still up. Telling a volunteer mid-service that their screens
// have gone when they have not is the same class of lie as a status badge that
// cannot detect its own failure (CLAUDE.md rule 35).

/** The Tauri bridge is not attached: `capture.available` is false. */
export const ENGINE_OFF =
  'Relay’s engine is not answering, so this cannot be done from here. ' +
  'Anything already on a screen is still there.';

/** Safe mode. It outranks everything and it is a deliberate operator choice. */
export const SAFE_MODE =
  'Relay is in safe mode, which deliberately keeps anything off your screens. ' +
  'Leave safe mode to use this.';

/** The service lock is engaged (servicelock.rs, CLAUDE.md section 40). */
export const SERVICE_LOCKED =
  'Not while a service is locked. Unlock the service in Settings first — ' +
  'unlocking does not end it.';

/** The microphone is open. */
export const MIC_LIVE = 'Not while the microphone is live. Stop listening first.';

/** This control is already doing the thing it was pressed for. */
export const BUSY = 'Already working on the last press.';

/**
 * The first reason that applies, or '' when the control is not gated at all.
 *
 * Pairs, in the order the gates should be READ OUT rather than the order they are
 * written in the `disabled` expression. Those are not the same list: a `disabled`
 * expression is an OR and its order is arbitrary, while an operator needs to be
 * told the most fundamental thing first. Safe mode before a missing engine before
 * a busy button, because fixing the first makes the others moot.
 *
 *     whyDisabled([$safeMode, SAFE_MODE], [!$capture.available, ENGINE_OFF])
 */
export function whyDisabled(...pairs) {
  for (const [when, why] of pairs) if (when) return why;
  return '';
}
