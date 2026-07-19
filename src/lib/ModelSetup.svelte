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
    cancelModelDownload,
    dismissModelError,
    modelProgress,
    modelError,
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

  const mb = (b) => `${Math.round(b / 1_000_000)} MB`;
  $: pct =
    $modelProgress?.total > 0
      ? Math.min(100, Math.round(($modelProgress.downloaded / $modelProgress.total) * 100))
      : 0;
  $: installed = models.some((m) => m.installed);
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
  {:else if installed}
    <div class="ms-head"><b class="ok">Speech recognition is ready.</b></div>
  {:else}
    <div class="ms-head">
      <b>Relay can't hear the sermon yet.</b>
    </div>
    <p class="ms-sub">
      It needs a speech model — a one-time download. Everything else already works:
      you can put any verse on screen by typing its reference.
    </p>

    {#each models as m}
      <div class="ms-opt">
        <div class="ms-opt-t">
          <b>{m.label}</b>
          <span class="r-mono ms-size">{mb(m.bytes)}</span>
        </div>
        <div class="ms-opt-d">{m.detail}</div>
        <button
          class="r-btn"
          class:amber={m.recommended}
          disabled={busy}
          on:click={() => get(m.id)}>
          {m.installed ? 'Installed' : `Download ${m.recommended ? '— recommended' : ''}`}
        </button>
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
  .ms-err {
    margin-top: 10px; padding: 8px 10px; border-radius: 8px;
    background: rgba(239,68,68, 0.18); border: 1px solid rgba(239,68,68, 0.3);
    color: var(--v-red); font-size: 12px; line-height: 1.55;
    display: flex; align-items: center; gap: 10px;
  }
  .ms-err span { flex: 1; min-width: 0; }
</style>
