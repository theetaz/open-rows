// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import type { EngineDto } from '@openrows/ipc';
import { ExportSheet, resultExport } from '../export';
import type { ExtraResultTab } from '../explain';
import { hasResultPane, ResultPane } from './ResultPane';
import type { ResultMeta, RunState } from './run';
import type { SelectedCell } from './cell';
import type { ResultView } from './editorSettings';
import { executionSettings } from './executionSettings';

/** The result pane and the export sheet of its shown result; nothing before the first run. */
export function ResultArea(props: {
  run: RunState;
  view: ResultView;
  /** The tab's open session; export needs it for the connection's dialect. */
  sessionId: string | null;
  /** Engine of the connection: quoting for copied SQL. */
  engine: EngineDto;
  onCell: (cell: SelectedCell) => void;
  /** Receives Export… of the shown result, for the Export results command. */
  onExportable?: ((exportShown: (() => void) | null) => void) | undefined;
  /** A tab beside the result sets (the query plan). */
  extra?: ExtraResultTab | null | undefined;
  /** Switch between result tabs and stacked results. */
  onView?: ((view: ResultView) => void) | undefined;
}): React.JSX.Element | null {
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
  const { showNotices } = executionSettings.useValue();
  if (!hasResultPane(run, props.extra)) return null;
  return (
    <>
      <ResultPane
        run={run}
        view={props.view}
        showNotices={showNotices}
        engine={props.engine}
        onCell={props.onCell}
        onExport={sessionId ? exportResult : undefined}
        onExportable={props.onExportable}
        extra={props.extra}
        onView={props.onView}
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
