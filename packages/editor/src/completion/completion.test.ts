// SPDX-License-Identifier: Apache-2.0
import { CompletionContext } from '@codemirror/autocomplete';
import type { Completion } from '@codemirror/autocomplete';
import { EditorState } from '@codemirror/state';
import { describe, expect, it, vi } from 'vitest';
import { sqlEditorExtensions } from '../extensions';
import { defaultPreferences } from '../preferences';
import type { EditorDialect } from '../dialects';
import type { CatalogObject, CompletionCatalog } from './catalog';
import { analyzeStatement } from './context';
import { quoteName } from './candidates';
import { tokenize } from './lexer';
import { sqlCompletionSource } from './source';

const OBJECTS: CatalogObject[] = [
  { schema: 'sales', name: 'users_archive', kind: 'table' },
  { schema: 'public', name: 'users', kind: 'table' },
  { schema: 'public', name: 'user_stats', kind: 'view' },
  { schema: 'public', name: 'orders', kind: 'table' },
  { schema: 'public', name: 'refresh_totals', kind: 'function' },
];

const COLUMNS: Record<string, string[]> = {
  'public.users': ['id', 'email', 'user_name'],
  'public.orders': ['id', 'user_id', 'total'],
};

function fakeCatalog(): CompletionCatalog {
  return {
    schemas: () =>
      Promise.resolve([
        { name: 'pg_catalog', system: true },
        { name: 'public', system: false },
        { name: 'sales', system: false },
        { name: 'information_schema', system: true },
      ]),
    objects: () => Promise.resolve(OBJECTS),
    objectsIn: vi.fn((schema: string) =>
      Promise.resolve(
        schema === 'information_schema'
          ? [
              { schema, name: 'tables', kind: 'view' as const },
              { schema, name: 'columns', kind: 'view' as const },
            ]
          : [],
      ),
    ),
    columns: vi.fn((schema: string, table: string) =>
      Promise.resolve(
        (COLUMNS[`${schema}.${table}`] ?? []).map((name) => ({
          name,
          dataType: 'text',
        })),
      ),
    ),
  };
}

type Options = { upper?: boolean; dialect?: EditorDialect };

/** Completions at `|` in `text`, in display order. */
async function complete(
  text: string,
  catalog: CompletionCatalog | null = fakeCatalog(),
  { upper = true, dialect = 'postgres' }: Options = {},
): Promise<readonly Completion[]> {
  const pos = text.indexOf('|');
  const doc = text.replace('|', '');
  const state = EditorState.create({
    doc,
    extensions: sqlEditorExtensions({
      doc,
      label: 'SQL',
      dialect,
      preferences: { ...defaultPreferences, upperKeywords: upper },
    }),
  });
  const source = sqlCompletionSource(dialect, catalog);
  const result = await source(new CompletionContext(state, pos, true));
  return result?.options ?? [];
}

const labels = (options: readonly Completion[]): string[] =>
  options.map((option) => option.label);

function scopeAt(text: string): ReturnType<typeof analyzeStatement> {
  const pos = text.indexOf('|');
  const tokens = tokenize(text.replace('|', ''));
  const word = /[\w$]*$/.exec(text.slice(0, pos))?.[0] ?? '';
  return analyzeStatement(tokens, pos - word.length);
}

describe('context detection', () => {
  it('expects relations after FROM, JOIN, UPDATE, INTO and FROM lists', () => {
    for (const text of [
      'SELECT * FROM |',
      'SELECT * FROM us|',
      'SELECT * FROM a JOIN |',
      'UPDATE |',
      'INSERT INTO |',
      'SELECT * FROM a, |',
    ])
      expect(scopeAt(text).scope, text).toEqual({ kind: 'relation' });
  });

  it('resolves aliases and schema qualifiers', () => {
    const context = scopeAt(
      'SELECT u.| FROM public.users AS u JOIN orders o ON o.user_id = u.id',
    );
    expect(context.scope).toEqual({ kind: 'member', path: ['u'] });
    expect(context.references).toEqual([
      { schema: 'public', name: 'users', alias: 'u' },
      { schema: null, name: 'orders', alias: 'o' },
    ]);
    expect(scopeAt('SELECT * FROM sales.|').scope).toEqual({
      kind: 'member',
      path: ['sales'],
    });
  });

  it('treats select lists and conditions as expressions', () => {
    expect(scopeAt('SELECT na| FROM users').scope.kind).toBe('expression');
    expect(scopeAt('SELECT * FROM users WHERE |').scope.kind).toBe(
      'expression',
    );
    expect(scopeAt('SELECT * FROM t ORDER BY |').scope.kind).toBe('expression');
    expect(scopeAt('|').scope.kind).toBe('start');
    expect(scopeAt('SELECT * FROM users w|').scope.kind).toBe('start');
  });
});

