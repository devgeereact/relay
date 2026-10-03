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
 *
 * ── 2026-09-16: asked again, and refused again ───────────────────────────────
 *
 * Wave 4's design proposed exactly the thing the paragraph above forbids, and it
 * is recorded here so the next reader knows it was CONSIDERED rather than never
 * raised. The proposal was to colour cues from the `--v-col-*` family in
 * `app.css` — already kind-mapped, already used this way in the Library — plus a
 * new `--v-col-timer`, on the argument that the Planner is never on air.
 *
 * It was examined and refused on 2026-09-16, on the operator's decision. The
 * argument does not survive one lookup: `--v-col-scripture` is `var(--v-amber)`
 * (`app.css:222` → `:169`), and amber means ON AIR. Reusing it would put the
 * live colour on a SCRIPTURE cue, which is the one kind the AI may fire by
 * itself — the exact row the measurement above was taken from. `--v-col-media`
 * is amethyst, which means rehearsal, and that is the second one. "The Planner
 * is never on air" explains why the Library gets away with it; it does not make
 * the colour mean something else on the surface an operator reads on a Sunday,
 * and the Planner's rows are rendered beside Live's in the same session.
 *
 * `colourlaw.test.js` holds the floor, and the paragraph that used to sit here
 * described a hole in it that has since been closed — the description is kept
 * because the hole is worth understanding. Its sweep over every `TYPE` entry was
 * a SUBSTRING match on the token text, so `var(--v-col-scripture)` sailed
 * through: the indirection was invisible to the scanner even though it resolves
 * to amber. What actually caught a ramp was the identity assertion, and that
 * covered `song` and `slideAccent('C')` and nothing else. A ramp built from
 * `--v-col-*` and left off `song` would have passed the whole file while painting
 * ON AIR amber on an auto-detect cue.
 *
 * **It does not sail through any more.** On 2026-09-20 `promiseIn` was made to
 * RESOLVE: it reads `tokens.css` and `app.css`, follows a `var()` chain to its
 * literal, and reports the whole path — `--v-col-scripture → --v-amber (ON AIR)`.
 * A third sweep reads every `var(--…)` in the CODE of this file, so a table added
 * later is covered on arrival rather than when somebody remembers to list it.
 * Both were watched to fail by pointing `TYPE.media` at `--v-col-media`.
 *
 * **The test is still not the reason to refuse a per-kind ramp; the meaning of
 * the colour is.** What the closed gap changes is only that reaching for one is
 * now loud instead of silent.
 *
 * ── 2026-09-20: SECTIONS, which is a different question ──────────────────────
 *
 * The operator asked for the running order to be colour coded "section by
 * section". That is not the refusal above wearing a new coat: a section is a
 * POSITION in the plan, not a kind of content and not a state, so a band on it
 * claims nothing about what a cue is or what a screen is doing. It is answered
 * with the two free hues this paragraph names — see `sectionBands` below for the
 * system, what happens when a plan has more sections than hues, and why colour is
 * never the only signal there.
 */
export const TAXONOMY_INK = 'var(--v-faint)';

/**
 * Cue-type presentation table. `trig` is how the cue is normally triggered;
 * `chip` is the short word the running order prints in a row's kind chip.
 *
 * `chip` is written out per kind on purpose. The prototype's chip fell back to
 * `kind.slice(0,4)`, and the first kind added after that read "LOWE" in every
 * running order — a truncation is a name nobody chose, and the row is the one
 * place an operator reads the kind at a glance. There is no rule generating these
 * five words; if a sixth cue type arrives it gets a word here, and until it does
 * `chipOf` prints its own name in full rather than a slice of it.
 */
