// SPDX-License-Identifier: Apache-2.0
import { Button, SelectField, TextField } from '@openrows/ui';
import type { AssistantProps } from './context';
import { cancelConversation, toggleAssistant } from './conversation';
import { useAssistantPanel } from './useAssistantPanel';
import { ConversationView } from './ConversationView';
import styles from './Assistant.module.css';
type Controls = ReturnType<typeof useAssistantPanel>;
/** Docked query assistant with explicit proposals, policy and every tool step visible. */
export function AssistantPanel(props: AssistantProps): React.JSX.Element {
  const controls = useAssistantPanel(props);
  const busy = controls.conversation.running !== null;
  return (
    <aside aria-label="Query assistant" className={styles.panel}>
      <header className={styles.row}>
        <strong>Assistant</strong>
        <Button
          size="sm"
          onClick={() => {
            toggleAssistant(props.tabId);
          }}
        >
          Close assistant
        </Button>
      </header>
      <ProviderPicker controls={controls} busy={busy} />
      <ContextNotice controls={controls} busy={busy} />
      <ConversationView
        connection={props.profile.id}
        turns={controls.conversation.turns}
        disabled={!!controls.reason || busy}
        actions={{
          insert: props.onInsert,
          open: props.onOpen,
          explain: (sql) => {
            void controls.inspect(sql);
          },
        }}
      />
      <QuickActions
        controls={controls}
        props={props}
        disabled={!!controls.reason || busy}
      />
      <Composer controls={controls} busy={busy} tabId={props.tabId} />
    </aside>
  );
}
function ContextNotice({
  controls,
  busy,
}: {
  controls: Controls;
  busy: boolean;
}): React.JSX.Element {
  return (
    <>
      <div className={styles.row}>
        <span>Sharing: {controls.policy?.level ?? 'loading'}</span>
        <Button
          size="sm"
          disabled={!!controls.reason || busy}
          onClick={() => {
            void controls.inspect();
          }}
        >
          Context preview
        </Button>
      </div>
      {controls.reason && <p role="status">{controls.reason}</p>}
      {controls.failure && <p role="alert">{controls.failure}</p>}
      {controls.preview && (
        <details open>
          <summary>
            What will be sent · {controls.preview.bytes} bytes
            {controls.preview.truncated ? ' · truncated' : ''}
          </summary>
          <pre aria-label="Exact assistant context">
            {controls.preview.payload}
          </pre>
        </details>
      )}
    </>
  );
}
function Composer({
  controls,
  busy,
  tabId,
}: {
  controls: Controls;
  busy: boolean;
  tabId: string;
}): React.JSX.Element {
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        controls.send(controls.question);
      }}
    >
      <label>
        Ask about this query
        <textarea
          aria-label="Assistant question"
          maxLength={4096}
          value={controls.question}
          disabled={!!controls.reason || busy}
          onChange={(event) => {
            controls.setQuestion(event.target.value);
          }}
        />
      </label>
      <div className={styles.row}>
        <Button
          type="submit"
          disabled={!!controls.reason || busy || !controls.question.trim()}
        >
          Send
        </Button>
        <Button
          type="button"
          disabled={!busy}
          onClick={() => {
            cancelConversation(tabId);
          }}
        >
          Cancel assistant
        </Button>
      </div>
    </form>
  );
}
function ProviderPicker({
  controls,
  busy,
}: {
  controls: Controls;
  busy: boolean;
}): React.JSX.Element {
  return (
    <>
      <SelectField
        label="Assistant provider"
        value={controls.provider}
        disabled={busy}
        options={[
          { value: '', label: 'Choose provider' },
          ...controls.settings.providers.map((provider) => ({
            value: provider.id,
            label: provider.name,
          })),
        ]}
        onValueChange={controls.setProvider}
      />
      <TextField
        label="Assistant model"
        value={controls.model}
        disabled={busy}
        maxLength={256}
        list="assistant-models"
        onChange={(event) => {
          controls.setModel(event.target.value);
        }}
      />
      <datalist id="assistant-models">
        {controls.verification.result?.models.map((model) => (
          <option key={model.id} value={model.id}>
            {model.name}
          </option>
        ))}
      </datalist>
      <Button
        size="sm"
        disabled={
          busy ||
          (Boolean(controls.reason) && controls.reason !== 'Choose a model.') ||
          controls.locked ||
          !controls.settings.enabled ||
          !controls.provider ||
          controls.verification.busy
        }
        onClick={() => {
          void controls.verification.verify();
        }}
      >
        Verify models
      </Button>
      {controls.verification.failure && (
        <p role="alert">{controls.verification.failure}</p>
      )}
      <TextField
        label="Assistant schema"
        value={controls.schema}
        disabled={busy}
        maxLength={256}
        onChange={(event) => {
          controls.setSchema(event.target.value);
        }}
      />
    </>
  );
}
function QuickActions({
  controls,
  props,
  disabled,
}: {
  controls: Controls;
  props: AssistantProps;
  disabled: boolean;
}): React.JSX.Element {
  return (
    <div className={styles.row} aria-label="Assistant quick actions">
      <Button
        size="sm"
        disabled={disabled || !props.sql().trim()}
        onClick={() => {
          controls.send('Explain this query');
        }}
      >
        Explain this query
      </Button>
      <Button
        size="sm"
        disabled={disabled || !props.lastError}
        title={props.lastError?.text}
        onClick={() => {
          controls.send(
            'Fix this error using the query and error code. Propose corrected SQL.',
            'fix',
          );
        }}
      >
        Fix this error
      </Button>
      <Button
        size="sm"
        disabled={disabled || !props.plan || props.plan.analyzed}
        onClick={() => {
          controls.send(
            'Optimise this query using the attached plan-only evidence. State limitations and propose SQL.',
            'optimise',
          );
        }}
      >
        Optimise from plan
      </Button>
    </div>
  );
}
