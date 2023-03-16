use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
use frame_support::{pallet_prelude::*, storage::bounded_vec::BoundedVec};
use scale_info::TypeInfo;
use sp_core::ConstU32;
use sp_runtime::traits::{Hash, MaybeDisplay};
use sp_std::str::FromStr;
use sp_std::prelude::*;
use strum_macros::EnumString;
use pallet_acurast::{JobId, JobRegistration};

use crate::{Config, Error};

pub const STATE_TRANSMITTER_UPDATES_MAX_LENGTH: u32 = 50;
pub type StateTransmitterUpdates<T> =
    BoundedVec<StateTransmitterUpdateFor<T>, ConstU32<STATE_TRANSMITTER_UPDATES_MAX_LENGTH>>;

pub type StateTransmitterUpdateFor<T> = StateTransmitterUpdate<
    <T as frame_system::Config>::AccountId,
    <T as frame_system::Config>::BlockNumber,
>;

/// Defines the transmitter activity window.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub struct ActivityWindow<BlockNumber> {
    /// From this block on, the transmitter is permitted to submit Merkle roots.
    pub start_block: BlockNumber,
    /// From this block on, the transmitter is not permitted to submit any Merkle root.
    pub end_block: BlockNumber,
}
impl<BlockNumber: From<u8>> Default for ActivityWindow<BlockNumber> {
    fn default() -> Self {
        Self {
            start_block: BlockNumber::from(0),
            end_block: BlockNumber::from(0),
        }
    }
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub enum StateTransmitterUpdate<AccountId, BlockNumber> {
    Add(AccountId, ActivityWindow<BlockNumber>),
    Remove(AccountId),
    Update(AccountId, ActivityWindow<BlockNumber>),
}

/// Defines the state proof.
///
/// The structure contains all necessary data to verify the proof and the leaf itself.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct StateProof<BlockNumber, Hash, Leaf> {
    /// The block number at which the state proof was generated.
    pub block: BlockNumber,
    /// Proof's path blinded nodes. Does not contain the leaf hash, nor the root.
    ///
    /// This vec contains all inner node hashes necessary to reconstruct the root hash given the
    /// leaf hash.
    pub proof: BoundedVec<StateProofNode<Hash>, ConstU32<256>>,
    /// Leaf content.
    pub leaf: Leaf,
}

pub type StateProofFor<BlockNumber, Hash, Key, Value> =
    StateProof<BlockNumber, Hash, StateLeaf<Key, Value>>;

/// A leaf node of state tree consisting of a (key, value) pair.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct StateLeaf<Key, Value> {
    /// The key used to store the state by key after proof is valid.
    pub key: Key,
    /// The value.
    ///
    /// Could be any target chain state or something understood like an encoded [`RawAction`].
    pub value: Value,
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub enum StateProofNode<Hash> {
    Left(Hash),
    Right(Hash),
}

/// Verify Merkle Proof correctness versus given root and leaf hash.
///
/// The proof is NOT expected to contain leaf hash as the first
/// element, but only all adjacent (blinded) nodes required to eventually by process of
/// concatenating and hashing end up with given root hash.
///
/// The proof must not contain the root hash.
pub fn verify_proof<'a, H, P>(root_hash: &'a H::Output, proof: P, leaf_hash: H::Output) -> bool
where
    H: Hash,
    H::Output: PartialEq + AsRef<[u8]>,
    P: IntoIterator<Item = StateProofNode<H::Output>>,
{
    let derived = derive_proof::<H, P>(proof, leaf_hash);
    root_hash == &derived
}

pub(crate) fn derive_proof<'a, H, P>(proof: P, leaf_hash: H::Output) -> <H as Hash>::Output
where
    H: Hash,
    H::Output: PartialEq + AsRef<[u8]>,
    P: IntoIterator<Item = StateProofNode<H::Output>>,
{
    let hash_len = <H as sp_core::Hasher>::LENGTH;
    let mut combined = vec![0_u8; hash_len * 2];
    let computed = proof.into_iter().fold(leaf_hash, |a, b| {
        match b {
            StateProofNode::Right(h) => {
                combined[..hash_len].copy_from_slice(&a.as_ref());
                combined[hash_len..].copy_from_slice(&h.as_ref());
            }
            StateProofNode::Left(h) => {
                combined[..hash_len].copy_from_slice(&h.as_ref());
                combined[hash_len..].copy_from_slice(&a.as_ref());
            }
        }
        let hash = <H as Hash>::hash(&combined);
        #[cfg(feature = "debug_assertions")]
        log::debug!(
            "[verify_proof]: (a, b) {:?}, {:?} => {:?} ({:?}) hash",
            array_bytes::bytes2hex("", &a.as_ref()),
            array_bytes::bytes2hex("", &b.as_ref()),
            array_bytes::bytes2hex("", &hash.as_ref()),
            array_bytes::bytes2hex("", &combined.as_ref())
        );
        hash
    });

    computed
}

pub const MESSAGE_MAX_LENGTH: u32 = 5;
pub type Message = BoundedVec<u8, ConstU32<MESSAGE_MAX_LENGTH>>;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq, EnumString)]
pub enum RawAction {
    #[strum(serialize = "REGISTER_JOB")]
    RegisterJob,
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub enum ParsedAction<AccountId, Extra>
where
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord,
    Extra: Parameter + Member,
{
    RegisterJob(JobId<AccountId>, JobRegistration<AccountId, Extra>),
}

pub type JobRegistrationFor<T> =
    JobRegistration<<T as frame_system::Config>::AccountId, <T as Config>::RegistrationExtra>;

pub trait MessageParser<AccountId, Extra>
where
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord,
    Extra: Parameter + Member,
{
    type Error;

    fn parse(encoded: &[u8]) -> Result<ParsedAction<AccountId, Extra>, Self::Error>;
}
