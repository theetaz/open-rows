// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { ReactNode } from 'react';
import styles from './Dump.module.css';
import { t } from './messages';

const PAGE = 80;

/** Render a bounded page of catalog choices; search and select-all still cover every object. */
export function ScopeList<T>(props: {
  rows: readonly T[];
  render: (row: T) => ReactNode;
  /** Tables flow in columns under their full-width schema rows. */
  columns?: boolean;
}): React.JSX.Element {
  const [page, setPage] = useState(0);
  const last = Math.max(0, Math.ceil(props.rows.length / PAGE) - 1);
  const current = Math.min(page, last);
  return (
    <>
      <div
        className={styles.scopeList}
        data-columns={props.columns ? '' : undefined}
      >
        {props.rows
          .slice(current * PAGE, (current + 1) * PAGE)
          .map(props.render)}
      </div>
      {last > 0 && (
        <div className={styles.buttons}>
          <Button
            size="sm"
            disabled={current === 0}
            onClick={() => {
              setPage(current - 1);
            }}
          >
            {t('previousObjects')}
          </Button>
          <span role="status">
            {t('pageValueValue2', {
              value: current + 1,
              value2: last + 1,
              count: props.rows.length,
            })}
          </span>
          <Button
            size="sm"
            disabled={current === last}
            onClick={() => {
              setPage(current + 1);
            }}
          >
            {t('nextObjects')}
          </Button>
        </div>
      )}
    </>
  );
}
