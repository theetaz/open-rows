// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button, SelectField } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { saveReport } from '@openrows/ipc';
import { exportable, FORMATS, reportFile } from './formats';
import type { ReportFormat } from './formats';
import type { Report, ReportSection } from './model';
import styles from './Report.module.css';

type Choice = { value: string; label: string };
type ReportDialogProps = {
  /** Time ranges to report; omitted when the content fixes its own range. */
  ranges?: readonly Choice[];
  /** Sections the user can include; all are chosen at first. */
  parts: readonly Choice[];
  /** Build the report of a range and the chosen sections. */
  build: (range: string, parts: ReadonlySet<string>) => Promise<Report>;
  onClose: () => void;
};

const messageOf = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

function useReport(
  props: ReportDialogProps,
  range: string,
  parts: ReadonlySet<string>,
) {
  const [state, setState] = useState<{
    report: Report | null;
    error: string | null;
  }>({
    report: null,
    error: null,
  });
  const { build } = props;
  useEffect(() => {
    let live = true;
    build(range, parts).then(
      (report) => {
        if (live) setState({ report, error: null });
      },
      (failure: unknown) => {
        if (live) setState({ report: null, error: messageOf(failure) });
      },
    );
    return () => {
      live = false;
    };
  }, [build, range, parts]);
  return state;
}

function Parts(props: {
  parts: readonly Choice[];
  chosen: ReadonlySet<string>;
  onChange: (next: ReadonlySet<string>) => void;
}): React.JSX.Element {
  return (
    <fieldset className={styles.parts}>
      <legend>Sections</legend>
      {props.parts.map((part) => (
        <label key={part.value}>
          <input
            type="checkbox"
            checked={props.chosen.has(part.value)}
            onChange={(event) => {
              const next = new Set(props.chosen);
              if (event.target.checked) next.add(part.value);
              else next.delete(part.value);
              props.onChange(next);
            }}
          />
          {part.label}
        </label>
      ))}
    </fieldset>
  );
}

/** CSV, SVG and PNG export one section. */
function single(format: ReportFormat): boolean {
  return format === 'csv' || format === 'svg' || format === 'png';
}

/** The table or chart a single-section format exports. */
function ItemPicker(props: {
  format: ReportFormat;
  items: ReportSection[] | null;
  chosen: ReportSection | undefined;
  onChoose: (id: string) => void;
}): React.JSX.Element | null {
  const { format, items } = props;
  if (!single(format) || !items) return null;
  const noun = format === 'csv' ? 'table' : 'chart';
  if (!items.length)
    return <p className={styles.note}>The chosen sections have no {noun}.</p>;
  return (
    <SelectField
      label={format === 'csv' ? 'Table' : 'Chart'}
      controlSize="compact"
      value={props.chosen?.id ?? ''}
      options={items.map((section) => ({
        value: section.id,
        label: section.title,
      }))}
      onValueChange={props.onChoose}
    />
  );
}

function Failure(props: { message: string | null }): React.JSX.Element | null {
  if (!props.message) return null;
  return (
    <p className={styles.error} role="alert">
      {props.message}
    </p>
  );
}

/** Render the chosen format and save it; closes when a file was written. */
function useSave(
  report: Report | null,
  format: ReportFormat,
  item: ReportSection | undefined,
  onClose: () => void,
): { saving: boolean; failure: string | null; save: () => void } {
  const [saving, setSaving] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const save = (): void => {
    if (!report) return;
    setSaving(true);
    setFailure(null);
    const name = single(format)
      ? `${report.title} ${item?.title ?? ''}`
      : report.title;
    reportFile(report, format, item)
      .then((file) => saveReport(name, file))
      .then(
        (path) => {
          setSaving(false);
          if (path !== null) onClose();
        },
        (cause: unknown) => {
          setSaving(false);
          setFailure(messageOf(cause));
        },
      );
  };
  return { saving, failure, save };
}

/** Compact range/sections/format choice, then the native save panel. */
export function ReportDialog(props: ReportDialogProps): React.JSX.Element {
  const [range, setRange] = useState(props.ranges?.[0]?.value ?? '');
  const [parts, setParts] = useState<ReadonlySet<string>>(
    () => new Set(props.parts.map((part) => part.value)),
  );
  const [format, setFormat] = useState<ReportFormat>('html');
  const [itemId, setItemId] = useState('');
  const { report, error } = useReport(props, range, parts);
  const items = report ? exportable(format, report.sections) : [];
  const item = items.find((section) => section.id === itemId) ?? items[0];
  const { saving, failure, save } = useSave(
    report,
    format,
    item,
    props.onClose,
  );
  return (
    <Sheet title="Export report" onClose={props.onClose}>
      <div className={styles.form}>
        {props.ranges && (
          <SelectField
            label="Range"
            controlSize="compact"
            value={range}
            options={props.ranges}
            onValueChange={setRange}
          />
        )}
        <Parts parts={props.parts} chosen={parts} onChange={setParts} />
        <SelectField
          label="Format"
          controlSize="compact"
          value={format}
          options={FORMATS}
          onValueChange={setFormat}
        />
        <ItemPicker
          format={format}
          items={report ? items : null}
          chosen={item}
          onChoose={setItemId}
        />
        <p className={styles.note}>
          Gaps, unknown values and limits are listed with each section; they are
          never shown as zeros.
        </p>
        <Failure message={error ?? failure} />
      </div>
      <SheetActions>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button
          variant="primary"
          disabled={!report || saving || (single(format) && !item)}
          onClick={save}
        >
          Export…
        </Button>
      </SheetActions>
    </Sheet>
  );
}
