// SPDX-License-Identifier: Apache-2.0
import type { EditorDialect, FormatKind } from '@openrows/editor';
import type { MenuEntry } from '../../shell/CascadeMenu';
import { FONT_SIZES, ROW_LIMITS } from './editorSettings';
import type { EditorSettings } from './editorSettings';
import { paramChoices } from './queryParams';

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
    toggle(settings, update, 'Auto Save SQL Query Tab', 'autoSave'),
    {
      kind: 'submenu',
      label: 'Query Params',
      entries: choices(settings, update, 'queryParams', paramChoices(dialect)),
    },
    {
      kind: 'submenu',
      label: 'Result View Mode',
      entries: choices(settings, update, 'resultView', [
        ['One Tab per Result', 'tabs'],
        ['Last Result Only', 'last'],
      ]),
    },
    toggle(settings, update, 'Return on Error', 'returnOnError'),
  ];
}

/**
 * The sliders menu: font size, invisibles, wrapping, highlight, completion,
 * keys, then auto save and run options.
 */
export function settingsEntries(
  settings: EditorSettings,
  update: Update,
  dialect: EditorDialect,
): MenuEntry[] {
  const sizes = FONT_SIZES.map((size) => [String(size), size] as const);
  return [
    {
      kind: 'submenu',
      label: 'Font Size',
      entries: choices(settings, update, 'fontSize', [
        ['Default', null],
        ...sizes,
      ]),
    },
    toggle(settings, update, 'Show invisible characters', 'invisibles'),
    toggle(settings, update, 'Wrap lines to editor width', 'wrap'),
    toggle(settings, update, 'Highlight current query', 'highlightQuery'),
    { kind: 'separator' },
    {
      kind: 'submenu',
      label: 'Autocomplete',
      entries: [
        toggle(settings, update, 'Enable autocomplete', 'autocomplete'),
        toggle(settings, update, 'Uppercase keywords', 'upperKeywords'),
      ],
    },
    {
      kind: 'submenu',
      label: 'Key Binding',
      entries: choices(settings, update, 'keyBinding', [
        ['Default', 'default'],
        ['Vim', 'vim'],
      ]),
    },
    { kind: 'separator' },
    ...runSettings(settings, update, dialect),
  ];
}

/** `No limit` or e.g. `1,000 rows`. */
export function limitLabel(limit: number | null): string {
  return limit === null ? 'No limit' : `${limit.toLocaleString('en-US')} rows`;
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

/** Beautify's dropdown: uglify and the layout options of both. */
export function beautifyEntries(
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
    item('Beautify', '⌘I', 'beautify'),
    item('Uglify', '⇧⌘I', 'uglify'),
    { kind: 'separator' },
    {
      kind: 'submenu',
      label: 'Indent Width',
      entries: choices(settings, update, 'indentWidth', [
        ['2 spaces', 2],
        ['4 spaces', 4],
      ]),
    },
    {
      kind: 'submenu',
      label: 'Indent With',
      entries: choices(settings, update, 'indentWith', [
        ['Spaces', 'spaces'],
        ['Tabs', 'tabs'],
      ]),
    },
    {
      kind: 'submenu',
      label: 'Keyword Case',
      entries: choices(settings, update, 'keywordCase', [
        ['Uppercase', 'upper'],
        ['Lowercase', 'lower'],
        ['Preserve', 'preserve'],
      ]),
    },
    {
      kind: 'submenu',
      label: 'Indent Type',
      entries: choices(settings, update, 'indentStyle', [
        ['Standard', 'standard'],
        ['Tabular Left', 'tabularLeft'],
        ['Tabular Right', 'tabularRight'],
      ]),
    },
    {
      kind: 'submenu',
      label: 'Lines Between Queries',
      entries: choices(settings, update, 'linesBetweenQueries', [
        ['1', 1],
        ['2', 2],
        ['3', 3],
      ]),
    },
    toggle(settings, update, 'Dense Operators', 'denseOperators'),
  ];
}
