// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { Button, Icon } from '@openrows/ui';
import type { ConnectionForm, TestState } from '../useConnectionForm';
import { FlowIcon } from './FlowIcon';
import { TestReportView } from './TestReportView';
import page from './Flow.module.css';
import styles from './Test.module.css';
import { t } from './messages';

/**
 * Step 5: run the existing diagnostics (every hop, real timings) and save.
 * A pasted URL arrives here and starts the test on its own.
 */
export function TestStep(props: {
  form: ConnectionForm;
  autoTest: boolean;
}): React.JSX.Element {
  const { form } = props;
  useAutoTest(props.autoTest, form);
  const keychain =
    form.draft.passwordStorage === 'keychain' || form.draft.ssh.enabled;
  return (
    <>
      <h2 className={page.title}>{t('testTitle')}</h2>
      <p className={page.lede}>{t('testLede')}</p>
      <section className={styles.card} aria-label={t('connectionTest')}>
        <header className={styles.header}>
          <b>{t('connectionTest')}</b>
          <Button
            size="sm"
            disabled={form.test.status === 'testing'}
            onClick={() => void form.runTest()}
          >
            <Icon name="refresh" />
            {runLabel(form.test)}
          </Button>
        </header>
        <TestBody state={form.test} />
      </section>
      <p className={page.notice}>
        {keychain ? t('storeNoteKeychain') : t('storeNoteOther')}
      </p>
    </>
  );
}

function runLabel(state: TestState): string {
  if (state.status === 'testing') return t('testing');
  return state.status === 'idle' ? t('runTest') : t('runAgain');
}

/** Run once when the step opens from a pasted URL (after its fields render). */
function useAutoTest(enabled: boolean, form: ConnectionForm): void {
  const started = useRef(false);
  const run = form.runTest;
  useEffect(() => {
    if (!enabled || started.current) return;
    started.current = true;
    void run();
  }, [enabled, run]);
}

function TestBody({ state }: { state: TestState }): React.JSX.Element {
  switch (state.status) {
    case 'idle':
      return <p className={styles.idle}>{t('notRun')}</p>;
    case 'testing':
      return (
        <div role="status" className={styles.report}>
          <span className={styles.bar} data-state="running" aria-hidden="true">
            <i />
          </span>
          <p className={styles.running}>
            <FlowIcon name="spinner" className={styles.spinner} />
            {t('testingConnection')}
          </p>
        </div>
      );
    case 'done':
      return <TestReportView report={state.report} />;
    case 'error':
      return (
        <div role="alert" className={styles.banner} data-state="failed">
          <FlowIcon name="xCircle" />
          <span>{state.message}</span>
        </div>
      );
  }
}
