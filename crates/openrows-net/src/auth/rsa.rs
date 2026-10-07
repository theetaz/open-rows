// SPDX-License-Identifier: Apache-2.0
//! RSA user-key signing with ring (constant-time), instead of the `rsa` crate
//! that carries RUSTSEC-2023-0071 (Marvin timing side channel).
//!
//! Supports `rsa-sha2-256` and `rsa-sha2-512` (RFC 8332). SHA-1 `ssh-rsa`
//! signatures are refused, as in current OpenSSH.

use std::future::Future;

use ring::{
    rand::SystemRandom,
    rsa::{KeyPairComponents, PublicKeyComponents},
    signature::{self, RsaKeyPair},
};
use russh::keys::{
    HashAlg, PrivateKey, PublicKey,
    agent::AgentIdentity,
    ssh_key::{Mpint, private::KeypairData, public::KeyData, public::RsaPublicKey},
};
use zeroize::Zeroizing;

use super::bigint::{minus_one, remainder};

/// Why an RSA signature could not be produced.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RsaSignError {
    #[error("the server only accepts SHA-1 ssh-rsa signatures, which are not supported")]
    Sha1Only,
    #[error("RSA signing failed")]
    Sign,
    #[error(transparent)]
    Send(#[from] russh::SendError),
}

/// An RSA private key held by ring, plus its SSH public key.
pub(crate) struct RsaSigner {
    key_pair: RsaKeyPair,
    public: PublicKey,
}

impl RsaSigner {
    pub(crate) fn from_pkcs1_der(der: &[u8]) -> Result<Self, String> {
        Self::from_key_pair(RsaKeyPair::from_der(der).map_err(|error| error.to_string())?)
    }

    pub(crate) fn from_pkcs8_der(der: &[u8]) -> Result<Self, String> {
        Self::from_key_pair(RsaKeyPair::from_pkcs8(der).map_err(|error| error.to_string())?)
    }

    /// Convert an OpenSSH-format RSA key, deriving the CRT exponents.
    pub(crate) fn from_openssh(key: &PrivateKey) -> Result<Self, String> {
        let KeypairData::Rsa(pair) = key.key_data() else {
            return Err("not an RSA key".into());
        };
        let private = pair.private();
        let (d, p, q) = (
            positive(private.d())?,
            positive(private.p())?,
            positive(private.q())?,
        );
        let d_p = crt_exponent(d, p)?;
        let d_q = crt_exponent(d, q)?;
        let components = KeyPairComponents {
            public_key: PublicKeyComponents {
                n: positive(pair.public().n())?,
                e: positive(pair.public().e())?,
            },
            d,
            p,
            q,
            dP: d_p.as_slice(),
            dQ: d_q.as_slice(),
            qInv: positive(private.iqmp())?,
        };
        let key_pair =
            RsaKeyPair::from_components(&components).map_err(|error| error.to_string())?;
        Self::from_key_pair(key_pair)
    }

    fn from_key_pair(key_pair: RsaKeyPair) -> Result<Self, String> {
        let components = PublicKeyComponents::<Vec<u8>>::from(key_pair.public());
        let public = RsaPublicKey::new(
            Mpint::from_positive_bytes(&components.e),
            Mpint::from_positive_bytes(&components.n),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            key_pair,
            public: PublicKey::new(KeyData::Rsa(public), ""),
        })
    }

    pub(crate) fn public_key(&self) -> PublicKey {
        self.public.clone()
    }

    /// `to_sign` followed by the SSH signature blob, the format russh expects from a signer.
    pub(super) fn sign(
        &self,
        hash: Option<HashAlg>,
        mut to_sign: Vec<u8>,
    ) -> Result<Vec<u8>, RsaSignError> {
        let (name, padding): (&str, &'static dyn signature::RsaEncoding) = match hash {
            Some(HashAlg::Sha512) => ("rsa-sha2-512", &signature::RSA_PKCS1_SHA512),
            Some(HashAlg::Sha256) => ("rsa-sha2-256", &signature::RSA_PKCS1_SHA256),
            _ => return Err(RsaSignError::Sha1Only),
        };
        let mut sig = vec![0; self.key_pair.public().modulus_len()];
        self.key_pair
            .sign(padding, &SystemRandom::new(), &to_sign, &mut sig)
            .map_err(|_| RsaSignError::Sign)?;
        let blob_len = 8 + name.len() + sig.len();
        put_string_len(&mut to_sign, blob_len)?;
        put_string_len(&mut to_sign, name.len())?;
        to_sign.extend_from_slice(name.as_bytes());
        put_string_len(&mut to_sign, sig.len())?;
        to_sign.extend_from_slice(&sig);
        Ok(to_sign)
    }
}

fn positive(value: &Mpint) -> Result<&[u8], String> {
    value
        .as_positive_bytes()
        .ok_or_else(|| "negative RSA component".into())
}

/// `d mod (prime - 1)`, wiped on drop.
fn crt_exponent(d: &[u8], prime: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
    let order = Zeroizing::new(minus_one(prime).ok_or("invalid RSA prime")?);
    remainder(d, &order)
        .map(Zeroizing::new)
        .ok_or_else(|| "invalid RSA prime".into())
}

fn put_string_len(out: &mut Vec<u8>, len: usize) -> Result<(), RsaSignError> {
    let len = u32::try_from(len).map_err(|_| RsaSignError::Sign)?;
    out.extend_from_slice(&len.to_be_bytes());
    Ok(())
}

impl russh::Signer for RsaSigner {
    type Error = RsaSignError;

    fn auth_sign(
        &mut self,
        _key: &AgentIdentity,
        hash_alg: Option<HashAlg>,
        to_sign: Vec<u8>,
    ) -> impl Future<Output = Result<Vec<u8>, Self::Error>> + Send {
        let signed = self.sign(hash_alg, to_sign);
        async move { signed }
    }
}
