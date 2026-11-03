// SPDX-License-Identifier: Apache-2.0
import type {
  AssistantRequest,
  EngineDto,
  Plan,
  PlanDto,
  ProfileDto,
  SharingPolicy,
} from '@openrows/ipc';
import type { RunMessage } from '../query/run';
import { conversationOf } from './conversation';
import { t } from './lazyMessages';
/** Inputs attached to the active query tab; SQL is read only when the user acts. */
export type AssistantProps = {
  monitoring?: boolean;
  tabId: string;
  profile: ProfileDto;
  database: string;
  connected: boolean;
  sql: () => string;
  lastError: RunMessage | null;
  plan: PlanDto | null;
  onInsert: (sql: string) => void;
  onOpen: (sql: string) => void;
};
/** Every engine has an explicit schema default; users can select another schema. */
export function assistantSchema(engine: EngineDto, database: string): string {
  switch (engine) {
    case 'postgres':
      return 'public';
    case 'mysql':
    case 'mariadb':
    case 'mongodb':
      return database;
    case 'sqlserver':
      return 'dbo';
    case 'clickhouse':
      return 'default';
    case 'sqlite':
    case 'libsql':
    case 'libsql-file':
    case 'duckdb':
      return 'main';
  }
}
/** Plans share only fixed operations and estimates; raw vendor properties are excluded. */
function assistantPlan(plan: PlanDto | null): Plan[] {
  if (!plan || plan.analyzed) return [];
  return plan.nodes.slice(0, 200).map((node) => ({
    operation: operation(node.nodeType),
    parent: node.parent,
    relation: node.relation ?? '',
    predicate: '',
    estimatedRows: Math.min(
      4294967295,
      Math.max(0, Math.floor(node.estimatedRows ?? 0)),
    ),
  }));
}
function operation(name: string): Plan['operation'] {
  const text = name.toLowerCase();
  if (text.includes('scan')) return 'scan';
  if (text.includes('join')) return 'join';
  if (text.includes('sort')) return 'sort';
  if (text.includes('aggregate')) return 'aggregate';
  return 'other';
}
/** Native gates repeat these checks before reading or sending any context. */
export function disabledReason(input: {
  enabled: boolean;
  provider: string;
  model: string;
  locked: boolean;
  props: AssistantProps;
  policy: Required<SharingPolicy> | null;
}): string {
  if (!input.enabled) return t('aiFeaturesOff');
  if (!input.provider) return t('noProviderConfigured');
  if (input.locked) return t('openrowsLocked');
  if (!input.props.connected) return t('openDatabaseUse');
  if (!input.policy) return t('loadingSharingPolicy');
  if (input.policy.level === 'none') return t('dataSharingNone');
  if (
    input.props.profile.env.kind === 'prod' &&
    !input.policy.productionConfirmed
  )
    return t('productionSharingRequires');
  if (!input.policy.providers.includes(input.provider))
    return t('providerNotAllowed');
  if (
    input.props.monitoring &&
    (!input.policy.telemetry ||
      !['schemaAndStats', 'sampleRows'].includes(input.policy.level))
  )
    return t('monitoringRequiresTelemetry');
  if (!input.model) return t('chooseModel');
  return '';
}
/** Build a typed request; fix-error shares only a bounded SQLSTATE/vendor code. */
export function assistantRequest(
  props: AssistantProps,
  selection: { provider: string; model: string; schema: string },
  question: string,
  action: 'ask' | 'fix' | 'optimise' = 'ask',
): AssistantRequest {
  const code = action === 'fix' ? props.lastError?.code : null;
  return {
    id: crypto.randomUUID(),
    monitoring: props.monitoring ?? false,
    connection: props.profile.id,
    database: props.database,
    ...selection,
    question: {
      question,
      history: conversationOf(props.tabId)
        .turns.slice(-3)
        .map((turn) => ({ question: turn.question, answer: turn.text })),
      errorCode: code && /^[a-zA-Z0-9]{1,5}$/.test(code) ? code : null,
    },
    query: props.sql() || null,
    plan: action === 'optimise' ? assistantPlan(props.plan) : [],
  };
}
