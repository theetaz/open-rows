// SPDX-License-Identifier: Apache-2.0
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

/** File, target, mapping, preview and options; dry-run results below. */
export function ImportSetup(props: {
  state: ImportDraftState;
  job: ImportJob;
  context: ImportContext;
}): React.JSX.Element {
  const { state, job, context } = props;
  const { draft, preview, table, update } = state;
  const types = previewTypes(
    draft,
    table?.columns ?? [],
    preview?.columns.length ?? 0,
  );
  return (
    <>
      <FileSection
        file={draft.file}
        options={draft.options}
        onPick={state.pick}
        onOptions={(options) => {
          update({ options });
        }}
      />
      {state.error && (
        <p className={styles.error} role="alert">
          {state.error}
        </p>
      )}
      {preview && (
        <>
          <TargetSection
            draft={draft}
            schema={context.schema}
            tables={state.tables}
            onChange={update}
          />
          {draft.target === 'existing' && table && (
            <MappingTable
              draft={draft}
              table={table}
              sources={preview.columns}
              onChange={update}
            />
          )}
          {draft.target === 'new' && (
            <NewColumnsTable
              draft={draft}
              sources={preview.columns}
              onChange={update}
            />
          )}
          <PreviewGrid preview={preview} types={types} />
          <LoadOptions draft={draft} onChange={update} />
        </>
      )}
      {state.loading && !preview && (
        <p className={styles.note} role="status">
          Reading the file…
        </p>
      )}
      <ImportStatus job={job} table={draft.table} />
    </>
  );
}
