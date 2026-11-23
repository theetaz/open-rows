// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useEffect, useState } from 'react';
import { Icon, IconButton, ToolbarGroup } from '@openrows/ui';
import { useAgentActivity } from './activityStore';
import styles from './AgentActivity.module.css';
import { t } from './toolbarMessages';

const ActivityPopover = lazy(() => import('./ActivityPopover'));

/** How long after its last tool call an agent still counts as active. */
const ACTIVE_SECONDS = 120;

/** True while the newest event is recent; flips off on its own when it ages. */
function useActiveSince(at: number | undefined): boolean {
  const [expired, setExpired] = useState<number | undefined>(undefined);
  useEffect(() => {
    if (at === undefined) return;
    const left = at + ACTIVE_SECONDS - Date.now() / 1000;
    const timer = window.setTimeout(
      () => {
        setExpired(at);
      },
      Math.max(0, left * 1000),
    );
    return () => {
      window.clearTimeout(timer);
    };
  }, [at]);
  return at !== undefined && expired !== at;
}

function agentName(app: string | null): string {
  if (app === 'claude') return t('claude');
  if (app === 'codex') return t('codex');
  if (app === 'gemini') return t('gemini');
  return t('externalAgent');
}

/**
 * A quiet toolbar indicator while an external agent is calling OpenRows'
 * MCP tools; it opens the recent activity list. Hidden otherwise.
 */
export function AgentActivityButton(): React.JSX.Element | null {
  const events = useAgentActivity();
  const latest = events[0];
  const active = useActiveSince(latest?.at);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const [open, setOpen] = useState(false);
  if (!latest || (!active && !open)) return null;
  const label = t('agentUsing', { name: agentName(latest.app) });
  return (
    <ToolbarGroup aria-label={t('agentActivity')}>
      <IconButton
        ref={setAnchor}
        aria-label={label}
        title={label}
        aria-haspopup="dialog"
        aria-expanded={open}
        variant="ghost"
        size="sm"
        className={styles.button}
        onClick={() => {
          setOpen(!open);
        }}
      >
        <Icon name="sparkles" />
        <span className={styles.pulse} aria-hidden="true" />
      </IconButton>
      {open && anchor && (
        <Suspense fallback={null}>
          <ActivityPopover
            anchor={anchor}
            onClose={() => {
              setOpen(false);
            }}
          />
        </Suspense>
      )}
    </ToolbarGroup>
  );
}
