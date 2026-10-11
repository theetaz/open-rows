# Public NIST curve domain parameters

These DER files contain only public mathematical constants for secp256r1,
secp384r1 and secp521r1 (NIST P-256/P-384/P-521). They contain no private key,
credential, certificate or user data. The domain `(p, a, b, G, n, h)` is specified
in [Standards for Efficient Cryptography 2 (SEC 2), version 2.0](https://www.secg.org/sec2-v2.pdf),
sections 2.4.2, 2.5.1 and 2.6.1. No document prose or implementation code is copied.

Generated with OpenSSL 3:

```sh
openssl ecparam -name prime256v1 -param_enc explicit -outform DER -out p256.der
openssl ecparam -name secp384r1 -param_enc explicit -outform DER -out p384.der
openssl ecparam -name secp521r1 -param_enc explicit -outform DER -out p521.der
```

The loader compares the prime, both curve coefficients, base point, order and
cofactor before replacing an explicit PKCS#8 domain with its named-curve OID.
An optional generation seed does not alter the mathematical domain. An omitted
cofactor is accepted as one for these exact standard domains. Other domains,
malformed ASN.1 and trailing fields are rejected; a matching bit length alone
never selects a curve. Regression tests change each component independently.
