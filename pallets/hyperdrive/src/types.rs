use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
use frame_support::{pallet_prelude::*, storage::bounded_vec::BoundedVec};
use scale_info::TypeInfo;
use sp_core::ConstU32;
use sp_runtime::traits::Hash;
use sp_std::prelude::*;
use sp_std::vec;
use strum_macros::{EnumString, IntoStaticStr};

use pallet_acurast::{JobId, JobRegistration};

use crate::Config;

pub const STATE_TRANSMITTER_UPDATES_MAX_LENGTH: u32 = 50;
pub type StateTransmitterUpdates<T> =
    BoundedVec<StateTransmitterUpdateFor<T>, ConstU32<STATE_TRANSMITTER_UPDATES_MAX_LENGTH>>;

pub type StateTransmitterUpdateFor<T> = StateTransmitterUpdate<
    <T as frame_system::Config>::AccountId,
    <T as frame_system::Config>::BlockNumber,
>;

/// Defines the transmitter activity window.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
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

/// Defines the state proof as a path of blinded nodes. Does not contain the leaf hash, nor the root.
///
/// This vec contains all inner node hashes necessary to reconstruct the root hash given the
/// leaf hash.
pub type StateProof<Hash> = BoundedVec<StateProofNode<Hash>, ConstU32<256>>;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
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

pub const STATE_OWNER_MAX_LENGTH: u32 = 64;
pub type StateOwner = BoundedVec<u8, ConstU32<STATE_OWNER_MAX_LENGTH>>;

pub const KEY_MAX_LENGTH: u32 = 64;
pub type StateKey = BoundedVec<u8, ConstU32<KEY_MAX_LENGTH>>;

pub const VALUE_MAX_LENGTH: u32 = 4096;
pub type StateValue = BoundedVec<u8, ConstU32<VALUE_MAX_LENGTH>>;

#[derive(
    RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq, EnumString, IntoStaticStr,
)]
pub enum RawAction {
    #[strum(serialize = "REGISTER_JOB")]
    RegisterJob,
    #[strum(serialize = "DEREGISTER_JOB")]
    DeregisterJob,
    #[strum(serialize = "FINALIZE_JOB")]
    FinalizeJob,
}

/// Convert an index to a RawAction
impl TryFrom<u16> for RawAction {
    type Error = Vec<u8>;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(RawAction::RegisterJob),
            1 => Ok(RawAction::DeregisterJob),
            2 => Ok(RawAction::FinalizeJob),
            _ => Err(b"Unknown action index".to_vec()),
        }
    }
}

impl<AccountId, MaxAllowedSources: Get<u32>, Extra>
    From<&ParsedAction<AccountId, MaxAllowedSources, Extra>> for RawAction
{
    fn from(action: &ParsedAction<AccountId, MaxAllowedSources, Extra>) -> Self {
        match action {
            ParsedAction::RegisterJob(_, _) => RawAction::RegisterJob,
            ParsedAction::DeregisterJob(_) => RawAction::DeregisterJob,
            ParsedAction::FinalizeJob(_) => RawAction::FinalizeJob,
        }
    }
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub enum ParsedAction<AccountId, MaxAllowedSources: Get<u32>, Extra> {
    RegisterJob(
        JobId<AccountId>,
        JobRegistration<AccountId, MaxAllowedSources, Extra>,
    ),
    DeregisterJob(JobId<AccountId>),
    FinalizeJob(Vec<JobId<AccountId>>),
}

pub type MessageIdentifier = u128;

pub type JobRegistrationFor<T> = JobRegistration<
    <T as frame_system::Config>::AccountId,
    <T as Config>::RegistrationExtra,
    <T as Config>::MaxAllowedSources,
>;

pub trait MessageParser<AccountId, MaxAllowedSources: Get<u32>, Extra> {
    type Error;

    fn parse_key(encoded: &[u8]) -> Result<MessageIdentifier, Self::Error>;
    fn parse_value(
        encoded: &[u8],
    ) -> Result<ParsedAction<AccountId, MaxAllowedSources, Extra>, Self::Error>;
}

pub trait ActionExecutor<AccountId, MaxAllowedSources: Get<u32>, Extra> {
    fn execute(
        action: ParsedAction<AccountId, MaxAllowedSources, Extra>,
    ) -> DispatchResultWithPostInfo;
}

/// Tracks the progress during `submit_message`, intended to be included in events.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub enum ProcessMessageResult {
    ParsingKeyFailed,
    ParsingValueFailed,
    ActionFailed(RawAction),
    ActionSuccess,
    InvalidSequenceId,
    ProcessingFailed(DispatchError),
}

impl From<DispatchError> for ProcessMessageResult {
    fn from(value: DispatchError) -> Self {
        ProcessMessageResult::ProcessingFailed(value)
    }
}
