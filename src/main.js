import './lib/fonts.js';
import './app.css';
import App from './App.svelte';
import { installCrashGuard } from './lib/crash.js';

// The crash guard goes up BEFORE the app is constructed, so an error thrown
// during initial render is caught too — that is exactly when a bad boot fails.
installCrashGuard();

const app = new App({
  target: document.getElementById('app'),
});

export default app;
