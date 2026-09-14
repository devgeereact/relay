<script>
  // THE WORKSPACE FRAME — the grammar every non-Live workspace is laid out in
  // (docs/REBRAND.md §2 and §11).
  //
  // The shell became a control room: a 34px chrome bar with the workspaces in it,
  // a dock row, a 26px status bar. Underneath it, three workspaces were still
  // three unrelated designs — a pill nav over a two-column table, a three-column
  // grid with 16px gutters, and a long document of bordered cards. This file is
  // the one answer to "what does a workspace body look like", so the three cannot
  // drift apart again:
  //
  //   a left RAIL (what there is), the MAIN area (the thing in hand), and an
  //   INSPECTOR (what is true of the thing in hand) — panes with hairline seams
  //   and 8px gutters, the same measurements the studio split on Live uses.
  //
  // ── One type scale, three roles ───────────────────────────────────────────
  //
  // §11 asks for page title / standfirst / row, with a footnote behind a
  // hairline. They live here as `.rw-h1`, `.rw-lead`, `.rw-nv*` and `.rw-foot`
  // rather than as three private copies, because a type scale that is defined
  // three times is three type scales. Everything is a token: the rebrand's
  // departure notes record that a hand-picked muted grey failed WCAG AA on two
  // of the four surfaces it sat on, and the only reason that was caught is that
  // nothing here is allowed to be a literal.
  //
  // The rules are `:global()` on purpose. Slotted content belongs to the caller's
  // stylesheet, so a scoped rule here would style the frame's own markup and
  // nothing an operator can see. Every class carries the `rw-` prefix so the
  // leak is a namespace rather than a surprise — and because this is a .svelte
  // file, `tokencontrast.test.js` still walks it. (A shared .css file would have
  // put the whole grammar outside that scanner's reach, which is how an
  // instrument quietly stops covering the thing it was written for.)

  /**
   * The page title — role one of the three.
   *
   * OPTIONAL, and the empty case renders NOTHING rather than an empty `<h1>`.
   * Outputs is the workspace that wanted this: its rail already says `OUTPUTS`
   * and its pane head already says `SCREENS`, so the page title was the same
   * word a third time above 110px of chrome, and a desk that starts 110px lower
   * than the prototype is most of why the app did not read as the prototype. An
   * empty heading is also a real a11y defect — a screen reader announces "heading
   * level 1" and then nothing.
   */
  export let title = '';
  /** The standfirst: one sentence about what this workspace is FOR. Role two. */
  export let standfirst = '';
  /** The body's column track. Rail · main · inspector, in that order. */
  export let columns = '206px minmax(0,1fr) 286px';
</script>

