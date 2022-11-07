# Acurast Proxy Pallet
## 🚧🚧🚧 The project is still a work in progress 🚧🚧🚧

## Introduction

The Acurast Proxy pallet serves to call the extrinsics of the main pallet on the acurast parachain, from any other parachain.
It uses the XCMP protocol and XCM format for transmitting and interpreting messages.

### register

Allows the registration of a job. A registration consists of:

- An ipfs URL to a `script` (written in Javascript).
    - The script will be run in the Acurast Trusted Virtual Machine that uses a Trusted Execution Environment (TEE) on the Acurast Data Transmitter.
- An optional `allowedSources` list of allowed sources.
    - A list of `AccountId`s that are allowed to `fulfill` the job. If no list is provided, all sources are accepted.
- An `allowOnlyVerifiedSources` boolean indicating if only verified source can fulfill the job.
    - A verified source is one that has provided a valid key attestation.
- An `extra` structure that can be used to provide custom parameters.

Registrations are saved per `AccountId` and `script`, meaning that `register` is called twice from the same `AccountId` with the same `script` value, the previous registration is overwritten.

### deregister

Allows the de-registration of a job.

### updateAllowedSources

Allows to update the list of allowed sources for a previously registered job.

### fulfill

Allows to post the fulfillment of a registered job. The fulfillment structure consists of:

- The ipfs url of the `script` executed.
- The `payload` bytes representing the output of the `script`.

In addition to the `fulfillment` structure, `fulfill` expects the `AccountId` of the `requester` of the job.

## Setup

Add the following dependency to your Cargo manifest:

```toml
[dependencies]
acurast-proxy = { git = "https://github.com/Acurast/acurast-core", default-features = false, branch = "feat/proxy-pallet" }
```

## Parachain Integration
To integrate the acurast proxy in a parachain, there is some runtime setup needed. First we need to add the pallet to
the [construct_runtime](https:://example.com) macro as following:
```rust
construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = opaque::Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
        
        ...
        
        AcurastProxy: acurast_proxy::{Pallet, Call, Event<T>} = 34,
	
    }
);
```
<br>
Then we have to add some parameter types to be used in the pallet config. Specifically the pallet id of the marketplace 
pallet and the parachain id of acurast parachain. The parachain id is used to route correctly the xcm messages from cumulus
to acurast. The pallet id is needed to properly encode the call that we want to execute into the xcm message.

```rust
parameter_types! {
	pub const AcurastParachainId: u32 = 2000;
	pub const AcurastPalletId: u8 = 41;
}
```
the parachain id should be found in the chainspec of acurast, and the pallet id in the definition inside the construct_runtime macro
(e.g "ExamplePallet: pallet_example = 42" would mean the pallet id is 42)


Lastly we need to configure the pallet with the parameter types defined before, and some default values like Event and ().
The XcmRouter is defined in xcm_config, and we are also using the default one, but whoever integrates this pallet should
make sure that the router is able to send XCMP messages.
```rust
impl acurast_proxy::Config for Runtime {
	type Event = Event;
	type AcurastParachainId = AcurastParachainId;
	type AcurastPalletId = AcurastPalletId;
	type XcmSender = XcmRouter;
	type RegistrationExtra = JobRequirements<AcurastAsset>;
}
```
