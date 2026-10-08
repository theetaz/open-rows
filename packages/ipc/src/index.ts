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
  closeConnection,
  closeSession,
  createProfile,
  deleteProfile,
  duplicateProfile,
  getProfile,
  listProfiles,
  onSessionState,
  openConnection,
  openSession,
  parsePostgresUrl,
  searchProfiles,
  testConnection,
  updateProfile,
} from './connections';
/** Native alerts for destructive confirmations (scoped dialog plugin). */
export { confirmDestructive, pickFileValue } from './dialog';
export type { DestructivePrompt } from './dialog';
/** Schema tree catalog reads on per-database metadata sessions (D-06). */
export {
  listDatabases,
  listObjects,
  listSchemas,
  searchCatalog,
} from './catalog';
/** Query execution, cancellation, result lifetime and history (D-07). */
export {
  cancelQuery,
  executeQuery,
  recentHistory,
  releaseResults,
} from './queries';
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
} from './tables';
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
export type {
  ColumnDto,
  ExecuteReply,
  ExecuteRequest,
  HistoryItemDto,
  QueryEventDto,
  RunMode,
  StatementDto,
} from './generated/bindings';
export { DEFAULT_CUSTOM_ENV_COLOR } from './generated/bindings';
export type {
  ConnectionOpened,
  DatabaseDto,
  EnvDto,
  EnvKindDto,
  IpcErrorKind,
  ObjectDto,
  ObjectKindDto,
  PasswordInput,
  ProfileDto,
  ProfileInput,
  ProfileList,
  SafetyDto,
  SchemaDto,
  ServerDto,
  SessionStateChanged,
  TestReport,
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
