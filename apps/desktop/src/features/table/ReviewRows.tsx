// SPDX-License-Identifier: Apache-2.0
import { Badge } from '@openrows/ui';
import type { CellValueDto } from '@openrows/ipc';
import type { StagedEdit } from './editHistory';
import styles from './Review.module.css';
import { t } from './messages/lazyData';

const kinds: Record<
  StagedEdit['kind'],
  { label: string; tone: 'warning' | 'success' | 'danger' }
> = {
  update: { label: t('kindUpdated'), tone: 'warning' },
  insert: { label: t('kindInserted'), tone: 'success' },
  delete: { label: t('kindDeleted'), tone: 'danger' },
};

function shown(value: CellValueDto | string | null | undefined): string {
  if (value === null || value === undefined) return 'NULL';
  if (typeof value === 'string') return value;
  if (value.kind === 'null') return 'NULL';
  if (value.kind === 'default') return 'DEFAULT';
  return value.value;
}

/** `id = 48207`: the stable key the statement matches on. */
function keyText(edit: StagedEdit, key: readonly string[]): string {
  if (edit.kind === 'insert') return t('newRow');
  return key
    .map((name) => `${name} = ${shown(edit.original[name])}`)
    .join(', ');
}

/** The changed values: `status pending → shipped` for updates, values for inserts. */
function Values({ edit }: { edit: StagedEdit }): React.JSX.Element {
  const names = Object.keys(edit.values);
  return (
    <>
      {names.map((name) => (
        <span key={name} className={styles.change}>
          <span className={styles.name}>{name}</span>
          {edit.kind === 'update' && (
            <>
              <s className={styles.old}>{shown(edit.original[name])}</s>
              <span aria-hidden="true">→</span>
            </>
          )}
          <span className={styles.new}>{shown(edit.values[name])}</span>
        </span>
      ))}
    </>
  );
}

/**
 * The staged rows of a table review (the mockup's change list above the SQL):
 * the change as a word on a soft fill, the row key and old → new values.
 */
export function ReviewRows({
  edits,
  keyColumns,
}: {
  edits: readonly StagedEdit[];
  keyColumns: readonly string[];
}): React.JSX.Element | null {
  if (!edits.length) return null;
  return (
    <section className={styles.section} aria-label={t('stagedRows')}>
      <h3 className={styles.title}>{t('stagedRows')}</h3>
      <table className={styles.rows}>
        <thead>
          <tr>
            <th scope="col">{t('reviewChange')}</th>
            <th scope="col">{t('reviewKey')}</th>
            <th scope="col">{t('reviewValues')}</th>
          </tr>
        </thead>
        <tbody>
          {edits.map((edit) => (
            <tr key={edit.id}>
              <td>
                <Badge tone={kinds[edit.kind].tone}>
                  {kinds[edit.kind].label}
                </Badge>
              </td>
              <td className={styles.mono}>{keyText(edit, keyColumns)}</td>
              <td className={styles.mono}>
                <Values edit={edit} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
