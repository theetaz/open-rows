// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { ConstraintDto } from '@openrows/ipc';
import { describe } from './schemaChanges';
import type { Structure } from './useStructure';
import styles from './forms/Forms.module.css';
import { t } from '../messages/lazyStructure';

const kindLabel: Record<ConstraintDto['kind'], string> = {
  primary: t('primaryKey'),
  unique: t('unique'),
  check: t('check2'),
  foreign: t('foreignKey2'),
  exclusion: t('exclusion'),
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
      <span>{dropped ? t('deleted') : kindLabel[constraint.kind]}</span>
      <Button
        size="sm"
        disabled={!schema.enabled}
        onClick={() => {
          if (dropped) schema.unstage(id);
          else schema.stage({ kind: 'dropConstraint', name: constraint.name });
        }}
      >
        {dropped ? t('restore') : t('delete')}
      </Button>
    </div>
  );
}

function AddConstraintButtons({
  structure,
}: {
  structure: Structure;
}): React.JSX.Element {
  const choices = [
    { sheet: 'key', label: t('addKey'), enabled: structure.features.keys },
    {
      sheet: 'foreignKey',
      label: t('addForeignKey'),
      enabled: structure.features.foreignKeys,
    },
    {
      sheet: 'check',
      label: t('addCheck2'),
      enabled: structure.features.checks,
    },
  ] as const;
  return (
    <>
      {choices.map(({ sheet, label, enabled }) => (
        <Button
          key={sheet}
          disabled={!structure.schema.enabled || !enabled}
          title={enabled ? undefined : t('notSupportedDatabase')}
          onClick={() => {
            structure.setSheet(sheet);
          }}
        >
          {label}
        </Button>
      ))}
    </>
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
  const { schema } = structure;
  const added = schema.history.current.filter(
    (item) =>
      item.change.kind === 'addForeignKey' ||
      item.change.kind === 'addCheck' ||
      item.change.kind === 'addPrimaryKey' ||
      item.change.kind === 'addUnique',
  );
  const constraints = schema.detail?.constraints ?? [];
  return (
    <Sheet title={t('constraints')} onClose={onClose}>
      <div className={styles.list} aria-label={t('constraints')}>
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
            <span>{t('new')}</span>
            <Button
              size="sm"
              disabled={!schema.enabled}
              onClick={() => {
                schema.unstage(item.id);
              }}
            >
              {t('remove')}
            </Button>
          </div>
        ))}
        {!constraints.length && !added.length && (
          <p className={styles.note}>{t('tableNoConstraints')}</p>
        )}
      </div>
      <SheetActions leading={<AddConstraintButtons structure={structure} />}>
        <Button variant="primary" onClick={onClose}>
          {t('done')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
