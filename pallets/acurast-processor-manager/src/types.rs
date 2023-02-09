use frame_support::{
    pallet_prelude::*,
    sp_runtime::traits::{IdentifyAccount, MaybeDisplay, Verify},
    traits::IsType,
    BoundedVec,
};

use acurast_common::ListUpdate;

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct ProcessorPairing<AccountId, Signature>
where
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay,
    Signature: Parameter + Member + Verify,
{
    pub processor: AccountId,
    pub proof: Option<ProcessorPairingProof<Signature>>,
}

impl<AccountId, Signature> ProcessorPairing<AccountId, Signature>
where
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay,
    Signature: Parameter + Member + Verify,
{
    pub fn new_with_proof(
        processor: AccountId,
        message: BoundedVec<u8, ConstU32<64>>,
        signature: Signature,
    ) -> Self {
        Self {
            processor,
            proof: Some(ProcessorPairingProof::<Signature> { message, signature }),
        }
    }

    pub fn new(processor: AccountId) -> Self {
        Self {
            processor,
            proof: None,
        }
    }
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct ProcessorPairingProof<Signature>
where
    Signature: Parameter + Member + Verify,
{
    pub message: BoundedVec<u8, ConstU32<64>>,
    pub signature: Signature,
}

impl<AccountId, Signature> ProcessorPairing<AccountId, Signature>
where
    AccountId: IsType<<<Signature as Verify>::Signer as IdentifyAccount>::AccountId>,
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord,
    Signature: Parameter + Member + Verify,
{
    pub fn validate(&self) -> bool {
        if let Some(proof) = &self.proof {
            return proof
                .signature
                .verify(proof.message.as_ref(), &self.processor.clone().into());
        }

        false
    }
}

pub type ProcessorPairingUpdate<AccountId, Signature> =
    ListUpdate<ProcessorPairing<AccountId, Signature>>;
