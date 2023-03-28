use crate::{Config, Error};
use core::marker::PhantomData;
use frame_support::{
    pallet_prelude::Member,
    sp_runtime::{
        traits::{AccountIdConversion, Get},
        DispatchError, Percent,
    },
    Never, PalletId, Parameter,
};
use pallet_acurast_assets_manager::traits::AssetValidator;
use xcm::{prelude::AssetId, v2::AssetId::Concrete};

pub type RewardFor<T> = <<T as Config>::RewardManager as RewardManager<T>>::Reward;

/// Trait representing the reward for the execution of a job.
pub trait Reward {
    type AssetId;
    type AssetAmount;
    type Error;

    /// Creates new reward with given amount.
    fn with_amount(&mut self, amount: Self::AssetAmount) -> Result<&Self, Self::Error>;
    /// Returns the reward asset id.
    fn try_get_asset_id(&self) -> Result<Self::AssetId, Self::Error>;
    /// Returns the reward amount.
    fn try_get_amount(&self) -> Result<Self::AssetAmount, Self::Error>;
}

impl Reward for () {
    type AssetId = Never;
    type AssetAmount = Never;
    type Error = ();

    fn with_amount(&mut self, _amount: Self::AssetAmount) -> Result<&Self, Self::Error> {
        Err(())
    }

    fn try_get_asset_id(&self) -> Result<Self::AssetId, Self::Error> {
        Err(())
    }

    fn try_get_amount(&self) -> Result<Self::AssetAmount, Self::Error> {
        Err(())
    }
}

/// Trait used to manage lock up and payments of rewards.
pub trait RewardManager<T: frame_system::Config> {
    type Reward: Parameter + Member + Reward;

    fn lock_reward(reward: &Self::Reward, owner: &T::AccountId) -> Result<(), DispatchError>;
    fn pay_reward(reward: &Self::Reward, target: &T::AccountId) -> Result<(), DispatchError>;
    fn pay_matcher_reward(
        reward: &Self::Reward,
        matcher: &T::AccountId,
    ) -> Result<(), DispatchError>;
}

impl<T: frame_system::Config> RewardManager<T> for () {
    type Reward = ();

    fn lock_reward(_reward: &Self::Reward, _owner: &T::AccountId) -> Result<(), DispatchError> {
        Ok(())
    }

    fn pay_reward(_reward: &Self::Reward, _target: &T::AccountId) -> Result<(), DispatchError> {
        Ok(())
    }

    fn pay_matcher_reward(
        _reward: &Self::Reward,
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
            Concrete(multi_location) => multi_location.is_here(),
            _ => false,
        }
    }
}

pub struct AssetRewardManager<Asset, AssetSplit, Currency, AssetTransfer>(
    PhantomData<(Asset, AssetSplit, Currency, AssetTransfer)>,
);

impl<T: Config, Asset, AssetSplit, Currency, AssetTransfer> RewardManager<T>
    for AssetRewardManager<Asset, AssetSplit, Currency, AssetTransfer>
where
    T: frame_system::Config,
    Asset: Parameter + Member + Reward<AssetId = AssetId, AssetAmount = Currency::Balance>,
    AssetSplit: FeeManager,
    Currency: frame_support::traits::Currency<T::AccountId>,
    AssetTransfer: pallet_acurast::AssetTransfer<
        AccountId = T::AccountId,
        AssetId = Asset::AssetId,
        Balance = Currency::Balance,
        Error = DispatchError,
    >,
{
    type Reward = Asset;

    fn lock_reward(reward: &Self::Reward, owner: &T::AccountId) -> Result<(), DispatchError> {
        let asset_id = reward
            .try_get_asset_id()
            .map_err(|_| Error::<T>::InvalidAssetId)?;

        let amount = reward
            .try_get_amount()
            .map_err(|_| Error::<T>::InvalidAssetAmount)?;

        let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();
        if asset_id.is_native_asset() {
            Currency::transfer(
                owner,
                &pallet_account,
                amount,
                frame_support::traits::ExistenceRequirement::KeepAlive,
            )?;
        } else {
            AssetTransfer::transfer(asset_id, owner, &pallet_account, amount)?;
        }

        Ok(())
    }

    fn pay_reward(reward: &Self::Reward, target: &T::AccountId) -> Result<(), DispatchError> {
        let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();

        let asset_id = reward
            .try_get_asset_id()
            .map_err(|_| Error::<T>::InvalidAssetId)?;

        let amount = reward
            .try_get_amount()
            .map_err(|_| Error::<T>::InvalidAssetAmount)?;

        // Extract fee from the processor reward
        let fee_percentage = AssetSplit::get_fee_percentage(); // TODO: fee will be indexed by version in the future
        let fee = fee_percentage.mul_floor(amount);

        // Subtract the fee from the reward
        let reward_after_fee = amount - fee;

        // Transfer fees to Acurast fees manager account
        let fee_pallet_account: T::AccountId = AssetSplit::pallet_id().into_account_truncating();

        if asset_id.is_native_asset() {
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
        } else {
            AssetTransfer::transfer(asset_id.clone(), &pallet_account, &fee_pallet_account, fee)?;
            AssetTransfer::transfer(asset_id, &pallet_account, target, reward_after_fee)?;
        }

        Ok(())
    }

    fn pay_matcher_reward(
        remaining_reward: &Self::Reward,
        matcher: &T::AccountId,
    ) -> Result<(), DispatchError> {
        let matcher_fee_percentage = AssetSplit::get_matcher_percentage(); // TODO: fee will be indexed by version in the future
        let amount = remaining_reward
            .try_get_amount()
            .map_err(|_| Error::<T>::InvalidAssetAmount)?;
        let mut r = remaining_reward.clone();
        r.with_amount(matcher_fee_percentage.mul_floor(amount))
            .map_err(|_| Error::<T>::InvalidAssetAmount)?;

        <Self as RewardManager<T>>::pay_reward(&r, matcher)
    }
}

impl<Asset, AssetSplit, Currency, AssetTransfer> AssetValidator<AssetId>
    for AssetRewardManager<Asset, AssetSplit, Currency, AssetTransfer>
where
    AssetTransfer: AssetValidator<AssetId>,
    DispatchError: From<AssetTransfer::Error>,
{
    type Error = DispatchError;

    fn validate(asset: &AssetId) -> Result<(), Self::Error> {
        if asset.is_native_asset() {
            Ok(())
        } else {
            Ok(AssetTransfer::validate(asset)?)
        }
    }
}
