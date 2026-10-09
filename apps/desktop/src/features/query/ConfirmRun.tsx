// SPDX-License-Identifier: Apache-2.0
import { Badge, Button } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import type { StatementDto } from '@openrows/ipc';
import styles from './Query.module.css';

const kinds: Record<StatementDto['kind'], string> = {
  read: 'Read',
  write: 'Write',
  ddl: 'Schema change',
  txControl: 'Transaction',
  unknown: 'Unrecognised (treated as a write)',
};

type Props = {
  connection: string;
  statements: readonly StatementDto[];
  onConfirm: () => void;
  onCancel: () => void;
};

/** The lead paragraph: what may change, or how many statements run. */
function Lead(props: {
  connection: string;
  flagged: number;
  total: number;
}): React.JSX.Element {
  const others = props.total - props.flagged;
  if (!props.flagged)
    return (
      <p>
        On <strong>{props.connection}</strong>, this run has{' '}
        {String(props.total)} statements:
      </p>
    );
  return (
    <p>
      On <strong>{props.connection}</strong>, these statements may modify the
      database
      {others > 0
        ? ` (${String(others)} other statement${others === 1 ? '' : 's'} also run)`
        : ''}
      :
    </p>
  );
}

/**
 * Explicit consent before writes on confirm-writes connections or dangerous
 * statements, and before multi-statement runs when Settings ▸ Execution asks.
 */
export function ConfirmRun(props: Props): React.JSX.Element {
  const writes = props.statements.filter(
    (statement) => statement.verdict === 'confirm',
  );
  const flagged = writes.length ? writes : props.statements;
  const dangerous = flagged.some((statement) => statement.dangerous);
  const title = writes.length
    ? 'Run statements that change data?'
    : `Run ${String(props.statements.length)} statements?`;
  return (
    <Sheet title={title} onClose={props.onCancel}>
      <div className={styles.confirm}>
        <Lead
          connection={props.connection}
          flagged={writes.length}
          total={props.statements.length}
        />
        <ol aria-label="Statements to confirm">
          {flagged.map((statement) => (
            <li key={statement.index}>
              <span className={styles.confirmTags}>
                <Badge
                  tone={
                    statement.dangerous
                      ? 'danger'
                      : statement.verdict === 'confirm'
                        ? 'warning'
                        : 'neutral'
                  }
                >
                  {statement.dangerous ? 'Dangerous' : kinds[statement.kind]}
                </Badge>
                <small>Statement {statement.index + 1}</small>
              </span>
              <pre>{statement.sql}</pre>
            </li>
          ))}
        </ol>
        <footer className={styles.confirmActions}>
          <Button onClick={props.onCancel}>Cancel</Button>
          <Button
            variant={dangerous ? 'destructive' : 'primary'}
            onClick={props.onConfirm}
          >
            Run {props.statements.length === 1 ? 'statement' : 'statements'}
          </Button>
        </footer>
      </div>
    </Sheet>
  );
}
