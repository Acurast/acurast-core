#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod types;

use frame_support::{dispatch::Weight, traits::Get};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use types::*;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type StateRoot: Parameter + Member + MaxEncodedLen;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        StateSubmitted {
            snapshot: u64,
            root: T::StateRoot,
        },
        StateTransmittersUpdate {
            added: Vec<(T::AccountId, types::ActivityWindow<T::BlockNumber>)>,
            updated: Vec<(T::AccountId, types::ActivityWindow<T::BlockNumber>)>,
            removed: Vec<T::AccountId>,
        },
    }

    /// This storage field maps the state transmitters to their respective activiti window.
    ///
    /// These transmitters are responsible for submitting the merkle roots of supported
    /// source chains to acurast.
    #[pallet::storage]
    #[pallet::getter(fn state_transmitter)]
    pub type StateTransmitter<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128, T::AccountId, ActivityWindow<T::BlockNumber>, ValueQuery>;

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// This extrinsic is used to add, update or remove state transmitters.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_ref_time(10_000).saturating_add(T::DbWeight::get().reads_writes(1, 2)))]
        pub fn update_state_transmitters(
            origin: OriginFor<T>,
            actions: Vec<StateTransmitterUpdate<T::AccountId, T::BlockNumber>>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Process actions
            let (added, updated, removed) =
                actions
                    .iter()
                    .fold((vec![], vec![], vec![]), |acc, action| {
                        let (mut added, mut updated, mut removed) = acc;
                        match action {
                            StateTransmitterUpdate::Add(account, activity_window) => {
                                <StateTransmitter<T, I>>::set(
                                    account.clone(),
                                    activity_window.clone(),
                                );
                                added.push((account.clone(), activity_window.clone()))
                            }
                            StateTransmitterUpdate::Update(account, activity_window) => {
                                <StateTransmitter<T, I>>::set(
                                    account.clone(),
                                    activity_window.clone(),
                                );
                                updated.push((account.clone(), activity_window.clone()))
                            }
                            StateTransmitterUpdate::Remove(account) => {
                                <StateTransmitter<T, I>>::remove(account);
                                removed.push(account.clone())
                            }
                        }
                        (added, updated, removed)
                    });

            // Emit event to inform that the state transmitters were updated
            Self::deposit_event(Event::StateTransmittersUpdate {
                added,
                updated,
                removed,
            });

            Ok(())
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {}
