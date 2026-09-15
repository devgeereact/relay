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
   * The page title — role one of the three, and an EYEBROW rather than a display
   * heading.
   *
   * ── Why it stopped being 24px ─────────────────────────────────────────────
   *
   * The prototype has no page title on any workspace: the chrome tab says where
   * you are and each pane head says what that pane is. Outputs had already taken
   * that answer and dropped its title block, while Templates, Planner, Library
   * and Settings still opened with an H1 over a two-line standfirst — so the app
   * read as two different products depending on the tab, which is the exact
   * drift this file exists to prevent.
   *
   * Both halves of the prototype's reasoning are right and one of them has an
   * exception:
   *
   *   · On four workspaces the H1 was the WORKSPACE'S OWN NAME — `Templates`, in
   *     24px, directly under the `Templates` tab you had just pressed. The same
   *     word twice, costing ~28px of grid in a room where the grid is the job.
   *   · On SETTINGS it is the active SECTION (`Diagnostics`, `Screens & looks`),
   *     which is the one thing here the chrome tab genuinely cannot say. Deleting
   *     it would take information away rather than stop repeating it.
   *
   * So the title is not deleted and it is not a display heading: it is drawn as a
   * mono caption on the same band as the standfirst and the head slot — the same
   * instrument a pane head and a dock card already use for exactly this job. It
   * remains a real `<h1>`, so heading navigation still lands on it and a screen
   * reader still hears Settings' section name.
   *
   * OPTIONAL, and the empty case renders NOTHING rather than an empty `<h1>` — a
   * screen reader announcing "heading level 1" and then nothing is a real defect,
   * and Outputs has no title to give.
   */
  export let title = '';
  /** The standfirst: one sentence about what this workspace is FOR. Role two.
   *  One line on the band, truncated, with the whole sentence in its `title`. */
  export let standfirst = '';
  /** The body's column track. Rail · main · inspector, in that order. */
  export let columns = 'var(--v-rail) minmax(0,1fr) var(--v-insp)';
</script>

<div class="rw" style="--rw-cols:{columns}">
  <!-- ONE ROW, on every workspace. The title is an EYEBROW beside the sentence,
       not a display heading above it — see the note on `title`. It is still an
       `<h1>`, so heading navigation still lands here; only its size changed. -->
  <header class="rw-head">
    {#if title || standfirst}
      <div class="rw-headtext">
        {#if title}<h1 class="rw-h1">{title}</h1>{/if}
        {#if standfirst}<p class="rw-lead" title={standfirst}>{standfirst}</p>{/if}
      </div>
    {/if}
    <slot name="head" />
  </header>
  <div class="rw-body"><slot /></div>
</div>

<style>
  .rw{ display:flex; flex-direction:column; gap:var(--v-sp-sm);
    height:100%; min-height:0; color:var(--v-txt); font-family:var(--f-body); }

  /* ── ROLE 1 + 2 · the page head, which is now ONE ROW ───────────────────── */
  /* Six workspaces had drifted into two products: Outputs had dropped its title
     block entirely while Templates, Planner, Library and Settings still opened
     with a 24px H1 over a two-line standfirst. The head is the same shape on all
     six now, and the shape is a BAND: eyebrow, sentence, controls, one line.
     Measured at 1440×960 on Templates — 52px of head became 24px, and the grid
     below it starts where the prototype's does. */
  .rw-head{ display:flex; align-items:center; gap:var(--v-sp-md); flex:0 0 auto; min-height:26px; }
  .rw-headtext{ min-width:0; flex:1; display:flex; align-items:baseline; gap:var(--v-sp-sm); }
  /* THE EYEBROW. Still an `<h1>` — a screen reader still navigates to it, and
     Settings' heading still names the SECTION, which is the one place this text
     says something the chrome tab cannot. It is drawn as a mono caption because
     that is what it is: a label on a band, the same instrument as a pane head's
     `rw-panettl` and a dock card's caption. Drawn at display size it was the tab
     you had just pressed, in 24px, directly under itself. */
  .rw-h1{ margin:0; flex:0 0 auto; font-family:var(--f-mono); font-size:var(--v-fs-cap);
    line-height:var(--v-lh-lbl); font-weight:600; letter-spacing:var(--v-tr-caps);
    text-transform:uppercase; color:var(--v-txt); white-space:nowrap; }
  /* The standfirst is ONE sentence and it is measured: past ~74 characters a line
     stops being read and starts being skimmed, which is the opposite of what a
     sentence saying "nothing here reaches an output" is for. On the band it gets
     one line and truncates; the whole sentence is in its `title`, and the ones
     that carry a guarantee rather than a description (the Planner's "nothing
     here can reach an output") are stated a second time as a pane footer, where
     they are read at the moment they matter. */
  .rw-lead{ margin:0; min-width:0; flex:1 1 auto; font-size:var(--v-fs-b2);
    line-height:var(--v-lh-b2); color:var(--v-dim); max-width:74ch;
    white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }

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
