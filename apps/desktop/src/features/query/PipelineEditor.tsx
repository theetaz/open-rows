// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Icon, TextArea, TextField } from '@openrows/ui';
import { usePipelineEditor } from './usePipelineEditor';
import styles from './PipelineEditor.module.css';
import { t } from './lazyMessages';
type Draft = ReturnType<typeof usePipelineEditor>;

/**
 * Aggregation pipeline editor (M3-04). Stage previews and Run go through the
 * tab's run path: the same safety admission, cancellation and result pane as
 * MQL typed in the editor.
 */
export function PipelineEditor({
  run,
  disabled,
}: {
  run: (source: string) => void;
  disabled: boolean;
}): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const draft = usePipelineEditor(run);
  return (
    <section className={styles.pipeline} aria-label={t('aggregationPipeline')}>
      <button
        type="button"
        className={styles.header}
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        <Icon name={open ? 'chevronDown' : 'chevronRight'} />
        {t('pipelineEditor')}
      </button>
      {open && (
        <div className={styles.body}>
          <PipelineFields draft={draft} />
          <StagePreviews draft={draft} disabled={disabled} />
          {draft.error && (
            <p className={styles.error} role="alert">
              {draft.error}
            </p>
          )}
        </div>
      )}
    </section>
  );
}

function PipelineFields({ draft }: { draft: Draft }): React.JSX.Element {
  return (
    <div className={styles.fields}>
      <TextField
        label={t('collection')}
        controlSize="compact"
        className={styles.collection}
        value={draft.collection}
        onChange={(event) => {
          draft.setCollection(event.target.value);
        }}
      />
      <TextArea
        aria-label={t('pipelineJson')}
        className={styles.stages}
        spellCheck={false}
        value={draft.text}
        onChange={(event) => {
          draft.setText(event.target.value);
        }}
      />
    </div>
  );
}

/** One Preview per stage (its prefix, sampled) and Run for the whole pipeline. */
function StagePreviews({
  draft,
  disabled,
}: {
  draft: Draft;
  disabled: boolean;
}): React.JSX.Element {
  return (
    <div
      className={styles.previews}
      role="toolbar"
      aria-label={t('stagePreviews')}
    >
      {draft.stages.map((stage, index) => (
        <Button
          size="sm"
          key={index}
          aria-label={t('previewStageValue', { value: String(index + 1) })}
          title={t('runStages1', { value: String(index + 1) })}
          disabled={disabled}
          onClick={() => {
            draft.execute(index);
          }}
        >
          {String(index + 1)} · {stage}
        </Button>
      ))}
      <Button
        size="sm"
        variant="primary"
        className={styles.run}
        disabled={disabled}
        onClick={() => {
          draft.execute();
        }}
      >
        {t('runPipeline')}
      </Button>
    </div>
  );
}
