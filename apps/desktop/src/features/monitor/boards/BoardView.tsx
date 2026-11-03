// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useMemo, useState } from 'react';
import { Button, SelectField, TextField } from '@openrows/ui';
import { confirmDestructive } from '@openrows/ipc';
import type { Board, BoardDto, MonitorTarget, Widget } from '@openrows/ipc';
import type { Connection } from '../report/connection';
import { ParamInputs, ParamsEditor } from './BoardParams';
import { EmptyBoard, Review, RunNotes } from './BoardNotices';
import { WidgetCard } from './WidgetCard';
import { WidgetEditor, newWidget } from './WidgetEditor';
import { useBoardRuns } from './useBoardRuns';
import type { BoardRuns } from './useBoardRuns';
import type { Boards } from './useBoards';
import styles from './Boards.module.css';
import { t } from '../messages/lazyBoards';
const BoardReportDialog = lazy(() =>
  import('../report/ReportEntry').then((module) => ({
    default: module.BoardReportDialog,
  })),
);

const REFRESH = [
  { value: '0', label: t('manual') },
  { value: '30', label: t('every30S') },
  { value: '60', label: t('every1Min') },
  { value: '300', label: t('every5Min') },
  { value: '900', label: t('every15Min') },
] as const;

type Editing = { kind: 'widget'; widget: Widget } | { kind: 'params' } | null;

/** "Export report…" and its dialog, loaded on first use. */
function BoardReportButton(props: {
  connection: Connection;
  board: BoardDto;
  runs: BoardRuns;
}): React.JSX.Element {
  const [reporting, setReporting] = useState(false);
  return (
    <>
      <Button
        size="sm"
        variant="ghost"
        onClick={() => {
          setReporting(true);
        }}
      >
        {t('exportReport')}
      </Button>
      {reporting && (
        <Suspense fallback={null}>
          <BoardReportDialog
            connection={props.connection}
            board={props.board}
            runs={props.runs.runs}
            onClose={() => {
              setReporting(false);
            }}
          />
        </Suspense>
      )}
    </>
  );
}

function BoardToolbar(props: {
  board: BoardDto;
  runs: BoardRuns;
  onSave: (board: Board) => void;
  onEdit: (editing: Editing) => void;
  onDelete: () => void;
  connection: Connection;
}): React.JSX.Element {
  const { board, runs } = props;
  const definition = board.board;
  return (
    <div className={styles.toolbar}>
      <TextField
        aria-label={t('boardName')}
        controlSize="compact"
        defaultValue={definition.name}
        onBlur={(event) => {
          const name = event.target.value.trim();
          if (name && name !== definition.name)
            props.onSave({ ...definition, name });
        }}
      />
      <SelectField
        aria-label={t('refreshInterval')}
        controlSize="compact"
        value={String(definition.refreshSecs ?? 0)}
        options={REFRESH}
        onValueChange={(value) => {
          props.onSave({ ...definition, refreshSecs: Number(value) || null });
        }}
      />
      <Button
        size="sm"
        variant="primary"
        disabled={!board.reviewed || !definition.widgets.length}
        onClick={runs.run}
      >
        {t('run')}
      </Button>
      {runs.live && (
        <Button size="sm" onClick={runs.stop}>
          {t('stopRefresh')}
        </Button>
      )}
      <span className={styles.spacer} />
      <Button
        size="sm"
        variant="ghost"
        onClick={() => {
          props.onEdit({ kind: 'params' });
        }}
      >
        {t('parameters')}
      </Button>
      {definition.widgets.length > 0 && (
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            props.onEdit({
              kind: 'widget',
              widget: newWidget(definition.widgets),
            });
          }}
        >
          {t('addWidget')}
        </Button>
      )}
      <BoardReportButton
        connection={props.connection}
        board={board}
        runs={runs}
      />
      <Button size="sm" variant="ghost" onClick={props.onDelete}>
        {t('deleteBoard')}
      </Button>
    </div>
  );
}

