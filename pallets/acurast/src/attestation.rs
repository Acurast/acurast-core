#![cfg_attr(all(feature = "alloc", not(feature = "std"), not(test)), no_std)]

pub mod asn;
pub mod error;

use asn::*;
use asn1::{oid, BitString, Null, ObjectIdentifier, ParseError, SequenceOf};
use core::cell::RefCell;
use ecdsa_vendored::hazmat::VerifyPrimitive;
use error::ValidationError;
use num_bigint::BigUint;
use p256::ecdsa::{signature::Verifier, VerifyingKey};

use sha2::Digest;
use sp_std::prelude::*;

// https://docs.rs/x509-parser/0.14.0/src/x509_parser/pem.rs.html#90-97
fn parse_pem(c: &str) -> Result<Vec<u8>, base64::DecodeError> {
    base64::decode(c)
}

fn parse_cert(serialized: &[u8]) -> Result<Certificate, ParseError> {
    let data = asn1::parse_single::<Certificate>(serialized)?;
    Ok(data)
}

fn parse_cert_payload(serialized: &[u8]) -> Result<&[u8], ParseError> {
    let payload = asn1::parse_single::<CertificateRawPayload>(serialized)?;

    Ok(payload.tbs_certificate.full_data())
}

/// The OID of the Attestation Extension to a X.509 certificate.
/// [See docs](https://source.android.com/docs/security/keystore/attestation#tbscertificate-sequence)
const KEY_ATTESTATION_OID: ObjectIdentifier = oid!(1, 3, 6, 1, 4, 1, 11129, 2, 1, 17);

fn extract_attestation<'a>(
    extensions: Option<SequenceOf<'a, Extension<'a>>>,
) -> Result<KeyDescription<'a>, ValidationError> {
    let extension = extensions
        .ok_or(ValidationError::ExtensionMissing())?
        .find(|e| e.extn_id == KEY_ATTESTATION_OID)
        .ok_or(ValidationError::ExtensionMissing())?;

    match peek_attestation_version(extension.extn_value)? {
        100 => Ok(asn1::parse_single::<KeyDescription>(extension.extn_value)?),
        v => Err(ValidationError::UnsupportedAttestationVersion(v)),
    }
}

const RSA_ALGORITHM: ObjectIdentifier = oid!(1, 2, 840, 113549, 1, 1, 11);
const ECDSA_ALGORITHM: ObjectIdentifier = oid!(1, 2, 840, 10045, 4, 3, 2);

const RSA_PBK: ObjectIdentifier = oid!(1, 2, 840, 113549, 1, 1, 1);
const ECDSA_PBK: ObjectIdentifier = oid!(1, 2, 840, 10045, 2, 1);

#[derive(Clone)]
enum PublicKey {
    RSA(RSAPbk),
    ECDSA(ECDSACurve),
}

#[derive(Clone)]
struct RSAPbk {
    exponent: BigUint,
    modulus: BigUint,
}

#[derive(Clone)]
enum ECDSACurve {
    CurveP256(VerifyingKey),
    CurveP384(p384::AffinePoint),
}

impl PublicKey {
    fn parse(info: &SubjectPublicKeyInfo) -> Result<Self, ValidationError> {
        match &info.algorithm.algorithm {
            &RSA_PBK => {
                let pbk = parse_rsa_pbk(info.subject_public_key.as_bytes())?;
                Ok(PublicKey::RSA(pbk))
            }
            &ECDSA_PBK => {
                let pbk_param = info
                    .algorithm
                    .parameters
                    .ok_or(ValidationError::MissingECDSAAlgorithmTyp())?;
                let typ = asn1::parse_single::<ObjectIdentifier>(pbk_param.full_data())?;
                match typ {
                    CURVE_P256 => {
                        let verifying_key =
                            VerifyingKey::from_sec1_bytes(&info.subject_public_key.as_bytes())
                                .or(Err(ValidationError::ParseP256PublicKey()))?;
                        Ok(PublicKey::ECDSA(ECDSACurve::CurveP256(verifying_key)))
                    }
                    CURVE_P384 => {
                        // the first byte tells us if compressed or not, we always assume uncompressed and ignore it.
                        let encoded = &info.subject_public_key.as_bytes()[1..];
                        let middle = encoded.len() / 2;
                        let point = p384::AffinePoint {
                            x: p384::FieldElement::from_be_slice(&encoded[..middle])?,
                            y: p384::FieldElement::from_be_slice(&encoded[middle..])?,
                            infinity: 0,
                        };
                        Ok(PublicKey::ECDSA(ECDSACurve::CurveP384(point)))
                    }
                    _ => Result::Err(ValidationError::UnsupportedSignatureAlgorithm())?,
                }
            }
            _ => Result::Err(ValidationError::UnsupportedPublicKeyAlgorithm()),
        }
    }
}

const CURVE_P256: ObjectIdentifier = oid!(1, 2, 840, 10045, 3, 1, 7);
const CURVE_P384: ObjectIdentifier = oid!(1, 3, 132, 0, 34);

