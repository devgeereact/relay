import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Relay = client NN=03 in the global port registry (~/.claude/CLAUDE.md).
// App surface (operator console) is pinned to 5032; strictPort makes a clash
// fail loud instead of drifting. Tauri's devUrl in tauri.conf.json must match.
// Svelte warnings that are not warnings — they are bugs that reach the operator
// as a blank tab mid-service, and the build must refuse to produce them.
//
// `missing-declaration` is how a real one got through: a store import was dropped
// during a refactor and `$capture.available` was left behind in the markup. Svelte
// printed "'capture' is not defined" and `vite build` exited 0. The app booted, the
// tests passed, CI was green — and the Planner tab would have thrown the moment an
// operator clicked it. Nothing in the pipeline was looking.
//
// The frontend has no type checker, so this is the seam that catches a bad
// reference. Keep the list narrow: only codes that mean "this will throw".
const FATAL_SVELTE_WARNINGS = new Set([
  'missing-declaration', // an undefined variable/store used in a component
]);

export default defineConfig({
  plugins: [
    svelte({
      onwarn(warning, defaultHandler) {
        if (FATAL_SVELTE_WARNINGS.has(warning.code)) {
          throw new Error(
            `[svelte] ${warning.code}: ${warning.message}\n` +
              `  at ${warning.filename}:${warning.start?.line}\n` +
              `  This throws at runtime. Fatal by policy — see vite.config.js.`,
          );
        }
        defaultHandler(warning);
      },
    }),
  ],
  clearScreen: false,
  server: {
    port: 5032,
    strictPort: true,
    // Bind all interfaces so LAN devices (kiosk screens, OBS on another machine)
    // can load the output page over http://<this-machine-ip>:5032 during dev.
    host: true,
  },
  build: {
    // Two entries: the operator console (index.html) and the native output
    // window (output.html) that channels.rs opens per output channel.
    rollupOptions: {
      input: {
        main: 'index.html',
        output: 'output.html',
        stage: 'stage.html',
      },
    },
  },
});
