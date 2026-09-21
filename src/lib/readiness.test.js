// RG-116 — THE PRE-SERVICE SCREEN NAMES THE TWO SETTINGS A SERVICE IS DECIDED BY.
//
// `Dashboard.svelte` said "Ready for a service." over "Engine, scripture,
// microphone and speech model all answered", and named NEITHER the speech model
// nor the recognition language. Those are the two settings the field services
// showed to be worth more than every threshold in the product put together:
// five of nine auto-fires correct on `ggml-base` against three of three on
// `turbo`, in one morning, after the model was switched mid-service.
//
// The status bar has carried a `Model` cell since the rebrand, and that is the
// wrong place for this. The status bar is read DURING a service; this is the
// screen somebody opens BEFORE one, which is the only moment either setting can
// still be changed without the service lock in the way.
//
// These assertions are about RENDERED TEXT, which is the level the defect lived
// at: every probe behind this screen was green and right, and the sentence over
// them still told an operator nothing they could act on.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: () => Promise.resolve('0.0.0-test') }));

const cap = await import('./stores/capture.js');

let host;
let app;
// The readiness hero renders "Checking this machine…" until every probe has
// answered, and the probes are async all the way down. A handful of microtask
// turns is not enough; this yields to the macrotask queue too, which is what lets
// the promise chain behind `freshChecks` actually finish.
const settle = async () => {
  for (let i = 0; i < 40; i++) {
    await tick();
    await new Promise((r) => setTimeout(r, 0));
  }
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue([]);
});
afterEach(() => {
  try {
    app?.$destroy();
  } catch {
    /* nothing to destroy */
  }
  host?.remove();
  app = null;
});

async function mountDashboard(stt) {
  cap.capture.update((s) => ({ ...s, stt: { loaded: true, ...stt } }));
  const mod = await import('./views/Dashboard.svelte');
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new mod.default({ target: host });
  await settle();
  return host;
}

const text = (el) => el.textContent.replace(/\s+/g, ' ');

describe('RG-116 · the readiness screen names the model and the language', () => {
  it('names the model in use, not just that one answered', async () => {
    const el = await mountDashboard({
      model: '/Users/x/Library/Application Support/com.relay.app/models/ggml-large-v3-turbo.bin',
      language: 'en',
    });
    expect(text(el)).toMatch(/Speech model/);
    // Shortened through `statusbar.js`'s own `modelLabel`, so the two surfaces
    // cannot disagree about what to call one path.
    expect(text(el)).toMatch(/large-v3-turbo/);
    expect(text(el), 'the raw path reached an operator').not.toMatch(/\.bin/);
  });

  it('says auto-detect rather than leaving the language blank', async () => {
    // The case this row is actually about. A fresh install's seeded profile
    // carries no language, so automatic election is what a church gets on its
    // first Sunday -- and RG-116 measured what that costs: 17 incoherent
    // transcripts and no verse found at all on one 70-second slice, against 161
    // coherent ones with the language fixed.
    //
    // `auto-detect` is a REAL answer, not an absence. Rendering nothing here
    // would read exactly like a language that had been chosen.
    const el = await mountDashboard({ model: '/m/ggml-base.bin', language: null });
    expect(text(el)).toMatch(/Recognition language/);
    expect(text(el)).toMatch(/auto-detect/);
  });

  it('a machine with no model says so in words, not as a blank', async () => {
    const el = await mountDashboard({ model: null, language: 'en' });
    expect(text(el)).toMatch(/Speech model/);
    expect(text(el)).toMatch(/nothing will be transcribed/);
  });
});

