#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod traits;
pub mod utils;
pub mod weights;
pub mod xcm_adapters;

pub use acurast_common::*;
pub use pallet::*;
pub use traits::*;

pub type JobRegistrationFor<T> =
    JobRegistration<<T as frame_system::Config>::AccountId, <T as Config>::RegistrationExtra>;

#[frame_support::pallet]
pub mod pallet {
    use acurast_common::*;
    use frame_support::{
        dispatch::DispatchResultWithPostInfo, ensure, pallet_prelude::*,
        sp_runtime::traits::StaticLookup, traits::UnixTime, Blake2_128Concat, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_std::prelude::*;

    use crate::{traits::*, utils::*, JobRegistrationFor};

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// Extra structure to include in the registration of a job.
        type RegistrationExtra: Parameter + Member;
        /// The fulfillment router to route a job fulfillment to its final destination.
        type FulfillmentRouter: FulfillmentRouter<Self>;
        /// The max length of the allowed sources list for a registration.
        #[pallet::constant]
        type MaxAllowedSources: Get<u16>;
        /// The ID for this pallet
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        /// Barrier for the update_certificate_revocation_list extrinsic call.
        type RevocationListUpdateBarrier: RevocationListUpdateBarrier<Self>;
        /// Barrier for update_job_assignments extrinsic call.
        type JobAssignmentUpdateBarrier: JobAssignmentUpdateBarrier<Self>;
        /// Barrier for submit_attestation extrinsic call.
        type KeyAttestationBarrier: KeyAttestationBarrier<Self>;
        /// Timestamp
        type UnixTime: UnixTime;
        /// Hooks used by tightly coupled subpallets.
        type JobHooks: JobHooks<Self>;
        /// Weight Info for extrinsics. Needs to include weight of hooks called. The weights in this pallet or only correct when using the default hooks [()].
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    /// The storage for [JobRegistration]s. They are stored by [AccountId] and [Script].
    #[pallet::storage]
    #[pallet::getter(fn stored_job_registration)]
    pub type StoredJobRegistration<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        Script,
        JobRegistrationFor<T>,
    >;

    /// The storage for [Attestation]s. They are stored by [AccountId].
    #[pallet::storage]
    #[pallet::getter(fn stored_attestation)]
    pub type StoredAttestation<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Attestation>;

    /// Certificate revocation list storage.
    #[pallet::storage]
    #[pallet::getter(fn stored_revoked_certificate)]
    pub type StoredRevokedCertificate<T: Config> =
        StorageMap<_, Blake2_128Concat, SerialNumber, ()>;

    /// Job assignments.
    #[pallet::storage]
    #[pallet::getter(fn stored_job_assignment)]
    pub type StoredJobAssignment<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<JobId<T::AccountId>>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A registration was successfully stored. [registration, who]
        JobRegistrationStored(JobRegistrationFor<T>, T::AccountId),
        /// A registration was successfully removed. [registration, who]
        JobRegistrationRemoved(Script, T::AccountId),
        /// A fulfillment has been posted. [who, fulfillment, registration, receiver]
        ReceivedFulfillment(
            T::AccountId,
            Fulfillment,
            JobRegistrationFor<T>,
            T::AccountId,
        ),
        /// The allowed sources have been updated. [who, old_registration, updates]
        AllowedSourcesUpdated(
            T::AccountId,
            JobRegistrationFor<T>,
            Vec<AllowedSourcesUpdate<T::AccountId>>,
        ),
        /// An attestation was successfully stored. [attestation, who]
        AttestationStored(Attestation, T::AccountId),
        /// The certificate revocation list has been updated. [who, updates]
        CertificateRecovationListUpdated(T::AccountId, Vec<CertificateRevocationListUpdate>),
        /// The job assignemts have been updated. [who, updates]
        JobAssignmentUpdate(T::AccountId, Vec<JobAssignmentUpdate<T::AccountId>>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Fulfill was executed for a not registered job.
        JobRegistrationNotFound,
        /// The source of the fulfill is not allowed for the job.
        FulfillSourceNotAllowed,
        /// The source of the fulfill is not verified. The source does not have a valid attestation submitted.
        FulfillSourceNotVerified,
        /// The allowed soruces list for a registration exeeded the max length.
        TooManyAllowedSources,
        /// The allowed soruces list for a registration cannot be empty if provided.
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
        /// Attestation was rejected by [Config::KeyAttestationBarrier].
        AttestationRejected,
        /// Timestamp error.
        FailedTimestampConversion,
        /// Certificate was revoked.
        RevokedCertificate,
        /// Origin is not allowed to update the certificate revocation list.
        CertificateRevocationListUpdateNotAllowed,
        /// The attestation was issued for an unsupported public key type.
        UnsupportedAttestationPublicKeyType,
        /// The submitted attestation public key does not match the source.
        AttestationPublicKeyDoesNotMatchSource,
        /// Job assignment update not allowed.
        JobAssignmentUpdateNotAllowed,
        /// Calling a job hook produced an error.
        JobHookFailed,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Registers a job by providing a [JobRegistration]. If a job for the same script was previously registered, it will be overwritten.
        #[pallet::weight(< T as Config >::WeightInfo::register())]
        pub fn register(
            origin: OriginFor<T>,
            registration: JobRegistrationFor<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!(
                is_valid_script(&registration.script),
                Error::<T>::InvalidScriptValue
            );
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

            <StoredJobRegistration<T>>::insert(&who, &registration.script, registration.clone());

            <T as Config>::JobHooks::register_hook(&who, &registration)?;

            Self::deposit_event(Event::JobRegistrationStored(registration, who));
            Ok(().into())
        }

        /// Deregisters a job for the given script.
        #[pallet::weight(< T as Config >::WeightInfo::deregister())]
        pub fn deregister(origin: OriginFor<T>, script: Script) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            <StoredJobRegistration<T>>::remove(&who, &script);

            <T as Config>::JobHooks::deregister_hook(&who, &script)?;

            Self::deposit_event(Event::JobRegistrationRemoved(script, who));
            Ok(().into())
        }

        /// Updates the allowed sources list of a [JobRegistration].
        #[pallet::weight(< T as Config >::WeightInfo::update_allowed_sources())]
        pub fn update_allowed_sources(
            origin: OriginFor<T>,
            script: Script,
            updates: Vec<AllowedSourcesUpdate<T::AccountId>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let registration = <StoredJobRegistration<T>>::get(&who, &script)
                .ok_or(Error::<T>::JobRegistrationNotFound)?;

            let mut current_allowed_sources =
                registration.allowed_sources.clone().unwrap_or_default();
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
            <StoredJobRegistration<T>>::insert(
                &who,
                &script,
                JobRegistration {
                    script: script.clone(),
                    allowed_sources,
                    extra: registration.extra.clone(),
                    allow_only_verified_sources: registration.allow_only_verified_sources,
                },
            );

            <T as Config>::JobHooks::update_allowed_sources_hook(&who, &script, &updates)?;

            Self::deposit_event(Event::AllowedSourcesUpdated(who, registration, updates));

            Ok(().into())
        }

        /// Assigns jobs to [AccountId]s. Those accounts can then later call `fulfill` for those jobs.
        #[pallet::weight(< T as Config >::WeightInfo::update_job_assignments())]
        pub fn update_job_assignments(
            origin: OriginFor<T>,
            updates: Vec<JobAssignmentUpdate<T::AccountId>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            if !T::JobAssignmentUpdateBarrier::can_update_assigned_jobs(&who, &updates) {
                return Err(Error::<T>::JobAssignmentUpdateNotAllowed)?;
            }
            for update in &updates {
                let job_registration =
                    <StoredJobRegistration<T>>::get(&update.job_id.0, &update.job_id.1)
                        .ok_or(Error::<T>::JobRegistrationNotFound)?;

                ensure_source_allowed::<T>(&update.assignee, &job_registration)?;

                let mut assignments =
                    <StoredJobAssignment<T>>::get(&update.assignee).unwrap_or_default();
                if !assignments.contains(&update.job_id) {
                    assignments.push(update.job_id.clone());
                }
                <StoredJobAssignment<T>>::set(&update.assignee, Some(assignments));
            }
            Self::deposit_event(Event::JobAssignmentUpdate(who, updates));
            Ok(().into())
        }

        /// Fulfills a previously registered job.
        #[pallet::weight(< T as Config >::WeightInfo::fulfill())]
        pub fn fulfill(
            origin: OriginFor<T>,
            fulfillment: Fulfillment,
            requester: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin.clone())?;
            let requester = T::Lookup::lookup(requester)?;

            // find registration
            let registration = <StoredJobRegistration<T>>::get(&requester, &fulfillment.script)
                .ok_or(Error::<T>::JobRegistrationNotFound)?;

            <T as Config>::JobHooks::fulfill_hook(
                &who,
                &fulfillment,
                requester.clone(),
                &registration,
            )?;

            let info = T::FulfillmentRouter::received_fulfillment(
                origin,
                who.clone(),
                fulfillment.clone(),
                registration.clone(),
                requester.clone(),
            )?;

            <StoredJobRegistration<T>>::remove(&requester, &fulfillment.script);

            Self::deposit_event(Event::ReceivedFulfillment(
                who,
                fulfillment,
                registration,
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
        #[pallet::weight(< T as Config >::WeightInfo::submit_attestation())]
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

            if !T::KeyAttestationBarrier::accept_attestation_for_origin(&who, &attestation) {
                return Err(Error::<T>::AttestationRejected.into());
            }

            ensure_not_expired::<T>(&attestation)?;
            ensure_not_revoked::<T>(&attestation)?;

            <StoredAttestation<T>>::insert(&who, attestation.clone());
            Self::deposit_event(Event::AttestationStored(attestation, who));
            Ok(().into())
        }

        #[pallet::weight(< T as Config >::WeightInfo::register())]
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
}
