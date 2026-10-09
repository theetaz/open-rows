// SPDX-License-Identifier: Apache-2.0
import {
  confirmDestructive,
  createProjectFolder,
  createProjectQuery,
  deleteProjectItem,
  duplicateProjectItem,
  moveProjectItem,
  renameProjectItem,
  revealProjectItem,
} from '@openrows/ipc';
import type { ProjectDto } from '@openrows/ipc';
import type { MenuEntry } from '../../shell/CascadeMenu';
import { reloadProject, reportProjectError } from './projectStore';
import { absolutePath } from './treeModel';
import type { ProjectNode } from './treeModel';

/** What the tree reports to the workspace: opened queries and moved files. */
export type ProjectHooks = {
  /** Open a query file (absolute path) in a tab bound to it. */
  onOpenQuery: (path: string) => void;
  /** A file or folder moved: tabs bound to files under `from` follow to `to`. */
  onMoved: (from: string, to: string) => void;
};

/** The sheets the tree's menu asks for. */
export type ProjectDialog =
  | { kind: 'newFolder' | 'newQuery' | 'rename'; node: ProjectNode }
  | { kind: 'delete'; node: ProjectNode };

/** Project file actions; each reloads the tree when it is done. */
export type ProjectActions = {
  open: (node: ProjectNode) => void;
  newFolder: (parent: string, name: string) => Promise<void>;
  /** Create an empty query and open it. */
  newQuery: (folder: string, name: string) => Promise<void>;
  rename: (node: ProjectNode, name: string) => Promise<void>;
  move: (path: string, folder: string) => void;
  duplicate: (path: string) => void;
  remove: (path: string) => void;
  reveal: (path: string) => void;
  copyPath: (path: string) => void;
  /** Ask natively before deleting; `fallback` shows the in-app alert. */
  askDelete: (node: ProjectNode, fallback: () => void) => void;
};

/** Errors of actions without a sheet show in the sidebar. */
function settle(work: Promise<unknown>): void {
  work.then(() => reloadProject(), reportProjectError);
}

/** The delete prompt of a folder or query. */
export function deletePrompt(node: ProjectNode): {
  title: string;
  message: string;
  action: string;
} {
  const folder = node.kind === 'folder';
  return {
    title: `Delete “${node.name}”?`,
    message: folder
      ? 'The folder and every query in it are deleted from disk.'
      : 'The query file is deleted from disk.',
    action: 'Delete',
  };
}

/** Folder and query actions on the open project. */
export function projectActions(
  project: ProjectDto,
  hooks: ProjectHooks,
): ProjectActions {
  const { root } = project;
  const absolute = (path: string): string => absolutePath(root, path);
  const moved = (from: string, to: string): void => {
    hooks.onMoved(absolute(from), absolute(to));
  };
  return {
    open: (node: ProjectNode): void => {
      if (node.kind === 'query') hooks.onOpenQuery(absolute(node.path));
    },
    newFolder: async (parent: string, name: string): Promise<void> => {
      await createProjectFolder(root, parent, name);
      await reloadProject();
    },
    newQuery: async (folder: string, name: string): Promise<void> => {
      const path = await createProjectQuery(root, folder, name, '');
      await reloadProject();
      hooks.onOpenQuery(absolute(path));
    },
    rename: async (node: ProjectNode, name: string): Promise<void> => {
      const path = await renameProjectItem(root, node.path, name);
      moved(node.path, path);
      await reloadProject();
    },
    move: (path: string, folder: string): void => {
      settle(
        moveProjectItem(root, path, folder).then((target) => {
          moved(path, target);
        }),
      );
    },
    duplicate: (path: string): void => {
      settle(duplicateProjectItem(root, path));
    },
    remove: (path: string): void => {
      settle(deleteProjectItem(root, path));
    },
    reveal: (path: string): void => {
      revealProjectItem(root, path).catch(reportProjectError);
    },
    copyPath: (path: string): void => {
      navigator.clipboard.writeText(absolute(path)).catch(reportProjectError);
    },
    askDelete: (node: ProjectNode, fallback: () => void): void => {
      confirmDestructive(deletePrompt(node)).then((answer) => {
        if (answer === null) fallback();
        else if (answer) settle(deleteProjectItem(root, node.path));
      }, fallback);
    },
  };
}

const item = (label: string, run: () => void): MenuEntry => ({
  kind: 'item',
  label,
  run,
});

/** The row menu of a folder or query; query folders cannot be renamed or deleted. */
export function nodeMenu(input: {
  node: ProjectNode;
  top: boolean;
  actions: ProjectActions;
  ask: (dialog: ProjectDialog) => void;
}): MenuEntry[] {
  const { node, actions, ask } = input;
  const folder = node.kind === 'folder';
  const create: MenuEntry[] = folder
    ? [
        item('New Query…', () => {
          ask({ kind: 'newQuery', node });
        }),
        item('New Folder…', () => {
          ask({ kind: 'newFolder', node });
        }),
      ]
    : [
        item('Open', () => {
          actions.open(node);
        }),
      ];
  const edit: MenuEntry[] = input.top
    ? []
    : [
        { kind: 'separator' },
        item('Rename…', () => {
          ask({ kind: 'rename', node });
        }),
        item('Duplicate', () => {
          actions.duplicate(node.path);
        }),
        {
          kind: 'item',
          label: 'Delete…',
          danger: true,
          run: () => {
            ask({ kind: 'delete', node });
          },
        },
      ];
  return [
    ...create,
    ...edit,
    { kind: 'separator' },
    item('Reveal in Finder', () => {
      actions.reveal(node.path);
    }),
    item('Copy Path', () => {
      actions.copyPath(node.path);
    }),
  ];
}
