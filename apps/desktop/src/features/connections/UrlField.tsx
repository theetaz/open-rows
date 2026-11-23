// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import { Icon, IconButton, TextField } from '@openrows/ui';
import type { EngineDto } from '@openrows/ipc';
import { isFileEngine } from './engines';
import { fieldError } from './form';
import type { Draft } from './form';
import { PASSWORD_MASK } from './urlText';
import type { UrlField as UrlState } from './useUrlField';
import { FlowIcon } from './flow/FlowIcon';
import styles from './flow/Flow.module.css';
import { t } from './messages/profile';
import { t as flow } from './flow/messages';

/** An example URL for the engine. */
const EXAMPLES: Record<EngineDto, string> = {
  duckdb: 'duckdb:///path/to/database.duckdb',
  postgres: 'postgres://user:password@host:5432/database',
  mysql: 'mysql://user:password@host:3306/database',
  mongodb: 'mongodb://user:password@host:27017/database',
  mariadb: 'mariadb://user:password@host:3306/database',
  sqlserver:
    'sqlserver://user:password@host:1433/database?encrypt=true&trustServerCertificate=false',
  sqlite: 'sqlite:/path/to/database.sqlite',
  clickhouse: 'clickhouse://user:password@host:8123/database',
  libsql: 'libsql://database.turso.io',
  'libsql-file': 'libsql-file:/path/to/database.db',
};

/** The URL with a password typed in this form shown in place of the dots. */
function unmasked(value: string, draft: Draft): string {
  if (draft.passwordMode !== 'set' || !draft.password) return value;
  return value.replace(
    `:${PASSWORD_MASK}@`,
    `:${encodeURIComponent(draft.password)}@`,
  );
}

/**
 * Connection URL in sync with the fields: typing fills them after a short
 * pause, a paste fills them at once, and the password shows only as dots
 * until the eye button reveals a password typed in this form.
 */
export function UrlField({
  url,
  draft,
}: {
  url: UrlState;
  draft: Draft;
}): React.JSX.Element {
  const pasted = useRef(false);
  const [revealed, setRevealed] = useState(false);
  const revealable = draft.passwordMode === 'set' && draft.password !== '';
  const shown = revealed && revealable;
  return (
    <div className={styles.urlRow}>
      <TextField
        className={styles.urlField}
        aria-label={t('url')}
        placeholder={EXAMPLES[draft.engine]}
        autoComplete="off"
        spellCheck={false}
        value={shown ? unmasked(url.value, draft) : url.value}
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
          event.stopPropagation();
          url.flush();
        }}
        trailing={
          !isFileEngine(draft.engine) && (
            <IconButton
              size="sm"
              variant="ghost"
              aria-label={
                shown ? flow('hideUrlPassword') : flow('showUrlPassword')
              }
              aria-pressed={shown}
              disabled={!revealable}
              onClick={() => {
                setRevealed(!shown);
              }}
            >
              <Icon name={shown ? 'eyeOff' : 'eye'} />
            </IconButton>
          )
        }
      />
      <SyncState url={url} />
    </div>
  );
}

function SyncState({ url }: { url: UrlState }): React.JSX.Element {
  const bad = url.error !== null;
  return (
    <span className={styles.sync} data-state={bad ? 'bad' : 'ok'}>
      {!url.pending && (
        <>
          <FlowIcon name={bad ? 'warning' : 'check'} />
          {bad ? flow('notValidYet') : flow('inSync')}
        </>
      )}
    </span>
  );
}
