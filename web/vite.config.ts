import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

// The site is served from a project GitHub Pages path (/<repo>/), so assets must
// be referenced relative to that base. Override with VITE_BASE for other hosts.
const base = process.env.VITE_BASE ?? './';

export default defineConfig({
  base,
  plugins: [svelte()],
  build: {
    target: 'es2022',
    outDir: 'dist',
  },
  // Under Vitest, resolve Svelte's *browser* build so component tests can mount.
  resolve: process.env.VITEST ? { conditions: ['browser'] } : undefined,
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./vitest-setup.ts'],
  },
});
