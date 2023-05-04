// This file is part of Substrate.

// Copyright (C) 2021-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Node-specific RPC methods for interaction with pallet-acurast-hyperdrive-outgoing.

use std::{marker::PhantomData, sync::Arc};

use codec::Codec;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorObject},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{HashFor, MaybeSerializeDeserialize};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};

use crate::{HyperdriveApi, LeafIndex, MMRError, SnapshotNumber, TargetChainProof};

const RUNTIME_ERROR: i32 = 8000;
const MMR_ERROR: i32 = 8010;

/// Hyperdrive RPC methods.
#[rpc(client, server)]
pub trait MmrApi<BlockHash, MmrHash: MaybeSerializeDeserialize> {
    /// Returns the snapshot MMR roots from `next_expected_snapshot_number, ...` onwards or an empty vec if no new snapshots.
    #[method(name = "snapshotRoots")]
    fn snapshot_roots(
        &self,
        next_expected_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Vec<(SnapshotNumber, MmrHash)>>;

    /// Returns the snapshot MMR root `next_expected_snapshot_number` or None if not snapshot not yet taken.
    #[method(name = "snapshotRoot")]
    fn snapshot_root(
        &self,
        next_expected_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Option<(SnapshotNumber, MmrHash)>>;

    /// Generates a self-contained MMR proof for the messages in the range `[next_message_number..last_message_excl]`.
    /// Leaves with their leaf index and position are part of the proof structure and contain the message encoded for the target chain.
    ///
    /// This rpc calls into the runtime function [`crate::Pallet::generate_target_chain_proof`].
    /// Optionally via `at`, a block hash at which the runtime should be queried can be specified.
    #[method(name = "generateProof")]
    fn generate_target_chain_proof(
        &self,
        next_message_number: LeafIndex,
        maximum_messages: Option<u64>,
        latest_known_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Option<TargetChainProof<MmrHash>>>;
}

/// MMR RPC methods.
pub struct Mmr<I, Client, Block> {
    client: Arc<Client>,
    _marker: PhantomData<(I, Block)>,
}

impl<I, C, B> Mmr<I, C, B> {
    /// Create new `Mmr` with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

#[async_trait]
impl<I: Send + Sync + 'static, Client, Block, MmrHash> MmrApiServer<HashFor<Block>, MmrHash>
    for Mmr<I, Client, (Block, MmrHash)>
where
    Block: BlockT,
    Client: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    Client::Api: HyperdriveApi<Block, MmrHash, I>,
    MmrHash: MaybeSerializeDeserialize + Codec + Send + Sync + 'static,
{
    fn snapshot_roots(
        &self,
        next_expected_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Vec<(SnapshotNumber, MmrHash)>> {
        let api = self.client.runtime_api();
        let roots = api
            .snapshot_roots(
                &BlockId::number(self.client.info().best_number),
                next_expected_snapshot_number,
            )
            .map_err(runtime_error_into_rpc_error)?
            .map_err(mmr_error_into_rpc_error)?;
        Ok(roots)
    }

    fn snapshot_root(
        &self,
        next_expected_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Option<(SnapshotNumber, MmrHash)>> {
        let api = self.client.runtime_api();
        let root = api
            .snapshot_root(
                &BlockId::number(self.client.info().best_number),
                next_expected_snapshot_number,
            )
            .map_err(runtime_error_into_rpc_error)?
            .map_err(mmr_error_into_rpc_error)?;
        Ok(root)
    }

    fn generate_target_chain_proof(
        &self,
        next_message_number: LeafIndex,
        maximum_messages: Option<u64>,
        latest_known_snapshot_number: SnapshotNumber,
    ) -> RpcResult<Option<TargetChainProof<MmrHash>>> {
        let api = self.client.runtime_api();

        let proof = api
            .generate_target_chain_proof(
                &BlockId::number(self.client.info().best_number),
                next_message_number,
                maximum_messages,
                latest_known_snapshot_number,
            )
            .map_err(runtime_error_into_rpc_error)?
            .map_err(mmr_error_into_rpc_error)?;

        Ok(proof)
    }
}

/// Converts an mmr-specific error into a [`CallError`].
fn mmr_error_into_rpc_error(err: MMRError) -> CallError {
    let error_code = MMR_ERROR
        + match err {
            MMRError::Push => 1,
            MMRError::GetRoot => 2,
            MMRError::Commit => 3,
            MMRError::GenerateProof => 4,
            MMRError::GenerateProofNoSnapshot => 5,
            MMRError::GenerateProofFutureSnapshot => 6,
            MMRError::GenerateProofFutureMessage => 7,
            MMRError::Verify => 8,
            MMRError::LeafNotFound => 9,
            MMRError::InconsistentSnapshotMeta => 10,
        };

    CallError::Custom(ErrorObject::owned(
        error_code,
        err.to_string(),
        Some(format!("{:?}", err)),
    ))
}

/// Converts a runtime trap into a [`CallError`].
fn runtime_error_into_rpc_error(err: impl std::fmt::Debug) -> CallError {
    CallError::Custom(ErrorObject::owned(
        RUNTIME_ERROR,
        "Runtime trapped",
        Some(format!("{:?}", err)),
    ))
}