fn validate<'a>(
    cert: Certificate<'a>,
    payload: &[u8],
    pbk: &PublicKey,
) -> Result<Null, ValidationError> {
    if cert.signature_algorithm.algorithm != cert.tbs_certificate.signature.algorithm {
        return Result::Err(ValidationError::SignatureMismatch());
    }

    match cert.signature_algorithm.algorithm {
        RSA_ALGORITHM => match pbk {
            PublicKey::RSA(pbk) => validate_rsa(&payload, cert.signature_value, &pbk),
            _ => Result::Err(ValidationError::UnsupportedPublicKeyAlgorithm()),
        },
        ECDSA_ALGORITHM => match pbk {
            PublicKey::ECDSA(pbk) => validate_ecdsa(&payload, cert.signature_value, &pbk),
            _ => Result::Err(ValidationError::UnsupportedPublicKeyAlgorithm()),
        },
        _ => Result::Err(ValidationError::UnsupportedSignatureAlgorithm()),
    }
}

fn validate_rsa(
    payload: &[u8],
    signature: BitString,
    pbk: &RSAPbk,
) -> Result<Null, ValidationError> {
    let computed = {
        let signature_num = BigUint::from_bytes_be(signature.as_bytes());
        let computed = signature_num.modpow(&pbk.exponent, &pbk.modulus);
        computed.to_bytes_be()
    };

    // read hash digest and consume hasher
    let hashed = &sha2::Sha256::digest(payload)[..];

    let unpadded = &computed[computed.len() - hashed.len()..];

    if hashed != unpadded {
        return Result::Err(ValidationError::InvalidSignature());
    }

    Ok(())
}

fn validate_ecdsa(
    payload: &[u8],
    signature: BitString,
    curve: &ECDSACurve,
) -> Result<Null, ValidationError> {
    match curve {
        ECDSACurve::CurveP256(verifying_key) => {
            let signature = p256::ecdsa::Signature::from_der(&signature.as_bytes())
                .or(Err(ValidationError::InvalidSignatureEncoding()))?;
            verifying_key
                .verify(payload, &signature)
                .or(Err(ValidationError::InvalidSignature()))?;
        }
        ECDSACurve::CurveP384(affine_point) => {
            let signature = ecdsa_vendored::Signature::from_der(&signature.as_bytes())
                .or(Err(ValidationError::InvalidSignatureEncoding()))?;

            let hashed = &sha2::Sha256::digest(payload);
            let mut padded: [u8; 48] = [0; 48];
            padded[16..].copy_from_slice(hashed);
            let payload = p384::FieldBytes::from_slice(&padded);

            affine_point
                .verify_prehashed(*payload, &signature)
                .or(Err(ValidationError::InvalidSignature()))?;
        }
    };

    Ok(())
}

fn parse_rsa_pbk(data: &[u8]) -> Result<RSAPbk, ParseError> {
    let pbk = asn1::parse_single::<RSAPublicKey>(data)?;
    Ok(RSAPbk {
        exponent: BigUint::from_bytes_be(pbk.exponent.as_bytes()),
        modulus: BigUint::from_bytes_be(pbk.modulus.as_bytes()),
    })
}

fn peek_attestation_version(data: &[u8]) -> Result<i64, ParseError> {
    let result: asn1::ParseResult<_> = asn1::parse(data, |d| {
        // as we are not reading the sequence to the end, the parser always returns an error result
        // therefore setup a cell to store the result and ignore result
        let attestation_version: RefCell<i64> = RefCell::from(0);
        let _: Result<_, ParseError> = d.read_element::<asn1::Sequence>()?.parse(|d| {
            *attestation_version.borrow_mut() = d.read_element::<i64>()?;
            // this gets always covered by parse error
            return Ok(());
        });

        Ok(attestation_version.into_inner())
    });
    result
}

/// Validates the chain by ensuring that
/// 
/// - the chain starts with a self-signed certificate at index 0 that matches one of the known [TRUSTED_ROOT_CERTS]
/// - that the root's contained public key signs the next certificate in the chain
/// - the next certificate's public key signs the next one and so on...
fn validate_certificate_chain(chain: Vec<&str>) -> Result<Null, ValidationError> {
    let first = chain.first().ok_or(ValidationError::EmptyChain())?;
    if !TRUSTED_ROOT_CERTS.contains(first) {
        return Err(ValidationError::UntrustedRoot());
    }
    chain.iter().try_fold::<_, _, Result<_, ValidationError>>(
        Option::<PublicKey>::None,
        |prev_pbk, cert| {
            let decoded = parse_pem(cert)?;
            let cert = parse_cert(&decoded)?;
            let payload = parse_cert_payload(&decoded)?;
            let current_pbk = PublicKey::parse(&cert.tbs_certificate.subject_public_key_info)?;
            validate(cert, payload, prev_pbk.as_ref().unwrap_or(&current_pbk))?;
            // it's crucial for security to pass on a non-null public key here,
            // otherwise self-signed certificates would get accepted later down the chain
            Ok(Some(current_pbk))
        },
    )?;

    Ok(())
}

