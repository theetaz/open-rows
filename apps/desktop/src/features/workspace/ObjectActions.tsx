// SPDX-License-Identifier: Apache-2.0
import { Icon } from '@openrows/ui';
import { useState } from 'react';
import type { IconName, MenuEntry } from '@openrows/ui';
import type { EngineDto, ObjectFeaturesDto } from '@openrows/ipc';
import { tableName } from '../../results/copy/sql';
import { useFavourite } from '../schema/FavouriteButton';
import { ContextMenu } from '../../shell/ContextMenu';
import { t as shell } from '../../shell/messages';
import { ExportSheet, tableExport } from '../export';
import { objectEntries } from '../objects';
import type { ObjectEditorRequest } from '../objects';
import type { MenuPoint, ObjectRef } from '../schema';
import { editorSections, hasUnsupported } from './editorMenu';
import { t } from './messages';

/** The open session of a database of the active connection, if any. */
export type SessionLookup = (
  database: string,
) => { profileId: string; sessionId: string } | null;

export type ObjectMenu = { object: ObjectRef; point: MenuPoint };

/** Relations that export and show in the ER diagram. */
const relations = new Set<ObjectRef['kind']>([
  'table',
  'foreignTable',
  'view',
  'materializedView',
]);

type Props = {
  menu: ObjectMenu | null;
  /** Quotes identifiers in Copy As ▸. */
  engine: EngineDto;
  onClose: () => void;
  sessionFor: SessionLookup;
  onOpen: (object: ObjectRef) => void;
  onDiagram: (object: ObjectRef) => void;
  onImport: (object: ObjectRef) => void;
  /** The engine's object editors; `null` hides them. */
  features: ObjectFeaturesDto | null;
  onEditor: (request: ObjectEditorRequest) => void;
  onBackup?: ((object: ObjectRef) => void) | undefined;
  /** Copy a relation's `openrows://` link. */
  onCopyLink?: ((object: ObjectRef) => void) | undefined;
};

/**
 * Context menu of an object in the schema tree (Open, copy, favourite,
 * Import… / Export… / Back Up…, the object's lifecycle editors, the
 * destructive ones last) and the export sheet it opens.
 */
export function ObjectActions(props: Props): React.JSX.Element | null {
  const [exporting, setExporting] = useState<ObjectRef | null>(null);
  const { menu } = props;
  const target = exporting ? props.sessionFor(exporting.database) : null;
  return (
    <>
      {menu && (
        <ObjectMenuView {...props} menu={menu} onExport={setExporting} />
      )}
      {exporting && target && (
        <ExportSheet
          {...tableExport({ ...target, ...exporting })}
          onClose={() => {
            setExporting(null);
          }}
        />
      )}
    </>
  );
}

const item = (
  label: string,
  icon: IconName,
  run: () => void,
  more: Partial<Extract<MenuEntry, { kind: 'item' }>> = {},
): MenuEntry => ({
  kind: 'item',
  label,
  icon: <Icon name={icon} />,
  run,
  ...more,
});

const separator: MenuEntry = { kind: 'separator' };

/** Open (and the ER diagram of a relation). */
function openEntries(props: Props, object: ObjectRef): MenuEntry[] {
  const open = item(
    t('open'),
    object.kind === 'table' ? 'table' : 'eye',
    () => {
      props.onOpen(object);
    },
    { shortcut: '↩' },
  );
  if (!relations.has(object.kind)) return [open];
  return [
    open,
    item(t('showErDiagram'), 'diagram', () => {
      props.onDiagram(object);
    }),
  ];
}

const copy = (text: string) => (): void => {
  navigator.clipboard.writeText(text).catch(() => undefined);
};

/** Copy As ▸ the name, the dialect-quoted qualified name or a SELECT. */
function copyAs(object: ObjectRef, engine: EngineDto): MenuEntry {
  const qualified = tableName(engine, object);
  return {
    kind: 'submenu',
    label: shell('copyAs'),
    entries: [
      item(shell('name'), 'copy', copy(object.name)),
      item(shell('qualifiedName'), 'copy', copy(qualified)),
      item(
        shell('selectStatement'),
        'terminal',
        copy(`SELECT * FROM ${qualified};`),
      ),
    ],
  };
}

/** Copy Name (and Copy As ▸ for relations), Copy Link, the favourite toggle. */
function copyEntries(
  props: Props,
  object: ObjectRef,
  favourite: ReturnType<typeof useFavourite> | null,
): MenuEntry[] {
  const { onCopyLink } = props;
  const relation = relations.has(object.kind);
  return [
    item(shell('copyName'), 'copy', copy(object.name)),
    ...(relation ? [copyAs(object, props.engine)] : []),
    ...(onCopyLink && relation
      ? [
          item(t('copyLink'), 'link', () => {
            onCopyLink(object);
          }),
        ]
      : []),
    ...(favourite ? [item(favourite.label, 'star', favourite.toggle)] : []),
  ];
}

/** Import… (tables), Export… (relations) and Back Up Table…. */
function transferEntries(
  props: Props & { onExport: (object: ObjectRef) => void },
  object: ObjectRef,
  open: boolean,
): MenuEntry[] {
  if (!relations.has(object.kind)) return [];
  const table = object.kind === 'table';
  const { onBackup } = props;
  return [
    ...(table
      ? [
          item(
            t('import'),
            'download',
            () => {
              props.onImport(object);
            },
            { disabled: !open },
          ),
        ]
      : []),
    item(
      t('export'),
      'upload',
      () => {
        props.onExport(object);
      },
      { disabled: !open },
    ),
    ...(table && onBackup
      ? [
          item(
            t('backUpTable'),
            'database',
            () => {
              onBackup(object);
            },
            { disabled: !open },
          ),
        ]
      : []),
  ];
}

/** Groups with one separator between each non-empty pair. */
function joined(groups: readonly MenuEntry[][]): MenuEntry[] {
  return groups
    .filter((group) => group.length > 0)
    .flatMap((group, index) => (index ? [separator, ...group] : group));
}

function ObjectMenuView(
  props: Props & {
    menu: ObjectMenu;
    onExport: (object: ObjectRef) => void;
  },
): React.JSX.Element {
  const { object, point } = props.menu;
  const session = props.sessionFor(object.database);
  const favourite = useFavourite(
    session ? { ...object, profileId: session.profileId } : null,
  );
  const editors = props.features ? objectEntries(object, props.features) : [];
  const footer =
    !session && relations.has(object.kind)
      ? t('openDatabaseExport')
      : hasUnsupported(editors) && shell('notSupported');
  const entries = joined([
    openEntries(props, object),
    copyEntries(props, object, session ? favourite : null),
    transferEntries(props, object, Boolean(session)),
    editorSections(editors, props.onEditor),
  ]);
  return (
    <ContextMenu
      label={t('nameActions', { name: object.name })}
      at={point}
      target={point.target}
      entries={entries}
      onClose={props.onClose}
      footer={footer || undefined}
    />
  );
}
