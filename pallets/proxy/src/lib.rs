#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;


#[frame_support::pallet]
pub mod pallet {
	use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*, sp_runtime::traits::{StaticLookup},};
	use frame_system::pallet_prelude::*;
	use pallet_acurast::{JobRegistration, Script, Fulfillment, AttestationChain,
						 CertificateRevocationListUpdate, AllowedSourcesUpdate};
	use xcm::v2::{OriginKind, SendError, };
	use xcm::v2::Instruction::{Transact, DescendOrigin};
	use xcm::v2::{Junction::{Parachain, AccountId32}, SendXcm, Xcm, Junctions::{X1}};
	use xcm::v2::prelude::*;
	use frame_support::inherent::Vec;
	// use bytes:str::ToBytes;

	#[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[allow(non_camel_case_types)]
	pub enum ProxyCall<T: Config> {
		#[codec(index = 0u8)]
		register { registration: JobRegistration<T::AccountId, T::RegistrationExtra> },

		#[codec(index = 1u8)]
		deregister { script: Script },

		#[codec(index = 2u8)]
		update_allowed_sources { script: Script, updates: Vec<AllowedSourcesUpdate<T::AccountId>> },

		#[codec(index = 3u8)]
		fulfill { fulfillment: Fulfillment, requester: <T::Lookup as StaticLookup>::Source },

		#[codec(index = 4u8)]
		submit_attestation { attestation_chain: AttestationChain },

		#[codec(index = 5u8)]
		update_certificate_revocation_list { update: CertificateRevocationListUpdate }
	}

	pub fn acurast_call<T: Config>(proxy: ProxyCall<T>, accountId: [u8; 32]) -> Result<(), SendError> {
		let mut xcm_message = Vec::new();

		// create an encoded version of the call composed of the first byte being the pallet id
		// on the destination chain, second byte the position of the calling function on the enum,
		// and then the arguments SCALE encoded in order
		let mut encoded_call = Vec::<u8>::new();
		encoded_call.push(T::AcurastPalletId::get() as u8);
		encoded_call.append(&mut proxy.encode());
		log::info!("encoded call is: {:?}", encoded_call);

		// xcm_message.push(ClearOrigin);
		// put our transact message in the vector of instructions
		xcm_message.push(DescendOrigin(X1(AccountId32 {
			network: NetworkId::Any,
			id: accountId
		})));

		xcm_message.push(Transact {
			origin_type: OriginKind::Xcm,
			require_weight_at_most: 1_000_000_000 as u64,
			call: encoded_call.into(),
		});


		// use router to send the xcm message
		return match T::XcmSender::send_xcm(
			(1, X1(Parachain(T::AcurastParachainId::get()))),
			Xcm(xcm_message),
		) {
			Ok(_) => {
				Ok(())
			},
			Err(e) => {
				Err(e)
			},
		};
	}
	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// Extra structure to include in the registration of a job.
		type RegistrationExtra: Parameter + Member + MaxEncodedLen + Eq;
		type XcmSender: SendXcm;
		type AcurastPalletId: Get<u8>;
		type AcurastParachainId: Get<u32>;
		// type AccountId: Parameter
		// + Member
		// + MaybeSerializeDeserialize
		// + Debug
		// + MaybeDisplay
		// + Ord
		// + MaxEncodedLen
		// + Into<[u8;32]>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		SomethingStored(u32, T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Registers a job by providing a [Registration]. If a job for the same script was previously registered, it will be overwritten.
		// TODO: Define proxy weight
		#[pallet::weight(10_000)]
		pub fn register(
			origin: OriginFor<T>,
			registration: JobRegistration<T::AccountId, T::RegistrationExtra>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let who_bytes = who.encode().try_into().unwrap();
			match acurast_call::<T>(ProxyCall::register { registration }, who_bytes) {
				Ok(_result) => return Ok(().into()),
				Err(_error) => return Err("xcm to acurast failed".into())
			}
		}

		/// Deregisters a job for the given script.
		#[pallet::weight(10_000)]
		pub fn deregister(origin: OriginFor<T>, script: Script) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			match acurast_call::<T>(ProxyCall::deregister { script }, who.encode().try_into().unwrap()) {
				Ok(_result) => return Ok(().into()),
				Err(_error) => return Err("xcm to acurast failed".into())
			}
		}

		/// Updates the allowed sources list of a [Registration].
		#[pallet::weight(10_000)]
		pub fn update_allowed_sources(
			origin: OriginFor<T>,
			script: Script,
			updates: Vec<AllowedSourcesUpdate<T::AccountId>>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			match acurast_call::<T>(ProxyCall::update_allowed_sources { script, updates }, who.encode().try_into().unwrap()) {
				Ok(_result) => return Ok(().into()),
				Err(_error) => return Err("xcm to acurast failed".into())
			}
		}

		/// Fulfills a previously registered job.
		#[pallet::weight(10_000 )]
		pub fn fulfill(
			origin: OriginFor<T>,
			fulfillment: Fulfillment,
			requester: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			match acurast_call::<T>(ProxyCall::fulfill { fulfillment, requester }, who.encode().try_into().unwrap()) {
				Ok(_result) => return Ok(().into()),
				Err(_error) => return Err("xcm to acurast failed".into())
			}
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
		#[pallet::weight(10_000)]
		pub fn submit_attestation(
			origin: OriginFor<T>,
			attestation_chain: AttestationChain,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			match acurast_call::<T>(ProxyCall::submit_attestation { attestation_chain }, who.encode().try_into().unwrap()) {
				Ok(_result) => return Ok(().into()),
				Err(_error) => return Err("xcm to acurast failed".into())
			}
		}

		#[pallet::weight(0)]
		pub fn update_certificate_revocation_list(
			origin: OriginFor<T>,
			update: CertificateRevocationListUpdate,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			match acurast_call::<T>(ProxyCall::update_certificate_revocation_list { update }, who.encode().try_into().unwrap()) {
				Ok(_result) => return Ok(().into()),
				Err(_error) => return Err("xcm to acurast failed".into())
			}
		}
	}
}
