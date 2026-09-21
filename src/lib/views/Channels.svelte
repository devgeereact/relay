<script>
  // The Outputs hub — one place to wire everything that puts pixels somewhere.
  // Three panes, one vocabulary (Decision §25):
  //
  //   Screens        every output target — a render target of the SAME template
  //                  engine. Nothing here branches on screen type: native_window
  //                  vs network_client changes where pixels land, never how
  //                  content is formatted. That is what templates are for.
  //   Content looks  the type → template default map ("when scripture fires,
  //                  which look does it wear on any screen that hasn't overridden
  //                  it"). THE one writer of that map, backed by the shared
  //                  `contentTemplates` store — three surfaces used to write it
  //                  with no shared state and silently overwrote each other.
  //   Sharing        the LAN address + the preacher's stage remote — the outputs
  //                  a church sets up by hand on other devices every week.
  //
  // ── The online light is computed, never stored ────────────────────────────
  //
  // `output_channels.status` exists in the schema and is a trap: both INSERTs
  // hardcode 'offline' and nothing in the codebase ever updates it, so it read
  // `offline` for every screen forever — including one filling a projector.
  // This screen asks `channel_status` instead, which derives liveness from facts
  // the running app actually has: which output windows are open, how many kiosk
  // clients are subscribed to each template — and, since the screens began
  // answering for themselves, whether each one has reported that it is still
  // PAINTING within the last few seconds. The first two are true of a frozen
  // projector; only the third can go false on its own.
  import { onMount } from 'svelte';
  import Field from '../ui/Field.svelte';
  import QRCode from 'qrcode';
  // The workspace grammar (docs/REBRAND.md §2 · §11): rail · main · inspector,
  // one type scale, a row that is a name and a value. Shared with the Planner and
  // Settings so the three read as one desk rather than three designs.
  import WorkspaceFrame from './WorkspaceFrame.svelte';
  // The SAME rule Live uses. Two surfaces describing one screen must not be able
  // to reach different conclusions about it — that asymmetry is how this
  // repository has produced four separate bugs with one root cause.
  import {
    describeScreen,
    screenFault,
    screenReporting,
    screenSwitch,
    SCREEN_BADGE,
    screenKind,
    screenTransport,
  } from '../outputHealth.js';
  // Was: `error = String(err)`, rendered in a MONOSPACE font, five times over — a raw
  // Rust Err string shown to a church volunteer who has never seen one.
  import ErrorState from '../ui/ErrorState.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Loading from '../ui/Loading.svelte';
  import TemplateRender from '../TemplateRender.svelte';
  // THE ONE CAMERA PLATE. This markup and its CSS lived here twice, four hundred
  // lines apart, and the Templates gallery needed a third copy for exactly the
  // same reason (a keyed template previewed against nothing is an empty dark
  // rectangle). One component now; the DECISION stays with each caller, because
  // `isKeyedTemplate` is about the template the caller resolved.
  import CameraPlate from '../ui/CameraPlate.svelte';
  // DEFAULT_TEMPLATE is the FLOOR, and it is the output page's floor too — a
  // screen that follows the content look when no look is set still has to paint
  // something legible. Imported here so the preview and the wall reach the same
  // answer rather than two different kinds of nothing.
  import { DEFAULT_TEMPLATE } from '../templates.js';
  import {
    CONTENT_KINDS,
    resolveOutputTemplate,
    isKeyedTemplate,
    templateById,
    lookIdFor,
  } from '../layers.js';
  import { outputUrl } from '../outputurl.js';
  import {
    CHANNEL_ROLES,
    NO_ROLE_LABEL,
    stageRemoteUrl,
    isSharableHost,
    // READ HERE TOO, not only by Live. Live is where a service is run; this is
    // where a screen is WIRED, and the question "will a Stage Timer reach
    // anybody" is asked with a tablet in the operator's hand. One helper, so the
    // two surfaces cannot come to different conclusions about one screen.
    describeStageReach,
  } from '../channelroles.js';
  // WHAT THE CONSOLE MAY SAY ABOUT A SCREEN IT CANNOT SEE — pure, and every
  // answer it cannot give comes back as `unknown` rather than as a guess.
  import { stageMirrorZones, stageMirrorReading } from '../stagemirror.js';
  import { stageTimers, timerRemainingMs, timerIsHeld } from '../timers.js';
  import { formatCountdown } from '../layers.js';
  import { STAGE_ZONES, DEFAULT_STAGE_ZONES, readStageZones } from '../stagelayout.js';
  import {
    capture,
    templates,
    live,
    liveContent,
    liveTemplateOverride,
    liveTemplatePinned,
    contentTemplates,
    setContentTemplate,
    channelLooks,
    loadChannelLooks,
    setChannelLook,
    loadTemplates,
    listOutputChannels,
    setChannelTemplate,
    setChannelRole,
    listStageLayouts,
    listTimers,
    setChannelStageLayout,
    upsertStageLayout,
    deleteStageLayout,
    setChannelShows,
    listMonitors,
    openChannelOutput,
    closeChannelOutput,
    channelHealth,
    channelWaiting,
    rehearsing,
    screenBlack,
    startChannelHealth,
    setChannelDisplay,
    renameChannel,
    clearScreen,
    blackoutScreen,
    restoreScreen,
    addChannel,
    deleteChannel,
    localIp,
    networkAddresses,
    defaultTemplateId,
    loadDefaultTemplate,
    readErrors,
  } from '../stores/capture.js';

  // Which pane. 'screens' is where an operator lives. All three now keep the
  // rail and the inspector: a section that drops two of the three columns is a
  // different workspace wearing the same tab, and that is what made Content
  // looks and Sharing read as a separate product.
  let view = 'screens'; // screens | stage | looks | layouts | sharing
  const VIEWS = [
    { key: 'screens', label: 'Screens',
      lead: 'Every target Relay can paint: a projector on HDMI, an OBS or kiosk browser source over the network.' },
    // ── THE PREACHER'S SCREEN, IN ONE PLACE (operator instruction, 2026-09-20)
    //
    // *"Build a proper Screen section on the Output page that shows what is
    // currently on the preacher's device, with the QR code available to scan
    // from that same section. This replaces the current journey of going to
    // Sharing and then Stage Layout. Keep the old path working until the new one
    // is verified."*
    //
    // ADDITIVE, on the operator's own instruction: Sharing and Stage layouts are
    // both still here and both still work. Nothing was moved out of them — this
    // section reads the same facts through the same helpers, which is what makes
    // two doors safe rather than two answers.
    { key: 'stage', label: "Preacher's screen",
      lead: 'What Relay has told the preacher\u2019s device to show, the link that opens it, and the layout behind it.' },
    { key: 'looks', label: 'Content looks',
      lead: 'Which template each kind of content wears on any screen that has no look of its own.' },
    { key: 'layouts', label: 'Stage layouts',
      lead: 'What a preacher\'s screen shows. Assign one to a stage screen in Screens; a screen with none is set from the device itself.' },
    { key: 'sharing', label: 'Sharing',
      lead: 'The addresses other devices in the building use to reach this machine.' },
  ];
  $: activeView = VIEWS.find((v) => v.key === view) ?? VIEWS[0];

  let channels = [];
  // "Loading" vs "empty" — listOutputChannels swallows to [], so without this the
  // empty-state ("No screens yet") flashes during a normal cold open.
  let loading = true;
  let monitors = [];
  let error = null; // the TYPED error from Rust — ErrorState decides what to show
  let copiedId = null;
  let lanIp = 'localhost';
  let qrOpen = null;
  let qrData = '';
  let qrUrl = '';
  let qrError = '';
  let addresses = [];
  let networkBusy = false;
  let networkError = '';
  let chosenAddress = null;
  let selectedStageId = null;

  async function refreshNetwork() {
    if (networkBusy) return;
    networkBusy = true;
    networkError = '';
    try {
      const [detected, preferred] = await Promise.all([networkAddresses(), localIp()]);
      addresses = Array.isArray(detected) ? detected : [];
      // Keep a deliberate choice through refresh. If it disappears, require a
      // new choice rather than handing out a different network's QR silently.
      if (chosenAddress != null) {
        lanIp = addresses.some((a) => a.address === chosenAddress) ? chosenAddress : 'localhost';
      } else {
        lanIp = addresses.find((a) => a.address === preferred)?.address || addresses[0]?.address || preferred || 'localhost';
      }
    } catch (e) {
      lanIp = 'localhost';
      networkError = 'Could not refresh local network addresses. Try Refresh addresses again.';
    } finally { networkBusy = false; }
  }

  function chooseAddress(e) {
    chosenAddress = e.target.value || null;
    lanIp = chosenAddress || 'localhost';
  }

  let filter = 'all'; // all | native_window | network_client
  let q = '';
  let selId = null;
  let showAdd = false;
  let newName = '';
  let newTarget = 'native_window';

  async function refresh() {
    channels = await listOutputChannels();
    if (selId && !channels.some((c) => c.id === selId)) selId = null;
  }
  // Liveness is polled, not pushed: a kiosk connecting or a window closing raises
  // no event Relay listens for, so the honest options are polling or a status that
  // goes stale. The poll itself lives in the store and is started by the shell —
  // the Live pane and the degraded banner want the same answer, and three timers
  // asking one question would let three surfaces disagree about one screen.
  $: status = $channelHealth;
  onMount(async () => {
    // Guarded: an unguarded reject here aborted mount before the poll was ever
    // scheduled, leaving status blank with no reason shown.
    try {
      await loadTemplates();
      await loadDefaultTemplate();
      // WHAT EACH SCREEN WEARS FOR EACH KIND (DECISIONS §97). Read once, here,
      // into the shared store — the disclosure and the card meta line both read
      // the store, so the desk cannot describe one screen two ways.
      await loadChannelLooks();
      monitors = await listMonitors();
      stageLayouts = (await listStageLayouts()) ?? [];
      await refreshNetwork();
      await refresh();
      // Make sure the poller is running even if this tab was opened before the
      // shell got there — idempotent, so this cannot create a second timer.
      startChannelHealth();
    } catch (e) {
      error = e; // the TYPED error; ErrorState humanises it (matches act())
    } finally {
      loading = false;
    }
  });

  const isNative = (c) => c.render_target === 'native_window';
  // NDI is parked (no proprietary SDK ships) and has no UI affordance, but an
  // older DB row could still carry `ndi_encode`, so rendering stays defensive.
  const isNdi = (c) => c.render_target === 'ndi_encode';
  // The URL carries the CHANNEL id (not just the template). That is what lets a
  // template change reach this output live — the output filters a channel-retemplate
  // broadcast by its own `channel`, so switching a screen's template needs no
  // re-copying of the URL. `template_id` stays for the first render before any push.
  // ONE BUILDER (`lib/outputurl.js`). This was written out twice, four lines
  // apart, and only one copy was corrected when a screen gained the ability to
  // have no look of its own — so Copy URL and the inspector's readout said
  // different things about the same screen.
  const obsUrl = (c) => outputUrl(lanIp, c.id, c.template_id, c.name);
  const templateOf = (c) => $templates.find((t) => t.id === c.template_id) || null;
  /** What a content look currently resolves to, by name — for a following screen. */
  const lookName = (kind) =>
    $templates.find((t) => t.id === $contentTemplates[kind])?.name ?? 'the default look';
  // ── PER-KIND LOOKS: WHAT A SCREEN WEARS FOR ONE KIND (DECISIONS §97) ────────
  //
  // `$channelLooks` is NAMED in every expression below rather than reached
  // through a helper that reads it. Svelte tracks the identifiers it can SEE in a
  // reactive expression, not the ones a called function happens to read, and this
  // file has been caught by exactly that twice (`stageUrl()`,
  // `lookName('scripture')`).
  //
  // The inherit option NAMES ITS DESTINATION, RESOLVED LIVE. "Same as this
  // screen · Classic · Scripture" and "Follow the content look · Aurora" and
  // "Follow the configured default · Classic Serif" are three different answers,
  // and an inherit option that reads identically whether a content look is set or
  // not is not a line — it is the shape rule 35 is about, on the control where
  // the operator is deciding what a congregation sees.
  $: inheritLabel = (kind) => {
    if (!sel) return 'Inherit';
    if (sel.template_id != null) {
      const own = $templates.find((t) => t.id === sel.template_id);
      return `Same as this screen · ${own?.name ?? 'its own look'}`;
    }
    const look = $templates.find((t) => t.id === $contentTemplates[kind]);
    if (look) return `Follow the content look · ${look.name}`;
    const dflt = $templates.find((t) => t.id === $defaultTemplateId);
    return dflt ? `Follow the configured default · ${dflt.name}` : 'Follow the bundled look';
  };
  // WHICH KINDS THIS SCREEN HAS A LOOK OF ITS OWN FOR. The summary NAMES them and
  // never counts them: "2 kinds" is a number an operator has to open the
  // disclosure to act on, and the whole reason the disclosure is closed by
  // default is that most screens have nothing in it.
  $: selLookKinds = sel
    ? CONTENT_KINDS.filter((k) => lookIdFor($channelLooks, sel.id, k.key) != null)
    : [];
  $: perKindSummary = selLookKinds.length
    ? selLookKinds.map((k) => k.label).join(' · ')
    : 'Same look for every kind';
  let perKindOpen = false;
  let perKindFor = null;
  // CLOSED AGAIN WHEN THE OPERATOR MOVES TO ANOTHER SCREEN. A disclosure that
  // stayed open would carry one screen's five choices onto the next screen's
  // panel, which reads as a setting that has followed them.
  $: if ((sel?.id ?? null) !== perKindFor) {
    perKindFor = sel?.id ?? null;
    perKindOpen = false;
  }
  async function pickKindLook(kind, e) {
    const v = e.target.value;
    try {
      await setChannelLook(sel.id, kind, v === '' ? null : parseInt(v, 10));
      error = null;
    } catch (err) {
      error = err;
    }
  }
  /** One term for the card's meta line — never a lamp and never a new colour. */
  const perKindTerm = (c, looks) =>
    CONTENT_KINDS.some((k) => lookIdFor(looks, c.id, k.key) != null) ? 'per-kind looks' : null;

  const monitorOf = (c) => {
    const i = parseInt(c.display_target ?? '', 10);
    return Number.isFinite(i) ? monitors.find((m) => m.index === i) || null : null;
  };
  /**
   * THE DISPLAY THIS SCREEN IS CONFIGURED FOR, WHEN IT IS NOT CONNECTED — as the
   * 1-based number every OS display panel and this picker use, or null.
   *
   * `display_target` is an INDEX into the OS monitor list, so unplugging a dock
   * renumbers it. A `<select>` whose value matches no `<option>` shows its FIRST
   * option, which here reads **Primary display** — so a screen configured for the
   * projector rendered identically to a screen configured for nothing, on the one
   * control that decides which physical screen a congregation sees. That is rule
   * 35: one reassuring sentence over two situations.
   *
   * A STABLE IDENTITY WOULD BE THE REAL FIX AND IS NOT AVAILABLE. Tauri 2.11
   * exposes `Monitor { name, size, position, work_area, scale_factor }` and no
   * native display id; `tao` names a Windows monitor by the `\\.\DISPLAY1`
   * device path the OS renumbers, and a macOS one by its EDID MODEL number, which
   * two identical projectors share. `main.rs::resolve_display` carries the full
   * reasoning, and `open_channel_output` now REFUSES rather than opening a
   * fullscreen output on a guessed monitor. This is the half that says so before
   * the operator presses Open.
   *
   * An empty list is deliberately NOT a missing display: `list_monitors` returns
   * `[]` rather than erroring, so a probe that failed looks exactly like a machine
   * with no screens, and claiming "not connected" from an ambiguity would be the
   * same defect pointing the other way. Same judgement as `resolve_display`.
   */
  /** A native screen with no display chosen while there is more than one to
   *  choose from: a manual Turn on would land on this console (RG-188). */
  const needsDisplay = (c) => {
    if (!c || !isNative(c)) return false;
    const i = parseInt(c.display_target ?? '', 10);
    return !Number.isFinite(i) && monitors.length > 1;
  };
  const missingDisplay = (c) => {
    if (!isNative(c) || !monitors.length) return null;
    const i = parseInt(c.display_target ?? '', 10);
    if (!Number.isFinite(i)) return null;
    return monitors.some((m) => m.index === i) ? null : i + 1;
  };
  /** The kind label shown in the TYPE column. One definition, shared with Live's
      Output Status pane — see `outputHealth.js::screenKind`. */
  const kindOf = (c) => screenKind(c.render_target);
  const transportOf = (c) => screenTransport(c.render_target);
  /**
   * "How do I reach this screen?" — one sentence, decided by the screen's own
   * `render_target` and nothing else. Pure, no command, no state.
   *
   * `Type` and `Transport` above it name the thing ("Native window · HDMI /
   * display"); neither tells a volunteer what to do with a cable. The full map,
   * including which ATEMs are HDMI and which are SDI, is `docs/OUTPUT_ROUTING.md`.
   *
   * WHY PLAIN TEXT rather than a disclosure or a link, both of which were
   * considered:
   *
   * - A `<details>`/`<summary>` would put a Space-activatable control inside the
   *   shell, and `shortcuts.js`'s `isActivatable` covers `<button>`, `role=button`,
   *   `role=switch` and real links — NOT `<summary>`. So an operator who tabbed to
   *   it and pressed Space would advance the programme while the disclosure stayed
   *   shut, and `preventDefault` would eat the toggle. That is rule 11 verbatim, on
   *   a surface an operator opens mid-service. The three `<details>` in this
   *   repository are all in boot gates and the crash panel, which render before
   *   `App.svelte` and so have no transport behind them.
   * - A link would either go nowhere (no docs ship with the binary) or navigate the
   *   operator's console away from a live service inside the webview.
   *
   * So the sentence carries the whole answer itself, and names no document.
   * `docs/OUTPUT_ROUTING.md` is the full map for whoever reads this file, but
   * `tauri.conf.json` has no `resources` key, so `docs/` is not bundled: a
   * shipped copy of Relay does not contain that file, and printing a path to it
   * on an operator's screen would send them looking for something they do not
   * have. Same failure as a dead cross-reference, one layer out.
   */
  const reachOf = (c) => {
    if (c.render_target === 'native_window')
      return 'Plug the display into this computer, choose it under Display above, then press Open. Relay paints a fullscreen picture on it, and restores it by itself at every later launch. An ATEM Mini takes that HDMI straight in; a rack-mount ATEM is SDI only and needs a small HDMI-to-SDI converter first.';
    if (c.render_target === 'ndi_encode')
      return 'NDI is parked, so nothing can reach this screen. Add a Native window for a projector or a switcher, or a Network client for OBS and kiosk screens.';
    return 'Press Copy URL and paste it into an OBS or vMix browser source, or open it in a browser on the other machine. It uses no video port on this computer, which is the answer when the ports run out.';
  };

  $: counts = {
    all: channels.length,
    native_window: channels.filter(isNative).length,
    network_client: channels.filter((c) => c.render_target === 'network_client').length,
  };
  $: shown = channels
    .filter((c) => filter === 'all' || c.render_target === filter)
    .filter((c) => !q.trim() || c.name.toLowerCase().includes(q.trim().toLowerCase()));
  $: sel = channels.find((c) => c.id === selId) || null;
  $: selStatus = sel ? status[sel.id] : null;
  // Names `lanIp` directly so Svelte re-runs it when the address resolves. Via
  // `{obsUrl(sel)}` it was only ever correct by luck of ordering — the same trap
  // the stage-remote URL fell into, one selection away from showing `localhost`
  // to someone about to type it into a phone.
  $: selAddr = sel ? outputUrl(lanIp, sel.id, sel.template_id, sel.name) : '';
  // ── THE RAIL'S TALLY IS THE SCREENS' OWN WORD, NOT RELAY'S ─────────────────
  //
  // This read `status[c.id]?.online`, and `online` is the fact rule 35 exists to
  // keep out of a status line. For a `network_client` `main.rs` sets it to `true`
  // UNCONDITIONALLY — the output is served the whole time the app runs, whether
  // or not any browser is pulling it — so a church whose three OBS sources had
  // all crashed read **3 / 3 in green**, on the rail of the tab they would open
  // to find out. The cards two columns away were correctly painting all three
  // rose at the same moment: one desk, two verdicts about the same screens.
  //
  // `screenFault(...) === 'ok'` is the half of the ONE helper that answers "is it
  // answering" — the screen's own beat, which is the only fact here that can go
  // false by itself. A native window nobody has opened and a browser source
  // nobody has pointed at Relay are both correctly NOT counted.
  $: answering = channels.filter((c) => screenFault(status[c.id] ?? null) === 'ok').length;
  // Rose the moment any screen is one the operator must act on, using the very
  // verdict the card shows. Green over a screen that has stopped answering is the
  // reassuring-sentence-over-a-broken-thing failure in the smallest possible
  // space, and this badge is above the fold on every one of the three sections.
  $: anyDown = channels.some((c) => verdicts[c.id]?.kind === 'down');
  // Which screens a content look actually reaches. A screen's OWN template wins
  // (DECISIONS §29), so a look changes nothing on a screen that has one — and
  // that is precisely the defect phase 4 found: the map could be filled in, saved
  // and change nothing in the building. The inspector answers it with the list.
  $: followers = channels.filter((c) => c.template_id == null);

  async function showQr(c) {
    const address = obsUrl(c);
    if (qrOpen === c.id && qrUrl === address) { qrOpen = null; return; }
    qrError = '';
    // A QR IS A SECOND DEVICE, BY DEFINITION. The URL beside it is not: a
    // loopback output address is exactly right for OBS on this computer, so
    // Copy URL keeps working and only the photograph is refused. `showStageQr`
    // has withheld a loopback link since its own fix; this door had no guard at
    // all and would happily photograph `http://localhost:8032/output.html`,
    // which names the PHONE that scans it.
    if (!isSharableHost(lanIp)) {
      qrOpen = null;
      qrError = 'Relay has no local network address yet, so a QR here could only point the other device at itself. Pick an address in Sharing and try again — or copy the URL, which is still correct for OBS on this computer.';
      return;
    }
    try {
      const data = await QRCode.toDataURL(address, { width: 240, margin: 4, color: { dark: '#0a0a0a', light: '#ffffff' } });
      if (address !== obsUrl(c)) return;
      qrData = data;
      qrUrl = address;
      qrOpen = c.id;
    } catch (e) {
      qrError = 'Could not create the QR code. Copy the output URL and open it on the other device.';
    }
  }

  // REACTIVE, not a function call in the markup.
  //
  // As `{stageUrl()}` this rendered once, before `local_ip` resolved, and then
  // never again — Svelte tracks the identifiers in a template expression, and
  // that one names `stageUrl`, not `lanIp`. So the operator was shown
  // `http://localhost:8032/stage.html` and told to open it on a phone, where
  // localhost is the phone. The QR was built on click and so was correct; only
  // the address anyone would actually type was wrong.
  //
  // AND IT NAMES A SCREEN, not just the page (`stageRemoteUrl`). `stage.html`
  // refuses a Stage Message unless its own channel holds the `stage` role, so a
  // bare address is one that renders the reading perfectly and never receives the
  // message it was set up for. `url` is null when no screen holds the role, and
  // the panel says that rather than printing an address that half works.
  $: stageScreens = channels.filter((c) => c.role === 'stage');
  $: stageRemote = stageRemoteUrl(lanIp, channels, selectedStageId);
  $: stageUrl = stageRemote.url;
  let stageQr = '';
  let stageQrOpen = false;
  let stageQrUrl = '';
  let stageQrError = '';
  let copiedStage = false;
  async function showStageQr() {
    if (stageQrOpen && stageQrUrl === stageUrl) { stageQrOpen = false; return; }
    const address = stageUrl;
    if (!address) return;
    stageQrError = '';
    try {
      const data = await QRCode.toDataURL(address, { width: 240, margin: 4, color: { dark: '#0a0a0a', light: '#ffffff' } });
      if (address !== stageUrl) return;
      stageQr = data;
      stageQrUrl = address;
      stageQrOpen = true;
    } catch (e) {
      if (address === stageUrl) stageQrError = 'Could not create the QR code. Copy the stage link and open it on the phone or tablet.';
    }
  }
  // ── A COPY THAT FAILED MUST NOT LOOK LIKE ONE THAT DID NOTHING ─────────────
  //
  // All three copy buttons swallowed to a `console.warn`: the label stayed
  // "Copy URL", nothing moved, and the operator's reasonable conclusion was that
  // they had missed the button. They then paste the previous thing on their
  // clipboard into OBS. The console is where nobody is looking during a service,
  // and a control that reports NOTHING on failure is the same defect as one that
  // reports success (rule 15's shape, on a non-panic control).
  //
  // Not `ErrorState`: that humanises a TYPED error from Rust and this is a
  // browser refusal, not a backend fault. The button says so itself, in the same
  // place and by the same mechanic as "Copied ✓", so the answer is where the
  // question was asked. The address is rendered as text beside every one of these
  // buttons, so a failed copy is recoverable by typing.
  const COPY_FAILED = 'Copy failed';
  /** Idle · copied · refused, for the buttons whose flag is a plain boolean. */
  const copyLabel = (flag, idle) =>
    flag === COPY_FAILED ? COPY_FAILED : flag ? 'Copied ✓' : idle;
  async function writeClip(text) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch (e) {
      console.warn('Clipboard write blocked', e);
      return false;
    }
  }
  async function copyStage() {
    copiedStage = (await writeClip(stageUrl)) ? true : COPY_FAILED;
    setTimeout(() => (copiedStage = false), 1500);
  }

  /** Run a mutation, refresh, and hand any error to the ONE humaniser. */
  async function act(fn) {
    try {
      await fn();
      await refresh();
      error = null;
    } catch (err) {
      error = err;
    }
  }

  // '' is the operator choosing FOLLOW THE CONTENT LOOK — a screen with no look
  // of its own (DECISIONS §70). It is a value, not an empty field.
  const assignTemplate = (c, e) =>
    act(() => setChannelTemplate(c.id, e.target.value === '' ? null : parseInt(e.target.value, 10)));
  const assignDisplay = (c, e) => act(() => setChannelDisplay(c.id, e.target.value === '' ? null : e.target.value));

  // ── TAKE ONE SCREEN OUT OF THE WALL, OR PUT IT BACK ────────────────────────
  //
  // BESIDE the panic controls, never instead of them. `Clear screens` and
  // `Blackout` live in the dock, are reachable in one action from anywhere,
  // address every screen and ask nothing about which (rule 15, DECISIONS §20).
  // These three live here, on the desk, on the screen they are about — because
  // "take the lobby TV down but leave the wall live" is a decision about one
  // screen and is made while looking at that screen's card.
  //
  // `act` hands a refusal to `src/lib/errors.js` like every other mutation on this
  // desk: the backend refuses during a rehearsal, by name, and an operator must
  // read that sentence rather than a raw Rust string.
  //
  // No two-step arm/confirm, deliberately. This is reversible in one click by the
  // control sitting next to it, and the arming pattern is for things that are not
  // (`TemplateGallery`'s delete). A confirmation step in front of a reversible
  // control is a step an operator learns to click through.
  /**
   * RENAME A SCREEN. Committed on blur and on Enter.
   *
   * An unchanged name is not a write. Without this check, clicking into the field
   * and out of it again would put a row through the backend, the refresh and the
   * error pane for no reason — and on a refused one (a screen deleted on another
   * surface) would show a failure the operator did nothing to cause.
   *
   * On a refusal the field is put back to the name the screen actually has. The
   * sentence is rendered by `act` through `src/lib/errors.js` like every other
   * mutation here, and a field still showing the rejected text under a message
   * saying it was rejected is a surface disagreeing with itself.
   */
  function rename(c, e) {
    const next = e.target.value.trim();
    if (!next || next === c.name) {
      e.target.value = c.name;
      return;
    }
    return act(async () => {
      try {
        await renameChannel(c.id, next);
      } catch (err) {
        e.target.value = c.name;
        throw err;
      }
    });
  }

  const takeDown = (c) => act(() => clearScreen(c.id));
  const blackDown = (c) => act(() => blackoutScreen(c.id));
  const putBack = (c) => act(() => restoreScreen(c.id));
  // WHAT THIS SCREEN HAS BEEN TOLD, off the same liveness row every badge on this
  // desk is derived from — never a second copy of the state kept here. A local
  // copy would disagree with the badge the moment a second console, a reconnect
  // or a refused call moved one of them.
  const downOf = (c) => $channelHealth?.[c.id]?.down ?? null;
  // WHAT THIS SCREEN IS FOR. '' is "no special role", which is the right answer
  // for a streaming feed and a lobby TV and is a value rather than an empty
  // field — the same distinction as Follow the content look above it.
  //
  // The backend refuses a SECOND main screen and names the one that already holds
  // it; `act` hands that refusal to `src/lib/errors.js` like every other mutation
  // on this desk. It is deliberately not pre-empted here by disabling the option:
  // a picker that silently cannot be chosen explains nothing, and the sentence
  // says which screen to clear.
  // ── THE LAYOUT A STAGE SCREEN WEARS ──────────────────────────────────────
  //
  // Global list, per-screen assignment — ProPresenter's own shape, and the
  // reason it is not a template: a stage layout has no regions, no style and no
  // `TemplateRender` output, because `stage.html` draws its own zones.
  //
  // "Whatever the device is set to" is a real choice and the DEFAULT, not a
  // missing value. A church already running a tablet with zones set by hand
  // keeps that arrangement until somebody deliberately picks a layout here.
  let stageLayouts = [];
  const assignStageLayout = (c, e) =>
    act(() => setChannelStageLayout(c.id, e.target.value === '' ? null : Number(e.target.value)));

  // ── THE LAYOUT EDITOR ────────────────────────────────────────────────────
  //
  // The draft is held apart from the saved row on purpose: a zone toggle that
  // wrote straight through would change what a preacher is looking at on every
  // tap while an operator was still deciding. Save is the moment it reaches a
  // screen, and until then `layoutDirty` says there is something unsaved rather
  // than leaving the operator to remember.
  let selLayout = null;
  let layoutName = '';
  let layoutZones = { ...DEFAULT_STAGE_ZONES };
  let layoutBusy = false;
  let layoutDelArm = null;

  $: layoutSaved = stageLayouts.find((l) => l.id === selLayout) ?? null;
  $: layoutDirty =
    selLayout != null &&
    layoutSaved != null &&
    (layoutName.trim() !== layoutSaved.name ||
      STAGE_ZONES.some((z) => !!layoutZones[z.key] !== !!(readStageZones(layoutSaved.zones) ?? DEFAULT_STAGE_ZONES)[z.key]));
  /** Which screens wear this layout — the same fact the delete refusal names. */
  $: layoutWornBy = channels.filter((c) => c.stage_layout_id === selLayout).map((c) => c.name);

  function pickLayout(l) {
    selLayout = l.id;
    layoutName = l.name;
    layoutZones = { ...(readStageZones(l.zones) ?? DEFAULT_STAGE_ZONES) };
    layoutDelArm = null;
  }
  function newLayout() {
    selLayout = 'new';
    layoutName = '';
    layoutZones = { ...DEFAULT_STAGE_ZONES };
    layoutDelArm = null;
  }
  const toggleLayoutZone = (key) =>
    (layoutZones = { ...layoutZones, [key]: !layoutZones[key] });

  // THROUGH `act`, LIKE EVERY OTHER MUTATION ON THIS DESK. It stores the TYPED
  // error and `ui/ErrorState.svelte` turns it into words — which is why this
  // desk calls the humaniser nowhere itself, a fact `r6-contracts.test.js`
  // records (by raw substring, so do not name the function here either) so that
  // a later audit does not read its absence as a defect. A second error surface
  // in this editor would be a second set of words for the same refusal.
  async function saveLayout() {
    layoutBusy = true;
    await act(async () => {
      const id = await upsertStageLayout(
        selLayout === 'new' ? null : selLayout,
        layoutName,
        layoutZones,
      );
      stageLayouts = (await listStageLayouts()) ?? [];
      // Select what was just saved BY THE ID THE ENGINE GAVE BACK, rather than
      // guessing which row is new from the list — two layouts saved in one
      // sitting would make that guess wrong.
      const saved = stageLayouts.find((l) => l.id === id);
      if (saved) pickLayout(saved);
    });
    layoutBusy = false;
  }

  async function removeLayout() {
    if (layoutDelArm !== selLayout) {
      layoutDelArm = selLayout;
      return;
    }
    layoutBusy = true;
    await act(async () => {
      await deleteStageLayout(selLayout);
      stageLayouts = (await listStageLayouts()) ?? [];
      selLayout = null;
    });
    layoutDelArm = null;
    layoutBusy = false;
  }

  const assignRole = (c, e) => act(() => setChannelRole(c.id, e.target.value === '' ? null : e.target.value));
  // ── WHAT THIS SCREEN SHOWS AT ALL (DECISIONS §98) ──────────────────────────
  //
  // Read off the channel row rather than kept in a second copy here, for the
  // reason `downOf` above states about liveness: a local copy disagrees with the
  // row the moment a second console, a reconnect or a refused call moves one of
  // them — and what it would disagree about is whether a congregation sees
  // something.
  //
  // NO OPINION IS NOT AN EMPTY SET, and the UI has to show that difference rather
  // than resolve it. A screen with no opinion shows all five ticked AND says so in
  // its hint: "ticked" and "showing everything because nobody has chosen" look
  // identical on a checkbox and are different facts about the setup.
  const showsOf = (c) => {
    if (!c?.shows_json) return null;
    try {
      const v = JSON.parse(c.shows_json);
      return Array.isArray(v) ? v : null;
    } catch {
      // An unreadable value is NO OPINION, never an empty set. The backend makes
      // the same judgement in `channel_shows_json`, and the safe direction is the
      // one where the template still decides.
      return null;
    }
  };
  $: selShows = showsOf(sel);
  $: selShowsAll = selShows == null;
  /**
   * Tick or untick one kind.
   *
   * Starting from NO OPINION, the first untick has to write the other four
   * explicitly — there is no "everything except" to store, and inventing one
   * would be a second spelling of a fact the column already holds one way.
   *
   * Ticking the last missing kind goes back to NO OPINION rather than to an
   * explicit list of all five. That is not tidiness: an explicit five is a
   * standing instruction that a SIXTH content kind would be silently excluded
   * from, which is the same trap `layout.shows` already sets on the twenty-five
   * preset templates.
   */
  const toggleShows = (c, kind, on) => {
    const current = showsOf(c) ?? CONTENT_KINDS.map((k) => k.key);
    const next = on ? [...new Set([...current, kind])] : current.filter((k) => k !== kind);
    const all = next.length === CONTENT_KINDS.length;
    return act(async () => {
      await setChannelShows(c.id, all ? null : next);
      await refresh();
    });
  };
  const openNative = (c) => act(() => openChannelOutput(c.id));
  const closeNative = (c) => act(() => closeChannelOutput(c.id));

  // The ONE writer of the content-look map. `setContentTemplate` updates the
  // shared store optimistically and persists; on failure it reloads truth and
  // throws, so we only have to surface the error. No refresh() — content looks
  // are not channels.
  async function pickLook(kind, e) {
    const v = e.target.value;
    try {
      await setContentTemplate(kind, v === '' ? null : parseInt(v, 10));
      error = null;
    } catch (err) {
      error = err;
    }
  }

  /**
   * IS AN ADD ALREADY IN FLIGHT?
   *
   * `newName` is cleared AFTER the await, and Enter in the name box calls this as
   * well as the button — so a held Enter, or a double click on a slow write, added
   * the same screen twice. Two output channels with one name is not a cosmetic
   * mess: every surface that picks a screen by name (the chrome lamps, the Live
   * status pane, Copy URL) then has two rows it cannot tell apart, and one of them
   * has no template assignment anybody made on purpose.
   */
  let adding = false;
  async function add() {
    if (adding) return;
    const name = newName.trim();
    if (!name) return;
    adding = true;
    // A new screen adopts the DEFAULT template (falling back to the first built-in
    // if none is set) — the operator can reassign it per screen afterwards.
    //
    // OPEN THE NEW SCREEN IN THE INSPECTOR. Adding a screen is never the whole
    // job: a network client is useless until its URL is pasted into OBS, and a
    // native window until it is pointed at a display. The form asks for neither,
    // and both live in the inspector — which stayed shut, so the operator was left
    // on a grid of cards with the thing they had just made somewhere in it. The
    // panel that holds the next step is the one that should be open.
    let newId = null;
    await act(async () => {
      newId = await addChannel(name, newTarget, $defaultTemplateId ?? 1);
    });
    if (newId != null) selId = newId;
    adding = false;
    // `act` never rethrows — it parks the reason in `error`, which the pane
    // renders. Keep the typed name on a failure so the operator can press again
    // rather than retype it.
    if (error) return;
    newName = '';
    newTarget = 'native_window';
    showAdd = false;
  }

  // Two-step delete (no native confirm — Tauri's webview doesn't implement it).
  // First click arms the row; second within 3s deletes.
  let delArm = null;
  let delArmT;
  async function remove(c) {
    if (delArm !== c.id) {
      delArm = c.id;
      clearTimeout(delArmT);
      delArmT = setTimeout(() => (delArm = null), 3000);
      return;
    }
    clearTimeout(delArmT);
    delArm = null;
    await act(() => deleteChannel(c.id));
  }

  // `copyFailedId` is a SECOND flag rather than a sentinel in `copiedId`, because
  // `copiedId` is compared against a channel id all over the markup and a string
  // parked in it would quietly match nothing.
  let copyFailedId = null;
  async function copyUrl(c) {
    if (await writeClip(obsUrl(c))) {
      copiedId = c.id;
      copyFailedId = null;
    } else {
      copyFailedId = c.id;
      copiedId = null;
    }
    setTimeout(() => {
      copiedId = null;
      copyFailedId = null;
    }, 1500);
  }

  let copiedLan = false;
  async function copyLan() {
    copiedLan = (await writeClip(lanIp)) ? true : COPY_FAILED;
    setTimeout(() => (copiedLan = false), 1500);
  }

  // A screen's preview shows what that screen REALLY shows — the same renderer
  // the wall uses, resolved by the same resolver, so it is WYSIWYG rather than a
  // drawing of one. The stand-in is scripture, which is why the idle preview
  // resolves against the SCRIPTURE content look below.
  const PREVIEW = { reference: 'John 3:16', text: 'For God so loved the world…', translation: 'KJV' };

  // WHAT THIS SCREEN WOULD ACTUALLY WEAR.
  //
  // `templateOf(sel)` is `null` for a screen set to FOLLOW THE CONTENT LOOK, and
  // null is the answer, not a missing one (DECISIONS §70). This used to be
  // written `templateOf(sel) ?? {}`, which is truthy — so `resolveOutputTemplate`
  // never reached its `if (!channelTpl) return override` branch, `isKeyedTemplate({})`
  // said "keyed" (no layers, no background), the transparency law kept the empty
  // object, and a following screen previewed as a blank frame. The one screen
  // whose look you cannot read off its own row was the one the preview could not
  // answer for, on the panel built to answer it.
  //
  // `Output.svelte` does exactly this — `resolveOutputTemplate(t, override, pinned)
  // || DEFAULT_TEMPLATE` with a null `t` — and two surfaces describing one screen
  // must not be able to reach different conclusions about it.
  //
  // Idle, the override is the SCRIPTURE content look, because the stand-in content
  // is a verse: that is the look this screen would wear if scripture fired now. A
  // content look is never `pinned` (only a cue's deliberate choice is), so a screen
  // with a template of its own is unaffected — which is DECISIONS §29, visible.
  //
  // NAMED, not called. `lookName('scripture')` would read `$templates` and
  // `$contentTemplates` INSIDE a function, and Svelte tracks the identifiers in
  // the expression — so the preview would be correct once, by luck of ordering,
  // and never update when the look changed. This file has already been caught by
  // exactly that (`stageUrl()`, a few lines up), twice.
  //
  // THROUGH THE SAME LOOKUP THE WALL USES, and that is not a tidy-up. This tile
  // used to be the ONE surface that rendered a content look correctly: idle it
  // resolved `scriptureLook` out of `$templates` by hand, while an output page
  // read no content look at all — so the panel an operator opens to CHECK the
  // setup showed the look working over a wall that was wearing the configured
  // default, and going live made the two agree again by both being wrong. A
  // preview that is right when nothing is on air and wrong when something is has
  // it exactly backwards. `templateById` is the lookup `Output.svelte` resolves
  // `content.template_id` through; `$liveTemplateOverride` now reads the id form
  // as well as the JSON form, so the live branch cannot disagree with it either.
  $: scriptureLook = templateById($templates, $contentTemplates.scripture);
  $: previewOverride = $live ? $liveTemplateOverride : scriptureLook;
  // `$templates` is NAMED here, not reached through `templateOf`. It used to be
  // `sel ? templateOf(sel) : null`, and `templateOf` reads the store inside a
  // function body — so the inspector's preview was correct when a screen was
  // selected and never again. Edit that template on the Templates tab and every
  // CARD repainted (they name the store) while the panel beside them kept the old
  // look: two previews of one screen, disagreeing, on the same page. Same trap as
  // `stageUrl()` and `lookName('scripture')`, which this file has already been
  // caught by twice.
  $: selOwn =
    sel && sel.template_id != null ? ($templates.find((t) => t.id === sel.template_id) ?? null) : null;
  // ── THE DESK RESOLVES RUNG 3 TOO, OR IT IS THE SURFACE THAT LIES ────────────
  //
  // DECISIONS §97. The paragraph above `scriptureLook` records what happens when
  // this desk resolves one rung differently from the wall: the tile showed a
  // content look working while idle over a wall wearing the configured default,
  // so the panel an operator opens to CHECK the setup was the one surface that
  // made a broken setup look right. A preview that stopped at rung 4 would
  // reproduce that exactly, one column along.
  //
  // THE KIND IS THE KIND THE PREVIEW IS SHOWING. Idle the stand-in is a verse, so
  // the look that applies is the scripture one; live it is whatever is on air. A
  // preview that always asked about scripture would be wrong on precisely the
  // screens this feature exists for.
  //
  // `$channelLooks`, `$templates` and `$liveContent` are all NAMED here, for the
  // reason this file states three times: Svelte tracks the identifiers it can see
  // in a reactive expression, not the ones a called function happens to read.
  $: previewKind = $live ? ($liveContent?.kind ?? 'scripture') : 'scripture';
  $: previewKindLook = sel
    ? templateById($templates, lookIdFor($channelLooks, sel.id, previewKind))
    : null;
  $: previewTemplate =
    resolveOutputTemplate(
      sel ? selOwn : null,
      previewOverride,
      $live ? $liveTemplatePinned : false,
      $templates.find((t) => t.id === $defaultTemplateId) || null,
      previewKindLook,
    ) || DEFAULT_TEMPLATE;
  // What the preview is a preview OF. "Sample" said the same thing for a screen
  // with its own look and for one following a look it never showed — rule 35 in
  // small: a line that reads the same in two different situations is not a line.
  $: previewNote = $live
    ? 'Live — mirroring the program'
    : sel && sel.template_id == null
      ? `Sample — follows the content look · ${scriptureLook?.name ?? 'the default look'}`
      : 'Sample — nothing on screen';

  // ── EVERY CARD RENDERS WHAT THAT SCREEN IS SHOWING RIGHT NOW ────────────────
  //
  // docs/REBRAND.md §5. The list used to be a TABLE of names and words, which is
  // the one thing an operator cannot check by looking: "Main screen · Classic
  // Serif · LIVE" is four true facts that do not answer *is the lower third
  // sitting over the camera, or filling the frame*. A card answers it by being
  // the wall's own renderer, fed the wall's own content, resolved by the wall's
  // own resolver — so a screen wearing the wrong look is visible rather than
  // inferable.
  //
  // ONE reactive statement, and every dependency NAMED IN IT. `$templates`,
  // `$contentTemplates` (through `previewOverride`), `monitors`, `status` and
  // `shown` all appear here as identifiers, because Svelte tracks the identifiers
  // in the expression and not the ones a called function happens to read. This
  // file has been caught by exactly that twice — `stageUrl()` and
  // `lookName('scripture')` — so the lookups are inline rather than delegated to
  // `templateOf` / `monitorOf`, which read stores inside a function body.
  // ── ONE VERDICT PER SCREEN, FOR EVERY SCREEN ───────────────────────────────
  //
  // Computed over `channels`, not over `shown`: the rail's tally is about the
  // building, and a screen typed out of the search box has not stopped being
  // down. The cards, the inspector and the rail badge all read THIS object, so
  // the three cannot describe one screen three ways — which is rule 35 stated as
  // a data structure rather than as a promise three call sites have to keep.
  //
  // EVERY DEPENDENCY IS NAMED IN THE EXPRESSION — `status`, `$rehearsing`,
  // `$live`, `$screenBlack`, `$channelWaiting`, `channels` — because Svelte
  // tracks the identifiers it can SEE and not the ones a called function, or a
  // pre-rolled object, happens to read. This file has been caught by exactly that
  // twice (`stageUrl()` and `lookName('scripture')`), and `outputhealth.test.js`
  // holds the rule by scanning this very call site: the `wall` object that used
  // to sit here was hoisted into a separate `$:` and the guard failed on it, as
  // it should have. The stores are written out.
  $: verdicts = Object.fromEntries(
    channels.map((c) => [
      c.id,
      describeScreen(
        status[c.id] ?? null,
        { rehearsing: $rehearsing, live: !!$live, black: $screenBlack },
        $channelWaiting[c.id] ? Date.now() - $channelWaiting[c.id] : 0,
      ),
    ]),
  );
  // What the cards paint. Live: the actual programme, so every card repaints
  // together the moment a verse fires. Idle: the stand-in, so a template is still
  // legible on a Tuesday.
  $: cardContent = $live ? $liveContent : PREVIEW;

  // ── THE LAST FRAME EACH SCREEN WAS KNOWN TO BE SHOWING (RG-211) ────────────
  //
  // A plain `Map`, deliberately NOT a store and NOT a `$:` value. It is written
  // from inside the `cards` computation below, and a reactive container written
  // where it is read is a loop. Nothing renders from it directly either: the
  // frame a card paints is chosen once, in that same computation, so there is one
  // place to read and one place to reason about.
  //
  // It only ever holds a frame a screen ITSELF said it was painting — see
  // `shows === 'content'` below — which is what makes it honest to keep showing
  // when that screen stops answering. A map filled from Relay's belief would
  // retain frames the screen never received, which is the bug in slower motion.
  const lastFrames = new Map();
  $: cards = shown.map((c) => {
    const st = status[c.id] ?? null;
    const own = c.template_id == null ? null : ($templates.find((t) => t.id === c.template_id) ?? null);
    // THE CARD'S OWN RUNG 3, per card. `previewKindLook` above answers for the
    // SELECTED screen; every card is a different screen, so each resolves its own
    // — and a card that borrowed the inspector's would paint the selected
    // screen's look on every tile, which is a worse lie than not resolving it.
    const kindLook = templateById($templates, lookIdFor($channelLooks, c.id, previewKind));
    const tpl =
      resolveOutputTemplate(
        own,
        previewOverride,
        $live ? $liveTemplatePinned : false,
        $templates.find((t) => t.id === $defaultTemplateId) || null,
        kindLook,
      ) || DEFAULT_TEMPLATE;
    const i = parseInt(c.display_target ?? '', 10);
    const mon = Number.isFinite(i) ? (monitors.find((m) => m.index === i) ?? null) : null;
    // ── WHAT THIS CARD MAY PAINT, from the SAME verdict as its badge ─────────
    //
    // Every card used to be handed `cardContent` — the programme — whatever its
    // screen was doing. So a screen that had dropped off the network painted the
    // verse that fired after it died, in full, under a badge reading **Not
    // responding**, and a screen the operator had taken down painted the
    // programme it had been taken out of. The picture is the half of this
    // surface an operator actually looks at, so the card was telling them the
    // opposite of its own label (rule 35, RG-211).
    //
    // `shows` comes from `describeScreen`, beside the word — one rule, one
    // place, so the two cannot drift apart.
    const shows = verdicts[c.id]?.shows ?? 'unknown';
    if ($live && shows === 'content' && cardContent) lastFrames.set(c.id, cardContent);
    // WITH NOTHING ON THE PROGRAMME every card shows the stand-in, and that is
    // not the same decision as `blank`. On a Tuesday there is no content to be
    // wrong about, and a grid of empty boxes answers none of the questions this
    // tab is opened to ask about the LOOKS.
    const frame = !$live
      ? cardContent
      : shows === 'content'
        ? cardContent
        : shows === 'stale'
          ? (lastFrames.get(c.id) ?? null)
          : null;
    return {
      c,
      st,
      tpl,
      mon,
      frame,
      // A frame is stale when it is the last thing this screen was KNOWN to be
      // showing and the programme has since moved on. It is said in words on the
      // card, because a picture that is merely old looks exactly like a picture
      // that is current.
      stale: !!($live && shows === 'stale' && frame),
      // THE SAME RULE LIVE USES, with the same four inputs (rule 35), and now the
      // same OBJECT the inspector and the rail read — see `verdicts`. The cards
      // used to derive their word from `FAULT_WORD[screenFault(st)]`, which knows
      // nothing about rehearsal or a blackout — so a card could read LIVE in
      // amber-adjacent green over a rehearsal no congregation was watching.
      d: verdicts[c.id],
      // The same helper the inspector's Actions row uses — see `selSwitch`.
      sw: screenSwitch(st, c),
      // A KEYED template is a lower third: it paints a band and leaves the rest
      // transparent, so on a black card it reads as a stripe floating in nothing.
      // The plate is what it is actually over — a camera — and it is LABELLED, so
      // it can never be mistaken for something Relay is sending.
      plate: isKeyedTemplate(tpl),
    };
  });
  // What drives this screen, in the words the prototype's meta line uses. A
  // native screen names its display; a networked one names the ports it is served
  // on. Neither is a picker for a networked screen on purpose — see the markup.
  const outputOf = (c, mon) => {
    if (c.render_target === 'native_window') {
      if (mon) return `${mon.name} · ${mon.width}×${mon.height}`;
      // A MISSING DISPLAY AND NO DISPLAY ARE DIFFERENT THINGS, and this line said
      // "Primary display" for both. `display_target` is an INDEX into the OS
      // monitor list, so unplugging a dock renumbers it and a screen configured
      // for the projector reads exactly like a screen configured for nothing —
      // one reassuring sentence over two situations, which is rule 35, on the
      // control that decides which physical screen a congregation sees. Pressing
      // Open in that state is now refused by the backend, by name; this is the
      // half that says so before the operator presses it.
      const want = parseInt(c.display_target ?? '', 10);
      if (Number.isFinite(want)) return `Display ${want + 1} — not connected`;
      return 'Primary display';
    }
    // NDI IS PARKED, AND THE CARD SHOULD SAY SO WHERE IT IS READ, not only in a
    // `title` nobody hovers. This was an em dash, which reads as "not set yet" —
    // a thing an operator would go looking for a way to configure. There is none
    // and there is not going to be one in this build: NDI needs a proprietary SDK
    // Relay does not ship, and `open_ndi_output` returns that as a plain error.
    // The card correctly offers NO control here, which is the half that matters
    // (an affordance that cannot work is worse than an absence); this is the
    // other half, which is telling the operator why the absence is deliberate.
    if (c.render_target === 'ndi_encode') return 'not available in this build';
    return ':8032 / :8031';
  };
  // The inspector resolves through `previewTemplate` above — the SAME expression,
  // not a lookup into `cards`, because `cards` is filtered by the search box and a
  // selected screen that has been typed out of the list must not lose its panel.
  $: selPlate = isKeyedTemplate(previewTemplate);
  $: selReport = screenReporting(selStatus);
  // The inspector's own lamp — the SAME OBJECT the card behind it reads, so the
  // panel and the card cannot describe one screen two ways.
  $: selDescribe = (sel && verdicts[sel.id]) || { kind: 'unknown', label: 'Checking…', note: '' };
  // ── THE ON/OFF CONTROL COMES FROM THE HELPER THAT OWNS IT ──────────────────
  //
  // This was a private `selStatus?.online ? 'Turn off' : 'Turn on'` ternary, in
  // markup, on both the card and the inspector — a third and fourth opinion about
  // a screen, in the one file that already carries a paragraph about why the
  // badge beside it is not one. `screenSwitch` exists for this, Live already uses
  // it, and `outputhealth.test.js` pins the case the ternary got wrong: before the
  // first poll `status[id]` is undefined, so `!online` was true and the card
  // offered **Turn on** for a screen that may well already be open. That is a
  // guess printed as a control, and pressing it opens a second window.
  $: selSwitch = screenSwitch(selStatus, sel);

  // ══ THE PREACHER'S SCREEN ══════════════════════════════════════
  //
  // ONE SECTION FOR THE THING AN OPERATOR HAS TO SET UP WITH A TABLET IN THEIR
  // HAND: what that tablet has been told to show, the link and QR that open it,
  // whether it is answering, and the layout behind it. The journey it replaces
  // is Sharing (for the link) then Stage layouts (for the zones) then Screens
  // (to assign one) — three sections for one screen.
  //
  // ── WHY THIS IS A PROJECTION AND NOT AN EMBEDDED `Stage.svelte` ─────────
  //
  // The high-fidelity option was to mount the real page in a frame. It was
  // rejected: `Stage.svelte` opens its OWN WebSocket to the kiosk hub and its own
  // HTTP calls, so every operator sitting on this tab would be a second client on
  // the hub — counted in the very liveness tally this desk renders, and the
  // `answering` badge would go up by one because somebody OPENED A TAB. An
  // instrument that changes the reading it takes is not an instrument. It would
  // also have mirrored THIS machine's view, not the tablet's: the zones a stage
  // page draws come from that device's own `localStorage` when no layout is
  // assigned, so an embedded copy would paint the console's defaults and call
  // them the preacher's.
  //
  // So the panel composes what the console genuinely knows, and `stagemirror.js`
  // is the pure module that decides which of those questions have an answer.
  // Everything else says so — rule 35, on the one panel that exists because the
  // screen is out of sight.
  // ── THE REASON IS RENDERED BY THE ONE HUMANISER, NOT SPLICED INTO A SENTENCE ─
  //
  // Live builds this field by calling `src/lib/errors.js` itself, and this desk
  // deliberately cannot: `r6-contracts.test.js` asserts by RAW SUBSTRING that the
  // humaniser is named nowhere in this file — comments included, which is why
  // this paragraph does not name it either — because every refusal here goes
  // through `ui/ErrorState.svelte`, and a second set of words for one failure is
  // the defect that test exists to hold. So the flag says only THAT the read
  // failed, which is all `describeStageReach` needs to stop claiming a quiet
  // Sunday, and the `<ErrorState>` in the pane beneath prints the sentence.
  $: stageList = {
    read: !loading,
    error: $readErrors.listOutputChannels ? 'the screens could not be read' : '',
  };
  // THE SCREEN THIS SECTION IS ABOUT — the same one `stageRemoteUrl` hands out an
  // address for, including the operator's choice between several stages (§89).
  // Reading it off `stageRemote` rather than re-finding it is what stops the QR
  // and the mirror describing two different tablets.
  $: mirrorChannel = stageRemote.channel;
  $: mirrorLayout =
    mirrorChannel?.stage_layout_id != null
      ? (stageLayouts.find((l) => l.id === mirrorChannel.stage_layout_id) ?? null)
      : null;
  $: mirrorZones = stageMirrorZones({
    channel: mirrorChannel,
    layout: mirrorLayout,
    list: stageList,
  });
  // The SAME verdict every card and the inspector read (`verdicts`), through the
  // SAME helper Live's Stage Timer band reads. Two surfaces describing one screen
  // must not be able to disagree about it.
  $: mirrorReach = describeStageReach(
    channels.map((c) => ({ c, st: status[c.id] ?? null, s: verdicts[c.id] ?? null })),
    stageList,
  );
  $: mirrorReadingZone =
    mirrorZones.zones.find((z) => z.key === 'reading')?.state ?? 'unknown';
  $: mirrorReading = stageMirrorReading({
    zone: mirrorReadingZone,
    live: $live,
    black: $screenBlack,
    down: mirrorChannel ? ($channelHealth?.[mirrorChannel.id]?.down ?? null) : null,
    rehearsing: $rehearsing,
  });
  /** The words for a zone row. Never a tick and a blank — see the markup. */
  const ZONE_WORD = { on: 'Showing', off: 'Hidden', unknown: 'Not known' };

  // ── THE STAGE TIMERS, READ WHILE THIS SECTION IS OPEN ──────────────────
  //
  // `null` = never read. `[]` = read, and there are none. Live's band draws that
  // same distinction and for the same reason: a failed read that answered `[]`
  // would render as a quiet programme nobody was told about.
  //
  // The read runs only while this section is showing. A poll behind a section
  // nobody is looking at is a cost with no reader, and this one has no second
  // consumer — Live owns the band that starts and stops these clocks.
  let mirrorTimers = null;
  // The TYPED error, not a string — `ui/ErrorState.svelte` is the one thing on
  // this desk that turns a Rust refusal into words.
  let mirrorTimersErr = null;
  let mirrorNow = Date.now();
  let mirrorTick;
  let mirrorTicks = 0;
  async function readStageTimers() {
    try {
      mirrorTimers = stageTimers(await listTimers());
      mirrorTimersErr = null;
    } catch (e) {
      // THE LAST GOOD LIST STAYS ON SCREEN, with the reason beside it. Emptying
      // it would report a quiet programme this desk was never told about.
      mirrorTimersErr = e;
    }
  }
  onMount(() => {
    mirrorTick = setInterval(() => {
      mirrorNow = Date.now();
      mirrorTicks += 1;
      // Every other second, and only while somebody is looking at it.
      if (view === 'stage' && mirrorTicks % 2 === 0) readStageTimers();
    }, 1000);
    return () => clearInterval(mirrorTick);
  });
  // The FIRST read is on arrival rather than up to two seconds later, so the
  // panel does not open on "Reading…" for a section an operator clicked into.
  $: if (view === 'stage' && mirrorTimers === null && !mirrorTimersErr && $capture.available)
    readStageTimers();
  $: mirrorRows = (mirrorTimers ?? []).map((t) => {
    // THE SAME CHAIN THE PREACHER'S RAIL USES — `timers.js` → `countdown.js`, one
    // reader, so this figure and the one on the tablet cannot drift. `past: true`
    // lifts the floor because past zero the rail counts UP (DECISIONS §99), and a
    // figure that stopped at 0:00 would tell an operator a sermon twelve minutes
    // over had just run out.
    const left = timerRemainingMs(t, mirrorNow, { past: true });
    return {
      id: t.id,
      label: (t.label ?? '').trim(),
      left,
      over: left != null && left <= 0,
      held: timerIsHeld(t),
    };
  });
  /** Assign a layout from THIS section — the same command the Screens inspector calls. */
  const assignMirrorLayout = (e) =>
    mirrorChannel ? assignStageLayout(mirrorChannel, e) : undefined;
  /** Open the layout editor on the layout this screen wears. */
  function editMirrorLayout() {
    if (mirrorLayout) pickLayout(mirrorLayout);
    else newLayout();
    view = 'layouts';
  }
