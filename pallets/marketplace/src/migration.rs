use frame_support::{traits::GetStorageVersion, weights::Weight};
use pallet_acurast::JobModules;
use sp_core::Get;

use super::*;

pub mod v1 {
    use frame_support::pallet_prelude::*;
    use pallet_acurast::MultiOrigin;
    use sp_std::prelude::*;

    /// The resource advertisement by a source containing the base restrictions.
    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub struct AdvertisementRestriction<AccountId> {
        /// Maximum memory in bytes not to be exceeded during any job's execution.
        pub max_memory: u32,
        /// Maximum network requests per second not to be exceeded.
        pub network_request_quota: u8,
        /// Storage capacity in bytes not to be exceeded in matching. The associated fee is listed in [pricing].
        pub storage_capacity: u32,
        /// An optional array of the [AccountId]s of consumers whose jobs should get accepted. If the array is [None], then jobs from all consumers are accepted.
        pub allowed_consumers: Option<Vec<MultiOrigin<AccountId>>>,
    }
}

pub fn migrate_to_v2<T: Config>() -> Weight {
    let onchain_version = Pallet::<T>::on_chain_storage_version();
    if onchain_version < crate::STORAGE_VERSION {
        StoredAdvertisementRestriction::<T>::translate_values::<
            v1::AdvertisementRestriction<T::AccountId>,
            _,
        >(|ad| {
            Some(AdvertisementRestriction {
                max_memory: ad.max_memory,
                network_request_quota: ad.network_request_quota,
                storage_capacity: ad.storage_capacity,
                allowed_consumers: ad.allowed_consumers,
                available_modules: JobModules::default(),
            })
        });
        STORAGE_VERSION.put::<Pallet<T>>();
        let count = StoredAdvertisementRestriction::<T>::iter_values().count() as u64;
        T::DbWeight::get().reads_writes(count + 1, count + 1)
    } else {
        Weight::zero()
    }
}
