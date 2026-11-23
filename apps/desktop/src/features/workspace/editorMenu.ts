// SPDX-License-Identifier: Apache-2.0
import { createElement } from 'react';
import { Icon } from '@openrows/ui';
import type { IconName, MenuEntry } from '@openrows/ui';
import type { objectEntries, ObjectEditorRequest } from '../objects';

/** One object-editor command: `request: null` when the engine cannot. */
type EditorEntry = ReturnType<typeof objectEntries>[number];

/** The icon of an editor command, from its verb. */
function verbIcon(label: string): React.JSX.Element {
  const verb = label.split(' ')[0] ?? '';
  const icons: Record<string, IconName> = {
    New: 'plus',
    Rename: 'pencil',
    Edit: 'pencil',
    Duplicate: 'duplicate',
    Truncate: 'drop',
    Drop: 'trash',
  };
  return createElement(Icon, { name: icons[verb] ?? 'pencil' });
}

/** Editor commands as menu rows; unsupported ones are dimmed. */
function editorItems(
  entries: readonly EditorEntry[],
  choose: (request: ObjectEditorRequest) => void,
): MenuEntry[] {
  return entries.map((entry) => ({
    kind: 'item',
    label: entry.label,
    icon: verbIcon(entry.label),
    danger: entry.danger === true,
    disabled: !entry.request,
    run: () => {
      if (entry.request) choose(entry.request);
    },
  }));
}

/** Safe commands first, then a separator and the destructive ones. */
export function editorSections(
  entries: readonly EditorEntry[],
  choose: (request: ObjectEditorRequest) => void,
): MenuEntry[] {
  const safe = editorItems(
    entries.filter((entry) => !entry.danger),
    choose,
  );
  const danger = editorItems(
    entries.filter((entry) => entry.danger),
    choose,
  );
  return [
    ...safe,
    ...(safe.length && danger.length ? [{ kind: 'separator' as const }] : []),
    ...danger,
  ];
}

/** A note under the rows when some commands are unsupported. */
export const hasUnsupported = (entries: readonly EditorEntry[]): boolean =>
  entries.some((entry) => !entry.request);
