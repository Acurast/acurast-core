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
    use core::{fmt::Debug, str::FromStr};
    use frame_support::{
        pallet_prelude::*,
        sp_runtime::traits::{
            AtLeast32BitUnsigned, Bounded, CheckEqual, MaybeDisplay, SimpleBitOps,
        },
    };
    use frame_system::pallet_prelude::*;
    use sp_arithmetic::traits::{CheckedRem, Zero};
    use sp_runtime::traits::Hash;
    use types::*;

    /// A instantiable pallet for receiving secure state synchronizations into Acurast.
    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    /// Configures the pallet instance for a specific target chain from which we synchronize state into Acurast.
    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The output of the `Hashing` function used to derive hashes of target chain state.
        type TargetChainHash: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + MaybeDisplay
            + SimpleBitOps
            + Ord
            + Default
            + Copy
            + CheckEqual
            + sp_std::hash::Hash
            + AsRef<[u8]>
            + AsMut<[u8]>
            + MaxEncodedLen;
        /// The block number type used by the target runtime.
        type TargetChainBlockNumber: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + MaybeDisplay
            + AtLeast32BitUnsigned
            + Default
            + Bounded
            + Copy
            + sp_std::hash::Hash
            + FromStr
            + MaxEncodedLen
            + TypeInfo
            + Zero
            + CheckedRem;
        /// The hashing system (algorithm) being used in the runtime (e.g. Blake2).
        type TargetChainHashing: Hash<Output = Self::TargetChainHash> + TypeInfo;
        /// Transmission rate in blocks; `block % transmission_rate == 0` must hold.
        type TransmissionRate: Get<Self::TargetChainBlockNumber>;
        /// The quorum size of transmitters that need to agree on a state merkle root before accepting in proofs.
        ///
        /// **NOTE**: the quorum size must be larger than `ceil(number of transmitters / 2)`, otherwise multiple root hashes could become valid in terms of [`Pallet::validate_state_merkle_root`].
        type TransmissionQuorum: Get<u8>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        StateTransmittersUpdate {
            added: Vec<(
                T::AccountId,
                types::ActivityWindow<<T as frame_system::Config>::BlockNumber>,
            )>,
            updated: Vec<(
                T::AccountId,
                types::ActivityWindow<<T as frame_system::Config>::BlockNumber>,
            )>,
            removed: Vec<T::AccountId>,
        },
        StateMerkleRootSubmitted {
            block: T::TargetChainBlockNumber,
            state_merkle_root: T::TargetChainHash,
        },
        StateMerkleRootAccepted {
            block: T::TargetChainBlockNumber,
            state_merkle_root: T::TargetChainHash,
        },
    }

    /// This storage field maps the state transmitters to their respective activity window.
    ///
    /// These transmitters are responsible for submitting the merkle roots of supported
    /// source chains to acurast.
    #[pallet::storage]
    #[pallet::getter(fn state_transmitter)]
    pub type StateTransmitter<T: Config<I>, I: 'static = ()> = StorageMap<
        _,
        Blake2_128,
        T::AccountId,
        ActivityWindow<<T as frame_system::Config>::BlockNumber>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn state_merkle_root)]
    pub type StateMerkleRootCount<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
        _,
        Blake2_128,
        T::TargetChainBlockNumber,
        Identity,
        T::TargetChainHash,
        u8,
    >;

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// A known transmitter submits outside the window of activity he is permissioned to.
        SubmitOutsideTransmitterActivityWindow,
        SubmitOutsideTransmissionRate,
        CalculationOverflow,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Used to add, update or remove state transmitters.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_ref_time(10_000).saturating_add(T::DbWeight::get().reads_writes(1, 2)))]
        pub fn update_state_transmitters(
            origin: OriginFor<T>,
            actions: Vec<
                StateTransmitterUpdate<T::AccountId, <T as frame_system::Config>::BlockNumber>,
            >,
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

        /// Used by transmitters to submit a `state_merkle_root` at the specified `block` on the target chain.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_ref_time(10_000).saturating_add(T::DbWeight::get().reads_writes(1, 2)))]
        pub fn submit_state_merkle_root(
            origin: OriginFor<T>,
            block: T::TargetChainBlockNumber,
            state_merkle_root: T::TargetChainHash,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let activity_window = <StateTransmitter<T, I>>::get(&who);
            let current_block = <frame_system::Pallet<T>>::block_number();
            // valid window is defined inclusive start_block, exclusive end_block
            ensure!(
                activity_window.start_block <= current_block
                    && current_block < activity_window.end_block,
                Error::<T, I>::SubmitOutsideTransmitterActivityWindow
            );
            ensure!(
                block
                    .checked_rem(&T::TransmissionRate::get())
                    .ok_or(Error::<T, I>::CalculationOverflow)?
                    .is_zero(),
                Error::<T, I>::SubmitOutsideTransmissionRate
            );

            // insert merkle root proposal since all checks passed
            // allows for constant-time validity checks
            let accepted =
                StateMerkleRootCount::<T, I>::mutate(&block, &state_merkle_root, |count| {
                    let count_ = count.unwrap_or(0) + 1;
                    *count = Some(count_);
                    count_ >= <T as Config<I>>::TransmissionQuorum::get()
                });

            // Emit event to inform that the state merkle root has been sumitted
            Self::deposit_event(Event::StateMerkleRootSubmitted {
                block,
                state_merkle_root,
            });

            if accepted {
                Self::deposit_event(Event::StateMerkleRootAccepted {
                    block,
                    state_merkle_root,
                });
            }

            Ok(())
        }
    }

    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Validates a state merkle root with respect to roots submitted by a quorum of transmitters.
        pub fn validate_state_merkle_root(
            block: T::TargetChainBlockNumber,
            state_merkle_root: T::TargetChainHash,
        ) -> bool {
            StateMerkleRootCount::<T, I>::get(&block, &state_merkle_root).map_or(false, |count| {
                count >= <T as Config<I>>::TransmissionQuorum::get()
            })
        }
    }
}
