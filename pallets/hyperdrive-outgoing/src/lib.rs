#![cfg_attr(not(feature = "std"), no_std)]

use core::cmp::min;
use core::ops::AddAssign;

use frame_support::dispatch::{Pays, PostDispatchInfo};
use frame_support::ensure;
use mmr_lib::leaf_index_to_pos;
use sp_core::Get;
use sp_runtime::traits::Saturating;
use sp_std::prelude::*;

pub use pallet::*;
pub use types::{
    Action, Leaf, LeafEncoder, LeafIndex, MMRError, Message, NodeIndex, OnNewRoot, RawAction,
    TargetChainConfig,
};
pub use utils::NodesUtils;

pub use crate::default_weights::WeightInfo;
use crate::mmr::Merger;
use crate::types::{Node, Proof, SnapshotNumber, TargetChainProof, TargetChainProofLeaf};

#[cfg(test)]
pub mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod default_weights;
mod mmr;
pub mod tezos;
mod types;
pub mod utils;

/// A MMR specific to this pallet instance.
type ModuleMmr<StorageType, T, I> = mmr::Mmr<StorageType, T, I, Merger<TargetChainConfigOf<T, I>>>;

/// Hashing for target chain used for this pallet instance.
pub(crate) type TargetChainConfigOf<T, I> = <T as Config<I>>::TargetChainConfig;

/// Hash used for this pallet instance.
pub(crate) type HashOf<T, I> = <<T as Config<I>>::TargetChainConfig as TargetChainConfig>::Hash;

/// Encoder used for this pallet instance.
pub(crate) type TargetChainEncoderOf<T, I> =
    <<T as Config<I>>::TargetChainConfig as TargetChainConfig>::TargetChainEncoder;

