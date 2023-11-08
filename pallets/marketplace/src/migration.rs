#![allow(deprecated)]

use frame_support::{
    pallet_prelude::*,
    traits::{GetStorageVersion, StorageVersion},
    weights::Weight,
};
use pallet_acurast::{
    job_registration_into, JobModules, JobRegistrationV4For, StoredJobRegistration,
};
use pallet_acurast::{MultiOrigin, ParameterBound};
use sp_core::Get;

use super::*;

pub mod v4 {
    use super::*;

    /// The resource advertisement by a source containing the base restrictions.
    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub struct AdvertisementRestriction<AccountId, MaxAllowedConsumers: ParameterBound> {
        /// Maximum memory in bytes not to be exceeded during any job's execution.
        pub max_memory: u32,
        /// Maximum network requests per second not to be exceeded.
        pub network_request_quota: u8,
        /// Storage capacity in bytes not to be exceeded in matching. The associated fee is listed in [pricing].
        pub storage_capacity: u32,
        /// An optional array of the [AccountId]s of consumers whose jobs should get accepted. If the array is [None], then jobs from all consumers are accepted.
        pub allowed_consumers: Option<BoundedVec<MultiOrigin<AccountId>, MaxAllowedConsumers>>,
        /// The modules available to the job on processor.
        pub available_modules: JobModules,
    }
}

pub fn migrate<T: Config>() -> Weight {
    let migrations: [(u16, &dyn Fn() -> Weight); 3] = [
        (3, &migrate_to_v3::<T>),
        (4, &migrate_to_v4::<T>),
        (5, &migrate_to_v5::<T>),
    ];

    let onchain_version = Pallet::<T>::on_chain_storage_version();
    let mut weight: Weight = Default::default();
    for (i, f) in migrations.into_iter() {
        if onchain_version < StorageVersion::new(i) {
            weight += f();
        }
    }

    STORAGE_VERSION.put::<Pallet<T>>();
    weight + T::DbWeight::get().writes(1)
}

fn migrate_to_v3<T: Config>() -> Weight {
    let mut count = 0u32;
    // we know they are reasonably few items and we can clear them within a single migration
    count += StoredJobStatus::<T>::clear(10_000, None).loops;
    count += StoredAdvertisementRestriction::<T>::clear(10_000, None).loops;
    count += StoredAdvertisementPricing::<T>::clear(10_000, None).loops;
    count += StoredStorageCapacity::<T>::clear(10_000, None).loops;
    count += StoredReputation::<T>::clear(10_000, None).loops;
    count += StoredMatches::<T>::clear(10_000, None).loops;

    T::DbWeight::get().writes((count + 1).into())
}

fn migrate_to_v4<T: Config>() -> Weight {
    // clear again all storages since we want to clear at the same time as pallet acurast for consistent state
    migrate_to_v3::<T>()
}

fn migrate_to_v5<T: Config>() -> Weight {
    let mut count: u64 = 0;

    StoredJobRegistration::<T>::translate_values::<JobRegistrationV4For<T>, _>(|job| {
        // two hops into(): first translate to the JobRegistrationV4 associated type, ...
        let job: JobRegistrationV4For<T> = job.into();
        // ...then translate into the new extra type with more fields
        Some(job_registration_into(job).into())
    });
    count += StoredJobRegistration::<T>::iter_values().count() as u64;

    StoredAdvertisementRestriction::<T>::translate_values::<
        v4::AdvertisementRestriction<T::AccountId, T::MaxAllowedConsumers>,
        _,
    >(|ad| {
        Some(AdvertisementRestriction {
            max_memory: ad.max_memory,
            network_request_quota: ad.network_request_quota,
            storage_capacity: ad.storage_capacity,
            allowed_consumers: ad.allowed_consumers,
            available_modules: ad.available_modules,
            source_location: Default::default(),
        })
    });
    count += StoredJobRegistration::<T>::iter_values().count() as u64;

    T::DbWeight::get().reads_writes(count + 1, count + 1)
}
