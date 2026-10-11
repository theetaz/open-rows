// SPDX-License-Identifier: Apache-2.0
//! Normalize fully validated explicit NIST domains to named-curve PKCS#8.

use pkcs8::{
    AssociatedOid, ObjectIdentifier, PrivateKeyInfoRef,
    der::{
        self, Decode, Encode, Reader, SliceReader, Tag, Tagged,
        asn1::{AnyRef, BitStringRef, OctetStringRef, UintRef},
    },
};
use russh::keys::PrivateKey;
use zeroize::Zeroizing;

use super::key::DecodeError;

const EC_ALGORITHM: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.2.1");
const PRIME_FIELD: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.1.1");

/// Decode explicit-domain ECDSA PKCS#8; other algorithms retain their loader.
pub(super) fn decode(bytes: &[u8]) -> Result<Option<PrivateKey>, DecodeError> {
    let Ok(mut info) = PrivateKeyInfoRef::try_from(bytes) else {
        return Ok(None);
    };
    if info.algorithm.oid != EC_ALGORITHM {
        return Ok(None);
    }
    let Some(parameters) = info
        .algorithm
        .parameters
        .filter(|value| value.tag() == Tag::Sequence)
    else {
        return Ok(None);
    };
    let domain = Domain::decode(parameters.value()).map_err(|_| invalid_domain())?;
    let oid = named_curve(&domain)?;
    info.algorithm.parameters =
        Some(AnyRef::new(Tag::ObjectIdentifier, oid.as_bytes()).map_err(|_| invalid_domain())?);
    let normalized = Zeroizing::new(info.to_der().map_err(|_| invalid_domain())?);
    russh::keys::pkcs8::decode_pkcs8(&normalized, None)
        .map(Some)
        .map_err(|error| DecodeError::Invalid(error.to_string()))
}

#[derive(PartialEq)]
struct Domain<'a> {
    prime: &'a [u8],
    a: &'a [u8],
    b: &'a [u8],
    generator: &'a [u8],
    order: &'a [u8],
    cofactor: u64,
}

impl<'a> Domain<'a> {
    fn decode(bytes: &'a [u8]) -> der::Result<Self> {
        let mut reader = SliceReader::new(bytes)?;
        if u8::decode(&mut reader)? != 1 {
            return Err(der::ErrorKind::Value { tag: Tag::Integer }.into());
        }
        let prime = prime_field(&mut reader)?;
        let (a, b) = reader.sequence(|curve| -> der::Result<_> {
            let a = <&OctetStringRef>::decode(curve)?.as_bytes();
            let b = <&OctetStringRef>::decode(curve)?.as_bytes();
            if !curve.is_finished() {
                BitStringRef::decode(curve)?;
            }
            Ok((a, b))
        })?;
        let generator = <&OctetStringRef>::decode(&mut reader)?.as_bytes();
        let order = UintRef::decode(&mut reader)?.as_bytes();
        let cofactor = if reader.is_finished() {
            1
        } else {
            u64::decode(&mut reader)?
        };
        reader.finish()?;
        Ok(Self {
            prime,
            a,
            b,
            generator,
            order,
            cofactor,
        })
    }
}

fn prime_field<'a>(reader: &mut impl Reader<'a>) -> der::Result<&'a [u8]> {
    reader.sequence(|field| -> der::Result<_> {
        if ObjectIdentifier::decode(field)? != PRIME_FIELD {
            return Err(der::ErrorKind::Value {
                tag: Tag::ObjectIdentifier,
            }
            .into());
        }
        Ok(UintRef::decode(field)?.as_bytes())
    })
}

fn named_curve(domain: &Domain<'_>) -> Result<ObjectIdentifier, DecodeError> {
    // Public SEC2 mathematical constants; compare the entire domain, not its bit length.
    for (oid, bytes) in [
        (
            p256::NistP256::OID,
            include_bytes!("ecdsa_parameters/p256.der").as_slice(),
        ),
        (
            p384::NistP384::OID,
            include_bytes!("ecdsa_parameters/p384.der").as_slice(),
        ),
        (
            p521::NistP521::OID,
            include_bytes!("ecdsa_parameters/p521.der").as_slice(),
        ),
    ] {
        let mut reader = SliceReader::new(bytes).map_err(|_| invalid_domain())?;
        let parameters = AnyRef::decode(&mut reader).map_err(|_| invalid_domain())?;
        reader.finish().map_err(|_| invalid_domain())?;
        let expected = Domain::decode(parameters.value()).map_err(|_| invalid_domain())?;
        if *domain == expected {
            return Ok(oid);
        }
    }
    Err(invalid_domain())
}

fn invalid_domain() -> DecodeError {
    DecodeError::Invalid("unsupported ECDSA domain parameters; convert to a named curve or OpenSSH format with `ssh-keygen -p`".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::key::pem_body;

    use crate::auth::ecdsa_fixtures::fixture;

    fn rejects_domain(parameters: &[u8]) {
        let bytes = pem_body(&fixture(256, "short.explicit.pkcs8")).expect("test fixture");
        let mut info = PrivateKeyInfoRef::try_from(bytes.as_slice()).expect("PKCS#8");
        info.algorithm.parameters =
            Some(AnyRef::new(Tag::Sequence, parameters).expect("parameters"));
        assert!(matches!(
            decode(&info.to_der().expect("encode")),
            Err(DecodeError::Invalid(_))
        ));
    }

    #[test]
    fn changing_any_domain_component_is_rejected() {
        let bytes = pem_body(&fixture(256, "short.explicit.pkcs8")).expect("test fixture");
        let info = PrivateKeyInfoRef::try_from(bytes.as_slice()).expect("PKCS#8");
        let parameters = info.algorithm.parameters.expect("parameters").value();
        let domain = Domain::decode(parameters).expect("standard domain");
        for component in [
            domain.prime,
            domain.a,
            domain.b,
            domain.generator,
            domain.order,
        ] {
            let offset = parameters
                .windows(component.len())
                .position(|value| value == component)
                .expect("component");
            let mut changed = parameters.to_vec();
            changed[offset + component.len() - 1] ^= 1;
            rejects_domain(&changed);
        }
        let mut changed = parameters.to_vec();
        let last = changed.len() - 1;
        changed[last] ^= 1; // cofactor
        rejects_domain(&changed);
        changed.push(0); // trailing ASN.1 bytes
        rejects_domain(&changed);
    }
}
