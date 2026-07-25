<script>
  import { onMount, onDestroy } from 'svelte';
  import { DEFAULT_TEMPLATE, builtinById } from './lib/templates.js';
  import TemplateRender from './lib/TemplateRender.svelte';
  import { parseTemplateOverride } from './lib/templates.js';
  import { isKeyedTemplate, resolveOutputTemplate, templateShows } from './lib/layers.js';
  import { resolveThemed, parseThemes } from './lib/themes.js';

  // Two modes, ONE renderer (TemplateRender): desktop (Tauri — DB template,
  // live edits over events) and kiosk/OBS (plain browser — built-in template by
  // id, state over the WebSocket hub). The page is transparent, so a template
  // with a transparent background (e.g. the lower third) lets an OBS/ATEM camera
  // source show through.
  const params = new URLSearchParams(location.search);
  const templateId = parseInt(params.get('template_id') || '1', 10);
  // The CHANNEL this output belongs to (0 = a raw template preview with no
  // channel). When the operator changes this screen's template, a channel-retemplate
  // broadcast arrives; this output swaps to the new template if the channel matches
  // — so a template change is live with NO URL change and NO re-configuration.
  const channelId = parseInt(params.get('channel') || '0', 10);

  let t = DEFAULT_TEMPLATE;
  let content = null;
  let visible = false;
  let black = false; // opaque blackout overlay

  // Per-content-type template: when the fired content carries a template override
  // (its content type's default / a cue's choice), render THAT; else the channel's
  // own template `t`.
  $: override = parseTemplateOverride(content?.template_json);
  // THE TRANSPARENCY LAW WINS OVER THE OVERRIDE. A keyed channel must NEVER render
  // opaque — an opaque content-type override (e.g. a full-screen scripture theme)
  // would blot out the very camera the lower third exists to caption. So on a
  // keyed channel an opaque override is ignored: the channel keeps its own keyed
  // template and the verse still flows into its band. Opaque channels apply the
  // override as before ("scripture looks like scripture"); a keyed override on a
  // keyed channel still applies.
  // `template_pinned` marks an override the operator DELIBERATELY chose for this
  // cue (a Planner item's own template) — it overrides the screen. A content-type
  // DEFAULT is not pinned and defers to the screen's own template.
  $: activeTemplate = resolveOutputTemplate(t, override, !!content?.template_pinned);
  // THE THEME LAYER. If the resolved template pins a theme (style.themeRef), fill
  // its unset style keys from that theme — the same merge the editor previews, so
  // the wall matches the editor. Custom themes are fetched on desktop (below);
  // kiosk/OBS has no DB, so `customThemes` stays [] and only BUILT-IN themes
  // resolve there. A template pinning a custom theme therefore themes on desktop
  // and degrades to its own look on a kiosk — never blanks (applyTheme is safe).
  // Style-only, so it can't change keyed-ness: isBand stays on activeTemplate.
  let customThemes = [];
  $: themedTemplate = resolveThemed(activeTemplate, customThemes);
  // "Keyed" for blackout purposes — resolved on what is ACTUALLY rendering.
  $: isBand = isKeyedTemplate(activeTemplate);
  // PAGE BACKGROUND. A KEYED (lower-third) channel stays transparent so OBS/ATEM
  // keys it over the camera. An OPAQUE (full-screen) channel is BLACK — so a
  // cleared or blacked-out screen is black, not the browser's default WHITE (a
  // white flash on the projector/stream is jarring). This is what makes "Clear"
  // and "Blackout" pleasant on a kiosk/browser output, which has no window
  // backdrop of its own (a native window already opens black).
  $: if (typeof document !== 'undefined') {
    const bg = isBand ? 'transparent' : '#000';
    document.documentElement.style.background = bg;
    document.body.style.background = bg;
  }
  let unlisten = [];
  let ws = null;
  let kioskClosed = false;

  async function invoke() {
    const core = await import('@tauri-apps/api/core');
    return core.invoke;
  }
  async function loadTemplate() {
    const call = await invoke();
    const tpl = await call('get_template', { id: templateId });
    if (tpl) t = tpl;
  }
  // Desktop only — the operator's custom themes, so a template pinning one wears
  // it on the real wall. Guarded: a missing command / corrupt blob leaves the
  // set empty (builtins still resolve), never throwing on a live output page.
  async function loadCustomThemes() {
    try {
      const call = await invoke();
      customThemes = parseThemes(await call('get_setting', { key: 'themes.custom' }));
    } catch {
      customThemes = [];
    }
  }
  async function fetchTemplate(id) {
    try {
      const call = await invoke();
      return await call('get_template', { id });
    } catch {
      return null; // kiosk/OBS has no backend — it gets the template over WS
    }
  }
  // Apply an edited template to whatever this output is showing, LIVE — no re-fire
  // needed. Two places it can be in use: the channel's own template, and the
  // content-type/cue OVERRIDE baked onto the content currently on screen. The
  // override is a snapshot (template_json), so refreshing it here is what makes an
  // edit re-render live scripture the instant it's saved.
  function applyTemplateUpdate(id, fresh) {
    if (!fresh) return;
    if (id === templateId) t = fresh;
    if (content) {
      const ov = parseTemplateOverride(content.template_json);
      if (ov && ov.id === id) content = { ...content, template_json: JSON.stringify(fresh) };
    }
  }

  function applyMessage(m) {
    if (m.kind === 'content') {
      // PER-SCREEN VISIBILITY. If THIS screen's template doesn't show this content
      // kind (e.g. a stage monitor set to scripture + songs + timer only, and a
      // picture just fired), ignore it and hold what's already up — the online
      // wall shows the picture, this screen keeps the passage.
      if (m.content_kind && !templateShows(t, m.content_kind)) return;
      content = { kind: m.content_kind, reference: m.reference, text: m.text, translation: m.translation, media_url: m.media_url, media_kind: m.media_kind, template_json: m.template_json, template_pinned: m.template_pinned, countdown_to: m.countdown_to, countdown_done: m.countdown_done, stage_note: m.stage_note, next_reference: m.next_reference, next_text: m.next_text, service_started_at: m.service_started_at, service_target_ms: m.service_target_ms };
      visible = true;
      black = false;
    } else if (m.kind === 'themes') {
      // The operator's custom themes, pushed by the hub on connect and whenever a
      // theme is saved. Lets THIS browser source resolve a template that pins a
      // custom theme; builtins it already knows (bundled). Safe-parsed.
      customThemes = parseThemes(JSON.stringify(m.themes ?? []));
    } else if (m.kind === 'clear') {
      visible = false;
      black = false;
    } else if (m.kind === 'black') {
      black = true;
      // On a band channel, blacking out means the band goes away — the camera
      // must not be covered.
      if (isBand) visible = false;
    } else if (m.kind === 'channel_template') {
      // This screen's assigned template was changed. Filter by our channel (the
      // hub broadcasts to all; each client applies only its own) — live, no re-copy.
      if (channelId && m.channel === channelId && m.template) t = m.template;
    } else if (m.kind === 'template' && m.template) {
      // The REAL saved template (with the operator's edits) — this is what makes
      // OBS/kiosk match the console preview exactly, and updates live on save.
      // Applies to the channel template AND to a matching on-screen override, so
      // editing the template that a live verse is using re-renders it at once.
      applyTemplateUpdate(m.id, m.template);
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
      await loadCustomThemes();
      const { listen } = await import('@tauri-apps/api/event');
      unlisten.push(await listen('output://content', (e) => {
        // Per-screen visibility (see applyMessage) — hold what's up if this screen
        // doesn't show the fired kind.
        if (e.payload?.kind && !templateShows(t, e.payload.kind)) return;
        content = e.payload;
        visible = true;
        black = false;
      }));
      unlisten.push(await listen('output://clear', () => { visible = false; black = false; }));
      unlisten.push(
        await listen('output://black', () => {
          black = true;
          if (isBand) visible = false;
        }),
      );
      unlisten.push(
        await listen('template://updated', async (e) => {
          const id = e.payload;
          applyTemplateUpdate(id, await fetchTemplate(id));
        }),
      );
      unlisten.push(
        await listen('channel://retemplate', (e) => {
          // This screen's assigned template was CHANGED (not edited). Swap to the
          // new one live if it is our channel — no reload, no URL change.
          if (channelId && e.payload?.channel === channelId && e.payload?.template) {
            t = e.payload.template;
          }
        }),
      );
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

<TemplateRender template={themedTemplate} content={visible ? content : null} />
<!-- BLACKOUT NEVER BLACKS OUT A LOWER THIRD. On a keyed channel "black" would
     paint an opaque rectangle over the live camera — the opposite of what the
     operator pressed it for. On that channel the panic control removes the
     BAND, which is all this channel was ever contributing, and the camera keeps
     going out. Every other channel goes properly black. -->
{#if black && !isBand}<div class="blackout"></div>{/if}

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
