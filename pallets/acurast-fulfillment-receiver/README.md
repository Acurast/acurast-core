# Acurast Fulfillment Receiver Pallet
## 🚧🚧🚧 The project is still a work in progress 🚧🚧🚧

## Introduction

The Acurast Fullfilment Receiver Pallet, in combination with the [Acurast P256 crypto](../../p256-crypto/README.md) package, allows a Parachain to accepts direct fulfillments from Acurast Processors.

The Pallet exposes a one extrinsic.

### fulfill

Allows to post the [Fulfillment] of a job. The fulfillment structure consists of:

- The ipfs url of the `script` executed.
- The `payload` bytes representing the output of the `script`.

## Parachain Integration

Implement `pallet_acurast_fulfillment_receiver::Config` for your `Runtime` and add the Pallet:

```rust
frame_support::construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
        AcurastFulfillmentReceiver: crate::{Pallet, Call, Event<T>}
    }
);

impl pallet_acurast_fulfillment_receiver::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnFulfillment = FulfillmentHandler;
    type WeightInfo = ();
}

pub struct FulfillmentHandler;
impl OnFulfillment<Runtime> for FulfillmentHandler {
    fn on_fulfillment(
        from: <Runtime as frame_system::Config>::AccountId,
        _fulfillment: pallet_acurast_fulfillment_receiver::Fulfillment,
    ) -> sp_runtime::DispatchResultWithInfo<frame_support::weights::PostDispatchInfo> {
        /// check if origin is a valid Acurast Processor AccountId
        if !is_valid(&from) {
            return Err(DispatchError::BadOrigin.into());
        }
        /// if valid, then fulfillment can be used
        Ok(().into())
    }
}
```

Provide and implementation of [OnFulfillment] to handle the received fulfillment. The implementation should check that the fulfillment is from a known Acurast Processor account id.
