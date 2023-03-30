//! Default weights for the MMR Pallet
//! This file was not auto-generated.

use frame_support::weights::{
    constants::{RocksDbWeight as DbWeight, WEIGHT_REF_TIME_PER_NANOS},
    Weight,
};

impl crate::WeightInfo for () {
    fn on_initialize() -> Weight {
        DbWeight::get().reads_writes(3, 3)
    }

    fn send_message() -> Weight {
        // TODO calculate
        DbWeight::get().reads_writes(3, 3)
    }

    fn send_message_actual(peaks: u64) -> Weight {
        // Reading the parent hash.
        let leaf_weight = DbWeight::get().reads(1);
        // Blake2 hash cost.
        let hash_weight = Weight::from_ref_time(2u64 * WEIGHT_REF_TIME_PER_NANOS);
        // No-op hook.
        let hook_weight = Weight::zero();

        leaf_weight
            .saturating_add(hash_weight)
            .saturating_add(hook_weight)
            .saturating_add(DbWeight::get().reads_writes(2 + peaks, 2 + peaks))
    }
}
