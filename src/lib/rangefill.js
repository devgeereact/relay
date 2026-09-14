/**
 * One slider, everywhere: the filled share of the track, as a CSS custom
 * property the stylesheet draws from.
 *
 * `app.css` paints the track with a gradient that stops at `--rp`. WebKit gives
 * no `::-webkit-slider-runnable-track` progress pseudo-element (Firefox does,
 * via `::-moz-range-progress`), and Relay ships on a WebKit webview on macOS, so
 * the filled part has to come from somewhere. This is that somewhere.
 *
 * It replaces `accent-color`, which drew a different slider on every platform —
 * the sensitivity dial an operator learns on one machine was a different
 * instrument on the next.
 *
 * Use it as a Svelte action on the input itself:
 *
 *     <input type="range" class="r-range" bind:value={gain} use:rangeFill={gain} />
 *
 * The argument is only a change signal: it makes Svelte call `update()` when the
 * bound value moves in CODE (a rebuilt panel, a preset loaded, a reset), which
 * fires no `input` event and is exactly the case a listener alone misses.
 */

/**
 * The value's share of the track, 0–100, clamped, and never NaN.
 *
 * NaN is the failure that matters here: it reaches CSS as an invalid value, the
 * declaration is dropped, and the track silently falls back to the stylesheet's
 * 50% default — a slider that reads half full whatever its value is.
 */
export function fillPercent(value, min, max) {
  const v = Number(value);
  const lo = Number(min);
  const hi = Number(max);
  if (!Number.isFinite(v) || !Number.isFinite(lo) || !Number.isFinite(hi)) return 0;
  const span = hi - lo;
  if (span <= 0) return 0;
  const share = ((v - lo) / span) * 100;
  return Math.max(0, Math.min(100, share));
}

/** Read the three facts off the element itself, so no caller can pass a stale set. */
function paint(node) {
  const min = node.min === '' ? 0 : node.min;
  const max = node.max === '' ? 100 : node.max;
  const pct = fillPercent(node.value, min, max);
  // Rounded to a tenth: a track is a few hundred pixels wide, so more precision
  // than that is a longer style attribute for no visible difference.
  node.style.setProperty('--rp', `${Math.round(pct * 10) / 10}%`);
}

/**
 * Svelte action. Paints on mount, on every `input` (a drag), and on `update()`
 * (the bound value changed in code).
 */
export function rangeFill(node) {
  const onInput = () => paint(node);
  node.addEventListener('input', onInput);
  paint(node);
  return {
    update() {
      paint(node);
    },
    destroy() {
      node.removeEventListener('input', onInput);
    },
  };
}