<div class="rw" style="--rw-cols:{columns}">
  <header class="rw-head">
    {#if title || standfirst}
      <div class="rw-headtext">
        {#if title}<h1 class="rw-h1">{title}</h1>{/if}
        {#if standfirst}<p class="rw-lead">{standfirst}</p>{/if}
      </div>
    {/if}
    <slot name="head" />
  </header>
  <div class="rw-body"><slot /></div>
</div>

<style>
  .rw{ display:flex; flex-direction:column; gap:var(--v-sp-sm);
    height:100%; min-height:0; color:var(--v-txt); font-family:var(--f-body); }

  /* ── ROLE 1 + 2 · the page head ─────────────────────────────────────────── */
  .rw-head{ display:flex; align-items:flex-start; gap:var(--v-sp-md); flex:0 0 auto; }
  .rw-headtext{ min-width:0; flex:1; }
  .rw-h1{ margin:0; font-family:var(--f-head); font-size:var(--v-fs-h1); line-height:var(--v-lh-h1);
    font-weight:600; letter-spacing:var(--v-tr-tight); color:var(--v-txt); }
  /* The standfirst is ONE sentence and it is measured: past ~74 characters a line
     stops being read and starts being skimmed, which is the opposite of what a
     sentence saying "nothing here reaches an output" is for. */
  .rw-lead{ margin:3px 0 0; font-size:var(--v-fs-b2); line-height:var(--v-lh-b2);
    color:var(--v-dim); max-width:74ch; }

  .rw-body{ flex:1; min-height:0; display:grid; grid-template-columns:var(--rw-cols);
    gap:var(--v-sp-sm); align-items:stretch; }

  /* ── PANES ──────────────────────────────────────────────────────────────── */
  /* The same measurements as Live's `.pane`, deliberately: an operator moving
     between the run surface and the desk should not be able to tell that two
     people built them. */
  :global(.rw-pane){ display:flex; flex-direction:column; min-height:0; overflow:hidden;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg);
    box-shadow:var(--v-shadow-sm); }
  :global(.rw-panehead){ flex:0 0 auto; display:flex; align-items:center; gap:var(--v-sp-sm);
    min-height:34px; padding:0 12px; border-bottom:1px solid var(--v-line); }
  :global(.rw-panettl){ margin:0; min-width:0; font-family:var(--f-head); font-size:var(--v-fs-cap);
    line-height:var(--v-lh-lbl); font-weight:600; letter-spacing:.06em; text-transform:uppercase;
    color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  :global(.rw-panehead .rw-spring){ flex:1; }
  :global(.rw-panebody){ flex:1; min-height:0; overflow-y:auto;
    scrollbar-width:thin; scrollbar-color:var(--v-surf3) transparent; }
  /* `pad` is opt-in. A pane full of seamed rows must reach both edges or the
     seams stop short of the border and read as a mistake. */
  :global(.rw-panebody.pad){ padding:12px; }
  :global(.rw-panefoot){ flex:0 0 auto; display:flex; flex-direction:column; gap:6px;
    padding:10px 12px; border-top:1px solid var(--v-line); }

  /* ── ROLE 3 · a row is a NAME and a VALUE ───────────────────────────────── */
  /* Hairline seams, not gutters. Cards down a page put a 12px trench between two
     facts that belong to one another; a seam says "same list, next item" in one
     pixel and gives the width back to the words. */
  :global(.rw-nv){ display:grid; grid-template-columns:minmax(0,1fr) auto; align-items:center;
    gap:var(--v-sp-md); padding:9px 12px; border-bottom:1px solid var(--v-line); }
  :global(.rw-nv:last-child){ border-bottom:0; }
  :global(.rw-nvk){ min-width:0; font-size:var(--v-fs-b2); line-height:var(--v-lh-b2);
    font-weight:500; color:var(--v-txt); }
  /* Every figure is mono so a value that changes cannot reflow the name beside
     it (§1). Right-aligned, so a column of values has one edge to read down. */
  :global(.rw-nvv){ font-family:var(--f-mono); font-size:var(--v-fs-mono); line-height:var(--v-lh-mono);
    font-variant-numeric:tabular-nums; color:var(--v-txt); text-align:right; justify-self:end; }
  :global(.rw-nvnote){ margin:2px 0 0; font-size:var(--v-fs-cap); line-height:var(--v-lh-cap);
    color:var(--v-faint); }
  /* A row whose control is wider than a figure — a select, a segmented control.
     It keeps the same seam and the same left edge; only the right half grows. */
  :global(.rw-nvctl){ justify-self:end; flex:0 0 auto; min-width:0; }

  /* A group heading inside a pane. Mono caption, uppercase, on the darker ground
     so it reads as furniture rather than as a row. */
  :global(.rw-group){ position:sticky; top:0; z-index:1; padding:7px 12px 6px;
    background:var(--v-bg); border-bottom:1px solid var(--v-line);
    font-family:var(--f-mono); font-size:var(--v-fs-cap); line-height:var(--v-lh-cap);
    font-weight:600; letter-spacing:var(--v-tr-caps); text-transform:uppercase; color:var(--v-faint); }

  /* ── RAIL ROWS ──────────────────────────────────────────────────────────── */
  /* One dense row per thing. Selection is steel blue (`--v-sel`) and nothing
     else: amber means the congregation is looking at it, amethyst means
     rehearsal, and a row you have merely clicked on is neither. */
  :global(.rw-item){ position:relative; display:flex; align-items:center; gap:9px; width:100%;
    min-height:34px; padding:7px 12px; text-align:left; cursor:pointer;
    background:transparent; border:0; border-bottom:1px solid var(--v-line);
    color:var(--v-dim); font-family:var(--f-body); font-size:var(--v-fs-b2);
    line-height:var(--v-lh-b2); font-weight:500;
    transition:background var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease); }
  :global(.rw-item:last-child){ border-bottom:0; }
  :global(.rw-item:hover:not(.on)){ background:var(--v-surf2); color:var(--v-txt); }
  :global(.rw-item.on){ background:var(--v-sel-soft); color:var(--v-txt); font-weight:600;
    box-shadow:inset 2px 0 0 var(--v-sel); }
  :global(.rw-itemname){ flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* The count beside a rail row. A figure, so: mono, dim, and NOT a pill —
     §1 is explicit that a pill in a control room reads as a toy. */
  :global(.rw-itemn){ flex:0 0 auto; font-family:var(--f-mono); font-size:var(--v-fs-cap);
    font-variant-numeric:tabular-nums; color:var(--v-faint); }
  :global(.rw-item.on .rw-itemn){ color:var(--v-dim); }

  /* ── THE FOOTNOTE, behind a hairline (§11) ──────────────────────────────── */
  /* What this surface cannot tell you, what it will not do, where the real
     answer lives. Below the rule, so it reads as an aside rather than as one
     more row of the list above it. */
  :global(.rw-foot){ margin:12px 0 0; padding-top:10px; border-top:1px solid var(--v-line);
    font-size:var(--v-fs-cap); line-height:1.55; color:var(--v-faint); }
  :global(.rw-foot b){ color:var(--v-dim); font-weight:600; }

  /* ── responsive ─────────────────────────────────────────────────────────── */
  /* The inspector goes first: it is derived from the main area, so it is the one
     column whose absence costs nothing an operator cannot get back by looking at
     what is selected. The rail goes second. */
  @media (max-width:1240px){
    .rw-body{ grid-template-columns:186px minmax(0,1fr); }
    .rw-body :global(> .rw-insp){ display:none; }
  }
  @media (max-width:900px){
    .rw{ height:auto; }
    .rw-body{ grid-template-columns:1fr; }
    .rw-body :global(> .rw-insp){ display:flex; }
  }
</style>
