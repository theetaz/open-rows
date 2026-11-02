// SPDX-License-Identifier: Apache-2.0
import { StateEffect } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { currentStatement, editingRange } from './statement';
import { Review, inlineTheme, reviewField } from './inlineReview';
/** App-owned streaming transport. Cancellation must interrupt native work too. */
export type InlineGenerate = (input: {
  prompt: string;
  sql: string;
  signal: AbortSignal;
  delta: (text: string) => void;
}) => Promise<string>;
/** Why proposals cannot be generated yet, with an optional way to fix it. */
export type InlineBlock = {
  reason: string;
  action?: { label: string; run: () => void };
};
/** Generation plus a synchronous readiness check, run when the prompt opens. */
export type InlineTransport = {
  generate: InlineGenerate;
  blocked?: () => InlineBlock | null;
};
/**
 * A proposal targets exactly the selection, else the statement at the caret
 * (also when the caret sits after its `;` on the same line), else an empty
 * insert point such as a blank line.
 */
export function inlineRange(view: EditorView): { from: number; to: number } {
  const { state } = view;
  const { from, to, head } = state.selection.main;
  if (from !== to) return { from, to };
  const range = editingRange(state, head);
  if (state.sliceDoc(range.from, range.to).trim()) return range;
  const previous = currentStatement(state, head);
  const sameLine =
    previous &&
    previous.to <= head &&
    state.doc.lineAt(previous.to).number === state.doc.lineAt(head).number &&
    !state.sliceDoc(previous.to, head).trim();
  return sameLine ? previous : { from: head, to: head };
}
function isInlineKey(event: KeyboardEvent): boolean {
  return (
    (event.metaKey || event.ctrlKey) &&
    !event.shiftKey &&
    !event.altKey &&
    event.key.toLowerCase() === 'i'
  );
}
/** Install the lazy transport shortcut; document changes invalidate pending reviews. */
export function installInline(
  view: EditorView,
  transport: InlineTransport,
  appKeys?: (event: KeyboardEvent) => boolean,
): { open: () => void; destroy: () => void } {
  let active: Review | null = null;
  view.dispatch({
    effects: StateEffect.appendConfig.of([
      reviewField,
      inlineTheme,
      EditorView.updateListener.of((update) => {
        if (!update.docChanged || !active) return;
        active.abort.abort();
        active = null;
      }),
    ]),
  });
  const open = (): void => {
    active?.cancel();
    const review = new Review(view, transport, inlineRange(view));
    active = review;
    review.abort.signal.addEventListener(
      'abort',
      () => {
        if (active === review) active = null;
      },
      { once: true },
    );
    review.show();
  };
  const key = (event: KeyboardEvent): void => {
    if (event.key === 'Escape' && active) {
      event.preventDefault();
      event.stopPropagation();
      active.cancel();
      active = null;
    } else if (isInlineKey(event)) {
      if (appKeys) {
        if (appKeys(event)) event.preventDefault();
        return;
      }
      event.preventDefault();
      open();
    }
  };
  view.dom.addEventListener('keydown', key, true);
  return {
    open,
    destroy: () => {
      active?.abort.abort();
      view.dom.removeEventListener('keydown', key, true);
    },
  };
}
