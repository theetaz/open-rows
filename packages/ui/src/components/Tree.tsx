// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import type {
  KeyboardEvent,
  MouseEvent,
  ReactNode,
  SyntheticEvent,
} from 'react';
import { moveTreeFocus } from './treeFocus';
import styles from './Tree.module.css';

/** ARIA tree container; rows are {@link TreeNode}s with one tab stop. */
export function Tree({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}): React.JSX.Element {
  return (
    <div
      role="tree"
      aria-label={label}
      className={styles.tree}
      onKeyDown={moveTreeFocus}
    >
      {children}
    </div>
  );
}

type TreeNodeProps = {
  /** Accessible name. */
  label: string;
  /** Nesting depth, 1 for top-level rows. */
  level: number;
  selected: boolean;
  /** The tree's single Tab stop (roving tabindex). */
  tabStop: boolean;
  /** Called when the row is clicked or receives focus. */
  onSelect: () => void;
  /** Double-click or Enter. */
  onOpen?: () => void;
  /** A single click, after selecting (preview tabs). */
  onPreview?: () => void;
  /** Disclosure state; omit for leaf rows. */
  expanded?: boolean;
  onExpandedChange?: (expanded: boolean) => void;
  /** Leading glyph or icon. */
  icon?: ReactNode;
  /** Icon colour: table blue or primary-key yellow. */
  iconTone?: 'info' | 'warning' | undefined;
  /** Right-aligned muted text, e.g. a row estimate or data type. */
  detail?: ReactNode;
  title?: string;
  /** Visible name (defaults to `label`). */
  name?: ReactNode;
  /** Child rows, rendered in a group while expanded. */
  children?: ReactNode;
  /** Secondary click on this row (not on a nested row), e.g. a context menu. */
  onContextMenu?: ((event: MouseEvent<HTMLDivElement>) => void) | undefined;
};

const stop = (event: MouseEvent): void => {
  event.stopPropagation();
};

/** The event happened on this row, not on a row of its nested group. */
const own = (event: SyntheticEvent<HTMLElement>): boolean =>
  event.target instanceof Element &&
  event.target.closest('[role="treeitem"]') === event.currentTarget;

/** Moves focus to `row` when focus sits in its group, which a collapse unmounts. */
function keepFocusOn(row: HTMLElement | null): void {
  if (
    row &&
    row.contains(document.activeElement) &&
    row !== document.activeElement
  )
    row.focus();
}

function rowKeys(event: KeyboardEvent, props: TreeNodeProps): void {
  const { expanded, onExpandedChange, onOpen } = props;
  if (event.key === 'Enter' && onOpen && !event.nativeEvent.isComposing) {
    event.preventDefault();
    props.onSelect();
    onOpen();
    return;
  }
  if (expanded === undefined || !onExpandedChange) return;
  const open = event.key === 'ArrowRight' && !expanded;
  if (!open && !(event.key === 'ArrowLeft' && expanded)) return;
  event.preventDefault();
  onExpandedChange(open);
}

/**
 * One tree row: optional disclosure chevron (click, → / ←), icon, name and
 * right-aligned detail. Clicking or focusing selects; the chevron toggles
 * without selecting or opening.
 */
export function TreeNode(props: TreeNodeProps): React.JSX.Element {
  const { expanded, onExpandedChange } = props;
  const self = useRef<HTMLDivElement>(null);
  return (
    <div
      ref={self}
      role="treeitem"
      aria-label={props.label}
      aria-level={props.level}
      aria-selected={props.selected}
      aria-expanded={expanded}
      tabIndex={props.tabStop ? 0 : -1}
      className={styles.node}
      onFocus={(event) => {
        if (event.target === event.currentTarget) props.onSelect();
      }}
      onKeyDown={(event) => {
        if (event.target === event.currentTarget) rowKeys(event, props);
      }}
      onClick={(event) => {
        if (!own(event)) return;
        props.onSelect();
        props.onPreview?.();
      }}
      onDoubleClick={(event) => {
        if (own(event)) props.onOpen?.();
      }}
      onContextMenu={(event) => {
        if (own(event)) props.onContextMenu?.(event);
      }}
    >
      <div className={styles.row} data-level={props.level} title={props.title}>
        <span
          className={styles.chevron}
          data-open={expanded}
          aria-hidden="true"
          onMouseDown={(event) => {
            if (expanded !== undefined) event.preventDefault();
          }}
          onClick={(event) => {
            if (expanded === undefined || !onExpandedChange) return;
            stop(event);
            if (expanded) keepFocusOn(self.current);
            onExpandedChange(!expanded);
          }}
          onDoubleClick={expanded === undefined ? undefined : stop}
        >
          {expanded !== undefined && '›'}
        </span>
        <span
          className={styles.icon}
          data-tone={props.iconTone}
          aria-hidden="true"
        >
          {props.icon}
        </span>
        <span className={styles.name}>{props.name ?? props.label}</span>
        {props.detail !== undefined && <small>{props.detail}</small>}
      </div>
      {expanded && <div role="group">{props.children}</div>}
    </div>
  );
}

/** A non-interactive status or error line inside a tree group. */
export function TreeNote({
  level,
  tone = 'quiet',
  children,
}: {
  level: number;
  tone?: 'quiet' | 'danger';
  children: ReactNode;
}): React.JSX.Element {
  return (
    <div
      role="treeitem"
      aria-level={level}
      aria-disabled="true"
      tabIndex={-1}
      className={styles.note}
      data-level={level}
      data-tone={tone}
    >
      {children}
    </div>
  );
}
