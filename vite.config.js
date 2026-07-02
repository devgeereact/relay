import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Relay = client NN=03 in the global port registry (~/.claude/CLAUDE.md).
// App surface (operator console) is pinned to 5032; strictPort makes a clash
// fail loud instead of drifting. Tauri's devUrl in tauri.conf.json must match.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5032,
    strictPort: true,
  },
  build: {
    // Two entries: the operator console (index.html) and the native output
    // window (output.html) that channels.rs opens per output channel.
    rollupOptions: {
      input: {
        main: 'index.html',
        output: 'output.html',
      },
    },
  },
});