describe('completion results', () => {
  it('offers tables after FROM, current schema first, matching tables before keywords', async () => {
    const options = await complete('SELECT * FROM user|');
    expect(labels(options).slice(0, 3)).toEqual([
      'users',
      'users_archive',
      'user_stats',
    ]);
    expect(options[0]).toMatchObject({ type: 'table', detail: 'public' });
    expect(options[1]).toMatchObject({ type: 'table', detail: 'sales' });
    expect(options[2]).toMatchObject({ type: 'view', detail: 'public' });
    expect(labels(options)).toContain('USER');
    expect(labels(options).indexOf('USER')).toBeGreaterThan(2);
  });

  it('offers the columns of an aliased table after `alias.`', async () => {
    const options = await complete('SELECT u.| FROM users u');
    expect(labels(options)).toEqual(['id', 'email', 'user_name']);
    expect(options[0]).toMatchObject({ type: 'column', detail: 'users' });
  });

  it('offers columns of referenced tables in WHERE before functions and keywords', async () => {
    const options = await complete(
      'SELECT * FROM users u JOIN orders o ON true WHERE u|',
    );
    const found = labels(options);
    expect(found.slice(0, 2)).toEqual(['user_id', 'user_name']);
    expect(found.indexOf('upper')).toBeGreaterThan(found.indexOf('user_name'));
  });
});

describe('qualified and keyword completion', () => {
  it('offers the objects of a schema after `schema.`', async () => {
    const catalog = fakeCatalog();
    const options = await complete(
      'SELECT * FROM information_schema.|',
      catalog,
    );
    expect(labels(options)).toEqual(['columns', 'tables']);
    expect(options[0]).toMatchObject({
      type: 'view',
      detail: 'information_schema',
    });
    expect(catalog.objectsIn).toHaveBeenCalledWith('information_schema');
  });

  it('ranks common keywords first and system schemas last', async () => {
    expect(labels(await complete('sel|'))[0]).toBe('SELECT');
    expect(labels(await complete('SELECT * fr|'))[0]).toBe('FROM');
    const schemas = (await complete('SELECT * FROM |')).filter(
      (option) => option.type === 'schema',
    );
    expect(labels(schemas)).toEqual([
      'public',
      'sales',
      'information_schema',
      'pg_catalog',
    ]);
  });

  it('inserts keywords in upper case unless switched off', async () => {
    expect(labels(await complete('sel|'))[0]).toBe('SELECT');
    expect(labels(await complete('sel|', null, { upper: false }))[0]).toBe(
      'select',
    );
  });

  it('offers built-in functions of engines without a function catalog', async () => {
    const options = await complete('SELECT julian|', null, {
      dialect: 'sqlite',
    });
    expect(options[0]).toMatchObject({
      label: 'julianday',
      type: 'function',
      detail: 'built-in',
    });
  });

  it('keeps keywords when catalog reads fail', async () => {
    const failing: CompletionCatalog = {
      schemas: () => Promise.reject(new Error('offline')),
      objects: () => Promise.reject(new Error('offline')),
      objectsIn: () => Promise.reject(new Error('offline')),
      columns: () => Promise.reject(new Error('offline')),
    };
    expect(labels(await complete('SELECT * FROM t WH|', failing))[0]).toBe(
      'WHERE',
    );
  });
});

it('quotes names the dialect would fold or reject', () => {
  expect(quoteName('users', 'postgres')).toBe('users');
  expect(quoteName('Users', 'postgres')).toBe('"Users"');
  expect(quoteName('Users', 'mysql')).toBe('Users');
  expect(quoteName('order items', 'sqlserver')).toBe('[order items]');
  expect(quoteName('a]b', 'sqlserver')).toBe('[a]]b]');
  expect(quoteName('order items', 'mysql')).toBe('`order items`');
  expect(quoteName('a"b', 'sqlite')).toBe('"a""b"');
});
