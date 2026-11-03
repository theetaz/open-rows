// SPDX-License-Identifier: Apache-2.0
import { StatusDot } from '@openrows/ui';
import type { StepStatus, TestReport, TestStepDto } from '@openrows/ipc';
import { serverLabel } from './sessions';
import { CertificateDetails, LatencyBadge } from './TestDetails';
import type { TestState } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/profile';

const tones = {
  ok: 'success',
  warning: 'warning',
  failed: 'danger',
} as const satisfies Record<StepStatus, string>;

const statusText: Record<StepStatus, string> = {
  ok: 'passed',
  warning: t('passedWarning'),
  failed: 'failed',
};

/**
 * Test connection outcome: a summary line with the latency, the
 * step-by-step diagnostics and the server certificate.
 */
export function TestResult({ state }: { state: TestState }): React.JSX.Element {
  switch (state.status) {
    case 'idle':
      return <div role="status" className={styles.test} />;
    case 'testing':
      return (
        <div role="status" className={styles.test}>
          <StatusDot tone="info" label={t('testingConnection')} />
        </div>
      );
    case 'done':
      return <Report report={state.report} />;
    case 'error':
      return (
        <div role="alert" className={styles.test}>
          <StatusDot tone="danger" label={t('connectionFailed')} />
          <p className={styles.error}>{state.message}</p>
        </div>
      );
  }
}

function Report({ report }: { report: TestReport }): React.JSX.Element {
  const elapsed = `${String(report.elapsedMs)} ms`;
  return (
    <div role={report.server ? 'status' : 'alert'} className={styles.test}>
      {report.server ? (
        <span className={styles.stepLine}>
          <StatusDot
            tone="success"
            label={t('connectedServerElapsed', {
              server: serverLabel(report.server),
              elapsed,
            })}
          />
          <LatencyBadge latency={report.latency} />
        </span>
      ) : (
        <StatusDot
          tone="danger"
          label={t('connectionFailedElapsed', { elapsed })}
        />
      )}
      <ol className={styles.steps} aria-label={t('connectionSteps')}>
        {report.steps.map((step, index) => (
          <Step key={`${step.kind}-${String(index)}`} step={step} />
        ))}
      </ol>
      {report.certificate && (
        <CertificateDetails certificate={report.certificate} />
      )}
      {report.error &&
        report.steps.every((step) => step.status !== 'failed') && (
          <p className={styles.error}>{report.error.message}</p>
        )}
    </div>
  );
}

function Step({ step }: { step: TestStepDto }): React.JSX.Element {
  return (
    <li className={styles.step} data-status={step.status}>
      <span className={styles.stepLine}>
        <StatusDot tone={tones[step.status]} label={step.label} />
        <span className={styles.visuallyHidden}>{statusText[step.status]}</span>
        {step.elapsedMs !== null && (
          <span className={styles.stepTime}>
            {t('elapsedmsMs', { elapsedMs: String(step.elapsedMs) })}
          </span>
        )}
      </span>
      {step.detail && (
        <span
          className={
            step.status === 'failed' ? styles.error : styles.stepDetail
          }
        >
          {step.detail}
        </span>
      )}
      {step.hint && <span className={styles.stepDetail}>{step.hint}</span>}
    </li>
  );
}
