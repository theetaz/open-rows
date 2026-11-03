// SPDX-License-Identifier: Apache-2.0
import { Badge } from '@openrows/ui';
import type { CertificateDto, LatencyDto } from '@openrows/ipc';
import { expiryText, latencyQuality } from './testReportText';
import styles from './TestDetails.module.css';
import { t } from './messages/profile';

const qualityTone = {
  fast: 'success',
  moderate: 'warning',
  slow: 'danger',
} as const;

/** Median round trip of `SELECT 1`, coloured and labelled by how it feels. */
export function LatencyBadge({
  latency,
}: {
  latency: LatencyDto | null;
}): React.JSX.Element | null {
  if (latency?.medianMs == null) return null;
  const quality = latencyQuality(latency.medianMs);
  return (
    <Badge
      tone={qualityTone[quality]}
      title={t('medianSamplesSelect', { samples: String(latency.samples) })}
    >
      {t('latencyMedianmsMs', {
        medianMs: latency.medianMs.toFixed(1),
        quality,
      })}
    </Badge>
  );
}

/** Subject, issuer, validity with days to expiry, and alternative names. */
export function CertificateDetails({
  certificate,
}: {
  certificate: CertificateDto;
}): React.JSX.Element {
  const expiry = expiryText(certificate.daysLeft);
  return (
    <section className={styles.certificate} aria-label={t('serverCertificate')}>
      <h3>
        {t('serverCertificate')}
        {!certificate.verified && (
          <Badge tone="warning">{t('notVerified')}</Badge>
        )}
      </h3>
      <dl>
        <Row term={t('subject')} value={certificate.subject} />
        <Row term={t('issuer')} value={certificate.issuer} />
        <Row term={t('valid')} value={certificate.notBefore} />
        <dt>{t('validUntil')}</dt>
        <dd>
          {certificate.notAfter ?? '—'}
          {expiry && (
            <Badge tone={certificate.expiringSoon ? 'warning' : 'neutral'}>
              {expiry}
            </Badge>
          )}
        </dd>
        <Row
          term={t('names')}
          value={certificate.altNames.join(', ') || null}
        />
      </dl>
    </section>
  );
}

function Row(props: { term: string; value: string | null }): React.JSX.Element {
  return (
    <>
      <dt>{props.term}</dt>
      <dd>{props.value ?? '—'}</dd>
    </>
  );
}
