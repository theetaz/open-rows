// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { Button } from '@openrows/ui';
import { getProfile } from '@openrows/ipc';
import type { EngineDto, ProfileDto } from '@openrows/ipc';
import { DetailsStep } from './DetailsStep';
import { EngineStep } from './EngineStep';
import { FlowFooter } from './FlowFooter';
import { FlowRail } from './FlowRail';
import { derivedName } from './flowDraft';
import { OptionsStep } from './OptionsStep';
import { SecurityStep } from './SecurityStep';
import { SummaryColumn } from './SummaryColumn';
import { TestStep } from './TestStep';
import { useFlowController } from './useFlowController';
import type { FlowController } from './useFlowController';
import styles from './Flow.module.css';
import { t } from './messages';

type NewConnectionFlowProps = {
  mode: 'new' | 'edit';
  profileId?: string;
  onDone: (profileId: string) => void;
  onCancel: () => void;
  /** A new connection of this engine starts at Details (a file opened from Finder). */
  engine?: EngineDto | undefined;
  /** The database file of a new file connection. */
  filePath?: string | undefined;
  /** Opened to change the safety policy: start at Options with it focused. */
  focus?: 'safety' | undefined;
  /** Offer "Save and Connect" on the last step. */
  onSaveAndConnect?: ((profileId: string) => void) | undefined;
};

type Loaded =
  | { state: 'loading' }
  | { state: 'ready'; profile: ProfileDto | undefined }
  | { state: 'failed'; reason: string };

/** The saved profile of an edit, read once; a new connection needs none. */
function useEditedProfile(mode: 'new' | 'edit', id?: string): Loaded {
  const [loaded, setLoaded] = useState<Loaded>(
    mode === 'edit'
      ? { state: 'loading' }
      : { state: 'ready', profile: undefined },
  );
  useEffect(() => {
    if (mode !== 'edit' || !id) return;
    let live = true;
    getProfile(id).then(
      (profile) => {
        if (live) setLoaded({ state: 'ready', profile });
      },
      (error: unknown) => {
        const reason = error instanceof Error ? error.message : String(error);
        if (live) setLoaded({ state: 'failed', reason });
      },
    );
    return () => {
      live = false;
    };
  }, [mode, id]);
  return loaded;
}

/**
 * New or edit connection as a tab (DESIGN.md §9): engine, details, security,
 * options, then test and save. Edits open at Details with the profile loaded.
 */
export function NewConnectionFlow(
  props: NewConnectionFlowProps,
): React.JSX.Element {
  const loaded = useEditedProfile(props.mode, props.profileId);
  if (loaded.state !== 'ready')
    return (
      <div className={styles.loading} role="status">
        {loaded.state === 'loading' ? (
          t('loading')
        ) : (
          <span>
            {t('loadFailed', { reason: loaded.reason })}{' '}
            <Button onClick={props.onCancel}>{t('cancel')}</Button>
          </span>
        )}
      </div>
    );
  return (
    <FlowView
      key={loaded.profile?.id ?? 'new'}
      {...props}
      profile={loaded.profile}
    />
  );
}

function FlowView(
  props: NewConnectionFlowProps & { profile: ProfileDto | undefined },
): React.JSX.Element {
  const { onDone, onSaveAndConnect } = props;
  const flow = useFlowController(
    {
      profile: props.profile,
      engine: props.engine,
      filePath: props.filePath,
      focusSafety: props.focus === 'safety',
    },
    (profile, connect) => {
      if (connect && onSaveAndConnect) onSaveAndConnect(profile.id);
      else onDone(profile.id);
    },
  );
  const { form, steps } = flow;
  const editing = props.profile !== undefined;
  return (
    <div className={styles.host}>
      <form
        className={styles.flow}
        aria-label={editing ? t('editConnection') : t('newConnection')}
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          if (steps.current === 'test') flow.save(false);
          else flow.advance();
        }}
      >
        <FlowRail
          steps={steps}
          draft={form.draft}
          tested={tested(flow)}
          title={editing ? t('editConnection') : t('newConnection')}
        />
        <FlowPage flow={flow} focusSafety={props.focus === 'safety'} />
        <SummaryColumn
          draft={flow.chosen ? form.draft : null}
          name={form.draft.name.trim() || derivedName(form.draft)}
          test={form.test}
        />
        <FlowFooter
          steps={steps}
          editing={editing}
          ready={flow.chosen}
          saving={form.saving}
          saveError={form.saveError}
          canConnect={!editing && onSaveAndConnect !== undefined}
          onCancel={props.onCancel}
          onSave={flow.save}
        />
      </form>
    </div>
  );
}

function tested(flow: FlowController): boolean {
  const { test } = flow.form;
  return test.status === 'done' && test.report.server !== null;
}

/** The current step's page, sliding in from the side it came from. */
function FlowPage(props: {
  flow: FlowController;
  focusSafety: boolean;
}): React.JSX.Element {
  const { flow } = props;
  const { form, steps } = flow;
  const page = useStepFocus(steps.current, !props.focusSafety);
  return (
    <div className={styles.main}>
      <div
        ref={page}
        key={steps.current}
        className={styles.page}
        data-direction={steps.direction}
      >
        {steps.current === 'engine' && (
          <EngineStep
            engine={flow.chosen ? form.draft.engine : null}
            paste={flow.paste}
            onPick={flow.pick}
            onPaste={flow.pasteGo}
          />
        )}
        {steps.current === 'details' && <DetailsStep form={form} />}
        {steps.current === 'security' && <SecurityStep form={form} />}
        {steps.current === 'options' && (
          <OptionsStep form={form} focusSafety={props.focusSafety} />
        )}
        {steps.current === 'test' && (
          <TestStep form={form} autoTest={flow.autoTest} />
        )}
      </div>
    </div>
  );
}

/** Focus the first field of a step when it opens (the engine step focuses its paste field). */
function useStepFocus(
  step: string,
  enabled: boolean,
): React.RefObject<HTMLDivElement | null> {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!enabled || step === 'engine') return;
    ref.current
      ?.querySelector<HTMLElement>(
        'input:not([type="hidden"]), select, textarea',
      )
      ?.focus({ preventScroll: true });
  }, [step, enabled]);
  return ref;
}
