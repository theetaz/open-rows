// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { PointMenu } from '../../shell/PointMenu';
import { ExportSheet, tableExport } from '../export';
import type { MenuPoint, ObjectRef } from '../schema';

/** The open session of a database of the active connection, if any. */
export type SessionLookup = (
  database: string,
) => { profileId: string; sessionId: string } | null;

export type ObjectMenu = { object: ObjectRef; point: MenuPoint };

/**
 * Context menu of a relation in the schema tree (Open, Export…, Show ER
 * Diagram) and the export sheet it opens.
 */
export function ObjectActions(props: {
  menu: ObjectMenu | null;
  onClose: () => void;
  sessionFor: SessionLookup;
  onOpen: (object: ObjectRef) => void;
  onDiagram: (object: ObjectRef) => void;
}): React.JSX.Element | null {
  const [exporting, setExporting] = useState<ObjectRef | null>(null);
  const { menu } = props;
  const session = menu ? props.sessionFor(menu.object.database) : null;
  const target = exporting ? props.sessionFor(exporting.database) : null;
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
            onClick={() => {
              props.onClose();
              props.onOpen(menu.object);
            }}
          >
            Open
          </button>
          <button
            type="button"
            role="menuitem"
            disabled={!session}
            title={session ? undefined : 'Open the database to export'}
            onClick={() => {
              props.onClose();
              setExporting(menu.object);
            }}
          >
            Export…
          </button>
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              props.onClose();
              props.onDiagram(menu.object);
            }}
          >
            Show ER Diagram
          </button>
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
