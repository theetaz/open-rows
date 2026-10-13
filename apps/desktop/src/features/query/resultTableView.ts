// SPDX-License-Identifier: Apache-2.0
import type { TableView } from '../table/model';
import { initialView } from '../table/model';
import type { ResultMeta } from './run';
import type { ResultView } from './useResultView';
import type { ColumnFilter } from './columnFilters';
import { columnKeys } from './resultColumns';

/** Translate the table menu's view actions into client-side result operations. */
export function changeResultView(
  args: {
    result: ResultMeta;
    view: ResultView;
    hidden: readonly string[];
    setHidden: (hidden: readonly string[]) => void;
  },
  update: (view: TableView) => TableView,
): void {
  const { result, view } = args;
  const keys = columnKeys(result.columns);
  const sort = view.sorted
    ? {
        column: keys[view.sorted.column] ?? '',
        descending: view.sorted.descending,
      }
    : null;
  const current = { ...initialView, sort, hidden: args.hidden };
  const next = update(current);
  if (next.hidden !== current.hidden) args.setHidden(next.hidden);
  if (next.sort !== current.sort) {
    const column = keys.indexOf(next.sort?.column ?? '');
    view.sortBy(
      next.sort && column >= 0
        ? { column, descending: next.sort.descending }
        : null,
    );
  }
  for (const filter of next.applied) {
    const column = keys.indexOf(filter.column);
    const operator = filter.operator;
    if (column < 0) continue;
    if (operator === 'isEmpty')
      view.filterColumn({ column, operator: 'equal', value: '' });
    else if (
      ['contains', 'equal', 'notEqual', 'isNull', 'isNotNull'].includes(
        operator,
      )
    )
      view.filterColumn({
        column,
        operator: operator as ColumnFilter['operator'],
        value: filter.value,
      });
  }
}
