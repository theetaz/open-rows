// SPDX-License-Identifier: Apache-2.0
import { syntaxTree } from '@codemirror/language';
import type { EditorState } from '@codemirror/state';

/** A document range. */
export type TextRange = { from: number; to: number };

/** Top-level statement nodes of the parsed document, in order. */
function* statements(state: EditorState): Generator<TextRange> {
  for (
    let node = syntaxTree(state).topNode.firstChild;
    node;
    node = node.nextSibling
  )
    if (node.name === 'Statement') yield { from: node.from, to: node.to };
}

/**
 * The statement at `pos`, else the closest one before it, else the first:
 * the statement Run Current executes. `null` for a document without SQL.
 */
export function currentStatement(
  state: EditorState,
  pos: number,
): TextRange | null {
  let found: TextRange | null = null;
  for (const statement of statements(state)) {
    if (found && statement.from > pos) break;
    found = statement;
  }
  return found;
}

/**
 * The text that belongs to the statement being typed at `pos`: it starts
 * after the last statement terminated before `pos`, so a fresh statement
 * after `;` never sees the previous one.
 */
export function editingRange(state: EditorState, pos: number): TextRange {
  let from = 0;
  for (const statement of statements(state)) {
    const closed = state.sliceDoc(statement.to - 1, statement.to) === ';';
    if (closed && statement.to <= pos) {
      from = statement.to;
      continue;
    }
    return statement.from > pos ? { from, to: pos } : statement;
  }
  return { from, to: state.doc.length };
}
