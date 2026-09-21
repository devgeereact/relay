<script>
  // Service Planner — BUILD a plan. Running it is not this screen's job.
  //
  // It used to be both, and that was the mistake: the operator ran the service
  // from here, which meant they were sitting on a tab that could not show them an
  // AI suggestion. The preacher would go off-script, Relay would detect the verse,
  // and the suggestion would appear on a tab nobody was looking at.
  //
  // So the two jobs are split along the line that actually exists in the church's
  // week. Building a plan is a Tuesday task: unhurried, fiddly, lots of searching
  // and reordering. Running it is a Sunday task: one screen, big targets, no
  // typing, nothing that can be dragged by accident. They want opposite designs.
  //
  // Build here. Run in LIVE.
  //
  // LAYOUT: plans rail · running order · cue inspector. The plans list used to be
  // a separate full-page step; as a rail it stays put, so comparing last week's
  // order with this week's is one click rather than three.
  import { onMount } from 'svelte';
  // The shared workspace grammar (docs/REBRAND.md §2 · §11) — the same columns,
  // panes, type roles and name/value row the Outputs and Settings desks use.
  import WorkspaceFrame from './WorkspaceFrame.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import ErrorState from '../ui/ErrorState.svelte';
  import Loading from '../ui/Loading.svelte';
  import TemplateRender from '../TemplateRender.svelte';
  import { humanError } from '../errors.js';
  import { trapFocus } from '../focus.js';
  import { songCue } from '../cues.js';
  import { setSession } from '../session.js';
  import {
    TYPE,
    typeOf,
    payloadOf,
    slidesOf,
    slideAccent,
    previewState,
    sectionsOf,
    planRuntime,
    fmtDuration,
    parseDuration,
    chipOf,
    planDateLabel,
    cueCountLabel,
    dropIndex,
    reorderTo,
    planChannelsOf,
    sectionBands,
  } from '../plan.js';
  // Picking cues out of the running order, and the words on a control that
  // deletes them. One door for all three modifier combinations; see the module.
  import {
    selectionAfterClick,
    selectionAfterRemoval,
    deleteLabel,
    deletedMessage,
    addedMessage,
    orderWithDuplicateInPlace,
  } from '../planselect.js';
  // ONE RULE FOR "WHICH SCREENS", SHARED WITH THE CONSOLE BAND. The inspector's
  // `toggleCueChannel` and the countdown add block below both mean the same thing
  // by ticking a screen, including what happens when the last one is ticked back
  // on, and Live's Screen Countdown band means it too. Stated once, in the pure
  // module, rather than three times.
  import { toggleScreen } from '../countdown.js';
  // THE SHARED URL BUILDER, and it has to be shared. `main.rs::media_url` builds
  // this for every output screen and `bundledbackgrounds.js` mirrors that rule for
  // the surfaces that show the file itself — the Library's media pane, and now
  // this inspector. A picture Relay SHIPS has no file under `/media/<id>` at all
  // (DECISIONS §90), so a hand-rolled copy here would render every seeded
  // background as a broken-image box in a preview that claims to be the wall.
  import { mediaUrl } from '../bundledbackgrounds.js';
  // THE TWO BLOCKS THAT CAME OUT OF THIS FILE. The add panel's local filter and
  // its four payload builders, and the pointer drag's paint arithmetic. Both were
  // already self-contained and both were untestable where they were: a payload
  // literal five lines deep in an async handler, and a neighbour's offset three
  // lines deep in a pointermove handler that needs a laid-out list and a mouse.
  // `plannerblocks.test.js` holds them, and holds that this file still calls them
  // rather than keeping a second copy of the rules.
  import {
    filterMedia,
    filterAnnouncements,
    verseCue,
    mediaCuePayload,
    announceCuePayload,
    countdownCuePayload,
  } from '../planneradd.js';
  import { dragFrame } from '../plannerdrag.js';
  // Dropping a file straight onto the running order. The triage, the gap the
  // operator is shown, and the order it produces — see the module for why a
  // document is refused at the drop rather than at the fire.
  import {
    triageDrop,
    refusalMessage,
    dropGapAt,
    orderWithDropAt,
    droppedMessage,
  } from '../plannerdrop.js';
  import {
    capture,
    templates,
    localIp,
    listPlans,
    createPlan,
    deletePlan,
    duplicatePlan,
    planItems,
    addPlanItem,
    removePlanItem,
    movePlanItem,
    setPlanNote,
    reorderPlan,
    setPlanSection,
    setPlanDuration,
    setPlanTimer,
    setPlanChannels,
    listOutputChannels,
    setPlanTemplate,
    searchScripture,
    searchSongs,
    getSong,
    listArrangements,
    listMedia,
    // The import path that already exists, reached from a drop instead of from a
    // file dialog. `fileToBase64` is the choke point that holds MAX_IMPORT_BYTES
    // (CLAUDE.md rule 36's shape), so a dropped file gets the same refusal a
    // chosen one does, with no second guard written here.
    importMedia,
    fileToBase64,
    listAnnouncements,
    loadTemplates,
    readErrors,
  } from '../stores/capture.js';

  // ── plans list ──
  let plans = [];
  let showNew = false;
  let newTitle = '';
  let planQ = ''; // rail filter

  // ── editor ──
  let openPlan = null;
  let items = [];
  let selId = null; // cue loaded in the inspector
  // MULTI-SELECT, so a run of cues can go at once (Requirement 8). `picked` is
  // what a bulk action is about and `anchor` is where a Shift-range extends FROM;
  // both are ordinary ids, and `planselect.selectionAfterClick` is the one place
  // that decides what a click does to them. `selId` stays what it always was —
  // the cue the inspector is showing — because the panel is about ONE cue and
  // saying otherwise would make its Template and Screens controls ambiguous.
  let picked = [];
  let anchor = null;

  /**
   * The lengths a cue may ask for, in minutes. A fixed list rather than a text
   * box: `Duration` next to it already takes typed text, and two typed fields one
   * above the other that mean different things is how an operator puts the sermon
   * estimate into the clock. A number is also the only thing the backend will
   * store — `set_plan_timer` clears anything non-positive rather than putting an
   * 0:00 clock on a preacher's rail.
   */
  const TIMER_CHOICES = [1, 2, 3, 5, 10, 15, 20, 25, 30, 45, 60];
  let msg = '';
  // Distinguish "still loading" from "genuinely empty": listPlans swallows errors
  // to [], so without this flag a slow cold-open renders "No plans yet" — telling
  // the operator to create a plan they may already have.
  let loading = true;
  let err = ''; // a FAILURE, humanised — never the green success slot (a failed
  //              save shown in success-green is how a lost cue reads as saved).

  // Every backend mutation routes through here. The store wrappers throw on
  // error; without a catch a rejected invoke() is a silently dead button — the
  // operator clicks Add, nothing happens, and no reason is shown. One wrapper,
  // one humanised error surface, no exceptions.
  async function act(fn) {
    err = '';
    try {
      await fn();
    } catch (e) {
      err = humanError(e);
    }
  }
  let leftMode = 'cues'; // 'cues' | 'add'
  let inspTab = 'general'; // 'general' | 'slides' | 'notes'
  // HOW MANY CUES THIS VISIT TO THE ADD PANEL HAS PUT IN THE PLAN.
  //
  // The add path's friction is the toggle and the search, not the commit — and
  // the reason an operator toggles BACK is to check that the clicks landed. The
  // count and the name of the last one are what answer that without leaving the
  // panel, and the way back out is one button beside them rather than a hunt for
  // the segmented control at the top of the pane.
  let added = 0;

  // one search (add mode) — scripture + songs + media together
  let addQ = '';
  let addVerses = [];
  let addSongs = [];
  let addMedia = [];
  let allMedia = []; // full media library, filtered locally by the query
  // The host the preview's thumbnails are served from — the app's HTTP server on
  // 8032, never the Vite port, which does not exist in a packaged build. Same
  // default and same fallback as the Library's media pane: `localhost` works on
  // this machine, and the LAN address is fetched so the URL shown here is the one
  // an output screen would use.
  let mediaHost = 'localhost';
  let addAnnounce = [];
  let allAnnounce = []; // full announcement list, filtered locally
  let addSearching = false;

  // THE DESK OPENS ON A PLAN, not on an invitation to pick one.
  //
  // This workspace used to open as a page title, a paragraph and three empty
  // panes: the running order said "Pick a plan on the left to open it" and the
  // inspector said "Pick a cue to edit it", so the first thing an operator saw was
  // two sentences telling them the screen was not ready yet. A desk is a desk with
  // the work already on it — the prototype opens on the most recent plan and so
  // does this.
  //
  // It is a READ and nothing else. `open()` lists media, announcements and cues;
  // it invokes nothing that takes a screen, which is what
  // `plannerbuildonly.test.js` holds. And it defers to the operator: if they have
  // already clicked a plan while the list was still arriving, `openPlan` is set and
  // this does nothing rather than yanking them back to the newest one.
  //
  // Templates are loaded here too, not assumed. The inspector offers the template
  // picker for the selected cue; without this the store is empty on a cold open of
  // this tab and every cue reads "Template 4".
  onMount(() => {
    refresh()
      .then(() => {
        if (!openPlan && plans.length) return open(plans[0]);
      })
      .finally(() => (loading = false));
    loadTemplates();
    // The screens a cue may be pointed at. A failed read leaves the list
    // empty and the control says so, rather than offering a picker with
    // nothing in it and no reason.
    loadScreens();
  });

  async function refresh() {
    plans = await listPlans();
    // Keep the open plan's header in step with the rail (cue count, title).
    if (openPlan) openPlan = plans.find((p) => p.id === openPlan.id) || openPlan;
  }

  async function addPlan() {
    const title = newTitle.trim();
    if (!title) return;
    const date = new Date().toISOString().slice(0, 10);
    await act(async () => {
      await createPlan(title, date);
      newTitle = '';
      showNew = false;
      await refresh();
    });
  }
  // TWO-STEP DELETE, in-app. A plan is an evening's work and this was ONE click:
  // `Delete Plan` went straight to `delete_plan` with nothing between it and the
  // database, on a rail row an operator reaches for while looking at the list. The
  // guard is an arm/confirm, NEVER a native `confirm()` — the Tauri webview does
  // not implement it, so it returns `false` without showing anything and a delete
  // guarded by one deletes nothing while reporting success (CLAUDE.md rule 41).
  // Same shape as `Channels.svelte` and `library/History.svelte`: first click arms,
  // second within 3s deletes, and the button says which state it is in.
  let planDelArm = null;
  let planDelArmT;
  function disarmPlanDelete() {
    clearTimeout(planDelArmT);
    planDelArm = null;
  }
  async function removePlan(p, ev) {
    ev.stopPropagation();
    if (planDelArm !== p.id) {
      planDelArm = p.id;
      clearTimeout(planDelArmT);
      planDelArmT = setTimeout(() => (planDelArm = null), 3000);
      return;
    }
    disarmPlanDelete();
    await act(async () => {
      await deletePlan(p.id);
      if (openPlan?.id === p.id) {
        openPlan = null;
        items = [];
        selId = null;
      }
      await refresh();
    });
  }
  async function clonePlan(p, ev) {
    ev.stopPropagation();
    await act(async () => {
      await duplicatePlan(p.id, `${p.title} (copy)`);
      await refresh();
    });
  }

  async function open(p) {
    // Opening a different plan disarms the delete. The arm is keyed by id so a
    // stale one could not fire on the wrong plan, but leaving a rail button
    // reading "Click again" about a plan nobody is looking at is a control whose
    // words have stopped describing its state.
    disarmPlanDelete();
    disarmCueDelete();
    openPlan = p;
    selId = null;
    // A selection is about a plan, so it does not survive opening another one —
    // and neither does an armed delete, which would otherwise be pointing at a
    // cue that is no longer on screen.
    picked = [];
    anchor = null;
    added = 0;
    leftMode = 'cues';
    inspTab = 'general';
    msg = '';
    allMedia = await listMedia().catch(() => []);
    // A failed lookup leaves `localhost`, which is correct on this machine — the
    // preview is never worth failing a plan over.
    const ip = await localIp().catch(() => null);
    if (ip) mediaHost = ip;
    allAnnounce = await listAnnouncements().catch(() => []);
    await loadItems();
    if (items.length) selId = items[0].id;
  }
  async function loadScreens() {
    screens = (await listOutputChannels()) ?? [];
  }

  async function loadItems() {
    items = await planItems(openPlan.id);
  }

  // ── one search: scripture + songs ──
  let addTimer;
  function onAddInput() {
    clearTimeout(addTimer);
    addTimer = setTimeout(doAddSearch, 220);
  }
  async function doAddSearch() {
    const q = addQ.trim();
    if (!q) {
      addVerses = [];
      addSongs = [];
      // Recent, not empty — the panel opens before anybody types. `planneradd.js`
      // owns the rule and the limit so an empty box can never become no results.
      addMedia = filterMedia(allMedia, '');
      addAnnounce = filterAnnouncements(allAnnounce, '');
      return;
    }
    // try/finally so a failed search always releases the spinner — otherwise the
    // panel is stuck on "Searching…" forever with no results and no reason.
    addSearching = true;
    try {
      const [v, s] = await Promise.all([searchScripture(q), searchSongs(q)]);
      addVerses = v;
      addSongs = s;
      addMedia = filterMedia(allMedia, q);
      addAnnounce = filterAnnouncements(allAnnounce, q);
    } catch (e) {
      err = humanError(e);
    } finally {
      addSearching = false;
    }
  }
  /**
   * ONE DOOR ONTO `add_plan_item`, taking a built cue.
   *
   * The four result kinds each spelled their own `addPlanItem(plan, type, label,
   * payload)` call, so the type, the label and the payload were three arguments a
   * call site could pair wrongly. `planneradd.js` builds all three together and
   * this hands them over in one shape, which is also what the drop path uses —
   * a fifth way in that cannot disagree with the other four.
   */
  async function commitCue(built) {
    await act(async () => {
      await addPlanItem(openPlan.id, built.cue_type, built.label, built.payload);
      await loadItems();
      await refresh();
      // SAY WHAT LANDED. The add path's measured friction is the toggle and the
      // search, not the commit — but a commit with no acknowledgement is WHY an
      // operator toggles back to the running order to check, which is the toggle
      // friction arriving by another route. One click, one named cue, said in the
      // panel the click happened in (`.sp-addnote`, not the pane head).
      msg = addedMessage(built.label);
      added += 1;
    });
  }
  const addVerse = (v) => commitCue(verseCue(v));
  const addMediaCue = (m) => commitCue(mediaCuePayload(m));
  const addAnnounceCue = (a) => commitCue(announceCuePayload(a));
  // THE WORDS BESIDE THE CLOCK ARE THE OPERATOR'S, AND THIS IS WHERE THEY ARE
  // TYPED. Both fields were written here as constants — 'Service begins in' and
  // 'Welcome' — with no control anywhere in Relay to edit them, so every church
  // ran the same two sentences whether or not they meant them. They are payload
  // (`content.reference` and `countdown_done`), not template: blank is a real
  // answer and it shows the digits alone, which is what the console's own Start
  // sends. A cue that wants words asks for them here.
  let cdAddMin = 5;
  let cdAddLabel = '';
  let cdAddDone = '';
  // ── AND WHICH SCREENS, AT THE MOMENT IT IS BUILT (2026-09-20) ────────────
  //
  // The inspector's `Screens` row has been able to aim any cue since RG-161, and
  // it still can — this changes nothing about that door. What it adds is the
  // question being asked where the answer is known: a pre-service countdown is
  // aimed at the streaming screen, and the operator knows that while they are
  // typing the length, not after hunting for a row in an inspector they have to
  // select the new cue to see.
  //
  // The operator's instruction of 2026-09-20 asked for the Screen Countdown out
  // of Quick tools and *"only where it is actually needed: the pre-service
  // countdown for the service going online"*. This is the half of that which is
  // a Tuesday job; the console band on Live is the half that is a Sunday one.
  //
  // `null` is every screen and is the default, for the same reason it is on the
  // inspector row and in the console band: "all of them" has one spelling, and a
  // list naming every screen silently stops including a screen added later.
  /** @type {number[]|null} */
  let cdAddChannels = null;
  /** The rule is `countdown.js`'s, shared with the console band and the row below. */
  function toggleCdAddChannel(id) {
    cdAddChannels = toggleScreen(cdAddChannels, id, screens);
  }
  async function addCountdownCue() {
    const built = countdownCuePayload(cdAddMin, cdAddLabel, cdAddDone);
    await act(async () => {
      await addPlanItem(openPlan.id, built.cue_type, built.label, built.payload);
      // A countdown is the one cue type whose length is known at build time, so it
      // seeds its own duration instead of making the operator retype it.
      await loadItems();
      const added = items[items.length - 1];
      // `built.seconds` and not `m * 60`: the payload builder owns the arithmetic
      // now (`planneradd.js`), so the cue's length and its stated duration come
      // from one place rather than from two that agree today.
      if (added) await setPlanDuration(added.id, built.seconds);
      // THE AIM IS A SECOND WRITE, NOT A THIRD DOOR. `addPlanItem` cannot express
      // a screen set, so the cue is created and then aimed through the SAME
      // `setPlanChannels` the inspector row uses — one command, two surfaces, so
      // the two cannot come to different conclusions about what `null` means.
      // Skipped entirely when the answer is "every screen", because that is what
      // a cue with no `channels_json` already says.
      if (added && cdAddChannels) await setPlanChannels(added.id, cdAddChannels);
      await loadItems();
      await refresh();
    });
  }
  // Song → plan. If the song has saved arrangements, open a picker so the
  // operator chooses one (or Standard); otherwise add the Standard order.
  let arrPick = null; // { song, arrangements } while choosing
  async function addSong(summary) {
    await act(async () => {
      const song = await getSong(summary.id);
      if (!song) return;
      const arrangements = await listArrangements(song.id);
      if (arrangements.length === 0) {
        await commitSong(song, null);
        return;
      }
      arrPick = { song, arrangements };
    });
  }
  async function commitSong(song, arr) {
    const { label, payload } = songCue(song, arr);
    await act(async () => {
      await addPlanItem(openPlan.id, 'song', label, payload);
      arrPick = null;
      await loadItems();
      await refresh();
    });
  }

  // ── DELETING, FROM THE ROW IT IS ABOUT (Requirement 8) ────────────────────
  //
  // A cue could only be removed from the INSPECTOR, so taking three cues out was
  // three select-then-travel round trips across the desk. The operator's line:
  // *"do not hide destructive deletion behind ambiguity, but do not add friction
  // that slows a live operator either."*
  //
  // NOT AMBIGUOUS — the control names the cue when there is one and counts them
  // when there are several (`planselect.deleteLabel`), and the arming step is a
  // state the BUTTON reports rather than a dialog appearing somewhere else.
  // RULE 41 rules out the shortcut: Tauri's webview does not implement
  // `confirm()`, so a two-step delete guarded by one deletes NOTHING and reports
  // success. Same in-app arm/confirm as the plan rail above and `TemplateGallery`.
  //
  // NOT SLOW — one press arms, the second within 3s does it, and the arm is keyed
  // by what it is about so it can never fire on the wrong cue. It also times out:
  // an arm left standing is a destructive control one stray press away from
  // firing, minutes later, about a cue nobody is looking at.
  let cueDelArm = null; // an id, or the string 'picked' for the whole selection
  let cueDelArmT;
  function disarmCueDelete() {
    clearTimeout(cueDelArmT);
    cueDelArm = null;
  }
  function armCueDelete(key) {
    cueDelArm = key;
    clearTimeout(cueDelArmT);
    cueDelArmT = setTimeout(() => (cueDelArm = null), 3000);
  }

  /**
   * Take `ids` out of the plan. The ONE door, so the row control, the bulk
   * control and the inspector's Delete cannot arrive at three different answers
   * about what happens to the selection afterwards.
   *
   * The removals run in sequence rather than in parallel: `reorder_plan` and
   * `remove_plan_item` both rewrite positions, and a plan is small enough that
   * the wait is invisible while a half-applied batch is not.
   */
  async function removeCues(ids) {
    const list = (ids ?? []).filter((n) => n != null);
    if (!list.length) return;
    const label = list.length === 1 ? items.find((i) => i.id === list[0])?.label ?? '' : '';
    // Decided BEFORE the delete, against the order the operator was looking at.
    const land = selectionAfterRemoval(items, list);
    disarmCueDelete();
    await act(async () => {
      for (const id of list) await removePlanItem(id);
      picked = [];
      anchor = null;
      selId = land;
      await loadItems();
      await refresh();
      msg = deletedMessage(list.length, label);
    });
  }
  function remove(id, ev) {
    ev?.stopPropagation?.();
    return removeCues([id]);
  }

  /**
   * A click on a cue row. The ONE place a click reaches the picked set.
   *
   * `selId` is set on EVERY click, including the modified ones: the inspector is
   * about one cue and it should be about the one last touched, so an operator
   * building a run can still read what they are picking. The picked set is what
   * a bulk action is about, and `planselect.selectionAfterClick` decides it.
   *
   * ⌘ on macOS and Ctrl elsewhere, both accepted rather than branched on the
   * platform — a laptop with an external Windows keyboard is a real church, and
   * neither key means anything else on this row.
   */
  function pick(id, ev) {
    const additive = Boolean(ev?.metaKey || ev?.ctrlKey);
    const range = Boolean(ev?.shiftKey);
    const next = selectionAfterClick({ picked, anchor, items, id, additive, range });
    picked = next.picked;
    anchor = next.anchor;
    selId = id;
    // A new selection is a new subject, so an arm aimed at the old one stands
    // down rather than waiting to be confirmed by a press meant for something else.
    if (cueDelArm !== null) disarmCueDelete();
  }
  // ── DROP A FILE STRAIGHT ONTO THE RUNNING ORDER (Requirement 13) ──────────
  //
  // There was no file drop anywhere in `src/`. Putting a picture in a plan meant
  // Library → Import → file dialog → the media review sheet → back here → Add cue
  // → search the filename → click. This is the same import path (`fileToBase64` →
  // `importMedia`) reached from a drag instead, with no new Rust and no new Tauri
  // capability.
  //
  // IT CANNOT COLLIDE WITH THE REORDER, which was deliberately migrated OFF
  // HTML5 drag onto pointer events. A file dragged from the operating system
  // fires `dragenter`/`dragover`/`drop` and never `pointerdown`, so the two never
  // see each other's events — and `dropAt` is held at null while a row is in hand
  // anyway, because an insertion marker and a half-finished reorder on screen at
  // once is two answers to "where will this land".
  //
  // `dragover` MUST `preventDefault()` or the webview navigates to the file and
  // the console is simply gone, mid-build, with no way back but a relaunch.
  let dropAt = null; // the gap index the marker is drawn at, or null when not over
  let dropBusy = false;
  let dropDepth = 0; // enter/leave nest: a child element's `dragleave` is not a leave

  /** Does this drag carry FILES? A row being dragged inside the app does not. */
  function dragHasFiles(e) {
    const t = e?.dataTransfer;
    if (!t) return false;
    if (t.types && typeof t.types.includes === 'function') return t.types.includes('Files');
    return Boolean(t.files?.length);
  }

  function onFileOver(e) {
    if (!openPlan || drag || !dragHasFiles(e)) return;
    // Without this the browser opens the file and the console is gone.
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
    const rows = [...(e.currentTarget?.querySelectorAll?.('.sp-row') ?? [])].map((r) =>
      r.getBoundingClientRect(),
    );
    dropAt = dropGapAt(rows, e.clientY);
  }
  function onFileEnter(e) {
    if (!openPlan || drag || !dragHasFiles(e)) return;
    dropDepth += 1;
  }
  function onFileLeave() {
    dropDepth = Math.max(0, dropDepth - 1);
    if (!dropDepth) dropAt = null;
  }

  /**
   * The drop itself.
   *
   * Refusals are reported and the rest still land — a mixed drop is not
   * all-or-nothing, because refusing the batch over one PDF makes an operator
   * drag the pictures again. The size refusal is NOT re-implemented here:
   * `fileToBase64` throws it before it allocates anything, and `humanError`
   * prints that shape verbatim.
   *
   * Every file is imported, then every cue is added, and THEN one `reorder_plan`
   * puts them where the marker was. The marker's gap is an index into the order
   * as it was BEFORE the adds, which is the list the operator was looking at.
   */
  async function onFileDrop(e) {
    if (!openPlan || drag || !dragHasFiles(e)) return;
    e.preventDefault();
    const at = dropAt;
    dropDepth = 0;
    dropAt = null;
    const { accept, refused } = triageDrop(e.dataTransfer?.files);
    if (!accept.length) {
      err = refusalMessage(refused) || 'Nothing in that drop could become a cue.';
      return;
    }
    const before = items.map((i) => i.id);
    dropBusy = true;
    await act(async () => {
      try {
        const newIds = [];
        for (const { file, kind } of accept) {
          // `import_media` answers the whole `media_assets` row, so the library
          // the inspector's preview looks the asset up in is kept in step here
          // rather than by a second `list_media` round trip.
          const asset = await importMedia(kind, file.name, await fileToBase64(file));
          if (asset) allMedia = [...allMedia, asset];
          const built = mediaCuePayload(asset ?? { id: null, kind, filename: file.name });
          const id = await addPlanItem(openPlan.id, built.cue_type, built.label, built.payload);
          if (id != null) newIds.push(id);
        }
        await loadItems();
        const order = orderWithDropAt(items.map((i) => i.id), newIds, at ?? before.length);
        if (order.length && order.join() !== items.map((i) => i.id).join()) {
          await reorderPlan(openPlan.id, order);
          await loadItems();
        }
        if (newIds.length) selId = newIds[0];
        await refresh();
        msg = droppedMessage(newIds.length);
        // A refusal alongside a success is still news. `err` is the rose slot and
        // outranks `msg` in the head, which is the right way round: the operator
        // can see the cues that landed, and cannot see the file that did not.
        if (refused.length) err = refusalMessage(refused);
      } finally {
        dropBusy = false;
      }
    });
  }

  /** Put the selection down without touching the plan. */
  function clearPicked() {
    picked = [];
    anchor = null;
    disarmCueDelete();
  }
  async function move(id, dir, ev) {
    ev.stopPropagation();
    await act(async () => {
      await movePlanItem(id, dir);
      await loadItems();
    });
  }

  // ── Drag-reorder: pointer events, 1:1 with the finger, committed on release ──
  //
  // This was HTML5 drag-and-drop (`draggable`, `dragover`, `drop`). Three things
  // were wrong with that for a running order. The row did not move — the browser
  // drew its own translucent ghost and the list sat still, so there was no moment
  // at which the operator could see the order they were about to get. The drop
  // target was whichever row the pointer happened to be over, which on a list of
  // 34px rows is a different row from the gap you were aiming at. And
  // `dragstart`/`drop` are mouse-only in practice: a touch drag on a laptop's
  // trackpad-as-touchscreen, or a pen, never begins one.
  //
  // Pointer events are one code path for mouse, pen and touch. The row follows the
  // pointer exactly, its neighbours slide out of the way by one row height so the
  // gap is visible, and NOTHING is persisted until release — a drag abandoned
  // mid-list leaves the plan exactly as it was.
  //
  // The arithmetic is `plan.dropIndex`, not a line in this handler, because the
  // ends of the list are where a reorder goes wrong and that is worth a test.
  let drag = null; // { id, from, rows, y0, h, dy } while a row is in hand
  let dragId = null; // the row currently in hand, for the class

  function onGripDown(id, e) {
    if (e.button != null && e.button !== 0) return; // left button / touch only
    const row = e.currentTarget?.closest?.('.sp-row');
    const list = row?.parentNode;
    if (!row || !list) return;
    const rows = [...list.querySelectorAll('.sp-row')];
    const from = rows.indexOf(row);
    if (from < 0) return;
    // `offsetHeight` is 0 in a list that has not been laid out (and always in
    // jsdom); `dropIndex` treats a non-positive height as "nothing moved", and the
    // fallback keeps a real drag working on a list mid-layout.
    drag = { id, from, rows, y0: e.clientY, h: row.offsetHeight || 34, dy: 0 };
    dragId = id;
    selId = id; // what you are dragging is what the inspector is about
    try {
      e.currentTarget.setPointerCapture?.(e.pointerId);
    } catch {
      /* no capture (jsdom, or a mouse fallback) — the window handlers still fire */
    }
    e.preventDefault();
  }

  function onDragMove(e) {
    if (!drag) return;
    drag.dy = e.clientY - drag.y0;
    // The ARITHMETIC is `plannerdrag.dragFrame`; this writes the styles. A still
    // row comes back as exactly 0 and its style is CLEARED rather than set to
    // `translateY(0px)` — the each block is keyed, so a leftover inline transform
    // would paint the new order shifted by a row.
    const frame = dragFrame(drag.rows.length, drag.from, drag.h, drag.dy);
    drag.rows.forEach((r, i) => {
      r.style.transform = frame[i] ? `translateY(${frame[i]}px)` : '';
    });
  }

  function onDragEnd() {
    if (!drag) return;
    const { from, rows, dy, h } = drag;
    drag = null;
    dragId = null;
    // Clear every transform BEFORE the list re-renders: the each block is keyed, so
    // Svelte reuses these exact nodes and an inline transform left behind would
    // paint the new order shifted by a row.
    rows.forEach((r) => {
      r.style.transform = '';
    });
    const to = dropIndex(from, dy, h, items.length);
    if (to === from) return;
    const arr = reorderTo(items, from, to);
    items = arr;
    // Optimistic reorder, but NOT fire-and-forget: if the backend rejects, the
    // on-screen order and the persisted order silently diverge. Reload from the
    // source of truth on failure so the two can never disagree.
    act(async () => {
      try {
        await reorderPlan(openPlan.id, arr.map((i) => i.id));
      } catch (e) {
        await loadItems();
        throw e;
      }
    });
  }

  /**
   * Hand this plan to the LIVE tab and go there. The one path from build to run.
   *
   * It hands over a PLAN and a PLAYHEAD and says nothing about the wall.
   * `liveCueId: null` starts this plan at the top; `liveOnAir` is deliberately
   * ABSENT from the patch, because position and on-air-ness are separate facts
   * (CLAUDE.md, `liveCue`) and the second belongs to Live and to the panic
   * controls. Adding `liveOnAir: false` here would have this workspace assert
   * that a congregation's screen is clear — which is a panic control's claim, and
   * one it is not allowed to make on someone else's behalf (DECISIONS §20).
   * Nothing is invoked: loading a plan must not clear the programme (§2).
   * Pinned by `plannerbuildonly.test.js`.
   */
  function runPlan() {
    setSession({ planId: openPlan.id, liveCueId: null, liveSlide: 0, activeTab: 'live' });
  }

  $: selCue = items.find((i) => i.id === selId) || null;

  // ── sections + running time ──
  //
  // Both derived from the ordered cue list, never stored beside it, so a section
  // can never claim cues the transport does not actually walk.
  $: sections = sectionsOf(items);
  // THE BANDS, parallel to `sections` and derived from the same list. Grouping is
  // a fact about the plan; banding is a fact about how it is drawn, which is why
  // the two are separate functions — see `plan.js::sectionBands` for what the two
  // colours mean, what happens at section seven, and why colour is never the only
  // signal here.
  $: bands = sectionBands(sections);
  $: runtime = planRuntime(items);
  $: railPlans = planQ.trim()
    ? plans.filter((p) => p.title.toLowerCase().includes(planQ.trim().toLowerCase()))
    : plans;

  $: selTemplate = selCue?.template_id != null
    ? $templates.find((t) => t.id === selCue.template_id) || null
    : null;

  /**
   * Begin a section at the selected cue and put the cursor in its heading field.
   *
   * Deliberately not a `prompt()`: a modal browser dialog blocks the webview's
   * whole thread, cannot be focus-trapped with the rest of the app, and — as with
   * the arrangement picker below — Escape inside it would not be visible to
   * `shortcuts.js`, so dismissing it could fall straight through to the panic keys
   * and clear the congregation's screens.
   */
  let sectionInput;
  async function addSection() {
    const target = selCue || items[0];
    if (!target) return;
    await act(async () => {
      if (!target.section_title) await setPlanSection(target.id, 'New Section');
      selId = target.id;
      inspTab = 'general';
      await loadItems();
      queueMicrotask(() => sectionInput?.select());
    });
  }
  /** Commit the section heading. Blank merges the cue back into the one above. */
  async function saveSection() {
    if (!selCue) return;
    await act(async () => {
      await setPlanSection(selCue.id, secDraft);
      await loadItems();
    });
  }
  /** Commit a typed cue length. Blank/unreadable = untimed, which is legitimate. */
  async function saveDuration() {
    if (!selCue) return;
    await act(async () => {
      await setPlanDuration(selCue.id, parseDuration(durDraft));
      await loadItems();
    });
  }
  /**
   * BIND THIS CUE TO A CLOCK, or clear the binding. It STORES AND NOTHING ELSE.
   *
   * Nothing on this surface can start, move, stop or SHOW a clock, and nothing
   * ever may: the Planner is the workspace an operator opens on a Tuesday with a
   * congregation in the room, and it may not reach an output or the preacher's
   * monitor
   * (`plannerbuildonly.test.js`, which names `start_timer`, `show_timer`,
   * `adjust_timer` and `stop_timer` among the commands it refuses). Live reads the
   * binding off the cue and starts the clock when the cue actually goes on air.
   *
   * `''` is the "No timer" option and clears it. `setPlanTimer` is not
   * `setPlanDuration`: one is a clock a preacher watches, the other the estimate
   * this workspace adds up in its header.
   */
  // ── WHICH SCREENS THIS CUE IS FOR (RG-161) ───────────────────────────────
  //
  // The Planner is where a plan is built, and "which screens" is a decision the
  // plan makes cue by cue — the only per-screen control before this was the
  // receiver filtering on content KIND against its own template, which is a
  // coarse standing preference rather than a choice a plan gets to make.
  //
  // EVERY SCREEN is the default and is shown as a real option, not as nothing
  // ticked. A screen a cue does not name is left showing what it already had,
  // so this narrows what a cue reaches and can never blank a screen.
  let screens = [];
  $: cueChannels = planChannelsOf(selCue?.channels_json ?? null);

  async function saveChannels(next) {
    if (!selCue) return;
    await act(async () => {
      await setPlanChannels(selCue.id, next);
      await loadItems();
    });
  }
  /**
   * Tick or untick one screen, starting from "every screen" if nothing is set.
   *
   * THE RULE IS `countdown.js`'S, not this function's — including the part that
   * matters most, which is going back to NULL when every screen is ticked again:
   * "all of them" has one spelling, and an explicit list of every screen would
   * silently stop including a screen added later. It was written here first and
   * moved on 2026-09-20, when the countdown add block above and Live's Screen
   * Countdown band both needed it; three copies of that rule is three chances for
   * one of them to mean something else by a tick.
   */
  function toggleCueChannel(id) {
    return saveChannels(toggleScreen(cueChannels, id, screens));
  }

  async function saveTimer(ev) {
    if (!selCue) return;
    const v = ev.target.value;
    await act(async () => {
      await setPlanTimer(selCue.id, v === '' ? null : Number(v));
      await loadItems();
    });
  }
  async function saveTemplate(ev) {
    if (!selCue) return;
    const v = ev.target.value;
    await act(async () => {
      await setPlanTemplate(selCue.id, v === '' ? null : Number(v));
      await loadItems();
    });
  }
  /**
   * Copy a cue IN PLACE — beside the one it was copied from.
   *
   * This comment has always said "in place" and the code has always appended to
   * the end of the plan. On a fourteen-cue running order that is a cue appearing
   * somewhere the operator is not looking, in whatever section the plan happens
   * to finish in, which then has to be dragged back up past everything.
   *
   * `add_plan_item` appends and there is no command that inserts, so the fix is
   * the reorder that already exists: add, then hand `reorder_plan` the order with
   * the new id lifted out of the tail and dropped in after its original.
   * `planselect.orderWithDuplicateInPlace` owns the arithmetic and refuses to
   * guess when either id is missing — a reorder built from a guess persists.
   *
   * The copy's SECTION then comes out right by construction: `add_plan_item`
   * takes no `section_title`, and `sectionsOf` reads an empty one as "still in the
   * section above".
   */
  async function duplicateCue() {
    if (!selCue) return;
    const sourceId = selCue.id;
    await act(async () => {
      const newId = await addPlanItem(
        openPlan.id,
        selCue.cue_type,
        selCue.label,
        payloadOf(selCue),
        selCue.template_id,
      );
      await loadItems();
      const order = orderWithDuplicateInPlace(items.map((i) => i.id), sourceId, newId);
      // Only when it actually moves something. A `reorder_plan` that rewrites the
      // order it was already in is a write nobody asked for.
      if (order.length && order.join() !== items.map((i) => i.id).join()) {
        await reorderPlan(openPlan.id, order);
        await loadItems();
      }
      // THE WHOLE CUE, NOT ITS FIRST WRITE (RG-204). `addPlanItem` inserts type,
      // label, payload and template; duration, timer binding and screens are
      // second writes, and a duplicate that dropped them lost a countdown's
      // five minutes and its screen set. Replayed from the source, in the order
      // the add panel writes them.
      if (newId != null) {
        if (selCue.duration_sec) await setPlanDuration(newId, selCue.duration_sec);
        if (selCue.timer_minutes != null) await setPlanTimer(newId, selCue.timer_minutes);
        if (selCue.channels_json) await setPlanChannels(newId, planChannelsOf(selCue.channels_json));
        selId = newId;
      }
      await refresh();
    });
  }

  // Per-cue drafts. Seeded only when the selected cue changes (id guard) so a
  // reload — or another field's save — cannot clobber an edit in progress.
  let noteDraft = '';
  let secDraft = '';
  let durDraft = '';
  let noteFor = null;
  $: if (selCue && selCue.id !== noteFor) {
    noteDraft = payloadOf(selCue).stage_note || '';
    secDraft = selCue.section_title || '';
    durDraft = selCue.duration_sec ? fmtDuration(selCue.duration_sec) : '';
    noteFor = selCue.id;
  }
  async function saveNote() {
    if (!selCue) return;
    const id = selCue.id;
    await act(async () => {
      await setPlanNote(id, noteDraft);
      await loadItems();
    });
  }
  $: selSlides = slidesOf(selCue);

  // The inspector preview goes through TemplateRender — the ONE renderer used by
  // the fullscreen output and the Templates editor — so what the operator sees
  // here is what the wall will show, by construction rather than by resemblance.
  // THE MEDIA CUE'S ASSET, resolved out of the library already in hand.
  //
  // A media cue's whole content is a picture, and this inspector used to answer
  // "what does this put on the wall?" with its filename. `TemplateRender` could
  // paint it all along — it branches on `content.media_kind` and reads
  // `content.media_url` — so the only thing missing was the lookup and the two
  // fields. `allMedia` is already loaded for the Add-cue search, so this costs
  // no extra call.
  //
  // `found` is a real three-way answer, not a truthiness test: a cue with no
  // `media_id` at all (an older or hand-edited payload) is not the same as one
  // pointing at a row that has been DELETED, and only the second is a defect
  // worth naming a file over. `plan.js::previewState` turns this into the
  // sentence, because the verdict lives there with the other four.
  $: selMedia = (() => {
    if (selCue?.cue_type !== 'media') return null;
    const p = payloadOf(selCue);
    if (p.media_id == null) return null;
    const row = allMedia.find((m) => m.id === p.media_id) || null;
    return { found: Boolean(row), filename: p.filename || selCue.label, row, kind: p.kind || 'image' };
  })();

  $: previewContent = !selCue
    ? null
    : selCue.cue_type === 'scripture'
      ? {
          reference: payloadOf(selCue).reference || selCue.label,
          text: payloadOf(selCue).text || '',
          translation: payloadOf(selCue).translation || '',
        }
      : selMedia?.found
        ? {
            reference: selCue.label,
            text: '',
            translation: '',
            // The two fields `TemplateRender` actually reads. Built by the shared
            // builder so this preview and the wall cannot disagree about where a
            // file lives.
            media_url: mediaUrl(mediaHost, selMedia.row),
            media_kind: selMedia.kind === 'video' ? 'video' : 'image',
          }
        : { reference: selCue.label, text: selSlides[0]?.text || '', translation: '' };

  // What the preview may honestly claim. `plan.js` owns the verdict so the four
  // situations it separates are testable without a component (CLAUDE.md rule 35):
  // a media or countdown cue draws its own content at fire time; a scripture, song
  // or notice cue with nothing to typeset is a cue that would put NOTHING in front
  // of a congregation, which is different news and must not read the same.
  // A media cue has no TEXT and never will, so `hasText` can never speak for it —
  // which is why the resolved asset is passed alongside rather than folded in.
  $: pv = previewState(selCue, Boolean(previewContent?.text), selMedia);
