// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { useAuditHistory } from './useAuditHistory';
import type { AuditEntry } from '@openrows/ipc';
import styles from './Ai.module.css';
/** Bounded local audit metadata, with explicit purge and no payload-content option. */
export function AuditViewer(): React.JSX.Element {
  const { entries, failure, busy, refresh, purge } = useAuditHistory();
  const [confirm, setConfirm] = useState(false);
  const confirmPurge = async (): Promise<void> => {
    if (await purge()) setConfirm(false);
  };
  return (
    <section aria-label="AI audit log" className={styles.form}>
      <h4>Audit log</h4>
      <p className={styles.notice}>
        Last 1,000 outbound attempts. Only time, provider/connection IDs, level,
        size, hash and categories are recorded. Previews do not send requests or
        create audit records.
      </p>
      <div className={styles.actions}>
        <Button
          type="button"
          disabled={busy}
          onClick={() => {
            void refresh();
          }}
        >
          Refresh audit
        </Button>
        <Button
          type="button"
          disabled={busy || entries.length === 0}
          onClick={() => {
            setConfirm(true);
          }}
        >
          Purge audit…
        </Button>
      </div>
      {confirm && (
        <PurgeConfirmation
          busy={busy}
          onPurge={() => {
            void confirmPurge();
          }}
          onCancel={() => {
            setConfirm(false);
          }}
        />
      )}
      {failure && <p role="alert">{failure}</p>}
      <AuditTable entries={entries} />
      {entries.length === 0 && (
        <p>No outbound database context requests recorded.</p>
      )}
    </section>
  );
}

function AuditTable({ entries }: { entries: AuditEntry[] }): React.JSX.Element {
  return (
    <div className={styles.audit}>
      <table aria-label="Outbound requests">
        <thead>
          <tr>
            <th>Time</th>
            <th>Provider</th>
            <th>Connection</th>
            <th>Level</th>
            <th>Bytes</th>
            <th>Categories</th>
            <th>SHA-256</th>
          </tr>
        </thead>
        <tbody>
          {entries.map((entry, index) => (
            <tr key={`${entry.time}-${String(index)}`}>
              <td>{new Date(Number(entry.time)).toLocaleString()}</td>
              <td>{entry.provider}</td>
              <td>{entry.connection}</td>
              <td>{entry.level}</td>
              <td>{entry.bytes}</td>
              <td>{entry.categories.join(', ')}</td>
              <td>
                <code>{entry.hash}</code>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function PurgeConfirmation({
  busy,
  onPurge,
  onCancel,
}: {
  busy: boolean;
  onPurge: () => void;
  onCancel: () => void;
}): React.JSX.Element {
  return (
    <div role="group" aria-label="Confirm audit purge">
      <p>Delete all local AI request audit records?</p>
      <Button
        type="button"
        disabled={busy}
        onClick={() => {
          onPurge();
        }}
      >
        Delete audit records
      </Button>
      <Button
        type="button"
        onClick={() => {
          onCancel();
        }}
      >
        Cancel purge
      </Button>
    </div>
  );
}
