// WHAT THE PLANNER'S ADD PANEL SEARCHES, AND WHAT EACH RESULT BECOMES.
//
// `ServicePlanner.svelte` is the largest view in the product and three separate
// requirements are growing it at once, so the two blocks inside it that were
// already self-contained come out first. This is the first: the local filter the
// add panel runs over the media and announcement lists it already holds, and the
// PAYLOAD each result row turns into on its way to `add_plan_item`.
//
// THE PAYLOAD IS THE PART WORTH MOVING. A cue's payload is what every surface
// downstream reads — `slidesOf` looks for `media_id` and `kind`, the inspector's
// media lookup reads `media_id` and `filename`, the countdown renderer reads
// `minutes`, `label` and `done`, and `previewState` decides from all of it what
// the preview may honestly claim. Four builders spelled inline in a view, each
// five lines of object literal, are four places for a key to be misspelled with
// nothing failing until a Sunday. They are pure, so here they can be read against
// what consumes them.
//
// The component keeps the async half: it still owns `act()`, the store calls, the
// reload and the error surface. What moved is only the arithmetic and the shapes.

/**
 * How many rows the add panel shows when the search box is EMPTY.
 *
 * Not "all of them": the panel is a search surface, and a media library with two
 * hundred pictures in it would render two hundred rows under a box the operator
 * has not typed in yet. Eight is the most recent eight, which is what an operator
 * reaching for the file they imported ten minutes ago actually wants.
 */
export const RECENT_LIMIT = 8;

/** Lowercased, trimmed — the one place the query is normalised. */
function needle(q) {
  return String(q ?? '').trim().toLowerCase();
}

/**
 * The media rows an add-panel query matches.
 *
 * An EMPTY query is not "no results" — it is the recent list, which is what the
 * panel shows before anybody types. Returning `[]` there would open the panel on
 * an empty pane under a box nobody has used yet.
 */
export function filterMedia(all, q) {
  const list = Array.isArray(all) ? all : [];
  const n = needle(q);
  if (!n) return list.slice(0, RECENT_LIMIT);
  return list.filter((m) => String(m?.filename ?? '').toLowerCase().includes(n));
}

/**
 * The announcements an add-panel query matches — title OR body.
 *
 * The body is searched deliberately: an announcement's title is often "Notice"
 * and the words the operator remembers are in the text.
 */
export function filterAnnouncements(all, q) {
  const list = Array.isArray(all) ? all : [];
  const n = needle(q);
  if (!n) return list.slice(0, RECENT_LIMIT);
  return list.filter(
    (a) =>
      String(a?.title ?? '').toLowerCase().includes(n) ||
      String(a?.body ?? '').toLowerCase().includes(n),
  );
}

// ── The four payload builders ──────────────────────────────────────────────
//
// Each returns `{ cue_type, label, payload }` — exactly the first three arguments
// `addPlanItem` takes, in that order, so a call site cannot pass a song's payload
// under a media cue's type.

/** A verse search result → a scripture cue. */
export function verseCue(v) {
  return {
    cue_type: 'scripture',
    label: v.reference,
    payload: {
      book: v.book,
      chapter: v.chapter,
      verse: v.verse,
      reference: v.reference,
      text: v.text,
      translation: v.translation,
    },
  };
}

/**
 * A media library row → a media cue.
 *
 * `media_id` and `kind` are not decoration: `slidesOf` carries both onto the
 * slide so the Planner's inspector and Live can look the asset up and PAINT it
 * (RG — the media-preview change). A payload that carried only the filename is
 * what left every surface downstream with nothing but a name to show.
 */
export function mediaCuePayload(m) {
  return {
    cue_type: 'media',
    label: m.filename,
    payload: { media_id: m.id, kind: m.kind, filename: m.filename },
  };
}

/** An announcement → a notice cue. A blank title still gets a name. */
export function announceCuePayload(a) {
  return {
    cue_type: 'announce',
    label: a.title || 'Announcement',
    payload: { announce_id: a.id, title: a.title, body: a.body },
  };
}

/**
 * The countdown the add panel builds, and the two sentences beside it.
 *
 * Both sentences are the OPERATOR'S and both are optional — blank shows the
 * digits alone, which is what the console's own Start sends. They are trimmed
 * here rather than at the call site so an operator who typed a trailing space
 * does not put one on a wall.
 *
 * A non-positive or unreadable minute count falls back to 5 rather than storing
 * a zero: `set_plan_timer` clears anything non-positive, so a 0 would produce a
 * countdown cue with no clock and nothing saying why.
 */
export function countdownCuePayload(minutes, label, done) {
  const m = Number(minutes) > 0 ? Math.round(Number(minutes)) : 5;
  return {
    cue_type: 'countdown',
    label: `Countdown · ${m} min`,
    payload: { minutes: m, label: String(label ?? '').trim(), done: String(done ?? '').trim() },
    /** Seconds the cue seeds its own Duration with — the one cue kind that knows. */
    seconds: m * 60,
  };
}
