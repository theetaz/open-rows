// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { ObjectFeaturesDto } from '@openrows/ipc';
import { FavouriteButton } from '../schema/FavouriteButton';
import { PointMenu } from '../../shell/PointMenu';
import { ExportSheet, tableExport } from '../export';
import { EntryItems, objectEntries } from '../objects';
import type { ObjectEditorRequest } from '../objects';
import type { MenuPoint, ObjectRef } from '../schema';

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

/** Export…, Import… (tables) and Show ER Diagram of a relation. */
function RelationItems(props: {
  exportable: boolean;
  onExport: () => void;
  /** `null` when the relation cannot be imported into. */
  onImport: (() => void) | null;
  onDiagram: () => void;
}): React.JSX.Element {
  return (
    <>
      <button
        type="button"
        role="menuitem"
        disabled={!props.exportable}
        title={props.exportable ? undefined : 'Open the database to export'}
        onClick={props.onExport}
      >
        Export…
      </button>
      {props.onImport && (
        <button
          type="button"
          role="menuitem"
          disabled={!props.exportable}
          title={props.exportable ? undefined : 'Open the database to import'}
          onClick={props.onImport}
        >
          Import…
        </button>
      )}
      <button type="button" role="menuitem" onClick={props.onDiagram}>
        Show ER Diagram
      </button>
    </>
  );
}

/**
 * Context menu of an object in the schema tree (Open, the object's
 * lifecycle editors, Export…, Import… for tables, Show ER Diagram) and the
 * export sheet it opens.
 */
export function ObjectActions(props: {
  menu: ObjectMenu | null;
  onClose: () => void;
  sessionFor: SessionLookup;
  onOpen: (object: ObjectRef) => void;
  onDiagram: (object: ObjectRef) => void;
  onImport: (object: ObjectRef) => void;
  /** The engine's object editors; `null` hides them. */
  features: ObjectFeaturesDto | null;
  onEditor: (request: ObjectEditorRequest) => void;
}): React.JSX.Element | null {
  const [exporting, setExporting] = useState<ObjectRef | null>(null);
  const { menu } = props;
  const session = menu ? props.sessionFor(menu.object.database) : null;
  const target = exporting ? props.sessionFor(exporting.database) : null;
  const relation = menu ? relations.has(menu.object.kind) : false;
  const choose = (run: () => void) => () => {
    props.onClose();
    run();
  };
  return (
    <>
      {menu && (
        <PointMenu
          {...menu.point}
          label={`${menu.object.name} actions`}
          onClose={props.onClose}
        >
          <button
            type="button"
            role="menuitem"
            onClick={choose(() => {
              props.onOpen(menu.object);
            })}
          >
            Open
          </button>
          <MenuFavourite menu={menu} session={session} />
          {props.features && (
            <EntryItems
              entries={objectEntries(menu.object, props.features)}
              onChoose={(request) => {
                props.onClose();
                props.onEditor(request);
              }}
            />
          )}
          {relation && (
            <RelationItems
              exportable={Boolean(session)}
              onExport={choose(() => {
                setExporting(menu.object);
              })}
              onImport={
                menu.object.kind === 'table'
                  ? choose(() => {
                      props.onImport(menu.object);
                    })
                  : null
              }
              onDiagram={choose(() => {
                props.onDiagram(menu.object);
              })}
            />
          )}
        </PointMenu>
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

function MenuFavourite(props: {
  menu: ObjectMenu;
  session: ReturnType<SessionLookup>;
}): React.JSX.Element | null {
  return props.session ? (
    <FavouriteButton
      menu
      object={{ ...props.menu.object, profileId: props.session.profileId }}
    />
  ) : null;
}
