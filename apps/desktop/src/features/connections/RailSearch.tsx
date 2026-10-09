// SPDX-License-Identifier: Apache-2.0
import { Icon, TextField } from '@openrows/ui';
import styles from './Rail.module.css';

/**
 * Filters the rail's connections as the user types. Down moves to the first
 * match, Return connects it and Escape clears the search.
 */
export function RailSearch(props: {
  query: string;
  onQuery: (query: string) => void;
  /** Select the first matching connection. */
  onSubmit: () => void;
}): React.JSX.Element {
  return (
    <TextField
      className={styles.search}
      controlSize="compact"
      type="search"
      aria-label="Search connections"
      placeholder="Search connections"
      autoComplete="off"
      spellCheck={false}
      leadingIcon={<Icon name="search" />}
      value={props.query}
      onChange={(event) => {
        props.onQuery(event.target.value);
      }}
      onKeyDown={(event) => {
        if (event.nativeEvent.isComposing) return;
        searchKey(event, props);
      }}
    />
  );
}

function searchKey(
  event: React.KeyboardEvent<HTMLInputElement>,
  props: Parameters<typeof RailSearch>[0],
): void {
  if (event.key === 'ArrowDown') {
    event.preventDefault();
    event.currentTarget
      .closest('nav')
      ?.querySelector<HTMLButtonElement>('[aria-pressed]')
      ?.focus();
  } else if (event.key === 'Enter') {
    event.preventDefault();
    props.onSubmit();
  } else if (event.key === 'Escape' && props.query) {
    event.preventDefault();
    event.stopPropagation();
    props.onQuery('');
  }
}
