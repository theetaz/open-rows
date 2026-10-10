// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextArea } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import { exportSettings, importSettings, resetSettings } from './settingsFile';

/** Confirm resets and exchange only schema-validated, non-secret preferences. */
export function SettingsActions({
  section,
}: {
  section: string;
}): React.JSX.Element {
  const [action, setAction] = useState<
    'group' | 'all' | 'import' | 'export' | null
  >(null);
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
  const open = (next: NonNullable<typeof action>): void => {
    setMessage('');
    setText(next === 'export' ? exportSettings() : '');
    setAction(next);
  };
  return (
    <>
      <div>
        <Button
          disabled={section === 'locations' || section === 'json'}
          onClick={() => {
            open('group');
          }}
        >
          Reset group
        </Button>
        <Button
          onClick={() => {
            open('all');
          }}
        >
          Reset all settings
        </Button>
        <Button
          onClick={() => {
            open('export');
          }}
        >
          Export settings
        </Button>
        <Button
          onClick={() => {
            open('import');
          }}
        >
          Import settings
        </Button>
      </div>
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
  action: 'group' | 'all' | 'import' | 'export';
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
          ? 'Reset to defaults?'
          : `${action === 'import' ? 'Import' : 'Export'} settings`
      }
      onClose={() => {
        close();
      }}
    >
      <p>
        {resetting
          ? `Reset ${action === 'all' ? 'all preferences' : `${section} preferences`}? Connection profiles, credentials, history entries and data locations remain intact.`
          : 'Portable preferences exclude credentials, history settings/data, query snippets and device tool paths. Copy or paste the JSON below.'}
      </p>
      {!resetting && (
        <TextArea
          aria-label="Portable settings JSON"
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
          {action === 'export' ? 'Done' : 'Cancel'}
        </Button>
        {action !== 'export' && (
          <Button
            variant="primary"
            onClick={() => {
              void submit();
            }}
          >
            {resetting ? 'Reset defaults' : 'Import preferences'}
          </Button>
        )}
      </SheetActions>
    </Sheet>
  );
}
