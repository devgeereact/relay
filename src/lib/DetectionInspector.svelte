<script>
  // AI DETECTION DETAIL / INSPECTOR — reference: relay-production-interface.png
  // panel 8.
  //
  // Live's claim panel already says WHAT the AI thinks and offers accept/dismiss.
  // This is the detail behind it: the transcript it heard, the verse it matched,
  // the actual evidence, and — the part nothing in the app said out loud until
  // now — that accepting or dismissing TUNES THE GATE.
  //
  // ── Two places this deliberately departs from the reference ────────────────
  //
  // 1. **"Why this match?" is not prose.** The mockup lists reassuring sentences:
  //    "Order and structure align closely", "Minimal words added or skipped",
  //    "Confidence computed from semantic similarity". Relay computes none of
  //    those things. Rendering them would be fabricated reasoning — a screen
  //    explaining a decision by describing an algorithm that does not exist,
  //    which is worse than no explanation because it is checkable and wrong.
  //
  //    The real evidence is already here: for a spoken reference, the span of
  //    transcript the parser read; for a paraphrase, the shared rare words that
  //    actually drove the cosine (`SemanticIndex::top_k_explained`, arriving as
  //    `matched_text`). That is what is shown, and it is labelled as what it is.
  //
  // 2. **No sensitivity sliders.** The mockup puts Sensitivity, Auto-fire and
  //    Minimum Confidence in this panel. Settings already owns them, and
  //    thresholds are configuration with exactly ONE baseline (router.rs). A
  //    second set of sliders is a second source of truth for the gate that
  //    decides what a congregation sees. Current values are shown read-only,
  //    with a way to go and change them where they live.
  //
  // The percentage rule is absolute: a paraphrase shows NO number, at any score.
  // A TF-IDF cosine is a distance in an arbitrary vector space, not a
  // probability (CLAUDE.md §18, DECISIONS §21).

  import { trapFocus } from './focus.js';
  import { t } from './i18n.js';
  // detect.js is the frontend half of the gate and is already tested: `heard`,
  // `methodKey` and — the important one — `showsConfidence`, which encodes the
  // rule that ONLY a heard reference may display a percentage. This screen must
  // not re-derive that rule; a second copy is a second thing to get wrong.
  import { heard, methodKey, showsConfidence, evidenceIsASpan } from './detect.js';
  import { capture, transcript } from './stores/capture.js';
  import { describeGate } from './gate.js';

  // One reading, derived from the store the engine writes through
  // `detection://thresholds`. Never a local copy: a second copy of the gate is the
  // defect the dial itself was just repaired for.
  $: gate = describeGate($capture);

  /** The detection being inspected. */
  export let detection = null;
  /** Other pending candidates — used for an ambiguous reference. */
  export let alternatives = [];
  export let onClose = () => {};
  export let onAccept = () => {};
  export let onDismiss = () => {};
  /** Jump to Settings, where the thresholds actually live. */
  export let onTuning = () => {};

  // `direct` is the only method that may auto-fire and the only one whose number
  // means anything. Everything else is a suggestion, forever.
  $: isDirect = heard(detection);
  $: isAmbiguous = detection?.method === 'ambiguous';
  // A VERSE RELAY HEARD BEING READ (DECISIONS §118). The one method other than
  // `direct` that may reach a wall unattended — so the sentence below it cannot
  // be the paraphrase's, which promises *"will never fire on its own, at any
  // score"*. That promise is still true of every other method and is exactly the
  // kind of guarantee that must not be left standing after it stops holding.
  $: isReading = detection?.method === 'reading';
  // The church's switch, read from the one store. Not asked again here: the
  // engine is what decides and `capture.js` holds its answer.
  $: followsReader = $capture.followsReader !== false;
  // AND THE CHURCH'S PARAPHRASE BAR (DECISIONS §125), from the same store for the
  // same reason. It belongs on this panel because this panel exists to answer *why
  // did Relay say that, or not say it* — and with the bar on, the answer for a whole
  // class of paraphrase is "because you asked me not to".
  $: paraphraseNeedsRun = $capture.paraphraseNeedsRun === true;
  $: showPct = showsConfidence(detection);
  $: pct = Math.round((detection?.confidence ?? 0) * 100);

  // The paraphrase evidence arrives as "word · word · word" (main.rs joins the
  // terms `top_k_explained` returned). Split it back out so each one can be a
  // chip the operator can actually scan.
  // A QUOTED match carries a contiguous phrase, not a term list, so it is shown
  // the way `direct` is: as words somebody actually said, in order. Splitting it
  // on `·` would find nothing and silently fall through to an empty chip list.
  $: isSpan = evidenceIsASpan(detection);
  $: terms =
    !isSpan && detection?.matched_text
      ? detection.matched_text.split('·').map((s) => s.trim()).filter(Boolean)
      : [];

  // The transcript line this most likely came from. Best-effort and LABELLED as
  // such: a detection carries no transcript id, so this is the most recent final
  // line, not a proven link. Saying "most recent" is the difference between
  // giving context and making a claim.
  //
  // `transcript` is `{ partial, finals }` and `finals` holds plain STRINGS — there
  // is no per-line timestamp to show, so none is invented.
  $: heardLine = $transcript?.finals?.length
    ? $transcript.finals[$transcript.finals.length - 1]
    : ($transcript?.partial ?? '');

  // RULE 44 · this panel takes the operator's panic key, so it owes them an outcome.
  //
  // `role="dialog"` makes `shortcuts.js` stand down (rule 16, correctly), and this
  // panel bound nothing — so Escape was withheld from the shell and delivered to
  // nobody. Of the overlays that had this defect, this is the one that opens
  // DURING a service: it is reached from a claim card on the run surface, by an
  // operator who is unsure about a verse, which is precisely when they are most
  // likely to reach for Escape.
  //
  // It closes the panel and nothing else. `onClose` is the same door the scrim and
  // the header's close button use, so there is one way out rather than three.
  // The second press is the operator's, and the global handler has the key back.
  //
  // Guarded on `detection`, because the component is always mounted and renders
  // nothing when there is no claim — an unguarded handler here would swallow
  // Escape for the whole service. That is the shape of the bug, not the fix.
  function onKey(e) {
    if (e.key !== 'Escape' || !detection) return;
    e.preventDefault();
    e.stopPropagation();
    onClose();
  }
