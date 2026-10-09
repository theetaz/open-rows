// SPDX-License-Identifier: Apache-2.0
import { Badge } from '@openrows/ui';
import type { CertificateDto, LatencyDto } from '@openrows/ipc';
import { expiryText, latencyQuality } from './testReportText';
import styles from './TestDetails.module.css';

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
      title={`Median of ${String(latency.samples)} SELECT 1 round trips`}
    >
      {`Latency ${latency.medianMs.toFixed(1)} ms · ${quality}`}
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
    <section className={styles.certificate} aria-label="Server certificate">
      <h3>
        Server certificate
        {!certificate.verified && <Badge tone="warning">Not verified</Badge>}
      </h3>
      <dl>
        <Row term="Subject" value={certificate.subject} />
        <Row term="Issuer" value={certificate.issuer} />
        <Row term="Valid from" value={certificate.notBefore} />
        <dt>Valid until</dt>
        <dd>
          {certificate.notAfter ?? '—'}
          {expiry && (
            <Badge tone={certificate.expiringSoon ? 'warning' : 'neutral'}>
              {expiry}
            </Badge>
          )}
        </dd>
        <Row term="Names" value={certificate.altNames.join(', ') || null} />
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
