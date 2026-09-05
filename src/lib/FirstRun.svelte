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
  // ── Shape: docs/design/relay-production-interface.png, panel 2 ─────────
  //
  // A vertical step rail on the left, one content pane on the right, Back and
  // Continue at the bottom. The reference's rail is Welcome · Audio Input ·
  // Model Download · Language · Finish.
  //
  // Two deliberate deviations from it. (They were logged in a working design log
  // that is not in this repository, so they are written out here instead — a
  // citation to a file nobody can open is worse than no citation.)
  //
  //   1. A SCREEN step is added. The reference has none, and without it the
  //      wizard never does the one thing it exists for — put a verse on the
  //      projector. It is the second step, before anything about audio, because
  //      it is the only one that produces something a congregation can see.
  //   2. Language is SINGLE-choice, not the reference's checkbox list. Whisper
  //      takes one language or auto-detect; a multi-select would be a control
  //      that cannot do what it appears to offer. Auto is the default and the
  //      recommendation, because code-switching mid-sentence is the normal case
  //      here, not an edge case (CLAUDE.md).
  //
  // The wizard still ASKS as little as it can: Welcome and Finish ask nothing,
  // and everything Relay can decide for itself — templates, channels, the gate —
  // is already seeded before the operator ever sees this.
  //
  // It can be skipped at any point, and it never comes back uninvited. An
  // operator who dismisses a wizard and then cannot find the setting again has
  // been actively harmed by it — everything here also lives in Settings.
  import { onMount, onDestroy } from 'svelte';
  import { trapFocus } from './focus.js';
  // CLAUDE.md: errors.js is the ONE backend-error humaniser. The wizard used to
  // render `String(e)` — so the first screen a brand-new volunteer ever sees was
  // capable of showing them a raw Rust `Err`, in the one moment they are least
  // able to interpret it.
  import { humanError } from './errors.js';
  import ModelSetup from './ModelSetup.svelte';
  import BrandMark from './ui/BrandMark.svelte';
  import {
    capture,
    meter,
    listMonitors,
    listOutputChannels,
    setChannelDisplay,
    openChannelOutput,
    setInputDevice,
    startCapture,
    stopCapture,
    setDetection,
    setSttLanguage,
    manualFire,
  } from './stores/capture.js';
  import { setSession } from './session.js';

  const STEPS = [
    { key: 'welcome', label: 'Welcome' },
    { key: 'screen', label: 'Screen' },
    { key: 'audio', label: 'Audio Input' },
    { key: 'model', label: 'Model Download' },
    { key: 'language', label: 'Language' },
    { key: 'finish', label: 'Finish' },
  ];

  let i = 0; // index into STEPS
  $: stepKey = STEPS[i].key;

  let monitors = [];
  let channel = null;
  let chosenMonitor = null;
  let screenOpened = false;
  let busy = false;
  let error = '';
  let fired = false;
  let hardware = null;

  $: hasModel = $capture.stt.loaded;
  // A live meter is the only honest proof a microphone works. A dropdown is not.
  $: level = Math.min(100, Math.round($meter.level * 320));
  // The reference draws the meter as 24 discrete segments.
  const SEGMENTS = 24;
  $: litSegments = Math.round((level / 100) * SEGMENTS);

  // Whisper accepts ONE language, or auto. See the header note.
  const LANGS = [
    { code: null, label: 'Auto-detect', hint: 'Handles English mixed with a local language mid-sentence' },
    { code: 'en', label: 'English', hint: '' },
    { code: 'yo', label: 'Yorùbá', hint: 'Tier 1' },
    { code: 'sw', label: 'Kiswahili', hint: 'Tier 1' },
    { code: 'ha', label: 'Hausa', hint: 'Tier 1' },
  ];
  let lang = null;

  onMount(async () => {
    monitors = await listMonitors();
    const chans = await listOutputChannels();
    channel =
      chans.find((c) => c.render_target === 'native_window' && c.name === 'Main screen') ??
      chans.find((c) => c.render_target === 'native_window');
    // A second screen on a church laptop is a projector essentially every time.
    chosenMonitor = monitors.find((m) => !m.primary)?.index ?? monitors[0]?.index ?? null;
    lang = $capture.stt.language ?? null;
    // Used on the Model step to say plainly whether whisper has any acceleration.
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      hardware = await invoke('system_hardware');
    } catch {
      hardware = null;
    }
  });

  async function openScreen() {
    if (!channel || chosenMonitor === null) return;
    busy = true;
    error = '';
    try {
      await setChannelDisplay(channel.id, String(chosenMonitor));
      await openChannelOutput(channel.id);
      screenOpened = true;
    } catch (e) {
      error = humanError(e);
    }
    busy = false;
  }

  // ── the microphone test ──────────────────────────────────────────────────
  //
  // The meter WAS DEAD. `$meter` is only fed by the `audio://chunk` listener, which
  // is registered inside startCapture() — and FirstRun never called it. So on the one
  // screen whose entire stated purpose is "a moving bar proves the microphone is
  // actually hearing something", `$capture.capturing` was false, the bar never moved,
  // and the hint fell through to "You can test this from the Live tab".
  //
  // The step that exists to PROVE the microphone works, proved nothing.
  //
  // Detection is disarmed for the duration. If the operator already has a speech
  // model installed, a live microphone during setup could auto-fire a detected verse
  // onto the projector we have just this second taught them to open — while they are
  // saying "testing, testing" into it. The wizard must not put scripture on a wall by
  // accident. The previous value is restored on the way out.
  let micOn = false;
  let detectionWas = true;

  async function startMicTest() {
    if (micOn) return;
    detectionWas = $capture.detectionOn;
    try {
      await setDetection(false);
      await startCapture($capture.inputDevice || undefined);
      micOn = true;
    } catch (e) {
      // Not fatal: the mic step is a convenience, not a gate. Say so and move on.
      error = humanError(e);
    }
  }

  async function stopMicTest() {
    if (!micOn) return;
    // Caught, not propagated — this also runs from onDestroy, where a rejection is
    // an unhandled promise with nobody left to catch it. But SHOWN, not swallowed:
    // `stopCapture` now rejects when the microphone did not actually stop, and the
    // wizard's whole job is proving to a volunteer that the microphone does what
    // the screen says it does.
    //
    // `micOn` is cleared only AFTER the backend confirms. Clearing it first meant a
    // failed stop left the flag saying "off" over a microphone that was still open,
    // and the `if (!micOn) return;` above then refused every retry — the same shape
    // as the `stopCapture` bug this wizard step exists to prove is gone.
    try {
      await stopCapture();
      micOn = false;
    } catch (e) {
      error = humanError(e);
    }
    // Restoring detection MUST NOT throw out of here — this also runs from onDestroy,
    // where a rejection is an unhandled promise and nobody is left to catch it.
    try {
      await setDetection(detectionWas);
    } catch (e) {
      error = humanError(e);
    }
  }

  // Changing the device mid-test must re-open the stream, or the meter keeps
  // showing the OLD microphone — which is worse than showing nothing, because it
  // looks like proof of a device that is not the one selected.
  async function chooseDevice(name) {
    await setInputDevice(name);
    if (micOn) {
      // Never open a second capture over one that did not close. A swallowed stop
      // here would leave the OLD device streaming under the NEW device's meter —
      // which is the failure this whole step exists to rule out.
      try {
        await stopCapture();
        micOn = false;
      } catch (e) {
        error = humanError(e);
        return;
      }
      await startCapture(name || undefined).catch((e) => (error = humanError(e)));
      micOn = true;
    }
  }

  async function pickLang(code) {
    lang = code;
    await setSttLanguage(code);
  }

  // Prove it. Not "setup complete" — an actual verse, on the actual screen.
  async function tryIt() {
    busy = true;
    error = '';
    try {
      await manualFire('John 3:16');
      fired = true;
    } catch (e) {
      error = humanError(e);
    }
    busy = false;
  }

  // The mic runs ONLY on its own step. Leaving it open across the rest of the
  // wizard means a hot microphone during a step that says nothing about audio.
  async function go(next) {
    error = '';
    if (STEPS[i].key === 'audio' && STEPS[next]?.key !== 'audio') await stopMicTest();
    i = Math.max(0, Math.min(STEPS.length - 1, next));
    if (STEPS[i].key === 'audio') startMicTest();
  }

  async function done() {
    // Never leave the wizard's microphone running behind it, and never leave
    // detection disarmed — an operator whose AI is silently off, because a wizard
    // they skipped turned it off, would have no way of knowing why.
    await stopMicTest();
    setSession({ setupDone: true });
  }

  // Covers the paths `done()` does not: a crash, a reload, a recover.
  onDestroy(() => {
    stopMicTest();
  });

  const gb = (b) => `${(b / 1e9).toFixed(1)} GB`;
