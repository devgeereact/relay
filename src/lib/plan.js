// Plan/cue logic, with no Svelte and no backend in it.
//
// This lived inside ServicePlanner.svelte, which meant the rules that decide
// WHAT GOES ON THE CONGREGATION'S SCREEN NEXT were welded to a component and
// could not be tested. They are now here: pure functions over a cue, so the
// run surface (Live) and the build surface (Planner) share one definition of a
// slide instead of drifting apart.
//
// The polymorphic-cue rule from db/plans.rs holds here too: every content type
// reduces to the same { tag, label, text } slide, so nothing downstream — not
// the slide grid, not the transport, not the stage monitor — branches per type.

/**
 * A TAXONOMY MAY NOT PAINT A PROMISE. (CLAUDE.md rule 18, DECISIONS §21, REBRAND §1.)
 *
 * Both tables in this file used to carry a colour per kind, and between them they
 * spent every colour the law has already spoken for. Measured on the running
 * console, on Live, with a plan open:
 *
 *   a SONG cue's stripe          #ffa31a  = --v-amber     = ON AIR
 *   a SCRIPTURE / COUNTDOWN cue  #4cc9f0  = --v-cyan      = a guess
 *   a MEDIA cue, a BG slide      #a96bf5  = --v-amethyst  = rehearsal
 *   a NOTICE cue, an OUTRO slide #f4515b  = --v-rose      = destructive
 *   a Chorus chip                #ffa31a  = --v-amber     = ON AIR
 *   a Verse chip                 #4cc9f0  = --v-cyan      = a guess
 *   a Bridge chip                #a96bf5  = --v-amethyst  = rehearsal
 *
 * All of it inches from `.slide.islive`, which signals the real ON AIR state with
 * a 15% amber wash — so the chorus chip was MORE saturated amber than a genuinely
 * live row. `Live.svelte` said so itself, five lines apart: "Amber = it is in
 * front of the congregation. Nothing else may use it", and then painted --acc.
 *
 * The fix is not a new palette. The law has taken orange, sky, violet, red and
 * neutral grey; selection has taken steel blue and emerald means healthy, which
 * leaves exactly two free hues on the wheel (magenta ~310°, lime ~80°) for five
 * content kinds and six section kinds. There is no honest ramp to invent here.
 *
 * So the taxonomy is carried by the WORDS, which were already there: every dot in
 * the Planner sits under a heading that names its kind ("Scripture", "Songs",
 * "Media", "Announcements") or beside its own label, every cue row prints
 * `ty.label`, and every section chip prints its own letter (`V1`, `C`, `BR`). The
 * colour was decoration, and it was decoration that lied. It is now one neutral,
 * the same metadata ramp `.cue-num` and `.cue-meta` already use beside it.
 *
 * If a real taxonomy ramp is ever wanted, magenta and lime are the only two gaps,
 * and it needs a designer looking at a rendered screen — not a constant edited
 * here. Do not reach for a promise colour because it is the one that reads well.
 */
export const TAXONOMY_INK = 'var(--v-faint)';

/** Cue-type presentation table. `trig` is how the cue is normally triggered. */
export const TYPE = {
  scripture: { label: 'SCRIPTURE', color: TAXONOMY_INK, trig: 'AUTO-DETECT' },
  song: { label: 'SONG', color: TAXONOMY_INK, trig: 'SUGGEST-ONLY' },
  media: { label: 'MEDIA', color: TAXONOMY_INK, trig: 'MANUAL/LOOP' },
  announce: { label: 'NOTICE', color: TAXONOMY_INK, trig: 'MANUAL/TIMER' },
  countdown: { label: 'COUNTDOWN', color: TAXONOMY_INK, trig: 'TIMER' },
  /* A cue_type this build does not know. The three surfaces that read this map
     used to fall back to `scripture`, which is the ONE type that says AUTO-DETECT
     — so an unrecognised row was presented as the only kind of cue the AI is
     allowed to fire by itself. `cue_type` is plain TEXT with no CHECK constraint,
     and `docs/data/schema.sql` still documented the notice type under a spelling
     the frontend has never used ('announcement' vs 'announce'), which is exactly
     how a row like that arrives. Say "unknown" and claim nothing. */
  unknown: { label: 'UNKNOWN', color: TAXONOMY_INK, trig: 'MANUAL' },
};

