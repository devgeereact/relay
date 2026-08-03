<script>
  // Get the speech model onto this machine — the one flow that decides whether
  // Relay works at all for a real user.
  //
  // What this replaces: a banner telling a church volunteer to find a folder and
  // drop a 148 MB file into it, and a Settings line telling them to "see README
  // dev setup". The AI was, in practice, unreachable.
  import { onMount } from 'svelte';
  import {
    listModels,
    downloadModel,
    selectModel,
    cancelModelDownload,
    dismissModelError,
    modelProgress,
    modelError,
    capture,
  } from './stores/capture.js';

  export let compact = false; // banner form (Console) vs full card (Settings)

  let models = [];
  let busy = false;

  onMount(refresh);
  async function refresh() {
    models = await listModels();
  }

  async function get(id) {
    busy = true;
    try {
      await downloadModel(id);
      await refresh();
    } catch {
      /* surfaced via $modelError */
    }
    busy = false;
  }

  async function use(filename) {
    busy = true;
    // `selectModel` reports a THROWN failure through $modelError, but it can also
    // resolve `false` — the model was chosen and then would not load. Nothing is
    // thrown, so without this the badge simply vanishes and the operator is left
    // reading a list that shows nothing in use, with no explanation. A control may
    // not report a success it did not achieve (CLAUDE.md #15).
    const ok = await selectModel(filename);
    if (!ok && !$modelError) {
      modelError.set(
        'That model could not be loaded. Relay has kept the one it was already using.',
      );
    }
    await refresh();
    busy = false;
  }

  const mb = (b) => `${Math.round(b / 1_000_000)} MB`;
  $: pct =
    $modelProgress?.total > 0
      ? Math.min(100, Math.round(($modelProgress.downloaded / $modelProgress.total) * 100))
      : 0;
  $: installed = models.some((m) => m.installed);

  // Which model is ACTUALLY loaded — `stt_status.model` is a full path, and the
  // catalogue keys on filename. This is the difference between "installed" and
  // "running", and once more than one model can be installed those stop being the
  // same thing. Showing only "installed" is how an operator ends up certain they
  // are on the large model while `base` is doing the listening.
  $: activeFile = ($capture.stt?.model || '').split(/[/\\]/).pop();

</script>

