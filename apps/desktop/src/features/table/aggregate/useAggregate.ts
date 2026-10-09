// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { displayWindow } from '../../../results/displayWindow';
import { readSelection } from '../../../results/copy/read';
import { selectedCells } from '../bulk/cells';
import { copySource } from '../gridSelection';
import type { GridSelection } from '../gridSelection';
import { aggregate, aggregateText } from './aggregates';

/** Pause after a selection change before its cells are read. */
const READ_DELAY_MS = 120;

/** Whether the selection spans more than one cell. */
function several(selection: GridSelection | null): boolean {
  return selection !== null && selectedCells(selection.ranges, 2).length > 1;
}

/**
 * The footer summary of a multi-cell selection (FR-GRID-10), read from
 * the result store off the paint path; `null` for one cell or none.
 */
export function useAggregate(selection: GridSelection | null): string | null {
  const [text, setText] = useState<{ key: string; text: string } | null>(null);
  const latest = useRef(selection);
  useEffect(() => {
    latest.current = selection;
  });
  const key = several(selection)
    ? JSON.stringify([selection?.result.id, selection?.ranges])
    : null;
  useEffect(() => {
    const current = latest.current;
    if (!key || !current) return;
    let live = true;
    const timer = window.setTimeout(() => {
      readSelection(copySource(current), displayWindow).then(
        (table) => {
          if (live) setText({ key, text: aggregateText(aggregate(table)) });
        },
        (error: unknown) => {
          if (live)
            setText({
              key,
              text: error instanceof Error ? error.message : String(error),
            });
        },
      );
    }, READ_DELAY_MS);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [key]);
  return key && text?.key === key ? text.text : null;
}
