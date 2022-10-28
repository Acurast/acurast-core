use super::Config;
use crate::{traits::FeeManager, Reward, RewardManager};
use frame_support::{
    dispatch::RawOrigin,
    sp_runtime::{DispatchError, traits::{AccountIdConversion, Get, StaticLookup}},
};
use xcm::latest::prelude::*;

pub struct StatemintRewardManager;
impl<T: Config> RewardManager<T> for StatemintRewardManager
where
    T: pallet_assets::Config,
    T::AssetId: TryFrom<u128>,
    T::Balance: TryFrom<u128>,
{
    type Reward = MultiAsset;

    fn lock_reward(
        asset: MultiAsset,
        owner: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();
        let raw_origin = RawOrigin::<T::AccountId>::Signed(pallet_account.clone());
        let pallet_origin: T::Origin = raw_origin.into();
        let (id, amount): (u128, u128) = match (asset.try_get_asset_id(), asset.try_get_amount()) {
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
        asset: MultiAsset,
        target: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();
        let raw_origin = RawOrigin::<T::AccountId>::Signed(pallet_account.clone());
        let pallet_origin: T::Origin = raw_origin.into();
        let (id, amount): (u128, u128) = match (asset.try_get_asset_id(), asset.try_get_amount()) {
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

impl Reward for MultiAsset {
    type AssetId = u128;
    type Balance = u128;
    type Error = ();

    fn try_get_asset_id(&self) -> Result<Self::AssetId, Self::Error> {
        return match self {
            MultiAsset {
                fun: _,
                id:
                    Concrete(MultiLocation {
                        parents: 1,
                        interior: X3(Parachain(1000), PalletInstance(50), GeneralIndex(id)),
                    }),
            } => Ok(*id),
            _ => return Err(()),
        };
    }

    fn try_get_amount(&self) -> Result<Self::Balance, Self::Error> {
        return match self {
            MultiAsset {
                fun: Fungible(amount),
                id: _,
            } => Ok(*amount),
            _ => return Err(()),
        };
    }
}
