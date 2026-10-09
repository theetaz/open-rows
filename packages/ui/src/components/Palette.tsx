// SPDX-License-Identifier: Apache-2.0
import { useEffect, useId, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { fuzzyMatch } from '../search/fuzzy';
import { rankPaletteItems } from '../search/paletteResults';
import { HighlightText } from './HighlightText';
import { PalettePopover } from './PalettePopover';
import styles from './Palette.module.css';
/** Presentation descriptor for a searchable command or object. */
export type PaletteItem = {
  id: string;
  label: string;
  detail?: string;
  glyph?: string;
  shortcut?: string;
  searchText?: string;
};
type Props = {
  title: string;
  items: readonly PaletteItem[];
  footer?: ReactNode;
  onChoose: (id: string) => void;
  onClose: () => void;
  /**
   * Search elsewhere (e.g. a prebuilt index off the UI thread): `items` are
   * then the current query's matches, best first, and are not re-ranked.
   */
  onQueryChange?: (query: string) => void;
};
/** Matches as given, with label highlights for the query. */
function rankedItems(
  items: readonly PaletteItem[],
  query: string,
): ReturnType<typeof rankPaletteItems> {
  return items.map((item) => ({
    item,
    indices: fuzzyMatch(item.label, query)?.indices ?? [],
    score: 0,
  }));
}
/** Shared combobox/listbox palette with virtual focus and deterministic fuzzy ranking. */
export function Palette(props: Props): React.JSX.Element {
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState('');
  const id = useId();
  const { onQueryChange } = props;
  const results = onQueryChange
    ? rankedItems(props.items, query)
    : rankPaletteItems(props.items, query);
  const active = results.findIndex(({ item }) => item.id === selected);
  const index = Math.max(0, active);
  const option = results[index];
  return (
    <PalettePopover
      title={props.title}
      onClose={props.onClose}
      onChoose={props.onChoose}
    >
      {(dismiss) => (
        <>
          <input
            role="combobox"
            aria-label={props.title}
            aria-autocomplete="list"
            aria-expanded="true"
            aria-controls={id}
            aria-activedescendant={
              option ? `${id}-${option.item.id}` : undefined
            }
            value={query}
            placeholder="Type to search…"
            onChange={(event) => {
              setQuery(event.target.value);
              setSelected('');
              onQueryChange?.(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.nativeEvent.isComposing) return;
              if (event.key === 'Enter' && option) {
                event.preventDefault();
                dismiss(option.item.id);
              }
              const next = paletteIndex(event.key, index, results.length);
              if (next !== undefined) {
                event.preventDefault();
                setSelected(results[next]?.item.id ?? '');
              }
            }}
          />
          <PaletteOptions
            results={results}
            index={index}
            id={id}
            onSelect={setSelected}
            onChoose={dismiss}
          />
          {!results.length && <p role="status">No matches</p>}
          {props.footer}
        </>
      )}
    </PalettePopover>
  );
}
function paletteIndex(
  key: string,
  index: number,
  length: number,
): number | undefined {
  if (!length) return undefined;
  if (key === 'Home') return 0;
  if (key === 'End') return length - 1;
  if (key === 'ArrowDown') return (index + 1) % length;
  if (key === 'ArrowUp') return (index - 1 + length) % length;
  return undefined;
}

function PaletteOptions(props: {
  results: ReturnType<typeof rankPaletteItems>;
  index: number;
  id: string;
  onSelect: (id: string) => void;
  onChoose: (id: string) => void;
}): React.JSX.Element {
  const { results, index, id } = props;
  const list = useRef<HTMLUListElement>(null);
  const activeId = results[index]?.item.id;
  useEffect(() => {
    const active = list.current?.querySelector<HTMLElement>(
      '[aria-selected="true"]',
    );
    if (typeof active?.scrollIntoView === 'function')
      active.scrollIntoView({ block: 'nearest' });
  }, [activeId]);
  return (
    <ul
      id={id}
      ref={list}
      role="listbox"
      aria-label="Results"
      className={styles.results}
    >
      {results.map(({ item, indices }, position) => (
        <li
          key={item.id}
          id={`${id}-${item.id}`}
          role="option"
          aria-label={[item.label, item.detail, item.shortcut]
            .filter(Boolean)
            .join(' ')}
          aria-selected={position === index}
          className={styles.option}
          onMouseMove={() => {
            props.onSelect(item.id);
          }}
          onClick={() => {
            props.onChoose(item.id);
          }}
        >
          <span aria-hidden="true">{item.glyph}</span>
          <span>
            <HighlightText text={item.label} indices={indices} />
            <small>{item.detail}</small>
          </span>
          <kbd>{item.shortcut}</kbd>
        </li>
      ))}
    </ul>
  );
}
