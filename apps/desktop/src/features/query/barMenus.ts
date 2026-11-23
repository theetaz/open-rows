// SPDX-License-Identifier: Apache-2.0
import type { EditorDialect, FormatKind } from '@openrows/editor';
import type { MenuEntry } from '../../shell/CascadeMenu';
import { FONT_SIZES, ROW_LIMITS } from './editorSettings';
import type { EditorSettings } from './editorSettings';
import { paramChoices } from './queryParams';
import { t } from './lazyMessages';

type Update = (change: Partial<EditorSettings>) => void;

const choice = (
  label: string,
  checked: boolean,
  run: () => void,
): MenuEntry => ({
  kind: 'item',
  label,
  checked,
  radio: true,
  run,
});

const toggle = (
  settings: EditorSettings,
  update: Update,
  label: string,
  key:
    | 'invisibles'
    | 'wrap'
    | 'highlightQuery'
    | 'autocomplete'
    | 'upperKeywords'
    | 'autoSave'
    | 'returnOnError'
    | 'denseOperators',
): MenuEntry => ({
  kind: 'item',
  label,
  checked: settings[key],
  run: () => {
    update({ [key]: !settings[key] });
  },
});

/** Radio choices of one setting. */
function choices<K extends keyof EditorSettings>(
  settings: EditorSettings,
  update: Update,
  key: K,
  options: readonly (readonly [string, EditorSettings[K]])[],
): MenuEntry[] {
  return options.map(([label, value]) =>
    choice(label, settings[key] === value, () => {
      update({ [key]: value });
    }),
  );
}

/** Auto save, parameters, result view and error handling of runs. */
function runSettings(
  settings: EditorSettings,
  update: Update,
  dialect: EditorDialect,
): MenuEntry[] {
  return [
    toggle(settings, update, t('autoSaveSql'), 'autoSave'),
    {
      kind: 'submenu',
      label: t('queryParams'),
      entries: choices(settings, update, 'queryParams', paramChoices(dialect)),
    },
    {
      kind: 'submenu',
      label: t('resultViewMode'),
      entries: choices(settings, update, 'resultView', [
        [t('oneTabPer'), 'tabs'],
        [t('stackedResults'), 'stacked'],
        [t('lastResultOnly'), 'last'],
      ]),
    },
    toggle(settings, update, t('returnError'), 'returnOnError'),
  ];
}

/**
 * Editor settings: font size, invisibles, wrapping, highlight, completion,
 * keys, then auto save and run options.
 */
function settingsEntries(
  settings: EditorSettings,
  update: Update,
  dialect: EditorDialect,
): MenuEntry[] {
  const sizes = FONT_SIZES.map((size) => [String(size), size] as const);
  return [
    {
      kind: 'submenu',
      label: t('fontSize'),
      entries: choices(settings, update, 'fontSize', [
        [t('default'), null],
        ...sizes,
      ]),
    },
    toggle(settings, update, t('showInvisibleCharacters'), 'invisibles'),
    toggle(settings, update, t('wrapLinesEditor'), 'wrap'),
    toggle(settings, update, t('highlightCurrentQuery'), 'highlightQuery'),
    { kind: 'separator' },
    {
      kind: 'submenu',
      label: t('autocomplete'),
      entries: [
        toggle(settings, update, t('enableAutocomplete'), 'autocomplete'),
        toggle(settings, update, t('uppercaseKeywords'), 'upperKeywords'),
      ],
    },
    {
      kind: 'submenu',
      label: t('keyBinding'),
      entries: choices(settings, update, 'keyBinding', [
        [t('default'), 'default'],
        [t('vim'), 'vim'],
      ]),
    },
    { kind: 'separator' },
    ...runSettings(settings, update, dialect),
  ];
}

/** `No limit` or e.g. `1,000 rows`. */
export function limitLabel(limit: number | null): string {
  return limit === null
    ? t('noLimit')
    : t('limitRows', { limit: limit.toLocaleString('en-US') });
}

/** Row limit choices for the next run. */
export function limitEntries(
  settings: EditorSettings,
  update: Update,
): MenuEntry[] {
  const limits = [null, ...ROW_LIMITS].map(
    (limit) => [limitLabel(limit), limit] as const,
  );
  return choices(settings, update, 'rowLimit', limits);
}

/** Beautify, uglify and the layout options of both. */
function beautifyEntries(
  settings: EditorSettings,
  update: Update,
  format: (kind: FormatKind) => void,
): MenuEntry[] {
  const item = (
    label: string,
    shortcut: string,
    kind: FormatKind,
  ): MenuEntry => ({
    kind: 'item',
    label,
    shortcut,
    run: () => {
      format(kind);
    },
  });
  return [
    item(t('beautify'), '⌥⌘B', 'beautify'),
    item(t('uglify'), '⇧⌘I', 'uglify'),
    { kind: 'separator' },
    {
      kind: 'submenu',
      label: t('indentWidth'),
      entries: choices(settings, update, 'indentWidth', [
        ['2 spaces', 2],
        ['4 spaces', 4],
      ]),
    },
    {
      kind: 'submenu',
      label: t('indent'),
      entries: choices(settings, update, 'indentWith', [
        [t('spaces'), 'spaces'],
        [t('tabs'), 'tabs'],
      ]),
    },
    {
      kind: 'submenu',
      label: t('keywordCase'),
      entries: choices(settings, update, 'keywordCase', [
        [t('uppercase'), 'upper'],
        [t('lowercase'), 'lower'],
        [t('preserve'), 'preserve'],
      ]),
    },
    {
      kind: 'submenu',
      label: t('indentType'),
      entries: choices(settings, update, 'indentStyle', [
        [t('standard'), 'standard'],
        [t('tabularLeft'), 'tabularLeft'],
        [t('tabularRight'), 'tabularRight'],
      ]),
    },
    {
      kind: 'submenu',
      label: t('linesBetweenQueries'),
      entries: choices(settings, update, 'linesBetweenQueries', [
        ['1', 1],
        ['2', 2],
        ['3', 3],
      ]),
    },
    toggle(settings, update, t('denseOperators'), 'denseOperators'),
  ];
}

/**
 * The bar's More menu: Edit SQL with AI, the formatting commands and their
 * layout options, then the editor settings.
 */
export function moreEntries(input: {
  settings: EditorSettings;
  update: Update;
  dialect: EditorDialect;
  format: (kind: FormatKind) => void;
  /** The ⌘I prompt; omitted where AI is not offered. */
  inline?: (() => void) | undefined;
}): MenuEntry[] {
  const { settings, update, inline } = input;
  const ai: MenuEntry[] = inline
    ? [
        { kind: 'item', label: t('editSqlAi'), shortcut: '⌘I', run: inline },
        { kind: 'separator' },
      ]
    : [];
  return [
    ...ai,
    ...beautifyEntries(settings, update, input.format),
    { kind: 'separator' },
    ...settingsEntries(settings, update, input.dialect),
  ];
}
