// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type {
  InvestigationAudit,
  ScheduleRequest,
  ScheduledJob,
} from './generated/bindings';
import { unwrap } from './connections';
/** Explicit session opt-in; native code validates sharing and finite budgets. */
export async function scheduleInvestigation(
  request: ScheduleRequest,
): Promise<void> {
  await unwrap(commands.investigationSchedule(request));
}
/** Cancel a scheduled job and its current provider/read work. */
export async function cancelInvestigationSchedule(id: string): Promise<void> {
  await unwrap(commands.investigationScheduleCancel(id));
}
/** Pause stops the in-flight run and keeps the job; resume lets the next tick run. */
export async function pauseInvestigationSchedule(
  id: string,
  paused: boolean,
): Promise<void> {
  await unwrap(commands.investigationSchedulePause(id, paused));
}
/** Start the next run now; pauses, lock and AI off still apply. */
export async function runInvestigationSchedule(id: string): Promise<void> {
  await unwrap(commands.investigationScheduleRun(id));
}
/** In-app results and conservative accounting; contents are hidden while locked. */
export function listInvestigationSchedules(): Promise<ScheduledJob[]> {
  return unwrap(commands.investigationSchedules());
}

export type {
  JobState,
  PauseReason,
  Rejection,
  InvestigationSchedule,
  InvestigationAudit,
  ScheduleRequest,
  ScheduledJob,
  ScheduledResult,
  ScheduledResult_Deserialize,
  ScheduledResult_Serialize,
} from './generated/bindings';

/** Persistent scrubbed usage/outcome records, hidden while locked. */
export function investigationAudit(): Promise<InvestigationAudit[]> {
  return unwrap(commands.investigationAudit());
}
