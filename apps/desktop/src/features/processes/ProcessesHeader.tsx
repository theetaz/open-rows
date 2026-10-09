// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton, SegmentedControl, TextField } from '@openrows/ui';
import { REFRESH_INTERVALS } from './processModel';
import type { RefreshInterval } from './processModel';
import type { Processes } from './useProcesses';
import structure from '../table/structure/Structure.module.css';
import styles from './Processes.module.css';

const INTERVAL_OPTIONS = REFRESH_INTERVALS.map((value) => ({
  value,
  label: value === '0' ? 'Off' : `${value} s`,
}));

/** `Filter… · Auto-refresh Off|2 s|5 s|10 s · ⟳ · n of m processes` above the list. */
export function ProcessesHeader(props: {
  list: Processes;
  filter: string;
  onFilter: (filter: string) => void;
  shown: number;
}): React.JSX.Element {
  const { list } = props;
  return (
    <div className={structure.header}>
      <TextField
        className={structure.search}
        controlSize="compact"
        type="search"
        aria-label="Filter processes"
        placeholder="Filter processes…"
        leadingIcon={<Icon name="filter" />}
        value={props.filter}
        onChange={(event) => {
          props.onFilter(event.target.value);
        }}
      />
      <span>Auto-refresh</span>
      <SegmentedControl<RefreshInterval>
        aria-label="Auto-refresh"
        size="compact"
        options={INTERVAL_OPTIONS}
        value={list.interval}
        onValueChange={list.setInterval}
      />
      <IconButton
        aria-label="Refresh processes"
        title="Refresh processes"
        size="sm"
        variant="ghost"
        onClick={list.reload}
      >
        <Icon name="refresh" />
      </IconButton>
      <span className={styles.count} role="status">
        {list.loading
          ? 'Loading processes…'
          : `${String(props.shown)} of ${String(list.processes.length)} processes`}
      </span>
    </div>
  );
}
