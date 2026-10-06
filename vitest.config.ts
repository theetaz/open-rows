// SPDX-License-Identifier: Apache-2.0
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // Bound DOM environments to avoid resource contention on shared runners.
    maxWorkers: 4,
    // Full-workspace render tests take several seconds on 2-vCPU CI runners.
    testTimeout: process.env.CI ? 30_000 : 10_000,
    environment: 'jsdom',
    setupFiles: ['apps/desktop/src/test/setup.ts'],
    include: ['{apps,packages}/**/src/**/*.test.{ts,tsx}'],
    clearMocks: true,
  },
});
