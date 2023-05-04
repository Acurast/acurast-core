#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod traits;

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;
pub use traits::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::inherent::Vec;
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use xcm::prelude::{
        Instruction::{DescendOrigin, Transact},
        Junction::{AccountId32, Parachain},
        Junctions::X1,
        MultiLocation, OriginKind, SendError, SendXcm, Xcm,
    };

    #[cfg(feature = "runtime-benchmarks")]
    use crate::benchmarking::BenchmarkHelper;
    use crate::WeightInfo;
    use acurast_common::{AllowedSourcesUpdate, JobIdSequence, JobRegistration};
    use pallet_acurast_marketplace::Advertisement;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Extra structure to include in the registration of a job.
        type RegistrationExtra: Parameter + Member;
        #[pallet::constant]
        type MaxAllowedSources: Get<u32>;
        type MaxAllowedConsumers: Get<u32> + Parameter;
        type AssetId: Parameter + Member;
        type AssetAmount: Parameter;
        type XcmSender: SendXcm;
        type AcurastPalletId: Get<u8>;
        type AcurastMarketplacePalletId: Get<u8>;
        type AcurastParachainId: Get<u32>;
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: BenchmarkHelper<Self>;
        type WeightInfo: WeightInfo;
    }

    #[pallet::error]
    pub enum Error<T> {
        XcmError,
        AccountIdToBytesConversionFailed,
    }

    #[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
    pub enum ProxyCall<T: Config> {
        #[codec(index = 0u8)]
        Register {
            registration: JobRegistration<T::AccountId, T::RegistrationExtra>,
        },

        #[codec(index = 1u8)]
        Deregister { local_job_id: JobIdSequence },

        #[codec(index = 2u8)]
        UpdateAllowedSources {
            job_id: JobIdSequence,
            updates: BoundedVec<AllowedSourcesUpdate<T::AccountId>, T::MaxAllowedSources>,
        },

        #[codec(index = 0u8)]
        Advertise {
            advertisement:
                Advertisement<T::AccountId, T::AssetId, T::AssetAmount, T::MaxAllowedConsumers>,
        },
    }

    #[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
    pub enum ExtrinsicName {
        Register,
        Deregister,
        UpdateAllowedSources,
        Advertise,
    }

    impl<T: Config> ProxyCall<T> {
        fn get_name(&self) -> ExtrinsicName {
            match self {
                ProxyCall::Register { .. } => ExtrinsicName::Register,
                ProxyCall::Deregister { .. } => ExtrinsicName::Deregister,
                ProxyCall::UpdateAllowedSources { .. } => ExtrinsicName::UpdateAllowedSources,
                ProxyCall::Advertise { .. } => ExtrinsicName::Advertise,
            }
        }
    }

    pub fn acurast_call<T: Config>(
        proxy_call: ProxyCall<T>,
        caller: T::AccountId,
        pallet_id: u8,
    ) -> DispatchResult {
        // extract bytes from struct
        let account_bytes: [u8; 32] = caller
            .encode()
            .try_into()
            .map_err(|_| Error::<T>::AccountIdToBytesConversionFailed)?;
        let mut xcm_message = Vec::new();
        let extrinsic = proxy_call.get_name();

        // create an encoded version of the call
        let mut encoded_call = Vec::<u8>::new();
        // first byte is the pallet id on the destination chain
        encoded_call.push(pallet_id);
        //second byte the position of the calling function on the enum,
        // and then the arguments SCALE encoded in order.
        encoded_call.append(&mut proxy_call.encode());

        // before calling transact, we want to use not the parachain origin, but a user's account
        xcm_message.push(DescendOrigin(X1(AccountId32 {
            network: None,
            id: account_bytes,
        })));

        // put our transact message in the vector of instructions
        xcm_message.push(Transact {
            origin_kind: OriginKind::Xcm,
            require_weight_at_most: 1_000_000_000u64.into(),
            call: encoded_call.into(),
        });

        let mut destination = Some(MultiLocation::new(
            1,
            X1(Parachain(T::AcurastParachainId::get())),
        ));
        let mut message = Some(Xcm(xcm_message));

        // use router to send the xcm message
        let ticket = T::XcmSender::validate(&mut destination, &mut message)
            .map_err(|_| Error::<T>::XcmError)?;
        return match T::XcmSender::deliver(ticket.0) {
            Ok(_) => {
                Pallet::<T>::deposit_event(Event::XcmSent { extrinsic, caller });
                Ok(())
            }
            Err(error) => {
                Pallet::<T>::deposit_event(Event::XcmNotSent {
                    extrinsic,
                    error,
                    caller,
                });
                Err(Error::<T>::XcmError.into())
            }
        };
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        XcmSent {
            extrinsic: ExtrinsicName,
            caller: T::AccountId,
        },
        XcmNotSent {
            extrinsic: ExtrinsicName,
            error: SendError,
            caller: T::AccountId,
        },
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Registers a job by providing a [Registration]. If a job for the same script was previously registered, it will be overwritten.
        // TODO: Define proxy weight
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register())]
        pub fn register(
            origin: OriginFor<T>,
            registration: JobRegistration<T::AccountId, T::RegistrationExtra>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let proxy_call = ProxyCall::Register { registration };
            acurast_call::<T>(proxy_call, caller, T::AcurastPalletId::get())
        }

        /// Deregisters a job for the given script.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::deregister())]
        pub fn deregister(origin: OriginFor<T>, job_id: JobIdSequence) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let proxy_call = ProxyCall::Deregister {
                local_job_id: job_id,
            };
            acurast_call::<T>(proxy_call, caller, T::AcurastPalletId::get())
        }

        /// Updates the allowed sources list of a [Registration].
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::update_allowed_sources())]
        pub fn update_allowed_sources(
            origin: OriginFor<T>,
            job_id: JobIdSequence,
            updates: BoundedVec<AllowedSourcesUpdate<T::AccountId>, T::MaxAllowedSources>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let proxy_call = ProxyCall::UpdateAllowedSources { job_id, updates };
            acurast_call::<T>(proxy_call, caller, T::AcurastPalletId::get())
        }

        /// Advertise resources by providing a [Advertisement]. If an advertisement for the same script was previously registered, it will be overwritten.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::advertise())]
        pub fn advertise(
            origin: OriginFor<T>,
            advertisement: Advertisement<
                T::AccountId,
                T::AssetId,
                T::AssetAmount,
                T::MaxAllowedConsumers,
            >,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let proxy_call = ProxyCall::Advertise { advertisement };
            acurast_call::<T>(proxy_call, caller, T::AcurastMarketplacePalletId::get())
        }
    }
}
