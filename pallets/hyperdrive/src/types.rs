use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;
use sp_core::bounded::BoundedVec;
use sp_core::ConstU32;

pub const STATE_TRANSMITTER_UPDATES_MAX_LENGTH: u32 = 50;
pub type StateTransmitterUpdates<T> =
    BoundedVec<StateTransmitterUpdateFor<T>, ConstU32<STATE_TRANSMITTER_UPDATES_MAX_LENGTH>>;

pub type StateTransmitterUpdateFor<T> = StateTransmitterUpdate<
    <T as frame_system::Config>::AccountId,
    <T as frame_system::Config>::BlockNumber,
>;

/// This struct defines the transmitter activity window
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub struct ActivityWindow<BlockNumber> {
    /// From this block on, the transmitter is permitted to submit Merkle roots.
    pub start_block: BlockNumber,
    /// From this block on, the transmitter is not permitted to submit any Merkle root.
    pub end_block: BlockNumber,
}
impl<BlockNumber: From<u8>> Default for ActivityWindow<BlockNumber> {
    fn default() -> Self {
        Self {
            start_block: BlockNumber::from(0),
            end_block: BlockNumber::from(0),
        }
    }
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub enum StateTransmitterUpdate<AccountId, BlockNumber> {
    Add(AccountId, ActivityWindow<BlockNumber>),
    Remove(AccountId),
    Update(AccountId, ActivityWindow<BlockNumber>),
}
