// SPDX-License-Identifier: Apache-2.0
import { cancelAssistant, runAssistant } from '@openrows/ipc';
import type { InlineGenerate } from '@openrows/editor';
import type { ProfileDto } from '@openrows/ipc';
import { aiSettings } from '../settings/ai/aiSettings';
import { assistantSchema } from './context';
/** Reuse native provider/sharing gates; inline mode supplies no tools or execution action. */
export function inlineGenerator(scope: {
  profile: ProfileDto;
  database: string;
}): InlineGenerate {
  return async ({ prompt, sql, signal, delta }): Promise<string> => {
    const settings = aiSettings.get();
    const provider =
      settings.defaultProvider || settings.providers[0]?.id || '';
    if (!settings.enabled || !provider || !settings.defaultModel)
      throw new Error(
        'Enable AI and choose a default provider/model in Settings → AI.',
      );
    if (signal.aborted) throw new Error('Cancelled');
    const id = crypto.randomUUID();
    const cancel = (): void => {
      void cancelAssistant(id).catch(() => undefined);
    };
    signal.addEventListener('abort', cancel, { once: true });
    let text = '';
    try {
      await runAssistant(
        {
          id,
          provider,
          model: settings.defaultModel,
          connection: scope.profile.id,
          database: scope.database,
          schema: assistantSchema(scope.profile.engine, scope.database),
          query: sql || null,
          plan: [],
          inline: true,
          question: {
            question: `Edit the supplied SQL or generate SQL at an empty insert point. Return exactly one fenced sql proposal. User request: ${prompt}`,
            history: [],
            errorCode: null,
          },
        },
        (event) => {
          if (signal.aborted || event.type !== 'text') return;
          text = (text + event.delta).slice(0, 32768);
          delta(text);
        },
      );
      if (isAborted(signal)) throw new Error('Cancelled');
      const block = /```sql\s*\n([\s\S]*?)```/i.exec(text);
      if (!block?.[1])
        throw new Error('The provider did not return a fenced SQL proposal.');
      return block[1].trim();
    } finally {
      signal.removeEventListener('abort', cancel);
    }
  };
}

function isAborted(signal: AbortSignal): boolean {
  return signal.aborted;
}
