// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { ProcessDto, ProfileDto } from '@openrows/ipc';
import type { ShowRow } from '../table';
import type { Cursor } from '../table/structure/StructureGrid';
import {
  DEFAULT_SORT,
  filterProcesses,
  nextSort,
  processFields,
  processLabel,
  sortProcesses,
} from './processModel';
import { useProcesses } from './useProcesses';
import { useProcessSignals } from './processSignals';
import { ProcessesHeader } from './ProcessesHeader';
import { ProcessGrid } from './ProcessGrid';
import { ProcessSignalSheet } from './ProcessSignalSheet';
import structure from '../table/structure/Structure.module.css';

type Props = {
  profile: ProfileDto;
  database: string;
  /** The connection is up; the list reads and refreshes only then. */
  connected: boolean;
  /** Shows the selected process, with its full query, in the inspector. */
  onRow: ShowRow;
};

/** Publish the selected process to the inspector whenever it changes. */
function useInspected(process: ProcessDto | undefined, onRow: ShowRow): void {
  const latest = useRef(onRow);
  useEffect(() => {
    latest.current = onRow;
  });
  useEffect(() => {
    if (process)
      latest.current({
        table: processLabel(process),
        number: 1,
        fields: processFields(process),
      });
  }, [process]);
}

/**
 * The server's process list (FR-ADMIN-01): filter, sort, auto-refresh, the
 * selected process in the inspector, and Copy Query, Cancel Query and Kill
 * Process (confirmed) on right-click.
 */
export function ProcessesTab(props: Props): React.JSX.Element {
  const { profile, database } = props;
  const list = useProcesses(profile.id, database, props.connected);
  const signals = useProcessSignals({
    profileId: profile.id,
    database,
    readOnly: profile.safety === 'readOnly',
    onSignalled: list.reload,
  });
  const [filter, setFilter] = useState('');
  const [sort, setSort] = useState(DEFAULT_SORT);
  const [selected, setSelected] = useState<{ id: number; col: number }>({
    id: -1,
    col: 0,
  });
  const shown = sortProcesses(filterProcesses(list.processes, filter), sort);
  // Selection follows the process across refreshes and re-sorting.
  const row = Math.max(
    0,
    shown.findIndex((process) => process.id === selected.id),
  );
  useInspected(shown[row], props.onRow);
  const error = signals.error ?? list.error;
  return (
    <section className={structure.pane} aria-label={`Processes on ${database}`}>
      <ProcessesHeader
        list={list}
        filter={filter}
        onFilter={setFilter}
        shown={shown.length}
      />
      {!props.connected && (
        <p className={structure.note}>
          Connect to list the server’s processes.
        </p>
      )}
      {error && (
        <p className={structure.error} role="alert">
          {error}
        </p>
      )}
      <ProcessGrid
        processes={shown}
        cursor={{ row, col: selected.col }}
        onCursor={(cursor: Cursor) => {
          setSelected({ id: shown[cursor.row]?.id ?? -1, col: cursor.col });
        }}
        sort={sort}
        onSort={(col) => {
          setSort((current) => nextSort(current, col));
        }}
        readOnly={profile.safety === 'readOnly'}
        onSignal={signals.request}
      />
      {signals.pending && (
        <ProcessSignalSheet
          pending={signals.pending}
          onCancel={signals.dismiss}
          onConfirm={signals.confirm}
        />
      )}
    </section>
  );
}
