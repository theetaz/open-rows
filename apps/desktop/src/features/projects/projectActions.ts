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
  readSqlFile,
} from '@openrows/ipc';
import type { ProjectDto } from '@openrows/ipc';
import type { MenuEntry } from '../../shell/CascadeMenu';
import { reloadProject, reportProjectError } from './projectStore';
import { insertSavedSql } from '../query/historyInsert';
import { copyProjectSql } from './sqlClipboard';
import { absolutePath } from './treeModel';
import type { ProjectNode } from './treeModel';
import { t } from './messages';

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
  insertSql: (path: string) => void;
  copySql: (path: string) => void;
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
    title: t('deleteName', { name: node.name }),
    message: folder ? t('folderEveryQuery') : t('queryFileDeleted'),
    action: t('delete'),
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
    ...sqlActions(absolute),
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

function sqlActions(
  absolute: (path: string) => string,
): Pick<ProjectActions, 'insertSql' | 'copySql'> {
  return {
    insertSql: (path: string): void => {
      readSqlFile(absolute(path)).then((file) => {
        if (!insertSavedSql(file.sql))
          reportProjectError(new Error(t('openQueryEditor')));
      }, reportProjectError);
    },
    copySql: (path: string): void => {
      copyProjectSql(absolute(path)).catch(reportProjectError);
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
        item(t('newQuery2'), () => {
          ask({ kind: 'newQuery', node });
        }),
        item(t('newFolder2'), () => {
          ask({ kind: 'newFolder', node });
        }),
      ]
    : [
        item(t('open'), () => {
          actions.open(node);
        }),
        item(t('insertSql'), () => {
          actions.insertSql(node.path);
        }),
        item(t('copySql'), () => {
          actions.copySql(node.path);
        }),
      ];
  const edit: MenuEntry[] = input.top
    ? []
    : [
        { kind: 'separator' },
        item(t('rename2'), () => {
          ask({ kind: 'rename', node });
        }),
        item(t('duplicate'), () => {
          actions.duplicate(node.path);
        }),
        {
          kind: 'item',
          label: t('delete2'),
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
    item(t('revealFinder'), () => {
      actions.reveal(node.path);
    }),
    item(t('copyPath'), () => {
      actions.copyPath(node.path);
    }),
  ];
}
