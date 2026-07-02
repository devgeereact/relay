<script>
  import { capture, openOutput } from '../stores/capture.js';

  // Phase 7: the native_window render target is live — "Open" launches a real
  // fullscreen output window on this channel's template. NDI/kiosk targets and
  // DB-backed channel config come later. `tmplId` maps to src/lib/templates.js.
  const rows = [
    { chip: 'var(--amber)',      name: 'Main screen',   target: 'HDMI',    dest: 'Display 2 — Sanctuary', tmpl: 'Classic Serif', tmplId: 'main',   native: true },
    { chip: 'var(--teal)',       name: 'Stage display', target: 'NDI',     dest: '→ ProPresenter',        tmpl: 'Stage Mono',    tmplId: 'stage',  native: false },
    { chip: 'var(--violet)',     name: 'Streaming',     target: 'NDI',     dest: '→ OBS input 3',         tmpl: 'Lower Third',   tmplId: 'stream', native: false },
    { chip: 'var(--rose)',       name: 'Lobby screen',  target: 'Network', dest: 'Kiosk-Lobby-01 (Pi)',   tmpl: 'Lobby Warm',    tmplId: 'lobby',  native: false },
    { chip: 'var(--text-faint)', name: 'Kids overflow', target: 'Network', dest: 'Kiosk-Kids-01 (Pi)',    tmpl: 'Lobby Warm',    tmplId: 'lobby',  native: false },
  ];

  let error = '';
  async function open(row) {
    try {
      await openOutput(row.tmplId, row.name);
      error = '';
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="panel">
  <div class="panel-title">Output channels <span class="count">{rows.length} configured</span></div>
  <table class="data-table">
    <tr><th>Channel</th><th>Render target</th><th>Destination</th><th>Template</th><th>Status</th><th></th></tr>
    {#each rows as r}
      <tr>
        <td><span class="chip" style="background:{r.chip};"></span> {r.name}</td>
        <td class="mono">{r.target}</td>
        <td class="mono">{r.dest}</td>
        <td>{r.tmpl}</td>
        <td>
          {#if r.native}<span class="status-ok">● native ready</span>{:else}<span class="status-off">○ {r.target.toLowerCase()} (Phase 10)</span>{/if}
        </td>
        <td>
          {#if r.native}
            <button class="btn-confirm" on:click={() => open(r)} disabled={!$capture.available}>Open</button>
          {:else}
            <button class="btn-ghost" disabled>Open</button>
          {/if}
        </td>
      </tr>
    {/each}
  </table>
  {#if error}<div style="color:var(--red); font-size:12px; margin-top:10px;">{error}</div>{/if}
  <div style="font-family:var(--f-mono); font-size:11px; color:var(--text-faint); margin-top:12px;">
    Native output windows open fullscreen on this machine. NDI/kiosk targets land in Phase 10.
  </div>
</div>
