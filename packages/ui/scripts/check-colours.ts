// SPDX-License-Identifier: Apache-2.0
import { readdir, readFile } from 'node:fs/promises';
import { hasRawColour } from './raw-colour.ts';

async function scan(directory: URL): Promise<void> {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = new URL(
      entry.name + (entry.isDirectory() ? '/' : ''),
      directory,
    );
    if (entry.isDirectory()) await scan(path);
    else if (entry.name.endsWith('.module.css')) {
      const css = await readFile(path, 'utf8');
      if (hasRawColour(css)) {
        process.stderr.write(`Raw colour in ${path.pathname}; use a token\n`);
        process.exitCode = 1;
      }
    }
  }
}
await scan(new URL('../src/', import.meta.url));
await scan(new URL('../../../apps/desktop/src/', import.meta.url));
if (!process.exitCode)
  process.stdout.write('Component CSS uses colour tokens only\n');
