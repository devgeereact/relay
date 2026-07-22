// Stage displays — the output surfaces a church runs alongside the main wall:
// the congregation wall, a confidence monitor, the preacher's expanded view, a
// musician's chord view, timer / countdown boards, and remote outputs.
//
// A display OWNS its layout (a layer-model template, the same shape the Templates
// editor edits — see layers.js) plus its own hardware metadata (resolution,
// refresh, connection). Everything is LOCAL and offline-first (CLAUDE.md): the
// list lives in localStorage, nothing leaves the device. Wiring a display to a
// real output channel is a separate, explicit step in the Channels tab.
//
// Previews render through the ONE renderer (TemplateRender) for scripture-style
// displays; timer / countdown / confidence / lyrics displays draw a bespoke
// preview because their content is computed live, not a fired verse.

import { writable } from 'svelte/store';
import { makeLayer } from '../layers.js';

const KEY = 'relay.stagedisplays.v1';

let _n = 0;
const nid = () => `sd_${++_n}_${Math.round(performance?.now?.() ?? 0)}`;

// A scripture layout used by the wall-style displays. Verse + reference over an
// optional background — the same layers the editor exposes.
function scriptureLayout(bg = 'radial-gradient(130% 130% at 50% 20%, #2a1c08, #0b0805)', verseColor = '#f4e4c8') {
  return {
    name: 'Stage layout',
    layout: {
      align: 'center',
      layers: [
        makeLayer('background', { name: 'Background', fill: bg, dim: 0.15 }),
        makeLayer('text', { name: 'Reference', bind: 'reference', x: 8, y: 20, w: 84, h: 12, size: 3.1, color: '#f0b74a', align: 'center', italic: false, shadow: 0.4 }),
        makeLayer('text', { name: 'Current Verse', bind: 'verse', x: 8, y: 34, w: 84, h: 44, size: 5.4, color: verseColor, align: 'center', valign: 'middle', font: 'var(--f-serif)', shadow: 0.45 }),
      ],
    },
    style: {},
  };
}

// The catalogue that seeds a fresh install — mirrors the reference gallery.
function seed() {
  return [
    { name: 'Stage Display', type: 'Stage Display', subtitle: 'Main output for the congregation', icon: 'monitor', primary: true, status: 'live', preview: 'scripture', res: [1920, 1080], fps: 60, connection: 'HDMI 1', colour: 'Rec. 709', template: scriptureLayout() },
    { name: 'Confidence Monitor', type: 'Confidence Monitor', subtitle: 'AI confidence and detection info', icon: 'gauge', status: 'live', preview: 'confidence', res: [1920, 1080], fps: 60, connection: 'HDMI 2', colour: 'Rec. 709', template: scriptureLayout() },
    { name: 'Preacher View', type: 'Preacher View', subtitle: 'Expanded view for the preacher', icon: 'eye', status: 'live', preview: 'scripture', res: [1920, 1080], fps: 60, connection: 'HDMI 3', colour: 'Rec. 709', template: scriptureLayout() },
    { name: 'Musician View', type: 'Musician View', subtitle: 'Lyrics, chords and cues', icon: 'music', status: 'live', preview: 'lyrics', res: [1280, 720], fps: 60, connection: 'Network', colour: 'sRGB', template: scriptureLayout() },
    { name: 'Timer View', type: 'Timer View', subtitle: 'Service timers and time elapsed', icon: 'timer', status: 'live', preview: 'timer', res: [1280, 720], fps: 30, connection: 'Network', colour: 'sRGB', template: scriptureLayout() },
    { name: 'Countdown View', type: 'Countdown View', subtitle: 'Countdown to next item', icon: 'ring', status: 'live', preview: 'countdown', res: [1280, 720], fps: 30, connection: 'Network', colour: 'sRGB', template: scriptureLayout() },
    { name: 'Remote Stage Display', type: 'Remote Stage Display', subtitle: 'Remote output for external locations', icon: 'monitor', status: 'live', preview: 'scripture', res: [1920, 1080], fps: 60, connection: 'Network', colour: 'Rec. 709', template: scriptureLayout('radial-gradient(130% 130% at 50% 20%, #0b1c3a, #05080f)', '#eef2f8') },
    { name: 'Countdown Clock', type: 'Countdown Clock', subtitle: 'Standalone countdown clock', icon: 'clock', status: 'live', preview: 'clock', res: [1920, 1080], fps: 60, connection: 'HDMI 4', colour: 'Rec. 709', template: scriptureLayout() },
  ].map((d) => ({ id: nid(), ...d }));
}

function load() {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) || 'null');
    if (Array.isArray(raw) && raw.length) return raw;
  } catch {
    /* fall through to seed */
  }
  return seed();
}

export const displays = writable(load());

displays.subscribe((v) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(v));
  } catch {
    /* locked-down webview — in-memory is fine for this session */
  }
});

export function updateDisplay(id, patch) {
  displays.update((list) => list.map((d) => (d.id === id ? { ...d, ...patch } : d)));
}

export function addDisplay() {
  const d = { id: nid(), name: 'New Display', type: 'Stage Display', subtitle: 'Custom output', icon: 'monitor', status: 'offline', preview: 'scripture', res: [1920, 1080], fps: 60, connection: 'Network', colour: 'Rec. 709', template: scriptureLayout() };
  displays.update((list) => [...list, d]);
  return d.id;
}

export function removeDisplay(id) {
  displays.update((list) => list.filter((d) => d.id !== id));
}

export const DISPLAY_TYPES = [
  'Stage Display', 'Confidence Monitor', 'Preacher View', 'Musician View',
  'Timer View', 'Countdown View', 'Remote Stage Display', 'Countdown Clock',
];

export const RESOLUTIONS = [
  { label: '1920 × 1080 (16:9)', res: [1920, 1080] },
  { label: '1280 × 720 (16:9)', res: [1280, 720] },
  { label: '3840 × 2160 (16:9)', res: [3840, 2160] },
  { label: '1080 × 1920 (9:16)', res: [1080, 1920] },
];
