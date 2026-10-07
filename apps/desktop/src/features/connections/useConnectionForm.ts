// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { createProfile, testConnection, updateProfile } from '@openrows/ipc';
import type { EnvKindDto, ProfileDto, TestReport } from '@openrows/ipc';
import {
  draftFromProfile,
  emptyDraft,
  toInput,
  validate,
  withEnvironment,
  withPassword,
  withPatch,
} from './form';
import type { Draft, DraftErrors } from './form';
import { useUrlField } from './useUrlField';
import type { UrlField } from './useUrlField';

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
  /** The connection URL, kept in sync with the fields both ways. */
  url: UrlField;
  update: (patch: Partial<Draft>) => void;
  setEnvironment: (kind: EnvKindDto) => void;
  setPassword: (value: string) => void;
  runTest: () => Promise<void>;
  save: () => Promise<void>;
};

/** Fields that appear in the connection URL. */
const URL_FIELDS = new Set<string>([
  'host',
  'port',
  'database',
  'user',
  'tlsMode',
  'password',
  'passwordMode',
]);

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
  const url = useUrlField(draft, setDraft);
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
    url,
    update: (patch) => {
      setDraft((current) => withPatch(current, patch));
      if (Object.keys(patch).some((key) => URL_FIELDS.has(key))) url.release();
    },
    setEnvironment: (kind) => {
      setDraft((current) => withEnvironment(current, kind));
    },
    setPassword: (value) => {
      setDraft((current) => withPassword(current, value));
      url.release();
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
