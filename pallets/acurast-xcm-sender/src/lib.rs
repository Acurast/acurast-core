#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::inherent::Vec;
    use frame_support::{
        dispatch::DispatchResult, pallet_prelude::*
    };
    use frame_system::pallet_prelude::*;
    use xcm::v2::Instruction::Transact;
    use xcm::v2::{
        Junction::{Parachain},
        Junctions::X1,
        SendXcm, Xcm,
    };
    use xcm::v2::{OriginKind, SendError};
    use scale_info::prelude::vec;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type XcmSender: SendXcm;
        type AcurastReceiverPalletId: Get<u8>;
        type AcurastParachainId: Get<u32>;
    }

    #[pallet::error]
    pub enum Error<T> {
        XcmError
    }

    #[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
    pub enum ReceiverCall {
        #[codec(index = 0u8)]
        Fulfill(Vec<u8>),
    }

    pub fn acurast_receiver_call<T: Config>(
        call: ReceiverCall,
        caller: T::AccountId,
    ) -> DispatchResult {
        // Encode extrinsic call version of the call
        // - first byte is the pallet id on the destination chain;
        let mut encoded_call = vec![T::AcurastReceiverPalletId::get() as u8];
        // - second byte is the position of the extrinsic function being called;
        // - and the remaining bytes are the extrinsic parameter SCALE encoded.
        encoded_call.append(&mut call.encode());

        // Add transact instruction with the fulfill call.
        let transaction = Transact {
            origin_type: OriginKind::Xcm,
            require_weight_at_most: 1_000_000_000u64, // TODO: Review this value
            call:encoded_call.into(),
        };

        // Submit the xcm message
        match T::XcmSender::send_xcm(
            (1, X1(Parachain(T::AcurastParachainId::get()))),
            Xcm(vec![transaction]),
        ) {
            Ok(()) => {
                Pallet::<T>::deposit_event(Event::XcmSent { call, caller });
                Ok(())
            }
            Err(error) => {
                Pallet::<T>::deposit_event(Event::XcmNotSent {
                    call,
                    caller,
                    error,
                });
                Err(Error::<T>::XcmError.into())
            }
        }
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        XcmSent {
            call: ReceiverCall,
            caller: T::AccountId,
        },
        XcmNotSent {
            call: ReceiverCall,
            caller: T::AccountId,
            error: SendError,
        },
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Fulfills a previously registered job.
        #[pallet::weight(10_000)] // TODO: update weight
        pub fn fulfill(origin: OriginFor<T>, fulfillment: Vec<u8>) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let proxy_call = ReceiverCall::Fulfill(fulfillment);
            acurast_receiver_call::<T>(proxy_call, caller)
        }
    }
}
