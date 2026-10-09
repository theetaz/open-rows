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

/** Open the trigger editor: a new trigger (`null`), an edit, or a drop. */
export type TriggerAction = (name: string | null, drop: boolean) => void;

/**
 * The table's triggers with their definitions; New, Edit and Drop open the
 * trigger editors when the engine has them.
 */
export function TriggersSheet({
  table,
  triggers,
  onTrigger,
  onClose,
}: {
  table: string;
  triggers: readonly TriggerDto[];
  /** Absent when triggers cannot be edited here. */
  onTrigger?: TriggerAction | undefined;
  onClose: () => void;
}): React.JSX.Element {
  const act = (name: string | null, drop: boolean) => () => {
    onClose();
    onTrigger?.(name, drop);
  };
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
            {onTrigger && (
              <p>
                <Button size="sm" onClick={act(trigger.name, false)}>
                  Edit…
                </Button>{' '}
                <Button size="sm" onClick={act(trigger.name, true)}>
                  Drop…
                </Button>
              </p>
            )}
            <SqlViewer
              sql={`${trigger.definition};`}
              label={`Definition of ${trigger.name}`}
            />
          </section>
        ))}
      </div>
      <SheetActions
        leading={
          onTrigger && <Button onClick={act(null, false)}>New trigger…</Button>
        }
      >
        <Button variant="primary" onClick={onClose}>
          Done
        </Button>
      </SheetActions>
    </Sheet>
  );
}
