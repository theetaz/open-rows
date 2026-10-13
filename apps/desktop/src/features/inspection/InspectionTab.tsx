// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type {
  AttributeDto,
  DetailSectionDto,
  ObjectDetailDto,
} from '@openrows/ipc';
import { recordRecent } from '../schema/sidebarStore';
import { SqlViewer } from '../table/structure/SqlViewer';
import { useInspection } from './useInspection';
import type { InspectionTarget } from './target';
import styles from './Inspection.module.css';

/** Lazy, read-only object metadata with optional definition export. */
export function InspectionTab(props: {
  profileId: string;
  database: string;
  target: InspectionTarget;
  connected: boolean;
  revision?: number | undefined;
  onOpenSql: (sql: string) => void;
}): React.JSX.Element {
  const [refresh, setReload] = useState(0);
  const resource = useInspection({
    ...props,
    revision: (props.revision ?? 0) + refresh,
  });
  return (
    <section className={styles.viewer} aria-label="Object inspection">
      <header className={styles.header}>
        <span>Read-only</span>
        <Button
          size="sm"
          variant="ghost"
          disabled={!props.connected}
          onClick={() => {
            setReload((count) => count + 1);
          }}
        >
          Reload
        </Button>
      </header>
      {!props.connected ? (
        <p role="status">Connect to inspect this object.</p>
      ) : resource.status === 'error' ? (
        <p role="alert">{resource.message}</p>
      ) : resource.status !== 'ready' ? (
        <p role="status">Loading object…</p>
      ) : (
        <Detail
          detail={resource.data}
          onOpenSql={(sql) => {
            if (props.target.kind === 'object')
              recordRecent({
                ...props.target.object,
                profileId: props.profileId,
                database: props.database,
              });
            props.onOpenSql(sql);
          }}
        />
      )}
    </section>
  );
}

function Detail(props: {
  detail: ObjectDetailDto;
  onOpenSql: (sql: string) => void;
}): React.JSX.Element {
  return (
    <>
      <Attributes attributes={props.detail.attributes} />
      {props.detail.sections.map((section) => (
        <DetailTable key={section.name} section={section} />
      ))}
      {props.detail.definition && (
        <Definition sql={props.detail.definition} onOpenSql={props.onOpenSql} />
      )}
    </>
  );
}

function Attributes(props: {
  attributes: AttributeDto[];
}): React.JSX.Element | null {
  if (!props.attributes.length) return null;
  return (
    <dl className={styles.attributes}>
      {props.attributes.map((attribute) => (
        <div key={attribute.name}>
          <dt>{attribute.name}</dt>
          <dd>{attribute.value ?? 'Unavailable'}</dd>
        </div>
      ))}
    </dl>
  );
}

function DetailTable(props: { section: DetailSectionDto }): React.JSX.Element {
  const { section } = props;
  return (
    <section className={styles.section}>
      <h2>{section.name}</h2>
      {section.rows.length ? (
        <table aria-label={section.name}>
          <thead>
            <tr>
              {section.columns.map((column) => (
                <th key={column} scope="col">
                  {column}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {section.rows.map((row, index) => (
              <tr key={index}>
                {section.columns.map((column, cell) => (
                  <td key={column}>{row[cell] ?? 'Unavailable'}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p>No {section.name.toLowerCase()}</p>
      )}
    </section>
  );
}

function Definition(props: {
  sql: string;
  onOpenSql: (sql: string) => void;
}): React.JSX.Element {
  const [error, setError] = useState<string | null>(null);
  const copy = (): void => {
    navigator.clipboard.writeText(props.sql).then(
      () => {
        setError(null);
      },
      (reason: unknown) => {
        setError(reason instanceof Error ? reason.message : String(reason));
      },
    );
  };
  return (
    <section className={styles.section}>
      <h2>Definition</h2>
      <div className={styles.actions}>
        <Button size="sm" variant="ghost" onClick={copy}>
          Copy DDL
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            props.onOpenSql(props.sql);
          }}
        >
          Open in Editor
        </Button>
      </div>
      {error && <p role="alert">{error}</p>}
      <SqlViewer sql={props.sql} label="Object definition SQL" />
    </section>
  );
}
