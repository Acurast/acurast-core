use core::marker::PhantomData;
use frame_support::{
    dispatch::RawOrigin,
    pallet_prelude::Member,
    sp_runtime::{
        traits::{AccountIdConversion, Get, StaticLookup},
        DispatchError, Percent,
    },
    PalletId, Parameter,
};

use crate::{Config, Reward, RewardManager};

pub trait AssetBarrier<Asset> {
    fn can_use_asset(asset: &Asset) -> bool;
}

impl<Asset> AssetBarrier<Asset> for () {
    fn can_use_asset(_asset: &Asset) -> bool {
        false
    }
}

// This trait provives methods for managing the fees.
pub trait FeeManager {
    fn get_fee_percentage() -> Percent;
    fn pallet_id() -> PalletId;
}

pub struct AssetRewardManager<Asset, Barrier, AssetSplit>(
    PhantomData<(Asset, Barrier, AssetSplit)>,
);
impl<T: Config, Asset, Barrier, AssetSplit> RewardManager<T>
    for AssetRewardManager<Asset, Barrier, AssetSplit>
where
    T: pallet_assets::Config,
    T::AssetId: TryInto<u32>,
    Asset: Parameter + Member + Reward<AssetId = T::AssetId, Balance = T::Balance>,
    Barrier: AssetBarrier<Asset>,
    AssetSplit: FeeManager,
{
    type Reward = Asset;

    fn lock_reward(
        reward: Self::Reward,
        owner: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        if !Barrier::can_use_asset(&reward) {
            return Err(DispatchError::Other("Invalid asset."));
        }
        let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();
        let raw_origin = RawOrigin::<T::AccountId>::Signed(pallet_account.clone());
        let pallet_origin: T::Origin = raw_origin.into();
        let (id, amount) = match (reward.try_get_asset_id(), reward.try_get_amount()) {
            (Ok(id), Ok(amount)) => (id, amount),
            (Err(_err), _) => return Err(DispatchError::Other("Invalid asset id.")),
            (_, Err(_err)) => return Err(DispatchError::Other("Invalid asset balance.")),
        };

        // transfer funds from caller to pallet account for holding until fulfill is called
        // this is a privileged operation, hence the force_transfer call.
        // we could do an approve_transfer first, but this would require the assets pallet being
        // public which we can't do at the moment due to our statemint assets 1 to 1 integration
        pallet_assets::Pallet::<T>::force_transfer(
            pallet_origin,
            id,
            owner,
            T::Lookup::unlookup(pallet_account),
            amount,
        )
    }

    fn pay_reward(
        reward: Self::Reward,
        target: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();
        let raw_origin = RawOrigin::<T::AccountId>::Signed(pallet_account.clone());
        let pallet_origin: T::Origin = raw_origin.into();
        let (id, amount) = match (reward.try_get_asset_id(), reward.try_get_amount()) {
            (Ok(id), Ok(amount)) => (id, amount),
            (Err(_err), _) => return Err(DispatchError::Other("Invalid asset id.")),
            (_, Err(_err)) => return Err(DispatchError::Other("Invalid asset balance.")),
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
            id,
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
}
