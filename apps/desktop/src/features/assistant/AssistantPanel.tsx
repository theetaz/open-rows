// SPDX-License-Identifier: Apache-2.0
import { Button, SelectField, TextField } from '@openrows/ui';
import type { AssistantProps } from './context';
import { cancelConversation, toggleAssistant } from './conversation';
import { useAssistantPanel } from './useAssistantPanel';
import { ConversationView } from './ConversationView';
import styles from './Assistant.module.css';
import { t } from './lazyMessages';
type Controls = ReturnType<typeof useAssistantPanel>;
/** Docked query assistant with explicit proposals, policy and every tool step visible. */
export function AssistantPanel(props: AssistantProps): React.JSX.Element {
  const controls = useAssistantPanel(props);
  const busy = controls.conversation.running !== null;
  return (
    <aside
      aria-label={
        props.monitoring ? t('monitoringAssistant') : t('queryAssistant')
      }
      className={styles.panel}
    >
      <header className={styles.row}>
        <strong>{t('assistant')}</strong>
        <Button
          size="sm"
          onClick={() => {
            toggleAssistant(props.tabId);
          }}
        >
          {t('closeAssistant')}
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
      {!props.monitoring && (
        <QuickActions
          controls={controls}
          props={props}
          disabled={!!controls.reason || busy}
        />
      )}
      <Composer
        controls={controls}
        busy={busy}
        tabId={props.tabId}
        label={props.monitoring ? t('askAboutHealth') : t('askAboutQuery')}
      />
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
        <span>
          {t('sharingLevel', { level: controls.policy?.level ?? 'loading' })}
        </span>
        <Button
          size="sm"
          disabled={!!controls.reason || busy}
          onClick={() => {
            void controls.inspect();
          }}
        >
          {t('contextPreview')}
        </Button>
      </div>
      {controls.reason && <p role="status">{controls.reason}</p>}
      {controls.failure && <p role="alert">{controls.failure}</p>}
      {controls.preview && (
        <details open>
          <summary>
            {controls.preview.truncated
              ? t('whatSentBytes', { bytes: controls.preview.bytes })
              : t('whatSentBytesBytes', { bytes: controls.preview.bytes })}
          </summary>
          <pre aria-label={t('exactAssistantContext')}>
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
  label,
}: {
  controls: Controls;
  busy: boolean;
  tabId: string;
  label: string;
}): React.JSX.Element {
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        controls.send(controls.question);
      }}
    >
      <label>
        {label}
        <textarea
          aria-label={t('assistantQuestion')}
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
          {t('send')}
        </Button>
        <Button
          type="button"
          disabled={!busy}
          onClick={() => {
            cancelConversation(tabId);
          }}
        >
          {t('cancelAssistant')}
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
        label={t('assistantProvider')}
        value={controls.provider}
        disabled={busy}
        options={[
          { value: '', label: t('chooseProvider') },
          ...controls.settings.providers.map((provider) => ({
            value: provider.id,
            label: provider.name,
          })),
        ]}
        onValueChange={controls.setProvider}
      />
      <TextField
        label={t('assistantModel')}
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
        {t('verifyModels')}
      </Button>
      {controls.verification.failure && (
        <p role="alert">{controls.verification.failure}</p>
      )}
      <TextField
        label={t('assistantSchema')}
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
    <div className={styles.row} aria-label={t('assistantQuickActions')}>
      <Button
        size="sm"
        disabled={disabled || !props.sql().trim()}
        onClick={() => {
          controls.send('Explain this query');
        }}
      >
        {t('explainQuery')}
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
        {t('fixError')}
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
        {t('optimisePlan')}
      </Button>
    </div>
  );
}
