use core::marker::PhantomData;
use frame_support::{
    dispatch::RawOrigin,
    pallet_prelude::Member,
    sp_runtime::{
        traits::{AccountIdConversion, Get, StaticLookup},
        DispatchError,
    },
    Parameter,
};

use super::Config;
use crate::{traits::FeeManager, Reward, RewardManager};

pub trait AssetBarrier<Asset> {
    fn can_use_asset(asset: &Asset) -> bool;
}

impl<Asset> AssetBarrier<Asset> for () {
    fn can_use_asset(_asset: &Asset) -> bool {
        false
    }
}

pub struct AssetRewardManager<Asset, Barrier>(PhantomData<(Asset, Barrier)>);
impl<T: Config, Asset, Barrier> RewardManager<T> for AssetRewardManager<Asset, Barrier>
where
    T: pallet_assets::Config,
    T::AssetId: TryInto<u32>,
    Asset: Parameter + Member + Reward,
    Asset::AssetId: TryInto<T::AssetId>,
    Asset::Balance: TryInto<T::Balance>,
    Barrier: AssetBarrier<Asset>,
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
            id.try_into()
                .map_err(|_| DispatchError::Other("Invalid asset id."))?,
            owner,
            T::Lookup::unlookup(pallet_account),
            amount
                .try_into()
                .map_err(|_| DispatchError::Other("Invalid asset balance."))?,
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
        let id: T::AssetId = id
            .try_into()
            .map_err(|_| DispatchError::Other("Invalid asset id."))?;
        let amount: T::Balance = amount
            .try_into()
            .map_err(|_| DispatchError::Other("Invalid asset balance."))?;

        // Extract fee from the processor reward
        let fee_percentage = T::FeeManager::get_fee_percentage(); // TODO: fee will be indexed by version in the future
        let fee = fee_percentage.mul_floor(amount);

        // Subtract the fee from the reward
        let reward_after_fee = amount - fee;

        // Transfer fees to Acurast fees manager account
        let fee_pallet_account: T::AccountId = T::FeeManager::pallet_id().into_account_truncating();
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
