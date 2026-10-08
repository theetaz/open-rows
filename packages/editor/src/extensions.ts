// SPDX-License-Identifier: Apache-2.0
import { autocompletion, completionKeymap } from '@codemirror/autocomplete';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { bracketMatching } from '@codemirror/language';
import { sql } from '@codemirror/lang-sql';
import { EditorState } from '@codemirror/state';
import type { Extension } from '@codemirror/state';
import {
  EditorView,
  drawSelection,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from '@codemirror/view';
import type { CompletionCatalog } from './completion/catalog';
import { completionIcon } from './completion/icons';
import { sqlCompletionSource } from './completion/source';
import { completionTheme } from './completion/theme';
import { sqlDialect } from './dialects';
import type { EditorDialect } from './dialects';
import {
  defaultPreferences,
  keyModeSlot,
  preferenceExtensions,
  preferenceSlot,
} from './preferences';
import type { EditorPreferences } from './preferences';
import { editorAppearance } from './theme';

export type { EditorDialect };

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
  /** Called after the selection or document changed (cursor position). */
  onSelection?: (view: EditorView) => void;
  /**
   * Application shortcuts (CodeMirror key names such as `Mod-Enter`) that win
   * over the default keymaps; `run` returns true when it handled the key.
   */
  keys?: readonly EditorKey[];
  /** A read-only viewer (DDL, trigger definitions): no edits, still selectable. */
  readOnly?: boolean;
  /** Highlighting and keyword completion; `postgres` when absent. */
  dialect?: EditorDialect;
  /** Schema objects and columns for completion; keywords only when absent. */
  catalog?: CompletionCatalog;
  /** Runtime preferences; `defaultPreferences` when absent. */
  preferences?: EditorPreferences;
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

function listeners(options: SqlEditorOptions): Extension {
  const { onDocChange, onSelection } = options;
  if (!onDocChange && !onSelection) return [];
  return EditorView.updateListener.of((update) => {
    if (update.docChanged) onDocChange?.();
    if (update.docChanged || update.selectionSet) onSelection?.(update.view);
  });
}

/** Context-aware completion with kind icons, styled by `completionTheme`. */
function completion(options: SqlEditorOptions): Extension {
  const source = sqlCompletionSource(
    options.dialect ?? 'postgres',
    options.catalog ?? null,
  );
  return [
    autocompletion({
      override: [source],
      icons: false,
      addToOptions: [completionIcon],
    }),
    completionTheme,
  ];
}

/**
 * Dialect highlighting and completion (PostgreSQL, MySQL, MariaDB or SQLite)
 * with the standard editing extensions.
 */
export function sqlEditorExtensions(options: SqlEditorOptions): Extension {
  return [
    keyModeSlot.of(options.keyMode ?? []),
    appKeys(options.keys),
    lineNumbers(),
    highlightActiveLineGutter(),
    history(),
    drawSelection(),
    bracketMatching(),
    completion(options),
    preferenceSlot.of(
      preferenceExtensions(options.preferences ?? defaultPreferences),
    ),
    sql({ dialect: sqlDialect(options.dialect ?? 'postgres') }),
    editorAppearance(),
    EditorView.contentAttributes.of({ 'aria-label': options.label }),
    keymap.of([...completionKeymap, ...defaultKeymap, ...historyKeymap]),
    listeners(options),
    options.readOnly
      ? [EditorState.readOnly.of(true), EditorView.editable.of(false)]
      : [],
  ];
}
