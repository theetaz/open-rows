// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Badge, Icon } from '@openrows/ui';
import type { PlanDto, PlanNodeDto } from '@openrows/ipc';
import {
  formatNumber,
  formatShare,
  hasChildren,
  heatOf,
  hotspotText,
  rowsText,
  selfText,
  visibleNodes,
} from './planModel';
import styles from './Explain.module.css';

type Props = {
  plan: PlanDto;
  selected: number;
  onSelect: (id: number) => void;
};

/** The id to select after an arrow key, or `undefined` when the key is not one. */
function nextSelection(
  key: string,
  rows: readonly PlanNodeDto[],
  current: PlanNodeDto,
): number | undefined {
  const index = rows.indexOf(current);
  if (key === 'ArrowDown')
    return rows[Math.min(index + 1, rows.length - 1)]?.id;
  if (key === 'ArrowUp') return rows[Math.max(index - 1, 0)]?.id;
  if (key === 'Home') return rows[0]?.id;
  if (key === 'End') return rows.at(-1)?.id;
  return undefined;
}

/** Collapse/expand on ←/→; ← on a leaf or collapsed node selects the parent. */
function toggleKey(
  key: string,
  node: PlanNodeDto,
  open: { expanded: boolean; parent: boolean },
): 'collapse' | 'expand' | 'parent' | undefined {
  if (key === 'ArrowRight' && open.parent && !open.expanded) return 'expand';
  if (key !== 'ArrowLeft') return undefined;
  if (open.parent && open.expanded) return 'collapse';
  return node.parent === null ? undefined : 'parent';
}

/**
 * Operators as an indented tree: share bar coloured by heat, exclusive
 * time (or cost), actual/estimated rows, loops and hotspot badges.
 */
export function PlanTree({
  plan,
  selected,
  onSelect,
}: Props): React.JSX.Element {
  const [collapsed, setCollapsed] = useState<ReadonlySet<number>>(new Set());
  const rows = visibleNodes(plan.nodes, collapsed);
  const current = rows.find((node) => node.id === selected) ?? rows[0];
  const toggle = (id: number, open: boolean): void => {
    const next = new Set(collapsed);
    if (open) next.delete(id);
    else next.add(id);
    setCollapsed(next);
  };
  const onKeyDown = (event: React.KeyboardEvent): void => {
    if (!current) return;
    const parent = hasChildren(plan.nodes, current);
    const action = toggleKey(event.key, current, {
      parent,
      expanded: !collapsed.has(current.id),
    });
    const next = nextSelection(event.key, rows, current);
    if (action === undefined && next === undefined) return;
    event.preventDefault();
    if (action === 'parent' && current.parent !== null)
      onSelect(current.parent);
    else if (action) toggle(current.id, action === 'expand');
    else if (next !== undefined) onSelect(next);
  };
  return (
    <div className={styles.tree}>
      <div className={styles.header} aria-hidden="true">
        <span>Operator</span>
        <span>Share</span>
        <span>{plan.weight === 'time' ? 'Self time' : 'Self cost'}</span>
        <span>{plan.analyzed ? 'Rows actual / est.' : 'Rows est.'}</span>
        <span>Loops</span>
      </div>
      <div
        role="tree"
        aria-label="Plan operators"
        tabIndex={0}
        aria-activedescendant={
          current ? `plan-node-${String(current.id)}` : undefined
        }
        onKeyDown={onKeyDown}
      >
        {rows.map((node) => (
          <PlanRow
            key={node.id}
            plan={plan}
            node={node}
            selected={node.id === current?.id}
            expanded={
              hasChildren(plan.nodes, node)
                ? !collapsed.has(node.id)
                : undefined
            }
            onSelect={onSelect}
            onToggle={toggle}
          />
        ))}
      </div>
    </div>
  );
}

function PlanRow(props: {
  plan: PlanDto;
  node: PlanNodeDto;
  selected: boolean;
  /** `undefined` for leaves. */
  expanded: boolean | undefined;
  onSelect: (id: number) => void;
  onToggle: (id: number, open: boolean) => void;
}): React.JSX.Element {
  const { plan, node, expanded } = props;
  const heat = heatOf(node.share);
  return (
    <div
      id={`plan-node-${String(node.id)}`}
      role="treeitem"
      aria-level={node.depth + 1}
      aria-selected={props.selected}
      aria-expanded={expanded}
      data-heat={heat}
      className={styles.row}
      onClick={() => {
        props.onSelect(node.id);
      }}
    >
      <span
        className={styles.operator}
        style={{ paddingInlineStart: `${String(node.depth)}rem` }}
      >
        {expanded === undefined ? (
          <span className={styles.twisty} aria-hidden="true" />
        ) : (
          <button
            type="button"
            tabIndex={-1}
            className={styles.twisty}
            aria-label={expanded ? 'Collapse' : 'Expand'}
            aria-expanded={expanded}
            onClick={(event) => {
              event.stopPropagation();
              props.onToggle(node.id, !expanded);
            }}
          >
            <Icon name="chevronDown" />
          </button>
        )}
        <NodeName node={node} />
      </span>
      <span
        className={styles.bar}
        data-heat={heat}
        title={formatShare(node.share)}
      >
        <span
          style={{
            inlineSize: `${String(Math.round((node.share ?? 0) * 100))}%`,
          }}
        />
      </span>
      <span>{selfText(plan, node)}</span>
      <span>{rowsText(plan, node)}</span>
      <span>{plan.analyzed ? formatNumber(node.loops) : '—'}</span>
    </div>
  );
}

function NodeName({ node }: { node: PlanNodeDto }): React.JSX.Element {
  const object = [node.relation, node.index && `using ${node.index}`]
    .filter(Boolean)
    .join(' ');
  return (
    <>
      <span className={styles.name} title={node.nodeType}>
        {node.nodeType}
        {object && <span className={styles.object}> {object}</span>}
      </span>
      {node.hotspots.length > 0 && (
        <span className={styles.flags}>
          {node.hotspots.map((hotspot, index) => {
            const text = hotspotText(hotspot);
            return (
              <Badge key={index} tone={text.tone} title={text.detail}>
                {text.label}
              </Badge>
            );
          })}
        </span>
      )}
    </>
  );
}