</script>

<!-- Escape closes the arrangement picker, from anywhere — bound at the window rather
     than on the backdrop, which never holds focus. Without this, Escape inside the
     picker fell through to the global panic key: it cleared the congregation's screens
     and left the picker open. (shortcuts.js now also refuses to clear while any
     [role="dialog"] is mounted, so the two halves cannot disagree.) -->
<!-- The drag listens at the WINDOW, not on the row. A pointer that leaves the
     list mid-drag — over the inspector, off the top of the pane, out of the
     window entirely — still has to end the drag somewhere, and a handler bound to
     the row never hears that `pointerup`. Bound to the row, an abandoned drag left
     a cue stuck to the cursor with its neighbours shifted, and the next click
     committed a reorder nobody asked for. `pointercancel` is in the list for the
     same reason: a touch drag interrupted by the OS never sends `pointerup`. -->
<svelte:window
  on:keydown={(e) => arrPick && e.key === 'Escape' && (arrPick = null)}
  on:pointermove={onDragMove}
  on:pointerup={onDragEnd}
  on:pointercancel={onDragEnd} />

<!-- NO STANDFIRST, and the caveat did not go with it.
     The paragraph under the page title said "nothing on this workspace can reach
     an output screen — running it is Live's job", and it cost two rows at the top
     of a desk that should open ON a plan. The sentence now sits under the running
     order as the pane's own footer (`.sp-caveat`), where the prototype puts it and
     where it is stronger than it was as a standfirst: it is a pane FOOTER, outside
     the scroller, so it never scrolls away; it carries no media query, so unlike
     the toolbar note this replaced it cannot `display:none` itself on a narrow
     screen; and it is rendered in every mode, with or without a plan open. Do not
     restore the standfirst as well — half a caveat beside the whole one is weaker
     than the whole one alone, which is why this sentence has now moved twice. -->
