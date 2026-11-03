// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { useAuditHistory } from './useAuditHistory';
import type { AuditEntry } from '@openrows/ipc';
import styles from './Ai.module.css';
import { t } from '../messages/lazyAi';
/** Bounded local audit metadata, with explicit purge and no payload-content option. */
export function AuditViewer(): React.JSX.Element {
  const { entries, failure, busy, refresh, purge } = useAuditHistory();
  const [confirm, setConfirm] = useState(false);
  const confirmPurge = async (): Promise<void> => {
    if (await purge()) setConfirm(false);
  };
  return (
    <section aria-label={t('aiAuditLog')} className={styles.form}>
      <h4>{t('auditLog')}</h4>
      <p className={styles.notice}>{t('last1000')}</p>
      <div className={styles.actions}>
        <Button
          type="button"
          disabled={busy}
          onClick={() => {
            void refresh();
          }}
        >
          {t('refreshAudit')}
        </Button>
        <Button
          type="button"
          disabled={busy || entries.length === 0}
          onClick={() => {
            setConfirm(true);
          }}
        >
          {t('purgeAudit')}
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
      {entries.length === 0 && <p>{t('noOutboundDatabase')}</p>}
    </section>
  );
}

function AuditTable({ entries }: { entries: AuditEntry[] }): React.JSX.Element {
  return (
    <div className={styles.audit}>
      <table aria-label={t('outboundRequests')}>
        <thead>
          <tr>
            <th>{t('time')}</th>
            <th>{t('provider')}</th>
            <th>{t('connection')}</th>
            <th>{t('level')}</th>
            <th>{t('bytes')}</th>
            <th>{t('categories')}</th>
            <th>{t('sha256')}</th>
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
    <div role="group" aria-label={t('confirmAuditPurge')}>
      <p>{t('deleteAllLocal')}</p>
      <Button
        type="button"
        disabled={busy}
        onClick={() => {
          onPurge();
        }}
      >
        {t('deleteAuditRecords')}
      </Button>
      <Button
        type="button"
        onClick={() => {
          onCancel();
        }}
      >
        {t('cancelPurge')}
      </Button>
    </div>
  );
}
