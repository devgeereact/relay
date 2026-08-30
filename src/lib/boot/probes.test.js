// The real launch probes.
//
// These turn a raw number into a VERDICT an operator acts on, and the verdict is
// the part that can be wrong in a way nobody notices. A laptop with 2 threads
// that reports "ok" is a laptop that will lag the preacher while the boot screen
// says everything is fine.
//
// Every test is the bug, not the fix.

import { describe, it, expect, vi } from 'vitest';
import { makeProbes } from './probes.js';

/** A backend that answers with whatever you hand it. */
function backend(answers) {
  return vi.fn(async (cmd) => {
    if (!(cmd in answers)) throw new Error(`unexpected command: ${cmd}`);
    const v = answers[cmd];
    if (v instanceof Error) throw v;
    return v;
  });
}

const HW = {
  cores: 8,
  physical_cores: 4,
  total_memory_bytes: 16e9,
  available_memory_bytes: 6e9,
  free_disk_bytes: 120e9,
  total_disk_bytes: 500e9,
  disk_mount: '/',
  gpu_backends: [],
  os: 'macOS 15.0',
  arch: 'aarch64',
};

const MIG = {
  version: 1,
  expected: 1,
  tables: [
    { label: 'Core tables', table: 'detections', present: true },
    { label: 'Songs', table: 'songs', present: true },
  ],
  manual_status: true,
  scratch_table: false,
};

const probesWith = (over = {}, migOver = {}, ports = []) =>
  makeProbes({
    invoke: backend({
      system_hardware: { ...HW, ...over },
      migration_status: { ...MIG, ...migOver },
      probe_integrations: ports,
    }),
    getVersion: async () => '0.1.0',
  });