/**
 * The presentation row for a cue type — the ONE door onto `TYPE`.
 *
 * This is a choke point, not a convenience (CLAUDE.md rule 36). The fix that
 * added `unknown` above was applied at three call sites and missed a fourth,
 * `cueSub`, which is rendered on BOTH the Planner's cue inspector and the Live
 * run surface: a cue of a kind this build does not recognise was badged UNKNOWN
 * with "SCRIPTURE · AUTO-DETECT" printed two lines under it — the panel
 * contradicting itself about the one kind of cue the AI is allowed to fire by
 * itself. A guarantee is only kept on the doors you checked, so there is now one
 * door. Never fall back to `TYPE.scripture`; an unrecognised row is a claim
 * nobody made.
 */
export function typeOf(cueType) {
  return TYPE[cueType] || TYPE.unknown;
}

/** A cue's payload. Never throws — a corrupt row must not take down the console. */
export function payloadOf(item) {
  try {
    const v = JSON.parse(item?.payload_json || '{}');
    // `typeof [] === 'object'` and `typeof null === 'object'`, so neither check
    // can be dropped: a payload of `[1,2]` would otherwise be handed downstream
    // as if it were a cue body.
    return v && typeof v === 'object' && !Array.isArray(v) ? v : {};
  } catch {
    return {};
  }
}

/** The slides of a cue, normalized to { tag, label, text }. */
export function slidesOf(item) {
  if (!item) return [];
  const p = payloadOf(item);
  switch (item.cue_type) {
    case 'song':
      return (p.sections || []).map((s) => ({ tag: s.tag, label: s.label, text: s.lyrics }));
    case 'scripture':
      return [
        {
          tag: p.verse != null ? String(p.verse) : 'SCR',
          label: p.reference || item.label,
          text: p.text || '',
        },
      ];
    case 'announce':
      return [{ tag: 'NOTE', label: item.label, text: p.body || p.text || '' }];
    case 'media':
      return [{ tag: 'BG', label: item.label, text: '' }];
    case 'countdown': {
      const m = Number(p.minutes) || 5;
      return [{ tag: '⏱', label: p.label || item.label, text: `${m}:00` }];
    }
    default:
      return [];
  }
}

/**
 * Slide-chip colour — one neutral, for every tag. See TAXONOMY_INK above.
 *
 * This took a tag and returned a hue, which is why a Chorus was ON-AIR amber on
 * the run surface. The chip already prints the tag (`V1`, `C`, `BR`, `NOTE`), so
 * the letter is the taxonomy and the colour was only ever saying it twice — once
 * truthfully and once in a colour that meant something else.
 *
 * It still takes `tag` and stays the one door, so the seam survives if a law-free
 * ramp is ever chosen. A caller must not read the tag and pick its own colour.
 */
// eslint-disable-next-line no-unused-vars
export function slideAccent(tag) {
  return TAXONOMY_INK;
}

/** The one-line summary under a cue's title in the plan rail. */
export function cueSub(item) {
  const ty = typeOf(item.cue_type);
  return item.cue_type === 'song'
    ? `SONG · ${slidesOf(item).length} SLIDES`
    : `${ty.label} · ${ty.trig}`;
}

function labelled(item, slide) {
  const p = payloadOf(item);
  const label = item.cue_type === 'song' ? `${p.title} · ${slide.label}` : slide.label || item.label;
  return { label, text: slide.text || slide.label };
}

/**
 * What comes AFTER (cueId, slideIdx): the next slide in the same cue, else the
 * first slide of the next cue, else null at the end of the plan. This is what the
 * preacher reads off their stage monitor, so an off-by-one here is a preacher
 * being told the wrong thing is coming.
 */
export function nextOf(items, cueId, slideIdx) {
  const idx = items.findIndex((it) => it.id === cueId);
  if (idx < 0) return null;
  const here = items[idx];
  const slides = slidesOf(here);
  if (slideIdx + 1 < slides.length) return labelled(here, slides[slideIdx + 1]);
  const nx = items[idx + 1];
  if (!nx) return null;
  const ns = slidesOf(nx)[0];
  return ns ? labelled(nx, ns) : { label: nx.label, text: '' };
}