</script>

<div class="fr-scrim">
  <div class="fr" role="dialog" aria-modal="true" aria-labelledby="fr-title" use:trapFocus>
    <!-- Left rail. The reference numbers every step and marks the current one
         with a filled amethyst disc. -->
    <nav class="fr-rail" aria-label="Setup progress">
      <div class="fr-brand"><BrandMark size="18px" /><span>RELAY</span></div>
      <ol>
        {#each STEPS as s, n}
          <li class:on={n === i} class:did={n < i}>
            <span class="n">
              {#if n < i}
                <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor"
                  stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="m5 12 5 5 9-10"/></svg>
              {:else}{n + 1}{/if}
            </span>
            {s.label}
          </li>
        {/each}
      </ol>
      <button class="fr-skip" on:click={done}>Skip setup</button>
    </nav>

    <div class="fr-pane">
      <div class="fr-body">
        {#if stepKey === 'welcome'}
          <h1 id="fr-title">Welcome to Relay</h1>
          <p class="fr-p">
            Relay listens to the live audio, detects scripture references — spoken or
            paraphrased — and routes the right content to your screens in real time.
            Everything runs on this machine. Nothing needs the internet.
          </p>
          <p class="fr-p">
            This takes about two minutes. You can skip it and change all of it later in
            Settings.
          </p>
          <ul class="fr-facts">
            <li><b>Offline</b><span>Speech, detection and output all run locally</span></li>
            <li><b>You are always in control</b><span>Nothing reaches a screen unless you or a rule you set puts it there</span></li>
            <li><b>Esc clears the screens</b><span>From anywhere in Relay, even mid-service</span></li>
          </ul>
        {:else if stepKey === 'screen'}
          <h1 id="fr-title">Which screen does the congregation see?</h1>
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

          {#if screenOpened}
            <p class="fr-ok-line">
              <span class="tick">✓</span> The output window is open on that screen. It is
              blank — nothing goes on it until you put something there.
            </p>
          {:else}
            <button class="r-btn ghost" disabled={busy || chosenMonitor === null} on:click={openScreen}>
              Open the congregation screen
            </button>
          {/if}
        {:else if stepKey === 'audio'}
          <h1 id="fr-title">Which microphone hears the preacher?</h1>
          <p class="fr-p">
            Usually the feed from the sound desk — not the laptop's built-in mic, which
            will mostly hear the room.
          </p>

          <label class="fr-lbl" for="fr-dev">Microphone</label>
          <select
            id="fr-dev"
            class="r-select"
            value={$capture.inputDevice}
            on:change={(e) => chooseDevice(e.target.value)}>
            <option value="">Default input</option>
            {#each $capture.devices as d}
              <option value={d.name}>{d.name}{d.is_default ? ' — default' : ''}</option>
            {/each}
          </select>

          <!-- The meter is the point. A dropdown proves nothing; a moving bar proves
               the microphone is actually hearing something. Segmented, as the
               reference draws it. -->
          <div class="fr-meter" role="img" aria-label="Microphone level">
            {#each Array(SEGMENTS) as _, n}
              <i class:lit={n < litSegments} class:hot={n >= SEGMENTS - 3}></i>
            {/each}
          </div>
          <p class="fr-hint">
            {#if $capture.capturing}
              {level > 4 ? '✓ Relay can hear that.' : 'Say something — the bar should move.'}
            {:else}
              Starting the microphone…
            {/if}
          </p>
        {:else if stepKey === 'model'}
          <h1 id="fr-title">Speech model</h1>
          <p class="fr-p">
            Relay transcribes on this machine, so it needs a speech model — a one-time
            download. It resumes if the connection drops, and the file is checksummed.
          </p>

          {#if hasModel}
            <p class="fr-ok-line">
              <span class="tick">✓</span> A model is installed:
              <span class="r-mono">{$capture.stt.model ?? 'ready'}</span>.
            </p>
          {:else}
            <ModelSetup />
          {/if}

          {#if hardware}
            <!-- An honest note, not a spec sheet: on a CPU-only build the processor
                 is what decides whether transcription keeps up with a preacher. -->
            <p class="fr-note">
              {#if hardware.gpu_backends?.length}
                Whisper is compiled with {hardware.gpu_backends.join(', ')} on this build.
              {:else}
                This build runs whisper on the CPU — {hardware.cores ?? '?'} threads,
                {gb(hardware.available_memory_bytes)} free. That is what decides whether
                transcription keeps up with a preacher.
              {/if}
            </p>
          {/if}

          <p class="fr-note">
            You can skip this. Relay still works — you put verses on the screen by typing
            a reference. It just will not hear them for you.
          </p>
        {:else if stepKey === 'language'}
          <h1 id="fr-title">What language is preached?</h1>
          <p class="fr-p">
            This is what Relay <em>listens</em> for. It is separate from the console's own
            language, which you set in Settings.
          </p>

          <div class="fr-langs" role="radiogroup" aria-label="Recognition language">
            {#each LANGS as l}
              <button
                class="fr-lang"
                role="radio"
                aria-checked={lang === l.code}
                class:sel={lang === l.code}
                on:click={() => pickLang(l.code)}>
                <span class="dot"></span>
                <span class="t">
                  <b>{l.label}</b>
                  {#if l.hint}<span>{l.hint}</span>{/if}
                </span>
              </button>
            {/each}
          </div>

          <!-- The honest caveat. CLAUDE.md is explicit that the multilingual claim is
               a reference-parsing table on stock Whisper, with no measured word error
               rate in any language. The wizard must not imply more than that. -->
          <p class="fr-note">
            Auto-detect is the recommendation: a preacher switching between English and a
            local language mid-sentence is the normal case here, not an edge case. Relay
            recognises scripture references in Yorùbá, Kiswahili and Hausa; accuracy in
            those languages has not yet been formally measured.
          </p>
        {:else}
          <h1 id="fr-title">Let's put something on the screen.</h1>
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
          {:else}
            <button class="r-btn ghost" disabled={busy} on:click={tryIt}>
              Put John 3:16 on the screen
            </button>
          {/if}

          <ul class="fr-facts">
            <li><kbd>Esc</kbd><span>Clear the screens — works everywhere, even while typing</span></li>
            <li><kbd>B</kbd><span>Black out every output</span></li>
            <li><kbd>Space</kbd><span>Advance</span></li>
            <li><kbd>?</kbd><span>Every other shortcut</span></li>
          </ul>

          <!-- The three instruments a first-time operator has no way to find.
               They are NOT extra wizard steps: this wizard's rule is that it asks
               as little as it can, and each of these is a thing to DO on another
               day rather than an answer to give now. What was missing was never
               the feature — all three shipped — it was that nothing told a new
               volunteer they exist, which is the whole of the onboarding gap
               (RELAY_GAP §2, brief §59/60). Naming where each one lives is the
               fix; moving them in here would be the wrong one. -->
          <div class="fr-next">
            <b>Before your first Sunday</b>
            <ul>
              <li>
                <span class="w">Practise</span>
                <span>Six drills on the <b>Help</b> tab, with the real controls, in
                rehearsal. The panic keys come first — the one you need under pressure
                is the one you should not be reading about at the time.</span>
              </li>
              <li>
                <span class="w">Check the whole chain</span>
                <span><b>Settings → Dashboard</b>: say one verse out loud and Relay
                shows which of the six stages between the microphone and the screen were
                reached. Everything you just set up can pass while the chain still does
                not work end to end.</span>
              </li>
              <li>
                <span class="w">Rehearse</span>
                <span><b>Rehearse</b> on the Live tab runs a whole service with nothing
                reaching the projector, the stage monitor or OBS. You can do it in the
                middle of a real service.</span>
              </li>
            </ul>
            <span class="fr-nextnote">All three are in Settings and on the tabs — this
            is not your last chance to find them.</span>
          </div>
        {/if}

        {#if error}<div class="fr-err">{error}</div>{/if}
      </div>

      <!-- Back / Continue, as the reference. Continue is the primary and is
           amethyst: setting a wizard up puts nothing on a wall (DECISIONS §22). -->
      <div class="fr-foot">
        <button class="r-btn ghost" disabled={i === 0} on:click={() => go(i - 1)}>Back</button>
        <span class="sp"></span>
        {#if i === STEPS.length - 1}
          <button class="r-btn primary" on:click={done}>Start using Relay</button>
        {:else}
          <button class="r-btn primary" on:click={() => go(i + 1)}>Continue</button>
        {/if}
      </div>
    </div>
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
    display: flex;
    width: 100%;
    max-width: 860px;
    min-height: 520px;
    background: var(--v-surf);
    border: 1px solid var(--v-line2);
    border-radius: var(--v-r-xl);
    overflow: hidden;
    box-shadow: var(--v-shadow-lg);
  }

  /* ── Left rail ── */
  .fr-rail {
    flex: 0 0 208px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 22px 16px;
    background: var(--v-bg);
    border-right: 1px solid var(--v-line);
  }
  .fr-brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 0 6px;
  }
  .fr-brand span {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.16em;
    color: var(--v-txt);
  }
  .fr-rail ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .fr-rail li {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 10px 10px;
    border-radius: 9px;
    font-size: 13px;
    color: var(--v-dim);
  }
  .fr-rail li .n {
    width: 22px;
    height: 22px;
    flex: 0 0 auto;
    border-radius: 50%;
    display: grid;
    place-items: center;
    border: 1px solid var(--v-line2);
    font-family: var(--f-mono);
    font-size: 10px;
    color: var(--v-faint);
  }
  /* The step you are ON. Chrome, not a tally light. */
  .fr-rail li.on {
    background: var(--v-accent-soft);
    color: var(--v-txt);
  }
  .fr-rail li.on .n {
    background: var(--v-accent-fill);
    border-color: transparent;
    color: #fff;
  }
  /* Done. Green is the design sheet's confirmed colour. */
  .fr-rail li.did .n {
    border-color: rgba(34, 197, 94, 0.5);
    color: var(--v-emerald);
  }
  .fr-skip {
    margin-top: auto;
    background: none;
    border: 0;
    padding: 6px;
    text-align: left;
    font: inherit;
    font-size: 12px;
    color: var(--v-faint);
    cursor: pointer;
    text-decoration: underline;
  }
  .fr-skip:hover {
    color: var(--v-dim);
  }

  /* ── Right pane ── */
  .fr-pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .fr-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 30px 32px;
  }
  h1 {
    margin: 0 0 10px;
    font-family: var(--f-head);
    font-size: var(--v-fs-h1);
    line-height: var(--v-lh-h1);
    letter-spacing: var(--v-tr-tight);
    font-weight: 600;
    color: var(--v-txt);
  }
  /* --v-dim, never --v-faint: faint is ~3.4:1 and fails WCAG AA, and this is the
     first text a brand-new operator ever reads. */
  .fr-p {
    margin: 0 0 14px;
    font-size: var(--v-fs-b1);
    line-height: 1.65;
    color: var(--v-dim);
    max-width: 56ch;
  }
  .fr-note {
    margin: 14px 0 0;
    font-size: var(--v-fs-b2);
    line-height: 1.6;
    color: var(--v-faint);
    max-width: 60ch;
  }
  .fr-lbl {
    display: block;
    margin: 6px 0 7px;
    font-size: var(--v-fs-lbl);
    font-weight: 500;
    letter-spacing: var(--v-tr-wide);
    color: var(--v-txt);
  }

  .fr-facts {
    list-style: none;
    margin: 18px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  /* The hand-off block. Quiet by construction — it must not compete with the
     verse that just went on the screen, which is the step's actual proof. */
  .fr-next{ margin-top:18px; padding:14px 16px; border:1px solid var(--v-line);
    border-radius:var(--v-r-md); background:var(--v-surf2); }
  .fr-next > b{ display:block; font-size:var(--v-fs-cap); letter-spacing:.06em;
    text-transform:uppercase; color:var(--v-faint); margin-bottom:10px; }
  .fr-next ul{ margin:0; padding:0; list-style:none; display:flex;
    flex-direction:column; gap:10px; }
  .fr-next li{ display:grid; grid-template-columns:132px 1fr; gap:12px;
    align-items:start; font-size:var(--v-fs-cap); line-height:1.55; }
  .fr-next .w{ color:var(--v-txt); font-weight:600; }
  .fr-next li > span:last-child{ color:var(--v-dim); }
  .fr-nextnote{ display:block; margin-top:12px; font-size:var(--v-fs-cap);
    color:var(--v-faint); }
  .fr-facts li {
    display: flex;
    align-items: baseline;
    gap: 12px;
    font-size: var(--v-fs-b2);
    color: var(--v-dim);
  }
  .fr-facts li b {
    flex: 0 0 auto;
    color: var(--v-txt);
    font-weight: 600;
  }
  .fr-facts kbd {
    flex: 0 0 auto;
    min-width: 42px;
    text-align: center;
    font-family: var(--f-mono);
    font-size: 10.5px;
    padding: 4px 7px;
    border-radius: var(--v-r-sm);
    background: var(--v-surf3);
    border: 1px solid var(--v-line2);
    color: var(--v-txt);
  }

  /* Monitors */
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
  /* A SELECTED monitor, not a live one — picking it puts nothing on it. */
  .fr-mon.sel {
    border-color: var(--v-accent);
    background: var(--v-accent-soft);
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
    font-size: 13px;
  }
  .fr-mon-d {
    font-size: 10px;
    color: var(--v-faint);
  }

  /* Segmented level meter, as the reference draws it. GREEN is correct here and
     is not a mode colour on this screen: it is a signal meter, and the top three
     segments go red because that is clipping, not "on air". */
  .fr-meter {
    display: flex;
    gap: 3px;
    height: 18px;
    margin: 12px 0 0;
  }
  .fr-meter i {
    flex: 1;
    border-radius: 2px;
    background: var(--v-surf3);
    transition: background 0.08s linear;
  }
  .fr-meter i.lit {
    background: var(--v-emerald);
  }
  .fr-meter i.hot.lit {
    background: var(--v-red);
  }
  .fr-hint {
    margin: 9px 0 0;
    font-size: var(--v-fs-b2);
    color: var(--v-dim);
  }

  /* Language choice — radios, because whisper takes one language or auto. */
  .fr-langs {
    display: flex;
    flex-direction: column;
    gap: 7px;
    max-width: 420px;
  }
  .fr-lang {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 11px 13px;
    border-radius: 10px;
    background: var(--v-surf2);
    border: 1px solid var(--v-line);
    color: var(--v-txt);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .fr-lang .dot {
    width: 15px;
    height: 15px;
    flex: 0 0 auto;
    border-radius: 50%;
    border: 1px solid var(--v-line2);
    display: grid;
    place-items: center;
  }
  .fr-lang.sel {
    border-color: var(--v-accent);
    background: var(--v-accent-soft);
  }
  .fr-lang.sel .dot {
    border-color: var(--v-accent);
    background: radial-gradient(circle, var(--v-accent) 45%, transparent 48%);
  }
  .fr-lang .t b {
    display: block;
    font-size: 13.5px;
    font-weight: 600;
  }
  .fr-lang .t span {
    display: block;
    font-size: 11.5px;
    color: var(--v-faint);
    margin-top: 2px;
  }

  .fr-ok {
    margin: 4px 0 0;
    padding: 14px 16px;
    border-radius: var(--v-r-md);
    background: var(--v-emerald-soft);
    border: 1px solid rgba(34, 197, 94, 0.3);
    font-size: var(--v-fs-b2);
    line-height: 1.6;
    color: var(--v-dim);
  }
  .fr-ok b {
    display: block;
    margin-bottom: 4px;
    color: var(--v-emerald);
    font-size: 14px;
  }
  .fr-ok kbd,
  .fr-facts li kbd {
    font-family: var(--f-mono);
  }
  .fr-ok kbd {
    font-size: 10.5px;
    padding: 2px 5px;
    border-radius: 4px;
    background: var(--v-surf3);
    border: 1px solid var(--v-line2);
    color: var(--v-txt);
  }
  .fr-ok-line {
    margin: 4px 0 0;
    font-size: var(--v-fs-b2);
    line-height: 1.6;
    color: var(--v-dim);
  }
  .fr-ok-line .tick {
    color: var(--v-emerald);
    font-weight: 700;
  }

  .fr-err {
    margin-top: 16px;
    padding: 11px 13px;
    border-radius: var(--v-r-md);
    background: var(--v-red-soft);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: var(--v-txt);
    font-size: 12.5px;
    line-height: 1.55;
  }

  .fr-foot {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 16px 32px;
    border-top: 1px solid var(--v-line);
  }
  .fr-foot .sp {
    flex: 1;
  }

  @media (max-width: 760px) {
    .fr {
      flex-direction: column;
      min-height: 0;
    }
    .fr-rail {
      flex: 0 0 auto;
      border-right: 0;
      border-bottom: 1px solid var(--v-line);
    }
    .fr-rail ol {
      flex-direction: row;
      overflow-x: auto;
    }
    .fr-rail li {
      white-space: nowrap;
    }
    .fr-skip {
      margin-top: 0;
    }
    .fr-body {
      padding: 22px 20px;
    }
    .fr-foot {
      padding: 14px 20px;
    }
  }
</style>
