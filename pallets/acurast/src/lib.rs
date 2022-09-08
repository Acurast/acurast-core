#![cfg_attr(not(feature = "std"), no_std)]

pub mod attestation;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use crate::attestation::*;
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
			registration: Registration<T::AccountId, T::RegistrationExtra>,
			requester: <T::Lookup as StaticLookup>::Target,
		) -> DispatchResultWithPostInfo;
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// Extra structure to include in the registration of a job.
		type RegistrationExtra: Parameter + Member + MaxEncodedLen;
		/// The fulfillment router to route a job fulfillment to its final destination.
		type FulfillmentRouter: FulfillmentRouter<Self>;
		/// The max length of the allowed sources list for a registration.
		#[pallet::constant]
		type MaxAllowedSources: Get<u16>;
		#[pallet::constant]
		type MaxAttestations: Get<u16>;
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
	pub struct Registration<A, T>
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
		pub operation: AllowedSourcesUpdateOperation,
		/// The [AccountId] to add or remove.
		pub account_id: A,
	}

	/// The allowed sources update operation.
	#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Copy)]
	pub enum AllowedSourcesUpdateOperation {
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
		Registration<T::AccountId, T::RegistrationExtra>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A registration was successfully stored. [registration, who]
		RegistrationStored(
			Registration<T::AccountId, T::RegistrationExtra>,
			T::AccountId,
		),
		/// A registration was successfully removed. [registration, who]
		RegistrationRemoved(Script, T::AccountId),
		/// A fulfillment has been posted. [who, fulfillment, registration, receiver]
		ReceivedFulfillment(
			T::AccountId,
			Fulfillment,
			Registration<T::AccountId, T::RegistrationExtra>,
			T::AccountId,
		),
		/// The allowed sources have been updated. [who, old_registration, allowed_sources, operation]
		AllowedSourcesUpdated(
			T::AccountId,
			Registration<T::AccountId, T::RegistrationExtra>,
			Vec<AllowedSourcesUpdate<T::AccountId>>,
		),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Fulfill was executed for a not registered job.
		RegistrationNotFound,
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
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Registers a job by providing a [Registration]. If a job for the same script was previously registered, it will be overwritten.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn register(
			origin: OriginFor<T>,
			registration: Registration<T::AccountId, T::RegistrationExtra>,
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
			Self::deposit_event(Event::RegistrationStored(registration, who));
			Ok(().into())
		}

		/// Deregisters a job for the given script.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn deregister(origin: OriginFor<T>, script: Script) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			<StoredRegistration<T>>::remove(who.clone(), script.clone());
			Self::deposit_event(Event::RegistrationRemoved(script, who));
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
				.ok_or(Error::<T>::RegistrationNotFound)?;

			let mut current_allowed_sources =
				(&registration).allowed_sources.clone().unwrap_or(vec![]);
			for update in &updates {
				let position = current_allowed_sources
					.iter()
					.position(|value| value == &update.account_id);
				match (position, update.operation) {
					(None, AllowedSourcesUpdateOperation::Add) => {
						current_allowed_sources.push(update.account_id.clone())
					}
					(Some(pos), AllowedSourcesUpdateOperation::Remove) => {
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
				Registration {
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
		#[pallet::weight(10_000 + T::DbWeight::get().reads(1))]
		pub fn fulfill(
			origin: OriginFor<T>,
			fulfillment: Fulfillment,
			requester: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin.clone())?;
			let requester = T::Lookup::lookup(requester)?;

			let registration =
				<StoredRegistration<T>>::get(requester.clone(), (&fulfillment).script.clone())
					.ok_or(Error::<T>::RegistrationNotFound)?;

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
				(&attestation_chain).certificate_chain.len() < 2,
				Error::<T>::AttestationInvalid
			);

			validate_certificate_chain_root(&attestation_chain.certificate_chain)
				.map_err(|_| Error::<T>::AttestationInvalid)?;

			let (cert_ids, cert) = validate_certificate_chain(&attestation_chain.certificate_chain)
				.map_err(|_| Error::<T>::AttestationInvalid)?;

			let key_description =
				extract_attestation(cert.extensions).map_err(|_| Error::<T>::AttestationInvalid)?;

			let cert_ids_bounded = cert_ids
				.iter()
				.map(|cert_id| {
					let (iss, sn) = cert_id;
					let iss_bounded = IssuerName::try_from(iss.clone())
						.map_err(|_| Error::<T>::AttestationInvalid)?;
					let sn_bounded = SerialNumber::try_from(sn.clone())
						.map_err(|_| Error::<T>::AttestationInvalid)?;
					Ok((iss_bounded, sn_bounded))
				})
				.collect::<Result<Vec<CertId>, Error<T>>>()?;
			let cert_ids_bounded_vec = ValidatingCertIds::try_from(cert_ids_bounded)
				.map_err(|_| Error::<T>::AttestationInvalid)?;

			<StoredAttestation<T>>::insert(
				who.clone(),
				Attestation {
					cert_ids: cert_ids_bounded_vec,
					key_description: key_description.into(),
				},
			);
			// Self::deposit_event(Event::AttestationStored(attestation, who));
			Ok(().into())
		}
	}

	fn ensure_source_allowed<T: Config>(
		source: &T::AccountId,
		registration: &Registration<T::AccountId, T::RegistrationExtra>,
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
			.unwrap_or(Ok(()))
	}

	/// The storage for [Attestation]s. They are stored by [AccountId] and [Attestation::serial_number].
	#[pallet::storage]
	#[pallet::getter(fn stored_attestation)]
	pub type StoredAttestation<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Attestation>;

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
	const ATTESTATION_ID_MAX_LENGTH: u32 = 16;

	pub type Purpose = BoundedVec<u8, ConstU32<PURPOSE_MAX_LENGTH>>;
	pub type Digest = BoundedVec<u8, ConstU32<DIGEST_MAX_LENGTH>>;
	pub type Padding = BoundedVec<u8, ConstU32<PADDING_MAX_LENGTH>>;
	pub type MgfDigest = BoundedVec<u8, ConstU32<MGF_DIGEST_MAX_LENGTH>>;
	pub type VerifiedBootKey = BoundedVec<u8, ConstU32<VERIFIED_BOOT_KEY_MAX_LENGTH>>;
	pub type VerifiedBootHash = BoundedVec<u8, ConstU32<VERIFIED_BOOT_HASH_MAX_LENGTH>>;
	pub type AttestationIdProperty = BoundedVec<u8, ConstU32<ATTESTATION_ID_MAX_LENGTH>>;
	pub type CertId = (IssuerName, SerialNumber);
	pub type ValidatingCertIds = BoundedVec<CertId, ConstU32<CHAIN_MAX_LENGTH>>;

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

	impl From<asn::KeyDescription<'_>> for BoundedKeyDescription {
		fn from(data: asn::KeyDescription) -> Self {
			BoundedKeyDescription {
				attestation_security_level: data.attestation_security_level.into(),
				key_mint_security_level: data.key_mint_security_level.into(),
				software_enforced: data.software_enforced.into(),
				tee_enforced: data.tee_enforced.into(),
			}
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

	#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
	pub struct BoundedAuthorizationList {
		// pub purpose: Option<Purpose>,
		// pub algorithm: Option<i64>,
		pub key_size: Option<i64>,
		// pub digest: Digest,
		// pub padding: Padding,
		// pub ec_curve: Option<i64>,
		// pub rsa_public_exponent: Option<i64>,
		// pub mgf_digest: MgfDigest,
		// pub rollback_resistance: bool,
		// pub early_boot_only: bool,
		// pub active_date_time: Option<i64>,
		// pub origination_expire_date_time: Option<i64>,
		// pub usage_expire_date_time: Option<i64>,
		// pub usage_count_limit: Option<i64>,
		// pub no_auth_required: bool,
		// pub user_auth_type: Option<i64>,
		// pub auth_timeout: Option<i64>,
		// pub allow_while_on_body: bool,
		// pub trusted_user_presence_required: bool,
		// pub trusted_confirmation_required: bool,
		// pub unlocked_device_required: bool,
		// pub creation_date_time: Option<i64>,
		// pub origin: Option<i64>,
		// pub root_of_trust: Option<BoundedRootOfTrust>,
		// pub os_version: Option<i64>,
		// pub os_patch_level: Option<i64>,
		// pub attestation_application_id: Option<AttestationIdProperty>,
		// pub attestation_id_brand: Option<AttestationIdProperty>,
		// pub attestation_id_device: Option<AttestationIdProperty>,
		// pub attestation_id_product: Option<AttestationIdProperty>,
		// pub attestation_id_serial: Option<AttestationIdProperty>,
		// pub attestation_id_imei: Option<AttestationIdProperty>,
		// pub attestation_id_meid: Option<AttestationIdProperty>,
		// pub attestation_id_manufacturer: Option<AttestationIdProperty>,
		// pub attestation_id_model: Option<AttestationIdProperty>,
		// pub vendor_patch_level: Option<i64>,
		// pub boot_patch_level: Option<i64>,
		// pub device_unique_attestation: bool,
	}

	impl From<asn::AuthorizationList<'_>> for BoundedAuthorizationList {
		fn from(data: asn::AuthorizationList) -> Self {
			BoundedAuthorizationList {
				key_size: data.key_size,
			}
		}
	}

	#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
	pub struct BoundedRootOfTrust {
		pub verified_boot_key: VerifiedBootKey,
		pub device_locked: bool,
		pub verified_boot_state: VerifiedBootState,
		pub verified_boot_hash: VerifiedBootHash,
	}

	#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
	pub enum VerifiedBootState {
		Verified,
		SelfSigned,
		Unverified,
		Failed,
	}
}
