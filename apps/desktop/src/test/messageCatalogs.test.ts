// SPDX-License-Identifier: Apache-2.0
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import ts from 'typescript';
import { expect, it } from 'vitest';

const ROOT = resolve(import.meta.dirname, '../../../..');
const SOURCES = ['apps/desktop/src', 'packages/ui/src'].map((folder) =>
  join(ROOT, folder),
);
const SOURCE_FILE = /\.tsx?$/;
const TEST_FILE = /\.test\.tsx?$/;
const CATALOG_IMPORT =
  /import \{[^}]*\} from '(\.[^']*[mM]essages(?:\/\w+)?)'/g;
const LOOKUP = /\b(?:t|tn|rich|richn)\(\s*['"](\w+)['"]/g;
const DEFINE = 'defineMessages';

function allSources(): string[] {
  return SOURCES.flatMap((folder) =>
    readdirSync(folder, { recursive: true, encoding: 'utf8' })
      .filter((file) => SOURCE_FILE.test(file) && !TEST_FILE.test(file))
      .map((file) => join(folder, file)),
  );
}

/** Each catalog's entries as `key → every text of that entry`. */
function catalogEntries(file: string, text: string): Map<string, string[]> {
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  const entries = new Map<string, string[]>();
  const strings = (node: ts.Node): string[] =>
    ts.isStringLiteralLike(node)
      ? [node.text]
      : node.getChildren().flatMap(strings);
  const visit = (node: ts.Node): void => {
    if (
      ts.isCallExpression(node) &&
      node.expression.getText() === DEFINE &&
      node.arguments[0] &&
      ts.isObjectLiteralExpression(node.arguments[0])
    ) {
      for (const property of node.arguments[0].properties) {
        if (ts.isPropertyAssignment(property))
          entries.set(
            property.name.getText().replace(/['"]/g, ''),
            strings(property.initializer),
          );
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  return entries;
}

function usedKeysByCatalog(files: readonly string[]): Map<string, Set<string>> {
  const used = new Map<string, Set<string>>();
  for (const file of files) {
    const text = readFileSync(file, 'utf8');
    const catalogs = [...text.matchAll(CATALOG_IMPORT)].map((match) =>
      join(dirname(file), `${match[1] ?? ''}.ts`),
    );
    const keys = [...text.matchAll(LOOKUP)].map((match) => match[1] ?? '');
    for (const catalog of catalogs) {
      const set = used.get(catalog) ?? new Set<string>();
      for (const key of keys) set.add(key);
      used.set(catalog, set);
    }
  }
  return used;
}

const files = allSources();
const catalogs = files.flatMap((file) => {
  const text = readFileSync(file, 'utf8');
  return text.includes(`${DEFINE}({`)
    ? [{ file, entries: catalogEntries(file, text) }]
    : [];
});
const used = usedKeysByCatalog(files);

it('finds the catalogs', () => {
  expect(catalogs.length).toBeGreaterThan(20);
});

it('holds no empty message', () => {
  const empty = catalogs.flatMap(({ file, entries }) =>
    [...entries]
      .filter(([, texts]) => texts.some((text) => !text.trim()))
      .map(([key]) => `${file}: ${key}`),
  );
  expect(empty).toEqual([]);
});

it('holds no unused key', () => {
  const unused = catalogs.flatMap(({ file, entries }) =>
    [...entries.keys()]
      .filter((key) => !used.get(file)?.has(key))
      .map((key) => `${file}: ${key}`),
  );
  expect(unused).toEqual([]);
});

it('keeps sentence spacing out of messages', () => {
  const padded = catalogs.flatMap(({ file, entries }) =>
    [...entries]
      .filter(([, texts]) => texts.some((text) => text !== text.trim()))
      .map(([key]) => `${file}: ${key}`),
  );
  expect(padded).toEqual([]);
});
