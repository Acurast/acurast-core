#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
pub use traits::*;
pub use types::*;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;

pub mod payments;
mod traits;
pub mod types;
mod utils;
pub mod weights;

pub use payments::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        dispatch::DispatchResultWithPostInfo, ensure, pallet_prelude::*,
        sp_runtime::traits::StaticLookup, Blake2_128Concat, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::CheckedMul;
    use sp_std::prelude::*;

    use crate::payments::{Reward, RewardFor};
    use crate::RewardManager;
    use pallet_acurast::{
        AllowedSourcesUpdate, Fulfillment, JobHooks, JobId, JobRegistrationFor, Script,
        StoredJobRegistration,
    };

    use crate::traits::*;
    use crate::types::*;
    use crate::utils::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_acurast::Config {
        type Event: From<Event<Self>>
            + IsType<<Self as pallet_acurast::Config>::Event>
            + IsType<<Self as frame_system::Config>::Event>;
        /// Extra structure to include in the registration of a job.
        type RegistrationExtra: IsType<<Self as pallet_acurast::Config>::RegistrationExtra>
            + Into<JobRequirements<RewardFor<Self>>>
            + Parameter
            + Member;
        /// The ID for this pallet
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        type AssetId: Parameter + Member + IsType<<RewardFor<Self> as Reward>::AssetId>;
        type AssetAmount: Parameter
            + Member
            + CheckedMul
            + From<u128>
            + Ord
            + IsType<<RewardFor<Self> as Reward>::Balance>;
        /// Logic for locking and paying tokens for job execution
        type RewardManager: RewardManager<Self>;
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    /// The storage for jobs' status as a map [AccountId] -> [Script] -> [JobStatus].
    #[pallet::storage]
    #[pallet::getter(fn stored_job_registration)]
    pub type StoredJobStatus<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, Script, JobStatus>;

    /// The storage for advertisements. They are stored as a map [AccountId] -> [Advertisment] since only one
    /// advertisement per client is allowed.
    #[pallet::storage]
    #[pallet::getter(fn stored_advertisement)]
    pub type StoredAdvertisement<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, AdvertisementFor<T>>;

    /// The storage for remaining capacity for each source. Can be negative if capacity is reduced beyond the number of jobs currently assigned.
    #[pallet::storage]
    #[pallet::getter(fn stored_capacity)]
    pub type StoredCapacity<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, i32>;

    /// Index with sorted advertisement by reward asset as a map [AssetId] -> Vec<([AccountId], [Price])>
    #[pallet::storage]
    #[pallet::getter(fn stored_ad_index)]
    pub type StoredAdIndex<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        <T as Config>::AssetId,
        Vec<AdvertismentIndexValue<T::AccountId, T::AssetAmount>>,
    >;

    /// Job assignments as a map [JobId] -> source's [AccountId] -> SlotId
    #[pallet::storage]
    #[pallet::getter(fn stored_job_assignment)]
    pub type StoredJobAssignment<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        JobId<T::AccountId>,
        Blake2_128Concat,
        T::AccountId,
        u8,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A registration was successfully matched. [registration, who]
        JobRegistrationMatched(JobId<T::AccountId>),
        /// A advertisement was successfully stored. [registration, who]
        AdvertisementStored(AdvertisementFor<T>, T::AccountId),
        /// A registration was successfully removed. [who]
        AdvertisementRemoved(T::AccountId),
        // /// The job assignemts have been updated. [who, updates]
        // JobAssignmentUpdate(T::AccountId, Vec<JobAssignmentUpdate<T::AccountId>>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The job registration's reward type is not supported.
        JobRegistrationUnsupportedReward,
        /// The job registration's reward was overflowing when calculating total amount to be paid.
        RewardCalculationOverflow,
        /// The reward could not be converted to different amount.
        RewardConversionFailed,
        /// The job registration's must specify non-zero `cpu_milliseconds`.
        JobRegistrationZeroCPUMilliseconds,
        /// The job registration's must specify non-zero `slots`.
        JobRegistrationZeroSlots,
        /// The job registration's must specify non-zero `reward`.
        JobRegistrationZeroReward,
        /// Job status not found. SEVERE error
        JobStatusNotFound,
        /// The job registration can't be modified.
        JobRegistrationUnmodifiable,
        /// Fulfill cannot be called for a job that does not have `JobStatus::Assigned` status.
        CannotFulfillJobWhenNotAssigned,
        /// Advertisement not found when attempt to delete it.
        AdvertisementNotFound,
        /// Fulfill was executed for a not registered job.
        EmptyPricing,
        /// Pricing cannot be changed (for now).
        PricingUnmodifiable,
        /// Payment wasn't recognized as valid. Probably didn't come from statemint assets pallet
        InvalidPayment,
        /// Failed to retrieve funds from pallet account to pay source. SEVERE error
        FailedToPay,
        /// StoredAdIndex holds inconsistent data. SEVERE error
        AdIndexInconsistent,
        /// Capacity not known for a source. SEVERE error
        CapacityNotFound,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Advertise resources by providing a [AdvertisementFor]. If an advertisement for the same script was previously registered, it will be overwritten.
        #[pallet::weight(< T as Config >::WeightInfo::advertise())]
        pub fn advertise(
            origin: OriginFor<T>,
            advertisement: AdvertisementFor<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!((&advertisement).pricing.len() > 0, Error::<T>::EmptyPricing);

            // update capacity to save on operations when checking available capacity
            if let Some(old) = <StoredAdvertisement<T>>::get(who.clone()) {
                // TODO: relax this check and resort ads according to updated pricing
                ensure!(
                    old.pricing == advertisement.pricing,
                    Error::<T>::PricingUnmodifiable
                );

                // allow capacity to become negative (in which case source remains assigned but does not receive new jobs assigned)
                <StoredCapacity<T>>::mutate(who.clone(), |c| {
                    c.unwrap_or(0) + advertisement.capacity as i32 - old.capacity as i32
                });
            } else {
                <StoredCapacity<T>>::insert(who.clone(), advertisement.capacity as i32);
            }

            <StoredAdvertisement<T>>::insert(who.clone(), advertisement.clone());

            // update index
            for pricing in &advertisement.pricing {
                let mut ads = <StoredAdIndex<T>>::get(&pricing.reward_asset).unwrap_or_default();

                let to_add = (who.clone(), pricing.price_per_cpu_millisecond.clone());
                // partition with predicate such that lower priced ads at start of ved
                let pos = ads.partition_point(|v| v.1 < to_add.1); // -> predicate holds for ads[i], i ∈ [0, pos)
                ads.insert(pos, to_add);

                <StoredAdIndex<T>>::set(&pricing.reward_asset, Some(ads));
            }

            Self::deposit_event(Event::AdvertisementStored(advertisement, who));
            Ok(().into())
        }

        /// Delete advertisement.
        #[pallet::weight(< T as Config >::WeightInfo::delete_advertisement())]
        pub fn delete_advertisement(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let ad = <StoredAdvertisement<T>>::get(who.clone())
                .ok_or(Error::<T>::AdvertisementNotFound)?;

            // update index
            for pricing in &ad.pricing {
                <StoredAdIndex<T>>::mutate(&pricing.reward_asset, |ads| {
                    let mut a = ads.clone().unwrap_or_default();
                    a.retain(|v| v.0 != who.clone())
                });
            }

            <StoredAdvertisement<T>>::remove(who.clone());
            <StoredCapacity<T>>::remove(who.clone());

            Self::deposit_event(Event::AdvertisementRemoved(who));
            Ok(().into())
        }
    }

    impl<T: Config> From<Error<T>> for pallet_acurast::Error<T> {
        fn from(_: Error<T>) -> Self {
            Self::JobHookFailed
        }
    }

    impl<T: Config> JobHooks<T> for Pallet<T> {
        type Error = Error<T>;

        /// Registers a job in the marketplace by providing a [JobRegistration].
        /// If a job for the same `(accountId, script)` was previously registered, it will be overwritten.
        fn register_hook(
            who: &T::AccountId,
            registration: &JobRegistrationFor<T>,
        ) -> Result<(), DispatchError> {
            let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
            let extra: JobRequirementsFor<T> = e.into();

            ensure!(
                extra.cpu_milliseconds > 0,
                Error::<T>::JobRegistrationZeroCPUMilliseconds
            );
            ensure!(extra.slots > 0, Error::<T>::JobRegistrationZeroSlots);
            let reward_amount: T::AssetAmount = extra
                .reward
                .try_get_amount()
                .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?
                .into();
            ensure!(
                reward_amount > 0.into(),
                Error::<T>::JobRegistrationZeroReward
            );

            // reward is understood per slot
            let mut total = extra.reward.clone();
            total
                .with_amount(
                    reward_amount
                        .checked_mul(&((extra.slots as u128).into()))
                        .ok_or(Error::<T>::RewardCalculationOverflow)?
                        .into(),
                )
                .map_err(|_| Error::<T>::RewardConversionFailed)?;

            <StoredJobStatus<T>>::insert(&who, &registration.script, JobStatus::default());

            if Self::match_job(&who, &registration)? {
                // TODO improve event to contain list of matched sources
                let job_id: JobId<T::AccountId> = (who.clone(), registration.script.clone());
                Self::deposit_event(Event::JobRegistrationMatched(job_id));
            }

            // lock only after all other steps succeeded without errors because locking reward is not revertable
            T::RewardManager::lock_reward(total.clone(), T::Lookup::unlookup(who.clone()))
                .map_err(|_| Error::<T>::InvalidPayment)?;

            Ok(().into())
        }

        /// Deregisters a job for the given script.
        fn deregister_hook(who: &T::AccountId, script: &Script) -> Result<(), DispatchError> {
            let job_status =
                <StoredJobStatus<T>>::get(&who, &script).ok_or(Error::<T>::JobStatusNotFound)?;
            ensure!(
                job_status == JobStatus::Open,
                Error::<T>::JobRegistrationUnmodifiable
            );

            <StoredJobStatus<T>>::remove(&who, &script);
            Ok(().into())
        }

        /// Updates the allowed sources list of a [JobRegistration].
        fn update_allowed_sources_hook(
            who: &T::AccountId,
            script: &Script,
            _updates: &Vec<AllowedSourcesUpdate<T::AccountId>>,
        ) -> Result<(), DispatchError> {
            let job_status =
                <StoredJobStatus<T>>::get(&who, &script).ok_or(Error::<T>::JobStatusNotFound)?;

            ensure!(
                job_status == JobStatus::Open,
                Error::<T>::JobRegistrationUnmodifiable
            );

            Ok(().into())
        }

        /// Fulfills a previously registered job.
        fn fulfill_hook(
            who: &T::AccountId, // processor
            fulfillment: &Fulfillment,
            requester: <T::Lookup as StaticLookup>::Target, // the consumer that registered the job originally
        ) -> Result<(), DispatchError> {
            // find assignment
            let job_id: JobId<T::AccountId> = (requester.clone(), fulfillment.script.clone());
            <StoredJobAssignment<T>>::get(&job_id, &who)
                .ok_or(pallet_acurast::Error::<T>::FulfillSourceNotAllowed)?;

            // find job
            let job_status = <StoredJobStatus<T>>::get(&who, &fulfillment.script)
                .ok_or(Error::<T>::JobStatusNotFound)?;
            let job_registration = <StoredJobRegistration<T>>::get(&job_id.0, &fulfillment.script)
                .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

            let e: <T as Config>::RegistrationExtra = job_registration.extra.clone().into();
            let extra: JobRequirementsFor<T> = e.into();

            // validate
            ensure!(
                job_status != JobStatus::Assigned,
                Error::<T>::CannotFulfillJobWhenNotAssigned
            );

            // removed fulfilled job from assigned jobs
            <StoredJobAssignment<T>>::remove(&job_id, &who);

            // pay only after all other steps succeeded without errors because locking reward is not revertable
            T::RewardManager::pay_reward(extra.reward.clone(), T::Lookup::unlookup(who.clone()))
                .map_err(|_| Error::<T>::FailedToPay)?;

            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        // fn match_ad(
        //     who: &T::AccountId,
        //     advertisement: &AdvertisementFor<T>,
        // ) -> Result<bool, Error<T>> {
        //     // TODO implement
        //     Ok(false)
        // }

        fn match_job(
            who: &T::AccountId,
            registration: &JobRegistrationFor<T>,
        ) -> Result<bool, Error<T>> {
            let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
            let extra: JobRequirementsFor<T> = e.into();

            // strips away the asset amount
            let reward_asset: <T as Config>::AssetId = extra
                .reward
                .try_get_asset_id()
                .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?
                .into();

            // filter candidates according to reward asset
            let ads_with_reward = <StoredAdIndex<T>>::get(reward_asset);
            if let Some(ads) = ads_with_reward {
                let reward_amount: T::AssetAmount = extra
                    .reward
                    .try_get_amount()
                    .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?
                    .into();

                // either all or no candidate gets assigned after checking if all slots can be filled
                let mut candidates = Vec::new();
                for ad_with_reward in ads {
                    // CHECK price not exceeding reward
                    let total = ad_with_reward
                        .1
                        .checked_mul(&((extra.cpu_milliseconds as u128).into()))
                        .ok_or(Error::<T>::RewardCalculationOverflow)?;
                    if total > reward_amount {
                        break;
                    }

                    // CHECK capacity sufficient
                    let capacity = <StoredCapacity<T>>::get(ad_with_reward.0.clone())
                        .ok_or(Error::<T>::CapacityNotFound)?;
                    if capacity <= 0 {
                        continue;
                    }

                    // CHECK source is whitelisted
                    if !is_source_whitelisted::<T>(&ad_with_reward.0, registration) {
                        continue;
                    }

                    let ad = <StoredAdvertisement<T>>::get(&ad_with_reward.0)
                        .ok_or(Error::<T>::AdIndexInconsistent)?;

                    // CHECK consumer is whitelisted
                    if !is_consumer_whitelisted::<T>(&who, &ad) {
                        continue;
                    }

                    // CANDIDATE FOUND
                    candidates.push((ad_with_reward.0, capacity));

                    if candidates.len() as u8 == extra.slots {
                        // all slots matched -> stop looking at pricier ads in sorted list
                        break;
                    }
                }

                if candidates.len() as u8 == extra.slots {
                    // all slots matched
                    for (slot, candidate) in candidates.iter().enumerate() {
                        <StoredJobAssignment<T>>::set(
                            (&who, &registration.script),
                            &candidate.0,
                            Some(slot as u8),
                        );

                        <StoredCapacity<T>>::set(&candidate.0, Some(candidate.1 - 1));
                    }

                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}
