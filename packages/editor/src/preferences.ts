// SPDX-License-Identifier: Apache-2.0
import { autocompletion } from '@codemirror/autocomplete';
import { Compartment } from '@codemirror/state';
import type { Extension } from '@codemirror/state';
import {
  EditorView,
  highlightActiveLine,
  highlightWhitespace,
} from '@codemirror/view';
import { upperKeywords } from './completion/source';
import { currentQueryHighlight } from './currentQuery';

/** Editor behaviour the user switches at runtime (bottom-bar settings menu). */
export type EditorPreferences = {
  /** Font size in px; `null` follows the appearance's monospace size. */
  fontSize: number | null;
  /** Draw spaces and tabs. */
  invisibles: boolean;
  /** Wrap long lines at the editor width. */
  wrap: boolean;
  /** Tint the current statement instead of the current line. */
  highlightQuery: boolean;
  /** Open completions while typing (Ctrl-Space always works). */
  autocomplete: boolean;
  /** Insert accepted keywords in upper case. */
  upperKeywords: boolean;
};

/** Preferences of a new editor. */
export const defaultPreferences: EditorPreferences = {
  fontSize: null,
  invisibles: false,
  wrap: false,
  highlightQuery: false,
  autocomplete: true,
  upperKeywords: true,
};

/** Holds the preference-driven extensions of each editor state. */
export const preferenceSlot = new Compartment();
/** Holds the key mode (Vim); placed first so it precedes every other keymap. */
export const keyModeSlot = new Compartment();

/** Extensions implementing `preferences`, for `preferenceSlot`. */
export function preferenceExtensions(
  preferences: EditorPreferences,
): Extension {
  const { fontSize } = preferences;
  return [
    fontSize === null
      ? []
      : EditorView.theme({
          '&.cm-editor': { fontSize: `${String(fontSize)}px` },
        }),
    preferences.invisibles ? highlightWhitespace() : [],
    preferences.wrap ? EditorView.lineWrapping : [],
    preferences.highlightQuery
      ? currentQueryHighlight()
      : highlightActiveLine(),
    autocompletion({ activateOnTyping: preferences.autocomplete }),
    upperKeywords.of(preferences.upperKeywords),
  ];
}
