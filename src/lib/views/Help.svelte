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
</script>

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

<section class="r-tile h-card h-keys">
  <div class="h-head static"><span class="h-ic" aria-hidden="true">⌨️</span><b>Every shortcut</b></div>
  <table>
    {#each SHORTCUTS as s}
      <tr>
        <td class="k">{#each s.keys as k}<kbd>{k}</kbd>{/each}</td>
        <td>{s.label}</td>
        <td class="sc">{s.always ? 'Everywhere' : 'Where it applies'}</td>
      </tr>
    {/each}
  </table>
  <p class="h-note">
    <kbd>Esc</kbd> and <kbd>B</kbd> work on every tab, always — even while you are typing.
    Press <kbd>?</kbd> anywhere for a quick reminder.
  </p>
</section>

<style>
  .h-search {
    max-width: 340px;
    margin-bottom: 16px;
  }
  .h-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 12px;
  }
  .h-card {
    padding: 0;
    overflow: hidden;
  }
  .h-head {
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    padding: 14px 16px;
    background: none;
    border: 0;
    color: var(--v-txt);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .h-head.static {
    cursor: default;
  }
  .h-head b {
    flex: 1;
    font-size: 13.5px;
  }
  .h-ic {
    font-size: 16px;
    flex: none;
  }
  .h-chev {
    color: var(--v-dim);
    font-size: 15px;
  }
  .h-body {
    margin: 0;
    padding: 0 16px 14px;
    border-top: 1px solid var(--v-line);
    padding-top: 12px;
  }
  .h-body dt {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--v-txt);
    margin-top: 11px;
  }
  .h-body dt:first-child {
    margin-top: 0;
  }
  .h-body dd {
    margin: 3px 0 0;
    font-size: 12.5px;
    line-height: 1.65;
    color: var(--v-dim);
  }
  .h-keys {
    margin-top: 12px;
    padding: 0 0 14px;
  }
  .h-keys table {
    width: 100%;
    border-collapse: collapse;
    padding: 0 16px;
  }
  .h-keys tr {
    border-top: 1px solid var(--v-line);
  }
  .h-keys td {
    padding: 9px 16px;
    font-size: 12.5px;
    color: var(--v-txt);
  }
  .h-keys td.k {
    width: 1%;
    white-space: nowrap;
  }
  .h-keys td.sc {
    text-align: right;
    font-family: var(--f-mono);
    font-size: 9.5px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--v-dim);
    white-space: nowrap;
  }
  .h-note {
    margin: 12px 16px 0;
    font-size: 12px;
    color: var(--v-dim);
    line-height: 1.6;
  }
  .h-none {
    margin-bottom: 14px;
  }
  kbd {
    display: inline-block;
    font-family: var(--f-mono);
    font-size: 10.5px;
    line-height: 1;
    padding: 4px 6px;
    margin-right: 4px;
    border-radius: 5px;
    background: var(--v-surf3);
    border: 1px solid var(--v-line2);
    color: var(--v-txt);
  }
</style>
