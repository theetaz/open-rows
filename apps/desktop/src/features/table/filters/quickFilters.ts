// SPDX-License-Identifier: Apache-2.0
import type { EngineDto, FilterOperatorDto } from '@openrows/ipc';
import { operatorsFor } from '../filterOperators';
import { filterRow } from '../model';
import type { TableView } from '../model';
import type { TableEntry } from '../entries';
import { withQuickFilter } from './groups';
import type { QuickOperator } from './QuickFilter';

/**
 * Header quick-filter operators for a column type: the filter bar's typed
 * operators with at most one value, `contains` first where text matches.
 */
export function quickOperators(
  dataType: string | null,
  engine: EngineDto,
): QuickOperator<FilterOperatorDto>[] {
  const offered = operatorsFor(dataType, engine).flatMap((item) =>
    item.value === 'raw' || item.arity === 2
      ? []
      : [{ value: item.value, label: item.label, arity: item.arity }],
  );
  const contains = offered.filter((item) => item.value === 'contains');
  return [...contains, ...offered.filter((item) => item.value !== 'contains')];
}

/** Quick-filter operators of a table column, typed once the structure has loaded. */
export function columnQuickOperators(
  structure: TableEntry['structure'],
  column: string,
  engine: EngineDto,
): QuickOperator<FilterOperatorDto>[] {
  const type =
    structure.status === 'ready'
      ? structure.data.columns.find((item) => item.name === column)?.dataType
      : undefined;
  return quickOperators(type ?? null, engine);
}

/** Apply one header quick filter on `column`, keeping the applied filters. */
export function quickFiltered(
  view: TableView,
  column: string,
  filter: { operator: FilterOperatorDto; value: string },
): TableView {
  return withQuickFilter(view, {
    ...filterRow(view.nextRow, column),
    ...filter,
  });
}