const TRUSTED_ROOT_CERTS: &'static [&str] = &[
    r"MIIFYDCCA0igAwIBAgIJAOj6GWMU0voYMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTYwNTI2MTYyODUyWhcNMjYwNTI0MTYyODUyWjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaOBpjCBozAdBgNVHQ4EFgQUNmHhAHyIBQlRi0RsR/8aTMnqTxIwHwYDVR0jBBgwFoAUNmHhAHyIBQlRi0RsR/8aTMnqTxIwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAYYwQAYDVR0fBDkwNzA1oDOgMYYvaHR0cHM6Ly9hbmRyb2lkLmdvb2dsZWFwaXMuY29tL2F0dGVzdGF0aW9uL2NybC8wDQYJKoZIhvcNAQELBQADggIBACDIw41L3KlXG0aMiS//cqrG+EShHUGo8HNsw30W1kJtjn6UBwRM6jnmiwfBPb8VA91chb2vssAtX2zbTvqBJ9+LBPGCdw/E53Rbf86qhxKaiAHOjpvAy5Y3m00mqC0w/Zwvju1twb4vhLaJ5NkUJYsUS7rmJKHHBnETLi8GFqiEsqTWpG/6ibYCv7rYDBJDcR9W62BW9jfIoBQcxUCUJouMPH25lLNcDc1ssqvC2v7iUgI9LeoM1sNovqPmQUiG9rHli1vXxzCyaMTjwftkJLkf6724DFhuKug2jITV0QkXvaJWF4nUaHOTNA4uJU9WDvZLI1j83A+/xnAJUucIv/zGJ1AMH2boHqF8CY16LpsYgBt6tKxxWH00XcyDCdW2KlBCeqbQPcsFmWyWugxdcekhYsAWyoSf818NUsZdBWBaR/OukXrNLfkQ79IyZohZbvabO/X+MVT3rriAoKc8oE2Uws6DF+60PV7/WIPjNvXySdqspImSN78mflxDqwLqRBYkA3I75qppLGG9rp7UCdRjxMl8ZDBld+7yvHVgt1cVzJx9xnyGCC23UaicMDSXYrB4I4WHXPGjxhZuCuPBLTdOLU8YRvMYdEvYebWHMpvwGCF6bAx3JBpIeOQ1wDB5y0USicV3YgYGmi+NZfhA4URSh77Yd6uuJOJENRaNVTzk",
    r"MIIFHDCCAwSgAwIBAgIJANUP8luj8tazMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTkxMTIyMjAzNzU4WhcNMzQxMTE4MjAzNzU4WjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaNjMGEwHQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1UdIwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQBOMaBc8oumXb2voc7XCWnuXKhBBK3e2KMGz39t7lA3XXRe2ZLLAkLM5y3J7tURkf5a1SutfdOyXAmeE6SRo83Uh6WszodmMkxK5GM4JGrnt4pBisu5igXEydaW7qq2CdC6DOGjG+mEkN8/TA6p3cnoL/sPyz6evdjLlSeJ8rFBH6xWyIZCbrcpYEJzXaUOEaxxXxgYz5/cTiVKN2M1G2okQBUIYSY6bjEL4aUN5cfo7ogP3UvliEo3Eo0YgwuzR2v0KR6C1cZqZJSTnghIC/vAD32KdNQ+c3N+vl2OTsUVMC1GiWkngNx1OO1+kXW+YTnnTUOtOIswUP/Vqd5SYgAImMAfY8U9/iIgkQj6T2W6FsScy94IN9fFhE1UtzmLoBIuUFsVXJMTz+Jucth+IqoWFua9v1R93/k98p41pjtFX+H8DslVgfP097vju4KDlqN64xV1grw3ZLl4CiOe/A91oeLm2UHOq6wn3esB4r2EIQKb6jTVGu5sYCcdWpXr0AUVqcABPdgL+H7qJguBw09ojm6xNIrw2OocrDKsudk/okr/AwqEyPKw9WnMlQgLIKw1rODG2NvU9oR3GVGdMkUBZutL8VuFkERQGt6vQ2OCw0sV47VMkuYbacK/xyZFiRcrPJPb41zgbQj9XAEyLKCHex0SdDrx+tWUDqG8At2JHA==",
    r"MIIFHDCCAwSgAwIBAgIJAMNrfES5rhgxMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMjExMTE3MjMxMDQyWhcNMzYxMTEzMjMxMDQyWjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaNjMGEwHQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1UdIwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQBTNNZe5cuf8oiq+jV0itTGzWVhSTjOBEk2FQvh11J3o3lna0o7rd8RFHnN00q4hi6TapFhh4qaw/iG6Xg+xOan63niLWIC5GOPFgPeYXM9+nBb3zZzC8ABypYuCusWCmt6Tn3+Pjbz3MTVhRGXuT/TQH4KGFY4PhvzAyXwdjTOCXID+aHud4RLcSySr0Fq/L+R8TWalvM1wJJPhyRjqRCJerGtfBagiALzvhnmY7U1qFcS0NCnKjoO7oFedKdWlZz0YAfu3aGCJd4KHT0MsGiLZez9WP81xYSrKMNEsDK+zK5fVzw6jA7cxmpXcARTnmAuGUeI7VVDhDzKeVOctf3a0qQLwC+d0+xrETZ4r2fRGNw2YEs2W8Qj6oDcfPvq9JySe7pJ6wcHnl5EZ0lwc4xH7Y4Dx9RA1JlfooLMw3tOdJZH0enxPXaydfAD3YifeZpFaUzicHeLzVJLt9dvGB0bHQLE4+EqKFgOZv2EoP686DQqbVS1u+9k0p2xbMA105TBIk7npraa8VM0fnrRKi7wlZKwdH+aNAyhbXRW9xsnODJ+g8eF452zvbiKKngEKirK5LGieoXBX7tZ9D1GNBH2Ob3bKOwwIWdEFle/YF/h6zWgdeoaNGDqVBrLr2+0DtWoiB1aDEjLWl9FmyIUyUm7mD/vFDkzF+wm7cyWpQpCVQ==",
];

