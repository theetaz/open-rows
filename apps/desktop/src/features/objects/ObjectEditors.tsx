// SPDX-License-Identifier: Apache-2.0
import type { ObjectChangeDto, ProfileDto } from '@openrows/ipc';
import { DatabaseSheet, SchemaSheet } from './ContainerSheets';
import { DefinitionEditor } from './DefinitionEditor';
import { DropSheet } from './DropSheet';
import { NewTableEditor } from './NewTableEditor';
import type { ObjectEditorRequest } from './requests';
import {
  DuplicateTableSheet,
  RenameTableSheet,
  TruncateTableSheet,
} from './TableSheets';
import { ViewEditor } from './ViewEditor';

type Props = {
  request: ObjectEditorRequest;
  profile: ProfileDto;
  /** Databases of the connection open in the app. */
  opened: readonly string[];
  onCloseDatabase: (name: string) => void;
  onClose: () => void;
  /** The change ran. */
  onDone: (request: ObjectEditorRequest, change: ObjectChangeDto) => void;
};

/** Editors whose request names a schema object. */
function SchemaObjectEditor(
  props: Props & { request: Extract<ObjectEditorRequest, { schema: string }> },
): React.JSX.Element {
  const { request, profile } = props;
  const scope = { profileId: profile.id, database: request.database };
  const common = {
    engine: profile.engine,
    onClose: props.onClose,
    onDone: (change: ObjectChangeDto) => {
      props.onDone(request, change);
    },
  };
  switch (request.kind) {
    case 'newTable':
      return (
        <NewTableEditor
          {...common}
          scope={scope}
          features={profile.objects}
          schema={request.schema}
        />
      );
    case 'renameTable':
      return <RenameTableSheet {...common} table={{ ...scope, ...request }} />;
    case 'duplicateTable':
      return (
        <DuplicateTableSheet {...common} table={{ ...scope, ...request }} />
      );
    case 'truncateTable':
      return (
        <TruncateTableSheet {...common} table={{ ...scope, ...request }} />
      );
    case 'view':
      return (
        <ViewEditor
          {...common}
          scope={scope}
          features={profile.objects}
          schema={request.schema}
          name={request.name}
          materialized={request.materialized}
        />
      );
    case 'trigger':
      return <DefinitionEditor {...common} scope={scope} target={request} />;
    case 'routine':
      return <DefinitionEditor {...common} scope={scope} target={request} />;
  }
}

/** The open object editor (create, edit, rename, duplicate, truncate, drop). */
export function ObjectEditors(props: Props): React.JSX.Element {
  const { request, profile } = props;
  const scope = {
    profileId: profile.id,
    database: 'database' in request ? request.database : '',
  };
  const done = (change: ObjectChangeDto): void => {
    props.onDone(request, change);
  };
  switch (request.kind) {
    case 'database':
      return (
        <DatabaseSheet
          scope={scope}
          engine={profile.engine}
          name={request.name}
          opened={props.opened}
          onCloseDatabase={props.onCloseDatabase}
          onClose={props.onClose}
          onDone={done}
        />
      );
    case 'schema':
      return (
        <SchemaSheet
          scope={scope}
          engine={profile.engine}
          name={request.name}
          onClose={props.onClose}
          onDone={done}
        />
      );
    case 'drop':
      return (
        <DropSheet
          target={request.target}
          engine={profile.engine}
          profileId={profile.id}
          opened={props.opened}
          onCloseDatabase={props.onCloseDatabase}
          onClose={props.onClose}
          onDone={done}
        />
      );
    default:
      return <SchemaObjectEditor {...props} request={request} />;
  }
}
