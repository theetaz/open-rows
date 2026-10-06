// SPDX-License-Identifier: Apache-2.0
import { contrastRows } from '../src/tokens/contrast.ts';

const rows = contrastRows();
const themes = ['light', 'dark', 'high-contrast-light', 'high-contrast-dark'];
const pairs = [...new Set(rows.map((row) => row.pair.split('/')[0] ?? ''))];
process.stdout.write(
  '| Pair (minimum across surfaces) | Light | Dark | HC light | HC dark |\n| --- | --- | --- | --- | --- |\n',
);
for (const pair of pairs) {
  const ratios = themes.map((theme) =>
    Math.min(
      ...rows
        .filter(
          (row) =>
            row.theme === theme && (row.pair.split('/')[0] ?? '') === pair,
        )
        .map((row) => row.ratio),
    ).toFixed(2),
  );
  process.stdout.write(`| ${pair} | ${ratios.join(' | ')} |\n`);
}
process.stdout.write(
  `${String(rows.length)} pairs; ${String(rows.filter((row) => row.ratio < row.minimum).length)} below budget\n`,
);
