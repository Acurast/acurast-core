use frame_support::{
    pallet_prelude::*,
    sp_runtime::traits::{IdentifyAccount, MaybeDisplay, Verify},
    traits::IsType,
    BoundedVec,
};

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct ProcessorPairing<AccountId, Proof>
where
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord,
    Proof: Parameter + Member + Verify,
{
    pub processor: AccountId,
    pub data: BoundedVec<u8, ConstU32<64>>,
    pub proof: Proof,
}

impl<AccountId, Proof> ProcessorPairing<AccountId, Proof>
where
    AccountId: IsType<<<Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord,
    Proof: Parameter + Member + Verify,
{
    pub fn validate(&self) -> bool {
        self.proof
            .verify(self.data.as_ref(), &self.processor.clone().into())
    }
}
