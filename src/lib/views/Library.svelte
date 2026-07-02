<script>
  // Phase 1 static list. Later reads from services + detections (SQLite),
  // all local-first — service history never leaves the device (CLAUDE.md).
  const rows = [
    { date: 'Today',  live: true,  title: 'Sunday Service — live',  dur: '24:10', verses: 2,  over: 0, action: 'Open' },
    { date: 'Jun 28', live: false, title: 'Sunday Service',         dur: '58:32', verses: 11, over: 2, action: 'Transcript' },
    { date: 'Jun 21', live: false, title: 'Sunday Service',         dur: '52:04', verses: 8,  over: 1, action: 'Transcript' },
    { date: 'Jun 14', live: false, title: 'Midweek Bible study',    dur: '41:17', verses: 15, over: 0, action: 'Transcript' },
  ];
</script>

<div class="panel">
  <div class="panel-title">Service library</div>
  <table class="data-table">
    <tr><th>Date</th><th>Title</th><th>Duration</th><th>Verses detected</th><th>Overrides</th><th></th></tr>
    {#each rows as r}
      <tr>
        <td class="mono" style={r.live ? 'color:var(--green);' : ''}>{r.date}</td>
        <td>{r.title}</td>
        <td class="mono">{r.dur}</td>
        <td class="mono">{r.verses}</td>
        <td class="mono">{r.over}</td>
        <td><button class="btn-ghost">{r.action}</button></td>
      </tr>
    {/each}
  </table>
</div>
