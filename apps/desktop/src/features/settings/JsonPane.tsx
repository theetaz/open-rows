// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { TextArea } from '@openrows/ui';
import { settingsText } from './settingsFile';
import styles from './Settings.module.css';

/** Settings ▸ JSON: the settings.json the app writes, read-only. */
export function JsonPane(): React.JSX.Element {
  const [text] = useState(settingsText);
  return (
    <section aria-label="settings.json">
      <h3>settings.json</h3>
      <p className={styles.hint}>
        Stored in the app data folder (com.prabhavalabs.openrows). Edits made
        outside the app apply on the next launch.
      </p>
      <TextArea
        aria-label="settings.json"
        className={styles.json}
        value={text}
        readOnly
        rows={18}
      />
    </section>
  );
}
