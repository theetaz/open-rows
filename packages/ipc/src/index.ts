// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type { AppInfo, SpikeReport, SpikeError } from './generated/bindings';

/** Application metadata returned by the Rust shell. */
export type { AppInfo };

/** Read application metadata through the generated command binding. */
export function getAppInfo(): Promise<AppInfo> {
  return commands.appInfo();
}

/** Binary request and typed rejection shape exported from Rust via Specta. */
export type {
  WindowRequest,
  ResultError,
  TileTiming,
  PageTimer,
  GridRun,
  GridFrame,
  EditorRun,
  KeystrokeSample,
} from './generated/bindings';

export { resultWindow } from './binary';

/** Connection profiles and sessions (D-05); secrets only travel to the core. */
export {
  CommandError,
  answerSshPrompt,
  attachDatabase,
  detachDatabase,
  closeConnection,
  closeSession,
  createProfile,
  deleteProfile,
  duplicateProfile,
  getProfile,
  listProfiles,
  onSessionState,
  onSshPrompt,
  openConnection,
  openSession,
  parseConnectionUrl,
  pickConnectionFile,
  revealSecret,
  searchProfiles,
  testConnection,
  updateProfile,
} from './connections';
/** Server process list with cancel and kill (FR-ADMIN-01). */
export { listProcesses, signalProcess } from './processes';
export type {
  ProcessDto,
  ProcessSignalDto,
  ProcessSignalRequest,
} from './generated/bindings';
/** Native alerts for destructive confirmations (scoped dialog plugin). */
export { confirmDestructive, pickFileValue } from './dialog';
export type { DestructivePrompt } from './dialog';
/** Schema tree catalog reads through the schema cache (D-06, M1-16). */
export {
  checkCatalog,
  invalidateCatalog,
  listDatabases,
  listObjects,
  listSchemas,
  onCatalogChanged,
  searchCatalog,
} from './catalog';
export type {
  CatalogChanged,
  CatalogReply,
  CatalogSearchReply,
  CatalogSearchRequest,
  ObjectsReply,
} from './generated/bindings';
/** Query execution, cancellation, result lifetime and history (D-07, FR-WS-01/02). */
export {
  cancelQuery,
  clearHistory,
  deleteHistory,
  executeQuery,
  releaseResults,
  searchHistory,
} from './queries';
/** Explain and explain-analyze plans with hotspots (M2-06, FR-EDIT-07). */
export { explainQuery } from './explain';
export type {
  ExplainReply,
  ExplainRequest,
  PlanBuffersDto,
  PlanDetailDto,
  PlanDto,
  PlanHotspotDto,
  PlanNodeDto,
  PlanRawFormatDto,
  PlanWeightDto,
} from './generated/bindings';
/** Exact values of single result cells for the value inspector (M1-20). */
export { resultCellInfo, resultCellValue } from './cells';
export type {
  CellInfoDto,
  CellKindDto,
  CellRequest,
  HistorySearchRequest,
} from './generated/bindings';
/** Table views: structure, parameterised pages, exact counts, SQL preview (D-08). */
export {
  tableCount,
  tablePage,
  tableSql,
  tableStructure,
  tableEditInfo,
  tableEditRow,
  tableEditRows,
  tableEdit,
  tableDetail,
  tableAlter,
  saveTableEdits,
  savedTableEdits,
  clearTableEdits,
  journaledTableEdits,
} from './tables';
export type { JournaledEditsDto } from './generated/bindings';
/** Streamed export of results, rows, tables and schemas as background jobs (M1-25, M2-13). */
export { openExport, revealExport, startExport } from './exports';
export type {
  CsvDelimiterDto,
  CsvQuoteDto,
  DelimitedDto,
  ExportEventDto,
  ExportFormatDto,
  ExportReply,
  ExportRequest,
  ExportSourceDto,
  LineEndingDto,
  ParquetCompressionDto,
  TextEncodingDto,
} from './generated/bindings';
/** Running `.sql` files too large for the editor (M2-12). */
export { checkRunFile, pickRunFile, startRunFile } from './runFile';
export type {
  FlaggedStatementDto,
  PickedScriptDto,
  RunFileCheckDto,
  RunFileEventDto,
  RunFileGateDto,
  RunFileOutcomeDto,
} from './generated/bindings';
/** Backup and restore with the installed client tools (M2-14). */
export {
  cancelDump,
  checkRestore,
  dumpContext,
  dumpTools,
  pickBackupFile,
  pickRestoreFile,
  previewBackup,
  restoreContents,
  startBackup,
  startRestore,
} from './dump';
export type {
  ArchiveEntryDto,
  BackupContentDto,
  BackupFormatDto,
  BackupOptionsDto,
  BackupRequest,
  DumpContextDto,
  DumpEngineDto,
  DumpEventDto,
  DumpFileKindDto,
  DumpOutcomeDto,
  DumpPreviewDto,
  DumpScopeRequest,
  DumpToolDto,
  DumpToolKindDto,
  PickedDumpDto,
  RestoreCheckDto,
  RestoreCleanDto,
  RestoreOptionsDto,
  RestoreRequest,
  RestoreTargetDto,
} from './generated/bindings';
/** Import of CSV, TSV, JSON and NDJSON files into tables (M2-11). */
export {
  cancelImport,
  pickImportFile,
  previewImport,
  startImport,
} from './imports';
export type {
  BadRowsDto,
  ConflictDto,
  ImportColumnDto,
  ImportEventDto,
  ImportFormatDto,
  ImportMappingDto,
  ImportOptionsDto,
  ImportPreviewDto,
  ImportPreviewRequest,
  ImportRequest,
  ImportSummaryDto,
  RowErrorDto,
  TransactionDto,
  TypeCategoryDto,
} from './generated/bindings';
/** The read-only ER diagram of a schema and its SVG/PNG export (M2-10). */
export { saveDiagram, schemaDiagram } from './diagram';
export type {
  DiagramColumnDto,
  DiagramFileDto,
  DiagramForeignKeyDto,
  DiagramTableDto,
  DiagramTarget,
  SchemaDiagramDto,
} from './generated/bindings';
/** settings.json and the native menu bar (M1-26, M1-27). */
export {
  onMenuCommand,
  onSettingsFileChanged,
  readSettingsFile,
  syncMenu,
  writeSettingsFile,
} from './app';
export type { MenuItemState } from './generated/bindings';
/** Diagnostics: crash status, the logs folder and webview reports (M1-28). */
export { crashRecoveryStatus, openLogsFolder, reportUiEvent } from './app';
export type { UiReport, UiReportKind } from './generated/bindings';
/** Native window closing, guarded by the webview (staged edits, running commits). */
export { onWindowClose, closeWindow } from './window';
export type {
  CellValueDto,
  EditInfoDto,
  EditRequest,
  FileEncodingDto,
  EditReply,
  RowEditDto,
  SavedEditsDto,
  StagedRowDto,
  CombinatorDto,
  CountReply,
  FilterDto,
  FilterOperatorDto,
  PageRequest,
  SortDto,
  TableColumnDto,
  TableIndexDto,
  TableQueryDto,
  TableStructureDto,
  AlterReply,
  AlterRequest,
  DefaultDto,
  ForeignKeyDto,
  GeneratedDto,
  IndexDetailDto,
  IndexMethodDto,
  PartitionDto,
  RelationKindDto,
  SchemaChangeDto,
  StructureColumnDto,
  TableDetailDto,
  TableInfoDto,
  TriggerDto,
  TriggerTimingDto,
} from './generated/bindings';
/** Structure previews and object lifecycle editors (M2-07, M2-08). */
export { objectChange, routineDefinition, tableAlterPreview } from './objects';
export type {
  AlterPreviewDto,
  ConstraintDto,
  ConstraintKindDto,
  ContainerChangeDto,
  DefinitionChangeDto,
  ForeignTargetDto,
  IdentityDto,
  NewCheckDto,
  NewColumnDto,
  NewForeignKeyDto,
  NewIndexDto,
  NewTableDto,
  ObjectChangeDto,
  ObjectFeaturesDto,
  ObjectNameDto,
  ObjectReply,
  ObjectRequest,
  ReferentialActionDto,
  RelationChangeDto,
  RoutineKindDto,
} from './generated/bindings';
/** Saved queries and the `.sql` files of query tabs (native panels). */
export {
  createSavedQuery,
  deleteSavedQuery,
  listSavedQueries,
  openSqlFile,
  readSqlFile,
  renameSavedQuery,
  saveSqlFile,
  saveSqlFileAs,
  sqlFileModified,
} from './savedQueries';
/** Projects: folders of `.sql` queries with `.openrows/project.yaml` (M2-17). */
export {
  chooseProject,
  createProjectFolder,
  createProjectQuery,
  deleteProjectItem,
  duplicateProjectItem,
  loadProject,
  moveProjectItem,
  renameProjectItem,
  revealProjectItem,
  setProjectConnection,
} from './projects';
export type {
  ProjectDto,
  ProjectEntryDto,
  ProjectEntryKind,
} from './generated/bindings';
/** Session and workspace state of the window in the local store (M1-21, M1-26). */
export { readWorkspaceState, writeWorkspaceState } from './workspace';
export type { WorkspacePartDto } from './generated/bindings';
export type {
  ColumnDto,
  ExecuteReply,
  ExecuteRequest,
  HistoryItemDto,
  ParamStyleDto,
  ParamsDto,
  ParamValueDto,
  QueryEventDto,
  RunMode,
  SavedQueryDto,
  SqlFileDto,
  StatementDto,
} from './generated/bindings';
export { DEFAULT_CUSTOM_ENV_COLOR } from './generated/bindings';
export type {
  AttachedFileDto,
  AttachReply,
  CertificateDto,
  CloudCaDto,
  ConnectionOpened,
  DatabaseDto,
  InsecureReasonDto,
  LatencyDto,
  LimitsDto,
  ProxyDto,
  ProxyInput,
  ProxyKindDto,
  EngineDto,
  FileDto,
  EnvDto,
  EnvKindDto,
  IpcErrorKind,
  ObjectDto,
  ObjectKindDto,
  PasswordInput,
  PasswordStorageDto,
  HelperHop,
  ProfileDto,
  ProfileInput,
  ProfileList,
  SafetyDto,
  SchemaDto,
  ServerDto,
  SessionStateChanged,
  PickFileKind,
  SecretFieldDto,
  SecretInput,
  SshAuthDto,
  SshDto,
  SshInput,
  SshPromptAnswer,
  SshPromptChanged,
  SshPromptDto,
  StepStatus,
  TestReport,
  TestStepDto,
  TestStepKind,
  TlsFilesDto,
  TlsFilesInput,
  TlsModeDto,
  UrlDraft,
} from './generated/bindings';

