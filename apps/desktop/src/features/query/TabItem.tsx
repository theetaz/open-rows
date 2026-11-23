// SPDX-License-Identifier: Apache-2.0
import { memo, useEffect, useRef } from 'react';
import type { CSSProperties } from 'react';
import { Icon } from '@openrows/ui';
import type { IconName } from '@openrows/ui';
import type { QueryTab } from './tabs';
import styles from './TabStrip.module.css';
import { t } from './tabMessages';

/** The icon before a tab's title, by kind. */
const KIND_ICONS: Readonly<Record<QueryTab['kind'], IconName>> = {
  query: 'terminal',
  table: 'table',
  processes: 'list',
  monitor: 'activity',
  inspection: 'eye',
  connection: 'plug',
};

/** The tab title; a file-bound tab with unsaved changes gets a dot. */
export function tabLabel(tab: QueryTab): string {
  return tab.file && tab.file.saved !== tab.sql ? `${tab.title} •` : tab.title;
}

/** What a tab does, routed by the strip (stable across renders). */
export type TabItemActions = {
  select: (id: string) => void;
  close: (id: string) => void;
  menu: (id: string, x: number, y: number) => void;
  /** Double-click: keep a preview tab and edit its title. */
  rename: (id: string) => void;
  renamed: (id: string, title: string | null) => void;
  pointerDown: (event: React.PointerEvent<HTMLDivElement>) => void;
  key: (event: React.KeyboardEvent<HTMLButtonElement>, id: string) => void;
};

type Props = {
  tab: QueryTab;
  active: boolean;
  /** The tab's menu is open: it keeps an accent ring. */
  menu: boolean;
  renaming: boolean;
  /** The group's colour token, e.g. `var(--or-group-blue)`. */
  group: string | undefined;
  prod: boolean;
  /** Tooltip: the connection, environment and title. */
  tooltip: string;
  actions: TabItemActions;
};

/** One tab: icon, fading title, a prod dot and the close button. */
export const TabItem = memo(function TabItem(props: Props): React.JSX.Element {
  const { tab, actions } = props;
  const label = tabLabel(tab);
  return (
    <div
      className={styles.tab}
      data-tab-id={tab.id}
      data-active={props.active}
      data-pinned={tab.pinned ?? false}
      data-preview={tab.preview ?? false}
      data-menu={props.menu}
      data-group={props.group ? '' : undefined}
      style={
        props.group ? ({ '--group': props.group } as CSSProperties) : undefined
      }
      onPointerDown={props.renaming ? undefined : actions.pointerDown}
      onAuxClick={(event) => {
        if (event.button !== 1 || tab.pinned) return;
        event.preventDefault();
        actions.close(tab.id);
      }}
      onContextMenu={(event) => {
        event.preventDefault();
        actions.menu(tab.id, event.clientX, event.clientY);
      }}
      onDoubleClick={() => {
        if (!props.renaming && !tab.pinned) actions.rename(tab.id);
      }}
    >
      {props.renaming ? (
        <TitleField
          title={tab.title}
          onDone={(title) => {
            actions.renamed(tab.id, title);
          }}
        />
      ) : (
        <button
          type="button"
          role="tab"
          className={styles.select}
          title={props.tooltip}
          aria-selected={props.active}
          tabIndex={props.active ? 0 : -1}
          onClick={() => {
            actions.select(tab.id);
          }}
          onKeyDown={(event) => {
            actions.key(event, tab.id);
          }}
        >
          <Icon name={KIND_ICONS[tab.kind]} />
          <span className={styles.title}>{label}</span>
        </button>
      )}
      {props.prod && (
        <span className={styles.prod} role="img" aria-label={t('production')} />
      )}
      {!tab.pinned && (
        <button
          type="button"
          className={styles.close}
          tabIndex={-1}
          aria-label={t('closeLabel', { label })}
          onPointerDown={(event) => {
            event.stopPropagation();
          }}
          onClick={() => {
            actions.close(tab.id);
          }}
        >
          <Icon name="close" />
        </button>
      )}
    </div>
  );
});

/**
 * Inline editor of a tab's title: Enter or leaving the field renames,
 * Escape cancels (`null`). An empty title keeps the old one.
 */
function TitleField(props: {
  title: string;
  onDone: (title: string | null) => void;
}): React.JSX.Element {
  const field = useRef<HTMLInputElement>(null);
  const done = useRef(false);
  useEffect(() => {
    field.current?.focus();
    field.current?.select();
  }, []);
  const finish = (title: string | null): void => {
    if (done.current) return;
    done.current = true;
    const trimmed = title?.trim();
    props.onDone(trimmed ? trimmed : null);
  };
  return (
    <input
      ref={field}
      className={styles.rename}
      aria-label={t('tabName')}
      defaultValue={props.title}
      spellCheck={false}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          event.preventDefault();
          finish(event.currentTarget.value);
        } else if (event.key === 'Escape') {
          event.preventDefault();
          event.stopPropagation();
          finish(null);
        }
      }}
      onBlur={(event) => {
        finish(event.currentTarget.value);
      }}
    />
  );
}
