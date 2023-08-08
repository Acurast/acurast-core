use core::marker::PhantomData;
use derive_more::{Display, Error, From};
use frame_support::{BoundedVec, RuntimeDebug};
use frame_support::pallet_prelude::{ConstU32};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_std::vec::Vec;
use pallet_acurast::ParameterBound;
use pallet_acurast_marketplace::RegistrationExtra;
use crate::{MessageParser, MessageIdentifier, ParsedAction, traits};
use crate::chain::tezos::TezosValidationError;
use super::util::evm;

/// Errors returned by this crate.
#[derive(RuntimeDebug, Display, From)]
#[cfg_attr(feature = "std", derive(Error))]
pub enum EthereumValidationError {
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
pub struct EthereumProof {
    items: BoundedVec<BoundedVec<u8, ConstU32<1024>>, ConstU32<32>>,
    path: BoundedVec<u8, ConstU32<256>>,
    value: BoundedVec<u8, ConstU32<1024>>
}

impl<Balance, AccountId, MaxAllowedSources, MaxSlots, Extra> traits::Proof<Balance, AccountId, MaxAllowedSources, MaxSlots, Extra> for EthereumProof where
    Balance: From<u128>,
    MaxAllowedSources: ParameterBound,
    MaxSlots: ParameterBound,
    Extra: From<RegistrationExtra<Balance, AccountId, MaxSlots>>
{
    type Error = TezosValidationError;

	fn calculate_root<T: crate::pallet::Config<I>, I: 'static>(self: &Self) -> Vec<u8> {
        let proof_items = self.items.iter().map(|node| node.as_slice()).collect();
        let _ = evm::verify_proof(&proof_items, &[0u8; 32], &self.path, &self.value);
        [0u8; 32].to_vec()
	}

    fn message_id(self: &Self) -> Result<MessageIdentifier, Self::Error> {
        todo!()
    }

    fn message(self: &Self) -> Result<ParsedAction<AccountId, MaxAllowedSources, Extra>, Self::Error> {
        todo!()
    }
}


pub struct EthereumParser<Balance, ParsableAccountId, AccountId, MaxSlots, Extra>(
    PhantomData<(Balance, ParsableAccountId, AccountId, MaxSlots, Extra)>,
);
impl<Balance, ParsableAccountId, AccountId, MaxAllowedSources, MaxSlots, Extra>
    MessageParser<AccountId, MaxAllowedSources, Extra>
    for EthereumParser<Balance, ParsableAccountId, AccountId, MaxSlots, Extra>
where
    ParsableAccountId: TryFrom<Vec<u8>> + Into<AccountId>,
    Extra: From<RegistrationExtra<Balance, AccountId, MaxSlots>>,
    Balance: From<u128>,
    MaxAllowedSources: ParameterBound,
    MaxSlots: ParameterBound,
{
    type Error = EthereumValidationError;

    /// Parses an encoded key from Tezos representing a message identifier.
    fn parse_key(encoded: &[u8]) -> Result<MessageIdentifier, Self::Error> {
        Ok(0u128)
    }

    fn parse_value(encoded: &[u8]) -> Result<ParsedAction<AccountId, MaxAllowedSources, Extra>, Self::Error> {
        todo!()
    }
}
