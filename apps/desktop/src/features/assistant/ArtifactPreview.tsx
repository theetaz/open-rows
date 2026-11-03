// SPDX-License-Identifier: Apache-2.0
import { useMemo, useState } from 'react';
import { Button } from '@openrows/ui';
import { importBoard } from '@openrows/ipc';
import type { Rejection } from '@openrows/ipc';
import { ReportDialog } from '../monitor/report/ReportDialog';
import { reportHtml } from '../monitor/report/html';
import styles from './Assistant.module.css';
import { parseReport } from '../monitor/report/validate';
import type { Report } from '../monitor/report/model';
import { rich, t } from './lazyMessages';
type Decision = 'pending' | 'accepted' | 'rejected';
/** Native validated JSON stays a preview until an explicit user action. */
export function ArtifactPreview(props: {
  connection: string;
  kind: string;
  document: string;
  disabled: boolean;
  evidence?: readonly string[];
}): React.JSX.Element {
  const [decision, setDecision] = useState<Decision>('pending');
  const [status, setStatus] = useState('');
  const [exporting, setExporting] = useState(false);
  const report = useMemo(
    () => (props.kind === 'report' ? parseReport(props.document) : null),
    [props.kind, props.document],
  );
  const savable = props.kind === 'board' || !!report;
  const accept = (): void => {
    setDecision('accepted');
    if (report) {
      setExporting(true);
      return;
    }
    setStatus(t('saving'));
    void importBoard(props.connection, props.document).then(
      () => {
        setStatus(t('savedPausedReview'));
      },
      () => {
        setDecision('pending');
        setStatus(t('boardCouldNot'));
      },
    );
  };
  const label = proposalLabel(props.kind);
  if (decision === 'rejected')
    return (
      <section aria-label={label} className={styles.proposal}>
        <p role="status">{t('proposalRejectedNothing')}</p>
      </section>
    );
  return (
    <section aria-label={label} className={styles.proposal}>
      <p className={styles.question}>
        {props.kind === 'finding'
          ? t('kindValidatedAgainst', { kind: title(props.kind) })
          : t('kindValidatedAgainstDelivered', { kind: title(props.kind) })}
      </p>
      {props.kind === 'finding' && <FindingView document={props.document} />}
      <details>
        <summary>{t('documentEvidenceReferences')}</summary>
        <p>
          {rich('evidenceIdsEvidence', {
            evidence: props.evidence?.join(', '),
          })}
        </p>
        <pre>{props.document}</pre>
      </details>
      {report && <ReportPreview report={report} />}
      {savable && (
        <Decision
          label={report ? t('acceptExportReport') : t('acceptSavePaused')}
          disabled={props.disabled || decision !== 'pending'}
          accept={accept}
          reject={() => {
            setDecision('rejected');
          }}
        />
      )}
      <p role="status">{status}</p>
      {exporting && report && (
        <ReportExport
          report={report}
          onClose={() => {
            setExporting(false);
            setDecision('pending');
          }}
        />
      )}
    </section>
  );
}
function Decision(props: {
  label: string;
  disabled: boolean;
  accept: () => void;
  reject: () => void;
}): React.JSX.Element {
  return (
    <div className={styles.row}>
      <Button size="sm" disabled={props.disabled} onClick={props.accept}>
        {props.label}
      </Button>
      <Button size="sm" disabled={props.disabled} onClick={props.reject}>
        {t('rejectProposal')}
      </Button>
    </div>
  );
}
const REASONS: Record<Rejection, string> = {
  invalidDocument: t('doesNotMatch'),
  wrongKind: t('kindDoesNot'),
  citationCount: t('mustCiteBetween'),
  unknownEvidence: t('citesEvidenceWas'),
  uncitedFinding: t('findingCitesEvidence'),
  unsafeSql: t('boardWidgetS'),
  invalidText: t('titleInterpretationEmpty'),
  tooLarge: t('exceedsDocumentFinding'),
};
/** A refused proposal; nothing was shown for acceptance. */
export function ArtifactRejected(props: {
  kind: string;
  reason: Rejection;
}): React.JSX.Element {
  return (
    <p role="alert" className={styles.proposal}>
      {t('kindRejectedReasons', {
        kind: title(props.kind),
        reason: REASONS[props.reason],
      })}
    </p>
  );
}
function proposalLabel(kind: string): string {
  if (kind === 'board') return t('boardProposalLabel');
  if (kind === 'report') return t('reportProposalLabel');
  return t('findingProposalLabel');
}
function title(kind: string): string {
  if (kind === 'board') return t('boardProposal');
  if (kind === 'report') return t('reportProposal');
  return t('healthFinding');
}
function FindingView(props: { document: string }): React.JSX.Element | null {
  const finding = useMemo(() => parseFinding(props.document), [props.document]);
  if (!finding) return null;
  return (
    <dl className={styles.finding}>
      <dt>{t('interpretation')}</dt>
      <dd>{finding.interpretation}</dd>
      <dt>{t('uncertainty')}</dt>
      <dd>{finding.uncertainty}</dd>
      <dt>{t('cites')}</dt>
      <dd>
        {finding.evidence.map((id) => (
          <code key={id} title={id} className={styles.evidence}>
            {id.slice(0, 12)}
          </code>
        ))}
      </dd>
    </dl>
  );
}
type FindingDocument = {
  interpretation: string;
  uncertainty: string;
  evidence: string[];
};
function parseFinding(document: string): FindingDocument | null {
  try {
    const value = JSON.parse(document) as Partial<FindingDocument>;
    return typeof value.interpretation === 'string' &&
      typeof value.uncertainty === 'string' &&
      Array.isArray(value.evidence)
      ? (value as FindingDocument)
      : null;
  } catch {
    return null;
  }
}
function ReportExport(props: {
  report: Report;
  onClose: () => void;
}): React.JSX.Element {
  const { report } = props;
  const build = (_range: string, parts: ReadonlySet<string>): Promise<Report> =>
    Promise.resolve({
      ...report,
      sections: report.sections.filter((section) => parts.has(section.id)),
    });
  return (
    <ReportDialog
      parts={report.sections.map((section) => ({
        value: section.id,
        label: section.title,
      }))}
      build={build}
      onClose={props.onClose}
    />
  );
}
function ReportPreview(props: { report: Report }): React.JSX.Element {
  const html = useMemo(() => reportHtml(props.report), [props.report]);
  return (
    <iframe
      title={t('reportChartPreview')}
      className={styles.reportPreview}
      sandbox=""
      srcDoc={html}
    />
  );
}
