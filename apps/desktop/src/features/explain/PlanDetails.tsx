// SPDX-License-Identifier: Apache-2.0
import { Fragment } from 'react';
import type { PlanDetailDto, PlanDto, PlanNodeDto } from '@openrows/ipc';
import {
  formatMs,
  formatNumber,
  formatShare,
  hotspotText,
  nodeLabel,
} from './planModel';
import styles from './Explain.module.css';

type Row = [label: string, value: string];

/** `format(value)`, or `null` when the engine did not report it. */
function shown(
  value: number | null,
  format: (value: number | null) => string,
): string | null {
  return value === null ? null : format(value);
}

/** Measured values of the node; rows without a value are left out. */
function measures(plan: PlanDto, node: PlanNodeDto): Row[] {
  const rows: [string, string | null][] = [
    ['Relation', node.relation],
    ['Index', node.index],
    ['Share of plan', plan.weight === 'none' ? null : formatShare(node.share)],
    ['Self time', shown(node.exclusiveMs, formatMs)],
    ['Total time', shown(node.inclusiveMs, formatMs)],
    ['Self cost', shown(node.exclusiveCost, formatNumber)],
    ['Startup cost', shown(node.startupCost, formatNumber)],
    ['Total cost', shown(node.totalCost, formatNumber)],
    ['Estimated rows', shown(node.estimatedRows, formatNumber)],
    ['Actual rows', shown(node.actualRows, formatNumber)],
    ['Loops', shown(node.loops, formatNumber)],
    ['First row', shown(node.actualStartupMs, formatMs)],
    ['All rows (per loop)', shown(node.actualTotalMs, formatMs)],
  ];
  return rows.filter((row): row is Row => row[1] !== null);
}

function buffers(node: PlanNodeDto): Row[] {
  const buffers = node.buffers;
  if (!buffers) return [];
  return [
    ['Shared hit', formatNumber(buffers.sharedHit)],
    ['Shared read', formatNumber(buffers.sharedRead)],
    ['Shared dirtied', formatNumber(buffers.sharedDirtied)],
    ['Shared written', formatNumber(buffers.sharedWritten)],
    ['Temp read', formatNumber(buffers.tempRead)],
    ['Temp written', formatNumber(buffers.tempWritten)],
  ];
}

function List(props: {
  rows: readonly Row[];
  code?: boolean;
}): React.JSX.Element {
  return (
    <dl>
      {props.rows.map(([label, value], index) => (
        <Fragment key={index}>
          <dt>{label}</dt>
          <dd className={props.code ? styles.code : undefined}>{value}</dd>
        </Fragment>
      ))}
    </dl>
  );
}

const rowsOf = (details: readonly PlanDetailDto[]): Row[] =>
  details.map((detail) => [detail.label, detail.value]);

/** The selected operator: measures, hotspots, conditions, buffers and every other value. */
export function PlanDetails(props: {
  plan: PlanDto;
  node: PlanNodeDto;
}): React.JSX.Element {
  const { plan, node } = props;
  const blocks = buffers(node);
  return (
    <aside className={styles.details} aria-label="Operator details">
      <h3>{nodeLabel(node)}</h3>
      <List rows={measures(plan, node)} />
      {node.hotspots.length > 0 && (
        <>
          <h4>Hotspots</h4>
          <ul aria-label="Hotspots">
            {node.hotspots.map((hotspot, index) => (
              <li key={index}>{hotspotText(hotspot).detail}</li>
            ))}
          </ul>
        </>
      )}
      {node.conditions.length > 0 && (
        <>
          <h4>Conditions</h4>
          <List rows={rowsOf(node.conditions)} code />
        </>
      )}
      {blocks.length > 0 && (
        <>
          <h4>Buffers (blocks, with inputs)</h4>
          <List rows={blocks} />
        </>
      )}
      {node.details.length > 0 && (
        <>
          <h4>Details</h4>
          <List rows={rowsOf(node.details)} code />
        </>
      )}
    </aside>
  );
}
