use core::fmt::Debug;
pub use mmr_lib;

use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;
use sp_runtime::traits;
#[cfg(not(feature = "std"))]
use sp_std::prelude::Vec;
use sp_std::prelude::*;

use pallet_acurast::{JobIdSequence, TezosAddressBytes};
use strum_macros::{EnumString, IntoStaticStr};

/// A type to describe node position in the MMR (node index).
pub type NodeIndex = u64;

/// A type to describe snapshot number.
pub type SnapshotNumber = u64;

/// A type to describe leaf position in the MMR.
///
/// Note this is different from [`NodeIndex`], which can be applied to
/// both leafs and inner nodes. Leafs will always have consecutive `LeafIndex`,
/// but might be actually at different positions in the MMR `NodeIndex`.
pub type LeafIndex = u64;

/// New MMR root notification hook.
pub trait OnNewRoot<Hash> {
    /// Function called by the pallet in case new MMR root has been computed.
    fn on_new_root(root: &Hash);
}

/// No-op implementation of [OnNewRoot].
impl<Hash> OnNewRoot<Hash> for () {
    fn on_new_root(_root: &Hash) {}
}

/// The encodable version of an [`Action`].
// #[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone)]
#[derive(
    RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq, EnumString, IntoStaticStr,
)]
pub enum RawAction {
    #[strum(serialize = "ASSIGN")]
    AssignJob,
}

impl From<&Action> for RawAction {
    fn from(action: &Action) -> Self {
        match action {
            Action::AssignJob(_, _) => RawAction::AssignJob,
        }
    }
}

/// The action is triggered over Hyperdrive as part of a [`Message`].
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone)]
pub enum Action {
    /// A subset of values expressed by [`pallet_acurast::JobId`], only for jobs created on Tezos.
    ///
    /// Consists of `(Job ID on Tezos, [processor addresses])`.
    AssignJob(JobIdSequence, Vec<TezosAddressBytes>), // (nat, address)
}

/// Message that is transferred to target chains.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Eq, PartialEq, Clone)]
pub struct Message {
    pub id: u64,
    pub action: Action,
}

pub type Leaf = Message;

/// An element representing either full data or its hash.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq, Eq)]
pub enum Node<Hash> {
    /// Arbitrary data in its full form.
    Data(Leaf),
    /// A hash of some data.
    Hash(Hash),
}

impl<H: traits::Hash> From<Leaf> for Node<H> {
    fn from(l: Leaf) -> Self {
        Self::Data(l)
    }
}

/// Extension trait for [`traits::Hash`] that adds hashing with previsously encoded value, using an encoding supported on target chain.
pub trait TargetChainHasher: traits::Hash {
    type TargetChainEncoder: LeafEncoder;

    /// Produce the hash of some encodable value, using an encoding supported on target chain.
    fn hash_for_target(
        leaf: &Leaf,
    ) -> Result<Self::Output, <Self::TargetChainEncoder as LeafEncoder>::Error> {
        Ok(<Self as traits::Hash>::hash(
            Self::TargetChainEncoder::encode(leaf)?.as_slice(),
        ))
    }
}

/// Hashing used for the pallet.
pub trait TargetChainNodeHasher<Hash> {
    type Error;
    fn hash_node(node: &Node<Hash>) -> Result<Hash, Self::Error>;
}

/// Implements node hashing for all nodes that contain leaves that support target chain hashing.
impl<H: TargetChainHasher> TargetChainNodeHasher<H::Output> for H {
    type Error = <H::TargetChainEncoder as LeafEncoder>::Error;
    fn hash_node(node: &Node<H::Output>) -> Result<H::Output, Self::Error> {
        match *node {
            Node::Data(ref leaf) => H::hash_for_target(leaf),
            Node::Hash(ref hash) => Ok(*hash),
        }
    }
}

/// An encoder for leaves that can be decoded on target chains.
///
/// Note that we can't use [`codec::Encode`] since we derive that trait for the SCALE-encoding used to store leaves
/// in the off-chain index.
pub trait LeafEncoder {
    type Error: Debug;
    fn encode(leaf: &Leaf) -> Result<Vec<u8>, Self::Error>;
}

/// An MMR proof for a group of leaves.
#[derive(codec::Encode, codec::Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo)]
pub struct Proof<Hash> {
    /// The indices of the leaves the proof is for.
    pub leaf_indices: Vec<LeafIndex>,
    /// Number of leaves in MMR, when the proof was generated.
    pub leaf_count: NodeIndex,
    /// Proof elements (hashes of siblings of inner nodes on the path to the leaf).
    pub items: Vec<Hash>,
}

/// A self-contained MMR proof for a group of leaves, containing messages encoded for target chain.
#[derive(codec::Encode, codec::Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo)]
pub struct TargetChainProof<Hash> {
    /// The indices of the leaves the proof is for.
    pub leaves: Vec<TargetChainProofLeaf>,
    /// Number of leaves in MMR, when the proof was generated.
    pub leaf_count: NodeIndex,
    /// Proof elements (hashes of siblings of inner nodes on the path to the leaf).
    /// Excluding MMR root.
    pub items: Vec<Hash>,
}

