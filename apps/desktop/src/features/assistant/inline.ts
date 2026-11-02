// SPDX-License-Identifier: Apache-2.0
import { cancelAssistant, runAssistant } from '@openrows/ipc';
import type { InlineGenerate } from '@openrows/editor';
import type { ProfileDto } from '@openrows/ipc';
import { aiSettings } from '../settings/ai/aiSettings';
import { assistantSchema } from './context';
import { inlineBlock, inlineFailure } from './inlineSetup';
const STREAM_LIMIT = 32768;
/** The SQL body of a partially streamed fenced answer, for the live diff. */
export function streamedSql(text: string): string {
  const start = /```sql[^\n]*\n/i.exec(text);
  if (!start) return text;
  const body = text.slice(start.index + start[0].length);
  const end = body.indexOf('```');
  return end < 0 ? body : body.slice(0, end);
}
/** Reuse native provider/sharing gates; inline mode supplies no tools or execution action. */
export function inlineGenerator(scope: {
  profile: ProfileDto;
  database: string;
}): InlineGenerate {
  return async ({ prompt, sql, signal, delta }): Promise<string> => {
    const settings = aiSettings.get();
    const block = inlineBlock(settings);
    if (block) throw new Error(block.reason);
    if (signal.aborted) throw new Error('Cancelled');
    const provider =
      settings.defaultProvider || settings.providers[0]?.id || '';
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
          text = (text + event.delta).slice(0, STREAM_LIMIT);
          delta(streamedSql(text));
        },
      );
    } catch (error) {
      throw new Error(inlineFailure(error));
    } finally {
      signal.removeEventListener('abort', cancel);
    }
    if (isAborted(signal)) throw new Error('Cancelled');
    const fenced = /```sql[^\n]*\n([\s\S]*?)```/i.exec(text);
    if (!fenced?.[1])
      throw new Error('The provider did not return a fenced SQL proposal.');
    return fenced[1].trim();
  };
}

/** Read after awaits; property narrowing would otherwise treat it as constant. */
function isAborted(signal: AbortSignal): boolean {
  return signal.aborted;
}
