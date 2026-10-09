// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import type { EngineDto } from '@openrows/ipc';
import { ExportSheet, resultExport } from '../export';
import { ResultPane } from './ResultPane';
import type { ResultMeta, RunState } from './run';
import type { SelectedCell } from './cell';
import type { ResultView } from './editorSettings';
import type { RunScope } from './request';
import { executionSettings } from './executionSettings';

/** The result pane and the export sheet of its shown result. */
export function ResultArea(props: {
  run: RunState;
  view: ResultView;
  runDefault: RunScope;
  /** The tab's open session; export needs it for the connection's dialect. */
  sessionId: string | null;
  /** Engine of the connection: quoting for copied SQL. */
  engine: EngineDto;
  onCell: (cell: SelectedCell) => void;
  /** Receives Export… of the shown result, for the Export results command. */
  onExportable?: ((exportShown: (() => void) | null) => void) | undefined;
}): React.JSX.Element {
  const { run, sessionId } = props;
  const [exporting, setExporting] = useState<{
    result: ResultMeta;
    rows?: number[] | undefined;
  } | null>(null);
  // Stable: the pane re-reports Export… whenever this changes.
  const exportResult = useCallback(
    (result: ResultMeta, rows?: number[]): void => {
      setExporting({ result, rows });
    },
    [],
  );
  const { showNotices } = executionSettings.use();
  return (
    <>
      <ResultPane
        run={run}
        view={props.view}
        runDefault={props.runDefault}
        showNotices={showNotices}
        engine={props.engine}
        onCell={props.onCell}
        onExport={sessionId ? exportResult : undefined}
        onExportable={props.onExportable}
      />
      {exporting && sessionId && (
        <ExportSheet
          {...resultExport({
            sessionId,
            result: exporting.result,
            statement: run.statements[exporting.result.statement],
            selected: exporting.rows,
          })}
          onClose={() => {
            setExporting(null);
          }}
        />
      )}
    </>
  );
}
