// The REAL probes behind the launch checks.
//
// One function per non-stub check in boot.js. Each returns
// `{ state, note }` and NEVER throws for an expected condition — "no network"
// and "no model yet" are facts about a church laptop, not errors, and a boot
// screen that paints them red teaches an operator to ignore red.
//
// The severity ladder used here:
//   ok      — probed, and it is what a working install looks like.
//   warn    — probed, and Relay will run, but something is missing that the
//             operator will notice later (no model = no speech recognition).
//   fail    — probed, and a core promise is broken (no database).
//   unknown — see boot.js. Only ever for `probe: 'stub'` checks.
//
// Every command name used here is asserted by src/lib/ipc.test.js.

import { invoke as tauriInvoke } from '@tauri-apps/api/core';

/** Bytes → the unit a human uses when deciding if there is enough. */
function gb(bytes) {
  return `${(bytes / 1e9).toFixed(1)} GB`;
}

/**
 * The STT model an operator is most likely to run, in bytes. Used to turn free
 * disk into an answer ("room for a model") rather than a number they have to do
 * arithmetic on. `ggml-base` is ~148 MB; leave real headroom for media too.
 */
const HEADROOM_BYTES = 2e9;

/** Injected in tests. */
export function makeProbes({ invoke = tauriInvoke, getVersion } = {}) {
  const version = getVersion ?? (async () => (await import('@tauri-apps/api/app')).getVersion());

  // Read ONCE per boot and share. Four hardware checks and four migration checks
  // are four views of one snapshot each; calling the command per row would let
  // the rows disagree with each other, which on a screen whose entire job is
  // being trustworthy is worse than being slow.
  //
  // Memoise the PROMISE, not the resolved value. `x ??= await invoke(…)` yields
  // before it assigns, so every concurrent caller still sees null and fires its
  // own request — four commands where one was intended. It happens not to bite
  // in the sequencer (which awaits each check in turn) and would have bitten the
  // moment anything ran two rows at once.
  let hw = null;
  const hardware = () => (hw ??= invoke('system_hardware'));
  let mig = null;
  const migration = () => (mig ??= invoke('migration_status'));
  let ports = null;
  const integrations = () => (ports ??= invoke('probe_integrations'));
  const port = async (label) => (await integrations()).find((p) => p.label === label);

  return {
    // ── Diagnostics ────────────────────────────────────────────────────────
    async engine() {
      // `ping`, NOT `greet`. `greet` prints the console's boot heartbeat, and
      // that line is only worth anything if it appears exactly once per launch —
      // this probe runs from the launch sequence AND the Dashboard, so calling it
      // here printed "webview up" three times and made a healthy boot
      // indistinguishable from a webview reloading. See `greet` in main.rs.
      await invoke('ping');
      return { state: 'ok', note: 'attached' };
    },
    async version() {
      const v = await version();
      return { state: 'ok', note: `v${v}` };
    },
    async database() {
      const verses = await invoke('data_health');
      if (!verses) {
        // The KJV is bundled and loaded on first open, so an empty verse table
        // means the database did open but is not usable for the one thing Relay
        // exists to do. That is a failure, not a warning.
        return { state: 'fail', note: 'no scripture loaded' };
      }
      return { state: 'ok', note: `${verses.toLocaleString()} verses` };
    },
    async stt() {
      const s = await invoke('stt_status');
      if (!s?.loaded) {
        return { state: 'warn', note: 'no model — detection will not hear anything' };
      }
      return { state: 'ok', note: `${s.model ?? 'model'} · ${s.language ?? 'auto'}` };
    },
    async audio() {
      const devices = await invoke('list_audio_devices');
      if (!devices?.length) return { state: 'fail', note: 'no input devices' };
      const def = devices.find((d) => d.is_default) ?? devices[0];
      return { state: 'ok', note: `${devices.length} input${devices.length > 1 ? 's' : ''} · ${def.name}` };
    },
    async network() {
      const ip = await invoke('local_ip');
      // Offline is the DESIGNED state. It is reported, in plain words, as a
      // normal outcome — never as a problem to be fixed.
      return ip
        ? { state: 'ok', note: `${ip} — LAN outputs reachable` }
        : { state: 'ok', note: 'offline — every core feature still works' };
    },

    // ── Hardware ───────────────────────────────────────────────────────────
    async inputs() {
      const devices = await invoke('list_audio_devices');
      if (!devices?.length) return { state: 'fail', note: 'no capture device' };
      return { state: 'ok', note: devices.map((d) => d.name).join(', ') };
    },
    async displays() {
      const monitors = await invoke('list_monitors');
      if (!monitors?.length) return { state: 'warn', note: 'none reported' };
      const named = monitors
        .map((m) => (m.size ? `${m.name} ${m.size.width}×${m.size.height}` : m.name))
        .join(' · ');
      return { state: 'ok', note: named };
    },
    async lan() {
      const ip = await invoke('local_ip');
      return ip
        ? { state: 'ok', note: `http://${ip}:8032/output.html` }
        : { state: 'warn', note: 'no LAN address — browser sources are local-only' };
    },
    async cpu() {
      const h = await hardware();
      if (!h.cores) {
        // available_parallelism can genuinely fail. Say that, rather than
        // printing a plausible number nobody measured.
        return { state: 'warn', note: 'the OS would not report a thread count' };
      }
      const phys = h.physical_cores ? ` (${h.physical_cores} physical)` : '';
      // whisper on the CPU wants real parallelism. Two threads will transcribe,
      // slowly enough that the detector is working on a sermon that has moved on.
      const state = h.cores >= 4 ? 'ok' : 'warn';
      const note =
        state === 'ok'
          ? `${h.cores} threads${phys} · ${h.arch}`
          : `only ${h.cores} threads${phys} — transcription will lag the preacher`;
      return { state, note };
    },
    async memory() {
      const h = await hardware();
      const free = h.available_memory_bytes;
      // The base model plus the app sits around 1 GB resident. Below that free,
      // a church laptop will swap mid-sermon, which is where the freezes come from.
      const state = free >= 1.5e9 ? 'ok' : free > 0 ? 'warn' : 'fail';
      return {
        state,
        note:
          state === 'ok'
            ? `${gb(free)} free of ${gb(h.total_memory_bytes)}`
            : `${gb(free)} free of ${gb(h.total_memory_bytes)} — close other apps before the service`,
      };
    },
    async gpu() {
      const h = await hardware();
      // A BUILD fact, not a hardware one (sysprobe.rs). Naming the GPU in this
      // machine next to a CPU-only build would be the most convincing lie on the
      // screen — so this reports what whisper.cpp was actually compiled with.
      if (!h.gpu_backends?.length) {
        // Short enough to sit on one line in the note column — the two-line
        // version orphaned the word "in" on its own right-aligned row.
        return { state: 'ok', note: 'CPU only — no GPU backend in this build' };
      }
      return { state: 'ok', note: `${h.gpu_backends.join(', ')} compiled in` };
    },
    async disk() {
      const h = await hardware();
      if (!h.total_disk_bytes) {
        return { state: 'warn', note: 'could not identify the app-data volume' };
      }
      const free = h.free_disk_bytes;
      const state = free >= HEADROOM_BYTES ? 'ok' : 'warn';
      return {
        state,
        note:
          state === 'ok'
            ? `${gb(free)} free on ${h.disk_mount}`
            : `only ${gb(free)} free on ${h.disk_mount} — not enough for a model`,
      };
    },

    // ── Plugins / integrations ─────────────────────────────────────────────
    // "Plugin" in Relay means an OUTPUT or CONTROL surface. There is no plugin
    // loader and no third-party code is executed at boot; these report what the
    // shipped integrations can actually do today.
    async kiosk() {
      const channels = await invoke('list_output_channels');
      return { state: 'ok', note: `${channels?.length ?? 0} channel(s) · ws://…:8031` };
    },
    async http() {
      const ip = await invoke('local_ip');
      return { state: 'ok', note: `:8032 — ${ip ? `http://${ip}:8032` : 'localhost only'}` };
    },
    async propresenter() {
      // Import-only, and honest about it: Relay reads .pro files, it does not
      // drive ProPresenter.
      return { state: 'ok', note: 'import only — no live control' };
    },
    async ndi() {
      // CLAUDE.md: parked, needs a proprietary SDK. `open_ndi_output` returns a
      // clear error. The boot screen says the same thing the command says.
      return { state: 'warn', note: 'not available in this build — needs the NDI SDK' };
    },
    async obs() {
      const p = await port('OBS WebSocket');
      // "Something is listening on 4455" is the STRONGEST claim Relay is
      // entitled to make: it does not speak the OBS WebSocket protocol, so it
      // cannot know the thing that answered is OBS. Worded accordingly.
      return p?.listening
        ? { state: 'ok', note: `something is listening on :${p.port} — Relay cannot control it` }
        : { state: 'warn', note: `nothing on :${p?.port ?? 4455} — use a browser source instead` };
    },
    async atem() {
      const p = await port('ATEM');
      return p?.listening
        ? { state: 'ok', note: `something is listening on :${p.port} — Relay cannot control it` }
        : { state: 'warn', note: `nothing on :${p?.port ?? 9910} — bridge via HDMI or NDI hardware` };
    },

    // ── Migration ──────────────────────────────────────────────────────────
    // The runner executes once, synchronously, before this webview exists, so
    // there is nothing to stream. These ASK THE DATABASE what it looks like now
    // (`migration_status` → sqlite_master + pragma_table_info). The previous
    // version asserted "already applied" from a hard-coded list and would have
    // drawn green ticks over a schema missing every object it named.
    async schema() {
      const m = await migration();
      if (m.version !== m.expected) {
        return {
          state: 'fail',
          note: `database says v${m.version}, this build expects v${m.expected}`,
        };
      }
      return { state: 'ok', note: `v${m.version} — matches this build` };
    },
    async objects() {
      const m = await migration();
      const missing = m.tables.filter((t) => !t.present);
      if (missing.length) {
        return {
          state: 'fail',
          note: `missing: ${missing.map((t) => t.table).join(', ')}`,
        };
      }
      return { state: 'ok', note: `all ${m.tables.length} present` };
    },
    async manualstatus() {
      const m = await migration();
      // CLAUDE.md §14/§25. If this is false, the router is learning from a
      // corrupted status column — every human override logged as the AI's.
      return m.manual_status
        ? { state: 'ok', note: "detections.status accepts 'manual'" }
        : { state: 'fail', note: "detections.status cannot record a human's fire" };
    },
    async scratch() {
      const m = await migration();
      // CLAUDE.md §25: a leftover `detections_new` is what bricked every
      // subsequent boot. If one is present, say so while someone can still act.
      return m.scratch_table
        ? { state: 'fail', note: 'detections_new left behind by a failed rebuild' }
        : { state: 'ok', note: 'none — the rebuild completed cleanly' };
    },
  };
}
