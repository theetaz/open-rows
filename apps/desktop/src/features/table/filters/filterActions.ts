// SPDX-License-Identifier: Apache-2.0
import { filterRow, toFilters } from '../model';
import type { FilterRow, TableView } from '../model';
import type { TableActions } from '../useTableTab';
import { appliedGroups, groupsOf, withNewGroup } from './groups';

type Change = (update: (view: TableView) => TableView) => void;
type Actions = Pick<
  TableActions,
  | 'apply'
  | 'clear'
  | 'combine'
  | 'editRow'
  | 'removeRow'
  | 'addRow'
  | 'addGroup'
  | 'combineGroup'
  | 'removeGroup'
  | 'toggleFilters'
>;

/** Insert a row beside its predecessor or into an explicitly chosen group. */
function withRow(
  view: TableView,
  column: string,
  place: { after?: number | undefined; group?: number | undefined },
): TableView {
  const prior = view.rows.find((row) => row.id === place.after);
  const group = place.group ?? prior?.group ?? groupsOf(view)[0]?.id ?? 1;
  const row = { ...filterRow(view.nextRow, column), group };
  const rows = [...view.rows];
  const index = rows.findIndex((item) => item.id === place.after);
  rows.splice(index < 0 ? rows.length : index + 1, 0, row);
  return { ...view, rows, nextRow: view.nextRow + 1, filtersOpen: true };
}

function groupActions(
  change: Change,
  column: string,
): Pick<Actions, 'addGroup' | 'combineGroup' | 'removeGroup'> {
  return {
    addGroup: () => {
      change((view) => withNewGroup(view, column));
    },
    combineGroup: (id, combinator) => {
      change((view) => ({
        ...view,
        groups: groupsOf(view).map((group) =>
          group.id === id ? { ...group, combinator } : group,
        ),
      }));
    },
    removeGroup: (id) => {
      change((view) =>
        groupsOf(view).length < 2
          ? view
          : {
              ...view,
              groups: groupsOf(view).filter((group) => group.id !== id),
              rows: view.rows.filter((row) => (row.group ?? 1) !== id),
            },
      );
    },
  };
}

/** Filter editing and application share one group model across the bar and shortcuts. */
export function filterActions(
  change: Change,
  columns: readonly string[],
): Actions {
  const editRows = (
    update: (rows: readonly FilterRow[]) => FilterRow[],
  ): void => {
    change((view) => ({ ...view, rows: update(view.rows) }));
  };
  return {
    ...groupActions(change, columns[0] ?? ''),
    apply: (only) => {
      change((view) => ({
        ...view,
        applied: toFilters(view.rows, only),
        appliedGroups: view.groups ? appliedGroups(view, only) : undefined,
        offset: 0,
      }));
    },
    clear: () => {
      change((view) => ({
        ...view,
        rows: [],
        applied: [],
        groups: undefined,
        appliedGroups: undefined,
        search: '',
        searchReset: (view.searchReset ?? 0) + 1,
        offset: 0,
      }));
    },
    combine: (combinator) => {
      change((view) => ({ ...view, combinator }));
    },
    editRow: (id, patch) => {
      editRows((rows) =>
        rows.map((row) => (row.id === id ? { ...row, ...patch } : row)),
      );
    },
    removeRow: (id) => {
      editRows((rows) => rows.filter((row) => row.id !== id));
    },
    addRow: (after, group) => {
      change((view) => withRow(view, columns[0] ?? '', { after, group }));
    },
    toggleFilters: () => {
      change((view) =>
        !view.filtersOpen && !view.rows.length
          ? withRow(view, columns[0] ?? '', {})
          : { ...view, filtersOpen: !view.filtersOpen },
      );
    },
  };
}
