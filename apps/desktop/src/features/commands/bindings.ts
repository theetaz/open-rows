// SPDX-License-Identifier: Apache-2.0
import { useMemo } from 'react';
import { localSetting } from '../settings/localSetting';
import { COMMANDS, RESERVED, commandSpec } from './registry';
import { comboOf, displayCombo, formatCombo, parseCombo } from './keys';
import type { KeyCombo } from './keys';

/** Rebound shortcuts by command id; `null` removes a default shortcut. */
type Overrides = Readonly<Record<string, string | null>>;

/** Key of the shortcut overrides in web storage (and `shortcuts` in settings.json). */
export const SHORTCUTS_KEY = 'openrows.shortcuts';

/** Keep only known commands with a valid shortcut or `null`. */
export function parseOverrides(stored: unknown): Overrides {
  if (typeof stored !== 'object' || stored === null) return {};
  const entries = Object.entries(stored as Record<string, unknown>).filter(
    ([id, value]) =>
      commandSpec(id) !== undefined &&
      (value === null || (typeof value === 'string' && parseCombo(value))),
  );
  return Object.fromEntries(entries) as Overrides;
}

const shortcutOverrides = localSetting(SHORTCUTS_KEY, parseOverrides);

/** The effective shortcut of every command (defaults with overrides applied). */
export type Bindings = Readonly<Record<string, KeyCombo | null>>;

export function bindingsOf(overrides: Overrides): Bindings {
  return Object.fromEntries(
    COMMANDS.map(({ id, shortcut }) => {
      const text = Object.hasOwn(overrides, id) ? overrides[id] : shortcut;
      return [id, text ? parseCombo(text) : null];
    }),
  );
}

/** The current bindings, read synchronously by key handlers. */
export function currentBindings(): Bindings {
  return bindingsOf(shortcutOverrides.get());
}

/** The current bindings, re-rendering on change. */
export function useBindings(): Bindings {
  const overrides = shortcutOverrides.useValue();
  return useMemo(() => bindingsOf(overrides), [overrides]);
}

/** The command bound to this key press among `ids` (all commands by default). */
export function commandForEvent(
  event: Parameters<typeof comboOf>[0],
  ids?: readonly string[],
): string | undefined {
  const combo = comboOf(event);
  if (!combo) return undefined;
  const key = formatCombo(combo);
  const bindings = currentBindings();
  return (ids ?? COMMANDS.map(({ id }) => id)).find((id) => {
    const bound = bindings[id];
    return bound ? formatCombo(bound) === key : false;
  });
}

/** Menu-style text of a command's shortcut, its hint, or `—`. */
export function shortcutText(bindings: Bindings, id: string): string {
  const combo = bindings[id];
  if (combo) return displayCombo(combo);
  return commandSpec(id)?.hint ?? '—';
}

/** Who already owns `combo`: a reserved system item or another command. */
export function conflictOf(
  bindings: Bindings,
  id: string,
  combo: KeyCombo,
): { kind: 'reserved' | 'command'; owner: string; id?: string } | null {
  const key = formatCombo(combo);
  const reserved = RESERVED[key];
  if (reserved) return { kind: 'reserved', owner: reserved };
  const other = COMMANDS.find(
    (command) =>
      command.id !== id &&
      bindings[command.id] &&
      formatCombo(bindings[command.id] as KeyCombo) === key,
  );
  return other ? { kind: 'command', owner: other.label, id: other.id } : null;
}

/**
 * Bind `combo` to `id` (or remove its shortcut with `null`); `steal` is the
 * command that loses the same shortcut. Defaults are stored as no override.
 */
export function rebind(
  id: string,
  combo: KeyCombo | null,
  steal?: string,
): void {
  const kept = Object.entries(shortcutOverrides.get()).filter(
    ([key]) => key !== id && key !== steal,
  );
  const next: Record<string, string | null> = Object.fromEntries(kept);
  const text = combo ? formatCombo(combo) : null;
  if (text !== (commandSpec(id)?.shortcut ?? null)) next[id] = text;
  if (steal) next[steal] = null;
  shortcutOverrides.set(next);
}

/** Restore every default shortcut. */
export function resetBindings(): void {
  shortcutOverrides.set({});
}
