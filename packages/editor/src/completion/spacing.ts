// SPDX-License-Identifier: Apache-2.0
import { pickedCompletion } from '@codemirror/autocomplete';
import { EditorState } from '@codemirror/state';
import type { Extension } from '@codemirror/state';

/** Kinds followed by more of the same name (`schema.`) or a call (`fn(`). */
const JOINED = new Set(['schema', 'function']);

/**
 * Accepting a completion types the space that separates it from the next
 * word, unless the next character already separates it.
 */
export const spaceAfterCompletion: Extension = EditorState.transactionFilter.of(
  (tr) => {
    const picked = tr.annotation(pickedCompletion);
    if (!picked || JOINED.has(picked.type ?? '')) return tr;
    const head = tr.newSelection.main.head;
    const next = tr.newDoc.sliceString(head, head + 1);
    if (next && /[\s,;)]/.test(next)) return tr;
    return [
      tr,
      {
        changes: { from: head, insert: ' ' },
        selection: { anchor: head + 1 },
        sequential: true,
      },
    ];
  },
);
