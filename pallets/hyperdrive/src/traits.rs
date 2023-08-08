use frame_support::dispatch::fmt::Debug;
use frame_support::weights::Weight;
use pallet_acurast::ParameterBound;
use pallet_acurast_marketplace::RegistrationExtra;
use crate::{MessageIdentifier, ParsedAction};

pub trait Proof<Balance, AccountId, MaxAllowedSources, MaxSlots, Extra> where
    Balance: From<u128>,
    MaxAllowedSources: ParameterBound,
    MaxSlots: ParameterBound,
    Extra: From<RegistrationExtra<Balance, AccountId, MaxSlots>>
{
    type Error: Debug;

    fn calculate_root<T: crate::pallet::Config<I>, I: 'static>(self: &Self) -> Vec<u8>;
    fn message_id(self: &Self) -> Result<MessageIdentifier, Self::Error>;
    fn message(self: &Self) -> Result<ParsedAction<AccountId, MaxAllowedSources, Extra>, Self::Error>;
}

/// Weight functions needed for pallet_acurast_hyperdrive.
pub trait WeightInfo {
    fn update_state_transmitters(l: u32) -> Weight;
    fn submit_state_merkle_root() -> Weight;
    fn submit_message() -> Weight;
    fn update_target_chain_owner() -> Weight;
}

pub trait FromBytes {
    fn from_bytes(bytes: &[u8]) -> Self;
}