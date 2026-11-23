// SPDX-License-Identifier: Apache-2.0
import type { ImportPreviewDto } from '@openrows/ipc';
import { categoryLabel } from './choices';
import { invalidCells } from './model';
import styles from './Import.module.css';
import { t, tn } from './messages';

/** Rows shown in the preview table. */
const SHOWN = 20;
/** Characters of a value shown in a preview cell. */
const CELL_CHARS = 80;

const shorten = (value: string): string =>
  value.length > CELL_CHARS ? `${value.slice(0, CELL_CHARS)}…` : value;

/**
 * The file's first rows under their source columns and inferred types;
 * values the mapped target column would reject are highlighted.
 */
export function PreviewGrid(props: {
  preview: ImportPreviewDto;
  types: readonly (string | null)[];
}): React.JSX.Element {
  const { preview } = props;
  const invalid = invalidCells(preview);
  const rejected = preview.invalid.length;
  return (
    <section className={styles.section} aria-label={t('preview')}>
      <h3 className={styles.subhead}>{t('filePreview')}</h3>
      <div className={`${styles.well ?? ''} ${styles.previewScroll ?? ''}`}>
        <table className={styles.preview} aria-label={t('filePreview')}>
          <thead>
            <tr>
              <th scope="col">{t('line')}</th>
              {preview.columns.map((column) => (
                <th key={column.name} scope="col">
                  <span>{column.name}</span>
                  <span className={styles.category}>
                    {categoryLabel(column.category)}
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {preview.rows.slice(0, SHOWN).map((row, rowIndex) => (
              <tr key={String(preview.lines[rowIndex] ?? rowIndex)}>
                <th scope="row">{preview.lines[rowIndex]}</th>
                {row.map((cell, column) => {
                  const bad = invalid.has(
                    `${String(rowIndex)}:${String(column)}`,
                  );
                  const type = props.types[column];
                  return (
                    <td
                      key={String(column)}
                      data-invalid={bad || undefined}
                      data-null={cell === null || undefined}
                      title={
                        bad && type ? t('notValidType', { type }) : undefined
                      }
                    >
                      {cell === null ? t('null') : shorten(cell)}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className={styles.note}>
        {t('typesInferred', { sampled: preview.sampled.toLocaleString() })}
        {rejected > 0 && ' '}
        {rejected > 0 &&
          tn('previewedValuesMisfit', rejected, {
            count: rejected.toLocaleString(),
          })}
        {preview.errors.length > 0 && ' '}
        {preview.errors.length > 0 &&
          tn('malformedRows', preview.errors.length, {
            count: preview.errors.length.toLocaleString(),
          })}
      </p>
    </section>
  );
}
