// SPDX-License-Identifier: Apache-2.0
import type { ProjectDto, ProjectEntryKind } from '@openrows/ipc';

/** A folder or query of the project tree; `path` is relative to the root. */
export type ProjectNode = {
  path: string;
  name: string;
  kind: ProjectEntryKind;
  children: ProjectNode[];
};

/** The folder holding `path` (`.` for top-level items of a `.` query folder). */
export function parentOf(path: string): string {
  const index = path.lastIndexOf('/');
  return index < 0 ? '.' : path.slice(0, index);
}

/** The last segment of a path. */
export function baseName(path: string): string {
  return path.slice(path.lastIndexOf('/') + 1);
}

/** `name.sql` → `name`, for display and rename. */
export function stem(name: string): string {
  return name.replace(/\.sql$/i, '');
}

/**
 * The tree of the project's flat, depth-first entries: one root per query
 * folder (the project's name for `.`), folders before queries.
 */
export function projectTree(project: ProjectDto): ProjectNode[] {
  const byPath = new Map<string, ProjectNode>();
  const roots: ProjectNode[] = [];
  for (const entry of project.entries) {
    const top = project.queryFolders.includes(entry.path);
    const node: ProjectNode = {
      path: entry.path,
      name: top && entry.path === '.' ? project.name : entry.path,
      kind: entry.kind,
      children: [],
    };
    if (!top) node.name = baseName(entry.path);
    byPath.set(entry.path, node);
    const parent = top ? undefined : byPath.get(parentOf(entry.path));
    (parent ? parent.children : roots).push(node);
  }
  return roots;
}

/**
 * Nodes whose name contains `needle` (case-insensitive) with their
 * ancestors; every node when the needle is blank.
 */
export function filterTree(
  nodes: readonly ProjectNode[],
  needle: string,
): ProjectNode[] {
  const wanted = needle.trim().toLowerCase();
  if (!wanted) return [...nodes];
  return nodes.flatMap((node) => {
    if (node.name.toLowerCase().includes(wanted)) return [node];
    const children = filterTree(node.children, wanted);
    return children.length ? [{ ...node, children }] : [];
  });
}

/** The absolute path of a project item, with the root's separator. */
export function absolutePath(root: string, path: string): string {
  if (!path || path === '.') return root;
  const separator = root.includes('\\') && !root.includes('/') ? '\\' : '/';
  return `${root.replace(/[\\/]+$/, '')}${separator}${path.split('/').join(separator)}`;
}

/** Whether moving `item` into `folder` is a real move (not into itself). */
export function canMove(item: string, folder: string): boolean {
  if (item === folder || parentOf(item) === folder) return false;
  return !folder.startsWith(`${item}/`);
}
