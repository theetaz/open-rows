// SPDX-License-Identifier: Apache-2.0
import type { ImportPreviewDto } from '@openrows/ipc';
import { MappingTable, NewColumnsTable } from './ColumnsSection';
import { FileSection } from './FileSection';
import { ImportStatus } from './ImportStatus';
import type { ImportJob } from './job';
import { LoadOptions } from './LoadOptions';
import { previewTypes } from './model';
import type { ImportContext } from './model';
import { PreviewGrid } from './PreviewGrid';
import { TargetSection } from './TargetSection';
import type { ImportDraftState } from './useImportDraft';
import styles from './Import.module.css';
import { t } from './messages';

type Props = {
  state: ImportDraftState;
  job: ImportJob;
  context: ImportContext;
};

/**
 * File, parsing, target and load options on the left; once the file is
 * read, the column mapping, the preview and dry-run results on the right.
 */
export function ImportSetup(props: Props): React.JSX.Element {
  const { state, job, context } = props;
  const { draft, preview, update } = state;
  const status = <ImportStatus job={job} table={draft.table} />;
  return (
    <div className={styles.columns} data-split={preview ? '' : undefined}>
      <div className={styles.column}>
        <FileSection
          file={draft.file}
          options={draft.options}
          onPick={state.pick}
          onOptions={(options) => {
            update({ options });
          }}
        />
        {preview && (
          <>
            <div className={styles.separator} />
            <TargetSection
              draft={draft}
              schema={context.schema}
              tables={state.tables}
              onChange={update}
            />
            <LoadOptions draft={draft} onChange={update} />
          </>
        )}
        {state.error && (
          <p className={styles.error} role="alert">
            {state.error}
          </p>
        )}
        {state.loading && !preview && (
          <p className={styles.note} role="status">
            {t('readingFile')}
          </p>
        )}
        {!preview && status}
      </div>
      {preview && (
        <div className={styles.column}>
          <Columns {...props} preview={preview} />
          {status}
        </div>
      )}
    </div>
  );
}

/** The mapping (existing table) or new columns, then the file preview. */
function Columns(
  props: Props & { preview: ImportPreviewDto },
): React.JSX.Element {
  const { state, preview } = props;
  const { draft, table, update } = state;
  const types = previewTypes(
    draft,
    table?.columns ?? [],
    preview.columns.length,
  );
  return (
    <>
      {draft.target === 'existing' && table && (
        <section className={styles.section}>
          <h3 className={styles.subhead}>{t('columnMapping')}</h3>
          <div className={styles.well}>
            <MappingTable
              draft={draft}
              table={table}
              sources={preview.columns}
              onChange={update}
            />
          </div>
        </section>
      )}
      {draft.target === 'new' && (
        <section className={styles.section}>
          <h3 className={styles.subhead}>{t('newColumns')}</h3>
          <div className={styles.well}>
            <NewColumnsTable
              draft={draft}
              sources={preview.columns}
              onChange={update}
            />
          </div>
        </section>
      )}
      <PreviewGrid preview={preview} types={types} />
    </>
  );
}
