// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import { tableLayoutKey } from '../../results/columnLayoutStore';
import { takeSeed, seededView, usePendingSeed } from './links/seeds';
import type { FilterDto } from '@openrows/ipc';
import { presentationOf, restoredView } from './presentation';
import type { TablePresentation } from './presentation';
import { readPresentation, writePresentation } from './presentationStore';
import type { TableTargetRef, TableView } from './model';
import type { TableViews } from './useTableViews';
import { useTrigger } from './useTrigger';

/**
 * Restore a table tab's remembered presentation (hidden columns, sort,
 * filters) before its first page, apply a foreign-key jump's filters when
 * one arrives, and remember the presentation as it changes (FR-GRID-12).
 * Returns the table's presentation key and the view to show now: before
 * the restore is stored, the restored view is derived synchronously so the
 * first page already loads with it.
 */
export function usePresentation({
  tabId,
  target,
  views,
  view,
}: {
  tabId: string;
  target: TableTargetRef;
  views: TableViews;
  view: TableView;
}): { key: string; view: TableView } {
  const key = tableLayoutKey(target);
  const seed = usePendingSeed(key);
  const shown = view.restored
    ? view
    : opened(view, readPresentation(key), seed);
  useTrigger(!view.restored ? 'restore' : seed ? 'seed' : null, () => {
    const filters = takeSeed(key);
    views.setView(tabId, (current) =>
      current.restored
        ? filters
          ? seededView(current, filters)
          : current
        : opened(current, readPresentation(key), filters),
    );
  });
  const remembered = JSON.stringify([
    view.hidden,
    view.sort,
    view.rows,
    view.applied,
    view.combinator,
  ]);
  const { restored } = view;
  useEffect(() => {
    if (!restored) return;
    const [hidden, sort, rows, applied, combinator] = JSON.parse(
      remembered,
    ) as [
      TableView['hidden'],
      TableView['sort'],
      TableView['rows'],
      TableView['applied'],
      TableView['combinator'],
    ];
    writePresentation(
      key,
      presentationOf(
        { hidden, sort, rows, applied, combinator },
        readPresentation(key).saved,
      ),
    );
  }, [key, remembered, restored]);
  return { key, view: shown };
}

/** A newly opened tab's view: remembered presentation, then any jump's filters. */
function opened(
  view: TableView,
  stored: TablePresentation,
  seed: readonly FilterDto[] | undefined,
): TableView {
  const restored = restoredView(view, stored);
  return seed ? seededView(restored, seed) : restored;
}
