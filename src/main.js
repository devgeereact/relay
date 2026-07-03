// DEBUG: register error reporting FIRST, then log every boot milestone to the
// backend so a blank/broken console is diagnosable without devtools.
async function feLog(level, msg) {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('log_frontend', { level, msg: String(msg).slice(0, 400) });
  } catch {
    /* no backend */
  }
}
window.addEventListener('error', (e) => feLog('error', `${e.message} @ ${e.filename}:${e.lineno}`));
window.addEventListener('unhandledrejection', (e) => feLog('reject', e.reason?.stack ?? e.reason));
feLog('boot', 'main.js start');

let app;
try {
  await import('./lib/fonts.js');
  await import('./app.css');
  const { default: App } = await import('./App.svelte');
  feLog('boot', 'modules imported, mounting App');
  app = new App({ target: document.getElementById('app') });
  feLog('boot', 'App mounted OK');
} catch (e) {
  feLog('error', 'mount failed: ' + (e?.stack ?? e));
}

export default app;
