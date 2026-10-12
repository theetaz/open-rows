// SPDX-License-Identifier: Apache-2.0
import { Button, SelectField, TextField } from '@openrows/ui';
import type { InvestigationSchedule, ProfileDto } from '@openrows/ipc';
import {
  useInvestigationSchedule,
  investigationTools,
} from './useInvestigationSchedule';
import { ScheduleResults } from './ScheduleResults';
/** Explicit provider, tool, recurrence and spend controls; schedules default off. */
export function InvestigationSchedulePane(props: {
  profile: ProfileDto;
  database: string;
}): React.JSX.Element {
  const controls = useInvestigationSchedule(props);
  const { settings, config, setConfig, status, setStatus, refresh, enable } =
    controls;
  return (
    <details>
      <summary>Scheduled read-only investigations (off by default)</summary>
      <SelectField
        label="Scheduled provider"
        value={config.provider}
        options={[
          { value: '', label: 'Choose provider' },
          ...settings.providers.map((provider) => ({
            value: provider.id,
            label: provider.name,
          })),
        ]}
        onValueChange={(provider) => {
          setConfig({ ...config, provider, model: '' });
        }}
      />
      <TextField
        label="Scheduled model"
        value={config.model}
        onChange={(event) => {
          setConfig({ ...config, model: event.target.value });
        }}
      />
      <Limits config={config} change={setConfig} />
      <ToolSelection config={config} change={setConfig} />
      <p>
        Cost is a conservative estimate from your maximum input/output price.
        Enter millionths of the provider currency. Unknown pricing cannot enable
        a schedule. Results stay in app memory; scrubbed accounting is retained
        locally.
      </p>
      <Button
        disabled={
          !settings.enabled ||
          !config.provider ||
          !config.model ||
          config.pricePerMillionMicros <= 0
        }
        onClick={() => {
          void enable();
        }}
      >
        Enable scheduled investigation
      </Button>
      <Button
        onClick={() => {
          void refresh().catch(() => {
            setStatus('Results unavailable.');
          });
        }}
      >
        Refresh scheduled results
      </Button>
      <p role="status">{status}</p>
      <ScheduleResults
        {...props}
        jobs={controls.jobs}
        refresh={refresh}
        setStatus={setStatus}
      />
    </details>
  );
}
function Limits(props: {
  config: InvestigationSchedule;
  change: (config: InvestigationSchedule) => void;
}): React.JSX.Element {
  const fields = [
    { key: 'intervalSecs', label: 'Interval seconds (300–86400)' },
    { key: 'maxRuns', label: 'Maximum runs (1–1000)' },
    { key: 'steps', label: 'Steps (1–12)' },
    { key: 'tokens', label: 'Tokens (1–65536)' },
    { key: 'timeoutSecs', label: 'Deadline seconds (1–60)' },
    { key: 'maxCostMicros', label: 'Maximum cost millionths per run' },
    {
      key: 'pricePerMillionMicros',
      label: 'Maximum price millionths per million tokens',
    },
  ] as const;
  return (
    <fieldset>
      <legend>Investigation limits</legend>
      {fields.map((field) => (
        <TextField
          key={field.key}
          label={field.label}
          type="number"
          value={String(props.config[field.key])}
          onChange={(event) => {
            props.change({
              ...props.config,
              [field.key]: Number(event.target.value),
            });
          }}
        />
      ))}
    </fieldset>
  );
}

function ToolSelection(props: {
  config: InvestigationSchedule;
  change: (value: InvestigationSchedule) => void;
}): React.JSX.Element {
  return (
    <fieldset>
      <legend>Permitted read/propose tools</legend>
      {investigationTools.map((tool) => (
        <label key={tool}>
          <input
            type="checkbox"
            checked={props.config.tools.includes(tool)}
            onChange={(event) => {
              props.change({
                ...props.config,
                tools: event.target.checked
                  ? [...props.config.tools, tool]
                  : props.config.tools.filter((item) => item !== tool),
              });
            }}
          />
          {tool}
        </label>
      ))}
    </fieldset>
  );
}
