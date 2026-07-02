<script>
  import { capture, transcript } from '../stores/capture.js';

  // Transcript panel is LIVE (Phase 4): finals + in-progress partial come from
  // the STT engine over `stt://transcript`. Detection cards + channel previews
  // are still static demo data until Phase 5/6 wire the detection pipeline.
  $: hasTranscript = $transcript.finals.length > 0 || $transcript.partial.length > 0;

  // Operator override (search box) is a first-class control per CLAUDE.md —
  // it stays here at the top of the console, always reachable, never a fallback.
  const detections = [
    { ref: 'John 3:16',   state: 'is-live',    tag: 'Auto-fired', pct: 96, conf: '0.96 · direct match',  action: 'undo' },
    { ref: 'Romans 8:28', state: 'is-suggest', tag: 'Suggested',  pct: 71, conf: '0.71 · paraphrase',    action: 'confirm' },
  ];

  const channels = [
    { name: 'Main screen',   chip: 'var(--amber)',  badge: 'HDMI',       prev: 'prev-main' },
    { name: 'Stage display', chip: 'var(--teal)',   badge: 'NDI',        prev: 'prev-stage' },
    { name: 'Streaming',     chip: 'var(--violet)', badge: 'NDI → OBS',  prev: 'prev-stream' },
    { name: 'Lobby screen',  chip: 'var(--rose)',   badge: 'Kiosk',      prev: 'prev-lobby' },
  ];
</script>

<div class="layout">
  <div class="panel">
    <div class="panel-title">
      Live transcript
      {#if !$capture.stt.loaded}<span class="count">no model</span>
      {:else if $capture.capturing}<span class="count">listening</span>{/if}
    </div>
    <div class="transcript">
      {#if hasTranscript}
        {$transcript.finals.join(' ')}
        {#if $transcript.partial}<mark>{$transcript.partial}</mark>{/if}
      {:else if $capture.capturing}
        <span style="color:var(--text-faint);">Waiting for speech…</span>
      {:else if !$capture.stt.loaded}
        <span style="color:var(--text-faint);">No speech model loaded — see Settings. Manual override still works.</span>
      {:else}
        <span style="color:var(--text-faint);">Start listening in Settings to transcribe live.</span>
      {/if}
    </div>

    <div class="panel-title" style="margin-top:16px;">AI detection <span class="count">{detections.length} active</span></div>
    <div class="detect-list">
      {#each detections as d}
        <div class="detect-card {d.state}">
          <div class="detect-top"><div class="detect-ref">{d.ref}</div><div class="detect-tag">{d.tag}</div></div>
          <div class="detect-meter"><i style="width:{d.pct}%;"></i></div>
          <div class="detect-bottom">
            <span class="detect-conf">{d.conf}</span>
            {#if d.action === 'confirm'}
              <button class="btn-confirm">Confirm</button>
            {:else}
              <button class="btn-ghost">Undo</button>
            {/if}
          </div>
        </div>
      {/each}
    </div>

    <div class="panel-title" style="margin-top:16px; margin-bottom:8px;">Manual override</div>
    <input class="search-input" type="text" placeholder="Type a reference or search by phrase…  ( / )" />
    <div class="kbd-row">
      <div class="kbd-hint"><span class="kbd">Space</span> confirm suggestion</div>
      <div class="kbd-hint"><span class="kbd">Esc</span> clear all screens</div>
      <div class="kbd-hint"><span class="kbd">/</span> focus search</div>
      <div class="kbd-hint"><span class="kbd">Z</span> undo last fire</div>
    </div>
  </div>

  <div>
    <div class="channels">
      {#each channels as c}
        <div class="channel">
          <div class="channel-head">
            <div class="channel-name"><span class="chip" style="background:{c.chip};"></span>{c.name}</div>
            <div class="channel-badge">{c.badge}</div>
          </div>
          <div class="channel-preview {c.prev}">
            {#if c.prev === 'prev-main'}
              <div><div class="verse">"For God so loved the world, that He gave His only begotten Son…"</div><div class="ref">John 3:16 · KJV</div></div>
            {:else if c.prev === 'prev-stage'}
              <div class="timer">SERMON · 24:10</div><div class="verse">John 3:16<br />"For God so loved…"</div>
            {:else if c.prev === 'prev-stream'}
              <div class="lower-third"><div class="verse">"For God so loved the world…"</div></div>
            {:else}
              <div class="mark">Grace Chapel</div><div class="verse">"For God so loved the world…"</div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
    <div class="controls">
      <button class="ctrl-btn primary"><span class="dot" style="background:#1b1204;"></span>AI detection: On</button>
      <button class="ctrl-btn"><span class="dot" style="background:var(--text-faint);"></span>Clear all screens</button>
      <button class="ctrl-btn">Manage channels</button>
    </div>
  </div>
</div>
