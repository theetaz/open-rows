// SPDX-License-Identifier: Apache-2.0
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    setupFiles: ['apps/desktop/src/test/setup.ts'],
    include: ['{apps,packages}/**/src/**/*.test.{ts,tsx}'],
    clearMocks: true,
  },
});
