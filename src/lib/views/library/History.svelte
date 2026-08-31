<script>
  import { humanError } from '../../errors.js';
  import { onMount } from 'svelte';
  import { showsConfidence } from '../../detect.js';
  import { sundayReport, replayAt, weekOnWeek, describeTrend } from '../../report.js';
  import Loading from '../../ui/Loading.svelte';
  import { capture, listServices, serviceDetail, serviceTimeline, servicePerf, perfHistory, endService, exportService } from '../../stores/capture.js';

  let exportMsg = '';
  async function doExport() {
    if (!selected) return;
    try {
      const path = await exportService(selected.id);
      exportMsg = `Saved to ${path}`;
    } catch (e) {
      // `${e}` on a typed error is "[object Object]" — error.rs sends an object.
      exportMsg = `Export failed — ${humanError(e)}`;
    }
  }

  // Service history is local-first (CLAUDE.md) — transcripts, fired detections,
  // and operator overrides recorded to SQLite during a service, read back here.
  let services = [];
  let selected = null; // { id, title } of the open detail
  let detail = null; // { transcripts, detections }
  let timeline = []; // the merged record: events + operator cues + detections
  let perf = []; // latency snapshots taken during the service
  // One row per SERVICE, for the question a single service cannot answer: is Relay
  // getting slower week by week? A church that adds a bigger model, or whose laptop
  // fills up over a winter, degrades gradually and every Sunday looks fine.
  let trendRows = [];
  $: trend = weekOnWeek(trendRows);
  let loading = false;

  // ── WHAT ACTUALLY HAPPENED ────────────────────────────────────────────────
  //
  // The transcript and the fired verses are two views of a service. Neither can
  // answer the question a church actually asks afterwards — "the projector went
  // blank for a bit, when was that?" — because nothing used to record it.
  //
  // Each row says which store it came from, and that is kept visible rather than
  // flattened: an AI claim, an operator's press, and something Relay observed
  // about itself carry different weight, and a replay that blurs them is a replay
  // that quietly rewrites who did what.
  const SOURCE_WORD = { event: 'Relay', cue: 'Operator', detection: 'Detection' };
  const EVENT_WORD = {
    service_started: 'Service started',
    service_ended: 'Service ended',
    rehearsal_on: 'Rehearsal on',
    rehearsal_off: 'Rehearsal off',
    panic_failed: 'A panic control did NOT reach the screens',
    output_lost: 'Screen stopped responding',
    output_recovered: 'Screen came back',
    lock_lifted: 'Service lock lifted by the operator',
    lock_restored: 'Service lock re-applied',
    // From `cues` — the operator's own actions.
    clear_screens: 'Screens cleared',
    blackout: 'Blackout',
    manual_override: 'Manual override',
    suggestion_accepted: 'Took Relay\u2019s suggestion',
    suggestion_dismissed: 'Rejected Relay\u2019s suggestion',
    // From `detections` — status is the useful word. `suggested` and `dismissed`
    // are kept because the column permits them and an old database may hold them;
    // nothing has written either since `persist_fire` became the only insert.
    auto: 'Fired by Relay',
    manual: 'Fired by the operator',
    suggested: 'Suggested',
    dismissed: 'Suggestion dismissed',
  };
  const eventWord = (r) => EVENT_WORD[r.kind] ?? r.kind.replace(/_/g, ' ');
  const fmtMs = (ms) => fmtDur((ms || 0) / 1000);
  // A row that says something went wrong reads as one. Rose is the failure colour;
  // amber is never spent here, because nothing on this screen is on air.
  const isFault = (r) => r.kind === 'panic_failed' || r.kind === 'output_lost';

  // ── THE REPORT ────────────────────────────────────────────────────────────
  //
  // Derived, never stored — so it cannot drift from the record it describes, and
  // so an older service reports exactly what was captured at the time rather than
  // a shape invented later. Every field can be null, and null renders "—".
  $: report = timeline.length || perf.length ? sundayReport(timeline, perf, detail) : null;
  const pct = (v) => (v === null ? '—' : `${Math.round(v * 100)}%`);
  const num = (v) => (v === null || v === undefined ? '—' : String(v));

  // ── THE REPLAY ────────────────────────────────────────────────────────────
  //
  // Pick a moment; see what was being said around it and what Relay did. The
  // transcript context is the point: a fire on its own says WHAT went up, and the
  // words either side say WHY — which is the question somebody actually has three
  // days later.
  let replayIdx = null;
  $: replay = replayIdx === null ? null : replayAt(timeline[replayIdx], detail, perf);
  const openReplay = (i) => (replayIdx = replayIdx === i ? null : i);

  // Keep the screen clean: 10 services per page, paginate the rest.
  let page = 0;
  const PER = 10;
  $: pageCount = Math.max(1, Math.ceil(services.length / PER));
  $: if (page > pageCount - 1) page = pageCount - 1;
  $: pageServices = services.slice(page * PER, page * PER + PER);

  async function refresh() {
    services = await listServices();
    page = 0;
  }
  onMount(refresh);

  function fmtDur(secs) {
    const s = Math.round(secs || 0);
    const m = Math.floor(s / 60);
    return `${m}:${String(s % 60).padStart(2, '0')}`;
  }
  function fmtTs(secs) {
    return fmtDur(secs);
  }

  // ── SEARCH TRANSCRIPT (§6) ────────────────────────────────────────────────
  //
  // Client-side, over the transcript already loaded for the open service. There
  // is no backend transcript search, so this deliberately does NOT pretend to
  // search every service — it says which service it is searching, and the empty
  // result says so too. A search box that silently covers less than the operator
  // assumes is worse than no search box.
  //
  // Case-insensitive substring. Not fuzzy, not stemmed: a volunteer looking for
  // what the preacher said about "covenant" wants the lines containing that word,
  // and a near-miss engine here would quietly hide the line they remember.
  let q = '';
  const norm = (v) => (v ?? '').toLowerCase();
  $: hits = detail?.transcripts && q.trim()
    ? detail.transcripts.filter((t) => norm(t.text).includes(norm(q).trim()))
    : (detail?.transcripts ?? []);
  $: searching = !!q.trim();

  /** Split a line into [before, match, after] runs so the hit can be marked. */
  function runs(text, needle) {
    const n = norm(needle).trim();
    if (!n) return [{ t: text, hit: false }];
    const out = [];
    let i = 0;
    const hay = norm(text);
    while (i < text.length) {
      const at = hay.indexOf(n, i);
      if (at === -1) {
        out.push({ t: text.slice(i), hit: false });
        break;
      }
      if (at > i) out.push({ t: text.slice(i, at), hit: false });
      out.push({ t: text.slice(at, at + n.length), hit: true });
      i = at + n.length;
    }
    return out;
  }

  async function open(svc) {
    selected = svc;
    detail = null;
    exportMsg = '';
    q = '';
    loading = true;
    timeline = [];
    perf = [];
    trendRows = [];
    replayIdx = null;
    try {
      detail = await serviceDetail(svc.id);
      // Read-only history, and both degrade to [] rather than throwing — a
      // service whose timeline is missing must still show its transcript.
      timeline = await serviceTimeline(svc.id);
      perf = await servicePerf(svc.id);
      trendRows = await perfHistory('audio_to_partial_transcript', 12);
    } catch (e) {
      // `error` is RENDERED now. It was set here and referenced nowhere in the
      // template, so a service whose detail failed to load was reported to the
      // operator as a service that had not been recorded — "No transcript
      // recorded", "No verses fired" — with the reason sitting one property away.
      // Telling somebody their Sunday was not captured, when in fact a query
      // failed, is the kind of wrong that gets acted on.
      detail = { transcripts: [], detections: [], error: humanError(e) };
    }
    loading = false;
  }
  function back() {
    selected = null;
    detail = null;
    refresh();
  }
  async function stopRecording() {
    await endService();
    refresh();
  }
