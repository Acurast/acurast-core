#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod attestation;
pub mod payments;
mod types;
mod utils;
pub mod weights;
pub mod xcm_adapters;

pub use pallet::*;
pub use payments::*;
pub use types::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        dispatch::DispatchResultWithPostInfo, ensure, pallet_prelude::*,
        sp_runtime::traits::StaticLookup, Blake2_128Concat, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_std::prelude::*;

    use crate::payments::*;
    use crate::types::*;
    use crate::utils::*;

    /// This trait provides the interface for a fulfillment router.
    pub trait FulfillmentRouter<T: Config> {
        fn received_fulfillment(
            origin: OriginFor<T>,
            from: T::AccountId,
            fulfillment: Fulfillment,
            registration: JobRegistration<T::AccountId, T::RegistrationExtra>,
            requester: <T::Lookup as StaticLookup>::Target,
        ) -> DispatchResultWithPostInfo;
    }

    pub trait RevocationListUpdateBarrier<T: Config> {
        fn can_update_revocation_list(
            origin: &T::AccountId,
            updates: &Vec<CertificateRevocationListUpdate>,
        ) -> bool;
    }

    impl<T: Config> RevocationListUpdateBarrier<T> for () {
        fn can_update_revocation_list(
            _origin: &T::AccountId,
            _updates: &Vec<CertificateRevocationListUpdate>,
        ) -> bool {
            false
        }
    }

    pub trait JobAssignmentUpdateBarrier<T: Config> {
        fn can_update_assigned_jobs(
            origin: &T::AccountId,
            updates: &Vec<JobAssignmentUpdate<T::AccountId>>,
        ) -> bool;
    }

    impl<T: Config> JobAssignmentUpdateBarrier<T> for () {
        fn can_update_assigned_jobs(
            _origin: &T::AccountId,
            _updates: &Vec<JobAssignmentUpdate<T::AccountId>>,
        ) -> bool {
            false
        }
    }

    pub trait WeightInfo {
        fn register() -> Weight;
        fn deregister() -> Weight;
        fn update_allowed_sources() -> Weight;
        fn advertise() -> Weight;
        fn delete_advertisement() -> Weight;
        fn update_job_assignments() -> Weight;
        fn fulfill() -> Weight;
        fn submit_attestation() -> Weight;
        fn update_certificate_revocation_list() -> Weight;
    }

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_timestamp::Config + pallet_assets::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// Extra structure to include in the registration of a job.
        type RegistrationExtra: Parameter + Member + MaxEncodedLen;
        /// The fulfillment router to route a job fulfillment to its final destination.
        type FulfillmentRouter: FulfillmentRouter<Self>;
        /// The max length of the allowed sources list for a registration.
        #[pallet::constant]
        type MaxAllowedSources: Get<u16>;
        /// Logic for locking and paying tokens for job execution
        type AssetTransactor: LockAndPayAsset<Self>;
        /// The ID for this pallet
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        /// Barrier for the update_certificate_revocation_list extrinsic call.
        type RevocationListUpdateBarrier: RevocationListUpdateBarrier<Self>;
        /// Barrier for update_job_assignments extrinsic call.
        type JobAssignmentUpdateBarrier: JobAssignmentUpdateBarrier<Self>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    /// The storage for jobs as a map [AccountId] -> [Script] -> [Job].
    #[pallet::storage]
    #[pallet::getter(fn stored_job_registration)]
    pub type StoredJob<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        Script,
        Job<T::AccountId, T::RegistrationExtra>,
    >;

    /// The storage for attestations as a map [AccountId] -> [Attestation].
    #[pallet::storage]
    #[pallet::getter(fn stored_attestation)]
    pub type StoredAttestation<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Attestation>;

    /// Certificate revocation list storage.
    #[pallet::storage]
    #[pallet::getter(fn stored_revoked_certificate)]
    pub type StoredRevokedCertificate<T: Config> =
        StorageMap<_, Blake2_128Concat, SerialNumber, ()>;

    /// The storage for advertisements. They are stored as a map [AccountId] -> [Advertisment] since only one
    /// advertisement per client is allowed.
    #[pallet::storage]
    #[pallet::getter(fn stored_advertisement)]
    pub type StoredAdvertisement<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Advertisement<T::AccountId>>;

    /// The storage for remaining capacity for each source. Can be negative if capacity is reduced beyond the number of jobs currently assigned.
    #[pallet::storage]
    #[pallet::getter(fn stored_capacity)]
    pub type StoredCapacity<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, i32>;

    /// Index with sorted advertisement by reward asset as a map [RewardAssetId] -> Vec<([AccountId], [Price])>
    #[pallet::storage]
    #[pallet::getter(fn stored_ad_index)]
    pub type StoredAdIndex<T: Config> =
        StorageMap<_, Blake2_128Concat, RewardAssetId, Vec<AdvertismentIndexValue<T::AccountId>>>;

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
        /// A registration was successfully stored. [registration, who]
        JobRegistrationStored(
            JobRegistration<T::AccountId, T::RegistrationExtra>,
            T::AccountId,
        ),
        /// A registration was successfully matched. [registration, who]
        JobRegistrationMatched(
            JobRegistration<T::AccountId, T::RegistrationExtra>,
            T::AccountId,
        ),
        /// A registration was successfully removed. [registration, who]
        JobRegistrationRemoved(Script, T::AccountId),
        /// A fulfillment has been posted. [who, fulfillment, registration, receiver]
        ReceivedFulfillment(
            T::AccountId,
            Fulfillment,
            JobRegistration<T::AccountId, T::RegistrationExtra>,
            T::AccountId,
        ),
        /// The allowed sources have been updated. [who, old_registration, updates]
        AllowedSourcesUpdated(
            T::AccountId,
            JobRegistration<T::AccountId, T::RegistrationExtra>,
            Vec<AllowedSourcesUpdate<T::AccountId>>,
        ),
        /// An attestation was successfully stored. [attestation, who]
        AttestationStored(Attestation, T::AccountId),
        /// The certificate revocation list has been updated. [who, updates]
        CertificateRecovationListUpdated(T::AccountId, Vec<CertificateRevocationListUpdate>),
        /// A advertisement was successfully stored. [registration, who]
        AdvertisementStored(Advertisement<T::AccountId>, T::AccountId),
        /// A registration was successfully removed. [who]
        AdvertisementRemoved(T::AccountId),
        /// The job assignemts have been updated. [who, updates]
        JobAssignmentUpdate(T::AccountId, Vec<JobAssignmentUpdate<T::AccountId>>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The job registration's reward type is not supported.
        JobRegistrationUnsupportedReward,
        /// The job registration's must specify non-zero `cpu_milliseconds`.
        JobRegistrationZeroCPUMilliseconds,
        /// The job registration's must specify non-zero `slots`.
        JobRegistrationZeroSlots,
        /// The job registration's must specify non-zero `reward`.
        JobRegistrationZeroReward,
        /// Fulfill was executed for a not registered job.
        JobRegistrationNotFound,
        /// The job registration can't be modified.
        JobRegistrationUnmodifiable,
        /// Fulfill cannot be called for a job that does not have `JobStatus::Assigned` status.
        CannotFulfillJobWhenNotAssigned,
        /// Advertisement not found when attempt to delete it.
        AdvertisementNotFound,
        /// The source of the fulfill is not allowed for the job.
        FulfillSourceNotAllowed,
        /// The source of the fulfill is not verified. The source does not have a valid attestation submitted.
        FulfillSourceNotVerified,
        /// The source of the fulfill is not allowed for the job.
        ConsumerNotWhitelisted,
        /// The allowed source list for a registration exceeds the max length.
        TooManyAllowedSources,
        /// The allowed source list for a registration cannot be empty if provided.
        TooFewAllowedSources,
        /// The provided script value is not valid. The value needs to be and ipfs:// url.
        InvalidScriptValue,
        /// The provided attestation could not be parsed or is invalid.
        AttestationUsageExpired,
        /// The certificate chain provided in the submit_attestation call is not long enough.
        CertificateChainTooShort,
        /// The submitted attestation root certificate is not valid.
        RootCertificateValidationFailed,
        /// The submitted attestation certificate chain is not valid.
        CertificateChainValidationFailed,
        /// The submitted attestation certificate is not valid
        AttestationCertificateNotValid,
        /// Failed to extract the attestation.
        AttestationExtractionFailed,
        /// Cannot get the attestation issuer name.
        CannotGetAttestationIssuerName,
        /// Cannot get the attestation serial number.
        CannotGetAttestationSerialNumber,
        /// Cannot get the certificate ID.
        CannotGetCertificateId,
        /// Failed to convert the attestation to its bounded type.
        AttestationToBoundedTypeConversionFailed,
        /// Timestamp error.
        FailedTimestampConversion,
        /// Certificate was revoked.
        RevokedCertificate,
        /// Origin is not allowed to update the certificate revocation list.
        CertificateRevocationListUpdateNotAllowed,
        /// Fulfill was executed for a not registered job.
        EmptyPricing,
        /// Pricing cannot be changed (for now).
        PricingUnmodifiable,
        /// The attestation was issued for an unsupported public key type.
        UnsupportedAttestationPublicKeyType,
        /// The submitted attestation public key does not match the source.
        AttestationPublicKeyDoesNotMatchSource,
        /// Job assignment update not allowed.
        JobAssignmentNotFound,
        /// Job assignment update not allowed.
        JobAssignmentUpdateNotAllowed,
        /// Job assignment update invalid because of job's status.
        JobAssignmentUpdateInvalidStatus,
        /// Job assignment update invalid because slot out of bounds.
        JobAssignmentUpdateInvalidSlot,
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
        /// Registers a job by providing a [JobRegistration]. If a job for the same script was previously registered, it will be overwritten.
        #[pallet::weight(<T as Config>::WeightInfo::register())]
        pub fn register(
            origin: OriginFor<T>,
            registration: JobRegistration<T::AccountId, T::RegistrationExtra>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let script_len: u32 = registration
                .script
                .len()
                .try_into()
                .map_err(|_| Error::<T>::InvalidScriptValue)?;
            ensure!(
                script_len == SCRIPT_LENGTH && registration.script.starts_with(SCRIPT_PREFIX),
                Error::<T>::InvalidScriptValue
            );
            ensure!(
                registration.cpu_milliseconds > 0,
                Error::<T>::JobRegistrationZeroCPUMilliseconds
            );
            ensure!(registration.slots > 0, Error::<T>::JobRegistrationZeroSlots);
            let reward_value = extract_value(&registration.reward)
                .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?;
            ensure!(reward_value > 0, Error::<T>::JobRegistrationZeroReward);

            let allowed_sources_len = registration
                .allowed_sources
                .as_ref()
                .map(|sources| sources.len());
            if let Some(allowed_sources_len) = allowed_sources_len {
                let max_allowed_sources_len = T::MaxAllowedSources::get() as usize;
                ensure!(allowed_sources_len > 0, Error::<T>::TooFewAllowedSources);
                ensure!(
                    allowed_sources_len <= max_allowed_sources_len,
                    Error::<T>::TooManyAllowedSources
                );
            }

            let reward_asset = extract_asset(registration.reward.clone())
                .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?;
            let reward_value = extract_value(&registration.reward)
                .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?;

            T::AssetTransactor::lock_asset(
                reward_asset.with_value(reward_value * registration.slots as u128), // reward is understood per slot
                T::Lookup::unlookup(who.clone()),
            )
            .map_err(|_| Error::<T>::InvalidPayment)?;

            <StoredJob<T>>::insert(
                &who,
                &registration.script,
                Job {
                    registration: registration.clone(),
                    status: JobStatus::default(),
                },
            );
            Self::deposit_event(Event::JobRegistrationStored(
                registration.clone(),
                who.clone(),
            ));

            if Self::match_job(&who, &registration)? {
                // TODO improve event to contain list of matched sources
                Self::deposit_event(Event::JobRegistrationMatched(registration, who));
            }

            Ok(().into())
        }

        /// Deregisters a job for the given script.
        #[pallet::weight(<T as Config>::WeightInfo::deregister())]
        pub fn deregister(origin: OriginFor<T>, script: Script) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let job =
                <StoredJob<T>>::get(&who, &script).ok_or(Error::<T>::JobRegistrationNotFound)?;
            ensure!(
                job.status == JobStatus::Open,
                Error::<T>::JobRegistrationUnmodifiable
            );

            <StoredJob<T>>::remove(&who, &script);
            Self::deposit_event(Event::JobRegistrationRemoved(script, who));
            Ok(().into())
        }

        /// Updates the allowed sources list of a [JobRegistration].
        #[pallet::weight(<T as Config>::WeightInfo::update_allowed_sources())]
        pub fn update_allowed_sources(
            origin: OriginFor<T>,
            script: Script,
            updates: Vec<AllowedSourcesUpdate<T::AccountId>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let job =
                <StoredJob<T>>::get(&who, &script).ok_or(Error::<T>::JobRegistrationNotFound)?;

            ensure!(
                job.status == JobStatus::Open,
                Error::<T>::JobRegistrationUnmodifiable
            );

            let mut current_allowed_sources =
                job.registration.allowed_sources.clone().unwrap_or_default();
            for update in &updates {
                let position = current_allowed_sources
                    .iter()
                    .position(|value| value == &update.account_id);
                match (position, update.operation) {
                    (None, ListUpdateOperation::Add) => {
                        current_allowed_sources.push(update.account_id.clone())
                    }
                    (Some(pos), ListUpdateOperation::Remove) => {
                        current_allowed_sources.remove(pos);
                    }
                    _ => {}
                }
            }
            let max_allowed_sources_len = T::MaxAllowedSources::get() as usize;
            let allowed_sources_len = current_allowed_sources.len();
            ensure!(
                allowed_sources_len <= max_allowed_sources_len,
                Error::<T>::TooManyAllowedSources
            );
            let allowed_sources = if current_allowed_sources.is_empty() {
                None
            } else {
                Some(current_allowed_sources)
            };
            <StoredJob<T>>::insert(
                &who,
                &script,
                Job {
                    registration: JobRegistration {
                        script: script.clone(),
                        slots: job.registration.slots,
                        cpu_milliseconds: job.registration.cpu_milliseconds,
                        allowed_sources,
                        extra: job.registration.extra.clone(),
                        allow_only_verified_sources: job.registration.allow_only_verified_sources,
                        reward: job.registration.reward.clone(),
                    },
                    status: JobStatus::default(),
                },
            );

            Self::deposit_event(Event::AllowedSourcesUpdated(who, job.registration, updates));

            Ok(().into())
        }

        /// Advertise resources by providing a [Advertisement]. If an advertisement for the same script was previously registered, it will be overwritten.
        #[pallet::weight(<T as Config>::WeightInfo::advertise())]
        pub fn advertise(
            origin: OriginFor<T>,
            advertisement: Advertisement<T::AccountId>,
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

                let to_add = (who.clone(), pricing.price_per_cpu_millisecond);
                // partition with predicate such that lower priced ads at start of ved
                let pos = ads.partition_point(|v| v.1 < to_add.1); // -> predicate holds for ads[i], i ∈ [0, pos)
                ads.insert(pos, to_add);

                <StoredAdIndex<T>>::set(&pricing.reward_asset, Some(ads));
            }

            Self::deposit_event(Event::AdvertisementStored(advertisement, who));
            Ok(().into())
        }

        /// Delete advertisement.
        #[pallet::weight(<T as Config>::WeightInfo::delete_advertisement())]
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

        /// Assigns jobs to [AccountId]s. Those accounts can then later call `fulfill` for those jobs.
        #[pallet::weight(<T as Config>::WeightInfo::update_job_assignments())]
        pub fn update_job_assignments(
            origin: OriginFor<T>,
            updates: Vec<JobAssignmentUpdate<T::AccountId>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            if !T::JobAssignmentUpdateBarrier::can_update_assigned_jobs(&who, &updates) {
                return Err(Error::<T>::JobAssignmentUpdateNotAllowed)?;
            }
            for update in &updates {
                // lookups
                let job_id: JobId<T::AccountId> = (update.requester.clone(), update.script.clone());

                let job: Job<T::AccountId, T::RegistrationExtra> =
                    <StoredJob<T>>::get(&update.requester, update.script.clone())
                        .ok_or(Error::<T>::JobRegistrationNotFound)?;

                // validate
                ensure_source_allowed::<T>(&update.assignee, &job.registration)?;
                ensure!(
                    job.status == JobStatus::Open,
                    Error::<T>::JobAssignmentUpdateInvalidStatus
                );

                match &update.operation {
                    JobAssignemntUpdateOperation::Add(slot_id) => {
                        ensure!(
                            *slot_id < job.registration.slots,
                            Error::<T>::JobAssignmentUpdateInvalidSlot
                        );
                        <StoredJobAssignment<T>>::set(job_id, &update.assignee, Some(*slot_id));
                    }
                    JobAssignemntUpdateOperation::Remove => {
                        // we allow overwriting existing assignment
                        <StoredJobAssignment<T>>::remove(job_id, &update.assignee);
                    }
                }
            }
            Self::deposit_event(Event::JobAssignmentUpdate(who, updates));
            Ok(().into())
        }

        /// Fulfills a previously registered job.
        #[pallet::weight(<T as Config>::WeightInfo::fulfill())]
        pub fn fulfill(
            origin: OriginFor<T>, // processor
            fulfillment: Fulfillment,
            requester: <T::Lookup as StaticLookup>::Source, // the consumer that registered the job originally
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin.clone())?;
            let requester = T::Lookup::lookup(requester)?;

            // find assignment
            let job_id: JobId<T::AccountId> = (requester.clone(), fulfillment.script.clone());
            <StoredJobAssignment<T>>::get(&job_id, &who)
                .ok_or(Error::<T>::FulfillSourceNotAllowed)?;

            // find job
            let job: Job<T::AccountId, T::RegistrationExtra> =
                <StoredJob<T>>::get(&job_id.0, &fulfillment.script)
                    .ok_or(Error::<T>::JobRegistrationNotFound)?;

            // validate
            ensure!(
                job.status != JobStatus::Assigned,
                Error::<T>::CannotFulfillJobWhenNotAssigned
            );
            ensure_source_allowed::<T>(&who, &job.registration)?;

            T::AssetTransactor::pay_asset(
                job.registration.reward.clone(),
                T::Lookup::unlookup(who.clone()),
            )
            .map_err(|_| Error::<T>::FailedToPay)?;

            // route fulfillment
            let info = T::FulfillmentRouter::received_fulfillment(
                origin,
                who.clone(),
                fulfillment.clone(),
                job.registration.clone(),
                requester.clone(),
            )?;

            // removed fulfilled job from assigned jobs
            <StoredJobAssignment<T>>::remove(&job_id, &who);

            Self::deposit_event(Event::ReceivedFulfillment(
                who,
                fulfillment,
                job.registration,
                requester,
            ));
            Ok(info)
        }

        /// Submits an attestation given a valid certificate chain.
        ///
        /// - As input a list of binary certificates is expected.
        /// - The list must be ordered, starting from one of the known [trusted root certificates](https://developer.android.com/training/articles/security-key-attestation#root_certificate).
        /// - If the represented chain is valid, the [Attestation] details are stored. An existing attestion for signing account gets overwritten.
        ///
        /// Revocation: Each atttestation is stored with the unique IDs of the certificates on the chain proofing the attestation's validity.
        #[pallet::weight(<T as Config>::WeightInfo::submit_attestation())]
        pub fn submit_attestation(
            origin: OriginFor<T>,
            attestation_chain: AttestationChain,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!(
                (&attestation_chain).certificate_chain.len() >= 2,
                Error::<T>::CertificateChainTooShort,
            );

            let attestation = validate_and_extract_attestation::<T>(&who, &attestation_chain)?;

            ensure_not_expired::<T>(&attestation)?;
            ensure_not_revoked::<T>(&attestation)?;

            <StoredAttestation<T>>::insert(&who, attestation.clone());
            Self::deposit_event(Event::AttestationStored(attestation, who));
            Ok(().into())
        }

        #[pallet::weight(<T as Config>::WeightInfo::register())]
        pub fn update_certificate_revocation_list(
            origin: OriginFor<T>,
            updates: Vec<CertificateRevocationListUpdate>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            if !T::RevocationListUpdateBarrier::can_update_revocation_list(&who, &updates) {
                return Err(Error::<T>::CertificateRevocationListUpdateNotAllowed)?;
            }
            for update in &updates {
                match &update.operation {
                    ListUpdateOperation::Add => {
                        <StoredRevokedCertificate<T>>::insert(&update.cert_serial_number, ());
                    }
                    ListUpdateOperation::Remove => {
                        <StoredRevokedCertificate<T>>::remove(&update.cert_serial_number);
                    }
                }
            }
            Self::deposit_event(Event::CertificateRecovationListUpdated(who, updates));
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        // fn match_ad(
        //     who: &T::AccountId,
        //     advertisement: &Advertisement<T::AccountId>,
        // ) -> Result<bool, Error<T>> {
        //     // TODO implement
        //     Ok(false)
        // }

        fn match_job(
            who: &T::AccountId,
            registration: &JobRegistration<T::AccountId, T::RegistrationExtra>,
        ) -> Result<bool, Error<T>> {
            // strips away the asset amount
            let reward_asset = extract_asset(registration.reward.clone())
                .map_err(|_| Error::<T>::AdIndexInconsistent)?;

            // filter candidates according to reward asset
            let ads_with_reward = <StoredAdIndex<T>>::get(reward_asset);
            if let Some(ads) = ads_with_reward {
                let reward_value = extract_value(&registration.reward)
                    .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?;

                // either all or no candidate gets assigned after checking if all slots can be filled
                let mut candidates = Vec::new();
                for ad_with_reward in ads {
                    // CHECK price not exceeding reward
                    if ad_with_reward.1 * registration.cpu_milliseconds > reward_value {
                        break;
                    }

                    // CHECK capacity sufficient
                    let capacity = <StoredCapacity<T>>::get(ad_with_reward.0.clone())
                        .ok_or(Error::<T>::CapacityNotFound)?;
                    if capacity <= 0 {
                        continue;
                    }

                    // CHECK source is whitelisted
                    if ensure_source_allowed::<T>(&ad_with_reward.0, &registration).is_err() {
                        continue;
                    }

                    let ad = <StoredAdvertisement<T>>::get(&ad_with_reward.0)
                        .ok_or(Error::<T>::AdIndexInconsistent)?;

                    // CHECK consumer is whitelisted
                    if ensure_consumer_allowed::<T>(&who, &ad).is_err() {
                        continue;
                    }

                    // CANDIDATE FOUND
                    candidates.push((ad_with_reward.0, capacity));

                    if candidates.len() as u8 == registration.slots {
                        // all slots matched -> stop looking at pricier ads in sorted list
                        break;
                    }
                }

                if candidates.len() as u8 == registration.slots {
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
