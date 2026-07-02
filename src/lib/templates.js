// Built-in output templates — layout + style + which regions bind, per
// docs/SPEC.md §5. One renderer (Output.svelte) interprets these; there is NO
// per-channel-type branching in logic (CLAUDE.md). Adding/editing templates is
// data, not code. Phase 8 makes these DB-backed + editable in the Templates tab.

export const TEMPLATES = {
  main: {
    name: 'Classic Serif',
    font: 'var(--f-serif)',
    background: 'radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)',
    accent: 'var(--amber)',
    verseColor: '#f4e4c8',
    align: 'center',
    verseSize: '4.6vw',
    refSize: '1.9vw',
    italicRef: true,
    regions: ['verse_text', 'reference'],
  },
  stage: {
    name: 'Stage Mono',
    font: 'var(--f-display)',
    background: '#000000',
    accent: 'var(--teal)',
    verseColor: '#f2f5f6',
    align: 'left',
    verseSize: '5vw',
    refSize: '2vw',
    refFirst: true,
    regions: ['reference', 'verse_text'],
  },
  stream: {
    name: 'Lower Third',
    font: 'var(--f-body)',
    background: 'transparent',
    accent: 'var(--violet)',
    verseColor: '#1c1224',
    align: 'left',
    verseSize: '2.4vw',
    refSize: '1.4vw',
    lowerThird: true,
    regions: ['verse_text', 'reference'],
  },
  lobby: {
    name: 'Lobby Warm',
    font: 'var(--f-serif)',
    background: 'linear-gradient(160deg, #241419, #120a0e)',
    accent: 'var(--rose)',
    verseColor: '#f0dfe3',
    align: 'center',
    verseSize: '3.2vw',
    refSize: '1.6vw',
    regions: ['reference', 'verse_text'],
  },
};

export function templateById(id) {
  return TEMPLATES[id] || TEMPLATES.main;
}
