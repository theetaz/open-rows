// SPDX-License-Identifier: Apache-2.0
import { memo, useMemo, useRef } from 'react';
import type { PlanDto, PlanNodeDto } from '@openrows/ipc';
import { flameLayout, flameNext } from './flameLayout';
import { nodeLabel, rowsText, selfText } from './planModel';
import styles from './Flame.module.css';

type Props = {
  plan: PlanDto;
  initialSelected: number;
  onInspect: (id: number) => void;
  onSelect: (id: number) => void;
};

/** SVG icicle with exclusive fills, subtree envelopes and roving keyboard focus. */
export const PlanFlame = memo(function PlanFlame(
  props: Props,
): React.JSX.Element {
  const { plan } = props;
  const cells = useMemo(() => flameLayout(plan), [plan]);
  const focused = useRef(props.initialSelected);
  const refs = useRef(new Map<number, SVGGElement>());
  const depth = plan.nodes.reduce((max, node) => Math.max(max, node.depth), 0);
  const { onInspect, onSelect } = props;
  const actions = useMemo(
    () => ({
      register: (id: number, element: SVGGElement | null): void => {
        if (element) refs.current.set(id, element);
        else refs.current.delete(id);
      },
      inspect: (id: number): void => {
        refs.current.get(focused.current)?.setAttribute('tabindex', '-1');
        refs.current.get(id)?.setAttribute('tabindex', '0');
        focused.current = id;
        onInspect(id);
      },
      select: onSelect,
      key: (event: React.KeyboardEvent, id: number): void => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          onSelect(id);
          return;
        }
        const next = flameNext(event.key, plan.nodes, id);
        if (next === undefined) return;
        event.preventDefault();
        refs.current.get(next)?.focus();
      },
    }),
    [plan, onInspect, onSelect],
  );
  return (
    <div className={styles.chart}>
      <p className={styles.legend}>
        Filled width: exclusive {plan.weight === 'time' ? 'time' : 'cost'}.
        Outlines include inputs.{' '}
        {plan.weight === 'none' && 'No measures: equal widths.'} Arrow keys
        explore; Enter opens the operator in Plan.
      </p>
      <svg
        role="group"
        aria-label="Plan icicle"
        viewBox={`0 0 1000 ${String((depth + 1) * 32)}`}
        style={{ height: (depth + 1) * 32 }}
      >
        {cells.map((cell) => (
          <FlameOperator
            key={cell.node.id}
            cell={cell}
            plan={plan}
            focused={cell.node.id === props.initialSelected}
            actions={actions}
          />
        ))}
      </svg>
    </div>
  );
}, sameDrawing);

// Selection is a mount-time starting point; focus updates only two SVG elements.
function sameDrawing(before: Props, after: Props): boolean {
  return (
    before.plan === after.plan &&
    before.onInspect === after.onInspect &&
    before.onSelect === after.onSelect
  );
}

function description(plan: PlanDto, node: PlanNodeDto): string {
  return `${nodeLabel(node)} · Self ${selfText(plan, node)} · Rows ${rowsText(plan, node)}`;
}

type Actions = {
  register: (id: number, element: SVGGElement | null) => void;
  inspect: (id: number) => void;
  select: (id: number) => void;
  key: (event: React.KeyboardEvent, id: number) => void;
};

const FlameOperator = memo(function FlameOperator(props: {
  cell: ReturnType<typeof flameLayout>[number];
  plan: PlanDto;
  focused: boolean;
  actions: Actions;
}): React.JSX.Element {
  const { node, x, width, span, heat } = props.cell;
  const { plan, focused } = props;
  return (
    <g
      role="button"
      aria-label={nodeLabel(node)}
      tabIndex={focused ? 0 : -1}
      data-heat={heat}
      className={styles.cell}
      ref={(element) => {
        props.actions.register(node.id, element);
      }}
      onFocus={() => {
        props.actions.inspect(node.id);
      }}
      onMouseEnter={() => {
        props.actions.inspect(node.id);
      }}
      onClick={() => {
        props.actions.select(node.id);
      }}
      onKeyDown={(event) => {
        props.actions.key(event, node.id);
      }}
    >
      <title>{description(plan, node)}</title>
      <rect
        className={styles.envelope}
        x={x * 1000}
        y={node.depth * 32}
        width={span * 1000}
        height={30}
      />
      <rect
        className={styles.self}
        x={x * 1000}
        y={node.depth * 32}
        width={width * 1000}
        height={30}
      />
      {span > 0.08 && (
        <text
          x={x * 1000 + 4}
          y={node.depth * 32 + 20}
          textLength={Math.min(span * 1000 - 8, node.nodeType.length * 7)}
          lengthAdjust="spacingAndGlyphs"
        >
          {node.nodeType}
        </text>
      )}
    </g>
  );
});