#[cfg(test)]
mod tests {
    use crate::attestation::error::ValidationError;

    use super::validate_certificate_chain;


    const SAMSUNG_ROOT_CERT: &str = r"MIIFHDCCAwSgAwIBAgIJANUP8luj8tazMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTkxMTIyMjAzNzU4WhcNMzQxMTE4MjAzNzU4WjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaNjMGEwHQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1UdIwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQBOMaBc8oumXb2voc7XCWnuXKhBBK3e2KMGz39t7lA3XXRe2ZLLAkLM5y3J7tURkf5a1SutfdOyXAmeE6SRo83Uh6WszodmMkxK5GM4JGrnt4pBisu5igXEydaW7qq2CdC6DOGjG+mEkN8/TA6p3cnoL/sPyz6evdjLlSeJ8rFBH6xWyIZCbrcpYEJzXaUOEaxxXxgYz5/cTiVKN2M1G2okQBUIYSY6bjEL4aUN5cfo7ogP3UvliEo3Eo0YgwuzR2v0KR6C1cZqZJSTnghIC/vAD32KdNQ+c3N+vl2OTsUVMC1GiWkngNx1OO1+kXW+YTnnTUOtOIswUP/Vqd5SYgAImMAfY8U9/iIgkQj6T2W6FsScy94IN9fFhE1UtzmLoBIuUFsVXJMTz+Jucth+IqoWFua9v1R93/k98p41pjtFX+H8DslVgfP097vju4KDlqN64xV1grw3ZLl4CiOe/A91oeLm2UHOq6wn3esB4r2EIQKb6jTVGu5sYCcdWpXr0AUVqcABPdgL+H7qJguBw09ojm6xNIrw2OocrDKsudk/okr/AwqEyPKw9WnMlQgLIKw1rODG2NvU9oR3GVGdMkUBZutL8VuFkERQGt6vQ2OCw0sV47VMkuYbacK/xyZFiRcrPJPb41zgbQj9XAEyLKCHex0SdDrx+tWUDqG8At2JHA==";
    const SAMSUNG_KEY_CERT: &str = r"MIIClzCCAj2gAwIBAgIBATAKBggqhkjOPQQDAjA5MQwwCgYDVQQMDANURUUxKTAnBgNVBAUTIGIyYzM3ZTM4MzI4ZDZhY2RmM2I2MDA2ZThhNzdmMDY0MB4XDTIxMTExNzIyNDcxMloXDTMxMTExNTIyNDcxMlowHzEdMBsGA1UEAxMUQW5kcm9pZCBLZXlzdG9yZSBLZXkwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAASDWA5xIavYEzjbcZneQy8gxkAo7nzJrSIqHbmPDy1kOFNWidIZLaKf86qLp73/n2VzK8qo5XsHexoC8wPaIcj8o4IBTjCCAUowggE2BgorBgEEAdZ5AgERBIIBJjCCASICAWQKAQECAWQKAQEEAAQAMGy/hT0IAgYBgddgKwm/hUVcBFowWDEyMDAEK2NvbS51YmluZXRpYy5hdHRlc3RlZC5leGVjdXRvci50ZXN0LnRlc3RuZXQCAQ4xIgQgvctFYPazxB2tkgZoFpwovh756knyPZjNjrLzeuRIj/kwgaGhBTEDAgECogMCAQOjBAICAQClBTEDAgEAqgMCAQG/g3cCBQC/hT4DAgEAv4VATDBKBCDnyVk+0qoHM1jC6eS+ScTwsvI1J6mtlFgzf0F3HTIMawEB/woBAAQgowcEEJQaU4V58HU/EPyCMBydcLlh8pR+qgnfWnuur+W/hUEFAgMB1MC/hUIFAgMDFdy/hU4GAgQBNInxv4VPBgIEATSJ8TAOBgNVHQ8BAf8EBAMCB4AwCgYIKoZIzj0EAwIDSAAwRQIgOQNrjHRHg9gcN6gFJFZHSjpIG1Gx1061FAEq3E9yUsgCIQD1FvhmjYsTWeQMQsj22ms/8dw9O3WsvE0y2AtrN0KWuw==";
    const SAMSUNG_INTERMEDIATE_1_CERT: &str = r"MIIB8zCCAXmgAwIBAgIQcH2ewbAt6vTdz/WwWLWu6zAKBggqhkjOPQQDAjA5MQwwCgYDVQQMDANURUUxKTAnBgNVBAUTIDgxYjU3ZmZmYjM3OTUxMjljZjNmYzUwZWNhMGNkMzljMB4XDTIxMTExNzIyNDcxMloXDTMxMTExNTIyNDcxMlowOTEMMAoGA1UEDAwDVEVFMSkwJwYDVQQFEyBiMmMzN2UzODMyOGQ2YWNkZjNiNjAwNmU4YTc3ZjA2NDBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABE3rCk6dqUilYhf1gsiVMFkOrEze/Ar318VMXFXDlOXDajQORIGWYVVtbcHYPNrews45k2CgHZg6ofN4lpONImyjYzBhMB0GA1UdDgQWBBRt1zXt/O233wIFRiNawaRD3KQPpTAfBgNVHSMEGDAWgBQNE845gvrI02p2mda2mk3SWwhGYjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwICBDAKBggqhkjOPQQDAgNoADBlAjEA0dNMiUn0+ftvhsFJP1byGMZkaWWOQbIOTItcQTrw29YV5FSjwZW7Ofrj8kR8WC4nAjB0yDVyt86uFrvWWzaa1EJmqR4L7PMUWf8yVey6KLrhQYMSGGhgief4pj3Hx6Eck6o=";
    const SAMSUNG_INTERMEDIATE_2_CERT: &str = r"MIIDlDCCAXygAwIBAgIRAJ3uw09QZQdXUqFIiXyf5uUwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEBRMQZjkyMDA5ZTg1M2I2YjA0NTAeFw0yMTExMTcyMjQ1MTBaFw0zMTExMTUyMjQ1MTBaMDkxDDAKBgNVBAwMA1RFRTEpMCcGA1UEBRMgODFiNTdmZmZiMzc5NTEyOWNmM2ZjNTBlY2EwY2QzOWMwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAARSfOriwm02QddIzGI1JpbUWTw93rtxu/BBMGpQopLCEsI1IMcO+YO75XEx5PJb0qpN0qZy4ZyohEOkXyqdD/KNkNCKWnhVk7wyyJCdnw35L8+adMpuHkp7Wc8nK14aXKKjYzBhMB0GA1UdDgQWBBQNE845gvrI02p2mda2mk3SWwhGYjAfBgNVHSMEGDAWgBQ2YeEAfIgFCVGLRGxH/xpMyepPEjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwICBDANBgkqhkiG9w0BAQsFAAOCAgEAVRzcron3lJ+sG5Jaqd9L2G33Dm/0/u0Ed+1jNJ7LrCLMKSHmEmoEiuNRKue2Tyv8UVb/Z9dENmC+gBqWkgOB6hxJ6lVcvIa38/CKNHBHr/Ras55+zZ68tQlpO6tdOVKUlfvlvI1BdpCv4qSEMpR9Zz4f4dzjEAbb24isT0PLcYvN0IrDELdCK+R+b+HaM5GrcFj1STv3uju/xHJnU6GeMdMPFf/rbMLNi1P6xVqdNUBGbKFx8J+px78z/Bcjq8Swt+uEoINvk/whROT8TQuzdccofx0hRFaoC1lgjRo8xgLlqFIyj0ICETuyYfEXbJwGgJczdS7ndte2SES4Rl3+NlYA2/mXjBUPnmGvJraOUZaw7ahIay7L7uUpvdJCHrlCDpRSLLCjuNss/sGn6bb3EDVGBaqzNRUBLNbsqrwKf8MbaJMhxOzHFlVXO1heFvmVdB+69Gkf0Kt2fK8N6VJIDGI9YoluItIbgJ/IqCicwLduxqMSXpPHEXf+f0lQH/AAP6Gz0aD4on3qTjPSl8p4LOqZSQoDqJKUukaXhMvgr/4u4E3ZX3EbxrF77hrML4NK4DfOj3LjLklPZZ3cLlMXzcSnMYvXkVU96qHqppyqjfioOZU2oSFQwPbXmKIYHVYJ2xIFBVy9ESQcqX04mevxMh1YHp+pTdMLXYE0EU+lB5Q=";

