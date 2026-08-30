<script>
  import { trapFocus } from '../focus.js';
  // LAUNCH & STARTUP · Recover Previous Session
  //
  // Relay persists a tiny, content-free resume point (lib/session.js): the tab,
  // the open plan, the cue and slide, the service id. If the last run ended with
  // one of those set, the operator gets asked — once, here — whether to pick it
  // back up.
  //
  // ── The thing this screen must never do ────────────────────────────────────
  //
  // Restoring the POSITION is not the same as putting it back ON AIR. `liveOnAir`
  // is a separate fact for exactly this reason, and recovering deliberately
  // resumes the playhead with the screens CLEAR. An app that boots straight back
  // into broadcasting last week's slide, unasked, is a bug with a congregation
  // watching. So the card says, in plain words, that nothing goes on a screen.

  import { describeResume } from './boot.js';

  export let session = null;
  export let onResume = () => {};
  export let onFresh = () => {};

  $: summary = describeResume(session);
</script>

<!-- `use:trapFocus` — a boot gate is the FIRST thing a keyboard operator meets, and
     without the trap Tab walks straight out of it into an app that is not ready yet.
     `focus.js` is the one implementation; it deliberately does not bind Escape,
     which stays in `shortcuts.js`. -->
<div class="b-gate" role="dialog" aria-modal="true" aria-labelledby="b-recover-h" use:trapFocus>
  <div class="b-card">
    <p class="b-kicker" style="color:var(--v-grey);">
      <span class="d"></span>Previous session
    </p>

    <h2 id="b-recover-h">Pick up where you left off?</h2>

    <p class="lead">
      Relay remembers where you were. Resuming restores your place only —
      <strong style="color:var(--v-txt);">nothing is put back on any screen.</strong>
      You choose what goes live, as always.
    </p>

    <div class="b-facts">
      <dl>
        <dt>Resume at</dt>
        <dd class="r-mono">{summary || 'the Live tab'}</dd>
        <dt>On screen</dt>
        <dd>Nothing — outputs start clear</dd>
      </dl>
    </div>

    <div class="b-btn-row">
      <button class="r-btn primary" on:click={onResume}>Resume</button>
      <button class="r-btn ghost" on:click={onFresh}>Start fresh</button>
      <span class="b-spring"></span>
    </div>

    <p style="margin:16px 0 0;font-size:12px;color:var(--v-faint);">
      Starting fresh forgets the position. It does not delete the service, the plan, or
      anything in your library.
    </p>
  </div>
</div>
