#![cfg_attr(not(feature = "std"), no_std)]

pub mod attestation;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
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
        pub cert_id: CertId,
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
        /// The allowed sources have been updated. [who, old_registration, allowed_sources, operation]
        AllowedSourcesUpdated(
            T::AccountId,
            JobRegistration<T::AccountId, T::RegistrationExtra>,
            Vec<AllowedSourcesUpdate<T::AccountId>>,
        ),
        /// An attestation was successfully stored. [attestation, who]
        AttestationStored(Attestation, T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Fulfill was executed for a not registered job.
        JobRegistrationNotFound,
        /// The source of the fulfill is not allowed for the job.
        FulfillSourceNotAllowed,
        /// The allowed soruces list for a registration exeeded the max length.
        TooManyAllowedSources,
        /// The allowed soruces list for a registration cannot be empty if provided.
        TooFewAllowedSources,
        /// The provided script value is not valid. The value needs to be and ipfs:// url.
        InvalidScriptValue,
        /// The provided attestation could not be parsed or is invalid.
        AttestationInvalid,
        /// The certificate chain provided in the submit_attestation call is not long enough.
        CertificateChainTooShort,
        /// The submitted attestation root certificate is not valid.
        RootCertificateValidationFailed,
        /// The submitted attestation certificate chain is not valid.
        CertificateChainValidationFailed,
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
                .map(|sources| sources.len())
                .unwrap_or(0);
            let max_allowed_sources_len = T::MaxAllowedSources::get() as usize;
            ensure!(allowed_sources_len > 0, Error::<T>::TooFewAllowedSources);
            ensure!(
                allowed_sources_len <= max_allowed_sources_len,
                Error::<T>::TooManyAllowedSources
            );
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
        ///
        /// TODO: implement revocation
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

            validate_certificate_chain_root(&attestation_chain.certificate_chain)
                .map_err(|_| Error::<T>::RootCertificateValidationFailed)?;

            let (cert_ids, cert) = validate_certificate_chain(&attestation_chain.certificate_chain)
                .map_err(|_| Error::<T>::CertificateChainValidationFailed)?;

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

            let attestation = Attestation {
                cert_ids: cert_ids_bounded_vec,
                key_description: match key_description {
                    KeyDescription::V1(kd) => kd
                        .try_into()
                        .map_err(|_| Error::<T>::AttestationToBoundedTypeConversionFailed)?,
                    KeyDescription::V2(kd) => kd
                        .try_into()
                        .map_err(|_| Error::<T>::AttestationToBoundedTypeConversionFailed)?,
                    KeyDescription::V3(kd) => kd
                        .try_into()
                        .map_err(|_| Error::<T>::AttestationToBoundedTypeConversionFailed)?,
                    KeyDescription::V4(kd) => kd
                        .try_into()
                        .map_err(|_| Error::<T>::AttestationToBoundedTypeConversionFailed)?,
                    KeyDescription::V100(kd) => kd
                        .try_into()
                        .map_err(|_| Error::<T>::AttestationToBoundedTypeConversionFailed)?,
                    KeyDescription::V200(kd) => kd
                        .try_into()
                        .map_err(|_| Error::<T>::AttestationToBoundedTypeConversionFailed)?,
                },
            };
            <StoredAttestation<T>>::insert(who.clone(), attestation.clone());
            Self::deposit_event(Event::AttestationStored(attestation, who));
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn update_certificate_revocation_list(
            origin: OriginFor<T>,
            update: CertificateRevocationListUpdate,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            if !T::AllowedRevocationListUpdate::get().contains(&who) {
                return Err(Error::<T>::CertificateRevocationListUpdateNotAllowed)?;
            }
            match &update.operation {
                ListUpdateOperation::Add => {
                    <StoredRevokedCertificate<T>>::insert(update.cert_id, ());
                }
                ListUpdateOperation::Remove => {
                    <StoredRevokedCertificate<T>>::remove(update.cert_id);
                }
            }
            Ok(().into())
        }
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
                <StoredAttestation<T>>::get(source).ok_or(Error::<T>::FulfillSourceNotAllowed)?;
            ensure_not_expired(&attestation)?;
            ensure_not_revoked(&attestation)?;
        }

        Ok(())
    }

    fn ensure_not_expired<T: Config>(attestation: &Attestation) -> Result<(), Error<T>> {
        let expire_date_time = (&attestation)
            .key_description
            .tee_enforced
            .usage_expire_date_time
            .unwrap_or(
                (&attestation)
                    .key_description
                    .software_enforced
                    .usage_expire_date_time
                    .unwrap_or_default(),
            );
        let now: u64 = <pallet_timestamp::Pallet<T>>::now()
            .try_into()
            .map_err(|_| Error::<T>::FailedTimestampConversion)?;
        if now >= expire_date_time {
            return Err(Error::<T>::FulfillSourceNotAllowed);
        }
        Ok(())
    }

    fn ensure_not_revoked<T: Config>(attestation: &Attestation) -> Result<(), Error<T>> {
        let ids = &attestation.cert_ids;
        for id in ids {
            _ = <StoredRevokedCertificate<T>>::get(id).ok_or(Error::<T>::RevokedCertificate)?;
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
    pub type StoredRevokedCertificate<T: Config> = StorageMap<_, Blake2_128Concat, CertId, ()>;

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
    }

    #[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
    pub struct BoundedKeyDescription {
        pub attestation_security_level: AttestationSecurityLevel,
        pub key_mint_security_level: AttestationSecurityLevel,
        pub software_enforced: BoundedAuthorizationList,
        pub tee_enforced: BoundedAuthorizationList,
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
        pub key_size: Option<u8>,
        pub digest: Option<Digest>,
        pub padding: Option<Padding>,
        pub ec_curve: Option<u8>,
        pub rsa_public_exponent: Option<u64>,
        pub mgf_digest: Option<MgfDigest>,
        pub rollback_resistance: bool,
        pub early_boot_only: bool,
        pub active_date_time: Option<u64>,
        pub origination_expire_date_time: Option<u64>,
        pub usage_expire_date_time: Option<u64>,
        pub usage_count_limit: Option<u64>,
        pub no_auth_required: bool,
        pub user_auth_type: Option<u8>,
        pub auth_timeout: Option<u32>,
        pub allow_while_on_body: bool,
        pub trusted_user_presence_required: bool,
        pub trusted_confirmation_required: bool,
        pub unlocked_device_required: bool,
        pub all_applications: bool,
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
        pub device_unique_attestation: bool,
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
                key_size: try_bound!(data.key_size, u8)?,
                digest: try_bound_set!(data.digest, Digest, u8)?,
                padding: try_bound_set!(data.padding, Padding, u8)?,
                ec_curve: try_bound!(data.ec_curve, u8)?,
                rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
                mgf_digest: None,
                rollback_resistance: data.rollback_resistance.is_some(),
                early_boot_only: false,
                active_date_time: try_bound!(data.active_date_time, u64)?,
                origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
                usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
                usage_count_limit: None,
                no_auth_required: data.no_auth_required.is_some(),
                user_auth_type: try_bound!(data.user_auth_type, u8)?,
                auth_timeout: try_bound!(data.user_auth_type, u32)?,
                allow_while_on_body: data.allow_while_on_body.is_some(),
                trusted_user_presence_required: false,
                trusted_confirmation_required: false,
                unlocked_device_required: false,
                all_applications: data.all_applications.is_some(),
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
                device_unique_attestation: false,
            })
        }
    }

    impl TryFrom<asn::AuthorizationListV2<'_>> for BoundedAuthorizationList {
        type Error = ();

        fn try_from(data: asn::AuthorizationListV2) -> Result<Self, Self::Error> {
            Ok(BoundedAuthorizationList {
                purpose: try_bound_set!(data.purpose, Purpose, u8)?,
                algorithm: try_bound!(data.algorithm, u8)?,
                key_size: try_bound!(data.key_size, u8)?,
                digest: try_bound_set!(data.digest, Digest, u8)?,
                padding: try_bound_set!(data.padding, Padding, u8)?,
                ec_curve: try_bound!(data.ec_curve, u8)?,
                rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
                mgf_digest: None,
                rollback_resistance: data.rollback_resistance.is_some(),
                early_boot_only: false,
                active_date_time: try_bound!(data.active_date_time, u64)?,
                origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
                usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
                usage_count_limit: None,
                no_auth_required: data.no_auth_required.is_some(),
                user_auth_type: try_bound!(data.user_auth_type, u8)?,
                auth_timeout: try_bound!(data.user_auth_type, u32)?,
                allow_while_on_body: data.allow_while_on_body.is_some(),
                trusted_user_presence_required: false,
                trusted_confirmation_required: false,
                unlocked_device_required: false,
                all_applications: data.all_applications.is_some(),
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
                device_unique_attestation: false,
            })
        }
    }

    impl TryFrom<asn::AuthorizationListV3<'_>> for BoundedAuthorizationList {
        type Error = ();

        fn try_from(data: asn::AuthorizationListV3) -> Result<Self, Self::Error> {
            Ok(BoundedAuthorizationList {
                purpose: try_bound_set!(data.purpose, Purpose, u8)?,
                algorithm: try_bound!(data.algorithm, u8)?,
                key_size: try_bound!(data.key_size, u8)?,
                digest: try_bound_set!(data.digest, Digest, u8)?,
                padding: try_bound_set!(data.padding, Padding, u8)?,
                ec_curve: try_bound!(data.ec_curve, u8)?,
                rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
                mgf_digest: None,
                rollback_resistance: data.rollback_resistance.is_some(),
                early_boot_only: false,
                active_date_time: try_bound!(data.active_date_time, u64)?,
                origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
                usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
                usage_count_limit: None,
                no_auth_required: data.no_auth_required.is_some(),
                user_auth_type: try_bound!(data.user_auth_type, u8)?,
                auth_timeout: try_bound!(data.user_auth_type, u32)?,
                allow_while_on_body: data.allow_while_on_body.is_some(),
                trusted_user_presence_required: false,
                trusted_confirmation_required: false,
                unlocked_device_required: false,
                all_applications: data.all_applications.is_some(),
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
                device_unique_attestation: false,
            })
        }
    }

    impl TryFrom<asn::AuthorizationListV4<'_>> for BoundedAuthorizationList {
        type Error = ();

        fn try_from(data: asn::AuthorizationListV4) -> Result<Self, Self::Error> {
            Ok(BoundedAuthorizationList {
                purpose: try_bound_set!(data.purpose, Purpose, u8)?,
                algorithm: try_bound!(data.algorithm, u8)?,
                key_size: try_bound!(data.key_size, u8)?,
                digest: try_bound_set!(data.digest, Digest, u8)?,
                padding: try_bound_set!(data.padding, Padding, u8)?,
                ec_curve: try_bound!(data.ec_curve, u8)?,
                rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
                mgf_digest: None,
                rollback_resistance: data.rollback_resistance.is_some(),
                early_boot_only: data.early_boot_only.is_some(),
                active_date_time: try_bound!(data.active_date_time, u64)?,
                origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
                usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
                usage_count_limit: None,
                no_auth_required: data.no_auth_required.is_some(),
                user_auth_type: try_bound!(data.user_auth_type, u8)?,
                auth_timeout: try_bound!(data.user_auth_type, u32)?,
                allow_while_on_body: data.allow_while_on_body.is_some(),
                trusted_user_presence_required: data.trusted_user_presence_required.is_some(),
                trusted_confirmation_required: data.trusted_confirmation_required.is_some(),
                unlocked_device_required: data.unlocked_device_required.is_some(),
                all_applications: data.all_applications.is_some(),
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
                device_unique_attestation: data.device_unique_attestation.is_some(),
            })
        }
    }

    impl TryFrom<asn::AuthorizationListV100V200<'_>> for BoundedAuthorizationList {
        type Error = ();

        fn try_from(data: asn::AuthorizationListV100V200) -> Result<Self, Self::Error> {
            Ok(BoundedAuthorizationList {
                purpose: try_bound_set!(data.purpose, Purpose, u8)?,
                algorithm: try_bound!(data.algorithm, u8)?,
                key_size: try_bound!(data.key_size, u8)?,
                digest: try_bound_set!(data.digest, Digest, u8)?,
                padding: try_bound_set!(data.padding, Padding, u8)?,
                ec_curve: try_bound!(data.ec_curve, u8)?,
                rsa_public_exponent: try_bound!(data.rsa_public_exponent, u64)?,
                mgf_digest: try_bound_set!(data.mgf_digest, MgfDigest, u8)?,
                rollback_resistance: data.rollback_resistance.is_some(),
                early_boot_only: data.early_boot_only.is_some(),
                active_date_time: try_bound!(data.active_date_time, u64)?,
                origination_expire_date_time: try_bound!(data.origination_expire_date_time, u64)?,
                usage_expire_date_time: try_bound!(data.usage_expire_date_time, u64)?,
                usage_count_limit: try_bound!(data.usage_count_limit, u64)?,
                no_auth_required: data.no_auth_required.is_some(),
                user_auth_type: try_bound!(data.user_auth_type, u8)?,
                auth_timeout: try_bound!(data.user_auth_type, u32)?,
                allow_while_on_body: data.allow_while_on_body.is_some(),
                trusted_user_presence_required: data.trusted_user_presence_required.is_some(),
                trusted_confirmation_required: data.trusted_confirmation_required.is_some(),
                unlocked_device_required: data.unlocked_device_required.is_some(),
                all_applications: true, // default because version 100 misses this field
                application_id: None,   // default because version 100 misses this field
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
                device_unique_attestation: data.device_unique_attestation.is_some(),
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
