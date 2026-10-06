// SPDX-License-Identifier: Apache-2.0
import { fuzzyMatch } from './fuzzy';
import type { PaletteItem } from '../components/Palette';
/** Rank a palette catalog and preserve literal-label highlight positions. */
export function rankPaletteItems(
  items: readonly PaletteItem[],
  query: string,
): {
  item: PaletteItem;
  indices: number[];
  score: number;
}[] {
  return items
    .flatMap((item) => {
      const match = fuzzyMatch(item.searchText ?? item.label, query);
      return match
        ? [
            {
              item,
              ...match,
              indices: fuzzyMatch(item.label, query)?.indices ?? [],
            },
          ]
        : [];
    })
    .sort((left, right) => right.score - left.score);
}
