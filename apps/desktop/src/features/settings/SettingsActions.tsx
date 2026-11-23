// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextArea } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { exportSettings, importSettings, resetSettings } from './settingsFile';
import { SettingsMenu } from './SettingsMenu';
import type { SettingsAction } from './SettingsMenu';
import { t } from './messages/lazyGeneral';

/**
 * The settings actions menu: confirm resets and exchange only
 * schema-validated, non-secret preferences.
 */
export function SettingsActions({
  section,
}: {
  section: string;
}): React.JSX.Element {
  const [action, setAction] = useState<SettingsAction | null>(null);
  const [text, setText] = useState('');
  const [message, setMessage] = useState('');
  const resetting = action === 'group' || action === 'all';
  const submit = async (): Promise<void> => {
    try {
      if (resetting) resetSettings(action === 'group' ? section : undefined);
      else await importSettings(text);
      setAction(null);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  };
  const open = (next: SettingsAction): void => {
    setMessage('');
    setText(next === 'export' ? exportSettings() : '');
    setAction(next);
  };
  return (
    <>
      <SettingsMenu
        groupDisabled={section === 'locations' || section === 'json'}
        onChoose={open}
      />
      {action && (
        <ActionDialog
          action={action}
          section={section}
          resetting={resetting}
          text={text}
          setText={setText}
          message={message}
          close={() => {
            setAction(null);
          }}
          submit={submit}
        />
      )}
    </>
  );
}

/** One reset confirmation or portable JSON exchange. */
function ActionDialog({
  action,
  section,
  resetting,
  text,
  setText,
  message,
  close,
  submit,
}: {
  action: SettingsAction;
  section: string;
  resetting: boolean;
  text: string;
  setText: (text: string) => void;
  message: string;
  close: () => void;
  submit: () => Promise<void>;
}): React.JSX.Element {
  return (
    <Sheet
      title={
        resetting
          ? t('resetDefaults')
          : action === 'import'
            ? t('importSettings')
            : t('exportSettings')
      }
      onClose={() => {
        close();
      }}
    >
      <p>
        {resetting
          ? action === 'all'
            ? t('resetAllPreferences')
            : t('resetSectionPreferences', { section })
          : t('portablePreferencesExclude')}
      </p>
      {!resetting && (
        <TextArea
          aria-label={t('portableSettingsJson')}
          value={text}
          readOnly={action === 'export'}
          rows={16}
          onChange={(event) => {
            setText(event.target.value);
          }}
        />
      )}
      {message && <p role="alert">{message}</p>}
      <SheetActions>
        <Button
          onClick={() => {
            close();
          }}
        >
          {action === 'export' ? t('done') : t('cancel')}
        </Button>
        {action !== 'export' && (
          <Button
            variant="primary"
            onClick={() => {
              void submit();
            }}
          >
            {resetting ? t('resetDefaults2') : t('importPreferences')}
          </Button>
        )}
      </SheetActions>
    </Sheet>
  );
}
