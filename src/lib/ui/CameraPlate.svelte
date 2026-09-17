<script>
  // WHAT A LOWER THIRD IS ACTUALLY OVER — a preview affordance, and only that.
  //
  // A KEYED template (`isKeyedTemplate`) paints a band and leaves the rest of the
  // frame transparent, because the rest of the frame is a camera the switcher
  // supplies. Relay never takes that feed — NDI is parked and a camera reaches
  // the building through OBS or an ATEM — so on any surface that previews a
  // template against nothing, a lower third renders as near-black type on
  // transparency: an empty dark rectangle. That is exactly right on the wall and
  // useless on a card.
  //
  // The Templates gallery hit this on `Lower Third`: `background: transparent`
  // with `verseColor: #1c1224`, legible over a camera and invisible over
  // nothing. The Outputs workspace had already solved it for its screen cards
  // and its inspector preview, with this markup written out twice, four hundred
  // lines apart. It is one component now rather than a third copy — the same
  // reasoning as the `Copy URL` builder this branch already records, where one
  // of two copies was corrected and the other was not. (`DeskStrip` was the
  // other precedent cited here and has itself been deleted: themes were folded
  // into templates, so the Templates workspace has one desk and a segmented
  // control offering one option is a control that does nothing — DECISIONS §87.)
  //
  // ── IT IS LABELLED, ALWAYS ────────────────────────────────────────────────
  // An unlabelled picture on these surfaces could be mistaken for something
  // Relay is sending, and **Relay sends no video**. The word is what keeps the
  // plate from being a claim.
  //
  // ── IT MAY NEVER REACH AN OUTPUT ──────────────────────────────────────────
  // The output page is transparent so a keyed template keys out for OBS/ATEM —
  // that transparency IS the feature. A plate behind a real output would paint
  // over the camera it exists to caption, which is the same failure as blacking
  // out a keyed channel (`isKeyedTemplate`'s own reason for existing). So this
  // component is rendered ONLY by preview surfaces, never by `Output.svelte`,
  // `Stage.svelte` or `TemplateRender` itself, and `cameraplate.test.js` holds
  // that boundary from the other side.
  //
  // The caller decides WHETHER to show it (`{#if isKeyedTemplate(tpl)}`), because
  // the caller is the one that knows which template it has resolved. The decision
  // is already shared — `isKeyedTemplate` in `layers.js` — so only the picture
  // needed a home.
</script>

<div class="cp" aria-hidden="true"><span class="cp-lbl r-mono">camera</span></div>

<style>
  .cp {
    position: absolute;
    inset: 0;
    background: linear-gradient(135deg, var(--v-surf3), var(--v-void) 46%, var(--v-surf2));
  }
  .cp-lbl {
    position: absolute;
    left: 5%;
    top: 6%;
    font-size: var(--v-fs-cap);
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--v-faint);
  }
</style>
