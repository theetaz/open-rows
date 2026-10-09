// SPDX-License-Identifier: Apache-2.0
import { useEffect, useEffectEvent, useState } from 'react';
import { Button, Icon, IconButton } from '@openrows/ui';
import type { ResultMeta } from '../query/run';
import { RowDetails } from './RowDetails';
import type { SelectedRow } from './row';
import { useSelectedRow } from './useSelectedRow';
import type { RowEditing } from './useRowEditing';
import styles from './TableExtras.module.css';

type Props = {
  table: string;
  result: ResultMeta;
  editing: RowEditing;
  ready: boolean;
  /** First row of the page within the table view. */
  offset: number;
  /** The shown page row. */
  row: number;
  onRow: (row: number) => void;
  /** Back to the grid. */
  onClose: () => void;
};

/**
 * The current row as a vertical form (FR-GRID-09): every field with its
 * type, edited through the staged change set, with previous/next row
 * navigation within the page.
 */
export function RowForm(props: Props): React.JSX.Element {
  const { result, row } = props;
  const [shown, setShown] = useState<SelectedRow | null>(null);
  const pick = useSelectedRow({
    table: props.table,
    offset: props.offset,
    editing: props.editing,
    ready: props.ready,
    onRow: setShown,
  });
  const select = useEffectEvent(() => {
    if (row < result.rows) pick(result, row);
  });
  useEffect(() => {
    select();
  }, [result, row]);
  return (
    <section className={styles.form} aria-label="Row form">
      <FormBar {...props} />
      <div className={styles.formBody}>
        {shown ? <RowDetails row={shown} /> : <p role="status">Loading row…</p>}
      </div>
    </section>
  );
}

/** ‹ Row 3 of 300 › · Grid */
function FormBar({
  result,
  row,
  offset,
  onRow,
  onClose,
}: Props): React.JSX.Element {
  const last = result.rows - 1;
  return (
    <div className={styles.formBar} role="toolbar" aria-label="Row navigation">
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Previous row"
        disabled={row <= 0}
        onClick={() => {
          onRow(row - 1);
        }}
      >
        <Icon name="chevronLeft" />
      </IconButton>
      <span role="status" aria-label="Current row">
        Row {(offset + row + 1).toLocaleString('en-US')} ·{' '}
        {(row + 1).toLocaleString('en-US')} of{' '}
        {result.rows.toLocaleString('en-US')} on this page
      </span>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Next row"
        disabled={row >= last}
        onClick={() => {
          onRow(row + 1);
        }}
      >
        <Icon name="chevronRight" />
      </IconButton>
      <span className={styles.spacer} />
      <Button size="sm" onClick={onClose}>
        Show grid
      </Button>
    </div>
  );
}
