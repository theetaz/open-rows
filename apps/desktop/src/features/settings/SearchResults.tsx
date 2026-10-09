// SPDX-License-Identifier: Apache-2.0
import { useLayoutEffect, useRef, useState } from 'react';
import styles from './Settings.module.css';

const ROW = '[data-setting-row]';

/**
 * Hide every pane and section under `root` without a matching row; returns
 * whether any row matched. Rows filter themselves; this tidies around them.
 */
function hideEmpty(root: HTMLElement): boolean {
  for (const element of root.querySelectorAll<HTMLElement>(
    '[data-search-pane], [data-search-pane] section',
  ))
    element.hidden = element.querySelector(ROW) === null;
  return root.querySelector(ROW) !== null;
}

/** The matching settings of every pane, each under its pane's name. */
export function SearchResults(props: {
  query: string;
  panes: readonly { value: string; label: string; pane: React.ReactNode }[];
}): React.JSX.Element {
  const root = useRef<HTMLDivElement>(null);
  const [found, setFound] = useState(true);
  const { query } = props;
  // Rows filter by the query alone, so the tidy-up follows the query.
  useLayoutEffect(() => {
    if (root.current) setFound(hideEmpty(root.current));
  }, [query]);
  return (
    <div ref={root} className={styles.results}>
      {props.panes.map(({ value, label, pane }) => (
        <section key={value} data-search-pane aria-label={`${label} settings`}>
          <h2 className={styles.paneTitle}>{label}</h2>
          {pane}
        </section>
      ))}
      {!found && (
        <p role="status" className={styles.noResults}>
          No settings match “{query.trim()}”.
        </p>
      )}
    </div>
  );
}
