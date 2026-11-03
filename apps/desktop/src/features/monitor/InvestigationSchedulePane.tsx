// SPDX-License-Identifier: Apache-2.0
import { Button, SelectField, TextField } from '@openrows/ui';
import type { InvestigationSchedule, ProfileDto } from '@openrows/ipc';
import {
  useInvestigationSchedule,
  investigationTools,
} from './useInvestigationSchedule';
import { ScheduleResults } from './ScheduleResults';
import styles from './Monitor.module.css';
import { t } from './messages/lazyInvestigation';
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
      <summary>{t('scheduledReadOnly')}</summary>
      <SelectField
        label={t('scheduledProvider')}
        value={config.provider}
        options={[
          { value: '', label: t('chooseProvider') },
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
        label={t('scheduledModel')}
        placeholder={t('modelId')}
        value={config.model}
        onChange={(event) => {
          setConfig({ ...config, model: event.target.value });
        }}
      />
      <Limits config={config} change={setConfig} />
      <ToolSelection config={config} change={setConfig} />
      <p className={styles.note}>{t('runsUseOnly')}</p>
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
        {t('enableScheduledInvestigation')}
      </Button>
      <Button
        onClick={() => {
          void refresh().catch(() => {
            setStatus(t('resultsUnavailable'));
          });
        }}
      >
        {t('refreshScheduledResults')}
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
    { key: 'intervalSecs', label: t('intervalSeconds300') },
    { key: 'maxRuns', label: t('maximumRuns1') },
    { key: 'steps', label: t('steps112') },
    { key: 'tokens', label: t('tokens165536') },
    { key: 'timeoutSecs', label: t('deadlineSeconds1') },
    { key: 'maxCostMicros', label: t('maximumCostMillionths') },
    {
      key: 'pricePerMillionMicros',
      label: t('maximumPriceMillionths'),
    },
  ] as const;
  return (
    <fieldset>
      <legend>{t('investigationLimits')}</legend>
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
      <legend>{t('permittedReadPropose')}</legend>
      {investigationTools.map((tool) => (
        <label key={tool} className={styles.tool}>
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
