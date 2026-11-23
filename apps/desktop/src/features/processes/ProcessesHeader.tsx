// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton, SegmentedControl, TextField } from '@openrows/ui';
import { REFRESH_INTERVALS } from './processModel';
import type { RefreshInterval } from './processModel';
import type { Processes } from './useProcesses';
import view from '../table/ViewBar.module.css';
import styles from './Processes.module.css';
import { t } from './messages';

const INTERVAL_OPTIONS = REFRESH_INTERVALS.map((value) => ({
  value,
  label: value === '0' ? t('off') : t('valueS', { value }),
}));

/**
 * The process list's view bar (the kit's `.viewbar`): `Processes on db ·
 * n of m`, a filter field that grows, then Auto-refresh Off|2 s|5 s|10 s and ⟳.
 */
export function ProcessesHeader(props: {
  list: Processes;
  database: string;
  filter: string;
  onFilter: (filter: string) => void;
  shown: number;
}): React.JSX.Element {
  const { list } = props;
  return (
    <div className={view.bar}>
      <strong>{t('processesDatabase', { database: props.database })}</strong>
      <span className={styles.count} role="status">
        {list.loading
          ? t('loadingProcesses')
          : t('shownCountProcesses', {
              shown: String(props.shown),
              count: String(list.processes.length),
            })}
      </span>
      <div className={view.query}>
        <TextField
          controlSize="compact"
          type="search"
          aria-label={t('filterProcesses')}
          placeholder={t('filterProcesses2')}
          leadingIcon={<Icon name="filter" />}
          value={props.filter}
          onChange={(event) => {
            props.onFilter(event.target.value);
          }}
        />
      </div>
      <span className={styles.label}>{t('autoRefresh')}</span>
      <SegmentedControl<RefreshInterval>
        aria-label={t('autoRefresh')}
        size="compact"
        options={INTERVAL_OPTIONS}
        value={list.interval}
        onValueChange={list.setInterval}
      />
      <IconButton
        aria-label={t('refreshProcesses')}
        title={t('refreshProcesses')}
        size="sm"
        variant="ghost"
        onClick={list.reload}
      >
        <Icon name="refresh" />
      </IconButton>
    </div>
  );
}
