// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SelectField, TextArea } from '@openrows/ui';
import { exportBoard } from '@openrows/ipc';
import type { Board, MonitorTarget } from '@openrows/ipc';
import { BoardView } from './BoardView';
import { useBoards } from './useBoards';
import styles from './Boards.module.css';

/** Mirrors the native import limit so oversize files fail before IPC. */
const MAX_IMPORT_BYTES = 256 * 1024;

const EMPTY: Board = {
  version: 1,
  name: 'Untitled board',
  params: [],
  widgets: [],
  refreshSecs: null,
};

type Mode = 'idle' | 'import' | 'export';

/** Monitor ▸ Boards: local per-profile analytics boards (docs/28 §5). */
export function BoardsPanel(props: {
  target: MonitorTarget;
  active: boolean;
}): React.JSX.Element {
  const boards = useBoards(props.target.profileId, props.active);
  const [mode, setMode] = useState<Mode>('idle');
  const [exported, setExported] = useState('');
  const { selected } = boards;
  const create = (): void => {
    void boards.save(EMPTY, null);
  };
  const openExport = (): void => {
    if (!selected) return;
    exportBoard(props.target.profileId, selected.id).then(
      (text) => {
        setExported(text);
        setMode('export');
      },
      () => {
        setExported('');
      },
    );
  };
  return (
    <section className={styles.boards} aria-label="Boards">
      <BoardsToolbar
        boards={boards}
        onCreate={create}
        onImport={() => {
          setMode(mode === 'import' ? 'idle' : 'import');
        }}
        onExport={openExport}
      />
      {boards.error && (
        <p role="alert" className={styles.error}>
          {boards.error}
        </p>
      )}
      {mode === 'import' && (
        <ImportPanel
          onImport={(text) => {
            void boards.importText(text).then(() => {
              setMode('idle');
            });
          }}
          onCancel={() => {
            setMode('idle');
          }}
        />
      )}
      {mode === 'export' && (
        <ExportPanel
          text={exported}
          onClose={() => {
            setMode('idle');
          }}
        />
      )}
      {selected ? (
        <BoardView
          key={selected.id}
          board={selected}
          boards={boards}
          target={props.target}
          active={props.active}
        />
      ) : (
        <NoBoards onCreate={create} />
      )}
    </section>
  );
}

function BoardsToolbar(props: {
  boards: ReturnType<typeof useBoards>;
  onCreate: () => void;
  onImport: () => void;
  onExport: () => void;
}): React.JSX.Element {
  const { boards } = props;
  const { selected } = boards;
  return (
    <div className={styles.toolbar}>
      {selected && (
        <SelectField
          aria-label="Board"
          controlSize="compact"
          value={selected.id}
          options={boards.boards.map((board) => ({
            value: board.id,
            label: board.reviewed
              ? board.board.name
              : `${board.board.name} (paused)`,
          }))}
          onValueChange={boards.select}
        />
      )}
      <Button size="sm" onClick={props.onCreate}>
        New board
      </Button>
      <Button size="sm" onClick={props.onImport}>
        Import…
      </Button>
      {selected && (
        <Button size="sm" onClick={props.onExport}>
          Export…
        </Button>
      )}
    </div>
  );
}

function NoBoards(props: { onCreate: () => void }): React.JSX.Element {
  return (
    <div className={styles.empty}>
      <p>No boards for this connection yet.</p>
      <Button size="sm" variant="primary" onClick={props.onCreate}>
        New board
      </Button>
    </div>
  );
}

/** Paste or choose a board file; importing never runs its queries. */
function ImportPanel(props: {
  onImport: (text: string) => void;
  onCancel: () => void;
}): React.JSX.Element {
  const [text, setText] = useState('');
  const [error, setError] = useState<string | null>(null);
  const choose = (file: File | undefined): void => {
    if (!file) return;
    if (file.size > MAX_IMPORT_BYTES) {
      setError('Board files are limited to 256 KiB.');
      return;
    }
    void file.text().then((content) => {
      setError(null);
      setText(content);
    });
  };
  return (
    <section className={styles.editor} aria-label="Import board">
      <p className={styles.note}>
        Imported boards stay paused: review every query and approve the board
        before anything runs.
      </p>
      <input
        type="file"
        accept=".json,application/json"
        aria-label="Board file"
        onChange={(event) => {
          choose(event.target.files?.[0]);
        }}
      />
      <TextArea
        aria-label="Board JSON"
        spellCheck={false}
        value={text}
        onChange={(event) => {
          setText(event.target.value);
        }}
      />
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <div className={styles.row}>
        <Button
          size="sm"
          variant="primary"
          disabled={!text.trim() || text.length > MAX_IMPORT_BYTES}
          onClick={() => {
            props.onImport(text);
          }}
        >
          Import paused
        </Button>
        <Button size="sm" onClick={props.onCancel}>
          Cancel
        </Button>
      </div>
    </section>
  );
}

/** Export preview: the exact definitions, including SQL literals; no results. */
function ExportPanel(props: {
  text: string;
  onClose: () => void;
}): React.JSX.Element {
  return (
    <section className={styles.editor} aria-label="Export board">
      <p className={styles.note}>
        The file holds widget SQL exactly as written, which may contain
        sensitive literals. It holds no results, credentials or connection
        details.
      </p>
      <pre className={styles.sql}>{props.text}</pre>
      <div className={styles.row}>
        <Button
          size="sm"
          variant="primary"
          onClick={() => {
            void navigator.clipboard.writeText(props.text);
          }}
        >
          Copy JSON
        </Button>
        <Button size="sm" onClick={props.onClose}>
          Close
        </Button>
      </div>
    </section>
  );
}
