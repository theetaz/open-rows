// SPDX-License-Identifier: Apache-2.0
import { autocompletion } from '@codemirror/autocomplete';
import { Compartment } from '@codemirror/state';
import type { Extension } from '@codemirror/state';
import {
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightWhitespace,
  lineNumbers,
} from '@codemirror/view';
import { completionKinds } from './completion/kinds';
import { qualifyNames, upperKeywords } from './completion/source';
import { currentQueryHighlight } from './currentQuery';
import { sqlFolding } from './folding';
import { closingDelimiters, completionKeys, indentation } from './typing';
import type { AcceptKey } from './typing';

export type { AcceptKey };

/** Editor behaviour the user switches at runtime (bottom-bar settings menu). */
export type EditorPreferences = {
  /** Font size in px; `null` follows the appearance's monospace size. */
  fontSize: number | null;
  /** Font family name; `null` follows the appearance's monospace font. */
  fontFamily: string | null;
  /** Show the line-number gutter. */
  lineNumbers: boolean;
  /** Fold statements, comments and bracketed blocks from a gutter. */
  folding: boolean;
  /** Draw spaces and tabs. */
  invisibles: boolean;
  /** Wrap long lines at the editor width. */
  wrap: boolean;
  /** Tint the current statement instead of the current line. */
  highlightQuery: boolean;
  /** Open completions while typing (Ctrl-Space always works). */
  autocomplete: boolean;
  /** Pause after typing before completions open, in ms. */
  completionDelay: number;
  /** Insert accepted keywords in upper case. */
  upperKeywords: boolean;
  /** Suggest schema names. */
  completeSchemas: boolean;
  /** Suggest tables and views. */
  completeTables: boolean;
  /** Suggest columns. */
  completeColumns: boolean;
  /** Offer snippets by prefix. */
  completeSnippets: boolean;
  /** Insert catalog objects as `schema.name`. */
  qualifyNames: boolean;
  /** Type the closing bracket or quote with the opening one. */
  closeDelimiters: boolean;
  /** Key that accepts the highlighted suggestion. */
  acceptKey: AcceptKey;
  /** Spaces per indent level, also the width of a tab. */
  indentWidth: 2 | 4;
  /** Indent with spaces or tab characters. */
  indentWith: 'spaces' | 'tabs';
};

/** Preferences of a new editor. */
export const defaultPreferences: EditorPreferences = {
  fontSize: null,
  fontFamily: null,
  lineNumbers: true,
  folding: true,
  invisibles: false,
  wrap: false,
  highlightQuery: false,
  autocomplete: true,
  completionDelay: 100,
  upperKeywords: true,
  completeSchemas: true,
  completeTables: true,
  completeColumns: true,
  completeSnippets: true,
  qualifyNames: false,
  closeDelimiters: true,
  acceptKey: 'enter',
  indentWidth: 2,
  indentWith: 'spaces',
};

/** Holds the preference-driven extensions of each editor state. */
export const preferenceSlot = new Compartment();
/** Holds the key mode (Vim); placed first so it precedes every other keymap. */
export const keyModeSlot = new Compartment();

/** Font size and family over the appearance defaults. */
function font(preferences: EditorPreferences): Extension {
  const { fontSize, fontFamily } = preferences;
  return [
    fontSize === null
      ? []
      : EditorView.theme({
          '&.cm-editor': { fontSize: `${String(fontSize)}px` },
        }),
    fontFamily === null
      ? []
      : EditorView.theme({
          '.cm-scroller': {
            fontFamily: `"${fontFamily.replaceAll('"', '')}", var(--or-font-mono)`,
          },
        }),
  ];
}

/** Gutters: line numbers and fold markers. */
function gutters(preferences: EditorPreferences): Extension {
  return [
    preferences.lineNumbers ? [lineNumbers(), highlightActiveLineGutter()] : [],
    preferences.folding ? sqlFolding() : [],
  ];
}

/** Completion trigger, delay, keys, keyword case, qualification and kinds. */
function completion(preferences: EditorPreferences): Extension {
  return [
    autocompletion({
      activateOnTyping: preferences.autocomplete,
      activateOnTypingDelay: preferences.completionDelay,
      defaultKeymap: false,
    }),
    completionKeys(preferences.acceptKey),
    upperKeywords.of(preferences.upperKeywords),
    qualifyNames.of(preferences.qualifyNames),
    completionKinds.of({
      schemas: preferences.completeSchemas,
      tables: preferences.completeTables,
      columns: preferences.completeColumns,
      snippets: preferences.completeSnippets,
    }),
  ];
}

/** Extensions implementing `preferences`, for `preferenceSlot`. */
export function preferenceExtensions(
  preferences: EditorPreferences,
): Extension {
  return [
    font(preferences),
    gutters(preferences),
    preferences.invisibles ? highlightWhitespace() : [],
    preferences.wrap ? EditorView.lineWrapping : [],
    preferences.highlightQuery
      ? currentQueryHighlight()
      : highlightActiveLine(),
    completion(preferences),
    closingDelimiters(preferences.closeDelimiters),
    indentation(preferences.indentWidth, preferences.indentWith === 'tabs'),
  ];
}
