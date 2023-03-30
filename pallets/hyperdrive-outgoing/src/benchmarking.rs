//! Benchmarks for the MMR pallet.

#![cfg(feature = "runtime-benchmarks")]

use crate::*;
use frame_benchmarking::benchmarks_instance_pallet;
use frame_support::traits::OnInitialize;

benchmarks_instance_pallet! {
    on_initialize {
        let x in 1 .. 1_000;

        let leaves = x as NodeIndex;
    }: {
        for b in 0..leaves {
            Pallet::<T, I>::on_initialize((b as u32).into());
        }
    } verify {
        assert_eq!(crate::NumberOfLeaves::<T, I>::get(), leaves);
    }

    impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::mock::Test);
}
