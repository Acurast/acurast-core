#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod traits;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use crate::traits::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::OriginFor;
    use sp_std::prelude::*;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The fulfillment payload.
        type Payload: Parameter + Member + Clone + Into<Vec<u8>>;
        /// Generic parameters
        type Parameters: Parameter + Member + Clone + Into<Vec<u8>>;
        /// Handler to notify the runtime when a new fulfillment is received.
        type OnFulfillment: OnFulfillment<Self>;
        /// Handle origin validation
        type Barrier: ParachainBarrier<Self>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// TODO: We may want to add a storage for job identifiers, allowing acurast parachain to send
    /// the (job identifer + payload). The requester address would be indexed by job identifier.

    // Pallets use events to inform users when important changes are made.
    // https://docs.substrate.io/v3/runtime/events-and-errors
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        FulfillReceived(T::Payload, Option<T::Parameters>),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Dispatchable function that notifies the runtime about a fulfilment coming from acurast parachain.
        #[pallet::weight(Weight::from_ref_time(10_000).saturating_add(T::DbWeight::get().writes(1)))]
        pub fn fulfill(
            origin: OriginFor<T>,
            payload: T::Payload,
            parameters: Option<T::Parameters>,
        ) -> DispatchResult {
            // Check that the extrinsic comes from a trusted xcm channel.
            T::Barrier::ensure_xcm_origin(origin)?;

            // Notify the runtime about the fulfillment.
            match T::OnFulfillment::fulfill(
                payload.clone().into(),
                parameters.clone().map(|parameters| parameters.into()),
            ) {
                Err(err) => Err(err.error),
                Ok(_) => {
                    // Emit events
                    Self::deposit_event(Event::FulfillReceived(payload, parameters));

                    Ok(())
                }
            }
        }
    }
}
