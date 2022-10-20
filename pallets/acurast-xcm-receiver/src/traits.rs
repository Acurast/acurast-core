#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    sp_runtime::{DispatchError, DispatchResultWithInfo},
    weights::PostDispatchInfo,
};
use frame_system::pallet_prelude::OriginFor;
use sp_std::prelude::Vec;

use crate::Config;

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
