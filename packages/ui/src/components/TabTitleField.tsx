// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import styles from './Navigation.module.css';

/**
 * Inline editor of a tab's title: Enter or leaving the field renames,
 * Escape cancels (`null`). An empty title keeps the old one.
 */
export function TabTitleField(props: {
  title: string;
  onDone: (title: string | null) => void;
}): React.JSX.Element {
  const field = useRef<HTMLInputElement>(null);
  const done = useRef(false);
  useEffect(() => {
    field.current?.focus();
    field.current?.select();
  }, []);
  const finish = (title: string | null): void => {
    if (done.current) return;
    done.current = true;
    const trimmed = title?.trim();
    props.onDone(trimmed ? trimmed : null);
  };
  return (
    <input
      ref={field}
      className={styles.titleField}
      aria-label="Tab name"
      defaultValue={props.title}
      spellCheck={false}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          event.preventDefault();
          finish(event.currentTarget.value);
        } else if (event.key === 'Escape') {
          event.preventDefault();
          event.stopPropagation();
          finish(null);
        }
      }}
      onBlur={(event) => {
        finish(event.currentTarget.value);
      }}
    />
  );
}
