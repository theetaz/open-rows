// SPDX-License-Identifier: Apache-2.0
import { Icon, Picker } from '@openrows/ui';
import type { PickerSection } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import type { QueryTab, TabsState } from './tabs';
import { tabLabel } from './TabItem';
import { t } from './tabMessages';

const ICONS = {
  query: 'terminal',
  table: 'table',
  processes: 'list',
  monitor: 'activity',
  inspection: 'eye',
  connection: 'plug',
} as const;

/** The section a tab is listed under: pinned, its group, or plain tabs. */
function headOf(state: TabsState, tab: QueryTab): string {
  if (tab.pinned) return t('pinned');
  const group = tab.group ? state.groups?.[tab.group] : undefined;
  if (group) return group.name || t('unnamedGroup');
  return t('tabs');
}

/** Tabs in strip order, one section per run of pinned, grouped or plain tabs. */
function sections(
  state: TabsState,
  list: readonly QueryTab[],
  active: string | undefined,
  profiles: readonly ProfileDto[],
): PickerSection[] {
  const out: { head: string; items: PickerSection['items'][number][] }[] = [];
  for (const tab of list) {
    const head = headOf(state, tab);
    if (out.at(-1)?.head !== head) out.push({ head, items: [] });
    const profile = profiles.find((item) => item.id === tab.connectionId);
    out.at(-1)?.items.push({
      id: tab.id,
      name: tabLabel(tab),
      lead: <Icon name={ICONS[tab.kind]} />,
      ...(profile ? { sub: `${profile.name} · ${tab.database}` } : {}),
      current: tab.id === active,
    });
  }
  return out;
}

/** The strip's ⌄ list: every tab of the window, searchable (DESIGN.md §7). */
export function TabListPicker(props: {
  anchor: Element;
  state: TabsState;
  list: readonly QueryTab[];
  active: string | undefined;
  profiles: readonly ProfileDto[];
  onPick: (id: string) => void;
  onClose: () => void;
}): React.JSX.Element {
  return (
    <Picker
      anchor={props.anchor}
      placeholder={t('searchTabs')}
      sections={sections(props.state, props.list, props.active, props.profiles)}
      dx={280}
      foot={
        <>
          <span>
            <kbd>↑↓</kbd>
            {t('navigate')}
          </span>
          <span>
            <kbd>↩</kbd>
            {t('switchTo')}
          </span>
          <span>{t('tabsOpen', { count: props.list.length })}</span>
        </>
      }
      onPick={(item) => {
        props.onClose();
        props.onPick(item.id);
      }}
      onClose={props.onClose}
    />
  );
}