</script>

<svelte:window on:keydown={onKey} />

{#if detection}
  <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
  <div class="ins-scrim" role="presentation" on:click={onClose}>
    <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
    <div
      class="ins"
      role="dialog"
      aria-modal="true"
      aria-label="AI detection detail"
      use:trapFocus
      on:click|stopPropagation
    >
      <header class="ins-head">
        <h2>AI Detection — {detection.reference}</h2>
        <span class="spring"></span>
        <button class="ins-x" on:click={onClose} aria-label="Close">✕</button>
      </header>

      <div class="ins-body">
        <!-- ── LEFT: what kind of claim is this? ── -->
        <aside class="ins-left">
          <p class="klbl">Claim type</p>
          <!-- CYAN for a guess — never amethyst (rehearsal), never amber (on air).
               And a paraphrase carries NO percentage. -->
          <span class="ctype" class:guess={!isDirect}>
            {$t(methodKey(detection))}
            {#if showPct}<b>{pct}%</b>{/if}
          </span>

          <h3>
            {#if isDirect}
              Direct match
            {:else if isReading}
              Read aloud
            {:else if isAmbiguous}
              Ambiguous reference
            {:else}
              Paraphrase
            {/if}
          </h3>
          <p class="ins-p">
            {#if isDirect}
              Relay read a scripture reference in the transcript. This is one of two kinds
              of claim allowed to go on a screen by itself.
            {:else if isReading}
              Nobody said a reference. Relay heard these words read out, in this order, and
              only this verse has them — so it followed the reader and put the verse up.
              Turn that off in Settings → AI &amp; Detection if your church would rather be
              asked first.
            {:else if isAmbiguous}
              Relay heard a reference that could mean more than one verse. It will never
              fire on its own — you choose which one is meant.
            {:else}
              Relay did not hear a reference. It found a verse whose <em>wording</em> is
              close to what was said. This is a suggestion and will never fire on its
              own, at any score.
            {/if}
          </p>

          {#if !isDirect}
            <!-- The one number that must never appear, explained rather than shown. -->
            <p class="ins-why-no-number">
              {#if isReading}
                There is no percentage here on purpose. What decided this was how many
                words ran together — a count, not a chance of being right.
              {:else}
                There is no percentage here on purpose. The match is a distance between
                word patterns, not a probability — a number would look like a chance of
                being right, and it is not one.
              {/if}
            </p>
          {/if}

          <hr />

          <!-- THE GATE, THROUGH THE ONE DESCRIBER. This panel printed the two
               figures straight off the store, which reads identically in three
               situations that are not the same fact: the engine answered and the
               gate is where the dial put it; the engine answered and Relay has
               MOVED the gate since, from what was confirmed and dismissed during
               services or from a profile or room being applied; and the engine has
               not answered at all, where `?? 0` painted a confident `0%` over a
               question nobody had asked. That is rule 35 on the panel an operator
               opens precisely to ask why a verse did or did not fire.
               `describeGate` is the one place this is turned into words, and it is
               the same one Settings reads, so the two cannot disagree. -->
          <p class="klbl">Gate</p>
          <!-- THE PARAPHRASE RULE IS NOT A READING, so it is outside the branch.
               A cosine can never auto-fire at any score and at any threshold
               (rule 10, DECISIONS §21), which is true whether or not the engine
               has answered with the gate. `inspector.test.js` caught this being
               hidden behind readability, and it was right to: the one sentence in
               this panel that is unconditionally true is the one that must not
               depend on a socket. -->
          <dl class="ins-dl">
            {#if gate.readable}
              <!-- WHAT EACH BAR NEEDS, 0-100 (DECISIONS §117, §121). Auto-fire is
                   always the larger, by exactly one band, because it is the
                   stricter rule — which is the whole point of printing it this
                   way round on the panel an operator opens to ask why a verse
                   did or did not fire. The words are `describeGate`'s and
                   Settings prints the same pair, so the two cannot disagree. -->
              <dt>Auto-fire needs</dt>
              <dd class="r-mono">{gate.autoPct} / 100</dd>
              <dt>Suggest needs</dt>
              <dd class="r-mono">{gate.suggestPct} / 100</dd>
            {/if}
            <dt>Paraphrases</dt>
            <dd>Suggestions only — never auto-fire</dd>
            <!-- WHAT IS BEING WITHHELD, not only what is capped. With the bar on,
                 about three quarters of the paraphrase list is never offered — and an
                 operator asking why the list went quiet has to be able to find that
                 out from the panel they opened to ask. Rule 35 on a settings
                 read-out: the row reads differently in the two states or it is not
                 telling them anything. -->
            <dt>Paraphrase evidence</dt>
            <dd>{paraphraseNeedsRun ? 'Must echo the verse’s words in order' : 'Any strong word match'}</dd>
            <!-- AND THE ONE THING THAT CHANGED, beside the thing that did not.
                 A panel that still says only paraphrases are capped, on the day a
                 second method started reaching walls, is a guarantee left standing
                 after it stopped holding. DECISIONS §118. -->
            <dt>Read aloud</dt>
            <dd>{followsReader ? 'Followed — goes up by itself' : 'Suggestions only — turned off'}</dd>
          </dl>
          {#if !gate.readable || gate.drifted}
            <p class="ins-gatenote">{gate.note}</p>
          {/if}
          <button class="ins-link" on:click={onTuning}>Change sensitivity in Settings</button>
        </aside>

        <!-- ── RIGHT: the evidence ── -->
        <div class="ins-right">
          <div class="ins-cards">
            <section class="ins-card">
              <p class="klbl">Heard (most recent transcript)</p>
              {#if heardLine}
                <p class="ins-quote">{heardLine}</p>
              {:else}
                <p class="ins-empty">No transcript yet — nothing has been heard this session.</p>
              {/if}
            </section>

            <section class="ins-card">
              <p class="klbl">Matched verse{detection.translation ? ` · ${detection.translation}` : ''}</p>
              {#if detection.text}
                <p class="ins-quote serif">{detection.text}</p>
                <p class="ins-time r-mono">{detection.reference}</p>
              {:else}
                <p class="ins-empty">
                  Not in the library — the verse text could not be looked up.
                </p>
              {/if}
            </section>
          </div>

          <!-- WHY. Real evidence only. See the header note. -->
          <section class="ins-card ins-why">
            <p class="klbl">Why this match?</p>
            {#if isDirect && detection.matched_text}
              <p class="ins-p">
                Relay read this reference in what was said:
              </p>
              <p class="ins-quote">“{detection.matched_text}”</p>
            {:else if isSpan && detection.matched_text}
              <p class="ins-p">
                These words were read aloud, in this order, and they are in this verse
                word for word. No reference was spoken:
              </p>
              <p class="ins-quote">“{detection.matched_text}”</p>
            {:else if terms.length}
              <p class="ins-p">
                These words are shared between what was said and this verse, and they are
                rare enough across the whole Bible to carry weight. Strongest first:
              </p>
              <ul class="ins-terms">
                {#each terms as term}<li>{term}</li>{/each}
              </ul>
              <p class="ins-note">
                That is the entire basis for this suggestion. Relay does not compare
                grammar, word order, or meaning — only which uncommon words overlap.
              </p>
            {:else if isAmbiguous}
              <p class="ins-p">
                The reference Relay heard is incomplete, so more than one verse fits it.
                There is no further evidence to weigh — pick the one that was meant.
              </p>
            {:else}
              <p class="ins-empty">
                No evidence was recorded for this match. That is unusual — treat the
                suggestion with more suspicion than usual, not less.
              </p>
            {/if}
          </section>

          {#if alternatives.length}
            <!-- VERSE MATCH COMPARISON (§5), folded in: an ambiguous reference's
                 other candidates, side by side, because that is the only moment
                 the comparison is actually useful. -->
            <section class="ins-card">
              <p class="klbl">Other verses this could be</p>
              <ul class="ins-alts">
                {#each alternatives as a}
                  <li>
                    <b>{a.reference}</b>
                    {#if a.text}<span>{a.text}</span>{/if}
                  </li>
                {/each}
              </ul>
            </section>
          {/if}
        </div>
      </div>

      <footer class="ins-foot">
        <!-- THE LOOP, MADE VISIBLE. `confirm_detection` and `dismiss_detection`
             both call `router.record_feedback` — every accept and every dismiss
             moves the gate. That has been true for months and no screen ever
             said so, which makes it invisible training: the operator changes the
             product's behaviour without being told they are doing it. -->
        <p class="ins-learn">
          Accepting or dismissing this <b>retunes the gate</b> — Relay becomes a little
          more or less eager to fire on its own. It does not learn during a rehearsal.
        </p>
        <span class="spring"></span>
        <button class="r-btn ghost" on:click={onDismiss}>Dismiss</button>
        <button class="r-btn primary" on:click={onAccept}>Accept &amp; fire</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .ins-scrim {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.66);
    padding: 24px;
    overflow: auto;
  }
  .ins {
    width: 100%;
    max-width: 940px;
    background: var(--v-surf);
    border: 1px solid var(--v-line2);
    border-radius: var(--v-r-xl);
    box-shadow: var(--v-shadow-lg);
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 48px);
  }
  .ins-head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 20px;
    border-bottom: 1px solid var(--v-line);
  }
  .ins-head h2 {
    margin: 0;
    font-family: var(--f-head);
    font-size: var(--v-fs-h2);
    font-weight: 600;
    color: var(--v-txt);
  }
  .spring {
    flex: 1;
  }
  /* A PANEL DISMISS, not a button. It is a bare glyph in the panel's own head at
     the type scale's title size, with no fill and no edge, because the head is
     the surface it sits on and an outlined control up there would read as an
     action the operator should take. Named so a shape census can tell it from a
     button that lost its class. */
  .ins-x {
    background: none;
    border: 0;
    color: var(--v-faint);
    font-size: var(--v-fs-ttl);
    cursor: pointer;
    padding: 4px 6px;
    border-radius:var(--v-r-2xl);
  }
  .ins-x:hover {
    color: var(--v-txt);
  }

  .ins-body {
    display: grid;
    grid-template-columns: 268px minmax(0, 1fr);
    gap: 0;
    min-height: 0;
    overflow-y: auto;
  }
  .ins-left {
    padding: 18px 20px;
    border-right: 1px solid var(--v-line);
    background: var(--v-bg);
  }
  .ins-right {
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }
  .ins-cards {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .klbl {
    margin: 0 0 7px;
    font-family: var(--f-mono);
    font-size:var(--v-fs-cap);
    font-weight: 600;
    letter-spacing: 0.13em;
    text-transform: uppercase;
    color: var(--v-faint);
  }

  /* CYAN = the AI is guessing. Never amethyst (rehearsal), never amber (on air). */
  .ctype {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-radius: 8px;
    border: 1px solid var(--v-cyan);
    background: var(--v-cyan-soft);
    color: var(--v-cyan);
    font-family: var(--f-mono);
    font-size:var(--v-fs-mono);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  .ctype b {
    font-weight: 700;
    color: var(--v-txt);
  }

  .ins h3 {
    margin: 16px 0 6px;
    font-family: var(--f-head);
    font-size: var(--v-fs-h3);
    font-weight: 600;
    color: var(--v-txt);
  }
  .ins-p {
    margin: 0 0 10px;
    font-size: var(--v-fs-b2);
    line-height: 1.6;
    color: var(--v-dim);
  }
  .ins-why-no-number {
    margin: 10px 0 0;
    padding: 10px 12px;
    border-radius: var(--v-r-md);
    background: var(--v-cyan-soft);
    border: 1px solid var(--v-cyan-line);
    font-size:var(--v-fs-b1);
    line-height: 1.6;
    color: var(--v-dim);
  }
  .ins hr {
    border: 0;
    border-top: 1px solid var(--v-line);
    margin: 18px 0 14px;
  }
  /* The gate's caveat, and the quietest thing in the panel. No law colour: none of
     amber, cyan or amethyst promises anything about a threshold, and this is a
     footnote about a number rather than a claim about a screen. */
  .ins-gatenote {
    margin: 0 0 var(--v-sp-xs);
    font-size: var(--v-fs-cap);
    line-height: 1.5;
    color: var(--v-faint);
  }
  .ins-dl {
    margin: 0 0 12px;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 12px;
  }
  .ins-dl dt {
    font-size:var(--v-fs-b1);
    color: var(--v-faint);
  }
  .ins-dl dd {
    margin: 0;
    font-size:var(--v-fs-b1);
    color: var(--v-txt);
    text-align: right;
  }
  /* A LINK, not a button. It navigates to Settings; it changes nothing by itself
     and it sits inside a sentence, so it wears the sentence's type and an
     underline rather than a control's box. Forcing it into `.r-btn` would put a
     26px lozenge in the middle of a paragraph. */
  .ins-link {
    background: none;
    border: 0;
    padding: 0;
    font: inherit;
    font-size:var(--v-fs-b1);
    color: var(--v-accent2);
    cursor: pointer;
    text-decoration: underline;
  }

  .ins-card {
    padding: 14px 16px;
    border-radius: var(--v-r-md);
    background: var(--v-bg);
    border: 1px solid var(--v-line);
    min-width: 0;
  }
  .ins-quote {
    margin: 0;
    font-size: var(--v-fs-ttl);
    line-height: 1.55;
    color: var(--v-txt);
  }
  .ins-quote.serif {
    font-family: var(--f-serif);
  }
  .ins-time {
    margin: 8px 0 0;
    font-size:var(--v-fs-lbl);
    color: var(--v-faint);
  }
  .ins-empty {
    margin: 0;
    font-size: var(--v-fs-b2);
    line-height: 1.6;
    color: var(--v-faint);
  }
  .ins-note {
    margin: 10px 0 0;
    font-size:var(--v-fs-b1);
    line-height: 1.6;
    color: var(--v-faint);
  }

  .ins-terms {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 7px;
  }
  .ins-terms li {
    padding: 5px 11px;
    border-radius: var(--v-r-sm);
    background: var(--v-cyan-soft);
    border: 1px solid var(--v-cyan-line);
    color: var(--v-cyan);
    font-family: var(--f-mono);
    font-size:var(--v-fs-b2);
  }

  .ins-alts {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .ins-alts li {
    display: flex;
    gap: 10px;
    font-size: var(--v-fs-b2);
    line-height: 1.55;
    color: var(--v-dim);
  }
  .ins-alts b {
    flex: 0 0 auto;
    color: var(--v-txt);
  }

  .ins-foot {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 20px;
    border-top: 1px solid var(--v-line);
    flex-wrap: wrap;
  }
  .ins-learn {
    margin: 0;
    max-width: 46ch;
    font-size:var(--v-fs-b1);
    line-height: 1.55;
    color: var(--v-faint);
  }
  .ins-learn b {
    color: var(--v-dim);
  }

  @media (max-width: 820px) {
    .ins-body {
      grid-template-columns: 1fr;
    }
    .ins-left {
      border-right: 0;
      border-bottom: 1px solid var(--v-line);
    }
    .ins-cards {
      grid-template-columns: 1fr;
    }
  }
</style>
