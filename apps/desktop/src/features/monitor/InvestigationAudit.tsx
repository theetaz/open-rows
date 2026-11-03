// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { investigationAudit } from '@openrows/ipc';
import type { InvestigationAudit as Entry } from '@openrows/ipc';
import { t } from './messages/lazyInvestigation';
/** Persistent accounting has no question, model response, SQL or payload content. */
export function InvestigationAudit(props: {
  connection: string;
}): React.JSX.Element {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [failure, setFailure] = useState('');
  return (
    <details>
      <summary>{t('retainedInvestigationAccounting')}</summary>
      <p>{t('newest1000')}</p>
      <Button
        onClick={() => {
          void investigationAudit().then(setEntries, () => {
            setFailure(t('auditUnavailable'));
          });
        }}
      >
        {t('loadAccounting')}
      </Button>
      <p role="alert">{failure}</p>
      <table>
        <caption>{t('scrubbedUsageOutcomes')}</caption>
        <thead>
          <tr>
            <th>{t('providerModel')}</th>
            <th>{t('outcome')}</th>
            <th>{t('tokenCeiling')}</th>
            <th>{t('costMillionths')}</th>
            <th>{t('evidenceIds')}</th>
          </tr>
        </thead>
        <tbody>
          {entries
            .filter((entry) => entry.connection === props.connection)
            .map((entry, index) => (
              <tr key={index}>
                <td>
                  {entry.provider} / {entry.model}
                </td>
                <td>{entry.outcome}</td>
                <td>{entry.tokens}</td>
                <td>{entry.costMicros}</td>
                <td>{entry.evidence.join(', ')}</td>
              </tr>
            ))}
        </tbody>
      </table>
    </details>
  );
}
