// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { TriggerDto } from '@openrows/ipc';
import { SqlViewer } from './SqlViewer';
import styles from './Structure.module.css';

const timing: Record<TriggerDto['timing'], string> = {
  before: 'BEFORE',
  after: 'AFTER',
  insteadOf: 'INSTEAD OF',
};

/** `BEFORE INSERT OR UPDATE · FOR EACH ROW · public.touch() · enabled`. */
function triggerSummary(trigger: TriggerDto): string {
  return [
    `${timing[trigger.timing]} ${trigger.events.join(' OR ')}`,
    trigger.rowLevel ? 'FOR EACH ROW' : 'FOR EACH STATEMENT',
    trigger.function,
    trigger.enabled ? 'enabled' : 'disabled',
  ].join(' · ');
}

/** The table's triggers with their definitions (read-only). */
export function TriggersSheet({
  table,
  triggers,
  onClose,
}: {
  table: string;
  triggers: readonly TriggerDto[];
  onClose: () => void;
}): React.JSX.Element {
  return (
    <Sheet title={`Triggers on ${table}`} onClose={onClose}>
      <div className={styles.triggers}>
        {triggers.length === 0 && (
          <p className={styles.formNote}>This table has no triggers.</p>
        )}
        {triggers.map((trigger) => (
          <section
            key={trigger.name}
            className={styles.trigger}
            aria-label={`Trigger ${trigger.name}`}
          >
            <h3>{trigger.name}</h3>
            <p>{triggerSummary(trigger)}</p>
            <SqlViewer
              sql={`${trigger.definition};`}
              label={`Definition of ${trigger.name}`}
            />
          </section>
        ))}
      </div>
      <SheetActions>
        <Button variant="primary" onClick={onClose}>
          Done
        </Button>
      </SheetActions>
    </Sheet>
  );
}
