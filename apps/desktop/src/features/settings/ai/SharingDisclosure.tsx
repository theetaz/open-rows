// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button } from '@openrows/ui';
import styles from './Ai.module.css';
import { t } from '../messages/lazyAi';
const SharingPane = lazy(() => import('./SharingPane'));
const McpPane = lazy(() => import('./McpPane'));
/**
 * Load MCP credential/client controls and connection sharing with its audit
 * only after an explicit settings action; both toggles share one track.
 */
export function SharingDisclosure(): React.JSX.Element {
  const [mcp, setMcp] = useState(false);
  const [sharing, setSharing] = useState(false);
  return (
    <>
      <div className={styles.switcher}>
        <Button
          size="sm"
          aria-expanded={mcp}
          onClick={() => {
            setMcp(!mcp);
          }}
        >
          {t('mcpServer')}
        </Button>
        <Button
          size="sm"
          aria-expanded={sharing}
          onClick={() => {
            setSharing(!sharing);
          }}
        >
          {t('dataSharing')}
        </Button>
      </div>
      {mcp && (
        <Suspense fallback={<p role="status">{t('loadingMcpSettings')}</p>}>
          <McpPane />
        </Suspense>
      )}
      {sharing && (
        <Suspense fallback={<p role="status">{t('loadingDataSharing')}</p>}>
          <SharingPane />
        </Suspense>
      )}
    </>
  );
}
