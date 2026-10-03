/**
 * THE URL THAT PAINTS A CLIP'S FIRST FRAME — RG-279.
 *
 * Every surface that shows a clip as a picture rather than as playback —
 * `ui/MediaThumb.svelte`, the Library's own grid, `TemplateRender`'s `still`
 * renders — mounts `<video preload="metadata">` and expects a frame. The
 * operator's screenshot of the Library is what that actually produces: a black
 * card with a play triangle, in a cell sized correctly for the clip.
 *
 * **`preload="metadata"` does not paint anything, and the attribute never
 * promised to.** It fetches duration, dimensions and codec — enough to lay the
 * element out, which is what it is for — and stops at `readyState`
 * `HAVE_METADATA`, one short of the `HAVE_CURRENT_DATA` a browser needs before
 * it will draw a pixel. So the element sizes itself and stays black. Three
 * separate operator reports about "no snapshot" are this one fact.
 *
 * A media fragment asks for the missing step. `#t=0.1` is a seek, and a seek
 * decodes the frame at that instant and paints it. Relay's media server answers
 * ranged requests (`Accept-Ranges: bytes`, 206 with `Content-Range`), so it
 * costs one short read rather than the file.
 *
 * **Only for a STILL.** The fragment IS a seek, so putting it on the wall's own
 * `<video>` would drag a clip a congregation is watching back to 0.1s every time
 * the element re-rendered. The callers are the still renderers alone, and this
 * doc is the reason a later reader should not "tidy" it into `mediaUrl`, which
 * the wall shares.
 */

/**
 * @param {string|null|undefined} url a media URL, or nothing
 * @returns {string|null} the same URL asking for one frame, or the input
 */
export function posterUrl(url) {
  if (!url) return url ?? null;
  // A URL that already names an instant is one somebody chose deliberately —
  // a cue starting part-way in, a thumbnail of a later moment — and replacing
  // it would be this function overruling its caller.
  if (/#t=/.test(url)) return url;
  // NOT `#t=0`. Several browsers read a zero fragment as "no seek requested"
  // and paint nothing, which is the defect wearing a fragment. A tenth of a
  // second is past the start and still inside the first frame of anything
  // recorded at ten frames a second or better.
  return `${url}#t=0.1`;
}
