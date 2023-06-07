use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_support::traits::Get;
use frame_system::RawOrigin;

use acurast_common::{AllowedSourcesUpdate, JobRegistration};
use pallet_acurast_marketplace::Advertisement;

use super::*;

pub trait BenchmarkHelper<T: Config> {
    fn create_job_registration() -> JobRegistration<T::AccountId, T::RegistrationExtra>;
    fn create_allowed_sources_update(index: u32) -> AllowedSourcesUpdate<T::AccountId>;
    fn create_advertisement() -> Advertisement<T::AccountId, T::Balance, T::MaxAllowedConsumers>;
}

benchmarks! {
    register {
        let caller: T::AccountId = whitelisted_caller();
        let registration = T::BenchmarkHelper::create_job_registration();
    }: _(RawOrigin::Signed(caller), registration)

    deregister {
        let caller: T::AccountId = whitelisted_caller();
        let registration = T::BenchmarkHelper::create_job_registration();
    }: _(RawOrigin::Signed(caller), 0)

    update_allowed_sources {
        let x in 1 .. T::MaxAllowedSources::get();
        let caller: T::AccountId = whitelisted_caller();
        let registration = T::BenchmarkHelper::create_job_registration();
        let mut updates = Vec::<AllowedSourcesUpdate<T::AccountId>>::new();
        for i in 0..x {
            updates.push(T::BenchmarkHelper::create_allowed_sources_update(i));
        }
    }: _(RawOrigin::Signed(caller), 0, updates.try_into().unwrap())

    advertise {
        let caller: T::AccountId = whitelisted_caller();
        let advertisement = T::BenchmarkHelper::create_advertisement();
    }: _(RawOrigin::Signed(caller), advertisement)
}
