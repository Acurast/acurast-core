#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/v3/runtime/frame>
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::inherent::Vec;
    use frame_support::{
        dispatch::DispatchResult, pallet_prelude::*, sp_runtime::traits::StaticLookup,
    };
    use frame_system::pallet_prelude::*;
    use xcm::v2::prelude::*;
    use xcm::v2::Instruction::{DescendOrigin, Transact};
    use xcm::v2::{
        Junction::{AccountId32, Parachain},
        Junctions::X1,
        SendXcm, Xcm,
    };
    use xcm::v2::{OriginKind, SendError};

    use acurast_common::{AllowedSourcesUpdate, Fulfillment, JobRegistration, Script};
    use pallet_acurast_marketplace::Advertisement;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Extra structure to include in the registration of a job.
        type RegistrationExtra: Parameter + Member;
        type AssetId: Parameter + Member;
        type AssetAmount: Parameter;
        type XcmSender: SendXcm;
        type AcurastPalletId: Get<u8>;
        type AcurastMarketplacePalletId: Get<u8>;
        type AcurastParachainId: Get<u32>;
    }

    #[pallet::error]
    pub enum Error<T> {
        XcmError,
    }

    #[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
    pub enum ProxyCall<T: Config> {
        #[codec(index = 0u8)]
        Register {
            registration: JobRegistration<T::AccountId, T::RegistrationExtra>,
        },

        #[codec(index = 1u8)]
        Deregister { script: Script },

        #[codec(index = 2u8)]
        UpdateAllowedSources {
            script: Script,
            updates: Vec<AllowedSourcesUpdate<T::AccountId>>,
        },

        #[codec(index = 4u8)]
        Fulfill {
            fulfillment: Fulfillment,
            requester: <T::Lookup as StaticLookup>::Source,
        },

        #[codec(index = 0u8)]
        Advertise {
            advertisement: Advertisement<T::AccountId, T::AssetId, T::AssetAmount>,
        },
    }

    #[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
    pub enum ExtrinsicName {
        Register,
        Deregister,
        UpdateAllowedSources,
        Fulfill,
        Advertise,
    }

    impl<T: Config> ProxyCall<T> {
        fn get_name(&self) -> ExtrinsicName {
            match self {
                ProxyCall::Register { .. } => ExtrinsicName::Register,
                ProxyCall::Deregister { .. } => ExtrinsicName::Deregister,
                ProxyCall::UpdateAllowedSources { .. } => ExtrinsicName::UpdateAllowedSources,
                ProxyCall::Fulfill { .. } => ExtrinsicName::Fulfill,
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
        let account_bytes = caller.encode().try_into().unwrap();
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
            network: NetworkId::Any,
            id: account_bytes,
        })));

        // put our transact message in the vector of instructions
        xcm_message.push(Transact {
            origin_type: OriginKind::Xcm,
            require_weight_at_most: 1_000_000_000u64,
            call: encoded_call.into(),
        });

        // use router to send the xcm message
        return match T::XcmSender::send_xcm(
            (1, X1(Parachain(T::AcurastParachainId::get()))),
            Xcm(xcm_message),
        ) {
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
    #[pallet::generate_store(pub(super) trait Store)]
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
        #[pallet::weight(10_000)]
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
        #[pallet::weight(10_000)]
        pub fn deregister(origin: OriginFor<T>, script: Script) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let proxy_call = ProxyCall::Deregister { script };
            acurast_call::<T>(proxy_call, caller, T::AcurastPalletId::get())
        }

        /// Updates the allowed sources list of a [Registration].
        #[pallet::call_index(2)]
        #[pallet::weight(10_000)]
        pub fn update_allowed_sources(
            origin: OriginFor<T>,
            script: Script,
            updates: Vec<AllowedSourcesUpdate<T::AccountId>>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let proxy_call = ProxyCall::UpdateAllowedSources { script, updates };
            acurast_call::<T>(proxy_call, caller, T::AcurastPalletId::get())
        }

        /// Fulfills a previously registered job.
        #[pallet::call_index(3)]
        #[pallet::weight(10_000)]
        pub fn fulfill(
            origin: OriginFor<T>,
            fulfillment: Fulfillment,
            requester: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let proxy_call = ProxyCall::Fulfill {
                fulfillment,
                requester,
            };
            acurast_call::<T>(proxy_call, caller, T::AcurastPalletId::get())
        }

        /// Advertise resources by providing a [Advertisement]. If an advertisement for the same script was previously registered, it will be overwritten.
        #[pallet::call_index(4)]
        #[pallet::weight(10_000)]
        pub fn advertise(
            origin: OriginFor<T>,
            advertisement: Advertisement<T::AccountId, T::AssetId, T::AssetAmount>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let proxy_call = ProxyCall::Advertise { advertisement };
            acurast_call::<T>(proxy_call, caller, T::AcurastMarketplacePalletId::get())
        }
    }
}
