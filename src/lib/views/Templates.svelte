<script>
  // Phase 1 static editor shell. Phase 8 binds this to the shared template
  // engine — regions/style/data-binding are ONE engine across every channel
  // type (CLAUDE.md): never a per-channel rendering branch.
  const templates = [
    { name: 'Classic Serif', tag: 'main' },
    { name: 'Stage Mono',    tag: 'stage' },
    { name: 'Lower Third',   tag: 'stream' },
    { name: 'Lobby Warm',    tag: 'lobby' },
  ];
  let active = 0;

  const regions = [
    { id: 'verse_text', on: true },
    { id: 'reference',  on: true },
    { id: 'timer',      on: false },
    { id: 'next_event', on: false },
  ];
  const accents = ['var(--amber)', 'var(--teal)', 'var(--violet)', 'var(--rose)'];
  let accentSel = 0;
</script>

<div class="templates-layout">
  <div class="tmpl-list">
    {#each templates as t, i}
      <div class="tmpl-row" class:active={i === active} on:click={() => (active = i)} role="button" tabindex="0">
        {t.name} <span class="tag">{t.tag}</span>
      </div>
    {/each}
    <button class="btn-ghost" style="margin-top:6px;">+ New template</button>
  </div>

  <div class="panel editor">
    <div class="panel-title">Editing — {templates[active].name}</div>
    <div style="display:grid; grid-template-columns:1fr 1fr; gap:20px;">
      <div>
        <div class="field-group">
          <div class="field-label">Regions</div>
          {#each regions as r}
            <label class="check-row"><input type="checkbox" checked={r.on} /> {r.id}</label>
          {/each}
        </div>
        <div class="field-group">
          <div class="field-label">Typeface</div>
          <select class="select-mock"><option>Fraunces (serif)</option><option>Space Grotesk</option><option>Inter</option></select>
        </div>
        <div class="field-group">
          <div class="field-label">Background</div>
          <select class="select-mock"><option>Radial warm dark</option><option>Solid charcoal</option><option>Transparent (keyed)</option></select>
        </div>
        <div class="field-group">
          <div class="field-label">Accent color</div>
          <div class="swatches">
            {#each accents as a, i}
              <div class="swatch" class:sel={i === accentSel} style="background:{a};" on:click={() => (accentSel = i)} role="button" tabindex="0"></div>
            {/each}
          </div>
        </div>
      </div>
      <div>
        <div class="field-label" style="margin-bottom:9px;">Live preview</div>
        <div class="channel-preview prev-main" style="border-radius:9px; height:220px;">
          <div>
            <div class="verse" style="font-size:15px;">"For God so loved the world, that He gave His only begotten Son, that whosoever believeth in Him should not perish."</div>
            <div class="ref">John 3:16 · KJV</div>
          </div>
        </div>
      </div>
    </div>
  </div>
</div>
