// SPDX-License-Identifier: Apache-2.0
import { gzipSync } from 'node:zlib';
import type { Plugin } from 'vite';

/** Gate the complete static entry graph, excluding workers and lazy imports. */
export function bundleCheck(): Plugin {
  return {
    name: 'initial-bundle-budget',
    writeBundle(_, bundle) {
      const initial = new Set<string>();
      const visit = (name: string): void => {
        if (initial.has(name)) return;
        const chunk = bundle[name];
        if (!chunk || chunk.type !== 'chunk') return;
        initial.add(name);
        chunk.imports.forEach(visit);
      };
      for (const chunk of Object.values(bundle)) {
        if (chunk.type === 'chunk' && chunk.isEntry) visit(chunk.fileName);
      }
      let bytes = 0;
      for (const name of initial) {
        const chunk = bundle[name];
        if (!chunk || chunk.type !== 'chunk') continue;
        if (chunk.moduleIds.some((id) => id.includes('/apache-arrow/')))
          this.error('apache-arrow found in initial chunk');
        bytes += gzipSync(chunk.code).length;
      }
      if (bytes > 350_000)
        this.error(`Initial JS exceeds 350 KB gzip: ${String(bytes)}`);
      this.info(
        `Initial JS ${String(bytes)} gzip bytes / 350000; apache-arrow absent`,
      );
    },
  };
}
