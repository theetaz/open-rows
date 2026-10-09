// SPDX-License-Identifier: Apache-2.0
import type { EngineDto } from '@openrows/ipc';

/** Catalog-level dialect policy shared by filtering and saved-object listings. */
export function isSystemObject(
  engine: EngineDto,
  object: { schema: string; name: string },
): boolean {
  const schema = object.schema.toLowerCase();
  switch (engine) {
    case 'postgres':
      return (
        schema === 'information_schema' ||
        schema === 'pg_catalog' ||
        schema.startsWith('pg_toast')
      );
    case 'mysql':
    case 'mariadb':
      return [
        'mysql',
        'sys',
        'performance_schema',
        'information_schema',
      ].includes(schema);
    case 'sqlserver':
      return schema === 'sys' || schema === 'information_schema';
    case 'sqlite':
      return object.name.toLowerCase().startsWith('sqlite_');
  }
}
