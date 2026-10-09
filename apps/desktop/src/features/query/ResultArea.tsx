// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { ExportSheet, resultExport } from '../export';
import { ResultPane } from './ResultPane';
import type { ResultMeta, RunState } from './run';
import type { SelectedCell } from './cell';
import type { ResultView } from './editorSettings';
import type { RunScope } from './request';

/** The result pane and the export sheet of its shown result. */
export function ResultArea(props: {
  run: RunState;
  view: ResultView;
  runDefault: RunScope;
  /** The tab's open session; export needs it for the connection's dialect. */
  sessionId: string | null;
  onCell: (cell: SelectedCell) => void;
  /** Receives Export… of the shown result, for the Export results command. */
  onExportable?: ((exportShown: (() => void) | null) => void) | undefined;
}): React.JSX.Element {
  const { run, sessionId } = props;
  const [exporting, setExporting] = useState<ResultMeta | null>(null);
  return (
    <>
      <ResultPane
        run={run}
        view={props.view}
        runDefault={props.runDefault}
        onCell={props.onCell}
        onExport={sessionId ? setExporting : undefined}
        onExportable={props.onExportable}
      />
      {exporting && sessionId && (
        <ExportSheet
          {...resultExport({
            sessionId,
            result: exporting,
            statement: run.statements[exporting.statement],
          })}
          onClose={() => {
            setExporting(null);
          }}
        />
      )}
    </>
  );
}
