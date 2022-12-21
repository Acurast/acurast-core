use crate::Config;
use frame_support::{
    dispatch::PostDispatchInfo,
    sp_runtime::{DispatchError, DispatchResultWithInfo},
};
use frame_system::pallet_prelude::OriginFor;
use sp_std::prelude::*;

/// Handle fulfillment messages.
pub trait OnFulfillment<T: Config> {
    fn fulfill(
        payload: Vec<u8>,
        parameters: Option<Vec<u8>>,
    ) -> DispatchResultWithInfo<PostDispatchInfo>;
}

/// Allows execution only from trusted origins.
pub trait ParachainBarrier<T: Config> {
    fn ensure_xcm_origin(origin: OriginFor<T>) -> Result<(), DispatchError>;
}
