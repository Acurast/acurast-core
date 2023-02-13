//! Benchmarking setup

use super::*;

use frame_benchmarking::{benchmarks_instance_pallet, whitelisted_caller};
use frame_system::RawOrigin;

benchmarks_instance_pallet! {

    impl_benchmark_test_suite!(crate::Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
