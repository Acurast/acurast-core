#![cfg_attr(not(feature = "std"), no_std)]

pub use functions::*;
pub use pallet::*;
pub use payments::*;
pub use types::*;

#[cfg(test)]
pub mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

mod functions;
mod migration;
pub mod payments;
pub mod types;
mod utils;
pub mod weights;
pub mod weights_with_hooks;

pub(crate) use pallet::STORAGE_VERSION;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        dispatch::DispatchResultWithPostInfo, ensure, pallet_prelude::*, traits::UnixTime,
        Blake2_128, Blake2_128Concat, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use itertools::Itertools;
    use reputation::{BetaParameters, BetaReputation, ReputationEngine};
    use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub};
    use sp_runtime::{FixedU128, Permill, SaturatedConversion};
    use sp_std::iter::once;
    use sp_std::prelude::*;

    use pallet_acurast::utils::ensure_source_verified;
    use pallet_acurast::{
        AllowedSourcesUpdate, JobHooks, JobId, JobIdSequence, JobRegistrationFor, MultiOrigin,
        Schedule, StoredJobRegistration,
    };
    use pallet_acurast_assets_manager::traits::AssetValidator;

    use crate::payments::{Reward, RewardFor};
    use crate::types::*;
    use crate::utils::*;
    use crate::weights::WeightInfo;
    use crate::RewardManager;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_acurast::Config {
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as pallet_acurast::Config>::RuntimeEvent>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The max length of the allowed sources list for a registration.
        #[pallet::constant]
        type MaxAllowedConsumers: Get<u32> + Parameter;
        type MaxProposedMatches: Get<u32>;
        /// Extra structure to include in the registration of a job.
        type RegistrationExtra: IsType<<Self as pallet_acurast::Config>::RegistrationExtra>
            + Into<JobRequirementsFor<Self>>;
        /// The ID for this pallet
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        /// The the time tolerance in milliseconds. Represents the delta by how much we expect `now` timestamp being stale,
        /// hence `now <= currentmillis <= now + ReportTolerance`.
        ///
        /// Should be at least the worst case block time. Otherwise valid reports that are included near the end of a block
        /// would be considered outide of the agreed schedule despite being within schedule.
        #[pallet::constant]
        type ReportTolerance: Get<u64>;
        type AssetId: Parameter + IsType<<RewardFor<Self> as Reward>::AssetId>;
        type AssetAmount: Parameter
            + CheckedAdd
            + CheckedSub
            + CheckedMul
            + CheckedDiv
            + From<u8>
            + From<u32>
            + From<u64>
            + From<u128>
            + Into<u128>
            + Default
            + Ord
            + Clone
            + IsType<<RewardFor<Self> as Reward>::AssetAmount>;
        /// Logic for locking and paying tokens for job execution
        type RewardManager: RewardManager<Self>;
        type AssetValidator: AssetValidator<Self::AssetId>;
        type WeightInfo: WeightInfo;

        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: crate::benchmarking::BenchmarkHelper<Self>;
    }

    pub(crate) const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    /// The storage for jobs' status as a map [`AccountId`] `(consumer)` -> [`Script`] -> [`JobStatus`].
    #[pallet::storage]
    #[pallet::getter(fn stored_job_status)]
    pub type StoredJobStatus<T: Config> = StorageDoubleMap<
        _,
        Blake2_128,
        MultiOrigin<T::AccountId>,
        Blake2_128,
        JobIdSequence,
        JobStatus,
    >;

    /// The storage for basic advertisements' restrictions (without pricing). They are stored as a map [`AccountId`] `(source)` -> [`AdvertisementRestriction`] since only one
    /// advertisement per client is allowed.
    #[pallet::storage]
    #[pallet::getter(fn stored_advertisement)]
    pub type StoredAdvertisementRestriction<T: Config> =
        StorageMap<_, Blake2_128, T::AccountId, AdvertisementRestriction<T::AccountId>>;

    /// The storage for advertisements' pricing variants. They are stored as a map [`AccountId`] `(source)` -> [`AssetId`] -> [`PricingVariant`] since only one
    /// advertisement per client, and at most one pricing for each distinct `AssetID` is allowed.
    #[pallet::storage]
    #[pallet::getter(fn stored_advertisement_pricing)]
    pub type StoredAdvertisementPricing<T: Config> =
        StorageDoubleMap<_, Blake2_128, T::AccountId, Blake2_128, T::AssetId, PricingVariantFor<T>>;

    /// The storage for remaining capacity for each source. Can be negative if capacity is reduced beyond the number of jobs currently assigned.
    #[pallet::storage]
    #[pallet::getter(fn stored_storage_capacity)]
    pub type StoredStorageCapacity<T: Config> = StorageMap<_, Blake2_128, T::AccountId, i64>;

    /// Reputation as a map [`AccountId`] `(source)` -> [`AssetId`] -> [`BetaParameters`].
    #[pallet::storage]
    #[pallet::getter(fn stored_reputation)]
    pub type StoredReputation<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        T::AssetId,
        BetaParameters<FixedU128>,
    >;

    /// Number of total jobs assigned as a map [`AssetId`] -> `AssetAmount`
    #[pallet::storage]
    #[pallet::getter(fn total_assigned)]
    pub type StoredTotalAssigned<T: Config> =
        StorageMap<_, Blake2_128Concat, <T as Config>::AssetId, u128>;

    /// Average job reward as a map [`AssetId`] -> `AssetAmount`
    #[pallet::storage]
    #[pallet::getter(fn average_reward)]
    pub type StoredAverageReward<T> = StorageMap<_, Blake2_128Concat, <T as Config>::AssetId, u128>;

    /// Job matches as a map [`AccountId`] `(source)` -> [`JobId`] -> `SlotId`
    #[pallet::storage]
    #[pallet::getter(fn stored_matches)]
    pub type StoredMatches<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        JobId<T::AccountId>,
        AssignmentFor<T>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn stored_matches_reverse_index)]
    pub type StoredMatchesReverseIndex<T: Config> =
        StorageMap<_, Blake2_128Concat, JobId<T::AccountId>, T::AccountId>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A registration was successfully matched. [Match]
        JobRegistrationMatched(Match<T::AccountId>),
        /// A registration was successfully matched. [JobId, SourceId, Assignment]
        JobRegistrationAssigned(JobId<T::AccountId>, T::AccountId, AssignmentFor<T>),
        /// A report for an execution has arrived. [JobId, SourceId, Assignment]
        Reported(JobId<T::AccountId>, T::AccountId, AssignmentFor<T>),
        /// A advertisement was successfully stored. [advertisement, who]
        AdvertisementStored(AdvertisementFor<T>, T::AccountId),
        /// A registration was successfully removed. [who]
        AdvertisementRemoved(T::AccountId),
        /// An execution is reported to be successful.
        ExecutionSuccess(JobId<T::AccountId>, ExecutionOperationHash),
        /// An execution is reported to have failed.
        ExecutionFailure(JobId<T::AccountId>, ExecutionFailureMessage),
        /// This event is emitted when a job is finalized.
        JobFinalized(JobId<T::AccountId>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The job registration's reward type is not supported.
        JobRegistrationUnsupportedReward,
        /// Generic overflow during a calculating with checked operatios.
        CalculationOverflow,
        /// The reward could not be converted to different amount.
        RewardConversionFailed,
        /// The job registration must specify non-zero `duration`.
        JobRegistrationZeroDuration,
        /// The job registration must specify a schedule that contains a maximum of [MAX_EXECUTIONS_PER_JOB] executions.
        JobRegistrationScheduleExceedsMaximumExecutions,
        /// The job registration must specify a schedule that contains at least one execution.
        JobRegistrationScheduleContainsZeroExecutions,
        /// The job registration's must specify `duration` < `interval`.
        JobRegistrationDurationExceedsInterval,
        /// The job registration's must specify `start` in the future.
        JobRegistrationStartInPast,
        /// The job registration's must specify `end` >= `start`.
        JobRegistrationEndBeforeStart,
        /// The job registration's must specify non-zero `slots`.
        JobRegistrationZeroSlots,
        /// Job status not found. SEVERE error
        JobStatusNotFound,
        /// The job registration can't be modified.
        JobRegistrationUnmodifiable,
        /// Acknowledge cannot be called for a job that does not have `JobStatus::Matched` status.
        CannotAcknowledgeWhenNotMatched,
        /// Report cannot be called for a job that was not acknowledged.
        CannotReportWhenNotAcknowledged,
        /// Advertisement not found when attempt to delete it.
        AdvertisementNotFound,
        /// Advertisement not found when attempt to delete it.
        AdvertisementPricingNotFound,
        /// Fulfill was executed for a not registered job.
        EmptyPricing,
        /// The allowed consumers list for a registration exeeded the max length.
        TooManyAllowedConsumers,
        /// The allowed consumers list for a registration cannot be empty if provided.
        TooFewAllowedConsumers,
        /// Advertisement cannot be deleted while matched to at least one job.
        ///
        /// Pricing and capacity can be updated, e.g. the capacity can be set to 0 no no longer receive job matches.
        CannotDeleteAdvertisementWhileMatched,
        /// Failed to retrieve funds from pallet account to pay source. SEVERE error
        FailedToPay,
        /// Asset is not allowed by `AssetBarrier`.
        AssetNotAllowedByBarrier,
        /// Invalid asset ID.
        InvalidAssetId,
        /// Invalid asset amount.
        InvalidAssetAmount,
        /// Capacity not known for a source. SEVERE error
        CapacityNotFound,
        /// Matching is empty.
        EmptyMatching,
        /// Match is invalid due to the start time already passed.
        OverdueMatch,
        /// Match is invalid due to incorrect source count.
        IncorrectSourceCountInMatch,
        /// Match is invalid due to a duplicate source for distinct slots.
        DuplicateSourceInMatch,
        /// Match is invalid due to an unverfied source while `allow_only_verified_sources` is true.
        UnverifiedSourceInMatch,
        /// Multiple different reward assets are currently not supported in a single matching.
        MultipleRewardAssetsInMatch,
        /// Match is invalid due to a source's maximum memory exceeded.
        SchedulingWindowExceededInMatch,
        /// Match is invalid due to a source's maximum memory exceeded.
        MaxMemoryExceededInMatch,
        /// Match is invalid due to a source's maximum memory exceeded.
        NetworkRequestQuotaExceededInMatch,
        /// Match is invalid due to a source not having enough capacity.
        InsufficientStorageCapacityInMatch,
        /// Match is invalid due to a source not part of the provided whitelist.
        SourceNotAllowedInMatch,
        /// Match is invalid due to a consumer not part of the provided whitelist.
        ConsumerNotAllowedInMatch,
        /// Match is invalid due to insufficient reward regarding the current source pricing.
        InsufficientRewardInMatch,
        /// Match is invalid due to insufficient reputation of a proposed source.
        InsufficientReputationInMatch,
        /// Match is invalid due to overlapping schedules.
        ScheduleOverlapInMatch,
        /// Received a report from a source that is not assigned.
        ReportFromUnassignedSource,
        /// More reports than expected total.
        MoreReportsThanExpected,
        /// Report received outside of schedule.
        ReportOutsideSchedule,
        /// Reputation not known for a source. SEVERE error
        ReputationNotFound,
        /// Job required module not available.
        ModuleNotAvailableInMatch,
        /// The job is not assigned to the given processor
        JobNotAssigned,
        /// The job cannot be finalized yet.
        JobCannotBeFinalized,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            crate::migration::migrate_to_v2::<T>()
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Advertise resources by providing a [AdvertisementFor].
        ///
        /// If the source has another active advertisement, the advertisement is updated given the updates does not
        /// violate any system invariants. For example, if the ad is currently assigned, changes to pricing are prohibited
        /// and only capacity updates will be tolerated.
        #[pallet::call_index(0)]
        #[pallet::weight(< T as Config >::WeightInfo::advertise())]
        pub fn advertise(
            origin: OriginFor<T>,
            advertisement: AdvertisementFor<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!((&advertisement).pricing.len() > 0, Error::<T>::EmptyPricing);

            Self::do_advertise(&who, &advertisement)?;

            Self::deposit_event(Event::AdvertisementStored(advertisement, who));
            Ok(().into())
        }

        /// Delete advertisement.
        #[pallet::call_index(1)]
        #[pallet::weight(< T as Config >::WeightInfo::delete_advertisement())]
        pub fn delete_advertisement(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            <StoredAdvertisementRestriction<T>>::get(&who)
                .ok_or(Error::<T>::AdvertisementNotFound)?;

            // prohibit updates as long as jobs assigned
            ensure!(
                !Self::has_matches(&who),
                Error::<T>::CannotDeleteAdvertisementWhileMatched
            );

            let _ = <StoredAdvertisementPricing<T>>::clear_prefix(&who, MAX_PRICING_VARIANTS, None);
            <StoredStorageCapacity<T>>::remove(&who);
            <StoredAdvertisementRestriction<T>>::remove(&who);

            Self::deposit_event(Event::AdvertisementRemoved(who));
            Ok(().into())
        }

        /// Proposes processors to match with a job. The match fails if it conflicts with the processor's schedule.
        #[pallet::call_index(2)]
        #[pallet::weight(< T as Config >::WeightInfo::propose_matching())]
        pub fn propose_matching(
            origin: OriginFor<T>,
            matches: BoundedVec<Match<T::AccountId>, <T as Config>::MaxProposedMatches>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let remaining_reward = Self::process_matching(&matches)?;

            // pay part of accumulated remaining reward (unspent to consumer) to matcher
            // pay only after all other steps succeeded without errors because paying reward is not revertable
            T::RewardManager::pay_matcher_reward(&remaining_reward, &who)?;

            Ok(().into())
        }

        /// Acknowledges a matched job. It fails if the origin is not the account that was matched for the job.
        #[pallet::call_index(3)]
        #[pallet::weight(< T as Config >::WeightInfo::acknowledge_match())]
        pub fn acknowledge_match(
            origin: OriginFor<T>,
            job_id: JobId<T::AccountId>,
            pub_keys: PubKeys,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let (changed, assignment) = <StoredMatches<T>>::try_mutate(
                &who,
                &job_id,
                |m| -> Result<(bool, AssignmentFor<T>), Error<T>> {
                    // CHECK that job was matched previously to calling source
                    let mut assignment = m
                        .as_mut()
                        .ok_or(Error::<T>::CannotAcknowledgeWhenNotMatched)?;
                    let changed = !assignment.acknowledged;
                    assignment.acknowledged = true;
                    assignment.pub_keys = Some(pub_keys);
                    Ok((changed, assignment.to_owned()))
                },
            )?;

            if changed {
                <StoredJobStatus<T>>::try_mutate(
                    &job_id.0,
                    &job_id.1,
                    |s| -> Result<(), Error<T>> {
                        let status = s.ok_or(Error::<T>::JobStatusNotFound)?;
                        *s = Some(match status {
                            JobStatus::Open => Err(Error::<T>::CannotAcknowledgeWhenNotMatched)?,
                            JobStatus::Matched => JobStatus::Assigned(1),
                            JobStatus::Assigned(count) => JobStatus::Assigned(count + 1),
                        });

                        Ok(())
                    },
                )?;

                Self::deposit_event(Event::JobRegistrationAssigned(
                    job_id,
                    who,
                    assignment.clone(),
                ));
            }
            Ok(().into())
        }

        /// Report on completion of fulfillments done on target chain for a previously registered and matched job.
        /// Reward is payed out to source if timing of this call is within expected interval. More precisely,
        /// the report is accepted if `[now, now + tolerance]` overlaps with an execution of the schedule agreed on.
        /// `tolerance` is a pallet config value.
        #[pallet::call_index(4)]
        #[pallet::weight(< T as Config >::WeightInfo::report())]
        pub fn report(
            origin: OriginFor<T>,
            job_id: JobId<T::AccountId>,
            execution_result: ExecutionResult,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // find assignment
            let assignment = <StoredMatches<T>>::try_mutate(
                &who,
                &job_id,
                |a| -> Result<AssignmentFor<T>, Error<T>> {
                    // NOTE: the None case is the "good case", used when there is *no entry yet and thus no duplicate assignment so far*.
                    if let Some(assignment) = a.as_mut() {
                        // CHECK that job is assigned
                        ensure!(
                            assignment.acknowledged,
                            Error::<T>::CannotReportWhenNotAcknowledged
                        );

                        // CHECK that we don't accept more reports than expected
                        ensure!(
                            assignment.sla.met < assignment.sla.total,
                            Error::<T>::MoreReportsThanExpected
                        );

                        assignment.sla.met += 1;
                        return Ok(assignment.to_owned());
                    } else {
                        return Err(Error::<T>::ReportFromUnassignedSource);
                    }
                },
            )?;

            let registration = <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1)
                .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

            let now = Self::now()?;
            let now_max = now
                .checked_add(T::ReportTolerance::get())
                .ok_or(Error::<T>::CalculationOverflow)?;

            ensure!(
                registration
                    .schedule
                    .overlaps(assignment.start_delay, now, now_max)
                    .ok_or(Error::<T>::CalculationOverflow)?,
                Error::<T>::ReportOutsideSchedule
            );

            // pay only after all other steps succeeded without errors because paying reward is not revertable
            T::RewardManager::pay_reward(&assignment.fee_per_execution, &who)?;

            match execution_result {
                ExecutionResult::Success(operation_hash) => {
                    Self::deposit_event(Event::ExecutionSuccess(job_id.clone(), operation_hash))
                }
                ExecutionResult::Failure(message) => {
                    Self::deposit_event(Event::ExecutionFailure(job_id.clone(), message))
                }
            }

            Self::deposit_event(Event::Reported(job_id, who, assignment.clone()));
            Ok(().into())
        }

        /// Called processors when the assigned job can be finalized.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::finalize_job())]
        pub fn finalize_job(
            origin: OriginFor<T>,
            job_id: JobId<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let registration = <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1)
                .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

            // find assignment
            let assignment =
                <StoredMatches<T>>::get(&who, &job_id).ok_or(Error::<T>::JobNotAssigned)?;

            let now = Self::now()?
                .checked_add(T::ReportTolerance::get())
                .ok_or(Error::<T>::CalculationOverflow)?;
            let (_actual_start, actual_end) = registration
                .schedule
                .range(assignment.start_delay)
                .ok_or(Error::<T>::CalculationOverflow)?;
            ensure!(actual_end.lt(&now), Error::<T>::JobCannotBeFinalized);

            // update reputation since we don't expect further reports for this job
            // (only update for attested devices!)
            if ensure_source_verified::<T>(&who).is_ok() {
                let extra: <T as Config>::RegistrationExtra = registration.extra.clone().into();
                let requirements: JobRequirementsFor<T> = extra.into();

                // parse reward into asset_id and amount
                let reward_asset: <T as Config>::AssetId = requirements
                    .reward
                    .try_get_asset_id()
                    .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?
                    .into();

                T::AssetValidator::validate(&reward_asset).map_err(|e| e.into())?;

                let reward_amount: <T as Config>::AssetAmount = requirements
                    .reward
                    .try_get_amount()
                    .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?
                    .into();

                // skip reputation update if reward is 0
                if reward_amount > 0u8.into() {
                    let average_reward = <StoredAverageReward<T>>::get(&reward_asset).unwrap_or(0);
                    let total_assigned =
                        <StoredTotalAssigned<T>>::get(&reward_asset).unwrap_or_default();

                    let total_reward = average_reward
                        .checked_mul(total_assigned - 1u128)
                        .ok_or(Error::<T>::CalculationOverflow)?;

                    let new_total_rewards = total_reward
                        .checked_add(reward_amount.clone().into())
                        .ok_or(Error::<T>::CalculationOverflow)?;

                    let mut beta_params = <StoredReputation<T>>::get(&who, &reward_asset)
                        .ok_or(Error::<T>::ReputationNotFound)?;

                    beta_params = BetaReputation::update(
                        beta_params,
                        assignment.sla.met,
                        assignment.sla.total - assignment.sla.met,
                        reward_amount.clone().into(),
                        average_reward,
                    )
                    .ok_or(Error::<T>::CalculationOverflow)?;

                    let new_average_reward = new_total_rewards
                        .checked_div(total_assigned)
                        .ok_or(Error::<T>::CalculationOverflow)?;

                    <StoredAverageReward<T>>::insert(reward_asset.clone(), new_average_reward);
                    <StoredReputation<T>>::insert(
                        &who,
                        &reward_asset,
                        BetaParameters {
                            r: beta_params.r,
                            s: beta_params.s,
                        },
                    );
                }
            }

            // removed completed job from all storage points (completed SLA gets still deposited in event below)
            <StoredMatches<T>>::remove(&who, &job_id);
            <StoredJobStatus<T>>::remove(&job_id.0, &job_id.1);
            <StoredMatchesReverseIndex<T>>::remove(&job_id);

            // increase capacity
            <StoredStorageCapacity<T>>::mutate(&who, |c| {
                *c = c.unwrap_or(0).checked_add(registration.storage.into())
            });

            <StoredJobRegistration<T>>::remove(&job_id.0, &job_id.1);

            Self::deposit_event(Event::JobFinalized(job_id));
            Ok(().into())
        }
    }

    impl<T: Config> From<Error<T>> for pallet_acurast::Error<T> {
        fn from(_: Error<T>) -> Self {
            Self::JobHookFailed
        }
    }

    impl<T: Config> JobHooks<T> for Pallet<T> {
        /// Registers a job in the marketplace by providing a [JobRegistration].
        /// If a job for the same `(accountId, script)` was previously registered, it will be overwritten.
        fn register_hook(
            who: &MultiOrigin<T::AccountId>,
            job_id: &JobId<T::AccountId>,
            registration: &JobRegistrationFor<T>,
        ) -> Result<(), DispatchError> {
            let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
            let requirements: JobRequirementsFor<T> = e.into();

            ensure!(
                registration.schedule.duration > 0,
                Error::<T>::JobRegistrationZeroDuration
            );
            let execution_count = registration.schedule.execution_count();
            ensure!(
                execution_count <= MAX_EXECUTIONS_PER_JOB,
                Error::<T>::JobRegistrationScheduleExceedsMaximumExecutions
            );
            ensure!(
                execution_count > 0,
                Error::<T>::JobRegistrationScheduleContainsZeroExecutions
            );
            ensure!(
                registration.schedule.duration < registration.schedule.interval,
                Error::<T>::JobRegistrationDurationExceedsInterval
            );
            ensure!(
                registration.schedule.start_time >= Self::now()?,
                Error::<T>::JobRegistrationStartInPast
            );
            ensure!(
                registration.schedule.start_time <= registration.schedule.end_time,
                Error::<T>::JobRegistrationEndBeforeStart
            );
            ensure!(requirements.slots > 0, Error::<T>::JobRegistrationZeroSlots);

            if let Some(job_status) = <StoredJobStatus<T>>::get(&job_id.0, &job_id.1) {
                ensure!(
                    job_status == JobStatus::Open,
                    Error::<T>::JobRegistrationUnmodifiable
                );
            } else {
                <StoredJobStatus<T>>::insert(&job_id.0, &job_id.1, JobStatus::default());
            }

            match requirements.instant_match {
                Some(sources) => {
                    Self::process_matching(once(&Match {
                        job_id: job_id.clone(),
                        sources,
                    }))?;
                }
                None => {}
            }

            // lock only after all other steps succeeded without errors because locking reward is not revertable
            if let MultiOrigin::Acurast(who) = who {
                // reward is understood per slot and execution
                let mut reward = requirements.reward;
                reward
                    .with_amount(Self::total_reward_amount(registration)?.into())
                    .map_err(|_| Error::<T>::RewardConversionFailed)?;
                T::RewardManager::lock_reward(&reward, &who)?;
            }

            Ok(().into())
        }

        /// Deregisters a job for the given script.
        fn deregister_hook(
            _who: &T::AccountId,
            job_id: &JobId<T::AccountId>,
        ) -> Result<(), DispatchError> {
            let job_status = <StoredJobStatus<T>>::get(&job_id.0, &job_id.1)
                .ok_or(Error::<T>::JobStatusNotFound)?;
            // lazily evaluated check if job is overdue
            let overdue = || -> Result<bool, DispatchError> {
                let registration = <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1)
                    .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

                Ok(Self::now()? >= registration.schedule.start_time)
            };
            ensure!(
                // allow to deregister overdue jobs
                job_status == JobStatus::Open || overdue()?,
                Error::<T>::JobRegistrationUnmodifiable
            );

            <StoredJobStatus<T>>::remove(&job_id.0, &job_id.1);
            Ok(().into())
        }

        /// Updates the allowed sources list of a [JobRegistration].
        fn update_allowed_sources_hook(
            _who: &T::AccountId,
            job_id: &JobId<T::AccountId>,
            _updates: &Vec<AllowedSourcesUpdate<T::AccountId>>,
        ) -> Result<(), DispatchError> {
            let job_status = <StoredJobStatus<T>>::get(&job_id.0, &job_id.1)
                .ok_or(Error::<T>::JobStatusNotFound)?;

            ensure!(
                job_status == JobStatus::Open,
                Error::<T>::JobRegistrationUnmodifiable
            );

            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Checks if a Processor - Job match is possible and returns the job reward.
        fn process_matching<'a>(
            matching: impl IntoIterator<Item = &'a Match<T::AccountId>>,
        ) -> Result<RewardFor<T>, DispatchError> {
            // Currently we require all matches to be rewarded with the same asset
            let mut remaining_reward: Option<(RewardFor<T>, T::AssetAmount)> = None;

            for m in matching {
                let registration = <StoredJobRegistration<T>>::get(&m.job_id.0, &m.job_id.1)
                    .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;
                let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
                let requirements: JobRequirementsFor<T> = e.into();

                let now = Self::now()?;
                ensure!(
                    now < registration.schedule.start_time,
                    Error::<T>::OverdueMatch
                );
                let l: u8 = m.sources.len().try_into().unwrap_or(0);
                ensure!(
                    // NOTE: we are checking for duplicates while inserting/mutating StoredMatches below
                    l == requirements.slots,
                    Error::<T>::IncorrectSourceCountInMatch
                );

                // parse reward into asset_id and amount
                let reward_asset: <T as Config>::AssetId = requirements
                    .reward
                    .try_get_asset_id()
                    .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?
                    .into();
                T::AssetValidator::validate(&reward_asset).map_err(|e| e.into())?;

                let reward_amount: <T as Config>::AssetAmount = requirements
                    .reward
                    .try_get_amount()
                    .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?
                    .into();

                // keep track of total fee in assignments to check later if it exceeds reward
                let mut total_fee: <T as Config>::AssetAmount = 0u8.into();

                // `slot` is used for detecting duplicate source proposed for distinct slots
                // TODO: add global (configurable) maximum of jobs assigned. This would limit the weight of `propose_matching` to a constant, since it depends on the number of active matches.
                for (slot, planned_execution) in m.sources.iter().enumerate() {
                    // CHECK attestation
                    ensure!(
                        !registration.allow_only_verified_sources
                            || ensure_source_verified::<T>(&planned_execution.source).is_ok(),
                        Error::<T>::UnverifiedSourceInMatch
                    );

                    let ad = <StoredAdvertisementRestriction<T>>::get(&planned_execution.source)
                        .ok_or(Error::<T>::AdvertisementNotFound)?;

                    for required_module in &registration.required_modules {
                        ensure!(
                            ad.available_modules.contains(required_module),
                            Error::<T>::ModuleNotAvailableInMatch
                        );
                    }

                    let pricing = <StoredAdvertisementPricing<T>>::get(
                        &planned_execution.source,
                        &reward_asset,
                    )
                    .ok_or(Error::<T>::AdvertisementPricingNotFound)?;

                    // CHECK the scheduling_window allow to schedule this job
                    match pricing.scheduling_window {
                        SchedulingWindow::End(end) => {
                            ensure!(
                                end >= registration
                                    .schedule
                                    .end_time
                                    .checked_add(planned_execution.start_delay)
                                    .ok_or(Error::<T>::CalculationOverflow)?,
                                Error::<T>::SchedulingWindowExceededInMatch
                            );
                        }
                        SchedulingWindow::Delta(delta) => {
                            ensure!(
                                now.checked_add(delta)
                                    .ok_or(Error::<T>::CalculationOverflow)?
                                    >= registration
                                        .schedule
                                        .end_time
                                        .checked_add(planned_execution.start_delay)
                                        .ok_or(Error::<T>::CalculationOverflow)?,
                                Error::<T>::SchedulingWindowExceededInMatch
                            );
                        }
                    }

                    // CHECK memory sufficient
                    ensure!(
                        ad.max_memory >= registration.memory,
                        Error::<T>::MaxMemoryExceededInMatch
                    );

                    // CHECK network request quota sufficient
                    ensure!(
                        // duration (s) * network_request_quota >= network_requests (per second)
                        // <=>
                        // duration (ms) / 1000 * network_request_quota >= network_requests (per second)
                        // <=>
                        // duration (ms) * network_request_quota >= network_requests (per second) * 1000
                        registration
                            .schedule
                            .duration
                            .checked_mul(ad.network_request_quota.into())
                            .unwrap_or(0u64)
                            >= registration
                                .network_requests
                                .saturated_into::<u64>()
                                .checked_mul(1000u64)
                                .unwrap_or(u64::MAX),
                        Error::<T>::NetworkRequestQuotaExceededInMatch
                    );

                    // CHECK remaining storage capacity sufficient
                    let capacity = <StoredStorageCapacity<T>>::get(&planned_execution.source)
                        .ok_or(Error::<T>::CapacityNotFound)?;
                    ensure!(capacity > 0, Error::<T>::InsufficientStorageCapacityInMatch);

                    // CHECK source is whitelisted
                    ensure!(
                        is_source_whitelisted::<T>(&planned_execution.source, &registration),
                        Error::<T>::SourceNotAllowedInMatch
                    );

                    // CHECK consumer is whitelisted
                    ensure!(
                        is_consumer_whitelisted::<T>(&m.job_id.0, &ad.allowed_consumers),
                        Error::<T>::ConsumerNotAllowedInMatch
                    );

                    // CHECK reputation sufficient
                    if let Some(min_reputation) = requirements.min_reputation {
                        let beta_params =
                            <StoredReputation<T>>::get(&planned_execution.source, &reward_asset)
                                .ok_or(Error::<T>::ReputationNotFound)?;

                        let reputation = BetaReputation::<u128>::normalize(beta_params)
                            .ok_or(Error::<T>::CalculationOverflow)?;

                        ensure!(
                            reputation >= Permill::from_parts(min_reputation as u32),
                            Error::<T>::InsufficientReputationInMatch
                        );
                    }

                    // CHECK schedule
                    Self::fits_schedule(
                        &planned_execution.source,
                        &registration.schedule,
                        planned_execution.start_delay,
                    )?;

                    // calculate fee
                    let fee_per_execution = Self::fee_per_execution(&registration, &pricing)?;

                    // CHECK price not exceeding reward
                    ensure!(
                        fee_per_execution <= reward_amount,
                        Error::<T>::InsufficientRewardInMatch
                    );

                    let execution_count = registration.schedule.execution_count();

                    total_fee = total_fee
                        .checked_add(
                            &fee_per_execution
                                .checked_mul(&execution_count.into())
                                .ok_or(Error::<T>::CalculationOverflow)?,
                        )
                        .ok_or(Error::<T>::CalculationOverflow)?;
                    let mut fee = requirements.reward.clone();
                    fee.with_amount(fee_per_execution.into())
                        .map_err(|_| Error::<T>::RewardConversionFailed)?;

                    // ASSIGN if not yet assigned (equals to CHECK that no duplicate source in a single mutate operation)
                    <StoredMatches<T>>::try_mutate(
                        &planned_execution.source,
                        &m.job_id,
                        |s| -> Result<(), Error<T>> {
                            // NOTE: the None case is the "good case", used when there is *no entry yet and thus no duplicate assignment so far*.
                            match s {
                                Some(_) => Err(Error::<T>::DuplicateSourceInMatch),
                                None => {
                                    *s = Some(Assignment {
                                        slot: slot as u8,
                                        start_delay: planned_execution.start_delay,
                                        fee_per_execution: fee,
                                        acknowledged: false,
                                        sla: SLA {
                                            total: execution_count,
                                            met: 0,
                                        },
                                        pub_keys: None,
                                    });
                                    Ok(())
                                }
                            }?;
                            Ok(())
                        },
                    )?;
                    <StoredMatchesReverseIndex<T>>::insert(
                        &m.job_id,
                        planned_execution.source.clone(),
                    );
                    <StoredStorageCapacity<T>>::set(
                        &planned_execution.source,
                        capacity.checked_sub(registration.storage.into()),
                    );
                }

                // CHECK total fee is not exceeding reward
                let total_reward_amount = Self::total_reward_amount(&registration)?;
                let diff = total_reward_amount
                    .checked_sub(&total_fee)
                    .ok_or(Error::<T>::InsufficientRewardInMatch)?;
                // We better check for diff positive <=> total_fee <= total_reward_amount
                // because we cannot assume that asset amount is an unsigned integer for all future
                ensure!(diff >= 0u32.into(), Error::<T>::InsufficientRewardInMatch);

                if let Some(a) = remaining_reward.as_mut() {
                    ensure!(
                        a.0 == requirements.reward,
                        Error::<T>::MultipleRewardAssetsInMatch
                    );

                    a.1 =
                        a.1.checked_add(&diff)
                            .ok_or(Error::<T>::CalculationOverflow)?;
                } else {
                    remaining_reward = Some((requirements.reward, diff));
                }

                <StoredTotalAssigned<T>>::mutate(&reward_asset, |t| {
                    *t = Some(t.unwrap_or(0u128).saturating_add(1));
                });

                <StoredJobStatus<T>>::insert(&m.job_id.0, &m.job_id.1, JobStatus::Matched);
                Self::deposit_event(Event::JobRegistrationMatched(m.clone()));
            }
            // If we arrive here with remaining_reward None, then matching was empty
            if let Some(reward) = remaining_reward.as_mut() {
                reward
                    .0
                    .with_amount(reward.1.clone().into())
                    .map_err(|_| Error::<T>::RewardConversionFailed)?;
                return Ok(reward.0.to_owned());
            } else {
                return Err(Error::<T>::EmptyMatching.into());
            }
        }

        /// Returns true if the source has currently at least one match (not necessarily assigned).
        fn has_matches(source: &T::AccountId) -> bool {
            // NOTE we use a trick to check if map contains *any* secondary key: we use `any` to short-circuit
            // whenever we encounter the first - so at least one - element in the iterator.
            <StoredMatches<T>>::iter_prefix_values(&source).any(|_| true)
        }

        /// Checks of a new job schedule fits with the existing schedule for a processor.
        fn fits_schedule(
            source: &T::AccountId,
            schedule: &Schedule,
            start_delay: u64,
        ) -> Result<(), DispatchError> {
            for (job_id, assignment) in <StoredMatches<T>>::iter_prefix(&source) {
                // TODO decide tradeoff: we could save this lookup at the cost of storing the schedule along with the match or even completly move it from StoredJobRegistration into StoredMatches
                let other = <StoredJobRegistration<T>>::get(&job_id.0, &job_id.1)
                    .ok_or(pallet_acurast::Error::<T>::JobRegistrationNotFound)?;

                // check if the whole schedule periods have an overlap
                if schedule.start_time >= other.schedule.end_time
                    || schedule.end_time <= other.schedule.start_time
                {
                    // periods don't overlap
                    continue;
                }

                let it = schedule
                    .iter(start_delay)
                    .ok_or(Error::<T>::CalculationOverflow)?
                    .map(|start| {
                        let end = start.checked_add(schedule.interval)?;
                        Some((start, end))
                    });
                let other_it = other
                    .schedule
                    .iter(assignment.start_delay)
                    .ok_or(Error::<T>::CalculationOverflow)?
                    .map(|start| {
                        let end = start.checked_add(other.schedule.interval)?;
                        Some((start, end))
                    });

                it.merge(other_it).try_fold(0u64, |prev_end, bounds| {
                    let (start, end) = bounds.ok_or(Error::<T>::CalculationOverflow)?;

                    if prev_end > start {
                        Err(Error::<T>::ScheduleOverlapInMatch)
                    } else {
                        Ok(end)
                    }
                })?;
            }

            Ok(().into())
        }

        /// Calculates the total reward amount.
        fn total_reward_amount(
            registration: &JobRegistrationFor<T>,
        ) -> Result<T::AssetAmount, Error<T>> {
            let e: <T as Config>::RegistrationExtra = registration.extra.clone().into();
            let requirements: JobRequirementsFor<T> = e.into();

            let reward_amount: T::AssetAmount = requirements
                .reward
                .try_get_amount()
                .map_err(|_| Error::<T>::JobRegistrationUnsupportedReward)?
                .into();

            Ok(reward_amount
                .checked_mul(&((requirements.slots as u128).into()))
                .ok_or(Error::<T>::CalculationOverflow)?
                .checked_mul(&registration.schedule.execution_count().into())
                .ok_or(Error::<T>::CalculationOverflow)?)
        }

        /// Calculates the fee per job execution.
        fn fee_per_execution(
            registration: &JobRegistrationFor<T>,
            pricing: &PricingVariantFor<T>,
        ) -> Result<T::AssetAmount, Error<T>> {
            Ok(pricing
                .fee_per_millisecond
                .checked_mul(&registration.schedule.duration.into())
                .ok_or(Error::<T>::CalculationOverflow)?
                .checked_add(
                    &pricing
                        .fee_per_storage_byte
                        .checked_mul(&registration.storage.into())
                        .ok_or(Error::<T>::CalculationOverflow)?,
                )
                .ok_or(Error::<T>::CalculationOverflow)?
                .checked_add(&pricing.base_fee_per_execution)
                .ok_or(Error::<T>::CalculationOverflow)?)
        }

        /// Returns the current timestamp.
        fn now() -> Result<u64, DispatchError> {
            Ok(<T as pallet_acurast::Config>::UnixTime::now()
                .as_millis()
                .try_into()
                .map_err(|_| pallet_acurast::Error::<T>::FailedTimestampConversion)?)
        }
    }
}
