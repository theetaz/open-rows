// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { ProfileDto } from '@openrows/ipc';
import { toggleAssistant } from '../assistant/conversation';
import { isDocumentEngine } from '../connections/engines';
import type { EditorHandle } from './request';
import { t } from './lazyMessages';

/** Why the AI button and ⌘I do nothing on a document engine. */
const AI_UNAVAILABLE = t('aiNotAvailable');

/**
 * The AI button and ⌘I: the assistant panel and the editor's inline prompt.
 * Document engines get a notice instead: the AI features reason about SQL.
 */
export function useAiActions(
  connection: ProfileDto,
  tabId: string,
  editor: { current: EditorHandle | null },
): [string | null, { onInline: () => void; onAssistant: () => void }] {
  const [notice, setNotice] = useState<string | null>(null);
  if (isDocumentEngine(connection.engine)) {
    const explain = (): void => {
      setNotice(AI_UNAVAILABLE);
    };
    return [notice, { onInline: explain, onAssistant: explain }];
  }
  return [
    null,
    {
      onInline: () => {
        editor.current?.promptInline?.();
      },
      onAssistant: () => {
        toggleAssistant(tabId);
      },
    },
  ];
}

/** Explain plans exist for SQL engines except DuckDB. */
export function explainable(connection: ProfileDto): boolean {
  return connection.engine !== 'duckdb' && !isDocumentEngine(connection.engine);
}
