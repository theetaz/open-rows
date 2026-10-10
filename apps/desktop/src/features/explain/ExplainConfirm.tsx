// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import type { ExplainControls } from './useExplain';
import styles from './Explain.module.css';

/** Consent before Explain Analyze runs a statement that may modify data. */
function ExplainConfirm(props: {
  connection: string;
  statement: string;
  onConfirm: () => void;
  onCancel: () => void;
}): React.JSX.Element {
  return (
    <Sheet title="Run Explain Analyze on a write?" onClose={props.onCancel}>
      <div className={styles.confirm}>
        <p>
          Explain Analyze runs this statement on{' '}
          <strong>{props.connection}</strong> to measure it, inside a
          transaction that is rolled back afterwards. Effects outside the
          transaction, such as identity or sequence values, are not undone.
        </p>
        <pre aria-label="Statement to analyse">{props.statement}</pre>
        <footer>
          <Button onClick={props.onCancel}>Cancel</Button>
          <Button variant="primary" onClick={props.onConfirm}>
            Run and Roll Back
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
