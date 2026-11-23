// SPDX-License-Identifier: Apache-2.0
import type { ObjectFeaturesDto } from '@openrows/ipc';
import { ContextMenu } from '../../shell/ContextMenu';
import { t as shell } from '../../shell/messages';
import { containerEntries, newObjectEntries } from '../objects';
import type { ObjectEditorRequest } from '../objects';
import type { MenuPoint } from '../schema';
import { editorSections, hasUnsupported } from './editorMenu';
import { t } from './messages';

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
  const objects = newObjectEntries(props.place, props.features);
  const containers = containerEntries(props.place, props.features);
  return (
    <ContextMenu
      label={t('create')}
      at={props.point}
      entries={[
        ...editorSections(objects, props.onChoose),
        { kind: 'separator' },
        ...editorSections(containers, props.onChoose),
      ]}
      onClose={props.onClose}
      footer={
        hasUnsupported([...objects, ...containers])
          ? shell('notSupported')
          : undefined
      }
    />
  );
}
