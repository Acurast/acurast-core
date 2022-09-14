#![cfg_attr(all(feature = "alloc", not(feature = "std"), not(test)), no_std)]

pub mod asn;
pub mod error;

use asn::*;
use asn1::{oid, BitString, ObjectIdentifier, ParseError, SequenceOf};
use core::cell::RefCell;
use ecdsa_vendored::hazmat::VerifyPrimitive;
use error::ValidationError;
use frame_support::{traits::ConstU32, BoundedVec};
use num_bigint::BigUint;
use p256::ecdsa::{signature::Verifier, VerifyingKey};

use sha2::Digest;
use sp_std::prelude::*;

pub const CHAIN_MAX_LENGTH: u32 = 5;
pub const CERT_MAX_LENGTH: u32 = 3000;
pub type CertificateInput = BoundedVec<u8, ConstU32<CERT_MAX_LENGTH>>;
pub type CertificateChainInput = BoundedVec<CertificateInput, ConstU32<CHAIN_MAX_LENGTH>>;

fn parse_cert(serialized: &[u8]) -> Result<Certificate, ParseError> {
    let data = asn1::parse_single::<Certificate>(serialized)?;
    Ok(data)
}

fn parse_cert_payload(serialized: &[u8]) -> Result<&[u8], ParseError> {
    let payload = asn1::parse_single::<CertificateRawPayload>(serialized)?;

    Ok(payload.tbs_certificate.full_data())
}

pub type CertificateId = (Vec<u8>, Vec<u8>);

pub fn unique_id<'a>(
    issuer: &Name,
    serial_number: &asn1::BigUint,
) -> Result<CertificateId, ValidationError> {
    let issuer_encoded = asn1::write_single(issuer).map_err(|_| ValidationError::InvalidIssuer)?;
    let serial_number_encoded = serial_number.as_bytes().to_vec();
    Ok((issuer_encoded, serial_number_encoded))
}

/// The OID of the Attestation Extension to a X.509 certificate.
/// [See docs](https://source.android.com/docs/security/keystore/attestation#tbscertificate-sequence)
pub const KEY_ATTESTATION_OID: ObjectIdentifier = oid!(1, 3, 6, 1, 4, 1, 11129, 2, 1, 17);

pub fn extract_attestation<'a>(
    extensions: Option<SequenceOf<'a, Extension<'a>>>,
) -> Result<KeyDescription, ValidationError> {
    let extension = extensions
        .ok_or(ValidationError::ExtensionMissing)?
        .find(|e| e.extn_id == KEY_ATTESTATION_OID)
        .ok_or(ValidationError::ExtensionMissing)?;

    let version = peek_attestation_version(extension.extn_value)?;

    match version {
        4 => {
            let parsed = asn1::parse_single::<KeyDescriptionV4>(extension.extn_value)
                .map_err(|_| ValidationError::ParseError)?;
            Ok(KeyDescription::V4(parsed))
        }
        100 => {
            let parsed = asn1::parse_single::<KeyDescriptionV100>(extension.extn_value)
                .map_err(|_| ValidationError::ParseError)?;
            Ok(KeyDescription::V100(parsed))
        }
        _ => Err(ValidationError::UnsupportedAttestationVersion),
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
                    .ok_or(ValidationError::MissingECDSAAlgorithmTyp)?;
                let typ = asn1::parse_single::<ObjectIdentifier>(pbk_param.full_data())?;
                match typ {
                    CURVE_P256 => {
                        let verifying_key =
                            VerifyingKey::from_sec1_bytes(&info.subject_public_key.as_bytes())
                                .or(Err(ValidationError::ParseP256PublicKey))?;
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
                    _ => Result::Err(ValidationError::UnsupportedSignatureAlgorithm)?,
                }
            }
            _ => Result::Err(ValidationError::UnsupportedPublicKeyAlgorithm),
        }
    }
}

const CURVE_P256: ObjectIdentifier = oid!(1, 2, 840, 10045, 3, 1, 7);
const CURVE_P384: ObjectIdentifier = oid!(1, 3, 132, 0, 34);

fn validate<'a>(
    cert: &Certificate<'a>,
    payload: &[u8],
    pbk: &PublicKey,
) -> Result<(), ValidationError> {
    if cert.signature_algorithm.algorithm != cert.tbs_certificate.signature.algorithm {
        return Err(ValidationError::SignatureMismatch);
    }

    match cert.signature_algorithm.algorithm {
        RSA_ALGORITHM => match pbk {
            PublicKey::RSA(pbk) => validate_rsa(&payload, &cert.signature_value, &pbk),
            _ => Err(ValidationError::UnsupportedPublicKeyAlgorithm),
        },
        ECDSA_ALGORITHM => match pbk {
            PublicKey::ECDSA(pbk) => validate_ecdsa(&payload, &cert.signature_value, &pbk),
            _ => Err(ValidationError::UnsupportedPublicKeyAlgorithm),
        },
        _ => Err(ValidationError::UnsupportedSignatureAlgorithm),
    }
}

fn validate_rsa(
    payload: &[u8],
    signature: &BitString,
    pbk: &RSAPbk,
) -> Result<(), ValidationError> {
    let computed = {
        let signature_num = BigUint::from_bytes_be(signature.as_bytes());
        let computed = signature_num.modpow(&pbk.exponent, &pbk.modulus);
        computed.to_bytes_be()
    };

    // read hash digest and consume hasher
    let hashed = &sha2::Sha256::digest(payload)[..];

    let unpadded = &computed[computed.len() - hashed.len()..];

    if hashed != unpadded {
        return Err(ValidationError::InvalidSignature);
    }

    Ok(())
}

