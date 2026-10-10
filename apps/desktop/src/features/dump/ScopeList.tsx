// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { ReactNode } from 'react';
import styles from './Dump.module.css';

const PAGE = 80;

/** Render a bounded page of catalog choices; search and select-all still cover every object. */
export function ScopeList<T>(props: {
  rows: readonly T[];
  render: (row: T) => ReactNode;
}): React.JSX.Element {
  const [page, setPage] = useState(0);
  const last = Math.max(0, Math.ceil(props.rows.length / PAGE) - 1);
  const current = Math.min(page, last);
  return (
    <>
      <div className={styles.scopeList}>
        {props.rows
          .slice(current * PAGE, (current + 1) * PAGE)
          .map(props.render)}
      </div>
      {last > 0 && (
        <div className={styles.row}>
          <Button
            size="sm"
            disabled={current === 0}
            onClick={() => {
              setPage(current - 1);
            }}
          >
            Previous objects
          </Button>
          <span role="status">
            Page {current + 1} of {last + 1} · {props.rows.length} objects
          </span>
          <Button
            size="sm"
            disabled={current === last}
            onClick={() => {
              setPage(current + 1);
            }}
          >
            Next objects
          </Button>
        </div>
      )}
    </>
  );
}
