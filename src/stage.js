// Entry for the mobile stage-display remote (stage.html) — the preacher's phone.
//
// Shares the design tokens and the self-hosted fonts with the console and the
// output window, and NOT the console's stylesheet: this page renders in front of
// the person preaching, and an unscoped console rule reaching it is the same leak
// `output.html` had. See `src/tokens.css` and `src/lib/seal.test.js`. Wave 5,
// Track E.
import './lib/fonts.js';
import './tokens.css';
import Stage from './Stage.svelte';

const app = new Stage({
  target: document.getElementById('stage'),
});

export default app;
