// SPDX-License-Identifier: Apache-2.0
import { useLayoutEffect, useRef } from 'react';
import type { ReactNode, TextareaHTMLAttributes } from 'react';
import styles from './TextArea.module.css';

type Props = Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, 'aria-label'> & {
  'aria-label': string;
  /** Accessory pinned to the frame's top trailing corner, such as a menu chevron. */
  trailing?: ReactNode;
};

/**
 * Multi-line field in the text-field frame. It grows with its content up to a
 * maximum height (`--or-text-area-max`, default ten lines), then scrolls.
 */
export function TextArea({
  trailing,
  className = '',
  ...props
}: Props): React.JSX.Element {
  const ref = useRef<HTMLTextAreaElement>(null);
  const { value } = props;
  useLayoutEffect(() => {
    const area = ref.current;
    if (!area) return;
    const fit = (): void => {
      area.style.blockSize = '0px';
      area.style.blockSize = `${String(area.scrollHeight)}px`;
    };
    fit();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(fit);
    observer.observe(area.parentElement ?? area);
    return () => {
      observer.disconnect();
    };
  }, [value]);
  return (
    <div className={`${styles.frame ?? ''} ${className}`}>
      <textarea ref={ref} rows={1} {...props} />
      {trailing && <span className={styles.trailing}>{trailing}</span>}
    </div>
  );
}
