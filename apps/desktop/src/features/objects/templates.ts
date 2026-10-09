// SPDX-License-Identifier: Apache-2.0
import type { EngineDto, RoutineKindDto } from '@openrows/ipc';

const mysqlLike = (engine: EngineDto): boolean =>
  engine === 'mysql' || engine === 'mariadb';

/** A double-quoted (or MySQL back-quoted) identifier. */
function quoted(engine: EngineDto, name: string): string {
  return mysqlLike(engine)
    ? `\`${name.replaceAll('`', '``')}\``
    : `"${name.replaceAll('"', '""')}"`;
}

/** The starting definition of a new trigger on `table`. */
export function triggerTemplate(
  engine: EngineDto,
  schema: string,
  table: string,
): string {
  const on = `${quoted(engine, schema)}.${quoted(engine, table)}`;
  if (mysqlLike(engine))
    return `CREATE TRIGGER ${table}_before_insert\nBEFORE INSERT ON ${on}\nFOR EACH ROW\nBEGIN\n  SET NEW.updated_at = NOW();\nEND`;
  if (engine === 'sqlite')
    return `CREATE TRIGGER ${quoted(engine, schema)}.${table}_after_insert\nAFTER INSERT ON ${quoted(engine, table)}\nFOR EACH ROW\nBEGIN\n  SELECT 1;\nEND`;
  const fn = `${quoted(engine, schema)}.${quoted(engine, `${table}_touch`)}`;
  return `CREATE OR REPLACE FUNCTION ${fn}()\nRETURNS trigger\nLANGUAGE plpgsql\nAS $$\nBEGIN\n  NEW.updated_at := now();\n  RETURN NEW;\nEND\n$$;\n\nCREATE TRIGGER ${table}_touch\nBEFORE UPDATE ON ${on}\nFOR EACH ROW\nEXECUTE FUNCTION ${fn}();`;
}

/** The starting definition of a new function or procedure in `schema`. */
export function routineTemplate(
  engine: EngineDto,
  schema: string,
  routine: RoutineKindDto,
): string {
  const name = `${quoted(engine, schema)}.${quoted(engine, `new_${routine}`)}`;
  if (mysqlLike(engine))
    return routine === 'function'
      ? `CREATE FUNCTION ${name}()\nRETURNS INT\nDETERMINISTIC\nBEGIN\n  RETURN 1;\nEND`
      : `CREATE PROCEDURE ${name}()\nBEGIN\n  SELECT 1;\nEND`;
  return routine === 'function'
    ? `CREATE OR REPLACE FUNCTION ${name}()\nRETURNS integer\nLANGUAGE sql\nAS $$\n  SELECT 1;\n$$`
    : `CREATE OR REPLACE PROCEDURE ${name}()\nLANGUAGE plpgsql\nAS $$\nBEGIN\n  NULL;\nEND\n$$`;
}
