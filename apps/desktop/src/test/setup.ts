// SPDX-License-Identifier: Apache-2.0
import '@testing-library/jest-dom/vitest';
import { performance as nodePerformance } from 'node:perf_hooks';
import { cleanup, configure } from '@testing-library/react';
import { afterEach, beforeEach, vi } from 'vitest';

// JSDOM lacks User Timing; keep its page clock separate from the worker's clock.
if (typeof performance.mark !== 'function')
  Object.defineProperty(performance, 'mark', {
    configurable: true,
    writable: true,
    value: nodePerformance.mark.bind(nodePerformance),
  });

// Lazy chunks resolve slowly on 2-vCPU CI runners.
configure({ asyncUtilTimeout: process.env.CI ? 5_000 : 1_000 });

const createRange = document.createRange.bind(document);
let restoreRange = (): void => undefined;

beforeEach(() => {
  localStorage.clear();
  // jsdom has no layout; CodeMirror's scheduled measurements still use these APIs.
  const rangeSpy = vi.spyOn(document, 'createRange').mockImplementation(() => {
    const range = createRange();
    range.getBoundingClientRect = () => new DOMRect();
    range.getClientRects = () => Object.assign([], { item: () => null });
    return range;
  });
  restoreRange = () => {
    rangeSpy.mockRestore();
  };
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe(): void {}
      unobserve(): void {}
      disconnect(): void {}
    },
  );
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({
      matches: false,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  );
});

afterEach(() => {
  cleanup();
  restoreRange();
});
