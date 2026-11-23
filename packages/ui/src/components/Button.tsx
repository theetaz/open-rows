// SPDX-License-Identifier: Apache-2.0
import type { ButtonHTMLAttributes, HTMLAttributes, Ref } from 'react';
import { Icon } from '../icons/Icon';
import styles from './Controls.module.css';

/** Compact 22 px, regular 28 px, or the 32 px toolbar capsule. */
type Size = 'sm' | 'md' | 'toolbar';
type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  /** Anchor for a menu or popover opened by this button. */
  ref?: Ref<HTMLButtonElement>;
  variant?: 'primary' | 'secondary' | 'ghost' | 'destructive';
  size?: Size;
};
/** Capsule action with native keyboard activation and disabled semantics. */
export function Button({
  variant = 'secondary',
  size = 'md',
  className = '',
  ...props
}: ButtonProps): React.JSX.Element {
  return (
    <button
      type="button"
      {...props}
      className={`${styles.button ?? ''} ${className}`}
      data-variant={variant}
      data-size={size}
    />
  );
}
/** Icon-only circle; its accessible name is required at compile time. */
export function IconButton(
  props: Omit<ButtonProps, 'aria-label'> & { 'aria-label': string },
): React.JSX.Element {
  return (
    <Button
      {...props}
      className={`${styles.iconButton ?? ''} ${props.className ?? ''}`}
    />
  );
}
/**
 * A pop-up button: the current choice and an up-down chevron, opening a
 * `Menu` or `Picker` anchored to it (`<PopUpButton ref={anchor} …>Public</PopUpButton>`).
 */
export function PopUpButton({
  children,
  ...props
}: ButtonProps): React.JSX.Element {
  return (
    <Button aria-haspopup="menu" {...props}>
      {children}
      <Icon name="chevronUpDown" className={styles.popUpChevron} />
    </Button>
  );
}
/**
 * A capsule of related buttons at one control height, e.g.
 * `<ButtonGroup aria-label="View"><IconButton …/><IconButton …/></ButtonGroup>`.
 * Pass the same `size` to the group as to the bar's other controls.
 */
export function ButtonGroup({
  size = 'md',
  className = '',
  ...props
}: HTMLAttributes<HTMLDivElement> & {
  'aria-label': string;
  size?: Size;
}): React.JSX.Element {
  return (
    <div
      role="group"
      {...props}
      data-size={size}
      className={`${styles.group ?? ''} ${className}`}
    />
  );
}
/**
 * A bar of controls (view bar, editor bar): 38 px, 8 px gaps, centres aligned.
 * Put a `<BarSpacer />` between the leading and trailing controls.
 */
export function Bar({
  size = 'bar',
  className = '',
  ...props
}: HTMLAttributes<HTMLDivElement> & {
  size?: 'bar' | 'toolbar';
}): React.JSX.Element {
  return (
    <div
      {...props}
      data-size={size}
      className={`${styles.bar ?? ''} ${className}`}
    />
  );
}
/** Pushes the controls after it to the bar's trailing edge. */
export function BarSpacer(): React.JSX.Element {
  return <span className={styles.spacer} aria-hidden="true" />;
}
