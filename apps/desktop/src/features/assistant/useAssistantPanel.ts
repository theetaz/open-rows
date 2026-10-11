// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import {
  getAiSharing,
  previewAssistant,
  explainAssistant,
} from '@openrows/ipc';
import type { ContextPreview, SharingPolicy } from '@openrows/ipc';
import { aiSettings } from '../settings/ai/aiSettings';
import { useSecurity } from '../security/useSecurity';
import { useVerification } from '../settings/ai/useVerification';
import { assistantRequest, assistantSchema, disabledReason } from './context';
import type { AssistantProps } from './context';
import {
  cancelConversation,
  sendQuestion,
  useConversation,
} from './conversation';
/** Explicit user actions and offline preview; mounting never calls a provider. */
export function useAssistantPanel(
  props: AssistantProps,
): ReturnType<typeof usePanelControls> {
  return usePanelControls(props);
}
function usePanelControls(props: AssistantProps) {
  const settings = aiSettings.useValue();
  const security = useSecurity();
  const [provider, setProvider] = useState(
    settings.defaultProvider || settings.providers[0]?.id || '',
  );
  const [model, setModel] = useState(settings.defaultModel);
  const [schema, setSchema] = useState(
    assistantSchema(props.profile.engine, props.database),
  );
  const [question, setQuestion] = useState('');
  const [preview, setPreview] = useState<ContextPreview | null>(null);
  const [failure, setFailure] = useState('');
  const [policy, policyFailure] = usePolicy(props.profile.id);
  const conversation = useConversation(props.tabId);
  const verification = useVerification(provider);
  useEffect(
    () => () => {
      cancelConversation(props.tabId);
    },
    [props.tabId],
  );
  const reason = disabledReason({
    enabled: settings.enabled,
    provider: settings.providers.some((item) => item.id === provider)
      ? provider
      : '',
    model,
    locked: security.locked,
    props,
    policy,
  });
  const selection = { provider, model, schema };
  const send = (
    text: string,
    action: 'ask' | 'fix' | 'optimise' = 'ask',
  ): void => {
    if (reason || conversation.running || !text.trim()) return;
    setPreview(null);
    setQuestion('');
    void sendQuestion(
      props.tabId,
      assistantRequest(props, selection, text, action),
    );
  };
  const inspect = inspector({
    props,
    selection,
    question,
    reason,
    busy: !!conversation.running,
    setPreview,
    setFailure,
  });
  return {
    settings,
    provider,
    model,
    schema,
    question,
    preview,
    failure: failure || policyFailure,
    policy,
    reason,
    locked: security.locked,
    conversation,
    verification,
    send,
    inspect,
    setQuestion,
    setModel,
    setSchema,
    setProvider: (id: string): void => {
      setProvider(id);
      setModel('');
      setPreview(null);
    },
  };
}

function usePolicy(
  connection: string,
): [Required<SharingPolicy> | null, string] {
  const [policy, setPolicy] = useState<Required<SharingPolicy> | null>(null);
  const [failure, setFailure] = useState('');
  useEffect(() => {
    let live = true;
    getAiSharing(connection).then(
      (value) => {
        if (live) setPolicy(value);
      },
      () => {
        if (live) setFailure('Sharing policy unavailable.');
      },
    );
    return () => {
      live = false;
    };
  }, [connection]);
  return [policy, failure];
}

function inspector({
  props,
  selection,
  question,
  reason,
  busy,
  setPreview,
  setFailure,
}: {
  props: AssistantProps;
  selection: { provider: string; model: string; schema: string };
  question: string;
  reason: string | null;
  busy: boolean;
  setPreview: (value: ContextPreview | null) => void;
  setFailure: (value: string) => void;
}): (sql?: string) => Promise<void> {
  return async (sql?: string): Promise<void> => {
    if (reason || busy) return;
    setFailure('');
    setPreview(null);
    const request = assistantRequest(
      props,
      selection,
      question || 'Explain this query',
    );
    try {
      setPreview(
        await (sql
          ? explainAssistant({ ...request, query: sql })
          : previewAssistant(request)),
      );
    } catch (error) {
      setFailure(
        error instanceof Error ? error.message : 'Context refused by policy.',
      );
    }
  };
}
