// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Icon, IconButton, SegmentedControl } from '@openrows/ui';
import { PointMenu } from '../../shell/PointMenu';
import { isActive } from '../query/run';
import { canNext } from './useTableTab';
import type { TableTabState } from './useTableTab';
import { countKey, rangeLabel } from './model';
import type { ViewMode } from './model';
import styles from './Table.module.css';

const modes: readonly { value: ViewMode; label: string }[] = [
  { value: 'data', label: 'Data' },
  { value: 'structure', label: 'Structure' },
  { value: 'indexes', label: 'Indexes' },
];

/** `1–300 of ~1,811 rows`, or what the page is doing. */
function pageText(state: TableTabState): string {
  const { entry, result, view, total } = state;
  if (isActive(entry.run)) return 'Loading rows…';
  if (!result?.done) return entry.run.status === 'failed' ? 'Failed' : '';
  return rangeLabel(view.offset, result.rows, total);
}

/** Exact count action: Count, Counting… with Cancel, or the error. */
function CountControl({
  state,
}: {
  state: TableTabState;
}): React.JSX.Element | null {
  const { count } = state.entry;
  const current = 'key' in count && count.key === countKey(state.query);
  if (current && count.status === 'counting')
    return (
      <Button size="sm" variant="ghost" onClick={state.cancelCount}>
        Counting… Cancel
      </Button>
    );
  if (current && count.status === 'done') return null;
  return (
    <Button
      size="sm"
      variant="ghost"
      title={
        current && count.status === 'error'
          ? count.message
          : 'Count rows exactly'
      }
      onClick={state.actions.count}
    >
      Count
    </Button>
  );
}

/** TablePlus-style footer: views, + Row, page range, Filters, Columns, paging. */
export function TableFooter({
  state,
  ready,
  onInsert,
  insertTitle,
}: {
  state: TableTabState;
  /** A session is available for paging and counting. */
  ready: boolean;
  /** Stage a new row; absent when the table cannot be edited. */
  onInsert?: (() => void) | undefined;
  /** Why rows cannot be added, or how editing works. */
  insertTitle: string;
}): React.JSX.Element {
  const { view, actions } = state;
  return (
    <footer className={styles.footer} aria-label="Table footer">
      <SegmentedControl
        aria-label="Table view"
        size="compact"
        tone="accent"
        options={modes}
        value={view.mode}
        onValueChange={actions.mode}
      />
      <Button
        size="sm"
        aria-label="Add row"
        disabled={!onInsert}
        title={insertTitle}
        onClick={onInsert}
      >
        <Icon name="plus" />
        <span className={styles.label}>Row</span>
      </Button>
      <span className={styles.status}>
        <span
          className={styles.range}
          role="status"
          aria-label="Rows shown"
          title={pageText(state)}
        >
          {pageText(state)}
        </span>
        {ready && view.mode === 'data' && <CountControl state={state} />}
      </span>
      <Button
        size="sm"
        aria-label="Filters"
        title="Filters (⌘F)"
        aria-pressed={view.filtersOpen}
        onClick={actions.toggleFilters}
      >
        <Icon name="filter" />
        <span className={styles.label}>Filters</span>
      </Button>
      <Button
        size="sm"
        aria-label="Columns"
        title="Columns"
        aria-pressed={view.columnsOpen}
        onClick={actions.toggleColumns}
      >
        <Icon name="columns" />
        <span className={styles.label}>Columns</span>
      </Button>
      <Paging state={state} ready={ready} />
    </footer>
  );
}

/** ‹ display options › like TablePlus' page controls. */
function Paging({
  state,
  ready,
}: {
  state: TableTabState;
  ready: boolean;
}): React.JSX.Element {
  const { view, actions, entry } = state;
  return (
    <span className={styles.paging}>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Previous page (⌘[)"
        disabled={!ready || view.offset === 0 || isActive(entry.run)}
        onClick={() => {
          actions.page(-1);
        }}
      >
        <Icon name="chevronLeft" />
      </IconButton>
      <DisplayMenu state={state} />
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Next page (⌘])"
        disabled={!ready || !canNext(entry)}
        onClick={() => {
          actions.page(1);
        }}
      >
        <Icon name="chevronRight" />
      </IconButton>
    </span>
  );
}

/** Row numbers and stripes, toggled from a small menu. */
function DisplayMenu({
  state: { view, actions },
}: {
  state: TableTabState;
}): React.JSX.Element {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const items = [
    ['rowNumbers', 'Row numbers', view.rowNumbers],
    ['striped', 'Alternate row backgrounds', view.striped],
  ] as const;
  return (
    <>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Display options"
        aria-haspopup="menu"
        aria-expanded={at !== null}
        onClick={(event) => {
          const box = event.currentTarget.getBoundingClientRect();
          setAt({ x: box.left, y: box.top });
        }}
      >
        <Icon name="settings" />
      </IconButton>
      {at && (
        <PointMenu
          x={at.x}
          y={at.y - 80}
          label="Display options"
          onClose={() => {
            setAt(null);
          }}
        >
          {items.map(([key, label, checked]) => (
            <button
              key={key}
              type="button"
              role="menuitemcheckbox"
              aria-checked={checked}
              onClick={() => {
                actions.display(key);
              }}
            >
              {label}
              <kbd>{checked ? '✓' : ''}</kbd>
            </button>
          ))}
        </PointMenu>
      )}
    </>
  );
}
