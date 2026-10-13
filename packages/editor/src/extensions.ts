// SPDX-License-Identifier: Apache-2.0
import { autocompletion } from '@codemirror/autocomplete';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { bracketMatching } from '@codemirror/language';
import { sql } from '@codemirror/lang-sql';
import { EditorState, Prec } from '@codemirror/state';
import type { Extension } from '@codemirror/state';
import { EditorView, drawSelection, keymap } from '@codemirror/view';
import type { CompletionCatalog } from './completion/catalog';
import { completionIcon } from './completion/icons';
import { sqlCompletionSource } from './completion/source';
import { spaceAfterCompletion } from './completion/spacing';
import { completionTheme } from './completion/theme';
import { multipleCarets } from './multiCursor';
import { sqlDialect } from './dialects';
import type { EditorDialect } from './dialects';
import {
  defaultPreferences,
  keyModeSlot,
  preferenceExtensions,
  preferenceSlot,
} from './preferences';
import type { EditorPreferences } from './preferences';
import { snippetSource } from './snippets';
import type { SnippetSource } from './snippets';
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
   * Application shortcuts, checked before every keymap (so they win over
   * defaults such as `Mod-Enter`); returns true when it handled the key,
   * which is then prevented. Read on every press, so rebinding applies live.
   */
  onKeyDown?: (event: KeyboardEvent) => boolean;
  /** A read-only viewer (DDL, trigger definitions): no edits, still selectable. */
  readOnly?: boolean;
  /** Highlighting and keyword completion; `postgres` when absent. */
  dialect?: EditorDialect;
  /** Schema objects and columns for completion; keywords only when absent. */
  catalog?: CompletionCatalog;
  /** Snippets offered when their prefix is typed; none when absent. */
  snippets?: SnippetSource;
  /** Runtime preferences; `defaultPreferences` when absent. */
  preferences?: EditorPreferences;
}

function appKeys(handle: SqlEditorOptions['onKeyDown']): Extension {
  if (!handle) return [];
  return Prec.highest(
    EditorView.domEventHandlers({
      keydown: (event) => {
        if (!handle(event)) return false;
        event.preventDefault();
        return true;
      },
    }),
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

/** Context-aware completion and snippets with kind icons, styled by `completionTheme`. */
function completion(options: SqlEditorOptions): Extension {
  const source = sqlCompletionSource(
    options.dialect ?? 'postgres',
    options.catalog ?? null,
  );
  const snippets = options.snippets ? [snippetSource(options.snippets)] : [];
  return [
    autocompletion({
      override: [source, ...snippets],
      icons: false,
      addToOptions: [completionIcon],
    }),
    spaceAfterCompletion,
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
    appKeys(options.onKeyDown),
    history(),
    drawSelection(),
    multipleCarets(),
    bracketMatching(),
    completion(options),
    preferenceSlot.of(
      preferenceExtensions(options.preferences ?? defaultPreferences),
    ),
    sql({ dialect: sqlDialect(options.dialect ?? 'postgres') }),
    editorAppearance(),
    EditorView.contentAttributes.of({ 'aria-label': options.label }),
    keymap.of([...defaultKeymap, ...historyKeymap]),
    listeners(options),
    options.readOnly
      ? [EditorState.readOnly.of(true), EditorView.editable.of(false)]
      : [],
  ];
}
