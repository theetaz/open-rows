// SPDX-License-Identifier: Apache-2.0
import { Button, IconButton, SegmentedControl } from '@openrows/ui';
import { tableSegments } from './data/content';
import styles from './Content.module.css';
/** Table presentation controls and removable mock filter. */
export function TableToolbar({
  view,
  onView,
  filter,
  onFilter,
  onAdd,
}: {
  view: string;
  onView: (view: string) => void;
  filter: boolean;
  onFilter: (filter: boolean) => void;
  onAdd: () => void;
}): React.JSX.Element {
  return (
    <div className={styles.tableToolbar}>
      <SegmentedControl
        aria-label="Table view"
        options={tableSegments}
        value={view}
        onValueChange={onView}
      />
      {filter && (
        <span className={styles.filter}>
          <span className={styles.filterLabel} title="status = pending">
            status = pending
          </span>
          <IconButton
            size="sm"
            variant="ghost"
            aria-label="Remove filter"
            onClick={() => {
              onFilter(false);
            }}
          >
            ×
          </IconButton>
        </span>
      )}
      <Button
        size="sm"
        className={styles.addFilter}
        variant="ghost"
        onClick={() => {
          onFilter(true);
        }}
      >
        + Filter
      </Button>
      <Button size="sm" className={styles.addRow} onClick={onAdd}>
        + Row
      </Button>
    </div>
  );
}
