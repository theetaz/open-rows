// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import { TextField } from '@openrows/ui';
import { fieldError } from './form';
import type { UrlField as UrlState } from './useUrlField';
import styles from './ConnectionSheet.module.css';

/**
 * Connection URL in sync with the fields: typing fills them after a short
 * pause, a paste fills them at once, and any password shows only as dots.
 */
export function UrlField({ url }: { url: UrlState }): React.JSX.Element {
  const pasted = useRef(false);
  return (
    <TextField
      className={styles.row}
      label="URL"
      placeholder="postgres://user:password@host:5432/database"
      autoComplete="off"
      spellCheck={false}
      value={url.value}
      {...fieldError(url.error)}
      onPaste={() => {
        pasted.current = true;
      }}
      onChange={(event) => {
        url.edit(event.target.value, pasted.current);
        pasted.current = false;
      }}
      onKeyDown={(event) => {
        if (event.key !== 'Enter' || !url.pending) return;
        event.preventDefault();
        url.flush();
      }}
    />
  );
}
