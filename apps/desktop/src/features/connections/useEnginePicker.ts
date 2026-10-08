// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { EngineDto } from '@openrows/ipc';
import { matchingChoices } from './engines';
import type { EngineChoice } from './engines';

/** Tiles per row in the picker grid. */
const PICKER_COLUMNS = 5;

const STEPS: Partial<Record<string, number>> = {
  ArrowLeft: -1,
  ArrowRight: 1,
  ArrowUp: -PICKER_COLUMNS,
  ArrowDown: PICKER_COLUMNS,
};

/** The tile an arrow key moves to; edges stop the move, as in Finder icon view. */
export function moveSelection(
  visible: readonly EngineChoice[],
  current: string | undefined,
  key: string,
): string | undefined {
  const step = STEPS[key];
  const index = visible.findIndex((choice) => choice.id === current);
  if (step === undefined || index < 0) return current;
  return (visible[index + step] ?? visible[index])?.id;
}

export type EnginePicker = {
  query: string;
  setQuery: (query: string) => void;
  visible: EngineChoice[];
  /** The outlined tile (the first match when the selection is filtered out). */
  current: EngineChoice | undefined;
  select: (id: string) => void;
  /** Arrow keys: true when the key belongs to the grid. */
  move: (key: string) => boolean;
  /** Open the form for the current engine; types without a driver do nothing. */
  create: () => void;
};

/** Search and selection of the connection-type picker. */
export function useEnginePicker(
  onCreate: (engine: EngineDto) => void,
): EnginePicker {
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState('postgres');
  const visible = matchingChoices(query);
  const current =
    visible.find((choice) => choice.id === selected) ?? visible[0];
  return {
    query,
    setQuery,
    visible,
    current,
    select: setSelected,
    move: (key) => {
      if (STEPS[key] === undefined) return false;
      const next = moveSelection(visible, current?.id, key);
      if (next) setSelected(next);
      return true;
    },
    create: () => {
      if (current?.engine) onCreate(current.engine);
    },
  };
}