</script>

<!-- NO PAGE TITLE, NO STANDFIRST — and that is the whole point (§2).
     Measured at 1280×900: the `Outputs` H1 plus its two-line standfirst started
     this desk 110px lower than the prototype's, which is grid-and-inspector with
     the words carried by the dock head. The H1 was also the third `Outputs` on
     the screen, after the tab in the chrome bar and the rail's own pane title.

     The RAIL STAYS, and the two rail entries are NOT folded into a segmented
     control in the dock head. Doing that would leave Outputs a two-column
     workspace, and `workspacegrammar.test.js` holds six desks to three tracks
     through one frame — dropping a column here is exactly the per-file drift
     that frame exists to prevent. Content looks and Sharing hold real controls
     and keep their place; only the chrome above them went.

     The sentence each section is FOR moved into the rail foot, where it costs
     the grid no height and is visible on all three sections. -->
<WorkspaceFrame columns="var(--v-rail) minmax(0,1fr) var(--v-insp)">
  <!-- ══ RAIL ══ One vocabulary, three sections. Every output concern lives
       behind exactly one of these words, and the rail keeps all three in view
       rather than making one of them a mode you have to remember you are in. -->
  <aside class="rw-pane">
    <div class="rw-panehead">
      <h2 class="rw-panettl">Outputs</h2>
      <span class="rw-spring"></span>
      {#if !$capture.available}
        <span class="r-badge rose sm-badge"><span class="bd"></span>No engine</span>
      {:else if $capture.outputError}
        <!-- THE LAN SERVER IS NOT RUNNING (RG-191). Every network screen is dead
             and Copy URL would hand out an address nothing answers. The fact was
             in the store and rendered on Live only; this is the desk that hands
             out the URLs, so it says so first. -->
        <span class="r-badge rose sm-badge" title="Another program is holding Relay's ports, so OBS, kiosk screens and the preacher's phone cannot connect. The projector window is unaffected. The status bar's Reduced cell has the detail."><span class="bd"></span>Server not running · :8032 / :8031</span>
      {:else}
        <!-- NEVER AMBER: amber means something is on the wall, and a screen
             answering does not put it there. Green is "confirmed", and it is now
             earned — the count is screens that have themselves reported painting,
             not screens Relay is serving. ROSE the moment one has stopped, using
             the same verdict the card shows, because a green tally over a dead
             screen is the reassuring-sentence-over-a-broken-thing failure in the
             smallest space on the desk. -->
        <span class="r-badge {anyDown ? 'rose' : 'green'} sm-badge"
          title="{answering} of {channels.length} screens are reporting that they are still painting">
          <span class="bd"></span>{answering}/{channels.length}</span>
      {/if}
    </div>
    <nav class="rw-panebody" aria-label="Outputs sections">
      {#each VIEWS as v (v.key)}
        <button class="rw-item r-focus" class:on={view === v.key}
          aria-current={view === v.key} on:click={() => (view = v.key)}>
          <span class="rw-itemname">{v.label}</span>
          {#if v.key === 'screens'}<span class="rw-itemn">{counts.all}</span>{/if}
          {#if v.key === 'looks'}<span class="rw-itemn">{followers.length}</span>{/if}
        </button>
      {/each}
    </nav>
    <div class="rw-panefoot ch-railfacts">
      <!-- What this section is FOR — role two of the type scale, moved off the
           page head and onto the rail. -->
      <p class="ch-raillead">{activeView.lead}</p>
      <!-- The two facts an operator asks this tab for without opening anything.
           `Backend not attached` still has to be sayable in words somewhere an
           operator will read it, not only as a badge. -->
      {#if !$capture.available}
        <div class="ch-railfact"><span class="ch-railk">Engine</span><span class="ch-railv ch-railbad r-mono">Backend not attached</span></div>
      {:else}
        <!-- "Answering", not "Live". The old word claimed the screens were
             showing something; the fact behind it only ever said Relay was
             serving them. This one names exactly what the number counts. -->
        <div class="ch-railfact"><span class="ch-railk">Answering</span><span class="ch-railv r-mono" class:ch-railbad={anyDown}>{answering} / {channels.length}</span></div>
      {/if}
      <div class="ch-railfact"><span class="ch-railk">This machine</span><span class="ch-railv r-mono">{lanIp}</span></div>
    </div>
  </aside>

  {#if view === 'screens'}
    <section class="rw-pane">
      <!-- Filter tabs. Relay's real taxonomy is the render target, so these ARE
           the render targets — the reference's separate "Network" and "Browser
           Sources" tabs are one thing here (a browser source IS a network
           client), and splitting them would imply a distinction the engine does
           not make. -->
      <!-- One clean pane head: name · search · Add. The old type-filter tab row
           (All / Network / Native) was chrome for a list of a handful of screens —
           removed to keep this surface calm. -->
      <div class="rw-panehead ch-panehead">
        <h2 class="rw-panettl">Screens</h2>
        <Field class="ch-search">
          <svelte:fragment slot="icon"><svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg></svelte:fragment>
          <input placeholder="Search screens…" bind:value={q} aria-label="Search screens" />
        </Field>
        <span class="rw-spring"></span>
        <span class="ch-headnote r-mono">each card renders what that screen shows right now</span>
        <button class="r-btn primary sm" on:click={() => (showAdd = !showAdd)} disabled={!$capture.available}>
          ＋ Add Screen
        </button>
      </div>

      {#if showAdd}
        <div class="ch-addbar">
          <input class="r-input" placeholder="New screen name" bind:value={newName} on:keydown={(e) => e.key === 'Enter' && add()} />
          <select class="r-select" bind:value={newTarget}>
            <option value="native_window">Native window (HDMI / display)</option>
            <option value="network_client">Network client (OBS / kiosk)</option>
          </select>
          <button class="r-btn primary sm" on:click={add} disabled={!newName.trim() || adding}>
            {adding ? 'Adding…' : 'Add'}
          </button>
          <button class="r-btn ghost sm" on:click={() => (showAdd = false)}>Cancel</button>
        </div>
      {/if}

      <div class="rw-panebody ch-gridwrap">
        {#if loading}
          <Loading what="screens" />
        {:else if cards.length}
          <!-- A GRID OF SCREENS, NOT A TABLE OF WORDS — docs/REBRAND.md §5.
               This was a table: name, type, template, target, status. Five true
               facts, and not one of them answers the question an operator opens
               this tab to ask — *is the lower third sitting over the camera or
               filling the frame; is the lobby screen wearing the warm look or the
               main one*. A word cannot be checked by looking.

               Each card is the wall's OWN renderer (`TemplateRender`, the one
               renderer), fed the wall's OWN content, resolved by the wall's OWN
               resolver — so every card repaints together the moment a verse
               fires, each in its own look. A screen wearing the wrong template is
               then visible rather than inferable. -->
          <div class="ch-cards">
            {#each cards as k (k.c.id)}
              <div class="ch-card" class:sel={k.c.id === selId} class:down={k.d.kind === 'down'}
                role="button" tabindex="0" aria-label="{k.c.name} — {k.d.label}"
                aria-pressed={k.c.id === selId}
                on:click={() => (selId = selId === k.c.id ? null : k.c.id)}
                on:keydown={(e) => {
                  // Only the card itself. Without this, Space inside the template
                  // picker — which is how a keyboard OPENS a select — was
                  // swallowed by preventDefault and toggled the selection instead.
                  if (e.target !== e.currentTarget) return;
                  if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selId = selId === k.c.id ? null : k.c.id; }
                }}>
                <div class="ch-frame">
                  <!-- A KEYED template is a lower third: it paints a band and
                       leaves the rest transparent, so against the card's black it
                       reads as a stripe floating in nothing. The plate is what the
                       band is actually over, and it is LABELLED — an unlabelled
                       picture on this surface could be mistaken for something
                       Relay is sending, and Relay sends no video. -->
                  {#if k.plate}
                    <CameraPlate />
                  {/if}
                  <!-- THE CARD PAINTS ITS OWN SCREEN, not the programme (RG-211).
                       `k.frame` is null for a screen that is showing nothing — a
                       blackout, a screen taken down, one that has said it is
                       clear — and an empty frame is the honest picture of that.
                       A screen that has stopped answering keeps the last frame it
                       was known to have and SAYS the frame is old, because a
                       picture that is merely old looks exactly like a current
                       one. -->
                  {#if k.frame}
                    <TemplateRender template={k.tpl} content={k.frame} />
                  {/if}
                  {#if k.stale}
                    <span class="ch-stale r-mono">Last seen</span>
                  {/if}
                </div>

                <div class="ch-cardtop">
                  <span class="ch-cardname">{k.c.name}</span>
                  <!-- THE SAME WORD LIVE USES, from the same helper on the same
                       backend fact (rule 35). The table decided this from
                       `FAULT_WORD[screenFault(st)]`, which knows nothing about a
                       rehearsal or a blackout — so one surface could call a screen
                       LIVE while the other called it Rehearsal, about the same
                       screen, in the same second. -->
                  <!-- `title` carries the note the card has no room for. A card
                       is 232px of picture and two words; the sentence explaining
                       why a screen is not confirmed lives in the inspector, and
                       this puts it one hover away rather than one click. -->
                  <span class="r-badge {SCREEN_BADGE[k.d.kind]} ch-lamp" title={k.d.note}><span class="bd"></span>{k.d.label}</span>
                </div>
                <!-- ONE TEXT TERM, no new lamp and no new colour. A per-kind look
                     is configuration, and configuration does not get amber, cyan
                     or amethyst — those mean ON AIR, a guess and a rehearsal. -->
                <div class="ch-cardmeta r-mono">{kindOf(k.c)} · {transportOf(k.c)}{perKindTerm(k.c, $channelLooks) ? ` · ${perKindTerm(k.c, $channelLooks)}` : ''}</div>

                <!-- `stopPropagation`: using a control must not also toggle the
                     selection of the card underneath it. -->
                <span class="ch-cardrow" on:click|stopPropagation role="presentation">
                  <select class="r-select ch-cardpick" aria-label="Template for {k.c.name}"
                    value={k.c.template_id ?? ''} on:change={(e) => assignTemplate(k.c, e)}
                    disabled={!$capture.available}>
                    <option value="">Follow the content look</option>
                    {#each $templates as t (t.id)}
                      <option value={t.id}>{t.name}</option>
                    {/each}
                  </select>
                </span>

                <span class="ch-cardrow" on:click|stopPropagation role="presentation">
                  {#if isNative(k.c)}
                    <select class="r-select ch-cardpick" aria-label="Display for {k.c.name}"
                      value={k.c.display_target ?? ''} on:change={(e) => assignDisplay(k.c, e)}
                      disabled={!$capture.available}>
                      <!-- NOT "Primary display" (RG-188): with nothing chosen a manual
                           Turn on now refuses rather than opening over this console,
                           and the option says what the state IS, not where it would go. -->
                      <option value="">{monitors.length > 1 ? 'Choose a display…' : 'This display'}</option>
                      {#each monitors as m (m.index)}
                        <option value={String(m.index)}>{m.name} · {m.width}×{m.height}{m.primary ? ' (primary)' : ''}</option>
                      {/each}
                      <!-- THE DISPLAY THIS SCREEN NAMES, WHEN IT IS NOT THERE.
                           Without this option the select falls back to showing
                           its first — "Primary display" — and a screen set to the
                           projector reads exactly like a screen set to nothing.
                           It is a real option so the operator can leave it alone
                           (plug the projector back in) as easily as change it. -->
                      {#if missingDisplay(k.c)}
                        <option value={k.c.display_target}>Display {missingDisplay(k.c)} — not connected</option>
                      {/if}
                    </select>
                    <!-- `screenSwitch`, not a ternary on `online`. Before the
                         first poll `k.st` is null and `!online` was true, so this
                         offered **Open** for a screen that may already be open —
                         a guess printed as a control, and pressing it opens a
                         second window on the projector. The helper answers
                         `action: null` for that case and the control says
                         Checking… instead. `outputhealth.test.js` pins it. -->
                    {#if k.sw.action === 'off'}
                      <button class="r-btn ghost sm" on:click={() => closeNative(k.c)}>Close</button>
                    {:else if k.sw.action === 'on'}
                      <button class="r-btn ghost sm" on:click={() => openNative(k.c)} disabled={!$capture.available}>Open</button>
                    {:else}
                      <span class="ch-cardout r-mono" title={k.sw.why}>{k.sw.label}</span>
                    {/if}
                  {:else}
                    <!-- READ-ONLY, and deliberately NOT the prototype's picker. A
                         networked screen's source is a page on somebody else's
                         device; `display_target` is parsed as a monitor INDEX and
                         is read for nothing else, so a picker here would save a
                         preference nothing in Relay reads — which is the exact
                         defect DECISIONS §69 closed seven of. -->
                    <span class="ch-cardout r-mono">{outputOf(k.c, k.mon)}</span>
                    {#if !isNdi(k.c)}
                      <button class="r-btn ghost sm" on:click={() => copyUrl(k.c)}>{copyFailedId === k.c.id ? COPY_FAILED : copiedId === k.c.id ? 'Copied ✓' : 'URL'}</button>
                    {/if}
                  {/if}
                </span>
              </div>
            {/each}
          </div>
        {:else if !channels.length && $readErrors.listOutputChannels}
          <!-- RG-95, second pass. This view HAD an `<ErrorState>` and it could not
               fire: every read in `onMount` is a GROUP 2 wrapper that swallows to a
               safe default, so `error` was only ever set by `act()` — a mutation.
               A database that would not open therefore read "No screens yet — add
               one below.", and the operator's answer to that sentence is to add a
               screen they already have. -->
          <ErrorState error={$readErrors.listOutputChannels} onRetry={refresh} />
        {:else}
          <EmptyState message={channels.length ? 'No screen matches this filter.' : 'No screens yet — add one below.'} />
        {/if}

        <button class="ch-addcard" on:click={() => (showAdd = true)} disabled={!$capture.available}>
          <span class="ch-addmark">＋</span>
          <span class="ch-addttl">Add New Screen</span>
          <span class="ch-addsub">Configure a display for HDMI, or a networked OBS / kiosk source for your venue.</span>
        </button>
        <ErrorState {error} />
      </div>
    </section>

  {:else if view === 'stage'}
    <!-- ══ THE PREACHER'S SCREEN ══ A PROJECTION, AND IT SAYS SO.
         Every row here is something Relay SENT or something Relay was told. The
         one thing this desk can never have is a readback: the kiosk hub records
         nothing about who connected (DECISIONS §35) and no frame carries back
         what a page decided to draw. So each answer that does not exist is
         printed as "Not known" rather than as a confident row — rule 35, on the
         one panel built because the screen is out of sight. -->
    <section class="rw-pane">
      <div class="rw-panehead">
        <h2 class="rw-panettl">Preacher's screen</h2>
        <span class="rw-spring"></span>
        <span class="ch-headnote r-mono">what Relay told it to show — not a readback</span>
      </div>
      <div class="rw-panebody pad">
        <p class="ch-mhead" class:ch-mwarn={mirrorZones.kind !== 'assigned'}>{mirrorZones.headline}</p>
        <!-- WHY the read failed, in the words the ONE humaniser gives it. The
             line above says only THAT it did, so that this desk keeps its single
             error surface (`r6-contracts.test.js`) rather than growing a second
             set of words for one refusal. -->
        {#if $readErrors.listOutputChannels}
          <ErrorState error={$readErrors.listOutputChannels} onRetry={refresh} />
        {/if}
        {#if mirrorZones.note}
          <p class="ch-stage-sub r-dim">{mirrorZones.note}</p>
        {/if}

        <!-- WHICH SCREEN, AND IS ANYBODY THERE. The same sentence Live's Stage
             Timer band prints, from the same helper on the same backend fact
             (rule 35). It says `attached` and never `on stage`: attached is a
             claim about a socket, and that is the most this side of the room can
             honestly make. -->
        <p class="ch-mreach r-mono"
          class:ch-mwarn={mirrorReach.kind === 'norole' || mirrorReach.kind === 'down' || mirrorReach.kind === 'unknown'}
          >{mirrorReach.text}</p>

        <div class="r-lbl ch-flbl">Zones</div>
        <ul class="ch-mzones">
          {#each mirrorZones.zones as z (z.key)}
            <!-- NEVER A TICK AND A BLANK. An unticked box and a box nobody can
                 see the state of look identical, and they are the difference
                 between a decision an operator made and one they cannot see. -->
            <li class="ch-mz" class:on={z.state === 'on'} class:off={z.state === 'off'}
              class:unk={z.state === 'unknown'}>
              <span class="ch-mzk">{z.label}</span>
              <span class="ch-mzv r-mono">{ZONE_WORD[z.state]}</span>
            </li>
          {/each}
        </ul>

        <div class="r-lbl ch-flbl">Reading</div>
        <p class="ch-mread" class:ch-mwarn={mirrorReading.kind === 'unknown'}>{mirrorReading.headline}</p>
        {#if mirrorReading.kind === 'content' && $liveContent}
          <!-- WHAT WENT OUT, IN WORDS, not through `TemplateRender`. That
               component renders an output LOOK and a stage page draws its own
               zones in its own type — so a template preview here would be a
               picture of a screen that does not exist, which is a worse answer
               than the words. -->
          <div class="ch-mcontent">
            {#if $liveContent.reference}
              <span class="ch-mref">{$liveContent.reference}</span>
            {/if}
            {#if $liveContent.text}
              <span class="ch-mtext">{$liveContent.text}</span>
            {/if}
          </div>
        {/if}
        {#if mirrorReading.caveat}
          <p class="ch-stage-sub r-dim">{mirrorReading.caveat}</p>
        {/if}

        <div class="r-lbl ch-flbl">Stage Timer</div>
        <!-- THREE ANSWERS, NOT TWO, exactly as Live's band draws them: not read
             yet · read and empty · read and refused. A failed read keeps the last
             good list and prints the reason beside it rather than reporting a
             quiet programme nobody was told about. -->
        {#if mirrorTimersErr}
          <ErrorState error={mirrorTimersErr} onRetry={readStageTimers} />
        {/if}
        {#if mirrorTimers == null && !mirrorTimersErr}
          <p class="ch-stage-sub r-dim">Reading…</p>
        {:else if mirrorRows.length}
          <ul class="ch-mtimers">
            {#each mirrorRows as t (t.id)}
              <li class="ch-mtimer">
                {#if t.label}<span class="ch-mtname">{t.label}</span>{/if}
                <!-- PAST ZERO IT COUNTS UP, the same way the preacher's rail
                     does (DECISIONS §99) — the word carries the sign, never a
                     colour, because amber is ON AIR and a sermon running long is
                     not any of the four law colours. -->
                <span class="ch-mtv r-mono"
                  >{t.left == null ? 'no deadline' : t.over ? `+${formatCountdown(-t.left)} over` : formatCountdown(t.left)}</span>
                {#if t.held}<span class="ch-mtheld">held</span>{/if}
              </li>
            {/each}
          </ul>
          <p class="ch-stage-sub r-dim">
            Started and stopped on <b>Live</b>. Whether this screen draws them is the
            <b>Stage Timer</b> zone above.
          </p>
        {:else}
          <p class="ch-stage-sub r-dim">No Stage Timer is running.</p>
        {/if}
      </div>
    </section>

  {:else if view === 'layouts'}
    <!-- ══ STAGE LAYOUTS ══ A layout is GLOBAL and its assignment is per screen
         (DECISIONS §103). The list lives here; which screen wears which is on a
         screen's own card in Screens, because that is a decision about a screen. -->
    <section class="rw-pane">
      <div class="rw-panehead">
        <h2 class="rw-panettl">Stage layouts</h2>
        <button class="r-btn ghost sm" on:click={newLayout} disabled={!$capture.available}>New layout</button>
      </div>
      <div class="rw-panebody">
        {#each stageLayouts as l (l.id)}
          <button
            class="rw-nv ch-lrow"
            class:on={selLayout === l.id}
            aria-pressed={selLayout === l.id}
            on:click={() => pickLayout(l)}>
            <span class="rw-nvk">{l.name}</span>
            <!-- WHICH SCREENS WEAR IT, on the row. Without it an operator has to
                 open every screen to find out what a layout is doing, and the
                 delete refusal would be the first time they were told. -->
            <span class="rw-nvv">{channels.filter((c) => c.stage_layout_id === l.id).map((c) => c.name).join(', ') || 'not in use'}</span>
          </button>
        {:else}
          <div class="ch-empty r-empty">No stage layouts. Make one with <b>New layout</b>.</div>
        {/each}
      </div>
    </section>
  {:else if view === 'looks'}
    <!-- ══ CONTENT LOOKS ══ THE one writer of the type → template default map.
         Every other surface that shows an assignment reads the shared store and
         is read-only (Decision §25). A row here is a name and a value, like every
         other row on the desk — the value happens to be a picker. -->
    <section class="rw-pane">
      <div class="rw-panehead"><h2 class="rw-panettl">Content looks</h2></div>
      <div class="rw-panebody">
        <!-- RG-95, third door. `$templates` starts empty and `loadTemplates`
             swallows to `[]`, so this pane said "No templates yet — make one in the
             Templates tab first" in BOTH of the other two situations: while the read
             was still in flight, and when it had failed. A fresh install ships five
             built-in templates, so that sentence can never be true of a working
             Relay — and the operator's answer to it is to go and build five more.
             Same three facts, same order, same components as Screens above. -->
        {#if loading && !$templates.length}
          <Loading what="templates" />
        {:else if !$templates.length && $readErrors.loadTemplates}
          <ErrorState error={$readErrors.loadTemplates} onRetry={loadTemplates} />
        {:else if !$templates.length}
          <EmptyState message="No templates yet — make one in the Templates tab first." />
        {:else}
          {#each CONTENT_KINDS as k (k.key)}
            <div class="rw-nv">
              <label class="rw-nvk" for="look-{k.key}">{k.label}</label>
              <select id="look-{k.key}" class="r-select rw-nvctl ch-lookselect"
                value={$contentTemplates[k.key] ?? ''}
                on:change={(e) => pickLook(k.key, e)}
                disabled={!$capture.available}>
                <option value="">Each screen's own template</option>
                {#each $templates as t (t.id)}
                  <option value={t.id}>{t.name}</option>
                {/each}
              </select>
            </div>
          {/each}
        {/if}
        <div class="ch-pad">
          <p class="rw-foot">
            When the AI fires a verse, a song or an announcement it wears the look set
            here — but only on a screen that has <b>no template of its own</b>. A
            screen's own look wins (DECISIONS §29), so setting one of these changes
            nothing on a screen you have already assigned. The inspector lists the
            screens this actually reaches.
          </p>
          <ErrorState {error} />
        </div>
      </div>
    </section>

  {:else}
    <!-- ══ SHARING ══ the addresses other devices in the building type in. -->
    <section class="rw-pane">
      <div class="rw-panehead"><h2 class="rw-panettl">This machine on the network</h2></div>
      <div class="rw-panebody">
        <div class="rw-nv">
          <span class="rw-nvk">This machine</span>
          <span class="rw-nvctl ch-addr-row">
            <span class="ch-addr">{lanIp}</span>
            <button class="r-btn ghost sm" on:click={copyLan}>{copyLabel(copiedLan, 'Copy')}</button>
          </span>
        </div>
        <div class="rw-nv"><span class="rw-nvk">Output / stage pages</span><span class="rw-nvv">:8032 · http</span></div>
        <div class="rw-nv"><span class="rw-nvk">Live update channel</span><span class="rw-nvv">:8031 · websocket</span></div>
        <div class="ch-pad">
          <p class="rw-foot">
            Kiosk screens, the OBS machine and the preacher's phone all pull the live
            output from this computer over the same Wi-Fi. Point a browser source at a
            screen's <b>Copy URL</b> in the Screens section — a hand-built address will
            not follow a template change.
          </p>
        </div>
      </div>
    </section>
  {/if}

  <!-- ══ INSPECTOR ══ what is true of the thing in hand. One rail for all three
       sections: on Screens the selected screen, on Content looks the screens a
       look actually reaches, on Sharing the one output a church sets up by hand
       on somebody else's device every week. -->
  {#if view === 'screens'}
    <aside class="rw-pane rw-insp">
      {#if !sel}
        <div class="rw-panehead"><h2 class="rw-panettl">Screen</h2></div>
        <div class="ch-empty r-empty">Pick a screen to configure it.</div>
      {:else}
        <div class="rw-panehead">
          <h2 class="rw-panettl ch-inspttl">{sel.name}</h2>
          <!-- ONE HELPER, not a ternary chain. This read
               `!supported ? UNAVAILABLE : online ? LIVE : IDLE` — a third ladder
               about a screen's health, written out in markup where no test could
               reach it without mounting the view, and blind to the rehearsal and
               the blackout the cards beside it already knew about. -->
          <span class="r-badge {SCREEN_BADGE[selDescribe.kind]} ch-lamp"><span class="bd"></span>{selDescribe.label}</span>
          <button class="r-iconbtn ch-close" aria-label="Close panel" on:click={() => (selId = null)}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M18 6 6 18M6 6l12 12"/></svg>
          </button>
        </div>

        <div class="rw-panebody pad">
          <!-- Same renderer as the wall, showing the SAME content the wall is
               showing: when something is live it mirrors the program (through this
               screen's template + any content-type override), exactly as this
               output is rendering it right now. Only when nothing is live does it
               fall back to a sample so the template is still previewable. This is
               what makes "select a screen" agree with what is actually on air. -->
          <div class="ch-preview">
            <!-- The same plate rule as the cards: a keyed template is a band over
                 something, and on this surface that something is a camera. -->
            {#if selPlate}
              <CameraPlate />
            {/if}
            <!-- Resolve EXACTLY like the real output: the screen's OWN template
                 wins (so a lower-third previews as a band, not a full screen), a
                 pinned cue choice overrides, a content look defers. The preview
                 shows how THIS screen actually looks live, not the program feed. -->
            <TemplateRender template={previewTemplate} content={$live ? $liveContent : PREVIEW} />
          </div>
          <p class="ch-prevnote r-mono">{previewNote}</p>
          <!-- THE SCREEN'S OWN LAST WORD, beside Relay's claim about it.
               `describeScreen` returns a note as well as a label, and until now
               this panel rendered only the label — so the one surface built to
               answer for a single screen dropped the half of the answer that
               says WHY. When Relay and the screen disagree (a verse is live and
               the screen says it is blank) the badge above reads `Not confirmed`
               and this line is what makes that word actionable. Live already
               renders the same string, from the same helper, so the two panes
               cannot describe one screen differently. Reuses `ch-prevnote`
               deliberately: same role, same muted mono line, no new CSS in a
               stylesheet six agents are editing this week. -->
          {#if selDescribe.note}
            <p class="ch-prevnote r-mono">{selDescribe.note}</p>
          {/if}

          <!-- NAME IS EDITABLE NOW, and the comment that used to sit here said why
               it was not: "there is no `rename_channel` anywhere in Relay … an
               input here would take an operator's typing and drop it, which is
               precisely the defect DECISIONS §69 closed seven of." That reasoning
               was right and the answer was to build the command, not to keep the
               field read-only: the name is the only handle anybody in the building
               has on a screen, and a church that hangs the seeded `Lobby screen`
               in the crèche had no way to say so.

               COMMITTED ON BLUR AND ON ENTER, never on every keystroke. A rename
               per character would be a write per character on the row four
               surfaces read, and would make every intermediate half-typed name a
               name the degraded banner could say out loud.

               `Escape` puts the old name back, and it is bound HERE with
               `stopPropagation`: rule 44, in the smallest possible form. Nothing
               on this desk mounts a dialog, so `shortcuts.js` still has `Esc` —
               and an operator abandoning a rename must not also clear the wall. A
               handler that returns early for every other key swallows nothing. -->
          <div class="r-lbl ch-flbl">Name</div>
          <input
            class="r-input ch-fin"
            aria-label="Name for {sel.name}"
            value={sel.name}
            disabled={!$capture.available}
            on:keydown={(e) => {
              if (e.key === 'Enter') { e.preventDefault(); e.currentTarget.blur(); return; }
              if (e.key !== 'Escape') return;
              e.stopPropagation();
              e.currentTarget.value = sel.name;
              e.currentTarget.blur();
            }}
            on:blur={(e) => rename(sel, e)} />

          <div class="r-lbl ch-flbl">Template</div>
          <select class="r-select ch-fin" value={sel.template_id ?? ''} on:change={(e) => assignTemplate(sel, e)} disabled={!$capture.available}>
            <option value="">Follow the content look</option>
            {#each $templates as t (t.id)}
              <option value={t.id}>{t.name}</option>
            {/each}
          </select>
          {#if sel.template_id == null}
            <p class="ch-finhint">
              This screen has no look of its own: each kind of content wears whatever the
              content look says. Right now —
              {#each CONTENT_KINDS as k, i}{i ? ' · ' : ' '}{k.label}: {lookName(k.key)}{/each}
            </p>
          {:else}
            <p class="ch-finhint">
              This screen's own look. It wins over a content look — only a cue that pins its
              own template overrides it (DECISIONS §29). To let the content looks decide here,
              choose <b>Follow the content look</b>.
            </p>
          {/if}

          <!-- ══ PER KIND ══ (DECISIONS §97)
               ONE CONTROL, IN THE INSPECTOR ONLY, AND CLOSED BY DEFAULT.
               The screen CARD does not gain this and must not: a card is 232px of
               picture and two words, and five choices do not go there. The card's
               `<select>` keeps its meaning exactly and becomes the screen's look
               for EVERYTHING ELSE.

               Four screens times five kinds is twenty choices only for a church
               that asks for twenty; for everyone else it is four choices and a
               closed line of text. The summary NAMES the kinds and never counts
               them — a count is a number an operator has to open the disclosure to
               act on.

               NOT a `<details>`: the summary has to state what is inside it while
               closed, and the whole row is the control, so a plain button with
               `aria-expanded` says what it does to a screen reader rather than
               relying on the element's own semantics to carry a sentence. -->
          <div class="r-lbl ch-flbl">Per kind</div>
          <div class="ch-fin ch-perkind">
            <button
              class="r-btn ghost sm ch-perkindhead"
              aria-expanded={perKindOpen}
              disabled={!$capture.available}
              on:click={() => (perKindOpen = !perKindOpen)}>
              <span class="ch-perkindsum">{perKindSummary}</span>
              <span class="ch-perkindmark" aria-hidden="true">{perKindOpen ? '−' : '+'}</span>
            </button>
            {#if perKindOpen}
              <div class="ch-perkindrows">
                {#each CONTENT_KINDS as k (k.key)}
                  <div class="ch-perkindrow">
                    <label class="r-lbl ch-perkindlbl" for="kindlook-{sel.id}-{k.key}">{k.label}</label>
                    <select
                      id="kindlook-{sel.id}-{k.key}"
                      class="r-select ch-perkindsel"
                      value={lookIdFor($channelLooks, sel.id, k.key) ?? ''}
                      on:change={(e) => pickKindLook(k.key, e)}
                      disabled={!$capture.available}>
                      <option value="">{inheritLabel(k.key)}</option>
                      {#each $templates as t (t.id)}
                        <option value={t.id}>{t.name}</option>
                      {/each}
                    </select>
                  </div>
                {/each}
                <p class="ch-finhint">
                  A look here changes what this screen <b>wears</b> for that kind, never
                  whether it <b>paints</b> it. Which kinds a screen shows at all is a
                  separate question, answered by the template.
                </p>
              </div>
            {/if}
          </div>

          <!-- ══ SHOWS ══ (DECISIONS §98)
               WHAT THIS SCREEN IS FOR, as against what its template CAN render.
               The two are ANDed at the output page and this half can only ever
               narrow: unticking a kind here can take the countdown off the wall,
               and ticking one can never make a lower third paint a countdown it
               has no regions for.

               Five checkboxes and not a picker, because this is a SET and an
               operator reading the panel has to see the four that are on as
               readily as the one that is off.

               It is not on the panic path and cannot be: `Clear screens` and
               `Blackout` address every screen and ask nothing about which. -->
          <div class="r-lbl ch-flbl">Shows</div>
          <div class="ch-fin ch-shows">
            {#each CONTENT_KINDS as k (k.key)}
              <label class="ch-showsrow">
                <input
                  type="checkbox"
                  checked={selShowsAll || selShows.includes(k.key)}
                  disabled={!$capture.available}
                  on:change={(e) => toggleShows(sel, k.key, e.currentTarget.checked)} />
                <span>{k.label}</span>
              </label>
            {/each}
          </div>
          {#if selShowsAll}
            <p class="ch-finhint">
              This screen has <b>no opinion</b>, so it shows whatever its template can
              render. Untick a kind to keep it off this screen — it never makes a
              template show something it has no room for, and it never affects
              <b>Clear screens</b> or <b>Blackout</b>.
            </p>
          {:else if selShows.length === 0}
            <p class="ch-finhint">
              This screen shows <b>nothing</b>. Its panic controls still reach it, and it
              still reports that it is painting.
            </p>
          {:else}
            <p class="ch-finhint">
              This screen shows only what is ticked, and only where its template can
              render it. Tick them all to go back to <b>no opinion</b>.
            </p>
          {/if}

          <!-- ROLE. A setting, not a guess: Live's programme pane used to decide
               which screen it was previewing from `render_target`, and a Stage
               Message is filtered on this at the receiving page. -->
          <div class="r-lbl ch-flbl">Role</div>
          <select class="r-select ch-fin" value={sel.role ?? ''} on:change={(e) => assignRole(sel, e)} disabled={!$capture.available}>
            <option value="">{NO_ROLE_LABEL}</option>
            {#each CHANNEL_ROLES as r (r.key)}
              <option value={r.key}>{r.label}</option>
            {/each}
          </select>
          {#if sel.role === 'main'}
            <p class="ch-finhint">
              The wall. Live's programme pane is a preview of THIS screen, through this
              screen's template. Only one screen may be the main screen.
            </p>
          {:else if sel.role === 'stage'}
            <p class="ch-finhint">
              A screen the platform reads, not the congregation. It is the only kind of
              screen a <b>Stage Message</b> is painted on, and several screens may be
              stages — a confidence monitor and a preacher's tablet, for instance.
            </p>
            <!-- ONLY FOR A STAGE, because only `stage.html` has zones. Rendered
                 inside the role branch rather than beside it, so the control
                 cannot be offered for a screen it would do nothing to. -->
            <label class="r-lbl" for="ch-stage-layout">Stage layout</label>
            <select
              id="ch-stage-layout"
              class="r-select ch-fin"
              value={sel.stage_layout_id ?? ''}
              on:change={(e) => assignStageLayout(sel, e)}
              disabled={!$capture.available}>
              <!-- THE DEFAULT IS A CHOICE, NOT AN ABSENCE. Naming it is what
                   tells an operator the screen is being set from the device and
                   not from here — the fact `Live.svelte` records as "not
                   available on this side of the room". -->
              <option value="">Whatever the device is set to</option>
              {#each stageLayouts as l (l.id)}
                <option value={l.id}>{l.name}</option>
              {/each}
            </select>
            <p class="ch-finhint">
              {#if sel.stage_layout_id}
                This screen shows what the layout says, and the Zones panel on the
                device is set from here.
              {:else}
                Whoever is holding this screen sets its zones, in its own Zones panel.
                Relay cannot see what they chose. Pick a layout to decide from here
                instead.
              {/if}
            </p>
          {:else}
            <p class="ch-finhint">
              A congregation screen with no special job. It is never shown a Stage
              Message, and Live's programme pane is not a preview of it.
            </p>
          {/if}

          <!-- ══ THIS SCREEN, RIGHT NOW ══ BESIDE THE PANIC CONTROLS, NEVER
               INSTEAD OF THEM. `Clear screens` and `Blackout` are in the dock,
               address every screen and ask nothing about which (rule 15,
               DECISIONS §20). These address THIS screen and leave the wall
               alone, which is the request that was impossible until now: take
               the lobby TV down for the sermon and leave the congregation's
               screen live.

               A screen stays down until it is put back, across every fire in
               between. That is what makes it worth having and it is also how an
               operator forgets, so the state is said out loud here and on every
               card's badge, and the way back is the button beside it. -->
          <div class="r-lbl ch-flbl">This screen</div>
          {#if downOf(sel)}
            <p class="ch-downnow">
              You took this screen down{downOf(sel) === 'black' ? ' (black)' : ''}. It shows
              nothing until you put it back, and firing a verse will not bring it up.
            </p>
            <div class="ch-downrow">
              <button class="r-btn primary sm" on:click={() => putBack(sel)} disabled={!$capture.available}>
                Put back in the wall
              </button>
            </div>
          {:else}
            <div class="ch-downrow">
              <button class="r-btn ghost sm" on:click={() => takeDown(sel)} disabled={!$capture.available}>
                Take this screen down
              </button>
              <button class="r-btn ghost sm" on:click={() => blackDown(sel)} disabled={!$capture.available}>
                Black this screen out
              </button>
            </div>
            <p class="ch-finhint">
              Only this screen. Every other screen keeps showing whatever is on the
              programme, and the panic controls are unchanged.
            </p>
          {/if}

          {#if isNative(sel)}
            <div class="r-lbl ch-flbl">Display</div>
            <select class="r-select ch-fin" value={sel.display_target ?? ''} on:change={(e) => assignDisplay(sel, e)} disabled={!$capture.available}>
              <option value="">Primary display</option>
              {#each monitors as m (m.index)}
                <option value={String(m.index)}>{m.name} · {m.width}×{m.height}{m.primary ? ' (primary)' : ''}</option>
              {/each}
              {#if missingDisplay(sel)}
                <option value={sel.display_target}>Display {missingDisplay(sel)} — not connected</option>
              {/if}
            </select>
            {#if missingDisplay(sel)}
              <p class="ch-downnow">
                This screen is set to open on a display that is not plugged in. Relay
                will not open it on a different one — plug that display back in, or
                choose another here.
              </p>
            {/if}
          {/if}

          <!-- Type · Transport · Output · URL · Reporting (docs/REBRAND.md §5),
               each a name and a value like every other row on the desk (§11). -->
          <div class="r-lbl ch-flbl">Screen info</div>
          <dl class="ch-info">
            <dt>Type</dt><dd>{kindOf(sel)}</dd>
            <dt>Transport</dt><dd>{transportOf(sel)}</dd>
            <dt>Output</dt><dd>{outputOf(sel, monitorOf(sel))}</dd>
            {#if isNdi(sel)}
              <dt>URL</dt><dd>—</dd>
            {:else}
              <dt>URL</dt><dd class="ch-addr">{selAddr}</dd>
            {/if}
            {#if !isNative(sel) && !isNdi(sel)}
              <!-- AN ABSENCE, NOT A ZERO (the rule `latency.rs` learned the hard
                   way, rule 31). The kiosk hub counts clients PER TEMPLATE ID:
                   `run_kiosk_server` registers a client only inside
                   `if let Some(id) = template_id`, and `main.rs` computes the
                   count as `c.template_id.map(|t| clients.count(t))`. A screen
                   that FOLLOWS THE CONTENT LOOK has no template id on either
                   side, so its count is structurally 0 — with OBS attached and
                   painting, the panel built to answer for one screen printed
                   `Clients 0`, which is the same thing it prints when nothing is
                   connected at all. Rule 35, in one integer.
                   Relay cannot count this, so it says so instead of guessing. The
                   question the operator actually wants is answered by Reporting
                   one row down, which is the screen's own word and does not go
                   through a template id at all. -->
              <dt>Clients</dt>
              <dd>
                {#if sel.template_id == null}
                  not counted<i class="ch-infonote">this screen follows the content look, and viewers are counted per template</i>
                {:else}
                  {selStatus?.clients ?? 0}
                {/if}
              </dd>
            {/if}
            <!-- REPORTING. The screen's own last word, kept separate from Relay's
                 — when the two disagree, that disagreement is the finding. The
                 word comes from `outputHealth.js` so it cannot drift from the
                 lamp above it, and `never` is deliberately not `no`: one is
                 "attached and has never once answered", the other is "nothing is
                 attached to ask", and they want different repairs. -->
            <dt>Reporting</dt>
            <dd>{selReport.word}{#if selReport.note}<i class="ch-infonote">{selReport.note}</i>{/if}</dd>
          </dl>

          <div class="r-lbl ch-flbl">Actions</div>
          <div class="ch-actions">
            {#if isNative(sel)}
              <!-- `screenSwitch` decides, not `online`. See the note beside
                   `selSwitch`: this ternary offered **Turn on** for a screen it
                   had not yet asked about. -->
              {#if selSwitch.action === 'off'}
                <button class="r-btn ghost sm" on:click={() => closeNative(sel)}>Turn off</button>
              {:else if selSwitch.action === 'on'}
                <button class="r-btn primary sm" on:click={() => openNative(sel)}
                  disabled={!$capture.available || needsDisplay(sel)}
                  title={needsDisplay(sel) ? 'Choose a display for this screen first, or it would open over this console (RG-188).' : 'Open this screen'}>Turn on</button>
              {:else}
                <span class="ch-fixed" title={selSwitch.why}>{selSwitch.label}</span>
              {/if}
            {:else if !isNdi(sel)}
              <button class="r-btn ghost sm" on:click={() => copyUrl(sel)}>{copyFailedId === sel.id ? COPY_FAILED : copiedId === sel.id ? 'Copied ✓' : 'Copy URL'}</button>
              <button class="r-btn ghost sm" on:click={() => showQr(sel)}>{qrOpen === sel.id && qrUrl === selAddr ? 'Hide QR' : 'Show QR'}</button>
            {/if}
            <button class="r-btn ghost sm ch-del" class:arm={delArm === sel.id} on:click={() => remove(sel)} disabled={!$capture.available}>
              {delArm === sel.id ? 'Click again to confirm' : 'Remove'}
            </button>
          </div>

          <!-- HOW DO I REACH THIS SCREEN? Always visible, never a click away: it
               is one line, and the question is asked by somebody standing at the
               desk holding a cable. See `reachOf` for why it is text rather than
               a disclosure or a link. -->
          <p class="ch-finhint ch-reach">{reachOf(sel)}</p>

          <!-- THE QR LIVES HERE NOW, and it had to move with the button.
               `showQr` sets `qrOpen`, and the only markup that rendered the code
               was inside the table's `{#each}` — so the inspector's own Show QR
               button set a flag that painted nothing once the table became a grid.
               A control whose result renders somewhere else is a control that
               stops working the moment that somewhere else changes shape. -->
          {#if qrError}<p class="ch-stage-warn" role="status">{qrError}</p>{/if}
          {#if qrOpen === sel.id && qrUrl === selAddr}
            <div class="ch-qr">
              <img class="ch-qr-img" src={qrData} alt="QR code to open {sel.name} output" width="240" height="240" />
              <div class="ch-qr-info">
                <div class="r-lbl">Scan on the other device</div>
                <div class="ch-qr-hint r-mono">Open Camera or a QR app and point it here. Same Wi-Fi required.</div>
              </div>
            </div>
          {/if}

          <!-- WHAT RELAY DOES NOT MEASURE.
               The reference puts a CHANNEL HEALTH panel here — bandwidth, dropped
               frames, uptime, latency, "Excellent". None of it exists: nothing in
               Relay times a delivery, counts a frame, or records a connect time.
               Inventing plausible numbers on a screen an operator uses to decide
               whether the projector is working would be the worst possible place
               to be decorative, so the panel states the limit instead. -->
          <div class="r-lbl ch-flbl">What this panel can tell you</div>
          <p class="rw-foot ch-nomargin">
            Relay reports whether an output window is open and how many clients are
            connected. It does <b>not</b> measure latency, bandwidth, frame rate or
            dropped frames — nothing in the pipeline times or counts delivery, so any
            such figure here would be invented. A screen that is <b>reporting</b> is
            one that says it is still painting, not one whose picture is good.
          </p>
        </div>
      {/if}
    </aside>

  {:else if view === 'stage'}
    <!-- THE LINK, THE QR AND THE LAYOUT, BESIDE THE MIRROR. The journey this
         replaces was Sharing → Stage layouts → Screens; this rail is all three,
         and every control on it calls the SAME function the old surface calls.
         Two doors onto one guarantee is only safe when it is literally one
         implementation, which is why nothing here is a second QR builder, a
         second address chooser or a second assign. -->
    <aside class="rw-pane rw-insp">
      <div class="rw-panehead"><h2 class="rw-panettl">Open it on the device</h2></div>
      <div class="rw-panebody pad">
        <label class="r-lbl" for="ch-mirror-device">Stage screen</label>
        <select id="ch-mirror-device" class="r-select ch-fin" value={stageRemote.channel?.id ?? ''}
          on:change={(e) => (selectedStageId = e.target.value ? Number(e.target.value) : null)}>
          {#if !stageRemote.channel}<option value="">Choose a stage screen</option>{/if}
          {#each stageScreens as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
        </select>
        {#if stageRemote.others.length}
          <p class="ch-stage-sub r-dim">
            Also a stage display: <b>{stageRemote.others.join(', ')}</b>. Each one has its
            own link.
          </p>
        {/if}

        {#if stageUrl}
          <p class="ch-stage-sub r-dim">
            Scan this on the phone or tablet, on the same Wi-Fi, or open
            <code class="r-mono">{stageUrl}</code>.
          </p>
          <div class="ch-stage-actions">
            <!-- THE SAME BUILDER (`showStageQr`), not a third one. This file
                 already carries two QR builders and a third would be a third
                 chance for one of them to photograph a loopback address, which
                 names the PHONE that scans it. -->
            <button class="r-btn primary sm" on:click={showStageQr} disabled={networkBusy}
              >{stageQrOpen && stageQrUrl === stageUrl ? 'Hide QR' : 'Show QR'}</button>
            <button class="r-btn ghost sm" on:click={copyStage} disabled={networkBusy}
              >{copyLabel(copiedStage, 'Copy link')}</button>
          </div>
          {#if stageQrError}<p class="ch-stage-warn" role="status">{stageQrError}</p>{/if}
          {#if stageQrOpen && stageQrUrl === stageUrl}
            <img class="ch-stage-qr" src={stageQr} alt="QR code to open the stage remote"
              width="240" height="240" />
          {/if}
        {:else if stageRemote.channel}
          <p class="ch-stage-warn" role="status">
            Relay could not find a local network address for the preacher's phone or tablet.
            Connect this computer and the device to the same local network, then pick an
            address in <b>Sharing</b>.
          </p>
        {:else}
          <!-- NO ADDRESS, AND A REASON — rule 35. A bare `stage.html` renders the
               reading perfectly well and silently never receives a Stage Message,
               so printing one would hand out a link that looks entirely correct. -->
          <p class="ch-stage-warn">
            No screen is set as a stage display, so there is no stage link to hand out.
            Set a screen's <b>Role</b> to <b>Stage display</b> in <b>Screens</b> and the
            address appears here.
          </p>
        {/if}

        {#if mirrorChannel}
          <div class="r-lbl ch-flbl">Stage layout</div>
          <!-- THE JOURNEY THIS SECTION REPLACES, WITHOUT LEAVING IT. The same
               command the Screens inspector's picker calls, on the same screen. -->
          <select id="ch-mirror-layout" class="r-select ch-fin"
            value={mirrorChannel.stage_layout_id ?? ''}
            on:change={assignMirrorLayout}
            disabled={!$capture.available}>
            <!-- THE DEFAULT IS A CHOICE, NOT AN ABSENCE — the same words the
                 Screens inspector uses, because it is the same setting. -->
            <option value="">Whatever the device is set to</option>
            {#each stageLayouts as l (l.id)}
              <option value={l.id}>{l.name}</option>
            {/each}
          </select>
          <div class="ch-stage-actions">
            <button class="r-btn ghost sm" on:click={editMirrorLayout} disabled={!$capture.available}
              >{mirrorLayout ? 'Edit this layout' : 'Make a layout'}</button>
          </div>
          <p class="ch-stage-sub r-dim">
            {#if mirrorLayout}
              The zones above are this layout's. Editing it changes what this screen
              shows straight away.
            {:else}
              Whoever is holding this screen sets its zones, in its own Zones panel.
              Relay cannot see what they chose. Pick a layout to decide from here instead.
            {/if}
          </p>
        {/if}

        <p class="rw-foot">
          Anyone on the same Wi-Fi who has the address can open it — Relay does not ask
          the device who it is (<b>DECISIONS §35</b>), so treat the link the way you
          would treat the Wi-Fi password.
        </p>
        <ErrorState {error} />
      </div>
    </aside>
  {:else if view === 'layouts'}
    <aside class="rw-pane rw-insp">
      <div class="rw-panehead">
        <h2 class="rw-panettl">{selLayout === 'new' ? 'New layout' : layoutSaved?.name ?? 'Stage layout'}</h2>
      </div>
      <div class="rw-panebody pad">
        {#if selLayout == null}
          <div class="ch-empty r-empty">
            Choose a layout on the left, or make one. A stage screen with no layout is
            set from the device itself.
          </div>
        {:else}
          <label class="r-lbl" for="ch-lname">Name</label>
          <input
            id="ch-lname"
            class="r-input"
            type="text"
            bind:value={layoutName}
            placeholder="Preacher"
            autocomplete="off"
            disabled={layoutBusy} />

          <div class="r-lbl ch-lzlbl">Shows</div>
          <div class="ch-lzones">
            {#each STAGE_ZONES as z (z.key)}
              <button
                class="r-btn ghost sm ch-lz"
                class:on={layoutZones[z.key]}
                aria-pressed={layoutZones[z.key]}
                disabled={layoutBusy}
                on:click={() => toggleLayoutZone(z.key)}>{z.label}</button>
            {/each}
          </div>

          <!-- UNSAVED IS SAID OUT LOUD. A zone toggle does not write through —
               it would change what a preacher is looking at on every tap while
               the operator was still deciding — so the operator has to be told
               there is something here that has not reached a screen yet. -->
          {#if layoutDirty}
            <p class="ch-stage-sub r-dim">Not saved yet. Nothing has changed on any screen.</p>
          {/if}

          {#if layoutWornBy.length}
            <p class="ch-stage-sub r-dim">
              Worn by <b>{layoutWornBy.join(', ')}</b>. Saving changes what
              {layoutWornBy.length === 1 ? 'it shows' : 'they show'} straight away.
            </p>
          {/if}

          <div class="ch-stage-actions">
            <button
              class="r-btn primary sm"
              on:click={saveLayout}
              disabled={layoutBusy || !$capture.available || !layoutName.trim()}>Save</button>
            {#if selLayout !== 'new'}
              <button
                class="r-btn ghost sm ch-del"
                class:arm={layoutDelArm === selLayout}
                on:click={removeLayout}
                disabled={layoutBusy || !$capture.available}>
                {layoutDelArm === selLayout ? 'Click again to confirm' : 'Delete'}
              </button>
            {/if}
          </div>
        {/if}
      </div>
    </aside>
  {:else if view === 'looks'}
    <aside class="rw-pane rw-insp">
      <div class="rw-panehead"><h2 class="rw-panettl">Screens that follow</h2></div>
      <div class="rw-panebody">
        {#if followers.length}
          {#each followers as c (c.id)}
            <div class="rw-nv">
              <span class="rw-nvk">{c.name}</span>
              <span class="rw-nvv">{kindOf(c)}</span>
            </div>
          {/each}
        {:else}
          <div class="ch-empty r-empty">
            No screen follows the content look. Every screen has a template of its own,
            so nothing on the left changes what a congregation sees.
          </div>
        {/if}
        <div class="ch-pad">
          <div class="r-lbl ch-flbl">Resolving right now</div>
          <dl class="ch-info">
            {#each CONTENT_KINDS as k (k.key)}
              <dt>{k.label}</dt><dd>{lookName(k.key)}</dd>
            {/each}
          </dl>
          <p class="rw-foot">
            To let a screen follow these, open it in <b>Screens</b> and set its template
            to <b>Follow the content look</b>.
          </p>
        </div>
      </div>
    </aside>

  {:else}
    <!-- Preacher's stage remote — the one output a church sets up by hand on
         somebody else's device every week, so it belongs on the rail you hand
         the phone from rather than in a tile at the bottom of a page. -->
    <aside class="rw-pane rw-insp">
      <div class="rw-panehead"><h2 class="rw-panettl">Preacher's stage remote</h2></div>
      <div class="rw-panebody pad">
        <label class="r-lbl" for="stage-device">Stage screen</label>
        <select id="stage-device" value={stageRemote.channel?.id ?? ''} on:change={(e) => selectedStageId = e.target.value ? Number(e.target.value) : null}>
          {#if !stageRemote.channel}<option value="">Choose a stage screen</option>{/if}
          {#each stageScreens as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
        </select>
        <label class="r-lbl" for="stage-network">Computer's network address</label>
        <select id="stage-network" value={lanIp} on:change={chooseAddress} disabled={networkBusy}>
          {#if !addresses.some((a) => a.address === lanIp)}
            <option value={lanIp}>{lanIp === 'localhost' ? 'No network address selected' : lanIp}</option>
          {/if}
          {#each addresses as a}<option value={a.address}>{a.interface}: {a.address}</option>{/each}
        </select>
        <button class="r-btn ghost sm" on:click={refreshNetwork} disabled={networkBusy}>{networkBusy ? 'Refreshing addresses…' : 'Refresh addresses'}</button>
        {#if networkError}<p class="ch-stage-warn" role="status">{networkError}</p>{/if}
        <p class="ch-stage-sub r-dim">Choose the network shared with the phone or tablet. An address alone does not prove the device can connect.</p>
        {#if stageUrl}
          <p class="ch-stage-sub r-dim">
            The live verse on a phone or iPad, updating in real time. Scan the QR (same
            Wi-Fi) or open <code class="r-mono">{stageUrl}</code>.
          </p>
          <!-- WHICH SCREEN THE LINK IS. The address carries a channel now, so it
               is an address FOR something and the operator is entitled to know
               which — and to know that the second stage screen is a different
               number rather than concluding the link is broken. -->
          <p class="ch-stage-sub r-dim">
            This is the link for <b>{stageRemote.channel.name}</b>.
          </p>
          <div class="ch-stage-actions">
            <button class="r-btn primary sm" on:click={showStageQr} disabled={networkBusy}>{stageQrOpen && stageQrUrl === stageUrl ? 'Hide QR' : 'Show QR'}</button>
            <button class="r-btn ghost sm" on:click={copyStage} disabled={networkBusy}>{copyLabel(copiedStage, 'Copy link')}</button>
          </div>
          {#if stageQrError}<p class="ch-stage-warn" role="status">{stageQrError}</p>{/if}
          {#if stageQrOpen && stageQrUrl === stageUrl}
            <img class="ch-stage-qr" src={stageQr} alt="QR code to open the stage remote" width="240" height="240" />
          {/if}
        {:else if stageRemote.channel}
          <p class="ch-stage-warn" role="status">
            Relay could not find a local network address for the preacher's phone or tablet.
            Connect this computer and the device to the same local network, then use Refresh addresses and choose an address above.
          </p>
        {:else if stageScreens.length}
          <p class="ch-stage-warn">The selected screen is no longer a stage display. Choose a stage screen above.</p>
        {:else}
          <!-- NO ADDRESS, AND A REASON — rule 35.
               A bare `stage.html` renders the reading, the countdown and the
               clock perfectly well, so printing it would hand the operator a link
               that looks entirely correct and silently never receives a Stage
               Message. The page itself says the same thing at the other end, so
               whichever half of the room notices first can act on it. -->
          <p class="ch-stage-warn">
            No screen is set as a stage display, so there is no stage link to hand out.
            A Stage Message is only ever painted on a screen whose <b>Role</b> is
            <b>Stage display</b> — set one in <b>Screens</b> and the address appears here.
          </p>
        {/if}
        <p class="rw-foot">
          Anyone on the same Wi-Fi who has the address can open it — Relay does not ask
          the device who it is (<b>DECISIONS §35</b>), so treat the link the way you
          would treat the Wi-Fi password.
        </p>
      </div>
    </aside>
  {/if}
</WorkspaceFrame>

<style>
  /* OUTPUTS — laid out in the shared workspace grammar (`WorkspaceFrame.svelte`,
     docs/REBRAND.md §2). Everything here is the part that is specific to screens;
     the columns, the panes, the type roles and the name/value row come from the
     frame so this workspace and the Planner cannot drift apart.

     What changed with the rebrand: the pill nav across the top became the rail —
     Content looks and Sharing were two solo pages wearing the Outputs tab, and a
     section that drops two of the three columns is a different workspace. Every
     section now keeps the rail and gets an inspector that answers the question
     that section actually raises. */

  /* ── rail ── */
  .ch-railfacts{ gap:0; padding:0; }
  /* Role two of the type scale, on the rail rather than above the grid. Quiet:
     it is the sentence you read once, not a heading you read every visit. */
  .ch-raillead{ margin:0; padding:9px 12px; border-bottom:1px solid var(--v-line);
    font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .ch-railfact{ display:flex; align-items:center; justify-content:space-between; gap:8px;
    padding:7px 12px; border-bottom:1px solid var(--v-line); }
  .ch-railfact:last-child{ border-bottom:0; }
  .ch-railk{ font-size:var(--v-fs-cap); color:var(--v-faint); }
  .ch-railbad{ color:var(--v-rose); }
  .ch-railv{ font-size:var(--v-fs-cap); color:var(--v-dim); font-variant-numeric:tabular-nums;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  /* ── the screens pane head ── */
  /* NO HEIGHT OVERRIDE (REBRAND §1: a component may override a shared control's
     width and padding, never its height). This was 40px against the frame's 34,
     so on the Screens section the middle column's bottom hairline sat six pixels
     below the two either side of it, across the whole desk. `min-height:34px`
     from `.rw-panehead` stands; wrapping still grows the box, because a
     min-height is a minimum. */
  .ch-panehead{ padding:0 12px; gap:10px; flex-wrap:wrap; }
  /* POSITION ONLY. The box — fill, edge, corner, height, the input's own
     borderlessness and the focus outline — is `.r-well` in app.css, reached
     through `ui/Field.svelte`. This rule used to draw all of it, and its focus
     treatment was one of four different ones across four search boxes. */
  :global(.ch-search){ flex:0 1 240px; min-width:140px; }
  /* What the grid IS, said once at the top. It drops out below the width where
     the head would otherwise wrap onto a second row and push the list down. */
  .ch-headnote{ font-size:var(--v-fs-cap); letter-spacing:.08em; text-transform:uppercase;
    color:var(--v-faint); }
  /* MEASURED, not guessed: at 1280 the pane head is ~686px and search + note +
     button is wider than that, so the button wrapped onto a second row and the
     grid lost 34px. The note is the half that can go — the cards say the same
     thing by being cards. */
  @media (max-width:1400px){ .ch-headnote{ display:none; } }
  .ch-search svg{ color:var(--v-faint); flex:0 0 auto; }
  .ch-search input{ flex:1; min-width:0; background:transparent; border:0; outline:none;
    color:var(--v-txt); font-size:var(--v-fs-b2); }
  .ch-search input::placeholder{ color:var(--v-faint); }

  .ch-addbar{ display:flex; gap:8px; align-items:center; flex:0 0 auto; padding:9px 12px;
    background:var(--v-bg); border-bottom:1px solid var(--v-accent-line); }
  .ch-addbar .r-input{ flex:1 1 200px; }

  /* Content that is prose rather than a row still needs a gutter; the pane body
     itself has none, because seamed rows must reach both edges. */
  .ch-pad{ padding:12px; }
  .ch-nomargin{ margin-top:0; }

  /* ── the screen cards ──
     The table's seven-column grid and its six responsive overrides are gone with
     it. A card is one column at any width; the GRID reflows instead, which is why
     this replaces roughly seventy lines of column bookkeeping with four. */
  .ch-gridwrap{ overflow-y:auto; padding:12px; }
  /* THE MIN TRACK IS MEASURED AGAINST THE RAIL, not copied from the prototype.
     The prototype's Outputs has no rail, so its main column is ~206px wider and
     `minmax(232px,1fr)` lands three across; with the rail the same number lands
     TWO at 1280 and an operator with five screens scrolls for the third.
       main column   = page − 28 (page pad) − 206 (rail) − 320 (inspector) − 16
       inner         = main − 24 (this pane's padding) − 2 (borders)
       columns       = floor((inner + 12) / (min + 12))
     At 200: 1280 → 3 · 1440 → 4 · 900 (no inspector) → 3. Checked at all three. */
  .ch-cards{ display:grid; grid-template-columns:repeat(auto-fill,minmax(200px,1fr));
    gap:12px; align-content:start; }

  .ch-card{ display:flex; flex-direction:column; gap:7px; padding:9px; min-width:0;
    cursor:pointer; text-align:left; background:var(--v-surf);
    border:1px solid var(--v-line); border-radius:var(--v-r-sm);
    transition:border-color .14s, background .14s; }
  .ch-card:hover{ border-color:var(--v-line2); }
  /* Steel blue = the thing you are working on. Never amber: amber means a
     congregation is looking at something, and selecting a card to configure it
     puts nothing anywhere. */
  .ch-card.sel{ border-color:var(--v-sel); background:var(--v-sel-soft); }
  /* A screen that is not answering, in the failure colour — on the CARD, so it is
     visible while the eye is on the picture rather than only in the lamp beside
     the name. `.down` wins over `.sel`, because a selected broken screen is still
     a broken screen. */
  .ch-card.down{ border-color:var(--v-rose); }

  /* The 16:9 frame. `position:relative` is load-bearing — TemplateRender's root is
     `position:absolute; inset:0`, so without it every card's preview escapes and
     lays itself out against the page. */
  .ch-frame{ position:relative; aspect-ratio:16/9; min-width:0; overflow:hidden;
    border:1px solid var(--v-line2); border-radius:var(--v-r-sm); background:var(--v-void); }
  /* A FRAME THAT IS OLD SAYS SO IN WORDS (RG-211). Ochre, because this is a
     caution and nothing else in the palette may be spent on one (rule 18,
     DECISIONS §111): it is not amber, which means the screen is on air, and it
     is not rose, because nothing has failed here that the badge has not already
     named. It sits over the picture rather than beside it, so it cannot be read
     as belonging to the card below. */
  .ch-stale{ position:absolute; left:6px; top:6px; z-index:2;
    padding:2px 6px; border-radius:var(--v-r-sm);
    font-size:var(--v-fs-lbl); line-height:var(--v-lh-lbl); letter-spacing:.04em;
    color:var(--v-caution); background:color-mix(in srgb, var(--v-void) 78%, transparent);
    border:1px solid color-mix(in srgb, var(--v-caution) 45%, transparent); }
  /* `.ch-plate` / `.ch-platelbl` moved into `ui/CameraPlate.svelte` with the
     markup they styled — a rule left behind here would be a rule nobody renders,
     and this file has already been caught by a `class:` directive naming a class
     no stylesheet defined. */

  .ch-cardtop{ display:flex; align-items:center; gap:8px; min-width:0; }
  .ch-cardname{ flex:1; min-width:0; font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* The lamp must never be squeezed to nothing by a long screen name: a run
     surface once rendered a failing screen's name seven pixels wide, and this is
     the same mistake turned the other way round. */
  .ch-lamp{ flex:0 0 auto; }
  .ch-cardmeta{ margin-top:-3px; font-size:var(--v-fs-cap); letter-spacing:.08em;
    text-transform:uppercase; color:var(--v-faint);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .ch-cardrow{ display:flex; align-items:center; gap:6px; min-width:0; }
  .ch-cardpick{ flex:1; min-width:0; }
  .ch-cardout{ flex:1; min-width:0; font-size:var(--v-fs-cap); color:var(--v-dim);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  /* A LAYOUT ROW. A name and what wears it, selectable — the same two-column
     shape as `.rw-nv` beside it, made pressable rather than redrawn, so a list
     of layouts reads as the desk's other lists do and not as a row of buttons. */
  .ch-lrow{ display:flex; width:100%; text-align:left; background:none; border:0;
    cursor:pointer; }
  .ch-lrow.on{ background:var(--v-sel); }
  /* A ZONE SWITCH. The shared button, pressed-state only — it is a toggle in a
     set rather than an action, so `on` is its whole visual job. */
  .ch-lz.on{ background:var(--v-sel); color:var(--v-txt); }
  .ch-lzlbl{ margin-top:14px; }
  .ch-lzones{ display:flex; flex-wrap:wrap; gap:6px; margin:6px 0 12px; }
  .ch-qr{ display:flex; flex-direction:column; align-items:flex-start; gap:14px; padding:12px 0 0; }
  .ch-qr-img{ border-radius:var(--v-r-sm); flex:0 0 auto; }
  .ch-qr-info{ flex:1; min-width:0; }
  .ch-qr-hint{ font-size:var(--v-fs-cap); color:var(--v-faint); }

  /* ── content looks ── */
  .ch-lookselect{ width:min(230px, 52vw); }

  /* ── sharing / stage remote ── */
  .ch-addr-row{ display:flex; align-items:center; gap:10px; min-width:0; }
  .ch-addr-row .ch-addr{ flex:1; min-width:0; }
  .ch-stage-sub{ margin:0 0 10px; font-size:var(--v-fs-b2); line-height:1.45; }
  .ch-stage-actions{ display:flex; gap:6px; flex-wrap:wrap; }
  .ch-downrow{ display:flex; gap:6px; flex-wrap:wrap; margin-bottom:6px; }
  /* A SCREEN THE OPERATOR TOOK DOWN. The caution ink, not rose: nothing has
     failed and nothing needs repairing — this is a decision somebody made, and
     rose here would send a volunteer hunting for a broken projector. Same
     reading as the badge `describeScreen` gives it. It was AMBER until
     2026-09-21 (RG-207): the tally light, on the box saying a screen is NOT on
     air, which is the one place that colour may never sit. */
  .ch-downnow{
    margin:0 0 8px; padding:8px 10px; border-radius:var(--r-sm, 6px);
    background:var(--v-caution-soft); border:1px solid var(--v-caution-line);
    color:var(--v-caution2); font-size:var(--v-fs-b2); line-height:1.45;
  }
  /* NO STAGE SCREEN. The caution ink, not red: nothing has failed, a screen
     simply has no role — a configuration answer, and red would send an operator
     looking for a fault. Same reading as the line `stage.html` shows at the
     other end. Never amber: a stage screen with no role is not on air. */
  .ch-stage-warn{
    margin:0; padding:10px 12px; border-radius:var(--r-sm, 6px);
    background:var(--v-caution-soft); border:1px solid var(--v-caution-line);
    color:var(--v-caution2); font-size:var(--v-fs-b2); line-height:1.45;
  }
  /* ── the preacher's screen ──
     NO LAW COLOUR ANYWHERE IN THIS BLOCK, and that is the point rather than an
     omission. Amber means ON AIR, cyan a guess, amethyst a rehearsal, rose a
     failure (rule 18, DECISIONS §22). Every fact here is honestly none of the
     four: a zone an operator switched on is not a claim that a congregation can
     see anything, and a zone Relay cannot see the state of is not a fault. The
     one thing this block does spend is CONTRAST — a row nobody can answer for is
     quieter than one that has an answer, so it reads as an absence rather than as
     a third setting. */
  .ch-mhead{ margin:0 0 8px; font-size:var(--v-fs-b1); line-height:1.45; color:var(--v-txt); }
  .ch-mreach{ margin:0 0 4px; font-size:var(--v-fs-cap); color:var(--v-faint); }
  .ch-mwarn{ color:var(--v-dim, var(--v-faint)); }
  .ch-mzones{ list-style:none; margin:0; padding:0; display:grid; gap:2px; }
  .ch-mz{ display:flex; align-items:baseline; gap:10px; min-width:0;
    padding:5px 0; border-bottom:1px solid var(--v-line2); }
  .ch-mzk{ flex:1; min-width:0; font-size:var(--v-fs-b2); color:var(--v-txt); }
  .ch-mzv{ flex:0 0 auto; font-size:var(--v-fs-cap); letter-spacing:.06em;
    text-transform:uppercase; color:var(--v-txt); }
  /* A ZONE NOBODY CAN ANSWER FOR reads as an absence, not as a third state with
     a colour of its own. `Hidden` is a decision and stays legible; `Not known` is
     the quietest thing on the panel, because it is the one row that is about
     Relay rather than about the screen. */
  .ch-mz.off .ch-mzk, .ch-mz.off .ch-mzv{ color:var(--v-faint); }
  .ch-mz.unk .ch-mzk, .ch-mz.unk .ch-mzv{ color:var(--v-faint); font-style:italic; }
  .ch-mread{ margin:0 0 6px; font-size:var(--v-fs-b2); line-height:1.45; color:var(--v-txt); }
  .ch-mcontent{ display:flex; flex-direction:column; gap:4px; padding:10px 12px;
    border-radius:var(--r-sm, 6px); border:1px solid var(--v-line2);
    background:transparent; }
  .ch-mref{ font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt); }
  /* THE VERSE IS CLAMPED, NEVER SCROLLED. This panel is a summary of a screen and
     a whole chapter in it would push the link, the QR and the layout picker off
     the bottom — the three controls the section exists for. */
  .ch-mtext{ font-size:var(--v-fs-b2); line-height:1.5; color:var(--v-faint);
    display:-webkit-box; -webkit-line-clamp:4; -webkit-box-orient:vertical; overflow:hidden; }
  .ch-mtimers{ list-style:none; margin:0 0 8px; padding:0; display:grid; gap:2px; }
  .ch-mtimer{ display:flex; align-items:baseline; gap:10px; min-width:0;
    padding:5px 0; border-bottom:1px solid var(--v-line2); }
  .ch-mtname{ flex:1; min-width:0; font-size:var(--v-fs-b2); color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* THE FIGURE IS NEVER CLIPPED. `formatCountdown` emits seven characters past an
     hour and what is left of a clipped clock reads as a valid time (RG-147). */
  .ch-mtv{ flex:0 0 auto; font-variant-numeric:tabular-nums; font-size:var(--v-fs-b2);
    color:var(--v-txt); }
  .ch-mtheld{ flex:0 0 auto; font-size:var(--v-fs-cap); letter-spacing:.06em;
    text-transform:uppercase; color:var(--v-faint); }
  #ch-mirror-device, #ch-mirror-layout{ display:block; width:100%; margin:6px 0 12px; }
  .ch-stage-qr{ display:block; max-width:100%; height:auto; margin-top:12px; border-radius:var(--v-r-sm); }
  .ch-qr-img { max-width:100%; height:auto; }
  #stage-device, #stage-network { display:block; width:100%; margin:6px 0 12px; }

  /* ── inspector ── */
  .ch-inspttl{ flex:1; text-transform:none; letter-spacing:var(--v-tr-h2);
    font-size:var(--v-fs-h3); line-height:var(--v-lh-h3); }
  /* THE SHARED ICON BUTTON, at the shared size. This forced 22×22 over
     `.r-iconbtn`'s 26 — a companion class quietly redrawing the control it was
     attached to, which is the `.wide` defect in miniature and invisible from
     either rule alone. It sits in a 34px `.rw-panehead`, so there was never a
     space problem to solve. All that is left is the flex, which is layout.
     It was not alone: `.br-fav` draws 24 and `.lib-more` 30, so four of this
     product's icon buttons were four different sizes. Those two are in the
     Library half and belong to whoever holds it. */
  .ch-close{ flex:0 0 auto; }

  /* position:relative is load-bearing — TemplateRender's root is
     position:absolute; inset:0, so without it the preview escapes this box and
     lays itself out against the page. It supplies its own container-type. */
  .ch-preview{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-sm);
    border:1px solid var(--v-line2); overflow:hidden; background:var(--v-void); }
  .ch-prevnote{ margin:6px 0 0; font-size:var(--v-fs-cap); color:var(--v-faint); }

  .ch-flbl{ margin:15px 0 6px; }
  /* A value where the prototype has an input — see the markup for why. It is
     styled as a value, not as a disabled field: a greyed-out box invites clicking
     and then says nothing. */
  .ch-fixed{ margin:0; font-size:var(--v-fs-b2); color:var(--v-txt); overflow-wrap:anywhere; }
  /* The evidence under the Reporting word, on its own line so a long one cannot
     push the word it is evidence for off the row. */
  .ch-infonote{ display:block; font-style:normal; font-size:var(--v-fs-cap); color:var(--v-faint); }
  .ch-fin{ width:100%; }
  .ch-finhint{ margin:6px 0 0; font-size:var(--v-fs-cap); line-height:1.45; color:var(--v-faint); }
  /* PER-KIND LOOKS (DECISIONS §97). Configuration, so it borrows the desk's own
     surfaces and introduces no colour of its own — amber means ON AIR, cyan means
     a guess and amethyst means a rehearsal, and none of the three is ever allowed
     to describe a setting. */
  .ch-perkind{ display:block; }
  .ch-perkindhead{ display:flex; align-items:center; justify-content:space-between;
    gap:8px; width:100%; text-align:left; }
  .ch-perkindsum{ overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .ch-perkindmark{ flex:0 0 auto; opacity:.7; font-variant-numeric:tabular-nums; }
  .ch-perkindrows{ margin-top:8px; display:flex; flex-direction:column; gap:6px; }
  .ch-perkindrow{ display:grid; grid-template-columns:minmax(0,1fr) minmax(0,1.4fr);
    align-items:center; gap:8px; }
  /* The label is allowed to shrink and the picker is not: a template name cut in
     half is a screen wearing a look nobody can read back. */
  .ch-perkindlbl{ min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .ch-perkindsel{ min-width:0; width:100%; }
  /* WHAT THIS SCREEN SHOWS (DECISIONS §98). A set, laid out as a set: the four
     that are on have to read as readily as the one that is off. */
  .ch-shows{ display:flex; flex-wrap:wrap; gap:4px 14px; }
  .ch-showsrow{ display:flex; align-items:center; gap:6px; min-width:0;
    font-size:var(--v-fs-cap); color:var(--v-txt); cursor:pointer; }
  .ch-showsrow input{ flex:0 0 auto; }
  /* Sits under Actions, so it needs the gap the actions row does not provide. */
  .ch-reach{ margin-top:10px; }

  /* A name and a VALUE (§11) — one hairline per fact, the value on the right
     edge so a column of them can be read down rather than hunted through. */
  .ch-info{ display:grid; grid-template-columns:auto minmax(0,1fr); gap:0 12px; margin:0;
    font-size:var(--v-fs-b2); }
  .ch-info dt{ color:var(--v-faint); padding:6px 0; border-bottom:1px solid var(--v-line); }
  .ch-info dd{ margin:0; color:var(--v-txt); text-align:right; overflow-wrap:anywhere;
    padding:6px 0; border-bottom:1px solid var(--v-line); }
  /* One line, truncated. Wrapping "anywhere" broke it mid-word into
     `output.h / tml?` — an address split across a line break invites being
     mis-typed, and Copy URL is right there for the real thing. */
  .ch-addr{ font-family:var(--f-mono); font-size:var(--v-fs-cap); color:var(--v-accent2);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .ch-actions{ display:flex; flex-wrap:wrap; gap:6px; }
  .ch-actions .r-btn{ flex:1 1 auto; justify-content:center; }
  .ch-del{ color:var(--v-rose); }
  .ch-del:hover:not(:disabled), .ch-del.arm{ border-color:var(--v-rose); background:var(--v-rose-soft); }

  .ch-empty{ margin:auto; padding:20px 14px; text-align:center; line-height:1.5; }

  /* A CREATE TARGET, not a button. A dashed full-width card at the end of the
     screens list, where the eye lands after reading it — the shape says "this
     is where a new one goes" before the words do. `.r-btn` would make it one
     more control at the bottom of a column of screen rows and lose exactly
     that — a dashed edge is the only thing here that means "empty". */
  .ch-addcard{ display:flex; flex-direction:column; align-items:center; gap:5px;
    width:calc(100% - 24px); margin:12px; padding:16px; cursor:pointer;
    background:transparent; border:1px dashed var(--v-line2); border-radius:var(--v-r-sm);
    color:inherit; transition:.14s; }
  .ch-addcard:hover:not(:disabled){ border-color:var(--v-accent); background:var(--v-accent-soft); }
  .ch-addcard:disabled{ opacity:.45; cursor:not-allowed; }
  .ch-addmark{ width:24px; height:24px; border-radius:50%; display:grid; place-items:center;
    background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-accent2);
    font-size:var(--v-fs-h2); line-height:1; }
  .ch-addttl{ font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt); }
  .ch-addsub{ font-size:var(--v-fs-cap); color:var(--v-faint); text-align:center; line-height:1.5; }
</style>
