// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { ExportSourceDto } from '@openrows/ipc';
import { ExportForm } from './ExportForm';
import { exportSettings } from './exportSettings';
import { startExportJob } from './exportJob';
import { formatDto, rememberedOptions, sheetOptions } from './options';
import styles from './Export.module.css';

/** One choice of which rows to export. */
export type ExportScope = { label: string; source: ExportSourceDto };

export type ExportSheetProps = {
  title: string;
  /** Suggested file name without extension. */
  fileName: string;
  /** Default target table of SQL INSERT statements. */
  defaultTable: string;
  /** The rows to export; the first scope is chosen at first. */
  scopes: readonly ExportScope[];
  onClose: () => void;
};

const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/**
 * Scope and format options, then the native save panel; the export then
 * runs as a background job in the jobs popover and the sheet closes.
 */
export function ExportSheet(props: ExportSheetProps): React.JSX.Element {
  const [options, setOptions] = useState(() =>
    sheetOptions(exportSettings.get(), props.defaultTable),
  );
  const [scope, setScope] = useState(0);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const format = formatDto(options);
  const source = props.scopes[scope]?.source;
  const run = (): void => {
    if (!format || !source) return;
    exportSettings.set(rememberedOptions(options));
    setStarting(true);
    setError(null);
    startExportJob({ source, format, fileName: props.fileName }).then(
      (started) => {
        setStarting(false);
        if (started) props.onClose();
      },
      (failure: unknown) => {
        setStarting(false);
        setError(messageOf(failure));
      },
    );
  };
  return (
    <Sheet title={props.title} onClose={props.onClose}>
      {props.scopes.length > 1 && (
        <ScopePicker scopes={props.scopes} chosen={scope} onChoose={setScope} />
      )}
      <ExportForm options={options} onOptions={setOptions} />
      {error && (
        <p className={styles.error} role="alert">
          {error}
        </p>
      )}
      <SheetActions>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button
          variant="primary"
          disabled={!format || !source || starting}
          onClick={run}
        >
          Export…
        </Button>
      </SheetActions>
    </Sheet>
  );
}

function ScopePicker(props: {
  scopes: readonly ExportScope[];
  chosen: number;
  onChoose: (index: number) => void;
}): React.JSX.Element {
  return (
    <fieldset className={styles.scope}>
      <legend>Rows</legend>
      {props.scopes.map((scope, index) => (
        <label key={scope.label}>
          <input
            type="radio"
            name="export-scope"
            checked={props.chosen === index}
            onChange={() => {
              props.onChoose(index);
            }}
          />
          {scope.label}
        </label>
      ))}
    </fieldset>
  );
}
