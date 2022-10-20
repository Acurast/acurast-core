# Acurast XCM Receiver Pallet

## 🚧🚧🚧 The project is still a work in progress 🚧🚧🚧

## Introduction

The `pallet-acurast-xcm-receiver` adds support for parachains to receive [XCM](https://wiki.polkadot.network/docs/learn-xcm) messages from [Acurast parachain](https://docs.acurast.com/).

The Pallet exposes the following extrinsics.

### fulfill

Allows to post the fulfillment of a registered job. The `fulfill` call will fail if the job was not previously assigned to the origin. The fulfillment structure consists of:

## Setup

1. Add the following dependency to your Cargo manifest:

```toml
[dependencies]
pallet-acurast-xcm-receiver = { git = "https://github.com/Acurast/acurast-core.git" }
```

2. Implement `pallet_acurast_xcm_receiver::Config` for your `Runtime` and add the Pallet:

```rust
/// Runtime example

pub struct ParachainBarrier;
impl pallet_acurast_xcm_receiver::traits::ParachainBarrier<Runtime> for ParachainBarrier {
	fn ensure_xcm_origin(
		origin: frame_system::pallet_prelude::OriginFor<Runtime>,
	) -> Result<(), sp_runtime::DispatchError> {
		// List of allowd parachains
		let allowed_parachains = [
			// The Acurast parachain identifier
			xcm::opaque::latest::Junction::Parachain(2001),
		];

		// Ensure that the call comes from an xcm message
		let location = pallet_xcm::ensure_xcm(origin)?;

		let is_valid_origin = location
			.interior()
			.iter()
			.any(|junction| allowed_parachains.contains(junction));

		if !is_valid_origin {
			return Err(sp_runtime::DispatchError::Other(
				"MultiLocation not allowed.",
			));
		}

		Ok(())
	}
}

pub struct OnAcurastFulfillment;
impl pallet_acurast_xcm_receiver::traits::OnFulfillment<Runtime> for OnAcurastFulfillment {
	fn fulfill(
		payload: &[u8],
	) -> sp_runtime::DispatchResultWithInfo<frame_support::weights::PostDispatchInfo> {
        // handle payload (e.i. Call a contract)
	}
}

impl pallet_acurast_xcm_receiver::Config for Runtime {
	type Event = Event;
	type Payload = sp_runtime::bounded::bounded_vec::BoundedVec<u8, ConstU32<128>>;
	type OnFulfillment = OnAcurastFulfillment;
	type Barrier = ParachainBarrier;
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
		AcurastReceiver: pallet_acurast_xcm_receiver::{Pallet, Storage, Call, Event<T>};
	}
);
```
