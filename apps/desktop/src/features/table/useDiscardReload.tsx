// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { DiscardSheet } from './DiscardSheet';
import type { DiscardKind } from './DiscardSheet';
import { restoreNext } from './gridPosition';
import type { PositionMemory } from './gridPosition';
import type { RowEditing } from './useRowEditing';

/** Explicit Discard Changes and Reload of a table tab (FR-DATA-08). */
export type DiscardReload = {
  /** Ask, then drop the staged edits and reload the rows. */
  discard: () => void;
  /** Reload the rows; with edits staged, ask whether to keep them. */
  reload: () => void;
  /** The confirmation sheet, while one is open. */
  sheet: React.JSX.Element | null;
};

/**
 * Discard and Reload with confirmation whenever edits are staged; a reload
 * returns the grid to its scroll position and focused cell where the new
 * page allows it.
 */
export function useDiscardReload({
  editing,
  reload,
  position,
}: {
  editing: RowEditing;
  /** Load the page again. */
  reload: () => void;
  position: React.RefObject<PositionMemory>;
}): DiscardReload {
  const [asking, setAsking] = useState<DiscardKind | null>(null);
  const count = editing.history.current.length;
  const refresh = (): void => {
    restoreNext(position);
    reload();
  };
  const drop = (): void => {
    const uncertain = editing.history.uncertain;
    editing.discard();
    // An uncertain commit is discarded with a reload already.
    if (uncertain) restoreNext(position);
    else refresh();
  };
  const close = (): void => {
    setAsking(null);
  };
  return {
    discard: () => {
      if (count && !editing.busy) setAsking('discard');
    },
    reload: () => {
      if (count) setAsking('reload');
      else refresh();
    },
    sheet: asking && (
      <DiscardSheet
        kind={asking}
        count={count}
        onClose={close}
        onKeep={() => {
          close();
          refresh();
        }}
        onDiscard={() => {
          close();
          drop();
        }}
      />
    ),
  };
}
