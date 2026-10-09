// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { HighlightText, Icon, Tree, TreeNode } from '@openrows/ui';
import type { ProjectDto } from '@openrows/ipc';
import { CascadeMenu } from '../../shell/CascadeMenu';
import { substringIndices } from '../schema';
import { ProjectSheets } from './ProjectSheets';
import { nodeMenu, projectActions } from './projectActions';
import type {
  ProjectActions,
  ProjectDialog,
  ProjectHooks,
} from './projectActions';
import { canMove, filterTree, projectTree } from './treeModel';
import type { ProjectNode } from './treeModel';

/** MIME type of a dragged project item; its data is the item's path. */
const DRAG_TYPE = 'application/x-openrows-project-item';

type Menu = { node: ProjectNode; top: boolean; x: number; y: number };

/** Tree state shared by every row. */
type RowContext = {
  search: string;
  actions: ProjectActions;
  expanded: ReadonlySet<string>;
  toggle: (path: string, open: boolean) => void;
  selected: string | null;
  select: (path: string) => void;
  drop: { target: string | null; set: (path: string | null) => void };
  onMenu: (menu: Menu) => void;
};

/** Drag a row onto a folder to move it there. */
function dragOf(node: ProjectNode, top: boolean, context: RowContext) {
  const folder = node.kind === 'folder';
  const dragged = (event: React.DragEvent): string =>
    event.dataTransfer.getData(DRAG_TYPE);
  return {
    draggable: !top,
    onDragStart: (event: React.DragEvent) => {
      event.dataTransfer.setData(DRAG_TYPE, node.path);
      event.dataTransfer.effectAllowed = 'move';
    },
    onDragOver: (event: React.DragEvent) => {
      if (!folder || !event.dataTransfer.types.includes(DRAG_TYPE)) return;
      event.preventDefault();
      event.dataTransfer.dropEffect = 'move';
      context.drop.set(node.path);
    },
    onDragLeave: () => {
      if (context.drop.target === node.path) context.drop.set(null);
    },
    onDrop: (event: React.DragEvent) => {
      const path = dragged(event);
      context.drop.set(null);
      if (!folder || !path || !canMove(path, node.path)) return;
      event.preventDefault();
      context.actions.move(path, node.path);
    },
    onDragEnd: () => {
      context.drop.set(null);
    },
  };
}

function Row(props: {
  node: ProjectNode;
  level: number;
  context: RowContext;
}): React.JSX.Element {
  const { node, level, context } = props;
  const folder = node.kind === 'folder';
  const top = level === 1;
  const open =
    folder && (context.expanded.has(node.path) || context.search !== '');
  return (
    <TreeNode
      label={node.name}
      level={level}
      selected={context.selected === node.path}
      tabStop={
        context.selected === node.path || (context.selected === null && top)
      }
      onSelect={() => {
        context.select(node.path);
      }}
      onOpen={() => {
        if (folder) context.toggle(node.path, !open);
        else context.actions.open(node);
      }}
      {...(folder
        ? {
            expanded: open,
            onExpandedChange: (next: boolean) => {
              context.toggle(node.path, next);
            },
          }
        : {
            onPreview: () => {
              context.actions.open(node);
            },
          })}
      icon={<Icon name={folder ? 'schema' : 'code'} />}
      iconTone={folder ? undefined : 'info'}
      name={
        <HighlightText
          text={node.name}
          indices={substringIndices(node.name, context.search)}
        />
      }
      title={node.path}
      drag={dragOf(node, top, context)}
      dropTarget={context.drop.target === node.path}
      onContextMenu={(event) => {
        event.preventDefault();
        context.onMenu({ node, top, x: event.clientX, y: event.clientY });
      }}
    >
      {node.children.map((child) => (
        <Row
          key={child.path}
          node={child}
          level={level + 1}
          context={context}
        />
      ))}
    </TreeNode>
  );
}

/**
 * The project's query folders as a tree (FR-EDIT-14): click a query to open
 * it in a tab bound to its file; drag items onto folders to move them;
 * right-click for new, rename, duplicate, delete, reveal and copy path.
 */
export function ProjectTree(
  props: ProjectHooks & { project: ProjectDto; search: string },
): React.JSX.Element {
  const { project } = props;
  const roots = filterTree(projectTree(project), props.search);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(
    () => new Set(project.queryFolders),
  );
  const [selected, select] = useState<string | null>(null);
  const [target, setTarget] = useState<string | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  const [dialog, setDialog] = useState<ProjectDialog | null>(null);
  const actions = projectActions(project, props);
  const ask = (next: ProjectDialog): void => {
    if (next.kind !== 'delete') setDialog(next);
    else
      actions.askDelete(next.node, () => {
        setDialog(next);
      });
  };
  const context: RowContext = {
    search: props.search,
    actions,
    expanded,
    toggle: (path, open) => {
      setExpanded((current) => {
        const next = new Set(current);
        if (open) next.add(path);
        else next.delete(path);
        return next;
      });
    },
    selected,
    select,
    drop: { target, set: setTarget },
    onMenu: setMenu,
  };
  return (
    <>
      <Tree label={`${project.name} queries`}>
        {roots.map((node) => (
          <Row key={node.path} node={node} level={1} context={context} />
        ))}
      </Tree>
      {menu && (
        <CascadeMenu
          x={menu.x}
          y={menu.y}
          label={`${menu.node.name} actions`}
          onClose={() => {
            setMenu(null);
          }}
          entries={nodeMenu({ ...menu, actions, ask })}
        />
      )}
      <ProjectSheets
        dialog={dialog}
        actions={actions}
        onClose={() => {
          setDialog(null);
        }}
      />
    </>
  );
}
