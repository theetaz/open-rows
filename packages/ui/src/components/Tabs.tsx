// SPDX-License-Identifier: Apache-2.0
import type { KeyboardEvent } from 'react';
import { IconButton } from './Button';
import styles from './Navigation.module.css';

type Tab = { id: string; label: string; kind?: 'table' | 'query' };
type Props = {
  tabs: readonly Tab[];
  active: string;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
  onAdd: () => void;
};
/** Shrinking, then scrolling document tabs with roving keyboard focus. */
export function Tabs(props: Props): React.JSX.Element {
  return (
    <div className={styles.tabStrip}>
      <div role="tablist" aria-label="Workspace tabs" className={styles.tabs}>
        {props.tabs.map((tab) => (
          <DocumentTab key={tab.id} tab={tab} owner={props} />
        ))}
      </div>
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
  return (
    <div
      className={styles.document}
      data-active={active}
      onAuxClick={(event) => {
        if (event.button === 1) {
          event.preventDefault();
          owner.onClose(tab.id);
        }
      }}
    >
      <button
        type="button"
        role="tab"
        aria-selected={active}
        tabIndex={active ? 0 : -1}
        onClick={() => {
          owner.onSelect(tab.id);
        }}
        onKeyDown={(event) => {
          navigate(event, tab.id, owner);
        }}
      >
        <span aria-hidden="true">{tab.kind === 'query' ? '›_' : '▦'}</span>
        <span>{tab.label}</span>
      </button>
      <button
        type="button"
        aria-label={`Close ${tab.label}`}
        onClick={() => {
          owner.onClose(tab.id);
        }}
      >
        ×
      </button>
    </div>
  );
}
function navigate(
  event: KeyboardEvent<HTMLButtonElement>,
  id: string,
  props: Props,
): void {
  const index = props.tabs.findIndex((tab) => tab.id === id);
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
