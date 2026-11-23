// SPDX-License-Identifier: Apache-2.0
import type { Connections } from '../connections';
import { copyLink, objectLink } from '../links/linkText';

/** Copy the link of a schema of the active connection's `database`. */
export function schemaLinkCopier(
  connections: Connections,
  database: string,
): ((schema: string) => void) | undefined {
  const copy = linkCopier(connections);
  return (
    copy &&
    ((schema) => {
      copy({ database, schema });
    })
  );
}

/** Copy the link of a schema or relation of the active connection. */
export function linkCopier(
  connections: Connections,
): ((target: Parameters<typeof objectLink>[1]) => void) | undefined {
  const profile = connections.active;
  if (!profile) return undefined;
  return (target) => {
    copyLink(objectLink(profile, target)).catch(() => undefined);
  };
}
