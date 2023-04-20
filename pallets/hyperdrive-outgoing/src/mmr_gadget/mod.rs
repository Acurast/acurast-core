// This file is part of Substrate.

// Copyright (C) 2021-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # MMR offchain gadget
//!
//! The MMR offchain gadget is run alongside `pallet-mmr` to assist it with offchain
//! canonicalization of finalized MMR leaves and nodes.
//! The gadget should only be run on nodes that have Indexing API enabled (otherwise
//! `pallet-mmr` cannot write to offchain and this gadget has nothing to do).
//!
//! The runtime `pallet-mmr` creates one new MMR leaf per block and all inner MMR parent nodes
//! generated by the MMR when adding said leaf. MMR nodes are stored both in:
//! - on-chain storage - hashes only; not full leaf content;
//! - off-chain storage - via Indexing API, full leaf content (and all internal nodes as well) is
//!   saved to the Off-chain DB using a key derived from `parent_hash` and node index in MMR. The
//!   `parent_hash` is also used within the key to avoid conflicts and overwrites on forks (leaf
//!   data is only allowed to reference data coming from parent block).
//!
//! This gadget is driven by block finality and in responsible for pruning stale forks from
//! offchain db, and moving finalized forks under a "canonical" key based solely on node `pos`
//! in the MMR.

#![warn(missing_docs)]

mod aux_schema;
mod offchain_mmr;
#[cfg(test)]
pub mod test_utils;

use offchain_mmr::OffchainMmr;
use futures::StreamExt;
use log::{debug, error, trace, warn};
use sc_client_api::{Backend, BlockchainEvents, FinalityNotifications};
use sc_offchain::OffchainDb;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::{HeaderBackend, HeaderMetadata};
use crate::{utils, LeafIndex, HyperdriveApi};
use sp_runtime::{
	generic::BlockId,
	traits::{Block, Header, NumberFor},
};
use std::{marker::PhantomData, sync::Arc};
use codec::Codec;

/// Logging target for the mmr gadget.
pub const LOG_TARGET: &str = "mmr";

struct OffchainMmrBuilder<B: Block, BE: Backend<B>, C, MmrHash: codec::Codec> {
	backend: Arc<BE>,
	client: Arc<C>,
	offchain_db: OffchainDb<BE::OffchainStorage>,
	indexing_prefix: Vec<u8>,

	_phantom: PhantomData<(B, MmrHash)>,
}

impl<B, BE, C, MmrHash> OffchainMmrBuilder<B, BE, C, MmrHash>
	where
		B: Block,
		BE: Backend<B>,
		C: ProvideRuntimeApi<B> + HeaderBackend<B> + HeaderMetadata<B>,
		MmrHash: Codec,
		C::Api: HyperdriveApi<B, MmrHash>,
{
	async fn try_build(
		self,
		finality_notifications: &mut FinalityNotifications<B>,
	) -> Option<OffchainMmr<B, BE, C>> {
		while let Some(notification) = finality_notifications.next().await {
			let best_block = *notification.header.number();
			match self.client.runtime_api().mmr_leaf_count(&BlockId::number(best_block)) {
				Ok(Ok(mmr_leaf_count)) => {
					debug!(
						target: LOG_TARGET,
						"pallet-mmr detected at block {:?} with mmr size {:?}",
						best_block,
						mmr_leaf_count
					);
					match utils::first_mmr_block_num::<B::Header>(best_block, mmr_leaf_count) {
						Ok(first_mmr_block) => {
							debug!(
								target: LOG_TARGET,
								"pallet-mmr genesis computed at block {:?}", first_mmr_block,
							);
							let best_canonicalized =
								match offchain_mmr::load_or_init_best_canonicalized::<B, BE>(
									&*self.backend,
									first_mmr_block,
								) {
									Ok(best) => best,
									Err(e) => {
										error!(
											target: LOG_TARGET,
											"Error loading state from aux db: {:?}", e
										);
										return None
									},
								};
							let mut offchain_mmr = OffchainMmr {
								backend: self.backend,
								client: self.client,
								offchain_db: self.offchain_db,
								indexing_prefix: self.indexing_prefix,
								first_mmr_block,
								best_canonicalized,
							};
							// We need to make sure all blocks leading up to current notification
							// have also been canonicalized.
							offchain_mmr.canonicalize_catch_up(&notification);
							// We have to canonicalize and prune the blocks in the finality
							// notification that lead to building the offchain-mmr as well.
							offchain_mmr.canonicalize_and_prune(notification);
							return Some(offchain_mmr)
						},
						Err(e) => {
							error!(
								target: LOG_TARGET,
								"Error calculating the first mmr block: {:?}", e
							);
						},
					}
				},
				_ => {
					trace!(
						target: LOG_TARGET,
						"Waiting for MMR pallet to become available... (best finalized {:?})",
						notification.header.number()
					);
				},
			}
		}

		error!(
			target: LOG_TARGET,
			"Finality notifications stream closed unexpectedly. \
			Couldn't build the canonicalization engine",
		);
		None
	}
}

