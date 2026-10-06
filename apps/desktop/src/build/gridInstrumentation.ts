// SPDX-License-Identifier: Apache-2.0
import type { Plugin } from 'vite';
/** Time synchronous React commits only while the grid spike registers an observer. */
export function gridInstrumentation(): Plugin {
  return {
    name: 'grid-spike-commit-timing',
    transform(code, id) {
      if (
        id
          .replaceAll('\\', '/')
          .endsWith('/react-dom/cjs/react-dom-client.production.js')
      )
        return instrumentReactCommit(code);
    },
  };
}
/** Time the complete synchronous production commit, including layout effects and nested paints. */
export function instrumentReactCommit(code: string): string {
  const start = code.indexOf('function commitRoot(\n  root,');
  const body = start < 0 ? -1 : code.indexOf(') {', start) + 3;
  const end =
    body < 0
      ? -1
      : code.indexOf('\n}\nfunction reportViewTransitionError(error) {', body);
  if (start < 0 || body <= start || end < body)
    throw new Error('React 19.3 commit signature changed');
  return (
    code.slice(0, body) +
    ' const timing = globalThis.__openrowsGridTiming; const started = timing ? timing.begin() : 0; try {' +
    code.slice(body, end) +
    '} finally { if (timing) timing.end(started); }' +
    code.slice(end)
  );
}
