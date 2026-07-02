// Entry for native output windows (channels.rs loads output.html). Shares the
// design tokens (app.css :root) and self-hosted fonts with the console.
import './lib/fonts.js';
import './app.css';
import Output from './Output.svelte';

const app = new Output({
  target: document.getElementById('output'),
});

export default app;
