// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import { HighlightText, Icon, TreeNode, TreeNote } from '@openrows/ui';
import type { ObjectDto, TableColumnDto } from '@openrows/ipc';
import {
  columnKey,
  formatEstimate,
  hasColumns,
  kindInfo,
  nodeKey,
  objectLabel,
  objectRef,
  substringIndices,
} from './catalog';
import type { ObjectRef } from './catalog';
import type { Catalog } from './useCatalog';
import type { Resource } from './useResources';

/** Shared by every row of one section tree. */
type RowContext = {
  catalog: Catalog;
  search: string;
  /** Selected tree key (object or column), if any. */
  selected: string | null;
  /** Key of the section tree's single Tab stop. */
  tabStop: string | undefined;
  onSelect: (key: string) => void;
  onOpen: (object: ObjectRef) => void;
};

type Disclosure =
  | { expanded: boolean; onExpandedChange: (open: boolean) => void }
  | Record<string, never>;

/** Disclosure of a relation row; its columns are read on first expand. */
function useDisclosure(
  catalog: Catalog,
  object: ObjectDto,
): { disclosure: Disclosure; columns: Resource<TableColumnDto[]> } {
  const expandable = hasColumns(object.kind);
  const expanded = expandable && catalog.expanded(object);
  const columns = catalog.columns(object);
  const { loadColumns, setExpanded } = catalog;
  useEffect(() => {
    if (expanded && columns.status === 'idle') loadColumns(object);
  }, [expanded, columns.status, loadColumns, object]);
  const disclosure: Disclosure = expandable
    ? {
        expanded,
        onExpandedChange: (open) => {
          setExpanded(object, open);
        },
      }
    : {};
  return { disclosure, columns };
}

/**
 * One object row. Tables, views and materialized views expand to list their
 * columns, read on first expand; opening stays on click/double-click/Enter.
 */
export function ObjectRow(
  props: RowContext & { object: ObjectDto },
): React.JSX.Element {
  const { catalog, object, search } = props;
  const label = objectLabel(object);
  const id = nodeKey(object);
  const target = objectRef(catalog.database, object);
  const count = formatEstimate(object.rowEstimate);
  const { glyph } = kindInfo(object.kind);
  const { disclosure, columns } = useDisclosure(catalog, object);
  const open = (): void => {
    props.onOpen(target);
  };
  return (
    <TreeNode
      label={count ? `${label} ${count}` : label}
      level={1}
      selected={props.selected === id}
      tabStop={props.tabStop === id}
      onSelect={() => {
        props.onSelect(id);
      }}
      onOpen={open}
      {...disclosure}
      icon={glyph}
      iconTone={glyph === '▦' ? 'info' : undefined}
      detail={count}
      title={objectTitle(catalog.database, object)}
      name={
        <HighlightText text={label} indices={substringIndices(label, search)} />
      }
    >
      <ColumnRows context={props} parent={id} columns={columns} onOpen={open} />
    </TreeNode>
  );
}

const objectTitle = (database: string, object: ObjectDto): string => {
  const kind = kindInfo(object.kind).label;
  const comment = object.comment ? ` · ${object.comment}` : '';
  return `${database}.${object.schema}.${objectLabel(object)} · ${kind}${comment}`;
};

const columnTitle = (column: TableColumnDto): string =>
  [
    column.name,
    column.dataType,
    column.primaryKey ? 'primary key' : '',
    column.nullable ? '' : 'NOT NULL',
    column.comment ?? '',
  ]
    .filter(Boolean)
    .join(' · ');

/** Loading, error or column rows of an expanded object. */
function ColumnRows(props: {
  context: RowContext;
  parent: string;
  columns: Resource<TableColumnDto[]>;
  onOpen: () => void;
}): React.JSX.Element {
  const { context, columns } = props;
  if (columns.status === 'error')
    return (
      <TreeNote level={2} tone="danger">
        {columns.message}
      </TreeNote>
    );
  if (columns.status !== 'ready')
    return <TreeNote level={2}>Loading columns…</TreeNote>;
  if (!columns.data.length) return <TreeNote level={2}>No columns</TreeNote>;
  return (
    <>
      {columns.data.map((column) => {
        const id = columnKey(props.parent, column.name);
        return (
          <TreeNode
            key={column.name}
            label={`${column.name} ${column.dataType}${column.primaryKey ? ', primary key' : ''}`}
            level={2}
            selected={context.selected === id}
            tabStop={context.tabStop === id}
            onSelect={() => {
              context.onSelect(id);
            }}
            onOpen={props.onOpen}
            icon={column.primaryKey ? <Icon name="key" /> : undefined}
            iconTone="warning"
            detail={column.dataType}
            title={columnTitle(column)}
            name={column.name}
          />
        );
      })}
    </>
  );
}
