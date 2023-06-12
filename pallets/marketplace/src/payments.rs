use crate::Config;
use core::marker::PhantomData;
use frame_support::traits::tokens::Balance;
use frame_support::{
    pallet_prelude::Member,
    sp_runtime::{
        traits::{AccountIdConversion, Get},
        DispatchError, Percent,
    },
    PalletId, Parameter,
};
use xcm::prelude::AssetId;

pub type RewardFor<T> = <<T as Config>::RewardManager as RewardManager<T>>::Reward;

/// Trait used to manage lock up and payments of rewards.
pub trait RewardManager<T: frame_system::Config> {
    type Reward: Parameter;

    fn lock_reward(reward: Self::Reward, owner: &T::AccountId) -> Result<(), DispatchError>;
    fn pay_reward(reward: Self::Reward, target: &T::AccountId) -> Result<(), DispatchError>;
    fn pay_matcher_reward(
        reward: Self::Reward,
        matcher: &T::AccountId,
    ) -> Result<(), DispatchError>;
}

impl<T: frame_system::Config> RewardManager<T> for () {
    type Reward = u128;

    fn lock_reward(_reward: Self::Reward, _owner: &T::AccountId) -> Result<(), DispatchError> {
        Ok(())
    }

    fn pay_reward(_reward: Self::Reward, _target: &T::AccountId) -> Result<(), DispatchError> {
        Ok(())
    }

    fn pay_matcher_reward(
        _reward: Self::Reward,
        _matcher: &T::AccountId,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}

// This trait provives methods for managing the fees.
pub trait FeeManager {
    fn get_fee_percentage() -> Percent;
    fn get_matcher_percentage() -> Percent;
    fn pallet_id() -> PalletId;
}

trait IsNativeAsset {
    fn is_native_asset(&self) -> bool;
}

impl IsNativeAsset for AssetId {
    fn is_native_asset(&self) -> bool {
        match self {
            AssetId::Concrete(multi_location) => multi_location.is_here(),
            _ => false,
        }
    }
}

pub struct AssetRewardManager<Reward, AssetSplit, Currency>(
    PhantomData<(Reward, AssetSplit, Currency)>,
);

impl<T, Reward, AssetSplit, Currency> RewardManager<T>
    for AssetRewardManager<Reward, AssetSplit, Currency>
where
    T: Config + frame_system::Config,
    Reward: Balance,
    AssetSplit: FeeManager,
    Currency: frame_support::traits::Currency<T::AccountId, Balance = Reward>,
    Currency::Balance: Member,
{
    type Reward = Reward;

    fn lock_reward(reward: Self::Reward, owner: &T::AccountId) -> Result<(), DispatchError> {
        let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();
        Currency::transfer(
            owner,
            &pallet_account,
            reward,
            frame_support::traits::ExistenceRequirement::KeepAlive,
        )?;

        Ok(())
    }

    fn pay_reward(reward: Self::Reward, target: &T::AccountId) -> Result<(), DispatchError> {
        let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();

        // Extract fee from the processor reward
        let fee_percentage = AssetSplit::get_fee_percentage(); // TODO: fee will be indexed by version in the future
        let fee = fee_percentage.mul_floor(reward);

        // Subtract the fee from the reward
        let reward_after_fee = reward - fee;

        // Transfer fees to Acurast fees manager account
        let fee_pallet_account: T::AccountId = AssetSplit::pallet_id().into_account_truncating();

        Currency::transfer(
            &pallet_account,
            &fee_pallet_account,
            fee,
            frame_support::traits::ExistenceRequirement::KeepAlive,
        )?;
        Currency::transfer(
            &pallet_account,
            target,
            reward_after_fee,
            frame_support::traits::ExistenceRequirement::KeepAlive,
        )?;

        Ok(())
    }

    fn pay_matcher_reward(
        remaining_reward: Self::Reward,
        matcher: &T::AccountId,
    ) -> Result<(), DispatchError> {
        let matcher_fee_percentage = AssetSplit::get_matcher_percentage(); // TODO: fee will be indexed by version in the future
        <Self as RewardManager<T>>::pay_reward(
            matcher_fee_percentage.mul_floor(remaining_reward),
            matcher,
        )
    }
}
