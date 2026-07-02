<script>
  // Phase 1 static list. Phase 7 populates from output_channels (SQLite) and
  // wires live status from the render-target runtime. render_target values
  // match the schema CHECK: native_window / ndi_encode / network_client.
  const rows = [
    { chip: 'var(--amber)',      name: 'Main screen',   target: 'HDMI',    dest: 'Display 2 — Sanctuary', tmpl: 'Classic Serif', online: true,  action: 'Edit' },
    { chip: 'var(--teal)',       name: 'Stage display', target: 'NDI',     dest: '→ ProPresenter',        tmpl: 'Stage Mono',    online: true,  action: 'Edit' },
    { chip: 'var(--violet)',     name: 'Streaming',     target: 'NDI',     dest: '→ OBS input 3',         tmpl: 'Lower Third',   online: true,  action: 'Edit' },
    { chip: 'var(--rose)',       name: 'Lobby screen',  target: 'Network', dest: 'Kiosk-Lobby-01 (Pi)',   tmpl: 'Lobby Warm',    online: true,  action: 'Edit' },
    { chip: 'var(--text-faint)', name: 'Kids overflow', target: 'Network', dest: 'Kiosk-Kids-01 (Pi)',    tmpl: 'Lobby Warm',    online: false, action: 'Edit' },
  ];
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
          {#if r.online}<span class="status-ok">● Online</span>{:else}<span class="status-off">○ Offline</span>{/if}
        </td>
        <td><button class="btn-ghost">{r.action}</button></td>
      </tr>
    {/each}
  </table>
  <button class="ctrl-btn primary" style="margin-top:14px;">+ Add output channel</button>
</div>
