// SPDX-License-Identifier: Apache-2.0
import { forceParsing } from '@codemirror/language';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { sqlEditorExtensions } from './extensions';
import type { SqlEditorOptions } from './extensions';

export type { SqlEditorOptions };
export type { EditorView } from '@codemirror/view';

/** A mounted SQL editor; callers own its lifetime. */
export interface SqlEditor {
  /** The underlying CodeMirror view, for commands and instrumentation. */
  readonly view: EditorView;
  /** Detach DOM listeners and remove the editor from its parent. */
  destroy(): void;
}

/** Mount a PostgreSQL editor into `parent`. Load this module lazily. */
export function mountSqlEditor(
  parent: HTMLElement,
  options: SqlEditorOptions,
): SqlEditor {
  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: options.doc,
      extensions: sqlEditorExtensions(options),
    }),
  });
  return {
    view,
    destroy: () => {
      view.destroy();
    },
  };
}

/**
 * Parse the whole document within `budgetMs`, so later measurements do not
 * include the initial background parse. Returns false if the budget ran out.
 */
export function parseFully(view: EditorView, budgetMs: number): boolean {
  return forceParsing(view, view.state.doc.length, budgetMs);
}