fn validate_ecdsa(
    payload: &[u8],
    signature: &BitString,
    curve: &ECDSACurve,
) -> Result<(), ValidationError> {
    match curve {
        ECDSACurve::CurveP256(verifying_key) => {
            let signature = p256::ecdsa::Signature::from_der(&signature.as_bytes())
                .or(Err(ValidationError::InvalidSignatureEncoding))?;
            verifying_key
                .verify(payload, &signature)
                .or(Err(ValidationError::InvalidSignature))?;
        }
        ECDSACurve::CurveP384(affine_point) => {
            let signature = ecdsa_vendored::Signature::from_der(&signature.as_bytes())
                .or(Err(ValidationError::InvalidSignatureEncoding))?;

            let hashed = &sha2::Sha256::digest(payload);
            let mut padded: [u8; 48] = [0; 48];
            padded[16..].copy_from_slice(hashed);
            let payload = p384::FieldBytes::from_slice(&padded);

            affine_point
                .verify_prehashed(*payload, &signature)
                .or(Err(ValidationError::InvalidSignature))?;
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

pub fn peek_attestation_version(data: &[u8]) -> Result<i64, ParseError> {
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

pub fn validate_certificate_chain_root(
    chain: &CertificateChainInput,
) -> Result<(), ValidationError> {
    let first = chain.first().ok_or(ValidationError::ChainTooShort)?;
    if !TRUSTED_ROOT_CERTS.contains(&first.as_slice()) {
        return Err(ValidationError::UntrustedRoot);
    }
    Ok(())
}

/// Validates the chain by ensuring that
///
/// - the chain starts with a self-signed certificate at index 0 that matches one of the known [TRUSTED_ROOT_CERTS]
/// - that the root's contained public key signs the next certificate in the chain
/// - the next certificate's public key signs the next one and so on...
pub fn validate_certificate_chain<'a>(
    chain: &'a CertificateChainInput,
) -> Result<(Vec<CertificateId>, TBSCertificate<'a>), ValidationError> {
    let mut cert_ids = Vec::<CertificateId>::new();
    let fold_result = chain.iter().try_fold::<_, _, Result<_, ValidationError>>(
        (Option::<PublicKey>::None, Option::<Certificate>::None),
        |(prev_pbk, _), cert_data| {
            let cert = parse_cert(&cert_data)?;
            let payload = parse_cert_payload(&cert_data)?;
            let current_pbk = PublicKey::parse(&cert.tbs_certificate.subject_public_key_info)?;

            validate(&cert, payload, prev_pbk.as_ref().unwrap_or(&current_pbk))?;

            let unique_id = unique_id(
                &cert.tbs_certificate.issuer,
                &cert.tbs_certificate.serial_number,
            )?;
            cert_ids.push(unique_id);

            // it's crucial for security to pass on a non-null public key here,
            // otherwise self-signed certificates would get accepted later down the chain
            Ok((Some(current_pbk), Some(cert)))
        },
    )?;

    let last_cert = fold_result.1.ok_or(ValidationError::ChainTooShort)?;

    // if the chain is non-empty as ensured above, we know that we always have Some certificate in option
    Ok((cert_ids, last_cert.tbs_certificate))
}

/// The list of trusted root certificates, as decoded bytes arrays. [Source](https://developer.android.com/training/articles/security-key-attestation#root_certificate)
const TRUSTED_ROOT_CERTS: &'static [&[u8]] = &[
    // base64 equivalent: r"MIIFYDCCA0igAwIBAgIJAOj6GWMU0voYMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTYwNTI2MTYyODUyWhcNMjYwNTI0MTYyODUyWjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaOBpjCBozAdBgNVHQ4EFgQUNmHhAHyIBQlRi0RsR/8aTMnqTxIwHwYDVR0jBBgwFoAUNmHhAHyIBQlRi0RsR/8aTMnqTxIwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAYYwQAYDVR0fBDkwNzA1oDOgMYYvaHR0cHM6Ly9hbmRyb2lkLmdvb2dsZWFwaXMuY29tL2F0dGVzdGF0aW9uL2NybC8wDQYJKoZIhvcNAQELBQADggIBACDIw41L3KlXG0aMiS//cqrG+EShHUGo8HNsw30W1kJtjn6UBwRM6jnmiwfBPb8VA91chb2vssAtX2zbTvqBJ9+LBPGCdw/E53Rbf86qhxKaiAHOjpvAy5Y3m00mqC0w/Zwvju1twb4vhLaJ5NkUJYsUS7rmJKHHBnETLi8GFqiEsqTWpG/6ibYCv7rYDBJDcR9W62BW9jfIoBQcxUCUJouMPH25lLNcDc1ssqvC2v7iUgI9LeoM1sNovqPmQUiG9rHli1vXxzCyaMTjwftkJLkf6724DFhuKug2jITV0QkXvaJWF4nUaHOTNA4uJU9WDvZLI1j83A+/xnAJUucIv/zGJ1AMH2boHqF8CY16LpsYgBt6tKxxWH00XcyDCdW2KlBCeqbQPcsFmWyWugxdcekhYsAWyoSf818NUsZdBWBaR/OukXrNLfkQ79IyZohZbvabO/X+MVT3rriAoKc8oE2Uws6DF+60PV7/WIPjNvXySdqspImSN78mflxDqwLqRBYkA3I75qppLGG9rp7UCdRjxMl8ZDBld+7yvHVgt1cVzJx9xnyGCC23UaicMDSXYrB4I4WHXPGjxhZuCuPBLTdOLU8YRvMYdEvYebWHMpvwGCF6bAx3JBpIeOQ1wDB5y0USicV3YgYGmi+NZfhA4URSh77Yd6uuJOJENRaNVTzk"
    &[
        48, 130, 5, 96, 48, 130, 3, 72, 160, 3, 2, 1, 2, 2, 9, 0, 232, 250, 25, 99, 20, 210, 250,
        24, 48, 13, 6, 9, 42, 134, 72, 134, 247, 13, 1, 1, 11, 5, 0, 48, 27, 49, 25, 48, 23, 6, 3,
        85, 4, 5, 19, 16, 102, 57, 50, 48, 48, 57, 101, 56, 53, 51, 98, 54, 98, 48, 52, 53, 48, 30,
        23, 13, 49, 54, 48, 53, 50, 54, 49, 54, 50, 56, 53, 50, 90, 23, 13, 50, 54, 48, 53, 50, 52,
        49, 54, 50, 56, 53, 50, 90, 48, 27, 49, 25, 48, 23, 6, 3, 85, 4, 5, 19, 16, 102, 57, 50,
        48, 48, 57, 101, 56, 53, 51, 98, 54, 98, 48, 52, 53, 48, 130, 2, 34, 48, 13, 6, 9, 42, 134,
        72, 134, 247, 13, 1, 1, 1, 5, 0, 3, 130, 2, 15, 0, 48, 130, 2, 10, 2, 130, 2, 1, 0, 175,
        182, 199, 130, 43, 177, 167, 1, 236, 43, 180, 46, 139, 204, 84, 22, 99, 171, 239, 152, 47,
        50, 199, 127, 117, 49, 3, 12, 151, 82, 75, 27, 95, 232, 9, 251, 199, 42, 169, 69, 31, 116,
        60, 189, 154, 111, 19, 53, 116, 74, 165, 94, 119, 246, 182, 172, 53, 53, 238, 23, 194, 94,
        99, 149, 23, 221, 156, 146, 230, 55, 74, 83, 203, 254, 37, 143, 143, 251, 182, 253, 18,
        147, 120, 162, 42, 76, 169, 156, 69, 45, 71, 165, 159, 50, 1, 244, 65, 151, 202, 28, 205,
        126, 118, 47, 178, 245, 49, 81, 182, 254, 178, 255, 253, 43, 111, 228, 254, 91, 198, 189,
        158, 195, 75, 254, 8, 35, 157, 170, 252, 235, 142, 181, 168, 237, 43, 58, 205, 156, 94, 58,
        119, 144, 225, 181, 20, 66, 121, 49, 89, 133, 152, 17, 173, 158, 178, 169, 107, 189, 215,
        165, 124, 147, 169, 28, 65, 252, 205, 39, 214, 127, 214, 246, 113, 170, 11, 129, 82, 97,
        173, 56, 79, 163, 121, 68, 134, 70, 4, 221, 179, 216, 196, 249, 32, 161, 155, 22, 86, 194,
        241, 74, 214, 208, 60, 86, 236, 6, 8, 153, 4, 28, 30, 209, 165, 254, 109, 52, 64, 181, 86,
        186, 209, 208, 161, 82, 88, 156, 83, 229, 93, 55, 7, 98, 240, 18, 46, 239, 145, 134, 27,
        27, 14, 108, 76, 128, 146, 116, 153, 192, 233, 190, 192, 184, 62, 59, 193, 249, 60, 114,
        192, 73, 96, 75, 189, 47, 19, 69, 230, 44, 63, 142, 38, 219, 236, 6, 201, 71, 102, 243,
        193, 40, 35, 157, 79, 67, 18, 250, 216, 18, 56, 135, 224, 107, 236, 245, 103, 88, 59, 248,
        53, 90, 129, 254, 234, 186, 249, 154, 131, 200, 223, 62, 42, 50, 42, 252, 103, 43, 241, 32,
        177, 53, 21, 139, 104, 33, 206, 175, 48, 155, 110, 238, 119, 249, 136, 51, 176, 24, 218,
        161, 14, 69, 31, 6, 163, 116, 213, 7, 129, 243, 89, 8, 41, 102, 187, 119, 139, 147, 8, 148,
        38, 152, 231, 78, 11, 205, 36, 98, 138, 1, 194, 204, 3, 229, 31, 11, 62, 91, 74, 193, 228,
        223, 158, 175, 159, 246, 164, 146, 167, 124, 20, 131, 136, 40, 133, 1, 91, 66, 44, 230,
        123, 128, 184, 140, 155, 72, 225, 59, 96, 122, 181, 69, 199, 35, 255, 140, 68, 248, 242,
        211, 104, 185, 246, 82, 13, 49, 20, 94, 191, 158, 134, 42, 215, 29, 246, 163, 191, 210, 69,
        9, 89, 214, 83, 116, 13, 151, 161, 47, 54, 139, 19, 239, 102, 213, 208, 165, 74, 110, 47,
        93, 154, 111, 239, 68, 104, 50, 188, 103, 132, 71, 37, 134, 31, 9, 61, 208, 230, 243, 64,
        93, 168, 150, 67, 239, 15, 77, 105, 182, 66, 0, 81, 253, 185, 48, 73, 103, 62, 54, 149, 5,
        128, 211, 205, 244, 251, 208, 139, 197, 132, 131, 149, 38, 0, 99, 2, 3, 1, 0, 1, 163, 129,
        166, 48, 129, 163, 48, 29, 6, 3, 85, 29, 14, 4, 22, 4, 20, 54, 97, 225, 0, 124, 136, 5, 9,
        81, 139, 68, 108, 71, 255, 26, 76, 201, 234, 79, 18, 48, 31, 6, 3, 85, 29, 35, 4, 24, 48,
        22, 128, 20, 54, 97, 225, 0, 124, 136, 5, 9, 81, 139, 68, 108, 71, 255, 26, 76, 201, 234,
        79, 18, 48, 15, 6, 3, 85, 29, 19, 1, 1, 255, 4, 5, 48, 3, 1, 1, 255, 48, 14, 6, 3, 85, 29,
        15, 1, 1, 255, 4, 4, 3, 2, 1, 134, 48, 64, 6, 3, 85, 29, 31, 4, 57, 48, 55, 48, 53, 160,
        51, 160, 49, 134, 47, 104, 116, 116, 112, 115, 58, 47, 47, 97, 110, 100, 114, 111, 105,
        100, 46, 103, 111, 111, 103, 108, 101, 97, 112, 105, 115, 46, 99, 111, 109, 47, 97, 116,
        116, 101, 115, 116, 97, 116, 105, 111, 110, 47, 99, 114, 108, 47, 48, 13, 6, 9, 42, 134,
        72, 134, 247, 13, 1, 1, 11, 5, 0, 3, 130, 2, 1, 0, 32, 200, 195, 141, 75, 220, 169, 87, 27,
        70, 140, 137, 47, 255, 114, 170, 198, 248, 68, 161, 29, 65, 168, 240, 115, 108, 195, 125,
        22, 214, 66, 109, 142, 126, 148, 7, 4, 76, 234, 57, 230, 139, 7, 193, 61, 191, 21, 3, 221,
        92, 133, 189, 175, 178, 192, 45, 95, 108, 219, 78, 250, 129, 39, 223, 139, 4, 241, 130,
        119, 15, 196, 231, 116, 91, 127, 206, 170, 135, 18, 154, 136, 1, 206, 142, 155, 192, 203,
        150, 55, 155, 77, 38, 168, 45, 48, 253, 156, 47, 142, 237, 109, 193, 190, 47, 132, 182,
        137, 228, 217, 20, 37, 139, 20, 75, 186, 230, 36, 161, 199, 6, 113, 19, 46, 47, 6, 22, 168,
        132, 178, 164, 214, 164, 111, 250, 137, 182, 2, 191, 186, 216, 12, 18, 67, 113, 31, 86,
        235, 96, 86, 246, 55, 200, 160, 20, 28, 197, 64, 148, 38, 139, 140, 60, 125, 185, 148, 179,
        92, 13, 205, 108, 178, 171, 194, 218, 254, 226, 82, 2, 61, 45, 234, 12, 214, 195, 104, 190,
        163, 230, 65, 72, 134, 246, 177, 229, 139, 91, 215, 199, 48, 178, 104, 196, 227, 193, 251,
        100, 36, 185, 31, 235, 189, 184, 12, 88, 110, 42, 232, 54, 140, 132, 213, 209, 9, 23, 189,
        162, 86, 23, 137, 212, 104, 115, 147, 52, 14, 46, 37, 79, 86, 14, 246, 75, 35, 88, 252,
        220, 15, 191, 198, 112, 9, 82, 231, 8, 191, 252, 198, 39, 80, 12, 31, 102, 232, 30, 161,
        124, 9, 141, 122, 46, 155, 24, 128, 27, 122, 180, 172, 113, 88, 125, 52, 93, 204, 131, 9,
        213, 182, 42, 80, 66, 122, 166, 208, 61, 203, 5, 153, 108, 150, 186, 12, 93, 113, 233, 33,
        98, 192, 22, 202, 132, 159, 243, 95, 13, 82, 198, 93, 5, 96, 90, 71, 243, 174, 145, 122,
        205, 45, 249, 16, 239, 210, 50, 102, 136, 89, 110, 246, 155, 59, 245, 254, 49, 84, 247,
        174, 184, 128, 160, 167, 60, 160, 77, 148, 194, 206, 131, 23, 238, 180, 61, 94, 255, 88,
        131, 227, 54, 245, 242, 73, 218, 172, 164, 137, 146, 55, 191, 38, 126, 92, 67, 171, 2, 234,
        68, 22, 36, 3, 114, 59, 230, 170, 105, 44, 97, 189, 174, 158, 212, 9, 212, 99, 196, 201,
        124, 100, 48, 101, 119, 238, 242, 188, 117, 96, 183, 87, 21, 204, 156, 125, 198, 124, 134,
        8, 45, 183, 81, 168, 156, 48, 52, 151, 98, 176, 120, 35, 133, 135, 92, 241, 163, 198, 22,
        110, 10, 227, 193, 45, 55, 78, 45, 79, 24, 70, 243, 24, 116, 75, 216, 121, 181, 135, 50,
        155, 240, 24, 33, 122, 108, 12, 119, 36, 26, 72, 120, 228, 53, 192, 48, 121, 203, 69, 18,
        137, 197, 119, 98, 6, 6, 154, 47, 141, 101, 248, 64, 225, 68, 82, 135, 190, 216, 119, 171,
        174, 36, 226, 68, 53, 22, 141, 85, 60, 228,
    ],
    // base64 equivalent: r"MIIFHDCCAwSgAwIBAgIJANUP8luj8tazMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMTkxMTIyMjAzNzU4WhcNMzQxMTE4MjAzNzU4WjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaNjMGEwHQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1UdIwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQBOMaBc8oumXb2voc7XCWnuXKhBBK3e2KMGz39t7lA3XXRe2ZLLAkLM5y3J7tURkf5a1SutfdOyXAmeE6SRo83Uh6WszodmMkxK5GM4JGrnt4pBisu5igXEydaW7qq2CdC6DOGjG+mEkN8/TA6p3cnoL/sPyz6evdjLlSeJ8rFBH6xWyIZCbrcpYEJzXaUOEaxxXxgYz5/cTiVKN2M1G2okQBUIYSY6bjEL4aUN5cfo7ogP3UvliEo3Eo0YgwuzR2v0KR6C1cZqZJSTnghIC/vAD32KdNQ+c3N+vl2OTsUVMC1GiWkngNx1OO1+kXW+YTnnTUOtOIswUP/Vqd5SYgAImMAfY8U9/iIgkQj6T2W6FsScy94IN9fFhE1UtzmLoBIuUFsVXJMTz+Jucth+IqoWFua9v1R93/k98p41pjtFX+H8DslVgfP097vju4KDlqN64xV1grw3ZLl4CiOe/A91oeLm2UHOq6wn3esB4r2EIQKb6jTVGu5sYCcdWpXr0AUVqcABPdgL+H7qJguBw09ojm6xNIrw2OocrDKsudk/okr/AwqEyPKw9WnMlQgLIKw1rODG2NvU9oR3GVGdMkUBZutL8VuFkERQGt6vQ2OCw0sV47VMkuYbacK/xyZFiRcrPJPb41zgbQj9XAEyLKCHex0SdDrx+tWUDqG8At2JHA=="
    &[
        48, 130, 5, 28, 48, 130, 3, 4, 160, 3, 2, 1, 2, 2, 9, 0, 213, 15, 242, 91, 163, 242, 214,
        179, 48, 13, 6, 9, 42, 134, 72, 134, 247, 13, 1, 1, 11, 5, 0, 48, 27, 49, 25, 48, 23, 6, 3,
        85, 4, 5, 19, 16, 102, 57, 50, 48, 48, 57, 101, 56, 53, 51, 98, 54, 98, 48, 52, 53, 48, 30,
        23, 13, 49, 57, 49, 49, 50, 50, 50, 48, 51, 55, 53, 56, 90, 23, 13, 51, 52, 49, 49, 49, 56,
        50, 48, 51, 55, 53, 56, 90, 48, 27, 49, 25, 48, 23, 6, 3, 85, 4, 5, 19, 16, 102, 57, 50,
        48, 48, 57, 101, 56, 53, 51, 98, 54, 98, 48, 52, 53, 48, 130, 2, 34, 48, 13, 6, 9, 42, 134,
        72, 134, 247, 13, 1, 1, 1, 5, 0, 3, 130, 2, 15, 0, 48, 130, 2, 10, 2, 130, 2, 1, 0, 175,
        182, 199, 130, 43, 177, 167, 1, 236, 43, 180, 46, 139, 204, 84, 22, 99, 171, 239, 152, 47,
        50, 199, 127, 117, 49, 3, 12, 151, 82, 75, 27, 95, 232, 9, 251, 199, 42, 169, 69, 31, 116,
        60, 189, 154, 111, 19, 53, 116, 74, 165, 94, 119, 246, 182, 172, 53, 53, 238, 23, 194, 94,
        99, 149, 23, 221, 156, 146, 230, 55, 74, 83, 203, 254, 37, 143, 143, 251, 182, 253, 18,
        147, 120, 162, 42, 76, 169, 156, 69, 45, 71, 165, 159, 50, 1, 244, 65, 151, 202, 28, 205,
        126, 118, 47, 178, 245, 49, 81, 182, 254, 178, 255, 253, 43, 111, 228, 254, 91, 198, 189,
        158, 195, 75, 254, 8, 35, 157, 170, 252, 235, 142, 181, 168, 237, 43, 58, 205, 156, 94, 58,
        119, 144, 225, 181, 20, 66, 121, 49, 89, 133, 152, 17, 173, 158, 178, 169, 107, 189, 215,
        165, 124, 147, 169, 28, 65, 252, 205, 39, 214, 127, 214, 246, 113, 170, 11, 129, 82, 97,
        173, 56, 79, 163, 121, 68, 134, 70, 4, 221, 179, 216, 196, 249, 32, 161, 155, 22, 86, 194,
        241, 74, 214, 208, 60, 86, 236, 6, 8, 153, 4, 28, 30, 209, 165, 254, 109, 52, 64, 181, 86,
        186, 209, 208, 161, 82, 88, 156, 83, 229, 93, 55, 7, 98, 240, 18, 46, 239, 145, 134, 27,
        27, 14, 108, 76, 128, 146, 116, 153, 192, 233, 190, 192, 184, 62, 59, 193, 249, 60, 114,
        192, 73, 96, 75, 189, 47, 19, 69, 230, 44, 63, 142, 38, 219, 236, 6, 201, 71, 102, 243,
        193, 40, 35, 157, 79, 67, 18, 250, 216, 18, 56, 135, 224, 107, 236, 245, 103, 88, 59, 248,
        53, 90, 129, 254, 234, 186, 249, 154, 131, 200, 223, 62, 42, 50, 42, 252, 103, 43, 241, 32,
        177, 53, 21, 139, 104, 33, 206, 175, 48, 155, 110, 238, 119, 249, 136, 51, 176, 24, 218,
        161, 14, 69, 31, 6, 163, 116, 213, 7, 129, 243, 89, 8, 41, 102, 187, 119, 139, 147, 8, 148,
        38, 152, 231, 78, 11, 205, 36, 98, 138, 1, 194, 204, 3, 229, 31, 11, 62, 91, 74, 193, 228,
        223, 158, 175, 159, 246, 164, 146, 167, 124, 20, 131, 136, 40, 133, 1, 91, 66, 44, 230,
        123, 128, 184, 140, 155, 72, 225, 59, 96, 122, 181, 69, 199, 35, 255, 140, 68, 248, 242,
        211, 104, 185, 246, 82, 13, 49, 20, 94, 191, 158, 134, 42, 215, 29, 246, 163, 191, 210, 69,
        9, 89, 214, 83, 116, 13, 151, 161, 47, 54, 139, 19, 239, 102, 213, 208, 165, 74, 110, 47,
        93, 154, 111, 239, 68, 104, 50, 188, 103, 132, 71, 37, 134, 31, 9, 61, 208, 230, 243, 64,
        93, 168, 150, 67, 239, 15, 77, 105, 182, 66, 0, 81, 253, 185, 48, 73, 103, 62, 54, 149, 5,
        128, 211, 205, 244, 251, 208, 139, 197, 132, 131, 149, 38, 0, 99, 2, 3, 1, 0, 1, 163, 99,
        48, 97, 48, 29, 6, 3, 85, 29, 14, 4, 22, 4, 20, 54, 97, 225, 0, 124, 136, 5, 9, 81, 139,
        68, 108, 71, 255, 26, 76, 201, 234, 79, 18, 48, 31, 6, 3, 85, 29, 35, 4, 24, 48, 22, 128,
        20, 54, 97, 225, 0, 124, 136, 5, 9, 81, 139, 68, 108, 71, 255, 26, 76, 201, 234, 79, 18,
        48, 15, 6, 3, 85, 29, 19, 1, 1, 255, 4, 5, 48, 3, 1, 1, 255, 48, 14, 6, 3, 85, 29, 15, 1,
        1, 255, 4, 4, 3, 2, 2, 4, 48, 13, 6, 9, 42, 134, 72, 134, 247, 13, 1, 1, 11, 5, 0, 3, 130,
        2, 1, 0, 78, 49, 160, 92, 242, 139, 166, 93, 189, 175, 161, 206, 215, 9, 105, 238, 92, 168,
        65, 4, 173, 222, 216, 163, 6, 207, 127, 109, 238, 80, 55, 93, 116, 94, 217, 146, 203, 2,
        66, 204, 231, 45, 201, 238, 213, 17, 145, 254, 90, 213, 43, 173, 125, 211, 178, 92, 9, 158,
        19, 164, 145, 163, 205, 212, 135, 165, 172, 206, 135, 102, 50, 76, 74, 228, 99, 56, 36,
        106, 231, 183, 138, 65, 138, 203, 185, 138, 5, 196, 201, 214, 150, 238, 170, 182, 9, 208,
        186, 12, 225, 163, 27, 233, 132, 144, 223, 63, 76, 14, 169, 221, 201, 232, 47, 251, 15,
        203, 62, 158, 189, 216, 203, 149, 39, 137, 242, 177, 65, 31, 172, 86, 200, 134, 66, 110,
        183, 41, 96, 66, 115, 93, 165, 14, 17, 172, 113, 95, 24, 24, 207, 159, 220, 78, 37, 74, 55,
        99, 53, 27, 106, 36, 64, 21, 8, 97, 38, 58, 110, 49, 11, 225, 165, 13, 229, 199, 232, 238,
        136, 15, 221, 75, 229, 136, 74, 55, 18, 141, 24, 131, 11, 179, 71, 107, 244, 41, 30, 130,
        213, 198, 106, 100, 148, 147, 158, 8, 72, 11, 251, 192, 15, 125, 138, 116, 212, 62, 115,
        115, 126, 190, 93, 142, 78, 197, 21, 48, 45, 70, 137, 105, 39, 128, 220, 117, 56, 237, 126,
        145, 117, 190, 97, 57, 231, 77, 67, 173, 56, 139, 48, 80, 255, 213, 169, 222, 82, 98, 0, 8,
        152, 192, 31, 99, 197, 61, 254, 34, 32, 145, 8, 250, 79, 101, 186, 22, 196, 156, 203, 222,
        8, 55, 215, 197, 132, 77, 84, 183, 57, 139, 160, 18, 46, 80, 91, 21, 92, 147, 19, 207, 226,
        110, 114, 216, 126, 34, 170, 22, 22, 230, 189, 191, 84, 125, 223, 249, 61, 242, 158, 53,
        166, 59, 69, 95, 225, 252, 14, 201, 85, 129, 243, 244, 247, 187, 227, 187, 130, 131, 150,
        163, 122, 227, 21, 117, 130, 188, 55, 100, 185, 120, 10, 35, 158, 252, 15, 117, 161, 226,
        230, 217, 65, 206, 171, 172, 39, 221, 235, 1, 226, 189, 132, 33, 2, 155, 234, 52, 213, 26,
        238, 108, 96, 39, 29, 90, 149, 235, 208, 5, 21, 169, 192, 1, 61, 216, 11, 248, 126, 234,
        38, 11, 129, 195, 79, 104, 142, 110, 177, 52, 138, 240, 216, 234, 28, 172, 50, 172, 185,
        217, 63, 162, 74, 255, 3, 10, 132, 200, 242, 176, 245, 105, 204, 149, 8, 11, 32, 172, 53,
        172, 224, 198, 216, 219, 212, 246, 132, 119, 25, 81, 157, 50, 69, 1, 102, 235, 75, 241, 91,
        133, 144, 68, 80, 26, 222, 175, 67, 99, 130, 195, 75, 21, 227, 181, 76, 146, 230, 27, 105,
        194, 191, 199, 38, 69, 137, 23, 43, 60, 147, 219, 227, 92, 224, 109, 8, 253, 92, 1, 50, 44,
        160, 135, 123, 29, 18, 116, 58, 241, 250, 213, 148, 14, 161, 188, 2, 221, 137, 28,
    ],
    // base64 equivalent: r"MIIFHDCCAwSgAwIBAgIJAMNrfES5rhgxMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNVBAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMjExMTE3MjMxMDQyWhcNMzYxMTEzMjMxMDQyWjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdSSxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggjnar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGqC4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQoVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+OJtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/EgsTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRiigHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+MRPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9EaDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5UmAGMCAwEAAaNjMGEwHQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1UdIwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQBTNNZe5cuf8oiq+jV0itTGzWVhSTjOBEk2FQvh11J3o3lna0o7rd8RFHnN00q4hi6TapFhh4qaw/iG6Xg+xOan63niLWIC5GOPFgPeYXM9+nBb3zZzC8ABypYuCusWCmt6Tn3+Pjbz3MTVhRGXuT/TQH4KGFY4PhvzAyXwdjTOCXID+aHud4RLcSySr0Fq/L+R8TWalvM1wJJPhyRjqRCJerGtfBagiALzvhnmY7U1qFcS0NCnKjoO7oFedKdWlZz0YAfu3aGCJd4KHT0MsGiLZez9WP81xYSrKMNEsDK+zK5fVzw6jA7cxmpXcARTnmAuGUeI7VVDhDzKeVOctf3a0qQLwC+d0+xrETZ4r2fRGNw2YEs2W8Qj6oDcfPvq9JySe7pJ6wcHnl5EZ0lwc4xH7Y4Dx9RA1JlfooLMw3tOdJZH0enxPXaydfAD3YifeZpFaUzicHeLzVJLt9dvGB0bHQLE4+EqKFgOZv2EoP686DQqbVS1u+9k0p2xbMA105TBIk7npraa8VM0fnrRKi7wlZKwdH+aNAyhbXRW9xsnODJ+g8eF452zvbiKKngEKirK5LGieoXBX7tZ9D1GNBH2Ob3bKOwwIWdEFle/YF/h6zWgdeoaNGDqVBrLr2+0DtWoiB1aDEjLWl9FmyIUyUm7mD/vFDkzF+wm7cyWpQpCVQ=="
    &[
        48, 130, 5, 28, 48, 130, 3, 4, 160, 3, 2, 1, 2, 2, 9, 0, 195, 107, 124, 68, 185, 174, 24,
        49, 48, 13, 6, 9, 42, 134, 72, 134, 247, 13, 1, 1, 11, 5, 0, 48, 27, 49, 25, 48, 23, 6, 3,
        85, 4, 5, 19, 16, 102, 57, 50, 48, 48, 57, 101, 56, 53, 51, 98, 54, 98, 48, 52, 53, 48, 30,
        23, 13, 50, 49, 49, 49, 49, 55, 50, 51, 49, 48, 52, 50, 90, 23, 13, 51, 54, 49, 49, 49, 51,
        50, 51, 49, 48, 52, 50, 90, 48, 27, 49, 25, 48, 23, 6, 3, 85, 4, 5, 19, 16, 102, 57, 50,
        48, 48, 57, 101, 56, 53, 51, 98, 54, 98, 48, 52, 53, 48, 130, 2, 34, 48, 13, 6, 9, 42, 134,
        72, 134, 247, 13, 1, 1, 1, 5, 0, 3, 130, 2, 15, 0, 48, 130, 2, 10, 2, 130, 2, 1, 0, 175,
        182, 199, 130, 43, 177, 167, 1, 236, 43, 180, 46, 139, 204, 84, 22, 99, 171, 239, 152, 47,
        50, 199, 127, 117, 49, 3, 12, 151, 82, 75, 27, 95, 232, 9, 251, 199, 42, 169, 69, 31, 116,
        60, 189, 154, 111, 19, 53, 116, 74, 165, 94, 119, 246, 182, 172, 53, 53, 238, 23, 194, 94,
        99, 149, 23, 221, 156, 146, 230, 55, 74, 83, 203, 254, 37, 143, 143, 251, 182, 253, 18,
        147, 120, 162, 42, 76, 169, 156, 69, 45, 71, 165, 159, 50, 1, 244, 65, 151, 202, 28, 205,
        126, 118, 47, 178, 245, 49, 81, 182, 254, 178, 255, 253, 43, 111, 228, 254, 91, 198, 189,
        158, 195, 75, 254, 8, 35, 157, 170, 252, 235, 142, 181, 168, 237, 43, 58, 205, 156, 94, 58,
        119, 144, 225, 181, 20, 66, 121, 49, 89, 133, 152, 17, 173, 158, 178, 169, 107, 189, 215,
        165, 124, 147, 169, 28, 65, 252, 205, 39, 214, 127, 214, 246, 113, 170, 11, 129, 82, 97,
        173, 56, 79, 163, 121, 68, 134, 70, 4, 221, 179, 216, 196, 249, 32, 161, 155, 22, 86, 194,
        241, 74, 214, 208, 60, 86, 236, 6, 8, 153, 4, 28, 30, 209, 165, 254, 109, 52, 64, 181, 86,
        186, 209, 208, 161, 82, 88, 156, 83, 229, 93, 55, 7, 98, 240, 18, 46, 239, 145, 134, 27,
        27, 14, 108, 76, 128, 146, 116, 153, 192, 233, 190, 192, 184, 62, 59, 193, 249, 60, 114,
        192, 73, 96, 75, 189, 47, 19, 69, 230, 44, 63, 142, 38, 219, 236, 6, 201, 71, 102, 243,
        193, 40, 35, 157, 79, 67, 18, 250, 216, 18, 56, 135, 224, 107, 236, 245, 103, 88, 59, 248,
        53, 90, 129, 254, 234, 186, 249, 154, 131, 200, 223, 62, 42, 50, 42, 252, 103, 43, 241, 32,
        177, 53, 21, 139, 104, 33, 206, 175, 48, 155, 110, 238, 119, 249, 136, 51, 176, 24, 218,
        161, 14, 69, 31, 6, 163, 116, 213, 7, 129, 243, 89, 8, 41, 102, 187, 119, 139, 147, 8, 148,
        38, 152, 231, 78, 11, 205, 36, 98, 138, 1, 194, 204, 3, 229, 31, 11, 62, 91, 74, 193, 228,
        223, 158, 175, 159, 246, 164, 146, 167, 124, 20, 131, 136, 40, 133, 1, 91, 66, 44, 230,
        123, 128, 184, 140, 155, 72, 225, 59, 96, 122, 181, 69, 199, 35, 255, 140, 68, 248, 242,
        211, 104, 185, 246, 82, 13, 49, 20, 94, 191, 158, 134, 42, 215, 29, 246, 163, 191, 210, 69,
        9, 89, 214, 83, 116, 13, 151, 161, 47, 54, 139, 19, 239, 102, 213, 208, 165, 74, 110, 47,
        93, 154, 111, 239, 68, 104, 50, 188, 103, 132, 71, 37, 134, 31, 9, 61, 208, 230, 243, 64,
        93, 168, 150, 67, 239, 15, 77, 105, 182, 66, 0, 81, 253, 185, 48, 73, 103, 62, 54, 149, 5,
        128, 211, 205, 244, 251, 208, 139, 197, 132, 131, 149, 38, 0, 99, 2, 3, 1, 0, 1, 163, 99,
        48, 97, 48, 29, 6, 3, 85, 29, 14, 4, 22, 4, 20, 54, 97, 225, 0, 124, 136, 5, 9, 81, 139,
        68, 108, 71, 255, 26, 76, 201, 234, 79, 18, 48, 31, 6, 3, 85, 29, 35, 4, 24, 48, 22, 128,
        20, 54, 97, 225, 0, 124, 136, 5, 9, 81, 139, 68, 108, 71, 255, 26, 76, 201, 234, 79, 18,
        48, 15, 6, 3, 85, 29, 19, 1, 1, 255, 4, 5, 48, 3, 1, 1, 255, 48, 14, 6, 3, 85, 29, 15, 1,
        1, 255, 4, 4, 3, 2, 2, 4, 48, 13, 6, 9, 42, 134, 72, 134, 247, 13, 1, 1, 11, 5, 0, 3, 130,
        2, 1, 0, 83, 52, 214, 94, 229, 203, 159, 242, 136, 170, 250, 53, 116, 138, 212, 198, 205,
        101, 97, 73, 56, 206, 4, 73, 54, 21, 11, 225, 215, 82, 119, 163, 121, 103, 107, 74, 59,
        173, 223, 17, 20, 121, 205, 211, 74, 184, 134, 46, 147, 106, 145, 97, 135, 138, 154, 195,
        248, 134, 233, 120, 62, 196, 230, 167, 235, 121, 226, 45, 98, 2, 228, 99, 143, 22, 3, 222,
        97, 115, 61, 250, 112, 91, 223, 54, 115, 11, 192, 1, 202, 150, 46, 10, 235, 22, 10, 107,
        122, 78, 125, 254, 62, 54, 243, 220, 196, 213, 133, 17, 151, 185, 63, 211, 64, 126, 10, 24,
        86, 56, 62, 27, 243, 3, 37, 240, 118, 52, 206, 9, 114, 3, 249, 161, 238, 119, 132, 75, 113,
        44, 146, 175, 65, 106, 252, 191, 145, 241, 53, 154, 150, 243, 53, 192, 146, 79, 135, 36,
        99, 169, 16, 137, 122, 177, 173, 124, 22, 160, 136, 2, 243, 190, 25, 230, 99, 181, 53, 168,
        87, 18, 208, 208, 167, 42, 58, 14, 238, 129, 94, 116, 167, 86, 149, 156, 244, 96, 7, 238,
        221, 161, 130, 37, 222, 10, 29, 61, 12, 176, 104, 139, 101, 236, 253, 88, 255, 53, 197,
        132, 171, 40, 195, 68, 176, 50, 190, 204, 174, 95, 87, 60, 58, 140, 14, 220, 198, 106, 87,
        112, 4, 83, 158, 96, 46, 25, 71, 136, 237, 85, 67, 132, 60, 202, 121, 83, 156, 181, 253,
        218, 210, 164, 11, 192, 47, 157, 211, 236, 107, 17, 54, 120, 175, 103, 209, 24, 220, 54,
        96, 75, 54, 91, 196, 35, 234, 128, 220, 124, 251, 234, 244, 156, 146, 123, 186, 73, 235, 7,
        7, 158, 94, 68, 103, 73, 112, 115, 140, 71, 237, 142, 3, 199, 212, 64, 212, 153, 95, 162,
        130, 204, 195, 123, 78, 116, 150, 71, 209, 233, 241, 61, 118, 178, 117, 240, 3, 221, 136,
        159, 121, 154, 69, 105, 76, 226, 112, 119, 139, 205, 82, 75, 183, 215, 111, 24, 29, 27, 29,
        2, 196, 227, 225, 42, 40, 88, 14, 102, 253, 132, 160, 254, 188, 232, 52, 42, 109, 84, 181,
        187, 239, 100, 210, 157, 177, 108, 192, 53, 211, 148, 193, 34, 78, 231, 166, 182, 154, 241,
        83, 52, 126, 122, 209, 42, 46, 240, 149, 146, 176, 116, 127, 154, 52, 12, 161, 109, 116,
        86, 247, 27, 39, 56, 50, 126, 131, 199, 133, 227, 157, 179, 189, 184, 138, 42, 120, 4, 42,
        42, 202, 228, 177, 162, 122, 133, 193, 95, 187, 89, 244, 61, 70, 52, 17, 246, 57, 189, 219,
        40, 236, 48, 33, 103, 68, 22, 87, 191, 96, 95, 225, 235, 53, 160, 117, 234, 26, 52, 96,
        234, 84, 26, 203, 175, 111, 180, 14, 213, 168, 136, 29, 90, 12, 72, 203, 90, 95, 69, 155,
        34, 20, 201, 73, 187, 152, 63, 239, 20, 57, 51, 23, 236, 38, 237, 204, 150, 165, 10, 66,
        85,
    ],
];

#[cfg(test)]
mod tests {
    use crate::attestation::{error::ValidationError, extract_attestation};

    use super::{
        asn::KeyDescription, validate_certificate_chain, validate_certificate_chain_root,
        CertificateChainInput, CertificateInput,
    };

    pub fn decode_certificate_chain(chain: &Vec<&str>) -> CertificateChainInput {
        let decoded = chain
            .iter()
            .map(|cert_data| {
                CertificateInput::truncate_from(
                    base64::decode(&cert_data).expect("error decoding test input"),
                )
            })
            .collect::<Vec<CertificateInput>>();
        CertificateChainInput::truncate_from(decoded)
    }

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

    type Error = ();

    impl From<ValidationError> for Error {
        fn from(_: ValidationError) -> Self {
            ()
        }
    }

    #[test]
    fn test_validate_samsung_chain() -> Result<(), Error> {
        let chain = vec![
            SAMSUNG_ROOT_CERT,
            SAMSUNG_INTERMEDIATE_2_CERT,
            SAMSUNG_INTERMEDIATE_1_CERT,
            SAMSUNG_KEY_CERT,
        ];
        let decoded_chain = decode_certificate_chain(&chain);
        validate_certificate_chain_root(&decoded_chain)?;
        let (_, cert) = validate_certificate_chain(&decoded_chain)?;
        let key_description = extract_attestation(cert.extensions)?;
        match key_description {
            KeyDescription::V100(key_description) => {
                assert_eq!(key_description.attestation_version, 100)
            }
            _ => return Err(()),
        }
        Ok(())
    }

    #[test]
    fn test_validate_pixel_chain() -> Result<(), Error> {
        let chain = vec![
            PIXEL_ROOT_CERT,
            PIXEL_INTERMEDIATE_2_CERT,
            PIXEL_INTERMEDIATE_1_CERT,
            PIXEL_KEY_CERT,
        ];
        let decoded_chain = decode_certificate_chain(&chain);
        validate_certificate_chain_root(&decoded_chain).expect("validating root failed");
        let (_, cert) =
            validate_certificate_chain(&decoded_chain).expect("validating chain failed");
        let key_description = extract_attestation(cert.extensions)?;
        match key_description {
            KeyDescription::V4(key_description) => {
                assert_eq!(key_description.attestation_version, 4)
            }
            _ => return Err(()),
        }
        Ok(())
    }

    #[test]
    fn test_validate_pixel_invalid_signature_chain() -> Result<(), ()> {
        let chain = vec![
            PIXEL_ROOT_CERT,
            PIXEL_INTERMEDIATE_2_CERT,
            PIXEL_INTERMEDIATE_1_CERT,
            PIXEL_KEY_CERT_INVALID,
        ];
        let decoded_chain = decode_certificate_chain(&chain);
        validate_certificate_chain_root(&decoded_chain).expect("validating root failed");
        let res = validate_certificate_chain(&decoded_chain);
        match res {
            Err(e) => assert_eq!(e, ValidationError::InvalidSignature),
            _ => return Err(()),
        };
        Ok(())
    }

    #[test]
    fn test_validate_pixel_untrusted_root_chain() -> Result<(), ()> {
        let chain = vec![
            PIXEL_ROOT_CERT_UNTRUSTED,
            PIXEL_INTERMEDIATE_2_CERT,
            PIXEL_INTERMEDIATE_1_CERT,
            PIXEL_KEY_CERT_INVALID,
        ];
        let decoded_chain = decode_certificate_chain(&chain);
        let res = validate_certificate_chain_root(&decoded_chain);
        match res {
            Err(e) => assert_eq!(e, ValidationError::UntrustedRoot),
            _ => return Err(()),
        };
        Ok(())
    }
}
