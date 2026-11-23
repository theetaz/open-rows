// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton } from '@openrows/ui';
import type { InsecureReasonDto } from '@openrows/ipc';
import type { SafeModeState } from '../security/policies';
import { InsecureBadge } from '../connections';
import { FavouriteButton } from '../schema/FavouriteButton';
import type { SidebarObject } from '../schema/sidebarIdentity';
import { ConnectionStatus } from './ConnectionStatus';
import styles from './PathBar.module.css';
import { t } from './messages';

/**
 * Reports the saved safety policy and opens the setting that changes it: a
 * quiet open lock when unrestricted, a green lock when on.
 */
function SafeModeButton({
  state,
  onClick,
}: {
  state: SafeModeState | null;
  onClick: () => void;
}): React.JSX.Element {
  return (
    <IconButton
      aria-label={
        state ? t('safeModeLabel', { label: state.label }) : t('safeMode')
      }
      aria-haspopup="dialog"
      title={
        state
          ? t('safeModeLabelDetail', {
              label: state.label,
              detail: state.detail,
            })
          : t('safeModeNot')
      }
      className={state?.on ? styles.locked : undefined}
      data-on={state?.on ?? false}
      disabled={!state}
      variant="ghost"
      size="sm"
      onClick={onClick}
    >
      <Icon name={state?.on ? 'lock' : 'unlock'} />
    </IconButton>
  );
}

/** The path bar's end: health, insecure warning, safety, star and refresh. */
export function PathTools(props: {
  health: string;
  route: string;
  insecure: InsecureReasonDto[];
  safeMode: SafeModeState | null;
  onSafeMode: () => void;
  favourite: SidebarObject | undefined;
  onRefresh: (() => void) | undefined;
}): React.JSX.Element {
  return (
    <>
      <ConnectionStatus health={props.health} route={props.route} />
      <InsecureBadge reasons={props.insecure} />
      <SafeModeButton state={props.safeMode} onClick={props.onSafeMode} />
      {props.favourite && <FavouriteButton object={props.favourite} />}
      <IconButton
        aria-label={t('refresh')}
        title={t('refreshKey')}
        disabled={!props.onRefresh}
        variant="ghost"
        size="sm"
        onClick={props.onRefresh}
      >
        <Icon name="refresh" />
      </IconButton>
    </>
  );
}
