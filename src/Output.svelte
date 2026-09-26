<script>
  import { onMount, onDestroy } from 'svelte';
  import { DEFAULT_TEMPLATE, builtinById } from './lib/templates.js';
  import { sameHostMediaUrl } from './lib/outputurl.js';
  import TemplateRender from './lib/TemplateRender.svelte';
  import { parseTemplateOverride } from './lib/templates.js';
  import {
    isKeyedTemplate,
    resolveContentOverride,
    resolveOutputTemplate,
    channelLookTemplate,
    channelShowsKind,
    templateShows,
    setCountdownWarnDefault,
  } from './lib/layers.js';
  import { acceptsStageMessage, roleOf } from './lib/channelroles.js';
  import { readTimerSize } from './lib/stagelayout.js';
  import { railScale } from './lib/bigstagetimer.js';
  import { resolveTokens } from './lib/styletokens.js';
  import { markOutput } from './lib/latency.js';
  import { startBeat, paintState, BEAT_INTERVAL_MS } from './lib/outputHealth.js';

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

  // ── THE LOOKS THIS SCREEN CAN BE ASKED TO WEAR, BY ID ───────────────────────
  //
  // A per-kind CONTENT LOOK reaches this page as `content.template_id` and as
  // NOTHING ELSE. That is deliberate and it is a hard performance rule recorded
  // at `main::cue_or_content_tpl`: one content look carrying an embedded `data:`
  // image was 13 MB, and serialising it onto every fire made verses take seconds
  // to leave the machine. The id costs a settings lookup; the JSON costs the
  // service. So the id rides and the bytes do not.
  //
  // Which means the bytes have to be HERE ALREADY when the id arrives, and this
  // is where they are kept: `{ [id]: template }`, filled once per connect and
  // never per fire. Until it existed nothing on this page read `template_id` at
  // all, so a screen set to follow the content look wore the configured default
  // for the life of the product — the id crossed the wire on both doors and died
  // at both receivers.
  //
  // FILLED ON BOTH DOORS, and that is the whole shape of it. A native output
  // window has the Tauri bridge and no socket, so it READS the looks (below); a
  // browser source has the socket and no bridge, so the hub SENDS them, in the
  // hello reply's configuration block — before the retained frame they dress, per
  // rule 43. A projector on HDMI and an OBS source in the same room wearing
  // different templates is the "guarantee kept on one door" mistake this file's
  // own comments already count five times.
  //
  // REPLACED, never mutated: Svelte reacts to the assignment, and a mutated
  // object would leave `override` reading a cache it cannot see has changed.
  let lookCache = {};
  function cacheTemplate(id, tpl) {
    if (id == null || !tpl) return;
    lookCache = { ...lookCache, [id]: tpl };
  }

  // ── WHAT THIS SCREEN WEARS FOR EACH KIND OF CONTENT ─────────────────────────
  //
  // `{ "1": { "scripture": 9, "song": 12 } }`, keyed by channel id (DECISIONS
  // §97). Ids only: the bytes are in `lookCache` above, for the 13 MB reason
  // recorded there, and this map is the CHOICE rather than the template.
  //
  // THE WHOLE SET ARRIVES EVERY TIME, never a delta — the same rule `screen_state`
  // and the retained service clocks follow, and `{}` is an ANSWER rather than
  // silence. A
  // page that could not tell "nobody has told me" from "I have no per-kind look"
  // would resolve the next fire against the wrong template for exactly one frame,
  // and the frame after a browser source restarts mid-reading is the one a
  // congregation is looking at.
  //
  // FILLED ON BOTH DOORS, like everything else on this page: a native output
  // window READS it over the Tauri bridge and hears `output://channel_looks`; a
  // browser source is SENT it in the hello reply's configuration block, before the
  // retained frame it dresses. One door only would put a projector on HDMI and an
  // OBS source in the same room in different templates for the same verse, which
  // is the whole failure this feature exists to make impossible.
  //
  // REPLACED, never mutated, for the same reason as `lookCache`.
  let channelLooks = {};
  function applyChannelLooks(next) {
    channelLooks = next && typeof next === 'object' && !Array.isArray(next) ? next : {};
  }

  // ── WHICH KINDS THIS SCREEN SHOWS AT ALL (DECISIONS §98) ────────────────────
  //
  // `{ "1": ["scripture", "song"] }`, keyed by channel id. A screen with no
  // opinion is OMITTED, and `{}` means no screen anywhere has one — two different
  // absences, both of which have to survive the wire, because the first means
  // "follow the template" and the second means "nobody has set this up" and they
  // resolve to the same behaviour for different reasons.
  //
  // ANDed with `templateShows` at the two call sites below, never replacing it: a
  // template's `shows` is what it CAN render and this is what the screen is FOR,
  // so this can only narrow. It is never consulted for `clear` or `black` — see
  // `channelShowsKind`, and rule 15.
  //
  // Whole set every time, both doors, `{}` is an answer: the same three rules as
  // the look map above it, for the same three reasons.
  let channelShows = {};
  function applyChannelShows(next) {
    channelShows = next && typeof next === 'object' && !Array.isArray(next) ? next : {};
  }
  /**
   * THE ONE PLACE THE TWO HALVES ARE ANDed.
   *
   * Both doors call this and neither answers the question itself, because a rule
   * kept on one of two surfaces is the mistake this file's own comments count
   * seven times — and here the cost is a countdown on the wall the operator took
   * it off, on whichever of the projector and the OBS source they are not looking
   * at.
   */
  function paintsKind(kind) {
    return templateShows(t, kind) && channelShowsKind(channelShows, channelId, kind);
  }

  /**
   * IS THIS CUE FOR THIS SCREEN? (RG-161)
   *
   * `channels` on a content frame is the set of screens the cue names. The hub
   * cannot address one client (DECISIONS §35), so the routing is the
   * receiver's — the same mechanic `channel_template` and the Stage Message
   * already use, and the reason each of those had to be fixed on BOTH doors.
   *
   * ABSENT, NULL OR MALFORMED IS EVERY SCREEN, and that is the safe direction:
   * every cue built before this existed says nothing about screens, and
   * content that silently reaches nothing is worse than content that reaches
   * more than it had to.
   *
   * An EMPTY array reaches nothing, deliberately — a cue that names no screen
   * is a real thing to ask for and must not be read as "all of them".
   *
   * Channel 0 is a raw preview belonging to no screen. It paints everything: it
   * is not a screen anybody is watching, and a preview that silently dropped a
   * targeted cue would be a preview that lies about the plan.
   */
  function paintsHere(list) {
    if (!Array.isArray(list)) return true;
    if (!channelId) return true;
    return list.includes(channelId);
  }

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
  /**
   * HOW BIG THE OPERATOR ASKED THIS SCREEN'S CLOCK TO BE — RG-265.
   *
   * `normal` until a `stage_zones` frame says otherwise, which is what a layout
   * saved before the key existed and an install that never opened the control
   * both mean.
   */
  let timerSize = 'normal';
  let stageMessage = '';
  /** Note or alarm (DECISIONS §116). False is the safe default on every door. */
  let stageUrgent = false;
  /**
   * WHERE THIS SCREEN'S CLIP IS — `{ pos_ms, dur_ms, paused }`, or `null`.
   *
   * Written by `TemplateRender` from the element that is actually playing, and
   * read once per beat. It is deliberately NOT derived from the content frame: the
   * frame says which clip was sent, and this says where that clip has got to on
   * this screen, which is the only one of the two an operator can time a cue
   * against.
   */
  let mediaReport = null;
  /**
   * THE PICTURE OR CLIP THIS SCREEN COULD NOT LOAD, in words, or `null` (O-4).
   * Rides the beat so the desk stops calling this screen On Air over a blank
   * frame. The renderer reports; this only carries.
   */
  let mediaError = null;
  const noteMediaError = (e) => {
    mediaError = e ? `${e.kind} not loading · ${e.url}` : null;
  };
  /**
   * WHAT THE OPERATOR HAS ASKED THE CLIP TO DO — `{ paused, loop, replayEpoch }`.
   *
   * `null` until somebody asks for something, which is the default a clip is fired
   * with: playing, not looping.
   */
  let mediaTransport = null;
  const noteMedia = (m) => {
    mediaReport = m;
  };
  /**
   * A CLIP THAT HAS COME OFF REPORTS NOTHING, AND THAT NEEDS SAYING OUT LOUD.
   *
   * When the content changes from a clip to a verse the `<video>` is destroyed, and
   * a destroyed element fires no event — so the last position it reported would sit
   * here for the rest of the service and the console would count down a clip nobody
   * is watching. The frame that replaced it is the thing that knows, so the clearing
   * happens here rather than being waited for.
   */
  $: if (!shownContent?.media_url) mediaReport = null;
  /**
   * A TRANSPORT BELONGS TO THE CLIP IT WAS PRESSED FOR.
   *
   * The hub empties its retained copy on any content frame, and this is the same
   * decision on this side. Without it a screen would carry a Pause across a fire
   * and the next video would arrive already held, with nothing in the product to
   * say why — and a reconnect would not correct it, because there would be no
   * retained frame left to send.
   */
  $: if (!shownContent?.media_url) mediaTransport = null;
  // ── THE STAGE TIMERS ────────────────────────────────────────────────────────
  //
  // `r6-contracts.test.js` recorded `timer: false` for this page, with a reason
  // that is still true word for word: *"Sermon · 4:12 left" behind a preacher is
  // the running order in front of the whole building.* That sentence is about a
  // CONGREGATION screen. A screen holding `role === 'stage'` is not one
  // (DECISIONS §89), which is why this is now a role gate rather than an absence
  // and why the safety argument survives whole.
  //
  // THE SET IS HELD, THE RENDER IS GATED - and the difference from `stageMessage`
  // above is deliberate rather than sloppy. A Stage Message is a private sentence
  // and is refused at the door, never assigned on a screen that may not have it.
  // The programme is the same frame the hub sends every client, so holding it
  // buys the page nothing it did not already receive, and gating the RENDER is
  // what makes the guarantee ORDERING-PROOF: `channel_roles` happens to precede
  // the retained `timer` frame on hello today, and a refusal at the door would
  // silently depend on that staying true for the rest of the service.
  //
  // One derived value feeds the one prop, so there is a single door (rule 36).
  let stageTimerSet = [];

  // ── THE OPERATOR TOOK THIS SCREEN OUT OF THE WALL ───────────────────────────
  //
  // `clear_screens` and `blackout` address every screen and ask nothing about
  // which — that is what makes them panic controls (rule 15, DECISIONS §20) and
  // they are untouched. `screen_state` is the separate, deliberate control beside
  // them: "take the lobby TV down but leave the wall live".
  //
  // Refused, or rather APPLIED, at the receiver, for the third time in this file
  // and for the same reason as `channel_template` and `stage_alert`: the hub
  // broadcasts to everybody and records nothing about who connected (DECISIONS
  // §35), so the only party that knows which screen this is, is this screen.
  //
  // THE WHOLE SET ARRIVES EVERY TIME, never a delta — the same rule the programme
  // timers follow. A page that missed one frame would otherwise be wrong about
  // itself for the rest of the service with no way to find out, and the thing it
  // would be wrong about is whether a congregation can see anything.
  //
  // `content` KEEPS BEING TRACKED WHILE THIS SCREEN IS DOWN, and only the
  // rendering is withheld. Throwing the content away would make coming back up
  // mean "blank until the next fire", which is RG-129 — a screen that rejoined
  // mid-service and stayed black — reached through a control instead of a
  // reconnect.
  let downMode = null; // 'clear' | 'black' | null
  /**
   * The set has changed. `screens` is `{"4":"clear"}`, keyed by channel id, and
   * `{}` is the answer that every screen is up.
   *
   * Keys arrive as strings because JSON objects have string keys and this page's
   * channel is a number — the same comparison `roleOf` documents, and the same
   * way a filter that looks right comes to refuse everything.
   *
   * A page on NO channel (a raw template preview, `?channel=` absent, parsed to
   * 0) can never be named: channel ids start at 1, and a preview that belongs to
   * no screen must not be taken down by a decision about a screen.
   */
  function applyScreenState(screens) {
    if (!channelId || !screens || typeof screens !== 'object') {
      downMode = null;
      return;
    }
    const mine = screens[String(channelId)];
    downMode = mine === 'clear' || mine === 'black' ? mine : null;
  }
  // WHAT IS ACTUALLY PAINTED. Two derived facts rather than two more writers of
  // `visible` and `black`: a control that WROTE those would be indistinguishable
  // from a whole-wall clear one line later, and coming back up would then have
  // nothing to come back to.
  $: shownContent = visible && !downMode ? content : null;
  // AND THE BACKDROP GOES DOWN WITH IT — a point neither branch could see alone.
  //
  // The per-screen control and the standing background landed on two branches on
  // the same day. Separately each is right; together they ask a question neither
  // was in a position to answer: what does a screen the operator has taken OUT of
  // the wall paint? A backdrop is congregation furniture, so a lobby TV that has
  // been taken down while still showing the church's picture has not been taken
  // down — it has been half taken down, which is the state `downMode` exists to
  // make impossible.
  //
  // Derived, not written, for the reason immediately above: a writer here would be
  // indistinguishable from a whole-wall clear, and coming back up would have
  // nothing to come back to. The retained hub slot still holds the picture, so
  // restoring the screen restores it.
  $: shownBackdrop = downMode ? null : backdrop;
  $: shownBlack = black || downMode === 'black';
  $: myRole = roleOf(roles, channelId);
  // ONLY A STAGE, and only a screen the operator has left in the wall. The second
  // half is the same reasoning as `shownContent` and `shownBackdrop` above: a
  // screen that has been taken down paints nothing, and a programme rail left
  // standing on it would be half taken down.
  $: shownProgramme = acceptsStageMessage(myRole) && !downMode ? stageTimerSet : [];
  // AND THE MESSAGE GOES DOWN WITH THE SCREEN TOO, for the same reason and by the
  // same mechanism - derived, never written, so coming back up has something to
  // come back to.
  $: shownStageMessage = downMode ? '' : stageMessage;
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
    // The programme needs no line here: `shownProgramme` is derived from
    // `myRole`, so a screen that stops being the stage loses the rail in the same
    // reactive pass, before anything paints. The Stage Message cannot be handled
    // that way - see the note at `stageTimerSet` for the difference.
  }

  /**
   * A PANIC CONTROL TAKES THE STAGE MESSAGE WITH IT - DECISIONS §91.
   *
   * `Stage.svelte` clears its own `alert` on exactly these two kinds and says at
   * the line why: the panel IS the screen, so a survivor meant an operator
   * pressed `B`, whose whole meaning is *every output goes opaque black*, and the
   * preacher's screen stayed the brightest thing in the room under a control the
   * console had just reported succeeding.
   *
   * This page used to keep the text and merely stop rendering it - the layer
   * stack went away with `content` and nothing reset the state - so the next
   * verse fired painted a private word nobody had re-sent (RG-156). It reset
   * nothing because nothing on this page was full-bleed; the alert panel is, so
   * the reset is now load-bearing rather than tidy.
   *
   * ONE WRITER, called from both kinds, because the harsher control must never do
   * less than the milder one. A rail of Stage Timers is deliberately NOT touched:
   * §91's line is between a thing that SAYS something and a thing that COUNTS.
   */
  function takeDownStageMessage() {
    stageMessage = '';
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
  //
  // TWO FORMS, ONE ANSWER. A cue's pinned choice arrives as JSON; a content look
  // arrives as an id against `lookCache` above. This used to be
  // `parseTemplateOverride(content?.template_json)` alone, which is null BY
  // CONSTRUCTION for a content look — so the second form resolved to nothing and
  // "Follow the content look" was a setting with no screen that could read it.
  // `resolveContentOverride` is the ONE place the two forms are reconciled, and
  // the console's program pane and the Outputs tile call the same function, so no
  // surface can reach a different conclusion about the same screen.
  $: override = resolveContentOverride(content, lookCache);
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
  // THIS SCREEN'S OWN LOOK FOR THIS KIND — rung 3, above its blanket template and
  // below a cue's deliberate pin (DECISIONS §97). Passed as an ARGUMENT rather
  // than resolved by a second call to `resolveOutputTemplate`, and that is not a
  // tidy-up: the two-call form applies the transparency law BETWEEN the per-kind
  // look and the blanket template, so an opaque Announcement look deliberately
  // chosen for a lower-third screen would be discarded in silence. The reasoning
  // is at the resolver; the consequence is that the operator's choice moves
  // nothing, which is §29's original complaint verbatim.
  //
  // `content?.kind` is the same field on both doors: the kiosk protocol calls it
  // `content_kind` on the wire and `applyMessage` rebuilds it as `kind`, so the
  // rule is shared and only the wire shape differs.
  $: kindLook = channelLookTemplate(channelLooks, channelId, content?.kind, lookCache);
  $: activeTemplate =
    // `myRole` (RG-272): a pinned plan cue may not redesign a STAGE screen.
    resolveOutputTemplate(t, override, !!content?.template_pinned, defaultTpl, kindLook, content?.kind, myRole) ||
    DEFAULT_TEMPLATE;
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
  let kioskHost = 'localhost';
  // ── A SOCKET IS NOT A SCREEN (RG-193, 2026-09-21) ──────────────────────────
  // `output.html` reconnected only on `onclose`/`onerror`, and a half-open socket
  // — wifi roaming, a sleeping kiosk, a NAT timeout — fires neither. The last
  // frame stood for the rest of a service while the desk read "Not responding".
  // The stage page has kept this rule since DECISIONS §100: three unanswered
  // beats and the socket is replaced. Same constant, same reason, on the door
  // that faces the room.
  const HOST_SAMPLES = 5;
  const STALE_AFTER_MS = BEAT_INTERVAL_MS * 3;
  let hostSamples = [];
  /** Relay's clock minus this screen's, a median of the last five acks (RG-194). */
  let hostOffsetMs = 0;
  let lastAckAt = null;
  let beatingSince = null;
  let staleTimer = null;
  $: expectsAck = channelId > 0;
  function noteHostClock(at) {
    if (typeof at !== 'number' || !Number.isFinite(at)) return;
    const seen = Date.now();
    lastAckAt = seen;
    hostSamples = [...hostSamples, at - seen].slice(-HOST_SAMPLES);
    const sorted = [...hostSamples].sort((a, b) => a - b);
    hostOffsetMs = sorted[Math.floor(sorted.length / 2)];
  }
  /** Drop whatever socket there is and open a fresh one, now. */
  function reconnectNow() {
    if (kioskClosed) return;
    if (ws) {
      const old = ws;
      ws = null;
      old.onclose = null; // the retry path must not race this one
      try { old.close(); } catch { /* already gone */ }
    }
    lastAckAt = null;
    beatingSince = Date.now();
    connectKiosk(kioskHost);
  }
  function staleTick() {
    if (!expectsAck || !ws || ws.readyState !== 1) return;
    const since = lastAckAt ?? beatingSince;
    if (since === null) return;
    if (Date.now() - since > STALE_AFTER_MS) reconnectNow();
  }
  const onWake = () => {
    if (typeof document !== 'undefined' && document.hidden) return;
    const since = lastAckAt ?? beatingSince;
    if (expectsAck && ws && ws.readyState === 1 && since !== null && Date.now() - since > STALE_AFTER_MS) {
      reconnectNow();
    } else if (!ws || ws.readyState === 3) {
      reconnectNow();
    }
  };
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
  // Desktop only — THE CONTENT LOOKS, read when this window opens, on exactly the
  // argument `loadDefaultTemplate` above makes and for exactly the same defect.
  //
  // A content look reaches this page as an id alone (the 13 MB reason is at
  // `lookCache`), so the id is only useful to a screen that already holds the
  // bytes. The kiosk hub sends them in its hello reply; a native output window has
  // no socket and was sent NOTHING, so a projector opened through
  // `open_channel_output` on a channel that follows the content look would resolve
  // every fire against an empty cache and paint the configured default — the exact
  // failure on the exact screen the whole feature exists for, and invisible in the
  // same way, because the OBS source in the same room would be correct.
  //
  // Five ids at most, read once, at mount — never on a fire. `get_content_templates`
  // is the same command the console reads the matrix from, so there is no second
  // notion of what the looks are.
  //
  // Guarded the same way as every read above it: a missing command or a backend
  // that says nothing leaves this screen resolving against an empty cache, which
  // is the behaviour before this existed and the safe one, and never throws on a
  // live output page.
  async function loadContentLooks() {
    try {
      const call = await invoke();
      const map = await call('get_content_templates');
      const ids = [...new Set(Object.values(map ?? {}).filter((v) => v != null))];
      for (const id of ids) {
        cacheTemplate(id, (await call('get_template', { id })) ?? null);
      }
    } catch {
      /* no looks in hand; the resolver falls through exactly as it did before */
    }
  }
  // Desktop only — WHAT THIS SCREEN WEARS FOR EACH KIND, read when this window
  // opens, on exactly the argument `loadContentLooks` above makes and for exactly
  // the same defect one rung up. The map reaches a browser source in the hello
  // reply; a native output window has no socket and would be sent nothing, so a
  // projector opened through `open_channel_output` would resolve every fire
  // against an empty map and paint its blanket template — the exact failure on the
  // exact screen the feature exists for, and invisible in the same way, because
  // the OBS source in the same room would be correct.
  //
  // The BYTES are read here too and not only the ids. `loadContentLooks` warms the
  // cache from the five global looks; a per-kind look can name a template no
  // global look does, and an id whose bytes this page does not hold resolves to
  // nothing and falls through to the blanket template — silently, which is the one
  // way this feature can look like it was never configured.
  //
  // Guarded the same way as every read above it: a missing command or a backend
  // that says nothing leaves this screen with no per-kind looks, which is the
  // behaviour before this existed and the safe one, and never throws on a live
  // output page.
  async function loadChannelLooks() {
    try {
      const call = await invoke();
      const map = await call('list_channel_looks');
      applyChannelLooks(map);
      const ids = [
        ...new Set(
          Object.values(map ?? {})
            .flatMap((byKind) => Object.values(byKind ?? {}))
            .filter((v) => v != null),
        ),
      ].filter((id) => !lookCache[id]);
      // FETCHED TOGETHER, not one after another, and that is a real property of
      // this page rather than a micro-optimisation. Every read on this mount path
      // runs in series and the `listen` registrations come after the last of
      // them, so each sequential await widens the window in which a window that
      // has opened is not yet hearing events. A screen can carry a look per kind
      // per screen, so a serial loop here is the one read on the path whose
      // length is not a constant.
      const got = await Promise.all(
        ids.map(async (id) => [id, (await call('get_template', { id })) ?? null]),
      );
      for (const [id, tpl] of got) cacheTemplate(id, tpl);
    } catch {
      applyChannelLooks({});
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
  // Desktop only — WHAT EVERY SCREEN IS FOR AND WHAT EACH ONE SHOWS, read when
  // this window opens. ONE read, two facts, and they are one function because they
  // come off ONE ROW: `list_output_channels` carries `role` and `shows_json`
  // together, and asking for the same rows twice is one more sequential IPC
  // round-trip on the path a projector opened mid-service is already waiting on.
  //
  // That is not a micro-optimisation here. Every opening read on this page runs in
  // SERIES and the `listen` registrations come after the last of them, so each
  // extra read widens the window in which a window that has visibly opened is not
  // yet hearing events. Two reads of one row were two turns of that window for
  // nothing.
  //
  // The kiosk hub sends both of these in the hello reply; a native output window
  // has no socket and would be sent neither, so a projector opened through
  // `open_channel_output` would know nothing about itself until the operator
  // happened to change something — the "guarantee kept on one door" mistake, on
  // the two screens most often in the same room.
  //
  // Guarded the same way as every read above it, and the two failures differ:
  // no role is the REFUSING answer (a screen that is not the stage may not be
  // shown a word meant for the preacher), while no `shows` opinion is the
  // PERMISSIVE one (follow the template, which is where the decision lived before
  // the column existed). An empty `shows` list arrived at by accident is a
  // congregation screen painting nothing, so the absence must stay an absence.
  async function loadChannelConfig() {
    try {
      const call = await invoke();
      const list = await call('list_output_channels');
      const roles = {};
      const shows = {};
      for (const c of Array.isArray(list) ? list : []) {
        if (c?.role) roles[String(c.id)] = c.role;
        if (!c?.shows_json) continue;
        try {
          const kinds = JSON.parse(c.shows_json);
          if (Array.isArray(kinds)) shows[String(c.id)] = kinds;
        } catch {
          /* an unreadable value is NO OPINION, never an empty set */
        }
      }
      applyRoles(roles);
      applyChannelShows(shows);
    } catch {
      applyRoles({});
      applyChannelShows({});
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
    // THE THIRD PLACE THIS TEMPLATE CAN BE IN USE, and the one that is not a
    // snapshot: a content look is held by ID, so an edit to it has to land in the
    // cache or this screen goes on painting the version it was handed on connect.
    // Unconditional, because the hub sends every template edit to every client and
    // the cache is the only thing that knows whether this screen cares.
    cacheTemplate(id, fresh);
    if (id === templateId) t = fresh;
    if (content) {
      const ov = parseTemplateOverride(content.template_json);
      if (ov && ov.id === id) content = { ...content, template_json: JSON.stringify(fresh) };
    }
  }

  /**
   * THE CONFIGURED WARNING WINDOW, DELIVERED RATHER THAN READ — RG-149(c).
   *
   * `Settings → Getting started → Countdown warning` is applied through
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
      // and a Stage Timer is never published as content so it cannot reach this
      // page by any route. The TWIN of this line is the `output://content` listener
      // in `onMount` — the native window has the Tauri bridge and no socket, so a
      // filter written here and not there is the "guarantee kept on one door"
      // mistake, on the two screens most often in the same room.
      // WHICH SCREENS, then which kinds. Both doors ask both questions — a
      // guarantee kept on one of two doors is the mistake this file counts
      // seven times, and the rehearsal guarantee was green and false for the
      // stage tablet for exactly that reason.
      if (!paintsHere(m.channels)) return;
      if (m.content_kind && !paintsKind(m.content_kind)) return;
      // The override takes effect WITH the content, never before it — see the
      // snapshot comment at the top of this file.
      appliedTransition = pendingTransition;
      // `countdown_warn_ms` is copied across like every other field this door
      // rebuilds by hand: it is the threshold chosen for THIS countdown, and
      // `TemplateRender` reads it off the content. The list is the reason this door
      // has dropped fields before (`next_reference`), so a field added to the wire
      // and not added here is a kiosk screen disagreeing with the wall beside it.
      content = { kind: m.content_kind, reference: m.reference, text: m.text, translation: m.translation, media_url: sameHostMediaUrl(m.media_url, location.hostname), media_kind: m.media_kind, template_id: m.template_id, template_json: m.template_json, template_pinned: m.template_pinned, countdown_to: m.countdown_to, countdown_from: m.countdown_from, countdown_paused_ms: m.countdown_paused_ms, countdown_done: m.countdown_done, countdown_warn_ms: m.countdown_warn_ms, stage_note: m.stage_note, next_reference: m.next_reference, next_text: m.next_text, service_started_at: m.service_started_at, service_target_ms: m.service_target_ms, media_started_at: m.media_started_at };
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
    } else if (m.kind === 'timer') {
      // THE WHOLE SET, OR NOTHING. A frame whose `timers` is missing or is not a
      // list is read as an empty programme rather than thrown on: this page has
      // no backend and cannot verify who is on the other end of its socket
      // (docs/SECURITY.md T4), and one throw inside `applyMessage` would kill
      // every frame after it - the reading included - for the rest of the
      // service. The same words, and the same reason, as `Stage.svelte`'s branch.
      //
      // Deliberately NOT cleared by `clear` or `black` below: the congregation's
      // timers go with the congregation's screens and the preacher's programme
      // stays, because the programme is not something a congregation was ever
      // looking at (DECISIONS §91).
      stageTimerSet = Array.isArray(m.timers) ? m.timers : [];
      // AND THE CONFIGURED WARNING WINDOW RIDES WITH THE SET. This is the frame
      // that can reach a stage screen FIRST - a Stage Timer runs during the
      // notices, before anything has been fired - so reading it here is what
      // stops the rail warning at a figure nothing on the machine holds
      // (RG-149(c)). The twin of the `countdown_warn_default_ms` line in the
      // content branch above; a guarantee kept on one of two doors is the
      // mistake this file counts seven times.
      applyWarnDefault(m.warn_default_ms);
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
      takeDownStageMessage();
    } else if (m.kind === 'black') {
      black = true;
      takeDownStageMessage();
      // On a band channel, blacking out means the band goes away — the camera
      // must not be covered.
      if (isBand) visible = false;
      // On EVERY channel the backdrop goes, band or not. The opaque overlay hides
      // it on an ordinary screen, but a keyed channel paints no overlay at all —
      // so leaving it here would put the church's picture over the live camera at
      // the one moment the operator asked for the camera alone.
      backdrop = null;
    } else if (m.kind === 'screen_state') {
      // WHICH SCREENS THE OPERATOR HAS TAKEN OUT OF THE WALL. Retained by the hub
      // in its own slot and replayed on hello AFTER the screen frame, so a browser
      // source that restarted mid-sermon comes back down rather than bringing
      // itself back up (rule 43, `channels::tests`).
      applyScreenState(m.screens);
    } else if (m.kind === 'channel_looks') {
      // WHAT THIS SCREEN WEARS FOR EACH KIND. Sent on every hello and whenever it
      // changes, with the configuration and BEFORE the retained screen frame, so
      // the look is in hand before the frame it dresses (rule 43). `{}` is an
      // answer and is applied as one — see `applyChannelLooks`.
      applyChannelLooks(m.looks);
    } else if (m.kind === 'channel_shows') {
      // WHICH KINDS THIS SCREEN SHOWS AT ALL. Sent on every hello with the
      // configuration and BEFORE the retained screen frame, because it decides
      // whether that frame paints here — sent after it, a screen the operator has
      // taken the countdown off would paint one and then drop it.
      applyChannelShows(m.shows);
    } else if (m.kind === 'channel_roles') {
      // WHAT EVERY SCREEN IS FOR. Sent on every hello and whenever it changes, so
      // this page can answer the only question it asks of it: am I the stage?
      applyRoles(m.roles);
    } else if (m.kind === 'media_transport') {
      // WHAT THE CLIP IS DOING. Applied to the element by `TemplateRender`, never
      // by re-mounting it — re-mounting to pause would restart the clip from zero,
      // which is the opposite of what Pause means.
      //
      // Not role-gated, unlike the stage frames: this changes what a screen is
      // ALREADY showing rather than putting something new in front of somebody, so
      // there is nothing here a congregation screen should be spared. A screen with
      // no clip up has no video to apply it to and ignores it by construction.
      mediaTransport = {
        paused: !!m.paused,
        loop: !!m.loop,
        replayEpoch: Number.isFinite(m.replay_epoch) ? m.replay_epoch : null,
        // The scrub and the room's level (RG-221). Counted apart from the replay,
        // for the reason `mediatransport.js` records: one epoch for both would
        // swallow a scrub made immediately after a replay.
        seekEpoch: Number.isFinite(m.seek_epoch) ? m.seek_epoch : null,
        seekMs: Number.isFinite(m.seek_ms) ? m.seek_ms : null,
        volume: Number.isFinite(m.volume) ? m.volume : null,
      };
      // AND A SCRUB MOVES THE SYNC BASELINE WITH IT (RG-220). Without this the
      // corrector pulls the clip back to where the clock says it should be
      // within two seconds, so the operator's own drag visibly undoes itself.
      if (Number.isFinite(m.started_at) && content?.media_url) {
        content = { ...content, media_started_at: m.started_at };
      }
    } else if (m.kind === 'stage_zones') {
      // THE OPERATOR'S STAGE LAYOUT REACHES THE BIG SCREEN TOO (RG-265).
      // `stage.html` has honoured Timer size since RG-240 and this page never
      // asked, so an operator who set Huge for the platform monitor moved the
      // phone and nothing else — a control reporting success over a screen it
      // did not touch, which is rule 35 in a different coat.
      //
      // The hub addresses no client (DECISIONS §35), so every page is sent
      // every screen's layout and picks its own out by id.
      const mine = m.zones?.[String(channelId)] ?? m.zones?.[channelId];
      timerSize = readTimerSize(mine);
    } else if (m.kind === 'stage_alert') {
      // ONLY A STAGE. No role is not a stage — a lobby TV and a streaming feed
      // both arrive here with no role at all, and a filter whose default is yes
      // is not a filter. `text: null` (or blank) clears it; an alert is an
      // instruction about a moment, never a state of the wall, which is why the
      // hub does not retain it (rule 43, FRAME_VERDICTS).
      if (!acceptsStageMessage(myRole)) return;
      stageMessage = (m.text || '').trim();
      stageUrgent = !!m.urgent;
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
    } else if (m.kind === 'beat_ack') {
      // The hub answers every beat with its clock (DECISIONS §100). This is how
      // the page knows the socket is alive AND what o'clock Relay thinks it is.
      noteHostClock(m.at);
    }
  }

  function connectKiosk(host) {
    if (kioskClosed) return;
    kioskHost = host;
    if (ws && ws.readyState <= 1) return; // one socket at a time
    try {
      ws = new WebSocket(`ws://${host}:8031`);
      if (beatingSince === null) beatingSince = Date.now();
      ws.onopen = () => {
        // Ask the hub for this channel's real template.
        try {
          // THE CHANNEL RIDES WITH IT NOW. The template id is what the hub
          // counts clients against; the channel is what it needs to replay a
          // state retained for ONE screen (`screen_state`). A client that never
          // says which screen it is cannot be told it is one the operator took
          // down. `channel: 0` is an honest answer for a raw preview and the hub
          // treats it as no channel, exactly as this page does.
          ws.send(JSON.stringify({ kind: 'hello', channel: channelId, template_id: templateId }));
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
        ws = null;
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
      await loadContentLooks();
      await loadChannelLooks();
      await loadChannelConfig();
      const { listen } = await import('@tauri-apps/api/event');
      unlisten.push(await listen('output://content', (e) => {
        // Per-screen visibility (see applyMessage) — hold what's up if this screen
        // doesn't show the fired kind. SITE 8's other half: the kiosk protocol
        // renames `kind` to `content_kind`, so the two doors read a differently
        // named field off differently shaped messages and only the rule is shared.
        // Swept with the kiosk door and needed nothing for the same reason.
        if (!paintsHere(e.payload?.channels)) return;
        if (e.payload?.kind && !paintsKind(e.payload.kind)) return;
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
      // BOTH DOORS, for the fifth time in this file. A native output window on
      // HDMI has the bridge and no socket; the browser source in the same room has
      // the socket and no bridge. A per-screen control wired to one of them would
      // take the lobby TV down over the network and leave the projector beside it
      // untouched — the "guarantee kept on one door" mistake, on the control whose
      // entire purpose is that exactly one screen changes.
      unlisten.push(
        await listen('output://screen_state', (e) => {
          // The Tauri door carries ONE screen and its state, because the emit is
          // per call; the kiosk door carries the whole set, because it is a
          // retained frame and a set is the only shape that survives a missed
          // frame. Both end in the same place.
          if (!channelId || e.payload?.channel !== channelId) return;
          const st = e.payload?.state;
          downMode = st === 'clear' || st === 'black' ? st : null;
        }),
      );
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
      // BOTH DOORS, and this one was MISSING (RG-156). `channels::stage_alert`
      // published to the kiosk hub and emitted nothing, so a screen wired as a
      // native window and given the `stage` role heard no Stage Message at all —
      // while the console reported one sent. The failure direction is silence,
      // which is why nobody had met it: it takes a church to put a confidence
      // monitor on HDMI rather than on the network, and then nothing says a word.
      //
      // THE SAME PREDICATE, not a second one. `acceptsStageMessage` is asked here
      // exactly as it is asked in the socket branch above, because the two doors
      // are answering one question and a second expression of it is the thing that
      // drifts. No role is not a stage: a lobby TV and a streaming feed arrive
      // with no role at all, and a filter whose default is yes is not a filter.
      unlisten.push(
        await listen('output://stage_alert', (e) => {
          if (!acceptsStageMessage(myRole)) return;
          stageMessage = (e.payload?.text || '').trim();
          stageUrgent = !!e.payload?.urgent;
        }),
      );
      // BOTH DOORS, for the seventh time in this file, and here the cost of one
      // door is two screens in one room wearing different templates for the same
      // verse. An operator who changes a look mid-service must see it move on the
      // projector and in OBS at the same instant, or the one they are not looking
      // at is the one that is wrong.
      //
      // The bytes may be new, so they are fetched for any id this page does not
      // already hold — `template://updated` only fires for a template that was
      // EDITED, and choosing an existing template for a kind edits nothing.
      // BOTH DOORS, for the eighth time in this file. A screen the operator takes
      // the countdown off must lose it on the projector and in OBS at the same
      // instant, or the one they are not looking at is the one still showing it.
      unlisten.push(
        await listen('output://channel_shows', (e) => {
          applyChannelShows(e.payload?.shows);
        }),
      );
      unlisten.push(
        await listen('output://channel_looks', async (e) => {
          applyChannelLooks(e.payload?.looks);
          for (const byKind of Object.values(e.payload?.looks ?? {})) {
            for (const id of Object.values(byKind ?? {})) {
              if (id == null || lookCache[id]) continue;
              cacheTemplate(id, await fetchTemplate(id));
            }
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
    if (typeof document !== 'undefined') document.addEventListener('visibilitychange', onWake);
    if (typeof window !== 'undefined') window.addEventListener('online', onWake);
    staleTimer = setInterval(staleTick, 1000);
    stopBeat = startBeat({
      channelId,
      // WHAT THIS SCREEN IS ACTUALLY SHOWING, not what it was last told. A screen
      // the operator took down is painting `clear`, and a beat that still claimed
      // `content` would put `describeScreen` into a standing `Not confirmed` —
      // an alarm about a screen doing exactly what it was told (rule 35).
      getState: () => paintState({ black: shownBlack, visible: !!shownContent, content }),
      getWs: () => ws,
      // WHERE THIS SCREEN'S CLIP IS. Read once per beat rather than per frame, and
      // `null` whenever there is no clip — see `noteMedia`.
      getMedia: () => mediaReport,
      // WHAT THIS SCREEN COULD NOT LOAD, or nothing. See `noteMediaError`.
      getMediaError: () => mediaError,
    });
  });
  onDestroy(() => {
    // Stop reporting FIRST. Anything after this is a page on its way out, and a
    // beat from it would say "still painting" about a screen that is closing.
    stopBeat();
    unlisten.forEach((u) => u());
    if (staleTimer) clearInterval(staleTimer);
    if (typeof document !== 'undefined') document.removeEventListener('visibilitychange', onWake);
    if (typeof window !== 'undefined') window.removeEventListener('online', onWake);
    kioskClosed = true;
    if (ws) ws.close();
  });
</script>

<TemplateRender
  template={renderedTemplate}
  content={shownContent}
  backdrop={shownBackdrop}
  audio={isDesktop}
  stageMessage={shownStageMessage}
  timerScale={railScale(timerSize)}
  stageClip={myRole === 'stage'}
  {stageUrgent}
  programme={shownProgramme}
  onMedia={noteMedia}
  onMediaError={noteMediaError}
  {hostOffsetMs}
  {mediaTransport}
  transitionOverride={appliedTransition} />
<!-- BLACKOUT NEVER BLACKS OUT A LOWER THIRD. On a keyed channel "black" would
     paint an opaque rectangle over the live camera — the opposite of what the
     operator pressed it for. On that channel the panic control removes the
     BAND, which is all this channel was ever contributing, and the camera keeps
     going out. Every other channel goes properly black. -->
{#if shownBlack && !isBand}<div class="blackout"></div>{/if}

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
