// SPDX-License-Identifier: Apache-2.0
import {
  selectNextOccurrence,
  selectSelectionMatches,
} from '@codemirror/search';
import { EditorState } from '@codemirror/state';
import type { Extension } from '@codemirror/state';
import {
  EditorView,
  crosshairCursor,
  keymap,
  rectangularSelection,
} from '@codemirror/view';

/** ⌥-click (or ⌘-click) adds a caret instead of moving the only one. */
export function addsCaret(event: MouseEvent): boolean {
  return event.altKey || event.metaKey;
}

/**
 * Multiple carets (FR-EDIT-12): ⌥-click adds one, ⌥-drag selects a column
 * (crosshair cursor while ⌥ is down), ⌘D selects the next occurrence of
 * the selection or word, ⇧⌘L every occurrence; Escape goes back to one caret.
 */
export function multipleCarets(): Extension {
  return [
    EditorState.allowMultipleSelections.of(true),
    EditorView.clickAddsSelectionRange.of(addsCaret),
    rectangularSelection(),
    crosshairCursor(),
    keymap.of([
      { key: 'Mod-d', run: selectNextOccurrence, preventDefault: true },
      { key: 'Mod-Shift-l', run: selectSelectionMatches, preventDefault: true },
    ]),
  ];
}
