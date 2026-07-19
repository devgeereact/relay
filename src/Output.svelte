<script>
  import { onMount, onDestroy } from 'svelte';
  import { DEFAULT_TEMPLATE, builtinById } from './lib/templates.js';
  import TemplateRender from './lib/TemplateRender.svelte';
  import { parseTemplateOverride } from './lib/templates.js';

  // Two modes, ONE renderer (TemplateRender): desktop (Tauri — DB template,
  // live edits over events) and kiosk/OBS (plain browser — built-in template by
  // id, state over the WebSocket hub). The page is transparent, so a template
  // with a transparent background (e.g. the lower third) lets an OBS/ATEM camera
  // source show through.
  const params = new URLSearchParams(location.search);
  const templateId = parseInt(params.get('template_id') || '1', 10);

  let t = DEFAULT_TEMPLATE;
  let content = null;
  let visible = false;
  let black = false; // opaque blackout overlay

  // Per-content-type template: when the fired content carries a template
  // override (its content type's default), render THAT; else the channel's own.
  $: activeTemplate = parseTemplateOverride(content?.template_json) ?? t;
  let unlisten = [];
  let ws = null;
  let kioskClosed = false;
  // Video sound is enabled on the NATIVE output window only (the one running on
  // the operator's machine, wired to the house speakers). The kiosk/OBS page is
  // a browser source: OBS captures and mixes its audio itself, so unmuting there
  // would push unexpected audio into a stream the operator did not ask for.
  let isDesktop = false;

  async function invoke() {
    const core = await import('@tauri-apps/api/core');
    return core.invoke;
  }
  async function loadTemplate() {
    const call = await invoke();
    const tpl = await call('get_template', { id: templateId });
    if (tpl) t = tpl;
  }

  function applyMessage(m) {
    if (m.kind === 'content') {
      content = { reference: m.reference, text: m.text, translation: m.translation, media_url: m.media_url, media_kind: m.media_kind, countdown_to: m.countdown_to, countdown_done: m.countdown_done };
      visible = true;
      black = false;
    } else if (m.kind === 'clear') {
      visible = false;
      black = false;
    } else if (m.kind === 'black') {
      black = true;
    } else if (m.kind === 'template' && m.id === templateId && m.template) {
      // The REAL saved template (with the operator's edits) — this is what makes
      // OBS/kiosk match the console preview exactly, and updates live on save.
      t = m.template;
    }
  }

  function connectKiosk(host) {
    if (kioskClosed) return;
    try {
      ws = new WebSocket(`ws://${host}:8031`);
      ws.onopen = () => {
        // Ask the hub for this channel's real template.
        try {
          ws.send(JSON.stringify({ kind: 'hello', template_id: templateId }));
        } catch {
          /* ignore */
        }
      };
      ws.onmessage = (ev) => {
        try {
          applyMessage(JSON.parse(ev.data));
        } catch {
          /* ignore */
        }
      };
      ws.onclose = () => {
        if (!kioskClosed) setTimeout(() => connectKiosk(host), 1500);
      };
      ws.onerror = () => {
        try {
          ws.close();
        } catch {
          /* onclose retries */
        }
      };
    } catch {
      if (!kioskClosed) setTimeout(() => connectKiosk(host), 1500);
    }
  }
  function startKiosk() {
    t = builtinById(templateId);
    connectKiosk(location.hostname || 'localhost');
  }

  onMount(async () => {
    try {
      await loadTemplate();
      const { listen } = await import('@tauri-apps/api/event');
      unlisten.push(await listen('output://content', (e) => { content = e.payload; visible = true; black = false; }));
      unlisten.push(await listen('output://clear', () => { visible = false; black = false; }));
      unlisten.push(await listen('output://black', () => (black = true)));
      unlisten.push(await listen('template://updated', (e) => { if (e.payload === templateId) loadTemplate(); }));
      isDesktop = true;
    } catch {
      startKiosk();
    }
  });
  onDestroy(() => {
    unlisten.forEach((u) => u());
    kioskClosed = true;
    if (ws) ws.close();
  });
</script>

<TemplateRender template={activeTemplate} content={visible ? content : null} audio={isDesktop} />
{#if black}<div class="blackout"></div>{/if}

<style>
  /* Transparent by default — a template with a transparent background keys out
     for OBS/ATEM. Solid templates paint their own background in TemplateRender. */
  :global(html, body) {
    margin: 0;
    height: 100%;
    background: transparent;
    overflow: hidden;
  }
  /* Blackout: opaque black over everything (kills the screen, unlike clear). */
  .blackout {
    position: fixed;
    inset: 0;
    background: #000;
    z-index: 9999;
  }
</style>
