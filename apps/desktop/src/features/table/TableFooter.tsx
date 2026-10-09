// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Icon, IconButton, SegmentedControl } from '@openrows/ui';
import { PointMenu } from '../../shell/PointMenu';
import { isActive } from '../query/run';
import { canNext } from './useTableTab';
import type { TableTabState } from './useTableTab';
import { countKey, rangeLabel } from './model';
import type { ViewMode } from './model';
import styles from './Table.module.css';
import extras from './TableExtras.module.css';

const modes: readonly { value: ViewMode; label: string }[] = [
  { value: 'data', label: 'Data' },
  { value: 'structure', label: 'Structure' },
  { value: 'indexes', label: 'Index' },
];

/** `1–300 of ~1,811 rows`, or what the page is doing. */
function pageText(state: TableTabState): string {
  const { entry, result, view, total } = state;
  if (isActive(entry.run)) return 'Loading rows…';
  if (!result?.done) return entry.run.status === 'failed' ? 'Failed' : '';
  return rangeLabel(view.offset, result.rows, total);
}

/** Exact count action: Count, Counting… with Cancel, or the error. */
function CountControl({
  state,
}: {
  state: TableTabState;
}): React.JSX.Element | null {
  const { count } = state.entry;
  const current = 'key' in count && count.key === countKey(state.query);
  if (current && count.status === 'counting')
    return (
      <Button size="sm" variant="ghost" onClick={state.cancelCount}>
        Counting… Cancel
      </Button>
    );
  if (current && count.status === 'done') return null;
  return (
    <Button
      size="sm"
      variant="ghost"
      title={
        current && count.status === 'error'
          ? count.message
          : 'Count rows exactly'
      }
      onClick={state.actions.count}
    >
      Count
    </Button>
  );
}

/** The footer's `+` button: + Row, + Column or + Index. */
export type FooterAdd = {
  label: 'Row' | 'Column' | 'Index';
  /** Absent when nothing can be added. */
  run?: (() => void) | undefined;
  title: string;
};

/** Structure and Index mode extras: Triggers, DDL and a centred status. */
export type FooterStructure = {
  status: string;
  onConstraints: () => void;
  onTriggers: () => void;
  onDdl: (anchor: DOMRect) => void;
  ddlOpen: boolean;
};

/** TablePlus-style footer: views, +, page range or status, then mode tools. */
/** The data view's selection statistics and its Reload and Form actions. */
export type FooterData = {
  /** Count, sum, average… of a multi-cell selection. */
  aggregate: string | null;
  reload: () => void;
  toggleForm: () => void;
};

export function TableFooter({
  state,
  ready,
  add,
  structure,
  data,
}: {
  state: TableTabState;
  /** A session is available for paging and counting. */
  ready: boolean;
  add: FooterAdd;
  /** Present in Structure and Index modes. */
  structure?: FooterStructure | undefined;
  /** Present in Data mode. */
  data?: FooterData | undefined;
}): React.JSX.Element {
  const { view, actions } = state;
  return (
    <footer className={styles.footer} aria-label="Table footer">
      <SegmentedControl
        aria-label="Table view"
        size="compact"
        tone="accent"
        options={modes}
        value={view.mode}
        onValueChange={actions.mode}
      />
      <Button
        size="sm"
        aria-label={`Add ${add.label.toLowerCase()}`}
        disabled={!add.run}
        title={add.title}
        onClick={add.run}
      >
        <Icon name="plus" />
        <span className={styles.label}>{add.label}</span>
      </Button>
      {structure ? (
        <StructureTools structure={structure} />
      ) : (
        <DataTools state={state} ready={ready} data={data} />
      )}
    </footer>
  );
}

