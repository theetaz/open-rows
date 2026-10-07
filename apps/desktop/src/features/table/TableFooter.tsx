// SPDX-License-Identifier: Apache-2.0
import { Button, IconButton, SegmentedControl } from '@openrows/ui';
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
}: {
  state: TableTabState;
  /** A session is available for paging and counting. */
  ready: boolean;
}): React.JSX.Element {
  const { view, actions, entry } = state;
  return (
    <footer className={styles.footer} aria-label="Table footer">
      <SegmentedControl
        aria-label="Table view"
        size="compact"
        options={modes}
        value={view.mode}
        onValueChange={actions.mode}
      />
      <span title="Editing arrives in D-09">
        <Button size="sm" disabled title="Editing arrives in D-09">
          + Row
        </Button>
      </span>
      <span className={styles.range} role="status" aria-label="Rows shown">
        {pageText(state)}
      </span>
      {ready && view.mode === 'data' && <CountControl state={state} />}
      <span className={styles.spacer} />
      <Button
        size="sm"
        aria-pressed={view.filtersOpen}
        onClick={actions.toggleFilters}
      >
        Filters
      </Button>
      <Button
        size="sm"
        aria-pressed={view.columnsOpen}
        onClick={actions.toggleColumns}
      >
        Columns
      </Button>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Previous page (⌘[)"
        disabled={!ready || view.offset === 0 || isActive(entry.run)}
        onClick={() => {
          actions.page(-1);
        }}
      >
        ‹
      </IconButton>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Next page (⌘])"
        disabled={!ready || !canNext(entry)}
        onClick={() => {
          actions.page(1);
        }}
      >
        ›
      </IconButton>
    </footer>
  );
}
