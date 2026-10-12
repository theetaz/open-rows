// SPDX-License-Identifier: Apache-2.0
import {
  COMPLETION_DELAYS,
  FONT_FAMILIES,
  FONT_SIZES,
  ROW_LIMITS,
} from '../query/editorSettings';
import type { EditorSettings } from '../query/editorSettings';

/** Settings that are on or off. */
type Flag = {
  [K in keyof EditorSettings]: EditorSettings[K] extends boolean ? K : never;
}[keyof EditorSettings];

export type FlagSpec = { key: Flag; title: string; description: string };

/** Display and saving switches of Settings ▸ Editor. */
export const FLAGS: readonly FlagSpec[] = [
  {
    key: 'lineNumbers',
    title: 'Line numbers',
    description: 'Number the lines in the gutter.',
  },
  {
    key: 'folding',
    title: 'Code folding',
    description: 'Fold statements, comments and bracketed blocks.',
  },
  { key: 'wrap', title: 'Wrap lines', description: 'Soft-wrap long lines.' },
  {
    key: 'invisibles',
    title: 'Show invisibles',
    description: 'Mark spaces and tabs.',
  },
  {
    key: 'highlightQuery',
    title: 'Highlight current query',
    description: 'Tint the statement ⌘↵ runs.',
  },
  {
    key: 'autoSave',
    title: 'Auto Save',
    description:
      'Keep query text across launches and write file-bound tabs as you type.',
  },
];

/** Completion switches (FR-EDIT-13); highlighting stays on either way. */
export const COMPLETION_FLAGS: readonly FlagSpec[] = [
  {
    key: 'autocomplete',
    title: 'Autocomplete',
    description: 'Open suggestions while typing; ⌃Space always opens them.',
  },
  {
    key: 'upperKeywords',
    title: 'Uppercase keywords',
    description: 'Complete keywords in upper case, else lower case.',
  },
  {
    key: 'completeSchemas',
    title: 'Suggest schemas',
    description: 'Include schema names in suggestions.',
  },
  {
    key: 'completeTables',
    title: 'Suggest tables',
    description: 'Include tables and views in suggestions.',
  },
  {
    key: 'completeColumns',
    title: 'Suggest columns',
    description: 'Include columns in suggestions.',
  },
  {
    key: 'completeSnippets',
    title: 'Suggest snippets',
    description: 'Offer snippets when their prefix is typed.',
  },
  {
    key: 'qualifyNames',
    title: 'Schema-qualified names',
    description: 'Insert catalog objects as schema.name.',
  },
  {
    key: 'closeDelimiters',
    title: 'Close brackets and quotes',
    description: 'Type the closing bracket or quote with the opening one.',
  },
];

/** Which key accepts a suggestion (FR-EDIT-13). */
export const ACCEPT_CHOICE: Choice = {
  key: 'acceptKey',
  title: 'Accept suggestion with',
  description: 'Key that inserts the highlighted suggestion.',
  options: [
    { value: '"enter"', label: 'Return' },
    { value: '"tab"', label: 'Tab' },
    { value: '"both"', label: 'Return or Tab' },
  ],
};

export type Choice = {
  key: keyof EditorSettings;
  title: string;
  description: string;
  options: readonly { value: string; label: string }[];
};

/** Font, keys and row limit of Settings ▸ Editor. */
export const CHOICES: readonly Choice[] = [
  {
    key: 'fontSize',
    title: 'Font size',
    description: 'Text size of SQL editors.',
    options: [
      { value: 'null', label: 'Appearance default' },
      ...FONT_SIZES.map((size) => ({
        value: String(size),
        label: `${String(size)} px`,
      })),
    ],
  },
  {
    key: 'fontFamily',
    title: 'Font',
    description: 'Typeface of SQL editors.',
    options: [
      { value: 'null', label: 'Appearance default' },
      ...FONT_FAMILIES.map((family) => ({
        value: JSON.stringify(family),
        label: family,
      })),
    ],
  },
  {
    key: 'keyBinding',
    title: 'Key bindings',
    description: 'Editing keys of SQL editors.',
    options: [
      { value: '"default"', label: 'Default' },
      { value: '"vim"', label: 'Vim' },
    ],
  },
  {
    key: 'rowLimit',
    title: 'Row limit',
    description: 'Rows kept per result set when running.',
    options: [
      { value: 'null', label: 'No limit' },
      ...ROW_LIMITS.map((rows) => ({
        value: String(rows),
        label: rows.toLocaleString('en-US'),
      })),
    ],
  },
];

/** When suggestions open while typing. */
export const DELAY_CHOICE: Choice = {
  key: 'completionDelay',
  title: 'Suggestion delay',
  description: 'Pause after typing before suggestions open.',
  options: COMPLETION_DELAYS.map((delay) => ({
    value: String(delay),
    label: delay ? `${String(delay)} ms` : 'Immediately',
  })),
};

/** Indentation of the editor's indent keys and new lines, and of Beautify (FR-EDIT-12). */
export const INDENT_CHOICES: readonly Choice[] = [
  {
    key: 'indentWidth',
    title: 'Indent width',
    description:
      'Spaces per indent level and tab width, in the editor and when beautifying.',
    options: [
      { value: '2', label: '2 spaces' },
      { value: '4', label: '4 spaces' },
    ],
  },
  {
    key: 'indentWith',
    title: 'Indent with',
    description: 'What the editor and Beautify indent with.',
    options: [
      { value: '"spaces"', label: 'Spaces' },
      { value: '"tabs"', label: 'Tabs' },
    ],
  },
];

/** Beautify (⌘I) and Uglify (⇧⌘I) layout. */
export const FORMAT_CHOICES: readonly Choice[] = [
  {
    key: 'keywordCase',
    title: 'Keyword case',
    description: 'Case of SQL keywords when beautifying.',
    options: [
      { value: '"upper"', label: 'UPPER' },
      { value: '"lower"', label: 'lower' },
      { value: '"preserve"', label: 'As typed' },
    ],
  },
  {
    key: 'indentStyle',
    title: 'Indent type',
    description: 'Standard indents, or keywords aligned in a column.',
    options: [
      { value: '"standard"', label: 'Standard' },
      { value: '"tabularLeft"', label: 'Tabular left' },
      { value: '"tabularRight"', label: 'Tabular right' },
    ],
  },
  {
    key: 'linesBetweenQueries',
    title: 'Lines between queries',
    description: 'Empty lines between statements.',
    options: [
      { value: '1', label: '1' },
      { value: '2', label: '2' },
      { value: '3', label: '3' },
    ],
  },
];

export const DENSE: FlagSpec = {
  key: 'denseOperators',
  title: 'Dense operators',
  description: 'Write operators without surrounding spaces (a=1).',
};