/** Atomically persist benchmark samples and exit; available only in spike mode. */
export async function finishSpike(report: SpikeReport): Promise<void> {
  const result = await commands.spikeFinish(report);
  if (result.status === 'error') throw spikeFailure(result.error);
}

/** Query spike mode and show the native window before the worker starts. */
export async function startSpike(): Promise<false | 'ipc' | 'grid' | 'editor'> {
  if (!isTauri()) return false;
  const result = await commands.spikeStart();
  if (result.status === 'error') throw spikeFailure(result.error);
  if (result.data === null) return false;
  if (
    result.data === 'ipc' ||
    result.data === 'grid' ||
    result.data === 'editor'
  )
    return result.data;
  throw new Error('Unknown spike mode');
}

/** Append one diagnostic line to the benchmark progress channel. */
export async function spikeProgress(
  stage: string,
  detail: string,
): Promise<void> {
  const result = await commands.spikeProgress(stage, detail);
  if (result.status === 'error') throw spikeFailure(result.error);
}

function spikeFailure(error: SpikeError): Error {
  switch (error.kind) {
    case 'failed':
      return new Error(error.message);
    case 'disabled':
      return new Error('IPC spike mode is disabled');
    case 'missingPath':
      return new Error('OPENROWS_SPIKE_REPORT is required');
  }
}

/** Non-secret connection portability with preview-first imports. */
export {
  exportConnections,
  previewConnections,
  importConnections,
  cancelConnectionImport,
} from './connectionExchange';
export type {
  ConnectionConflictDto,
  ImportSource,
  ImportPreview,
  ImportRow,
  ImportDecisionDto,
  ImportChoiceDto,
  CredentialCommandDto,
} from './generated/bindings';
/** Explicitly trusted credential commands. */
export { testCredential, cancelCredential } from './credentials';
/** Read-only object and account catalog viewers (M2-09). */
export {
  catalogCapabilities,
  inspectObject,
  listExtensions,
  listRoles,
  roleGrants,
} from './inspection';
export type {
  AttributeDto,
  CatalogFeaturesDto,
  DetailSectionDto,
  ExtensionDto,
  GrantDto,
  ObjectDetailDto,
  ObjectIdentityDto,
  RoleDto,
  RoleIdentityDto,
} from './generated/bindings';
