<script>
  import { onMount, onDestroy } from 'svelte';
  import { DEFAULT_TEMPLATE, builtinById } from './lib/templates.js';
  import TemplateRender from './lib/TemplateRender.svelte';
  import { parseTemplateOverride } from './lib/templates.js';
  import { isKeyedTemplate, resolveOutputTemplate, templateShows } from './lib/layers.js';
  import { resolveTokens } from './lib/styletokens.js';
  import { markOutput } from './lib/latency.js';
  import { startBeat, paintState } from './lib/outputHealth.js';

  // Two modes, ONE renderer (TemplateRender): desktop (Tauri — DB template,
  // live edits over events) and kiosk/OBS (plain browser — built-in template by
  // id, state over the WebSocket hub). The page is transparent, so a template
  // with a transparent background (e.g. the lower third) lets an OBS/ATEM camera
  // source show through.
  const params = new URLSearchParams(location.search);
  // NO `template_id` MEANS THE SCREEN HAS NO LOOK OF ITS OWN and follows the
  // content look (DECISIONS §70). Defaulting to 1 here made a follower's browser
  // source wear built-in 1 until a `channel_template` message arrived.
  const templateIdParam = params.get('template_id');
  const templateId = templateIdParam == null ? null : parseInt(templateIdParam, 10) || null;
  // The CHANNEL this output belongs to (0 = a raw template preview with no
  // channel). When the operator changes this screen's template, a channel-retemplate
  // broadcast arrives; this output swaps to the new template if the channel matches
  // — so a template change is live with NO URL change and NO re-configuration.
  const channelId = parseInt(params.get('channel') || '0', 10);

  let t = DEFAULT_TEMPLATE;
  let content = null;
  let visible = false;
  let black = false; // opaque blackout overlay

  // ── THE OPERATOR'S TRANSITION OVERRIDE, SNAPSHOTTED (DECISIONS §84) ──────────
  //
  // Two variables, and the difference between them is the whole rule.
  //
  // `pendingTransition` is what the operator has chosen. It arrives on its own
  // frame, at a moment of the operator's choosing — which on a console is the
  // point (§8: "choosing one replays it on the programme at once"), and on a
  // CONGREGATION SCREEN would be a repaint nobody asked for. `TemplateRender`
  // keys its slide on the override, so passing it straight through would make a
  // verse that is already up re-animate the instant a preference changed,
  // mid-reading, on the wall.
  //
  // `appliedTransition` is therefore only moved forward when CONTENT arrives, so
  // the re-key happens exactly once, together with the thing it is transitioning.
  // A preference change alone paints nothing here, which is also what makes the
  // frame safe to publish during a rehearsal.
  //
  // `undefined` would mean "follow the store" inside TemplateRender; this page
  // passes an explicit value (null = follow the template) so a console store can
  // never be what a wall reads.
  let pendingTransition = null;
  let appliedTransition = null;
  const noteTransition = (mode, ms) => {
    pendingTransition = mode ? { mode, ms } : null;
  };

  // Per-content-type template: when the fired content carries a template override
  // (its content type's default / a cue's choice), render THAT; else the channel's
  // own template `t`.
  $: override = parseTemplateOverride(content?.template_json);
  // THE TRANSPARENCY LAW WINS OVER THE OVERRIDE. A keyed channel must NEVER render
  // opaque — an opaque content-type override (e.g. a full-screen scripture look)
  // would blot out the very camera the lower third exists to caption. So on a
  // keyed channel an opaque override is ignored: the channel keeps its own keyed
  // template and the verse still flows into its band. Opaque channels apply the
  // override as before ("scripture looks like scripture"); a keyed override on a
  // keyed channel still applies.
  // `template_pinned` marks an override the operator DELIBERATELY chose for this
  // cue (a Planner item's own template) — it overrides the screen. A content-type
  // DEFAULT is not pinned and defers to the screen's own template.
  // DEFAULT_TEMPLATE is the floor: a screen that follows the content look, when
  // no content look is set either, still has to paint something legible rather
  // than nothing at all.
  $: activeTemplate =
    resolveOutputTemplate(t, override, !!content?.template_pinned) || DEFAULT_TEMPLATE;
  // Set on mount; a no-op until then so onDestroy is safe if mounting threw.
  let stopBeat = () => {};
  // LAYER STYLE TOKENS. A layer bound to `theme:accent` resolves against the
  // template's own style, so a stage or confidence starter wears whatever
  // template it was dropped into. Style-only, so it can't change keyed-ness:
  // isBand stays on activeTemplate.
  $: renderedTemplate = resolveTokens(activeTemplate);
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
    // A screen with NO template of its own follows the content look, so `null`
    // here is an answer rather than a missing one (DECISIONS §70).
    if (templateId == null) {
      t = null;
      return;
    }
    const tpl = await call('get_template', { id: templateId });
    t = tpl ?? null;
  }
  // Desktop only — the override already in force when this window opened. The
  // kiosk hub replays it on `hello`; a native output window has no socket, so
  // without this read a projector opened mid-service is the one screen still
  // cutting. Guarded: a missing command or a backend that says nothing leaves
  // the screen following its template, never throwing on a live output page.
  async function loadLiveTransition() {
    try {
      const call = await invoke();
      const cur = await call('live_transition');
      // Rust hands back `[mode, ms]` or null.
      noteTransition(Array.isArray(cur) ? cur[0] : null, Array.isArray(cur) ? cur[1] : null);
    } catch {
      noteTransition(null, null);
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
      // The override takes effect WITH the content, never before it — see the
      // snapshot comment at the top of this file.
      appliedTransition = pendingTransition;
      content = { kind: m.content_kind, reference: m.reference, text: m.text, translation: m.translation, media_url: m.media_url, media_kind: m.media_kind, template_json: m.template_json, template_pinned: m.template_pinned, countdown_to: m.countdown_to, countdown_from: m.countdown_from, countdown_paused_ms: m.countdown_paused_ms, countdown_done: m.countdown_done, stage_note: m.stage_note, next_reference: m.next_reference, next_text: m.next_text, service_started_at: m.service_started_at, service_target_ms: m.service_target_ms };
      visible = true;
      black = false;
      // Report the paint back over the SAME socket the content came in on. This is
      // the only measurement of the real last leg — the church's own network, to a
      // browser source in OBS, to the projector — and it is the leg every other
      // instrument in this codebase has had to assume was fast.
      markOutput(m.trace_id, ws);
    } else if (m.kind === 'transition') {
      // HOW the next thing appears. Deliberately NOT applied here: it arms the
      // next content and repaints nothing. `mode: null` clears the override and
      // this screen goes back to following its template (DECISIONS §71 unchanged).
      noteTransition(m.mode, m.ms);
    } else if (m.kind === 'clear') {
      // A PANIC CONTROL NEVER TRANSITIONS. `clear` and `black` do not touch
      // `appliedTransition`, and `TemplateRender` has no `out:` transition at all
      // (DECISIONS §27), so a clear is instant at every duration the picker
      // offers. A blackout that could fade is a blackout that can be late.
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
      // `template: null` is the operator setting this screen to follow the content
      // look. Ignoring a null (which `if (m.template)` did) leaves an open screen
      // wearing the look it was given until something reloads it.
      if (channelId && m.channel === channelId && 'template' in m) t = m.template ?? null;
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
    // A kiosk client has no database, so it resolves its id against the bundled
    // built-ins — and a follower resolves to nothing, deliberately.
    t = templateId == null ? null : builtinById(templateId);
    connectKiosk(location.hostname || 'localhost');
  }

  onMount(async () => {
    try {
      await loadTemplate();
      await loadLiveTransition();
      const { listen } = await import('@tauri-apps/api/event');
      unlisten.push(await listen('output://content', (e) => {
        // Per-screen visibility (see applyMessage) — hold what's up if this screen
        // doesn't show the fired kind.
        if (e.payload?.kind && !templateShows(t, e.payload.kind)) return;
        appliedTransition = pendingTransition;
        content = e.payload;
        visible = true;
        black = false;
        // The native output window has the bridge, not the kiosk socket.
        markOutput(e.payload?.trace_id);
      }));
      // A NATIVE OUTPUT WINDOW IS NOT ON THE KIOSK HUB. It has the Tauri bridge
      // and no socket, so the override has to arrive on both doors or the
      // projector on HDMI and the browser source in OBS would transition
      // differently — the "guarantee kept on one door" mistake, on the two screens
      // that are most often in the same room.
      unlisten.push(await listen('output://transition', (e) => noteTransition(e.payload?.mode, e.payload?.ms)));
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
          // `template: null` means the operator set this screen to FOLLOW the
          // content look, which is news the screen has to act on — testing the
          // template for truthiness would drop it silently (DECISIONS §70).
          if (channelId && e.payload?.channel === channelId && 'template' in (e.payload ?? {})) {
            t = e.payload.template ?? null;
          }
        }),
      );
      isDesktop = true;
    } catch {
      startKiosk();
    }
    // START REPORTING LAST, and start it on BOTH paths.
    //
    // Last, because a beat sent before the listeners (or the socket) are up would
    // claim a screen was working during the one window in which it demonstrably was
    // not yet. Both paths, because the projector on HDMI and the browser source in
    // OBS fail in exactly the same invisible way, and a health signal that only
    // covered one of them would be the "guarantee kept on one door" mistake this
    // repository has now made four times.
    //
    // `ws` is read through a getter: the kiosk socket is replaced on every
    // reconnect, so a captured reference would keep beating into a dead one.
    stopBeat = startBeat({
      channelId,
      getState: () => paintState({ black, visible, content }),
      getWs: () => ws,
    });
  });
  onDestroy(() => {
    // Stop reporting FIRST. Anything after this is a page on its way out, and a
    // beat from it would say "still painting" about a screen that is closing.
    stopBeat();
    unlisten.forEach((u) => u());
    kioskClosed = true;
    if (ws) ws.close();
  });
</script>

<TemplateRender
  template={renderedTemplate}
  content={visible ? content : null}
  audio={isDesktop}
  transitionOverride={appliedTransition} />
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
