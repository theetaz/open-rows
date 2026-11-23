// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import { Button, Icon } from '@openrows/ui';
import { CascadeMenu, useMenuAnchor } from '../../shell/CascadeMenu';
import type { MenuEntry } from '../../shell/CascadeMenu';
import styles from './EditorBar.module.css';

type Variant = 'primary' | 'secondary' | 'ghost' | 'destructive';

/** A small button whose menu opens below it (the bars sit at the top of a pane). */
export function MenuButton(props: {
  label: string;
  title?: string | undefined;
  entries: readonly MenuEntry[];
  variant?: Variant | undefined;
  disabled?: boolean | undefined;
  className?: string | undefined;
  children: ReactNode;
}): React.JSX.Element {
  const { at, openAt, close } = useMenuAnchor(true);
  return (
    <>
      <Button
        size="sm"
        variant={props.variant ?? 'secondary'}
        className={props.className ?? ''}
        aria-label={props.label}
        title={props.title ?? props.label}
        aria-haspopup="menu"
        aria-expanded={at !== null}
        disabled={props.disabled}
        onClick={(event) => {
          openAt(event.currentTarget);
        }}
      >
        {props.children}
      </Button>
      {at && (
        <CascadeMenu
          x={at.x}
          y={at.y}
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
        size="sm"
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
