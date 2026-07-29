<script>
  // Mobile stage-display remote — the preacher opens this on a phone/iPad (via
  // QR or the LAN URL) to see the live verse + reference in real time. No Tauri
  // runtime: it connects to the kiosk WebSocket hub (:8031) for content, exactly
  // like an OBS/kiosk output, but rendered as a readable mobile confidence view.
  import { onMount, onDestroy } from 'svelte';

  let content = null;
  let visible = false;
  let note = ''; // operator's confidence-monitor note for the live cue
  let next = null; // { label, text } — the "up next" preview
  let connected = false;
  let ws = null;
  let closed = false;
  let clock = '';
  let timer;

  // Countdown mirror — ticked by the same 1s timer as the wall clock.
  let cdTo = null;
  let cdDone = '';
  let nowMs = 0;
  $: cdRemain = cdTo ? Math.max(0, cdTo - nowMs) : null;
  $: cdFinished = cdRemain === 0;
  $: cdText = (() => {
    if (cdRemain == null) return '';
    const s = Math.round(cdRemain / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  })();

  function apply(m) {
    if (m.kind === 'content') {
      content = { reference: m.reference, text: m.text, translation: m.translation };
      note = m.stage_note || '';
      cdTo = m.countdown_to || null;
      cdDone = m.countdown_done || '';
      nowMs = Date.now();
      visible = true;
    } else if (m.kind === 'clear') {
      visible = false;
      note = '';
      cdTo = null;
      next = null;
    } else if (m.kind === 'stage_next') {
      next = m.label || m.text ? { label: m.label || '', text: m.text || '' } : null;
    }
  }

  function connect(host) {
    if (closed) return;
    try {
      ws = new WebSocket(`ws://${host}:8031`);
      ws.onopen = () => (connected = true);
      ws.onmessage = (e) => {
        try { apply(JSON.parse(e.data)); } catch { /* ignore */ }
      };
      ws.onclose = () => {
        connected = false;
        if (!closed) setTimeout(() => connect(host), 1500);
      };
      ws.onerror = () => { try { ws.close(); } catch { /* onclose retries */ } };
    } catch {
      if (!closed) setTimeout(() => connect(host), 1500);
    }
  }

  onMount(() => {
    connect(location.hostname || 'localhost');
    const tick = () => {
      clock = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
      nowMs = Date.now(); // drives the countdown mirror
    };
    tick();
    timer = setInterval(tick, 1000);
  });
  onDestroy(() => {
    closed = true;
    if (ws) ws.close();
    clearInterval(timer);
  });
</script>

<div class="sr">
  <header>
    <span class="brand">Relay · Stage</span>
    <span class="status" class:on={connected}><i></i>{connected ? 'live' : 'connecting…'}</span>
    <span class="clock">{clock}</span>
  </header>
  <main>
    {#if visible && cdTo}
      {#if content.reference && !cdFinished}<div class="ref">{content.reference}</div>{/if}
      <div class="verse countdown">{cdFinished ? (cdDone || '0:00') : cdText}</div>
      {#if note}<div class="note"><span class="note-lbl">Note</span>{note}</div>{/if}
    {:else if visible && content}
      {#if content.reference}<div class="ref">{content.reference}{content.translation ? ' · ' + content.translation : ''}</div>{/if}
      {#if content.text}<div class="verse">{#if content.reference}“{content.text}”{:else}{content.text}{/if}</div>{/if}
      {#if note}<div class="note"><span class="note-lbl">Note</span>{note}</div>{/if}
    {:else}
      <div class="idle">— standby —</div>
    {/if}
  </main>
  {#if next}
    <footer class="next">
      <span class="next-lbl">Up next</span>
      <div class="next-body">
        {#if next.label}<span class="next-ref">{next.label}</span>{/if}
        {#if next.text}<span class="next-text">{next.text}</span>{/if}
      </div>
    </footer>
  {/if}
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; background: var(--v-void); overflow: hidden; -webkit-font-smoothing: antialiased; }
  .sr {
    height: 100dvh; display: flex; flex-direction: column; color: var(--v-txt);
    font-family: var(--f-body);
    padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
  }
  header { display: flex; align-items: center; gap: 12px; padding: 14px 18px; border-bottom: 1px solid rgba(255,255,255,.08); flex: 0 0 auto; }
  .brand { font-family: var(--f-head); font-weight: 700; font-size: 16px; color: var(--v-amber); }
  /* CONTRAST. This page is read on a phone, at arm's length, in a lit auditorium —
     by the preacher, mid-sermon. It is the least forgiving reading condition in the
     whole product, and it had the worst text in it.

     These were #6c6b71 (3.75:1) and, for .idle, #4a4a50 (2.25:1) — both below the
     WCAG AA floor of 4.5:1 on this background. #6c6b71 is the exact value app.css
     documents as REMOVED for failing AA; the console was fixed and the phone was
     left behind, because it hardcodes hexes instead of using the --v-* tokens.

     #88888d is --v-faint: 5.61:1 here. Still quiet, and actually readable. */
  .status { margin-left: auto; display: inline-flex; align-items: center; gap: 6px; font-family: var(--f-mono); font-size: 11px; color: var(--v-faint); }
  .status.on { color: var(--v-emerald); }
  .status i { width: 7px; height: 7px; border-radius: 50%; background: currentColor; }
  .status.on i { box-shadow: 0 0 8px currentColor; animation: p 1.7s ease-in-out infinite; }
  @keyframes p { 0%, 100% { opacity: 1; } 50% { opacity: .4; } }
  .clock { font-family: var(--f-mono); font-size: 13px; color: var(--v-dim); }
  main { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; padding: 24px; gap: 18px; min-height: 0; }
  .ref { font-family: var(--f-mono); font-size: clamp(13px, 3.5vw, 20px); letter-spacing: .18em; text-transform: uppercase; color: var(--v-amber); }
  .verse { font-family: var(--f-serif); font-size: clamp(26px, 7vw, 64px); line-height: 1.28; color: var(--v-txt); max-width: 16ch; }
  /* The DEFAULT resting state of the preacher's phone — the thing on screen before
     anything is fired, and therefore the text most likely to be looked at. It was
     2.25:1: the worst contrast in the product, in its least forgiving location. */
  .idle { font-family: var(--f-mono); color: var(--v-faint); font-size: 14px; letter-spacing: .1em; }
  .countdown { font-family: var(--f-mono); font-variant-numeric: tabular-nums; font-weight: 700;
    font-size: clamp(56px, 20vw, 160px); color: var(--v-amber); line-height: 1; letter-spacing: .02em; max-width: none; }
  /* Operator's cue note — confidence-monitor only, never on the main output. */
  .note { display: inline-flex; align-items: center; gap: 10px; max-width: 30ch; margin-top: 4px;
    padding: 10px 16px; border-radius: 12px; background: rgba(255,176,0,.1);
    border: 1px solid rgba(255,176,0,.32); color: var(--v-amber2);
    font-family: var(--f-body); font-size: clamp(14px, 2.6vw, 20px); line-height: 1.35; }
  .note-lbl { font-family: var(--f-mono); font-size: 9px; font-weight: 700; letter-spacing: .16em;
    text-transform: uppercase; color: var(--v-amber); flex: 0 0 auto; }
  /* Up-next panel — confidence info the preacher wants, kept off the main output. */
  .next { flex: 0 0 auto; display: flex; align-items: baseline; gap: 14px; padding: 14px 20px;
    border-top: 1px solid rgba(255,255,255,.08); background: rgba(255,255,255,.02); }
  .next-lbl { font-family: var(--f-mono); font-size: 10px; font-weight: 700; letter-spacing: .16em;
    text-transform: uppercase; color: var(--v-faint); flex: 0 0 auto; }
  .next-body { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .next-ref { font-family: var(--f-mono); font-size: 12px; letter-spacing: .06em; color: var(--v-amber);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .next-text { font-family: var(--f-head); font-size: 16px; color: var(--v-dim); line-height: 1.3;
    display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  @media (orientation: landscape) { .verse { font-size: clamp(28px, 6vw, 72px); max-width: 22ch; } }
  @media (prefers-reduced-motion: reduce) { .status.on i { animation: none; } }
</style>
