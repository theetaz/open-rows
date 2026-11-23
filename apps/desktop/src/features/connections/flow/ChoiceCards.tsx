// SPDX-License-Identifier: Apache-2.0
import type { KeyboardEvent } from 'react';
import { FlowIcon } from './FlowIcon';
import type { FlowIconName } from './FlowIcon';
import styles from './Cards.module.css';

export type Card<Value extends string> = {
  value: Value;
  icon: FlowIconName;
  title: string;
  detail: string;
  disabled?: boolean;
};

const STEP: Partial<Record<string, number>> = {
  ArrowRight: 1,
  ArrowDown: 1,
  ArrowLeft: -1,
  ArrowUp: -1,
};

/** The enabled card an arrow key moves to, wrapping. */
function moved<Value extends string>(
  cards: readonly Card<Value>[],
  value: Value,
  key: string,
): Value | undefined {
  const step = STEP[key];
  const enabled = cards.filter((card) => !card.disabled);
  const index = enabled.findIndex((card) => card.value === value);
  if (step === undefined || enabled.length === 0) return undefined;
  return enabled[(index + step + enabled.length) % enabled.length]?.value;
}

/** A radio group drawn as cards with a glyph, a title and one line of detail. */
export function ChoiceCards<Value extends string>(props: {
  label: string;
  cards: readonly Card<Value>[];
  value: Value;
  onChange: (value: Value) => void;
  autoFocus?: boolean | undefined;
}): React.JSX.Element {
  const onKeyDown = (event: KeyboardEvent): void => {
    const next = moved(props.cards, props.value, event.key);
    if (next === undefined) return;
    event.preventDefault();
    props.onChange(next);
    const group = event.currentTarget;
    queueMicrotask(() => {
      group.querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
    });
  };
  return (
    <div
      role="radiogroup"
      aria-label={props.label}
      className={styles.cards}
      onKeyDown={onKeyDown}
    >
      {props.cards.map((card) => {
        const checked = card.value === props.value;
        return (
          <button
            key={card.value}
            type="button"
            role="radio"
            aria-checked={checked}
            tabIndex={checked ? 0 : -1}
            disabled={card.disabled}
            autoFocus={checked && props.autoFocus}
            className={styles.card}
            onClick={() => {
              props.onChange(card.value);
            }}
          >
            <FlowIcon name={card.icon} />
            <b>{card.title}</b>
            <span>{card.detail}</span>
          </button>
        );
      })}
    </div>
  );
}
