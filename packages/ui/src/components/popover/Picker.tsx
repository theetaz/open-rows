// SPDX-License-Identifier: Apache-2.0
import { memo, useEffect, useId, useRef, useState } from 'react';
import type { KeyboardEvent, ReactNode } from 'react';
import { Icon } from '../../icons/Icon';
import { t } from '../../messages';
import { Popover } from './Popover';
import { initialIndex, pickerRows, splitMatch } from './pickerModel';
import type { PickerItem, PickerRow, PickerSection } from './pickerModel';
import styles from './Picker.module.css';

export type PickerProps = {
  anchor: Element;
  /** Search placeholder and accessible name, e.g. "Find a table". */
  placeholder: string;
  sections: readonly PickerSection[];
  /** ↩ or a click; `newTab` is ⌘↩ or ⌘-click. */
  onPick: (item: PickerItem, options: { newTab: boolean }) => void;
  onClose: () => void;
  /** ← → with an empty search: jump to the neighbouring path segment. */
  onStep?: (dir: -1 | 1) => void;
  /** Key hints in the footer; defaults to navigate, open and new tab. */
  foot?: ReactNode;
  /** 372 px instead of 320 px (two-line rows). */
  wide?: boolean;
  /** Shift left of the anchor (lines the search icon up with the segment). */
  dx?: number;
};
type List = {
  rows: PickerRow[];
  active: number;
  setActive: (index: number) => void;
};

const Row = memo(function Row(props: {
  row: PickerRow;
  index: number;
  active: boolean;
  query: string;
  id: string;
}): React.JSX.Element {
  const { item, head } = props.row;
  const [before, match, after] = splitMatch(item.name, props.query);
  const name = (
    <span className={styles.name}>
      {before}
      {match && <b>{match}</b>}
      {after}
    </span>
  );
  return (
    <>
      {head && (
        <div className={styles.head} role="presentation">
          {head}
        </div>
      )}
      <div
        id={`${props.id}-${String(props.index)}`}
        role="option"
        aria-selected={props.active}
        data-index={props.index}
        data-two={Boolean(item.sub)}
        className={styles.row}
      >
        {item.lead && <span className={styles.lead}>{item.lead}</span>}
        {item.sub ? (
          <span className={styles.lines}>
            {name}
            <span className={styles.sub}>{item.sub}</span>
          </span>
        ) : (
          name
        )}
        <span className={styles.meta}>
          {item.meta}
          {item.status && (
            <span className={styles.dot} data-status={item.status} />
          )}
          {item.current && <Icon name="check" className={styles.check} />}
        </span>
      </div>
    </>
  );
});

function DefaultFoot(): React.JSX.Element {
  return (
    <>
      <span>
        <kbd>↑↓</kbd>
        {t('pickerNavigate')}
      </span>
      <span>
        <kbd>↩</kbd>
        {t('pickerOpen')}
      </span>
      <span>
        <kbd>⌘↩</kbd>
        {t('newTab')}
      </span>
    </>
  );
}

/** Arrow, Enter and segment keys inside the search field. */
function pickerKey(
  event: KeyboardEvent<HTMLInputElement>,
  props: PickerProps,
  { rows, active, setActive }: List,
): void {
  const { key } = event;
  const empty = !event.currentTarget.value;
  if (key === 'ArrowDown') setActive(Math.min(rows.length - 1, active + 1));
  else if (key === 'ArrowUp') setActive(Math.max(0, active - 1));
  else if (key === 'Enter') {
    const row = rows[active];
    if (row) props.onPick(row.item, { newTab: event.metaKey });
  } else if (
    (key === 'ArrowLeft' || key === 'ArrowRight') &&
    empty &&
    props.onStep
  )
    props.onStep(key === 'ArrowLeft' ? -1 : 1);
  else return;
  event.preventDefault();
}

function rowAt(target: EventTarget): number | null {
  const row = target instanceof Element ? target.closest('[data-index]') : null;
  return row ? Number(row.getAttribute('data-index')) : null;
}

/** The result list: hover highlights, a click picks. */
function Results(props: {
  id: string;
  label: string;
  query: string;
  total: number;
  list: List;
  onPick: PickerProps['onPick'];
}): React.JSX.Element {
  const { rows, active, setActive } = props.list;
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const row = ref.current?.querySelector<HTMLElement>(
      `[data-index="${String(active)}"]`,
    );
    if (typeof row?.scrollIntoView === 'function')
      row.scrollIntoView({ block: 'nearest' });
  }, [active]);
  return (
    <div
      ref={ref}
      id={props.id}
      role="listbox"
      aria-label={props.label}
      className={styles.list}
      onPointerMove={(event) => {
        const index = rowAt(event.target);
        if (index !== null && index !== active) setActive(index);
      }}
      onClick={(event) => {
        const row = rows[rowAt(event.target) ?? -1];
        if (row) props.onPick(row.item, { newTab: event.metaKey });
      }}
    >
      {rows.map((row, index) => (
        <Row
          key={row.item.id}
          row={row}
          index={index}
          active={index === active}
          query={props.query}
          id={props.id}
        />
      ))}
      {!rows.length && (
        <p className={styles.empty}>
          {t('pickerNoMatches', { query: props.query })}
        </p>
      )}
      {props.total > rows.length && (
        <p className={styles.more}>
          {t('pickerMore', {
            shown: String(rows.length),
            total: String(props.total),
          })}
        </p>
      )}
    </div>
  );
}

/**
 * A searchable popover list (path bar segments, tab list, connection
 * switcher): a focused search, sections, 28 px rows (40 px with `sub`),
 * right-aligned meta with status dots, a check on the current item, and
 * ↑↓ ↩ ⌘↩ ←→ Esc.
 *
 * ```tsx
 * <Picker anchor={segment} placeholder="Find a database" sections={[
 *   { head: 'Recent', items: recent }, { head: 'All databases', items: all },
 * ]} onPick={(item, { newTab }) => go(item.id, newTab)} onClose={close} />
 * ```
 */
export function Picker(props: PickerProps): React.JSX.Element {
  const [query, setQuery] = useState('');
  const { rows, total } = pickerRows(props.sections, query);
  const [active, setActive] = useState(() => initialIndex(rows, ''));
  const id = useId();
  const list = { rows, active, setActive };
  return (
    <Popover
      anchor={props.anchor}
      label={props.placeholder}
      onClose={props.onClose}
      dx={props.dx ?? 8}
    >
      <div className={styles.picker} data-wide={props.wide}>
        <label className={styles.search}>
          <Icon name="search" />
          <input
            type="text"
            role="combobox"
            aria-expanded="true"
            aria-controls={id}
            aria-activedescendant={
              rows.length ? `${id}-${String(active)}` : undefined
            }
            aria-label={props.placeholder}
            placeholder={props.placeholder}
            spellCheck={false}
            autoComplete="off"
            autoFocus
            value={query}
            onChange={(event) => {
              const next = event.target.value;
              setQuery(next);
              setActive(
                initialIndex(pickerRows(props.sections, next).rows, next),
              );
            }}
            onKeyDown={(event) => {
              pickerKey(event, props, list);
            }}
          />
        </label>
        <Results
          id={id}
          label={props.placeholder}
          query={query}
          total={total}
          list={list}
          onPick={props.onPick}
        />
        <footer className={styles.foot}>{props.foot ?? <DefaultFoot />}</footer>
      </div>
    </Popover>
  );
}
