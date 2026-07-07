// Entry for the mobile stage-display remote (stage.html). Shares the design
// tokens (app.css :root) and self-hosted fonts with the console/output.
import './lib/fonts.js';
import './app.css';
import Stage from './Stage.svelte';

const app = new Stage({
  target: document.getElementById('stage'),
});

export default app;
