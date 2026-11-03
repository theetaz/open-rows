// SPDX-License-Identifier: Apache-2.0
import { Button, TextField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { PendingAttach } from './useAttachDatabase';
import styles from './AttachDatabase.module.css';
import { t } from './messages';

/**
 * Names the schema a chosen `SQLite` file is attached under; Return
 * attaches, Escape cancels. The file itself was chosen in the open panel.
 */
export function AttachDatabaseSheet(props: {
  pending: PendingAttach;
  onAlias: (alias: string) => void;
  onAttach: () => void;
  onCancel: () => void;
}): React.JSX.Element {
  const { pending } = props;
  return (
    <Sheet title={t('attachDatabase')} size="alert" onClose={props.onCancel}>
      <p className={styles.path} title={pending.path}>
        {pending.path}
      </p>
      <TextField
        label={t('schemaName')}
        value={pending.alias}
        spellCheck={false}
        {...(pending.error ? { error: pending.error } : {})}
        onChange={(event) => {
          props.onAlias(event.target.value);
        }}
      />
      <SheetActions>
        <Button onClick={props.onCancel}>{t('cancel')}</Button>
        <Button
          variant="primary"
          disabled={pending.busy || !pending.alias.trim()}
          onClick={props.onAttach}
        >
          {t('attach')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