    const PIXEL_ROOT_CERT: &str = r"MIIFYDCCA0igAwIBAgIJAOj6GWMU0voYMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTYwNTI2MTYyODUyWhcNMjYwNTI0MTYyODUyWjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaOBpjCBozAdBgNVHQ4EFgQUNmHhAHyIBQlRi0RsR/8aTMnqTxIwHwYDVR0jBBgwFoAUNmHhAHyIBQlRi0RsR/8aTMnqTxIwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAYYwQAYDVR0fBDkwNzA1oDOgMYYvaHR0cHM6Ly9hbmRyb2lkLmdvb2dsZWFwaXMuY29tL2F0dGVzdGF0aW9uL2NybC8wDQYJKoZIhvcNAQELBQADggIBACDIw41L3KlXG0aMiS//cqrG+EShHUGo8HNsw30W1kJtjn6UBwRM6jnmiwfBPb8VA91chb2vssAtX2zbTvqBJ9+LBPGCdw/E53Rbf86qhxKaiAHOjpvAy5Y3m00mqC0w/Zwvju1twb4vhLaJ5NkUJYsUS7rmJKHHBnETLi8GFqiEsqTWpG/6ibYCv7rYDBJDcR9W62BW9jfIoBQcxUCUJouMPH25lLNcDc1ssqvC2v7iUgI9LeoM1sNovqPmQUiG9rHli1vXxzCyaMTjwftkJLkf6724DFhuKug2jITV0QkXvaJWF4nUaHOTNA4uJU9WDvZLI1j83A+/xnAJUucIv/zGJ1AMH2boHqF8CY16LpsYgBt6tKxxWH00XcyDCdW2KlBCeqbQPcsFmWyWugxdcekhYsAWyoSf818NUsZdBWBaR/OukXrNLfkQ79IyZohZbvabO/X+MVT3rriAoKc8oE2Uws6DF+60PV7/WIPjNvXySdqspImSN78mflxDqwLqRBYkA3I75qppLGG9rp7UCdRjxMl8ZDBld+7yvHVgt1cVzJx9xnyGCC23UaicMDSXYrB4I4WHXPGjxhZuCuPBLTdOLU8YRvMYdEvYebWHMpvwGCF6bAx3JBpIeOQ1wDB5y0USicV3YgYGmi+NZfhA4URSh77Yd6uuJOJENRaNVTzk";
    const PIXEL_INTERMEDIATE_2_CERT: &str = r"MIID1zCCAb+gAwIBAgIKA4gmZ2BliZaF9TANBgkqhkiG9w0BAQsFADAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MB4XDTE5MDgwOTIzMDMyM1oXDTI5MDgwNjIzMDMyM1owLzEZMBcGA1UEBRMQNTRmNTkzNzA1NDJmNWE5NTESMBAGA1UEDAwJU3Ryb25nQm94MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAE41Inb5v86kMBpfBCf6ZHjlcyCa5E/XYs+8V8u9RxNjFQnoAuoOlAU25U+iVwyihGFUaYB1UJKTsxALOVW0MXdosoa/b+JlHFmvbGsNszYAkKRkfHhg527MO4p9tc5XrMo4G2MIGzMB0GA1UdDgQWBBRpkLEMOwiK7ir4jDOHtCwS2t/DpjAfBgNVHSMEGDAWgBQ2YeEAfIgFCVGLRGxH/xpMyepPEjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwICBDBQBgNVHR8ESTBHMEWgQ6BBhj9odHRwczovL2FuZHJvaWQuZ29vZ2xlYXBpcy5jb20vYXR0ZXN0YXRpb24vY3JsLzhGNjczNEM5RkE1MDQ3ODkwDQYJKoZIhvcNAQELBQADggIBAFxZEyegsCSeytyUkYTJZR7R8qYXoXUWQ5h1Qp6b0h+H/SNl0NzedHAiwZQQ8jqzgP4c7w9HrrxEPCpFMd8+ykEBv5bWvDDf2HjtZzRlMRG154KgM1DMJgXhKLSKV+f/H+S/QQTeP3yprOavsBvdkgX6ELkYN6M3JXr7gpCvpFb6Ypz65Ud7FysAm/KNQ9zU0x7cvz3Btvz8ylw4p5dz04tanTzNgVLVHyX5kAcB2ftPvxMH4X/PXdx1lAmGPS8PsubCRGjJxdhRVOEEMYyxCuYLonuyUggOByZFaBw55WDoWGpkVQhnFi9L3p23VkWILLnq/07+GwoxL1vUAiQpjJHxNQYbjgTo+kxhjDP3uULAKPANGBE7+25VqVLMtdce4Eb5v9yFqgg+JtlL41RUWVS3DIEqxOMm/fB3A7t55TbUKf8dCZyBci2BcUWTx8K7VnQMy8gBMyu1SGleKPLIrBRSomDP5X8xGtwTLo3aAdY4+aSjEoimI6kX9bbIfhyDFpJxKaDRHzhCUdLfJrlCp2hEq5GWj0lT50hPLs0tbhh/l3LTtFhKyYbiB5vHXyB3P4gUui0WxyZnYdajUF+Tn8MW79qHhwhaXU9HnflE+dBh0smazOc+0xdwZZKXET+UFAUAMGiHvhuICCuWsY4SPKv8/715toeCoECHSMv08C9C";
    const PIXEL_INTERMEDIATE_1_CERT: &str = r"MIICMDCCAbegAwIBAgIKFZBYV0ZxdmNYNDAKBggqhkjOPQQDAjAvMRkwFwYDVQQFExA1NGY1OTM3MDU0MmY1YTk1MRIwEAYDVQQMDAlTdHJvbmdCb3gwHhcNMTkwNzI3MDE1MjE5WhcNMjkwNzI0MDE1MjE5WjAvMRkwFwYDVQQFExA5NzM1Mzc3OTM2ZDBkZDc0MRIwEAYDVQQMDAlTdHJvbmdCb3gwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAR2OZY6u30za18jjYs1Xv2zlaIrLM3me9okMo5Lv4Av76l/IE3YvbRQMyy15Wb3Wb3G/6+587x443R9/Ognjl8Co4G6MIG3MB0GA1UdDgQWBBRBPjyps0vHpRy7ASXAQhvmUa162DAfBgNVHSMEGDAWgBRpkLEMOwiK7ir4jDOHtCwS2t/DpjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwICBDBUBgNVHR8ETTBLMEmgR6BFhkNodHRwczovL2FuZHJvaWQuZ29vZ2xlYXBpcy5jb20vYXR0ZXN0YXRpb24vY3JsLzE1OTA1ODU3NDY3MTc2NjM1ODM0MAoGCCqGSM49BAMCA2cAMGQCMBeg3ziAoi6h1LPfvbbASk5WVdC6cL3IpaxIOycMHm1SDNqYALOtd1uujfzMeobs+AIwKJj5XySGe7MRL0QNtdrSd2nkK+fbjcUc8LKvVapDwRAC40CiTzllAy+aOnyDxrvb";
    const PIXEL_KEY_CERT: &str = r"MIICnDCCAkGgAwIBAgIBATAMBggqhkjOPQQDAgUAMC8xGTAXBgNVBAUTEDk3MzUzNzc5MzZkMGRkNzQxEjAQBgNVBAwMCVN0cm9uZ0JveDAiGA8yMDIyMDcwOTEwNTE1NVoYDzIwMjgwNTIzMjM1OTU5WjAfMR0wGwYDVQQDDBRBbmRyb2lkIEtleXN0b3JlIEtleTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABLIMHRVHdmJiPs9DAQSJgAbg+BwNsbrofLlqh8d3dARlnlhdPZBXuKL/iuYfQBoHj8dc9SyMQmjoEPk3mMcp6GKjggFWMIIBUjAOBgNVHQ8BAf8EBAMCB4AwggE+BgorBgEEAdZ5AgERBIIBLjCCASoCAQQKAQICASkKAQIECHRlc3Rhc2RmBAAwbL+FPQgCBgGB4pZhH7+FRVwEWjBYMTIwMAQrY29tLnViaW5ldGljLmF0dGVzdGVkLmV4ZWN1dG9yLnRlc3QudGVzdG5ldAIBDjEiBCC9y0Vg9rPEHa2SBmgWnCi+HvnqSfI9mM2OsvN65EiP+TCBoaEFMQMCAQKiAwIBA6MEAgIBAKUFMQMCAQCqAwIBAb+DdwIFAL+FPgMCAQC/hUBMMEoEIIec0/GOp24kTU1Kw7y5wzfBO0ZnGQsZA1r+JTZVAFDxAQH/CgEABCA/QTbuNYHmq6jqM3prQ9cD3h7KJB+bfyd+zfr/96jc8b+FQQUCAwHUwL+FQgUCAwMV3r+FTgYCBAE0ir2/hU8GAgQBNIq9MAwGCCqGSM49BAMCBQADRwAwRAIgM6YTzOmm7SUCakkrZR8Kxnw8AonU5HQxaMaQPi+qC9oCIDJM01xL8mldca0Sooho5pIyESki6vDjaZ9q3YEz1SjZ";
    
