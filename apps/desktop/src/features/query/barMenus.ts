// SPDX-License-Identifier: Apache-2.0
import type { FormatKind } from '@openrows/editor';
import type { MenuEntry } from '../../shell/CascadeMenu';
import { FONT_SIZES, ROW_LIMITS } from './editorSettings';
import type { EditorSettings } from './editorSettings';
import type { RunScope } from './request';

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
    'invisibles' | 'wrap' | 'highlightQuery' | 'autocomplete' | 'upperKeywords',
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

/** The sliders menu: font size, invisibles, wrapping, highlight, completion, keys. */
export function settingsEntries(
  settings: EditorSettings,
  update: Update,
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

/** Beautify's dropdown: uglify and the layout options. */
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
  ];
}

/** Run's dropdown. */
export function runEntries(onRun: (scope: RunScope) => void): MenuEntry[] {
  return [
    {
      kind: 'item',
      label: 'Run All',
      shortcut: '⇧⌘↵',
      run: () => {
        onRun('all');
      },
    },
    {
      kind: 'item',
      label: 'Run Current',
      shortcut: '⌘↵',
      run: () => {
        onRun('current');
      },
    },
  ];
}
