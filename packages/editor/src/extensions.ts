// SPDX-License-Identifier: Apache-2.0
import { autocompletion, completionKeymap } from '@codemirror/autocomplete';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { bracketMatching } from '@codemirror/language';
import { PostgreSQL, sql } from '@codemirror/lang-sql';
import { EditorState } from '@codemirror/state';
import type { Extension } from '@codemirror/state';
import {
  EditorView,
  drawSelection,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from '@codemirror/view';
import { editorAppearance } from './theme';

/** Options shared by every SQL editor surface. */
export interface SqlEditorOptions {
  /** Initial document text. */
  doc: string;
  /** Accessible name of the editable region. */
  label: string;
  /**
   * Key handling that must win over the default keymaps (for example Vim mode
   * from `@openrows/editor/vim`), installed ahead of every other extension.
   */
  keyMode?: Extension;
  /** Called synchronously after every transaction that changes the document. */
  onDocChange?: () => void;
  /**
   * Application shortcuts (CodeMirror key names such as `Mod-Enter`) that win
   * over the default keymaps; `run` returns true when it handled the key.
   */
  keys?: readonly EditorKey[];
  /** A read-only viewer (DDL, trigger definitions): no edits, still selectable. */
  readOnly?: boolean;
}

/** One application shortcut inside the editor. */
export interface EditorKey {
  key: string;
  run: () => boolean;
}

function appKeys(keys: readonly EditorKey[] | undefined): Extension {
  if (!keys?.length) return [];
  return keymap.of(
    keys.map(({ key, run }) => ({ key, run, preventDefault: true })),
  );
}

function docChangeListener(listener: (() => void) | undefined): Extension {
  if (!listener) return [];
  return EditorView.updateListener.of((update) => {
    if (update.docChanged) listener();
  });
}

/**
 * PostgreSQL dialect highlighting and keyword completion with the standard
 * editing extensions; other dialects arrive with the production editor (M1).
 */
export function sqlEditorExtensions(options: SqlEditorOptions): Extension {
  return [
    options.keyMode ?? [],
    appKeys(options.keys),
    lineNumbers(),
    highlightActiveLineGutter(),
    history(),
    drawSelection(),
    highlightActiveLine(),
    bracketMatching(),
    autocompletion(),
    sql({ dialect: PostgreSQL }),
    editorAppearance(),
    EditorView.contentAttributes.of({ 'aria-label': options.label }),
    keymap.of([...completionKeymap, ...defaultKeymap, ...historyKeymap]),
    docChangeListener(options.onDocChange),
    options.readOnly
      ? [EditorState.readOnly.of(true), EditorView.editable.of(false)]
      : [],
  ];
}
