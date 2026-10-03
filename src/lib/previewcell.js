/**
 * WHAT THE NEXT-UP PANE PAINTS FOR ONE CELL (RG-236).
 *
 * The preview built `{ reference, text }` out of a cell's LABEL, which is right
 * for a verse and wrong for the one cue type whose whole content is a picture: a
 * media cue previewed as its own filename, typeset across a congregation
 * template at whatever size the template asks for. `IMG_3427.mov` at 37px is not
 * what is coming, and it is not anything a screen will ever show.
 *
 * A media cue previews as the MEDIA — the picture itself for a still, the first
 * frame for a clip (the renderer is asked for a `still`, so nothing plays in the
 * pane). The words go: there are none to show.
 *
 * **A cue whose asset has gone keeps its words.** No id, a deleted row and a
 * resolved one are three different situations, and only the third has a picture;
 * an empty frame reads as a cue with nothing in it, where the filename at least
 * says which cue cannot be found.
 *
 * Pure, so the pane and its test share one definition, and so the rule can be
 * read without mounting a run surface.
 */
export function previewOfCell(cell, mediaUrl) {
  const label = cell?.reference ?? cell?.label ?? '';
  const words = { reference: label, text: cell?.text || label, translation: null };
  if (cell?.ctype !== 'media' || !mediaUrl) return words;
  return {
    reference: null,
    text: '',
    translation: null,
    media_url: mediaUrl,
    media_kind: cell?.mediaKind === 'video' ? 'video' : 'image',
  };
}
