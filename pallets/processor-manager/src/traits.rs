use frame_support::{dispatch::Weight, pallet_prelude::DispatchResult, sp_runtime::DispatchError};

use crate::Config;

pub trait ManagerIdProvider<T: Config> {
    fn create_manager_id(id: T::ManagerId, owner: &T::AccountId) -> DispatchResult;
    fn manager_id_for(owner: &T::AccountId) -> Result<T::ManagerId, DispatchError>;
    fn owner_for(manager_id: T::ManagerId) -> Result<T::AccountId, DispatchError>;
}

pub trait ProcessorAssetRecovery<T: Config> {
    fn recover_assets(
        processor: &T::AccountId,
        destination_account: &T::AccountId,
    ) -> DispatchResult;
}

pub trait AdvertisementHandler<T: Config> {
    fn advertise_for(processor: &T::AccountId, advertisement: &T::Advertisement) -> DispatchResult;
}

impl<T: Config> AdvertisementHandler<T> for () {
    fn advertise_for(
        _processor: &T::AccountId,
        _advertisement: &T::Advertisement,
    ) -> DispatchResult {
        Ok(())
    }
}

pub trait WeightInfo {
    fn create_manager() -> Weight;
    fn update_processor_pairings() -> Weight;
    fn pair_with_manager() -> Weight;
    fn recover_funds() -> Weight;
    fn heartbeat() -> Weight;
    fn advertise_for() -> Weight;
}

impl WeightInfo for () {
    fn create_manager() -> Weight {
        Weight::from_ref_time(10_000)
    }

    fn update_processor_pairings() -> Weight {
        Weight::from_ref_time(10_000)
    }

    fn pair_with_manager() -> Weight {
        Weight::from_ref_time(10_000)
    }

    fn recover_funds() -> Weight {
        Weight::from_ref_time(10_000)
    }

    fn heartbeat() -> Weight {
        Weight::from_ref_time(10_000)
    }

    fn advertise_for() -> Weight {
        Weight::from_ref_time(10_000)
    }
}
