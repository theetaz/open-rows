// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import { TextField } from '@openrows/ui';
import type { EngineDto } from '@openrows/ipc';
import { fieldError } from './form';
import type { UrlField as UrlState } from './useUrlField';
import styles from './ConnectionSheet.module.css';

/**
 * Connection URL in sync with the fields: typing fills them after a short
 * pause, a paste fills them at once, and any password shows only as dots.
 */
/** An example URL for the engine. */
const EXAMPLES: Record<EngineDto, string> = {
  postgres: 'postgres://user:password@host:5432/database',
  mysql: 'mysql://user:password@host:3306/database',
  mariadb: 'mariadb://user:password@host:3306/database',
  sqlserver:
    'sqlserver://user:password@host:1433/database?encrypt=true&trustServerCertificate=false',
  sqlite: 'sqlite:/path/to/database.sqlite',
  clickhouse: 'clickhouse://user:password@host:8123/database',
  libsql: 'libsql://database.turso.io',
  'libsql-file': 'libsql-file:/path/to/database.db',
};

export function UrlField({
  url,
  engine,
}: {
  url: UrlState;
  engine: EngineDto;
}): React.JSX.Element {
  const pasted = useRef(false);
  return (
    <TextField
      className={styles.row}
      label="URL"
      placeholder={EXAMPLES[engine]}
      autoComplete="off"
      spellCheck={false}
      value={url.value}
      {...fieldError(url.error)}
      onPaste={() => {
        pasted.current = true;
      }}
      onChange={(event) => {
        url.edit(event.target.value, pasted.current);
        pasted.current = false;
      }}
      onKeyDown={(event) => {
        if (event.key !== 'Enter' || !url.pending) return;
        event.preventDefault();
        url.flush();
      }}
    />
  );
}
