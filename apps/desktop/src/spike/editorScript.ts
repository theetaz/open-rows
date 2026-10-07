// SPDX-License-Identifier: Apache-2.0
/** `sql`: PostgreSQL highlighting + completion; `vim`: the same plus Vim mode. */
export const CONFIGS = ['sql', 'vim'] as const;
export type EditorConfig = (typeof CONFIGS)[number];

/** Edit sites: end of these 1-based lines in the 5 000-line document. */
export const POSITIONS = [
  { name: 'top', line: 2 },
  { name: 'middle', line: 2_500 },
  { name: 'end', line: 5_000 },
] as const;

/** Sixty characters typed one at a time at every position (identifier, string, operators). */
export const TYPED_TEXT =
  " AND lower(c.email) ILIKE '%@example.org' OR o.total >= 4200";
/** Measured single-character deletions after typing at every position. */
const DELETES = 20;
/** Gap from one keystroke's paint to the next (fast typing: ≈ 15 keys/s at 60 Hz). */
export const KEY_INTERVAL_MS = 50;

/** One scripted keystroke; `mode` keys switch Vim modes and are not sampled. */
export type Keystroke =
  | { kind: 'insert'; key: string }
  | { kind: 'delete'; key: 'Backspace' | 'x' }
  | { kind: 'mode'; key: 'A' | 'Escape'; mode: 'insert' | 'normal' };

/** Deterministic keystrokes for one edit site: append text, then delete. */
export function keystrokes(config: EditorConfig): Keystroke[] {
  // ASCII only, so UTF-16 units are characters.
  const typed = TYPED_TEXT.split('').map((key): Keystroke => ({
    kind: 'insert',
    key,
  }));
  const deleteKey = config === 'vim' ? 'x' : 'Backspace';
  const deletes = Array.from({ length: DELETES }, (): Keystroke => ({
    kind: 'delete',
    key: deleteKey,
  }));
  if (config === 'sql') return [...typed, ...deletes];
  return [
    { kind: 'mode', key: 'A', mode: 'insert' },
    ...typed,
    { kind: 'mode', key: 'Escape', mode: 'normal' },
    ...deletes,
  ];
}
