/**
 * HOW MUCH OF A BIG SCREEN THE FALLBACK PROGRAMME RAIL TAKES — RG-265.
 *
 * RG-224 gave `output.html` a rail for a stage-role screen wearing a template
 * that declares no timer layer: full width, the bottom `RAIL_BASE_PCT`, with the
 * figure a share of that. On a 1080p projector across a platform that is 86
 * pixels of rail for the clock a preacher is working to.
 *
 * RG-240 put Normal / Large / Huge on the stage layout, `stage_zones` carries
 * it, and `stage.html` honours it — but `output.html` never asked. Neither
 * `Output.svelte` nor `TemplateRender.svelte` mentioned `stage_zones` or
 * `timer_size` at all, so an operator who set Huge for the platform monitor
 * moved the phone and nothing else: a control that reports success and changes
 * nothing on the screen it was set for, which is rule 35 wearing a different
 * coat.
 *
 * **The steps are the phone's own**, re-exported rather than restated:
 * `stagelayout.js::timerScale` is the one place that knows what `huge` means,
 * and a second copy here is a second thing that can disagree with the surface
 * the operator checked it against.
 */
import { timerScale } from './stagelayout.js';

/**
 * The rail's height with the size on Normal, as a percentage of the screen.
 *
 * **12, and RG-224's figure was 8.** The instinct was to leave the base alone so
 * an install that never opens the control renders as it did yesterday — but the
 * operator's complaint is about exactly that install: *"make sure this is
 * visible enough for preacher to know what time to get ready"*, at whatever it
 * is set to now. Preserving the old default would have answered a question
 * nobody asked and left the reported one open.
 *
 * 8% of 1080 is 86 pixels of rail, and the figure is a share of that. A
 * projector is read from ten metres and a phone from fifty centimetres; the two
 * were sharing a number that was chosen for neither.
 */
export const RAIL_BASE_PCT = 12;

/**
 * The multiplier for a size key — 1 for Normal, and never 0 or absent.
 *
 * An unknown key, a layout saved before `timer_size` existed, or a frame from
 * an older build all answer Normal. A zero would take the rail off a screen
 * whose operator never touched the setting.
 */
export function railScale(key) {
  return timerScale(typeof key === 'string' ? key : '');
}
