// SPDX-License-Identifier: Apache-2.0
import { forceParsing } from '@codemirror/language';
import { EditorState } from '@codemirror/state';
import type { Extension } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { sqlEditorExtensions } from './extensions';
import type { EditorDialect, SqlEditorOptions } from './extensions';
import { formatInView } from './format';
import type { FormatKind, FormatOptions } from './format';
import { insertSnippet } from './snippets';
import { currentStatement } from './statement';
import {
  keyModeSlot,
  preferenceExtensions,
  preferenceSlot,
} from './preferences';
import type { AcceptKey, EditorPreferences } from './preferences';

export type { EditorDialect, SqlEditorOptions };
export type { AcceptKey, EditorPreferences, FormatKind, FormatOptions };
export type {
  CatalogColumn,
  CatalogObject,
  CatalogSchema,
  CompletionCatalog,
} from './completion/catalog';
export type { SqlSnippet, SnippetSource } from './snippets';
export type { EditorView } from '@codemirror/view';
export { defaultPreferences } from './preferences';

/** A mounted SQL editor; callers own its lifetime. */
export interface SqlEditor {
  /** The underlying CodeMirror view, for commands and instrumentation. */
  readonly view: EditorView;
  /** Apply new runtime preferences without losing text, history or cursor. */
  readonly setPreferences: (preferences: EditorPreferences) => void;
  /** Install a key mode (Vim) or go back to the default keys with `null`. */
  readonly setKeyMode: (keyMode: Extension | null) => void;
  /**
   * Beautify or uglify the selection, else the current statement, else the
   * whole document, as one undoable change.
   */
  readonly format: (kind: FormatKind, options: FormatOptions) => Promise<void>;
  /** Insert a snippet template over the selection, its first field selected. */
  readonly insertSnippet: (body: string) => void;
  /** Insert literal SQL over the selection without interpreting snippet placeholders. */
  readonly insertText: (text: string) => void;
  /**
   * Where the text Run Current executes starts: the selection, else the
   * statement at the cursor, else the document start.
   */
  readonly runStart: () => number;
  /** Detach DOM listeners and remove the editor from its parent. */
  destroy(): void;
}

function runStart(view: EditorView): number {
  const { from, to, head } = view.state.selection.main;
  if (from !== to) return from;
  return currentStatement(view.state, head)?.from ?? 0;
}

/** Mount a SQL editor into `parent`. Load this module lazily. */
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
    setPreferences: (preferences) => {
      view.dispatch({
        effects: preferenceSlot.reconfigure(preferenceExtensions(preferences)),
      });
    },
    setKeyMode: (keyMode) => {
      view.dispatch({ effects: keyModeSlot.reconfigure(keyMode ?? []) });
    },
    format: (kind, formatOptions) => formatInView(view, kind, formatOptions),
    insertSnippet: (body) => {
      insertSnippet(view, body);
    },
    insertText: (text) => {
      view.dispatch(view.state.replaceSelection(text));
      view.focus();
    },
    runStart: () => runStart(view),
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
