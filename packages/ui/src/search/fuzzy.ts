// SPDX-License-Identifier: Apache-2.0
/** Ordered character match with contiguous and word-boundary ranking. */
export function fuzzyMatch(
  text: string,
  query: string,
):
  | {
      indices: number[];
      score: number;
    }
  | undefined {
  const source = text.toLowerCase();
  const needle = query.trim().toLowerCase();
  const indices: number[] = [];
  let cursor = 0;
  let score = 0;
  for (const character of needle) {
    const index = source.indexOf(character, cursor);
    if (index < 0) return undefined;
    score += index === cursor ? 8 : 0;
    score += index === 0 || /[_.\s]/u.test(source[index - 1] ?? '') ? 4 : 0;
    score -= index - cursor;
    indices.push(index);
    cursor = index + character.length;
  }
  return { indices, score };
}
