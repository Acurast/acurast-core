
//! Autogenerated weights for `pallet_acurast`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-10-20, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `dhcp-19-130.guest.tezos.foundation`, CPU: `<UNKNOWN>`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("acurast-dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/acurast-node benchmark \
// pallet \
// --chain=acurast-dev \
// --execution=wasm \
// --wasm-execution=compiled \
// --pallet=pallet_acurast \
// --extrinsic \
// "*" \
// --steps=50 \
// --repeat=20 \
// --output=../acurast-core/pallets/acurast/src/weights.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight}};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_acurast`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
	// Storage: Assets Asset (r:1 w:1)
	// Storage: Assets Account (r:2 w:2)
	// Storage: System Account (r:1 w:1)
	// Storage: Acurast StoredJobRegistration (r:0 w:1)
	fn register() -> Weight {
		Weight::from_ref_time(45_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().writes(5 as u64))
	}
	// Storage: Acurast StoredJobRegistration (r:0 w:1)
	fn deregister() -> Weight {
		Weight::from_ref_time(13_000_000 as u64)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Acurast StoredJobRegistration (r:1 w:1)
	fn update_allowed_sources() -> Weight {
		Weight::from_ref_time(22_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(1 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Timestamp Now (r:1 w:0)
	// Storage: Acurast StoredRevokedCertificate (r:4 w:0)
	// Storage: Acurast StoredAttestation (r:0 w:1)
	fn submit_attestation() -> Weight {
		Weight::from_ref_time(10_046_000_000 as u64)
			.saturating_add(T::DbWeight::get().reads(5 as u64))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
	// Storage: Acurast StoredRevokedCertificate (r:0 w:1)
	fn update_certificate_revocation_list() -> Weight {
		Weight::from_ref_time(12_000_000 as u64)
			.saturating_add(T::DbWeight::get().writes(1 as u64))
	}
}
