// Cue builders: turning Library content into a plan cue.
//
// Every content type reduces to the same polymorphic cue — `cue_type` plus a
// `payload` (docs/DECISIONS.md) — so the Planner, the monitors and the one
// renderer never branch per type. These builders are the single definition of
// each payload's shape.
//
// The song builder in particular was written out twice, character for character,
// in ServicePlanner and in the Lyrics library view. Two copies of the payload
// that a plan is *stored* with is a migration hazard: change one and a song added
// from the Library silently stops matching a song added from the Planner.

import { expandSections } from './stores/capture.js';

/**
 * Build the cue for adding a song to a plan.
 *
 * `arr` is a saved arrangement (a named play-order stored as section indices), or
 * null for "Standard" — every section, once, in order. Standard is never
 * persisted as an arrangement; it is just the absence of one.
 *
 * The sections are EXPANDED here, at add time, so a chorus that repeats four
 * times becomes four slides. The arrangement sequence still rides along on the
 * payload so a later lyric edit can re-expand into the right (possibly repeated)
 * slots — which is why the song is stored by reference AND snapshot.
 */
export function songCue(song, arr) {
  const base = song.sections.map((s) => ({ tag: s.tag, label: s.label, lyrics: s.lyrics }));
  const sections = arr ? expandSections(base, arr.sequence) : base;
  return {
    label: arr ? `${song.title} · ${arr.name}` : song.title,
    payload: {
      song_id: song.id,
      title: song.title,
      author: song.author,
      song_key: song.song_key,
      sections,
      arrangement_name: arr ? arr.name : 'Standard',
      arrangement_seq: arr ? arr.sequence : null,
    },
  };
}
