import './lib/fonts.js';
import './app.css';
import App from './App.svelte';

// DEBUG: forward frontend errors + a heartbeat to the backend log so a webview
// freeze/exception is visible without devtools. Remove once diagnosed.
async function feLog(level, msg) {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('log_frontend', { level, msg: String(msg).slice(0, 300) });
  } catch {
    /* no backend */
  }
}
window.addEventListener('error', (e) => feLog('error', `${e.message} @ ${e.filename}:${e.lineno}`));
window.addEventListener('unhandledrejection', (e) => feLog('reject', e.reason?.message ?? e.reason));

const app = new App({
  target: document.getElementById('app'),
});

export default app;
