// SPDX-License-Identifier: Apache-2.0
const segmenter = new Intl.Segmenter('en', { granularity: 'grapheme' });
function graphemes(value: string): string[] {
  return Array.from(segmenter.segment(value), (part) => part.segment);
}
/** First graphemes of name parts, or two from one word; single letters repeat. */
export function workspaceMonogram(name: string): string {
  const parts =
    name.normalize('NFC').match(/[\p{L}\p{N}][\p{L}\p{N}\p{M}]*/gu) ?? [];
  const first = graphemes(parts[0] ?? '');
  const letters =
    parts.length > 1
      ? [first[0], graphemes(parts[1] ?? '')[0]]
      : [first[0], first[1] ?? first[0]];
  return graphemes(letters.join('').toUpperCase()).slice(0, 2).join('') || 'WS';
}
