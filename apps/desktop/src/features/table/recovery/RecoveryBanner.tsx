// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { recoveryNote } from './journal';
import type { EditRecovery } from './useEditRecovery';
import styles from '../Table.module.css';

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
  const blocked =
    offer.kind === 'edits' && !canRestore
      ? ' · cannot be restored: the table key changed'
      : '';
  return (
    <div role="status" className={styles.recovery} aria-label="Unsaved changes">
      <span className={styles.recoveryNote}>
        {note}
        {blocked}
      </span>
      <span className={styles.spacer} />
      {offer.kind === 'edits' && (
        <Button
          size="sm"
          variant="primary"
          disabled={!canRestore}
          onClick={recovery.restore}
        >
          Restore
        </Button>
      )}
      <Button size="sm" onClick={recovery.discard}>
        Discard
      </Button>
    </div>
  );
}
