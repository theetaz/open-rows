// SPDX-License-Identifier: Apache-2.0
import {
  codeFolding,
  foldGutter,
  foldKeymap,
  foldService,
  syntaxTree,
} from '@codemirror/language';
import type { EditorState, Extension } from '@codemirror/state';
import { keymap } from '@codemirror/view';

/** Bracket nodes of the SQL grammar that fold when they span lines. */
const BLOCKS = new Set(['Parens', 'Braces', 'Brackets']);

/**
 * The fold range of a bracketed block that opens on the line from
 * `lineStart` to `lineEnd` and closes on a later line: its inside.
 */
export function blockFold(
  state: EditorState,
  lineStart: number,
  lineEnd: number,
): { from: number; to: number } | null {
  let found: { from: number; to: number } | null = null;
  syntaxTree(state).iterate({
    from: lineStart,
    to: lineEnd,
    enter: (node) => {
      if (found) return false;
      if (!BLOCKS.has(node.name) || node.from < lineStart) return undefined;
      if (node.from >= lineEnd || node.to - node.from < 2) return undefined;
      if (state.doc.lineAt(node.to).from <= lineEnd) return undefined;
      found = { from: node.from + 1, to: node.to - 1 };
      return false;
    },
  });
  return found;
}

/**
 * Code folding: statements and block comments (from the SQL grammar) and
 * multi-line bracketed blocks, with a gutter and the standard fold keys
 * (⌥⌘[ fold, ⌥⌘] unfold, ⌃⌥[ fold all, ⌃⌥] unfold all).
 */
export function sqlFolding(): Extension {
  return [
    codeFolding(),
    foldService.of(blockFold),
    foldGutter(),
    keymap.of(foldKeymap),
  ];
}
