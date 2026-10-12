// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SelectField, TextArea, TextField } from '@openrows/ui';
import type { ChartMapping, Widget, WidgetKind } from '@openrows/ipc';
import styles from './Boards.module.css';

const KINDS = [
  { value: 'chart', label: 'Chart' },
  { value: 'stat', label: 'Stat card' },
  { value: 'table', label: 'Table' },
] as const;
const MARKS = [
  { value: 'line', label: 'Line' },
  { value: 'area', label: 'Area' },
  { value: 'bar', label: 'Bar' },
  { value: 'scatter', label: 'Scatter' },
] as const;

/** A new chart widget with default limits and a unique id. */
export function newWidget(taken: readonly Widget[]): Widget {
  let index = taken.length + 1;
  while (taken.some((widget) => widget.id === `w${String(index)}`)) index += 1;
  return {
    id: `w${String(index)}`,
    title: `Widget ${String(index)}`,
    kind: 'chart',
    sql: '',
    chart: { mark: 'line', x: '', y: [] },
    limits: { maxRows: 1_000, timeoutMs: 15_000, maxBytes: 1_048_576 },
  };
}

function columns(text: string): string[] {
  return text
    .split(',')
    .map((name) => name.trim())
    .filter(Boolean);
}

/** Title, presentation, one read-only statement, chart mapping and limits. */
export function WidgetEditor(props: {
  widget: Widget;
  onSave: (widget: Widget) => void;
  onCancel: () => void;
}): React.JSX.Element {
  const [draft, setDraft] = useState(props.widget);
  const [series, setSeries] = useState(props.widget.chart?.y.join(', ') ?? '');
  const chart: ChartMapping = draft.chart ?? { mark: 'line', x: '', y: [] };
  const setKind = (kind: WidgetKind): void => {
    setDraft({ ...draft, kind, chart: kind === 'chart' ? chart : null });
  };
  const save = (): void => {
    props.onSave(
      draft.kind === 'chart'
        ? { ...draft, chart: { ...chart, y: columns(series) } }
        : { ...draft, chart: null },
    );
  };
  return (
    <section className={styles.editor} aria-label="Edit widget">
      <div className={styles.row}>
        <TextField
          label="Title"
          controlSize="compact"
          value={draft.title}
          onChange={(event) => {
            setDraft({ ...draft, title: event.target.value });
          }}
        />
        <SelectField
          label="Show as"
          controlSize="compact"
          value={draft.kind}
          options={KINDS}
          onValueChange={setKind}
        />
      </div>
      <TextArea
        aria-label="Widget SQL"
        placeholder="SELECT day, count(*) AS orders FROM orders WHERE region = :region GROUP BY day"
        spellCheck={false}
        value={draft.sql}
        onChange={(event) => {
          setDraft({ ...draft, sql: event.target.value });
        }}
      />
      <p className={styles.note}>
        One read-only query. Reference board parameters as :name; values are
        bound, never inserted into the SQL.
      </p>
      {draft.kind === 'chart' && (
        <ChartFields
          chart={chart}
          series={series}
          onChart={(next) => {
            setDraft({ ...draft, chart: next });
          }}
          onSeries={setSeries}
        />
      )}
      <Limits draft={draft} onChange={setDraft} />
      <div className={styles.row}>
        <Button size="sm" variant="primary" onClick={save}>
          Save widget
        </Button>
        <Button size="sm" onClick={props.onCancel}>
          Cancel
        </Button>
      </div>
    </section>
  );
}

function ChartFields(props: {
  chart: ChartMapping;
  series: string;
  onChart: (chart: ChartMapping) => void;
  onSeries: (series: string) => void;
}): React.JSX.Element {
  const { chart } = props;
  return (
    <div className={styles.row}>
      <SelectField
        label="Mark"
        controlSize="compact"
        value={chart.mark}
        options={MARKS}
        onValueChange={(mark) => {
          props.onChart({ ...chart, mark });
        }}
      />
      <TextField
        label="X column"
        controlSize="compact"
        value={chart.x}
        onChange={(event) => {
          props.onChart({ ...chart, x: event.target.value });
        }}
      />
      <TextField
        label="Y columns"
        controlSize="compact"
        placeholder="orders, revenue"
        value={props.series}
        onChange={(event) => {
          props.onSeries(event.target.value);
        }}
      />
    </div>
  );
}

function Limits(props: {
  draft: Widget;
  onChange: (widget: Widget) => void;
}): React.JSX.Element {
  const { draft } = props;
  const limit = (key: 'maxRows' | 'timeoutMs', value: string): void => {
    const number = Math.round(Number(value) || 0);
    props.onChange({
      ...draft,
      limits: {
        ...draft.limits,
        [key]: key === 'timeoutMs' ? number * 1_000 : number,
      },
    });
  };
  return (
    <div className={styles.row}>
      <TextField
        label="Max rows (≤ 10,000)"
        controlSize="compact"
        inputMode="numeric"
        value={String(draft.limits.maxRows)}
        onChange={(event) => {
          limit('maxRows', event.target.value);
        }}
      />
      <TextField
        label="Time limit, s (1–60)"
        controlSize="compact"
        inputMode="numeric"
        value={String(draft.limits.timeoutMs / 1_000)}
        onChange={(event) => {
          limit('timeoutMs', event.target.value);
        }}
      />
    </div>
  );
}
