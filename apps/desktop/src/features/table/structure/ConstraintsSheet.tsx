// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { ConstraintDto } from '@openrows/ipc';
import { describe } from './schemaChanges';
import type { Structure } from './useStructure';
import styles from './forms/Forms.module.css';

const kindLabel: Record<ConstraintDto['kind'], string> = {
  primary: 'Primary key',
  unique: 'Unique',
  check: 'Check',
  foreign: 'Foreign key',
  exclusion: 'Exclusion',
};

/** One catalog constraint with Delete, or Restore once its drop is staged. */
function ConstraintItem({
  constraint,
  structure,
}: {
  constraint: ConstraintDto;
  structure: Structure;
}): React.JSX.Element {
  const { schema } = structure;
  const id = `dropConstraint:${constraint.name}`;
  const dropped = schema.history.current.some((item) => item.id === id);
  return (
    <div className={styles.item} data-dropped={dropped}>
      <span title={constraint.definition}>
        {constraint.name} · {constraint.definition}
      </span>
      <span>{dropped ? 'Deleted' : kindLabel[constraint.kind]}</span>
      <Button
        size="sm"
        disabled={!schema.enabled}
        onClick={() => {
          if (dropped) schema.unstage(id);
          else schema.stage({ kind: 'dropConstraint', name: constraint.name });
        }}
      >
        {dropped ? 'Restore' : 'Delete'}
      </Button>
    </div>
  );
}

/**
 * The table's constraints (keys, checks, foreign keys) with staged drops
 * and additions, and the forms that add foreign keys and checks.
 */
export function ConstraintsSheet({
  structure,
  onClose,
}: {
  structure: Structure;
  onClose: () => void;
}): React.JSX.Element {
  const { schema, features } = structure;
  const added = schema.history.current.filter(
    (item) =>
      item.change.kind === 'addForeignKey' || item.change.kind === 'addCheck',
  );
  const constraints = schema.detail?.constraints ?? [];
  return (
    <Sheet title="Constraints" onClose={onClose}>
      <div className={styles.list} aria-label="Constraints">
        {constraints.map((constraint) => (
          <ConstraintItem
            key={constraint.name}
            constraint={constraint}
            structure={structure}
          />
        ))}
        {added.map((item) => (
          <div key={item.id} className={styles.item}>
            <span>{describe(item.change)}</span>
            <span>New</span>
            <Button
              size="sm"
              disabled={!schema.enabled}
              onClick={() => {
                schema.unstage(item.id);
              }}
            >
              Remove
            </Button>
          </div>
        ))}
        {!constraints.length && !added.length && (
          <p className={styles.note}>This table has no constraints.</p>
        )}
      </div>
      <SheetActions
        leading={
          <>
            <Button
              disabled={!schema.enabled || !features.foreignKeys}
              title={
                features.foreignKeys
                  ? undefined
                  : 'Not supported by this database'
              }
              onClick={() => {
                structure.setSheet('foreignKey');
              }}
            >
              Add foreign key…
            </Button>
            <Button
              disabled={!schema.enabled || !features.checks}
              title={
                features.checks ? undefined : 'Not supported by this database'
              }
              onClick={() => {
                structure.setSheet('check');
              }}
            >
              Add check…
            </Button>
          </>
        }
      >
        <Button variant="primary" onClick={onClose}>
          Done
        </Button>
      </SheetActions>
    </Sheet>
  );
}
