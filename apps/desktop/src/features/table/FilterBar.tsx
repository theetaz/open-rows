// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { CombinatorDto, EngineDto, TableQueryDto } from '@openrows/ipc';
import { tableSql } from '@openrows/ipc';
import { Button, SelectField } from '@openrows/ui';
import { FilterGroups } from './filters/FilterGroups';
import { appliedGroups } from './filters/groups';
import { SavedFilters } from './filters/SavedFilters';
import { WherePreview } from './filters/WherePreview';
import { withParams } from './filters/whereClause';
import { toFilters } from './model';
import type { TableView } from './model';
import type { TableActions } from './useTableTab';
import styles from './Table.module.css';

type Props = {
  view: TableView;
  columns: readonly string[];
  /** Database type of each column: the operators a row offers. */
  types: ReadonlyMap<string, string>;
  engine: EngineDto;
  /** The request of the shown page; the preview swaps in the edited filters. */
  query: TableQueryDto;
  actions: TableActions;
  /** The table's presentation key, for its saved filters. */
  storageKey: string;
};

const combinators: readonly { value: CombinatorDto; label: string }[] = [
  { value: 'and', label: 'AND' },
  { value: 'or', label: 'OR' },
];

/** The full generated SELECT of the draft filters, or why it cannot be built. */
function useSqlPreview(
  draft: TableQueryDto,
  view: TableView,
): [string | null, () => void] {
  const [preview, setPreview] = useState<string | null>(null);
  const toggle = (): void => {
    if (preview !== null) {
      setPreview(null);
      return;
    }
    tableSql(draft, view.offset, view.limit).then(
      (reply) => {
        setPreview(withParams(reply.sql, reply.params));
      },
      (error: unknown) => {
        setPreview(
          `-- ${error instanceof Error ? error.message : String(error)}`,
        );
      },
    );
  };
  return [preview, toggle];
}

/**
 * Filter rows (typed operators, Raw SQL with a live WHERE check), saved
 * filters, SQL preview, AND/OR, Clear and Apply All (FR-GRID-04/11/12).
 */
export function FilterBar(props: Props): React.JSX.Element {
  const { view, query, actions } = props;
  const draft = {
    ...query,
    combinator: view.combinator,
    filters: view.groups ? [] : toFilters(view.rows),
    groups: view.groups ? appliedGroups(view) : [],
  };
  const [preview, togglePreview] = useSqlPreview(draft, view);
  const raw = view.rows.some((row) => row.enabled && row.operator === 'raw');
  return (
    <div className={styles.filters} role="search" aria-label="Filters">
      <FilterGroups {...props} />
      {raw && <WherePreview draft={draft} />}
      <div className={styles.filterActions}>
        <Button
          size="sm"
          onClick={() => {
            actions.addRow();
          }}
        >
          Add filter
        </Button>
        <Button size="sm" onClick={actions.addGroup}>
          Add group
        </Button>
        <Button
          size="sm"
          aria-pressed={preview !== null}
          onClick={togglePreview}
        >
          SQL
        </Button>
        <SelectField<CombinatorDto>
          aria-label="Combine groups"
          controlSize="compact"
          value={view.combinator}
          options={combinators}
          onValueChange={actions.combine}
        />
        <SavedFilters
          storageKey={props.storageKey}
          view={view}
          onApply={actions.applySaved}
        />
        <span className={styles.spacer} />
        <Button size="sm" variant="ghost" onClick={actions.clear}>
          Clear
        </Button>
        <Button
          size="sm"
          variant="primary"
          onClick={() => {
            actions.apply();
          }}
        >
          Apply All
        </Button>
      </div>
      {preview !== null && (
        <pre className={styles.preview} aria-label="Generated SQL">
          {preview}
        </pre>
      )}
    </div>
  );
}