/// A leaf of a self-contained MMR [`TargetChainProof`].
#[derive(codec::Encode, codec::Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo)]
pub struct TargetChainProofLeaf {
    /// The k-index of this leaf.
    pub k_index: NodeIndex,
    /// The position of this leaf.
    pub position: NodeIndex,
    /// The encoded message on this leaf.
    pub message: Vec<u8>,
}

/// Merkle Mountain Range operation error.
#[cfg_attr(feature = "std", derive(thiserror::Error))]
#[derive(RuntimeDebug, codec::Encode, codec::Decode, PartialEq, Eq)]
pub enum MMRError {
    /// Error while pushing new node.
    #[cfg_attr(feature = "std", error("Error pushing new node"))]
    Push,
    /// Error getting the new root.
    #[cfg_attr(feature = "std", error("Error getting new root"))]
    GetRoot,
    /// Error committing changes.
    #[cfg_attr(feature = "std", error("Error committing changes"))]
    Commit,
    /// Error during proof generation.
    #[cfg_attr(feature = "std", error("Error generating proof"))]
    GenerateProof,
    /// Error during proof generation when no snapshot was taken yet.
    #[cfg_attr(
        feature = "std",
        error("Error generating proof: no snapshot taken yet")
    )]
    GenerateProofNoSnapshot,
    /// Error during proof generation when requested snapshot lies in the future.
    #[cfg_attr(
        feature = "std",
        error("Error generating proof: snapshot in the future")
    )]
    GenerateProofFutureSnapshot,
    /// Error during proof generation when requested message start lies in the future.
    #[cfg_attr(
        feature = "std",
        error("Error generating proof: message in the future")
    )]
    GenerateProofFutureMessage,
    /// Proof verification error.
    #[cfg_attr(feature = "std", error("Invalid proof"))]
    Verify,
    /// Leaf not found in the storage.
    #[cfg_attr(feature = "std", error("Leaf was not found"))]
    LeafNotFound,
}

impl MMRError {
    #![allow(unused_variables)]
    /// Consume given error `e` with `self` and generate a native log entry with error details.
    pub fn log_error(self, e: impl Debug) -> Self {
        log::error!(
            target: "runtime::mmr",
            "[{:?}] MMR error: {:?}",
            self,
            e,
        );
        self
    }

    /// Consume given error `e` with `self` and generate a native log entry with error details.
    pub fn log_debug(self, e: impl Debug) -> Self {
        log::debug!(
            target: "runtime::mmr",
            "[{:?}] MMR error: {:?}",
            self,
            e,
        );
        self
    }
}

sp_api::decl_runtime_apis! {
    /// API to interact with MMR pallet.
    pub trait HyperdriveApi<Hash: codec::Codec, BlockNumber: codec::Codec> {
        /// Return the on-chain MMR root hash.
        fn snapshot_mmr_root() -> Result<Hash, MMRError>;

        /// Return the number of MMR blocks in the chain.
        fn mmr_leaf_count() -> Result<LeafIndex, MMRError>;

        /// Generates a MMR proof for the messages in the range `[next_message_number..last_message_excl]`.
        fn generate_proof(
            next_message_number: LeafIndex,
            maximum_messages: Option<u64>,
            latest_known_snapshot_number: SnapshotNumber,
        ) -> Result<Option<(Vec<Leaf>, Proof<Hash>)>, MMRError>;

        /// Generates a self-contained MMR proof for the messages in the range `[next_message_number..last_message_excl]`.
        /// Leaves with their leaf index and position are part of the proof structure and contain the message encoded for the target chain.
        ///
        /// This function wraps [`Self::generate_proof`] and converts result to [`TargetChainProof`].
        fn generate_target_chain_proof(
            next_message_number: LeafIndex,
            maximum_messages: Option<u64>,
            latest_known_snapshot_number: SnapshotNumber,
        ) -> Result<Option<TargetChainProof<Hash>>, MMRError>;

        /// Verify MMR proof against on-chain MMR for a batch of leaves.
        ///
        /// Note this function will use on-chain MMR root hash and check if the proof matches the hash.
        /// Note, the leaves should be sorted such that corresponding leaves and leaf indices have the
        /// same position in both the `leaves` vector and the `leaf_indices` vector contained in the [Proof]
        fn verify_proof(leaves: Vec<Leaf>, proof: Proof<Hash>) -> Result<(), MMRError>;

        /// Verify MMR proof against given root hash for a batch of leaves.
        ///
        /// Note this function does not require any on-chain storage - the
        /// proof is verified against given MMR root hash.
        ///
        /// Note, the leaves should be sorted such that corresponding leaves and leaf indices have the
        /// same position in both the `leaves` vector and the `leaf_indices` vector contained in the [Proof]
        fn verify_proof_stateless(root: Hash, leaves: Vec<Leaf>, proof: Proof<Hash>)
            -> Result<(), MMRError>;
    }
}
