// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, IconButton } from '@openrows/ui';
import type { HistoryItemDto } from '@openrows/ipc';
import { summaryOf } from './historyView';
import styles from './HistoryList.module.css';
import { t } from './initialMessages';
/** Reuse actions available for each history statement. */
export type HistoryActions = {
  onInsert: (sql: string) => void;
  onSave: (sql: string) => void;
};

/** A history statement with insert, save, copy, open and delete actions. */
export function HistoryRow(
  props: HistoryActions & {
    item: HistoryItemDto;
    /** Connection name, shown when the list spans every connection. */
    connection: string | undefined;
    onOpen: (sql: string) => void;
    onDelete: (id: number) => Promise<void>;
  },
): React.JSX.Element {
  const { item } = props;
  const { id } = item;
  const label = snippet(item.statement);
  const meta = [props.connection, item.database, ...summaryOf(item)];
  return (
    <li className={styles.row} data-status={item.status}>
      <button
        type="button"
        className={styles.open}
        title={item.errorMessage ?? item.statement}
        onClick={() => {
          props.onOpen(item.statement);
        }}
      >
        <Icon name={item.status === 'error' ? 'warning' : 'clock'} />
        <span className={styles.text}>
          <code>{label}</code>
          <small>{meta.filter(Boolean).join(' · ')}</small>
        </span>
      </button>
      <span className={styles.actions}>
        <Button
          size="sm"
          variant="ghost"
          aria-label={t('insertLabelCursor', { label })}
          onClick={() => {
            props.onInsert(item.statement);
          }}
        >
          {t('insert')}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          aria-label={t('saveLabelQuery', { label })}
          onClick={() => {
            props.onSave(item.statement);
          }}
        >
          {t('save')}
        </Button>
        <IconButton
          size="sm"
          variant="ghost"
          aria-label={t('copyLabel', { label })}
          onClick={() => {
            void navigator.clipboard
              .writeText(item.statement)
              .catch(() => undefined);
          }}
        >
          <Icon name="copy" />
        </IconButton>
        {id !== null && (
          <IconButton
            size="sm"
            variant="ghost"
            aria-label={t('deleteLabel', { label })}
            onClick={() => {
              void props.onDelete(id);
            }}
          >
            <Icon name="close" />
          </IconButton>
        )}
      </span>
    </li>
  );
}

/** The statement on one line, cut for labels. */
function snippet(statement: string): string {
  const flat = statement.replace(/\s+/g, ' ').trim();
  return flat.length > 60 ? `${flat.slice(0, 59)}…` : flat;
}
