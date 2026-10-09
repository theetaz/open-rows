// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import styles from './Table.module.css';

/** What the confirmation is for: Discard Changes, or Reload with edits staged. */
export type DiscardKind = 'discard' | 'reload';

const changes = (count: number): string =>
  `${count.toLocaleString('en-US')} staged ${count === 1 ? 'change' : 'changes'}`;

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
      title={reload ? 'Reload rows?' : `Discard ${changes(count)}?`}
      size="alert"
      onClose={onClose}
    >
      <p className={styles.editNote}>
        {reload
          ? `The table has ${changes(count)}. Reloading reads the rows again from the server; keep the changes staged, or discard them.`
          : `The ${changes(count)} are removed and the rows reload from the server. Nothing was written yet, so nothing is rolled back; SQL already executed or committed is not undone.`}
      </p>
      <SheetActions>
        <Button onClick={onClose}>Cancel</Button>
        {reload && <Button onClick={onKeep}>Reload, keep changes</Button>}
        <Button variant="destructive" onClick={onDiscard}>
          {reload ? 'Discard & reload' : 'Discard changes'}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
