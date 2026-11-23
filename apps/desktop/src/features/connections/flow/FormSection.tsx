// SPDX-License-Identifier: Apache-2.0
import { useId } from 'react';
import type { ReactNode } from 'react';
import { FlowIcon } from './FlowIcon';
import type { FlowIconName } from './FlowIcon';
import styles from './Form.module.css';

/** A labelled section: heading with its glyph, the inset group, an optional note. */
export function FormSection(props: {
  title: string;
  icon?: FlowIconName;
  note?: ReactNode;
  /** Content that is not an inset group (cards, a file well). */
  bare?: boolean;
  children: ReactNode;
}): React.JSX.Element {
  const id = useId();
  return (
    <section className={styles.section} aria-labelledby={id}>
      <h3 id={id} className={styles.heading}>
        {props.icon && <FlowIcon name={props.icon} />}
        {props.title}
      </h3>
      {props.bare ? (
        props.children
      ) : (
        <div className={styles.group}>{props.children}</div>
      )}
      {props.note && <p className={styles.note}>{props.note}</p>}
    </section>
  );
}
