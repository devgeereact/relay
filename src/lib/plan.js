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

/** Cue-type presentation table. `trig` is how the cue is normally triggered. */
export const TYPE = {
  scripture: { label: 'SCRIPTURE', color: 'var(--v-cyan)', trig: 'AUTO-DETECT' },
  song: { label: 'SONG', color: 'var(--v-amber)', trig: 'SUGGEST-ONLY' },
  media: { label: 'MEDIA', color: 'var(--v-amethyst)', trig: 'MANUAL/LOOP' },
  announce: { label: 'NOTICE', color: 'var(--v-rose)', trig: 'MANUAL/TIMER' },
  countdown: { label: 'COUNTDOWN', color: 'var(--v-cyan)', trig: 'TIMER' },
};

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

/** Slide-group colour. Matches the Song Editor so a chorus is the same colour everywhere. */
export function slideAccent(tag) {
  const t = (tag || '').toUpperCase();
  if (/^\d+$/.test(t)) return 'var(--v-faint)';
  if (t.startsWith('PC')) return 'var(--v-emerald)';
  if (t.startsWith('V')) return 'var(--v-cyan)';
  if (t.startsWith('BR') || /^B\d?$/.test(t)) return 'var(--v-amethyst)';
  if (t.startsWith('C')) return 'var(--v-amber)';
  if (t.startsWith('INT') || t.startsWith('IL')) return 'var(--v-emerald)';
  if (t.startsWith('OUT') || t.startsWith('END') || t.startsWith('TAG') || t.startsWith('REF'))
    return 'var(--v-rose)';
  if (t === 'NOTE') return 'var(--v-rose)';
  if (t === 'BG') return 'var(--v-amethyst)';
  return 'var(--v-cyan)';
}

/** The one-line summary under a cue's title in the plan rail. */
export function cueSub(item) {
  const ty = TYPE[item.cue_type] || TYPE.scripture;
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
