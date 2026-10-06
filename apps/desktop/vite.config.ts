// SPDX-License-Identifier: Apache-2.0
import { defineConfig } from 'vite';
import type { Plugin } from 'vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';

// Vite's manifest omits module identities; emit evidence for the single size gate.
function chunkModules(): Plugin {
  return {
    name: 'chunk-module-metadata',
    generateBundle(_, bundle) {
      const modules: Record<string, string[]> = {};
      for (const chunk of Object.values(bundle)) {
        if (chunk.type === 'chunk') modules[chunk.fileName] = chunk.moduleIds;
      }
      this.emitFile({
        type: 'asset',
        fileName: '.vite/chunk-modules.json',
        source: JSON.stringify(modules),
      });
    },
  };
}

export default defineConfig({
  plugins: [
    react(),
    babel({ presets: [reactCompilerPreset()] }),
    chunkModules(),
  ],
  server: {
    port: 1420,
    strictPort: true,
    host: '127.0.0.1',
  },
  clearScreen: false,
  build: { target: 'safari15', manifest: true },
  worker: { format: 'iife' },
});
