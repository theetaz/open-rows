// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Icon, IconButton, SegmentedControl } from '@openrows/ui';
import { PointMenu } from '../../shell/PointMenu';
import { isActive } from '../query/run';
import type { TableTabState } from './useTableTab';
import type { ViewMode } from './model';
import { tableSettings } from '../settings/tableSettings';
import { TableSearch } from './TableSearch';
import styles from './ViewBar.module.css';
import { t } from './messages/lazyData';

const modes: readonly { value: ViewMode; label: string }[] = [
  { value: 'data', label: t('data') },
  { value: 'structure', label: t('structure') },
  { value: 'indexes', label: t('index') },
];

/** The view bar's `+` button: + Row, + Column or + Index. */
export type BarAdd = {
  label: string;
  ariaLabel: string;
  /** Absent when nothing can be added. */
  run?: (() => void) | undefined;
  title: string;
};

/** Structure and Index mode tools: Constraints, Triggers and DDL. */
export type BarStructure = {
  onConstraints: () => void;
  onTriggers: () => void;
  onDdl: (anchor: DOMRect) => void;
  ddlOpen: boolean;
};

/** The data view's Reload and Form actions. */
export type BarData = {
  reload: () => void;
  toggleForm: () => void;
};

/**
 * The bar at the top of a table pane (the kit's `.viewbar`): the Data /
 * Structure / Index switch on the left, then the mode's tools — the filter
 * toggle and any-column search, + Row, Reload, Form, Columns and display
 * options for data; + Column or + Index, Constraints, Triggers and DDL for
 * the structure views.
 */
export function TableViewBar({
  state,
  ready,
  add,
  structure,
  data,
}: {
  state: TableTabState;
  /** A session is available for searching and reloading. */
  ready: boolean;
  add: BarAdd;
  /** Present in Structure and Index modes. */
  structure?: BarStructure | undefined;
  /** Present in Data mode. */
  data?: BarData | undefined;
}): React.JSX.Element {
  const { view, actions } = state;
  return (
    <div className={styles.bar}>
      <SegmentedControl
        aria-label={t('tableView')}
        size="compact"
        options={modes}
        value={view.mode}
        onValueChange={actions.mode}
      />
      {structure ? (
        <>
          <span className={styles.spacer} />
          <AddButton add={add} />
          <StructureTools structure={structure} />
        </>
      ) : (
        <>
          <QueryField state={state} ready={ready} />
          <AddButton add={add} />
          {data && <DataTools state={state} ready={ready} data={data} />}
        </>
      )}
    </div>
  );
}

function AddButton({ add }: { add: BarAdd }): React.JSX.Element {
  return (
    <Button
      size="sm"
      variant="ghost"
      aria-label={add.ariaLabel}
      disabled={!add.run}
      title={add.title}
      onClick={add.run}
    >
      <Icon name="plus" />
      <span className={styles.label}>{add.label}</span>
    </Button>
  );
}

/** The filter toggle and the any-column search, sharing one filled field. */
function QueryField({
  state,
  ready,
}: {
  state: TableTabState;
  ready: boolean;
}): React.JSX.Element {
  const { view, actions } = state;
  return (
    <div className={styles.query}>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('filters')}
        title={t('filtersF')}
        aria-pressed={view.filtersOpen}
        onClick={actions.toggleFilters}
      >
        <Icon name="filter" />
      </IconButton>
      {ready && <TableSearch state={state} />}
    </div>
  );
}

function StructureTools({
  structure,
}: {
  structure: BarStructure;
}): React.JSX.Element {
  return (
    <>
      <Button size="sm" variant="ghost" onClick={structure.onConstraints}>
        {t('constraints')}
      </Button>
      <Button size="sm" variant="ghost" onClick={structure.onTriggers}>
        {t('triggers')}
      </Button>
      <Button
        size="sm"
        variant="ghost"
        data-ddl-button
        aria-pressed={structure.ddlOpen}
        aria-haspopup="dialog"
        onClick={(event) => {
          structure.onDdl(event.currentTarget.getBoundingClientRect());
        }}
      >
        {t('ddl')}
      </Button>
    </>
  );
}

/** Reload (⌘R), the row form, columns and display options. */
function DataTools({
  state,
  ready,
  data,
}: {
  state: TableTabState;
  ready: boolean;
  data: BarData;
}): React.JSX.Element {
  const { view, actions } = state;
  return (
    <>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('reloadRowsR')}
        title={t('reloadRowsR')}
        disabled={!ready || isActive(state.entry.run)}
        onClick={data.reload}
      >
        <Icon name="refresh" />
      </IconButton>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('rowForm')}
        title={t('showFocusedRow')}
        aria-pressed={view.form}
        disabled={!state.result?.done}
        onClick={data.toggleForm}
      >
        <Icon name="list" />
      </IconButton>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('columns')}
        title={t('columns')}
        aria-pressed={view.columnsOpen}
        onClick={actions.toggleColumns}
      >
        <Icon name="columns" />
      </IconButton>
      <DisplayMenu state={state} />
    </>
  );
}

/** Row numbers and stripes, toggled from a small menu under its button. */
function DisplayMenu({
  state: { view, actions },
}: {
  state: TableTabState;
}): React.JSX.Element {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const defaults = tableSettings.useValue();
  const items = [
    ['rowNumbers', t('rowNumbers'), view.rowNumbers],
    ['striped', t('alternateRowBackgrounds'), view.striped ?? defaults.striped],
  ] as const;
  return (
    <>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('displayOptions')}
        title={t('displayOptions')}
        aria-haspopup="menu"
        aria-expanded={at !== null}
        onClick={(event) => {
          const box = event.currentTarget.getBoundingClientRect();
          setAt({ x: box.right - 200, y: box.bottom + 4 });
        }}
      >
        <Icon name="more" />
      </IconButton>
      {at && (
        <PointMenu
          x={at.x}
          y={at.y}
          label={t('displayOptions')}
          onClose={() => {
            setAt(null);
          }}
        >
          {items.map(([key, label, checked]) => (
            <button
              key={key}
              type="button"
              role="menuitemcheckbox"
              aria-checked={checked}
              onClick={() => {
                actions.display(key);
              }}
            >
              {label}
              <kbd>{checked ? '✓' : ''}</kbd>
            </button>
          ))}
        </PointMenu>
      )}
    </>
  );
}
