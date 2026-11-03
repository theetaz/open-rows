// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Badge } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import type { Connections } from '../connections';
import { routeStatus } from './routeStatus';
import { POLICY_LABELS } from '../security/policies';
import { t } from './lazyMessages';

/** Preview the target connection's environment and safety before connecting. */
export function ConnectionSwitcher(props: {
  connections: Connections;
  onClose: () => void;
}): React.JSX.Element {
  const { connections } = props;
  const [id, setId] = useState(
    connections.active?.id ?? connections.profiles[0]?.id ?? '',
  );
  const profile = connections.profiles.find((item) => item.id === id);
  return (
    <Sheet title={t('switchConnection')} size="alert" onClose={props.onClose}>
      <label>
        {t('connection')}
        <select
          data-initial-focus
          value={id}
          onChange={(event) => {
            setId(event.target.value);
          }}
        >
          {connections.profiles.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
      </label>
      {profile && (
        <div role="group" aria-label={t('connectionPreview')}>
          <Badge style={{ borderColor: profile.env.color }}>
            {profile.env.label ?? profile.env.kind}
          </Badge>
          <p>
            {profile.safety === 'readOnly'
              ? t('readOnlyPolicy', {
                  policyLabels: POLICY_LABELS[profile.safety],
                })
              : t('readWritePolicy', {
                  policyLabels: POLICY_LABELS[profile.safety],
                })}
          </p>
          {profile.env.kind === 'prod' && <p>{t('productionConnection')}</p>}
          <p>{routeStatus(profile)}</p>
          <p>{t('existingTabsStay')}</p>
        </div>
      )}
      <SheetActions>
        <Button onClick={props.onClose}>{t('cancel')}</Button>
        <Button
          variant="primary"
          disabled={!profile}
          onClick={() => {
            connections.select(id);
            props.onClose();
          }}
        >
          {t('switchConnection')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
