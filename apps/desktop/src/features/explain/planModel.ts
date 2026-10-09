// SPDX-License-Identifier: Apache-2.0
import type { PlanDto, PlanHotspotDto, PlanNodeDto } from '@openrows/ipc';

/** How hot an operator is, from its share of the plan. */
type Heat = 'none' | 'low' | 'medium' | 'high';

/** `high` from half the plan, `medium` from a fifth, `low` from 5 %. */
export function heatOf(share: number | null): Heat {
  const value = share ?? 0;
  if (value >= 0.5) return 'high';
  if (value >= 0.2) return 'medium';
  if (value >= 0.05) return 'low';
  return 'none';
}

const EMPTY = '—';

/** `0.012 ms`, `12.3 ms`, `1.23 s`. */
export function formatMs(ms: number | null): string {
  if (ms === null) return EMPTY;
  if (ms >= 1000) return `${(ms / 1000).toFixed(2)} s`;
  if (ms >= 10) return `${ms.toFixed(1)} ms`;
  return `${ms.toFixed(ms >= 1 ? 2 : 3)} ms`;
}

/** Rows, loops and costs: grouped, at most two decimals. */
export function formatNumber(value: number | null): string {
  if (value === null) return EMPTY;
  return value.toLocaleString('en-US', { maximumFractionDigits: 2 });
}

/** `45%`; below 1 % as `<1%`. */
export function formatShare(share: number | null): string {
  const value = share ?? 0;
  if (value > 0 && value < 0.01) return '<1%';
  return `${String(Math.round(value * 100))}%`;
}

/** `Seq Scan on public.orders using orders_pkey`. */
export function nodeLabel(node: PlanNodeDto): string {
  const on = node.relation ? ` on ${node.relation}` : '';
  const using = node.index ? ` using ${node.index}` : '';
  return `${node.nodeType}${on}${using}`;
}

/** The node's exclusive measure in the plan's weight, as text. */
export function selfText(plan: PlanDto, node: PlanNodeDto): string {
  if (plan.weight === 'time') return formatMs(node.exclusiveMs);
  if (plan.weight === 'cost') return formatNumber(node.exclusiveCost);
  return EMPTY;
}

/** `200 / 1` (actual / estimated) for analysed plans, else the estimate. */
export function rowsText(plan: PlanDto, node: PlanNodeDto): string {
  if (!plan.analyzed) return formatNumber(node.estimatedRows);
  return `${formatNumber(node.actualRows)} / ${formatNumber(node.estimatedRows)}`;
}

/** How far actual and estimated rows differ, as a whole factor ≥ 1. */
export function misestimateFactor(estimated: number, actual: number): number {
  const high = Math.max(estimated, actual, 1);
  const low = Math.max(Math.min(estimated, actual), 1);
  return Math.round(high / low);
}

/** A hotspot as a short badge label, an explanation and a tone. */
type HotspotText = {
  label: string;
  detail: string;
  tone: 'danger' | 'warning' | 'info';
};

export function hotspotText(hotspot: PlanHotspotDto): HotspotText {
  switch (hotspot.kind) {
    case 'expensive':
      return {
        label: `${formatShare(hotspot.share)} of plan`,
        detail: `Takes ${formatShare(hotspot.share)} of the plan on its own.`,
        tone: 'danger',
      };
    case 'misestimate': {
      const estimated = hotspot.estimated ?? 0;
      const actual = hotspot.actual ?? 0;
      return {
        label: `Rows ×${String(misestimateFactor(estimated, actual))}`,
        detail: `Estimated ${formatNumber(estimated)} rows per loop, got ${formatNumber(actual)}: statistics may be stale (run ANALYZE).`,
        tone: 'warning',
      };
    }
    case 'fullScan':
      return {
        label: 'Full scan',
        detail:
          hotspot.rows === null
            ? 'Reads the whole table; an index may help.'
            : `Reads ${formatNumber(hotspot.rows)} rows of the whole table; an index may help.`,
        tone: 'warning',
      };
    case 'spill':
      return { label: 'Disk spill', detail: hotspot.reason, tone: 'danger' };
  }
}

/** Every hotspot with its node, the most expensive first. */
export function hotspotsOf(
  plan: PlanDto,
): { node: PlanNodeDto; hotspot: PlanHotspotDto }[] {
  const all = plan.nodes.flatMap((node) =>
    node.hotspots.map((hotspot) => ({ node, hotspot })),
  );
  const rank = (hotspot: PlanHotspotDto): number =>
    hotspot.kind === 'expensive' ? -(hotspot.share ?? 0) : 0;
  return all.sort((left, right) => rank(left.hotspot) - rank(right.hotspot));
}

/** Whether a node of the pre-order list has inputs. */
export function hasChildren(
  nodes: readonly PlanNodeDto[],
  node: PlanNodeDto,
): boolean {
  return nodes[node.id + 1]?.parent === node.id;
}

/** Nodes not hidden under a collapsed ancestor, in pre-order. */
export function visibleNodes(
  nodes: readonly PlanNodeDto[],
  collapsed: ReadonlySet<number>,
): PlanNodeDto[] {
  const visible: PlanNodeDto[] = [];
  let hiddenBelow: number | null = null;
  for (const node of nodes) {
    if (hiddenBelow !== null && node.depth > hiddenBelow) continue;
    hiddenBelow = collapsed.has(node.id) ? node.depth : null;
    visible.push(node);
  }
  return visible;
}

/** The plan's headline: kind, times and whether changes were rolled back. */
export function planSummary(plan: PlanDto): string[] {
  const parts = [plan.analyzed ? 'Explain Analyze' : 'Explain'];
  if (plan.executionMs !== null)
    parts.push(`Execution ${formatMs(plan.executionMs)}`);
  if (plan.planningMs !== null)
    parts.push(`Planning ${formatMs(plan.planningMs)}`);
  parts.push(`${String(plan.nodes.length)} operators`);
  if (plan.rolledBack) parts.push('Changes rolled back');
  return parts;
}
