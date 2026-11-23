// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, IconButton } from '@openrows/ui';
import { isActive } from '../query/run';
import { canNext } from './useTableTab';
import type { TableTabState } from './useTableTab';
import { countKey, rangeLabel } from './model';
import styles from './Table.module.css';
import extras from './TableExtras.module.css';
import { t } from './messages/lazyData';

/** `1–300 of ~1,811 rows`, or what the page is doing. */
function pageText(state: TableTabState): string {
  const { entry, result, view, total } = state;
  if (isActive(entry.run)) return t('loadingRows');
  if (!result?.done) return entry.run.status === 'failed' ? t('failed') : '';
  return rangeLabel(view.offset, result.rows, total);
}

/** How long the shown page took, once it finished. */
function timing(state: TableTabState): string | null {
  const { run } = state.entry;
  if (isActive(run) || run.elapsedMs === null) return null;
  return t('elapsedMs', { ms: String(Math.round(run.elapsedMs)) });
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
        {t('countingCancel')}
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
          : t('countRowsExactly')
      }
      onClick={state.actions.count}
    >
      {t('count')}
    </Button>
  );
}

/**
 * The table pane's quiet status line (the kit's `.statusline`): the row
 * range or the selection statistics, Count, the page's timing and paging in
 * Data mode; the column or index total in the structure views.
 */
export function TableFooter({
  state,
  ready,
  structureStatus,
  aggregate,
}: {
  state: TableTabState;
  /** A session is available for paging and counting. */
  ready: boolean;
  /** Structure and Index modes: `9 columns`, `3 indexes`. */
  structureStatus?: string | undefined;
  /** Count, sum, average… of a multi-cell selection. */
  aggregate?: string | null | undefined;
}): React.JSX.Element {
  return (
    <footer className={styles.footer} aria-label={t('tableFooter')}>
      {structureStatus === undefined ? (
        <DataStatus state={state} ready={ready} aggregate={aggregate ?? null} />
      ) : (
        <span className={styles.range} role="status">
          {structureStatus}
        </span>
      )}
    </footer>
  );
}

function DataStatus({
  state,
  ready,
  aggregate,
}: {
  state: TableTabState;
  ready: boolean;
  aggregate: string | null;
}): React.JSX.Element {
  const elapsed = timing(state);
  return (
    <>
      {aggregate ? (
        <span
          className={extras.aggregate}
          role="status"
          aria-label={t('selectionStatistics')}
          title={aggregate}
        >
          {aggregate}
        </span>
      ) : (
        <span
          className={styles.range}
          role="status"
          aria-label={t('rowsShown')}
          title={pageText(state)}
        >
          {pageText(state)}
        </span>
      )}
      {ready && <CountControl state={state} />}
      <span className={styles.spacer} />
      {elapsed && (
        <span className={styles.timing}>
          <Icon name="clock" />
          {elapsed}
        </span>
      )}
      <Paging state={state} ready={ready} />
    </>
  );
}

/** ‹ › page controls, like TablePlus. */
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
        aria-label={t('previousPage')}
        disabled={!ready || view.offset === 0 || isActive(entry.run)}
        onClick={() => {
          actions.page(-1);
        }}
      >
        <Icon name="chevronLeft" />
      </IconButton>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('nextPage')}
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
