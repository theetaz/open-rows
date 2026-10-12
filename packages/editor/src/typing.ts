// SPDX-License-Identifier: Apache-2.0
import {
  acceptCompletion,
  closeBrackets,
  closeBracketsKeymap,
  completionKeymap,
} from '@codemirror/autocomplete';
import { indentUnit } from '@codemirror/language';
import { EditorState, Prec } from '@codemirror/state';
import type { Extension } from '@codemirror/state';
import { keymap } from '@codemirror/view';
import type { KeyBinding } from '@codemirror/view';

/** Which key accepts the highlighted suggestion. */
export type AcceptKey = 'enter' | 'tab' | 'both';

/** Tab display width and the unit that indent commands and new lines use. */
export function indentation(width: number, withTabs: boolean): Extension {
  return [
    EditorState.tabSize.of(width),
    indentUnit.of(withTabs ? '\t' : ' '.repeat(width)),
  ];
}

/**
 * The completion keys with Enter, Tab or both accepting; Tab only acts while
 * suggestions are open, so it still leaves the editor otherwise.
 */
export function completionKeys(accept: AcceptKey): Extension {
  const keys: KeyBinding[] = completionKeymap.filter(
    (binding) => binding.key !== 'Enter' || accept !== 'tab',
  );
  if (accept !== 'enter') keys.push({ key: 'Tab', run: acceptCompletion });
  return Prec.highest(keymap.of(keys));
}

/** Typing an opening bracket or quote also types its closing pair. */
export function closingDelimiters(enabled: boolean): Extension {
  return enabled ? [closeBrackets(), keymap.of(closeBracketsKeymap)] : [];
}
