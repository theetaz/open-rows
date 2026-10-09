// SPDX-License-Identifier: Apache-2.0
import { recordRecent } from '../schema/sidebarStore';
import type { ObjectIdentityDto } from '@openrows/ipc';
import type { QueryTabs, TabScope } from '../query';

/** One read-only catalog tab: an object, extension or scoped account. */
export type InspectionTarget =
  | { kind: 'object'; object: ObjectIdentityDto }
  | { kind: 'extension'; name: string }
  | { kind: 'role'; name: string; scope: string };

/** Open or refocus the same catalog identity, including routine overloads. */
export function openInspection(
  tabs: QueryTabs,
  scope: TabScope,
  target: InspectionTarget,
  preview = false,
): void {
  if (target.kind === 'object')
    recordRecent({
      ...target.object,
      profileId: scope.connectionId,
      database: scope.database,
    });
  const identity = JSON.stringify(target);
  const existing = tabs
    .tabsFor(scope)
    .find(
      (tab) => tab.inspection && JSON.stringify(tab.inspection) === identity,
    );
  if (existing) {
    tabs.select(existing.id);
    if (!preview) tabs.keep(existing.id);
    return;
  }
  const title =
    target.kind === 'object'
      ? target.object.name +
        (target.object.arguments === null ? '' : `(${target.object.arguments})`)
      : target.name;
  tabs.open(scope, { title, inspection: target, preview });
}
