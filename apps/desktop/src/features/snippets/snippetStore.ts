// SPDX-License-Identifier: Apache-2.0
import type { EditorDialect, SqlSnippet } from '@openrows/editor';
import { localSetting } from '../settings/localSetting';
import { BUILTIN_SNIPPETS } from './builtins';

/** Engines a snippet is offered for; MariaDB uses the MySQL snippets. */
export type SnippetScope =
  'all' | 'postgres' | 'mysql' | 'sqlserver' | 'sqlite';

/** A built-in or user snippet. */
export type Snippet = SqlSnippet & { id: string; scope: SnippetScope };

/** `snippets` in settings.json: the user's snippets and turned-off built-ins. */
type SnippetSettings = {
  items: readonly Snippet[];
  disabledBuiltins: readonly string[];
};

/** Key in web storage (and `snippets` in settings.json). */
export const SNIPPETS_KEY = 'openrows.snippets';

const SCOPES: readonly SnippetScope[] = [
  'all',
  'postgres',
  'mysql',
  'sqlserver',
  'sqlite',
];
/** What completion can match as a trigger (letters, digits, `_`, `$`). */
const PREFIX = /^[\w$]{1,32}$/;
const MAX_NAME = 80;
const MAX_BODY = 10_000;
const MAX_ITEMS = 500;

const defaultSnippets: SnippetSettings = { items: [], disabledBuiltins: [] };

/** Why a snippet cannot be saved, or `null` when it can. */
export function snippetProblem(
  snippet: Pick<Snippet, 'name' | 'prefix' | 'body'>,
): string | null {
  const name = snippet.name.trim();
  if (!name || name.length > MAX_NAME)
    return `Name must be 1–${String(MAX_NAME)} characters.`;
  if (!PREFIX.test(snippet.prefix))
    return 'Prefix must be 1–32 letters, digits, _ or $.';
  if (!snippet.body.trim() || snippet.body.length > MAX_BODY)
    return 'Body must not be empty.';
  return null;
}

const isString = (value: unknown): value is string => typeof value === 'string';

/** A stored snippet, or `null` when invalid; `id` is made when missing. */
function snippetOf(stored: unknown, makeId: () => string): Snippet | null {
  if (typeof stored !== 'object' || stored === null) return null;
  const { id, name, prefix, body, scope } = stored as Record<string, unknown>;
  if (!isString(name) || !isString(prefix) || !isString(body)) return null;
  const snippet: Snippet = {
    id: isString(id) && id ? id : makeId(),
    name: name.trim(),
    prefix,
    body,
    scope: SCOPES.includes(scope as SnippetScope)
      ? (scope as SnippetScope)
      : 'all',
  };
  return snippetProblem(snippet) ? null : snippet;
}

const newId = (): string => `user-${crypto.randomUUID()}`;

/** Valid snippets of a stored list, without duplicate ids. */
function snippetsOf(stored: unknown, makeId: () => string): Snippet[] {
  if (!Array.isArray(stored)) return [];
  const seen = new Set<string>();
  return stored.slice(0, MAX_ITEMS).flatMap((item) => {
    const snippet = snippetOf(item, makeId);
    if (!snippet || seen.has(snippet.id)) return [];
    seen.add(snippet.id);
    return [snippet];
  });
}

/** Validate stored settings, keeping defaults for anything invalid. */
export function parseSnippets(stored: unknown): SnippetSettings {
  if (typeof stored !== 'object' || stored === null) return defaultSnippets;
  const { items, disabledBuiltins } = stored as Record<string, unknown>;
  const builtins = new Set(BUILTIN_SNIPPETS.map(({ id }) => id));
  return {
    items: snippetsOf(items, newId),
    disabledBuiltins: Array.isArray(disabledBuiltins)
      ? disabledBuiltins.filter(
          (id): id is string => isString(id) && builtins.has(id),
        )
      : [],
  };
}

export const snippetSettings = localSetting(SNIPPETS_KEY, parseSnippets);

/** Merge a change into the snippet settings. */
export function updateSnippets(change: Partial<SnippetSettings>): void {
  snippetSettings.set({ ...snippetSettings.get(), ...change });
}

/** The scope an engine's snippets come from. */
function scopeOf(dialect: EditorDialect): Exclude<SnippetScope, 'all'> {
  return dialect === 'mariadb' ? 'mysql' : dialect;
}

/** Built-in (enabled) and user snippets offered for an engine. */
export function snippetsFor(
  settings: SnippetSettings,
  dialect: EditorDialect,
): Snippet[] {
  const scope = scopeOf(dialect);
  const offered = (snippet: Snippet): boolean =>
    snippet.scope === 'all' || snippet.scope === scope;
  const builtins = BUILTIN_SNIPPETS.filter(
    (snippet) =>
      offered(snippet) && !settings.disabledBuiltins.includes(snippet.id),
  );
  return [...builtins, ...settings.items.filter(offered)];
}

/** The user's snippets as shareable JSON (ids are local and left out). */
export function exportSnippets(items: readonly Snippet[]): string {
  const shared = items.map(({ name, prefix, body, scope }) => ({
    name,
    prefix,
    body,
    scope,
  }));
  return JSON.stringify(shared, null, 2);
}

/**
 * Snippets from shared JSON (an array, or `{ "snippets": [...] }`), each
 * with a new id.
 *
 * @throws SyntaxError for invalid JSON; Error when no snippet is valid.
 */
export function importSnippets(text: string): Snippet[] {
  const parsed = JSON.parse(text) as unknown;
  const list =
    typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>).snippets
      : parsed;
  const items = Array.isArray(list)
    ? snippetsOf(
        list.map((item: unknown) =>
          typeof item === 'object' && item !== null
            ? { ...item, id: undefined }
            : item,
        ),
        newId,
      )
    : [];
  if (!items.length) throw new Error('No valid snippets found in the JSON.');
  return items;
}