// 2026-09-21 · E-2 (RG-190). The hero ran SIX of twenty-three probes — only the
// `diagnostics` stage — so nothing on :8031, nothing on :8032, a missing table
// and rule 25's boot-bricking scratch table all rendered "Ready for a service."
// The same ladder the launch sequence runs is the one this screen must run.
describe('RG-190 · readiness runs every probe, not the first six', () => {
  const HEALTHY = {
    ping: () => true,
    data_health: () => 31102,
    stt_status: () => ({ loaded: true, model: '/m/ggml-large-v3-turbo.bin', language: 'en' }),
    list_audio_devices: () => [{ id: 'a', name: 'Mic' }],
    list_monitors: () => [{ index: 0, name: 'Built-in', width: 1512, height: 982, primary: true }],
    list_output_channels: () => [{ id: 4, name: 'Lobby', render_target: 'network_client' }],
    local_ip: () => '192.168.1.42',
    system_hardware: () => ({ threads: 10, cores: 10, memory_total_gb: 32, memory_free_gb: 11, total_memory_bytes: 3.2e10, available_memory_bytes: 1.1e10, gpu_backends: ['Metal'], disk_free_gb: 300, total_disk_bytes: 9.9e11, free_disk_bytes: 3.1e11 }),
    probe_integrations: () => [
      { label: 'Relay kiosk hub', listening: true, port: 8031 },
      { label: 'Relay HTTP', listening: true, port: 8032 },
    ],
    migration_status: () => ({ version: 5, expected: 5, manual_status: true, scratch_table: false, tables: ['verses', 'templates', 'output_channels', 'service_plans', 'plan_items', 'songs', 'song_sections', 'detections', 'services', 'service_events', 'perf_samples', 'voice_profiles', 'media_assets'].map((t) => ({ table: t, present: true })) }),
    channel_status: () => [],
    update_preflight: () => ({ ok: true, during_service: false, checks: [] }),
  };
  const withMock = (over = {}) => {
    const table = { ...HEALTHY, ...over };
    invoke.mockImplementation(async (cmd) => (cmd in table ? table[cmd]() : null));
  };
  const settleLong = async () => { for (let i = 0; i < 40; i++) await new Promise((r) => setTimeout(r, 0)); };

  it('says nothing is ready when the kiosk hub is not listening and a browser screen needs it', async () => {
    withMock({ probe_integrations: () => [{ label: 'Relay kiosk hub', listening: false, port: 8031 }, { label: 'Relay HTTP', listening: true, port: 8032 }] });
    const el = await mountDashboard({ model: '/m/ggml-large-v3-turbo.bin', language: 'en' });
    await settleLong();
    expect(text(el)).not.toMatch(/Ready for a service\./);
    expect(text(el)).toMatch(/8031/);
  });

  it('and a leftover scratch table from an interrupted migration is not "ready" either', async () => {
    withMock({ migration_status: () => ({ ...HEALTHY.migration_status(), scratch_table: true }) });
    const el = await mountDashboard({ model: '/m/ggml-large-v3-turbo.bin', language: 'en' });
    await settleLong();
    expect(text(el)).not.toMatch(/Ready for a service\./);
  });
});

describe('RG-190 · and a healthy machine still reads ready, so the two tests above mean something', () => {
  it('Ready for a service. when every probe answers well', async () => {
    const table = {
      ping: () => true,
      data_health: () => 31102,
      stt_status: () => ({ loaded: true, model: '/m/ggml-large-v3-turbo.bin', language: 'en' }),
      list_audio_devices: () => [{ id: 'a', name: 'Mic' }],
      list_monitors: () => [{ index: 0, name: 'Built-in', width: 1512, height: 982, primary: true }],
      list_output_channels: () => [{ id: 4, name: 'Lobby', render_target: 'network_client' }],
      local_ip: () => '192.168.1.42',
      system_hardware: () => ({ threads: 10, cores: 10, memory_total_gb: 32, memory_free_gb: 11, total_memory_bytes: 3.2e10, available_memory_bytes: 1.1e10, gpu_backends: ['Metal'], disk_free_gb: 300, total_disk_bytes: 9.9e11, free_disk_bytes: 3.1e11 }),
      probe_integrations: () => [{ label: 'Relay kiosk hub', listening: true, port: 8031 }, { label: 'Relay HTTP', listening: true, port: 8032 }],
      migration_status: () => ({ version: 5, expected: 5, manual_status: true, scratch_table: false, tables: ['verses', 'templates', 'output_channels', 'service_plans', 'plan_items', 'songs', 'song_sections', 'detections', 'services', 'service_events', 'perf_samples', 'voice_profiles', 'media_assets'].map((t) => ({ table: t, present: true })) }),
      channel_status: () => [],
      update_preflight: () => ({ ok: true, during_service: false, checks: [] }),
    };
    invoke.mockImplementation(async (cmd) => (cmd in table ? table[cmd]() : null));
    const el = await mountDashboard({ model: '/m/ggml-large-v3-turbo.bin', language: 'en' });
    for (let i = 0; i < 40; i++) await new Promise((r) => setTimeout(r, 0));
    expect(text(el)).toMatch(/Ready|worth a look/);
    expect(text(el)).not.toMatch(/not working/);
  });
});
