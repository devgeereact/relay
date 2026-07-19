<script>
  // In-app help. There was none.
  //
  // The operator guide lives in a markdown file on GitHub, which is exactly no use
  // to a volunteer in a dark booth at 10:25 on a Sunday morning with the service
  // starting in five minutes. Help that requires a working internet connection is
  // help that is missing precisely when Relay is most useful — offline.
  //
  // So: written for a volunteer, not a developer. Every answer says what to DO.
  import { SHORTCUTS } from '../shortcuts.js';

  let q = '';
  let open = null;

  const TOPICS = [
    {
      id: 'panic',
      icon: '🛑',
      title: 'Something wrong is on the screen',
      body: [
        ['Press <kbd>Esc</kbd>.', 'It clears every screen instantly. It works from any tab in Relay, and it works even while you are typing in a box.'],
        ['Press <kbd>B</kbd> to black out.', 'This goes further: it turns every output screen completely black. Use it if you need the screens gone, not just empty.'],
        ['Or click <b>Emergency Stop</b>', 'It is in the top-right of every screen, always. You never have to go looking for it.'],
      ],
    },
    {
      id: 'nothing',
      icon: '📽️',
      title: 'Nothing appears on the projector',
      body: [
        ['Is the output window open?', 'On the <b>Live</b> tab, click <b>Open output</b>. It opens on the projector — the screen that is <i>not</i> this laptop.'],
        ['Is it on the right screen?', 'Go to <b>Channels</b> and set the display for “Main screen”. Relay lists every screen it can see.'],
        ['Still nothing?', 'Some projectors take a few seconds to wake. If Relay shows the verse in its own preview but the projector is dark, the problem is the cable or the projector input, not Relay.'],
      ],
    },
    {
      id: 'ai',
      icon: '🎧',
      title: 'The AI is not detecting verses',
      body: [
        ['Is the speech model installed?', 'If Relay says it cannot hear the sermon, there is a <b>Download</b> button. It is a one-time download and takes a few minutes.'],
        ['Is Relay listening?', 'Press <b>Start listening</b> on the <b>Live</b> tab. The transcript should start filling up as the preacher speaks.'],
        ['Can it actually hear?', 'Go to <b>Settings</b> and watch the microphone meter while someone speaks. If the bar does not move, Relay is listening to the wrong microphone — usually it should be the feed from the sound desk, not the laptop’s own mic.'],
        ['It hears, but finds no verses.', 'It only detects a verse when the preacher <i>says the reference</i> — “John chapter three verse sixteen”. If they only quote the words, Relay will <i>offer</i> it as a suggestion, and wait for you. It will never put a guess on the screen by itself.'],
      ],
    },
    {
      id: 'suggestion',
      icon: '✋',
      title: 'What the AI will and will not do on its own',
      body: [
        ['It can put a spoken reference straight up.', 'If the preacher clearly says “Romans 8:28”, Relay is confident and shows it.'],
        ['It will NEVER put a guess up.', 'If it only <i>thinks</i> a sentence sounds like a verse, it offers it to you as a suggestion. Press <kbd>A</kbd> to accept, <kbd>D</kbd> to dismiss. Nothing reaches the congregation without you.'],
        ['You always win.', 'Type any reference in the box and press Enter, and it goes up immediately — regardless of what the AI thinks.'],
        ['It is too eager / too cautious', 'Settings → <b>AI Detection Thresholds</b>. Relay also learns from every suggestion you accept or reject.'],
      ],
    },
    {
      id: 'service',
      icon: '📋',
      title: 'Running a service',
      body: [
        ['Build the plan first.', 'Go to <b>Planner</b>, create a plan, and add songs, scripture, media, announcements and a countdown in the order they happen. Nothing you do here reaches a screen — it is safe to build in the middle of a service.'],
        ['Run it on the Live tab.', 'Press <b>Run this plan</b>. Then use <kbd>→</kbd> / <kbd>←</kbd> to move through the slides. The output screens follow you.'],
        ['Watch the arrow key.', 'The bar tells you what <kbd>→</kbd> will do: <b>steps SLIDE</b> means it moves through your plan; <b>steps VERSE</b> means it walks through the passage on screen. It switches by itself when you accept a verse the AI suggested — accept it, read on with <kbd>→</kbd>, then press <kbd>Esc</kbd> to return to the plan.'],
        ['Suggestions are right there.', 'When the preacher goes off-script and quotes a verse, the AI offers it on the same screen you are running the plan from. You never change tabs mid-sermon.'],
        ['Nothing is lost if Relay crashes.', 'The output screens keep showing the last thing you fired, and Relay comes back to the same place in the plan.'],
      ],
    },
    {
      id: 'practice',
      icon: '🎬',
      title: 'I want to practise without anyone seeing',
      body: [
        ['Press <b>Rehearse</b> on the Live tab.', 'Everything works exactly as it will on Sunday — the AI listens, suggestions appear, the plan runs, the arrow keys move — but <b>nothing reaches the projector, the stage monitor, OBS, or any other screen</b>. You can practise in the middle of a service if you want to.'],
        ['You cannot miss it.', 'A purple band sits across the top of the app the whole time it is on, and the top bar says <b>REHEARSAL</b> instead of <b>On Air</b> on every tab.'],
        ['It does not count.', 'A rehearsal is not saved to your service history, and the AI does not learn from it — otherwise practising would train it on things that never really happened.'],
        ['Going live clears the screens.', 'When you end the rehearsal, Relay blanks every output. The projector had been sitting on whatever was there before you started, and you have not looked at it in twenty minutes — so you put the next thing up deliberately, rather than being handed a wall you have forgotten about.'],
      ],
    },
    {
      id: 'privacy',
      icon: '🔒',
      title: 'Does anything leave this computer?',
      body: [
        ['No.', 'No accounts, no cloud, no server. Unplug the internet and everything still works. The sermon audio is never even saved — it is transcribed and discarded.'],
        ['Crash reports are off.', 'They stay off unless you switch them on in Settings. Even then, transcripts, verse text and lyrics are never sent — only the technical details of the crash.'],
      ],
    },
  ];

  $: hits = q.trim()
    ? TOPICS.filter((t) => {
        const hay = (t.title + ' ' + t.body.map((b) => b.join(' ')).join(' ')).toLowerCase();
        return hay.includes(q.trim().toLowerCase());
      })
    : TOPICS;

  // ── THE SHORTCUT BOARD ────────────────────────────────────────────────────
  //
  // Laid out to docs/relaydesign/relay-helpandshortcut-screen.png. The LAYOUT is
  // the reference's. The KEYS are not, and must never be: every row below is
  // generated from `SHORTCUTS` in lib/shortcuts.js — the same table the keydown
  // handler reads — so this screen cannot drift from the real bindings.
  //
  // The reference mockup prints bindings this app does not have, and some that
  // contradict it outright:
  //
  //   Ctrl+Shift+C for Clear Screens   — it is Esc
  //   Ctrl+Shift+B for Blackout        — it is B
  //   Space as Play / Pause            — Space means ADVANCE, app-wide, and
  //                                      nothing else (CLAUDE.md)
  //   Enter as Confirm & Fire          — accepting the AI's suggestion is A
  //   S / M / I / L / P                — no such bindings exist
  //
  // Copying those would put a false fact about a PANIC key in front of someone
  // who will only ever read this screen under pressure. So the visual grammar is
  // reproduced exactly and the content comes from the source of truth.
  //
  // Grouping is by KEY, with a safe default: anything not named here that is
  // `always` lands in "Other", never in Panic. A binding added to shortcuts.js
  // therefore still appears on this screen, but can never silently claim to be a
  // panic control.
  const PANIC_KEYS = { Esc: true, B: true };

  // View copy only — an icon and a plain-language line per binding. A key with no
  // entry still renders, using its own label from the table.
  const DETAIL = {
    Esc: { icon: 'monitor', sub: 'Blank every output screen, from any tab — even mid-typing.' },
    B: { icon: 'moon', sub: 'Go further: every output goes completely black.' },
    '?': { icon: 'keyboard', sub: 'Show the quick cheatsheet over whatever you are on.' },
    A: { icon: 'flame', sub: 'Put the AI’s top suggestion on the screens.' },
    D: { icon: 'x', sub: 'Reject it. Nothing reaches the congregation.' },
    '→': { icon: 'next', sub: 'Next plan slide, or next verse of the passage.' },
    '←': { icon: 'prev', sub: 'Previous plan slide, or previous verse.' },
    '/': { icon: 'search', sub: 'Type any reference and it goes up, whatever the AI thinks.' },
  };
  const detailOf = (s) => DETAIL[s.keys[0]] ?? { icon: 'key', sub: '' };

  $: panic = SHORTCUTS.filter((s) => s.always && s.keys.some((k) => PANIC_KEYS[k]));
  const ORDER = ['→', '←', 'A', 'D', '/'];
  const rank = (s) => {
    const i = ORDER.indexOf(s.keys[0]);
    return i === -1 ? ORDER.length : i;
  };
  $: transport = SHORTCUTS.filter((s) => !s.always).sort((a, b) => rank(a) - rank(b));
  $: other = SHORTCUTS.filter((s) => s.always && !s.keys.some((k) => PANIC_KEYS[k]));
