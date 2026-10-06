// SPDX-License-Identifier: Apache-2.0
import { readFile, readdir } from 'node:fs/promises';
import { gzipSync } from 'node:zlib';
import { initialFiles } from './initial-bundle.ts';
import type { BundleChunk } from './initial-bundle.ts';

const components = new URL('../src/components/', import.meta.url);
const stylesheets = (await readdir(components)).filter((name) =>
  name.endsWith('.css'),
);
const css = [
  await readFile(new URL('../src/tokens/tokens.css', import.meta.url), 'utf8'),
];
for (const name of stylesheets)
  css.push(await readFile(new URL(name, components), 'utf8'));
const uiSize = gzipSync(css.join('\n')).length;
process.stdout.write(
  `Tokens + component CSS: ${String(uiSize)} gzip bytes / 15000 budget\n`,
);
const assets = new URL('../../../apps/desktop/dist/assets/', import.meta.url);
for (const name of await readdir(assets)) {
  const size = gzipSync(await readFile(new URL(name, assets))).length;
  process.stdout.write(`${name}: ${String(size)} gzip bytes\n`);
  if (name.endsWith('.js') && size > 350000) process.exitCode = 1;
}
if (uiSize > 15000) process.exitCode = 1;

const dist = new URL('../../../apps/desktop/dist/', import.meta.url);
const manifest = JSON.parse(
  await readFile(new URL('.vite/manifest.json', dist), 'utf8'),
) as Record<string, BundleChunk>;
let initialSize = 0;
for (const file of initialFiles(manifest))
  initialSize += gzipSync(await readFile(new URL(file, dist))).length;
process.stdout.write(
  `Initial JS (all static chunks): ${String(initialSize)} gzip bytes / 350000 budget\n`,
);
if (initialSize > 350000) process.exitCode = 1;
