use frame_support::{dispatch::Weight, sp_runtime::DispatchError};

use crate::Config;

/// Trait used to manage vesting stakes and accrued rewards.
pub trait VestingBalance<T: frame_system::Config + Config> {
    ///
    fn lock_stake(
        target: &T::AccountId,
        stake: <T as Config>::Balance,
    ) -> Result<(), DispatchError>;
    fn pay_accrued(
        target: &T::AccountId,
        accrued: <T as Config>::Balance,
    ) -> Result<(), DispatchError>;
    fn pay_kicker(
        target: &T::AccountId,
        accrued: <T as Config>::Balance,
    ) -> Result<(), DispatchError>;
    fn unlock_stake(
        target: &T::AccountId,
        stake: <T as Config>::Balance,
    ) -> Result<(), DispatchError>;
}

pub trait WeightInfo {
    fn vest() -> Weight;
    fn revest() -> Weight;
    fn divest() -> Weight;
    fn cooldown() -> Weight;
    fn kick_out() -> Weight;
    fn distribute_reward() -> Weight;
}

impl WeightInfo for () {
    fn vest() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn revest() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn divest() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn cooldown() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn kick_out() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn distribute_reward() -> Weight {
        Weight::from_parts(10_000, 0)
    }
}
