// SPDX-License-Identifier: Apache-2.0
import { getCM, vim } from '@replit/codemirror-vim';
import type { Extension } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';

/** Vim keybindings (FR-EDIT-09); pass as `keyMode` so it precedes other keymaps. */
export function vimMode(): Extension {
  return vim();
}

/** Current Vim mode, or null when the view has no Vim extension. */
export function vimModeOf(view: EditorView): 'insert' | 'normal' | null {
  const state = getCM(view)?.state.vim;
  if (!state) return null;
  return state.insertMode ? 'insert' : 'normal';
}