<WorkspaceFrame title="Planner" columns="var(--v-rail) minmax(0,1fr) var(--v-insp)">
  <svelte:fragment slot="head">
    {#if !$capture.available}
      <span class="r-badge rose"><span class="bd"></span>Backend not attached — plans need the desktop app</span>
    {/if}
    {#if err}<span class="sp-err r-mono" role="alert">{err}</span>
    {:else if msg}<span class="sp-msg r-mono">{msg}</span>{/if}
  </svelte:fragment>

  <!-- ══ RAIL: every plan, always reachable ══ -->
  <aside class="rw-pane sp-rail">
    <div class="rw-panehead">
      <h2 class="rw-panettl">Service plans</h2>
      <span class="rw-spring"></span>
      <span class="rw-itemn">{plans.length}</span>
    </div>
    <div class="sp-railsearch">
      <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
      <input placeholder="Search plans…" bind:value={planQ} aria-label="Search plans" />
    </div>

    <div class="rw-panebody sp-raillist">
      {#if loading}
        <Loading compact what="plans" />
      {:else if railPlans.length}
        {#each railPlans as p (p.id)}
          <button class="sp-railcard r-focus" class:sel={openPlan?.id === p.id} on:click={() => open(p)}>
            <span class="sp-railtitle" title={p.title}>{p.title}</span>
            <!-- Both halves go through `plan.js`, and neither interpolates the
                 field raw. `{p.cue_count} cue{s}` printed "undefined cues" in this
                 rail against a plan summary that did not carry the field, and
                 `{p.plan_date || 'No date'}` is only right until somebody
                 shortens it to an em dash, which this repository already spends
                 on an untimed cue. A rail row is read, so it says words. -->
            <span class="sp-railfootline">
              <span class="sp-railmeta r-mono">{planDateLabel(p.plan_date)}</span>
              <span class="sp-railcues r-mono">{cueCountLabel(p.cue_count)}</span>
            </span>
          </button>
        {/each}
      {:else if plans.length}
        <div class="sp-hint">No plan matches “{planQ}”.</div>
      {:else if $readErrors.listPlans}
        <!-- RG-95. `listPlans` swallows to `[]`, so a database that did not answer
             read as "No plans yet" — and the answer to that sentence, on a Tuesday
             evening, is to build Sunday's service again from nothing. -->
        <ErrorState compact error={$readErrors.listPlans} onRetry={refresh} />
      {:else}
        <div class="sp-hint">No plans yet.</div>
      {/if}
    </div>

    <div class="rw-panefoot">
      {#if showNew}
        <form class="sp-newform" on:submit|preventDefault={addPlan}>
          <!-- svelte-ignore a11y-autofocus -->
          <input class="r-input" placeholder="Plan title…" bind:value={newTitle} autofocus />
          <div class="sp-newbtns">
            <button class="r-btn primary sm" type="submit">Create</button>
            <button class="r-btn ghost sm" type="button" on:click={() => (showNew = false)}>Cancel</button>
          </div>
        </form>
      {:else}
        <!-- ONE ROW, and the nouns are gone (prototype `.railfoot`: `+ New`,
             `Duplicate`). The three buttons were stacked and each repeated the
             word "Plan" under a pane already headed "Service plans" — three tall
             rows saying the same noun three times, which is what pushed the plan
             list itself up the rail. Delete stays, because this is the only route
             to it, and it is now a two-step: the button says which state it is in
             rather than deleting an evening's work on one press. -->
        <div class="sp-railfoot">
          <button class="r-btn primary sm" on:click={() => (showNew = true)}>＋ New</button>
          <button class="r-btn ghost sm" disabled={!openPlan} on:click={(e) => clonePlan(openPlan, e)}>Duplicate</button>
          <button class="r-btn ghost sm sp-raildel" class:arm={planDelArm === openPlan?.id}
            disabled={!openPlan} on:click={(e) => removePlan(openPlan, e)}>
            {planDelArm === openPlan?.id ? 'Click again' : 'Delete'}
          </button>
        </div>
      {/if}
    </div>
  </aside>

  <!-- ══ MAIN: the running order ══ -->
  <section class="rw-pane sp-main">
    <!-- The dock head: the plan's name on the left, what it costs and what you
         can do to it on the right — the prototype's shape (REBRAND §2). `Run in
         Live` moved here from the page head so the one path off this workspace
         sits beside the plan it would hand over, not above three panes. -->
    <div class="rw-panehead sp-panehead">
      <h2 class="rw-panettl sp-plantitle">{openPlan ? openPlan.title : 'Running order'}</h2>
      {#if openPlan}
        <span class="sp-hm r-mono">{planDateLabel(openPlan.plan_date)}</span>
        <!-- "est" is not decoration. Most plans contain a scripture cue, which is
             untimed by nature, so the sum is a floor and never the service length.
             Presenting a partial total as a real one is how a service runs long. -->
        <span class="sp-hm r-mono">{cueCountLabel(items.length)} · {fmtDuration(runtime.seconds, true)}{runtime.partial ? ' est' : ''}</span>
      {/if}
      <span class="rw-spring"></span>
      {#if openPlan}
        <div class="r-seg sp-toolseg">
          <button class:on={leftMode === 'cues'} on:click={() => (leftMode = 'cues')}>Running order</button>
          <button class:on={leftMode === 'add'} on:click={() => { leftMode = 'add'; added = 0; msg = ''; if (!addQ.trim()) { addMedia = filterMedia(allMedia, ''); addAnnounce = filterAnnouncements(allAnnounce, ''); } }}>＋ Add cue</button>
        </div>
        <button class="r-btn ghost sm" disabled={!items.length} on:click={addSection}>＋ Section</button>
      {/if}
      <!-- The ONLY path from build to run, and it hands over a plan — it does not
           put anything on a screen (`runPlan`, and `plannerbuildonly.test.js`).
           Steel blue, NOT the prototype's amber: amber means a congregation is
           looking at something (CLAUDE.md rule 18, DECISIONS §21), and a button on
           the one workspace that cannot reach an output is the last place allowed
           to borrow it. -->
      <button class="r-btn primary sm" on:click={runPlan} disabled={!openPlan || !items.length}>
        Run in Live
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg>
      </button>
    </div>

    {#if loading}
      <div class="rw-panebody pad"><Loading what="plans" /></div>
    {:else if !openPlan && !plans.length && $readErrors.listPlans}
      <div class="rw-panebody pad"><ErrorState error={$readErrors.listPlans} onRetry={refresh} /></div>
    {:else if !openPlan}
      <div class="rw-panebody pad">
        <EmptyState message={plans.length ? 'Pick a plan on the left to open it.' : 'No plans yet — create one to start building a service.'} />
      </div>
    {:else}
      {#if leftMode === 'cues'}
        <!-- THE DROP ZONE is the whole running order, including the empty-plan
             placeholder inside it, so a picture can be dropped onto a plan that
             has nothing in it yet. The handlers ignore anything that is not a
             FILE drag and anything arriving while a row is in hand, so the
             pointer-based reorder underneath is untouched.
             `role="presentation"` and the a11y-ignore: this is not a control and
             does not claim to be one. Everything it does is reachable without a
             mouse through ＋ Add cue → the media search, which is the path it is
             a shortcut for — a drop cannot be a keyboard's only route to
             anything. -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div class="rw-panebody sp-tablewrap" class:dropping={dropAt !== null}
          role="presentation"
          on:dragenter={onFileEnter}
          on:dragover={onFileOver}
          on:dragleave={onFileLeave}
          on:drop={onFileDrop}>
          {#if items.length}
            {#each sections as sec, si (sec.items[0].id)}
              <!-- A section heading is a CAPTION and a hairline to the right edge,
                   not a container: `sectionsOf` derives the grouping from the same
                   ordered list the transport walks, so a heading can never claim a
                   cue the plan does not have in it.
                   THE BAND (`plan.js::sectionBands`). Two hues, alternating, and
                   they say one thing: this is a different section from the one
                   above. Never a kind, never a state — the colours a running
                   service needs are all spoken for. The ORDINAL beside the name is
                   what survives greyscale, so the running order loses the banding
                   and nothing else when the colour is gone. An untitled leading
                   group gets `null`: no number, no ink, because numbering a group
                   the operator never named invents a section. -->
              {@const band = bands[si]}
              {#if sec.title}
                <div class="sp-sec" style={band ? `--sec-ink:${band.ink};--sec-line:${band.line}` : ''}>
                  {#if band}
                    <span class="sp-secn r-mono"
                      ><span class="sr-only">Section&nbsp;</span>{band.ordinal}</span>
                  {/if}
                  <span class="sp-seccap">{sec.title}</span>
                  <span class="sp-secln"></span>
                </div>
              {/if}

              {#each sec.items as c (c.id)}
                {@const n = items.findIndex((i) => i.id === c.id)}
                <!-- WHERE THE DROPPED FILE WILL LAND, shown before the mouse is
                     released. The gap is `plannerdrop.dropGapAt`, the same number
                     the insertion afterwards uses, so the marker cannot promise a
                     position the reorder does not deliver. It carries WORDS as
                     well as a line: a 2px rule is not a signal on its own, and
                     this one appears over a list the operator is mid-drag on. -->
                {#if dropAt === n}
                  <div class="sp-mark"><span class="sp-markw r-mono">Drop here</span></div>
                {/if}
                <div class="sp-row" class:sel={c.id === selId} class:dragging={dragId === c.id}
                  class:inband={Boolean(band)} class:picked={picked.includes(c.id)}
                  style={band ? `--sec-ink:${band.ink}` : ''}
                  on:click={(e) => pick(c.id, e)} role="button" tabindex="0"
                  aria-pressed={c.id === selId || picked.includes(c.id)}
                  on:keydown={(e) => {
                    // A role="button" must answer to Enter AND Space; this one only
                    // took Enter, so it was focusable but half-operable. preventDefault
                    // on Space, or the page scrolls under the operator instead.
                    // The MODIFIERS come through here too, so a keyboard operator
                    // can build the same run a mouse can — the event carries
                    // `shiftKey`/`metaKey` whichever device produced it.
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      pick(c.id, e);
                    }
                  }}>
                  <!-- The grip is a real control, not a decoration with a cursor.
                       It was an `aria-hidden` span, so reordering a plan was a
                       mouse-only act and a keyboard operator had no way to do it
                       from the running order at all. Pointer-drag for a mouse, pen
                       or finger; ↑/↓ for a keyboard, through the same
                       `move_plan_item` the inspector uses. -->
                  <button
                    class="sp-grip"
                    type="button"
                    aria-label="Reorder {c.label} — drag, or use arrow up and arrow down"
                    on:pointerdown={(e) => onGripDown(c.id, e)}
                    on:click|stopPropagation={(e) => pick(c.id, e)}
                    on:keydown={(e) => {
                      // Only the keys this grip uses stop here (RG-198); Esc and B
                      // go on to the shell.
                      if (e.key === 'ArrowUp' && n > 0) { e.preventDefault(); e.stopPropagation(); move(c.id, -1, e); }
                      else if (e.key === 'ArrowDown' && n < items.length - 1) { e.preventDefault(); e.stopPropagation(); move(c.id, 1, e); }
                    }}>
                    <svg width="10" height="14" viewBox="0 0 10 14" fill="currentColor" aria-hidden="true"><circle cx="2" cy="2" r="1.1"/><circle cx="8" cy="2" r="1.1"/><circle cx="2" cy="7" r="1.1"/><circle cx="8" cy="7" r="1.1"/><circle cx="2" cy="12" r="1.1"/><circle cx="8" cy="12" r="1.1"/></svg>
                  </button>
                  <!-- The kind, in a word. One neutral ink for every kind: the
                       colours a taxonomy would want are all spoken for
                       (`TAXONOMY_INK`, CLAUDE.md rule 18), so the WORD is the
                       taxonomy and `chipOf` never truncates it. -->
                  <span class="sp-ck r-mono">{chipOf(c.cue_type)}</span>
                  <!-- One line unless the cue actually has something extra to say.
                       A subtitle under every row doubled the row height and
                       squeezed the cue name — the one thing an operator scans for. -->
                  <span class="sp-cuebody">
                    <span class="sp-cuetitle" title={c.label}>{c.label}</span>
                    {#if payloadOf(c).stage_note}
                      <span class="sp-cuenote" title={payloadOf(c).stage_note}>
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
                        {payloadOf(c).stage_note}
                      </span>
                    {/if}
                  </span>
                  <span class="sp-dur r-mono">{fmtDuration(c.duration_sec)}</span>
                  <!-- DELETE, ON THE ROW IT IS ABOUT. Two presses, never a native
                       `confirm()` (rule 41 — the Tauri webview returns false
                       without showing anything, so a delete guarded by one deletes
                       nothing and reports success). The ACCESSIBLE NAME names the
                       cue and states which press this is, so the control is never
                       a bare glyph and never ambiguous about what it will take.
                       `stopPropagation` so arming a delete does not also re-pick
                       the row and throw away a selection the operator built. -->
                  <button
                    class="sp-del r-focus"
                    class:arm={cueDelArm === c.id}
                    type="button"
                    aria-label={deleteLabel(1, c.label, cueDelArm === c.id)}
                    title={deleteLabel(1, c.label, cueDelArm === c.id)}
                    on:click|stopPropagation={() => (cueDelArm === c.id ? removeCues([c.id]) : armCueDelete(c.id))}
                    >
                    {#if cueDelArm === c.id}
                      <span class="sp-delarm r-mono" aria-hidden="true">AGAIN</span>
                    {:else}
                      <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M5 5l14 14M19 5L5 19"/></svg>
                    {/if}
                  </button>
                </div>
              {/each}
            {/each}
            <!-- The last gap: below every row. Outside the section loop, so it
                 belongs to the plan rather than to whichever section happens to
                 be last. -->
            {#if dropAt === items.length}
              <div class="sp-mark"><span class="sp-markw r-mono">Drop here</span></div>
            {/if}
          {:else if $readErrors.planItems}
            <!-- RG-95, last two surfaces. `planItems` swallowed to `[]`, so a read
                 that failed said "Empty plan" about a plan the operator spent an
                 evening building. -->
            <ErrorState error={$readErrors.planItems} onRetry={loadItems} />
          {:else}
            <!-- An EMPTY plan is a drop target too, and it says so — a placeholder
                 that only ever names one way in would have an operator hunting for
                 a file dialog with the file already under their hand. -->
            <div class="sp-drop r-mono" class:over={dropAt !== null}>
              {#if dropAt !== null}
                Drop to add it as the first cue.
              {:else}
                Empty plan — use ＋ Add cue, or drop a picture or a video here.
              {/if}
            </div>
          {/if}
          {#if dropBusy}
            <div class="sp-drophint r-mono" role="status">Importing the dropped file…</div>
          {/if}
        </div>
      {:else}
        <div class="rw-panebody sp-addpanel">
          <div class="sp-addsearch">
            <svg class="sp-searchic" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.9"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
            <!-- svelte-ignore a11y-autofocus -->
            <input placeholder="Search scripture, songs &amp; media to add…" bind:value={addQ} on:input={onAddInput} autofocus />
          </div>
          <div class="sp-cdadd">
            <span class="sp-dot" style="background:{TYPE.countdown.color};"></span>
            <span class="sp-cdlbl">Countdown</span>
            <input class="sp-cdmin" type="number" min="1" max="120" bind:value={cdAddMin} aria-label="Countdown minutes" />
            <span class="sp-cdunit r-mono">min</span>
            <button class="r-btn ghost sm sp-cdgo" on:click={addCountdownCue}>＋ Add</button>
          </div>
          <!-- THE WORDS, AND THEY ARE OPTIONAL. A countdown's label and its
               message at zero are payload the operator writes, and until now they
               were two constants nothing could edit — so this is the interface
               that never existed rather than a new feature. Blank is a real
               answer: it puts the digits on the wall and nothing else, which is
               what the console's own Start now sends.
               A VISIBLE LABEL, BOUND BY `for`/`id`. A placeholder is not an
               accessible name — it is unread by some screen readers and it
               disappears the moment somebody types — and this planner has already
               had that defect once, on the two controls in its inspector. -->
          <div class="sp-cdwords">
            <label class="r-lbl sp-cdwlbl" for="sp-cdlabel">Words above the clock</label>
            <input id="sp-cdlabel" class="r-input sp-cdw" maxlength="60" bind:value={cdAddLabel}
              placeholder="Optional — e.g. Service begins in" />
            <label class="r-lbl sp-cdwlbl" for="sp-cddone">Words at zero</label>
            <input id="sp-cddone" class="r-input sp-cdw" maxlength="60" bind:value={cdAddDone}
              placeholder="Optional — e.g. Welcome" />
            <p class="sp-fhelp sp-cdhelp">Leave both blank for a timer that shows the digits alone.</p>
            <!-- ── AND WHICH SCREENS (2026-09-20) ────────────────────────────
                 A pre-service countdown usually belongs on one screen — the
                 stream — and the operator knows which one while they are
                 building the cue. The inspector's `Screens` row still aims any
                 cue and this does not replace it; it asks the question where the
                 answer is known, through the same `setPlanChannels`.

                 EVERY SCREEN IS THE DEFAULT and is shown as a real choice, not
                 as nothing ticked. A screen a cue does NOT name keeps showing
                 whatever it already had, so this narrows what the cue reaches
                 and can never blank one. -->
            {#if screens.length}
              <div class="r-lbl sp-cdwlbl">Screens</div>
              <div class="sp-cdchset" role="group" aria-label="Screens for this countdown">
                {#each screens as c (c.id)}
                  <button
                    class="r-btn ghost sm sp-cdch"
                    class:on={(cdAddChannels ?? screens.map((x) => x.id)).includes(c.id)}
                    aria-pressed={(cdAddChannels ?? screens.map((x) => x.id)).includes(c.id)}
                    on:click={() => toggleCdAddChannel(c.id)}>{c.name}</button>
                {/each}
              </div>
              <p class="sp-fhelp sp-cdhelp">
                {#if cdAddChannels == null}
                  Every screen.
                {:else if cdAddChannels.length === 0}
                  No screen — this cue would reach nothing.
                {:else}
                  Other screens keep what they are showing.
                {/if}
              </p>
            {/if}
          </div>
          <div class="sp-results">
            {#if addSearching}
              <div class="sp-hint">Searching…</div>
            {:else if addVerses.length || addSongs.length || addMedia.length || addAnnounce.length}
              {#if addVerses.length}<div class="r-lbl sp-reslbl">Scripture</div>{/if}
              {#each addVerses as v}
                <button class="sp-result r-focus" on:click={() => addVerse(v)}>
                  <span class="sp-dot" style="background:{TYPE.scripture.color};"></span>
                  <span class="sp-resbody"><span class="sp-resref">{v.reference}</span><span class="sp-restext">{v.text}</span></span>
                  <span class="sp-plus">＋</span>
                </button>
              {/each}
              {#if addSongs.length}<div class="r-lbl sp-reslbl">Songs</div>{/if}
              {#each addSongs as s}
                <button class="sp-result r-focus" on:click={() => addSong(s)}>
                  <span class="sp-dot" style="background:{TYPE.song.color};"></span>
                  <span class="sp-resbody"><span class="sp-resref">{s.title}</span><span class="sp-restext">{[s.author, s.song_key && `Key ${s.song_key}`, `${s.section_count} slides`].filter(Boolean).join(' · ')}</span></span>
                  <span class="sp-plus">＋</span>
                </button>
              {/each}
              {#if addMedia.length}<div class="r-lbl sp-reslbl">{addQ.trim() ? 'Media' : 'Recent media'}</div>{/if}
              {#each addMedia as m (m.id)}
                <button class="sp-result r-focus" on:click={() => addMediaCue(m)}>
                  <span class="sp-dot" style="background:{TYPE.media.color};"></span>
                  <span class="sp-resbody"><span class="sp-resref">{m.filename}</span><span class="sp-restext r-mono">{m.kind}</span></span>
                  <span class="sp-plus">＋</span>
                </button>
              {/each}
              {#if addAnnounce.length}<div class="r-lbl sp-reslbl">{addQ.trim() ? 'Announcements' : 'Recent announcements'}</div>{/if}
              {#each addAnnounce as a (a.id)}
                <button class="sp-result r-focus" on:click={() => addAnnounceCue(a)}>
                  <span class="sp-dot" style="background:{TYPE.announce.color};"></span>
                  <span class="sp-resbody"><span class="sp-resref">{a.title || 'Untitled'}</span><span class="sp-restext">{a.body}</span></span>
                  <span class="sp-plus">＋</span>
                </button>
              {/each}
            {:else if addQ.trim()}
              <div class="sp-hint">Nothing found — save scripture, import songs, or add media in the Library.</div>
            {:else}
              <div class="sp-hint">Type to search scripture, songs and media.</div>
            {/if}
          </div>
        </div>
      {/if}
    {/if}

    <!-- WHAT THE ADD PANEL HAS PUT IN THE PLAN, AND THE WAY BACK.
         The measured friction on the add path is the toggle and the search, not
         the commit — one click per result row already adds a cue. But a commit
         with no acknowledgement is exactly WHY an operator toggles back to the
         running order to check, which is the toggle friction arriving by another
         route. This says what landed and how many, in the panel the click
         happened in, and puts the way out beside it rather than back up at the
         segmented control. Nothing here adds friction to the commit: it appears
         after the first add and is never in the way of the second. -->
    {#if openPlan && leftMode === 'add' && added}
      <div class="rw-panefoot sp-addnote">
        <span class="sp-msg r-mono" role="status">{msg}</span>
        <span class="rw-spring"></span>
        <span class="sp-hm r-mono">{added} added</span>
        <button class="r-btn primary sm" on:click={() => { leftMode = 'cues'; msg = ''; }}>
          Done — see the running order
        </button>
      </div>
    {/if}

    <!-- THE CAVEAT. Outside every `{#if}` above, so it is on screen while a plan
         is loading, while none is open, in both left modes and on an empty plan —
         and outside the scroller, so a long running order cannot push it off. It
         is the sentence that says what this whole workspace is, and the two
         places it lived before could each hide it: a toolbar note with a
         `display:none` below 1240px, then a page standfirst that cost two rows of
         a desk. A caveat that can disappear is not a caveat. -->
    <!-- THE SELECTION BAR, above the caveat and outside the scroller for the same
         reason the caveat is: an action about cues that may have scrolled out of
         sight has to stay where the operator can see how many it is about. It is
         only mounted when MORE THAN ONE cue is picked — a single pick is what a
         plain click has always done, and putting a bulk control over it would be
         a destructive button that is permanently on screen meaning something
         different depending on state nobody can read. -->
    {#if openPlan && leftMode === 'cues' && picked.length > 1}
      <div class="rw-panefoot sp-picked">
        <span class="sp-pickedn r-mono">{picked.length} cues selected</span>
        <span class="rw-spring"></span>
        <button class="r-btn ghost sm" on:click={clearPicked}>Clear selection</button>
        <!-- Two presses, and the label states which one this is and what it will
             take. Never a native `confirm()` — rule 41. -->
        <button class="r-btn ghost sm sp-raildel" class:arm={cueDelArm === 'picked'}
          on:click={() => (cueDelArm === 'picked' ? removeCues(picked) : armCueDelete('picked'))}>
          {deleteLabel(picked.length, '', cueDelArm === 'picked')}
        </button>
      </div>
    {/if}

    <!-- THE CAVEAT. Outside every `{#if}` above, so it is on screen while a plan
         is loading, while none is open, in both left modes and on an empty plan —
         and outside the scroller, so a long running order cannot push it off. It
         is the sentence that says what this whole workspace is, and the two
         places it lived before could each hide it: a toolbar note with a
         `display:none` below 1240px, then a page standfirst that cost two rows of
         a desk. A caveat that can disappear is not a caveat. -->
    <div class="rw-panefoot sp-caveat">
      <p>
        {#if leftMode === 'cues'}Drag <b>⠿</b> to reorder. Shift-click for a run, ⌘/Ctrl-click to
          pick several. Drop a picture or a video anywhere on the list to add it there.
          {/if}Build only — nothing here reaches an output. Run it in <b>Live</b>.
      </p>
    </div>
  </section>

  <!-- ══ INSPECTOR: the selected cue ══ -->
  <aside class="rw-pane rw-insp sp-insp">
    {#if !selCue}
      <div class="rw-panehead"><h2 class="rw-panettl">Cue details</h2></div>
      <div class="sp-empty r-empty">Select a cue.</div>
    {:else}
      <!-- `typeOf`, NEVER a bare `TYPE[…]` with a scripture fallback. A cue of a
           kind this build does not recognise was drawn as Scripture — amber dot,
           "SCRIPTURE", the scripture trigger — which is a claim about what will
           reach a screen, made from an absence. `typeOf` says UNKNOWN and claims
           nothing, and being the one door is what stopped this panel badging the
           cue UNKNOWN while `cueSub` printed SCRIPTURE · AUTO-DETECT under it. -->
      {@const ty = typeOf(selCue.cue_type)}
      <!-- One head, one title. The trigger used to be badged up here as well as
           stated in the Fires row below, and two copies of one fact is how they
           come to disagree — this panel has already had a badge say UNKNOWN while
           the line under it said SCRIPTURE · AUTO-DETECT. `Fires` is the copy. -->
      <div class="rw-panehead"><h2 class="rw-panettl">Cue details</h2></div>

      <!-- THE ORDER IS THE POINT (prototype, planner inspector): what this cue
           will LOOK like, then the three things about it you can change, then the
           three that are simply true of it, then the three things you can do to
           it. The panel used to open with a kind badge, a heading and a subtitle
           — three restatements of the row the operator had just clicked — and
           put the rendered preview five fields down, below the fold on a 900px
           window. The preview is the answer to the only question this panel is
           asked on a Tuesday: what does this put on the wall? -->
      <div class="rw-panebody pad sp-inspbody">
        <!-- THE SLIDE IS THE FIRST THING IN THE PANEL (prototype, planner
             inspector). It used to sit under the tab strip, so a panel whose whole
             job is answering "what does this put on the wall?" opened on three
             buttons. The tabs now choose what appears BELOW the answer rather than
             standing in front of it, and nothing they carried was removed: the
             slide list is where rule 39's stale-arrangement warning is read, and
             the Stage Note is operator-only text that must keep a home. -->
        {#if pv.plate}
          <!-- The rendered slide, through the ONE renderer, over a chequered
               plate. The plate is what makes a KEYED template visible here: the
               `Lower Third` builtin is `background:transparent` with
               `verseColor:#1c1224`, so on the preview's old near-black ground a
               notice rendered as an empty frame under a caption promising this
               was what the wall would show. See `.sp-preview` in the style block
               for why the plate is unconditional whenever a slide IS rendered. -->
          <div class="sp-preview">
            <TemplateRender template={selTemplate ?? {}} content={previewContent} />
          </div>
          <p class="sp-fhelp">
            Rendered by the same engine as the output screens, so this is what the
            wall will show. The chequer is not part of the design — it is where
            this template is transparent and a camera, or whatever is behind the
            screen, shows through. Nothing here is on air.
          </p>
        {:else}
          <!-- No slide to render, so no plate: the chequer is a statement ABOUT a
               rendered slide, and an empty one read as a broken template. The WORDS
               carry it, and they are not one sentence over four situations —
               `previewState` separates a cue that draws its own content at fire
               time from one that has no words saved and would put nothing in front
               of a congregation (CLAUDE.md rule 35). -->
          <div class="sp-noslide" class:warn={pv.state === 'empty' || pv.state === 'unknown'}>
            <p class="sp-noslidemsg">{pv.message}</p>
          </div>
        {/if}
        {#if pv.warning}
          <!-- THE CODEC WARNING (F5). Shown beside a cue that renders fine HERE and
               may paint nothing on a browser screen; `previewState` decides. -->
          <div class="sp-noslide warn" role="status">
            <p class="sp-noslidemsg">{pv.warning}</p>
          </div>
          <p class="sp-fhelp">Nothing here is on air.</p>
        {/if}

        <div class="r-seg sp-insptabs sp-insptabs-top">
          <button class:on={inspTab === 'general'} on:click={() => (inspTab = 'general')}>General</button>
          <button class:on={inspTab === 'slides'} on:click={() => (inspTab = 'slides')}>Slides</button>
          <button class:on={inspTab === 'notes'} on:click={() => (inspTab = 'notes')}>Notes</button>
        </div>

        {#if inspTab === 'general'}
          <!-- LABEL is a VALUE, not an input, and that is deliberate rather than
               unfinished. A cue's label is written when the cue is built — from the
               reference, the song and its arrangement, the media filename, the
               announcement's title — and there is no `set_plan_label` command on
               the bridge to write a new one back. A box an operator can type into
               that silently discards what they typed is worse than a line of text:
               it is a control that reports a success it did not achieve. Renaming
               a cue needs a backend command first; until it exists, this says what
               the cue is called and claims nothing else. -->
          <div class="r-lbl sp-flbl">Label</div>
          <div class="sp-fval" title={selCue.label}>{selCue.label}</div>

          <!-- A placeholder is NOT an accessible name: it is unread by some screen
               readers and it disappears the moment somebody types. Both fields had
               only one, so the two editable controls in this inspector were the only
               unnamed controls in the product (measured, 2026-09-14). The visible
               label above each is the name, bound by `for`/`id`. -->
          <label class="r-lbl sp-flbl" for="sp-section">Section</label>
          <input id="sp-section" class="r-input sp-fin" bind:this={sectionInput} bind:value={secDraft}
            placeholder="No section — part of the one above"
            on:blur={saveSection} on:keydown={(e) => e.key === 'Enter' && e.target.blur()} />

          <label class="r-lbl sp-flbl" for="sp-duration">Duration</label>
          <input id="sp-duration" class="r-input sp-fin r-mono" bind:value={durDraft}
            placeholder={selCue.cue_type === 'scripture' ? 'Untimed — fires on cue' : 'e.g. 5 or 4:30'}
            on:blur={saveDuration} on:keydown={(e) => e.key === 'Enter' && e.target.blur()} />
          <p class="sp-fhelp">A bare number is minutes. Leave blank for a cue that fires when it is reached rather than on a clock.</p>

          <!-- Name and VALUE rows, the frame's third type role (REBRAND §11). Kind
               and Fires are read-only facts about the cue; Template is the one of
               the three an operator sets, so it keeps its control in the value
               column rather than being demoted to a sentence. -->
          <div class="sp-kv">
            <div class="rw-nv">
              <span class="rw-nvk">Kind</span>
              <span class="rw-nvv">{ty.label}</span>
            </div>
            <div class="rw-nv">
              <span class="rw-nvk">Template</span>
              <span class="rw-nvctl">
                <select class="r-select sp-tplsel" aria-label="Template for this cue"
                  value={selCue.template_id ?? ''} on:change={saveTemplate}>
                  <option value="">Channel default</option>
                  {#each $templates as t (t.id)}
                    <option value={t.id}>{t.name}</option>
                  {/each}
                </select>
              </span>
            </div>
            <!-- A CLOCK THE CUE ASKS FOR — stored here, started in Live. It is
                 deliberately in the value column beside Template rather than up
                 with Duration: Duration is an estimate this workspace adds up,
                 and this is a timer a preacher will be watching. The words say
                 which is which; no colour is spent on the difference, because
                 the taxonomy is carried by the words already printed beside
                 every cue and `colourlaw.test.js` is not amended by this wave. -->
            <div class="rw-nv">
              <span class="rw-nvk">Timer</span>
              <span class="rw-nvctl">
                <select class="r-select sp-tmrsel" aria-label="Stage Timer for this cue"
                  value={selCue.timer_minutes ?? ''} on:change={saveTimer}>
                  <option value="">No timer</option>
                  {#each TIMER_CHOICES as m (m)}
                    <option value={m}>{m} min</option>
                  {/each}
                </select>
              </span>
            </div>
            <div class="rw-nv sp-chrow">
              <span class="rw-nvk">Screens</span>
              <span class="rw-nvctl">
                {#if screens.length}
                  <!-- EVERY SCREEN IS A CHOICE, not nothing ticked. A cue that
                       names no screens reaches all of them, which is what every
                       cue written before this existed does — and a screen a cue
                       does NOT name keeps showing whatever it already had, so
                       this narrows what a cue reaches and can never blank one. -->
                  <span class="sp-chset">
                    {#each screens as c (c.id)}
                      <button
                        class="r-btn ghost sm sp-ch"
                        class:on={(cueChannels ?? screens.map((x) => x.id)).includes(c.id)}
                        aria-pressed={(cueChannels ?? screens.map((x) => x.id)).includes(c.id)}
                        on:click={() => toggleCueChannel(c.id)}>{c.name}</button>
                    {/each}
                  </span>
                  <span class="sp-chnote r-dim">
                    {#if cueChannels == null}
                      Every screen.
                    {:else if cueChannels.length === 0}
                      No screen — this cue reaches nothing.
                    {:else}
                      Other screens keep what they are showing.
                    {/if}
                  </span>
                {:else}
                  <span class="sp-chnote r-dim">No screens are set up yet — add one in Outputs.</span>
                {/if}
              </span>
            </div>
            <div class="rw-nv">
              <span class="rw-nvk">Fires</span>
              <!-- `ty.trig`, never a guess from the kind at this call site: it is
                   the ONE door (`typeOf`), so an unrecognised cue says MANUAL here
                   rather than claiming the auto-detect only scripture has. -->
              <span class="rw-nvv">{ty.trig}</span>
            </div>
          </div>
          <!-- THE DIFFERENCE, IN WORDS. Two fields in one inspector can both be
               read as "how long this cue is", and they are not the same fact —
               one is arithmetic on a build surface, the other is a clock a person
               watches. The taxonomy is carried by the words here, as it is for
               every cue kind in this workspace: no colour is spent separating
               them, and `colourlaw.test.js` is not amended. -->
          <p class="sp-fhelp">Timer starts a clock on the preacher’s monitor when this cue goes on air, in Live. Duration, above, is only the running-time estimate this plan adds up.</p>

          <!-- Move up · MOVE DOWN · Delete. "Move down" was missing, and its absence
               was load-bearing once the running order's per-row ↑↓✕ buttons went:
               with only "Move up" on the panel, a cue could be walked towards the
               top of a plan and never back down it without a drag, which is to say
               never at all without a mouse. Duplicate stays — it is the quickest
               route to a second cue of the same shape and nothing else offers it. -->
          <div class="r-lbl sp-flbl">Actions</div>
          <div class="sp-actions">
            <button class="r-btn ghost sm" on:click={duplicateCue}>Duplicate</button>
            <button class="r-btn ghost sm" disabled={items[0]?.id === selCue.id} on:click={(e) => move(selCue.id, -1, e)}>Move up</button>
            <button class="r-btn ghost sm" disabled={items[items.length - 1]?.id === selCue.id} on:click={(e) => move(selCue.id, 1, e)}>Move down</button>
            <button class="r-btn ghost sm sp-raildel" on:click={(e) => remove(selCue.id, e)}>Delete</button>
          </div>
        {:else if inspTab === 'slides'}
          <div class="sp-slidemeta r-mono">
            {#if selCue.cue_type === 'song'}
              {#if payloadOf(selCue).arrangement_stale}
                <!-- The song's sections moved after this cue was built, so the
                     slides below are the song's own order, NOT the arrangement
                     named on the cue. Saying the arrangement's name here would be
                     the badge lying about what is in the plan. -->
                <span class="sp-chip stale">
                  ARRANGEMENT: {(payloadOf(selCue).arrangement_name || 'Standard').toUpperCase()} — NEEDS
                  CHECKING, PLAYING IN THE SONG’S OWN ORDER
                </span>
              {:else}
                <span class="sp-chip">ARRANGEMENT: {(payloadOf(selCue).arrangement_name || 'Standard').toUpperCase()}</span>
              {/if}
            {/if}
            <span class="sp-chip">{selSlides.length} {selCue.cue_type === 'song' ? 'SECTIONS' : 'SLIDE' + (selSlides.length === 1 ? '' : 'S')}</span>
          </div>
          <div class="sp-slides">
            {#each selSlides as s, i}
              <div class="sp-slide">
                <span class="sp-slidetag" style="color:{slideAccent(s.tag)};border-color:{slideAccent(s.tag)}">{s.tag}</span>
                <span class="sp-slidetext">{s.text || s.label}</span>
                <span class="sp-slideidx r-mono">{String(i + 1).padStart(2, '0')}</span>
              </div>
            {/each}
          </div>
        {:else}
          <!-- Named the native way (`for`/`id`) rather than with an aria-label:
               the visible text and the accessible name are then the same string,
               and cannot drift apart. -->
          <label class="r-lbl sp-flbl" for="sp-stage-note">Stage Note</label>
          <textarea id="sp-stage-note" class="r-input sp-note" rows="5" bind:value={noteDraft}
            placeholder="Shows on the confidence monitor only, never on the congregation screen."
            on:blur={saveNote}></textarea>
          <p class="sp-fhelp">
            The preacher's monitor shows this beside the cue. It never reaches an
            output screen.
          </p>
        {/if}
      </div>
    {/if}
  </aside>
</WorkspaceFrame>

<!-- arrangement picker — shown when a song with saved arrangements is added -->
{#if arrPick}
  <!-- The backdrop is a mouse convenience, not a control: it is not focusable and
       does not claim to be a button. The keyboard path is Escape, handled at the
       window (top of this file) — bound to this element it only fired when the
       backdrop held focus, which it does not, so Escape fell through to the
       global panic key and cleared the congregation's screens instead. -->
  <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
  <div class="sp-arrback" role="presentation" on:click={() => (arrPick = null)}>
    <div class="sp-arrsheet" role="dialog" aria-modal="true" aria-label="Choose arrangement" use:trapFocus
      on:click|stopPropagation
      on:keydown={(e) => {
        // Rule 44 (RG-198): an overlay that disarms Escape must consume it. This
        // was a bare `|stopPropagation` under `trapFocus`, so Escape never reached
        // the window handler that closes the sheet, and `shortcuts.js` had already
        // stood down for the dialog. Every other key passes through untouched.
        if (e.key === 'Escape') { e.stopPropagation(); arrPick = null; }
      }}>
      <div class="sp-arrtitle">Add “{arrPick.song.title}”</div>
      <div class="r-lbl sp-arrsub">Choose an arrangement</div>
      <button class="sp-arropt r-focus" on:click={() => commitSong(arrPick.song, null)}>
        <span class="sp-arroptname">Standard</span>
        <span class="sp-arroptseq r-mono">{arrPick.song.sections.length} sections · in order</span>
      </button>
      {#each arrPick.arrangements as a (a.id)}
        <!-- A stale arrangement is offered but not choosable, and it says why.
             Its indices no longer name the sections the operator picked, so
             adding it would put the wrong words in the plan; quietly hiding it
             would leave them hunting for an arrangement they know they made. -->
        <button
          class="sp-arropt r-focus"
          class:stale={a.stale}
          disabled={a.stale}
          on:click={() => commitSong(arrPick.song, a)}>
          <span class="sp-arroptname">{a.name}</span>
          <span class="sp-arroptseq r-mono">{a.sequence.map((i) => (arrPick.song.sections[i]?.tag ?? '?')).join(' · ')}</span>
          {#if a.stale}
            <span class="sp-arrstale">
              The song’s sections changed since this was built — open it in Library →
              Lyrics → Arrangements and check it.
            </span>
          {/if}
        </button>
      {/each}
      <button class="r-btn ghost sm sp-arrcancel" on:click={() => (arrPick = null)}>Cancel</button>
    </div>
  </div>
{/if}

<style>
  /* PLANNER — laid out in the shared workspace grammar (`WorkspaceFrame.svelte`,
     docs/REBRAND.md §2): plans rail · running order · cue inspector, as three
     panes with hairline seams and 8px gutters rather than three floating cards
     with 16px trenches between them. The columns, the type roles and the
     name/value row live in the frame; what is here is what is specific to a plan.

     The plans list used to be a separate full-page step; as a rail it stays put,
     so comparing last week's order with this week's is one click rather than
     three. */

  /* ── rail ── */
  .sp-railsearch{ display:flex; align-items:center; gap:8px; background:var(--v-bg);
    border-bottom:1px solid var(--v-line); padding:0 12px; height:30px; flex:0 0 auto; }
  .sp-railsearch:focus-within{ box-shadow:inset 0 0 0 1px var(--v-accent-line); }
  .sp-railsearch svg{ color:var(--v-faint); flex:0 0 auto; }
  .sp-railsearch input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt);
    font-size:var(--v-fs-b2); }
  .sp-railsearch input::placeholder{ color:var(--v-faint); }

  .sp-raillist{ display:flex; flex-direction:column; }
  /* A dense row with a seam, not a card with a gutter. Selection is steel blue —
     the thing you are working on — and never amber, which means a congregation is
     looking at something. */
  .sp-railcard{ position:relative; display:flex; flex-direction:column; gap:3px; width:100%; text-align:left;
    padding:7px 12px; background:transparent; border:0; border-bottom:1px solid var(--v-line);
    color:inherit; cursor:pointer; transition:background var(--v-dur) var(--v-ease); }
  .sp-railcard:last-child{ border-bottom:0; }
  .sp-railcard:hover:not(.sel){ background:var(--v-surf2); }
  .sp-railcard.sel{ background:var(--v-sel-soft); box-shadow:inset 2px 0 0 var(--v-sel); }
  .sp-railtitle{ font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt); line-height:1.25;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-railfootline{ display:flex; align-items:center; justify-content:space-between; gap:6px; }
  .sp-railmeta, .sp-railcues{ font-family:var(--f-mono); font-size:var(--v-fs-cap); color:var(--v-faint);
    letter-spacing:.02em; }

  /* One row of three, each taking an equal share (prototype `.railfoot`). */
  .sp-railfoot{ display:flex; gap:6px; }
  .sp-railfoot .r-btn{ flex:1 1 0; min-width:0; justify-content:center; }
  .sp-raildel{ color:var(--v-rose); }
  .sp-raildel:hover:not(:disabled), .sp-raildel.arm{ border-color:var(--v-rose); background:var(--v-rose-soft); }
  .sp-newform{ display:flex; flex-direction:column; gap:6px; }
  .sp-newbtns{ display:flex; gap:6px; }
  .sp-newbtns .r-btn{ flex:1; }

  /* ── main ── */
  /* One head, not a header plus a toolbar: the plan's name, what it costs, and
     the two things you do to it, on the seam that already divides the pane. */
  /* Same as Channels: no height override (REBRAND §1). Measured at 1280x800, the
     Planner's three pane heads were 34 / 38 / 34, so the middle one's seam sat
     four pixels low. */
  .sp-panehead{ padding:0 12px; gap:10px; flex-wrap:wrap; }
  .sp-plantitle{ flex:0 0 auto; text-transform:none; letter-spacing:var(--v-tr-h2);
    font-size:var(--v-fs-h3); line-height:var(--v-lh-h3); max-width:46ch; }
  .sp-hm{ display:inline-flex; align-items:center; gap:5px; flex:0 0 auto;
    font-size:var(--v-fs-cap); color:var(--v-dim); }
  /* Nothing open yet: the sentence sits in the middle of the space it is talking
     about, the way the Outputs inspector and the Cue Details panel already do.
     Top-left in a 600×750 void read as a stray line of text rather than an
     invitation. The three `.rw-panebody.pad` bodies under this pane are exactly
     the empty, loading and error voids — the running order and the add panel
     carry their own class — so centring them is centring that one sentence. */
  .sp-main > :global(.rw-panebody.pad){ display:flex; }
  .sp-main > :global(.rw-panebody.pad) > :global(.r-empty),
  .sp-main > :global(.rw-panebody.pad) > :global(.es){ margin:auto; text-align:center; max-width:44ch; }
  .sp-toolseg{ flex:0 0 auto; }
  .sp-msg{ font-size:var(--v-fs-lbl); color:var(--v-emerald); max-width:220px;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* A FAILURE is rose, never the success-green above — the two must never share
     a colour, or a lost save reads as a completed one. */
  .sp-err{ font-size:var(--v-fs-lbl); color:var(--v-red); max-width:280px;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  /* ── the running order ── */
  /* Rows, not a table. This was an eight-column grid — grip, number, cue, type,
     template, duration, trigger, three buttons — with three media queries
     dropping columns as the pane narrowed, which is a lot of machinery for a list
     whose job is to be read top to bottom. The prototype's row is four things:
     the grip you drag, the KIND in a word, the name, and how long it runs. What
     the columns used to carry has not been lost — the inspector states the
     template, the trigger and the kind in full for the selected cue, which is
     also the only cue any of the three is true of — and a row that fits at every
     width needs no breakpoints at all. */
  .sp-tablewrap{ overflow-y:auto; padding:6px 0; }
  .sp-row{ display:flex; align-items:center; gap:8px; padding:5px 10px; min-height:30px;
    cursor:pointer; user-select:none; background:transparent;
    border:1px solid transparent; border-radius:var(--v-r-sm);
    margin:0 6px 2px; width:calc(100% - 12px);
    transition:background var(--v-dur) var(--v-ease), border-color var(--v-dur) var(--v-ease); }
  /* A ROW INSIDE A NUMBERED SECTION carries a left edge in that section's ink.
     It is an EDGE — a shape that is present or absent — before it is a hue: a row
     in a section looks different from a row outside one with every colour
     stripped out, which is what keeps the band from being the only signal. The
     heading above it has scrolled away on a long plan and this has not, so it is
     also the answer to "which section am I looking at?" halfway down.

     `border-left-color` rather than a pseudo-element, so it rides the row's own
     transform during a drag and cannot be left behind by one. */
  .sp-row.inband{ border-left-width:2px; border-left-color:var(--sec-ink); padding-left:9px; }
  .sp-row:hover:not(.sel){ background:var(--v-surf2); border-color:var(--v-line); }
  /* Hover and selection paint the other three edges; the band keeps its own, or a
     row would lose which section it is in at the moment it is pointed at. */
  .sp-row.inband:hover, .sp-row.inband.sel, .sp-row.inband.dragging{ border-left-color:var(--sec-ink); }
  /* Selection is steel blue — the thing you are working on (docs/REBRAND.md §1).
     It is NOT amber: amber means a cue is live on the wall, and a cue merely
     being edited on a Tuesday is not. */
  .sp-row.sel{ background:var(--v-sel-soft); border-color:var(--v-sel); }
  /* The row in hand: lifted, and above its neighbours as they slide past it.
     `transform` is set from the drag handler, so this must not declare one. */
  .sp-row.dragging{ position:relative; z-index:5; background:var(--v-surf3);
    border-color:var(--v-sel); box-shadow:var(--v-shadow-lg); transition:none; }
  .sp-row.dragging .sp-grip{ cursor:grabbing; }

  /* A HANDLE, not a button. It is the grab point of the row it sits in — no
     fill, no edge, `cursor:grab`, and the pointer is meant to press and HOLD
     it rather than click and release. It is still a button ELEMENT because the
     keyboard has to be able to reorder too (↑/↓ on it call the same
     `move_plan_item` the inspector uses), which is the only reason it has a
     focus ring of its own. A `.r-btn` here would put a lozenge in every row. */
  .sp-grip{ display:grid; place-items:center; flex:0 0 auto; padding:2px; cursor:grab;
    background:transparent; border:0; border-radius:var(--v-r-sm); color:var(--v-500);
    touch-action:none; /* or the browser scrolls the pane instead of dragging the row */ }
  .sp-row:hover .sp-grip{ color:var(--v-faint); }
  .sp-grip:focus-visible{ outline:2px solid var(--v-sel); outline-offset:1px; color:var(--v-txt); }

  /* The kind chip. One neutral ink for every kind (`TAXONOMY_INK`): the colours a
     per-kind ramp would want are all spoken for by the colour law, and a chip that
     borrows one is a promise nobody made. The WORD is the taxonomy. */
  .sp-ck{ flex:0 0 auto; min-width:52px; text-align:center; font-size:var(--v-fs-cap);
    font-weight:600; letter-spacing:.08em; padding:2px 6px; border-radius:var(--v-r-sm);
    background:var(--v-surf3); border:1px solid var(--v-line);
    /* `--v-dim`, not `--v-faint`: the muted token is 3.76:1 on `--v-surf3` and
       `tokencontrast.test.js` fails the pair on sight. A kind an operator cannot
       read is a chip that is decoration after all. */
    color:var(--v-dim); }
  .sp-row.sel .sp-ck{ color:var(--v-txt); }

  .sp-cuebody{ flex:1; min-width:0; }
  .sp-cuetitle{ display:block; font-size:var(--v-fs-b2); font-weight:500; color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-cuenote{ display:flex; align-items:center; gap:4px; margin-top:1px; max-width:100%;
    font-size:var(--v-fs-cap); color:var(--v-accent2); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .sp-cuenote svg{ flex:0 0 auto; }
  .sp-dur{ flex:0 0 auto; font-family:var(--f-mono); font-size:var(--v-fs-cap); color:var(--v-faint);
    font-variant-numeric:tabular-nums; }
  .sp-drop{ padding:22px; text-align:center; font-size:var(--v-fs-b2); color:var(--v-faint);
    border:1px dashed transparent; border-radius:var(--v-r-lg); margin:6px;
    transition:border-color var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease); }
  .sp-drop.over{ border-color:var(--v-sel-line); color:var(--v-txt); background:var(--v-sel-soft); }

  /* ── A FILE OVER THE RUNNING ORDER ─────────────────────────────────────────
     STEEL, not a new colour and not a promise one: a drop target is the thing you
     are working on, which is exactly what `--v-sel` means here and on every other
     desk (docs/REBRAND.md §1). It is emphatically not amber — a file being
     dragged over a build surface has nothing to do with a congregation.

     The marker carries WORDS. A 2px rule appearing under a moving cursor is not a
     signal on its own, and this is the whole of the promise the drop makes about
     where the cue will land. */
  .sp-tablewrap.dropping{ box-shadow:inset 0 0 0 1px var(--v-sel-line); }
  .sp-mark{ position:relative; display:flex; align-items:center; gap:8px;
    height:2px; margin:2px 8px; background:var(--v-sel); border-radius:1px; }
  .sp-markw{ position:absolute; left:0; top:-8px; padding:1px 5px;
    font-size:var(--v-fs-cap); line-height:1.2; font-weight:600; letter-spacing:.04em;
    color:var(--v-sel-ink); background:var(--v-sel-fill); border-radius:var(--v-r-sm);
    white-space:nowrap; }
  .sp-drophint{ padding:8px 12px; font-size:var(--v-fs-cap); color:var(--v-dim); }

  /* THE ROW'S DELETE. Always present, never hidden behind a hover: "do not hide
     destructive deletion behind ambiguity" cuts both ways, and a control that
     only exists while the pointer is over it is a control a keyboard operator has
     to discover. It is QUIET at rest — the same metadata ink as the duration
     beside it — and rose the moment it is reached for, which is where the
     destructive colour belongs (it is genuinely destructive, so this is the one
     promise colour this pane is entitled to).

     ARMED it stops being a glyph and says AGAIN, in words, at the same width, so
     the row does not reflow between the two presses an operator is making in
     quick succession. The accessible name carries the whole sentence either way
     ("Delete “Welcome” — click again"), because the glyph never could. */
  .sp-del{ flex:0 0 auto; display:grid; place-items:center; min-width:22px; height:18px;
    padding:0 4px; background:transparent; border:1px solid transparent;
    border-radius:var(--v-r-sm); color:var(--v-500); cursor:pointer;
    transition:color var(--v-dur) var(--v-ease), background var(--v-dur) var(--v-ease),
      border-color var(--v-dur) var(--v-ease); }
  .sp-row:hover .sp-del{ color:var(--v-faint); }
  .sp-del:hover{ color:var(--v-red); border-color:var(--v-red-line); background:var(--v-red-soft); }
  .sp-del:focus-visible{ outline:2px solid var(--v-sel); outline-offset:1px; color:var(--v-red); }
  .sp-del.arm{ color:var(--v-red); border-color:var(--v-red); background:var(--v-red-soft); }
  .sp-delarm{ font-size:var(--v-fs-cap); font-weight:700; letter-spacing:.04em; }

  /* A PICKED ROW is steel — the thing you are working on — and it is a FILL plus
     a left mark rather than a colour alone, so a run of picked cues is a
     continuous block an operator can see the ends of. It is deliberately the same
     family as `.sel` and not a sixth colour: selection and multi-selection are the
     same idea at two sizes, and the running order already spends its one free
     accent on the section band. */
  .sp-row.picked{ background:var(--v-sel-soft); border-color:var(--v-sel-line); }
  .sp-row.picked.sel{ border-color:var(--v-sel); }
  .sp-row.inband.picked{ border-left-color:var(--sec-ink); }

  /* The bar that appears once more than one cue is picked. A pane FOOTER, outside
     the scroller, for the same reason the caveat is: an action about cues that
     may have scrolled out of sight must stay where the count can be read. */
  .sp-picked{ flex-direction:row; align-items:center; gap:8px; padding:7px 12px; }
  .sp-pickedn{ font-size:var(--v-fs-cap); color:var(--v-txt); font-weight:600; }
  /* The add panel's acknowledgement, same shape. */
  .sp-addnote{ flex-direction:row; align-items:center; gap:8px; padding:7px 12px; }
  .sp-addnote .sp-msg{ max-width:none; }

  /* A section heading: a NUMBER, a caption and a hairline that runs to the right
     edge. It was a sticky bar with an amber rule down its left side — amber, on a
     build surface, for a heading. The line is the furniture; the word is the
     heading; and the number is what the colour cannot say on its own.

     THE BAND. `--sec-ink` and `--sec-line` are set on this element by the view
     from `plan.js::sectionBands` — two hues, alternating by the section's
     position, repeating from section three. They are the only two the colour law
     leaves free, and they carry NO promise: the band says "a different section
     from the one above" and nothing about what any cue is or what any screen is
     doing. See the doc comment on `sectionBands` for why there are two and what
     happens at section seven.

     A heading with no band (the untitled leading group) sets neither variable and
     falls back to the hairline it always had — `--sec-line` is unset there, so
     `var(--sec-line, var(--v-line))` is the furniture again and the group is
     plainly not a numbered section. */
  .sp-sec{ display:flex; align-items:center; gap:8px; padding:10px 12px 4px; }
  /* The ordinal. Tabular digits in the section's own ink, over its own soft fill,
     with a real border so the badge is a SHAPE before it is a colour: in
     greyscale it is still a boxed number beside a heading. `sr-only` makes the
     accessible name "Section 3" rather than the digit alone, which a screen
     reader would announce with no idea what it counts. */
  .sp-secn{ flex:0 0 auto; display:inline-grid; place-items:center;
    min-width:17px; height:15px; padding:0 4px; border-radius:var(--v-r-sm);
    font-size:var(--v-fs-cap); line-height:1; font-weight:700;
    font-variant-numeric:tabular-nums;
    color:var(--sec-ink, var(--v-faint));
    border:1px solid var(--sec-line, var(--v-line2)); }
  .sp-seccap{ flex:0 0 auto; font-family:var(--f-mono); font-size:var(--v-fs-cap);
    line-height:var(--v-lh-cap); font-weight:600; letter-spacing:var(--v-tr-caps);
    text-transform:uppercase; color:var(--v-faint); }
  .sp-secln{ flex:1; height:1px; background:var(--sec-line, var(--v-line)); }

  /* THE CAVEAT — a pane footer, never a media query. See the markup. */
  .sp-caveat{ padding:8px 12px; }
  .sp-caveat p{ margin:0; font-size:var(--v-fs-cap); line-height:var(--v-lh-cap); color:var(--v-faint); }
  .sp-caveat b{ color:var(--v-dim); font-weight:600; }

  /* ── add panel ── */
  .sp-addpanel{ overflow-y:auto; }
  .sp-addsearch{ display:flex; align-items:center; gap:9px; background:var(--v-bg);
    border-bottom:1px solid var(--v-line); padding:0 12px; height:34px; }
  .sp-addsearch:focus-within{ box-shadow:inset 0 0 0 1px var(--v-accent-line); }
  .sp-searchic{ color:var(--v-faint); flex:0 0 auto; }
  .sp-addsearch input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt);
    font-size:var(--v-fs-b2); }
  .sp-addsearch input::placeholder{ color:var(--v-faint); }
  .sp-reslbl{ padding:8px 12px 5px; background:var(--v-bg); border-bottom:1px solid var(--v-line); }
  .sp-hint{ font-size:var(--v-fs-b2); color:var(--v-faint); padding:10px 12px; }

  .sp-cdadd{ display:flex; align-items:center; gap:8px; padding:7px 12px;
    border-bottom:1px solid var(--v-line); background:var(--v-surf2); }
  .sp-cdlbl{ font-size:var(--v-fs-b2); color:var(--v-txt); }
  .sp-cdmin{ width:52px; padding:3px 6px; border-radius:var(--v-r-sm); border:1px solid var(--v-line2);
    background:var(--v-surf); color:var(--v-txt); font-family:var(--f-mono); font-size:var(--v-fs-b2); text-align:center; }
  .sp-cdunit{ font-size:var(--v-fs-lbl); color:var(--v-faint); margin-left:-3px; }
  .sp-cdgo{ margin-left:auto; }

  /* The words rail sits UNDER the minutes row rather than beside it: three
     controls and a button already fill a 12px-gutter row at this pane's narrowest,
     and a wrapping flex of fixed-width boxes is how the countdown figure ended up
     beside its own caption in the dock. Two rows, each a label over its field. */
  .sp-cdwords{ display:flex; flex-direction:column; gap:3px; padding:8px 12px 10px;
    border-bottom:1px solid var(--v-line); background:var(--v-surf2); }
  .sp-cdwlbl{ margin-top:4px; }
  .sp-cdwlbl:first-child{ margin-top:0; }
  .sp-cdw{ width:100%; }
  .sp-cdhelp{ margin-top:7px; }

  /* The add panel's kind dot. One neutral for every kind, like the row chip: the
     heading above each group names the kind, and a dot that borrowed a colour
     would be the taxonomy painting a promise again. */
  .sp-dot{ width:6px; height:6px; border-radius:2px; flex:0 0 auto; }

  .sp-results{ display:flex; flex-direction:column; }
  /* A SEARCH RESULT ROW, not a button. Two lines — a reference and the opening
     of the verse, clamped — in a seamed list that scrolls. It is pressable and
     that is all it shares with a button: a column of these at 26px with a fill
     and an edge each would be a list of controls rather than a list of things,
     and the verse text would have nowhere to go. */
  .sp-result{ display:flex; align-items:flex-start; gap:9px; width:100%; padding:8px 12px;
    background:transparent; border:0; border-bottom:1px solid var(--v-line);
    color:var(--v-txt); cursor:pointer; text-align:left;
    transition:background var(--v-dur) var(--v-ease); }
  .sp-result:hover{ background:var(--v-surf2); }
  .sp-result .sp-dot{ margin-top:5px; }
  .sp-resbody{ flex:1; min-width:0; }
  .sp-resref{ display:block; font-family:var(--f-head); font-weight:600; font-size:var(--v-fs-b2); color:var(--v-txt); }
  .sp-restext{ font-size:var(--v-fs-cap); color:var(--v-dim); line-height:1.45; margin-top:2px;
    display:-webkit-box; -webkit-line-clamp:2; -webkit-box-orient:vertical; overflow:hidden; }
  .sp-plus{ color:var(--v-accent); font-family:var(--f-mono); font-weight:700; flex:0 0 auto; }

  /* ── inspector ── */
  .sp-inspbody{ padding:12px; }
  .sp-insptabs{ margin:14px 0 4px; width:100%; }
  .sp-insptabs :global(button){ flex:1; }
  /* The tab strip is the first thing in the body now that the kind badge, the
     heading and the subtitle — three restatements of the row already selected in
     the running order — are gone, so it does not need a 14px run-up. */
  .sp-insptabs-top{ margin-top:0; margin-bottom:10px; }

  .sp-flbl{ margin:14px 0 6px; }
  .sp-fin{ width:100%; }
  /* A FIELD-SHAPED VALUE, deliberately not an input. It lines up with the boxes
     above and below it and it is plainly not one of them: no border, no cursor,
     nothing to click. A disabled input in this slot would look like a box that
     had stopped working rather than a fact. */
  .sp-fval{ font-size:var(--v-fs-b2); line-height:var(--v-lh-b2); color:var(--v-txt);
    padding:2px 0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  /* The three name/value rows. They carry the frame's `.rw-nv` grammar, so the
     seams and the right-aligned mono value are the same ones the Outputs and
     Settings desks use — hence the surrounding box rather than a restatement of
     the row itself. */
  .sp-kv{ margin-top:16px; border:1px solid var(--v-line); border-radius:var(--v-r-sm);
    background:var(--v-surf2); overflow:hidden; }
  .sp-tplsel{ max-width:172px; }
/* A SCREEN TICK. The shared button, pressed-state only: it is a toggle in a
     set rather than an action, so `on` is its whole visual job. */
  .sp-ch.on{ background:var(--v-sel); color:var(--v-txt); }
  .sp-chset{ display:flex; flex-wrap:wrap; gap:4px; }
  /* The add block's own screen choice. The SAME shape as the inspector row's
     above — one ghost button per screen, `.on` for ticked — because it is the
     same question, asked earlier; two shapes for one question is how an operator
     comes to believe they are two settings. */
  .sp-cdchset{ display:flex; flex-wrap:wrap; gap:4px; margin:2px 0 0; }
  .sp-cdch{ max-width:100%; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-cdch.on{ background:var(--v-sel); color:var(--v-txt); }
  .sp-chrow{ align-items:flex-start; }
  .sp-chnote{ display:block; margin-top:4px; font-size:var(--v-fs-cap); }
    .sp-tmrsel{ max-width:172px; }
  .sp-fhelp{ margin:6px 0 0; font-size:var(--v-fs-cap); line-height:1.45; color:var(--v-faint); }
  .sp-note{ width:100%; resize:vertical; font-family:inherit; line-height:1.45; }

  /* The preview is 16:9 because every output Relay drives is. A preview at a
     different aspect than the wall is a preview that lies about line breaks. */
  /* `position:relative` is load-bearing: TemplateRender's root is
     `position:absolute; inset:0`, so without a positioned ancestor the preview
     escapes this box and lays itself out against the page — which reads as a
     dead black rectangle here and a mystery elsewhere. It also supplies its own
     `container-type:size` for the cqw units, so this element must not. */
  /* THE PLATE. A preview on a near-black ground cannot show a KEYED template.
     Measured: the `Lower Third` builtin asks for a transparent ground and a
     near-black verse ink — correct over a camera, invisible over `--v-void`.
     (The value is named in the markup comment above the preview, not here: this
     is a stylesheet, and `workspacegrammar.test.js` reads the whole block, so a
     hex quoted in prose reads to it exactly like a hex somebody picked. It is
     right to — that is how a literal gets in.) A notice cue with that template
     rendered here as an
     empty dark frame under a caption reading "this is what the wall will show",
     which is a claim the box was not keeping. The words were reaching
     `TemplateRender` the whole time and being painted black on black; `Lower
     Third` is also `layout.lowerThird`, and the transparency law in
     TemplateRender makes such a template transparent AT ALL TIMES, so no
     template setting could have rescued it.

     The chequer is UNCONDITIONAL, which is the point: a heuristic for "is this
     template keyed?" can be wrong, and its false NEGATIVE is the defect coming
     back. An opaque template paints straight over the plate and hides it
     completely, so the chequer appears exactly where the template is genuinely
     transparent and nowhere else — which is also what that part of the frame
     means on the wall. Two mid greys mixed from `--v-txt` rather than picked:
     the lighter clears 5:1 against near-black ink and the darker 3.5:1, so a
     keyed template is legible over both halves of the square. (Not a legibility
     instrument — `legibility.js` is that. This is a preview that can be seen.)

     The caption beside it says the chequer is not part of the design; a plate
     nobody explains is a background an operator thinks they chose. */
  .sp-preview{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-sm);
    border:1px solid var(--v-line2); overflow:hidden;
    background-color:color-mix(in srgb, var(--v-txt) 55%, var(--v-void));
    background-image:
      linear-gradient(45deg, color-mix(in srgb, var(--v-txt) 40%, var(--v-void)) 25%, transparent 25%, transparent 75%, color-mix(in srgb, var(--v-txt) 40%, var(--v-void)) 75%),
      linear-gradient(45deg, color-mix(in srgb, var(--v-txt) 40%, var(--v-void)) 25%, transparent 25%, transparent 75%, color-mix(in srgb, var(--v-txt) 40%, var(--v-void)) 75%);
    background-size:14px 14px;
    background-position:0 0, 7px 7px;
    display:grid; place-items:center; }
  /* On the plate, not on the void: this note needs its own ground or it is grey
     text over a chequer. It is the one thing in the box that is Relay speaking
     rather than the template rendering. */
  /* A cue with no slide gets a SENTENCE, not an empty 16:9 plate. The plate is a
     statement about a rendered slide (see `.sp-preview`), so drawing one with
     nothing in it was the panel claiming a render it had not done. `.warn` is the
     rose rule — a cue that would put nothing in front of a congregation is a
     defect in the plan, and rose already means "this is wrong" everywhere else on
     this desk. It is not a promise colour on the run surface: amber, cyan,
     amethyst and grey are, and none of them is spent here. */
  .sp-noslide{ padding:10px 12px; border-radius:var(--v-r-sm);
    background:var(--v-surf2); border:1px solid var(--v-line2); }
  .sp-noslide.warn{ border-color:var(--v-rose); background:var(--v-rose-soft); }
  .sp-noslidemsg{ margin:0; font-size:var(--v-fs-b2); line-height:1.45; color:var(--v-dim); }
  .sp-noslide.warn .sp-noslidemsg{ color:var(--v-txt); }

  .sp-actions{ display:flex; flex-wrap:wrap; gap:6px; }
  .sp-actions .r-btn{ flex:1 1 auto; justify-content:center; }

  .sp-slidemeta{ display:flex; flex-wrap:wrap; gap:6px; margin:12px 0 10px; }
  .sp-chip{ font-size:var(--v-fs-cap); letter-spacing:.05em; color:var(--v-dim); padding:3px 8px;
    border-radius:var(--v-r-sm); background:var(--v-surf2); border:1px solid var(--v-line2); white-space:nowrap; }
  .sp-slides{ display:flex; flex-direction:column; gap:6px; }
  .sp-slide{ position:relative; border-radius:var(--v-r-sm); border:1px solid var(--v-line);
    background:var(--v-surf2); padding:9px 12px 9px 42px; min-height:46px;
    display:flex; align-items:center; }
  .sp-slidetag{ position:absolute; left:10px; top:9px; font-family:var(--f-mono); font-size:var(--v-fs-cap); font-weight:700;
    letter-spacing:.06em; padding:2px 5px; border-radius:var(--v-r-sm); border:1px solid currentColor; }
  .sp-slidetext{ font-size:var(--v-fs-b2); line-height:1.45; color:var(--v-dim); white-space:pre-line;
    display:-webkit-box; -webkit-line-clamp:3; -webkit-box-orient:vertical; overflow:hidden; }
  .sp-slideidx{ position:absolute; right:10px; bottom:7px; font-size:var(--v-fs-cap); color:var(--v-500); }

  .sp-empty{ margin:auto; padding:24px; text-align:center; }

  /* ── arrangement picker ── */
  .sp-arrback{ position:fixed; inset:0; background:rgba(6,6,8,.6); backdrop-filter:blur(3px); z-index:200;
    display:flex; align-items:center; justify-content:center; padding:24px; }
  .sp-arrsheet{ width:min(400px, 92%); max-height:80%; overflow:auto; background:var(--v-surf);
    border:1px solid var(--v-line2); border-radius:var(--v-r-xl); padding:18px;
    display:flex; flex-direction:column; gap:8px; box-shadow:var(--v-shadow-lg); }
  .sp-arrtitle{ font-family:var(--f-head); font-weight:600; font-size:var(--v-fs-h3); color:var(--v-txt); }
  .sp-arrsub{ margin:2px 0 6px; }
  /* A CHOICE CARD, not a button. Each one is an arrangement — a name over the
     section sequence it would play, and for a stale one a third line saying
     why it cannot be chosen — so it is a thing being described, not an action
     being offered. The Cancel beneath them IS a button and wears `.r-btn`,
     which is the distinction this dialog is making visible. */
  .sp-arropt{ display:flex; flex-direction:column; gap:3px; width:100%; text-align:left; padding:10px 12px;
    border-radius:var(--v-r-sm); background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-txt);
    cursor:pointer; transition:.12s; }
  .sp-arropt:hover{ border-color:var(--v-accent); background:var(--v-accent-soft); }
  /* Rose, never amber: this is a thing that is wrong, not a thing that is live
     (DECISIONS §22). */
  .sp-arropt.stale{ border-color:var(--v-rose); opacity:.75; cursor:not-allowed; }
  .sp-arropt.stale:hover{ border-color:var(--v-rose); background:transparent; }
  .sp-arrstale{ font-size:var(--v-fs-lbl); color:var(--v-rose); }
  .sp-chip.stale{ color:var(--v-rose); border-color:currentColor; }
  .sp-arroptname{ font-weight:600; font-size:var(--v-fs-b1); }
  .sp-arroptseq{ font-size:var(--v-fs-cap); letter-spacing:.03em; color:var(--v-faint);
    white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .sp-arrcancel{ align-self:flex-end; margin-top:4px; }
</style>
