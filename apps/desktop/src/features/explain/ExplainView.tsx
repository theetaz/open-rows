// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SegmentedControl } from '@openrows/ui';
import type { PlanDto } from '@openrows/ipc';
import { PlanTree } from './PlanTree';
import { PlanDetails } from './PlanDetails';
import { hotspotText, hotspotsOf, nodeLabel, planSummary } from './planModel';
import styles from './Explain.module.css';

type ExplainViewProps = { plan: PlanDto };

type Tab = 'plan' | 'raw';

const TABS = [
  { value: 'plan', label: 'Plan' },
  { value: 'raw', label: 'Raw' },
] as const;

/**
 * A plan result tab: the operator tree with heat and hotspots and the
 * selected operator's details, or the engine's raw output; Copy Plan copies
 * the raw output.
 */
export function ExplainView({ plan }: ExplainViewProps): React.JSX.Element {
  const [tab, setTab] = useState<Tab>('plan');
  const [selected, setSelected] = useState(0);
  const node = plan.nodes[selected] ?? plan.nodes[0];
  return (
    <section className={styles.view} aria-label="Query plan">
      <div className={styles.toolbar}>
        <SegmentedControl
          size="compact"
          aria-label="Plan view"
          options={TABS}
          value={tab}
          onValueChange={setTab}
        />
        <span className={styles.summary}>{planSummary(plan).join(' · ')}</span>
        <CopyPlan raw={plan.raw} />
      </div>
      {tab === 'raw' ? (
        <pre className={styles.raw} aria-label="Raw plan" tabIndex={0}>
          {plan.raw}
        </pre>
      ) : (
        <>
          <HotspotStrip plan={plan} onSelect={setSelected} />
          <div className={styles.body}>
            <PlanTree plan={plan} selected={selected} onSelect={setSelected} />
            {node && <PlanDetails plan={plan} node={node} />}
          </div>
        </>
      )}
    </section>
  );
}

function CopyPlan({ raw }: { raw: string }): React.JSX.Element {
  const [copied, setCopied] = useState(false);
  return (
    <Button
      size="sm"
      variant="ghost"
      onClick={() => {
        navigator.clipboard.writeText(raw).then(
          () => {
            setCopied(true);
          },
          () => {
            setCopied(false);
          },
        );
      }}
    >
      {copied ? 'Copied' : 'Copy Plan'}
    </Button>
  );
}

/** Every hotspot as a chip selecting its operator. */
function HotspotStrip(props: {
  plan: PlanDto;
  onSelect: (id: number) => void;
}): React.JSX.Element {
  const hotspots = hotspotsOf(props.plan);
  return (
    <ul className={styles.hotspots} aria-label="Hotspots">
      <li className={styles.hotspotCount}>
        {hotspots.length
          ? `${String(hotspots.length)} hotspot${hotspots.length === 1 ? '' : 's'}:`
          : 'No hotspots found.'}
      </li>
      {hotspots.map(({ node, hotspot }, index) => {
        const text = hotspotText(hotspot);
        return (
          <li key={index}>
            <button
              type="button"
              className={styles.hotspot}
              title={text.detail}
              onClick={() => {
                props.onSelect(node.id);
              }}
            >
              <strong>{text.label}</strong> {nodeLabel(node)}
            </button>
          </li>
        );
      })}
    </ul>
  );
}
