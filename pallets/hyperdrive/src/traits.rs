use crate::{MessageIdentifier, ParsedAction};
use frame_support::weights::Weight;
use pallet_acurast::ParameterBound;
use pallet_acurast_marketplace::RegistrationExtra;
use sp_std::fmt::Debug;

pub trait Proof<Balance, AccountId, MaxAllowedSources, MaxSlots, Extra>
where
    Balance: From<u128>,
    MaxAllowedSources: ParameterBound,
    MaxSlots: ParameterBound,
    Extra: From<RegistrationExtra<Balance, AccountId, MaxSlots>>,
{
    type Error: Debug;

    fn calculate_root<T: crate::pallet::Config<I>, I: 'static>(
        self: &Self,
    ) -> Result<[u8; 32], Self::Error>;
    fn message_id(self: &Self) -> Result<MessageIdentifier, Self::Error>;
    fn message<T: crate::pallet::Config<I>, I: 'static>(
        self: &Self,
    ) -> Result<ParsedAction<AccountId, T, I, MaxAllowedSources, Extra>, Self::Error>;
}

/// Weight functions needed for pallet_acurast_hyperdrive.
pub trait WeightInfo {
    fn update_state_transmitters(l: u32) -> Weight;
    fn submit_state_merkle_root() -> Weight;
    fn submit_message() -> Weight;
    fn update_target_chain_owner() -> Weight;
    fn update_current_snapshot() -> Weight;
}
