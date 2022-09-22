#![cfg_attr(not(feature = "std"), no_std)]

pub mod attestation;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use core::convert::TryFrom;

    use crate::attestation::{asn::KeyDescription, *};
    use codec::{Decode, Encode};
    use frame_support::{
        dispatch::DispatchResultWithPostInfo,
        ensure,
        pallet_prelude::*,
        sp_runtime::traits::{MaybeDisplay, StaticLookup},
        storage::bounded_vec::BoundedVec,
        Blake2_128Concat,
    };
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_std::prelude::*;

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

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_timestamp::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// Extra structure to include in the registration of a job.
        type RegistrationExtra: Parameter + Member + MaxEncodedLen;
        /// The fulfillment router to route a job fulfillment to its final destination.
        type FulfillmentRouter: FulfillmentRouter<Self>;
        /// The max length of the allowed sources list for a registration.
        #[pallet::constant]
        type MaxAllowedSources: Get<u16>;
        /// AccountIDs that are allowed to call update_certificate_revocation_list.
        #[pallet::constant]
        type AllowedRevocationListUpdate: Get<Vec<Self::AccountId>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    const SCRIPT_PREFIX: &'static [u8] = b"ipfs://";
    const SCRIPT_LENGTH: u32 = 53;

    /// Type representing the utf8 bytes of a string containing the value of an ipfs url.
    /// The ipfs url is expected to point to a script.
    pub type Script = BoundedVec<u8, ConstU32<SCRIPT_LENGTH>>;

    /// Structure representing a job fulfillment. It contains the script that generated the payload and the actual payload.
    #[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
    pub struct Fulfillment {
        /// The script that generated the payload.
        pub script: Script,
        /// The output of a script.
        pub payload: Vec<u8>,
    }

    /// Structure representing a job registration.
    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub struct JobRegistration<A, T>
    where
        A: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord + MaxEncodedLen,
        T: Parameter + Member + MaxEncodedLen,
    {
        /// The script to execute. It is a vector of bytes representing a utf8 string. The string needs to be a ipfs url that points to the script.
        pub script: Script,
        /// An optional array of the [AccountId]s allowed to fulfill the job. If the array is [None], then all sources are allowed.
        pub allowed_sources: Option<Vec<A>>,
        /// A boolean indicating if only verified sources can fulfill the job. A verified source is one that has provided a valid key attestation.
        pub allow_only_verified_sources: bool,
        /// Extra parameters. This type can be configured through [Config::RegistrationExtra].
        pub extra: T,
    }

    /// Structure used to updated the allowed sources list of a [Registration].
    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub struct AllowedSourcesUpdate<A>
    where
        A: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord + MaxEncodedLen,
    {
        /// The update operation
        pub operation: ListUpdateOperation,
        /// The [AccountId] to add or remove.
        pub account_id: A,
    }

    /// Structure used to updated the certificate recovation list.
    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub struct CertificateRevocationListUpdate {
        /// The update operation
        pub operation: ListUpdateOperation,
        /// The [AccountId] to add or remove.
        pub cert_serial_number: SerialNumber,
    }

    /// The allowed sources update operation.
    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Copy)]
    pub enum ListUpdateOperation {
        Add,
        Remove,
    }

    /// The storage for [Registration]s. They are stored by [AccountId] and [Script].
    #[pallet::storage]
    #[pallet::getter(fn stored_registration)]
    pub type StoredRegistration<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        Script,
        JobRegistration<T::AccountId, T::RegistrationExtra>,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A registration was successfully stored. [registration, who]
        JobRegistrationStored(
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
        /// Timestamp error
        FailedTimestampConversion,
        /// Certificate was revoked
        RevokedCertificate,
        /// Origin is not allowed to update the certificate revocation list
        CertificateRevocationListUpdateNotAllowed,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Registers a job by providing a [Registration]. If a job for the same script was previously registered, it will be overwritten.
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
        pub fn register(
            origin: OriginFor<T>,
            registration: JobRegistration<T::AccountId, T::RegistrationExtra>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let script_len = (&registration).script.len() as u32;
            ensure!(
                script_len == SCRIPT_LENGTH && (&registration).script.starts_with(SCRIPT_PREFIX),
                Error::<T>::InvalidScriptValue
            );
            let allowed_sources_len = (&registration)
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
            <StoredRegistration<T>>::insert(
                who.clone(),
                (&registration).script.clone(),
                registration.clone(),
            );
            Self::deposit_event(Event::JobRegistrationStored(registration, who));
            Ok(().into())
        }

        /// Deregisters a job for the given script.
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
        pub fn deregister(origin: OriginFor<T>, script: Script) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            <StoredRegistration<T>>::remove(who.clone(), script.clone());
            Self::deposit_event(Event::JobRegistrationRemoved(script, who));
            Ok(().into())
        }

        /// Updates the allowed sources list of a [Registration].
        #[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1, 1))]
        pub fn update_allowed_sources(
            origin: OriginFor<T>,
            script: Script,
            updates: Vec<AllowedSourcesUpdate<T::AccountId>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let registration = <StoredRegistration<T>>::get(who.clone(), script.clone())
                .ok_or(Error::<T>::JobRegistrationNotFound)?;

            let mut current_allowed_sources =
                (&registration).allowed_sources.clone().unwrap_or(vec![]);
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
            <StoredRegistration<T>>::insert(
                who.clone(),
                script.clone(),
                JobRegistration {
                    script,
                    allowed_sources,
                    extra: (&registration).extra.clone(),
                    allow_only_verified_sources: (&registration).allow_only_verified_sources,
                },
            );

            Self::deposit_event(Event::AllowedSourcesUpdated(who, registration, updates));

            Ok(().into())
        }

        /// Fulfills a previously registered job.
        #[pallet::weight(10_000 + T::DbWeight::get().reads(7))]
        pub fn fulfill(
            origin: OriginFor<T>,
            fulfillment: Fulfillment,
            requester: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin.clone())?;
            let requester = T::Lookup::lookup(requester)?;

            let registration =
                <StoredRegistration<T>>::get(requester.clone(), (&fulfillment).script.clone())
                    .ok_or(Error::<T>::JobRegistrationNotFound)?;

            ensure_source_allowed::<T>(&who, &registration)?;

            let info = T::FulfillmentRouter::received_fulfillment(
                origin,
                who.clone(),
                fulfillment.clone(),
                registration.clone(),
                requester.clone(),
            )?;
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
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
        pub fn submit_attestation(
            origin: OriginFor<T>,
            attestation_chain: AttestationChain,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!(
                (&attestation_chain).certificate_chain.len() >= 2,
                Error::<T>::CertificateChainTooShort,
            );

            let attestation = validate_and_extract_attestation::<T>(&attestation_chain)?;

            ensure_not_expired::<T>(&attestation)?;
            ensure_not_revoked::<T>(&attestation)?;

            <StoredAttestation<T>>::insert(who.clone(), attestation.clone());
            Self::deposit_event(Event::AttestationStored(attestation, who));
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn update_certificate_revocation_list(
            origin: OriginFor<T>,
            updates: Vec<CertificateRevocationListUpdate>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            if !T::AllowedRevocationListUpdate::get().contains(&who) {
                return Err(Error::<T>::CertificateRevocationListUpdateNotAllowed)?;
            }
            for update in &updates {
                match &update.operation {
                    ListUpdateOperation::Add => {
                        <StoredRevokedCertificate<T>>::insert(
                            update.cert_serial_number.clone(),
                            (),
                        );
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

    pub(crate) fn validate_and_extract_attestation<T: Config>(
        attestation_chain: &AttestationChain,
    ) -> Result<Attestation, Error<T>> {
        validate_certificate_chain_root(&attestation_chain.certificate_chain)
            .map_err(|_| Error::<T>::RootCertificateValidationFailed)?;

        let (cert_ids, cert) = validate_certificate_chain(&attestation_chain.certificate_chain)
            .map_err(|_| Error::<T>::CertificateChainValidationFailed)?;

        let attestation_validity = AttestationValidity {
            not_before: cert.validity.not_before.timestamp_millis(),
            not_after: cert.validity.not_after.timestamp_millis(),
        };

        let key_description = extract_attestation(cert.extensions)
            .map_err(|_| Error::<T>::AttestationExtractionFailed)?;

        let cert_ids_bounded = cert_ids
            .into_iter()
            .map(|cert_id| {
                let (iss, sn) = cert_id;
                let iss_bounded = IssuerName::try_from(iss)
                    .map_err(|_| Error::<T>::CannotGetAttestationIssuerName)?;
                let sn_bounded = SerialNumber::try_from(sn)
                    .map_err(|_| Error::<T>::CannotGetAttestationSerialNumber)?;
                Ok((iss_bounded, sn_bounded))
            })
            .collect::<Result<Vec<CertId>, Error<T>>>()?;
        let cert_ids_bounded_vec = ValidatingCertIds::try_from(cert_ids_bounded)
            .map_err(|_| Error::<T>::CannotGetCertificateId)?;

        Ok(Attestation {
            cert_ids: cert_ids_bounded_vec,
            key_description: key_description
                .try_into()
                .map_err(|_| Error::<T>::AttestationToBoundedTypeConversionFailed)?,
            validity: attestation_validity,
        })
    }

    fn ensure_source_allowed<T: Config>(
        source: &T::AccountId,
        registration: &JobRegistration<T::AccountId, T::RegistrationExtra>,
    ) -> Result<(), Error<T>> {
        registration
            .allowed_sources
            .as_ref()
            .map(|allowed_sources| {
                allowed_sources
                    .iter()
                    .position(|allowed_source| allowed_source == source)
                    .map(|_| ())
                    .ok_or(Error::<T>::FulfillSourceNotAllowed)
            })
            .unwrap_or(Ok(()))?;

        if registration.allow_only_verified_sources {
            let attestation =
                <StoredAttestation<T>>::get(source).ok_or(Error::<T>::FulfillSourceNotVerified)?;
            ensure_not_expired(&attestation)?;
            ensure_not_revoked(&attestation)?;
        }

        Ok(())
    }

    fn ensure_not_expired<T: Config>(attestation: &Attestation) -> Result<(), Error<T>> {
        let now: u64 = <pallet_timestamp::Pallet<T>>::now()
            .try_into()
            .map_err(|_| Error::<T>::FailedTimestampConversion)?;

        if now >= attestation.validity.not_after || now < attestation.validity.not_before {
            return Err(Error::<T>::AttestationCertificateNotValid);
        }
        let expire_date_time = (&attestation)
            .key_description
            .tee_enforced
            .usage_expire_date_time
            .or_else(|| {
                (&attestation)
                    .key_description
                    .software_enforced
                    .usage_expire_date_time
            });
        if let Some(expire_date_time) = expire_date_time {
            if now >= expire_date_time {
                return Err(Error::<T>::AttestationUsageExpired);
            }
        }
        Ok(())
    }

    fn ensure_not_revoked<T: Config>(attestation: &Attestation) -> Result<(), Error<T>> {
        let ids = &attestation.cert_ids;
        for id in ids {
            if <StoredRevokedCertificate<T>>::get(&id.1).is_some() {
                return Err(Error::<T>::RevokedCertificate);
            }
        }
        Ok(())
    }

    /// The storage for [Attestation]s. They are stored by [AccountId].
    #[pallet::storage]
    #[pallet::getter(fn stored_attestation)]
    pub type StoredAttestation<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Attestation>;

    #[pallet::storage]
    #[pallet::getter(fn stored_revoked_certificate)]
    pub type StoredRevokedCertificate<T: Config> =
        StorageMap<_, Blake2_128Concat, SerialNumber, ()>;

    /// https://datatracker.ietf.org/doc/html/rfc5280#section-4.1.2.2
    const ISSUER_NAME_MAX_LENGTH: u32 = 64;
    const SERIAL_NUMBER_MAX_LENGTH: u32 = 20;

    pub type IssuerName = BoundedVec<u8, ConstU32<ISSUER_NAME_MAX_LENGTH>>;
    pub type SerialNumber = BoundedVec<u8, ConstU32<SERIAL_NUMBER_MAX_LENGTH>>;

    const PURPOSE_MAX_LENGTH: u32 = 50;
    const DIGEST_MAX_LENGTH: u32 = 32;
    const PADDING_MAX_LENGTH: u32 = 32;
    const MGF_DIGEST_MAX_LENGTH: u32 = 32;
    const VERIFIED_BOOT_KEY_MAX_LENGTH: u32 = 32;
    const VERIFIED_BOOT_HASH_MAX_LENGTH: u32 = 32;
    const ATTESTATION_ID_MAX_LENGTH: u32 = 256;
    const BOUDNED_SET_PROPERTY: u32 = 16;

    pub type Purpose = BoundedVec<u8, ConstU32<PURPOSE_MAX_LENGTH>>;
    pub type Digest = BoundedVec<u8, ConstU32<DIGEST_MAX_LENGTH>>;
    pub type Padding = BoundedVec<u8, ConstU32<PADDING_MAX_LENGTH>>;
    pub type MgfDigest = BoundedVec<u8, ConstU32<MGF_DIGEST_MAX_LENGTH>>;
    pub type VerifiedBootKey = BoundedVec<u8, ConstU32<VERIFIED_BOOT_KEY_MAX_LENGTH>>;
    pub type VerifiedBootHash = BoundedVec<u8, ConstU32<VERIFIED_BOOT_HASH_MAX_LENGTH>>;
    pub type AttestationIdProperty = BoundedVec<u8, ConstU32<ATTESTATION_ID_MAX_LENGTH>>;
    pub type CertId = (IssuerName, SerialNumber);
    pub type ValidatingCertIds = BoundedVec<CertId, ConstU32<CHAIN_MAX_LENGTH>>;
    pub type BoundedSetProperty = BoundedVec<CertId, ConstU32<BOUDNED_SET_PROPERTY>>;

    /// Structure representing a submitted attestation chain.
    #[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
    pub struct AttestationChain {
        /// An ordered array of [CertificateInput]s describing a valid chain from known root certificate to attestation certificate.
        pub certificate_chain: CertificateChainInput,
    }

    /// Structure representing a stored attestation.
    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub struct Attestation {
        pub cert_ids: ValidatingCertIds,
        pub key_description: BoundedKeyDescription,
        pub validity: AttestationValidity,
    }

    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Copy, PartialEq)]
    pub struct AttestationValidity {
        pub not_before: u64,
        pub not_after: u64,
    }

    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub struct BoundedKeyDescription {
        pub attestation_security_level: AttestationSecurityLevel,
        pub key_mint_security_level: AttestationSecurityLevel,
        pub software_enforced: BoundedAuthorizationList,
        pub tee_enforced: BoundedAuthorizationList,
    }

    impl TryFrom<KeyDescription<'_>> for BoundedKeyDescription {
        type Error = ();

        fn try_from(value: KeyDescription) -> Result<Self, Self::Error> {
            match value {
                KeyDescription::V1(kd) => kd.try_into(),
                KeyDescription::V2(kd) => kd.try_into(),
                KeyDescription::V3(kd) => kd.try_into(),
                KeyDescription::V4(kd) => kd.try_into(),
                KeyDescription::V100(kd) => kd.try_into(),
                KeyDescription::V200(kd) => kd.try_into(),
            }
        }
    }

    use crate::attestation::asn;

    impl TryFrom<asn::KeyDescriptionV1<'_>> for BoundedKeyDescription {
        type Error = ();

        fn try_from(data: asn::KeyDescriptionV1) -> Result<Self, Self::Error> {
            Ok(BoundedKeyDescription {
                attestation_security_level: data.attestation_security_level.into(),
                key_mint_security_level: data.key_mint_security_level.into(),
                software_enforced: data.software_enforced.try_into()?,
                tee_enforced: data.tee_enforced.try_into()?,
            })
        }
    }

    impl TryFrom<asn::KeyDescriptionV2<'_>> for BoundedKeyDescription {
        type Error = ();

        fn try_from(data: asn::KeyDescriptionV2) -> Result<Self, Self::Error> {
            Ok(BoundedKeyDescription {
                attestation_security_level: data.attestation_security_level.into(),
                key_mint_security_level: data.key_mint_security_level.into(),
                software_enforced: data.software_enforced.try_into()?,
                tee_enforced: data.tee_enforced.try_into()?,
            })
        }
    }

    impl TryFrom<asn::KeyDescriptionV3<'_>> for BoundedKeyDescription {
        type Error = ();

        fn try_from(data: asn::KeyDescriptionV3) -> Result<Self, Self::Error> {
            Ok(BoundedKeyDescription {
                attestation_security_level: data.attestation_security_level.into(),
                key_mint_security_level: data.key_mint_security_level.into(),
                software_enforced: data.software_enforced.try_into()?,
                tee_enforced: data.tee_enforced.try_into()?,
            })
        }
    }

    impl TryFrom<asn::KeyDescriptionV4<'_>> for BoundedKeyDescription {
        type Error = ();

        fn try_from(data: asn::KeyDescriptionV4) -> Result<Self, Self::Error> {
            Ok(BoundedKeyDescription {
                attestation_security_level: data.attestation_security_level.into(),
                key_mint_security_level: data.key_mint_security_level.into(),
                software_enforced: data.software_enforced.try_into()?,
                tee_enforced: data.tee_enforced.try_into()?,
            })
        }
    }

    impl TryFrom<asn::KeyDescriptionV100V200<'_>> for BoundedKeyDescription {
        type Error = ();

        fn try_from(data: asn::KeyDescriptionV100V200) -> Result<Self, Self::Error> {
            Ok(BoundedKeyDescription {
                attestation_security_level: data.attestation_security_level.into(),
                key_mint_security_level: data.key_mint_security_level.into(),
                software_enforced: data.software_enforced.try_into()?,
                tee_enforced: data.tee_enforced.try_into()?,
            })
        }
    }

    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub enum AttestationSecurityLevel {
        Software,
        TrustedEnvironemnt,
        StrongBox,
        Unknown,
    }

    impl From<asn::SecurityLevel> for AttestationSecurityLevel {
        fn from(data: asn::SecurityLevel) -> Self {
            match data.value() {
                0 => AttestationSecurityLevel::Software,
                1 => AttestationSecurityLevel::TrustedEnvironemnt,
                2 => AttestationSecurityLevel::StrongBox,
                _ => AttestationSecurityLevel::Unknown,
            }
        }
    }

    /// The Authorization List tags. [Tag descriptions](https://source.android.com/docs/security/keystore/tags)
    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub struct BoundedAuthorizationList {
        pub purpose: Option<Purpose>,
        pub algorithm: Option<u8>,
        pub key_size: Option<u16>,
        pub digest: Option<Digest>,
        pub padding: Option<Padding>,
        pub ec_curve: Option<u8>,
        pub rsa_public_exponent: Option<u64>,
        pub mgf_digest: Option<MgfDigest>,
        pub rollback_resistance: Option<bool>,
        pub early_boot_only: Option<bool>,
        pub active_date_time: Option<u64>,
        pub origination_expire_date_time: Option<u64>,
        pub usage_expire_date_time: Option<u64>,
        pub usage_count_limit: Option<u64>,
        pub no_auth_required: bool,
        pub user_auth_type: Option<u8>,
        pub auth_timeout: Option<u32>,
        pub allow_while_on_body: bool,
        pub trusted_user_presence_required: Option<bool>,
        pub trusted_confirmation_required: Option<bool>,
        pub unlocked_device_required: Option<bool>,
        pub all_applications: Option<bool>,
        pub application_id: Option<AttestationIdProperty>,
        pub creation_date_time: Option<u64>,
        pub origin: Option<u8>,
        pub root_of_trust: Option<BoundedRootOfTrust>,
        pub os_version: Option<u32>,
        pub os_patch_level: Option<u32>,
        pub attestation_application_id: Option<AttestationIdProperty>,
        pub attestation_id_brand: Option<AttestationIdProperty>,
        pub attestation_id_device: Option<AttestationIdProperty>,
        pub attestation_id_product: Option<AttestationIdProperty>,
        pub attestation_id_serial: Option<AttestationIdProperty>,
        pub attestation_id_imei: Option<AttestationIdProperty>,
        pub attestation_id_meid: Option<AttestationIdProperty>,
        pub attestation_id_manufacturer: Option<AttestationIdProperty>,
        pub attestation_id_model: Option<AttestationIdProperty>,
        pub vendor_patch_level: Option<u32>,
        pub boot_patch_level: Option<u32>,
        pub device_unique_attestation: Option<bool>,
    }

    macro_rules! try_bound_set {
        ( $set:expr, $target_vec_type:ty, $target_type:ty ) => {{
            $set.map(|v| {
                v.map(|i| <$target_type>::try_from(i))
                    .collect::<Result<Vec<$target_type>, _>>()
            })
            .map_or(Ok(None), |r| r.map(Some))
            .map_err(|_| ())?
            .map(|v| <$target_vec_type>::try_from(v))
            .map_or(Ok(None), |r| r.map(Some))
        }};
    }

    macro_rules! try_bound {
        ( $v:expr, $target_type:ty ) => {{
            $v.map(|v| <$target_type>::try_from(v))
                .map_or(Ok(None), |r| r.map(Some))
                .map_err(|_| ())
        }};
    }

    impl TryFrom<asn::AuthorizationListV1<'_>> for BoundedAuthorizationList {
        type Error = ();

        fn try_from(data: asn::AuthorizationListV1) -> Result<Self, Self::Error> {
            Ok(BoundedAuthorizationList {
                purpose: try_bound_set!(data.purpose, Purpose, u8)?,
                algorithm: try_bound!(data.algorithm, u8)?,
                key_size: try_bound!(data.key_size, u16)?,
                digest: try_bound_set!(data.digest, Digest, u8)?,
                padding: try_bound_set!(data.padding, Padding, u8)?,
                ec_curve: try_bound!(data.ec_curve, u8)?,
                rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
                mgf_digest: None,
                rollback_resistance: Some(data.rollback_resistance.is_some()),
                early_boot_only: None,
                active_date_time: try_bound!(data.active_date_time, u64)?,
                origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
                usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
                usage_count_limit: None,
                no_auth_required: data.no_auth_required.is_some(),
                user_auth_type: try_bound!(data.user_auth_type, u8)?,
                auth_timeout: try_bound!(data.user_auth_type, u32)?,
                allow_while_on_body: data.allow_while_on_body.is_some(),
                trusted_user_presence_required: None,
                trusted_confirmation_required: None,
                unlocked_device_required: None,
                all_applications: Some(data.all_applications.is_some()),
                application_id: data
                    .application_id
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                creation_date_time: try_bound!(data.creation_date_time, u64)?,
                origin: try_bound!(data.origin, u8)?,
                root_of_trust: data
                    .root_of_trust
                    .map(|v| v.try_into())
                    .map_or(Ok(None), |r| r.map(Some))?,
                os_version: try_bound!(data.os_version, u32)?,
                os_patch_level: try_bound!(data.os_patch_level, u32)?,
                vendor_patch_level: None,
                attestation_application_id: None,
                attestation_id_brand: None,
                attestation_id_device: None,
                attestation_id_product: None,
                attestation_id_serial: None,
                attestation_id_imei: None,
                attestation_id_meid: None,
                attestation_id_manufacturer: None,
                attestation_id_model: None,
                boot_patch_level: None,
                device_unique_attestation: None,
            })
        }
    }

    impl TryFrom<asn::AuthorizationListV2<'_>> for BoundedAuthorizationList {
        type Error = ();

        fn try_from(data: asn::AuthorizationListV2) -> Result<Self, Self::Error> {
            Ok(BoundedAuthorizationList {
                purpose: try_bound_set!(data.purpose, Purpose, u8)?,
                algorithm: try_bound!(data.algorithm, u8)?,
                key_size: try_bound!(data.key_size, u16)?,
                digest: try_bound_set!(data.digest, Digest, u8)?,
                padding: try_bound_set!(data.padding, Padding, u8)?,
                ec_curve: try_bound!(data.ec_curve, u8)?,
                rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
                mgf_digest: None,
                rollback_resistance: Some(data.rollback_resistance.is_some()),
                early_boot_only: None,
                active_date_time: try_bound!(data.active_date_time, u64)?,
                origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
                usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
                usage_count_limit: None,
                no_auth_required: data.no_auth_required.is_some(),
                user_auth_type: try_bound!(data.user_auth_type, u8)?,
                auth_timeout: try_bound!(data.user_auth_type, u32)?,
                allow_while_on_body: data.allow_while_on_body.is_some(),
                trusted_user_presence_required: None,
                trusted_confirmation_required: None,
                unlocked_device_required: None,
                all_applications: Some(data.all_applications.is_some()),
                application_id: data
                    .application_id
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                creation_date_time: try_bound!(data.creation_date_time, u64)?,
                origin: try_bound!(data.origin, u8)?,
                root_of_trust: data
                    .root_of_trust
                    .map(|v| v.try_into())
                    .map_or(Ok(None), |r| r.map(Some))?,
                os_version: try_bound!(data.os_version, u32)?,
                os_patch_level: try_bound!(data.os_patch_level, u32)?,
                attestation_application_id: data
                    .attestation_application_id
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_brand: data
                    .attestation_id_brand
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_device: data
                    .attestation_id_device
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_product: data
                    .attestation_id_product
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_serial: data
                    .attestation_id_serial
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_imei: data
                    .attestation_id_imei
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_meid: data
                    .attestation_id_meid
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_manufacturer: data
                    .attestation_id_manufacturer
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_model: data
                    .attestation_id_model
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                vendor_patch_level: None,
                boot_patch_level: None,
                device_unique_attestation: None,
            })
        }
    }

    impl TryFrom<asn::AuthorizationListV3<'_>> for BoundedAuthorizationList {
        type Error = ();

        fn try_from(data: asn::AuthorizationListV3) -> Result<Self, Self::Error> {
            Ok(BoundedAuthorizationList {
                purpose: try_bound_set!(data.purpose, Purpose, u8)?,
                algorithm: try_bound!(data.algorithm, u8)?,
                key_size: try_bound!(data.key_size, u16)?,
                digest: try_bound_set!(data.digest, Digest, u8)?,
                padding: try_bound_set!(data.padding, Padding, u8)?,
                ec_curve: try_bound!(data.ec_curve, u8)?,
                rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
                mgf_digest: None,
                rollback_resistance: Some(data.rollback_resistance.is_some()),
                early_boot_only: None,
                active_date_time: try_bound!(data.active_date_time, u64)?,
                origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
                usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
                usage_count_limit: None,
                no_auth_required: data.no_auth_required.is_some(),
                user_auth_type: try_bound!(data.user_auth_type, u8)?,
                auth_timeout: try_bound!(data.user_auth_type, u32)?,
                allow_while_on_body: data.allow_while_on_body.is_some(),
                trusted_user_presence_required: None,
                trusted_confirmation_required: None,
                unlocked_device_required: None,
                all_applications: Some(data.all_applications.is_some()),
                application_id: data
                    .application_id
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                creation_date_time: try_bound!(data.creation_date_time, u64)?,
                origin: try_bound!(data.origin, u8)?,
                root_of_trust: data
                    .root_of_trust
                    .map(|v| v.try_into())
                    .map_or(Ok(None), |r| r.map(Some))?,
                os_version: try_bound!(data.os_version, u32)?,
                os_patch_level: try_bound!(data.os_patch_level, u32)?,
                attestation_application_id: data
                    .attestation_application_id
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_brand: data
                    .attestation_id_brand
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_device: data
                    .attestation_id_device
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_product: data
                    .attestation_id_product
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_serial: data
                    .attestation_id_serial
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_imei: data
                    .attestation_id_imei
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_meid: data
                    .attestation_id_meid
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_manufacturer: data
                    .attestation_id_manufacturer
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_model: data
                    .attestation_id_model
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                vendor_patch_level: try_bound!(data.vendor_patch_level, u32)?,
                boot_patch_level: try_bound!(data.boot_patch_level, u32)?,
                device_unique_attestation: None,
            })
        }
    }

    impl TryFrom<asn::AuthorizationListV4<'_>> for BoundedAuthorizationList {
        type Error = ();

        fn try_from(data: asn::AuthorizationListV4) -> Result<Self, Self::Error> {
            Ok(BoundedAuthorizationList {
                purpose: try_bound_set!(data.purpose, Purpose, u8)?,
                algorithm: try_bound!(data.algorithm, u8)?,
                key_size: try_bound!(data.key_size, u16)?,
                digest: try_bound_set!(data.digest, Digest, u8)?,
                padding: try_bound_set!(data.padding, Padding, u8)?,
                ec_curve: try_bound!(data.ec_curve, u8)?,
                rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
                mgf_digest: None,
                rollback_resistance: Some(data.rollback_resistance.is_some()),
                early_boot_only: Some(data.early_boot_only.is_some()),
                active_date_time: try_bound!(data.active_date_time, u64)?,
                origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
                usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
                usage_count_limit: None,
                no_auth_required: data.no_auth_required.is_some(),
                user_auth_type: try_bound!(data.user_auth_type, u8)?,
                auth_timeout: try_bound!(data.user_auth_type, u32)?,
                allow_while_on_body: data.allow_while_on_body.is_some(),
                trusted_user_presence_required: Some(data.trusted_user_presence_required.is_some()),
                trusted_confirmation_required: Some(data.trusted_confirmation_required.is_some()),
                unlocked_device_required: Some(data.unlocked_device_required.is_some()),
                all_applications: Some(data.all_applications.is_some()),
                application_id: data
                    .application_id
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                creation_date_time: try_bound!(data.creation_date_time, u64)?,
                origin: try_bound!(data.origin, u8)?,
                root_of_trust: data
                    .root_of_trust
                    .map(|v| v.try_into())
                    .map_or(Ok(None), |r| r.map(Some))?,
                os_version: try_bound!(data.os_version, u32)?,
                os_patch_level: try_bound!(data.os_patch_level, u32)?,
                attestation_application_id: data
                    .attestation_application_id
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_brand: data
                    .attestation_id_brand
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_device: data
                    .attestation_id_device
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_product: data
                    .attestation_id_product
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_serial: data
                    .attestation_id_serial
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_imei: data
                    .attestation_id_imei
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_meid: data
                    .attestation_id_meid
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_manufacturer: data
                    .attestation_id_manufacturer
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_model: data
                    .attestation_id_model
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                vendor_patch_level: try_bound!(data.vendor_patch_level, u32)?,
                boot_patch_level: try_bound!(data.boot_patch_level, u32)?,
                device_unique_attestation: Some(data.device_unique_attestation.is_some()),
            })
        }
    }

    impl TryFrom<asn::AuthorizationListV100V200<'_>> for BoundedAuthorizationList {
        type Error = ();

        fn try_from(data: asn::AuthorizationListV100V200) -> Result<Self, Self::Error> {
            Ok(BoundedAuthorizationList {
                purpose: try_bound_set!(data.purpose, Purpose, u8)?,
                algorithm: try_bound!(data.algorithm, u8)?,
                key_size: try_bound!(data.key_size, u16)?,
                digest: try_bound_set!(data.digest, Digest, u8)?,
                padding: try_bound_set!(data.padding, Padding, u8)?,
                ec_curve: try_bound!(data.ec_curve, u8)?,
                rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
                mgf_digest: try_bound_set!(data.mgf_digest, MgfDigest, u8)?,
                rollback_resistance: Some(data.rollback_resistance.is_some()),
                early_boot_only: Some(data.early_boot_only.is_some()),
                active_date_time: try_bound!(data.active_date_time, u64)?,
                origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
                usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
                usage_count_limit: try_bound!(data.usage_count_limit, u64)?,
                no_auth_required: data.no_auth_required.is_some(),
                user_auth_type: try_bound!(data.user_auth_type, u8)?,
                auth_timeout: try_bound!(data.user_auth_type, u32)?,
                allow_while_on_body: data.allow_while_on_body.is_some(),
                trusted_user_presence_required: Some(data.trusted_user_presence_required.is_some()),
                trusted_confirmation_required: Some(data.trusted_confirmation_required.is_some()),
                unlocked_device_required: Some(data.unlocked_device_required.is_some()),
                all_applications: None,
                application_id: None,
                creation_date_time: try_bound!(data.creation_date_time, u64)?,
                origin: try_bound!(data.origin, u8)?,
                root_of_trust: data
                    .root_of_trust
                    .map(|v| v.try_into())
                    .map_or(Ok(None), |r| r.map(Some))?,
                os_version: try_bound!(data.os_version, u32)?,
                os_patch_level: try_bound!(data.os_patch_level, u32)?,
                attestation_application_id: data
                    .attestation_application_id
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_brand: data
                    .attestation_id_brand
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_device: data
                    .attestation_id_device
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_product: data
                    .attestation_id_product
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_serial: data
                    .attestation_id_serial
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_imei: data
                    .attestation_id_imei
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_meid: data
                    .attestation_id_meid
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_manufacturer: data
                    .attestation_id_manufacturer
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                attestation_id_model: data
                    .attestation_id_model
                    .map(|v| AttestationIdProperty::try_from(v.to_vec()))
                    .map_or(Ok(None), |r| r.map(Some))?,
                vendor_patch_level: try_bound!(data.vendor_patch_level, u32)?,
                boot_patch_level: try_bound!(data.boot_patch_level, u32)?,
                device_unique_attestation: Some(data.device_unique_attestation.is_some()),
            })
        }
    }

    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub struct BoundedRootOfTrust {
        pub verified_boot_key: VerifiedBootKey,
        pub device_locked: bool,
        pub verified_boot_state: VerifiedBootState,
        pub verified_boot_hash: Option<VerifiedBootHash>,
    }

    impl TryFrom<asn::RootOfTrustV1V2<'_>> for BoundedRootOfTrust {
        type Error = ();

        fn try_from(data: asn::RootOfTrustV1V2) -> Result<Self, Self::Error> {
            Ok(BoundedRootOfTrust {
                verified_boot_key: VerifiedBootKey::try_from(data.verified_boot_key.to_vec())?,
                device_locked: data.device_locked,
                verified_boot_state: data.verified_boot_state.into(),
                verified_boot_hash: None,
            })
        }
    }

    impl TryFrom<asn::RootOfTrust<'_>> for BoundedRootOfTrust {
        type Error = ();

        fn try_from(data: asn::RootOfTrust) -> Result<Self, Self::Error> {
            Ok(BoundedRootOfTrust {
                verified_boot_key: VerifiedBootKey::try_from(data.verified_boot_key.to_vec())?,
                device_locked: data.device_locked,
                verified_boot_state: data.verified_boot_state.into(),
                verified_boot_hash: Some(VerifiedBootHash::try_from(
                    data.verified_boot_hash.to_vec(),
                )?),
            })
        }
    }

    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub enum VerifiedBootState {
        Verified,
        SelfSigned,
        Unverified,
        Failed,
    }

    impl From<asn::VerifiedBootState> for VerifiedBootState {
        fn from(data: asn::VerifiedBootState) -> Self {
            match data.value() {
                0 => VerifiedBootState::Verified,
                1 => VerifiedBootState::SelfSigned,
                2 => VerifiedBootState::Unverified,
                _ => VerifiedBootState::Failed,
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use frame_support::{assert_err, assert_ok};
    use hex_literal::hex;
    use sp_io;
    use sp_runtime::{testing::Header, traits::IdentityLookup};

    type AccountId = u64;
    type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
    type Block = frame_system::mocking::MockBlock<Test>;

    frame_support::construct_runtime!(
        pub enum Test where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
            Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
            Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
            Acurast: crate::{Pallet, Call, Storage, Event<T>},
        }
    );

    impl frame_system::Config for Test {
        type BaseCallFilter = frame_support::traits::Everything;
        type BlockWeights = BlockWeights;
        type BlockLength = ();
        type DbWeight = ();
        type Origin = Origin;
        type Index = u64;
        type BlockNumber = u64;
        type Call = Call;
        type Hash = sp_core::H256;
        type Hashing = ::sp_runtime::traits::BlakeTwo256;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type Event = Event;
        type BlockHashCount = frame_support::traits::ConstU64<250>;
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = ();
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = frame_support::traits::ConstU32<16>;
    }

    frame_support::parameter_types! {
        pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(1024);
        pub const MinimumPeriod: u64 = 6000;
        pub Admins: Vec<AccountId> = vec![1];
        pub static ExistentialDeposit: u64 = 0;
    }

    impl pallet_balances::Config for Test {
        type Balance = u64;
        type DustRemoval = ();
        type Event = Event;
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = frame_support::traits::StorageMapShim<
            pallet_balances::Account<Test>,
            frame_system::Provider<Test>,
            u64,
            pallet_balances::AccountData<u64>,
        >;
        type MaxLocks = frame_support::traits::ConstU32<50>;
        type MaxReserves = frame_support::traits::ConstU32<2>;
        type ReserveIdentifier = [u8; 8];
        type WeightInfo = ();
    }

    impl pallet_timestamp::Config for Test {
        type Moment = u64;
        type OnTimestampSet = ();
        type MinimumPeriod = MinimumPeriod;
        type WeightInfo = ();
    }

    impl crate::Config for Test {
        type Event = Event;
        type RegistrationExtra = ();
        type FulfillmentRouter = Router;
        type MaxAllowedSources = frame_support::traits::ConstU16<4>;
        type AllowedRevocationListUpdate = Admins;
    }

    pub struct Router;

    impl crate::FulfillmentRouter<Test> for Router {
        fn received_fulfillment(
            _origin: frame_system::pallet_prelude::OriginFor<Test>,
            _from: <Test as frame_system::Config>::AccountId,
            _fulfillment: crate::Fulfillment,
            _registration: crate::JobRegistration<
                <Test as frame_system::Config>::AccountId,
                <Test as crate::Config>::RegistrationExtra,
            >,
            _requester: <<Test as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Target,
        ) -> frame_support::pallet_prelude::DispatchResultWithPostInfo {
            Ok(().into())
        }
    }

    pub struct ExtBuilder;

    impl ExtBuilder {
        pub fn build(self) -> sp_io::TestExternalities {
            let mut t = frame_system::GenesisConfig::default()
                .build_storage::<Test>()
                .unwrap();

            pallet_balances::GenesisConfig::<Test> {
                balances: vec![(1, 10), (2, 20), (3, 30), (4, 40), (12, 10)],
            }
            .assimilate_storage(&mut t)
            .unwrap();

            let mut ext = sp_io::TestExternalities::new(t);
            ext.execute_with(|| System::set_block_number(1));
            ext
        }
    }

    impl Default for ExtBuilder {
        fn default() -> Self {
            Self {}
        }
    }

    fn events() -> Vec<Event> {
        let evt = System::events()
            .into_iter()
            .map(|evt| evt.event)
            .collect::<Vec<_>>();

        System::reset_events();

        evt
    }

    #[test]
    fn test_job_registration() {
        let script: Script = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec().try_into().unwrap();
        let registration = JobRegistration {
            script: script.clone(),
            allowed_sources: None,
            allow_only_verified_sources: false,
            extra: (),
        };
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(Acurast::register(
                Origin::signed(1).into(),
                registration.clone(),
            ));

            assert_ok!(Acurast::deregister(
                Origin::signed(1).into(),
                script.clone()
            ));

            assert_eq!(
                events(),
                [
                    Event::Acurast(crate::Event::JobRegistrationStored(registration, 1)),
                    Event::Acurast(crate::Event::JobRegistrationRemoved(script, 1))
                ]
            );
        });
    }

    #[test]
    fn test_job_registration_failure_1() {
        let script = hex!("597066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec();
        let registration = JobRegistration {
            script: script.try_into().unwrap(),
            allowed_sources: None,
            allow_only_verified_sources: false,
            extra: (),
        };
        ExtBuilder::default().build().execute_with(|| {
            assert_err!(
                Acurast::register(Origin::signed(1).into(), registration),
                Error::<Test>::InvalidScriptValue
            );

            assert_eq!(events(), []);
        });
    }

    #[test]
    fn test_job_registration_failure_2() {
        let script = hex!("697066733A2F2F000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec();
        let registration = JobRegistration {
            script: script.try_into().unwrap(),
            allowed_sources: None,
            allow_only_verified_sources: false,
            extra: (),
        };
        ExtBuilder::default().build().execute_with(|| {
            assert_err!(
                Acurast::register(Origin::signed(1).into(), registration),
                Error::<Test>::InvalidScriptValue
            );

            assert_eq!(events(), []);
        });
    }

    #[test]
    fn test_job_registration_failure_3() {
        let script = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec();
        let registration_1 = JobRegistration {
            script: script.clone().try_into().unwrap(),
            allowed_sources: Some(vec![1, 2, 3, 4, 12]),
            allow_only_verified_sources: false,
            extra: (),
        };
        let registration_2 = JobRegistration::<u64, ()> {
            script: script.try_into().unwrap(),
            allowed_sources: Some(vec![]),
            allow_only_verified_sources: false,
            extra: (),
        };
        ExtBuilder::default().build().execute_with(|| {
            assert_err!(
                Acurast::register(Origin::signed(1).into(), registration_1),
                Error::<Test>::TooManyAllowedSources
            );

            assert_err!(
                Acurast::register(Origin::signed(1).into(), registration_2),
                Error::<Test>::TooFewAllowedSources
            );

            assert_eq!(events(), []);
        });
    }

    #[test]
    fn test_update_allowed_sources() {
        let script: Script = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec().try_into().unwrap();
        let registration_1 = JobRegistration {
            script: script.clone(),
            allowed_sources: None,
            allow_only_verified_sources: false,
            extra: (),
        };
        let registration_2 = JobRegistration {
            script: script.clone(),
            allowed_sources: Some(vec![1, 2]),
            allow_only_verified_sources: false,
            extra: (),
        };
        let updates_1 = vec![
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Add,
                account_id: 1,
            },
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Add,
                account_id: 2,
            },
        ];
        let updates_2 = vec![
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Remove,
                account_id: 1,
            },
            AllowedSourcesUpdate {
                operation: ListUpdateOperation::Remove,
                account_id: 2,
            },
        ];
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(Acurast::register(
                Origin::signed(1).into(),
                registration_1.clone(),
            ));

            assert_ok!(Acurast::update_allowed_sources(
                Origin::signed(1).into(),
                script.clone(),
                updates_1.clone()
            ));

            assert_ok!(Acurast::update_allowed_sources(
                Origin::signed(1).into(),
                script.clone(),
                updates_2.clone()
            ));

            assert_eq!(
                events(),
                [
                    Event::Acurast(crate::Event::JobRegistrationStored(
                        registration_1.clone(),
                        1
                    )),
                    Event::Acurast(crate::Event::AllowedSourcesUpdated(
                        1,
                        registration_1,
                        updates_1
                    )),
                    Event::Acurast(crate::Event::AllowedSourcesUpdated(
                        1,
                        registration_2,
                        updates_2
                    ))
                ]
            );
        });
    }

    #[test]
    fn test_update_allowed_sources_failure() {
        let script: Script = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec().try_into().unwrap();
        let registration = JobRegistration {
            script: script.clone(),
            allowed_sources: Some(vec![1, 2, 3, 4]),
            allow_only_verified_sources: false,
            extra: (),
        };
        let updates = vec![AllowedSourcesUpdate {
            operation: ListUpdateOperation::Add,
            account_id: 12,
        }];
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(Acurast::register(
                Origin::signed(1).into(),
                registration.clone(),
            ));

            assert_err!(
                Acurast::update_allowed_sources(
                    Origin::signed(1).into(),
                    script.clone(),
                    updates.clone()
                ),
                Error::<Test>::TooManyAllowedSources
            );

            assert_eq!(
                events(),
                [Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    1
                )),]
            );
        });
    }

    #[test]
    fn test_fulfill() {
        let script: Script = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec().try_into().unwrap();
        let registration = JobRegistration {
            script: script.clone(),
            allowed_sources: None,
            allow_only_verified_sources: false,
            extra: (),
        };
        let fulfillment = Fulfillment {
            script: script.clone(),
            payload: hex!("00").to_vec(),
        };
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(Acurast::register(
                Origin::signed(1).into(),
                registration.clone(),
            ));
            assert_ok!(Acurast::fulfill(
                Origin::signed(2).into(),
                fulfillment.clone(),
                1
            ));

            assert_eq!(
                events(),
                [
                    Event::Acurast(crate::Event::JobRegistrationStored(registration.clone(), 1)),
                    Event::Acurast(crate::Event::ReceivedFulfillment(
                        2,
                        fulfillment,
                        registration,
                        1
                    )),
                ]
            );
        });
    }

    #[test]
    fn test_fulfill_failure_1() {
        let script: Script = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec().try_into().unwrap();
        let fulfillment = Fulfillment {
            script: script.clone(),
            payload: hex!("00").to_vec(),
        };
        ExtBuilder::default().build().execute_with(|| {
            assert_err!(
                Acurast::fulfill(Origin::signed(2).into(), fulfillment.clone(), 1),
                Error::<Test>::JobRegistrationNotFound
            );

            assert_eq!(events(), []);
        });
    }

    #[test]
    fn test_fulfill_failure_2() {
        let script: Script = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec().try_into().unwrap();
        let registration = JobRegistration {
            script: script.clone(),
            allowed_sources: None,
            allow_only_verified_sources: true,
            extra: (),
        };
        let fulfillment = Fulfillment {
            script: script.clone(),
            payload: hex!("00").to_vec(),
        };
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(Acurast::register(
                Origin::signed(1).into(),
                registration.clone(),
            ));
            assert_err!(
                Acurast::fulfill(Origin::signed(2).into(), fulfillment.clone(), 1),
                Error::<Test>::FulfillSourceNotVerified
            );

            assert_eq!(
                events(),
                [Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    1
                ))]
            );
        });
    }

    #[test]
    fn test_fulfill_failure_3() {
        let script: Script = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec().try_into().unwrap();
        let registration = JobRegistration {
            script: script.clone(),
            allowed_sources: Some(vec![3]),
            allow_only_verified_sources: false,
            extra: (),
        };
        let fulfillment = Fulfillment {
            script: script.clone(),
            payload: hex!("00").to_vec(),
        };
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(Acurast::register(
                Origin::signed(1).into(),
                registration.clone(),
            ));
            assert_err!(
                Acurast::fulfill(Origin::signed(2).into(), fulfillment.clone(), 1),
                Error::<Test>::FulfillSourceNotAllowed
            );

            assert_eq!(
                events(),
                [Event::Acurast(crate::Event::JobRegistrationStored(
                    registration.clone(),
                    1
                ))]
            );
        });
    }

    #[test]
    fn test_submit_attestation() {
        ExtBuilder::default().build().execute_with(|| {
            let chain = AttestationChain {
                certificate_chain: vec![
                    hex!("3082056030820348a003020102020900e8fa196314d2fa18300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3136303532363136323835325a170d3236303532343136323835325a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a381a63081a3301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302018630400603551d1f043930373035a033a031862f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f300d06092a864886f70d01010b0500038202010020c8c38d4bdca9571b468c892fff72aac6f844a11d41a8f0736cc37d16d6426d8e7e9407044cea39e68b07c13dbf1503dd5c85bdafb2c02d5f6cdb4efa8127df8b04f182770fc4e7745b7fceaa87129a8801ce8e9bc0cb96379b4d26a82d30fd9c2f8eed6dc1be2f84b689e4d914258b144bbae624a1c70671132e2f0616a884b2a4d6a46ffa89b602bfbad80c1243711f56eb6056f637c8a0141cc54094268b8c3c7db994b35c0dcd6cb2abc2dafee252023d2dea0cd6c368bea3e6414886f6b1e58b5bd7c730b268c4e3c1fb6424b91febbdb80c586e2ae8368c84d5d10917bda2561789d4687393340e2e254f560ef64b2358fcdc0fbfc6700952e708bffcc627500c1f66e81ea17c098d7a2e9b18801b7ab4ac71587d345dcc8309d5b62a50427aa6d03dcb05996c96ba0c5d71e92162c016ca849ff35f0d52c65d05605a47f3ae917acd2df910efd2326688596ef69b3bf5fe3154f7aeb880a0a73ca04d94c2ce8317eeb43d5eff5883e336f5f249daaca4899237bf267e5c43ab02ea44162403723be6aa692c61bdae9ed409d463c4c97c64306577eef2bc7560b75715cc9c7dc67c86082db751a89c30349762b0782385875cf1a3c6166e0ae3c12d374e2d4f1846f318744bd879b587329bf018217a6c0c77241a4878e435c03079cb451289c5776206069a2f8d65f840e1445287bed877abae24e24435168d553ce4").to_vec().try_into().unwrap(),
                    hex!("308203d7308201bfa003020102020a038826676065899685f5300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139303830393233303332335a170d3239303830363233303332335a302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f783076301006072a8648ce3d020106052b8104002203620004e352276f9bfcea4301a5f0427fa6478e573209ae44fd762cfbc57cbbd4713631509e802ea0e940536e54fa2570ca2846154698075509293b3100b3955b4317768b286bf6fe2651c59af6c6b0db3360090a4647c7860e76ecc3b8a7db5ce57acca381b63081b3301d0603551d0e041604146990b10c3b088aee2af88c3387b42c12dadfc3a6301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430500603551d1f044930473045a043a041863f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f38463637333443394641353034373839300d06092a864886f70d01010b050003820201005c591327a0b0249ecadc949184c9651ed1f2a617a17516439875429e9bd21f87fd2365d0dcde747022c19410f23ab380fe1cef0f47aebc443c2a4531df3eca4101bf96d6bc30dfd878ed6734653111b5e782a03350cc2605e128b48a57e7ff1fe4bf4104de3f7ca9ace6afb01bdd9205fa10b91837a337257afb8290afa456fa629cfae5477b172b009bf28d43dcd4d31edcbf3dc1b6fcfcca5c38a79773d38b5a9d3ccd8152d51f25f9900701d9fb4fbf1307e17fcf5ddc759409863d2f0fb2e6c24468c9c5d85154e104318cb10ae60ba27bb252080e072645681c39e560e8586a64550867162f4bde9db75645882cb9eaff4efe1b0a312f5bd40224298c91f135061b8e04e8fa4c618c33f7b942c028f00d18113bfb6e55a952ccb5d71ee046f9bfdc85aa083e26d94be354545954b70c812ac4e326fdf07703bb79e536d429ff1d099c81722d81714593c7c2bb56740ccbc801332bb548695e28f2c8ac1452a260cfe57f311adc132e8dda01d638f9a4a31288a623a917f5b6c87e1c8316927129a0d11f384251d2df26b942a76844ab91968f4953e7484f2ecd2d6e187f9772d3b4584ac986e2079bc75f20773f8814ba2d16c7266761d6a3505f939fc316efda8787085a5d4f479df944f9d061d2c99acce73ed31770659297113f94140500306887be1b88082b96b18e123cabfcffbd79b68782a0408748cbf4f02f42").to_vec().try_into().unwrap(),
                    hex!("30820230308201b7a003020102020a15905857467176635834300a06082a8648ce3d040302302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f78301e170d3139303732373031353231395a170d3239303732343031353231395a302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783059301306072a8648ce3d020106082a8648ce3d030107034200047639963abb7d336b5f238d8b355efdb395a22b2ccde67bda24328e4bbf802fefa97f204dd8bdb450332cb5e566f759bdc6ffafb9f3bc78e3747dfce8278e5f02a381ba3081b7301d0603551d0e04160414413e3ca9b34bc7a51cbb0125c0421be651ad7ad8301f0603551d230418301680146990b10c3b088aee2af88c3387b42c12dadfc3a6300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430540603551d1f044d304b3049a047a045864368747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f3135393035383537343637313736363335383334300a06082a8648ce3d0403020367003064023017a0df3880a22ea1d4b3dfbdb6c04a4e5655d0ba70bdc8a5ac483b270c1e6d520cda9800b3ad775bae8dfccc7a86ecf802302898f95f24867bb3112f440db5dad27769e42be7db8dc51cf0b2af55aa43c11002e340a24f3965032f9a3a7c83c6bbdb").to_vec().try_into().unwrap(),
                    hex!("3082029c30820241a003020102020101300c06082a8648ce3d0403020500302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783022180f32303232303730393130353135355a180f32303238303532333233353935395a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004b20c1d15477662623ecf430104898006e0f81c0db1bae87cb96a87c7777404659e585d3d9057b8a2ff8ae61f401a078fc75cf52c8c4268e810f93798c729e862a382015630820152300e0603551d0f0101ff0404030207803082013e060a2b06010401d6790201110482012e3082012a0201040a01020201290a0102040874657374617364660400306cbf853d0802060181e296611fbf85455c045a305831323030042b636f6d2e7562696e657469632e61747465737465642e6578656375746f722e746573742e746573746e657402010e31220420bdcb4560f6b3c41dad920668169c28be1ef9ea49f23d98cd8eb2f37ae4488ff93081a1a1053103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420879cd3f18ea76e244d4d4ac3bcb9c337c13b4667190b19035afe2536550050f10101ff0a010004203f4136ee3581e6aba8ea337a6b43d703de1eca241f9b7f277ecdfafff7a8dcf1bf854105020301d4c0bf85420502030315debf854e06020401348abdbf854f06020401348abd300c06082a8648ce3d04030205000347003044022033a613cce9a6ed25026a492b651f0ac67c3c0289d4e4743168c6903e2faa0bda0220324cd35c4bf2695d71ad12a28868e69232112922eaf0e3699f6add8133d528d9").to_vec().try_into().unwrap(),
                ].try_into().unwrap()
            };
            _ = Timestamp::set(Origin::none(), 1657363915001);
            assert_ok!(Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()));

            let attestation = validate_and_extract_attestation::<Test>(&chain).unwrap();

            assert_eq!(
                events(),
                [Event::Acurast(crate::Event::AttestationStored(
                    attestation,
                    1
                ))]
            );
        });
    }

    #[test]
    fn test_submit_attestation_register_fulfill() {
        ExtBuilder::default().build().execute_with(|| {
            let chain = AttestationChain {
                certificate_chain: vec![
                    hex!("3082056030820348a003020102020900e8fa196314d2fa18300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3136303532363136323835325a170d3236303532343136323835325a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a381a63081a3301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302018630400603551d1f043930373035a033a031862f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f300d06092a864886f70d01010b0500038202010020c8c38d4bdca9571b468c892fff72aac6f844a11d41a8f0736cc37d16d6426d8e7e9407044cea39e68b07c13dbf1503dd5c85bdafb2c02d5f6cdb4efa8127df8b04f182770fc4e7745b7fceaa87129a8801ce8e9bc0cb96379b4d26a82d30fd9c2f8eed6dc1be2f84b689e4d914258b144bbae624a1c70671132e2f0616a884b2a4d6a46ffa89b602bfbad80c1243711f56eb6056f637c8a0141cc54094268b8c3c7db994b35c0dcd6cb2abc2dafee252023d2dea0cd6c368bea3e6414886f6b1e58b5bd7c730b268c4e3c1fb6424b91febbdb80c586e2ae8368c84d5d10917bda2561789d4687393340e2e254f560ef64b2358fcdc0fbfc6700952e708bffcc627500c1f66e81ea17c098d7a2e9b18801b7ab4ac71587d345dcc8309d5b62a50427aa6d03dcb05996c96ba0c5d71e92162c016ca849ff35f0d52c65d05605a47f3ae917acd2df910efd2326688596ef69b3bf5fe3154f7aeb880a0a73ca04d94c2ce8317eeb43d5eff5883e336f5f249daaca4899237bf267e5c43ab02ea44162403723be6aa692c61bdae9ed409d463c4c97c64306577eef2bc7560b75715cc9c7dc67c86082db751a89c30349762b0782385875cf1a3c6166e0ae3c12d374e2d4f1846f318744bd879b587329bf018217a6c0c77241a4878e435c03079cb451289c5776206069a2f8d65f840e1445287bed877abae24e24435168d553ce4").to_vec().try_into().unwrap(),
                    hex!("308203d7308201bfa003020102020a038826676065899685f5300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139303830393233303332335a170d3239303830363233303332335a302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f783076301006072a8648ce3d020106052b8104002203620004e352276f9bfcea4301a5f0427fa6478e573209ae44fd762cfbc57cbbd4713631509e802ea0e940536e54fa2570ca2846154698075509293b3100b3955b4317768b286bf6fe2651c59af6c6b0db3360090a4647c7860e76ecc3b8a7db5ce57acca381b63081b3301d0603551d0e041604146990b10c3b088aee2af88c3387b42c12dadfc3a6301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430500603551d1f044930473045a043a041863f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f38463637333443394641353034373839300d06092a864886f70d01010b050003820201005c591327a0b0249ecadc949184c9651ed1f2a617a17516439875429e9bd21f87fd2365d0dcde747022c19410f23ab380fe1cef0f47aebc443c2a4531df3eca4101bf96d6bc30dfd878ed6734653111b5e782a03350cc2605e128b48a57e7ff1fe4bf4104de3f7ca9ace6afb01bdd9205fa10b91837a337257afb8290afa456fa629cfae5477b172b009bf28d43dcd4d31edcbf3dc1b6fcfcca5c38a79773d38b5a9d3ccd8152d51f25f9900701d9fb4fbf1307e17fcf5ddc759409863d2f0fb2e6c24468c9c5d85154e104318cb10ae60ba27bb252080e072645681c39e560e8586a64550867162f4bde9db75645882cb9eaff4efe1b0a312f5bd40224298c91f135061b8e04e8fa4c618c33f7b942c028f00d18113bfb6e55a952ccb5d71ee046f9bfdc85aa083e26d94be354545954b70c812ac4e326fdf07703bb79e536d429ff1d099c81722d81714593c7c2bb56740ccbc801332bb548695e28f2c8ac1452a260cfe57f311adc132e8dda01d638f9a4a31288a623a917f5b6c87e1c8316927129a0d11f384251d2df26b942a76844ab91968f4953e7484f2ecd2d6e187f9772d3b4584ac986e2079bc75f20773f8814ba2d16c7266761d6a3505f939fc316efda8787085a5d4f479df944f9d061d2c99acce73ed31770659297113f94140500306887be1b88082b96b18e123cabfcffbd79b68782a0408748cbf4f02f42").to_vec().try_into().unwrap(),
                    hex!("30820230308201b7a003020102020a15905857467176635834300a06082a8648ce3d040302302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f78301e170d3139303732373031353231395a170d3239303732343031353231395a302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783059301306072a8648ce3d020106082a8648ce3d030107034200047639963abb7d336b5f238d8b355efdb395a22b2ccde67bda24328e4bbf802fefa97f204dd8bdb450332cb5e566f759bdc6ffafb9f3bc78e3747dfce8278e5f02a381ba3081b7301d0603551d0e04160414413e3ca9b34bc7a51cbb0125c0421be651ad7ad8301f0603551d230418301680146990b10c3b088aee2af88c3387b42c12dadfc3a6300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430540603551d1f044d304b3049a047a045864368747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f3135393035383537343637313736363335383334300a06082a8648ce3d0403020367003064023017a0df3880a22ea1d4b3dfbdb6c04a4e5655d0ba70bdc8a5ac483b270c1e6d520cda9800b3ad775bae8dfccc7a86ecf802302898f95f24867bb3112f440db5dad27769e42be7db8dc51cf0b2af55aa43c11002e340a24f3965032f9a3a7c83c6bbdb").to_vec().try_into().unwrap(),
                    hex!("3082029c30820241a003020102020101300c06082a8648ce3d0403020500302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783022180f32303232303730393130353135355a180f32303238303532333233353935395a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004b20c1d15477662623ecf430104898006e0f81c0db1bae87cb96a87c7777404659e585d3d9057b8a2ff8ae61f401a078fc75cf52c8c4268e810f93798c729e862a382015630820152300e0603551d0f0101ff0404030207803082013e060a2b06010401d6790201110482012e3082012a0201040a01020201290a0102040874657374617364660400306cbf853d0802060181e296611fbf85455c045a305831323030042b636f6d2e7562696e657469632e61747465737465642e6578656375746f722e746573742e746573746e657402010e31220420bdcb4560f6b3c41dad920668169c28be1ef9ea49f23d98cd8eb2f37ae4488ff93081a1a1053103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420879cd3f18ea76e244d4d4ac3bcb9c337c13b4667190b19035afe2536550050f10101ff0a010004203f4136ee3581e6aba8ea337a6b43d703de1eca241f9b7f277ecdfafff7a8dcf1bf854105020301d4c0bf85420502030315debf854e06020401348abdbf854f06020401348abd300c06082a8648ce3d04030205000347003044022033a613cce9a6ed25026a492b651f0ac67c3c0289d4e4743168c6903e2faa0bda0220324cd35c4bf2695d71ad12a28868e69232112922eaf0e3699f6add8133d528d9").to_vec().try_into().unwrap(),
                ].try_into().unwrap()
            };
            let script: Script = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec().try_into().unwrap();
            let registration = JobRegistration {
                script: script.clone(),
                allowed_sources: None,
                allow_only_verified_sources: true,
                extra: (),
            };
            let fulfillment = Fulfillment {
                script: script.clone(),
                payload: hex!("00").to_vec(),
            };

            _ = Timestamp::set(Origin::none(), 1657363915001);
            assert_ok!(Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()));
            assert_ok!(Acurast::register(Origin::signed(2).into(), registration.clone()));
            assert_ok!(Acurast::fulfill(Origin::signed(1), fulfillment.clone(), 2));

            let attestation = validate_and_extract_attestation::<Test>(&chain).unwrap();

            assert_eq!(
                events(),
                [
                    Event::Acurast(crate::Event::AttestationStored(attestation, 1)),
                    Event::Acurast(crate::Event::JobRegistrationStored(registration.clone(), 2)),
                    Event::Acurast(crate::Event::ReceivedFulfillment(
                        1,
                        fulfillment,
                        registration,
                        2
                    )),
                ]
            );
        });
    }

    #[test]
    fn test_submit_attestation_failure_1() {
        ExtBuilder::default().build().execute_with(|| {
            let chain = AttestationChain {
                certificate_chain: vec![
                    hex!("3082029c30820241a003020102020101300c06082a8648ce3d0403020500302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783022180f32303232303730393130353135355a180f32303238303532333233353935395a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004b20c1d15477662623ecf430104898006e0f81c0db1bae87cb96a87c7777404659e585d3d9057b8a2ff8ae61f401a078fc75cf52c8c4268e810f93798c729e862a382015630820152300e0603551d0f0101ff0404030207803082013e060a2b06010401d6790201110482012e3082012a0201040a01020201290a0102040874657374617364660400306cbf853d0802060181e296611fbf85455c045a305831323030042b636f6d2e7562696e657469632e61747465737465642e6578656375746f722e746573742e746573746e657402010e31220420bdcb4560f6b3c41dad920668169c28be1ef9ea49f23d98cd8eb2f37ae4488ff93081a1a1053103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420879cd3f18ea76e244d4d4ac3bcb9c337c13b4667190b19035afe2536550050f10101ff0a010004203f4136ee3581e6aba8ea337a6b43d703de1eca241f9b7f277ecdfafff7a8dcf1bf854105020301d4c0bf85420502030315debf854e06020401348abdbf854f06020401348abd300c06082a8648ce3d04030205000347003044022033a613cce9a6ed25026a492b651f0ac67c3c0289d4e4743168c6903e2faa0bda0220324cd35c4bf2695d71ad12a28868e69232112922eaf0e3699f6add8133d528d9").to_vec().try_into().unwrap(),
                ].try_into().unwrap()
            };

            assert_err!(Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()), Error::<Test>::CertificateChainTooShort);

            assert_eq!(
                events(),
                []
            );
        });
    }

    #[test]
    fn test_submit_attestation_failure_2() {
        ExtBuilder::default().build().execute_with(|| {
            let chain = AttestationChain {
                certificate_chain: vec![
                    hex!("3082056030820348a003020102020900e8fa196314d2fa18300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3136303532363136323835325a170d3236303532343136323835325a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a381a63081a3301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302018630400603551d1f043930373035a033a031862f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f300d06092a864886f70d01010b0500038202010020c8c38d4bdca9571b468c892fff72aac6f844a11d41a8f0736cc37d16d6426d8e7e9407044cea39e68b07c13dbf1503dd5c85bdafb2c02d5f6cdb4efa8127df8b04f182770fc4e7745b7fceaa87129a8801ce8e9bc0cb96379b4d26a82d30fd9c2f8eed6dc1be2f84b689e4d914258b144bbae624a1c70671132e2f0616a884b2a4d6a46ffa89b602bfbad80c1243711f56eb6056f637c8a0141cc54094268b8c3c7db994b35c0dcd6cb2abc2dafee252023d2dea0cd6c368bea3e6414886f6b1e58b5bd7c730b268c4e3c1fb6424b91febbdb80c586e2ae8368c84d5d10917bda2561789d4687393340e2e254f560ef64b2358fcdc0fbfc6700952e708bffcc627500c1f66e81ea17c098d7a2e9b18801b7ab4ac71587d345dcc8309d5b62a50427aa6d03dcb05996c96ba0c5d71e92162c016ca849ff35f0d52c65d05605a47f3ae917acd2df910efd2326688596ef69b3bf5fe3154f7aeb880a0a73ca04d94c2ce8317eeb43d5eff5883e336f5f249daaca4899237bf267e5c43ab02ea44162403723be6aa692c61bdae9ed409d463c4c97c64306577eef2bc7560b75715cc9c7dc67c86082db751a89c30349762b0782385875cf1a3c6166e0ae3c12d374e2d4f1846f318744bd879b587329bf018217a6c0c77241a4878e435c03079cb451289c5776206069a2f8d65f840e1445287bed877abae24e24435168d553ce4").to_vec().try_into().unwrap(),
                    hex!("308203d7308201bfa003020102020a038826676065899685f5300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139303830393233303332335a170d3239303830363233303332335a302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f783076301006072a8648ce3d020106052b8104002203620004e352276f9bfcea4301a5f0427fa6478e573209ae44fd762cfbc57cbbd4713631509e802ea0e940536e54fa2570ca2846154698075509293b3100b3955b4317768b286bf6fe2651c59af6c6b0db3360090a4647c7860e76ecc3b8a7db5ce57acca381b63081b3301d0603551d0e041604146990b10c3b088aee2af88c3387b42c12dadfc3a6301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430500603551d1f044930473045a043a041863f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f38463637333443394641353034373839300d06092a864886f70d01010b050003820201005c591327a0b0249ecadc949184c9651ed1f2a617a17516439875429e9bd21f87fd2365d0dcde747022c19410f23ab380fe1cef0f47aebc443c2a4531df3eca4101bf96d6bc30dfd878ed6734653111b5e782a03350cc2605e128b48a57e7ff1fe4bf4104de3f7ca9ace6afb01bdd9205fa10b91837a337257afb8290afa456fa629cfae5477b172b009bf28d43dcd4d31edcbf3dc1b6fcfcca5c38a79773d38b5a9d3ccd8152d51f25f9900701d9fb4fbf1307e17fcf5ddc759409863d2f0fb2e6c24468c9c5d85154e104318cb10ae60ba27bb252080e072645681c39e560e8586a64550867162f4bde9db75645882cb9eaff4efe1b0a312f5bd40224298c91f135061b8e04e8fa4c618c33f7b942c028f00d18113bfb6e55a952ccb5d71ee046f9bfdc85aa083e26d94be354545954b70c812ac4e326fdf07703bb79e536d429ff1d099c81722d81714593c7c2bb56740ccbc801332bb548695e28f2c8ac1452a260cfe57f311adc132e8dda01d638f9a4a31288a623a917f5b6c87e1c8316927129a0d11f384251d2df26b942a76844ab91968f4953e7484f2ecd2d6e187f9772d3b4584ac986e2079bc75f20773f8814ba2d16c7266761d6a3505f939fc316efda8787085a5d4f479df944f9d061d2c99acce73ed31770659297113f94140500306887be1b88082b96b18e123cabfcffbd79b68782a0408748cbf4f02f42").to_vec().try_into().unwrap(),
                    hex!("30820230308201b7a003020102020a15905857467176635834300a06082a8648ce3d040302302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f78301e170d3139303732373031353231395a170d3239303732343031353231395a302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783059301306072a8648ce3d020106082a8648ce3d030107034200047639963abb7d336b5f238d8b355efdb395a22b2ccde67bda24328e4bbf802fefa97f204dd8bdb450332cb5e566f759bdc6ffafb9f3bc78e3747dfce8278e5f02a381ba3081b7301d0603551d0e04160414413e3ca9b34bc7a51cbb0125c0421be651ad7ad8301f0603551d230418301680146990b10c3b088aee2af88c3387b42c12dadfc3a6300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430540603551d1f044d304b3049a047a045864368747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f3135393035383537343637313736363335383334300a06082a8648ce3d0403020367003064023017a0df3880a22ea1d4b3dfbdb6c04a4e5655d0ba70bdc8a5ac483b270c1e6d520cda9800b3ad775bae8dfccc7a86ecf802302898f95f24867bb3112f440db5dad27769e42be7db8dc51cf0b2af55aa43c11002e340a24f3965032f9a3a7c83c6bbdb").to_vec().try_into().unwrap(),
                    hex!("3082029c30820241a003020102020101300c06082a8648ce3d0403020500302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783022180f32303232303730393130353135355a180f32303238303532333233353935395a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004b20c1d15477662623ecf430104898006e0f81c0db1bae87cb96a87c7777404659e585d3d9057b8a2ff8ae61f401a078fc75cf52c8c4268e810f93798c729e862a382015630820152300e0603551d0f0101ff0404030207803082013e060a2b06010401d6790201110482012e3082012a0201040a01020201290a0102040874657374617364660400306cbf853d0802060181e296611fbf85455c045a305831323030042b636f6d2e7562696e657469632e61747465737465642e6578656375746f722e746573742e746573746e657402010e31220420bdcb4560f6b3c41dad920668169c28be1ef9ea49f23d98cd8eb2f37ae4488ff93081a1a1053103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420879cd3f18ea76e244d4d4ac3bcb9c337c13b4667190b19035afe2536550050f10101ff0a010004203f4136ee3581e6aba8ea337a6b43d703de1eca241f9b7f277ecdfafff7a8dcf1bf854105020301d4c0bf85420502030315debf854e06020401348abdbf854f06020401348abd300c06082a8648ce3d04030205000347003044022033a613cce9a6ed25026a492b651f0ac67c3c0289d4e4743168c6903e2faa0bda0220324cd35c4bf2695d71ad12a28868e69232112922eaf0e3699f6add8133d528d9").to_vec().try_into().unwrap(),
                ].try_into().unwrap()
            };

            _ = Timestamp::set(Origin::none(), 1657363914000);
            assert_err!(Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()), Error::<Test>::AttestationCertificateNotValid);

            assert_eq!(
                events(),
                []
            );
        });
    }

    #[test]
    fn test_submit_attestation_failure_3() {
        ExtBuilder::default().build().execute_with(|| {
            let chain = AttestationChain {
                certificate_chain: vec![
                    hex!("3082056030820348a003020102020900e8fa196314d2fa18300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3136303532363136323835325a170d3236303532343136323835325a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a381a63081a3301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302018630400603551d1f043930373035a033a031862f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f300d06092a864886f70d01010b0500038202010020c8c38d4bdca9571b468c892fff72aac6f844a11d41a8f0736cc37d16d6426d8e7e9407044cea39e68b07c13dbf1503dd5c85bdafb2c02d5f6cdb4efa8127df8b04f182770fc4e7745b7fceaa87129a8801ce8e9bc0cb96379b4d26a82d30fd9c2f8eed6dc1be2f84b689e4d914258b144bbae624a1c70671132e2f0616a884b2a4d6a46ffa89b602bfbad80c1243711f56eb6056f637c8a0141cc54094268b8c3c7db994b35c0dcd6cb2abc2dafee252023d2dea0cd6c368bea3e6414886f6b1e58b5bd7c730b268c4e3c1fb6424b91febbdb80c586e2ae8368c84d5d10917bda2561789d4687393340e2e254f560ef64b2358fcdc0fbfc6700952e708bffcc627500c1f66e81ea17c098d7a2e9b18801b7ab4ac71587d345dcc8309d5b62a50427aa6d03dcb05996c96ba0c5d71e92162c016ca849ff35f0d52c65d05605a47f3ae917acd2df910efd2326688596ef69b3bf5fe3154f7aeb880a0a73ca04d94c2ce8317eeb43d5eff5883e336f5f249daaca4899237bf267e5c43ab02ea44162403723be6aa692c61bdae9ed409d463c4c97c64306577eef2bc7560b75715cc9c7dc67c86082db751a89c30349762b0782385875cf1a3c6166e0ae3c12d374e2d4f1846f318744bd879b587329bf018217a6c0c77241a4878e435c03079cb451289c5776206069a2f8d65f840e1445287bed877abae24e24435168d553ce4").to_vec().try_into().unwrap(),
                    hex!("308203d7308201bfa003020102020a038826676065899685f5300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139303830393233303332335a170d3239303830363233303332335a302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f783076301006072a8648ce3d020106052b8104002203620004e352276f9bfcea4301a5f0427fa6478e573209ae44fd762cfbc57cbbd4713631509e802ea0e940536e54fa2570ca2846154698075509293b3100b3955b4317768b286bf6fe2651c59af6c6b0db3360090a4647c7860e76ecc3b8a7db5ce57acca381b63081b3301d0603551d0e041604146990b10c3b088aee2af88c3387b42c12dadfc3a6301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430500603551d1f044930473045a043a041863f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f38463637333443394641353034373839300d06092a864886f70d01010b050003820201005c591327a0b0249ecadc949184c9651ed1f2a617a17516439875429e9bd21f87fd2365d0dcde747022c19410f23ab380fe1cef0f47aebc443c2a4531df3eca4101bf96d6bc30dfd878ed6734653111b5e782a03350cc2605e128b48a57e7ff1fe4bf4104de3f7ca9ace6afb01bdd9205fa10b91837a337257afb8290afa456fa629cfae5477b172b009bf28d43dcd4d31edcbf3dc1b6fcfcca5c38a79773d38b5a9d3ccd8152d51f25f9900701d9fb4fbf1307e17fcf5ddc759409863d2f0fb2e6c24468c9c5d85154e104318cb10ae60ba27bb252080e072645681c39e560e8586a64550867162f4bde9db75645882cb9eaff4efe1b0a312f5bd40224298c91f135061b8e04e8fa4c618c33f7b942c028f00d18113bfb6e55a952ccb5d71ee046f9bfdc85aa083e26d94be354545954b70c812ac4e326fdf07703bb79e536d429ff1d099c81722d81714593c7c2bb56740ccbc801332bb548695e28f2c8ac1452a260cfe57f311adc132e8dda01d638f9a4a31288a623a917f5b6c87e1c8316927129a0d11f384251d2df26b942a76844ab91968f4953e7484f2ecd2d6e187f9772d3b4584ac986e2079bc75f20773f8814ba2d16c7266761d6a3505f939fc316efda8787085a5d4f479df944f9d061d2c99acce73ed31770659297113f94140500306887be1b88082b96b18e123cabfcffbd79b68782a0408748cbf4f02f42").to_vec().try_into().unwrap(),
                    hex!("30820230308201b7a003020102020a15905857467176635834300a06082a8648ce3d040302302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f78301e170d3139303732373031353231395a170d3239303732343031353231395a302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783059301306072a8648ce3d020106082a8648ce3d030107034200047639963abb7d336b5f238d8b355efdb395a22b2ccde67bda24328e4bbf802fefa97f204dd8bdb450332cb5e566f759bdc6ffafb9f3bc78e3747dfce8278e5f02a381ba3081b7301d0603551d0e04160414413e3ca9b34bc7a51cbb0125c0421be651ad7ad8301f0603551d230418301680146990b10c3b088aee2af88c3387b42c12dadfc3a6300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430540603551d1f044d304b3049a047a045864368747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f3135393035383537343637313736363335383334300a06082a8648ce3d0403020367003064023017a0df3880a22ea1d4b3dfbdb6c04a4e5655d0ba70bdc8a5ac483b270c1e6d520cda9800b3ad775bae8dfccc7a86ecf802302898f95f24867bb3112f440db5dad27769e42be7db8dc51cf0b2af55aa43c11002e340a24f3965032f9a3a7c83c6bbdb").to_vec().try_into().unwrap(),
                    hex!("3082029c30820241a003020102020101300c06082a8648ce3d0403020500302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783022180f32303232303730393130353135355a180f32303238303532333233353935395a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004b20c1d15477662623ecf430104898006e0f81c0db1bae87cb96a87c7777404659e585d3d9057b8a2ff8ae61f401a078fc75cf52c8c4268e810f93798c729e862a382015630820152300e0603551d0f0101ff0404030207803082013e060a2b06010401d6790201110482012e3082012a0201040a01020201290a0102040874657374617364660400306cbf853d0802060181e296611fbf85455c045a305831323030042b636f6d2e7562696e657469632e61747465737465642e6578656375746f722e746573742e746573746e657402010e31220420bdcb4560f6b3c41dad920668169c28be1ef9ea49f23d98cd8eb2f37ae4488ff93081a1a1053103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420879cd3f18ea76e244d4d4ac3bcb9c337c13b4667190b19035afe2536550050f10101ff0a010004203f4136ee3581e6aba8ea337a6b43d703de1eca241f9b7f277ecdfafff7a8dcf1bf854105020301d4c0bf85420502030315debf854e06020401348abdbf854f06020401348abd300c06082a8648ce3d04030205000347003044022033a613cce9a6ed25026a492b651f0ac67c3c0289d4e4743168c6903e2faa0bda0220324cd35c4bf2695d71ad12a28868e69232112922eaf0e3699f6add8133d528d9").to_vec().try_into().unwrap(),
                ].try_into().unwrap()
            };

            _ = Timestamp::set(Origin::none(), 1842739199001);
            assert_err!(Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()), Error::<Test>::AttestationCertificateNotValid);

            assert_eq!(
                events(),
                []
            );
        });
    }

    #[test]
    fn test_update_revocation_list() {
        ExtBuilder::default().build().execute_with(|| {
            let updates_1 = vec![CertificateRevocationListUpdate {
                operation: ListUpdateOperation::Add,
                cert_serial_number: hex!("15905857467176635834").to_vec().try_into().unwrap(),
            }];
            assert_ok!(Acurast::update_certificate_revocation_list(
                Origin::signed(1).into(),
                updates_1.clone(),
            ));
            assert_eq!(
                Some(()),
                Acurast::stored_revoked_certificate::<SerialNumber>(
                    hex!("15905857467176635834").to_vec().try_into().unwrap(),
                )
            );

            let updates_2 = vec![CertificateRevocationListUpdate {
                operation: ListUpdateOperation::Remove,
                cert_serial_number: hex!("15905857467176635834").to_vec().try_into().unwrap(),
            }];
            assert_ok!(Acurast::update_certificate_revocation_list(
                Origin::signed(1).into(),
                updates_2.clone(),
            ));
            assert_eq!(
                None,
                Acurast::stored_revoked_certificate::<SerialNumber>(
                    hex!("15905857467176635834").to_vec().try_into().unwrap(),
                )
            );

            assert_err!(
                Acurast::update_certificate_revocation_list(
                    Origin::signed(2).into(),
                    updates_1.clone(),
                ),
                Error::<Test>::CertificateRevocationListUpdateNotAllowed
            );
            assert_eq!(
                None,
                Acurast::stored_revoked_certificate::<SerialNumber>(
                    hex!("15905857467176635834").to_vec().try_into().unwrap(),
                )
            );

            assert_eq!(
                events(),
                [
                    Event::Acurast(crate::Event::CertificateRecovationListUpdated(1, updates_1)),
                    Event::Acurast(crate::Event::CertificateRecovationListUpdated(1, updates_2))
                ]
            );
        });
    }

    #[test]
    fn test_update_revocation_list_submit_attestation() {
        ExtBuilder::default().build().execute_with(|| {
            let updates = vec![CertificateRevocationListUpdate {
                operation: ListUpdateOperation::Add,
                cert_serial_number: hex!("15905857467176635834").to_vec().try_into().unwrap(),
            }];
            assert_ok!(Acurast::update_certificate_revocation_list(
                Origin::signed(1).into(),
                updates.clone(),
            ));

            let chain = AttestationChain {
                certificate_chain: vec![
                    hex!("3082056030820348a003020102020900e8fa196314d2fa18300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3136303532363136323835325a170d3236303532343136323835325a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a381a63081a3301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302018630400603551d1f043930373035a033a031862f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f300d06092a864886f70d01010b0500038202010020c8c38d4bdca9571b468c892fff72aac6f844a11d41a8f0736cc37d16d6426d8e7e9407044cea39e68b07c13dbf1503dd5c85bdafb2c02d5f6cdb4efa8127df8b04f182770fc4e7745b7fceaa87129a8801ce8e9bc0cb96379b4d26a82d30fd9c2f8eed6dc1be2f84b689e4d914258b144bbae624a1c70671132e2f0616a884b2a4d6a46ffa89b602bfbad80c1243711f56eb6056f637c8a0141cc54094268b8c3c7db994b35c0dcd6cb2abc2dafee252023d2dea0cd6c368bea3e6414886f6b1e58b5bd7c730b268c4e3c1fb6424b91febbdb80c586e2ae8368c84d5d10917bda2561789d4687393340e2e254f560ef64b2358fcdc0fbfc6700952e708bffcc627500c1f66e81ea17c098d7a2e9b18801b7ab4ac71587d345dcc8309d5b62a50427aa6d03dcb05996c96ba0c5d71e92162c016ca849ff35f0d52c65d05605a47f3ae917acd2df910efd2326688596ef69b3bf5fe3154f7aeb880a0a73ca04d94c2ce8317eeb43d5eff5883e336f5f249daaca4899237bf267e5c43ab02ea44162403723be6aa692c61bdae9ed409d463c4c97c64306577eef2bc7560b75715cc9c7dc67c86082db751a89c30349762b0782385875cf1a3c6166e0ae3c12d374e2d4f1846f318744bd879b587329bf018217a6c0c77241a4878e435c03079cb451289c5776206069a2f8d65f840e1445287bed877abae24e24435168d553ce4").to_vec().try_into().unwrap(),
                    hex!("308203d7308201bfa003020102020a038826676065899685f5300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139303830393233303332335a170d3239303830363233303332335a302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f783076301006072a8648ce3d020106052b8104002203620004e352276f9bfcea4301a5f0427fa6478e573209ae44fd762cfbc57cbbd4713631509e802ea0e940536e54fa2570ca2846154698075509293b3100b3955b4317768b286bf6fe2651c59af6c6b0db3360090a4647c7860e76ecc3b8a7db5ce57acca381b63081b3301d0603551d0e041604146990b10c3b088aee2af88c3387b42c12dadfc3a6301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430500603551d1f044930473045a043a041863f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f38463637333443394641353034373839300d06092a864886f70d01010b050003820201005c591327a0b0249ecadc949184c9651ed1f2a617a17516439875429e9bd21f87fd2365d0dcde747022c19410f23ab380fe1cef0f47aebc443c2a4531df3eca4101bf96d6bc30dfd878ed6734653111b5e782a03350cc2605e128b48a57e7ff1fe4bf4104de3f7ca9ace6afb01bdd9205fa10b91837a337257afb8290afa456fa629cfae5477b172b009bf28d43dcd4d31edcbf3dc1b6fcfcca5c38a79773d38b5a9d3ccd8152d51f25f9900701d9fb4fbf1307e17fcf5ddc759409863d2f0fb2e6c24468c9c5d85154e104318cb10ae60ba27bb252080e072645681c39e560e8586a64550867162f4bde9db75645882cb9eaff4efe1b0a312f5bd40224298c91f135061b8e04e8fa4c618c33f7b942c028f00d18113bfb6e55a952ccb5d71ee046f9bfdc85aa083e26d94be354545954b70c812ac4e326fdf07703bb79e536d429ff1d099c81722d81714593c7c2bb56740ccbc801332bb548695e28f2c8ac1452a260cfe57f311adc132e8dda01d638f9a4a31288a623a917f5b6c87e1c8316927129a0d11f384251d2df26b942a76844ab91968f4953e7484f2ecd2d6e187f9772d3b4584ac986e2079bc75f20773f8814ba2d16c7266761d6a3505f939fc316efda8787085a5d4f479df944f9d061d2c99acce73ed31770659297113f94140500306887be1b88082b96b18e123cabfcffbd79b68782a0408748cbf4f02f42").to_vec().try_into().unwrap(),
                    hex!("30820230308201b7a003020102020a15905857467176635834300a06082a8648ce3d040302302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f78301e170d3139303732373031353231395a170d3239303732343031353231395a302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783059301306072a8648ce3d020106082a8648ce3d030107034200047639963abb7d336b5f238d8b355efdb395a22b2ccde67bda24328e4bbf802fefa97f204dd8bdb450332cb5e566f759bdc6ffafb9f3bc78e3747dfce8278e5f02a381ba3081b7301d0603551d0e04160414413e3ca9b34bc7a51cbb0125c0421be651ad7ad8301f0603551d230418301680146990b10c3b088aee2af88c3387b42c12dadfc3a6300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430540603551d1f044d304b3049a047a045864368747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f3135393035383537343637313736363335383334300a06082a8648ce3d0403020367003064023017a0df3880a22ea1d4b3dfbdb6c04a4e5655d0ba70bdc8a5ac483b270c1e6d520cda9800b3ad775bae8dfccc7a86ecf802302898f95f24867bb3112f440db5dad27769e42be7db8dc51cf0b2af55aa43c11002e340a24f3965032f9a3a7c83c6bbdb").to_vec().try_into().unwrap(),
                    hex!("3082029c30820241a003020102020101300c06082a8648ce3d0403020500302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783022180f32303232303730393130353135355a180f32303238303532333233353935395a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004b20c1d15477662623ecf430104898006e0f81c0db1bae87cb96a87c7777404659e585d3d9057b8a2ff8ae61f401a078fc75cf52c8c4268e810f93798c729e862a382015630820152300e0603551d0f0101ff0404030207803082013e060a2b06010401d6790201110482012e3082012a0201040a01020201290a0102040874657374617364660400306cbf853d0802060181e296611fbf85455c045a305831323030042b636f6d2e7562696e657469632e61747465737465642e6578656375746f722e746573742e746573746e657402010e31220420bdcb4560f6b3c41dad920668169c28be1ef9ea49f23d98cd8eb2f37ae4488ff93081a1a1053103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420879cd3f18ea76e244d4d4ac3bcb9c337c13b4667190b19035afe2536550050f10101ff0a010004203f4136ee3581e6aba8ea337a6b43d703de1eca241f9b7f277ecdfafff7a8dcf1bf854105020301d4c0bf85420502030315debf854e06020401348abdbf854f06020401348abd300c06082a8648ce3d04030205000347003044022033a613cce9a6ed25026a492b651f0ac67c3c0289d4e4743168c6903e2faa0bda0220324cd35c4bf2695d71ad12a28868e69232112922eaf0e3699f6add8133d528d9").to_vec().try_into().unwrap(),
                ].try_into().unwrap()
            };
            _ = Timestamp::set(Origin::none(), 1657363915001);
            assert_err!(Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()), Error::<Test>::RevokedCertificate);

            assert_eq!(
                events(),
                [
                    Event::Acurast(crate::Event::CertificateRecovationListUpdated(1, updates)),
                ]
            );
        });
    }

    #[test]
    fn test_update_revocation_list_fulfill() {
        ExtBuilder::default().build().execute_with(|| {
            let updates = vec![CertificateRevocationListUpdate {
                operation: ListUpdateOperation::Add,
                cert_serial_number: hex!("15905857467176635834").to_vec().try_into().unwrap(),
            }];
            let chain = AttestationChain {
                certificate_chain: vec![
                    hex!("3082056030820348a003020102020900e8fa196314d2fa18300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3136303532363136323835325a170d3236303532343136323835325a301b31193017060355040513106639323030396538353362366230343530820222300d06092a864886f70d01010105000382020f003082020a0282020100afb6c7822bb1a701ec2bb42e8bcc541663abef982f32c77f7531030c97524b1b5fe809fbc72aa9451f743cbd9a6f1335744aa55e77f6b6ac3535ee17c25e639517dd9c92e6374a53cbfe258f8ffbb6fd129378a22a4ca99c452d47a59f3201f44197ca1ccd7e762fb2f53151b6feb2fffd2b6fe4fe5bc6bd9ec34bfe08239daafceb8eb5a8ed2b3acd9c5e3a7790e1b51442793159859811ad9eb2a96bbdd7a57c93a91c41fccd27d67fd6f671aa0b815261ad384fa37944864604ddb3d8c4f920a19b1656c2f14ad6d03c56ec060899041c1ed1a5fe6d3440b556bad1d0a152589c53e55d370762f0122eef91861b1b0e6c4c80927499c0e9bec0b83e3bc1f93c72c049604bbd2f1345e62c3f8e26dbec06c94766f3c128239d4f4312fad8123887e06becf567583bf8355a81feeabaf99a83c8df3e2a322afc672bf120b135158b6821ceaf309b6eee77f98833b018daa10e451f06a374d50781f359082966bb778b9308942698e74e0bcd24628a01c2cc03e51f0b3e5b4ac1e4df9eaf9ff6a492a77c1483882885015b422ce67b80b88c9b48e13b607ab545c723ff8c44f8f2d368b9f6520d31145ebf9e862ad71df6a3bfd2450959d653740d97a12f368b13ef66d5d0a54a6e2f5d9a6fef446832bc67844725861f093dd0e6f3405da89643ef0f4d69b6420051fdb93049673e36950580d3cdf4fbd08bc58483952600630203010001a381a63081a3301d0603551d0e041604143661e1007c880509518b446c47ff1a4cc9ea4f12301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302018630400603551d1f043930373035a033a031862f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f300d06092a864886f70d01010b0500038202010020c8c38d4bdca9571b468c892fff72aac6f844a11d41a8f0736cc37d16d6426d8e7e9407044cea39e68b07c13dbf1503dd5c85bdafb2c02d5f6cdb4efa8127df8b04f182770fc4e7745b7fceaa87129a8801ce8e9bc0cb96379b4d26a82d30fd9c2f8eed6dc1be2f84b689e4d914258b144bbae624a1c70671132e2f0616a884b2a4d6a46ffa89b602bfbad80c1243711f56eb6056f637c8a0141cc54094268b8c3c7db994b35c0dcd6cb2abc2dafee252023d2dea0cd6c368bea3e6414886f6b1e58b5bd7c730b268c4e3c1fb6424b91febbdb80c586e2ae8368c84d5d10917bda2561789d4687393340e2e254f560ef64b2358fcdc0fbfc6700952e708bffcc627500c1f66e81ea17c098d7a2e9b18801b7ab4ac71587d345dcc8309d5b62a50427aa6d03dcb05996c96ba0c5d71e92162c016ca849ff35f0d52c65d05605a47f3ae917acd2df910efd2326688596ef69b3bf5fe3154f7aeb880a0a73ca04d94c2ce8317eeb43d5eff5883e336f5f249daaca4899237bf267e5c43ab02ea44162403723be6aa692c61bdae9ed409d463c4c97c64306577eef2bc7560b75715cc9c7dc67c86082db751a89c30349762b0782385875cf1a3c6166e0ae3c12d374e2d4f1846f318744bd879b587329bf018217a6c0c77241a4878e435c03079cb451289c5776206069a2f8d65f840e1445287bed877abae24e24435168d553ce4").to_vec().try_into().unwrap(),
                    hex!("308203d7308201bfa003020102020a038826676065899685f5300d06092a864886f70d01010b0500301b311930170603550405131066393230303965383533623662303435301e170d3139303830393233303332335a170d3239303830363233303332335a302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f783076301006072a8648ce3d020106052b8104002203620004e352276f9bfcea4301a5f0427fa6478e573209ae44fd762cfbc57cbbd4713631509e802ea0e940536e54fa2570ca2846154698075509293b3100b3955b4317768b286bf6fe2651c59af6c6b0db3360090a4647c7860e76ecc3b8a7db5ce57acca381b63081b3301d0603551d0e041604146990b10c3b088aee2af88c3387b42c12dadfc3a6301f0603551d230418301680143661e1007c880509518b446c47ff1a4cc9ea4f12300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430500603551d1f044930473045a043a041863f68747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f38463637333443394641353034373839300d06092a864886f70d01010b050003820201005c591327a0b0249ecadc949184c9651ed1f2a617a17516439875429e9bd21f87fd2365d0dcde747022c19410f23ab380fe1cef0f47aebc443c2a4531df3eca4101bf96d6bc30dfd878ed6734653111b5e782a03350cc2605e128b48a57e7ff1fe4bf4104de3f7ca9ace6afb01bdd9205fa10b91837a337257afb8290afa456fa629cfae5477b172b009bf28d43dcd4d31edcbf3dc1b6fcfcca5c38a79773d38b5a9d3ccd8152d51f25f9900701d9fb4fbf1307e17fcf5ddc759409863d2f0fb2e6c24468c9c5d85154e104318cb10ae60ba27bb252080e072645681c39e560e8586a64550867162f4bde9db75645882cb9eaff4efe1b0a312f5bd40224298c91f135061b8e04e8fa4c618c33f7b942c028f00d18113bfb6e55a952ccb5d71ee046f9bfdc85aa083e26d94be354545954b70c812ac4e326fdf07703bb79e536d429ff1d099c81722d81714593c7c2bb56740ccbc801332bb548695e28f2c8ac1452a260cfe57f311adc132e8dda01d638f9a4a31288a623a917f5b6c87e1c8316927129a0d11f384251d2df26b942a76844ab91968f4953e7484f2ecd2d6e187f9772d3b4584ac986e2079bc75f20773f8814ba2d16c7266761d6a3505f939fc316efda8787085a5d4f479df944f9d061d2c99acce73ed31770659297113f94140500306887be1b88082b96b18e123cabfcffbd79b68782a0408748cbf4f02f42").to_vec().try_into().unwrap(),
                    hex!("30820230308201b7a003020102020a15905857467176635834300a06082a8648ce3d040302302f31193017060355040513103534663539333730353432663561393531123010060355040c0c095374726f6e67426f78301e170d3139303732373031353231395a170d3239303732343031353231395a302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783059301306072a8648ce3d020106082a8648ce3d030107034200047639963abb7d336b5f238d8b355efdb395a22b2ccde67bda24328e4bbf802fefa97f204dd8bdb450332cb5e566f759bdc6ffafb9f3bc78e3747dfce8278e5f02a381ba3081b7301d0603551d0e04160414413e3ca9b34bc7a51cbb0125c0421be651ad7ad8301f0603551d230418301680146990b10c3b088aee2af88c3387b42c12dadfc3a6300f0603551d130101ff040530030101ff300e0603551d0f0101ff04040302020430540603551d1f044d304b3049a047a045864368747470733a2f2f616e64726f69642e676f6f676c65617069732e636f6d2f6174746573746174696f6e2f63726c2f3135393035383537343637313736363335383334300a06082a8648ce3d0403020367003064023017a0df3880a22ea1d4b3dfbdb6c04a4e5655d0ba70bdc8a5ac483b270c1e6d520cda9800b3ad775bae8dfccc7a86ecf802302898f95f24867bb3112f440db5dad27769e42be7db8dc51cf0b2af55aa43c11002e340a24f3965032f9a3a7c83c6bbdb").to_vec().try_into().unwrap(),
                    hex!("3082029c30820241a003020102020101300c06082a8648ce3d0403020500302f31193017060355040513103937333533373739333664306464373431123010060355040c0c095374726f6e67426f783022180f32303232303730393130353135355a180f32303238303532333233353935395a301f311d301b06035504030c14416e64726f6964204b657973746f7265204b65793059301306072a8648ce3d020106082a8648ce3d03010703420004b20c1d15477662623ecf430104898006e0f81c0db1bae87cb96a87c7777404659e585d3d9057b8a2ff8ae61f401a078fc75cf52c8c4268e810f93798c729e862a382015630820152300e0603551d0f0101ff0404030207803082013e060a2b06010401d6790201110482012e3082012a0201040a01020201290a0102040874657374617364660400306cbf853d0802060181e296611fbf85455c045a305831323030042b636f6d2e7562696e657469632e61747465737465642e6578656375746f722e746573742e746573746e657402010e31220420bdcb4560f6b3c41dad920668169c28be1ef9ea49f23d98cd8eb2f37ae4488ff93081a1a1053103020102a203020103a30402020100a5053103020100aa03020101bf8377020500bf853e03020100bf85404c304a0420879cd3f18ea76e244d4d4ac3bcb9c337c13b4667190b19035afe2536550050f10101ff0a010004203f4136ee3581e6aba8ea337a6b43d703de1eca241f9b7f277ecdfafff7a8dcf1bf854105020301d4c0bf85420502030315debf854e06020401348abdbf854f06020401348abd300c06082a8648ce3d04030205000347003044022033a613cce9a6ed25026a492b651f0ac67c3c0289d4e4743168c6903e2faa0bda0220324cd35c4bf2695d71ad12a28868e69232112922eaf0e3699f6add8133d528d9").to_vec().try_into().unwrap(),
                ].try_into().unwrap()
            };
            let script: Script = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec().try_into().unwrap();
            let registration = JobRegistration {
                script: script.clone(),
                allowed_sources: None,
                allow_only_verified_sources: true,
                extra: (),
            };
            let fulfillment = Fulfillment {
                script: script.clone(),
                payload: hex!("00").to_vec(),
            };
            _ = Timestamp::set(Origin::none(), 1657363915001);
            assert_ok!(Acurast::submit_attestation(Origin::signed(1).into(), chain.clone()));
            assert_ok!(Acurast::update_certificate_revocation_list(
                Origin::signed(1).into(),
                updates.clone(),
            ));
            assert_ok!(Acurast::register(Origin::signed(2).into(), registration.clone()));
            assert_err!(Acurast::fulfill(Origin::signed(1), fulfillment.clone(), 2), Error::<Test>::RevokedCertificate);

            let attestation = validate_and_extract_attestation::<Test>(&chain).unwrap();

            assert_eq!(
                events(),
                [
                    Event::Acurast(crate::Event::AttestationStored(attestation, 1)),
                    Event::Acurast(crate::Event::CertificateRecovationListUpdated(1, updates)),
                    Event::Acurast(crate::Event::JobRegistrationStored(registration.clone(), 2)),
                ]
            );
        });
    }
}
