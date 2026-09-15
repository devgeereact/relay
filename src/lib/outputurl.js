/**
 * THE BROWSER-SOURCE URL — one builder, because it is a contract with OBS.
 *
 * This is what an operator pastes into OBS, vMix or a kiosk page on a Raspberry
 * Pi, and it is how that screen learns which channel it is and what to wear. It
 * was built in two places, four lines apart, and only one of them was corrected
 * when a screen gained the ability to have no look of its own — so the Copy URL
 * button and the inspector's readout disagreed about the same screen.
 *
 * The rule that matters: **a screen with no template of its own says so by
 * saying nothing.** Writing `template_id=1` for a follower started its browser
 * source on a built-in and left it there until a `channel_template` message
 * arrived — the operator's own window following the content look while the
 * stream wore something else, which is the divergence nobody at the desk can
 * see (DECISIONS §70).
 */

/** The port the embedded HTTP server serves the output and stage pages on. */
export const OUTPUT_PORT = 8032;

/**
 * @param host        the LAN address of this machine (or a placeholder)
 * @param channel     the screen's id — ALWAYS present; it is what the screen is
 * @param templateId  the screen's own template, or null/undefined when it follows
 * @param name        the screen's name, for the page title
 */
export function outputUrl(host, channel, templateId, name = '') {
  const base = `http://${host}:${OUTPUT_PORT}/output.html?channel=${channel}`;
  const tpl = templateId == null || templateId === '' ? '' : `&template_id=${templateId}`;
  const label = name ? `&name=${encodeURIComponent(name)}` : '';
  return `${base}${tpl}${label}`;
}
