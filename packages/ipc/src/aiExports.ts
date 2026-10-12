// SPDX-License-Identifier: Apache-2.0
/** API-key/local provider settings and metadata-only verification (M45-01). */
export {
  setAiKey,
  deleteAiKey,
  verifyAiProvider,
  cancelAiVerification,
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
} from './mcpClient';
export type {
  ServerConfig,
  McpClientTool,
  Endpoint,
  McpSecretInput,
} from './generated/bindings';
