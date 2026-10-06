// SPDX-License-Identifier: Apache-2.0
import { readFile, writeFile } from 'node:fs/promises';
import { generateCss } from './token-css.ts';

const target = new URL('../src/tokens/tokens.css', import.meta.url);
const css = generateCss();
if (process.argv.includes('--check')) {
  if ((await readFile(target, 'utf8')) !== css) {
    process.stderr.write('tokens.css is stale; run just gen-tokens\n');
    process.exitCode = 1;
  } else process.stdout.write('tokens.css is fresh\n');
} else await writeFile(target, css);
