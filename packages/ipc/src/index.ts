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
/** Native alerts for destructive confirmations (scoped dialog plugin). */
export { confirmDestructive, pickFileValue } from './dialog';
export type { DestructivePrompt } from './dialog';
/** Schema tree catalog reads through the schema cache (D-06, M1-16). */
export {
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
/** Streamed export of results and tables to CSV, JSON and SQL files (M1-25). */
export { startExport } from './exports';
export type {
  CsvDelimiterDto,
  ExportEventDto,
  ExportFormatDto,
  ExportReply,
  ExportRequest,
  ExportSourceDto,
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
  RelationKindDto,
  SchemaChangeDto,
  StructureColumnDto,
  TableDetailDto,
  TableInfoDto,
  TriggerDto,
  TriggerTimingDto,
} from './generated/bindings';
/** Saved queries and the `.sql` files of query tabs (native panels). */
export {
  createSavedQuery,
  deleteSavedQuery,
  listSavedQueries,
  openSqlFile,
  renameSavedQuery,
  saveSqlFile,
  saveSqlFileAs,
} from './savedQueries';
export type {
  ColumnDto,
  ExecuteReply,
  ExecuteRequest,
  HistoryItemDto,
  ParamStyleDto,
  ParamsDto,
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
