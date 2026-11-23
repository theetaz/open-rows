// SPDX-License-Identifier: Apache-2.0
/** API-key/local/Codex CLI provider settings and metadata-only verification (M45-01/04). */
export {
  promptAiKey,
  deleteAiKey,
  verifyAiProvider,
  cancelAiVerification,
  signInCodex,
} from './ai';
export type {
  AiSettings,
  ProviderConfig,
  ProviderKind,
  Verification,
  Health,
  Capabilities,
  Model,
  Usage,
  AiError,
  CodexStatus,
  AccountMode,
  SecretEntry,
} from './generated/bindings';

/** Connection context sharing, offline preview and metadata-only audit (M45-02). */
export {
  getAiSharing,
  saveAiSharing,
  previewAiContext,
  listAiAudit,
  purgeAiAudit,
} from './aiSharing';
export type {
  SharingPolicy,
  SharingLevel,
  RedactionRule,
  ContextBundle,
  ContextPreview,
  SchemaObject,
  Plan,
  PlanOperation,
  Aggregate,
  Sample,
  AuditEntry,
  Category,
} from './generated/bindings';

export {
  mcpStatus,
  createMcpToken,
  revokeMcpToken,
  disconnectMcpClient,
  takeMcpProposals,
  onMcpProposals,
} from './mcp';
export type {
  McpSettings,
  ServerStatus,
  TokenInfo,
  TokenScope,
  CreatedToken,
  SqlProposal,
} from './generated/bindings';

/** Connect the user's agent CLIs to the built-in MCP server (Apps page). */
export {
  listAgentApps,
  checkAgentApp,
  planAgentApp,
  connectAgentApp,
  disconnectAgentApp,
  renewAgentApp,
  testAgentApp,
  listAgentActivity,
  onAgentActivity,
} from './agentApps';
export type {
  ActivityEvent,
  ActivityOutcome,
  AgentApp,
  AppCard,
  AppCheck,
  AppInstall,
  AppPlan,
  AppsOverview,
  ConnectRequest,
  LinkView,
  Registration,
  TokenUsage,
} from './generated/bindings';

export {
  runAssistant,
  cancelAssistant,
  previewAssistant,
  explainAssistant,
} from './assistant';
export type {
  AgentEvent,
  Trace,
  AssistantRequest,
  AssistantContext,
  AssistantContext_Deserialize,
  AssistantContext_Serialize,
  AssistantHistory,
  AssistantScope,
} from './generated/bindings';

export {
  listMcpServers,
  saveMcpServers,
  setMcpServerSecrets,
  deleteMcpServer,
  discoverMcpTools,
  confirmMcpTool,
  listProjectMcpServers,
  trustProjectMcpServer,
  revokeProjectMcpServer,
  enableProjectMcpTools,
  promptProjectMcpSecret,
} from './mcpClient';
export type {
  ProjectServer,
  ProjectServer_Deserialize,
  ProjectServer_Serialize,
  ProjectServerView,
  ProjectServerView_Deserialize,
  ProjectServerView_Serialize,
  ProjectTrustState,
  ServerConfig,
  McpClientTool,
  Endpoint,
  McpSecretInput,
  McpServerList,
} from './generated/bindings';

/** Opt-in scheduled read-only monitoring investigations (M45-12). */
export * from './investigations';