/// A MMR Gadget.
pub struct MmrGadget<B: Block, BE: Backend<B>, C, MmrHash: Codec> {
	finality_notifications: FinalityNotifications<B>,

	_phantom: PhantomData<(B, BE, C, MmrHash)>,
}

impl<B, BE, C, MmrHash> MmrGadget<B, BE, C, MmrHash>
	where
		B: Block,
		<B::Header as Header>::Number: Into<LeafIndex>,
		BE: Backend<B>,
		C: BlockchainEvents<B> + HeaderBackend<B> + HeaderMetadata<B> + ProvideRuntimeApi<B>,
		MmrHash: Codec,
		C::Api: HyperdriveApi<B, MmrHash>,
{
	async fn run(mut self, builder: OffchainMmrBuilder<B, BE, C, MmrHash>) {
		let mut offchain_mmr = match builder.try_build(&mut self.finality_notifications).await {
			Some(offchain_mmr) => offchain_mmr,
			None => return,
		};

		while let Some(notification) = self.finality_notifications.next().await {
			offchain_mmr.canonicalize_and_prune(notification);
		}
	}

	/// Create and run the MMR gadget.
	pub async fn start(client: Arc<C>, backend: Arc<BE>, indexing_prefix: Vec<u8>) {
		let offchain_db = match backend.offchain_storage() {
			Some(offchain_storage) => OffchainDb::new(offchain_storage),
			None => {
				warn!(
					target: LOG_TARGET,
					"Can't spawn a MmrGadget for a node without offchain storage."
				);
				return
			},
		};

		let mmr_gadget = MmrGadget::<B, BE, C, MmrHash> {
			finality_notifications: client.finality_notification_stream(),

			_phantom: Default::default(),
		};
		mmr_gadget
			.run(OffchainMmrBuilder {
				backend,
				client,
				offchain_db,
				indexing_prefix,
				_phantom: Default::default(),
			})
			.await
	}
}

#[cfg(test)]
mod tests {
	use crate::mmr_gadget::test_utils::run_test_with_mmr_gadget;
	use sp_runtime::generic::BlockId;
	use std::time::Duration;

	#[test]
	fn mmr_first_block_is_computed_correctly() {
		// Check the case where the first block is also the first block with MMR.
		run_test_with_mmr_gadget(|client| async move {
			// G -> A1 -> A2
			//      |
			//      | -> first mmr block

			let a1 = client.import_block(&BlockId::Number(0), b"a1", Some(0)).await;
			let a2 = client.import_block(&BlockId::Hash(a1.hash()), b"a2", Some(1)).await;

			client.finalize_block(a1.hash(), Some(1));
			tokio::time::sleep(Duration::from_millis(200)).await;
			// expected finalized heads: a1
			client.assert_canonicalized(&[&a1]);
			client.assert_not_pruned(&[&a2]);
		});

		// Check the case where the first block with MMR comes later.
		run_test_with_mmr_gadget(|client| async move {
			// G -> A1 -> A2 -> A3 -> A4 -> A5 -> A6
			//                        |
			//                        | -> first mmr block

			let a1 = client.import_block(&BlockId::Number(0), b"a1", None).await;
			let a2 = client.import_block(&BlockId::Hash(a1.hash()), b"a2", None).await;
			let a3 = client.import_block(&BlockId::Hash(a2.hash()), b"a3", None).await;
			let a4 = client.import_block(&BlockId::Hash(a3.hash()), b"a4", Some(0)).await;
			let a5 = client.import_block(&BlockId::Hash(a4.hash()), b"a5", Some(1)).await;
			let a6 = client.import_block(&BlockId::Hash(a5.hash()), b"a6", Some(2)).await;

			client.finalize_block(a5.hash(), Some(2));
			tokio::time::sleep(Duration::from_millis(200)).await;
			// expected finalized heads: a4, a5
			client.assert_canonicalized(&[&a4, &a5]);
			client.assert_not_pruned(&[&a6]);
		});
	}

	#[test]
	fn does_not_panic_on_invalid_num_mmr_blocks() {
		run_test_with_mmr_gadget(|client| async move {
			// G -> A1
			//      |
			//      | -> first mmr block

			let a1 = client.import_block(&BlockId::Number(0), b"a1", Some(0)).await;

			// Simulate the case where the runtime says that there are 2 mmr_blocks when in fact
			// there is only 1.
			client.finalize_block(a1.hash(), Some(2));
			tokio::time::sleep(Duration::from_millis(200)).await;
			// expected finalized heads: -
			client.assert_not_canonicalized(&[&a1]);
		});
	}
}
