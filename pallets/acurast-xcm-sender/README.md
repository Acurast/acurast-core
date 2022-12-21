# Acurast XCM Sender

## 🚧🚧🚧 The project is still a work in progress 🚧🚧🚧

## Introduction

The `pallet-acurast-xcm-sender` adds support for [Acurast parachain](https://docs.acurast.com/) to send [XCM](https://wiki.polkadot.network/docs/learn-xcm) messages to acurast enabled parachains.

## Setup

1. Add the following dependency to your Cargo manifest:

```toml
[dependencies]
pallet-acurast-xcm-sender = { git = "https://github.com/Acurast/acurast-core.git" }
```

2. Implement `pallet_acurast_xcm_sender::Config` for your `Runtime` and add the Pallet:

```rust
/// Runtime example

parameter_types! {
	pub const AcurastParachainId: u32 = 2000;
	pub const AcurastReceiverPalletId: u8 = 130;
}

impl pallet_acurast_xcm_sender::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmSender = crate::xcm_config::XcmRouter;
	type AcurastReceiverPalletId = AcurastReceiverPalletId;
	type AcurastParachainId = AcurastParachainId;
}


// Add pallet to the runtime
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		// All your other pallets
        ...
		AcurastSender: pallet_acurast_xcm_sender::{Pallet, Event<T>}
	}
);
```

3. Calling `Acurast XCM sender`:

```rust
match AcurastSender::fulfill(_origin, _fulfillment.payload) {
    Ok(()) => {
		log::info!("SUCCESS");
    },
    Err(err) => log::error!("Error: {:?}", err);
};
```
