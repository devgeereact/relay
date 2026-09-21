<script>
  /**
   * THE LIBRARY INSPECTOR — what is true of the thing in hand (REBRAND §10).
   *
   * "Same grammar as Live: collections across the top, items down the left rail,
   * slides in the big grid, inspector on the right."
   *
   * ── WHAT THIS REPLACED, AND WHY ───────────────────────────────────────────
   *
   * The Library's right column used to be a second run surface: a LIVE OUTPUT ·
   * PROGRAM monitor, a HEARD panel of AI suggestions, a TRANSCRIPT feed, Go
   * Live, Listen, Clear Screens, Blank Screen, Countdown and Rehearse. Every one
   * of those already had an owner — `Live.svelte` for the programme and the
   * suggestions, the dock row for the transcript and the four controls — so
   * three surfaces answered one question and the two copies were free to drift.
   * They had: the rail's Approve and Live's Approve were two implementations of
   * the RG-63 absent-verse guard, and the rail's Listen opened the microphone
   * without the operator's chosen input device while every other caller passed
   * it.
   *
   * What the Library needed instead is what a browser needs: the thing you
   * picked, drawn the way the room will see it, and the two things you do with
   * it. The queue it uniquely owned did NOT go away: `LiveOutputRail.svelte`
   * still renders Up Next and Go Live, below this pane, and its name is more
   * accurate than it was — that file now concerns OUTPUT and nothing else.
   *
   * ── THE PREVIEW IS THE REAL RENDERER ──────────────────────────────────────
   *
   * `TemplateRender` is the one renderer (CLAUDE.md), so the frame at the top of
   * this pane is the same code that paints the projector. A preview drawn any
   * other way is a drawing of a slide rather than the slide.
   *
   * ── WHAT THE TWO BUTTONS MEAN ─────────────────────────────────────────────
   *
   * `Cue in Live` stages the item on the Up Next queue. It reaches no output —
   * CUED is grey on this console and amber is never allowed to lie — and it is
   * how a one-press take stays one press away from a selected card now that a
   * single click on a card selects rather than fires.
   *
   * `Add to plan` appends a cue to the plan the Planner has open. It is the §10
   * rule said as a control: "no add all to Live — a song joins a service through
   * the plan". When no plan is open it is DISABLED and says which screen opens
   * one, rather than silently doing nothing or guessing a plan for the operator.
   */
  import { onMount } from 'svelte';
  import TemplateRender from '../../TemplateRender.svelte';
  import { humanError } from '../../errors.js';
  import { songCue } from '../../cues.js';
  import { session } from '../../session.js';
  import { addPlanItem, getSong, listPlans, planItems } from '../../stores/capture.js';

  /**
   * The selected item, normalised by whichever pane owns it.
   *
   * `null` is a real state and has its own words: nothing is selected yet. It is
   * not an error and it is not an empty library.
   *
   * @type {{
   *   kind: 'scripture'|'song'|'notice'|'media',
   *   title: string,
   *   titleLabel: string,
   *   hotkey?: string|null,
   *   translation?: string|null,
   *   words: string,
   *   slide: { reference: string|null, text: string, translation?: string|null },
   *   media?: string|null, mediaKind?: string|null,
   *   reference?: string|null,
   *   mediaId?: number|null,
   *   plan?: { cueType: string, label: string, payload: object }|null,
   *   songId?: number|null,
   * } | null}
   */
  export let item = null;
  /** The template the OUTPUT actually uses, so the frame is the real thing. */
  export let template = null;
  /** The Up Next queue, so "Cue in Live" can say it is already there. */
  export let queue = [];
  export let onQueueChange = () => {};

  const KIND_WORD = {
    scripture: 'Scripture',
    song: 'Song',
    notice: 'Announcement',
    media: 'Media',
  };

  let plans = [];
  let cues = [];
  let busy = false;
  let msg = '';
  let err = '';

  // The plan the Planner has open. `session.planId` is the same fact the Planner
  // reads, so the two cannot disagree about which plan "the plan" is.
  $: openPlan = plans.find((p) => p.id === $session.planId) ?? null;
  $: queued = !!item?.reference && queue.some((q) => q.reference === item.reference);
  // "In this plan" is a question about THIS item, answered from the cues that are
  // actually in the plan — never from a guess about what an operator might have
  // added. An unloaded plan says so rather than saying "not yet".
  $: inPlan = !openPlan ? null : cues.some((c) => c.label === item?.plan?.label);

  onMount(async () => {
    plans = (await listPlans().catch(() => [])) ?? [];
    await refreshCues();
  });

  async function refreshCues() {
    const id = $session.planId;
    cues = id ? ((await planItems(id).catch(() => [])) ?? []) : [];
  }

  function cue() {
    err = '';
    if (!item?.reference) return;
    if (queued) {
      onQueueChange(queue.filter((q) => q.reference !== item.reference));
      msg = `${item.title} removed from Staging`;
      return;
    }
    onQueueChange([
      ...queue,
      // TAGGED (RG-185). `fireQueued` refuses an untagged row rather than guessing,
      // and this door queued four kinds with no tag, so stage-then-fire from the
      // Library threw for everything but a picture. The Inspector's own `kind`
      // vocabulary says `notice`; the fire path's says `announce`.
      {
        reference: item.reference,
        text: item.words,
        mediaId: item.mediaId ?? null,
        kind: item.kind === 'notice' ? 'announce' : item.kind,
      },
    ]);
    msg = `${item.title} is cued — it is not on a screen`;
  }

  async function addToPlan() {
    err = '';
    msg = '';
    if (!openPlan || !item?.plan) return;
    busy = true;
    try {
      let { cueType, label, payload } = item.plan;
      // A SONG is expanded here rather than stored as a reference, the same way
      // the Planner does it, and deliberately in the song's OWN order: choosing
      // an arrangement is a decision with a staleness rule behind it (rule 39,
      // DECISIONS §55) and it belongs on the surface that can show the operator
      // what they are choosing between.
      if (cueType === 'song' && item.songId != null) {
        const song = await getSong(item.songId);
        if (!song) throw new Error('That song could not be read.');
        const built = songCue(song, null);
        label = built.label;
        payload = built.payload;
      }
      await addPlanItem(openPlan.id, cueType, label, payload);
      await refreshCues();
      msg = `Added to ${openPlan.title}`;
    } catch (e) {
      err = humanError(e);
    }
    busy = false;
  }
