use frame_support::{
    dispatch::{PostDispatchInfo, Weight},
    sp_runtime::DispatchResultWithInfo,
};

use crate::{Config, Fulfillment};

/// Handles an acurast job fulfillment.
///
/// Implementations should check the origin and reject it with a [DispatchError::BadOrigin] if
/// it is not a valid/expected origin for an Acurast processor.
pub trait OnFulfillment<T: Config> {
    fn on_fulfillment(
        from: T::AccountId,
        fulfillment: Fulfillment,
    ) -> DispatchResultWithInfo<PostDispatchInfo>;
}

pub trait WeightInfo {
    fn fulfill() -> Weight;
}

impl WeightInfo for () {
    fn fulfill() -> Weight {
        Weight::from_ref_time(10_000)
    }
}
