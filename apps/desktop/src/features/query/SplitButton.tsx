// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import { Button, Icon, Menu } from '@openrows/ui';
import type { MenuEntry } from '@openrows/ui';
import { usePullDown } from '../table/usePullDown';
import styles from './EditorBar.module.css';

type Variant = 'primary' | 'secondary' | 'ghost' | 'destructive';

/** A bar button whose pull-down menu opens under it. */
export function MenuButton(props: {
  label: string;
  title?: string | undefined;
  entries: readonly MenuEntry[];
  variant?: Variant | undefined;
  disabled?: boolean | undefined;
  className?: string | undefined;
  /** Align the menu to the button's leading (default) or trailing edge. */
  align?: 'start' | 'end' | undefined;
  children: ReactNode;
}): React.JSX.Element {
  const { anchor, button, close } = usePullDown();
  return (
    <>
      <Button
        variant={props.variant ?? 'secondary'}
        className={props.className ?? ''}
        aria-label={props.label}
        title={props.title ?? props.label}
        aria-haspopup="menu"
        disabled={props.disabled}
        {...button}
      >
        {props.children}
      </Button>
      {anchor && (
        <Menu
          anchor={anchor}
          align={props.align ?? 'start'}
          label={props.label}
          entries={props.entries}
          onClose={close}
        />
      )}
    </>
  );
}

/**
 * Main action plus a chevron opening related commands, joined as one control.
 * While the action is disabled the chevron turns neutral; its menu stays usable.
 */
export function SplitButton(props: {
  label: string;
  title: string;
  onClick: () => void;
  menuLabel: string;
  entries: readonly MenuEntry[];
  variant?: Variant;
  disabled?: boolean;
  className?: string | undefined;
  children: ReactNode;
}): React.JSX.Element {
  const variant = props.variant ?? 'secondary';
  return (
    <span className={styles.split}>
      <Button
        variant={variant}
        className={props.className ?? ''}
        aria-label={props.label}
        title={props.title}
        disabled={props.disabled}
        onClick={props.onClick}
      >
        {props.children}
      </Button>
      <MenuButton
        label={props.menuLabel}
        entries={props.entries}
        variant={props.disabled ? 'secondary' : variant}
        className={styles.chevron}
      >
        <Icon name="chevronDown" />
      </MenuButton>
    </span>
  );
}
