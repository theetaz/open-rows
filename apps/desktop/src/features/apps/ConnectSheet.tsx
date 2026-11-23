// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button, SelectField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { connectAgentApp, planAgentApp } from '@openrows/ipc';
import type { AgentApp, AppPlan } from '@openrows/ipc';
import { useProfiles } from '../connections/useProfiles';
import { aiSettings } from '../settings/ai/aiSettings';
import { mcpSettings, saveMcpSettings } from '../settings/ai/mcpSettings';
import { AppGlyph } from './AppGlyph';
import { preferredProvider } from './appState';
import { appName } from './appNames';
import { CommandBlock } from './CommandBlock';
import styles from './Apps.module.css';
import { t } from './messages';

/** Saved connections a token may cover at most (the server's limit). */
const MAX_CONNECTIONS = 32;

function failureText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Turn the server on at the planned port, then register through the tool. */
async function connect(
  app: AgentApp,
  plan: AppPlan,
  scope: { provider: string; connections: string[] },
): Promise<void> {
  const settings = mcpSettings.get();
  if (!settings.enabled || settings.port !== plan.port)
    await saveMcpSettings({ ...settings, enabled: true, port: plan.port });
  await connectAgentApp({
    app,
    client: plan.client,
    port: plan.port,
    ...scope,
  });
}

/** Saved provider identity and the connections the app's token covers. */
function Scope(props: {
  provider: string;
  connections: readonly string[];
  onProvider: (id: string) => void;
  onConnections: (ids: string[]) => void;
  onProviders: () => void;
}): React.JSX.Element {
  const { providers } = aiSettings.useValue();
  const { profiles } = useProfiles();
  if (!providers.length)
    return (
      <div className={styles.callout}>
        <p>{t('noProviders')}</p>
        <Button size="sm" onClick={props.onProviders}>
          {t('goToProviders')}
        </Button>
      </div>
    );
  return (
    <fieldset className={styles.scope}>
      <legend>{t('access')}</legend>
      <SelectField
        label={t('countsAs')}
        controlSize="compact"
        value={props.provider}
        options={providers.map((item) => ({
          value: item.id,
          label: item.name,
        }))}
        onValueChange={props.onProvider}
      />
      <p className={styles.hint}>{t('providerHelp')}</p>
      <div
        className={styles.checks}
        role="group"
        aria-label={t('connectionsLabel')}
      >
        {profiles.length === 0 && (
          <p className={styles.hint}>{t('noConnections')}</p>
        )}
        {profiles.slice(0, MAX_CONNECTIONS).map((profile) => (
          <label key={profile.id} className={styles.check}>
            <input
              type="checkbox"
              aria-label={t('allowConnection', { name: profile.name })}
              checked={props.connections.includes(profile.id)}
              onChange={(event) => {
                props.onConnections(
                  event.target.checked
                    ? [...props.connections, profile.id]
                    : props.connections.filter((id) => id !== profile.id),
                );
              }}
            />
            <span>{profile.name}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}

/** Plan once on open; preselect the provider and every saved connection. */
function usePlan(app: AgentApp): {
  plan: AppPlan | null;
  failure: string;
  setFailure: (text: string) => void;
} {
  const [plan, setPlan] = useState<AppPlan | null>(null);
  const [failure, setFailure] = useState('');
  useEffect(() => {
    let alive = true;
    planAgentApp(app).then(
      (next) => {
        if (alive) setPlan(next);
      },
      (error: unknown) => {
        if (alive) setFailure(failureText(error));
      },
    );
    return () => {
      alive = false;
    };
  }, [app]);
  return { plan, failure, setFailure };
}

/** The exact commands and what else changes. */
function PlanNotes(props: { plan: AppPlan; name: string }): React.JSX.Element {
  const server = mcpSettings.useValue();
  const { plan } = props;
  return (
    <>
      <CommandBlock commands={plan.commands} />
      <ul className={styles.notes}>
        {plan.replaces && <li>{t('willReplace')}</li>}
        {(!server.enabled || server.port !== plan.port) && (
          <li>{t('noteServer', { port: String(plan.port) })}</li>
        )}
        <li>{t('noteKeychain', { name: props.name })}</li>
        <li>{t('noteExpiry')}</li>
      </ul>
    </>
  );
}

/**
 * Centred confirmation: the exact commands OpenRows runs with the app's own
 * CLI, what changes, and the token's scope. Confirming is the consent to turn
 * the MCP server on.
 */
export function ConnectSheet(props: {
  app: AgentApp;
  onClose: () => void;
  onConnected: () => void;
  onProviders: () => void;
}): React.JSX.Element {
  const name = appName(props.app);
  const ai = aiSettings.useValue();
  const { profiles } = useProfiles();
  const { plan, failure, setFailure } = usePlan(props.app);
  const [provider, setProvider] = useState(() =>
    preferredProvider(props.app, ai.providers, ai.defaultProvider),
  );
  const [chosen, setChosen] = useState<string[] | null>(null);
  const [busy, setBusy] = useState(false);
  const connections =
    chosen ?? profiles.slice(0, MAX_CONNECTIONS).map((profile) => profile.id);
  const ready =
    plan !== null && ai.enabled && provider !== '' && connections.length > 0;
  return (
    <Sheet
      title={t('connectName', { name })}
      size="form"
      onClose={props.onClose}
    >
      <div className={styles.sheetBody}>
        <div className={styles.sheetLead}>
          <AppGlyph app={props.app} />
          <p>{t('appsIntro')}</p>
        </div>
        {!ai.enabled && <p className={styles.reason}>{t('aiOff')}</p>}
        <h4 className={styles.label}>{t('willRun')}</h4>
        {plan ? (
          <PlanNotes plan={plan} name={name} />
        ) : (
          !failure && <p role="status">{t('planning')}</p>
        )}
        <Scope
          provider={provider}
          connections={connections}
          onProvider={setProvider}
          onConnections={setChosen}
          onProviders={props.onProviders}
        />
        {failure && <p role="alert">{failure}</p>}
      </div>
      <SheetActions>
        <Button onClick={props.onClose}>{t('cancel')}</Button>
        <Button
          variant="primary"
          disabled={!ready || busy}
          onClick={() => {
            if (!plan) return;
            setBusy(true);
            setFailure('');
            connect(props.app, plan, { provider, connections }).then(
              props.onConnected,
              (error: unknown) => {
                setBusy(false);
                setFailure(failureText(error));
              },
            );
          }}
        >
          {busy ? t('connecting') : t('connect')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
