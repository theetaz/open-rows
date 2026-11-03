// SPDX-License-Identifier: Apache-2.0
import { createElement, Fragment } from 'react';
import type { ReactNode } from 'react';

/** The source language; a translation replaces catalog text, never a component. */
const LOCALE = 'en';

/** Text per plural category; a language with more categories adds more keys. */
export interface PluralForms {
  readonly one: string;
  readonly other: string;
}

type Catalog = Readonly<Record<string, string | PluralForms>>;

/** The `{name}` placeholders of a message, as a union of names. */
type Placeholders<Text extends string> =
  Text extends `${string}{${infer Name}}${infer Rest}`
    ? Name | Placeholders<Rest>
    : never;

type TextKey<M extends Catalog> = {
  [K in keyof M]: M[K] extends string ? K : never;
}[keyof M];

type PluralKey<M extends Catalog> = {
  [K in keyof M]: M[K] extends PluralForms ? K : never;
}[keyof M];

type Values<Name extends string, Value> = [Name] extends [never]
  ? []
  : [values: Readonly<Record<Name, Value>>];

type PluralPlaceholders<Forms> = Forms extends PluralForms
  ? Placeholders<Forms['one']> | Placeholders<Forms['other']>
  : never;

/** `{count}` is filled from the count unless the caller passes a formatted one. */
type PluralValues<Name extends string> = [Exclude<Name, 'count'>] extends [
  never,
]
  ? [values?: { readonly count?: string | number }]
  : [
      values: Readonly<Record<Exclude<Name, 'count'>, string | number>> & {
        readonly count?: string | number;
      },
    ];

type RichPluralValues<Name extends string> = [Exclude<Name, 'count'>] extends [
  never,
]
  ? [values?: { readonly count?: ReactNode }]
  : [
      values: Readonly<Record<Exclude<Name, 'count'>, ReactNode>> & {
        readonly count?: ReactNode;
      },
    ];

/** The typed lookups of one catalog. */
export interface Messages<M extends Catalog> {
  /** The text of `key`, with its `{name}` placeholders filled in. */
  t: <K extends TextKey<M>>(
    key: K,
    ...values: Values<Placeholders<M[K] & string>, string | number>
  ) => string;
  /** The plural form of `key` for `count`. */
  tn: <K extends PluralKey<M>>(
    key: K,
    count: number,
    ...values: PluralValues<PluralPlaceholders<M[K]>>
  ) => string;
  /** Like `t`, for placeholders that are elements or may render nothing. */
  rich: <K extends TextKey<M>>(
    key: K,
    ...values: Values<Placeholders<M[K] & string>, ReactNode>
  ) => ReactNode;
  /** Like `tn`, for placeholders that are elements or may render nothing. */
  richn: <K extends PluralKey<M>>(
    key: K,
    count: number,
    ...values: RichPluralValues<PluralPlaceholders<M[K]>>
  ) => ReactNode;
}

const PLACEHOLDER = /\{(\w+)\}/g;

function fill(
  text: string,
  values: Readonly<Record<string, string | number>>,
): string {
  // One pass only, so a value that contains braces is never expanded again.
  return text.replace(PLACEHOLDER, (whole, name: string) => {
    const value = values[name];
    return value === undefined ? whole : String(value);
  });
}

let pluralRules: Intl.PluralRules | undefined;

function pluralCategory(count: number): keyof PluralForms {
  pluralRules ??= new Intl.PluralRules(LOCALE);
  return pluralRules.select(count) === 'one' ? 'one' : 'other';
}

function richParts(
  text: string,
  values: Readonly<Record<string, ReactNode>>,
): ReactNode {
  // Odd indexes hold the placeholder names captured by the split.
  const parts = text
    .split(PLACEHOLDER)
    .map((part, index) => (index % 2 === 0 ? part : values[part]));
  return createElement(Fragment, null, ...parts);
}

/**
 * Wraps a feature's message catalog in lookups checked by `tsc`: an unknown key
 * or a missing placeholder value is a compile error.
 */
export function defineMessages<const M extends Catalog>(
  messages: M,
): Messages<M> {
  const lookup = messages as Catalog;
  return {
    t: (key, ...[values]) => {
      const text = lookup[key as string] as string;
      return values ? fill(text, values) : text;
    },
    tn: (key, count, ...[values]) => {
      const forms = lookup[key as string] as PluralForms;
      return fill(forms[pluralCategory(count)], { count, ...values });
    },
    rich: (key, ...[values]) =>
      richParts(lookup[key as string] as string, values ?? {}),
    richn: (key, count, ...[values]) => {
      const forms = lookup[key as string] as PluralForms;
      return richParts(forms[pluralCategory(count)], { count, ...values });
    },
  };
}
