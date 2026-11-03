// SPDX-License-Identifier: Apache-2.0
import {
  COMPLETION_DELAYS,
  FONT_FAMILIES,
  FONT_SIZES,
  ROW_LIMITS,
} from '../query/editorSettings';
import type { EditorSettings } from '../query/editorSettings';
import { t } from './messages/lazyGeneral';

/** Settings that are on or off. */
type Flag = {
  [K in keyof EditorSettings]: EditorSettings[K] extends boolean ? K : never;
}[keyof EditorSettings];

export type FlagSpec = { key: Flag; title: string; description: string };

/** Display and saving switches of Settings ▸ Editor. */
export const FLAGS: readonly FlagSpec[] = [
  {
    key: 'lineNumbers',
    title: t('lineNumbers'),
    description: t('numberLinesGutter'),
  },
  {
    key: 'folding',
    title: t('codeFolding'),
    description: t('foldStatementsComments'),
  },
  { key: 'wrap', title: t('wrapLines'), description: t('softWrapLong') },
  {
    key: 'invisibles',
    title: t('showInvisibles'),
    description: t('markSpacesTabs'),
  },
  {
    key: 'highlightQuery',
    title: t('highlightCurrentQuery'),
    description: t('tintStatementRuns'),
  },
  {
    key: 'autoSave',
    title: t('autoSave'),
    description: t('keepQueryText'),
  },
];

/** Completion switches (FR-EDIT-13); highlighting stays on either way. */
export const COMPLETION_FLAGS: readonly FlagSpec[] = [
  {
    key: 'autocomplete',
    title: t('autocomplete'),
    description: t('openSuggestionsWhile'),
  },
  {
    key: 'upperKeywords',
    title: t('uppercaseKeywords'),
    description: t('completeKeywordsUpper'),
  },
  {
    key: 'completeSchemas',
    title: t('suggestSchemas'),
    description: t('includeSchemaNames'),
  },
  {
    key: 'completeTables',
    title: t('suggestTables'),
    description: t('includeTablesViews'),
  },
  {
    key: 'completeColumns',
    title: t('suggestColumns'),
    description: t('includeColumnsSuggestions'),
  },
  {
    key: 'completeSnippets',
    title: t('suggestSnippets'),
    description: t('offerSnippetsWhen'),
  },
  {
    key: 'qualifyNames',
    title: t('schemaQualifiedNames'),
    description: t('insertCatalogObjects'),
  },
  {
    key: 'closeDelimiters',
    title: t('closeBracketsQuotes'),
    description: t('typeClosingBracket'),
  },
];

/** Which key accepts a suggestion (FR-EDIT-13). */
export const ACCEPT_CHOICE: Choice = {
  key: 'acceptKey',
  title: t('acceptSuggestion'),
  description: t('keyInsertsHighlighted'),
  options: [
    { value: '"enter"', label: t('return') },
    { value: '"tab"', label: t('tab') },
    { value: '"both"', label: t('returnTab') },
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
    title: t('fontSize'),
    description: t('textSizeSql'),
    options: [
      { value: 'null', label: t('appearanceDefault') },
      ...FONT_SIZES.map((size) => ({
        value: String(size),
        label: t('sizePx', { size: String(size) }),
      })),
    ],
  },
  {
    key: 'fontFamily',
    title: t('font'),
    description: t('typefaceSqlEditors'),
    options: [
      { value: 'null', label: t('appearanceDefault') },
      ...FONT_FAMILIES.map((family) => ({
        value: JSON.stringify(family),
        label: family,
      })),
    ],
  },
  {
    key: 'keyBinding',
    title: t('keyBindings'),
    description: t('editingKeysSql'),
    options: [
      { value: '"default"', label: t('default') },
      { value: '"vim"', label: t('vim') },
    ],
  },
  {
    key: 'rowLimit',
    title: t('rowLimit'),
    description: t('rowsKeptPer'),
    options: [
      { value: 'null', label: t('noLimit') },
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
  title: t('suggestionDelay'),
  description: t('pauseAfterTyping'),
  options: COMPLETION_DELAYS.map((delay) => ({
    value: String(delay),
    label: delay ? t('delayMs', { delay: String(delay) }) : t('immediately'),
  })),
};

/** Indentation of the editor's indent keys and new lines, and of Beautify (FR-EDIT-12). */
export const INDENT_CHOICES: readonly Choice[] = [
  {
    key: 'indentWidth',
    title: t('indentWidth'),
    description: t('spacesPerIndent'),
    options: [
      { value: '2', label: t('n2Spaces') },
      { value: '4', label: t('n4Spaces') },
    ],
  },
  {
    key: 'indentWith',
    title: t('indent'),
    description: t('whatEditorBeautify'),
    options: [
      { value: '"spaces"', label: t('spaces') },
      { value: '"tabs"', label: t('tabs') },
    ],
  },
];

/** Beautify (⌘I) and Uglify (⇧⌘I) layout. */
export const FORMAT_CHOICES: readonly Choice[] = [
  {
    key: 'keywordCase',
    title: t('keywordCase'),
    description: t('caseSqlKeywords'),
    options: [
      { value: '"upper"', label: t('upper') },
      { value: '"lower"', label: t('lower') },
      { value: '"preserve"', label: t('typed') },
    ],
  },
  {
    key: 'indentStyle',
    title: t('indentType'),
    description: t('standardIndentsKeywords'),
    options: [
      { value: '"standard"', label: t('standard') },
      { value: '"tabularLeft"', label: t('tabularLeft') },
      { value: '"tabularRight"', label: t('tabularRight') },
    ],
  },
  {
    key: 'linesBetweenQueries',
    title: t('linesBetweenQueries'),
    description: t('emptyLinesBetween'),
    options: [
      { value: '1', label: '1' },
      { value: '2', label: '2' },
      { value: '3', label: '3' },
    ],
  },
];

export const DENSE: FlagSpec = {
  key: 'denseOperators',
  title: t('denseOperators'),
  description: t('writeOperatorsWithout'),
};
