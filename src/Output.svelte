<script>
  import { onMount, onDestroy } from 'svelte';
  import { DEFAULT_TEMPLATE, builtinById } from './lib/templates.js';
  import TemplateRender from './lib/TemplateRender.svelte';
  import { parseTemplateOverride } from './lib/templates.js';
  import {
    isKeyedTemplate,
    resolveOutputTemplate,
    templateShows,
    setCountdownWarnDefault,
  } from './lib/layers.js';
  import { acceptsStageMessage, roleOf } from './lib/channelroles.js';
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
  // ── THE STANDING BACKGROUND (the second payload) ───────────────────────────
  //
  // `{ media_url, media_kind }`, or null. It is NOT a field on the content and
  // that is the whole point: the content is what the screen is showing, this is
  // what it is showing it ON, and a verse arriving must leave it exactly where it
  // is. Until it existed the two were one field, so a church could have scripture
  // or its own backdrop and never both.
  //
  // **A PANIC CONTROL TAKES IT.** `clear` and `black` set this to null on both
  // doors below, beside the `visible`/`black` they already set — the single most
  // important line in this file's half of the feature, because a clear that left
  // the church's picture on the wall is the worst class of bug in this product.
  // The hub empties its retained slot at the same instant, so a screen that
  // reconnects a second later is not painted it back either.
  let backdrop = null;
  /**
   * ONE WRITER, called from both doors, for the reason `applyRoles` above it
   * gives: a native output window has the Tauri bridge and no socket, a kiosk
   * page has the socket and no bridge, and a rule kept on one of the two is the
   * mistake this file's own comments count four times — here on the two screens
   * most often in the same room. An absent or blank URL is a take-down, never a
   * background with nothing in it.
   */
  function applyBackdrop(url, kind) {
    backdrop = url ? { media_url: url, media_kind: kind || 'image' } : null;
  }
  // THE OPERATOR'S CONFIGURED DEFAULT (`default_template_id`) — the LAST link in
  // the resolver's chain, applied only when nothing above it answered. Pushed by
  // the kiosk hub on connect and whenever the operator changes it, and mirrored
  // to a native output window over `output://default_template`.
  let defaultTpl = null;

  // ── THE STAGE MESSAGE, AND WHY THIS PAGE MAY REFUSE IT ─────────────────────
  //
  // `channels::stage_alert` publishes to EVERY kiosk client. It has to: the hub
  // records nothing about who connected and DECISIONS §35 is not being reversed,
  // so it cannot address one screen. Until now this page was safe by OMISSION —
  // it had no `stage_alert` branch at all, and docs/REBRAND.md §5's guarantee
  // ("no congregation screen can show it") rested on that absence.
  //
  // A `stage_message` layer binding ends the omission: a renderer now reads the
  // value, so the absence protects nothing and the refusal has to be explicit.
  // It lives HERE, at the receiver, because the receiver is the only party that
  // knows which screen it is — the URL is channel-keyed (DECISIONS §29) and the
  // backend publishes what each channel is for.
  //
  // `stageMessage` is renderer state and NEVER a field on `OutputContent`. On the
  // content it would be broadcast to every screen, and the only thing between it
  // and a lobby TV would be which layers that TV's template happens to have.
  let roles = {};
  let stageMessage = '';
  $: myRole = roleOf(roles, channelId);
  /**
   * The role map has changed. ONE writer, called from both doors, because a
   * screen that stops being a stage must lose the message AT ONCE: an operator
   * who moves the stage role off a tablet has said that tablet is a congregation
   * screen now, and a refusal that only applied to the NEXT message would leave
   * the last one painted on it for the rest of the service.
   *
   * Deliberately NOT a blanket `$:` that re-clears whenever the role is not
   * `stage`. That reads as belt and braces and is worse than either: it makes the
   * acceptance check in `stage_alert` redundant, so removing the check breaks no
   * test — and the message would still be ASSIGNED for an instant before the
   * reactive pass took it away, which on a congregation screen is a flash of
   * something private. Each path guards its own case, once.
   */
  function applyRoles(next) {
    roles = next && typeof next === 'object' ? next : {};
    if (!acceptsStageMessage(roleOf(roles, channelId))) stageMessage = '';
  }

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
    resolveOutputTemplate(t, override, !!content?.template_pinned, defaultTpl) || DEFAULT_TEMPLATE;
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
  // Desktop only — the picture already behind everything when this window opened,
  // on exactly the argument `loadLiveTransition` above makes. The kiosk hub
  // replays the retained background on `hello`; a native output window has no
  // socket, so without this read a projector opened mid-service is the one screen
  // in the building painting the words on black. Guarded the same way: a missing
  // command or a backend that says nothing leaves this screen with no backdrop,
  // which is the plain answer and the safe one, and never throws on a live output
  // page.
  async function loadLiveBackground() {
    try {
      const call = await invoke();
      const cur = await call('live_background');
      // Rust hands back `[url, kind]` or null.
      applyBackdrop(Array.isArray(cur) ? cur[0] : null, Array.isArray(cur) ? cur[1] : null);
    } catch {
      applyBackdrop(null, null);
    }
  }
  // Desktop only — the configured default already in force when this window
  // opened, on exactly the same argument as `loadLiveTransition` above and for
  // exactly the same reason. The kiosk hub seeds `defaultTpl` on `hello`; a
  // native output window has no socket and was seeded by NOTHING, so a projector
  // opened through `open_channel_output` on a channel that follows the content
  // look rendered the bundled Classic Serif until the operator happened to CHANGE
  // the default and `output://default_template` fired. That is the half of "the
  // default does not activate on all screens" this wave exists to close, hidden
  // because the other half works. Guarded the same way: a missing command or a
  // backend that says nothing leaves this screen following its template, never
  // throwing on a live output page.
  async function loadDefaultTemplate() {
    try {
      const call = await invoke();
      const raw = await call('get_setting', { key: 'default_template_id' });
      const id = parseInt(raw, 10);
      if (!Number.isFinite(id)) {
        defaultTpl = null;
        return;
      }
      defaultTpl = (await call('get_template', { id })) ?? null;
    } catch {
      defaultTpl = null;
    }
  }
  // Desktop only — what each screen is for, read when this window opens. Exactly
  // the argument `loadDefaultTemplate` above makes: the hub replays the role map
  // on `hello`, a native output window has no socket, and the event only fires
  // when the operator CHANGES something. Without this read a projector opened
  // mid-service knows nothing about itself until the next change.
  //
  // Guarded the same way — a missing command or a backend that says nothing
  // leaves this screen with no role, which is the refusing answer and the safe
  // one, and never throws on a live output page.
  async function loadChannelRoles() {
    try {
      const call = await invoke();
      const list = await call('list_output_channels');
      const next = {};
      for (const c of Array.isArray(list) ? list : []) {
        if (c?.role) next[String(c.id)] = c.role;
      }
      applyRoles(next);
    } catch {
      applyRoles({});
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

  /**
   * THE CONFIGURED WARNING WINDOW, DELIVERED RATHER THAN READ — RG-149(c).
   *
   * `Settings → General → Countdown warning` is applied through
   * `layers.js::setCountdownWarnDefault`, whose only writer is `stores/capture.js`
   * — a module a browser source cannot import, because it has no Tauri bridge. So
   * the figure rides with the content instead, on both doors, and this is where it
   * is applied. The RULE does not move: `countdownWarning` still ranks a threshold
   * chosen for one countdown ahead of this, and this ahead of the tenth-of-span
   * rule behind it.
   *
   * An absent figure resets to the shipped minute rather than leaving the last
   * delivered one standing — that is what the setting means when it is cleared, and
   * a screen warning at a figure nothing on the machine holds is the shape of
   * defect this whole chain exists to close.
   */
  function applyWarnDefault(ms) {
    setCountdownWarnDefault(ms);
  }

  function applyMessage(m) {
    if (m.kind === 'content') {
      // PER-SCREEN VISIBILITY. If THIS screen's template doesn't show this content
      // kind (e.g. a stage monitor set to scripture + songs + timer only, and a
      // picture just fired), ignore it and hold what's already up — the online
      // wall shows the picture, this screen keeps the passage.
      //
      // SITE 8 OF THE CONTENT-KIND SWEEP, THE KIOSK DOOR. Nothing changed: a
      // congregation timer arrives as `countdown`, which every `shows` list names,
      // and a programme timer is never published as content so it cannot reach this
      // page by any route. The TWIN of this line is the `output://content` listener
      // in `onMount` — the native window has the Tauri bridge and no socket, so a
      // filter written here and not there is the "guarantee kept on one door"
      // mistake, on the two screens most often in the same room.
      if (m.content_kind && !templateShows(t, m.content_kind)) return;
      // The override takes effect WITH the content, never before it — see the
      // snapshot comment at the top of this file.
      appliedTransition = pendingTransition;
      // `countdown_warn_ms` is copied across like every other field this door
      // rebuilds by hand: it is the threshold chosen for THIS countdown, and
      // `TemplateRender` reads it off the content. The list is the reason this door
      // has dropped fields before (`next_reference`), so a field added to the wire
      // and not added here is a kiosk screen disagreeing with the wall beside it.
      content = { kind: m.content_kind, reference: m.reference, text: m.text, translation: m.translation, media_url: m.media_url, media_kind: m.media_kind, template_json: m.template_json, template_pinned: m.template_pinned, countdown_to: m.countdown_to, countdown_from: m.countdown_from, countdown_paused_ms: m.countdown_paused_ms, countdown_done: m.countdown_done, countdown_warn_ms: m.countdown_warn_ms, stage_note: m.stage_note, next_reference: m.next_reference, next_text: m.next_text, service_started_at: m.service_started_at, service_target_ms: m.service_target_ms };
      // THE CONFIGURED DEFAULT, which this page cannot read for itself.
      applyWarnDefault(m.countdown_warn_default_ms);
      visible = true;
      black = false;
      // Report the paint back over the SAME socket the content came in on. This is
      // the only measurement of the real last leg — the church's own network, to a
      // browser source in OBS, to the projector — and it is the leg every other
      // instrument in this codebase has had to assume was fast.
      markOutput(m.trace_id, ws);
    } else if (m.kind === 'default_template') {
      // THE CONFIGURED DEFAULT, pushed by the hub on connect and whenever the
      // operator changes it. This screen follows the content look, so a change
      // here is a change to what it wears — it is applied live rather than at
      // the next reload, which is what "the default does not activate on all
      // screens" actually was.
      defaultTpl = m.template ?? null;
    } else if (m.kind === 'transition') {
      // HOW the next thing appears. Deliberately NOT applied here: it arms the
      // next content and repaints nothing. `mode: null` clears the override and
      // this screen goes back to following its template (DECISIONS §71 unchanged).
      noteTransition(m.mode, m.ms);
    } else if (m.kind === 'background') {
      // THE STANDING BACKGROUND, up or down. `media_url: null` is the take-down
      // and arrives as an explicit null rather than as silence, because an absent
      // frame cannot say "there is none now" and a screen that missed it would
      // carry the picture for the rest of the service.
      applyBackdrop(m.media_url, m.media_kind);
    } else if (m.kind === 'clear') {
      // A PANIC CONTROL NEVER TRANSITIONS. `clear` and `black` do not touch
      // `appliedTransition`, and `TemplateRender` has no `out:` transition at all
      // (DECISIONS §27), so a clear is instant at every duration the picker
      // offers. A blackout that could fade is a blackout that can be late.
      visible = false;
      black = false;
      // AND IT TAKES THE BACKGROUND. `Clear screens` means everything, and the
      // background is part of everything.
      backdrop = null;
    } else if (m.kind === 'black') {
      black = true;
      // On a band channel, blacking out means the band goes away — the camera
      // must not be covered.
      if (isBand) visible = false;
      // On EVERY channel the backdrop goes, band or not. The opaque overlay hides
      // it on an ordinary screen, but a keyed channel paints no overlay at all —
      // so leaving it here would put the church's picture over the live camera at
      // the one moment the operator asked for the camera alone.
      backdrop = null;
    } else if (m.kind === 'channel_roles') {
      // WHAT EVERY SCREEN IS FOR. Sent on every hello and whenever it changes, so
      // this page can answer the only question it asks of it: am I the stage?
      applyRoles(m.roles);
    } else if (m.kind === 'stage_alert') {
      // ONLY A STAGE. No role is not a stage — a lobby TV and a streaming feed
      // both arrive here with no role at all, and a filter whose default is yes
      // is not a filter. `text: null` (or blank) clears it; an alert is an
      // instruction about a moment, never a state of the wall, which is why the
      // hub does not retain it (rule 43, FRAME_VERDICTS).
      if (!acceptsStageMessage(myRole)) return;
      stageMessage = (m.text || '').trim();
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
      await loadLiveBackground();
      await loadDefaultTemplate();
      await loadChannelRoles();
      const { listen } = await import('@tauri-apps/api/event');
      unlisten.push(await listen('output://content', (e) => {
        // Per-screen visibility (see applyMessage) — hold what's up if this screen
        // doesn't show the fired kind. SITE 8's other half: the kiosk protocol
        // renames `kind` to `content_kind`, so the two doors read a differently
        // named field off differently shaped messages and only the rule is shared.
        // Swept with the kiosk door and needed nothing for the same reason.
        if (e.payload?.kind && !templateShows(t, e.payload.kind)) return;
        appliedTransition = pendingTransition;
        content = e.payload;
        // BOTH DOORS. The struct emit carries every field, so the chosen threshold
        // arrives here for free — the configured DEFAULT does not, because it is
        // module state rather than content, and applying it on one door only is the
        // mistake this file's own comments count four times. A projector on HDMI
        // and a browser source in OBS are usually in the same room, and a warning
        // colour that comes on at different moments on the two is worse than one
        // that comes on late on both.
        applyWarnDefault(e.payload?.countdown_warn_default_ms);
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
      // THE STANDING BACKGROUND, on the door a native window has. Its twin is the
      // `background` branch in `applyMessage`; a projector on HDMI and a browser
      // source in OBS are usually in the same room, and a backdrop that reached
      // one of the two is this file's own recurring mistake.
      unlisten.push(
        await listen('output://background', (e) =>
          applyBackdrop(e.payload?.media_url, e.payload?.media_kind),
        ),
      );
      // …AND THE PANIC CONTROLS TAKE IT, on this door too. The line that matters:
      // a clear means everything.
      unlisten.push(await listen('output://clear', () => { visible = false; black = false; backdrop = null; }));
      unlisten.push(
        await listen('output://black', () => {
          black = true;
          if (isBand) visible = false;
          backdrop = null;
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
      unlisten.push(
        await listen('output://default_template', (e) => {
          defaultTpl = e.payload?.template ?? null;
        }),
      );
      // BOTH DOORS. A native output window has the bridge and no socket; the
      // kiosk page has the socket and no bridge. A role change that reached one
      // of the two would leave a projector and a browser source disagreeing about
      // which of them may be shown a word meant for the preacher.
      unlisten.push(
        await listen('output://channel_roles', (e) => {
          applyRoles(e.payload?.roles);
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
  {backdrop}
  audio={isDesktop}
  stageMessage={stageMessage}
  transitionOverride={appliedTransition} />
<!-- BLACKOUT NEVER BLACKS OUT A LOWER THIRD. On a keyed channel "black" would
     paint an opaque rectangle over the live camera — the opposite of what the
     operator pressed it for. On that channel the panic control removes the
     BAND, which is all this channel was ever contributing, and the camera keeps
     going out. Every other channel goes properly black. -->
{#if black && !isBand}<div class="blackout"></div>{/if}

<style>
  /* Transparent by default — a template with a transparent background keys out
     for OBS/ATEM. Solid templates paint their own background in TemplateRender.

     THE TYPE BASE IS DECLARED HERE, AND IT IS NOT DECORATION (wave 5, Track E).
     These four properties used to arrive from `app.css`'s `body{}` rule, because
     `output.js` imported the operator console's whole stylesheet — which also put
     every unscoped console rule on the congregation's screen. The page now imports
     `tokens.css` and takes no rules, so it declares the base it always rendered
     with. `line-height` in particular is inherited by any template element that
     does not set its own (`.reference` is one), and it is an input to the fit
     loop: dropping it would have changed both what a reference looks like and the
     size the binary search settles on, on a wall, silently. Measured in a browser
     before and after — with the console sheet and without, these four are the
     whole difference. */
  :global(html, body) {
    margin: 0;
    height: 100%;
    background: transparent;
    overflow: hidden;
    font-family: var(--f-body);
    font-size: var(--v-fs-b1);
    line-height: 1.45;
    color: var(--v-txt);
  }
  /* Blackout: opaque black over everything (kills the screen, unlike clear). */
  .blackout {
    position: fixed;
    inset: 0;
    background: #000;
    z-index: 9999;
  }
</style>