</script>

<aside class="li rw-pane rw-insp" aria-label="Inspector">
  <header class="rw-panehead">
    <h2 class="rw-panettl">Preview</h2>
  </header>

  {#if !item}
    <div class="rw-panebody pad">
      <p class="r-empty">Select an item to preview it.</p>
    </div>
  {:else}
    <div class="rw-panebody">
      <!-- THE FRAME. Sticky, because the groups below it scroll and the whole
           point of the pane is the picture at the top of it. -->
      <div class="li-frame">
        {#if item.media}
          {#if item.mediaKind === 'video'}
            <!-- svelte-ignore a11y-media-has-caption -->
            <video src={item.media} preload="metadata" muted playsinline></video>
          {:else}
            <img src={item.media} alt="" />
          {/if}
        {:else if template}
          <TemplateRender
            {template}
            content={{
              reference: item.slide.reference,
              text: item.slide.text,
              translation: item.slide.translation ?? null,
            }} />
        {:else}
          <!-- Even with no template loaded the frame must show what the WALL
               would show. `VerseDeck`'s card makes the same promise in the same
               words: a lyric slide projects the lyric, and a verse carries its
               reference, because `slide.reference` is null for exactly the kinds
               whose label is the operator's own bookkeeping (DECISIONS §73). -->
          <span class="li-plain">
            {#if item.slide.reference}<b>{item.slide.reference}</b>{/if}
            {item.slide.text}
          </span>
        {/if}
      </div>

      <section class="li-grp">
        <h3 class="r-lbl li-grph">{KIND_WORD[item.kind] ?? 'Item'}</h3>
        <div class="rw-nv">
          <span class="rw-nvk">{item.titleLabel}</span>
          <span class="rw-nvv">{item.title}</span>
        </div>
        {#if item.hotkey}
          <div class="rw-nv">
            <!-- The key is a fact about the keyboard, printed here and on the
                 slide it fires, and on no other slide (DECISIONS §76). -->
            <span class="rw-nvk">Fires with</span>
            <span class="rw-nvv">{item.hotkey.toUpperCase()}</span>
          </div>
        {/if}
        <div class="rw-nv">
          <span class="rw-nvk">Renders as</span>
          <span class="rw-nvv">{template?.name ?? 'no template'}</span>
        </div>
        {#if item.translation}
          <div class="rw-nv">
            <span class="rw-nvk">Translation</span>
            <span class="rw-nvv">{item.translation}</span>
          </div>
        {/if}
        <div class="rw-nv">
          <span class="rw-nvk">In this plan</span>
          <!-- Three answers, not two. "not yet" over a plan nobody opened would
               be a claim from an absence (rule 35). -->
          <span class="rw-nvv">{inPlan === null ? 'no plan open' : inPlan ? 'yes' : 'not yet'}</span>
        </div>
      </section>

      <section class="li-grp">
        <h3 class="r-lbl li-grph">Words</h3>
        <p class="li-words">{item.words}</p>
      </section>

      <div class="li-acts">
        <!-- Steel blue: this is the thing you are working on, and cueing is not
             a live act. Amber would be a lie and red would be a threat. -->
        <button class="r-btn primary sm" disabled={!item.reference} on:click={cue}>
          {queued ? 'Remove from Staging' : 'Cue in Live'}
        </button>
        <button
          class="r-btn ghost sm"
          disabled={busy || !openPlan || !item.plan}
          title={openPlan ? null : 'Open a plan in Planner first'}
          on:click={addToPlan}>
          {busy ? 'Adding…' : 'Add to plan'}
        </button>
      </div>
      {#if msg}<p class="li-msg" role="status" aria-live="polite">{msg}</p>{/if}
      {#if err}<p class="li-err" role="alert">{err}</p>{/if}

      <p class="rw-foot">
        Double-click a card to open it for editing. A whole song joins a service
        through the <b>plan</b>, not straight onto Live.
      </p>
    </div>
  {/if}
</aside>

<style>
  .li { min-height: 0; }
  .li-frame {
    position: sticky; top: 0; z-index: 2;
    aspect-ratio: 16 / 9; container-type: inline-size;
    display: grid; place-items: center; overflow: hidden;
    background: var(--v-void); border-bottom: 1px solid var(--v-line);
  }
  .li-frame img, .li-frame video { width: 100%; height: 100%; object-fit: contain; }
  .li-plain { padding: 10px; font-size:var(--v-fs-b1); line-height: 1.5; color: var(--v-dim); text-align: center; }
  .li-plain b { display: block; color: var(--v-txt); }

  .li-grp { display: flex; flex-direction: column; border-bottom: 1px solid var(--v-line); }
  .li-grph { margin: 0; padding: 9px 12px 5px; }
  .li-words {
    margin: 0; padding: 0 12px 12px;
    font-size: var(--v-fs-b2); line-height: 1.55; color: var(--v-txt);
  }

  .li-acts { display: flex; gap: 6px; padding: 12px; }
  .li-acts .r-btn { flex: 1; }
  .li-msg, .li-err { margin: 0; padding: 0 12px 8px; font-size: var(--v-fs-cap); line-height: 1.5; }
  .li-msg { color: var(--v-emerald); }
  .li-err { color: var(--v-red); }
  .rw-foot { margin: 0 12px 12px; }
</style>
