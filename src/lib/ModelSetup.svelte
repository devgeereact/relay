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
    findModelFiles,
    installModelFile,
    readErrors,
  } from './stores/capture.js';
  import ErrorState from './ui/ErrorState.svelte';
  import Button from './ui/Button.svelte';
  import { whyDisabled, SERVICE_LOCKED, BUSY } from './ui/whydisabled.js';
  import { serviceLock } from './stores/capture.js';
  import { humanError } from './errors.js';

  export let compact = false; // banner form (Console) vs full card (Settings)

  let models = [];
  let busy = false;

  // ── OFFLINE (RG-19) ───────────────────────────────────────────────────────
  //
  // Everything else a church needs works without a network — the app is an
  // installer, the KJV is compiled in, the templates are seeded — and the 148 MB
  // speech model could only ever arrive over a connection they do not have. So
  // Relay looks for one already on the machine: Downloads, its own data folder, and
  // the model folder. Three directories, no recursion, and a size check before
  // anything is hashed.
  let found = [];
  let installMsg = '';

  onMount(refresh);
  async function refresh() {
    models = await listModels();
    found = await findModelFiles();
  }

  async function installFound(f) {
    busy = true;
    installMsg = '';
    try {
      await installModelFile(f.path);
      installMsg = `Installed ${f.label}.`;
    } catch (e) {
      // THROUGH THE ONE HUMANISER, like its six siblings on this surface. This
      // read `e?.message ?? String(e)`, which renders a Rust error verbatim.
      //
      // AND THE VOLUNTEER COPY SURVIVES, for a reason worth writing down because
      // it is not obvious and it could be undone by accident. `install_from_file`
      // itself returns `Result<String, String>`, but the COMMAND wraps it:
      // `main.rs::install_model_file` ends `.map_err(error::Error::refused)`, so
      // what crosses the bridge is `{ kind: 'refused', message }`, and
      // `humanError` returns a refusal's message untouched. That matters more
      // than it looks. Two of this function's failures interpolate an OS error —
      // "Could not copy the model: No such file or directory (os error 2)" — and
      // if the typed wrapper were ever dropped, that bare string matches
      // `errors.js`'s `/no such file/` pattern and would be REPLACED with "The
      // speech model is not on this machine yet. Download it from Settings.":
      // advice to download, in the flow a church uses precisely because it
      // cannot. `Permission denied` would likewise become firewall guidance.
      // Pinned by `errors.test.js` — "the offline model install".
      installMsg = humanError(e);
    }
    await refresh();
    busy = false;
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

  // ── THE SERVICE LOCK IS VISIBLE HERE NOW, AND IT WAS NOT ──────────────────
  //
  // Every write on this panel is guarded in Rust: `select_stt_model`,
  // `download_model` and `install_model_file` all call `lock.guard(...)`, which
  // returns a REFUSED error — the typed kind that means pressing again will not
  // help. That guard is right and it is the whole reason this panel exists in the
  // state it does: the 2026-09-06 service changed model mid-service and produced a
  // 44% wrong-verse rate, which is the event the lock was written for.
  //
  // What was missing was any sign of it BEFORE the press. The cards carried no
  // `disabled` and no reason, so an operator pressed `Use this one` during a
  // service and received a backend error for an outcome the interface already
  // knew. A refusal an operator could have been spared is a refusal that reads as
  // a fault, on the panel that decides how well Relay hears.
  //
  // `SERVICE_LOCKED` is `whydisabled.js`'s sentence, not one written here — the
  // same one every other guarded control in the product says, naming the unlock
  // and saying plainly that unlocking does not end the service. It reaches both
  // channels through `ui/Button`: `title` for a pointer, `aria-describedby` for a
  // keyboard or screen-reader operator, because neither alone reaches everybody.
  $: locked = !!$serviceLock.engaged;
  $: modelWriteReason = whyDisabled([locked, SERVICE_LOCKED], [busy, BUSY]);

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
    <Button variant="ghost" size="sm" on:click={cancelModelDownload}>Cancel</Button>
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

    <!-- SAID ONCE, ABOVE THE CARDS. Every button below is held back while a
         service is being recorded, and a tooltip is invisible to somebody who is
         not hovering — so the panel would simply look broken. It is `role="status"`
         because it appears under the operator rather than being navigated to, and
         it names the way out, which is the half a refusal usually leaves off. -->
    {#if locked}
      <p class="ms-locked" role="status">
        <b>A service is being recorded</b>, so the speech model cannot be changed,
        downloaded or installed — swapping it mid-sermon takes the ears away, and a
        service that did exactly that produced four wrong verses in one morning.
        Unlock it in <b>Settings → Before the service</b> if you mean to; unlocking
        does not end the service.
      </p>
    {/if}

    {#if !models.length && $readErrors.listModels}
      <!-- RG-95. The model list swallows to `[]`, and this screen renders a list —
           so a failed read showed an operator a BLANK panel with no models, no
           message and no action, on the screen whose whole job is getting speech
           recognition working before a service. -->
      <ErrorState error={$readErrors.listModels} onRetry={refresh} />
    {/if}

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
          <!-- Disabled because it is already the answer, which is a different
               reason from every other disabled control on this panel and is worth
               saying rather than leaving a grey button to be puzzled over. -->
          <Button
            disabled
            disabledReason="This is the model Relay is listening with. There is nothing to press.">
            In use
          </Button>
        {:else if m.installed}
          <Button
            disabled={busy || locked}
            disabledReason={modelWriteReason}
            on:click={() => use(m.filename)}>
            Use this one
          </Button>
        {:else}
          <!-- AMBER IS NOT AVAILABLE TO THIS BUTTON, and it used to wear it.
               `class:amber={m.recommended}` painted the recommended download in
               `--v-amber`, which means ON AIR and only that (DESIGN_SYSTEM §1,
               rule 18): the loudest colour in the product, reserved for "the
               congregation is looking at this right now", spent on a download
               button in a settings panel. `primary` is the house accent — steel
               blue, "the thing you are working on" — which is exactly what a
               recommended action is, and it is the same fill every other primary
               button in the product wears. -->
          <Button
            variant={m.recommended ? 'primary' : ''}
            disabled={busy || locked}
            disabledReason={modelWriteReason}
            on:click={() => get(m.id)}>
            Download {m.recommended ? '— recommended' : ''}
          </Button>
        {/if}
      </div>
    {/each}
  {/if}

  <!-- ALREADY ON THIS MACHINE. Shown only when there is something to offer, so a
       church with a working connection never sees it — and the one without a
       connection finds it exactly when they need it. -->
  {#if found.length}
    <div class="ms-found">
      <div class="r-lbl">Found on this computer</div>
      <p class="ms-foundnote">
        No internet needed. Relay checked the file against the one it expects, so this
        is the same model it would have downloaded.
      </p>
      {#each found as f (f.id)}
        <div class="ms-foundrow">
          <span class="ms-foundname"><b>{f.label}</b><span class="r-mono">{f.path}</span></span>
          <Button
            size="sm"
            disabled={busy || locked}
            disabledReason={modelWriteReason}
            on:click={() => installFound(f)}>Install</Button>
        </div>
      {/each}
    </div>
  {/if}
  {#if installMsg}<p class="ms-foundnote" role="status">{installMsg}</p>{/if}

  {#if $modelError}
    <!-- Dismissable. It used to have no way out, so a stale failure (or, before the
         fix, the operator's own Cancel) sat in a red box until the component
         remounted — with a working Try again button sitting right underneath it. -->
    <div class="ms-err" role="alert">
      <span>{$modelError}</span>
      <Button variant="ghost" size="sm" on:click={dismissModelError}>Dismiss</Button>
    </div>
  {/if}
</div>

<style>
  .ms-found { margin-top: 14px; }
  .ms-foundnote {
    font-size: var(--v-fs-cap);
    color: var(--v-dim);
    line-height: 1.45;
    margin: 6px 0 8px;
  }
  .ms-foundrow {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 0;
    border-top: 1px solid var(--v-line2);
  }
  .ms-foundname {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .ms-foundname span {
    font-size: var(--v-fs-b3);
    color: var(--v-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
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
  .ms-pct { font-size:var(--v-fs-b1); color: var(--v-txt); }
  /* --v-dim, not --v-faint: faint is ~3.4:1 and fails WCAG AA, and this is the
     text a brand-new operator most needs to be able to read. */
  .ms-sub { margin: 6px 0 0; font-size:var(--v-fs-h3); color: var(--v-dim); line-height: 1.6; }
  .ms-bar {
    height: 7px; border-radius: var(--v-r-sm); background: var(--v-surf3);
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
  .ms-opt-t b { font-size: var(--v-fs-pr); color: var(--v-txt); }
  .ms-size { font-size:var(--v-fs-lbl); color: var(--v-dim); }
  .ms-opt-d { font-size:var(--v-fs-b1); color: var(--v-dim); line-height: 1.55; margin: 3px 0 8px; }
  /* A CAUTION, AND IT WAS AMBER UNTIL THIS PASS. The reasoning beside it was
     half right: nothing here is broken and the operator may still have a good
     reason to pick the model, so red would read as a failure and be clicked
     past. But amber is not the alternative to red — it means ON AIR and only
     that, and this panel is never on air (DESIGN_SYSTEM §1, rule 18).

     The caution ink, the same one `.s-netwarn` and `.b-check.warn` wear. Until
     2026-09-21 all three borrowed amethyst, because this palette published no
     caution colour and DECISIONS §93 chose to enumerate the borrowers rather
     than invent one; §111 paid that gap with `--v-caution`, which promises
     nothing about a screen. This panel was always a caution, and it was
     wearing the one colour it was not allowed to wear. */
  .ms-caution {
    font-size:var(--v-fs-b1); line-height: 1.55; margin: 0 0 8px;
    padding: 7px 9px; border-radius: 7px;
    background: var(--v-caution-soft);
    border: 1px solid var(--v-caution-line);
    color: var(--v-caution2);
  }
  /* The service lock, stated once at the top of the panel rather than only as a
     tooltip per card: a row of greyed buttons with the reason behind a hover is
     still a panel that looks broken to somebody who is not hovering. Same ink as
     the caution above, for the same reason — this is not a failure, it is Relay
     holding something back on purpose, and the operator can lift it. */
  .ms-locked {
    font-size: var(--v-fs-b1); line-height: 1.55; margin: 8px 0 0;
    color: var(--v-caution2);
  }
  .ms-live {
    font-size: 10.5px; letter-spacing: 0.04em; text-transform: uppercase;
    padding: 2px 6px; border-radius: var(--v-r-sm);
    background: var(--v-emerald-soft);
    border: 1px solid var(--v-emerald-line);
    color: var(--v-emerald);
  }
  .ms-err {
    margin-top: 10px; padding: 8px 10px; border-radius: 8px;
    background: var(--v-red-soft); border: 1px solid var(--v-red-line);
    color: var(--v-red); font-size:var(--v-fs-b1); line-height: 1.55;
    display: flex; align-items: center; gap: 10px;
  }
  .ms-err span { flex: 1; min-width: 0; }
</style>
