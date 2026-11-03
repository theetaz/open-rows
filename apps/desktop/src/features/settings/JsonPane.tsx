// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { TextArea } from '@openrows/ui';
import { settingsText } from './settingsFile';
import styles from './Settings.module.css';
import { t } from './messages/lazyGeneral';

/** Settings ▸ JSON: the settings.json the app writes, read-only. */
export function JsonPane(): React.JSX.Element {
  const [text] = useState(settingsText);
  return (
    <section aria-label={t('settingsJson')}>
      <h3>{t('settingsJson')}</h3>
      <p className={styles.hint}>{t('storedAppData')}</p>
      <TextArea
        aria-label={t('settingsJson')}
        className={styles.json}
        value={text}
        readOnly
        rows={18}
      />
    </section>
  );
}
