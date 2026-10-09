// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { tableSql } from '@openrows/ipc';
import type { TableQueryDto } from '@openrows/ipc';
import { whereClause, withParams } from './whereClause';
import styles from '../TableExtras.module.css';

/** Pause after typing before the predicate is checked again. */
const CHECK_DELAY_MS = 300;

type Check =
  | { status: 'checking' }
  | { status: 'valid'; text: string }
  | { status: 'invalid'; message: string };

/**
 * Live check of a draft with a raw SQL predicate (FR-GRID-11): the core
 * validates it and generates the parameterised SELECT, whose WHERE clause
 * is shown; a refused predicate shows why.
 */
export function WherePreview({
  draft,
}: {
  draft: TableQueryDto;
}): React.JSX.Element {
  const [check, setCheck] = useState<Check>({ status: 'checking' });
  const key = JSON.stringify({ ...draft, sort: null });
  useEffect(() => {
    let live = true;
    const request = JSON.parse(key) as TableQueryDto;
    const timer = window.setTimeout(() => {
      tableSql(request, 0, 1).then(
        (reply) => {
          if (live)
            setCheck({
              status: 'valid',
              text: withParams(whereClause(reply.sql) ?? '', reply.params),
            });
        },
        (error: unknown) => {
          if (live)
            setCheck({
              status: 'invalid',
              message: error instanceof Error ? error.message : String(error),
            });
        },
      );
    }, CHECK_DELAY_MS);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [key]);
  if (check.status === 'invalid')
    return (
      <p role="alert" className={styles.invalid}>
        {check.message}
      </p>
    );
  return (
    <pre
      className={styles.where}
      aria-label="Generated WHERE"
      aria-busy={check.status === 'checking'}
    >
      {check.status === 'valid' ? check.text : 'Checking…'}
    </pre>
  );
}