    const PIXEL_KEY_CERT_INVALID: &str = r"MIICnDCCAkGgAwIBAgIBATAMBggqhkjOPQQDAgUAMC8xGTAXBgNVBAUTEDk3MzUzNzc5MzZkMGRkNzQxEjAQBgNVBAwMCVN0cm9uZ0JveDAiGA8yMDIyMDcwOTEwNTE1NVoYDzIwMjgwNTIzMjM1OTU5WjAfMR0wGwYDVQQDDBRBbmRyb2lkIEtleXN0b3JlIEtleTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABLIMHRVHdmJiPs9DAQSJgAbg+BwNsbrofLlqh8d3dARlnlhdPZBXuKL/iuYfQBoHj8dc9SyMQmjoEPk3mMcp6GKjggFWMIIBUjAOBgNVHQ8BAf8EBAMCB4AwggE+BgorBgEEAdZ5AgERBIIBLjCCASoCAQQKAQICASkKAQIECHRlc3Rhc2RmBAAwbL+FPQgCBgGB4pZhH7+FRVwEWjBYMTIwMAQrY29tLnViaW5ldGljLmF0dGVzdGVkLmV4ZWN1dG9yLnRlc3QudGVzdG5ldAIBDjEiBCC9y0Vg9rPEHa2SBmgWnCi+HvnqSfI9mM2OsvN65EiP+TCBoaEFMQMCAQKiAwIBA6MEAgIBAKUFMQMCAQCqAwIBAb+DdwIFAL+FPgMCAQC/hUBMMEoEIIec0/GOp24kTU1Kw7y5wzfBO0ZnGQsZA1r+JTZVAFDxAQH/CgEABCA/QTbuNYHmq6jqM3prQ9cD3h7KJB+bfyd+zfr/96jc8b+FQQUCAwHUwL+FQgUCAwMV3r+FTgYCBAE0ir2/hU8GAgQBNIq9MAwGCCqGSM49BAMCBQADRwAwRAIgM6YTzOmm7SUCakkrZR8Kxnw8AonU5HQxaMaQPi+qC9oCIDJM01xL8mldca0Sooho5pIyESki6vDjaZ9q3YAz1SjZ";
    const PIXEL_ROOT_CERT_UNTRUSTED: &str = r"MIIFYDCCA0igAwIBAgIJAOj6GWMU0voYMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTYwNTI2MTYyODUyWhcNMjYwNTI0MTYyODUyWjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaOBpjCBozAdBgNVHQ4EFgQUNmHhAHyIBQlRi0RsR/8aTMnqTxIwHwYDVR0jBBgwFoAUNmHhAHyIBQlRi0RsR/8aTMnqTxIwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAYYwQAYDVR0fBDkwNzA1oDOgMYYvaHR0cHM6Ly9hbmRyb2lkLmdvb2dsZWFwaXMuY29tL2F0dGVzdGF0aW9uL2NybC8wDQYJKoZIhvcNAQELBQADggIBACDIw41L3KlXG0aMiS//cqrG+EShHUGo8HNsw30W1kJtjn6UBwRM6jnmiwfBPb8VA91chb2vssAtX2zbTvqBJ9+LBPGCdw/E53Rbf86qhxKaiAHOjpvAy5Y3m00mqC0w/Zwvju1twb4vhLaJ5NkUJYsUS7rmJKHHBnETLi8GFqiEsqTWpG/6ibYCv7rYDBJDcR9W62BW9jfIoBQcxUCUJouMPH25lLNcDc1ssqvC2v7iUgI9LeoM1sNovqPmQUiG9rHli1vXxzCyaMTjwftkJLkf6724DFhuKug2jITV0QkXvaJWF4nUaHOTNA4uJU9WDvZLI1j83A+/xnAJUucIv/zGJ1AMH2boHqF8CY16LpsYgBt6tKxxWH00XcyDCdW2KlBCeqbQPcsFmWyWugxdcekhYsAWyoSf818NUsZdBWBaR/OukXrNLfkQ79IyZohZbvabO/X+MVT3rriAoKc8oE2Uws6DF+60PV7/WIPjNvXySdqspImSN78mflxDqwLqRBYkA3I75qppLGG9rp7UCdRjxMl8ZDBld+7yvHVgt1cVzJx9xnyGCC23UaicMDSXYrB4I4WHXPGjxhZuCuPBLTdOLU8YRvMYdEvYebWHMpvwGCF6bAx3JBpIeOQ1wDB5y0USicV3YgYGmi+NZfhA4URSh77Yd6uuJOJENRaNVTzl";

