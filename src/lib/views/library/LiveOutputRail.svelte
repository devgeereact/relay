<script>
  // THE LIVE-OUTPUT RAIL — the one thing in the Library that can reach a screen:
  // the queue, in order, and the press that fires the top of it.
  //
  // ── WHAT THIS PANE USED TO BE, AND WHY FOUR PANELS LEFT IT ────────────────
  //
  // It was a second run surface inside a browsing workspace: a LIVE OUTPUT ·
  // PROGRAM monitor, a HEARD panel of AI suggestions, a TRANSCRIPT feed, and a
  // block of Listen / Clear Screens / Blank Screen / Countdown / Rehearse.
  // `docs/REBRAND.md` §10 asks the Library's right column to be an INSPECTOR for
  // the selected item, and every one of those four panels already had an owner:
  //
  //   the programme monitor and the suggestions   → `Live.svelte`, the run surface
  //   the transcript                              → the dock row, app-wide
  //   Clear / Blackout / Rehearse / Countdown     → `Dock.svelte`, app-wide
  //   Listen                                      → Live, Settings, Dashboard
  //
  // Three surfaces answering one question is how two of them come to disagree,
  // and these two already had: this rail carried its own copy of the RG-63
  // absent-verse guard beside Live's, and its Listen opened the microphone with
  // NO device argument while every other caller passed the operator's chosen
  // input. Neither was visible from either side. Removing the duplicates removes
  // the drift, and nothing became unreachable — every command behind them is
  // still one press away on a surface that owns it.
  //
  // ── WHAT DID NOT LEAVE ────────────────────────────────────────────────────
  //
  // Up Next. Nothing else in Relay renders the queue, reorders it, clears it or
  // fires from it, and five Library panes have an "Add to queue" control feeding
  // it. Deleting this pane would have turned all five into controls that do
  // nothing — the exact failure `VerseDeck`'s `can.select` note describes.
  //
  // ── THE NAME ──────────────────────────────────────────────────────────────
  //
  // Kept, and it is more accurate now than it was: this is the Library's rail
  // that concerns OUTPUT, and it no longer also concerns listening, transcribing
  // or what the AI thinks it heard.
  import { humanError } from '../../errors.js';
  import { safeMode } from '../../boot/boot.js';
  import { move, dequeue, take, clear as clearAll } from '../../queue.js';

  export let queue = [];
  export let onQueueChange = () => {};
  export let onFireQueued = () => {};

  let error = '';
  let msg = '';
  /**
   * IS A TAKE ALREADY IN FLIGHT?
   *
   * `onQueueChange(rest)` runs AFTER the await, which is correct — a fire that
   * failed must leave the item in Up Next — and it is also what made the second
   * press dangerous: until the first one resolves, `queue` still has the same item
   * at its head, so pressing twice takes the SAME verse twice and neither press
   * advances. Two broadcasts, two `manual_fire` rows, and a router that calibrates
   * itself from that column (CLAUDE.md rule 14).
   *
   * This is not a panic control, so gating it is allowed. Clear screens and
   * Blackout are never gated and are not on this rail (rule 15, DECISIONS §20).
   */
  let taking = false;

  const moveInQueue = (ref, d) => onQueueChange(move(queue, ref, d));
  const dropFromQueue = (ref) => onQueueChange(dequeue(queue, ref));

  async function goLive() {
    if (taking) return;
    error = '';
    msg = '';
    // The QUEUE is the staging area — "Up Next" is a switcher that holds N items
    // rather than one. The `preview` prop this used to check first had no producer
    // in the shipping app and is gone (audit P1-2); see Library.svelte for why the
    // AI path stayed at one press.
    const { item, rest } = take(queue);
    if (!item) {
      msg = 'Nothing staged and nothing queued.';
      return;
    }
    taking = true;
    try {
      await onFireQueued(item);
      onQueueChange(rest);
    } catch (e) {
      error = humanError(e);
    }
    taking = false;
  }
</script>

