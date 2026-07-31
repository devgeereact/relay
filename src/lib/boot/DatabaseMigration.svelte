<script>
  // LAUNCH & STARTUP · Database Migration
  //
  // ── Why this screen does not animate a progress bar ────────────────────────
  //
  // The migration runner (src-tauri/src/db/mod.rs `run_migrations`) executes
  // ONCE, synchronously, when the connection is opened — which happens before
  // this webview exists. There is no stream to subscribe to and no percentage to
  // read. Faking one would mean drawing a bar that finishes after the work it
  // claims to be measuring already finished.
  //
  // So this screen VERIFIES the finished state instead: `migration_status` asks
  // SQLite what actually exists, via `sqlite_master` and `pragma_table_info`.
  // It is a confirmation screen, not a progress screen, and it is labelled as one.
  //
  // The first version of it asserted "already applied" from a hard-coded list —
  // it would have drawn six green ticks over a database missing every object it
  // named. Writing the real query immediately proved the point: two of the names
  // in that list did not exist (`media` is `media_assets`, and `template_active`
  // is a COLUMN, `templates.console_active`).
  //
  // The last two rows are CLAUDE.md §14 and §25 respectively: whether a human's
  // fire can be logged as manual (the router learns from that column), and
  // whether a `detections_new` scratch table was left behind — the fingerprint
  // of the rebuild failure that once made every subsequent boot fail, forever.

  import BootShell from './BootShell.svelte';
  import CheckList from './CheckList.svelte';
  import { checks, rollUp } from './boot.js';

  export let version = '';
  export let safe = false;
  export let onContinue = () => {};
  // Accepted for a uniform stage contract; only Diagnostics offers a retry.
  export let onRetry = () => {};

  $: items = $checks.migration;
  $: done = items.filter((c) => c.state !== 'pending' && c.state !== 'running').length;
  $: pct = items.length ? Math.round((done / items.length) * 100) : 0;
  $: verdict = rollUp(items);
</script>

<BootShell
  stage="migration"
  {version}
  {safe}
  footer={verdict === 'running' ? 'Verifying the database' : 'Database ready'}
>
  <p class="b-eyebrow">Step 4 of 4</p>
  <h1 class="b-h1">Local database</h1>
  <p class="b-lead">
    Transcripts, verse text, templates and service history live in one SQLite file on this
    machine. Nothing leaves it. Schema changes are applied when the file is opened — before
    this window exists — so this asks the database what it actually contains, rather than
    animating a bar over work that already finished.
  </p>

  <div class="b-prog">
    <div class="track"><div class="fill" style="transform:scaleX({pct / 100})"></div></div>
    <span class="pct">{pct}%</span>
  </div>

  <CheckList {items} />

  {#if verdict === 'fail'}
    <p class="b-lead" style="color:var(--v-red);margin-top:var(--v-sp-md);">
      The database opened but is not readable. Relay will run, but nothing can be looked up
      or saved. This is worth reporting before the service, not during it.
    </p>
  {/if}

  {#if verdict !== 'running'}
    <div class="b-actions">
      <button class="r-btn primary" on:click={onContinue}>Open the console</button>
    </div>
  {/if}
</BootShell>
