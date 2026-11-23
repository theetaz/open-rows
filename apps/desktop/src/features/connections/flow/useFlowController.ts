// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type { EngineDto, ProfileDto } from '@openrows/ipc';
import { validate } from '../form';
import { isFileEngine } from '../engines';
import { useConnectionForm } from '../useConnectionForm';
import type { ConnectionForm } from '../useConnectionForm';
import { derivedName, guessEnvironment, stepsWithErrors } from './flowDraft';
import type { StepId } from './flowDraft';
import { usePasteUrl } from './usePasteUrl';
import type { PasteUrl } from './usePasteUrl';
import { useFlowSteps } from './useFlowSteps';
import type { FlowSteps } from './useFlowSteps';

type FlowStart = {
  profile: ProfileDto | undefined;
  engine: EngineDto | undefined;
  filePath: string | undefined;
  focusSafety: boolean;
};

export type FlowController = {
  form: ConnectionForm;
  steps: FlowSteps;
  paste: PasteUrl;
  /** An engine was chosen (always, when editing). */
  chosen: boolean;
  /** The test step was opened from a pasted URL and runs on its own. */
  autoTest: boolean;
  pick: (engine: EngineDto) => void;
  pasteGo: () => void;
  /** Continue: validate this step's fields, then the next step. */
  advance: () => void;
  /** Save, after showing the first step with an invalid field. */
  save: (connect: boolean) => void;
};

function startStep(start: FlowStart): StepId {
  if (start.profile) return start.focusSafety ? 'options' : 'details';
  return start.engine ? 'details' : 'engine';
}

/** State and actions of the new/edit connection flow over the existing form. */
export function useFlowController(
  start: FlowStart,
  onSaved: (profile: ProfileDto, connect: boolean) => void,
): FlowController {
  const connect = useRef(false);
  const form = useConnectionForm(
    start.profile,
    (profile) => {
      onSaved(profile, connect.current);
    },
    start.engine,
    start.filePath,
  );
  const editing = start.profile !== undefined;
  const steps = useFlowSteps(form.draft.engine, editing, startStep(start));
  const paste = usePasteUrl();
  const [chosen, setChosen] = useState(editing || start.engine !== undefined);
  const [autoTest, setAutoTest] = useState(false);
  return {
    form,
    steps,
    paste,
    chosen,
    autoTest,
    pick: (engine) => {
      form.chooseEngine(engine);
      setChosen(true);
      steps.go('details');
    },
    pasteGo: () => {
      const { parsed } = paste;
      if (!parsed) return;
      form.applyUrl(parsed, paste.password);
      if (!editing)
        form.setEnvironment(
          isFileEngine(parsed.engine) ? 'local' : guessEnvironment(parsed.host),
        );
      setChosen(true);
      setAutoTest(true);
      steps.go('test');
    },
    advance: () => {
      advance(form, steps);
    },
    save: (andConnect) => {
      connect.current = andConnect;
      const invalid = stepsWithErrors(validate(form.draft), steps.order)[0];
      if (invalid) steps.go(invalid);
      void form.save();
    },
  };
}

/** Check this step; a valid Details names an unnamed connection after its host or file. */
function advance(form: ConnectionForm, steps: FlowSteps): void {
  const unnamed = steps.current === 'details' && !form.draft.name.trim();
  const draft = unnamed
    ? { ...form.draft, name: derivedName(form.draft) }
    : form.draft;
  const invalid = stepsWithErrors(validate(draft), steps.order);
  if (invalid.includes(steps.current)) {
    form.showErrors();
    return;
  }
  if (unnamed) form.update({ name: draft.name });
  steps.next();
}
