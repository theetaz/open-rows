// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import type { SchemaChangeDto } from '@openrows/ipc';
import type { Structure } from '../useStructure';
import { t } from '../../messages/lazyStructure';

/** Optional conversion and ordinal position for an existing column. */
type ColumnOptionsDraft = { expression: string; position: string };

/** Changes chosen in the conversion and positioning fields. */
export function optionChanges(
  column: string,
  dataType: string,
  draft: ColumnOptionsDraft,
  hadUsing = false,
): SchemaChangeDto[] {
  const changes: SchemaChangeDto[] = [];
  if (draft.expression.trim())
    changes.push({
      kind: 'columnTypeUsing',
      column,
      dataType,
      expression: draft.expression.trim(),
    });
  else if (hadUsing) changes.push({ kind: 'columnType', column, dataType });
  if (draft.position !== '')
    changes.push({
      kind: 'reorderColumn',
      column,
      after: draft.position === 'first' ? null : draft.position.slice(6),
    });
  return changes;
}

/** Capability-gated advanced fields and the reason reordering is hidden. */
export function ColumnOptions({
  structure,
  column,
  draft,
  onChange,
}: {
  structure: Structure;
  column: string;
  draft: ColumnOptionsDraft;
  onChange: (draft: ColumnOptionsDraft) => void;
}): React.JSX.Element {
  const { features } = structure;
  const positions = [
    { value: '', label: t('keepPosition') },
    { value: 'first', label: t('first') },
    ...structure.rows
      .filter(
        (row) => row.column !== null && row.column !== column && !row.dropped,
      )
      .map((row) => ({
        value: `after:${row.column ?? ''}`,
        label: t('afterName', { name: row.name }),
      })),
  ];
  return (
    <>
      {features.typeUsing && (
        <TextField
          label={t('usingConversionExpression')}
          placeholder={t('columnInteger')}
          value={draft.expression}
          onChange={(event) => {
            onChange({ ...draft, expression: event.target.value });
          }}
        />
      )}
      {features.columnReordering ? (
        <SelectField
          label={t('position')}
          value={draft.position}
          options={positions}
          onValueChange={(position) => {
            onChange({ ...draft, position });
          }}
        />
      ) : (
        <p>{t('columnReorderingUnavailable')}</p>
      )}
    </>
  );
}
