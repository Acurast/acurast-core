use frame_support::pallet_prelude::*;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::Config;

pub type VestingFor<T> = Vesting<<T as Config>::Balance, <T as Config>::BlockNumber>;
pub type VesterStateFor<T> = VesterState<<T as Config>::Balance, <T as Config>::BlockNumber>;
pub type PoolStateFor<T> = PoolState<<T as Config>::Balance>;

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Vesting<Balance, BlockNumber> {
    pub stake: Balance,
    pub locking_period: BlockNumber,
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Copy, Clone, PartialEq, Eq)]
pub struct VesterState<Balance, BlockNumber> {
    pub locking_period: BlockNumber,
    pub weight: Balance,
    pub stake: Balance,
    pub accrued: Balance,
    pub s: Balance,
    pub cooldown_started: Option<BlockNumber>,
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Default)]
pub struct PoolState<Balance> {
    pub total_weight: Balance,
    pub total_stake: Balance,
    /// Sum `s = sum_k=0^t [reward_t / weight_t]` as a tuple `(upper, lower)` tracking range of possible value of s
    /// that we don't know exactly due to rounding of fixed point numbers.
    pub s: (Balance, Balance),
}

impl<Balance, BlockNumber> From<VesterState<Balance, BlockNumber>>
    for Vesting<Balance, BlockNumber>
{
    fn from(state: VesterState<Balance, BlockNumber>) -> Self {
        Vesting {
            stake: state.stake,
            locking_period: state.locking_period,
        }
    }
}

// /// Vesting states for defining transition operations [`VestingOps`].
// #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Copy)]
// pub enum VestingState {
//     VESTING,
//     COOLDOWN,
// }
//
// /// Vesting operations for that transition between various [`VestingState`].
// #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Copy)]
// pub enum VestingOps {
//     VEST,
//     COOLDOWN,
//     REVEST,
// }
