// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, IconButton, SelectField } from '@openrows/ui';
import { FilterRowEditor } from '../FilterRowEditor';
import type { FilterRow } from '../model';
import { groupsOf } from './groups';
import type { TableView } from '../model';
import type { TableActions } from '../useTableTab';
import styles from './Filters.module.css';
import { t } from '../messages/lazyData';

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
          className={styles.group}
          aria-label={t('filterGroupValue', { value: String(index + 1) })}
        >
          <legend className={styles.legend}>
            {t('groupValue', { value: index + 1 })}
          </legend>
          <div className={styles.head}>
            <span className={styles.title}>
              {t('groupValue', { value: index + 1 })}
            </span>
            <SelectField
              aria-label={t('combineRowsGroup', { value: String(index + 1) })}
              controlSize="compact"
              value={group.combinator}
              options={[
                { value: 'and', label: t('and') },
                { value: 'or', label: t('or') },
              ]}
              onValueChange={(value) => {
                props.actions.combineGroup(group.id, value);
              }}
            />
            <span className={styles.spacer} />
            {groups.length > 1 && (
              <IconButton
                size="sm"
                variant="ghost"
                aria-label={t('removeGroupValue', { value: index + 1 })}
                title={t('removeGroupValue', { value: index + 1 })}
                onClick={() => {
                  props.actions.removeGroup(group.id);
                }}
              >
                <Icon name="close" />
              </IconButton>
            )}
          </div>
          <GroupRows {...props} id={group.id} />
          <Button
            size="sm"
            variant="ghost"
            className={styles.add}
            onClick={() => {
              props.actions.addRow(undefined, group.id);
            }}
          >
            <Icon name="plus" />
            {t('addFilterGroup', { value: index + 1 })}
          </Button>
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