    #[test]
    fn test_validate_samsung_chain() {
        let cert_chain = vec![
            SAMSUNG_ROOT_CERT,
            SAMSUNG_INTERMEDIATE_2_CERT,
            SAMSUNG_INTERMEDIATE_1_CERT,
            SAMSUNG_KEY_CERT,
        ];
        validate_certificate_chain(cert_chain).expect("validation failed");
    }

    #[test]
    fn test_validate_pixel_chain() {
        let cert_chain = vec![
            PIXEL_ROOT_CERT,
            PIXEL_INTERMEDIATE_2_CERT,
            PIXEL_INTERMEDIATE_1_CERT,
            PIXEL_KEY_CERT,
        ];
        validate_certificate_chain(cert_chain).expect("validation failed");
    }

    #[test]
    fn test_validate_pixel_invalid_signature_chain() {
        let cert_chain = vec![
            PIXEL_ROOT_CERT,
            PIXEL_INTERMEDIATE_2_CERT,
            PIXEL_INTERMEDIATE_1_CERT,
            PIXEL_KEY_CERT_INVALID,
        ];
        assert_eq!(validate_certificate_chain(cert_chain), Err(ValidationError::InvalidSignature()));
    }
    
    #[test]
    fn test_validate_pixel_untrusted_root_chain() {
        let cert_chain = vec![
            PIXEL_ROOT_CERT_UNTRUSTED,
            PIXEL_INTERMEDIATE_2_CERT,
            PIXEL_INTERMEDIATE_1_CERT,
            PIXEL_KEY_CERT_INVALID,
        ];
        assert_eq!(validate_certificate_chain(cert_chain), Err(ValidationError::UntrustedRoot()));
    }
}
