// SPDX-License-Identifier: Apache-2.0
import { useLayoutEffect, useRef } from 'react';
import type { PointerEvent, ReactNode } from 'react';
import { Icon } from '../../icons/Icon';
import type { MenuEntry } from './menuModel';
import styles from './Menu.module.css';

type Choice = Extract<MenuEntry, { kind: 'item' | 'submenu' }>;
/** What a row reports to the menu that owns it. */
type RowEvents = {
  onMove: (level: number, index: number | null, event: PointerEvent) => void;
  onLeave: (level: number) => void;
  onChoose: (level: number, index: number) => void;
};
type PanelProps = RowEvents & {
  level: number;
  label: string;
  entries: readonly MenuEntry[];
  active: number | null;
  open: number | null;
  chosen: number | null;
  closing: boolean;
  place: (panel: HTMLElement, level: number) => void;
  register: (level: number, panel: HTMLElement | null) => void;
  footer?: ReactNode;
};

function role(entry: Choice): string {
  if (entry.kind === 'submenu' || entry.checked === undefined)
    return 'menuitem';
  return entry.radio ? 'menuitemradio' : 'menuitemcheckbox';
}

function Lead({ entry }: { entry: Choice }): React.JSX.Element {
  if (entry.kind === 'item' && entry.checked) return <Icon name="check" />;
  if (entry.kind === 'item' && entry.swatch)
    return (
      <span className={styles.swatch} style={{ background: entry.swatch }} />
    );
  return <>{entry.icon}</>;
}

function Row(props: {
  entry: Choice;
  index: number;
  panel: PanelProps;
}): React.JSX.Element {
  const { entry, index, panel } = props;
  const item = entry.kind === 'item' ? entry : undefined;
  return (
    <div
      role={role(entry)}
      tabIndex={-1}
      data-index={index}
      data-active={panel.active === index}
      data-open={panel.open === index}
      data-chosen={panel.chosen === index}
      data-danger={item?.danger}
      aria-checked={item?.checked}
      aria-disabled={entry.disabled}
      aria-haspopup={entry.kind === 'submenu' ? 'menu' : undefined}
      aria-expanded={
        entry.kind === 'submenu' ? panel.open === index : undefined
      }
      className={styles.row}
      onPointerMove={(event) => {
        panel.onMove(panel.level, entry.disabled ? null : index, event);
      }}
      onClick={() => {
        panel.onChoose(panel.level, index);
      }}
    >
      <span className={styles.lead} aria-hidden="true">
        <Lead entry={entry} />
      </span>
      <span className={styles.label}>{entry.label}</span>
      <span className={styles.shortcut} aria-hidden="true">
        {item?.shortcut}
      </span>
      <span className={styles.chevron} aria-hidden="true">
        {entry.kind === 'submenu' && <Icon name="chevronRight" />}
      </span>
    </div>
  );
}

function rows(panel: PanelProps): React.JSX.Element[] {
  return panel.entries.map((entry, index) => {
    const key = `${String(index)}:${'label' in entry ? entry.label : ''}`;
    if (entry.kind === 'separator')
      return <div key={key} role="separator" className={styles.separator} />;
    if (entry.kind === 'header')
      return (
        <div key={key} role="presentation" className={styles.header}>
          {entry.label}
        </div>
      );
    return <Row key={key} entry={entry} index={index} panel={panel} />;
  });
}

/** One level of a menu: the root or an open submenu. */
export function MenuPanel(props: PanelProps): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null);
  const { place, register, level } = props;
  useLayoutEffect(() => {
    const panel = ref.current;
    register(level, panel);
    if (panel) place(panel, level);
    return () => {
      register(level, null);
    };
  }, [place, register, level]);
  return (
    <div
      ref={ref}
      role="menu"
      tabIndex={-1}
      aria-label={props.label}
      className={styles.panel}
      data-state={props.closing ? 'closing' : 'open'}
      onPointerLeave={() => {
        props.onLeave(level);
      }}
      onContextMenu={(event) => {
        event.preventDefault();
      }}
    >
      {rows(props)}
      {props.footer && <div className={styles.footer}>{props.footer}</div>}
    </div>
  );
}
