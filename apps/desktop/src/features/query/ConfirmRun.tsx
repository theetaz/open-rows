// SPDX-License-Identifier: Apache-2.0
import { Badge, Button } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import type { StatementDto } from '@openrows/ipc';
import styles from './Query.module.css';
import { rich, richn, t, tn } from './lazyMessages';

const kinds: Record<StatementDto['kind'], string> = {
  read: t('read'),
  write: t('write'),
  ddl: t('schemaChange'),
  txControl: t('transaction'),
  unknown: t('unrecognisedTreatedWrite'),
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
        {rich('connectionRunTotal', {
          connection: <strong>{props.connection}</strong>,
          total: String(props.total),
        })}
      </p>
    );
  return (
    <p>
      {others > 0
        ? richn('connectionTheseStatements', others, {
            connection: <strong>{props.connection}</strong>,
            count: String(others),
          })
        : rich('connectionTheseStatementsMay', {
            connection: <strong>{props.connection}</strong>,
          })}
    </p>
  );
}

/**
 * Explicit consent before writes on confirm-writes connections or dangerous
 * statements, and before multi-statement runs when Settings ▸ Execution asks.
 */
export function ConfirmRun(props: Props): React.JSX.Element {
  const confirmations = props.statements.filter(
    (statement) => statement.verdict === 'confirm',
  );
  const flagged = confirmations.length ? confirmations : props.statements;
  const writes = flagged.filter(
    (statement) => statement.kind !== 'read' && statement.kind !== 'txControl',
  );
  const dangerous = flagged.some((statement) => statement.dangerous);
  const title = writes.length
    ? t('runStatementsChange')
    : t('runCountStatements', { count: String(props.statements.length) });
  return (
    <Sheet title={title} onClose={props.onCancel}>
      <div className={styles.confirm}>
        <Lead
          connection={props.connection}
          flagged={writes.length}
          total={props.statements.length}
        />
        <ol aria-label={t('statementsConfirm')}>
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
                  {statement.dangerous ? t('dangerous') : kinds[statement.kind]}
                </Badge>
                <small>
                  {t('statementValue', { value: statement.index + 1 })}
                </small>
              </span>
              <pre>{statement.sql}</pre>
            </li>
          ))}
        </ol>
        <footer className={styles.confirmActions}>
          <Button onClick={props.onCancel}>{t('cancel')}</Button>
          <Button
            variant={dangerous ? 'destructive' : 'primary'}
            onClick={props.onConfirm}
          >
            {tn('runStatements', props.statements.length)}
          </Button>
        </footer>
      </div>
    </Sheet>
  );
}