/**
 * Where `dir` takes the transport from (cueId, slideIdx). Returns
 * { item, slide } or null when the move runs off either end of the plan.
 *
 * A null cueId means "nothing from the plan is live" — so the first press starts
 * the plan at cue 1, rather than doing nothing. That is the case after the panic
 * keys have cleared the screens, and after the operator has taken a detour to an
 * AI-suggested verse: pressing → puts them back at the top of the plan, which is
 * the only unsurprising thing it could do.
 */
export function stepFrom(items, cueId, slideIdx, dir) {
  if (!items.length) return null;
  const idx = items.findIndex((i) => i.id === cueId);
  if (idx < 0) return { item: items[0], slide: 0 };

  const item = items[idx];
  const ns = slideIdx + dir;
  if (ns >= 0 && ns < slidesOf(item).length) return { item, slide: ns };

  const ni = idx + dir;
  if (ni < 0 || ni >= items.length) return null; // ends of the plan are hard stops
  const next = items[ni];
  // Stepping BACK into a cue lands on its LAST slide, not its first — otherwise
  // ← from the top of a chorus skips the whole verse before it.
  const slide = dir > 0 ? 0 : Math.max(0, slidesOf(next).length - 1);
  return { item: next, slide };
}

/**
 * Group an ordered cue list into the sections the Planner draws.
 *
 * A cue carrying a `section_title` BEGINS a section; the section runs until the
 * next cue that carries one. Cues before the first titled cue belong to an
 * untitled leading group (`title: ''`) — a plan is not required to start with a
 * heading, and dropping those cues on the floor would hide them from the operator.
 *
 * Returns `[{ title, items, seconds, timed }]`, where `seconds` totals only the
 * cues that have a duration and `timed` says whether every cue in the section had
 * one. Grouping is derived, never stored, so it cannot disagree with the order.
 */
export function sectionsOf(items) {
  const out = [];
  for (const it of items ?? []) {
    const title = (it.section_title || '').trim();
    if (title || out.length === 0) {
      out.push({ title: out.length === 0 && !title ? '' : title, items: [], seconds: 0, timed: true });
    }
    const sec = out[out.length - 1];
    sec.items.push(it);
    const d = Number(it.duration_sec) || 0;
    if (d > 0) sec.seconds += d;
    else sec.timed = false;
  }
  return out;
}

/**
 * A plan's total planned length in seconds, and whether it is a complete figure.
 *
 * `partial` is true when any cue is untimed — the Planner must render that as an
 * estimate rather than a total. A scripture cue fires when the preacher reaches
 * it, so most real plans are partial, and presenting a partial sum as the service
 * length is how a service runs long.
 */
export function planRuntime(items) {
  let seconds = 0;
  let partial = false;
  for (const it of items ?? []) {
    const d = Number(it.duration_sec) || 0;
    if (d > 0) seconds += d;
    else partial = true;
  }
  return { seconds, partial };
}

/**
 * Read an operator-typed cue length into seconds. Accepts `5` (minutes), `5:30`,
 * and `90s`. Returns 0 for anything it cannot read — including a blank box, which
 * is how a cue is set back to untimed.
 *
 * A bare number is MINUTES, not seconds: an operator typing "5" for a song means
 * five minutes, and reading it as five seconds would silently shrink the plan's
 * running time to nonsense.
 */
export function parseDuration(input) {
  const s = String(input ?? '').trim().toLowerCase();
  if (!s) return 0;
  const clock = s.match(/^(\d+):([0-5]?\d)$/);
  if (clock) return Number(clock[1]) * 60 + Number(clock[2]);
  const secs = s.match(/^(\d+)\s*s$/);
  if (secs) return Number(secs[1]);
  const mins = s.match(/^(\d+(?:\.\d+)?)\s*m?$/);
  if (mins) return Math.round(Number(mins[1]) * 60);
  return 0;
}

/** `m:ss` for a cue length; `1h 32m` for a whole plan. 0/absent → an em dash. */
export function fmtDuration(seconds, long = false) {
  const s = Math.max(0, Math.floor(Number(seconds) || 0));
  if (!s) return '—';
  if (long) {
    const h = Math.floor(s / 3600);
    const m = Math.round((s % 3600) / 60);
    return h ? `${h}h ${m}m` : `${m}m`;
  }
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}
