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

/**
 * Does the dev server listen on the LAN? Off unless asked (RG-98, DECISIONS §65).
 *
 * `true` binds every interface, `false` binds loopback. See the comment at
 * `server.host` for why the default moved and what it costs.
 */
const DEV_LAN = process.env.RELAY_DEV_LAN === '1';

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
  // WHO CAN REACH THE DEV SERVER (RG-98, DECISIONS §65).
  //
  // This bound every interface. The reason was real — a kiosk screen or an OBS
  // machine loading `http://<this-machine-ip>:5032/output.html` during development
  // — and it put a Vite dev server on a church LAN, which is where the cost is.
  // `npm audit` reports ten advisories and **every one is in dev tooling**
  // (production is clean: `npm audit --omit=dev` → 0). Two of them are reachable
  // exactly because of this: Vite's path traversal in optimized-deps `.map`
  // handling (HIGH) and esbuild's *"any website can send any request to the dev
  // server and read the response"* (MODERATE). Every fix is a semver major (vite
  // 5 → 8, vitest 2 → 4, svelte 4 → 5) — Svelte 4 is a recorded stack choice and a
  // runes migration would touch every component — so the version numbers are not
  // the lever. The lever is who can connect.
  //
  // **The default is now loopback.** A packaged Relay has no server on 5032 at all
  // (that is `:8032`, the Rust one), so the LAN case is a DEVELOPMENT convenience,
  // and a convenience should not be the thing that is on by default on somebody
  // else's network. Set `RELAY_DEV_LAN=1` for the session where you actually need
  // an OBS machine to reach the dev output page:
  //
  //   RELAY_DEV_LAN=1 npm run tauri dev
  //
  // It is an env var and not a config edit so that turning it on cannot be
  // committed by accident, which is the shape this whole finding has.
  server: {
    port: 5032,
    strictPort: true,
    host: DEV_LAN,
  },
  // Same port as dev: `vite preview` otherwise drifts to the shared 4173,
  // which no project owns and which collides across checkouts. Same LAN rule.
  preview: {
    port: 5032,
    strictPort: true,
    host: DEV_LAN,
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
