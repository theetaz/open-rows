// SPDX-License-Identifier: Apache-2.0
import { Button, SelectField } from '@openrows/ui';
import { FilterRowEditor } from '../FilterRowEditor';
import type { FilterRow } from '../model';
import { groupsOf } from './groups';
import type { TableView } from '../model';
import type { TableActions } from '../useTableTab';

/** Parenthesised groups with independent row joins and stable row identities. */
export function FilterGroups(
  props: Omit<Parameters<typeof FilterRowEditor>[0], 'row' | 'index'> & {
    view: TableView;
    actions: TableActions;
  },
): React.JSX.Element {
  const groups = groupsOf(props.view);
  return (
    <>
      {groups.map((group, index) => (
        <fieldset
          key={group.id}
          aria-label={`Filter group ${String(index + 1)}`}
        >
          <legend>Group {index + 1}</legend>
          <SelectField
            aria-label={`Combine rows in group ${String(index + 1)}`}
            controlSize="compact"
            value={group.combinator}
            options={[
              { value: 'and', label: 'AND' },
              { value: 'or', label: 'OR' },
            ]}
            onValueChange={(value) => {
              props.actions.combineGroup(group.id, value);
            }}
          />
          <GroupRows {...props} id={group.id} />
          <Button
            size="sm"
            onClick={() => {
              props.actions.addRow(undefined, group.id);
            }}
          >
            Add filter to group {index + 1}
          </Button>
          {groups.length > 1 && (
            <Button
              size="sm"
              onClick={() => {
                props.actions.removeGroup(group.id);
              }}
            >
              Remove group {index + 1}
            </Button>
          )}
        </fieldset>
      ))}
    </>
  );
}

function GroupRows(
  props: Parameters<typeof FilterGroups>[0] & { id: number },
): React.JSX.Element {
  return (
    <>
      {props.view.rows.flatMap((row: FilterRow, index) =>
        (row.group ?? 1) === props.id
          ? [
              <FilterRowEditor
                {...props}
                key={row.id}
                row={row}
                index={index}
              />,
            ]
          : [],
      )}
    </>
  );
}
