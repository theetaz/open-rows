// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import {
  createProfile,
  parsePostgresUrl,
  testConnection,
  updateProfile,
} from '@openrows/ipc';
import type { EnvKindDto, ProfileDto, TestReport } from '@openrows/ipc';
import {
  draftFromProfile,
  emptyDraft,
  toInput,
  validate,
  withEnvironment,
  withPassword,
  withUrl,
} from './form';
import type { Draft, DraftErrors } from './form';

export type TestState =
  | { status: 'idle' }
  | { status: 'testing' }
  | { status: 'ok'; report: TestReport }
  | { status: 'error'; message: string };

export type ConnectionForm = {
  draft: Draft;
  errors: DraftErrors;
  test: TestState;
  saving: boolean;
  saveError: string | null;
  urlError: string | null;
  update: (patch: Partial<Draft>) => void;
  setEnvironment: (kind: EnvKindDto) => void;
  setPassword: (value: string) => void;
  /** Resolves `true` when the URL filled the form. */
  pasteUrl: (url: string) => Promise<boolean>;
  runTest: () => Promise<void>;
  save: () => Promise<void>;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** Form state for the new/edit connection sheet; errors show after the first Test or Save. */
export function useConnectionForm(
  profile: ProfileDto | undefined,
  onSaved: (profile: ProfileDto) => void,
): ConnectionForm {
  const [draft, setDraft] = useState(() =>
    profile ? draftFromProfile(profile) : emptyDraft(),
  );
  const [submitted, setSubmitted] = useState(false);
  const [urlError, setUrlError] = useState<string | null>(null);
  const checked = (): boolean => {
    setSubmitted(true);
    return Object.keys(validate(draft)).length === 0;
  };
  const test = useTest(profile?.id ?? null, draft, checked);
  const saving = useSave(profile, draft, checked, onSaved);
  return {
    draft,
    errors: submitted ? validate(draft) : {},
    test: test.state,
    saving: saving.busy,
    saveError: saving.error,
    urlError,
    update: (patch) => {
      setDraft((current) => ({ ...current, ...patch }));
    },
    setEnvironment: (kind) => {
      setDraft((current) => withEnvironment(current, kind));
    },
    setPassword: (value) => {
      setDraft((current) => withPassword(current, value));
    },
    pasteUrl: async (url) => {
      try {
        const parsed = await parsePostgresUrl(url);
        setDraft((current) => withUrl(current, parsed, url));
        setUrlError(null);
        return true;
      } catch (error) {
        setUrlError(message(error));
        return false;
      }
    },
    runTest: test.run,
    save: saving.run,
  };
}

function useTest(
  profileId: string | null,
  draft: Draft,
  checked: () => boolean,
): { state: TestState; run: () => Promise<void> } {
  const [state, setState] = useState<TestState>({ status: 'idle' });
  const run = async (): Promise<void> => {
    if (!checked()) return;
    setState({ status: 'testing' });
    try {
      const report = await testConnection(profileId, toInput(draft));
      setState({ status: 'ok', report });
    } catch (error) {
      setState({ status: 'error', message: message(error) });
    }
  };
  return { state, run };
}

function useSave(
  profile: ProfileDto | undefined,
  draft: Draft,
  checked: () => boolean,
  onSaved: (profile: ProfileDto) => void,
): { busy: boolean; error: string | null; run: () => Promise<void> } {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (): Promise<void> => {
    if (!checked() || busy) return;
    setBusy(true);
    setError(null);
    try {
      const input = toInput(draft);
      onSaved(
        profile
          ? await updateProfile(profile.id, input)
          : await createProfile(input),
      );
    } catch (failure) {
      setError(message(failure));
      setBusy(false);
    }
  };
  return { busy, error, run };
}
