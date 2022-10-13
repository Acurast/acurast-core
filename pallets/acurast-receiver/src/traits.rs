use frame_support::weights::PostDispatchInfo;
use frame_system::pallet_prelude::OriginFor;

use crate::Config;

/// Handle fulfillment messages.
pub trait OnFulfillment<T: Config> {
	fn fulfill(
		payload: &[u8],
	) -> frame_support::sp_runtime::DispatchResultWithInfo<PostDispatchInfo>;
}

/// Allows execution only from trusted origins.
pub trait ParachainBarrier<T: Config> {
	fn ensure_xcm_origin(
		origin: OriginFor<T>,
	) -> Result<(), frame_support::sp_runtime::DispatchError>;
}
