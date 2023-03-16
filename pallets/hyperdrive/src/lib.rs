#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
pub use types::*;

#[cfg(test)]
mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod tezos;
mod types;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use core::{fmt::Debug, str::FromStr};

    use frame_support::dispatch::PostDispatchInfo;
    use frame_support::traits::Get;
    use frame_support::{
        pallet_prelude::*,
        sp_runtime::traits::{
            AtLeast32BitUnsigned, Bounded, CheckEqual, MaybeDisplay, SimpleBitOps,
        },
    };
    use frame_system::pallet_prelude::*;
    use sp_arithmetic::traits::{CheckedRem, Zero};
    use sp_runtime::traits::Hash;
    use sp_std::collections::btree_set::BTreeSet;
    use sp_std::prelude::*;
    use sp_std::vec;

    use pallet_acurast_marketplace::types::RegistrationExtra;
    use pallet_acurast_marketplace::Reward;

    use crate::weights::WeightInfo;

    use super::*;

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

        type ParsableAccountId: IsType<<Self as frame_system::Config>::AccountId> + FromStr;
        /// The parachain ID used in [`MultiLocation`].
        type TargetChainId: Get<u32>;
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
            + From<u8>
            + CheckedRem;
        type TargetChainStateKey: Parameter + Member + Debug;
        type TargetChainStateValue: Parameter + Member + Debug;
        type Reward: Parameter + Member + Reward + TryFrom<Vec<u8>>;
        type Balance: Member
            + Parameter
            + AtLeast32BitUnsigned
            + From<u128>
            // required to translate Tezos Ints of unknown precision (Alternative: use Tezos SDK types in clients of this pallet)
            + Default
            + Copy
            + MaybeSerializeDeserialize
            + MaxEncodedLen
            + TypeInfo;
        type RegistrationExtra: Parameter
            + Member
            + From<
                RegistrationExtra<
                    Self::Reward,
                    Self::Balance,
                    Self::AccountId,
                >,
            >;

        /// The hashing system (algorithm) being used in the runtime (e.g. Blake2).
        type TargetChainHashing: Hash<Output = Self::TargetChainHash> + TypeInfo;
        /// Transmission rate in blocks; `block % transmission_rate == 0` must hold.
        type TransmissionRate: Get<Self::TargetChainBlockNumber>;
        /// The quorum size of transmitters that need to agree on a state merkle root before accepting in proofs.
        ///
        /// **NOTE**: the quorum size must be larger than `ceil(number of transmitters / 2)`, otherwise multiple root hashes could become valid in terms of [`Pallet::validate_state_merkle_root`].
        type TransmissionQuorum: Get<u8>;
        type MessageParser: MessageParser<
            Self::AccountId,
            Self::RegistrationExtra,
        >;

        type WeightInfo: WeightInfo;
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
            source: T::AccountId,
            snapshot: T::TargetChainBlockNumber,
            state_merkle_root: T::TargetChainHash,
        },
        StateMerkleRootAccepted {
            snapshot: T::TargetChainBlockNumber,
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

    #[pallet::type_value]
    pub fn FirstSnapshot<T: Config<I>, I: 'static>() -> T::TargetChainBlockNumber {
        1u8.into()
    }

    /// This storage field contains the latest validated snapshot number.
    #[pallet::storage]
    #[pallet::getter(fn latest_snapshot)]
    pub type CurrentSnapshot<T: Config<I>, I: 'static = ()> =
        StorageValue<_, T::TargetChainBlockNumber, ValueQuery, FirstSnapshot<T, I>>;

    #[pallet::storage]
    #[pallet::getter(fn state_merkle_root)]
    pub type StateMerkleRootCount<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
        _,
        Blake2_128,
        T::TargetChainBlockNumber,
        Identity,
        T::TargetChainHash,
        BTreeSet<T::AccountId>,
    >;

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// A known transmitter submits outside the window of activity he is permissioned to.
        SubmitOutsideTransmitterActivityWindow,
        CalculationOverflow,
        UnexpectedSnapshot,
        ProofInvalid,
        MessageParsingFailed,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Used to add, update or remove state transmitters.
        #[pallet::call_index(0)]
        #[pallet::weight(< T as Config<I>>::WeightInfo::update_state_transmitters())]
        pub fn update_state_transmitters(
            origin: OriginFor<T>,
            actions: StateTransmitterUpdates<T>,
        ) -> DispatchResultWithPostInfo {
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

            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Used by Acurast transmitters to submit a `state_merkle_root` at the specified `block` on the target chain.
        #[pallet::call_index(1)]
        #[pallet::weight(< T as Config<I>>::WeightInfo::submit_state_merkle_root())]
        pub fn submit_state_merkle_root(
            origin: OriginFor<T>,
            snapshot: T::TargetChainBlockNumber,
            state_merkle_root: T::TargetChainHash,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let expected_snapshot = Self::latest_snapshot();

            // Ensure merkle roots are submitted sequentially
            ensure!(
                snapshot == expected_snapshot,
                Error::<T, I>::UnexpectedSnapshot
            );

            let activity_window = <StateTransmitter<T, I>>::get(&who);
            let current_block = <frame_system::Pallet<T>>::block_number();
            // valid window is defined inclusive start_block, exclusive end_block
            ensure!(
                activity_window.start_block <= current_block
                    && current_block < activity_window.end_block,
                Error::<T, I>::SubmitOutsideTransmitterActivityWindow
            );

            // insert merkle root proposal since all checks passed
            // allows for constant-time validity checks
            let accepted = StateMerkleRootCount::<T, I>::mutate(
                &snapshot,
                &state_merkle_root,
                |submissions| {
                    // This can be improved once [let chains feature](https://github.com/rust-lang/rust/issues/53667) lands
                    if let Some(transmitters) = submissions {
                        if !transmitters.contains(&who) {
                            transmitters.insert(who.clone());
                        }
                    } else {
                        let mut set = BTreeSet::<T::AccountId>::new();
                        set.insert(who.clone());
                        *submissions = Some(set);
                    }

                    let submissions_count = submissions
                        .as_ref()
                        .map_or(0usize, |transmitters| transmitters.len());
                    return submissions_count >= T::TransmissionQuorum::get().into();
                },
            );

            // Emit event to inform that the state merkle root has been sumitted
            Self::deposit_event(Event::StateMerkleRootSubmitted {
                source: who,
                snapshot,
                state_merkle_root,
            });

            if accepted {
                CurrentSnapshot::<T, I>::set(expected_snapshot + T::TransmissionRate::get());
                Self::deposit_event(Event::StateMerkleRootAccepted {
                    snapshot,
                    state_merkle_root,
                });
            }

            Ok(())
        }

        /// Used by any transmitter to submit a `state` that is at the specified `block` on the target chain.
        #[pallet::call_index(2)]
        #[pallet::weight(< T as Config<I>>::WeightInfo::submit_message())]
        pub fn submit_message(
            origin: OriginFor<T>,
            proof: StateProofFor<
                T::TargetChainBlockNumber,
                T::TargetChainHash,
                T::TargetChainStateKey,
                T::TargetChainStateValue,
            >,
            message: Message,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;

            let message_bytes = &message.to_vec();
            let leaf_hash = T::TargetChainHashing::hash(&message_bytes);
            let derived_root = derive_proof::<T::TargetChainHashing, _>(proof.proof, leaf_hash);

            ensure!(
                Self::validate_state_merkle_root(proof.block, derived_root),
                Error::<T, I>::ProofInvalid
            );

            match T::MessageParser::parse(message_bytes).map_err(|_| Error::<T, I>::MessageParsingFailed)? {
                ParsedAction::RegisterJob(_, _) => {}
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
            StateMerkleRootCount::<T, I>::get(&block, &state_merkle_root)
                .map_or(false, |submissions| {
                    submissions.len() >= T::TransmissionQuorum::get().into()
                })
        }
    }
}
