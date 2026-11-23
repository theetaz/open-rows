// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import { Icon, Popover, TextField } from '@openrows/ui';
import type { IconName } from '@openrows/ui';
import { GROUP_COLOURS, changeGroup, groupTabs, ungroup } from './tabGroups';
import type { TabGroup } from './tabGroups';
import type { QueryTabs } from './useQueryTabs';
import styles from './TabStrip.module.css';
import { t } from './tabMessages';

/** What the group's actions ask of the strip. */
export type GroupEditorActions = {
  newTabIn: (group: string) => void;
  toggle: (group: string) => void;
};

/** One of the editor's menu-like rows. */
function Action(props: {
  icon: IconName;
  label: string;
  danger?: boolean;
  onClick: () => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      className={styles.action}
      data-danger={props.danger ?? false}
      onClick={props.onClick}
    >
      <Icon name={props.icon} />
      {props.label}
    </button>
  );
}

/** Eight colour swatches as a radio group. */
function Swatches(props: {
  group: TabGroup;
  onPick: (colour: TabGroup['colour']) => void;
}): React.JSX.Element {
  return (
    <div
      className={styles.swatches}
      role="radiogroup"
      aria-label={t('groupColour')}
    >
      {GROUP_COLOURS.map((colour) => (
        <button
          key={colour}
          type="button"
          role="radio"
          aria-label={colour}
          aria-checked={props.group.colour === colour}
          className={styles.swatch}
          style={{ '--swatch': `var(--or-group-${colour})` } as CSSProperties}
          onClick={() => {
            props.onPick(colour);
          }}
        />
      ))}
    </div>
  );
}

/** New Tab in Group, Collapse, Ungroup and Close Group; each closes the editor first. */
function GroupActions(props: {
  tabs: QueryTabs;
  group: TabGroup;
  actions: GroupEditorActions;
  then: (run: () => void) => () => void;
}): React.JSX.Element {
  const { tabs, group, then } = props;
  return (
    <>
      <Action
        icon="plus"
        label={t('newTabInGroup')}
        onClick={then(() => {
          props.actions.newTabIn(group.id);
        })}
      />
      <Action
        icon="chevronUpDown"
        label={group.collapsed ? t('expandGroup') : t('collapseGroup')}
        onClick={then(() => {
          props.actions.toggle(group.id);
        })}
      />
      <Action
        icon="list"
        label={t('ungroup')}
        onClick={then(() => tabs.apply?.((state) => ungroup(state, group.id)))}
      />
      <Action
        icon="close"
        label={t('closeGroup')}
        danger
        onClick={then(() => {
          const ids = groupTabs(tabs.state, group.id);
          if (tabs.closeMany) tabs.closeMany(ids);
          else
            ids.forEach((id) => {
              tabs.close(id);
            });
        })}
      />
    </>
  );
}

/**
 * A group's editor under its chip (right-click or double-click): its name,
 * colour, New Tab in Group, Collapse, Ungroup and Close Group.
 */
export function TabGroupEditor(props: {
  tabs: QueryTabs;
  group: TabGroup;
  anchor: Element;
  actions: GroupEditorActions;
  onClose: () => void;
}): React.JSX.Element {
  const { tabs, group } = props;
  const editor = useRef<HTMLDivElement>(null);
  // The name is kept here while typing and stored once, on closing.
  const [name, setName] = useState(group.name);
  useEffect(() => {
    const field = editor.current?.querySelector('input');
    field?.focus();
    field?.select();
  }, []);
  const change = (next: Partial<Omit<TabGroup, 'id'>>): void => {
    tabs.apply?.((state) => changeGroup(state, group.id, next));
  };
  const onClose = (): void => {
    if (name !== group.name) change({ name });
    props.onClose();
  };
  const then = (run: () => void) => () => {
    onClose();
    run();
  };
  return (
    <Popover
      anchor={props.anchor}
      label={t('editGroup', { name: group.name || t('unnamedGroup') })}
      onClose={onClose}
    >
      <div ref={editor} className={styles.editor}>
        <TextField
          aria-label={t('groupName')}
          placeholder={t('nameGroup')}
          maxLength={24}
          value={name}
          onChange={(event) => {
            setName(event.currentTarget.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') onClose();
          }}
        />
        <Swatches
          group={group}
          onPick={(colour) => {
            change({ colour });
          }}
        />
        <div className={styles.separator} role="separator" />
        <GroupActions
          tabs={tabs}
          group={group}
          actions={props.actions}
          then={then}
        />
      </div>
    </Popover>
  );
}
