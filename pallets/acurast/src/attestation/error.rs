#![cfg_attr(all(feature = "alloc", not(feature = "std"), not(test)), no_std)]

use asn1::ParseError;
use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]

pub enum ValidationError {
    ParseKeyDescription,
    ChainTooShort,
    ChainTooLong,
    DecodeError,
    ParseError,
    UntrustedRoot,
    ExtensionMissing,
    ParseExtension,
    UnsupportedAttestationVersion,
    ParseP256PublicKey,
    ParseP384PublicKey,
    MissingECDSAAlgorithmTyp,
    InvalidSignatureEncoding,
    InvalidSignature,
    UnsupportedSignatureAlgorithm,
    UnsupportedPublicKeyAlgorithm,
    InvalidIssuer,
    /// Specified signature algorithms do not match.
    ///
    /// The signature field in the sequence
    /// [tbsCertificate](https://www.rfc-editor.org/rfc/rfc5280#section-4.1.2.3)
    /// MUST contain the same algorithm identifier as the signatureAlgorithm
    /// field in the sequence
    /// [Certificate](https://www.rfc-editor.org/rfc/rfc5280#section-4.1.1.2).
    SignatureMismatch,
}

impl From<ParseError> for ValidationError {
    fn from(_: ParseError) -> Self {
        Self::ParseExtension
    }
}

impl From<p384::elliptic_curve::Error> for ValidationError {
    fn from(_: p384::elliptic_curve::Error) -> Self {
        Self::ParseP384PublicKey
    }
}
