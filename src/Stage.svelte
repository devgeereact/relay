<script>
  import { formatCountdown, countdownWarning, formatElapsed } from './lib/layers.js';
  import { countdownRemainingMs, countdownIsPaused, countdownTotalMs } from './lib/countdown.js';
  // Mobile stage-display remote — the preacher opens this on a phone/iPad (via
  // QR or the LAN URL) to see the live verse + reference in real time. No Tauri
  // runtime: it connects to the kiosk WebSocket hub (:8031) for content, exactly
  // like an OBS/kiosk output, but rendered as a readable mobile confidence view.
  import { onMount, onDestroy } from 'svelte';

  let content = null;
  let visible = false;
  let note = ''; // operator's confidence-monitor note for the live cue
  // A WORD TO THE PREACHER. Takes the whole screen until the operator clears it.
  // It lives here, in the stage renderer, which is what makes "no congregation
  // screen can show it" a property of the system rather than a promise: the
  // output page has an explicit `false` verdict for this message kind
  // (r6-contracts.test.js).
  let alert = '';
  let next = null; // { label, text } — the "up next" preview
  let connected = false;
  let ws = null;
  let closed = false;
  let clock = '';
  let timer;

  // Preacher control plane — the phone can DRIVE the wall, not just mirror it.
  // Hits the LAN HTTP API on :8031's sibling port (:8032/api/*), which runs the
  // SAME fire/nav path the console does. LAN-only, no auth (see channels.rs).
  let showCtl = false;
  let q = '';
  let results = [];
  let searching = false;
  let busy = false; // a nav/fire request is in flight
  let ctlErr = '';
  const API = `http://${location.hostname || 'localhost'}:8032/api`;

  // Anything that CHANGES the wall goes by POST, and the backend refuses it as a
  // GET (405). That is what stops `<img src=".../api/black">` on any page anyone on
  // the church network happens to open from blacking out the congregation's wall —
  // an image, a script, a stylesheet and a link can only ever issue GET.
  // DECISIONS §35. `search` and `live` mutate nothing and stay GET.
  const MUTATES = new Set(['fire', 'next', 'prev', 'clear', 'black']);

  async function api(path) {
    const route = path.split('?')[0];
    const method = MUTATES.has(route) ? 'POST' : 'GET';
    const r = await fetch(`${API}/${path}`, { method });
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    const j = await r.json();
    if (!j.ok) throw new Error(j.error || 'failed');
    return j;
  }

  let searchSeq = 0;
  async function doSearch() {
    const term = q.trim();
    if (!term) { results = []; return; }
    const seq = ++searchSeq;
    searching = true;
    ctlErr = '';
    try {
      const j = await api(`search?q=${encodeURIComponent(term)}`);
      if (seq === searchSeq) results = j.results || [];
    } catch (e) {
      if (seq === searchSeq) { results = []; ctlErr = 'Search failed — check the connection.'; }
    } finally {
      if (seq === searchSeq) searching = false;
    }
  }

  async function fire(reference) {
    if (busy) return;
    busy = true; ctlErr = '';
    try {
      await api(`fire?ref=${encodeURIComponent(reference)}`);
      results = []; q = '';
    } catch (e) {
      ctlErr = 'Could not put that on screen.';
    } finally { busy = false; }
  }

  // Not every outcome is a failure, and the preacher is entitled to know WHICH.
  // The end of a reading is a correct boundary; a verse missing from the library
  // is a real fault. Only `fired` moved the wall.
  const NAV_SAID = {
    end_of_passage: 'End of the reading.',
    no_passage: 'Nothing on screen yet — tap a verse first.',
    not_in_library: 'That verse is not in the library.',
  };

  async function nav(dir) {
    if (busy) return;
    busy = true; ctlErr = '';
    try {
      // The backend answers `ok: true` for every outcome it handled — including
      // the ones where NOTHING MOVED. So the catch below is not enough on its own:
      // it only ever fires on a transport failure, which meant tapping Next at the
      // end of a reading did nothing, said nothing, and left the preacher tapping.
      const j = await api(dir); // 'next' | 'prev'
      if (j.nav && j.nav.kind !== 'fired') {
        ctlErr = NAV_SAID[j.nav.kind] ?? (dir === 'next' ? 'No next verse.' : 'No previous verse.');
      }
    } catch (e) {
      ctlErr = dir === 'next' ? 'No next verse.' : 'No previous verse.';
    } finally { busy = false; }
  }

  // ── ZONES (docs/REBRAND.md §5) ─────────────────────────────────────────────
  //
  // A stage monitor is not a congregation screen in other colours. Six things a
  // preacher might want, each switchable, and the figures either BESIDE the
  // reading or ACROSS THE BOTTOM.
  //
  // THE SWITCHES REMOVE THINGS. THEY ARE NOT HOW THINGS ARRIVE.
  //
  // §5 said "clean by default: reading, countdown and clock; the rest is switched
  // on", and three of the six shipped OFF — `next`, `note` and `elapsed`. Every
  // one of those three is something an OPERATOR deliberately produced FOR THE
  // PREACHER and has no other audience:
  //
  //   · `note`    — a line typed against a cue in the Planner (`stage_note`),
  //                 which no congregation template renders.
  //   · `next`    — the up-next the operator published (`channels::stage_next`),
  //                 stage-only by contract.
  //   · `elapsed` — the service clock, which only exists while a service is
  //                 actually recording.
  //
  // So the operator typed a word to the preacher, the console showed it had gone,
  // and the preacher's screen showed nothing — because of a switch on a device the
  // operator cannot see, which nobody had been told to find. Nothing anywhere
  // reports that. That is rule 35's shape on the one screen whose reader cannot
  // glance at the console to find out what happened, and it is what the operator
  // meant by "should be ACTIVE".
  //
  // CLEAN BY DEFAULT SURVIVES, and that is the reason this is safe rather than a
  // busier screen: four of the six render NOTHING unless something exists to
  // render. No note typed, no `.noterow`. No up-next published, no `.next`. No
  // service recording, no elapsed figure. A stage screen with nothing sent to it
  // still shows the reading, the countdown and the clock and nothing else — which
  // is exactly the state §5's sentence was describing.
  //
  // The switches are still the point, and they still go the other way: a lobby TV
  // that should NOT carry the preacher's note switches it off, once, on that
  // device — which is why this is stored per device, in `localStorage`, and not in
  // Relay's database. Two stage screens in one building are allowed to want
  // different things, and the console must not have to know about either.
  //
  // Every read and write is guarded: a private window, blocked site data or a
  // kiosk with storage disabled must give the DEFAULT layout, never a blank page.
  const ZONES = [
    { key: 'reading', label: 'Reading' },
    { key: 'next', label: 'Next' },
    { key: 'note', label: 'Note' },
    { key: 'countdown', label: 'Countdown' },
    { key: 'clock', label: 'Clock' },
    { key: 'elapsed', label: 'Service elapsed' },
  ];
  const DEFAULT_ZONES = {
    reading: true,
    next: true,
    note: true,
    countdown: true,
    clock: true,
    elapsed: true,
  };
  const ZONE_KEY = 'relay.stage.zones';
  let zones = { ...DEFAULT_ZONES };
  let figures = 'bottom'; // 'bottom' | 'beside'
  let showZones = false;

  function loadZones() {
    try {
      const raw = localStorage.getItem(ZONE_KEY);
      if (!raw) return;
      const saved = JSON.parse(raw);
      if (!saved || typeof saved !== 'object') return;
      // Key by key, off the DEFAULTS — so a zone added in a later version is on
      // its own default rather than absent, and a corrupt value cannot delete one.
      for (const z of ZONES) if (typeof saved[z.key] === 'boolean') zones[z.key] = saved[z.key];
      if (saved.figures === 'beside' || saved.figures === 'bottom') figures = saved.figures;
    } catch {
      /* defaults stand */
    }
  }
  function saveZones() {
    try {
      localStorage.setItem(ZONE_KEY, JSON.stringify({ ...zones, figures }));
    } catch {
      /* the layout still applies to this session */
    }
  }
  function toggleZone(key) {
    zones = { ...zones, [key]: !zones[key] };
    saveZones();
  }
  function setFigures(v) {
    figures = v;
    saveZones();
  }

  // Countdown mirror — ticked by the same 1s timer as the wall clock.
  let cdTo = null;
  let cdFrom = null;
  let cdPaused = null;
  let cdDone = '';
  let svcStart = null; // service-start epoch, for the elapsed zone
  let nowMs = 0;
  // ONE READER, shared with the wall and the console (docs/REBRAND.md §7). This was
  // its own subtraction, which was fine while the answer was a subtraction — and is
  // not, now that it has an exception. A preacher's own screen counting down through
  // a countdown the operator has HELD is the surface it matters most on.
  $: cdContent = { countdown_to: cdTo, countdown_from: cdFrom, countdown_paused_ms: cdPaused };
  $: cdRemain = countdownRemainingMs(cdContent, nowMs);
  $: cdFinished = cdRemain === 0 && !countdownIsPaused(cdContent);
  // ONE FORMATTER, shared with the wall (docs/REBRAND.md §7) — this page used to
  // carry its own copy of the same arithmetic.
  $: cdText = cdRemain == null ? '' : formatCountdown(cdRemain);
  // The span now genuinely rides with the content (`countdown_from`), so the
  // short-countdown half of the warning rule finally has an answer here too.
  $: cdWarn = cdRemain != null && countdownWarning(cdRemain, countdownTotalMs(cdContent));
  // THREE STACKED PAIRS, and no second piece of arithmetic. `hms` is the one
  // formatter's own `H:MM:SS`, split into its fields and the hours padded — so the
  // rail cannot drift from the figure beneath the reading, or from the wall.
  //
  // A FINISHED countdown is one row, not three zeroes: the operator's done message
  // is words, and "00 / 00 / 00" stacked down a rail says the clock is still
  // running. `formatCountdown` clamps at zero, so the pairs would be truthful and
  // useless.
  $: cdPairs =
    cdRemain == null || cdFinished
      ? []
      : formatCountdown(cdRemain, 'hms')
          .split(':')
          .map((p, i) => (i === 0 ? p.padStart(2, '0') : p));

  // ── THE PROGRAMME ───────────────────────────────────────────────────────────
  //
  // `Stage`-scoped timers, as the hub last sent them. A whole SET every time, never
  // a delta: a tablet that missed one frame would otherwise be wrong about the
  // programme for the rest of the service with no way to find out, and an empty
  // list is how the last clock comes OFF this screen.
  //
  // They live outside `content` on purpose, which is the whole of wave 3: a timer
  // has a lifetime of its own, so a verse, a song or a notice replacing the live
  // content cannot forget it — and neither can a panic control, which takes the
  // congregation's timers and leaves the preacher's.
  let stageTimers = [];
  // ONE READER AND ONE FORMATTER, the same two the wall and the console use
  // (docs/REBRAND.md §7). Not a third subtraction: a held timer whose exception one
  // surface has never heard of counts down on that surface while the others hold,
  // and this is the surface somebody is reading from mid-sermon.
  //
  // A row whose figure cannot be read AT ALL is dropped rather than shown empty —
  // `countdownRemainingMs` answers null when there is no countdown in the entry, and
  // a rail row with no digits in it says nothing a preacher can act on.
  //
  // AND ONE WARNING RULE. `warn_ms` rides in every timer frame and for a while had
  // no reader here at all — the comment that used to sit on this line said the
  // threshold was wave 3 Track D's and that this page did not read it on purpose,
  // which stopped being true the day Track D landed (RG-148). The surface whose
  // entire purpose is telling a preacher how long is left was the one with no
  // signal that the time was nearly gone, while the congregation's screen had one.
  //
  // `layers.js::countdownWarning` is still the ONLY reading of when to worry, and
  // the third argument is the override it already takes: a threshold somebody CHOSE
  // for this timer beats the shared rule, and absent one the shared rule applies —
  // the last minute, or the last tenth of a countdown shorter than ten minutes.
  // That default is the shipped 60 s here rather than whatever the operator set in
  // Settings, because the setting does not reach this bundle at all (RG-149, which
  // is a different row and not this page's to fix).
  $: programme = stageTimers
    .map((t) => ({ t, id: t?.id, label: (t?.label || '').trim(), ms: countdownRemainingMs(t, nowMs) }))
    .filter((r) => r.ms != null)
    .map(({ t, ...r }) => ({
      ...r,
      v: formatCountdown(r.ms),
      warn: countdownWarning(r.ms, countdownTotalMs(t), t?.warn_ms),
    }));
  // THE ROW IS SIZED FROM THE TEXT IT IS ACTUALLY PAINTING.
  //
  // `.tval` budgeted a flat SIX characters and `formatCountdown` emits seven once a
  // timer passes an hour, so a 95-minute clock painted 215.3 px into a 199 px
  // `overflow: hidden` box at 1280 x 720 and read as `1:30:1`, running into its
  // neighbour's `0:00` with no gap (RG-147). What is left of a clipped clock reads
  // as a valid time, which is the part that matters: a preacher glancing down
  // mid-sermon cannot tell it from a correct one.
  //
  // The widest value decides for every row, so the figures stay one size and the
  // longest of them still cannot be clipped. Same instrument as `figCh` across the
  // bottom, not a new one. Floored at four, the width of `0:00`.
  $: progCh = programme.reduce((n, r) => Math.max(n, r.v.length), 4);

  // SERVICE ELAPSED — counts up from the epoch the fired content carries. There is
  // no epoch when no service is recording, and an absence is shown as an absence:
  // a zero here would say "this service just started", which is a different claim.
  $: elapsedText = svcStart != null ? formatElapsed(nowMs - svcStart) : '';

  // THE COUNTDOWN IS A FIGURE, ALWAYS — it is never a mode the reading disappears
  // into. The first draft of this gave the countdown the reading's room whenever
  // the content had no verse text, which is EVERY countdown: `countdown_to` only
  // ever rides on a `countdown` cue (`main::start_countdown`), never beside a
  // verse. So the rail's stacked pairs — the thing §5 actually asks for — could
  // not be reached from any state Relay can be in. A zone nothing can render is a
  // zone nobody is looking at.
  $: figureList = [
    ...(zones.countdown && cdRemain != null ? ['countdown'] : []),
    ...(zones.clock ? ['clock'] : []),
    ...(zones.elapsed && elapsedText ? ['elapsed'] : []),
  ];
  // EVERY FIGURE ON THE RAIL SAYS WHAT IT IS.
  //
  // ProPresenter's stage display puts each element in its own region under a
  // small upper-case label, and the reason is not decoration: a platform monitor
  // is read in one glance, from ten metres, by somebody mid-sentence. The figure
  // row ACROSS THE BOTTOM already did this — COUNTDOWN · TIME · ELAPSED — and the
  // rail BESIDE THE READING did not, so the same two facts were labelled in one
  // layout and bare in the other. Driven in a browser with a countdown running,
  // the rail read
  //
  //     00 · 03 · 42 · 12:01 AM · 45:00
  //
  // five rows of identical white mono, and nothing on the screen said which was
  // the countdown, which was the wall clock and which was the service. `45:00`
  // could as easily have been a countdown as an elapsed time; the preacher's only
  // way to tell was to watch which direction it moved.
  //
  // So the rail is built here, row by row, as {label, value} — which also puts
  // the two layouts on ONE list instead of two `{#if}` ladders that could drift.
  // The pairs are labelled from the END, so a formatter that ever returned MM:SS
  // rather than H:MM:SS still labels the minutes as minutes.
  const PAIR_KEYS = ['Hrs', 'Min', 'Sec'];
  $: railList = figureList.flatMap((f) => {
    if (f === 'countdown') {
      return cdFinished
        ? [{ k: 'Countdown', v: cdDone || '0:00', done: true }]
        : cdPairs.map((p, i) => ({
            k: PAIR_KEYS[PAIR_KEYS.length - cdPairs.length + i] ?? '',
            v: p,
            warn: cdWarn,
          }));
    }
    if (f === 'clock') return [{ k: 'Time', v: clock }];
    return [{ k: 'Elapsed', v: elapsedText }];
  });
  // How many ROWS the beside-rail holds: a running countdown is three of them, a
  // finished one is a single line of words. It is the list's own length now, so a
  // row added to the list cannot be missed by a second piece of counting.
  $: railRows = railList.length;
  $: beside = figures === 'beside' && figureList.length > 0 && zones.reading;
  // Has the reading anything of its own to fill the screen with? A countdown cue
  // carries a LABEL and no body, and a pre-service countdown on a phone is the one
  // thing that page is being looked at for — so the figures take the room the
  // reading is not using. Still a flex BASIS, still clipped.
  $: readingHasBody = !!(visible && content?.text);
  // …AND "THE READING HAS NO BODY" IS NOT THE SAME CLAIM AS "A COUNTDOWN IS
  // RUNNING", WHICH IS THE ONE THE EXCEPTION WAS WRITTEN FOR.
  //
  // `.figrow.tall` was keyed on `!readingHasBody` alone and justified by "a
  // pre-service countdown is the whole reason anyone is looking at this page".
  // The commonest state with no body is not a countdown — it is STANDBY, before
  // anything has been fired at all. Driven at 1920×1080 with nothing on screen,
  // the wall clock took 58% of a platform monitor at 361px while "— standby —"
  // sat above it at 34px: the time of day, four times the size of the only words
  // on the screen, because of an exception meant for a countdown that was not
  // running. The condition now says what the comment always said.
  //
  // The rail is the same exception facing sideways, and it did not have it at
  // all: a countdown BESIDE a bodiless reading left 74% of the screen black and
  // squeezed the figures into a quarter. One flag, both layouts — the twin door
  // this repository keeps finding a guarantee missing from.
  $: figuresTakeTheRoom = !readingHasBody && figureList.includes('countdown');
  // ACROSS THE BOTTOM, EVERY FIGURE IS ONE SIZE.
  //
  // `--ch` was per-figure, so each one filled its own cell — and side by side on
  // one baseline that is three type sizes pretending to be a row. It only shows
  // when the row is tall enough that the height bound stops binding, which is
  // exactly the case nobody checks: on a 1080×1920 portrait panel TIME rendered
  // at 94px beside ELAPSED at 150px. The widest value decides for all of them, so
  // the row is uniform and the longest figure still cannot be clipped.
  //
  // THE RAIL IS DELIBERATELY NOT DOING THIS. Its rows are stacked, where a size
  // difference reads as emphasis rather than as raggedness, and §5 asks for the
  // countdown's pairs to FILL the rail — which sizing them for an eight-character
  // clock two rows down would quietly undo.
  $: figCh = figureList
    .map((f) => (f === 'countdown' ? (cdFinished ? cdDone || '0:00' : cdText) : f === 'clock' ? clock : elapsedText))
    .reduce((n, v) => Math.max(n, v.length || 5), 5);

  // HOW MANY CHARACTERS THE READING HAS, handed to the stylesheet so the verse can
  // be sized to the room instead of to a fixed ceiling. docs/REBRAND.md §3.4 —
  // "measured, not tabled", and a stage reading is one of the nested contexts it
  // names. The arithmetic is in the CSS beside the box it is about; this is just
  // the one number CSS cannot count for itself.
  $: verseChars = content?.text ? content.text.length : 60;

  // HOW WIDE THE COLUMN SHOULD BE, which is the other half of the same question.
  //
  // The fit below takes the SMALLER of a width bound and a height bound, and the
  // measure decides both: a narrow column makes the line short (raising the width
  // bound) and the passage tall (lowering the height bound). One fixed measure is
  // therefore wrong at one end or the other, and 22 was wrong at the long end —
  // **Esther 8:9 (530 characters) rendered at the 26px floor on a 1920×1080
  // platform monitor while the time of day beneath it was 84px.** Scripture a third
  // the size of the clock, on the screen the preacher reads from.
  //
  // The two bounds are equal at the measure that balances them. Writing k for the
  // width constant and h for the height constant, `h·v² − k·v − k·n = 0`, so
  //
  //     v = (R + √(R² + 4·R·n)) / 2,      R = k/h = 2.923 · (width / height)
  //
  // of the reading area. R is a CONSTANT here, not a measurement: the aspect it
  // stands for is the shape of the region, the function is a square root and so is
  // forgiving of being handed the wrong one, and measuring the box would mean a
  // forced layout on the one page whose job is to be still. 6.5 is the 16:9 case
  // with the figure row on — the shape this page has on almost every screen it is
  // opened on. A taller region just gets a slightly narrower column than its own
  // optimum, which is the old behaviour, not a new failure.
  //
  // Clamped to 16…64: the lower end is the portrait default and the upper end is
  // the top of the 45–75 character measure that is comfortable to read at all.
  // Rounded, because `1ch` times a fraction is a sub-pixel column.
  //
  // Checked against the real backend at 1920×1080: Psalms 23:1 (64 characters)
  // asks for 24 and renders at 153px; Esther 8:9 asks for 62 and renders at 59px.
  // Both were 139px and 26px under the fixed 22.
  $: verseCpl = Math.min(
    64,
    Math.max(16, Math.round((6.5 + Math.sqrt(6.5 * 6.5 + 4 * 6.5 * verseChars)) / 2)),
  );

  // A WORD TO THE PREACHER, SIZED TO ITS LENGTH.
  //
  // §5 fixes the type at 8.5cqw and the panel at `overflow: hidden`, which is the
  // right pair for the message §5 describes ("Wrap up — 5 minutes"). It is the
  // wrong pair for the message an operator actually types when something has gone
  // wrong, which is a sentence or three — those ran past the bottom of the screen
  // and were CLIPPED, silently, on the one surface in the product whose whole
  // purpose is that a person reads every word of it while facing a congregation.
  //
  // Four steps rather than a continuous fit: the fit this page can afford has no
  // measurement in it (there is no renderer here and no `TemplateRender` to borrow
  // — this page draws its own chrome), so a formula would be a guess with a
  // decimal point on it. Steps are a guess that cannot produce a pathological
  // size, and the first one is §5's own figure, unchanged, for §5's own case.
  const ALERT_STEPS = [
    { max: 24, size: 'xl' }, // a phrase — §5's 8.5cqw
    { max: 64, size: 'lg' },
    { max: 150, size: 'md' },
  ];
  $: alertSize = ALERT_STEPS.find((s) => alert.length <= s.max)?.size ?? 'sm';

  function apply(m) {
    if (m.kind === 'content') {
      content = { reference: m.reference, text: m.text, translation: m.translation };
      note = m.stage_note || '';
      cdTo = m.countdown_to || null;
      cdFrom = m.countdown_from || null;
      cdPaused = m.countdown_paused_ms ?? null;
      cdDone = m.countdown_done || '';
      svcStart = m.service_started_at ?? null;
      nowMs = Date.now();
      visible = true;
    } else if (m.kind === 'clear' || m.kind === 'black') {
      // `black` HAS to be here, and it was not.
      //
      // The hub publishes four kinds and this page handled three. `Output.svelte`
      // honours `black`; this one did not — so the operator hit `B`, the
      // congregation's wall went dark, and the screen the preacher is READING FROM
      // kept the verse. The console reported success, correctly: the message did
      // leave the machine. Nobody was told a screen had ignored it.
      //
      // Blanking on `black` rather than ignoring it is the conservative reading of
      // a genuine ambiguity, and the ambiguity is worth stating because the other
      // answer is defensible. A stage monitor faces the PREACHER, not the
      // congregation, so one could argue a blackout — whose purpose is "the
      // congregation must see nothing" — should leave it alone. But `clear`
      // already blanks this page, and **the harsher control must never do less
      // than the milder one**. An operator who has just hit the emergency key
      // cannot be asked to remember that it reaches three screens out of four.
      //
      // If Relay ever decides the stage monitor should survive a panic, it must
      // survive BOTH controls, deliberately, in both branches — not by one of them
      // being forgotten.
      visible = false;
      note = '';
      cdTo = null;
      // Both halves of the countdown go, or a HELD figure survives the control that
      // removed the countdown and the rail keeps showing it: `countdown_paused_ms`
      // is read ahead of the instant, so clearing the instant alone would not be
      // enough. The same trap as `black` above, one field along.
      cdFrom = null;
      cdPaused = null;
      next = null;
      // A WORD TO THE PREACHER COMES DOWN WITH THE SCREENS — DECISIONS §89.
      //
      // This line is the answer to a question that used to be left unasked. The
      // five fields above were reset and `alert` was not, and nothing anywhere
      // recorded why — a third answer, given by nobody, to exactly the question
      // §89 exists to settle.
      //
      // `.alert` is `position: fixed; inset: 0` — it IS the screen, not a figure
      // on it. So without this line an operator pressed `B`, whose entire meaning
      // is *every output goes opaque black*, and the preacher's tablet stayed a
      // full-bleed pulsing red panel: the brightest thing in the room, under a
      // control the console had just reported succeeding. That is the failure the
      // comment above is about, one field further along again.
      //
      // And the two halves of the room disagreed. `stage_alert` is deliberately
      // NOT a retained frame (rule 43 — a private word must not arrive again
      // later), so a tablet that reloaded or dropped off the wifi came back with
      // no alert while the one beside it that stayed connected kept the panel.
      // Clearing here is what makes the live path agree with the reconnect path.
      //
      // `svcStart` deliberately SURVIVES, and so does a programme timer (§89). A
      // cleared or blacked wall is not the end of a service, and the elapsed zone
      // is the preacher's own clock — taking it away when the operator hits Esc
      // would answer a question nobody asked. The line §89 draws is between a
      // thing that COUNTS and a thing that SAYS something: a panic control takes
      // back every sentence anybody put on a screen, and stops none of the clocks.
      alert = '';
    } else if (m.kind === 'stage_alert') {
      // `text: null` (or empty) clears it. An alert is an instruction, not a
      // state of the wall, so nothing here is retained or restored on reconnect —
      // and a panic control takes it down with everything else it says (§89).
      alert = (m.text || '').trim();
    } else if (m.kind === 'stage_next') {
      next = m.label || m.text ? { label: m.label || '', text: m.text || '' } : null;
    } else if (m.kind === 'timer') {
      // THE WHOLE SET, OR NOTHING. A frame whose `timers` is missing or is not a
      // list is read as an empty programme rather than thrown on: this page has no
      // backend and cannot verify who is on the other end of its socket
      // (docs/SECURITY.md T4), and one throw inside `apply` would kill every frame
      // after it — the reading included — for the rest of the service.
      //
      // Deliberately NOT cleared by `clear` or `black` above. That is the
      // operator's decision this wave carries: the congregation's timers go with
      // the congregation's screens and the preacher's programme stays, because the
      // programme is not something a congregation was ever looking at. The backend
      // half is the registry's (`stop_scope(Both)`); this half holds because the
      // rows are rendered outside the `visible` gate rather than because anything
      // remembered to re-send them.
      stageTimers = Array.isArray(m.timers) ? m.timers : [];
    }
  }

  // How many times the socket has failed since it was last up.
  //
  // "connecting…" reads the same at two seconds and at ten minutes, and the
  // preacher holding the phone cannot tell a page that is about to work from one
  // that never will. After a few failed attempts it says so plainly instead.
  let attempts = 0;
  $: reach = connected ? 'live' : attempts > 3 ? "can't reach Relay — retrying" : 'connecting…';

  function connect(host) {
    if (closed) return;
    try {
      ws = new WebSocket(`ws://${host}:8031`);
      ws.onopen = () => {
        connected = true;
        attempts = 0;
        // ── RULE 43, ON THE ONE SCREEN THAT IS CARRIED AROUND ─────────────────
        //
        // `KioskHub` retains the last `content` / `clear` / `black` frame and
        // replays it to a screen that joins mid-service — and it replays it in
        // exactly ONE place, inside `run_kiosk_server`'s `hello` handler. A
        // client that never says hello is never sent what it missed.
        //
        // This page did not say hello. It opened the socket and waited, so the
        // preacher's phone locking its screen, dropping off the wifi for a
        // moment, or simply being reloaded came back BLANK and stayed blank
        // until the next fire — in the middle of the reading it exists to
        // carry. That is RG-129's failure on the screen whose reader cannot
        // glance at the console to find out what happened, and it is the
        // "guarantee kept on one door" shape again: the rule, the retained
        // frame and the hub test were all real, and all of them were about a
        // client that says hello.
        //
        // NO `template_id`, deliberately. The stage monitor is not a render
        // target of a congregation template — it draws its own zones — so it
        // has no template to be registered or counted against, and the hub's
        // registration branch is keyed on that id. The themes, the transition
        // and the retained frame are sent regardless, because they are about
        // what is ON THE SCREENS rather than which look this screen wears.
        //
        // Nothing private replays: `stage_alert` and `stage_next` are NOT
        // retained frames (`channels::tests::FRAME_VERDICTS` holds both at
        // `false`), so a word meant for the preacher cannot arrive again later,
        // and a rehearsal publishes nothing to this hub at all.
        try {
          ws.send(JSON.stringify({ kind: 'hello' }));
        } catch {
          /* onclose retries; a failed hello must never take the page down */
        }
      };
      ws.onmessage = (e) => {
        try { apply(JSON.parse(e.data)); } catch { /* ignore */ }
      };
      ws.onclose = () => {
        connected = false;
        attempts += 1;
        if (!closed) setTimeout(() => connect(host), 1500);
      };
      ws.onerror = () => { try { ws.close(); } catch { /* onclose retries */ } };
    } catch {
      attempts += 1;
      if (!closed) setTimeout(() => connect(host), 1500);
    }
  }

  onMount(() => {
    loadZones();
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
    <span class="status" class:on={connected}><i></i>{reach}</span>
    <button class="ctl-toggle" class:active={showZones} on:click={() => (showZones = !showZones)} aria-label="Choose what this screen shows">
      Zones
    </button>
    <button class="ctl-toggle" class:active={showCtl} on:click={() => (showCtl = !showCtl)} aria-label="Control panel">
      {showCtl ? 'Done' : 'Control'}
    </button>
  </header>

  {#if alert}
    <!-- THE WHOLE SCREEN. A preacher reads this from a platform, mid-sentence,
         without looking for it. Outside the zone layout on purpose: an
         instruction that a switched-off zone could hide is not an instruction. -->
    <div class="alert {alertSize}" role="status" aria-live="assertive">{alert}</div>
  {/if}

  <!-- ══ ZONES ══ NOTHING MAY LEAVE THE SCREEN (docs/REBRAND.md §5).
       The reading takes what is LEFT (`flex: 1 1 0`); the rows and the rail are
       the only fixed sizes, and they are `flex-basis`, never `height` — a height
       is a floor a long passage pushes past, which is how a clock leaves the top
       of a monitor nobody is standing next to. -->
  <!-- A ZONE THAT IS SWITCHED OFF GIVES UP ITS ROOM.
       `main.stage` holds the reading and the rail, and `beside` already requires
       the reading zone — so with Reading off it was an EMPTY `flex: 1 1 0`
       competing with `.figrow.only`, which is the same. The two split the screen
       and a stage monitor showing only a clock gave half of itself to a region
       with nothing in it. Rendered at 1920×1080 with Reading off: 480px of black
       above the figures. -->
  {#if zones.reading}
  <main class="stage" class:beside>
    <section class="reading" aria-label="Reading">
      {#if visible && content}
        {#if content.reference}<div class="ref">{content.reference}{content.translation ? ' · ' + content.translation : ''}</div>{/if}
        {#if content.text}<div class="verse" style="--vn:{verseChars}; --vcpl:{verseCpl}">{#if content.reference}“{content.text}”{:else}{content.text}{/if}</div>{/if}
      {:else}
        <div class="idle">— standby —</div>
      {/if}
    </section>

    {#if beside}
      <!-- THE RAIL IS ITS OWN CONTAINER (`container-type: size`), so every figure
           in it is a share of the RAIL and not of the frame. A figure sized
           against the frame is the bug this replaces: it looked right at one rail
           width and overflowed at every other. `--rows` is what keeps the stack
           inside its own height however many zones are switched on. -->
      <aside class="rail" class:wide={figuresTakeTheRoom} style="--rows:{railRows}" aria-label="Figures">
        {#each railList as r, i (i)}
          <div class="railrow" class:done={r.done} class:warn={r.warn} style="--ch:{r.v.length || 5}">
            <span class="figk">{r.k}</span>
            <span class="figv">{r.v}</span>
          </div>
        {/each}
      </aside>
    {/if}
  </main>
  {/if}

  {#if !beside && figureList.length}
    <!-- ACROSS THE BOTTOM. Also its own container, for the same reason. -->
    <div class="figrow" class:tall={figuresTakeTheRoom} class:only={!zones.reading}
      style="--figs:{figureList.length}; --ch:{figCh}" aria-label="Figures">
      {#each figureList as f (f)}
        {@const v = f === 'countdown' ? (cdFinished ? cdDone || '0:00' : cdText) : f === 'clock' ? clock : elapsedText}
        <div class="fig" class:warn={f === 'countdown' && cdWarn}>
          <span class="figk">{f === 'countdown' ? 'Countdown' : f === 'clock' ? 'Time' : 'Elapsed'}</span>
          <span class="figv">{v}</span>
        </div>
      {/each}
    </div>
  {/if}

  <!-- ══ THE PROGRAMME ══ One row per stage timer, and no row at all when there are
       none — the same rule as the note row and the up-next above it: nothing sent,
       nothing rendered, no room taken (docs/REBRAND.md §5).
       Deliberately OUTSIDE `{#if zones.reading}` and outside the `visible` gate: a
       programme timer outlives the content that replaced it, and it outlives a
       panic control aimed at the congregation's screens.
       NOT amber, which means ON AIR and is never allowed to lie; not cyan, which
       means the AI is guessing; not amethyst, which means rehearsal. Slate, the
       page's own neutral — the programme is the operator's bookkeeping shown to one
       person, and it makes no claim about any screen.
       A timer inside its warning window is the countdown's own red, which is the
       fourth colour this page already uses for exactly this rule and is none of the
       three above. It is a claim about TIME, not about a screen. -->
  {#if programme.length}
    <div class="progrow" style="--tmrs:{programme.length}; --tch:{progCh}" aria-label="Programme">
      {#each programme as t (t.id)}
        <div class="tmr" class:warn={t.warn} data-timer-id={t.id}>
          {#if t.label}<span class="tlabel">{t.label}</span>{/if}
          <span class="tval">{t.v}</span>
        </div>
      {/each}
    </div>
  {/if}

  {#if zones.note && note}
    <div class="noterow"><span class="note-lbl">Note</span><span class="notetxt">{note}</span></div>
  {/if}

  {#if showZones}
    <section class="zonepanel" aria-label="Zones">
      <div class="zonegrid">
        {#each ZONES as z (z.key)}
          <button class="zonebtn" class:on={zones[z.key]} aria-pressed={zones[z.key]} on:click={() => toggleZone(z.key)}>
            {z.label}
          </button>
        {/each}
      </div>
      <div class="zonegrid">
        <button class="zonebtn" class:on={figures === 'bottom'} aria-pressed={figures === 'bottom'} on:click={() => setFigures('bottom')}>
          Figures across the bottom
        </button>
        <button class="zonebtn" class:on={figures === 'beside'} aria-pressed={figures === 'beside'} on:click={() => setFigures('beside')}>
          Figures beside the reading
        </button>
      </div>
      <p class="zonefoot">Kept on this device only. Nothing here changes any other screen.</p>
    </section>
  {/if}
  {#if showCtl}
    <section class="ctl">
      <div class="nav-row">
        <button class="nav-btn" on:click={() => nav('prev')} disabled={busy}>‹ Prev</button>
        <button class="nav-btn" on:click={() => nav('next')} disabled={busy}>Next ›</button>
      </div>
      <form class="search" on:submit|preventDefault={doSearch}>
        <input
          type="search"
          inputmode="search"
          enterkeyhint="search"
          placeholder="Search a verse — “John 3:16” or “shepherd”"
          aria-label="Search for a verse to put on the screens"
          bind:value={q}
          on:input={doSearch}
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false" />
        <button type="submit" class="go" disabled={searching}>{searching ? '…' : 'Go'}</button>
      </form>
      {#if ctlErr}<div class="ctl-err">{ctlErr}</div>{/if}
      {#if results.length}
        <ul class="results">
          {#each results as r}
            <li>
              <button class="result" on:click={() => fire(r.reference)} disabled={busy}>
                <span class="r-ref">{r.reference}</span>
                <span class="r-text">{r.text}</span>
              </button>
            </li>
          {/each}
        </ul>
      {:else if q.trim() && !searching}
        <div class="no-results">No matches.</div>
      {/if}
    </section>
  {/if}
  {#if zones.next && next}
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
  .status { margin-left: auto; display: inline-flex; align-items: center; gap: 6px; font-family: var(--f-mono); font-size:var(--v-fs-mono); color: var(--v-faint); }
  .status.on { color: var(--v-emerald); }
  .status i { width: 7px; height: 7px; border-radius: 50%; background: currentColor; }
  .status.on i { box-shadow: 0 0 8px currentColor; animation: p 1.7s ease-in-out infinite; }
  @keyframes p { 0%, 100% { opacity: 1; } 50% { opacity: .4; } }
  /* NOTHING LEAVES THE SCREEN (docs/REBRAND.md §5). The reading takes what is
     left and scrolls INSIDE itself, so the header — the connection state — and
     every row beneath cannot be pushed off by a long passage.

     The spec says clip. The READING scrolls instead, deliberately: on a platform
     monitor the two are the same because the reading is sized to fit, and on the
     preacher's phone, which is the other thing this page is, clipping would take
     the end of a passage away from the person reading it aloud. Everything else
     — the rail, the figure row, the note, the up-next — is clipped as the spec
     asks, because those are fixed-size rows and a fixed row that overflows is
     just a row nobody sized. */
  main.stage { flex: 1 1 0; display: flex; flex-direction: row; min-height: 0; min-width: 0; }
  /* THE READING IS ITS OWN CONTAINER, for the same reason the rail is: the type in
     it is a share of the ROOM THE READING ACTUALLY HAS, not of the frame. With the
     figures across the bottom the reading is the frame minus a fifth; beside them
     it is the frame minus a quarter of its width. A verse sized against the frame
     is right in one of those layouts and wrong in the other, and the zones are
     switchable, so both happen on the same device. */
  .reading { flex: 1 1 0; display: flex; flex-direction: column; align-items: center; justify-content: center;
    text-align: center; padding: 24px; gap: 18px; min-height: 0; min-width: 0;
    container-type: size;
    overflow: auto; overscroll-behavior: contain; }
  /* THE RAIL IS ITS OWN CONTAINER. `size`, not `inline-size`, so the stack can be
     a share of the rail's HEIGHT as well — which is what stops three stacked
     pairs from running off the bottom when the countdown passes an hour. Its
     basis is a share of the frame and is a BASIS, never a height. */
  .rail { flex: 0 0 26%; max-width: 26%; min-width: 0; min-height: 0; overflow: hidden;
    container-type: size; display: flex; flex-direction: column;
    border-left: 1px solid rgba(255,255,255,.08); background: rgba(255,255,255,.02); }
  /* THE SAME EXCEPTION `.figrow.tall` MAKES, FACING SIDEWAYS. A countdown with no
     verse beneath it left three quarters of a platform monitor black and put the
     figures the room is actually watching into a quarter of the width. Still a
     BASIS, still clipped — the rail is its own container, so every figure in it
     simply re-reads the wider rail. */
  .rail.wide { flex-basis: 52%; max-width: 52%; }
  /* A FIGURE ON THIS RAIL WEARS NO PROMISE COLOUR. The clock, the elapsed time
     and the countdown were all `--v-amber`, and amber on this console means ON
     AIR and nothing else (rule 18, DECISIONS §21, `colourlaw.test.js`). Driven
     against the real backend, `stage.html` rendered "— standby —" with an amber
     clock beside it: the ON AIR colour, at the largest size on the page, over a
     page with nothing on air. It is not a template's saved default and no
     operator chose it — this page renders no template at all, so the colour was
     a stylesheet literal and nothing else.
     NOT the prototype's `--stg-mc` cyan either: cyan on this console promises A
     GUESS, and swapping one promise for another is the same defect in a
     different hue. A clock is a fact about time, so it takes the page's own ink.
     `--v-txt` is also BRIGHTER than amber on `--v-void`, so the figure a
     preacher reads from a platform did not get quieter. */
  .railrow { flex: 1 1 0; min-height: 0; overflow: hidden;
    /* LABEL ON TOP, FIGURE BENEATH — the region shape the bottom row already had.
       `auto auto` + `align-content: center`, not `auto 1fr`: a rail with one zone
       switched on is a row a thousand pixels tall, and a figure centred in what is
       left of that leaves its own label stranded at the ceiling. The label belongs
       to the figure, so the two are centred together as one block. */
    display: grid; grid-template-rows: auto auto; align-content: center;
    justify-items: center; gap: 2px;
    font-family: var(--f-mono); font-variant-numeric: tabular-nums; font-weight: 700;
    color: var(--v-txt); line-height: 1; letter-spacing: .01em;
    /* A share of the rail in BOTH axes: wide enough to fill it, never taller than
       its own share of the stack. `--rows` counts the rows actually switched on and
       `--ch` how many characters this row holds.
       `--ch` IS LOAD-BEARING, and leaving it out was a real defect measured in a
       browser: `62cqw` fills a rail with a TWO-character pair and puts a clock
       ("02:14 AM", eight characters) at 205px in a 333px rail — clipped to about a
       character and a half, silently, because the row is `overflow: hidden`. The
       0.62 advance is the mono figure docs/REBRAND.md §3.4 already measured.
       The height share dropped from 78% to 58% when the label arrived above it:
       a row is the label plus the figure now, and a figure still sized for the
       whole row would push its own label off the top of a box that clips. */
    font-size: min(
      calc(88cqw / (var(--ch, 2) * 0.62)),
      calc(58cqh / var(--rows, 3))
    ); }
  .railrow + .railrow { border-top: 1px solid rgba(255,255,255,.06); }
  .railrow.warn { color: var(--v-red); }
  /* A finished countdown is the operator's own words, not a figure — prose, at a
     size that still fits the rail it is a share of. */
  .railrow.done { font-family: var(--f-body); letter-spacing: 0; line-height: 1.15;
    padding: 0 6cqw; text-align: center;
    font-size: min(16cqw, calc(52cqh / var(--rows, 1))); }
  /* ACROSS THE BOTTOM — a fixed BASIS, clipped, never a height.
     A FIFTH OF A PLATFORM MONITOR FOR A WALL CLOCK IS NOT A HIERARCHY.
     ProPresenter's stage display has one rule above every other: the current
     slide dominates and everything else is visibly subordinate. This did the
     opposite, and it was measurable rather than a matter of taste — at 1920×1080
     with the clock and the service timer on, the READING settled at 76px and the
     time of day rendered at 110px. At 1024×768 it was 40px of scripture under
     62px of clock. The preacher's own screen said the loudest thing in the room
     was what o'clock it was.
     15% gives the reading 54px back at 1080 and leaves the clock at a size no
     platform has ever struggled with (the figure is bounded below). */
  .figrow { flex: 0 0 15%; min-height: 0; overflow: hidden; container-type: size;
    display: flex; border-top: 1px solid rgba(255,255,255,.08); background: rgba(255,255,255,.02); }
  /* A pre-service countdown is the whole reason anyone is looking at this page, and
     a countdown cue has a label and no body. The figures take the room the reading
     is not using — a different BASIS, never a height, and still clipped. */
  .figrow.tall { flex-basis: 58%; }
  .figrow.only { flex: 1 1 0; }
  /* A GRID, NOT A CENTRED COLUMN. Each figure used to be sized to its OWN
     character count, so the three blocks were three different heights and their
     labels landed 647px, 706px and 671px down a 1080px screen — three labels on
     three lines pretending to be a row. Two things put them back on one line:
     every figure in the row is now one size (`figCh`), and the label row is the
     same height in every cell because the label is the same size everywhere. */
  .fig { flex: 1 1 0; min-width: 0; min-height: 0; overflow: hidden;
    display: grid; grid-template-rows: auto auto; align-content: center;
    justify-items: center; gap: 2px; }
  .fig + .fig { border-left: 1px solid rgba(255,255,255,.06); }
  /* ── ONE LABEL, EVERY REGION ────────────────────────────────────────────────
     ProPresenter's stage display is a set of labelled regions, and what makes it
     read as ONE instrument rather than five widgets is that every label is the
     same label: same size, same weight, same tracking, same case.
     Relay had three treatments and all three were CONSOLE pixels on a PLATFORM
     monitor — `.figk` at `--v-fs-fig` (9px), `.note-lbl` at 9px, `.next-lbl` at a
     hardcoded 10px — on a page where the reference, the verse, the note, the
     up-next and every figure are all sized to the room. Photographed at
     1920×1080, TIME and ELAPSED were hairlines: legible on the phone this page is
     also for, invisible from the platform it is mostly for.
     A LABEL IS DELIBERATELY NOT A SHARE OF ITS REGION. Every other size on this
     page is a container unit, and that is right for content — a figure should
     fill the box it is in. A label is not content; it is the same small word in
     every box, and the rail (a quarter of the width) and the figure row (all of
     it) would give the same word two wildly different sizes. `vmin` is the frame,
     which is what "the same everywhere" means here, floored at the console's own
     figure token so a phone still gets the size the phone was designed at. */
  .figk, .note-lbl, .next-lbl {
    font-family: var(--f-mono); font-weight: 700; letter-spacing: .16em;
    text-transform: uppercase; line-height: 1.1;
    font-size: clamp(var(--v-fs-fig), 1.9vmin, 24px); }
  .figk { color: var(--v-faint); }
  /* The bottom row is the same three figures in the other layout. Same rule. */
  .fig .figv { font-family: var(--f-mono); font-variant-numeric: tabular-nums; font-weight: 700;
    color: var(--v-txt); line-height: 1;
    /* Same rule as the rail: the width a figure may take is its share of the row
       divided by the characters it actually has.
       Height share 58% → 52%: the row is shorter now (see `.figrow`) and it
       carries a label that has to fit above the figure rather than beside it. */
    font-size: min(
      calc(92cqw / var(--figs, 1) / (var(--ch, 5) * 0.62)),
      52cqh
    ); }
  .fig.warn .figv { color: var(--v-red); }
  @media (prefers-reduced-motion: no-preference) {
    .fig.warn .figv, .railrow.warn { animation: cdwarn 2s ease-in-out infinite; }
    .tmr.warn .tval { animation: cdwarn 2s ease-in-out infinite; }
  }
  /* `inline-size`, not `size`: the row's WIDTH is definite (it is the frame) and
     its height is what its content asks for under a ceiling. `container-type: size`
     here would take the content out of the height calculation and collapse the row
     to nothing — the rail can use `size` because its height is a flex basis. */
  .noterow { flex: 0 0 auto; flex-basis: auto; max-height: 22%; overflow: hidden;
    container-type: inline-size;
    display: flex; align-items: baseline; gap: 10px; padding: 10px 18px;
    border-top: 1px solid rgba(255,176,0,.24); background: rgba(255,176,0,.08); color: var(--v-amber2);
    /* The operator's own words to the preacher. `2.6vw` capped at 20px is a phone
       size on a platform monitor, on the row whose whole purpose is that somebody
       standing ten feet away reads it. */
    font-family: var(--f-body); font-size: clamp(14px, 2.2cqw, 34px); line-height: 1.3; }
  .notetxt { min-width: 0; overflow: hidden; }
  /* THE PROGRAMME ROW. `flex: 0 0 auto` with `flex-basis: auto`, like `.noterow`:
     it takes what its content needs and never competes with the reading, which is
     the zone that must keep the room (§5 — nothing may leave the screen).
     Its own container, so the digits are a share of THIS row and not of the frame —
     the same rule the rail and the figure row each keep, and the bug that rule
     replaces is a figure that looked right at one width and overflowed at every
     other. `--tmrs` divides the row by the number of timers actually in it, and
     `--tch` is how many characters its widest figure has.

     `--progmax` IS THE ROW'S CEILING AND THE DIGITS' CAP, STATED ONCE. It used to
     be two figures that could not agree: `max-height: 20%` here and `9cqh` on the
     digits. `container-type: inline-size` establishes an INLINE-axis container
     only, so `cqh` inside it has no eligible container and falls back, silently, to
     the small viewport — measured at 1920 x 500 with three timers, 9% of the row
     would be 6.70 px, 9% of the viewport is 45.00 px, and the digits came out at
     45.00 px (RG-154). It clipped nothing, because a cap that tracks the viewport
     shrinks with it; the defect is that the cap was not the cap anybody wrote, so
     nothing bounded the digits against the row if the row's own height ever
     changed. `container-type: size` here is not the fix: it would take the content
     out of the height calculation and collapse the row, which is the same reason
     `.noterow` above is `inline-size`.
     So the cap is a share of the CEILING instead, in `dvh` — which resolves, and
     which is this page's own frame (`.sr` is `100dvh`). 45% of 20dvh is 9dvh: the
     same number that was being computed by accident, now computed on purpose and
     tied to the ceiling it is a share of. On a mobile browser with a collapsing
     toolbar it tracks the frame the row is in rather than the smallest viewport
     that frame might become. */
  .progrow { flex: 0 0 auto; flex-basis: auto; --progmax: 20dvh; max-height: var(--progmax);
    overflow: hidden;
    container-type: inline-size;
    display: flex; gap: 10px; padding: 8px 18px;
    border-top: 1px solid rgba(255,255,255,.1); background: rgba(255,255,255,.035); }
  /* `min-width: 0` on the item, or a long label refuses to shrink and pushes the
     last timer off the end of a screen nobody is standing next to. */
  .tmr { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; gap: 2px; overflow: hidden; }
  /* A LABEL-LESS TIMER IS DIGITS ALONE. The label element is not rendered at all
     rather than rendered empty, so the row closes up instead of leaving a gap the
     height of a word — wave 5 Track G makes label-less the dock's default and this
     page has to survive it already. */
  .tlabel { font-family: var(--f-mono); font-weight: 700; letter-spacing: .16em;
    text-transform: uppercase; line-height: 1.1; color: var(--v-faint);
    font-size: clamp(var(--v-fs-fig), 1.9vmin, 24px);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tval { font-family: var(--f-mono); font-variant-numeric: tabular-nums; font-weight: 700;
    color: var(--v-txt); line-height: 1;
    /* The width a figure may take is its share of the row divided by the characters
       IT ACTUALLY HAS — `0.62` is the mono advance. That divisor was the constant
       six for as long as this row existed, and `formatCountdown` emits seven past
       an hour, so `1:30:13` was 12.9% wider than its box at every width below about
       `259 x timers` px: 215.3 px into 199 px at 1280 x 720, sliced through the
       last digit, reading `1:30:1` (RG-147). `--tch` is the row's own longest
       figure, handed over by `progCh` — the same instrument `--ch` already is for
       the figure row across the bottom. Capped so one timer on a wide screen does
       not become the whole page, and against the row's ceiling (see `--progmax`).

       AND IT ELLIPSISES RATHER THAN SLICING. Shrink and show is rule 37's answer
       and it is this row's answer too, but a fit that cannot report is the defect
       that rule exists for: `0.62` is an assumed advance against a measured 0.600,
       and the 16 px floor can still bind on a narrow enough row with enough timers.
       In either case the digits overflow a box that is `overflow: hidden`, and a
       sliced clock is a lie the one person reading it cannot detect. An ellipsis is
       the report — it says the figure did not fit instead of showing a shorter one
       that looks correct — and it is the discipline `.tlabel` two rules above
       already keeps on this same row. */
    min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    font-size: min(clamp(16px, calc(92cqw / var(--tmrs) / var(--tch, 6) / 0.62), 64px), calc(var(--progmax) * .45)); }
  /* THE LAST MINUTE, ON THE PREACHER'S OWN PROGRAMME. One rule
     (`layers.js::countdownWarning`), with the per-timer `warn_ms` the frame already
     carries. The same red as the countdown figure above it and as the wall: a
     countdown that reads as two different states depending on which screen you are
     looking at is worse than one that reads as none. Stated here, unconditionally,
     so a viewer who asked for no motion still learns the time is nearly gone. */
  .tmr.warn .tval { color: var(--v-red); }
  /* The zone panel — one instrument, no native dialog (rule 41). */
  .zonepanel { flex: 0 0 auto; max-height: 46dvh; overflow-y: auto; padding: 14px 18px;
    display: flex; flex-direction: column; gap: 10px;
    border-top: 1px solid rgba(255,255,255,.1); background: rgba(255,255,255,.03); }
  .zonegrid { display: flex; flex-wrap: wrap; gap: 8px; }
  .zonebtn { flex: 1 1 auto; min-height: 44px; padding: 0 14px; cursor: pointer;
    font-family: var(--f-mono); font-size:var(--v-fs-b1); font-weight: 700; letter-spacing: .08em;
    color: var(--v-dim); background: rgba(255,255,255,.04);
    border: 1px solid rgba(255,255,255,.14); border-radius: 8px; }
  .zonebtn.on { color: var(--v-amber); border-color: rgba(255,176,0,.45); background: rgba(255,176,0,.1); }
  .zonefoot { margin: 0; font-family: var(--f-mono); font-size:var(--v-fs-mono); color: var(--v-faint); }
  /* A SHARE OF THE READING, not of the viewport. `3.5vw` capped at 20px put the
     reference of the passage a preacher is reading aloud at twenty pixels on a
     fifty-inch platform monitor, which is the size it is on a phone. */
  .ref { font-family: var(--f-mono); font-size: clamp(13px, 2.6cqw, 40px); letter-spacing: .18em; text-transform: uppercase; color: var(--v-amber); }
  /* THE READING FILLS THE ROOM IT HAS.
     `clamp(26px, 7vw, 64px)` is a ceiling, and on the screen this page exists for
     it was the binding one: a 1920×1080 platform monitor gave a verse 64px of type
     in an 800px-tall reading area, about a third of the height available, while
     ProPresenter's stage display fills it. A ceiling cannot know how much text it
     was given, so it has to be set for the longest passage and is then wrong for
     every ordinary one — and an ordinary one, a verse or two, is what a stage
     monitor shows for almost all of a service.

     So it is a FIT instead, in the two bounds that actually constrain it, taking
     the SMALLER (docs/REBRAND.md §3.4 — measured, not tabled):

       width   the block is capped at `--vcpl` characters and a serif advances 0.49em
               per character, so the line is `--vcpl × 0.49` ems wide.
       height  `--vn` characters at `--vcpl` per line is `--vn / --vcpl` lines, plus
               one for the last part-line, each 1.28em of leading.

     Division by a var() inside calc is the same construction the rail beneath this
     already uses and which was measured in a browser (`--ch`), not a new trick.

     THE FLOOR IS DELIBERATE AND IT IS THE OLD CEILING'S FLOOR, 26px. Below it the
     fit has decided a passage cannot be shown whole, and §5's recorded deviation
     takes over: the reading SCROLLS rather than clipping, because a preacher
     reading aloud must not lose the end of a passage. That is unchanged behaviour
     for long passages; what changed is every short one. */
  .verse { --vcpl: 16; font-family: var(--f-serif); line-height: 1.28; color: var(--v-txt);
    max-width: calc(var(--vcpl) * 1ch);
    font-size: max(26px, min(
      calc(94cqw / var(--vcpl) / 0.49),
      calc(84cqh / (var(--vn, 60) / var(--vcpl) + 1) / 1.28)
    )); }
  /* The DEFAULT resting state of the preacher's phone — the thing on screen before
     anything is fired, and therefore the text most likely to be looked at. It was
     2.25:1: the worst contrast in the product, in its least forgiving location. */
  .idle { font-family: var(--f-mono); color: var(--v-faint); letter-spacing: .1em;
    /* The scale step is the FLOOR on a phone, not the size on a platform monitor —
       a stage screen resting at "— standby —" in 14px type reads as a screen that
       has failed rather than one that is waiting. */
    font-size: max(var(--v-fs-h2), min(3.4cqw, 34px)); }
  /* The last minute — the same rule and the same red as the wall. Reduced motion
     gets a glow instead of a pulse; the colour is the same either way. */
  @media (prefers-reduced-motion: reduce) {
    .fig.warn .figv, .railrow.warn { text-shadow: 0 0 .25em rgba(244, 81, 91, .85); }
    .tmr.warn .tval { text-shadow: 0 0 .25em rgba(244, 81, 91, .85); }
  }
  @keyframes cdwarn { 0%, 100% { opacity: 1; } 50% { opacity: .55; } }
  /* Operator's cue note — confidence-monitor only, never on the main output. */
  /* A WORD TO THE PREACHER — docs/REBRAND.md §5. The pulse is the point: a
     platform is a bright place and a flat red panel reads as part of the set. */
  .alert {
    /* FIXED, and above everything. This is read by somebody mid-sentence in front
       of a congregation; it does not share the screen with a clock. */
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 4cqw;
    text-align: center;
    font-family: var(--f-body);
    font-weight: 700;
    line-height: 1.15;
    color: #fff;
    text-shadow: 0 0.02em 0.06em rgba(0, 0, 0, 0.75);
    background: #c8121c;
    overflow: hidden;
  }
  /* FOUR STEPS, AND THE FIRST IS §5's FIGURE UNCHANGED. `.alert` is `position:
     fixed` with no query container above it, so `cqw` here resolves against the
     small viewport — which is what is wanted: this panel IS the screen. A message
     the operator typed in a hurry is longer than a phrase, and at 8.5cqw a
     three-sentence one ran off the bottom of a box that clips. */
  .alert.xl { font-size: 8.5cqw; }
  .alert.lg { font-size: 6cqw; }
  .alert.md { font-size: 4.2cqw; }
  .alert.sm { font-size: 3cqw; }
  @media (prefers-reduced-motion: no-preference) {
    .alert { animation: stagealert 1.4s ease-in-out infinite; }
  }
  @keyframes stagealert {
    0%, 100% { background: #c8121c; }
    50% { background: #7a0a11; }
  }
  /* The ink stays as it is. Amber is this page's own accent — the lockup, the
     reference, the up-next citation and every active control wear it too — so
     repainting one label would make the page less coherent, not more. What
     changed is the FORM: it is now the same label as every other. */
  .note-lbl { color: var(--v-amber); flex: 0 0 auto; }
  /* Up-next panel — confidence info the preacher wants, kept off the main output. */
  /* BOUNDED AND CLIPPED, like every other row beneath the reading. It was neither,
     and it got away with it for as long as the zone was off by default: nothing on
     this row had a ceiling except a `-webkit-line-clamp`, which is a vendor
     property doing load-bearing layout work. "Nothing may leave the screen" has to
     hold for the rows that are ON, so switching this one on is what makes the
     bound necessary. A BASIS, never a height — same reason as the rail. */
  .next { flex: 0 0 auto; max-height: 20%; overflow: hidden;
    container-type: inline-size;
    display: flex; align-items: baseline; gap: 14px; padding: 14px 20px;
    border-top: 1px solid rgba(255,255,255,.08); background: rgba(255,255,255,.02); }
  .next-lbl { color: var(--v-faint); flex: 0 0 auto; }
  .next-body { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  /* "The next item in smaller type beneath the reading" — SMALLER THAN THE READING,
     which is what it is beneath, not smaller than a phone. Both were fixed sizes
     and both were a twelfth of the verse on a platform monitor. */
  .next-ref { font-family: var(--f-mono); font-size: clamp(12px, 1.4cqw, 22px); letter-spacing: .06em; color: var(--v-amber);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .next-text { font-family: var(--f-head); font-size: clamp(16px, 1.9cqw, 30px); color: var(--v-dim); line-height: 1.3;
    display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  /* Landscape — a platform monitor, a lobby TV, a phone turned sideways — gets a
     wider measure, and the fit above re-reads it: more characters per line is
     fewer lines, so the height bound relaxes and the verse grows. One constant,
     both bounds. */
  /* THE MEASURE IS NOW SET PER PASSAGE, in the script, from the passage's own
     length — see `verseCpl`. These two rules are the FALLBACK for the case the
     inline property cannot cover: a `.verse` rendered with no `--vcpl` on it. They
     are the values that shipped before, so nothing gets worse where the inline one
     is missing. Do not raise the landscape number back to a one-size-fits-all
     answer: 22 is right for a verse and puts a long passage on the 26px floor. */
  @media (orientation: landscape) { .verse { --vcpl: 22; } }
  @media (prefers-reduced-motion: reduce) { .status.on i { animation: none; } }

  /* Preacher control panel — a phone that DRIVES the wall. Touch-sized targets
     (44px+), high contrast, and it never touches the mirror above it. */
  .ctl-toggle { flex: 0 0 auto; font-family: var(--f-mono); font-size:var(--v-fs-mono); font-weight: 700;
    letter-spacing: .12em; text-transform: uppercase; color: var(--v-dim); cursor: pointer;
    background: rgba(255,255,255,.04); border: 1px solid rgba(255,255,255,.14);
    border-radius: 8px; padding: 7px 12px; }
  .ctl-toggle.active { color: var(--v-amber); border-color: rgba(255,176,0,.4); background: rgba(255,176,0,.08); }
  .ctl { flex: 0 0 auto; display: flex; flex-direction: column; gap: 12px; padding: 16px 18px;
    border-top: 1px solid rgba(255,255,255,.1); background: rgba(255,255,255,.02);
    max-height: 60dvh; overflow-y: auto; }
  .nav-row { display: flex; gap: 12px; }
  .nav-btn { flex: 1; min-height: 52px; font-family: var(--f-head); font-weight: 700; font-size: 18px;
    color: var(--v-txt); background: rgba(255,255,255,.05); border: 1px solid rgba(255,255,255,.16);
    border-radius: 12px; cursor: pointer; }
  .nav-btn:active { background: rgba(255,255,255,.1); }
  .nav-btn:disabled { opacity: .4; }
  .search { display: flex; gap: 10px; }
  .search input { flex: 1; min-height: 48px; padding: 0 16px; font-family: var(--f-body); font-size:var(--v-fs-h1);
    color: var(--v-txt); background: var(--v-void); border: 1px solid rgba(255,255,255,.18);
    border-radius: 12px; -webkit-appearance: none; }
  .search input::placeholder { color: var(--v-faint); }
  .search input:focus { outline: none; border-color: rgba(255,176,0,.5); }
  .go { flex: 0 0 auto; min-width: 56px; min-height: 48px; font-family: var(--f-mono); font-weight: 700;
    font-size:var(--v-fs-h2); color: var(--v-void); background: var(--v-amber); border: none; border-radius: 12px; cursor: pointer; }
  .go:disabled { opacity: .5; }
  .ctl-err { font-family: var(--f-mono); font-size:var(--v-fs-b1); color: var(--v-amber2); }
  .results { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .result { display: flex; flex-direction: column; align-items: flex-start; gap: 3px; width: 100%; text-align: left;
    padding: 12px 14px; background: rgba(255,255,255,.03); border: 1px solid rgba(255,255,255,.1);
    border-radius: 12px; cursor: pointer; }
  .result:active { background: rgba(255,176,0,.1); border-color: rgba(255,176,0,.35); }
  .result:disabled { opacity: .5; }
  .r-ref { font-family: var(--f-mono); font-size:var(--v-fs-b1); letter-spacing: .08em; text-transform: uppercase; color: var(--v-amber); }
  .r-text { font-family: var(--f-serif); font-size: var(--v-fs-ttl); color: var(--v-dim); line-height: 1.35;
    display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .no-results { font-family: var(--f-mono); font-size:var(--v-fs-b1); color: var(--v-faint); }
</style>
