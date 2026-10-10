// SPDX-License-Identifier: Apache-2.0
import type { FilterGroupDto } from '@openrows/ipc';
import { filterRow, toFilters } from '../model';
import type { FilterGroup, FilterRow, TableView } from '../model';

/** Legacy rows form one group using the old join. */
export function groupsOf(
  view: Pick<TableView, 'groups' | 'combinator'>,
): readonly FilterGroup[] {
  return view.groups ?? [{ id: 1, combinator: view.combinator }];
}

/** Enabled rows grouped for the parameterised SQL generator. */
export function appliedGroups(
  view: TableView,
  only?: number,
): FilterGroupDto[] {
  return groupsOf(view)
    .map((group) => ({
      combinator: group.combinator,
      filters: toFilters(
        view.rows.filter((row) => (row.group ?? 1) === group.id),
        only,
      ),
    }))
    .filter((group) => group.filters.length > 0);
}

/** Add a group with one row; ids stay stable as other groups are removed. */
export function withNewGroup(view: TableView, column: string): TableView {
  const groups = groupsOf(view);
  const id = Math.max(...groups.map((group) => group.id)) + 1;
  return {
    ...view,
    groups: [...groups, { id, combinator: 'and' }],
    rows: [...view.rows, { ...filterRow(view.nextRow, column), group: id }],
    nextRow: view.nextRow + 1,
    filtersOpen: true,
  };
}

/** Quick filters preserve applied groups without applying unrelated draft edits. */
export function withQuickFilter(view: TableView, row: FilterRow): TableView {
  const groups = groupsOf(view);
  const id = Math.max(...groups.map((group) => group.id)) + 1;
  const filters = toFilters([row]);
  return {
    ...view,
    rows: [...view.rows, view.groups ? { ...row, group: id } : row],
    applied: [...view.applied, ...filters],
    ...(view.groups
      ? {
          groups: [...groups, { id, combinator: 'and' as const }],
          appliedGroups: [
            ...(view.appliedGroups ?? [
              { combinator: view.combinator, filters: [...view.applied] },
            ]),
            { combinator: 'and' as const, filters },
          ],
        }
      : {}),
    nextRow: view.nextRow + 1,
    filtersOpen: true,
    offset: 0,
  };
}