<aside class="lo rw-pane" aria-label="Up Next">
  <header class="rw-panehead">
    <h2 class="rw-panettl">Up Next</h2>
    <span class="rw-spring"></span>
    {#if queue.length}
      <span class="r-mono lo-count">{queue.length}</span>
      <button class="r-btn quiet sm" on:click={() => onQueueChange(clearAll())}>Clear</button>
    {/if}
  </header>

  <div class="rw-panebody">
    {#if !queue.length}
      <p class="lo-none">
        Nothing queued. <b>Cue in Live</b> on a selected item stages it here — cueing
        reaches no screen, and the queue survives a change of collection.
      </p>
    {:else}
      <ol class="lo-queue">
        {#each queue as item, i (item.reference)}
          <li class="lo-q" class:next={i === 0}>
            <span class="lo-qn r-mono">{i + 1}</span>
            <span class="lo-qc">
              <b>{item.reference}</b>
              {#if item.text}<span>{item.text}</span>{/if}
            </span>
            <span class="lo-qacts">
              <button class="lo-ic r-focus" aria-label="Move up" disabled={i === 0} on:click={() => moveInQueue(item.reference, -1)}>↑</button>
              <button class="lo-ic r-focus" aria-label="Move down" disabled={i === queue.length - 1} on:click={() => moveInQueue(item.reference, 1)}>↓</button>
              <button class="lo-ic r-focus" aria-label="Remove" on:click={() => dropFromQueue(item.reference)}>×</button>
            </span>
          </li>
        {/each}
      </ol>
    {/if}
  </div>

  <footer class="rw-panefoot lo-quick">
    <!-- AMBER: this is the one control in the Library that puts something in
         front of people, and it is the only amber on this workspace. -->
    <button
      class="r-btn amber lo-golive"
      disabled={$safeMode || !queue.length || taking}
      on:click={goLive}>
      {taking ? 'Sending…' : queue.length ? `Go Live — ${queue[0].reference}` : 'Go Live'}
    </button>

    <!-- Announced. "John 3:16 is on the screens" is the confirmation that content
         reached a congregation, and it was silent to a screen reader — the error
         half of these panes carries `role="alert"` and this half carried nothing. -->
    {#if msg}<p class="lo-msg" role="status" aria-live="polite">{msg}</p>{/if}
    {#if error}<p class="lo-err" role="alert">{error}</p>{/if}
  </footer>
</aside>

<style>
  .lo { min-height: 0; }
  .lo-count { font-size: var(--v-fs-cap); color: var(--v-faint); }
  /* CONVERTED — B2. "Clear" was `.lo-link`: accent-coloured text with no box and
     no hover, sitting in a pane head. It is `.r-btn.quiet` — no fill, no edge,
     but the button's metrics and a hover fill, which is how an operator finds
     out it is pressable before pressing it. Nothing is left, so the rule is
     gone rather than emptied. */

  .lo-none {
    margin: 0;
    padding: 12px;
    font-size: var(--v-fs-cap);
    line-height: 1.6;
    color: var(--v-faint);
  }
  .lo-none b { color: var(--v-dim); font-weight: 600; }

  .lo-queue {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .lo-q {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--v-line);
  }
  .lo-q:last-child { border-bottom: 0; }
  /* The NEXT item is steel — it is the thing you are working on, and it is not on
     a screen. Grey and amber both already mean something else here. */
  .lo-q.next { box-shadow: inset 2px 0 0 var(--v-sel); background: var(--v-sel-soft); }
  .lo-qn { flex: 0 0 auto; font-size: var(--v-fs-b3); color: var(--v-faint); padding-top: 2px; }
  .lo-qc { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .lo-qc b { font-size: var(--v-fs-b2); font-weight: 600; color: var(--v-txt); }
  .lo-qc span {
    font-size: var(--v-fs-cap);
    color: var(--v-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lo-qacts { flex: 0 0 auto; display: flex; gap: 2px; }
  /* A ROW AFFORDANCE, not a button — B2. Up · down · remove at the right end of
     a queue row. 22px, because three shared icon buttons would be taller than
     the row they sit in and would turn a scannable queue into a stack of
     toolbars. Kept deliberately; not drift. */
  .lo-ic {
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border: 1px solid var(--v-line2);
    border-radius: var(--v-r-sm);
    background: var(--v-surf2);
    color: var(--v-dim);
    font-size:var(--v-fs-b1);
    cursor: pointer;
  }
  .lo-ic:hover:not(:disabled) { color: var(--v-txt); border-color: var(--v-sel-line); }
  .lo-ic:disabled { opacity: 0.4; cursor: not-allowed; }

  .lo-quick { overflow: visible; }
  .lo-golive { width: 100%; height: 34px; }
  .lo-msg, .lo-err { margin: 0; font-size: var(--v-fs-cap); line-height: 1.5; }
  .lo-msg { color: var(--v-emerald); }
  .lo-err { color: var(--v-red); }
</style>
