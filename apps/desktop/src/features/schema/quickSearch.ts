// SPDX-License-Identifier: Apache-2.0
import type { PaletteItem } from '@openrows/ui';
/** Cached names used by quick-open for objects and both query stores. */
type SearchEntry = { item: PaletteItem; name: string; qualified: string };
/** Fold names once when the cached catalog changes. */
export function quickIndex(items: readonly PaletteItem[]): SearchEntry[] {
  return items.map((item) => ({
    item,
    name: item.label.toLowerCase(),
    qualified: (item.searchText ?? item.label).toLowerCase(),
  }));
}
function subsequence(name: string, needle: string): boolean {
  let position = 0;
  for (const character of needle) {
    const found = name.indexOf(character, position);
    if (found < 0) return false;
    position = found + character.length;
  }
  return true;
}
/** Match the existing Rust NameIndex ranking (docs/10 §1.1). */
function rank(entry: SearchEntry, needle: string): number | null {
  if (!needle || entry.name === needle) return 0;
  if (entry.name.startsWith(needle)) return 1;
  if (entry.qualified.startsWith(needle)) return 2;
  if (entry.name.includes(needle)) return 3;
  if (entry.qualified.includes(needle)) return 4;
  return subsequence(entry.name, needle) ? 5 : null;
}
/** Ranked bounded matches without file or network reads per keystroke. */
export function searchQuick(
  index: readonly SearchEntry[],
  text: string,
  limit = 100,
): PaletteItem[] {
  const needle = text.trim().toLowerCase();
  const buckets: PaletteItem[][] = Array.from({ length: 6 }, () => []);
  for (const entry of index) {
    const score = rank(entry, needle);
    if (score !== null && (buckets[score]?.length ?? 0) < limit)
      buckets[score]?.push(entry.item);
  }
  return buckets.flat().slice(0, limit);
}
