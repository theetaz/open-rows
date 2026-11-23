// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import { Button, Popover } from '@openrows/ui';
import { showAgentApps } from '../settings/ai/aiPage';
import { ActivityList } from './ActivityList';
import { seedAgentActivity, useAgentActivity } from './activityStore';
import { nowSeconds } from './appNames';
import styles from './AgentActivity.module.css';
import { t } from './messages';

/** Recent external tool calls under the toolbar indicator, with a way to Apps. */
export default function ActivityPopover(props: {
  anchor: Element;
  onClose: () => void;
}): React.JSX.Element {
  const events = useAgentActivity();
  useEffect(() => {
    void seedAgentActivity();
  }, []);
  return (
    <Popover
      anchor={props.anchor}
      label={t('activityList')}
      onClose={props.onClose}
    >
      <div className={styles.panel}>
        <header className={styles.header}>
          <span>{t('recentActivity')}</span>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              props.onClose();
              showAgentApps();
            }}
          >
            {t('apps')}
          </Button>
        </header>
        <ActivityList events={events} now={nowSeconds()} limit={8} />
      </div>
    </Popover>
  );
}
