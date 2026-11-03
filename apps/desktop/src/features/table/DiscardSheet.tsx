// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import styles from './Table.module.css';
import { t, tn } from './messages/lazyData';

/** What the confirmation is for: Discard Changes, or Reload with edits staged. */
export type DiscardKind = 'discard' | 'reload';

const changes = (count: number): string =>
  tn('stagedChanges', count, { count: count.toLocaleString('en-US') });

/**
 * Confirm dropping staged edits (FR-DATA-08). Discard removes edits that
 * were never sent and reloads the rows; it never rolls back SQL that was
 * already executed. Reload with edits staged can keep them staged.
 */
export function DiscardSheet({
  kind,
  count,
  onDiscard,
  onKeep,
  onClose,
}: {
  kind: DiscardKind;
  count: number;
  /** Drop the staged edits and reload. */
  onDiscard: () => void;
  /** Reload and keep the staged edits (Reload only). */
  onKeep: () => void;
  onClose: () => void;
}): React.JSX.Element {
  const reload = kind === 'reload';
  return (
    <Sheet
      title={
        reload ? t('reloadRows') : t('discardCount', { count: changes(count) })
      }
      size="alert"
      onClose={onClose}
    >
      <p className={styles.editNote}>
        {reload
          ? t('tableCountReloading', { count: changes(count) })
          : t('countRemovedRows', { count: changes(count) })}
      </p>
      <SheetActions>
        <Button onClick={onClose}>{t('cancel')}</Button>
        {reload && <Button onClick={onKeep}>{t('reloadKeepChanges')}</Button>}
        <Button variant="destructive" onClick={onDiscard}>
          {reload ? t('discardReload') : t('discardChanges')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
