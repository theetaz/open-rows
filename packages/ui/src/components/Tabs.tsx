// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import type { KeyboardEvent } from 'react';
import { IconButton } from './Button';
import { Icon } from '../icons/Icon';
import { startTabDrag } from './tabDrag';
import { TabTitleField } from './TabTitleField';
import { useOverflowing } from './useOverflowing';
import styles from './Navigation.module.css';

type Tab = {
  id: string;
  label: string;
  kind?: 'table' | 'query' | 'processes';
  title?: string;
  /** Pinned: kept left, no close button, not closed by middle-click. */
  pinned?: boolean;
  /** The preview tab: italic until kept. */
  preview?: boolean;
  /** The editable title when `label` adds decorations (e.g. an unsaved dot). */
  name?: string;
};
/** The glyph before a tab's title, by kind. */
const KIND_GLYPHS: Readonly<Record<NonNullable<Tab['kind']>, string>> = {
  query: '›_',
  table: '▦',
  processes: '≡',
};
type Props = {
  tabs: readonly Tab[];
  active: string;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
  onAdd: () => void;
  /** Drag (or ⌥← / ⌥→) moved a tab to `index`. */
  onReorder?: ((id: string, index: number) => void) | undefined;
  /** Right-click: open the tab menu at the pointer. */
  onMenu?: ((id: string, x: number, y: number) => void) | undefined;
  /** Double-click: keep a preview tab. */
  onKeep?: ((id: string) => void) | undefined;
  /** The tab whose title is being edited in place. */
  renaming?: string | null | undefined;
  /** Double-click asks to edit a tab's title. */
  onRenameStart?: ((id: string) => void) | undefined;
  /** The edited title, or `null` when editing was cancelled. */
  onRename?: ((id: string, title: string | null) => void) | undefined;
  /** The overflow button (shown while tabs do not fit) at its bottom-left. */
  onOverflow?: ((x: number, y: number) => void) | undefined;
};
/** Shrinking, then scrolling document tabs with roving keyboard focus. */
export function Tabs(props: Props): React.JSX.Element {
  const list = useRef<HTMLDivElement>(null);
  const overflowing = useOverflowing(list, props.tabs.length);
  const { onOverflow } = props;
  return (
    <div className={styles.tabStrip}>
      <div
        ref={list}
        role="tablist"
        aria-label="Workspace tabs"
        className={styles.tabs}
      >
        {props.tabs.map((tab) => (
          <DocumentTab key={tab.id} tab={tab} owner={props} />
        ))}
      </div>
      {overflowing && onOverflow && (
        <IconButton
          aria-label="All tabs"
          aria-haspopup="menu"
          variant="ghost"
          size="sm"
          onClick={(event) => {
            const box = event.currentTarget.getBoundingClientRect();
            onOverflow(box.left, box.bottom);
          }}
        >
          <Icon name="chevronDown" />
        </IconButton>
      )}
      <IconButton
        aria-label="New tab"
        variant="ghost"
        size="sm"
        onClick={props.onAdd}
      >
        +
      </IconButton>
    </div>
  );
}
function DocumentTab({
  tab,
  owner,
}: {
  tab: Tab;
  owner: Props;
}): React.JSX.Element {
  const active = tab.id === owner.active;
  const renaming = owner.renaming === tab.id;
  const { onReorder } = owner;
  return (
    <div
      className={styles.document}
      data-active={active}
      data-kind={tab.kind ?? 'table'}
      data-pinned={tab.pinned ?? false}
      data-preview={tab.preview ?? false}
      onPointerDown={
        onReorder && !renaming
          ? (event) => {
              startTabDrag(event, tab.id, onReorder);
            }
          : undefined
      }
      onAuxClick={(event) => {
        if (event.button === 1 && !tab.pinned) {
          event.preventDefault();
          owner.onClose(tab.id);
        }
      }}
      onContextMenu={(event) => {
        if (!owner.onMenu) return;
        event.preventDefault();
        owner.onMenu(tab.id, event.clientX, event.clientY);
      }}
      onDoubleClick={() => {
        if (renaming) return;
        owner.onKeep?.(tab.id);
        owner.onRenameStart?.(tab.id);
      }}
    >
      {renaming ? (
        <TabTitleField
          title={tab.name ?? tab.label}
          onDone={(title) => owner.onRename?.(tab.id, title)}
        />
      ) : (
        <TabButton tab={tab} active={active} owner={owner} />
      )}
      {tab.pinned ? (
        <span className={styles.pin} aria-label="Pinned" role="img">
          <Icon name="pin" />
        </span>
      ) : (
        <button
          type="button"
          aria-label={`Close ${tab.label}`}
          onClick={() => {
            owner.onClose(tab.id);
          }}
        >
          ×
        </button>
      )}
    </div>
  );
}
function TabButton({
  tab,
  active,
  owner,
}: {
  tab: Tab;
  active: boolean;
  owner: Props;
}): React.JSX.Element {
  return (
    <button
      type="button"
      role="tab"
      title={tab.title}
      aria-selected={active}
      tabIndex={active ? 0 : -1}
      onClick={() => {
        owner.onSelect(tab.id);
      }}
      onKeyDown={(event) => {
        navigate(event, tab.id, owner);
      }}
    >
      <span aria-hidden="true">{KIND_GLYPHS[tab.kind ?? 'table']}</span>
      <span>{tab.label}</span>
    </button>
  );
}
/** ⌥← / ⌥→ on a focused tab moves it one place. */
function reorderKey(
  event: KeyboardEvent<HTMLButtonElement>,
  index: number,
  props: Props,
): boolean {
  const step = { ArrowLeft: -1, ArrowRight: 1 }[event.key];
  const tab = props.tabs[index];
  if (!event.altKey || !step || !tab || !props.onReorder) return false;
  event.preventDefault();
  props.onReorder(tab.id, Math.max(0, index + step));
  return true;
}
function navigate(
  event: KeyboardEvent<HTMLButtonElement>,
  id: string,
  props: Props,
): void {
  const index = props.tabs.findIndex((tab) => tab.id === id);
  if (reorderKey(event, index, props)) return;
  const targets: Record<string, number> = {
    ArrowRight: (index + 1) % props.tabs.length,
    ArrowLeft: (index - 1 + props.tabs.length) % props.tabs.length,
    Home: 0,
    End: props.tabs.length - 1,
  };
  const next = targets[event.key];
  if (next === undefined) return;
  event.preventDefault();
  const tab = props.tabs[next];
  if (tab) props.onSelect(tab.id);
  const peers = event.currentTarget
    .closest('[role="tablist"]')
    ?.querySelectorAll<HTMLButtonElement>('[role="tab"]');
  peers?.[next]?.focus();
}
