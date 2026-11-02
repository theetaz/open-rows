// SPDX-License-Identifier: Apache-2.0
import type { Connections } from '../connections';
import type { QueryTabs, TabScope } from '../query';
import type { Catalog } from '../schema';
import { useOpenLinks } from '../links/useOpenLinks';
import { useWindowWork } from '../windows/useWindowWork';
import { useNativeFiles } from './useNativeFiles';

/**
 * What reaches this window from outside: opened files, `openrows://` links
 * (confirmed in `linkSheet` before anything connects) and tabs moved from
 * other windows.
 */
export function useWorkspaceHandoffs(input: {
  connections: Connections;
  tabs: QueryTabs;
  catalog: Catalog;
  scope: TabScope | undefined;
  enabled: boolean;
}): { fileNotice: string | null; linkSheet: React.JSX.Element | null } {
  const { connections, tabs, catalog, scope, enabled } = input;
  const links = useOpenLinks({ connections, tabs, catalog });
  const fileNotice = useNativeFiles({
    tabs,
    scope,
    connections,
    enabled,
    onLink: links.request,
  });
  useWindowWork({
    tabs,
    connections,
    enabled,
    database: scope?.database ?? '',
  });
  return { fileNotice, linkSheet: links.sheet };
}
