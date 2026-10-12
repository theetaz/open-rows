// SPDX-License-Identifier: Apache-2.0
import { useMemo, useState } from 'react';
import { Button, SelectField, TextField } from '@openrows/ui';
import { confirmDestructive } from '@openrows/ipc';
import type { Board, BoardDto, MonitorTarget, Widget } from '@openrows/ipc';
import { ParamInputs, ParamsEditor } from './BoardParams';
import { WidgetCard } from './WidgetCard';
import { WidgetEditor, newWidget } from './WidgetEditor';
import { useBoardRuns } from './useBoardRuns';
import type { BoardRuns } from './useBoardRuns';
import type { Boards } from './useBoards';
import styles from './Boards.module.css';

const REFRESH = [
  { value: '0', label: 'Manual' },
  { value: '30', label: 'Every 30 s' },
  { value: '60', label: 'Every 1 min' },
  { value: '300', label: 'Every 5 min' },
  { value: '900', label: 'Every 15 min' },
] as const;

type Editing = { kind: 'widget'; widget: Widget } | { kind: 'params' } | null;

/** An imported board: every query shown, nothing runs until approved. */
function Review(props: {
  board: BoardDto;
  onApprove: () => void;
}): React.JSX.Element {
  return (
    <section className={styles.review} aria-label="Review imported board">
      <strong>Imported board — paused.</strong>
      <span>
        Review every query below. Nothing runs until you approve this board.
      </span>
      {props.board.board.widgets.map((widget) => (
        <div key={widget.id}>
          <div>{widget.title}</div>
          <pre className={styles.sql}>{widget.sql}</pre>
        </div>
      ))}
      <div>
        <Button size="sm" variant="primary" onClick={props.onApprove}>
          Approve and enable
        </Button>
      </div>
    </section>
  );
}

function BoardToolbar(props: {
  board: BoardDto;
  runs: BoardRuns;
  onSave: (board: Board) => void;
  onEdit: (editing: Editing) => void;
  onDelete: () => void;
}): React.JSX.Element {
  const { board, runs } = props;
  const definition = board.board;
  return (
    <div className={styles.toolbar}>
      <TextField
        aria-label="Board name"
        controlSize="compact"
        defaultValue={definition.name}
        onBlur={(event) => {
          const name = event.target.value.trim();
          if (name && name !== definition.name)
            props.onSave({ ...definition, name });
        }}
      />
      <SelectField
        aria-label="Refresh interval"
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
        Run
      </Button>
      {runs.live && (
        <Button size="sm" onClick={runs.stop}>
          Stop refresh
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
        Parameters…
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
          Add widget
        </Button>
      )}
      <Button size="sm" variant="ghost" onClick={props.onDelete}>
        Delete board
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
    title: `Delete board ${board.board.name}?`,
    message:
      'The board and its widget definitions are removed from this device.',
    action: 'Delete',
  });
  if (confirmed !== false) await boards.remove(board.id);
}

function RunNotes(props: {
  runs: BoardRuns;
  seconds: number | null;
}): React.JSX.Element {
  return (
    <>
      {props.runs.note && <p className={styles.note}>{props.runs.note}</p>}
      {props.runs.live && (
        <p className={styles.note}>
          Refreshing every {String(props.seconds)} s while this board is open.
        </p>
      )}
    </>
  );
}

/** An empty board offers exactly one action. */
function EmptyBoard(props: { onAdd: () => void }): React.JSX.Element {
  return (
    <div className={styles.empty}>
      <p>This board has no widgets.</p>
      <Button size="sm" variant="primary" onClick={props.onAdd}>
        Add widget
      </Button>
    </div>
  );
}

/** The selected board: parameters, explicit/interval refresh and widgets. */
export function BoardView(props: {
  board: BoardDto;
  boards: Boards;
  target: MonitorTarget;
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
