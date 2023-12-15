use frame_support::{
    pallet_prelude::*,
    sp_runtime::traits::{IdentifyAccount, MaybeDisplay, Verify},
    traits::{IsType, UnixTime},
};
use sp_core::H256;

use acurast_common::ListUpdate;

use crate::Config;

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct ProcessorPairing<AccountId, Signature>
where
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay,
    Signature: Parameter + Member + Verify,
{
    pub account: AccountId,
    pub proof: Option<Proof<Signature>>,
}

impl<AccountId, Signature> ProcessorPairing<AccountId, Signature>
where
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay,
    Signature: Parameter + Member + Verify,
{
    pub fn new_with_proof(account: AccountId, timestamp: u128, signature: Signature) -> Self {
        Self {
            account,
            proof: Some(Proof {
                timestamp,
                signature,
            }),
        }
    }

    pub fn new(account: AccountId) -> Self {
        Self {
            account,
            proof: None,
        }
    }
}

impl<AccountId, Signature> ProcessorPairing<AccountId, Signature>
where
    AccountId: IsType<<<Signature as Verify>::Signer as IdentifyAccount>::AccountId>,
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord,
    Signature: Parameter + Member + Verify,
{
    pub fn validate_timestamp<T: Config>(&self) -> bool {
        if let Some(proof) = &self.proof {
            let now = T::UnixTime::now().as_millis();
            if let Some(diff) = now.checked_sub(proof.timestamp) {
                return proof.timestamp <= now && diff < T::PairingProofExpirationTime::get();
            }
        }
        false
    }

    pub fn validate_signature<T: Config>(
        &self,
        account_id: &AccountId,
        counter: T::Counter,
    ) -> bool {
        if let Some(proof) = &self.proof {
            let message = [
                b"<Bytes>".to_vec(),
                account_id.encode(),
                proof.timestamp.encode(),
                counter.encode(),
                b"</Bytes>".to_vec(),
            ]
            .concat();
            return proof
                .signature
                .verify(message.as_ref(), &self.account.clone().into());
        }

        false
    }
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct Proof<Signature>
where
    Signature: Parameter + Member + Verify,
{
    pub timestamp: u128,
    pub signature: Signature,
}

pub type ProcessorPairingUpdate<AccountId, Signature> =
    ListUpdate<ProcessorPairing<AccountId, Signature>>;

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct Version {
    /// Number representing the device's platform:
    /// 0: Android
    pub platform: u32,
    pub build_number: u32,
}

pub type BinaryHash = H256;
pub(crate) const MAX_LOCATION_LENGTH: u32 = 200;
pub type BinaryLocation = BoundedVec<u8, ConstU32<MAX_LOCATION_LENGTH>>;
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: Version,
    pub binary_location: BinaryLocation,
}
