// SPDX-License-Identifier: Apache-2.0
import type { ResultMeta, RunMessage } from './run';

/** The messages panel's level filter. */
export type LevelFilter = 'all' | RunMessage['level'];

/**
 * Messages of the shown result (FR-EDIT-15): with several results, those of
 * the shown result's statement plus those of statements without a result
 * (so no message is ever hidden); server notices only when they are shown.
 */
export function resultMessages(input: {
  messages: readonly RunMessage[];
  results: readonly ResultMeta[];
  current: ResultMeta | undefined;
  showNotices: boolean;
}): RunMessage[] {
  const { results, current } = input;
  const withResult = new Set(results.map((result) => result.statement));
  const scoped = current !== undefined && results.length > 1;
  return input.messages.filter((message) => {
    if (!input.showNotices && message.level !== 'error') return false;
    if (!scoped || !withResult.has(message.statement)) return true;
    return message.statement === current.statement;
  });
}

/** Messages at the chosen level. */
export function filterLevel(
  messages: readonly RunMessage[],
  level: LevelFilter,
): RunMessage[] {
  return level === 'all'
    ? [...messages]
    : messages.filter((message) => message.level === level);
}
