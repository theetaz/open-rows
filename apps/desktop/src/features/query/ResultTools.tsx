// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, TextField } from '@openrows/ui';
import type { ResultView } from './useResultView';
import styles from './Query.module.css';

/** One compact result toolbar: filtering, row actions, presentation and export. */
export function ResultTools(props: {
  view: ResultView;
  ready: boolean;
  onInsert: () => void;
  onFit: () => void;
  onColumns: () => void;
  onForm: () => void;
  onExport: (() => void) | undefined;
  hasRows: boolean;
}): React.JSX.Element {
  const { view } = props;
  return (
    <div
      className={styles.resultTools}
      role="toolbar"
      aria-label="Result toolbar"
    >
      <TextField
        aria-label="Filter rows"
        placeholder="Filter rows"
        controlSize="compact"
        className={styles.resultFilter}
        leadingIcon={<Icon name="search" />}
        value={view.needle}
        onChange={(event) => {
          view.setNeedle(event.target.value);
        }}
      />
      {view.columnFilters.length > 0 && (
        <Button size="sm" variant="ghost" onClick={view.clearColumnFilters}>
          Clear column filters ({view.columnFilters.length})
        </Button>
      )}
      <Button
        size="sm"
        variant="ghost"
        disabled={!props.ready}
        onClick={props.onInsert}
      >
        + Row
      </Button>
      <Button size="sm" variant="ghost" onClick={props.onFit}>
        Fit columns
      </Button>
      <Button size="sm" variant="ghost" onClick={props.onColumns}>
        Columns
      </Button>
      <Button
        size="sm"
        variant="ghost"
        disabled={!props.hasRows}
        onClick={props.onForm}
      >
        Form
      </Button>
      {props.onExport && (
        <Button
          size="sm"
          variant="ghost"
          className={styles.exportButton}
          onClick={props.onExport}
        >
          Export…
        </Button>
      )}
    </div>
  );
}
