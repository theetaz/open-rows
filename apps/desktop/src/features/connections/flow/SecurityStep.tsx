// SPDX-License-Identifier: Apache-2.0
import { chooseRoute, HelperFields, routeOf } from '../HelperFields';
import type { RouteKind } from '../HelperFields';
import { ProxyFields } from '../ProxyFields';
import { SshFields } from '../SshFields';
import { TlsFields } from '../TlsFields';
import type { ConnectionForm } from '../useConnectionForm';
import { ChoiceCards } from './ChoiceCards';
import type { Card } from './ChoiceCards';
import { FormSection } from './FormSection';
import formStyles from './Form.module.css';
import page from './Flow.module.css';
import { t } from './messages';

const ROUTES: readonly Card<RouteKind>[] = [
  {
    value: 'direct',
    icon: 'globe',
    title: t('routeDirect'),
    detail: t('routeDirectSub'),
  },
  {
    value: 'cloud',
    icon: 'cloud',
    title: t('routeCloud'),
    detail: t('routeCloudSub'),
  },
  {
    value: 'zero',
    icon: 'shield',
    title: t('routeZero'),
    detail: t('routeZeroSub'),
  },
];

/**
 * Step 3 (servers only): the route as three cards with its hop settings,
 * TLS with certificates, then SSH and proxy as switches that reveal rows.
 * Cloud and zero-trust routes cannot combine with SSH or a proxy.
 */
export function SecurityStep({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const route = routeOf(form.draft.helper);
  return (
    <>
      <h2 className={page.title}>{t('securityTitle')}</h2>
      <p className={page.lede}>{t('securityLede')}</p>
      <FormSection title={t('route')} icon="route" bare>
        <ChoiceCards
          label={t('route')}
          cards={ROUTES}
          value={route}
          onChange={(next) => {
            chooseRoute(form, next);
          }}
        />
        {route !== 'direct' && (
          <div className={formStyles.group}>
            <HelperFields form={form} />
          </div>
        )}
      </FormSection>
      <FormSection title={t('encryption')} icon="lock">
        <TlsFields form={form} />
      </FormSection>
      <FormSection title={t('sshTunnel')} icon="terminal">
        <SshFields form={form} />
      </FormSection>
      <FormSection title={t('proxy')} icon="plug">
        <ProxyFields form={form} />
      </FormSection>
    </>
  );
}