function WidgetGrid(props: {
  board: Board;
  runs: BoardRuns;
  onEdit: (widget: Widget) => void;
  onSave: (board: Board) => void;
}): React.JSX.Element {
  const { board } = props;
  return (
    <div className={styles.grid}>
      {board.widgets.map((widget) => (
        <WidgetCard
          key={widget.id}
          widget={widget}
          run={props.runs.runs[widget.id]}
          onEdit={() => {
            props.onEdit(widget);
          }}
          onRemove={() => {
            const widgets = board.widgets.filter((w) => w.id !== widget.id);
            props.onSave({ ...board, widgets });
          }}
        />
      ))}
    </div>
  );
}

function Editors(props: {
  board: Board;
  editing: Editing;
  onSave: (board: Board) => void;
  onCancel: () => void;
}): React.JSX.Element | null {
  const { board, editing } = props;
  if (editing?.kind === 'params')
    return (
      <ParamsEditor
        params={board.params}
        onSave={(params) => {
          props.onSave({ ...board, params });
        }}
      />
    );
  if (editing?.kind === 'widget')
    return (
      <WidgetEditor
        widget={editing.widget}
        onSave={(widget) => {
          props.onSave(withWidget(board, widget));
        }}
        onCancel={props.onCancel}
      />
    );
  return null;
}

function withWidget(board: Board, widget: Widget): Board {
  const exists = board.widgets.some((w) => w.id === widget.id);
  return {
    ...board,
    widgets: exists
      ? board.widgets.map((w) => (w.id === widget.id ? widget : w))
      : [...board.widgets, widget],
  };
}

async function remove(boards: Boards, board: BoardDto): Promise<void> {
  const confirmed = await confirmDestructive({
    title: t('deleteBoardName', { name: board.board.name }),
    message: t('boardWidgetDefinitions'),
    action: t('delete'),
  });
  if (confirmed !== false) await boards.remove(board.id);
}

/** The selected board: parameters, explicit/interval refresh and widgets. */
export function BoardView(props: {
  board: BoardDto;
  boards: Boards;
  target: MonitorTarget;
  connection: Connection;
  active: boolean;
}): React.JSX.Element {
  const { board, boards } = props;
  const definition = board.board;
  const [entered, setEntered] = useState<Record<string, string>>({});
  const [editing, setEditing] = useState<Editing>(null);
  const values = useMemo(
    () =>
      definition.params.map((param) => ({
        name: param.name,
        value: entered[param.name] ?? null,
      })),
    [definition.params, entered],
  );
  const runs = useBoardRuns(board, props.target, values, props.active);
  const save = (next: Board): void => {
    setEditing(null);
    void boards.save(next, board.id);
  };
  const empty = definition.widgets.length === 0 && editing === null;
  return (
    <>
      <BoardToolbar
        board={board}
        runs={runs}
        onSave={save}
        onEdit={setEditing}
        onDelete={() => void remove(boards, board)}
        connection={props.connection}
      />
      {!board.reviewed && (
        <Review board={board} onApprove={() => void boards.approve(board.id)} />
      )}
      <RunNotes runs={runs} seconds={definition.refreshSecs} />
      <ParamInputs
        params={definition.params}
        values={entered}
        onChange={(name, value) => {
          setEntered({ ...entered, [name]: value });
        }}
      />
      <Editors
        board={definition}
        editing={editing}
        onSave={save}
        onCancel={() => {
          setEditing(null);
        }}
      />
      {empty ? (
        <EmptyBoard
          onAdd={() => {
            setEditing({ kind: 'widget', widget: newWidget([]) });
          }}
        />
      ) : (
        <WidgetGrid
          board={definition}
          runs={runs}
          onEdit={(widget) => {
            setEditing({ kind: 'widget', widget });
          }}
          onSave={save}
        />
      )}
    </>
  );
}
