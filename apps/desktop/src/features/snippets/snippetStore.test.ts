// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  exportSnippets,
  importSnippets,
  parseSnippets,
  snippetProblem,
  snippetsFor,
} from './snippetStore';

const mine = {
  id: 'user-1',
  name: 'Recent orders',
  prefix: 'ro',
  body: 'SELECT * FROM orders WHERE created_at > ${1:now()};',
  scope: 'postgres' as const,
};

it('keeps valid snippets and known built-ins only', () => {
  const parsed = parseSnippets({
    items: [mine, { ...mine }, { name: 'x', prefix: 'bad prefix', body: 'b' }],
    disabledBuiltins: ['pg-explain', 'nope', 3],
  });
  expect(parsed.items).toEqual([mine]);
  expect(parsed.disabledBuiltins).toEqual(['pg-explain']);
  expect(parseSnippets(null)).toEqual({ items: [], disabledBuiltins: [] });
  expect(snippetProblem({ name: '', prefix: 'a', body: 'b' })).toMatch(/Name/);
});

it('offers built-ins per engine unless turned off, plus matching user snippets', () => {
  const settings = { items: [mine], disabledBuiltins: ['pg-explain'] };
  const pg = snippetsFor(settings, 'postgres').map(({ id }) => id);
  expect(pg).toContain('pg-upsert');
  expect(pg).toContain('select');
  expect(pg).toContain('user-1');
  expect(pg).not.toContain('pg-explain');
  const maria = snippetsFor(settings, 'mariadb').map(({ id }) => id);
  expect(maria).toContain('mysql-upsert');
  expect(maria).not.toContain('user-1');
});

it('round-trips snippets through JSON with fresh ids', () => {
  const text = exportSnippets([mine]);
  expect(text).not.toContain('user-1');
  const [imported] = importSnippets(text);
  expect(imported).toMatchObject({ name: mine.name, body: mine.body });
  expect(imported?.id).not.toBe(mine.id);
  expect(importSnippets(JSON.stringify({ snippets: [mine] }))).toHaveLength(1);
  expect(() => importSnippets('[]')).toThrow(/No valid snippets/);
  expect(() => importSnippets('{')).toThrow(SyntaxError);
});

it('preserves ClickHouse snippets and offers them only for that engine', () => {
  const snippet = { ...mine, scope: 'clickhouse' as const };
  const settings = parseSnippets({ items: [snippet] });
  expect(settings.items).toEqual([snippet]);
  expect(snippetsFor(settings, 'clickhouse')).toContainEqual(snippet);
  expect(snippetsFor(settings, 'postgres')).not.toContainEqual(snippet);
  expect(importSnippets(exportSnippets([snippet]))[0]?.scope).toBe(
    'clickhouse',
  );
});
