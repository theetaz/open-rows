// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import { Icon } from '../icons/Icon';
import type { IconName } from '../icons/Icon';
import styles from './Navigation.module.css';

/** Catalog glyphs drawn as line icons; other glyphs render as text. */
const glyphIcons: Record<string, IconName> = {
  '▦': 'table',
  '▤': 'eye',
  '▥': 'eye',
  ƒ: 'function',
  '≡': 'list',
};

/**
 * Object selection row with a name and optional estimated row count. With
 * `onOpen`, a click selects and double-click or Enter opens.
 */
export function TreeItem({
  label,
  count,
  selected,
  onSelect,
  onOpen,
  glyph = '▦',
  title,
  children,
}: {
  label: string;
  count?: string;
  selected: boolean;
  onSelect: () => void;
  onOpen?: () => void;
  glyph?: string;
  title?: string;
  children?: ReactNode;
}): React.JSX.Element {
  return (
    <button
      type="button"
      aria-label={count ? `${label} ${count}` : label}
      title={title}
      className={styles.treeItem}
      aria-pressed={selected}
      onClick={onSelect}
      onDoubleClick={onOpen}
      onKeyDown={(event) => {
        if (!onOpen || event.key !== 'Enter' || event.nativeEvent.isComposing)
          return;
        event.preventDefault();
        onSelect();
        onOpen();
      }}
    >
      <span aria-hidden="true" data-glyph={glyph}>
        {glyphIcons[glyph] ? <Icon name={glyphIcons[glyph]} /> : glyph}
      </span>
      <span>{children ?? label}</span>
      <small>{count}</small>
    </button>
  );
}
