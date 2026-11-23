// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import styles from './Apps.module.css';
import { t } from './messages';

/** Exact, selectable commands with a Copy button; they hold no secret. */
export function CommandBlock(props: {
  commands: readonly string[];
}): React.JSX.Element {
  const [copied, setCopied] = useState(false);
  return (
    <div className={styles.command}>
      <pre>
        {props.commands.map((command) => (
          <code key={command}>{command}</code>
        ))}
      </pre>
      <Button
        size="sm"
        variant="ghost"
        onClick={() => {
          navigator.clipboard.writeText(props.commands.join('\n')).then(
            () => {
              setCopied(true);
            },
            () => {
              setCopied(false);
            },
          );
        }}
      >
        {copied ? t('copied') : t('copyCommands')}
      </Button>
    </div>
  );
}
