// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { EngineDto } from '@openrows/ipc';
import { stepOrder } from './flowDraft';
import type { StepId } from './flowDraft';

export type Direction = 'forward' | 'back' | 'none';

export type FlowSteps = {
  order: readonly StepId[];
  current: StepId;
  /** How the page arrived, for its slide-in. */
  direction: Direction;
  /** The furthest step reached; later steps stay locked. */
  reached: number;
  go: (step: StepId) => void;
  next: () => void;
  back: () => void;
};

/**
 * Step navigation: a new connection unlocks each step as it is reached, an
 * edit starts at Details with every step open.
 */
export function useFlowSteps(
  engine: EngineDto,
  editing: boolean,
  start: StepId,
): FlowSteps {
  const order = stepOrder(engine, editing);
  const [state, setState] = useState<{
    current: StepId;
    direction: Direction;
    reached: number;
  }>(() => ({
    current: start,
    direction: 'none',
    reached: editing ? order.length - 1 : order.indexOf(start),
  }));
  const current = order.includes(state.current) ? state.current : 'details';
  const index = order.indexOf(current);
  const go = (step: StepId): void => {
    const target = order.indexOf(step);
    if (target < 0 || step === current) return;
    setState((previous) => ({
      current: step,
      direction: target < index ? 'back' : 'forward',
      reached: Math.max(previous.reached, target),
    }));
  };
  return {
    order,
    current,
    direction: state.direction,
    reached: Math.min(state.reached, order.length - 1),
    go,
    next: () => {
      const step = order[index + 1];
      if (step) go(step);
    },
    back: () => {
      const step = order[index - 1];
      if (step) go(step);
    },
  };
}
