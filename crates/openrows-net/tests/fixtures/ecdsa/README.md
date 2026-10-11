# Runtime ECDSA regression fixtures

No encoded key material is stored in this directory. `src/auth/ecdsa_fixtures.rs`
builds keys in memory from visibly synthetic hex scalars (a leading `01` or `80`
followed by repeated `11` bytes), deriving the matching public points with the
already locked RustCrypto curve crates. Never use these synthetic values for
real authentication.

| Curve | Short magnitude | Sign-padded mpint | Full-width magnitude |
| --- | ---: | ---: | ---: |
| P-256 | 31 bytes | 33 bytes | 32 bytes |
| P-384 | 47 bytes | 49 bytes | 48 bytes |
| P-521 | 65 bytes | 66 bytes | 66 bytes |

The OpenSSH envelope is encoded with the existing SSH codec, including canonical
positive mpints. Named/explicit-domain SEC1 and PKCS#8 are encoded with the locked
curve and ASN.1 codecs. Explicit domains reuse the public mathematical constants
needed by the production validator. Encryption runs `ssh-keygen -p` on a runtime
key in an automatically removed temporary directory; no new dependency is needed.

The P-256 short case deterministically reproduces the upstream 32-byte minimum.
Tests verify key identity and signatures, cipher variants, malformed scalars and
envelopes, compressed points, and every explicit-domain component. Randomized
coverage remains opt-in (200 generated keys per curve):

```sh
cargo nextest run -p openrows-net --locked generated_ecdsa_keys_decode --run-ignored ignored-only
```
