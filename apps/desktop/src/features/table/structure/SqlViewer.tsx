// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { SqlEditor } from '@openrows/editor';
import styles from './Structure.module.css';

/**
 * Read-only SQL with highlighting and line numbers (the lazily loaded
 * `@openrows/editor`); plain text shows until the editor has mounted.
 */
export function SqlViewer({
  sql,
  label,
}: {
  sql: string;
  label: string;
}): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null);
  const [loaded, setLoaded] = useState(false);
  useEffect(() => {
    let cancelled = false;
    let editor: SqlEditor | undefined;
    void import('@openrows/editor').then(({ mountSqlEditor }) => {
      const parent = host.current;
      if (cancelled || !parent) return;
      editor = mountSqlEditor(parent, {
        doc: sql,
        label,
        readOnly: true,
      });
      setLoaded(true);
    });
    return () => {
      cancelled = true;
      editor?.destroy();
    };
  }, [sql, label]);
  return (
    <div className={styles.code} data-loaded={loaded}>
      <div ref={host} />
      <pre aria-label={label}>{sql}</pre>
    </div>
  );
}
