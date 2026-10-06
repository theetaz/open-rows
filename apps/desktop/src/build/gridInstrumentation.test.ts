// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { expect, it } from 'vitest';
import { instrumentReactCommit } from './gridInstrumentation';
it('instruments the synchronous React commit with finally cleanup', () => {
  const react = instrumentReactCommit(
    'function commitRoot(\n  root,\n  work\n) { return root;\n}\nfunction reportViewTransitionError(error) {}',
  );
  expect(react).toContain('function commitRoot(\n  root,');
  expect(react).toContain('finally { if (timing) timing.end(started); }');
  expect(() => instrumentReactCommit('changed API')).toThrow(
    'signature changed',
  );
});

it('matches the installed production React commit signature', () => {
  const require = createRequire(
    join(process.cwd(), 'apps/desktop/package.json'),
  );
  const client = require.resolve('react-dom/client');
  const code = readFileSync(
    join(dirname(client), 'cjs/react-dom-client.production.js'),
    'utf8',
  );
  expect(instrumentReactCommit(code)).toContain(
    'finally { if (timing) timing.end(started); }',
  );
});
