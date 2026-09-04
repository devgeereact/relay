import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Relay = project block NN=03 in the workspace port registry
// (~/.claude/CLAUDE.md, "Dev-server ports").
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
  // A second checkout of this repo keeps 5032 deliberately: a diverging port
  // committed in a branch lands on main at merge. To run two copies at once,
  // override at the CLI for the one you care less about, and never in this file:
  // npm run dev -- --port 5033
  server: {
    port: 5032,
    strictPort: true,
    // Bind all interfaces so LAN devices (kiosk screens, OBS on another machine)
    // can load the output page over http://<this-machine-ip>:5032 during dev.
    //
    // ⚠️ WEIGH THIS BEFORE RUNNING `tauri dev` ON A CHURCH NETWORK (RG-98).
    // `npm audit` reports ten vulnerabilities and **every one of them is in a dev
    // tool** — production dependencies are clean (`npm audit --omit=dev` → 0). Two
    // of them are reachable precisely because of this line: Vite's path traversal
    // in optimized-deps `.map` handling (HIGH), and esbuild's "any website can send
    // any request to the dev server and read the response" (MODERATE). Both are
    // dev-server bugs, and this dev server is deliberately on the LAN.
    //
    // Every fix is a semver MAJOR (vite 5 → 8, vitest 2 → 4, svelte 4 → 5), so none
    // of them is an audit-pass edit — svelte 4 is a recorded stack choice and a
    // runes migration would touch every component. **The exposure only exists while
    // `npm run tauri dev` is running**, and a packaged Relay has no server on 5032
    // at all. Leaving the default LAN-bound is a deliberate choice, not an
    // oversight: set `host: 'localhost'` here for a session on a network you do not
    // control, and accept that an OBS machine can no longer reach the dev output
    // page while you do.
    host: true,
  },
  // Same port as dev: `vite preview` otherwise drifts to the shared 4173,
  // which no project owns and which collides across checkouts.
  preview: {
    port: 5032,
    strictPort: true,
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