function StructureTools({
  structure,
}: {
  structure: FooterStructure;
}): React.JSX.Element {
  return (
    <>
      <span className={styles.status}>
        <span className={styles.range} role="status">
          {structure.status}
        </span>
      </span>
      <Button size="sm" onClick={structure.onConstraints}>
        Constraints
      </Button>
      <Button size="sm" onClick={structure.onTriggers}>
        Triggers
      </Button>
      <Button
        size="sm"
        data-ddl-button
        aria-pressed={structure.ddlOpen}
        aria-haspopup="dialog"
        onClick={(event) => {
          structure.onDdl(event.currentTarget.getBoundingClientRect());
        }}
      >
        DDL
      </Button>
    </>
  );
}

function DataTools({
  state,
  ready,
  data,
}: {
  state: TableTabState;
  ready: boolean;
  data: FooterData | undefined;
}): React.JSX.Element {
  const { view, actions } = state;
  return (
    <>
      <span className={styles.status}>
        {data?.aggregate ? (
          <span
            className={extras.aggregate}
            role="status"
            aria-label="Selection statistics"
            title={data.aggregate}
          >
            {data.aggregate}
          </span>
        ) : (
          <span
            className={styles.range}
            role="status"
            aria-label="Rows shown"
            title={pageText(state)}
          >
            {pageText(state)}
          </span>
        )}
        {ready && view.mode === 'data' && <CountControl state={state} />}
      </span>
      {data && <DataActions state={state} ready={ready} data={data} />}
      <Button
        size="sm"
        aria-label="Filters"
        title="Filters (⌘F)"
        aria-pressed={view.filtersOpen}
        onClick={actions.toggleFilters}
      >
        <Icon name="filter" />
        <span className={styles.label}>Filters</span>
      </Button>
      <Button
        size="sm"
        aria-label="Columns"
        title="Columns"
        aria-pressed={view.columnsOpen}
        onClick={actions.toggleColumns}
      >
        <Icon name="columns" />
        <span className={styles.label}>Columns</span>
      </Button>
      <Paging state={state} ready={ready} />
    </>
  );
}

/** Reload (⌘R) and the row form toggle. */
function DataActions({
  state,
  ready,
  data,
}: {
  state: TableTabState;
  ready: boolean;
  data: FooterData;
}): React.JSX.Element {
  return (
    <>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Reload rows (⌘R)"
        title="Reload rows (⌘R)"
        disabled={!ready || isActive(state.entry.run)}
        onClick={data.reload}
      >
        <Icon name="refresh" />
      </IconButton>
      <Button
        size="sm"
        aria-label="Row form"
        title="Show the focused row as a form (⌥⌘F)"
        aria-pressed={state.view.form}
        disabled={!state.result?.done}
        onClick={data.toggleForm}
      >
        <Icon name="list" />
        <span className={styles.label}>Form</span>
      </Button>
    </>
  );
}

/** ‹ display options › like TablePlus' page controls. */
function Paging({
  state,
  ready,
}: {
  state: TableTabState;
  ready: boolean;
}): React.JSX.Element {
  const { view, actions, entry } = state;
  return (
    <span className={styles.paging}>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Previous page (⌘[)"
        disabled={!ready || view.offset === 0 || isActive(entry.run)}
        onClick={() => {
          actions.page(-1);
        }}
      >
        <Icon name="chevronLeft" />
      </IconButton>
      <DisplayMenu state={state} />
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Next page (⌘])"
        disabled={!ready || !canNext(entry)}
        onClick={() => {
          actions.page(1);
        }}
      >
        <Icon name="chevronRight" />
      </IconButton>
    </span>
  );
}

/** Row numbers and stripes, toggled from a small menu. */
function DisplayMenu({
  state: { view, actions },
}: {
  state: TableTabState;
}): React.JSX.Element {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const items = [
    ['rowNumbers', 'Row numbers', view.rowNumbers],
    ['striped', 'Alternate row backgrounds', view.striped],
  ] as const;
  return (
    <>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label="Display options"
        aria-haspopup="menu"
        aria-expanded={at !== null}
        onClick={(event) => {
          const box = event.currentTarget.getBoundingClientRect();
          setAt({ x: box.left, y: box.top });
        }}
      >
        <Icon name="settings" />
      </IconButton>
      {at && (
        <PointMenu
          x={at.x}
          y={at.y - 80}
          label="Display options"
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
