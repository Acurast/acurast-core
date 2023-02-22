#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
    use sp_std::prelude::*;
    use xcm::v2::{
        Instruction::Transact, Junction, MultiLocation, OriginKind, SendError, SendXcm, Xcm,
    };

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type XcmSender: SendXcm;
    }

    #[pallet::error]
    pub enum Error<T> {
        XcmError,
        InvalidDestination,
    }

    #[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
    pub enum ReceiverCall {
        #[codec(index = 0u8)]
        Fulfill(Vec<u8>, Option<Vec<u8>>),
    }

    fn split_multi_location<T: Config>(
        multi_location: MultiLocation,
    ) -> Result<(MultiLocation, u8), Error<T>> {
        match multi_location.split_last_interior() {
            (multi_location, Some(Junction::PalletInstance(value))) => Ok((multi_location, value)),
            _ => Err(Error::<T>::InvalidDestination),
        }
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    impl<T: Config> Pallet<T> {
        pub fn send(
            caller: T::AccountId,
            destination: MultiLocation,
            // SBP-M1 review: use BoundedVec...
            payload: Vec<u8>,
            parameters: Option<Vec<u8>>,
        ) -> DispatchResult {
            let (xcm_destination, pallet_instance) = split_multi_location::<T>(destination)?;
            // Encode extrinsic call version of the call
            // - first byte is the pallet id on the destination chain;
            let mut encoded_call = vec![pallet_instance];
            // - second byte is the position of the extrinsic function being called;
            // - and the remaining bytes are the extrinsic parameter SCALE encoded.
            let call = ReceiverCall::Fulfill(payload, parameters);
            encoded_call.append(&mut call.encode());

            // Add transact instruction with the fulfill call.
            let transaction = Transact {
                origin_type: OriginKind::Xcm,
                // SBP-M1 review: just a note, 2D weights are used in feature releases
                require_weight_at_most: 1_000_000_000u64, // TODO: Review this value
                call: encoded_call.into(),
            };

            // Submit the xcm message
            match T::XcmSender::send_xcm(xcm_destination, Xcm(vec![transaction])) {
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
    }

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
}
