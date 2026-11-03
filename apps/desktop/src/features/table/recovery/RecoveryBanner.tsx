// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { recoveryNote } from './journal';
import type { EditRecovery } from './useEditRecovery';
import styles from '../Table.module.css';
import { t } from '../messages/lazyData';

/** "N unsaved changes from <time> — Restore · Discard" above the rows. */
export function RecoveryBanner({
  recovery,
}: {
  recovery: EditRecovery;
}): React.JSX.Element | null {
  const { offer, canRestore } = recovery;
  if (!offer) return null;
  const note =
    offer.kind === 'error'
      ? offer.message
      : recoveryNote(offer.edits.length, offer.saved.savedAtMs);
  return (
    <div
      role="status"
      className={styles.recovery}
      aria-label={t('unsavedChanges')}
    >
      <span className={styles.recoveryNote}>
        {offer.kind === 'edits' && !canRestore
          ? t('noteCannotRestore', { note })
          : note}
      </span>
      <span className={styles.spacer} />
      {offer.kind === 'edits' && (
        <Button
          size="sm"
          variant="primary"
          disabled={!canRestore}
          onClick={recovery.restore}
        >
          {t('restore')}
        </Button>
      )}
      <Button size="sm" onClick={recovery.discard}>
        {t('discard')}
      </Button>
    </div>
  );
}
