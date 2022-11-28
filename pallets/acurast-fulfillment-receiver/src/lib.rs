#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
pub use pallet_acurast::Fulfillment;

pub mod traits;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use traits::*;
    use types::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::OriginFor;
    use sp_std::prelude::*;
    use pallet_acur

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// The fulfillment payload.
        type Payload: Parameter + Member + Clone + IsType<Vec<u8>>;
        /// Handler to notify the runtime when a new fulfillment is received.
        type OnFulfillment: OnFulfillment<Self>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

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

        #[pallet::weight(Weight::from_ref_time(10_000).saturating_add(T::DbWeight::get().writes(1)))]
        pub fn fulfill(
            origin: OriginFor<T>,
            payload: T::Payload,
        ) -> DispatchResult {
            // Check that the extrinsic comes from a trusted xcm channel.
            let who = ensure_signed(origin)?;

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
