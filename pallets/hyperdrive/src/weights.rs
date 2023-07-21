
//! Autogenerated weights for `pallet_acurast_hyperdrive`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-07-21, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `jenova`, CPU: `<UNKNOWN>`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("acurast-dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/acurast-node
// benchmark
// pallet
// --chain=acurast-dev
// --execution=wasm
// --wasm-execution=compiled
// --pallet
// pallet_acurast_hyperdrive
// --extrinsic
// *
// --steps=50
// --repeat=20
// --output=./benchmarks/

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_acurast_hyperdrive`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
	/// Storage: AcurastHyperdriveTezos StateTransmitter (r:0 w:1)
	/// Proof: AcurastHyperdriveTezos StateTransmitter (max_values: None, max_size: Some(24), added: 2499, mode: MaxEncodedLen)
	/// The range of component `l` is `[0, 50]`.
	fn update_state_transmitters(l: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 7_000_000 picoseconds.
		Weight::from_parts(8_591_379, 0)
			.saturating_add(Weight::from_parts(0, 0))
			// Standard Error: 1_559
			.saturating_add(Weight::from_parts(1_290_074, 0).saturating_mul(l.into()))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: AcurastHyperdriveTezos CurrentSnapshot (r:1 w:1)
	/// Proof: AcurastHyperdriveTezos CurrentSnapshot (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: AcurastHyperdriveTezos StateTransmitter (r:1 w:0)
	/// Proof: AcurastHyperdriveTezos StateTransmitter (max_values: None, max_size: Some(24), added: 2499, mode: MaxEncodedLen)
	/// Storage: AcurastHyperdriveTezos StateMerkleRootCount (r:1 w:1)
	/// Proof: AcurastHyperdriveTezos StateMerkleRootCount (max_values: None, max_size: Some(2098), added: 4573, mode: MaxEncodedLen)
	fn submit_state_merkle_root() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `125`
		//  Estimated: `10545`
		// Minimum execution time: 20_000_000 picoseconds.
		Weight::from_parts(21_000_000, 0)
			.saturating_add(Weight::from_parts(0, 10545))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: AcurastHyperdriveTezos CurrentTargetChainOwner (r:1 w:0)
	/// Proof: AcurastHyperdriveTezos CurrentTargetChainOwner (max_values: Some(1), max_size: Some(66), added: 561, mode: MaxEncodedLen)
	/// Storage: AcurastHyperdriveTezos StateMerkleRootCount (r:1 w:0)
	/// Proof: AcurastHyperdriveTezos StateMerkleRootCount (max_values: None, max_size: Some(2098), added: 4573, mode: MaxEncodedLen)
	fn submit_message() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `295`
		//  Estimated: `7114`
		// Minimum execution time: 20_000_000 picoseconds.
		Weight::from_parts(21_000_000, 0)
			.saturating_add(Weight::from_parts(0, 7114))
			.saturating_add(T::DbWeight::get().reads(2))
	}
	/// Storage: AcurastHyperdriveTezos CurrentTargetChainOwner (r:0 w:1)
	/// Proof: AcurastHyperdriveTezos CurrentTargetChainOwner (max_values: Some(1), max_size: Some(66), added: 561, mode: MaxEncodedLen)
	fn update_target_chain_owner() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_000_000 picoseconds.
		Weight::from_parts(9_000_000, 0)
			.saturating_add(Weight::from_parts(0, 0))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
