// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { investigationAudit } from '@openrows/ipc';
import type { InvestigationAudit as Entry } from '@openrows/ipc';
/** Persistent accounting has no question, model response, SQL or payload content. */
export function InvestigationAudit(props: {
  connection: string;
}): React.JSX.Element {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [failure, setFailure] = useState('');
  return (
    <details>
      <summary>Retained investigation accounting</summary>
      <p>
        Newest 1,000 records. Purge through Settings → AI → Data sharing →
        Audit.
      </p>
      <Button
        onClick={() => {
          void investigationAudit().then(setEntries, () => {
            setFailure('Audit unavailable.');
          });
        }}
      >
        Load accounting
      </Button>
      <p role="alert">{failure}</p>
      <table>
        <caption>Scrubbed usage and outcomes</caption>
        <thead>
          <tr>
            <th>Provider/model</th>
            <th>Outcome</th>
            <th>Token ceiling</th>
            <th>Cost millionths</th>
            <th>Evidence ids</th>
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
