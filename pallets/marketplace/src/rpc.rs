//! Node-specific RPC methods for interaction with pallet-acurast-marketplace.

use std::{marker::PhantomData, sync::Arc};

use crate::{MarketplaceRuntimeApi, PartialJobRegistration, Reward, RuntimeApiError};
use codec::Codec;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorObject},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use pallet_acurast::MultiOrigin;
use sp_runtime::traits::{HashFor, MaybeSerializeDeserialize};

const RUNTIME_ERROR: i32 = 8001;
const MARKETPLACE_ERROR: i32 = 8011;

/// Hyperdrive RPC methods.
#[rpc(client, server)]
pub trait MarketplaceApi<
    BlockHash,
    AssetId,
    AssetAmount,
    Error,
    R: Reward<AssetId = AssetId, AssetAmount = AssetAmount, Error = Error> + MaybeSerializeDeserialize,
    AccountId: MaybeSerializeDeserialize,
>
{
    /// Filters the given `sources` by those recently seen and matching partially specified `registration`.
    #[method(name = "filterMatchingSources")]
    fn filter_matching_sources(
        &self,
        registration: PartialJobRegistration<R, AccountId>,
        sources: Vec<AccountId>,
        consumer: Option<MultiOrigin<AccountId>>,
        latest_seen_after: Option<u128>,
    ) -> RpcResult<Vec<AccountId>>;
}

/// RPC methods.
pub struct Marketplace<Client, B> {
    client: Arc<Client>,
    _marker: PhantomData<B>,
}

impl<C, B> Marketplace<C, B> {
    /// Create new `Marketplace` with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

#[async_trait]
impl<Client, Block, AssetId, AssetAmount, Error, R, AccountId>
    MarketplaceApiServer<HashFor<Block>, AssetId, AssetAmount, Error, R, AccountId>
    for Marketplace<Client, (Block, AssetId, AssetAmount, Error, R, AccountId)>
where
    Block: BlockT,
    Client: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    Client::Api: MarketplaceRuntimeApi<Block, R, AccountId>,
    AssetId: MaybeSerializeDeserialize + Codec + Send + Sync + 'static,
    AssetAmount: MaybeSerializeDeserialize + Codec + Send + Sync + 'static,
    Error: MaybeSerializeDeserialize + Codec + Send + Sync + 'static,
    R: Reward<AssetId = AssetId, AssetAmount = AssetAmount, Error = Error>
        + MaybeSerializeDeserialize
        + Codec
        + Send
        + Sync
        + 'static,
    AccountId: MaybeSerializeDeserialize + Codec + Send + Sync + 'static,
{
    fn filter_matching_sources(
        &self,
        registration: PartialJobRegistration<R, AccountId>,
        sources: Vec<AccountId>,
        consumer: Option<MultiOrigin<AccountId>>,
        latest_seen_after: Option<u128>,
    ) -> RpcResult<Vec<AccountId>> {
        let api = self.client.runtime_api();
        let roots = api
            .filter_matching_sources(
                self.client.info().best_hash,
                registration,
                sources,
                consumer,
                latest_seen_after,
            )
            .map_err(runtime_error_into_rpc_error)?
            .map_err(marketplace_error_into_rpc_error)?;
        Ok(roots)
    }
}

/// Converts an marketplace-specific error into a [`CallError`].
fn marketplace_error_into_rpc_error(err: RuntimeApiError) -> CallError {
    let error_code = MARKETPLACE_ERROR
        + match err {
            RuntimeApiError::FilterMatchingSources => 1,
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
