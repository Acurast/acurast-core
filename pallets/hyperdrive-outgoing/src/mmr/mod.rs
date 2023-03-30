use sp_runtime::traits;

use mmr_lib;
use mmr_lib::Merge;

use crate::types::{Node, TargetChainHasher, TargetChainNodeHasher};
use crate::Config;

pub use self::mmr::{verify_leaves_proof, Mmr};

mod mmr;
pub mod storage;

/// Node type for runtime `T`.
pub type NodeOf<T, I> = Node<HashOf<T, I>>;

pub type HashOf<T, I> = <T as Config<I>>::Hash;

/// Default Merging & Hashing behavior for MMR.
pub struct Merger<H: TargetChainHasher>(sp_std::marker::PhantomData<H>);

impl<H: TargetChainHasher> Merge for Merger<H> {
    type Item = Node<H::Output>;
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

        Ok(Node::Hash(<H as traits::Hash>::hash(&concat)))
    }
}
