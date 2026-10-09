// SPDX-License-Identifier: Apache-2.0
import { CascadeMenu } from '../../shell/CascadeMenu';
import type { MenuEntry } from '../../shell/CascadeMenu';
import { copyAsMenu } from '../../results/copy/copySelection';
import type { CopyFormat } from '../../results/copy/formats';
import type { RowOrder } from './localRows';

/** What a query result's context menu acts on and can do. */
type ResultMenuProps = {
  x: number;
  y: number;
  kind: 'cell' | 'row' | 'column';
  /** Name of the right-clicked column. */
  name: string;
  /** Result column index of the right-clicked column. */
  column: number;
  sorted: RowOrder | null;
  pinned: boolean;
  copy: (format: CopyFormat) => void;
  copyText: (text: string) => void;
  /** Export the selected rows; omitted when the result cannot be exported. */
  exportRows?: (() => void) | undefined;
  sortBy: (order: RowOrder | null) => void;
  togglePin: () => void;
  fit: () => void;
  resetColumns: () => void;
  onClose: () => void;
};

function item(label: string, run: () => void, disabled = false): MenuEntry {
  return { kind: 'item', label, run, disabled };
}

function sortEntries(props: ResultMenuProps): MenuEntry[] {
  const { column, sorted, sortBy } = props;
  return [
    item('Sort ascending', () => {
      sortBy({ column, descending: false });
    }),
    item('Sort descending', () => {
      sortBy({ column, descending: true });
    }),
    item(
      'Clear sort',
      () => {
        sortBy(null);
      },
      sorted === null,
    ),
  ];
}

function columnEntries(props: ResultMenuProps): MenuEntry[] {
  return [
    item(props.pinned ? 'Unpin column' : 'Pin column', props.togglePin),
    item('Size column to fit', props.fit),
    item('Reset columns', props.resetColumns),
  ];
}

/** Menu rows: copy, `Copy as ▸`, sorting and, on headers, column layout. */
function resultMenuEntries(props: ResultMenuProps): MenuEntry[] {
  const header = props.kind === 'column';
  const copy = header
    ? item('Copy column name', () => {
        props.copyText(props.name);
      })
    : item('Copy', () => {
        props.copy('tsv');
      });
  const exportRows = props.exportRows;
  return [
    copy,
    copyAsMenu(props.copy),
    ...(exportRows && !header
      ? [item('Export selected rows…', exportRows)]
      : []),
    { kind: 'separator' },
    ...sortEntries(props),
    ...(header
      ? [{ kind: 'separator' as const }, ...columnEntries(props)]
      : []),
  ];
}

/** Right-click menu of a query result grid, like TablePlus's. */
export function ResultContextMenu(props: ResultMenuProps): React.JSX.Element {
  return (
    <CascadeMenu
      x={props.x}
      y={props.y}
      label="Result actions"
      entries={resultMenuEntries(props)}
      onClose={props.onClose}
    />
  );
}
