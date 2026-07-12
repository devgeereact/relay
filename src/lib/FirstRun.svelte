<script>
  // First run: get a verse onto a projector, without a terminal.
  //
  // The audit's exit criterion for "installable" is one sentence:
  //
  //   A volunteer who has never seen a terminal installs Relay and gets a verse
  //   on a projector in under 10 minutes.
  //
  // Everything else in the app is optional refinement. THIS is the path.
  //
  // TWO questions, no more. Each is a thing only a human standing in the room can
  // answer — which screen faces the congregation, which microphone hears the
  // preacher. Anything Relay can decide for itself, it already has: the templates,
  // the channels and the default gate are seeded before the operator ever sees it.
  //
  // It can be skipped, and it never comes back uninvited. An operator who dismisses
  // a wizard and then cannot find the setting again has been actively harmed by it —
  // everything here also lives in Settings, permanently.
  import { onMount } from 'svelte';
  import ModelSetup from './ModelSetup.svelte';
  import {
    capture,
    meter,
    listMonitors,
    listOutputChannels,
    setChannelDisplay,
    openChannelOutput,
    setInputDevice,
    manualFire,
  } from './stores/capture.js';
  import { setSession } from './session.js';

  let step = 1;
  let monitors = [];
  let channel = null;
  let chosenMonitor = null;
  let busy = false;
  let error = '';
  let fired = false;

  $: hasModel = $capture.stt.loaded;
  // A live meter is the only honest proof a microphone works. A dropdown is not.
  $: level = Math.min(100, Math.round($meter.level * 320));

  onMount(async () => {
    monitors = await listMonitors();
    const chans = await listOutputChannels();
    channel =
      chans.find((c) => c.render_target === 'native_window' && c.name === 'Main screen') ??
      chans.find((c) => c.render_target === 'native_window');
    // A second screen on a church laptop is a projector essentially every time.
    chosenMonitor = monitors.find((m) => !m.primary)?.index ?? monitors[0]?.index ?? null;
  });

  async function useThisScreen() {
    if (!channel || chosenMonitor === null) return;
    busy = true;
    error = '';
    try {
      await setChannelDisplay(channel.id, String(chosenMonitor));
      await openChannelOutput(channel.id);
      step = 2;
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }

  // Prove it. Not "setup complete" — an actual verse, on the actual screen.
  async function tryIt() {
    busy = true;
    error = '';
    try {
      await manualFire('John 3:16');
      fired = true;
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }

  function done() {
    setSession({ setupDone: true });
  }
</script>

<div class="fr-scrim">
  <div class="fr" role="dialog" aria-modal="true" aria-labelledby="fr-title">
    <div class="fr-head">
      <div>
        <h1 id="fr-title">Let's get a verse on the screen.</h1>
        <p class="fr-sub">Two questions. Then you're running.</p>
      </div>
      <button class="fr-skip" on:click={done}>Skip setup</button>
    </div>

    <ol class="fr-steps" aria-label="Setup progress">
      <li class:on={step === 1} class:did={step > 1}>Screen</li>
      <li class:on={step === 2} class:did={step > 2}>Microphone</li>
      <li class:on={step === 3}>Try it</li>
    </ol>

    {#if step === 1}
      <h2>Which screen does the congregation see?</h2>
      {#if monitors.length > 1}
        <p class="fr-p">Relay found {monitors.length} screens. Pick the projector.</p>
      {:else}
        <p class="fr-p">
          Relay can only see one screen. Plug in the projector or TV and it will appear
          here — or carry on, and the output opens in a window you can drag across.
        </p>
      {/if}

      <div class="fr-mons">
        {#each monitors as m}
          <button
            class="fr-mon"
            class:sel={chosenMonitor === m.index}
            on:click={() => (chosenMonitor = m.index)}>
            <span class="fr-mon-box" style="aspect-ratio:{m.width}/{m.height}"></span>
            <b>{m.name}</b>
            <span class="r-mono fr-mon-d">
              {m.width}×{m.height}{m.primary ? ' · this laptop' : ''}
            </span>
          </button>
        {/each}
      </div>

      <button
        class="r-btn amber fr-go"
        disabled={busy || chosenMonitor === null}
        on:click={useThisScreen}>
        Open the congregation screen
      </button>
    {:else if step === 2}
      <h2>Which microphone hears the preacher?</h2>
      <p class="fr-p">
        Usually the feed from the sound desk — not the laptop's built-in mic, which
        will mostly hear the room.
      </p>

      <select
        class="r-select"
        value={$capture.inputDevice}
        on:change={(e) => setInputDevice(e.target.value)}>
        <option value="">Default input</option>
        {#each $capture.devices as d}
          <option value={d.name}>{d.name}{d.is_default ? ' — default' : ''}</option>
        {/each}
      </select>

      <!-- The meter is the point. A dropdown proves nothing; a moving bar proves the
           microphone is actually hearing something. -->
      <div class="fr-meter" role="img" aria-label="Microphone level">
        <i style="transform:scaleX({level / 100})"></i>
      </div>
      <p class="fr-hint">
        {#if $capture.capturing}
          {level > 4 ? '✓ Relay can hear that.' : 'Say something — the bar should move.'}
        {:else}
          You can test this from the Console, and change it in Settings at any time.
        {/if}
      </p>

      {#if !hasModel}
        <ModelSetup compact />
      {/if}

      <button class="r-btn amber fr-go" on:click={() => (step = 3)}>Next</button>
    {:else}
      <h2>Let's put something on the screen.</h2>
      <p class="fr-p">
        This fires John 3:16 to the congregation screen — exactly what happens during
        a service.
      </p>

      {#if fired}
        <div class="fr-ok">
          <b>That's it. It's on the screen.</b>
          Press <kbd>Esc</kbd> to clear it — that works from anywhere in Relay, even
          mid-service. <kbd>B</kbd> blacks the screens out entirely.
        </div>
        <button class="r-btn amber fr-go" on:click={done}>Start using Relay</button>
      {:else}
        <button class="r-btn amber fr-go" disabled={busy} on:click={tryIt}>
          Put John 3:16 on the screen
        </button>
        <button class="fr-skip fr-skip-b" on:click={done}>Skip this</button>
      {/if}
    {/if}

    {#if error}<div class="fr-err">{error}</div>{/if}
  </div>
</div>

<style>
  .fr-scrim {
    position: fixed;
    inset: 0;
    z-index: 950;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.72);
    padding: 24px;
    overflow: auto;
  }
  .fr {
    width: 100%;
    max-width: 560px;
    background: var(--v-surf);
    border: 1px solid var(--v-line2);
    border-radius: 16px;
    padding: 26px 28px 28px;
    box-shadow: 0 24px 70px rgba(0, 0, 0, 0.55);
  }
  .fr-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }
  h1 {
    margin: 0;
    font-family: var(--f-display);
    font-size: 22px;
    font-weight: 600;
    color: var(--v-txt);
    line-height: 1.25;
  }
  /* --v-dim, never --v-faint: faint is ~3.4:1 and fails WCAG AA, and this is the
     first text a brand-new operator ever reads. */
  .fr-sub {
    margin: 5px 0 0;
    font-size: 13px;
    color: var(--v-dim);
  }
  h2 {
    margin: 20px 0 6px;
    font-size: 15px;
    font-weight: 600;
    color: var(--v-txt);
  }
  .fr-p {
    margin: 0 0 14px;
    font-size: 13px;
    line-height: 1.6;
    color: var(--v-dim);
  }
  .fr-skip {
    background: none;
    border: 0;
    color: var(--v-dim);
    font-size: 12px;
    cursor: pointer;
    padding: 4px 2px;
    text-decoration: underline;
    flex: none;
  }
  .fr-skip:hover {
    color: var(--v-txt);
  }
  .fr-skip-b {
    display: block;
    margin: 10px auto 0;
  }
  .fr-steps {
    display: flex;
    gap: 8px;
    list-style: none;
    margin: 18px 0 0;
    padding: 0;
  }
  .fr-steps li {
    flex: 1;
    font-size: 10.5px;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--v-dim);
    padding-top: 8px;
    border-top: 2px solid var(--v-line2);
  }
  .fr-steps li.on {
    color: var(--v-amber2);
    border-top-color: var(--v-amber);
  }
  .fr-steps li.did {
    color: var(--v-emerald);
    border-top-color: var(--v-emerald);
  }
  .fr-mons {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 16px;
  }
  .fr-mon {
    flex: 1 1 150px;
    background: var(--v-surf2);
    border: 1px solid var(--v-line);
    border-radius: 11px;
    padding: 12px;
    text-align: left;
    cursor: pointer;
    color: var(--v-txt);
    font: inherit;
  }
  .fr-mon.sel {
    border-color: var(--v-amber);
    background: var(--v-amber-soft);
  }
  .fr-mon-box {
    display: block;
    width: 100%;
    background: var(--v-surf3);
    border-radius: 5px;
    margin-bottom: 9px;
  }
  .fr-mon b {
    display: block;
    font-size: 12.5px;
  }
  .fr-mon-d {
    font-size: 10px;
    color: var(--v-dim);
  }
  .fr-meter {
    height: 9px;
    border-radius: 99px;
    background: var(--v-surf3);
    overflow: hidden;
    margin: 12px 0 6px;
  }
  .fr-meter i {
    display: block;
    height: 100%;
    width: 100%;
    background: var(--v-emerald);
    transform-origin: left center;
    transform: scaleX(0);
    transition: transform 0.08s linear;
  }
  @media (prefers-reduced-motion: reduce) {
    .fr-meter i {
      transition: none;
    }
  }
  .fr-hint {
    margin: 0 0 16px;
    font-size: 12px;
    color: var(--v-dim);
    min-height: 1.4em;
  }
  .fr-go {
    width: 100%;
    margin-top: 6px;
  }
  .fr-ok {
    background: rgba(16, 185, 129, 0.12);
    border: 1px solid rgba(16, 185, 129, 0.35);
    border-radius: 10px;
    padding: 13px 15px;
    font-size: 12.5px;
    line-height: 1.65;
    color: var(--v-dim);
    margin-bottom: 6px;
  }
  .fr-ok b {
    display: block;
    color: var(--v-emerald);
    font-size: 13.5px;
    margin-bottom: 3px;
  }
  .fr-ok kbd {
    font-family: var(--f-mono);
    font-size: 10.5px;
    padding: 2px 5px;
    border-radius: 4px;
    background: var(--v-surf3);
    border: 1px solid var(--v-line2);
    color: var(--v-txt);
  }
  .fr-err {
    margin-top: 12px;
    padding: 9px 11px;
    border-radius: 8px;
    background: rgba(147, 0, 10, 0.18);
    border: 1px solid rgba(255, 157, 148, 0.3);
    color: var(--v-txt);
    font-size: 12px;
  }
</style>
