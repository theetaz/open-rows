// SPDX-License-Identifier: Apache-2.0
import type { PlanDto, PlanNodeDto } from '@openrows/ipc';
import { heatOf } from './planModel';

/** Icicle envelope contains descendants; its filled self segment is exclusive. */
type FlameCell = {
  node: PlanNodeDto;
  x: number;
  width: number;
  span: number;
  heat: ReturnType<typeof heatOf>;
};

function measure(plan: PlanDto, node: PlanNodeDto): number {
  const value = plan.weight === 'time' ? node.exclusiveMs : node.exclusiveCost;
  return value !== null && Number.isFinite(value) ? Math.max(0, value) : 0;
}

/** Hotspot severity shares the tree's heat tokens; spills always stand out. */
export function flameHeat(node: PlanNodeDto): ReturnType<typeof heatOf> {
  if (node.hotspots.some((hotspot) => hotspot.kind === 'spill')) return 'high';
  const heat = heatOf(node.share);
  if (
    heat !== 'high' &&
    node.hotspots.some((hotspot) =>
      ['misestimate', 'fullScan'].includes(hotspot.kind),
    )
  )
    return 'medium';
  return heat;
}

/** Linear, iterative layout: self width / total weight, envelopes / subtree weight. */
export function flameLayout(plan: PlanDto): FlameCell[] {
  const weights = plan.nodes.map((node) => measure(plan, node));
  const total = weights.reduce((sum, value) => sum + value, 0);
  const own = total > 0 ? weights : weights.map(() => 1);
  const scale = total > 0 ? total : Math.max(plan.nodes.length, 1);
  const spans = [...own];
  const indices = new Map(plan.nodes.map((node, index) => [node.id, index]));
  for (let index = plan.nodes.length - 1; index >= 0; index--) {
    const parent = indices.get(plan.nodes[index]?.parent ?? -1);
    if (parent !== undefined)
      spans[parent] = (spans[parent] ?? 0) + (spans[index] ?? 0);
  }
  const cursors = new Map<number | null, number>();
  return plan.nodes.map((node, index) => {
    const x = cursors.get(node.parent) ?? 0;
    const width = (own[index] ?? 0) / scale;
    const span = (spans[index] ?? 0) / scale;
    cursors.set(node.parent, x + span);
    cursors.set(node.id, x + width);
    return { node, x, width, span, heat: flameHeat(node) };
  });
}

/** Arrow navigation follows pre-order vertically and hierarchy horizontally. */
export function flameNext(
  key: string,
  nodes: readonly PlanNodeDto[],
  id: number,
): number | undefined {
  const index = nodes.findIndex((node) => node.id === id);
  if (key === 'ArrowDown')
    return nodes[Math.min(index + 1, nodes.length - 1)]?.id;
  if (key === 'ArrowUp') return nodes[Math.max(index - 1, 0)]?.id;
  if (key === 'ArrowLeft') return nodes[index]?.parent ?? id;
  if (key === 'ArrowRight')
    return nodes.find((node) => node.parent === id)?.id ?? id;
  if (key === 'Home') return nodes[0]?.id;
  if (key === 'End') return nodes.at(-1)?.id;
  return undefined;
}