<div class="ms" class:compact>
  {#if $modelProgress}
    <!-- Downloading. Show real numbers: a volunteer staring at a spinner for six
         minutes assumes it has hung. -->
    <div class="ms-head">
      <b>Downloading the speech model…</b>
      <span class="r-mono ms-pct">{pct}%</span>
    </div>
    <div
      class="ms-bar"
      role="progressbar"
      aria-valuenow={pct}
      aria-valuemin="0"
      aria-valuemax="100"
      aria-label="Speech model download">
      <i style="transform:scaleX({pct / 100})"></i>
    </div>
    <div class="ms-sub r-mono">
      {mb($modelProgress.downloaded)} of {mb($modelProgress.total || 148_000_000)}
      · you can keep using Relay while this runs
    </div>
    <button class="r-btn ghost sm" on:click={cancelModelDownload}>Cancel</button>
  {:else if installed && compact}
    <!-- The Live banner exists to get a first model onto the machine. Once one is
         here its job is done; choosing between models is a Settings job, not
         something to offer an operator mid-service. -->
    <div class="ms-head"><b class="ok">Speech recognition is ready.</b></div>
  {:else}
    <div class="ms-head">
      <b>{installed ? 'Speech model' : "Relay can't hear the sermon yet."}</b>
    </div>
    <p class="ms-sub">
      {#if installed}
        A bigger model hears more accurately but needs a faster computer. Relay uses
        the one marked <b>In use</b>.
      {:else}
        It needs a speech model — a one-time download. Everything else already works:
        you can put any verse on screen by typing its reference.
      {/if}
    </p>

    {#each models as m}
      {@const active = m.installed && m.filename === activeFile}
      <div class="ms-opt">
        <div class="ms-opt-t">
          <b>{m.label}</b>
          <span class="r-mono ms-size">{mb(m.bytes)}</span>
          {#if active}<span class="ms-live">In use</span>{/if}
        </div>
        <div class="ms-opt-d">{m.detail}</div>
        {#if m.caution}
          <!-- Not an error: nothing has gone wrong, and the operator may still
               have good reason to pick it. It must be readable BEFORE the
               download, which is why it sits above the button. -->
          <p class="ms-caution">{m.caution}</p>
        {/if}
        {#if active}
          <button class="r-btn" disabled>In use</button>
        {:else if m.installed}
          <button class="r-btn" disabled={busy} on:click={() => use(m.filename)}>
            Use this one
          </button>
        {:else}
          <button
            class="r-btn"
            class:amber={m.recommended}
            disabled={busy}
            on:click={() => get(m.id)}>
            Download {m.recommended ? '— recommended' : ''}
          </button>
        {/if}
      </div>
    {/each}
  {/if}

  {#if $modelError}
    <!-- Dismissable. It used to have no way out, so a stale failure (or, before the
         fix, the operator's own Cancel) sat in a red box until the component
         remounted — with a working Try again button sitting right underneath it. -->
    <div class="ms-err" role="alert">
      <span>{$modelError}</span>
      <button class="r-btn ghost sm" on:click={dismissModelError}>Dismiss</button>
    </div>
  {/if}
</div>

<style>
  .ms {
    background: var(--v-accent-soft);
    border: 1px solid var(--v-accent-line);
    border-radius: 11px;
    padding: 14px 16px;
    margin-top: 12px;
  }
  .ms.compact { padding: 12px 14px; }
  .ms-head { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  .ms-head b { color: var(--v-accent2); font-size: 13.5px; }
  .ms-head b.ok { color: var(--v-emerald); }
  .ms-pct { font-size: 12px; color: var(--v-txt); }
  /* --v-dim, not --v-faint: faint is ~3.4:1 and fails WCAG AA, and this is the
     text a brand-new operator most needs to be able to read. */
  .ms-sub { margin: 6px 0 0; font-size: 12.5px; color: var(--v-dim); line-height: 1.6; }
  .ms-bar {
    height: 7px; border-radius: 99px; background: var(--v-surf3);
    overflow: hidden; margin: 9px 0 6px;
  }
  /* scaleX, not width: animating width thrashes layout on every progress tick. */
  .ms-bar i {
    display: block;
    height: 100%;
    width: 100%;
    background: var(--v-accent);
    transform-origin: left center;
    transform: scaleX(0);
    transition: transform 0.25s ease-out;
  }
  @media (prefers-reduced-motion: reduce) {
    .ms-bar i { transition: none; }
  }
  .ms-opt {
    margin-top: 11px; padding-top: 11px;
    border-top: 1px solid var(--v-line);
  }
  .ms-opt-t { display: flex; align-items: baseline; gap: 8px; }
  .ms-opt-t b { font-size: 13px; color: var(--v-txt); }
  .ms-size { font-size: 11px; color: var(--v-dim); }
  .ms-opt-d { font-size: 12px; color: var(--v-dim); line-height: 1.55; margin: 3px 0 8px; }
  /* Amber, not red: nothing is broken, and the operator may still have a good
     reason to choose it. Red here would read as a failure and be clicked past. */
  .ms-caution {
    font-size: 12px; line-height: 1.55; margin: 0 0 8px;
    padding: 7px 9px; border-radius: 7px;
    background: rgba(245, 158, 11, 0.14);
    border: 1px solid rgba(245, 158, 11, 0.3);
    color: var(--v-amber, #f59e0b);
  }
  .ms-live {
    font-size: 10.5px; letter-spacing: 0.04em; text-transform: uppercase;
    padding: 2px 6px; border-radius: 99px;
    background: rgba(16, 185, 129, 0.16);
    border: 1px solid rgba(16, 185, 129, 0.32);
    color: var(--v-emerald);
  }
  .ms-err {
    margin-top: 10px; padding: 8px 10px; border-radius: 8px;
    background: rgba(239,68,68, 0.18); border: 1px solid rgba(239,68,68, 0.3);
    color: var(--v-red); font-size: 12px; line-height: 1.55;
    display: flex; align-items: center; gap: 10px;
  }
  .ms-err span { flex: 1; min-width: 0; }
</style>
