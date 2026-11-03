// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import type { ExplainControls } from './useExplain';
import styles from './Explain.module.css';
import { rich, t } from './messages';

/** Consent before Explain Analyze executes the reviewed statement. */
function ExplainConfirm(props: {
  connection: string;
  statement: string;
  onConfirm: () => void;
  onCancel: () => void;
}): React.JSX.Element {
  return (
    <Sheet title={t('runExplainAnalyze')} onClose={props.onCancel}>
      <div className={styles.confirm}>
        <p>
          {rich('explainAnalyzeRuns', {
            connection: <strong>{props.connection}</strong>,
          })}
        </p>
        <pre aria-label={t('statementAnalyse')}>{props.statement}</pre>
        <footer>
          <Button onClick={props.onCancel}>{t('cancel')}</Button>
          <Button variant="primary" onClick={props.onConfirm}>
            {t('runRollBack')}
          </Button>
        </footer>
      </div>
    </Sheet>
  );
}

/** The tab's Explain Analyze consent sheet while one waits for it. */
export function ExplainSheet(props: {
  explain: ExplainControls;
  connection: string;
}): React.JSX.Element | null {
  const { explain } = props;
  if (explain.state?.status !== 'confirm') return null;
  return (
    <ExplainConfirm
      connection={props.connection}
      statement={explain.state.statement}
      onConfirm={explain.confirm}
      onCancel={explain.close}
    />
  );
}
