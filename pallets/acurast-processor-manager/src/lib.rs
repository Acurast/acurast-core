#![cfg_attr(not(feature = "std"), no_std)]

pub mod traits;
pub mod types;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use pallet::*;
pub use traits::*;
pub use types::*;

pub type ProcessorPairingFor<T> =
    ProcessorPairing<<T as frame_system::Config>::AccountId, <T as Config>::Proof>;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        dispatch::DispatchResultWithPostInfo,
        pallet_prelude::*,
        sp_runtime::traits::{CheckedAdd, IdentifyAccount, Verify},
        Blake2_128,
    };
    use frame_system::{ensure_signed, pallet_prelude::OriginFor};
    use sp_std::prelude::*;

    use crate::types::*;
    use crate::{traits::*, ProcessorPairingFor};

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Proof: Parameter + Member + Verify;
        type ManagerId: Member + Parameter + MaxEncodedLen + Copy + CheckedAdd + From<u128>;
        type ManagerToken: ManagerToken<Self>;
        /// Weight Info for extrinsics.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn last_manager_id)]
    pub(super) type LastManagerId<T: Config> = StorageValue<_, T::ManagerId>;

    #[pallet::storage]
    #[pallet::getter(fn managed_processors)]
    pub(super) type ManagedProcessors<T: Config> =
        StorageDoubleMap<_, Blake2_128, T::ManagerId, Blake2_128Concat, T::AccountId, ()>;

    #[pallet::storage]
    #[pallet::getter(fn manager_id_for_processor)]
    pub(super) type ProcessorToManagerIdIndex<T: Config> =
        StorageMap<_, Blake2_128, T::AccountId, T::ManagerId>;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ManagerCreated(T::AccountId),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        ManagerAlreadyCreated,
        FailedToCreateManagerId,
        ProcessorAlreadyPaired,
        InvalidPairingProof,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
    {
        /// Submit a fulfillment for an acurast job.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_manager())]
        pub fn create_manager(
            origin: OriginFor<T>,
            pairings: Option<Vec<ProcessorPairingFor<T>>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let id = <LastManagerId<T>>::get()
                .unwrap_or(0u128.into())
                .checked_add(&1u128.into())
                .ok_or(Error::<T>::FailedToCreateManagerId)?;

            T::ManagerToken::create_token(id, &who)?;
            <LastManagerId<T>>::set(Some(id));

            if let Some(pairings) = pairings {
                for pairing in pairings {
                    if Self::manager_id_for_processor(&pairing.processor).is_some() {
                        return Err(Error::<T>::ProcessorAlreadyPaired)?;
                    }
                    if !pairing.validate() {
                        return Err(Error::<T>::InvalidPairingProof)?;
                    }
                    <ManagedProcessors<T>>::insert(id, pairing.processor, ());
                }
            }

            Self::deposit_event(Event::ManagerCreated(who));
            Ok(().into())
        }
    }
}