/// Encoder error returned by Hasher/Encoder used for this pallet instance.
pub(crate) type HasherError<T, I> = <TargetChainEncoderOf<T, I> as LeafEncoder>::Error;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    use crate::default_weights::WeightInfo;

    use super::*;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    /// This pallet's configuration trait
    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Prefix for elements stored in the Off-chain DB via Indexing API.
        ///
        /// Each node of the MMR is inserted both on-chain and off-chain via Indexing API.
        /// The former does not store full leaf content, just its compact version (hash),
        /// and some of the inner mmr nodes might be pruned from on-chain storage.
        /// The latter will contain all the entries in their full form.
        ///
        /// Each node is stored in the Off-chain DB under key derived from the
        /// [`Self::INDEXING_PREFIX`] and its in-tree index (MMR position).
        const INDEXING_PREFIX: &'static [u8];

        /// The bundled config of encoder/hasher using an encoding/hash function supported on target chain.
        type TargetChainConfig: TargetChainConfig;

        /// The usual number of blocks included before a new snapshot of the current MMR's [`RootHash`] is stored into [`SnapshotRootHash`].
        ///
        /// A snapshot can be delayed by more then the configured value if no messages get sent, but when a message is sent in block `b`,
        /// latest in block `b + MaximumBlocksBeforeSnapshot` a new snapshot will be taken.
        type MaximumBlocksBeforeSnapshot: Get<BlockNumberFor<Self>>;

        /// A hook to act on the new MMR root.
        ///
        /// For some applications it might be beneficial to make the MMR root available externally
        /// apart from having it in the storage. For instance you might output it in the header
        /// digest (see [`frame_system::Pallet::deposit_log`]) to make it available for Light
        /// Clients. Hook complexity should be `O(1)`.
        type OnNewRoot: OnNewRoot<HashOf<Self, I>>;

        /// Weights for this pallet.
        type WeightInfo: WeightInfo;
    }

    /// A tuple `(included_message_number_excl, next_message_number)`
    /// (where `next_message_number - 1` is not necessarily included in a snapshot).
    ///
    /// The [`next_message_number`] is strictly increasing, sequential order of messages sent.
    /// `next_message_number` is the ID for the next message sent.
    ///
    /// The relationship between blocks, messages=leaves and snapshots is sketched below:
    /// ```text
    /// |------block 3-----|  |---------block 4-------------|  |-------block 5---- - - -
    ///  m11       m12          m13   m14   m15   m16    m17     m18 m19
    /// -------------------------------------------snapshot-|  |------------------ - - -
    ///                                                          ↑   ↑
    ///                                    included_message_number   next_message_number-1
    /// ```
    #[pallet::storage]
    #[pallet::getter(fn message_numbers)]
    pub type MessageNumbers<T: Config<I>, I: 'static = ()> =
        StorageValue<_, (LeafIndex, LeafIndex), ValueQuery>;

    /// An index `leaf_index -> parent_block_hash`.
    ///
    /// Useful to recover the block hash of the parent that added a certain leaf.
    /// This block hash is used in temporary keys for offchain-indexing full leaves.
    #[pallet::storage]
    #[pallet::getter(fn leaf_index_to_parent_block_hash)]
    pub type LeafIndexToParentBlockHash<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Identity, LeafIndex, <T as frame_system::Config>::Hash, OptionQuery>;

    /// Next snapshot number. The latest completed snapshot is the stored value - 1.
    #[pallet::storage]
    #[pallet::getter(fn next_snapshot_number)]
    pub type NextSnapshotNumber<T: Config<I>, I: 'static = ()> =
        StorageValue<_, SnapshotNumber, ValueQuery>;

    /// Latest snapshot's MMR root hash.
    #[pallet::storage]
    #[pallet::getter(fn snapshot_root_hash)]
    pub type SnapshotRootHash<T: Config<I>, I: 'static = ()> =
        StorageValue<_, HashOf<T, I>, ValueQuery>;

    /// Meta data for a snapshot as a map `snapshot_number -> (last_block, last_message_excl)`.
    ///
    /// Used to ensure a maximum number of blocks per snapshot, even if no messages get sent.
    #[pallet::storage]
    #[pallet::getter(fn snapshot_meta)]
    pub type SnapshotMeta<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Identity, SnapshotNumber, (BlockNumberFor<T>, LeafIndex), OptionQuery>;

    /// Latest MMR root hash.
    #[pallet::storage]
    #[pallet::getter(fn root_hash)]
    pub type RootHash<T: Config<I>, I: 'static = ()> = StorageValue<_, HashOf<T, I>, ValueQuery>;

    /// Current size of the MMR (number of leaves).
    #[pallet::storage]
    #[pallet::getter(fn mmr_leaves)]
    pub type NumberOfLeaves<T, I = ()> = StorageValue<_, LeafIndex, ValueQuery>;

    /// Hashes of the nodes in the MMR.
    ///
    /// Note this collection only contains MMR peaks, the inner nodes (and leaves)
    /// are pruned and only stored in the Offchain DB.
    #[pallet::storage]
    #[pallet::getter(fn mmr_peak)]
    pub type Nodes<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Identity, NodeIndex, HashOf<T, I>, OptionQuery>;

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
        fn on_finalize(current_block: BlockNumberFor<T>) {
            let (included_message_number_excl, next_message_number) = Self::message_numbers();
            // check if we should create new snapshot
            if included_message_number_excl < next_message_number
                && Self::maximum_blocks_before_snapshot_reached(current_block)
            {
                // there was at least one message since last snapshot and enough blocks passed -> take snapshot
                let current_snapshot = <NextSnapshotNumber<T, I>>::mutate(|s| {
                    let current_snapshot = *s;
                    s.add_assign(1);
                    current_snapshot
                });
                SnapshotRootHash::<T, I>::put(RootHash::<T, I>::get());
                SnapshotMeta::<T, I>::insert(
                    current_snapshot,
                    (current_block, next_message_number),
                );
                MessageNumbers::<T, I>::put((next_message_number, next_message_number));
            }
        }

        fn on_initialize(current_block: T::BlockNumber) -> Weight {
            // add weight used here in on_finalize

            // We did the check already and will repeat it in on_finalize
            let weight = T::WeightInfo::check_snapshot().saturating_mul(2);

            // predict if we definitely will not create snapshot in the initialized block and estimate lower weight
            if !Self::maximum_blocks_before_snapshot_reached(current_block) {
                return weight;
            }

            // we can't avoid that sometimes there is no message at the end of MaximumBlocksBeforeSnapshot
            // and we unnecessarily reserve weight for snapshotting
            weight.saturating_add(T::WeightInfo::create_snapshot())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// A message was successfully sent. [JobId, SourceId, Assignment]
        MessageSent(Message),
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    /// Sends a message with the given [`Action`] over Hyperdrive.
    pub fn send_message(action: Action) -> Result<PostDispatchInfo, MMRError> {
        let leaves = Self::mmr_leaves();
        // used to calculate actual weight, see below
        let peaks_before = NodesUtils::new(leaves).number_of_peaks();

        let (included_message_number_excl, next_message_number) = Self::message_numbers();
        let message = Message {
            id: next_message_number,
            action,
        };

        // append new leaf to MMR
        let mut mmr: ModuleMmr<mmr::storage::RuntimeStorage, T, I> = mmr::Mmr::new(leaves);
        // MMR push never fails, but better safe than sorry.
        mmr.push(message.clone()).ok_or(MMRError::Push)?;
        // Update the size, `mmr.finalize()` should also never fail.
        let (leaves, root) = mmr.finalize()?;
        <T::OnNewRoot as OnNewRoot<_>>::on_new_root(&root);

        <NumberOfLeaves<T, I>>::put(leaves);
        <RootHash<T, I>>::put(root);
        MessageNumbers::<T, I>::put((included_message_number_excl, next_message_number + 1));

        Self::deposit_event(Event::MessageSent(message));

        // use peaks_after - peaks_before difference to calculate actual weight
        let peaks_after = NodesUtils::new(leaves).number_of_peaks();
        Ok(PostDispatchInfo {
            actual_weight: Some(T::WeightInfo::send_message_actual_weight(
                peaks_before.max(peaks_after),
            )),
            pays_fee: Pays::Yes,
        })
    }

    /// Build offchain key from `parent_hash` of block that originally added node `pos` to MMR.
    ///
    /// This combination makes the offchain (key, value) entry resilient to chain forks.
    fn node_temp_offchain_key(
        pos: NodeIndex,
        parent_hash: <T as frame_system::Config>::Hash,
    ) -> Vec<u8> {
        NodesUtils::node_temp_offchain_key::<<T as frame_system::Config>::Header>(
            &T::INDEXING_PREFIX,
            pos,
            parent_hash,
        )
    }

    /// Build canonical offchain key for node `pos` in MMR.
    ///
    /// Used for nodes added by now finalized blocks.
    /// Never read keys using `node_canon_offchain_key` unless you sure that
    /// there's no `node_offchain_key` key in the storage.
    fn node_canon_offchain_key(pos: NodeIndex) -> Vec<u8> {
        NodesUtils::node_canon_offchain_key(&T::INDEXING_PREFIX, pos)
    }

    /// Check if we should create new snapshot at the end of `current_block`,
    /// according to [`T::MaximumBlocksBeforeSnapshot`].
    ///
    /// This function should be combined with a check (not included!) if there was at least one new message to snapshot.
    fn maximum_blocks_before_snapshot_reached(current_block: T::BlockNumber) -> bool {
        // check if we should create new snapshot
        let (last_block, _last_message_excl): (T::BlockNumber, LeafIndex) =
            Self::snapshot_meta(Self::next_snapshot_number().saturating_sub(1))
                .unwrap_or((0u32.into(), 0));

        current_block.saturating_sub(last_block) >= T::MaximumBlocksBeforeSnapshot::get().into()
    }

    /// Generates a MMR proof for the messages in the range `[next_message_number..last_message_excl]`.
    ///
    /// If `next_message_number` is not yet sent, an error is returned.
    /// `last_message_excl` is the exclusive upper bound of messages to transmit and is bounded by latest message's index.
    /// If `maximum_messages` is provided, `next_message_number + maximum_messages` it the potentially lower bound used to
    /// limit the number of messages transfered at once.
    ///
    /// The proof is generated for the root at the end of the block that also produced the snapshot with `latest_known_snapshot_number`.
    ///
    /// If no new messages exist that have to be transmitted or they are not included in snapshot with `latest_known_snapshot_number`,
    /// this function returns `Ok(None)`.
    ///
    /// Note this function can only be used from an off-chain context
    /// (Offchain Worker or Runtime API call), since it requires
    /// all the leaves to be present.
    /// It may return an error or panic if used incorrectly.
    pub fn generate_proof(
        next_message_number: LeafIndex,
        maximum_messages: Option<u64>,
        latest_known_snapshot_number: SnapshotNumber,
    ) -> Result<Option<(Vec<Leaf>, Proof<HashOf<T, I>>)>, MMRError> {
        let (_last_block, last_message_excl) = Self::snapshot_meta(latest_known_snapshot_number)
            .ok_or(MMRError::GenerateProofFutureSnapshot)?;

        ensure!(
            next_message_number <= last_message_excl,
            MMRError::GenerateProofFutureMessage
        );

        let last_message_excl = if let Some(maximum) = maximum_messages {
            min(last_message_excl, next_message_number + maximum)
        } else {
            last_message_excl
        };

        if next_message_number == last_message_excl {
            // no new messages to transmit
            return Ok(None);
        }

        // since we create one leaf per message, the number of leaves at the end of the block where latest_known_snapshot_number
        // was taken is equal to the messages included at that time which is equal to last_message_excl
        let leaves_count = last_message_excl;
        // retrieve proof for the leaf index range [next_message_number..last_message_excl]
        let mmr: ModuleMmr<mmr::storage::OffchainStorage, T, I> = mmr::Mmr::new(leaves_count);
        mmr.generate_proof((next_message_number..last_message_excl).collect())
            .map(|result| Some(result))
    }

    /// Generates a self-contained MMR proof for the messages in the range `[next_message_number..last_message_excl]`.
    /// Leaves with their leaf index and position are part of the proof structure and contain the message encoded for the target chain.
    ///
    /// This function wraps [`Self::generate_proof`] and converts result to [`TargetChainProof`].
    pub fn generate_target_chain_proof(
        next_message_number: LeafIndex,
        maximum_messages: Option<u64>,
        latest_known_snapshot_number: SnapshotNumber,
    ) -> Result<Option<TargetChainProof<HashOf<T, I>>>, MMRError> {
        let proof = Self::generate_proof(
            next_message_number,
            maximum_messages,
            latest_known_snapshot_number,
        )?;
        proof
            .map(|(leaves, proof)| {
                let mmr_size = NodesUtils::new(Self::mmr_leaves()).size();
                let leaf_positions: Vec<NodeIndex> = proof
                    .leaf_indices
                    .iter()
                    .map(|leaf_index| leaf_index_to_pos(leaf_index.to_owned()))
                    .collect();
                let leaf_k_indices = mmr::node_pos_to_k_index(leaf_positions.clone(), mmr_size);
                let leaves = leaf_positions
                    .iter()
                    .zip(leaf_k_indices.iter())
                    .zip(leaves.iter())
                    .map(|((position, (pos, k_index)), leaf)| {
                        assert_eq!(pos, position);
                        Ok(TargetChainProofLeaf {
                            k_index: k_index.to_owned() as NodeIndex,
                            position: position.to_owned(),
                            message: TargetChainEncoderOf::<T, I>::encode(leaf)
                                .map_err(|_| MMRError::GenerateProof)?,
                        })
                    })
                    .collect::<Result<Vec<TargetChainProofLeaf>, MMRError>>()?;
                Ok(TargetChainProof {
                    leaves,
                    mmr_size,
                    items: proof.items,
                })
            })
            .transpose()
    }

    /// Return the on-chain MMR root hash.
    pub fn mmr_root() -> HashOf<T, I> {
        Self::root_hash()
    }

    /// Verify MMR proof for given `leaves`.
    ///
    /// This method is safe to use within the runtime code.
    /// It will return `Ok(())` if the proof is valid
    /// and an `Err(..)` if MMR is inconsistent (some leaves are missing)
    /// or the proof is invalid.
    pub fn verify_proof(leaves: Vec<Leaf>, proof: Proof<HashOf<T, I>>) -> Result<(), MMRError> {
        if proof.leaf_count > Self::mmr_leaves()
            || proof.leaf_count == 0
            || (proof.items.len().saturating_add(leaves.len())) as u64 > proof.leaf_count
        {
            return Err(MMRError::Verify
                .log_debug("The proof has incorrect number of leaves or proof items."));
        }

        let mmr: ModuleMmr<mmr::storage::OffchainStorage, T, I> = mmr::Mmr::new(proof.leaf_count);
        let is_valid = mmr.verify_leaves_proof(leaves, proof)?;
        if is_valid {
            Ok(())
        } else {
            Err(MMRError::Verify.log_debug("The proof is incorrect."))
        }
    }

    /// Stateless MMR proof verification for batch of leaves.
    ///
    /// This function can be used to verify received MMR [`Proof`] (`proof`)
    /// for given leaves set (`leaves`) against a known MMR root hash (`root`).
    /// Note, the leaves should be sorted such that corresponding leaves and leaf indices have the
    /// same position in both the `leaves` vector and the `leaf_indices` vector contained in the
    /// [`Proof`].
    pub fn verify_proof_stateless(
        root: HashOf<T, I>,
        leaves: Vec<Leaf>,
        proof: Proof<HashOf<T, I>>,
    ) -> Result<(), MMRError> {
        let is_valid = mmr::verify_leaves_proof::<T, I, Merger<TargetChainConfigOf<T, I>>>(
            root,
            leaves.iter().map(|leaf| Node::Data(leaf.clone())).collect(),
            proof,
        )?;
        if is_valid {
            Ok(())
        } else {
            Err(MMRError::Verify.log_debug(("The proof is incorrect.", root)))
        }
    }
}
