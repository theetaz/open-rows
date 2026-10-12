// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
  listInvestigationSchedules,
  scheduleInvestigation,
} from '@openrows/ipc';
import type {
  InvestigationSchedule,
  ProfileDto,
  ScheduledJob,
} from '@openrows/ipc';
import { aiSettings } from '../settings/ai/aiSettings';
import { assistantSchema } from '../assistant/context';
/** Static read/propose tools eligible for explicit scheduled selection. */
export const investigationTools = [
  'read_health_snapshot',
  'read_metric_history',
  'read_query_statistics',
  'read_lock_graph',
  'read_monitor_plans',
  'record_health_finding',
  'build_report',
  'propose_board',
];
const initial = {
  enabled: false,
  provider: '',
  model: '',
  intervalSecs: 900,
  maxRuns: 4,
  steps: 12,
  tokens: 32768,
  timeoutSecs: 60,
  maxCostMicros: 1000000,
  pricePerMillionMicros: 0,
  tools: investigationTools,
};
type ScheduleControls = {
  settings: ReturnType<typeof aiSettings.useValue>;
  config: InvestigationSchedule;
  setConfig: (value: InvestigationSchedule) => void;
  jobs: ScheduledJob[];
  status: string;
  setStatus: (value: string) => void;
  refresh: () => Promise<void>;
  enable: () => Promise<void>;
};

/** Session opt-in state and explicit native scheduling actions. */
export function useInvestigationSchedule(props: {
  profile: ProfileDto;
  database: string;
}): ScheduleControls {
  const settings = aiSettings.useValue();
  const [config, setConfig] = useState<InvestigationSchedule>(initial);
  const [id] = useState(() => crypto.randomUUID());
  const [jobs, setJobs] = useState<ScheduledJob[]>([]);
  const [status, setStatus] = useState('');
  const refresh = async (): Promise<void> => {
    setJobs(await listInvestigationSchedules());
  };
  const enable = async (): Promise<void> => {
    const request = {
      id,
      provider: config.provider,
      model: config.model,
      connection: props.profile.id,
      database: props.database,
      schema: assistantSchema(props.profile.engine, props.database),
      monitoring: true,
      query: null,
      plan: [],
      question: {
        question:
          'Investigate health and regressions. Record grounded findings with uncertainty and evidence ids. Propose a report if useful.',
        history: [],
        errorCode: null,
      },
    };
    try {
      await scheduleInvestigation({
        request,
        config: { ...config, enabled: true },
      });
      setStatus(
        'Enabled for this app session; first run follows the selected interval. Lock or disconnect disables it.',
      );
      await refresh();
    } catch {
      setStatus(
        'Schedule refused. Check provider, telemetry policy, connection and budgets.',
      );
    }
  };
  return {
    settings,
    config,
    setConfig,
    jobs,
    status,
    setStatus,
    refresh,
    enable,
  };
}
