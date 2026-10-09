// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { NameSheet } from '../query/NameSheet';
import { deletePrompt } from './projectActions';
import type { ProjectActions, ProjectDialog } from './projectActions';
import { stem } from './treeModel';

/** The in-app delete alert, when the native one is unavailable. */
function DeleteAlert(props: {
  dialog: Extract<ProjectDialog, { kind: 'delete' }>;
  actions: ProjectActions;
  onClose: () => void;
}): React.JSX.Element {
  const prompt = deletePrompt(props.dialog.node);
  return (
    <Sheet title={prompt.title} size="alert" onClose={props.onClose}>
      <p>{prompt.message}</p>
      <SheetActions>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button
          variant="destructive"
          onClick={() => {
            props.actions.remove(props.dialog.node.path);
            props.onClose();
          }}
        >
          {prompt.action}
        </Button>
      </SheetActions>
    </Sheet>
  );
}

/**
 * New Folder…, New Query…, Rename… and the in-app Delete… alert (shown when
 * the native one is unavailable) of the project tree.
 */
export function ProjectSheets(props: {
  dialog: ProjectDialog | null;
  actions: ProjectActions;
  onClose: () => void;
}): React.JSX.Element | null {
  const { dialog, actions, onClose } = props;
  if (!dialog) return null;
  if (dialog.kind === 'delete')
    return <DeleteAlert dialog={dialog} actions={actions} onClose={onClose} />;
  const { node } = dialog;
  if (dialog.kind === 'rename')
    return (
      <NameSheet
        title={`Rename ${node.kind === 'folder' ? 'folder' : 'query'}`}
        label="Name"
        initial={node.kind === 'folder' ? node.name : stem(node.name)}
        action="Rename"
        onSubmit={(name) => actions.rename(node, name)}
        onClose={onClose}
      />
    );
  const folder = dialog.kind === 'newFolder';
  return (
    <NameSheet
      title={folder ? 'New folder' : 'New query'}
      label={folder ? 'Folder name' : 'Query name'}
      initial=""
      action="Create"
      onSubmit={(name) =>
        folder
          ? actions.newFolder(node.path, name)
          : actions.newQuery(node.path, name)
      }
      onClose={onClose}
    />
  );
}
