// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { projectSqlItems } from '@openrows/ipc';
import type { ProjectSqlDto } from '@openrows/ipc';
import type { SqlSnippet } from '@openrows/editor';
import { reportProjectError, useProject } from './projectStore';
import { baseName, stem } from './treeModel';
import { snippetProblem } from '../snippets/snippetStore';
import type { Snippet } from '../snippets/snippetStore';

const HEADER = '-- openrows-snippet: ';
const SCOPES = [
  'all',
  'postgres',
  'mysql',
  'sqlserver',
  'sqlite',
  'clickhouse',
];

/** Snippet metadata is a SQL comment so shared files remain readable SQL. */
export function snippetFile(snippet: Snippet): string {
  const { name, prefix, scope, body } = snippet;
  return `${HEADER}${JSON.stringify({ name, prefix, scope })}\n${body}`;
}

/** SQL file names are completion keywords; SQL is inserted literally. */
export function queryKeyword(path: string): string {
  return stem(baseName(path)).replace(/[^\w$]/g, '_');
}

/** Parse project snippets without executing SQL or interpreting arbitrary metadata. */
export function projectSnippet(item: ProjectSqlDto): Snippet | null {
  try {
    const split = item.sql.indexOf('\n');
    const line = split < 0 ? item.sql : item.sql.slice(0, split);
    const metadata = line.startsWith(HEADER)
      ? (JSON.parse(line.slice(HEADER.length)) as Record<string, unknown>)
      : {};
    const name =
      typeof metadata.name === 'string'
        ? metadata.name
        : stem(baseName(item.path));
    const prefix =
      typeof metadata.prefix === 'string'
        ? metadata.prefix
        : queryKeyword(item.path);
    const scope = typeof metadata.scope === 'string' ? metadata.scope : 'all';
    if (!SCOPES.includes(scope)) return null;
    if (line.startsWith(HEADER) && split < 0) return null;
    const body = line.startsWith(HEADER) ? item.sql.slice(split + 1) : item.sql;
    const snippet = {
      id: item.path,
      name,
      prefix,
      scope: scope as Snippet['scope'],
      body,
    };
    return snippetProblem(snippet) ? null : snippet;
  } catch {
    return null;
  }
}

/** Reload project completion sources when its tree changes, discarding stale reads. */
export function useProjectSql(): readonly ProjectSqlDto[] {
  const { project } = useProject();
  const [state, setState] = useState<{
    project: typeof project;
    items: ProjectSqlDto[];
  }>({ project: null, items: [] });
  useEffect(() => {
    let live = true;
    if (project)
      projectSqlItems(project.root).then(
        (items) => {
          if (live) setState({ project, items });
        },
        (error: unknown) => {
          if (live) reportProjectError(error);
        },
      );
    return () => {
      live = false;
    };
  }, [project]);
  return project === state.project ? state.items : [];
}

/** Literal saved-query completions use filenames as keywords and retain folder detail. */
export function projectQueries(items: readonly ProjectSqlDto[]): SqlSnippet[] {
  return items
    .filter((item) => !item.snippet)
    .map((item) => ({
      name: item.path,
      prefix: queryKeyword(item.path),
      body: item.sql,
      literal: true,
    }));
}
