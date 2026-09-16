// Entry for native output windows and for every OBS/kiosk browser source
// (channels.rs serves output.html).
//
// THE CONSOLE'S STYLESHEET IS NOT IMPORTED HERE, deliberately. This page is a
// congregation screen: it takes the shared design tokens and the self-hosted
// fonts, and none of the operator console's rules. Svelte does not scope a global
// stylesheet, so importing `app.css` put every unscoped console rule on the wall —
// see `src/tokens.css` for what that cost and `src/lib/seal.test.js` for the rule
// that keeps it sealed. Wave 5, Track E.
import './lib/fonts.js';
import './tokens.css';
import Output from './Output.svelte';

const app = new Output({
  target: document.getElementById('output'),
});

export default app;
