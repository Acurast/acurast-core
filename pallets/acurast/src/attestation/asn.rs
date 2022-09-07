#![cfg_attr(all(feature = "alloc", not(feature = "std"), not(test)), no_std)]

use asn1::{
    Asn1Read, Asn1Write, BitString, Enumerated, Null, ObjectIdentifier, SequenceOf, SetOf, Tlv,
};
use sp_std::prelude::*;

#[derive(Asn1Read, Asn1Write)]
/// Represents the root structure of a [X.509 v3 certificate](https://www.rfc-editor.org/rfc/rfc5280#section-4.1)
/// See how to map these to [asn1 structs](https://docs.rs/asn1/0.11.0/asn1/#structs)
pub struct Certificate<'a> {
    // https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
    pub tbs_certificate: TBSCertificate<'a>,
    pub signature_algorithm: AlgorithmIdentifier<'a>,
    pub signature_value: BitString<'a>,
}

#[derive(Asn1Read, Asn1Write)]
/// As Certificate, represents the root structure of a [X.509 v3 certificate](https://www.rfc-editor.org/rfc/rfc5280#section-4.1).
/// This version does not decode the payload.
/// See how to map these to [asn1 structs](https://docs.rs/asn1/0.11.0/asn1/#structs)
pub struct CertificateRawPayload<'a> {
    // https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html
    pub tbs_certificate: Tlv<'a>,
    pub signature_algorithm: AlgorithmIdentifier<'a>,
    pub signature_value: BitString<'a>,
}

#[derive(Asn1Read, Asn1Write, Clone)]
/// [See RFC](https://www.rfc-editor.org/rfc/rfc5280#section-4.1.1.2)
pub struct AlgorithmIdentifier<'a> {
    pub algorithm: ObjectIdentifier,
    pub parameters: Option<Tlv<'a>>,
}

#[derive(Asn1Read, Asn1Write)]
pub struct TBSCertificate<'a> {
    #[explicit(0)]
    #[default(1u64)]
    pub version: u64,
    pub serial_number: asn1::BigUint<'a>,
    pub signature: AlgorithmIdentifier<'a>,
    // TODO
    pub issuer: Name<'a>,
    pub validity: Validity,
    // TODO
    pub subject: Name<'a>,
    pub subject_public_key_info: SubjectPublicKeyInfo<'a>,
    // If present, version MUST be v2 or v3
    #[implicit(1)]
    pub issuer_unique_id: Option<BitString<'a>>,
    // If present, version MUST be v2 or v3
    #[implicit(2)]
    pub subject_unique_id: Option<BitString<'a>>,
    // If present, version MUST be v3
    #[explicit(3)]
    pub extensions: Option<SequenceOf<'a, Extension<'a>>>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write, Clone)]
pub enum Name<'a> {
    RDNSequence(RDNSequence<'a>),
}

type RDNSequence<'a> = SequenceOf<'a, RelativeDistinguishedName<'a>>;

type RelativeDistinguishedName<'a> = SetOf<'a, AttributeTypeAndValue<'a>>;

#[derive(Asn1Read, Asn1Write)]
pub struct AttributeTypeAndValue<'a> {
    pub typ: ObjectIdentifier,
    /// defined by typ
    /// TODO https://www.rfc-editor.org/rfc/rfc5280#section-4.1.2.4
    pub value: Tlv<'a>,
}

#[derive(Asn1Read, Asn1Write)]
pub struct Validity {
    pub not_before: Time,
    pub not_after: Time,
}

#[derive(Asn1Read, Asn1Write)]
pub enum Time {
    UTCTime(asn1::UtcTime),
    GeneralizedTime(asn1::GeneralizedTime),
}

#[derive(Asn1Read, Asn1Write, Clone)]
pub struct SubjectPublicKeyInfo<'a> {
    pub algorithm: AlgorithmIdentifier<'a>,
    pub subject_public_key: BitString<'a>,
}

#[derive(Asn1Read, Asn1Write, Clone)]
pub struct Extension<'a> {
    pub extn_id: ObjectIdentifier,
    #[default(false)]
    pub critical: bool,
    /// contains the DER encoding of an ASN.1 value
    /// corresponding to the extension type identified by extnID
    pub extn_value: &'a [u8],
}

