// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import { fieldError } from './form';
import styles from './ConnectionSheet.module.css';

/** Paste a postgres:// URL to fill the form; the core parses it. */
export function UrlField(props: {
  error: string | null;
  onFill: (url: string) => Promise<boolean>;
}): React.JSX.Element {
  const [url, setUrl] = useState('');
  const fill = (): void => {
    if (!url.trim()) return;
    void props.onFill(url.trim()).then((filled) => {
      if (filled) setUrl('');
    });
  };
  return (
    <div className={styles.url}>
      <TextField
        label="Paste connection URL"
        placeholder="postgres://user:password@host:5432/database"
        autoComplete="off"
        spellCheck={false}
        value={url}
        {...fieldError(props.error)}
        onChange={(event) => {
          setUrl(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key !== 'Enter' || event.nativeEvent.isComposing) return;
          event.preventDefault();
          fill();
        }}
      />
      <Button size="sm" onClick={fill} disabled={!url.trim()}>
        Fill from URL
      </Button>
    </div>
  );
}
