// SPDX-License-Identifier: Apache-2.0
import { Icon } from '@openrows/ui';
import { useWorkspaceCollapse } from './useWorkspaceCollapse';
import styles from './Rail.module.css';
import { t } from './messages/profile';

/** Collapsing changes visibility only; no connection/session actions are fired. */
export function WorkspaceDisclosure(props: {
  id: string;
  label: string;
  environment?: string;
  count: number;
  /** A search shows every matching group open. */
  forceOpen?: boolean;
  children: React.ReactNode;
}): React.JSX.Element {
  const [collapsed, toggle] = useWorkspaceCollapse(props.id);
  return (
    <section className={styles.group} data-environment={props.environment}>
      <button
        type="button"
        className={styles.disclosure}
        aria-label={
          collapsed
            ? t('expandLabel', { label: props.label })
            : t('collapseLabel', { label: props.label })
        }
        aria-expanded={!collapsed}
        title={props.label}
        onClick={toggle}
      >
        <span aria-hidden="true">
          <Icon name={collapsed ? 'chevronRight' : 'chevronDown'} />
        </span>
        <span>{props.label}</span>
        <small className={styles.groupCount}>{props.count}</small>
      </button>
      {(!collapsed || props.forceOpen) && props.children}
    </section>
  );
}
