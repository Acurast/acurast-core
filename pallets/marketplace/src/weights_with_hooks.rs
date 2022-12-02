//! Autogenerated weights for pallet_acurast_marketplace
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-11-25, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `smartnuance`, CPU: `Intel(R) Core(TM) i7-10510U CPU @ 1.80GHz`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("acurast-dev"), DB CACHE: 1024

// Executed Command:
// ../../../acurast-substrate/target/release/acurast-node
// benchmark
// pallet
// --chain=acurast-dev
// --execution=wasm
// --wasm-execution=compiled
// --pallet=pallet_acurast_marketplace
// --extrinsic
// register,deregister,update_allowed_sources
// --steps=50
// --repeat=20
// --output=./src/weights_with_hooks.rs
// --template=./src/weights_with_hooks.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;
use pallet_acurast;

/// Weights for pallet_acurast_marketplace using the Substrate node and recommended hardware.
pub struct Weights<T>(PhantomData<T>);
impl<T: frame_system::Config + pallet_acurast::Config> pallet_acurast::WeightInfo for Weights<T> {
    // Storage: AcurastMarketplace StoredJobStatus (r:1 w:1)
    // Storage: AcurastMarketplace StoredAdIndex (r:1 w:0)
    // Storage: AcurastMarketplace StoredCapacity (r:1 w:1)
    // Storage: AcurastMarketplace StoredAdvertisement (r:1 w:0)
    // Storage: Assets Asset (r:1 w:1)
    // Storage: Assets Account (r:2 w:2)
    // Storage: System Account (r:1 w:1)
    // Storage: AcurastMarketplace StoredJobAssignment (r:0 w:1)
    // Storage: Acurast StoredJobRegistration (r:0 w:1)
    fn register() -> Weight {
        // Minimum execution time:  nanoseconds.
        Weight::from_ref_time(155_697_000)
            .saturating_add(T::DbWeight::get().reads(8))
            .saturating_add(T::DbWeight::get().writes(8))
    }
    // Storage: AcurastMarketplace StoredJobStatus (r:1 w:1)
    // Storage: Acurast StoredJobRegistration (r:0 w:1)
    fn deregister() -> Weight {
        // Minimum execution time:  nanoseconds.
        Weight::from_ref_time(50_437_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    fn fulfill() -> Weight {
        // Minimum execution time:  nanoseconds.
        Weight::from_ref_time(50_437_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2))
    }

    fn update_allowed_sources() -> Weight {
        pallet_acurast::weights::WeightInfo::<T>::update_allowed_sources()
    }
    fn update_job_assignments() -> Weight {
        pallet_acurast::weights::WeightInfo::<T>::update_job_assignments()
    }
    fn submit_attestation() -> Weight {
        pallet_acurast::weights::WeightInfo::<T>::submit_attestation()
    }
    fn update_certificate_revocation_list() -> Weight {
        pallet_acurast::weights::WeightInfo::<T>::update_certificate_revocation_list()
    }
}
