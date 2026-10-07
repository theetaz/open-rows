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

/** Explicit consent before writes on confirm-writes connections or dangerous statements. */
export function ConfirmRun(props: Props): React.JSX.Element {
  const flagged = props.statements.filter(
    (statement) => statement.verdict === 'confirm',
  );
  const others = props.statements.length - flagged.length;
  const dangerous = flagged.some((statement) => statement.dangerous);
  return (
    <Sheet title="Run statements that change data?" onClose={props.onCancel}>
      <div className={styles.confirm}>
        <p>
          On <strong>{props.connection}</strong>, these statements may modify
          the database
          {others > 0
            ? ` (${String(others)} other statement${others === 1 ? '' : 's'} also run)`
            : ''}
          :
        </p>
        <ol aria-label="Statements to confirm">
          {flagged.map((statement) => (
            <li key={statement.index}>
              <span className={styles.confirmTags}>
                <Badge tone={statement.dangerous ? 'danger' : 'warning'}>
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
