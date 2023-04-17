use mmr_lib;
use mmr_lib::Merge;
use sp_runtime::traits;
use sp_std::prelude::ToOwned;

use crate::types::{Node, TargetChainConfig, TargetChainNodeHasher};
use crate::HashOf;

pub use self::mmr::{node_pos_to_k_index, verify_leaves_proof, Mmr};

mod mmr;
pub mod storage;

/// Node type for runtime `T`.
pub type NodeOf<T, I> = Node<HashOf<T, I>>;

/// Default Merging & Hashing behavior for MMR.
pub struct Merger<H: TargetChainConfig>(sp_std::marker::PhantomData<H>);

impl<H: TargetChainConfig> Merge for Merger<H> {
    type Item = Node<H::Hash>;
    fn merge(left: &Self::Item, right: &Self::Item) -> mmr_lib::Result<Self::Item> {
        let mut concat = H::hash_node(left)
            .map_err(|_| mmr_lib::Error::MergeError("hasher failed".to_owned()))?
            .as_ref()
            .to_vec();
        concat.extend_from_slice(
            H::hash_node(right)
                .map_err(|_| mmr_lib::Error::StoreError("hasher failed".to_owned()))?
                .as_ref(),
        );

        Ok(Node::Hash(<H::Hasher as traits::Hash>::hash(&concat)))
    }
}
