// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, IconButton, SelectField, TextField } from '@openrows/ui';
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
      <header className={styles.head}>
        <Icon name="sparkles" />
        <strong>{t('assistant')}</strong>
        <IconButton
          size="sm"
          variant="ghost"
          aria-label={t('closeAssistant')}
          title={t('closeAssistant')}
          onClick={() => {
            toggleAssistant(props.tabId);
          }}
        >
          <Icon name="close" />
        </IconButton>
      </header>
      <div className={styles.body}>
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
      </div>
      <div className={styles.ask}>
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
      </div>
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
      <div className={styles.meta}>
        <span>
          {t('sharingLevel', { level: controls.policy?.level ?? 'loading' })}
        </span>
        <Button
          size="sm"
          variant="ghost"
          disabled={!!controls.reason || busy}
          onClick={() => {
            void controls.inspect();
          }}
        >
          {t('contextPreview')}
        </Button>
      </div>
      {controls.reason && (
        <p role="status" className={styles.notice}>
          {controls.reason}
        </p>
      )}
      {controls.failure && (
        <p role="alert" className={styles.notice}>
          {controls.failure}
        </p>
      )}
      {controls.preview && (
        <details open className={styles.preview}>
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
  const blocked = !!controls.reason || busy;
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        controls.send(controls.question);
      }}
    >
      <div className={styles.composer}>
        <textarea
          aria-label={t('assistantQuestion')}
          placeholder={label}
          rows={1}
          maxLength={4096}
          value={controls.question}
          disabled={blocked}
          onChange={(event) => {
            controls.setQuestion(event.target.value);
          }}
        />
        <IconButton
          type="submit"
          size="sm"
          variant="ghost"
          aria-label={t('send')}
          title={t('send')}
          disabled={blocked || !controls.question.trim()}
        >
          <Icon name="send" />
        </IconButton>
      </div>
      <div className={styles.askFoot}>
        <Button
          type="button"
          size="sm"
          variant="ghost"
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
    <div className={styles.setup}>
      <SelectField
        className={styles.field}
        label={t('provider')}
        aria-label={t('assistantProvider')}
        controlSize="compact"
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
        className={styles.field}
        label={t('model')}
        aria-label={t('assistantModel')}
        controlSize="compact"
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
      <TextField
        className={styles.field}
        label={t('schema')}
        aria-label={t('assistantSchema')}
        controlSize="compact"
        value={controls.schema}
        disabled={busy}
        maxLength={256}
        onChange={(event) => {
          controls.setSchema(event.target.value);
        }}
      />
      <VerifyModels controls={controls} busy={busy} />
    </div>
  );
}
/** Explicit model discovery for the chosen provider, with its failure. */
function VerifyModels({
  controls,
  busy,
}: {
  controls: Controls;
  busy: boolean;
}): React.JSX.Element {
  return (
    <>
      <div className={styles.setupFoot}>
        <Button
          size="sm"
          disabled={
            busy ||
            (Boolean(controls.reason) &&
              controls.reason !== 'Choose a model.') ||
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
      </div>
      {controls.verification.failure && (
        <p role="alert" className={styles.notice}>
          {controls.verification.failure}
        </p>
      )}
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
