// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { Icon, IconButton } from '@openrows/ui';
import { menuKey } from '../../shell/SidebarMenu';
import menuStyles from '../../shell/SidebarMenu.module.css';
import styles from './Settings.module.css';
import { t } from './messages/lazyGeneral';

export type SettingsAction = 'group' | 'all' | 'import' | 'export';

/** Close the open menu on a press or focus outside it and its opener. */
function useDismiss(
  root: React.RefObject<HTMLElement | null>,
  open: boolean,
  close: () => void,
): void {
  useEffect(() => {
    if (!open) return;
    const outside = (event: Event): void => {
      if (event.target instanceof Node && !root.current?.contains(event.target))
        close();
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('focusin', outside);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('focusin', outside);
    };
  }, [root, open, close]);
}

/** The open menu: resets first, then the portable JSON exchange. */
function ActionsPanel(props: {
  groupDisabled: boolean;
  choose: (action: SettingsAction) => void;
  close: () => void;
}): React.JSX.Element {
  const item = (action: SettingsAction, label: string, disabled = false) => (
    <button
      type="button"
      role="menuitem"
      disabled={disabled}
      onClick={() => {
        props.choose(action);
      }}
    >
      {label}
    </button>
  );
  return (
    <div
      role="menu"
      aria-label={t('settingsActions')}
      className={`${menuStyles.menu ?? ''} ${styles.actionsPanel ?? ''}`}
      onKeyDown={(event) => {
        menuKey(event, props.close);
      }}
    >
      {item('group', t('resetGroup'), props.groupDisabled)}
      {item('all', t('resetAllSettings'))}
      <hr />
      {item('export', t('exportSettings'))}
      {item('import', t('importSettings'))}
    </div>
  );
}

/**
 * The settings actions behind one `…` button beside Done. The menu renders
 * inside the sheet so it stays in the modal's top layer.
 */
export function SettingsMenu(props: {
  groupDisabled: boolean;
  onChoose: (action: SettingsAction) => void;
}): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const opener = useRef<HTMLButtonElement>(null);
  const close = (): void => {
    setOpen(false);
    opener.current?.focus();
  };
  useDismiss(root, open, () => {
    setOpen(false);
  });
  useEffect(() => {
    if (open)
      root.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
  }, [open]);
  return (
    <div ref={root} className={styles.actionsMenu}>
      <IconButton
        ref={opener}
        variant="ghost"
        aria-label={t('settingsActions')}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => {
          setOpen((value) => !value);
        }}
      >
        <Icon name="more" />
      </IconButton>
      {open && (
        <ActionsPanel
          groupDisabled={props.groupDisabled}
          close={close}
          choose={(action) => {
            close();
            props.onChoose(action);
          }}
        />
      )}
    </div>
  );
}
