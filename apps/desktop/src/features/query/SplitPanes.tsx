// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import { Icon, IconButton } from '@openrows/ui';
import type { QueryTab } from './tabs';
import type { Split } from './useSplit';
import styles from './SplitPanes.module.css';

/** Keyboard resize step of the divider. */
const STEP = 0.05;

type Props = {
  split: Split;
  /** The scope's active tab, shown in the second (or only) pane. */
  active: QueryTab;
  render: (tab: QueryTab) => ReactNode;
  connection?: string;
};

/** Arrow keys move the divider; Home and End jump to the limits. */
function dividerKey(event: React.KeyboardEvent, split: Split): void {
  const { layout } = split;
  if (!layout) return;
  const back = layout.orientation === 'right' ? 'ArrowLeft' : 'ArrowUp';
  const forward = layout.orientation === 'right' ? 'ArrowRight' : 'ArrowDown';
  const moves: Record<string, number> = {
    [back]: layout.ratio - STEP,
    [forward]: layout.ratio + STEP,
    Home: 0,
    End: 1,
  };
  const next = moves[event.key];
  if (next === undefined) return;
  event.preventDefault();
  split.setRatio(next);
}

/** Drag the divider: the pointer position sets the first pane's share. */
function dragFrom(
  event: React.PointerEvent<HTMLDivElement>,
  split: Split,
  container: HTMLElement | null,
): void {
  const { layout } = split;
  if (!layout || !container) return;
  const target = event.currentTarget;
  target.setPointerCapture(event.pointerId);
  const move = (moved: PointerEvent): void => {
    const box = container.getBoundingClientRect();
    const ratio =
      layout.orientation === 'right'
        ? (moved.clientX - box.left) / box.width
        : (moved.clientY - box.top) / box.height;
    if (Number.isFinite(ratio)) split.setRatio(ratio);
  };
  const stop = (): void => {
    target.removeEventListener('pointermove', move);
    target.removeEventListener('pointerup', stop);
  };
  target.addEventListener('pointermove', move);
  target.addEventListener('pointerup', stop);
}

/**
 * One pane, or two panes split right or down with a resizable divider.
 * Panes are keyed by tab, so splitting keeps the active tab's editor.
 */
export function SplitPanes({
  split,
  active,
  render,
  connection,
}: Props): React.JSX.Element {
  const container = useRef<HTMLDivElement>(null);
  const { layout } = split;
  const style = layout
    ? ({ '--split-ratio': `${String(layout.ratio * 100)}%` } as CSSProperties)
    : undefined;
  return (
    <div
      ref={container}
      className={styles.panes}
      data-orientation={layout?.orientation}
      style={style}
    >
      {layout && (
        <section
          key={`pane-${layout.pinned.id}`}
          className={styles.pane}
          data-pane="pinned"
          aria-label={`Split pane: ${layout.pinned.title}`}
          tabIndex={-1}
          onFocus={() => {
            split.focused('pinned');
          }}
        >
          <PinnedBar
            tab={layout.pinned}
            connection={connection}
            onClose={split.close}
          />
          {render(layout.pinned)}
        </section>
      )}
      {layout && (
        <div
          role="separator"
          aria-label="Resize split panes"
          aria-orientation={
            layout.orientation === 'right' ? 'vertical' : 'horizontal'
          }
          aria-valuemin={20}
          aria-valuemax={80}
          aria-valuenow={Math.round(layout.ratio * 100)}
          tabIndex={0}
          className={styles.divider}
          onKeyDown={(event) => {
            dividerKey(event, split);
          }}
          onPointerDown={(event) => {
            dragFrom(event, split, container.current);
          }}
        />
      )}
      <section
        key={`pane-${active.id}`}
        className={styles.pane}
        data-pane="active"
        aria-label={layout ? `Split pane: ${active.title}` : undefined}
        tabIndex={-1}
        onFocus={() => {
          split.focused('active');
        }}
      >
        {layout && (
          <div className={styles.paneBar}>
            <span>
              {connection ?? active.connectionId} · {active.database} ·{' '}
              {active.title}
            </span>
          </div>
        )}
        {render(active)}
      </section>
    </div>
  );
}

/** Context and close action of the first pane. */
function PinnedBar(props: {
  tab: QueryTab;
  connection: string | undefined;
  onClose: () => void;
}): React.JSX.Element {
  const { tab } = props;
  return (
    <div className={styles.paneBar}>
      <span>
        {props.connection ?? tab.connectionId} · {tab.database} · {tab.title}
      </span>
      <IconButton
        aria-label="Close split pane"
        title="Close Split"
        size="sm"
        variant="ghost"
        onClick={props.onClose}
      >
        <Icon name="close" />
      </IconButton>
    </div>
  );
}
