use super::xcm_adapters::get_statemint_asset;
use super::Config;
use frame_support::{
    dispatch::RawOrigin,
    sp_runtime::{DispatchError, traits::{AccountIdConversion, Get, StaticLookup}},
};
use xcm::latest::prelude::*;
use crate::traits::FeeManager;

pub trait LockAndPayAsset<T: Config> {
    fn lock_asset(asset: MultiAsset, owner: <T::Lookup as StaticLookup>::Source) -> Result<(), DispatchError>;

    fn pay_asset(asset: MultiAsset, target: <T::Lookup as StaticLookup>::Source) -> Result<(), DispatchError>;
}

pub struct StatemintAssetTransactor;
impl<T: Config> LockAndPayAsset<T> for StatemintAssetTransactor
where
    T::AssetId: TryFrom<u128>,
    T::Balance: TryFrom<u128>,
{
    fn lock_asset(asset: MultiAsset, owner: <T::Lookup as StaticLookup>::Source) -> Result<(), DispatchError> {
        let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();
        let raw_origin = RawOrigin::<T::AccountId>::Signed(pallet_account.clone());
        let pallet_origin: T::Origin = raw_origin.into();

        let (id, amount) = get_statemint_asset(&asset).or(Err(DispatchError::Other("Asset not found.")))?;
        let (id, amount): (T::AssetId, T::Balance) = match (id.try_into(), amount.try_into()) {
            (Ok(id), Ok(amount)) => (id, amount),
            (Err(_err), _) => return Err(DispatchError::Other("Invalid asset id.")),
            (_, Err(_err)) => return Err(DispatchError::Other("Invalid asset balance."))
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

    fn pay_asset(asset: MultiAsset, target: <T::Lookup as StaticLookup>::Source) -> Result<(), DispatchError> {
        let pallet_account: T::AccountId = T::PalletId::get().into_account_truncating();
        let raw_origin = RawOrigin::<T::AccountId>::Signed(pallet_account.clone());
        let pallet_origin: T::Origin = raw_origin.into();

        let (id, amount) = get_statemint_asset(&asset).or(Err(DispatchError::Other("Asset not found.")))?;
        let (id, amount): (T::AssetId, T::Balance) = match (id.try_into(), amount.try_into()) {
            (Ok(id), Ok(amount)) => (id, amount),
            (Err(_err), _) => return Err(DispatchError::Other("Invalid asset id.")),
            (_, Err(_err)) => return Err(DispatchError::Other("Invalid asset balance."))
        };

        // Extract fee from the processor reward
        let fee_percentage = T::FeeManager::get_fee_percentage(); // TODO: fee will be indexed by version in the future
        let fee = fee_percentage.mul_floor(amount);

        // Subtract the fee from the reward
        let reward_after_fee = amount - fee;

        // Transfer fees to Acurast fees manager account
        let fee_pallet_account: T::AccountId = T::FeeManager::pallet_id().into_account_truncating();
        pallet_assets::Pallet::<T>::transfer(pallet_origin.clone(), id, T::Lookup::unlookup(fee_pallet_account), fee)?;

        // Transfer reward to the processor
        pallet_assets::Pallet::<T>::transfer(pallet_origin, id, target, reward_after_fee)
    }
}
