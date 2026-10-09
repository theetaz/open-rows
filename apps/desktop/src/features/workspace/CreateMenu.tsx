// SPDX-License-Identifier: Apache-2.0
import type { ObjectFeaturesDto } from '@openrows/ipc';
import { PointMenu } from '../../shell/PointMenu';
import { containerEntries, EntryItems, newObjectEntries } from '../objects';
import type { ObjectEditorRequest } from '../objects';
import type { MenuPoint } from '../schema';

/**
 * The sidebar's `+` menu: new table, view, materialized view, function
 * and procedure in the selected schema, then the schema and database
 * themselves (new, rename, drop), each disabled where the engine cannot.
 */
export function CreateMenu(props: {
  point: MenuPoint;
  place: { database: string; schema: string };
  features: ObjectFeaturesDto;
  onChoose: (request: ObjectEditorRequest) => void;
  onClose: () => void;
}): React.JSX.Element {
  const choose = (request: ObjectEditorRequest): void => {
    props.onClose();
    props.onChoose(request);
  };
  return (
    <PointMenu {...props.point} above label="Create" onClose={props.onClose}>
      <EntryItems
        entries={newObjectEntries(props.place, props.features)}
        onChoose={choose}
      />
      <hr />
      <EntryItems
        entries={containerEntries(props.place, props.features)}
        onChoose={choose}
      />
    </PointMenu>
  );
}