describe('hardware probes', () => {
  it('reads the host once and shares it across every row', async () => {
    // Four rows, one snapshot. Calling per row lets the rows disagree with each
    // other on a screen whose only job is being trustworthy.
    const invoke = backend({ system_hardware: HW });
    const p = makeProbes({ invoke });
    await Promise.all([p.cpu(), p.memory(), p.gpu(), p.disk()]);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('warns about a machine that cannot keep up with a preacher', async () => {
    const r = await probesWith({ cores: 2, physical_cores: 2 }).cpu();
    expect(r.state).toBe('warn');
    expect(r.note).toMatch(/lag the preacher/);
  });

  it('passes a machine with enough threads', async () => {
    const r = await probesWith().cpu();
    expect(r.state).toBe('ok');
    expect(r.note).toContain('8 threads');
  });

  it('says the OS would not answer rather than inventing a core count', async () => {
    const r = await probesWith({ cores: null }).cpu();
    expect(r.state).toBe('warn');
    expect(r.note).toMatch(/would not report/);
  });

  it('warns when free memory would make Relay swap mid-sermon', async () => {
    const r = await probesWith({ available_memory_bytes: 0.6e9 }).memory();
    expect(r.state).toBe('warn');
    expect(r.note).toMatch(/close other apps/i);
  });

  it('reports GPU as a BUILD fact, never as a fact about the machine', async () => {
    // The bug this exists for: printing the machine's GPU next to a CPU-only
    // build. It would be the most convincing lie on the screen.
    const r = await probesWith({ gpu_backends: [] }).gpu();
    expect(r.note).toMatch(/CPU/);
  });

  it('WARNS about a CPU-only build on macOS — the same verdict degraded.js gives', async () => {
    // One fact, one verdict. `degraded.js` calls this "reduced" — the transcript
    // will lag the preacher on anything but the smallest model — and the launch
    // screen used to call the identical fact `ok`, in green, on the screen an
    // operator reads before a service. Rule 27 measured it: ~1710 ms per window
    // against a ~1000 ms budget, which is slower than real time.
    const r = await probesWith({ gpu_backends: [], os: 'macOS 15.0' }).gpu();
    expect(r.state).toBe('warn');
    expect(r.note).toMatch(/3x slower/);
  });

  it('…and stays ok off macOS, where nobody has measured it', async () => {
    // A warning nobody measured is a warning an operator learns to scroll past.
    const r = await probesWith({ gpu_backends: [], os: 'Windows 11' }).gpu();
    expect(r.state).toBe('ok');
    expect(r.note).toMatch(/no GPU backend in this build/);
  });

  it('the two surfaces cannot disagree — they ask the same function', async () => {
    const { gpuIsReduced } = await import('../degraded.js');
    const { degradations } = await import('../degraded.js');
    const state = { macos: true, gpuBackends: [] };
    expect(gpuIsReduced(state)).toBe(true);
    expect(degradations({ ...state, sttLoaded: true }).some((d) => d.id === 'gpu')).toBe(true);
    expect((await probesWith({ gpu_backends: [], os: 'macOS 15.0' }).gpu()).state).toBe('warn');
  });

  it('names the compiled backends when there are some', async () => {
    const r = await probesWith({ gpu_backends: ['Metal'] }).gpu();
    expect(r.note).toContain('Metal');
  });

  it('warns when there is not room for a model', async () => {
    const r = await probesWith({ free_disk_bytes: 0.4e9 }).disk();
    expect(r.state).toBe('warn');
    expect(r.note).toMatch(/not enough for a model/);
  });

  it('names the volume it measured so a human can check it', async () => {
    const r = await probesWith({ disk_mount: '/Volumes/Media' }).disk();
    expect(r.note).toContain('/Volumes/Media');
  });

  it('does not pretend to know the disk when the volume could not be found', async () => {
    const r = await probesWith({ total_disk_bytes: 0, free_disk_bytes: 0 }).disk();
    expect(r.state).toBe('warn');
    expect(r.note).toMatch(/could not identify/);
  });
});

describe('integration probes', () => {
  const ports = [
    { label: 'OBS WebSocket', port: 4455, listening: true },
    { label: 'ATEM', port: 9910, listening: false },
  ];

  it('never claims the listening thing IS OBS', async () => {
    // Relay does not speak the OBS WebSocket protocol. "OBS is running" is a
    // claim it is not entitled to make, and an operator would act on it.
    const r = await probesWith({}, {}, ports).obs();
    expect(r.state).toBe('ok');
    expect(r.note).toMatch(/something is listening/);
    expect(r.note).toMatch(/cannot control/);
    expect(r.note).not.toMatch(/OBS is running/);
  });

  it('names the port it looked at when nothing answered', async () => {
    // An operator who moved the port off the default needs to know which one
    // was tried, or "not detected" is unactionable.
    const r = await probesWith({}, {}, ports).atem();
    expect(r.state).toBe('warn');
    expect(r.note).toContain('9910');
  });

  it('still names a default port if the backend returned nothing at all', async () => {
    const r = await probesWith({}, {}, []).obs();
    expect(r.note).toContain('4455');
  });
});

describe('migration probes', () => {
  it('fails when the database version does not match the build', async () => {
    const r = await probesWith({}, { version: 0 }).schema();
    expect(r.state).toBe('fail');
    expect(r.note).toMatch(/v0.*expects v1/);
  });

  it('names the objects that are missing rather than a count', async () => {
    // The original bug: a hard-coded list drew green ticks whether or not the
    // tables were there. This must name what is gone, so it can be fixed.
    const r = await probesWith(
      {},
      { tables: [{ label: 'Songs', table: 'songs', present: false }] },
    ).objects();
    expect(r.state).toBe('fail');
    expect(r.note).toContain('songs');
  });

  it('fails when a human fire cannot be logged as manual', async () => {
    // CLAUDE.md §14: the self-calibrating router learns from that column. If it
    // cannot record 'manual', the router is training on a corrupted log.
    const r = await probesWith({}, { manual_status: false }).manualstatus();
    expect(r.state).toBe('fail');
  });

  it('surfaces a leftover scratch table', async () => {
    // CLAUDE.md §25: this is what made every subsequent boot fail, forever.
    const r = await probesWith({}, { scratch_table: true }).scratch();
    expect(r.state).toBe('fail');
    expect(r.note).toContain('detections_new');
  });

  it('reads the schema once for all four migration rows', async () => {
    const invoke = backend({ migration_status: MIG });
    const p = makeProbes({ invoke });
    await Promise.all([p.schema(), p.objects(), p.manualstatus(), p.scratch()]);
    expect(invoke).toHaveBeenCalledTimes(1);
  });
});

describe('diagnostics probes', () => {
  it('treats being offline as a normal, passing state', async () => {
    // Relay's entire premise is that it works with the cable out. Painting the
    // designed state amber teaches an operator that normal is broken.
    const p = makeProbes({ invoke: backend({ local_ip: null }) });
    const r = await p.network();
    expect(r.state).toBe('ok');
    expect(r.note).toMatch(/offline/);
  });

  it('fails — not warns — when there is no scripture to look up', async () => {
    const p = makeProbes({ invoke: backend({ data_health: 0 }) });
    expect((await p.database()).state).toBe('fail');
  });

  it('warns, not fails, when there is no STT model yet', async () => {
    // Relay runs fine without a model; the operator fires by hand. That is a
    // degraded install, not a broken one.
    const p = makeProbes({ invoke: backend({ stt_status: { loaded: false } }) });
    const r = await p.stt();
    expect(r.state).toBe('warn');
  });

  it('fails when there is no microphone at all', async () => {
    const p = makeProbes({ invoke: backend({ list_audio_devices: [] }) });
    expect((await p.audio()).state).toBe('fail');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// RG-53 · THE TWO ROWS THAT COULD ONLY EVER SAY "ok"
//
// The kiosk row counted configured channels and printed `ws://…:8031`. The HTTP
// row printed `http://<ip>:8032`. Neither asked whether anything was LISTENING —
// so both were structurally incapable of reporting a failure, on the one screen
// whose entire job is answering "is this machine going to work?".
//
// They are ordinary TCP ports on a volunteer's laptop and binding them can fail:
// another program has them, or a security tool refuses. When that happens every
// OBS browser source and the preacher's stage page go dead, and the operator has
// just been told the setup is fine.
//
// Same rule as `channel_status` and the output heartbeat: a control that cannot
// detect its own failure is not a control.
// ─────────────────────────────────────────────────────────────────────────────

const HUB_UP = { label: 'Relay kiosk hub', port: 8031, listening: true };
const HUB_DOWN = { label: 'Relay kiosk hub', port: 8031, listening: false };
const HTTP_UP = { label: 'Relay HTTP', port: 8032, listening: true };
const HTTP_DOWN = { label: 'Relay HTTP', port: 8032, listening: false };

/** The kiosk row also needs the channel list. */
const withPorts = (ports, channels = []) =>
  makeProbes({
    invoke: backend({
      probe_integrations: ports,
      list_output_channels: channels,
      local_ip: '192.168.1.9',
    }),
    getVersion: async () => '0.1.0',
  });

describe('RG-53 · the kiosk hub and the HTTP server are actually probed', () => {
  it('kiosk FAILS when nothing is listening and a browser source needs it', async () => {
    const p = withPorts([HUB_DOWN, HTTP_UP], [{ render_target: 'network_client' }]);
    const r = await p.kiosk();
    expect(r.state).toBe('fail');
    expect(r.note).toMatch(/nothing is listening on :8031/);
    // Name the consequence, not the port: an operator needs to know what breaks.
    expect(r.note).toMatch(/browser source/);
  });

  it('…and only WARNS when nothing is using it yet', async () => {
    // A church running one HDMI projector and no OBS loses nothing. Painting that
    // red teaches an operator to ignore red, which is how a real fault gets past.
    const p = withPorts([HUB_DOWN, HTTP_UP], [{ render_target: 'native_window' }]);
    const r = await p.kiosk();
    expect(r.state).toBe('warn');
    expect(r.note).toMatch(/no browser source needs it yet/);
  });

  it('kiosk is ok when the hub answers', async () => {
    const p = withPorts([HUB_UP, HTTP_UP], [{ render_target: 'network_client' }]);
    expect((await p.kiosk()).state).toBe('ok');
  });

  it('HTTP fails when nothing is listening, and says what stops working', async () => {
    const p = withPorts([HUB_UP, HTTP_DOWN]);
    const r = await p.http();
    expect(r.state).toBe('fail');
    // This is the URL an operator copies into OBS and types into a phone.
    expect(r.note).toMatch(/output pages, the stage page and media/);
  });

  it('HTTP is ok when the server answers', async () => {
    const p = withPorts([HUB_UP, HTTP_UP]);
    const r = await p.http();
    expect(r.state).toBe('ok');
    expect(r.note).toMatch(/192\.168\.1\.9:8032/);
  });

  it('neither row can report ok without the port answering', async () => {
    // The regression that matters: if somebody restores the old body, both of
    // these go green with the hub and the server dead.
    const p = withPorts([HUB_DOWN, HTTP_DOWN], [{ render_target: 'network_client' }]);
    expect((await p.kiosk()).state).not.toBe('ok');
    expect((await p.http()).state).not.toBe('ok');
  });
});