#[derive(Asn1Read, Asn1Write)]
pub struct KeyDescription<'a> {
    // TODO: parse correct version, not only Version 100
    // see https://developer.android.com/training/articles/security-key-attestation#certificate_schema
    // Note that it's probably necessary to peak parse the version
    pub attestation_version: i64,
    pub attestation_security_level: SecurityLevel,
    pub key_mint_version: i64,
    pub key_mint_security_level: SecurityLevel,
    pub attestation_challenge: &'a [u8],
    pub unique_id: &'a [u8],
    pub software_enforced: AuthorizationList<'a>,
    pub tee_enforced: AuthorizationList<'a>,
}

/// One of
/// Software (0),
/// TrustedEnvironment (1),
/// StrongBox (2)
pub type SecurityLevel = Enumerated;

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct AuthorizationList<'a> {
    #[explicit(1)]
    pub purpose: Option<SetOf<'a, i64>>,
    #[explicit(2)]
    pub algorithm: Option<i64>,
    #[explicit(3)]
    pub key_size: Option<i64>,
    #[explicit(5)]
    pub digest: Option<SetOf<'a, i64>>,
    #[explicit(6)]
    pub padding: Option<SetOf<'a, i64>>,
    #[explicit(10)]
    pub ec_curve: Option<i64>,
    #[explicit(200)]
    pub rsa_public_exponent: Option<i64>,
    #[explicit(203)]
    pub mgf_digest: Option<SetOf<'a, i64>>,
    #[explicit(303)]
    pub rollback_resistance: Option<Null>,
    #[explicit(305)]
    pub early_boot_only: Option<Null>,
    #[explicit(400)]
    pub active_date_time: Option<i64>,
    #[explicit(401)]
    pub origination_expire_date_time: Option<i64>,
    #[explicit(402)]
    pub usage_expire_date_time: Option<i64>,
    #[explicit(405)]
    pub usage_count_limit: Option<i64>,
    #[explicit(503)]
    pub no_auth_required: Option<Null>,
    #[explicit(504)]
    pub user_auth_type: Option<i64>,
    #[explicit(505)]
    pub auth_timeout: Option<i64>,
    #[explicit(506)]
    pub allow_while_on_body: Option<Null>,
    #[explicit(507)]
    pub trusted_user_presence_required: Option<Null>,
    #[explicit(508)]
    pub trusted_confirmation_required: Option<Null>,
    #[explicit(509)]
    pub unlocked_device_required: Option<Null>,
    #[explicit(701)]
    pub creation_date_time: Option<i64>,
    #[explicit(702)]
    pub origin: Option<i64>,
    #[explicit(704)]
    pub root_of_trust: Option<RootOfTrust<'a>>,
    #[explicit(705)]
    pub os_version: Option<i64>,
    #[explicit(706)]
    pub os_patch_level: Option<i64>,
    #[explicit(709)]
    pub attestation_application_id: Option<&'a [u8]>,
    #[explicit(710)]
    pub attestation_id_brand: Option<&'a [u8]>,
    #[explicit(711)]
    pub attestation_id_device: Option<&'a [u8]>,
    #[explicit(712)]
    pub attestation_id_product: Option<&'a [u8]>,
    #[explicit(713)]
    pub attestation_id_serial: Option<&'a [u8]>,
    #[explicit(714)]
    pub attestation_id_imei: Option<&'a [u8]>,
    #[explicit(715)]
    pub attestation_id_meid: Option<&'a [u8]>,
    #[explicit(716)]
    pub attestation_id_manufacturer: Option<&'a [u8]>,
    #[explicit(717)]
    pub attestation_id_model: Option<&'a [u8]>,
    #[explicit(718)]
    pub vendor_patch_level: Option<i64>,
    #[explicit(719)]
    pub boot_patch_level: Option<i64>,
    #[explicit(720)]
    pub device_unique_attestation: Option<Null>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct RootOfTrust<'a> {
    pub verified_boot_key: &'a [u8],
    pub device_locked: bool,
    pub verified_boot_state: VerifiedBootState,
    pub verified_boot_hash: &'a [u8],
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct RSAPublicKey<'a> {
    pub modulus: asn1::BigUint<'a>,
    pub exponent: asn1::BigUint<'a>,
}

#[derive(asn1::Asn1Read, asn1::Asn1Write)]
pub struct ECDSASignature<'a> {
    pub r: asn1::BigInt<'a>,
    pub s: asn1::BigInt<'a>,
}

/// One of Verified (0),
/// SelfSigned (1),
/// Unverified (2),
/// Failed (3)
pub type VerifiedBootState = Enumerated;
