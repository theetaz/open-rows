// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, IconButton, TextField } from '@openrows/ui';
import type { ResultView } from './useResultView';
import styles from './Query.module.css';
import { t } from './lazyMessages';

/**
 * One compact result toolbar (the kit's barline): a filled filter field,
 * + Row, then Fit columns, Columns and Form as quiet icon buttons, and
 * Export trailing.
 */
export function ResultTools(props: {
  view: ResultView;
  ready: boolean;
  onInsert: () => void;
  onFit: () => void;
  onColumns: () => void;
  onForm: () => void;
  onExport: (() => void) | undefined;
  hasRows: boolean;
  /** The Columns panel and the row form are open. */
  open: { columns: boolean; form: boolean };
}): React.JSX.Element {
  const { view } = props;
  return (
    <div
      className={styles.resultTools}
      role="toolbar"
      aria-label={t('resultToolbar')}
    >
      <TextField
        aria-label={t('filterRows')}
        placeholder={t('filterRows')}
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
          {t('clearColumnFilters', { count: view.columnFilters.length })}
        </Button>
      )}
      <Button
        size="sm"
        variant="ghost"
        disabled={!props.ready}
        onClick={props.onInsert}
      >
        {t('row')}
      </Button>
      <ViewButtons {...props} />
      <span className={styles.toolsSpacer} />
      {props.onExport && (
        <Button size="sm" variant="ghost" onClick={props.onExport}>
          {t('export')}
        </Button>
      )}
    </div>
  );
}

/** Fit columns, the Columns panel and the row form as quiet icon buttons. */
function ViewButtons(
  props: Pick<
    Parameters<typeof ResultTools>[0],
    'onFit' | 'onColumns' | 'onForm' | 'hasRows' | 'open'
  >,
): React.JSX.Element {
  return (
    <>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('fitColumns')}
        title={t('fitColumns')}
        onClick={props.onFit}
      >
        <Icon name="fit" />
      </IconButton>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('columns')}
        title={t('columns')}
        aria-pressed={props.open.columns}
        onClick={props.onColumns}
      >
        <Icon name="columns" />
      </IconButton>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('form')}
        title={t('form')}
        aria-pressed={props.open.form}
        disabled={!props.hasRows}
        onClick={props.onForm}
      >
        <Icon name="list" />
      </IconButton>
    </>
  );
}
