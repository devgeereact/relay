<script>
  // Stage Displays — the gallery (relay-stagedisplay-screen). A grid of every
  // output surface + a settings rail for the selected one. Previews render the
  // display's real layout (TemplateRender) for scripture displays, and a bespoke
  // live-content preview for timer / countdown / confidence / lyrics displays.
  import { createEventDispatcher } from 'svelte';
  import TemplateRender from '../../TemplateRender.svelte';
  import { displays, updateDisplay, addDisplay, removeDisplay, DISPLAY_TYPES, RESOLUTIONS } from '../../stores/stagedisplays.js';

  const dispatch = createEventDispatcher();

  const SAMPLE = { text: 'In the beginning God created the heaven and the earth.', reference: 'Genesis 1:1 · KJV', translation: 'KJV' };

  let q = '';
  let grid = true; // grid | list view toggle
  let selId = null;
  $: list = $displays.filter((d) => !q.trim() || (d.name + ' ' + d.type).toLowerCase().includes(q.trim().toLowerCase()));
  $: activeCount = $displays.filter((d) => d.status === 'live').length;
  $: if (selId === null && $displays.length) selId = $displays[0].id;
  $: sel = $displays.find((d) => d.id === selId) || null;

  let tab = 'general'; // general | layout | content | advanced
  let quickMsg = '';
  function quick(msg) { quickMsg = msg; setTimeout(() => (quickMsg = ''), 1800); }

  function pickRes(label) {
    const r = RESOLUTIONS.find((x) => x.label === label);
    if (r && sel) updateDisplay(sel.id, { res: r.res });
  }
  $: resLabel = sel ? (RESOLUTIONS.find((r) => r.res[0] === sel.res[0] && r.res[1] === sel.res[1])?.label ?? `${sel.res[0]} × ${sel.res[1]}`) : '';

  function newDisplay() {
    const id = addDisplay();
    selId = id;
  }
  function del(id) {
    if (!confirm('Delete this display? Its layout is removed from this machine.')) return;
    removeDisplay(id);
    selId = null;
  }

  const ICONS = {
    monitor: '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>',
    gauge: '<path d="M12 14l4-4"/><path d="M4 18a8 8 0 1 1 16 0"/><circle cx="12" cy="18" r="1"/>',
    eye: '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/>',
    music: '<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>',
    timer: '<circle cx="12" cy="13" r="8"/><path d="M12 9v4l2 2M9 2h6"/>',
    ring: '<circle cx="12" cy="12" r="9"/><path d="M12 3a9 9 0 0 1 6.4 15.3"/>',
    clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
  };
</script>

