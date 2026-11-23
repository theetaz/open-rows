// SPDX-License-Identifier: Apache-2.0
import type { StepStatus, TestReport, TestStepDto } from '@openrows/ipc';
import { serverLabel } from '../sessions';
import { CertificateDetails, LatencyBadge } from '../TestDetails';
import { t as profile } from '../messages/profile';
import { FlowIcon } from './FlowIcon';
import type { FlowIconName } from './FlowIcon';
import styles from './Test.module.css';
import { t } from './messages';

const ICONS: Record<StepStatus, FlowIconName> = {
  ok: 'checkCircle',
  warning: 'warning',
  failed: 'xCircle',
};
const STATUS_TEXT: Record<StepStatus, string> = {
  ok: t('passed'),
  warning: t('passedWarning'),
  failed: t('failed'),
};

/**
 * The diagnostics of one test run: each hop with its real timing, revealed
 * one after another (CSS delays), then the result banner and certificate.
 */
export function TestReportView({
  report,
}: {
  report: TestReport;
}): React.JSX.Element {
  const count = report.steps.length;
  const style = { '--count': String(count) } as React.CSSProperties;
  return (
    <div
      role={report.server ? 'status' : 'alert'}
      className={styles.report}
      style={style}
    >
      <span
        className={styles.bar}
        data-state={report.server ? 'ok' : 'failed'}
        aria-hidden="true"
      >
        <i />
      </span>
      <ol className={styles.steps} aria-label={t('connectionStepsList')}>
        {report.steps.map((step, index) => (
          <Step
            key={`${step.kind}-${String(index)}`}
            step={step}
            index={index}
          />
        ))}
      </ol>
      <ResultBanner report={report} />
      {report.certificate && (
        <CertificateDetails certificate={report.certificate} />
      )}
    </div>
  );
}

function Step(props: { step: TestStepDto; index: number }): React.JSX.Element {
  const { step } = props;
  const style = { '--i': String(props.index) } as React.CSSProperties;
  return (
    <li className={styles.step} data-status={step.status} style={style}>
      <span className={styles.indicator}>
        <FlowIcon name={ICONS[step.status]} />
      </span>
      <span className={styles.lines}>
        <span>
          {step.label}
          <span className={styles.visuallyHidden}>
            {` ${STATUS_TEXT[step.status]}`}
          </span>
        </span>
        {step.detail && <span className={styles.detail}>{step.detail}</span>}
        {step.hint && <span className={styles.sub}>{step.hint}</span>}
      </span>
      <span className={styles.ms}>
        {step.elapsedMs !== null && t('stepMs', { ms: String(step.elapsedMs) })}
      </span>
    </li>
  );
}

function ResultBanner({ report }: { report: TestReport }): React.JSX.Element {
  const elapsed = profile('elapsedmsMs', {
    elapsedMs: String(report.elapsedMs),
  });
  const failure =
    report.error && report.steps.every((step) => step.status !== 'failed')
      ? report.error.message
      : null;
  return (
    <div className={styles.banner} data-state={report.server ? 'ok' : 'failed'}>
      <FlowIcon name={report.server ? 'checkCircle' : 'xCircle'} />
      <span className={styles.lines}>
        <span>
          {report.server
            ? profile('connectedServerElapsed', {
                server: serverLabel(report.server),
                elapsed,
              })
            : profile('connectionFailedElapsed', { elapsed })}
        </span>
        {failure && <span className={styles.sub}>{failure}</span>}
      </span>
      <LatencyBadge latency={report.latency} />
    </div>
  );
}
