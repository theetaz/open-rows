// SPDX-License-Identifier: Apache-2.0
import { Channel, isTauri } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type {
  ArchiveEntryDto,
  BackupPickRequest,
  BackupRequest,
  DumpContextDto,
  DumpEventDto,
  DumpPreviewDto,
  DumpScopeRequest,
  DumpStarted,
  DumpToolDto,
  PickedDumpDto,
  RestoreCheckDto,
  RestoreContentsRequest,
  RestoreRequest,
} from './generated/bindings';
import { unwrap } from './connections';

/**
 * Every client tool: the copy found or set in Settings, and its version.
 * Outside the native shell there are none.
 */
export async function dumpTools(): Promise<DumpToolDto[]> {
  if (!isTauri()) return [];
  return unwrap(commands.dumpTools());
}

/** Engine, safety switches and tools of a session's database. */
export function dumpContext(
  request: DumpScopeRequest,
): Promise<DumpContextDto> {
  return unwrap(commands.dumpContext(request));
}

/** The exact command a backup would run (password file redacted). */
export function previewBackup(request: BackupRequest): Promise<DumpPreviewDto> {
  return unwrap(commands.dumpBackupPreview(request));
}

/**
 * Choose where a backup goes in the native save panel. Resolves `null`
 * when cancelled or outside the native shell.
 */
export async function pickBackupFile(
  request: BackupPickRequest,
): Promise<string | null> {
  if (!isTauri()) return null;
  return unwrap(commands.dumpBackupPick(request));
}

/** Start a backup; events end with `finished`. Cancel with `cancelDump`. */
export function startBackup(
  request: BackupRequest,
  onEvent: (event: DumpEventDto) => void,
): Promise<DumpStarted> {
  const channel = new Channel<DumpEventDto>();
  channel.onmessage = onEvent;
  return unwrap(commands.dumpBackupStart(request, channel));
}

/** Stop a running backup or restore. */
export async function cancelDump(jobId: string): Promise<void> {
  await unwrap(commands.dumpCancel(jobId));
}

/**
 * Choose a backup file (or a `pg_dump` folder) in the native open panel.
 * Resolves `null` when cancelled or outside the native shell.
 */
export async function pickRestoreFile(
  folder: boolean,
): Promise<PickedDumpDto | null> {
  if (!isTauri()) return null;
  return unwrap(commands.dumpRestorePick(folder));
}

/** The table of contents of a picked `pg_dump` archive. */
export function restoreContents(
  request: RestoreContentsRequest,
): Promise<ArchiveEntryDto[]> {
  return unwrap(commands.dumpRestoreContents(request));
}

/** Commands, warnings and the confirmation a restore needs; nothing runs. */
export function checkRestore(
  request: RestoreRequest,
): Promise<RestoreCheckDto> {
  return unwrap(commands.dumpRestoreCheck(request));
}

/** Start a confirmed restore; events end with `finished`. */
export function startRestore(
  request: RestoreRequest,
  onEvent: (event: DumpEventDto) => void,
): Promise<DumpStarted> {
  const channel = new Channel<DumpEventDto>();
  channel.onmessage = onEvent;
  return unwrap(commands.dumpRestoreStart(request, channel));
}
