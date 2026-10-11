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
/** Inputs attached to the active query tab; SQL is read only when the user acts. */
export type AssistantProps = {
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
  if (!input.enabled)
    return 'AI features are off. Enable them in Settings → AI.';
  if (!input.provider)
    return 'No provider configured. Add one in Settings → AI.';
  if (input.locked) return 'OpenRows is locked.';
  if (!input.props.connected) return 'Open this database to use the assistant.';
  if (!input.policy) return 'Loading sharing policy…';
  if (input.policy.level === 'none')
    return 'Data sharing is None. Set a policy in Settings → AI → Data sharing.';
  if (
    input.props.profile.env.kind === 'prod' &&
    !input.policy.productionConfirmed
  )
    return 'Production sharing requires explicit opt-in.';
  if (!input.policy.providers.includes(input.provider))
    return 'Provider is not allowed for this connection.';
  if (!input.model) return 'Choose a model.';
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
