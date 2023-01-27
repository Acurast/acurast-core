use crate::{Config, Error};
use core::marker::PhantomData;
use frame_support::{
    dispatch::RawOrigin,
    pallet_prelude::Member,
    sp_runtime::{
        traits::{AccountIdConversion, Get, StaticLookup},
        DispatchError, Percent,
    },
    Never, PalletId, Parameter,
};

/// Asset barrier that allows to customize which asset can be used as reward.
pub trait AssetBarrier<Asset> {
    fn can_use_asset(asset: &Asset) -> bool;
}

impl<Asset> AssetBarrier<Asset> for () {
    fn can_use_asset(_asset: &Asset) -> bool {
        false
    }
}

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

    fn lock_reward(
        reward: Self::Reward,
        owner: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError>;
    fn pay_reward(
        reward: Self::Reward,
        target: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError>;
    fn pay_matcher_reward(
        reward: Self::Reward,
        matcher: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError>;
}

impl<T: frame_system::Config> RewardManager<T> for () {
    type Reward = ();

    fn lock_reward(
        _reward: Self::Reward,
        _owner: <<T>::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn pay_reward(
        _reward: Self::Reward,
        _target: <<T>::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn pay_matcher_reward(
        _reward: Self::Reward,
        _matcher: <<T>::Lookup as StaticLookup>::Source,
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

pub struct AssetRewardManager<Asset, Barrier, AssetSplit>(
    PhantomData<(Asset, Barrier, AssetSplit)>,
);
impl<T: Config, Asset, Barrier, AssetSplit> RewardManager<T>
    for AssetRewardManager<Asset, Barrier, AssetSplit>
where
    T: pallet_assets::Config,
    <T as pallet_assets::Config>::AssetId: TryInto<u32>,
    Asset: Parameter
        + Member
        + Reward<
            AssetId = <T as pallet_assets::Config>::AssetId,
            AssetAmount = <T as pallet_assets::Config>::Balance,
        >,
    Barrier: AssetBarrier<Asset>,
    AssetSplit: FeeManager,
{
    type Reward = Asset;

    fn lock_reward(
        reward: Self::Reward,
        owner: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        if !Barrier::can_use_asset(&reward) {
            Err(Error::<T>::AssetNotAllowedByBarrier)?;
        }
        let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();
        let raw_origin = RawOrigin::<T::AccountId>::Signed(pallet_account.clone());
        let pallet_origin: T::RuntimeOrigin = raw_origin.into();
        let (id, amount) = match (reward.try_get_asset_id(), reward.try_get_amount()) {
            (Ok(id), Ok(amount)) => (id, amount),
            (Err(_err), _) => Err(Error::<T>::InvalidAssetId)?,
            (_, Err(_err)) => Err(Error::<T>::InvalidAssetAmount)?,
        };

        // transfer funds from caller to pallet account for holding until fulfill is called
        // this is a privileged operation, hence the force_transfer call.
        // we could do an approve_transfer first, but this would require the assets pallet being
        // public which we can't do at the moment due to our statemint assets 1 to 1 integration
        pallet_assets::Pallet::<T>::force_transfer(
            pallet_origin,
            id.into(),
            owner,
            T::Lookup::unlookup(pallet_account),
            amount,
        )
    }

    fn pay_reward(
        reward: Self::Reward,
        target: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();
        let raw_origin = RawOrigin::<T::AccountId>::Signed(pallet_account.clone());
        let pallet_origin: T::RuntimeOrigin = raw_origin.into();
        let (id, amount) = match (reward.try_get_asset_id(), reward.try_get_amount()) {
            (Ok(id), Ok(amount)) => (id, amount),
            (Err(_err), _) => Err(Error::<T>::InvalidAssetId)?,
            (_, Err(_err)) => Err(Error::<T>::InvalidAssetAmount)?,
        };

        // Extract fee from the processor reward
        let fee_percentage = AssetSplit::get_fee_percentage(); // TODO: fee will be indexed by version in the future
        let fee = fee_percentage.mul_floor(amount);

        // Subtract the fee from the reward
        let reward_after_fee = amount - fee;

        // Transfer fees to Acurast fees manager account
        let fee_pallet_account: T::AccountId = AssetSplit::pallet_id().into_account_truncating();
        pallet_assets::Pallet::<T>::transfer(
            pallet_origin.clone(),
            id.into(),
            T::Lookup::unlookup(fee_pallet_account),
            fee,
        )?;

        // Transfer reward to the processor
        pallet_assets::Pallet::<T>::transfer(
            pallet_origin,
            id.into(),
            target,
            reward_after_fee.into(),
        )
    }

    fn pay_matcher_reward(
        remaining_reward: Self::Reward,
        matcher: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        let matcher_fee_percentage = AssetSplit::get_matcher_percentage(); // TODO: fee will be indexed by version in the future
        let amount = remaining_reward
            .try_get_amount()
            .map_err(|_| Error::<T>::InvalidAssetAmount)?;
        let mut r = remaining_reward.clone();
        r.with_amount(matcher_fee_percentage.mul_floor(amount))
            .map_err(|_| Error::<T>::InvalidAssetAmount)?;

        <Self as RewardManager<T>>::pay_reward(r, matcher)
    }
}
