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
import { t } from './messages';

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
    [t('relation'), node.relation],
    [t('index'), node.index],
    [t('sharePlan'), plan.weight === 'none' ? null : formatShare(node.share)],
    [t('selfTime'), shown(node.exclusiveMs, formatMs)],
    [t('totalTime'), shown(node.inclusiveMs, formatMs)],
    [t('selfCost'), shown(node.exclusiveCost, formatNumber)],
    [t('startupCost'), shown(node.startupCost, formatNumber)],
    [t('totalCost'), shown(node.totalCost, formatNumber)],
    [t('estimatedRows'), shown(node.estimatedRows, formatNumber)],
    [t('actualRows'), shown(node.actualRows, formatNumber)],
    [t('loops'), shown(node.loops, formatNumber)],
    [t('firstRow'), shown(node.actualStartupMs, formatMs)],
    [t('allRowsPer'), shown(node.actualTotalMs, formatMs)],
  ];
  return rows.filter((row): row is Row => row[1] !== null);
}

function buffers(node: PlanNodeDto): Row[] {
  const buffers = node.buffers;
  if (!buffers) return [];
  return [
    [t('sharedHit'), formatNumber(buffers.sharedHit)],
    [t('sharedRead'), formatNumber(buffers.sharedRead)],
    [t('sharedDirtied'), formatNumber(buffers.sharedDirtied)],
    [t('sharedWritten'), formatNumber(buffers.sharedWritten)],
    [t('tempRead'), formatNumber(buffers.tempRead)],
    [t('tempWritten'), formatNumber(buffers.tempWritten)],
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
    <aside className={styles.details} aria-label={t('operatorDetails')}>
      <h3>{nodeLabel(node)}</h3>
      <List rows={measures(plan, node)} />
      {node.hotspots.length > 0 && (
        <>
          <h4>{t('hotspots')}</h4>
          <ul aria-label={t('hotspots')}>
            {node.hotspots.map((hotspot, index) => (
              <li key={index}>{hotspotText(hotspot).detail}</li>
            ))}
          </ul>
        </>
      )}
      {node.conditions.length > 0 && (
        <>
          <h4>{t('conditions')}</h4>
          <List rows={rowsOf(node.conditions)} code />
        </>
      )}
      {blocks.length > 0 && (
        <>
          <h4>{t('buffersBlocksInputs')}</h4>
          <List rows={blocks} />
        </>
      )}
      {node.details.length > 0 && (
        <>
          <h4>{t('details')}</h4>
          <List rows={rowsOf(node.details)} code />
        </>
      )}
    </aside>
  );
}
