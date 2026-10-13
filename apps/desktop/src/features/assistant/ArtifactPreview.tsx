// SPDX-License-Identifier: Apache-2.0
import { useMemo, useState } from 'react';
import { Button } from '@openrows/ui';
import { importBoard } from '@openrows/ipc';
import { ReportDialog } from '../monitor/report/ReportDialog';
import { reportHtml } from '../monitor/report/html';
import styles from './Assistant.module.css';
import { parseReport } from '../monitor/report/validate';
import type { Report } from '../monitor/report/model';
/** Native validated JSON stays a preview until an explicit user action. */
export function ArtifactPreview(props: {
  connection: string;
  kind: string;
  document: string;
  disabled: boolean;
  evidence?: readonly string[];
}): React.JSX.Element {
  const [status, setStatus] = useState('');
  const [exporting, setExporting] = useState(false);
  const report = useMemo(
    () => (props.kind === 'report' ? parseReport(props.document) : null),
    [props.kind, props.document],
  );
  const build = useMemo(
    () =>
      (_range: string, parts: ReadonlySet<string>): Promise<Report> => {
        if (!report) return Promise.reject(new Error('Report unavailable'));
        return Promise.resolve({
          ...report,
          sections: report.sections.filter((section) => parts.has(section.id)),
        });
      },
    [report],
  );
  const save = (): void => {
    setStatus('Saving…');
    void importBoard(props.connection, props.document).then(
      () => {
        setStatus('Saved paused. Review and enable it in Monitor → Boards.');
      },
      () => {
        setStatus('Board could not be saved.');
      },
    );
  };
  return (
    <section aria-label={`${props.kind} proposal`}>
      <p>
        Validated {props.kind} preview · model interpretations require review
      </p>
      <details>
        <summary>Document and evidence references</summary>
        <p>Evidence ids: {props.evidence?.join(', ')}</p>
        <pre>{props.document}</pre>
      </details>
      {report && <ReportPreview report={report} />}
      {props.kind === 'board' && (
        <Button disabled={props.disabled || !!status} onClick={save}>
          Accept and save paused board
        </Button>
      )}
      {report && (
        <Button
          disabled={props.disabled}
          onClick={() => {
            setExporting(true);
          }}
        >
          Accept and export report…
        </Button>
      )}
      <p role="status">{status}</p>
      {exporting && report && (
        <ReportDialog
          parts={report.sections.map((section) => ({
            value: section.id,
            label: section.title,
          }))}
          build={build}
          onClose={() => {
            setExporting(false);
          }}
        />
      )}
    </section>
  );
}

function ReportPreview(props: { report: Report }): React.JSX.Element {
  const html = useMemo(() => reportHtml(props.report), [props.report]);
  return (
    <iframe
      title="Report and chart preview"
      className={styles.reportPreview}
      sandbox=""
      srcDoc={html}
    />
  );
}