<div class="dg">
  <!-- ══ TOOLBAR ══ -->
  <div class="dg-toolbar">
    <div class="dg-tl">
      <span class="dg-h2">All Displays</span>
      <span class="dg-count r-mono">{activeCount} Active</span>
    </div>
    <div class="dg-tr">
      <div class="dg-search">
        <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M21 21l-4-4"/></svg>
        <input class="dg-searchin" placeholder="Search displays…" bind:value={q} />
      </div>
      <button class="r-btn ghost sm"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>Group</button>
      <div class="dg-vtoggle">
        <button class="dg-vbtn" class:on={grid} on:click={() => (grid = true)} aria-label="Grid view"><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg></button>
        <button class="dg-vbtn" class:on={!grid} on:click={() => (grid = false)} aria-label="List view"><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/></svg></button>
      </div>
      <button class="r-btn primary sm" on:click={newDisplay}>
        <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
        Add Display
      </button>
    </div>
  </div>

  <div class="dg-body">
    <!-- ══ CARD GRID ══ -->
    <div class="dg-grid" class:list={!grid}>
      {#each list as d (d.id)}
        <div class="dg-card" class:sel={selId === d.id} role="button" tabindex="0"
          on:click={() => (selId = d.id)}
          on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selId = d.id; } }}>
          <div class="dg-chead">
            <span class="dg-cico"><svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html ICONS[d.icon] || ICONS.monitor}</svg></span>
            <span class="dg-ctext">
              <span class="dg-cname">{d.name}{#if d.primary}<span class="dg-primary">PRIMARY</span>{/if}</span>
              <span class="dg-csub">{d.subtitle}</span>
            </span>
            <span class="r-badge" class:emerald={d.status === 'live'} class:grey={d.status !== 'live'}><span class="bd" style="box-shadow:none;"></span>{d.status === 'live' ? 'LIVE' : 'OFFLINE'}</span>
          </div>

          <div class="dg-preview">
            {#if d.preview === 'scripture'}
              <TemplateRender template={d.template} content={SAMPLE} />
            {:else if d.preview === 'confidence'}
              <div class="pv pv-conf">
                <div class="pv-lbl r-lbl">Confidence Score</div>
                <div class="pv-big r-mono">92%</div>
                <svg class="pv-spark" viewBox="0 0 200 54" preserveAspectRatio="none"><polyline points="0,44 22,40 44,42 66,30 88,34 110,20 132,26 154,12 176,16 200,8" fill="none" stroke="var(--v-emerald)" stroke-width="2"/><circle cx="200" cy="8" r="3" fill="var(--v-emerald)"/></svg>
                <div class="pv-foot"><span class="r-lbl" style="color:var(--v-emerald)">Strong match</span><span class="pv-ref">Genesis 1:1</span></div>
              </div>
            {:else if d.preview === 'lyrics'}
              <div class="pv pv-lyr">
                <div class="pv-song">Amazing Grace</div>
                <div class="pv-meta r-mono">Key: G · BPM 72 · 4/4</div>
                <div class="pv-verse r-lbl">[Verse 1]</div>
                <div class="pv-lines">
                  <div><span>Amazing grace how sweet the sound</span><b>G</b></div>
                  <div><span>That saved a wretch like me</span><b>C</b></div>
                  <div><span>I once was lost but now am found</span><b>G</b></div>
                  <div><span>Was blind but now I see</span><b>G</b></div>
                </div>
              </div>
            {:else if d.preview === 'timer'}
              <div class="pv pv-timer">
                <div class="pv-trow"><span class="r-lbl">Service Timer</span><span class="pv-tbig r-mono">00:32:45</span></div>
                <div class="pv-titem"><span class="r-lbl">Current Item</span><span class="pv-tname">Sermon</span><span class="pv-tdur r-mono">30:00</span></div>
                <div class="pv-titem"><span class="r-lbl">Next Item</span><span class="pv-tname" style="color:var(--v-amber)">Response Song</span><span class="pv-tdur r-mono">05:00</span></div>
              </div>
            {:else if d.preview === 'countdown'}
              <div class="pv pv-count">
                <svg class="pv-ring" viewBox="0 0 120 120"><circle cx="60" cy="60" r="52" fill="none" stroke="var(--v-surf3)" stroke-width="7"/><circle cx="60" cy="60" r="52" fill="none" stroke="var(--v-amber)" stroke-width="7" stroke-linecap="round" stroke-dasharray="327" stroke-dashoffset="90" transform="rotate(-90 60 60)"/></svg>
                <div class="pv-cinner"><span class="r-lbl">Next item in</span><span class="pv-cbig r-mono">05:00</span></div>
              </div>
            {:else}
              <div class="pv pv-clock"><span class="pv-clockbig r-mono">05:00</span></div>
            {/if}
          </div>

          <div class="dg-meta">
            <span class="dg-mi r-mono"><svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="5" width="18" height="12" rx="1"/></svg>{d.res[0]} × {d.res[1]}</span>
            <span class="dg-mi r-mono"><svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="8"/><path d="M12 8v4l2 2" stroke-linecap="round"/></svg>{d.fps} FPS</span>
            <span class="dg-mi r-mono"><svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="6" width="18" height="12" rx="1"/></svg>16:9</span>
          </div>

          <div class="dg-cbtns">
            <button class="dg-cgear" title="Settings" on:click|stopPropagation={() => { selId = d.id; }} aria-label="Settings"><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M19.1 4.9L17 7M7 17l-2.1 2.1"/></svg></button>
            <button class="r-btn ghost sm dg-edit" on:click={() => dispatch('edit', { id: d.id })}>Edit Layout</button>
            <button class="dg-cmore" title="Delete display" on:click={() => del(d.id)} aria-label="Delete display"><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/></svg></button>
          </div>
        </div>
      {/each}
      {#if !list.length}<div class="r-empty dg-empty">No displays match “{q}”.</div>{/if}
    </div>

    <!-- ══ DISPLAY SETTINGS RAIL ══ -->
    {#if sel}
      <aside class="dg-rail">
        <div class="dg-railhead r-lbl">Display Settings</div>
        <div class="dg-tabs">
          {#each ['general', 'layout', 'content', 'advanced'] as t}
            <button class="dg-tab" class:on={tab === t} on:click={() => (tab = t)}>{t[0].toUpperCase() + t.slice(1)}</button>
          {/each}
        </div>

        <div class="dg-railbody r-scroll">
          {#if tab === 'general'}
            <div class="dg-frow"><span class="dg-fk">Display Name</span><input class="r-input dg-fv" value={sel.name} on:input={(e) => updateDisplay(sel.id, { name: e.target.value })} /></div>
            <div class="dg-frow"><span class="dg-fk">Display Type</span>
              <select class="r-select dg-fv" value={sel.type} on:change={(e) => updateDisplay(sel.id, { type: e.target.value })}>
                {#each DISPLAY_TYPES as t}<option value={t}>{t}</option>{/each}
              </select>
            </div>
            <div class="dg-frow"><span class="dg-fk">Status</span><span class="dg-fv dg-status"><span class="dg-sdot" class:live={sel.status === 'live'}></span>{sel.status === 'live' ? 'Live' : 'Offline'}</span></div>
            <div class="dg-frow"><span class="dg-fk">Resolution</span>
              <select class="r-select dg-fv" value={resLabel} on:change={(e) => pickRes(e.target.value)}>
                {#each RESOLUTIONS as r}<option value={r.label}>{r.label}</option>{/each}
                {#if !RESOLUTIONS.some((r) => r.label === resLabel)}<option value={resLabel}>{resLabel}</option>{/if}
              </select>
            </div>
            <div class="dg-frow"><span class="dg-fk">Refresh Rate</span>
              <select class="r-select dg-fv" value={sel.fps} on:change={(e) => updateDisplay(sel.id, { fps: +e.target.value })}>
                {#each [24, 30, 50, 60] as f}<option value={f}>{f} FPS</option>{/each}
              </select>
            </div>
            <div class="dg-frow"><span class="dg-fk">Connection</span><span class="dg-fv dg-conn r-mono">{sel.connection}<span class="dg-connok">Connected</span></span></div>
            <div class="dg-frow"><span class="dg-fk">Colour Profile</span>
              <select class="r-select dg-fv" value={sel.colour} on:change={(e) => updateDisplay(sel.id, { colour: e.target.value })}>
                {#each ['Rec. 709', 'sRGB', 'Display P3', 'Rec. 2020'] as c}<option value={c}>{c}</option>{/each}
              </select>
            </div>

            <div class="dg-railsec r-lbl">Preview</div>
            <div class="dg-prevbox">
              <div class="dg-prevrender">
                {#if sel.preview === 'scripture'}<TemplateRender template={sel.template} content={SAMPLE} />{:else}<div class="dg-prevtype r-mono">{sel.type}</div>{/if}
              </div>
              <div class="dg-vu">
                {#each Array(16) as _, i}<i class:hot={i > 12} class:warm={i > 9 && i <= 12} style="height:{20 + i * 5}%"></i>{/each}
              </div>
            </div>
            <button class="r-btn ghost sm dg-prevbtn" on:click={() => quick('Preview opened on the output.')}>Preview Output</button>

            <div class="dg-railsec r-lbl">Quick Actions</div>
            <div class="dg-qa">
              <button class="dg-qbtn" on:click={() => quick('Test pattern sent.')}><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7"><rect x="3" y="4" width="18" height="14" rx="2"/><path d="M3 11h18M12 4v14" stroke-linecap="round"/></svg>Send Test Pattern</button>
              <button class="dg-qbtn" on:click={() => quick('Screen blacked.')}><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7"><rect x="3" y="4" width="18" height="14" rx="2"/></svg>Black Screen</button>
              <button class="dg-qbtn" on:click={() => quick('Content cleared.')}><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14"/></svg>Clear Content</button>
              <button class="dg-qbtn" on:click={() => quick('Output restarted.')}><svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.5 9a9 9 0 0 1 14.9-3.4L23 10M1 14l4.6 4.4A9 9 0 0 0 20.5 15"/></svg>Restart Output</button>
            </div>
            {#if quickMsg}<div class="dg-qmsg r-mono">{quickMsg}</div>{/if}

          {:else if tab === 'layout'}
            <p class="dg-note">Open the full layer editor to move, style and add elements on this display.</p>
            <button class="r-btn primary sm" on:click={() => dispatch('edit', { id: sel.id })}>Edit Layout</button>
            <div class="dg-railsec r-lbl">Current Layout</div>
            <div class="dg-prevbox"><div class="dg-prevrender">{#if sel.preview === 'scripture'}<TemplateRender template={sel.template} content={SAMPLE} />{:else}<div class="dg-prevtype r-mono">{sel.type}</div>{/if}</div></div>

          {:else if tab === 'content'}
            <p class="dg-note">This display shows <b>{sel.preview === 'scripture' ? 'fired scripture' : sel.type.toLowerCase()}</b>. Scripture displays follow the live verse; timer / countdown displays run on the service clock.</p>
            <div class="dg-frow"><span class="dg-fk">Source</span><span class="dg-fv dg-conn r-mono">{sel.preview === 'scripture' ? 'Live detection + operator' : 'Service clock'}</span></div>

          {:else}
            <div class="dg-frow"><span class="dg-fk">Display ID</span><span class="dg-fv r-mono" style="font-size:11px">{sel.id}</span></div>
            <div class="dg-frow"><span class="dg-fk">Primary</span><button class="s-toggle-mini" class:on={sel.primary} on:click={() => updateDisplay(sel.id, { primary: !sel.primary })}><span></span></button></div>
            <div class="dg-frow"><span class="dg-fk">Status</span>
              <select class="r-select dg-fv" value={sel.status} on:change={(e) => updateDisplay(sel.id, { status: e.target.value })}>
                <option value="live">Live</option><option value="offline">Offline</option>
              </select>
            </div>
            <p class="dg-note">Outputs are wired to real screens (HDMI · NDI · kiosk) in the <b>Channels</b> tab. This panel configures the display surface itself.</p>
          {/if}
        </div>

        <button class="dg-del" on:click={() => del(sel.id)}>
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/></svg>
          Delete Display
        </button>
      </aside>
    {/if}
  </div>
</div>

<style>
  .dg{ display:flex; flex-direction:column; gap:16px; height:100%; min-height:0; }

  /* toolbar */
  .dg-toolbar{ display:flex; align-items:center; justify-content:space-between; gap:16px; flex-wrap:wrap; }
  .dg-tl{ display:flex; align-items:center; gap:12px; }
  .dg-h2{ font-family:var(--f-head); font-size:var(--v-fs-h2); font-weight:600; color:var(--v-txt); }
  .dg-count{ font-size:10px; letter-spacing:.05em; color:var(--v-accent2); background:var(--v-accent-soft);
    border:1px solid var(--v-accent-line); border-radius:99px; padding:3px 9px; }
  .dg-tr{ display:flex; align-items:center; gap:10px; }
  .dg-search{ display:flex; align-items:center; gap:8px; height:34px; padding:0 12px; border-radius:var(--v-r-md);
    background:var(--v-bg); border:1px solid var(--v-line2); color:var(--v-faint); }
  .dg-searchin{ border:0; background:none; outline:none; color:var(--v-txt); font-size:12.5px; width:180px; }
  .dg-vtoggle{ display:flex; gap:2px; padding:3px; border-radius:var(--v-r-md); background:var(--v-bg); border:1px solid var(--v-line); }
  .dg-vbtn{ width:30px; height:26px; display:grid; place-items:center; border:0; border-radius:var(--v-r-sm); background:none; color:var(--v-faint); cursor:pointer; }
  .dg-vbtn.on{ background:var(--v-surf3); color:var(--v-txt); }

  .dg-body{ flex:1; min-height:0; display:grid; grid-template-columns:minmax(0,1fr) 316px; gap:18px; align-items:start; }

  /* grid */
  .dg-grid{ display:grid; grid-template-columns:repeat(auto-fill, minmax(300px, 1fr)); gap:16px; align-content:start;
    max-height:100%; overflow-y:auto; padding-right:4px; }
  .dg-grid.list{ grid-template-columns:1fr; }
  .dg-card{ display:flex; flex-direction:column; text-align:left; cursor:pointer; padding:0; overflow:hidden;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); transition:border-color .13s; }
  .dg-card:hover{ border-color:var(--v-line2); }
  .dg-card.sel{ border-color:var(--v-accent-line); box-shadow:0 0 0 1px var(--v-accent-line); }
  .dg-chead{ display:flex; align-items:center; gap:11px; padding:14px 15px 11px; }
  .dg-cico{ width:34px; height:34px; flex:0 0 auto; display:grid; place-items:center; border-radius:var(--v-r-md);
    background:var(--v-accent-soft); color:var(--v-accent2); }
  .dg-ctext{ flex:1; min-width:0; display:flex; flex-direction:column; gap:2px; }
  .dg-cname{ display:flex; align-items:center; gap:8px; font-size:14px; font-weight:600; color:var(--v-txt); }
  .dg-primary{ font-family:var(--f-mono); font-size:8px; font-weight:700; letter-spacing:.08em; color:var(--v-amethyst2);
    background:var(--v-amethyst-soft); border-radius:3px; padding:2px 5px; }
  .dg-csub{ font-size:11.5px; color:var(--v-faint); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .dg-preview{ position:relative; aspect-ratio:16/9; margin:0 15px; border-radius:var(--v-r-md); overflow:hidden;
    border:1px solid var(--v-line); background:var(--v-void); }
  .dg-meta{ display:flex; gap:16px; padding:11px 16px; }
  .dg-mi{ display:inline-flex; align-items:center; gap:5px; font-size:10.5px; color:var(--v-faint); }
  .dg-cbtns{ display:flex; align-items:center; gap:8px; padding:0 15px 14px; }
  .dg-cgear, .dg-cmore{ width:34px; height:34px; flex:0 0 auto; display:grid; place-items:center; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-dim); cursor:pointer; }
  .dg-cgear:hover, .dg-cmore:hover{ color:var(--v-txt); border-color:var(--v-line2); }
  .dg-cmore:hover{ color:var(--v-red); }
  .dg-edit{ flex:1; justify-content:center; }
  .dg-empty{ grid-column:1/-1; padding:40px; }

  /* bespoke previews */
  .pv{ position:absolute; inset:0; padding:16px 18px; display:flex; flex-direction:column; }
  .pv-conf .pv-big{ font-size:34px; font-weight:600; color:var(--v-txt); margin-top:2px; }
  .pv-conf .pv-lbl{ color:var(--v-faint); }
  .pv-spark{ flex:1; width:100%; margin:8px 0; }
  .pv-foot{ display:flex; align-items:baseline; justify-content:space-between; }
  .pv-ref{ font-family:var(--f-head); font-size:14px; font-weight:600; color:var(--v-txt); }
  .pv-lyr .pv-song{ font-size:15px; font-weight:700; color:var(--v-txt); }
  .pv-meta{ font-size:10px; color:var(--v-faint); margin:2px 0 8px; }
  .pv-verse{ color:var(--v-amber); margin-bottom:4px; }
  .pv-lines{ display:flex; flex-direction:column; gap:3px; font-size:11.5px; color:var(--v-dim); }
  .pv-lines div{ display:flex; justify-content:space-between; gap:10px; }
  .pv-lines b{ font-family:var(--f-mono); color:var(--v-amber); }
  .pv-timer{ justify-content:center; gap:12px; }
  .pv-trow{ display:flex; flex-direction:column; gap:2px; }
  .pv-tbig{ font-size:28px; font-weight:600; color:var(--v-txt); }
  .pv-titem{ display:flex; align-items:baseline; gap:8px; padding-top:8px; border-top:1px solid var(--v-line); }
  .pv-titem .r-lbl{ flex:0 0 auto; }
  .pv-tname{ flex:1; font-size:13px; font-weight:600; color:var(--v-txt); }
  .pv-tdur{ font-size:12px; color:var(--v-dim); }
  .pv-count{ align-items:center; justify-content:center; }
  .pv-ring{ width:118px; height:118px; }
  .pv-cinner{ position:absolute; inset:0; display:flex; flex-direction:column; align-items:center; justify-content:center; gap:2px; }
  .pv-cbig{ font-size:26px; font-weight:600; color:var(--v-txt); }
  .pv-clock{ align-items:center; justify-content:center; }
  .pv-clockbig{ font-size:52px; font-weight:600; color:var(--v-txt); }

  /* settings rail */
  .dg-rail{ position:sticky; top:0; display:flex; flex-direction:column; min-height:0; max-height:100%;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); overflow:hidden; }
  .dg-railhead{ padding:14px 16px 0; }
  .dg-tabs{ display:flex; gap:2px; padding:10px 16px 0; border-bottom:1px solid var(--v-line); }
  .dg-tab{ padding:8px 10px; border:0; background:none; color:var(--v-faint); font-size:12.5px; font-weight:500;
    cursor:pointer; border-bottom:2px solid transparent; margin-bottom:-1px; }
  .dg-tab:hover{ color:var(--v-txt); }
  .dg-tab.on{ color:var(--v-accent2); border-bottom-color:var(--v-accent); }
  .dg-railbody{ flex:1; min-height:0; overflow-y:auto; padding:16px; display:flex; flex-direction:column; gap:12px; }

  .dg-frow{ display:grid; grid-template-columns:100px minmax(0,1fr); align-items:center; gap:10px; }
  .dg-fk{ font-size:12.5px; color:var(--v-dim); }
  .dg-fv{ min-width:0; }
  .dg-status{ display:inline-flex; align-items:center; gap:7px; font-size:13px; color:var(--v-txt); }
  .dg-sdot{ width:8px; height:8px; border-radius:50%; background:var(--v-faint); }
  .dg-sdot.live{ background:var(--v-emerald); box-shadow:0 0 7px var(--v-emerald); }
  .dg-conn{ font-size:12px; color:var(--v-txt); display:inline-flex; align-items:center; gap:8px; }
  .dg-connok{ font-size:10px; color:var(--v-amethyst2); }
  .dg-railsec{ margin-top:8px; padding-top:14px; border-top:1px solid var(--v-line); }

  .dg-prevbox{ display:flex; gap:10px; }
  .dg-prevrender{ position:relative; flex:1; aspect-ratio:16/9; border-radius:var(--v-r-md); overflow:hidden;
    border:1px solid var(--v-line2); background:var(--v-void); }
  .dg-prevtype{ position:absolute; inset:0; display:grid; place-items:center; font-size:11px; color:var(--v-faint); }
  .dg-vu{ flex:0 0 22px; display:flex; align-items:flex-end; gap:2px; padding:2px 0; }
  .dg-vu i{ flex:1; background:var(--v-emerald); border-radius:1px; opacity:.85; }
  .dg-vu i.warm{ background:var(--v-amber); }
  .dg-vu i.hot{ background:var(--v-red); }
  .dg-prevbtn{ width:100%; justify-content:center; }

  .dg-qa{ display:grid; grid-template-columns:1fr 1fr; gap:8px; }
  .dg-qbtn{ display:flex; align-items:center; gap:8px; padding:10px 11px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-dim); font-size:12px; cursor:pointer;
    transition:border-color .13s, color .13s; }
  .dg-qbtn:hover{ color:var(--v-txt); border-color:var(--v-line2); }
  .dg-qmsg{ font-size:10.5px; color:var(--v-emerald); }
  .dg-note{ margin:0; font-size:12px; line-height:1.6; color:var(--v-dim); }
  .dg-note b{ color:var(--v-accent); }

  .s-toggle-mini{ position:relative; width:38px; height:20px; border-radius:99px; border:1px solid var(--v-line2);
    background:var(--v-surf3); cursor:pointer; padding:0; }
  .s-toggle-mini.on{ background:var(--v-accent-fill); border-color:var(--v-accent-fill); }
  .s-toggle-mini span{ position:absolute; top:1px; left:1px; width:16px; height:16px; border-radius:50%; background:#fff; transition:transform .15s; }
  .s-toggle-mini.on span{ transform:translateX(18px); }

  .dg-del{ flex:0 0 auto; display:flex; align-items:center; justify-content:center; gap:8px; margin:0; padding:13px;
    border:0; border-top:1px solid var(--v-red-soft); background:var(--v-red-soft); color:var(--v-red);
    font-size:13px; font-weight:600; cursor:pointer; }
  .dg-del:hover{ background:rgba(239,68,68,.18); }

  @media (max-width:1100px){
    .dg-body{ grid-template-columns:1fr; }
    .dg-rail{ position:static; max-height:none; }
  }
</style>
