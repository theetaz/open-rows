// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { ReactNode } from 'react';
import { Button } from '@openrows/ui';
import styles from '../table/structure/forms/Forms.module.css';

/** The inline form that adds an item to a list section. */
export type ItemForm<T> = {
  empty: T;
  problem: (draft: T) => string | null;
  render: (draft: T, setDraft: (draft: T) => void) => ReactNode;
  onAdd: (draft: T) => void;
};

/** The open add form: its fields, why it is incomplete, Cancel and Add. */
function AddForm<T>(props: {
  form: ItemForm<T>;
  draft: T;
  onDraft: (draft: T | null) => void;
}): React.JSX.Element {
  const { form, draft, onDraft } = props;
  const problem = form.problem(draft);
  return (
    <>
      {form.render(draft, onDraft)}
      {problem && <p className={styles.problem}>{problem}</p>}
      <div className={styles.sectionHead}>
        <span />
        <span>
          <Button
            size="sm"
            onClick={() => {
              onDraft(null);
            }}
          >
            Cancel
          </Button>{' '}
          <Button
            size="sm"
            variant="primary"
            disabled={problem !== null}
            onClick={() => {
              form.onAdd(draft);
              onDraft(null);
            }}
          >
            Add
          </Button>
        </span>
      </div>
    </>
  );
}

/**
 * A titled list (indexes, foreign keys or checks of a new table) with
 * Remove per item and an inline form that adds one.
 */
export function ListSection<T>(props: {
  title: string;
  items: readonly T[];
  describe: (item: T) => string;
  onRemove: (index: number) => void;
  /** Absent when the engine cannot add these. */
  form?: ItemForm<T>;
}): React.JSX.Element {
  const [draft, setDraft] = useState<T | null>(null);
  const { form } = props;
  return (
    <section className={styles.section} aria-label={props.title}>
      <div className={styles.sectionHead}>
        <h3>{props.title}</h3>
        {form && draft === null && (
          <Button
            size="sm"
            aria-label={`Add to ${props.title}`}
            onClick={() => {
              setDraft(form.empty);
            }}
          >
            Add…
          </Button>
        )}
      </div>
      <div className={styles.list}>
        {props.items.map((item, index) => (
          <div
            key={`${props.describe(item)}-${String(index)}`}
            className={styles.item}
          >
            <span>{props.describe(item)}</span>
            <span />
            <Button
              size="sm"
              onClick={() => {
                props.onRemove(index);
              }}
            >
              Remove
            </Button>
          </div>
        ))}
      </div>
      {form && draft !== null && (
        <AddForm form={form} draft={draft} onDraft={setDraft} />
      )}
    </section>
  );
}
