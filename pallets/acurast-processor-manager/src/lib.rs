#![cfg_attr(not(feature = "std"), no_std)]

mod functions;
mod traits;
mod types;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use functions::*;
pub use pallet::*;
pub use traits::*;
pub use types::*;

pub type ProcessorPairingFor<T> =
    ProcessorPairing<<T as frame_system::Config>::AccountId, <T as Config>::Proof>;
pub type ProcessorPairingUpdateFor<T> =
    ProcessorPairingUpdate<<T as frame_system::Config>::AccountId, <T as Config>::Proof>;

#[frame_support::pallet]
pub mod pallet {
    use acurast_common::ListUpdateOperation;
    use frame_support::{
        dispatch::DispatchResultWithPostInfo,
        pallet_prelude::*,
        sp_runtime::traits::{CheckedAdd, IdentifyAccount, StaticLookup, Verify},
        traits::{Currency, Get},
        Blake2_128,
    };
    use frame_system::{ensure_signed, pallet_prelude::OriginFor};
    use sp_std::prelude::*;

    use crate::{traits::*, ProcessorPairingUpdateFor};

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Proof: Parameter + Member + Verify + MaxEncodedLen;
        type ManagerId: Member + Parameter + MaxEncodedLen + Copy + CheckedAdd + From<u128>;
        type ManagerIdProvider: ManagerIdProvider<Self>;
        type Currency: Currency<Self::AccountId>;
        type ProcessorAssetRecovery: ProcessorAssetRecovery<Self>;
        type MaxPairingUpdates: Get<u32>;
        /// Weight Info for extrinsics.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn last_manager_id)]
    pub(super) type LastManagerId<T: Config> = StorageValue<_, T::ManagerId>;

    #[pallet::storage]
    #[pallet::getter(fn managed_processors)]
    pub(super) type ManagedProcessors<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::ManagerId, Blake2_128Concat, T::AccountId, ()>;

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
        ManagerCreated(T::AccountId, T::ManagerId),
        ProcessorPairingsUpdated(T::AccountId, Vec<ProcessorPairingUpdateFor<T>>),
        ProcessorFundsRecovered(T::AccountId, T::AccountId),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        FailedToCreateManagerId,
        ProcessorAlreadyPaired,
        ProcessorPairedWithAnotherManager,
        InvalidPairingProof,
        ProcessorHasNoManager,
        TooManyPairingUpdates,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        T::AccountId: IsType<<<T::Proof as Verify>::Signer as IdentifyAccount>::AccountId>,
    {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::update_processor_pairings())]
        pub fn update_processor_pairings(
            origin: OriginFor<T>,
            pairing_updates: Vec<ProcessorPairingUpdateFor<T>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            if pairing_updates.len() > T::MaxPairingUpdates::get() as usize {
                return Err(Error::<T>::TooManyPairingUpdates)?;
            }

            let (manager_id, created) = Self::do_get_or_create_manager_id(&who)?;
            if created {
                Self::deposit_event(Event::<T>::ManagerCreated(who.clone(), manager_id));
            }

            for update in &pairing_updates {
                match update.operation {
                    ListUpdateOperation::Add => {
                        if !update.item.validate() {
                            return Err(Error::<T>::InvalidPairingProof)?;
                        }
                        Self::do_add_processor_manager_pairing(&update.item.processor, manager_id)?
                    }
                    ListUpdateOperation::Remove => Self::do_remove_processor_manager_pairing(
                        &update.item.processor,
                        manager_id,
                    )?,
                }
            }

            Self::deposit_event(Event::<T>::ProcessorPairingsUpdated(who, pairing_updates));

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::recover_funds())]
        pub fn recover_funds(
            origin: OriginFor<T>,
            processor: <T::Lookup as StaticLookup>::Source,
            destination: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let manager_id = T::ManagerIdProvider::manager_id_for(&who)?;
            let processor_account_id = <T::Lookup as StaticLookup>::lookup(processor)?;
            let destination_account_id = <T::Lookup as StaticLookup>::lookup(destination)?;
            let processor_manager_id = Self::manager_id_for_processor(&processor_account_id)
                .ok_or(Error::<T>::ProcessorHasNoManager)?;

            if manager_id != processor_manager_id {
                return Err(Error::<T>::ProcessorPairedWithAnotherManager)?;
            }

            T::ProcessorAssetRecovery::recover_assets(
                &processor_account_id,
                &destination_account_id,
            )?;

            Self::deposit_event(Event::<T>::ProcessorFundsRecovered(
                processor_account_id,
                destination_account_id,
            ));

            Ok(().into())
        }
    }
}