</script>

{#if selected}
  <!-- ══ DETAIL STATE ══ -->
  <div class="lib-view">
    <div class="lib-detail-top">
      <button class="r-btn ghost sm" on:click={back}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>
        History
      </button>
      <div class="lib-detail-head">
        <div class="lib-detail-title">{selected.title}</div>
        <div class="lib-detail-date r-mono">{selected.date}</div>
      </div>
      <div class="lib-detail-actions">
        <span class="lib-detail-count r-mono">{selected.verses} verses · {selected.overrides} overrides · {fmtDur(selected.duration_secs)}</span>
        <button class="r-btn primary sm" on:click={doExport}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>
          Export .md
        </button>
      </div>
    </div>

    {#if exportMsg}<div class="lib-exportmsg" role="status">{exportMsg}</div>{/if}

    {#if loading}
      <Loading what="services" />
    {:else if detail}
      <div class="lib-detail-grid">
        <div class="lib-transcript-col">
          <div class="lib-collabel-row">
            <div class="r-lbl lib-collabel">Transcript</div>
            <span class="spring"></span>
            {#if searching}
              <span class="lib-hits r-mono">{hits.length} of {detail.transcripts.length}</span>
            {/if}
          </div>

          {#if detail.transcripts.length}
            <input
              class="r-input lib-tsearch"
              type="search"
              bind:value={q}
              placeholder="Search this service's transcript…"
              aria-label="Search this service's transcript" />

            {#if hits.length}
              <div class="r-tile lib-transcript r-scroll">
                {#each hits as t}
                  <div class="lib-tline">
                    <span class="lib-tmeta r-mono">{fmtTs(t.timestamp)} · {t.language}</span>
                    <span class="lib-ttext">
                      {#if searching}
                        {#each runs(t.text, q) as r}{#if r.hit}<mark>{r.t}</mark>{:else}{r.t}{/if}{/each}
                      {:else}{t.text}{/if}
                    </span>
                  </div>
                {/each}
              </div>
            {:else}
              <!-- Says WHAT was searched. This searches the open service only —
                   there is no backend transcript search across services, and an
                   operator must not read "no results" as "he never said it". -->
              <div class="r-tile lib-emptytile">
                <span class="r-empty">
                  Nothing matching “{q.trim()}” in this service. Only
                  <b>{selected?.title || 'this service'}</b> is searched — other services
                  are not.
                </span>
              </div>
            {/if}
          {:else if detail?.error}
            <div class="r-tile lib-emptytile" role="alert">
              <span class="lib-detailerr">Could not open this service — {detail.error}</span>
            </div>
          {:else}
            <div class="r-tile lib-emptytile"><span class="r-empty">No transcript recorded.</span></div>
          {/if}
        </div>

        <div class="lib-detect-col">
          <div class="r-lbl lib-collabel">Detected verses <span class="lib-collabel-n">({detail.detections.length})</span></div>
          {#if detail.detections.length}
            <div class="lib-detect-list">
              {#each detail.detections as d}
                <div class="r-tile lib-detect">
                  <div class="lib-detect-top">
                    <div class="lib-detect-ref">{d.reference ?? 'unresolved'}</div>
                    <!-- GREY. This is service HISTORY — nothing on this screen is on air,
                         and the method of a detection that happened last Sunday is a
                         label, not a tally light. -->
                    <span class="r-badge grey lib-detect-method">{d.method}</span>
                  </div>
                  <div class="lib-detect-bottom r-mono">
                    <!-- THE NUMBER RULE APPLIES TO THE ARCHIVE TOO.
                         This printed `conf 0.61` for every method, including
                         paraphrases — the exact number CLAUDE.md §18 and
                         DECISIONS §21 forbid showing, just formatted as a raw
                         decimal instead of a percentage, which if anything reads
                         MORE authoritative. A TF-IDF cosine does not become a
                         probability by being a week old.

                         `showsConfidence` is the same tested helper the Live
                         panel and the Inspector use. -->
                    {#if showsConfidence(d)}
                      <span class="lib-detect-conf">conf {d.confidence.toFixed(2)}</span>
                    {:else}
                      <span class="lib-detect-conf muted">no score — a guess</span>
                    {/if}
                    <span class="lib-detect-fired">fired {fmtTs(d.fired_at)}</span>
                  </div>
                </div>
              {/each}
            </div>
          {:else if detail?.error}
            <div class="r-tile lib-emptytile" role="alert">
              <span class="lib-detailerr">Could not open this service — {detail.error}</span>
            </div>
          {:else}
            <div class="r-tile lib-emptytile"><span class="r-empty">No verses fired.</span></div>
          {/if}
        </div>
      </div>

      <!-- THE SUNDAY REPORT. Derived from the record, never stored — so it cannot
           drift from what it describes, and an older service reports what was
           actually captured at the time rather than a shape invented later. -->
      {#if report}
        <div class="lib-rep">
          <div class="r-lbl lib-collabel">This service</div>
          <div class="lib-rep-grid">
            <div class="lib-rep-cell"><b>{report.durationMs === null ? '—' : fmtMs(report.durationMs)}</b><span>length</span></div>
            <div class="lib-rep-cell"><b>{num(report.autoFired)}</b><span>fired by Relay</span></div>
            <div class="lib-rep-cell"><b>{num(report.manualFired)}</b><span>fired by you</span></div>
            <div class="lib-rep-cell"><b>{num(report.suggestionsAccepted)}</b><span>suggestions taken</span></div>
            <div class="lib-rep-cell"><b>{num(report.suggestionsRejected)}</b><span>suggestions rejected</span></div>
            <div class="lib-rep-cell"><b>{pct(report.suggestionUptake)}</b><span>of the ones you answered</span></div>
            <div class="lib-rep-cell" class:bad={report.panicFailures > 0}>
              <b>{num(report.panicFailures)}</b><span>panic controls that failed</span>
            </div>
            <div class="lib-rep-cell" class:bad={report.outputsLost > 0}>
              <b>{num(report.outputsLost)}</b><span>screens that stopped</span>
            </div>
          </div>

          {#if report.latency.length}
            <div class="r-lbl lib-collabel">Speed over the whole service</div>
            <table class="lib-perf r-mono">
              <thead><tr><th>stage</th><th>n</th><th>p50</th><th>p95</th><th>p99</th><th>worst</th><th>grew?</th></tr></thead>
              <tbody>
                {#each report.latency as l (l.metric)}
                  <tr>
                    <td class="lib-perf-metric">{l.metric.replace(/_/g, ' ')}</td>
                    <td>{l.samples}</td>
                    <td>{l.p50_ms === null ? '—' : Math.round(l.p50_ms)}</td>
                    <td>{l.p95_ms === null ? '—' : Math.round(l.p95_ms)}</td>
                    <!-- One window in a hundred: about one visibly late verse per
                         service. "—" on a service recorded before p99 existed. -->
                    <td>{l.p99_ms === null ? '—' : Math.round(l.p99_ms)}</td>
                    <td>{l.worst_ms === null ? '—' : Math.round(l.worst_ms)}</td>
                    <!-- A rising line is the finding whatever the median says. And
                         `null` is "we did not look", which is not "it did not grow". -->
                    <td class:bad={l.grew === true}>{l.grew === null ? '—' : l.grew ? 'yes' : 'no'}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          {/if}

          {#if trend}
            <!-- WEEK BY WEEK. Compared against the MEDIAN of the previous services,
                 not their mean: one catastrophic Sunday — a laptop that was
                 compiling something — would otherwise either hide a real trend or
                 invent one. Silent below three services, because two points are a
                 line through anything. -->
            <p class="lib-rep-trend" class:bad={trend.slower}>
              {describeTrend(trend, 'The transcript')}
            </p>
          {/if}

          <!-- SAID OUT LOUD, not left for a reader to notice. A report that lists
               only what it measured, without naming what it did not, invites
               somebody to read the absence as a pass. -->
          <p class="lib-rep-not">
            <b>What this does not tell you:</b>
            {report.notMeasured.join('. ')}.
          </p>
        </div>
      {/if}

      <!-- WHAT ACTUALLY HAPPENED. The one view that can answer "the projector went
           blank for a bit, when?" — and the only place a panic control that failed
           is recorded after the operator dismissed the banner. -->
      <div class="lib-tl">
        <div class="r-lbl lib-collabel">
          What happened <span class="lib-collabel-n">({timeline.length})</span>
        </div>
        {#if timeline.length}
          <ol class="lib-tl-list">
            {#each timeline as r, i (i)}
              <li>
                <button
                  type="button"
                  class="lib-tl-row"
                  class:fault={isFault(r)}
                  class:open={replayIdx === i}
                  aria-expanded={replayIdx === i}
                  on:click={() => openReplay(i)}
                >
                  <span class="lib-tl-at r-mono">{fmtMs(r.at_ms)}</span>
                  <span class="lib-tl-src r-mono">{SOURCE_WORD[r.source] ?? r.source}</span>
                  <span class="lib-tl-what">{eventWord(r)}</span>
                  {#if r.detail}<span class="lib-tl-detail r-mono">{r.detail}</span>{/if}
                </button>
                {#if replayIdx === i && replay}
                  <div class="lib-rp">
                    <div class="r-lbl">What was being said</div>
                    {#if replay.lines.length}
                      <ol class="lib-rp-lines">
                        {#each replay.lines as l, li (li)}
                          <li><span class="r-mono">{fmtMs(l.at_ms)}</span> {l.text}</li>
                        {/each}
                      </ol>
                    {:else}
                      <!-- An absence, said plainly. Only FINAL transcripts are
                           stored, so a moment can genuinely have no line near it —
                           which is different from the service having no transcript. -->
                      <p class="r-empty">No transcript was recorded within 20 seconds of this.</p>
                    {/if}

                    {#if replay.detection}
                      <div class="r-lbl">What Relay decided</div>
                      <dl class="ch-info lib-rp-dl">
                        <dt>Verse</dt><dd>{replay.detection.reference ?? 'unresolved'}</dd>
                        <dt>How</dt><dd>{replay.detection.method}</dd>
                        <dt>Who</dt>
                        <dd>{replay.detection.status === 'manual' ? 'the operator' : 'Relay'}</dd>
                        <!-- THE NUMBER RULE, in the archive as on the wall. A cosine
                             does not become a probability by being a week old. -->
                        <dt>Score</dt>
                        <dd>
                          {#if showsConfidence(replay.detection)}
                            {replay.detection.confidence.toFixed(2)}
                          {:else}
                            no score — a guess
                          {/if}
                        </dd>
                      </dl>
                    {/if}

                    {#if replay.latency}
                      <div class="r-lbl">Speed at that point</div>
                      <p class="r-mono lib-rp-lat">
                        {replay.latency.metric.replace(/_/g, ' ')} ·
                        p50 {replay.latency.p50_ms === null ? '—' : Math.round(replay.latency.p50_ms)}ms ·
                        p95 {replay.latency.p95_ms === null ? '—' : Math.round(replay.latency.p95_ms)}ms
                      </p>
                    {/if}
                  </div>
                {/if}
              </li>
            {/each}
          </ol>
        {:else if detail?.error}
          <div class="r-tile lib-emptytile" role="alert">
            <span class="lib-detailerr">Could not open this service — {detail.error}</span>
          </div>
        {:else}
          <div class="r-tile lib-emptytile">
            <span class="r-empty">
              No record for this service. Services recorded before this version of Relay
              have no timeline — nothing was watching.
            </span>
          </div>
        {/if}

        {#if perf.length}
          <!-- Latency, as it was measured DURING the service rather than as it is
               now. Percentiles only; a trace carries what was heard. -->
          <div class="r-lbl lib-collabel lib-tl-perfhead">Speed, as measured at the time</div>
          <table class="lib-perf r-mono">
            <thead><tr><th>at</th><th>stage</th><th>n</th><th>p50</th><th>p95</th><th>p99</th><th>worst</th></tr></thead>
            <tbody>
              {#each perf as p, i (i)}
                <tr>
                  <td>{fmtMs(p.at_ms)}</td>
                  <td class="lib-perf-metric">{p.metric.replace(/_/g, ' ')}</td>
                  <td>{p.samples}</td>
                  <!-- A stage never reached is an ABSENCE, not a zero. Printing 0
                       here would make every service look instantaneous on the
                       stages it never performed. -->
                  <td>{p.p50_ms === null ? '—' : Math.round(p.p50_ms)}</td>
                  <td>{p.p95_ms === null ? '—' : Math.round(p.p95_ms)}</td>
                  <td>{p.p99_ms === null ? '—' : Math.round(p.p99_ms)}</td>
                  <td>{p.worst_ms === null ? '—' : Math.round(p.worst_ms)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
    {/if}
  </div>
{:else}
  <!-- ══ LIST STATE ══ -->
  <div class="lib-view">
    <div class="lib-actionbar">
      <p class="r-lead">Every processed service is recorded locally to SQLite — transcript, fired detections, and operator overrides — and read back here.</p>
      <div class="lib-actions">
        <button class="r-btn ghost" on:click={refresh} disabled={!$capture.available}>
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
          Refresh
        </button>
        <button class="r-btn danger" on:click={stopRecording} disabled={!$capture.available}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>
          End current service
        </button>
      </div>
    </div>

    {#if !$capture.available}
      <div class="lib-warn"><span class="r-badge rose"><span class="bd"></span>Backend not attached</span></div>
    {/if}

    <!-- Real-data stat cards -->
    <div class="lib-stats">
      <div class="r-stat">
        <span class="r-lbl">Total Services</span>
        <div class="n">{services.length}</div>
      </div>
      <div class="r-stat cyan">
        <span class="r-lbl">Verses Detected</span>
        <div class="n">{services.reduce((a, s) => a + (s.verses || 0), 0)}</div>
      </div>
      <div class="r-stat rose">
        <span class="r-lbl">Operator Overrides</span>
        <div class="n">{services.reduce((a, s) => a + (s.overrides || 0), 0)}</div>
      </div>
    </div>

    <!-- Column labels -->
    <div class="lib-head r-lbl">
      <span class="c-date">Date</span>
      <span class="c-title">Service Title</span>
      <span class="c-dur">Duration</span>
      <span class="c-verses">Verses</span>
      <span class="c-over">Overrides</span>
      <span class="c-open"></span>
    </div>

    <!-- Service rows (10 per page) -->
    <div class="lib-list">
      {#if services.length}
        {#each pageServices as s, i}
          {@const gi = page * PER + i}
          <div class="r-row lib-row">
            <span class="bar" style="background:{gi === 0 ? 'var(--v-accent)' : 'var(--v-line2)'};"></span>
            <span class="c-date r-mono" class:is-latest={gi === 0}>{s.date}</span>
            <span class="c-title lib-svctitle">{s.title}</span>
            <span class="c-dur r-mono">{fmtDur(s.duration_secs)}</span>
            <span class="c-verses"><span class="lib-pill r-mono">{s.verses}</span></span>
            <span class="c-over r-mono">{s.overrides}</span>
            <span class="c-open">
              <button class="r-iconbtn lib-openbtn" title="Open service" on:click={() => open(s)}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 18l6-6-6-6"/></svg>
              </button>
            </span>
          </div>
        {/each}
      {:else}
        <div class="r-row"><span class="r-empty">No services yet — press Start listening on the Live tab to record one.</span></div>
      {/if}
    </div>

    {#if pageCount > 1}
      <div class="lib-pager">
        <span class="r-mono">Showing {page * PER + 1}–{Math.min(services.length, page * PER + PER)} of {services.length}</span>
        <div class="pg">
          <button class="pgbtn" disabled={page === 0} on:click={() => (page -= 1)} aria-label="Previous page">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 18l-6-6 6-6"/></svg>
          </button>
          <span class="pgnum r-mono">Page {page + 1} / {pageCount}</span>
          <button class="pgbtn" disabled={page >= pageCount - 1} on:click={() => (page += 1)} aria-label="Next page">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18l6-6-6-6"/></svg>
          </button>
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  /* THE TIMELINE. Quiet by default; a fault reads as one. Rose is the failure
     colour — amber is never spent on this screen, because nothing here is on air. */
  .lib-tl{ margin-top:18px; }
  .lib-tl-list{ list-style:none; margin:0; padding:0; display:flex; flex-direction:column; gap:2px; }
  .lib-tl-row{ display:flex; align-items:baseline; gap:10px; padding:5px 8px;
    border-radius:var(--v-r-sm); background:var(--v-surf2); font-size:var(--v-fs-b2); }
  .lib-tl-at{ flex:0 0 52px; color:var(--v-faint); font-size:10px; }
  .lib-tl-src{ flex:0 0 68px; color:var(--v-faint); font-size:9px; letter-spacing:.06em;
    text-transform:uppercase; }
  .lib-tl-what{ flex:1; min-width:0; color:var(--v-txt); }
  .lib-tl-detail{ color:var(--v-dim); font-size:10px; overflow:hidden;
    text-overflow:ellipsis; white-space:nowrap; max-width:40%; }
  .lib-tl-row.fault{ background:color-mix(in srgb, var(--v-rose) 8%, var(--v-surf2));
    border:1px solid color-mix(in srgb, var(--v-rose) 40%, transparent); }
  .lib-tl-row.fault .lib-tl-what{ color:var(--v-rose); }
  .lib-tl-perfhead{ margin-top:16px; }
  /* The report. Numbers first, and the two that matter go rose when non-zero —
     a panic control that failed is not a statistic. */
  .lib-rep{ margin-top:18px; }
  .lib-rep-grid{ display:grid; grid-template-columns:repeat(auto-fit, minmax(120px, 1fr));
    gap:8px; margin-bottom:14px; }
  .lib-rep-cell{ background:var(--v-surf2); border:1px solid var(--v-line);
    border-radius:var(--v-r-sm); padding:9px 10px; display:flex; flex-direction:column; gap:2px; }
  .lib-rep-cell b{ font-size:var(--v-fs-h3); font-weight:600; color:var(--v-txt); }
  .lib-rep-cell span{ font-size:9px; letter-spacing:.05em; color:var(--v-faint);
    text-transform:uppercase; }
  .lib-rep-cell.bad{ border-color:color-mix(in srgb, var(--v-rose) 45%, transparent); }
  .lib-rep-cell.bad b{ color:var(--v-rose); }
  .lib-perf td.bad{ color:var(--v-rose); }
  .lib-rep-trend{ font-size:var(--v-fs-b2); color:var(--v-dim); margin-top:10px; }
  .lib-rep-trend.bad{ color:var(--v-rose); }
  .lib-rep-not{ font-size:var(--v-fs-cap); color:var(--v-faint); margin-top:12px;
    line-height:1.5; }
  /* The replay. A timeline row is a button now, so it has to keep looking like a
     row and gain a focus ring rather than a button's chrome. */
  .lib-tl-row{ width:100%; text-align:left; border:1px solid transparent; cursor:pointer;
    font:inherit; }
  .lib-tl-row:hover{ background:var(--v-surf3); }
  .lib-tl-row.open{ border-color:var(--v-line2); }
  .lib-tl-list li{ list-style:none; }
  .lib-rp{ margin:2px 0 8px 24px; padding:10px 12px; background:var(--v-surf2);
    border-left:2px solid var(--v-line2); border-radius:var(--v-r-sm); }
  .lib-rp-lines{ margin:4px 0 12px; padding-left:0; list-style:none;
    display:flex; flex-direction:column; gap:3px; font-size:var(--v-fs-b2); }
  .lib-rp-lines span{ color:var(--v-faint); margin-right:6px; font-size:10px; }
  .lib-rp-dl{ margin-bottom:10px; }
  .lib-rp-lat{ font-size:10px; color:var(--v-dim); }
  .lib-perf{ width:100%; border-collapse:collapse; font-size:10px; color:var(--v-dim); }
  .lib-perf th{ text-align:left; font-weight:500; color:var(--v-faint); padding:4px 6px;
    border-bottom:1px solid var(--v-line); }
  .lib-perf td{ padding:3px 6px; border-bottom:1px solid var(--v-line2); }
  .lib-perf-metric{ color:var(--v-txt); }

  .lib-view{ display:flex; flex-direction:column; gap:18px; max-width:1080px; }

  /* ── List: action bar ── */
  .lib-actionbar{ display:flex; align-items:flex-end; justify-content:space-between; gap:24px; flex-wrap:wrap; }
  .lib-actionbar .r-lead{ margin:0; }
  .lib-actions{ display:flex; gap:10px; flex-shrink:0; }

  .lib-warn{ margin-top:-6px; }

  /* ── Stat cards ── */
  .lib-stats{ display:grid; grid-template-columns:repeat(3, 1fr); gap:14px; }

  /* ── Table-like list ── */
  .lib-head{
    display:grid; grid-template-columns:130px 1fr 100px 84px 90px 46px;
    align-items:center; gap:16px; padding:0 18px;
  }
  .lib-list{ display:flex; flex-direction:column; gap:8px; }
  .lib-row{
    display:grid; grid-template-columns:130px 1fr 100px 84px 90px 46px;
    gap:16px; cursor:default;
  }
  .c-verses, .c-over{ text-align:center; }
  .lib-head .c-verses, .lib-head .c-over{ text-align:center; }
  .c-open{ display:flex; justify-content:flex-end; }

  .c-date{ color:var(--v-dim); font-size:12px; }
  .c-date.is-latest{ color:var(--v-accent); }
  .lib-svctitle{ font-weight:600; color:var(--v-txt); font-size:14px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .c-dur{ color:var(--v-dim); font-size:12px; }
  .c-over{ color:var(--v-dim); font-size:12px; }
  .lib-pill{
    display:inline-block; min-width:30px; text-align:center; padding:3px 9px; border-radius:99px;
    background:var(--v-cyan-soft); border:1px solid rgba(34,211,238,.32); color:var(--v-cyan); font-size:11px;
  }
  .lib-openbtn svg{ transition:transform .15s; }
  .lib-row:hover .lib-openbtn{ color:var(--v-accent); border-color:var(--v-line2); }
  .lib-row:hover .lib-openbtn svg{ transform:translateX(2px); }

  /* ── Detail ── */
  .lib-detail-top{ display:flex; align-items:center; gap:16px; flex-wrap:wrap; }
  .lib-detail-head{ display:flex; align-items:baseline; gap:12px; min-width:0; flex:1; }
  .lib-detail-title{ font-family:var(--f-head); font-size:22px; font-weight:700; color:var(--v-txt); line-height:1.1;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .lib-detail-date{ font-size:12px; color:var(--v-faint); flex-shrink:0; }
  .lib-detail-actions{ display:flex; align-items:center; gap:12px; flex-shrink:0; }
  .lib-detail-count{ font-size:11px; color:var(--v-dim); }

  .lib-exportmsg{ font-size:11px; color:var(--v-emerald); word-break:break-word; margin-top:-8px; }
  /* `r-mono` is gone: it now carries a humanised sentence, and monospace is what
     made the old raw-error dumps read like a crash to a volunteer. */
  .lib-detailerr{ font-size:12px; color:var(--v-rose); }

  .lib-detail-grid{ display:grid; grid-template-columns:1fr 340px; gap:16px; align-items:start; }
  .lib-collabel{ margin-bottom:10px; }
  .lib-collabel-n{ color:var(--v-faint); letter-spacing:0; }

  .lib-transcript{ padding:14px 16px; max-height:420px; overflow:auto; font-size:13px; line-height:1.6; }
  .lib-collabel-row{ display:flex; align-items:baseline; gap:8px; }
  .lib-collabel-row .spring{ flex:1; }
  .lib-hits{ font-size:10px; color:var(--v-faint); }
  .lib-tsearch{ margin:8px 0 10px; height:34px; font-size:12.5px; }
  .lib-ttext :global(mark){ background:var(--v-accent-soft); color:var(--v-accent2);
    border-radius:3px; padding:0 2px; }
  .lib-detect-conf.muted{ color:var(--v-faint); }

  .lib-tline{ margin-bottom:12px; }
  .lib-tline:last-child{ margin-bottom:0; }
  .lib-tmeta{ display:block; font-size:10px; color:var(--v-faint); margin-bottom:3px; }
  .lib-ttext{ color:var(--v-dim); }

  .lib-detect-list{ display:flex; flex-direction:column; gap:9px; }
  .lib-detect{ padding:12px 14px; }
  .lib-detect-top{ display:flex; align-items:center; justify-content:space-between; gap:10px; margin-bottom:8px; }
  .lib-detect-ref{ font-family:var(--f-head); font-weight:700; font-size:15px; color:var(--v-txt); }
  .lib-detect-method{ text-transform:uppercase; }
  .lib-detect-bottom{ display:flex; align-items:center; justify-content:space-between; font-size:10px; color:var(--v-faint); }
  .lib-detect-conf{ color:var(--v-accent); }

  .lib-emptytile{ padding:18px 16px; }

  /* ── Pager ── */
  .lib-pager{ display:flex; align-items:center; justify-content:space-between; gap:14px; padding:4px 6px; }
  .lib-pager .r-mono{ font-size:10.5px; color:var(--v-faint); }
  .pg{ display:flex; align-items:center; gap:8px; }
  .pgnum{ font-size:11px; color:var(--v-dim); }
  .pgbtn{ width:32px; height:32px; display:grid; place-items:center; border-radius:8px; cursor:pointer;
    background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-dim); }
  .pgbtn:hover:not(:disabled){ color:var(--v-accent); border-color:var(--v-line2); }
  .pgbtn:disabled{ opacity:.35; cursor:not-allowed; }

  /* ── Responsive ── */
  @media (max-width:820px){
    .lib-stats{ grid-template-columns:1fr; }
    .lib-detail-grid{ grid-template-columns:1fr; }
    .lib-transcript{ max-height:300px; }
    .lib-actionbar{ flex-direction:column; align-items:stretch; }
    .lib-actions{ width:100%; }
    .lib-actions .r-btn{ flex:1; }

    .lib-head{ display:none; }
    .lib-row{
      grid-template-columns:1fr auto; grid-auto-rows:auto;
      grid-template-areas:
        "title open"
        "meta  meta";
      gap:8px 12px; padding:14px 16px;
    }
    .lib-row .c-title{ grid-area:title; }
    .lib-row .c-open{ grid-area:open; align-self:start; }
    .lib-row .c-date, .lib-row .c-dur, .lib-row .c-verses, .lib-row .c-over{
      grid-area:meta; display:inline-flex; align-items:center; text-align:left;
    }
    .lib-row .c-date::before, .lib-row .c-dur::before, .lib-row .c-over::before{ content:""; }
    .lib-row{ align-items:start; }
    .lib-row .c-date, .lib-row .c-dur, .lib-row .c-verses, .lib-row .c-over{ margin-right:14px; }
  }
</style>
