import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  // WITHOUT THIS LINE, `onMount` DOES NOTHING IN ANY TEST IN THIS REPO.
  //
  // Svelte 4's package exports map `.` to `src/runtime/ssr.js` under every
  // resolution condition except `browser`, and that file is, verbatim:
  //
  //     export function onMount() {}
  //     export function beforeUpdate() {}
  //     export function afterUpdate() {}
  //
  // `environment: 'jsdom'` does not imply the `browser` condition. So a component
  // was compiled for the DOM and then handed the SSR stubs: it mounted, rendered,
  // and silently skipped every load-on-mount path. No list ever fetched, no event
  // subscription ever ran, and a mount test whose setup never executed passed by
  // doing nothing. `liveoutputrail.test.js` mocked `list_output_channels` "for
  // onMount" against a call that had never once happened.
  //
  // The asymmetry is what made it invisible: `onDestroy` and `tick` resolve from
  // the DOM runtime and are REAL, so teardown assertions worked and nobody
  // suspected the other half. Architecture rule 1 names `afterUpdate` as the safe
  // alternative to a reactive `tick()` — untestable here until this line existed.
  //
  // Found 2026-08-14 by four agents independently (R3-00 / R2-G / R6 P1-11).
  resolve: { conditions: ['browser'] },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.js'],
    // Repairs the Web Storage globals that Node >= 22 stubs out from under jsdom.
    // Without it the suite only passes on Node 20. See src/test-setup.js.
    setupFiles: ['./src/test-setup.js'],
    restoreMocks: true,
  },
});
