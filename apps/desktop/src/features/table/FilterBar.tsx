// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { CombinatorDto, TableQueryDto } from '@openrows/ipc';
import { tableSql } from '@openrows/ipc';
import { Button, SelectField } from '@openrows/ui';
import { FilterRowEditor } from './FilterRowEditor';
import { toFilters } from './model';
import type { TableView } from './model';
import type { TableActions } from './useTableTab';
import styles from './Table.module.css';

type Props = {
  view: TableView;
  columns: readonly string[];
  /** The request of the shown page; the preview swaps in the edited filters. */
  query: TableQueryDto;
  actions: TableActions;
};

const combinators: readonly { value: CombinatorDto; label: string }[] = [
  { value: 'and', label: 'AND' },
  { value: 'or', label: 'OR' },
];

/** The generated SELECT with its bind parameters listed as comments. */
function previewText(sql: string, params: readonly (string | null)[]): string {
  const values = params.map(
    (value, index) =>
      `-- $${String(index + 1)} = ${value === null ? 'NULL' : JSON.stringify(value)}`,
  );
  return [sql, ...values].join('\n');
}

/** Filter rows plus SQL preview, AND/OR, Clear and Apply All (FR-GRID-04/11). */
export function FilterBar({
  view,
  columns,
  query,
  actions,
}: Props): React.JSX.Element {
  const [preview, setPreview] = useState<string | null>(null);
  const showSql = (): void => {
    if (preview !== null) {
      setPreview(null);
      return;
    }
    const draft = { ...query, filters: toFilters(view.rows) };
    tableSql(draft, view.offset, view.limit).then(
      (reply) => {
        setPreview(previewText(reply.sql, reply.params));
      },
      (error: unknown) => {
        setPreview(
          `-- ${error instanceof Error ? error.message : String(error)}`,
        );
      },
    );
  };
  return (
    <div className={styles.filters} role="search" aria-label="Filters">
      {view.rows.map((row, index) => (
        <FilterRowEditor
          key={row.id}
          row={row}
          index={index}
          columns={columns}
          actions={actions}
        />
      ))}
      <div className={styles.filterActions}>
        <Button
          size="sm"
          onClick={() => {
            actions.addRow();
          }}
        >
          Add filter
        </Button>
        <Button size="sm" aria-pressed={preview !== null} onClick={showSql}>
          SQL
        </Button>
        <SelectField<CombinatorDto>
          aria-label="Combine filters"
          controlSize="compact"
          value={view.combinator}
          options={combinators}
          onValueChange={actions.combine}
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