export const TYPE = {
  scripture: { label: 'SCRIPTURE', chip: 'WORD', color: TAXONOMY_INK, trig: 'AUTO-DETECT' },
  song: { label: 'SONG', chip: 'SONG', color: TAXONOMY_INK, trig: 'SUGGEST-ONLY' },
  media: { label: 'MEDIA', chip: 'MEDIA', color: TAXONOMY_INK, trig: 'MANUAL/LOOP' },
  announce: { label: 'NOTICE', chip: 'NOTE', color: TAXONOMY_INK, trig: 'MANUAL/TIMER' },
  countdown: { label: 'COUNTDOWN', chip: 'TIMER', color: TAXONOMY_INK, trig: 'TIMER' },
  /* A cue_type this build does not know. The three surfaces that read this map
     used to fall back to `scripture`, which is the ONE type that says AUTO-DETECT
     — so an unrecognised row was presented as the only kind of cue the AI is
     allowed to fire by itself. `cue_type` is plain TEXT with no CHECK constraint,
     and `docs/data/schema.sql` still documented the notice type under a spelling
     the frontend has never used ('announcement' vs 'announce'), which is exactly
     how a row like that arrives. Say "unknown" and claim nothing. */
  unknown: { label: 'UNKNOWN', chip: 'UNKNOWN', color: TAXONOMY_INK, trig: 'MANUAL' },
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

/**
 * The word a running-order row prints in its kind chip. Never a slice.
 *
 * A `cue_type` this build does not know prints its OWN name, in full and in
 * capitals — quoting the row rather than guessing at it, which is the same
 * discipline as `typeOf` answering UNKNOWN instead of falling back to scripture.
 * A row with no cue_type at all has nothing to quote, so it says UNKNOWN.
 */
export function chipOf(cueType) {
  const known = TYPE[cueType];
  if (known) return known.chip;
  const raw = typeof cueType === 'string' ? cueType.trim() : '';
  return raw ? raw.toUpperCase() : TYPE.unknown.chip;
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
    // A media slide has no words and never will — but it does have an ASSET, and
    // dropping the id here is what left every surface downstream with nothing but
    // a filename to show. `media_id`/`media_kind` ride along so a caller can look
    // the row up; `text` stays empty because there is still nothing to typeset.
    case 'media':
      return [
        {
          tag: 'BG',
          label: item.label,
          text: '',
          media_id: p.media_id ?? null,
          media_kind: p.kind || 'image',
        },
      ];
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
 * A SECTION BEGINS WHERE THE SECTION CHANGES. A cue whose `section_title` is
 * empty continues the section above it, and so does one that repeats the title
 * already open — which is the correction this function needed.
 *
 * The rule used to be "any cue carrying a title begins a section", which is the
 * convention `db/plans.rs` documents and exactly one of the two the data uses.
 * The other is the obvious one: every cue records the section it is IN, which is
 * how a plan looks after an import, after a duplicate, and after an operator has
 * typed the same heading into two consecutive cues. Under the old rule that plan
 * became one group per cue, and the running order rendered EIGHT headings over
 * eight cues — `GATHERING / Welcome & notices`, `GATHERING / Great Is Thy
 * Faithfulness` — for a service with four sections in it. A heading that repeats
 * on every row is not a heading; it is a column, and a noisy one.
 *
 * Comparing against the OPEN GROUP rather than against the previous row is what
 * makes both conventions land on the same four groups: a run of empty titles does
 * not close the section, so a titled cue after one of them is still inside it.
 *
 * Cues before the first titled cue belong to an untitled leading group
 * (`title: ''`) — a plan is not required to start with a heading, and dropping
 * those cues on the floor would hide them from the operator.
 *
 * Returns `[{ title, items, seconds, timed }]`, where `seconds` totals only the
 * cues that have a duration and `timed` says whether every cue in the section had
 * one. Grouping is derived, never stored, so it cannot disagree with the order.
 */
export function sectionsOf(items) {
  const out = [];
  for (const it of items ?? []) {
    const title = (it.section_title || '').trim();
    const open = out[out.length - 1];
    if (!open || (title && title !== open.title)) {
      out.push({ title, items: [], seconds: 0, timed: true });
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
 * SECTION BANDING — the running order, coloured section by section.
 *
 * ── WHAT THE COLOURS MEAN, AND WHAT THEY DELIBERATELY DO NOT ────────────────
 *
 * There are TWO, they alternate, and that is the whole system:
 *
 *   section 1 · magenta   `--v-sec-a`   (hue 314°)
 *   section 2 · lime      `--v-sec-b`   (hue  81°)
 *   section 3 · magenta … and so on, repeating every two.
 *
 * A band says ONE thing: **this is a different section from the one above it.**
 * It is not a kind of content, not a state, not urgency, not whether a cue is
 * timed. Nothing in this running order changes colour because of what happens to
 * it — the colours a running service needs are spoken for, and a build surface
 * borrowing one is the defect the top of this file refuses twice.
 *
 * ── WHY TWO, AND WHAT HAPPENS AT SECTION SEVEN ──────────────────────────────
 *
 * Two is not a first instalment. The law has taken orange (ON AIR), red
 * (destructive), sky (a guess), violet (rehearsal), steel (selection), green
 * (healthy) and neutral grey (CUED); magenta near 310° and lime near 80° are
 * what is left on the wheel, and the paragraph at the top of this file says so
 * with the measurements. A third hue does not exist to be found.
 *
 * So the palette REPEATS. Section 7 wears magenta, exactly as sections 1, 3 and
 * 5 do. That is a deliberate answer rather than a limit worked around: banding
 * two colours down a list is how a reader sees where one group ends and the next
 * begins, and it makes no claim that section 7 and section 1 are related. What
 * makes a section ITSELF is its ordinal and its name, both of which are printed.
 * Inventing a seventh hue would mean either a colour the law has spoken for, or
 * a hue sitting 20° from one — a magenta that is nearly rose, a lime that is
 * nearly amber — which is worse than repeating, because a near-miss reads AS the
 * promise colour at a glance and under a projector's light.
 *
 * ── COLOUR IS NEVER THE ONLY SIGNAL ─────────────────────────────────────────
 *
 * Same reason a paraphrase shows no percentage (rule 18): a channel that can be
 * absent may not be the only channel. `ordinal` is printed in the heading, the
 * heading prints the section's name, and a cue row's left edge is a SHAPE whose
 * presence — not its hue — says the row is inside a numbered section. Read in
 * greyscale, or by an operator who cannot separate magenta from lime, the
 * running order loses the banding and nothing else.
 *
 * ── AN UNTITLED GROUP IS NOT A SECTION ──────────────────────────────────────
 *
 * `sectionsOf` opens an untitled leading group for the cues before the first
 * heading, so that they are not dropped on the floor. Those get `null` here: no
 * number, no ink, no edge. Numbering a group the operator never named would be
 * inventing a section and then colouring the invention, which is a claim from an
 * absence — the same mistake as an empty `built_shape` being called stale.
 *
 * Returns an array PARALLEL to `sections`: `null`, or
 * `{ ordinal, ink, soft, line }`. Parallel rather than folded into `sectionsOf`
 * because grouping is a fact about the plan and banding is a fact about how it
 * is drawn, and the Planner is not the only surface that groups a plan.
 */
export const SECTION_BANDS = [
  { ink: 'var(--v-sec-a)', soft: 'var(--v-sec-a-soft)', line: 'var(--v-sec-a-line)' },
  { ink: 'var(--v-sec-b)', soft: 'var(--v-sec-b-soft)', line: 'var(--v-sec-b-line)' },
];

export function sectionBands(sections) {
  let ordinal = 0;
  return (sections ?? []).map((sec) => {
    if (!sec || !String(sec.title || '').trim()) return null;
    ordinal += 1;
    return bandForOrdinal(ordinal);
  });
}

/**
 * The band a single section wears, by its ordinal among the TITLED sections.
 *
 * The one door onto the cycle (`SECTION_BANDS`), so a caller cannot index the
 * array itself and get the modulo wrong at the wrap. A non-positive or
 * unreadable ordinal is not a section and answers `null` rather than silently
 * taking the first colour — an off-by-one that painted every section magenta
 * would look exactly like a working feature.
 */
export function bandForOrdinal(ordinal) {
  const n = Number(ordinal);
  if (!Number.isFinite(n) || n < 1) return null;
  const i = (Math.trunc(n) - 1) % SECTION_BANDS.length;
  return { ordinal: Math.trunc(n), ...SECTION_BANDS[i] };
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

/**
 * Where a pointer-drag of `dy` pixels, started on the row at `from`, lands.
 *
 * The arithmetic of the running order's drag lives here rather than in the
 * component because it is the half that can be wrong: a drag that lands one row
 * off, or that runs past the end of the plan and throws the cue away, is a
 * Tuesday-evening reorder that silently is not the order the operator saw. The
 * component owns the transforms; this owns the index.
 *
 * `rowHeight` is the measured row height and may be 0 (an unlaid-out list, and
 * jsdom always) — a divide by zero would yield `Infinity` and then `NaN`, so a
 * non-positive height means nothing moved.
 */
export function dropIndex(from, dy, rowHeight, count) {
  const h = Number(rowHeight) || 0;
  if (!count || h <= 0) return from;
  const shift = Math.round((Number(dy) || 0) / h);
  return Math.max(0, Math.min(count - 1, from + shift));
}

/** The list with the item at `from` moved to `to`. Never mutates its argument. */
export function reorderTo(items, from, to) {
  const arr = (items ?? []).slice();
  if (from < 0 || from >= arr.length || to < 0 || to >= arr.length || from === to) return arr;
  const [moved] = arr.splice(from, 1);
  arr.splice(to, 0, moved);
  return arr;
}

/**
 * What the plan rail prints for a plan's date — in WORDS when there is no date.
 *
 * `PlanSummary.plan_date` is a `String` in Rust and a plan created in-app always
 * carries today's date, so an absent one is the unusual case: a row written by an
 * older build, an import, or a hand-edited database. That is exactly the case a
 * rail row must not garble. Two failures this closes, both of rule 35's family —
 * a line that says the same thing whether or not the thing behind it worked:
 *
 *   `{p.plan_date}`            → the literal word `undefined`, in a rail of plans
 *   `{p.plan_date || '—'}`     → an em dash, which in this repository already
 *                                means "untimed cue" (`fmtDuration`), so a dateless
 *                                plan would read as a cue length.
 *
 * Say the absence. `No date` is a fact about the plan; `undefined` is a fact about
 * the frontend leaking onto a screen an operator is reading.
 */
export function planDateLabel(date) {
  const s = typeof date === 'string' ? date.trim() : '';
  return s || 'No date';
}

/**
 * What the plan rail prints for a plan's cue count.
 *
 * The count is `i64` in `PlanSummary` and cannot be absent from the real backend —
 * which is the whole reason a missing one has to be said out loud rather than
 * interpolated: `{p.cue_count} cues` renders `undefined cues`, and the one place
 * that string can appear is a build where the shape the frontend expects and the
 * shape the backend sends have come apart. A zero would be a LIE about a plan that
 * may be full; the honest answer is that this row does not know.
 */
export function cueCountLabel(n) {
  const v = typeof n === 'number' && Number.isFinite(n) ? Math.max(0, Math.trunc(n)) : null;
  if (v == null) return 'Cue count unknown';
  return `${v} cue${v === 1 ? '' : 's'}`;
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

/**
 * What the cue inspector's preview can honestly say about a cue.
 *
 * The panel used to print ONE sentence — "No text to preview" — over the chequered
 * plate for every cue whose `previewContent.text` was empty, and that sentence read
 * exactly the same in four situations that are not the same news (CLAUDE.md rule
 * 35: a status line that says the same thing when the thing behind it is broken as
 * when it is fine is not a status line):
 *
 *   a MEDIA cue          the slide IS the picture; there is nothing to typeset  — fine
 *   a COUNTDOWN cue      the clock is drawn when it fires, not now              — fine
 *   a SCRIPTURE / SONG /
 *     NOTICE cue with no
 *     words saved        it WOULD reach the screen and there is nothing on it   — BROKEN
 *   an unrecognised
 *     `cue_type`         this build cannot say what it renders                  — unknown
 *
 * The third is the one worth a Tuesday evening: a cue that will be reached during
 * a service and put nothing in front of the congregation. Under the old sentence
 * it was indistinguishable from a countdown behaving correctly.
 *
 * `plate` is whether the chequered ground is drawn. The chequer exists to make a
 * KEYED template visible (see `.sp-preview`) — it is a statement about a rendered
 * slide, so a cue with no slide to render gets words instead of an empty plate.
 *
 * A MEDIA CUE IS A FIFTH SITUATION, and it is why `media` was added rather than
 * an `{#if}` in the view. "The slide is the picture" was true and useless: the
 * picture can be SHOWN, because `TemplateRender` already paints `media_url` +
 * `media_kind`, and nothing upstream ever handed it either field for a plan cue.
 * Once the caller looks the asset up, two answers are possible and they are not
 * the same news — the row is there and the thumbnail IS the preview, or the row
 * has been deleted out from under a cue that still points at it, which is the
 * third situation above wearing a different coat. Naming the file is the whole
 * value of the second: "a picture is missing" tells an operator nothing they can
 * act on, and the cue's own label is often just `Background`.
 *
 * `media` is what the caller found: `{ found, filename }`, or nothing at all when
 * no lookup was made. Absent, the four verdicts above are exactly as they were —
 * the argument is additive on purpose, because this function's existing answers
 * are themselves pinned.
 *
 * `plate` is whether the chequered ground is drawn. The chequer exists to make a
 * KEYED template visible (see `.sp-preview`) — it is a statement about a rendered
 * slide, so a cue with no slide to render gets words instead of an empty plate.
 *
 * Pure, and here rather than in the component, because this is a rule about what
 * may be claimed and rules of that shape in this file are the ones that get tested.
 */
/**
 * What the run surface says beside a song cue whose arrangement went stale
 * (RG-203, 2026-09-21). Rule 39: an arrangement built against a shape the song
 * no longer has is "shown as needing checking" — and it was, on the Planner,
 * which is a Tuesday surface. This is the sentence for Sunday's. Empty when
 * there is nothing to say, so the header prints nothing rather than a dash.
 */
export function staleNote(item) {
  if (!item || item.cue_type !== 'song') return '';
  let stale = false;
  try {
    stale = !!JSON.parse(item.payload_json || '{}')?.arrangement_stale;
  } catch {
    stale = false;
  }
  return stale ? 'arrangement needs checking — the song changed since it was built' : '';
}

export function previewState(item, hasText, media) {
  if (!item) return { state: 'none', plate: false, message: '' };
  if (hasText) return { state: 'render', plate: true, message: '' };
  const known = TYPE[item.cue_type];
  if (item.cue_type === 'media') {
    // Nothing was looked up — the caller cannot say, so neither may this.
    if (!media) {
      return { state: 'self', plate: false, message: 'The slide is the picture — media plays full-frame.' };
    }
    if (media.found) {
      // THE CODEC WARNING (F5, 2026-09-21). The projector's own window decodes an
      // iPhone's HEVC; an OBS browser source or a Windows screen may paint
      // nothing, and until the beat carried a media failure nothing said so. The
      // cue still renders here — the warning is read where the cue is built.
      const warning =
        media.codec === 'hevc'
          ? `“${media.filename ?? 'this clip'}” is HEVC (H.265). It plays in Relay's own output window and on a Mac; an OBS browser source or a Windows screen may show nothing. Convert it to H.264 to be safe.`
          : '';
      return { state: 'render', plate: true, message: '', warning };
    }
    const named = media.filename ? `“${media.filename}”` : 'the file it was built from';
    return {
      state: 'empty',
      plate: false,
      message: `This media cue points at ${named}, which is no longer in the media library, so firing it would put nothing on the screen.`,
    };
  }
  if (item.cue_type === 'countdown') {
    return { state: 'self', plate: false, message: 'The clock is drawn when this cue fires, so there is nothing to show yet.' };
  }
  if (!known) {
    return {
      state: 'unknown',
      plate: false,
      message: 'This build does not recognise this kind of cue, so it cannot say what it would put on the screen.',
    };
  }
  return {
    state: 'empty',
    plate: false,
    message: `This ${known.label.toLowerCase()} cue has no words saved, so firing it would put nothing on the screen.`,
  };
}


/**
 * WHICH SCREENS A CUE IS FOR (RG-161), read off `plan_items.channels_json`.
 *
 * `null` is EVERY screen and is what every cue written before targeting
 * existed carries — so a plan that has never been told about screens behaves
 * exactly as it always did. A value that will not parse is also every screen:
 * content that silently reaches nothing is worse than content that reaches
 * more than it had to.
 *
 * An EMPTY array survives as an empty array. "Reaches no screen" and "reaches
 * every screen" are opposite instructions, and this is the one place the
 * difference could quietly be lost.
 */
export function planChannelsOf(raw) {
  if (raw == null) return null;
  try {
    const v = JSON.parse(raw);
    if (!Array.isArray(v)) return null;
    return v.filter((n) => Number.isFinite(n));
  } catch {
    return null;
  }
}
