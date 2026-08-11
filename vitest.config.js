import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.js'],
    // Repairs the Web Storage globals that Node >= 22 stubs out from under jsdom.
    // Without it the suite only passes on Node 20. See src/test-setup.js.
    setupFiles: ['./src/test-setup.js'],
    restoreMocks: true,
  },
});