</script>

<!-- HELP / SHORTCUTS — laid out to docs/relaydesign/relay-helpandshortcut-screen.png.
     Board first (Panic · Transport · Other), then the troubleshooting topics, which
     the reference does not show but which are this tab's other half. -->
<div class="help">
  <div class="board">
    <!-- ══ PANIC ══ Red, per the design system's Error/Panic. These are the only
         two controls in Relay that are wired straight to the store rather than
         through a view's context, so they still work on a tab whose view has
         crashed — which is exactly when someone reads this. -->
    <section class="pane">
      <header class="pane-head"><h2>Panic controls</h2></header>
      <div class="pane-body">
        {#each panic as s (s.keys[0])}
          <div class="panic-card">
            <span class="panic-ic" aria-hidden="true">
              {#if detailOf(s).icon === 'moon'}
                <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5Z"/></svg>
              {:else}
                <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="4" width="20" height="13" rx="2"/><path d="M8 21h8M12 17v4"/></svg>
              {/if}
            </span>
            <b class="panic-t">{s.label}</b>
            <span class="keys">{#each s.keys as k}<kbd>{k}</kbd>{/each}</span>
          </div>
        {/each}
        <p class="panic-note">These work in any mode.</p>

        <div class="callout">
          <span class="callout-ic" aria-hidden="true">
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><path d="M12 3 4 6v6c0 4.4 3.2 7.9 8 9 4.8-1.1 8-4.6 8-9V6l-8-3Z"/><path d="m9 12 2 2 4-4" stroke-linecap="round"/></svg>
          </span>
          <p>Panic controls are always live and take priority over every other action.</p>
        </div>

        <!-- The one exception, and it is the one people get wrong. Stated here
             rather than left to be discovered while a help overlay is open. -->
        <p class="panic-fine">
          <kbd>Esc</kbd> closes an open dialog instead of clearing, when one is open —
          dismissing a help overlay is not a live action.
          <b>{panic.find((s) => s.keys.includes('B')) ? 'B' : ''}</b>
          does not fire while your cursor is in a text box, so typing “Habakkuk” cannot
          black out the congregation.
        </p>
      </div>
    </section>

    <!-- ══ TRANSPORT ══ The context keys. Each is `needs`-gated in shortcuts.js, so
         it works only where the surface registered that action — the board says so
         rather than implying they are global. -->
    <section class="pane">
      <header class="pane-head">
        <h2>Transport <span class="head-sub">(Live)</span></h2>
        <span class="spring"></span>
        <span class="chip ok"><i class="bd"></i>Live shortcuts</span>
      </header>
      <div class="pane-body">
        {#each transport as s (s.keys[0])}
          <div class="row">
            <span class="row-ic" aria-hidden="true">
              {#if detailOf(s).icon === 'next'}
                <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="6 4 16 12 6 20 6 4"/><path d="M19 4v16"/></svg>
              {:else if detailOf(s).icon === 'prev'}
                <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="18 4 8 12 18 20 18 4"/><path d="M5 4v16"/></svg>
              {:else if detailOf(s).icon === 'flame'}
                <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><path d="M12 2s5 5 5 9a5 5 0 0 1-10 0c0-1.5.8-2.8.8-2.8S9 11 10 11c0-3 2-6 2-9Z"/></svg>
              {:else if detailOf(s).icon === 'x'}
                <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M6 6l12 12M18 6 6 18"/></svg>
              {:else}
                <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>
              {/if}
            </span>
            <span class="row-t">
              <b>{s.label}</b>
              {#if detailOf(s).sub}<span>{detailOf(s).sub}</span>{/if}
            </span>
            <span class="keys">{#each s.keys as k}<kbd>{k}</kbd>{/each}</span>
          </div>
        {/each}

        <!-- MODE. `→` genuinely does two different things, and the whole reason the
             transport bar prints its mode is that a key silently meaning two things
             is how the wrong thing reaches a congregation. Neither mode gets a
             semantic colour: amber means ON AIR and amethyst means REHEARSAL, and a
             transport mode is neither. -->
        <div class="mode">
          <span class="mode-ic" aria-hidden="true">
            <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M4 10v4M8 6v12M12 3v18M16 7v10M20 11v2"/></svg>
          </span>
          <p>
            <b>→ steps VERSE</b> when a detected or manually-fired verse is on screen —
            it walks the passage. <b>→ steps SLIDE</b> when plan content is on air — it
            moves through your plan. The Live transport bar always prints which one you
            are about to get.
          </p>
        </div>
      </div>
    </section>

    <!-- ══ OTHER ══ Everything global that is not a panic control. -->
    <section class="pane">
      <header class="pane-head"><h2>Other shortcuts</h2></header>
      <div class="pane-body">
        {#each other as s (s.keys[0])}
          <div class="row compact">
            <span class="row-ic" aria-hidden="true">
              <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/></svg>
            </span>
            <span class="row-t"><b>{s.label}</b></span>
            <span class="keys">{#each s.keys as k}<kbd>{k}</kbd>{/each}</span>
          </div>
        {/each}

        <div class="row compact">
          <span class="row-ic" aria-hidden="true">
            <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><path d="m12 2 9 5-9 5-9-5 9-5Z"/><path d="m3 12 9 5 9-5"/></svg>
          </span>
          <span class="row-t"><b>Close an open overlay</b></span>
          <span class="keys"><kbd>Esc</kbd></span>
        </div>

        <p class="esc-note">
          <kbd class="ok">Esc</kbd> closes this kind of overlay. It will <b>not</b> clear
          the wall while a dialog is open.
        </p>

        <p class="src-note">
          Every key on this board is read from the same table the keyboard handler
          uses, so it cannot fall out of step with the app.
        </p>
      </div>
    </section>
  </div>

  <!-- ── Troubleshooting. Not in the reference; this tab's other half, and the
       reason it exists offline at all. ── -->
  <p class="r-lead">
    Written for whoever is running the service — not for a programmer. Everything here
    works offline, because that is when you need it most.
  </p>

  <input
    class="r-input h-search"
    type="search"
    placeholder="What's going wrong?"
    bind:value={q}
    aria-label="Search help" />

  {#if hits.length === 0}
    <div class="r-empty h-none">
      Nothing matches “{q}”. Try <b>screen</b>, <b>microphone</b>, or <b>suggestion</b>.
    </div>
  {/if}

  <div class="h-grid">
    {#each hits as t (t.id)}
      <section class="r-tile h-card">
        <button
          class="h-head"
          aria-expanded={open === t.id}
          on:click={() => (open = open === t.id ? null : t.id)}>
          <span class="h-ic" aria-hidden="true">{t.icon}</span>
          <b>{t.title}</b>
          <span class="h-chev">{open === t.id ? '−' : '+'}</span>
        </button>
        {#if open === t.id || q.trim()}
          <dl class="h-body">
            {#each t.body as [step, detail]}
              <dt>{@html step}</dt>
              <dd>{@html detail}</dd>
            {/each}
          </dl>
        {/if}
      </section>
    {/each}
  </div>
</div>

<style>
  /* HELP / SHORTCUTS — the reference's three-column board, styled only from the
     --v-* design tokens. */
  .help{display:flex;flex-direction:column;gap:var(--v-sp-md)}
  .spring{flex:1}

  .board{display:grid;grid-template-columns:1fr 1.25fr 1fr;gap:var(--v-sp-sm);align-items:start}
  .pane{display:flex;flex-direction:column;background:var(--v-surf);
    border:1px solid var(--v-line);border-radius:var(--v-r-lg);box-shadow:var(--v-shadow-sm)}
  .pane-head{display:flex;align-items:center;gap:var(--v-sp-sm);padding:14px 16px;
    border-bottom:1px solid var(--v-line)}
  .pane-head h2{margin:0;font-family:var(--f-head);font-size:var(--v-fs-h2);
    line-height:var(--v-lh-h2);font-weight:600;letter-spacing:var(--v-tr-h2);color:var(--v-txt)}
  .head-sub{color:var(--v-faint);font-weight:400}
  .pane-body{padding:14px;display:flex;flex-direction:column;gap:10px}

  .chip{display:inline-flex;align-items:center;gap:7px;flex:0 0 auto;padding:4px 10px;
    border-radius:99px;font-size:var(--v-fs-cap);background:var(--v-surf2);
    border:1px solid var(--v-line2);color:var(--v-faint)}
  .chip.ok{color:var(--v-emerald);border-color:rgba(34,197,94,.32);background:var(--v-emerald-soft)}
  .chip .bd{width:6px;height:6px;border-radius:50%;background:currentColor;
    box-shadow:0 0 6px currentColor}

  /* ── panic ── Red is the design system's Error/Panic. Never amber (that means the
     congregation is looking at it) and never amethyst (rehearsal). */
  .panic-card{display:flex;align-items:center;gap:14px;padding:22px 16px;
    border-radius:var(--v-r-lg);background:linear-gradient(100deg,var(--v-red),#c8302f);
    border:1px solid var(--v-red);box-shadow:0 8px 24px -10px var(--v-red)}
  .panic-ic{flex:0 0 auto;color:#fff;opacity:.95}
  .panic-t{flex:1;min-width:0;font-size:var(--v-fs-h3);line-height:var(--v-lh-h3);
    font-weight:700;color:#fff}
  .panic-card .keys kbd{background:rgba(0,0,0,.32);border-color:rgba(255,255,255,.28);color:#fff}
  .panic-note{margin:2px 0 0;font-style:italic;font-size:var(--v-fs-b2);color:var(--v-faint)}

  .callout{display:flex;align-items:flex-start;gap:12px;margin-top:4px;padding:13px 14px;
    border-radius:var(--v-r-lg);background:var(--v-amethyst-soft);
    border:1px solid rgba(139,92,246,.32)}
  .callout-ic{flex:0 0 auto;width:32px;height:32px;display:grid;place-items:center;
    border-radius:50%;background:rgba(139,92,246,.18);color:var(--v-amethyst)}
  .callout p{margin:0;font-size:var(--v-fs-b2);line-height:1.55;color:var(--v-dim)}
  .panic-fine{margin:0;font-size:var(--v-fs-cap);line-height:1.7;color:var(--v-faint)}
  .panic-fine b{color:var(--v-dim)}

  /* ── rows ── */
  .row{display:flex;align-items:center;gap:12px;padding:12px 13px;
    border-radius:var(--v-r-md);background:var(--v-surf2);border:1px solid var(--v-line)}
  .row.compact{padding:10px 13px}
  .row-ic{flex:0 0 auto;width:34px;height:34px;display:grid;place-items:center;
    border-radius:var(--v-r-md);background:var(--v-surf3);color:var(--v-dim)}
  .row.compact .row-ic{width:30px;height:30px}
  .row-t{flex:1;min-width:0;display:flex;flex-direction:column;gap:2px}
  .row-t b{font-size:var(--v-fs-b1);font-weight:600;color:var(--v-txt)}
  .row-t span{font-size:var(--v-fs-cap);line-height:1.5;color:var(--v-faint)}
  .keys{flex:0 0 auto;display:flex;gap:4px;white-space:nowrap}

  .mode{display:flex;align-items:flex-start;gap:12px;margin-top:4px;padding:13px 14px;
    border-radius:var(--v-r-lg);background:var(--v-bg);border:1px solid var(--v-line2)}
  .mode-ic{flex:0 0 auto;color:var(--v-faint)}
  .mode p{margin:0;font-size:var(--v-fs-b2);line-height:1.6;color:var(--v-dim)}
  .mode b{color:var(--v-txt);font-family:var(--f-mono);font-size:var(--v-fs-cap);
    letter-spacing:.04em}

  .esc-note{margin:4px 0 0;font-size:var(--v-fs-b2);line-height:1.6;color:var(--v-dim)}
  .esc-note b{color:var(--v-txt)}
  .src-note{margin:0;font-size:var(--v-fs-cap);line-height:1.6;color:var(--v-faint)}

  kbd{display:inline-block;font-family:var(--f-mono);font-size:var(--v-fs-cap);line-height:1;
    padding:6px 9px;border-radius:var(--v-r-sm);background:var(--v-surf3);
    border:1px solid var(--v-line2);color:var(--v-txt)}
  kbd.ok{background:var(--v-emerald-soft);border-color:rgba(34,197,94,.32);color:var(--v-emerald)}

  /* ── troubleshooting (unchanged behaviour, tokenised) ── */
  .h-search{max-width:340px}
  .h-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(320px,1fr));gap:var(--v-sp-sm)}
  .h-card{padding:0;overflow:hidden}
  .h-head{display:flex;align-items:center;gap:11px;width:100%;padding:14px 16px;
    background:none;border:0;color:var(--v-txt);font:inherit;text-align:left;cursor:pointer}
  .h-head b{flex:1;font-size:var(--v-fs-b2)}
  .h-ic{font-size:16px;flex:none}
  .h-chev{color:var(--v-dim);font-size:15px}
  .h-body{margin:0;padding:12px 16px 14px;border-top:1px solid var(--v-line)}
  .h-body dt{font-size:var(--v-fs-b2);font-weight:600;color:var(--v-txt);margin-top:11px}
  .h-body dt:first-child{margin-top:0}
  .h-body dd{margin:3px 0 0;font-size:var(--v-fs-b2);line-height:1.65;color:var(--v-dim)}
  .h-none{margin-bottom:0}

  .h-head:focus-visible{outline:2px solid var(--v-accent);outline-offset:-2px}

  @media (max-width:1180px){
    .board{grid-template-columns:1fr 1fr}
  }
  @media (max-width:760px){
    .board{grid-template-columns:1fr}
  }
</style>
