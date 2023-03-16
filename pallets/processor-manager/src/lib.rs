#![cfg_attr(not(feature = "std"), no_std)]

mod functions;
mod traits;
mod types;

#[cfg(test)]
pub mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
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

pub type ProcessorUpdatesFor<T> =
    frame_support::BoundedVec<ProcessorPairingUpdateFor<T>, <T as Config>::MaxPairingUpdates>;

#[frame_support::pallet]
pub mod pallet {
    use acurast_common::ListUpdateOperation;
    use codec::MaxEncodedLen;
    use frame_support::{
        dispatch::DispatchResultWithPostInfo,
        pallet_prelude::{Member, *},
        sp_runtime::traits::{CheckedAdd, IdentifyAccount, StaticLookup, Verify},
        traits::{Get, UnixTime},
        Blake2_128,
    };
    use frame_system::{ensure_signed, pallet_prelude::OriginFor};
    use sp_std::prelude::*;

    use crate::{traits::*, ProcessorPairingFor, ProcessorUpdatesFor};

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Proof: Parameter + Member + Verify + MaxEncodedLen;
        type ManagerId: Parameter + Member + MaxEncodedLen + Copy + CheckedAdd + From<u128>;
        type ManagerIdProvider: ManagerIdProvider<Self>;
        type ProcessorAssetRecovery: ProcessorAssetRecovery<Self>;
        type MaxPairingUpdates: Get<u32>;
        type Counter: Parameter + Member + MaxEncodedLen + Copy + CheckedAdd + Ord + From<u8>;
        type PairingProofExpirationTime: Get<u128>;
        /// Timestamp
        type UnixTime: UnixTime;
        /// Weight Info for extrinsics.
        type WeightInfo: WeightInfo;
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub managers: Vec<(T::AccountId, Vec<T::AccountId>)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                managers: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for (manager, processors) in self.managers.clone() {
                let manager_id =
                    T::ManagerIdProvider::manager_id_for(&manager).unwrap_or_else(|_| {
                        // Get the latest manager identifier in the sequence.
                        let id = <LastManagerId<T>>::get().unwrap_or(0.into()) + 1.into();

                        // Using .expect here should be fine it is only applied at the genesis block.
                        T::ManagerIdProvider::create_manager_id(id, &manager)
                            .expect("Could not create manager id.");

                        // Update sequencial manager identifier
                        <LastManagerId<T>>::set(Some(id));

                        id
                    });

                processors.iter().for_each(|processor| {
                    // Set manager/processor indexes
                    <ManagedProcessors<T>>::insert(manager_id, &processor, ());
                    <ProcessorToManagerIdIndex<T>>::insert(&processor, manager_id);

                    // Update the processor counter for the manager
                    let counter =
                        <ManagerCounter<T>>::get(&manager).unwrap_or(0u8.into()) + 1.into();
                    <ManagerCounter<T>>::insert(&manager, counter);
                });
            }
        }
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

    #[pallet::storage]
    #[pallet::getter(fn counter_for_manager)]
    pub(super) type ManagerCounter<T: Config> = StorageMap<_, Blake2_128, T::AccountId, T::Counter>;

    #[pallet::storage]
    #[pallet::getter(fn processor_last_seen)]
    pub(super) type ProcessorHeartbeat<T: Config> = StorageMap<_, Blake2_128, T::AccountId, u128>;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ManagerCreated(T::AccountId, T::ManagerId),
        ProcessorPairingsUpdated(T::AccountId, ProcessorUpdatesFor<T>),
        ProcessorFundsRecovered(T::AccountId, T::AccountId),
        ProcessorPaired(T::AccountId, ProcessorPairingFor<T>),
        ProcessorHeartbeat(T::AccountId),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        FailedToCreateManagerId,
        ProcessorAlreadyPaired,
        ProcessorPairedWithAnotherManager,
        InvalidPairingProof,
        ProcessorHasNoManager,
        CounterOverflow,
        PairingProofExpired,
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
            pairing_updates: ProcessorUpdatesFor<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let (manager_id, created) = Self::do_get_or_create_manager_id(&who)?;
            if created {
                Self::deposit_event(Event::<T>::ManagerCreated(who.clone(), manager_id));
            }

            for update in &pairing_updates {
                match update.operation {
                    ListUpdateOperation::Add => {
                        if !update.item.validate_timestamp::<T>() {
                            return Err(Error::<T>::PairingProofExpired)?;
                        }
                        let counter = Self::counter_for_manager(&who)
                            .unwrap_or(0u8.into())
                            .checked_add(&1u8.into())
                            .ok_or(Error::<T>::CounterOverflow)?;
                        if !update.item.validate_signature::<T>(&who, counter) {
                            return Err(Error::<T>::InvalidPairingProof)?;
                        }
                        Self::do_add_processor_manager_pairing(&update.item.account, manager_id)?;
                        <ManagerCounter<T>>::insert(&who, counter);
                    }
                    ListUpdateOperation::Remove => {
                        Self::do_remove_processor_manager_pairing(&update.item.account, manager_id)?
                    }
                }
            }

            Self::deposit_event(Event::<T>::ProcessorPairingsUpdated(who, pairing_updates));

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::pair_with_manager())]
        pub fn pair_with_manager(
            origin: OriginFor<T>,
            pairing: ProcessorPairingFor<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            if !pairing.validate_timestamp::<T>() {
                return Err(Error::<T>::PairingProofExpired)?;
            }

            let (manager_id, created) = Self::do_get_or_create_manager_id(&pairing.account)?;
            if created {
                Self::deposit_event(Event::<T>::ManagerCreated(
                    pairing.account.clone(),
                    manager_id,
                ));
            }

            let counter = Self::counter_for_manager(&pairing.account)
                .unwrap_or(0u8.into())
                .checked_add(&1u8.into())
                .ok_or(Error::<T>::CounterOverflow)?;

            if !pairing.validate_signature::<T>(&pairing.account, counter) {
                return Err(Error::<T>::InvalidPairingProof)?;
            }
            Self::do_add_processor_manager_pairing(&who, manager_id)?;
            <ManagerCounter<T>>::insert(&pairing.account, counter);

            Self::deposit_event(Event::<T>::ProcessorPaired(who, pairing));

            Ok(().into())
        }

        #[pallet::call_index(2)]
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

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::heartbeat())]
        pub fn heartbeat(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let _ =
                Self::manager_id_for_processor(&who).ok_or(Error::<T>::ProcessorHasNoManager)?;

            <ProcessorHeartbeat<T>>::insert(&who, T::UnixTime::now().as_millis());

            Self::deposit_event(Event::<T>::ProcessorHeartbeat(who));

            Ok(().into())
        }
    }
}
