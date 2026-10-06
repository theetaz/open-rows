// SPDX-License-Identifier: Apache-2.0
/** Build-manifest fields used to account for synchronous startup dependencies. */
export type BundleChunk = {
  file: string;
  isEntry?: boolean;
  imports?: string[];
};
/** Count entries and their static imports once; dynamic imports load on demand. */
export function initialFiles(manifest: Record<string, BundleChunk>): string[] {
  const entries = Object.keys(manifest).filter((key) => manifest[key]?.isEntry);
  if (!entries.length) throw new Error('Build manifest has no initial entry');
  const seen = new Set<string>();
  const files = new Set<string>();
  function visit(key: string): void {
    if (seen.has(key)) return;
    const chunk = manifest[key];
    if (!chunk) throw new Error(`Missing build manifest dependency: ${key}`);
    seen.add(key);
    files.add(chunk.file);
    for (const dependency of chunk.imports ?? []) visit(dependency);
  }
  for (const entry of entries) visit(entry);
  return [...files];
}

/** Reject configured packages using the build's module identities, not minified text. */
export function assertInitialModules(
  files: string[],
  modules: Record<string, string[]>,
  denyList: string[],
): void {
  for (const file of files) {
    const ids = modules[file];
    if (!ids) throw new Error(`Missing build module metadata: ${file}`);
    for (const name of denyList) {
      const marker = `/node_modules/${name}/`;
      if (ids.some((id) => id.replaceAll('\\', '/').includes(marker)))
        throw new Error(`${name} found in initial chunk ${file}`);
    }
  }
}
