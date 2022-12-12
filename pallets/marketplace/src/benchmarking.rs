use frame_benchmarking::{account, benchmarks, whitelist_account};
use frame_support::{assert_ok, sp_runtime::traits::StaticLookup};
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_core::*;
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;
use sp_std::prelude::*;

pub use pallet::Config;
pub use pallet_acurast::benchmarking::{consumer_account, processor_account};
use pallet_acurast::Pallet as Acurast;
use pallet_acurast::{Event as AcurastEvent, Fulfillment, JobRegistrationFor, Script};

pub use crate::stub::*;
use crate::Pallet as AcurastMarketplace;

use super::*;

pub fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

pub fn assert_last_acurast_event<T: Config>(generic_event: <T as pallet_acurast::Config>::Event) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn extract_reward_id<T: Config>() -> T::AssetId {
    // extract the reward id from the BenchmarkDefault implementation by the runtime.
    // first get the job requirements by getting default RegistrationExtra, and converting
    let benchmark_job_requirements: JobRequirementsFor<T> =
        <T as Config>::RegistrationExtra::benchmark_default().into();
    // then convert the nested reward to a T::Reward that is equal to it, but extended with other traits
    let reward_asset: T::Reward = benchmark_job_requirements.reward.into();
    // convert the reward to MinimumAssetImplementation to extract the asset id
    let reward_base_impl: MinimumAssetImplementation = reward_asset.into();
    reward_base_impl.id.into()
}
// return a usable advertisement for use inside an extrinsic call
pub fn advertisement<T: Config>(
    price_per_cpu_millisecond: u128,
    capacity: u32,
) -> AdvertisementFor<T> {
    // extract asset to be matched by processor
    let mut pricing: BoundedVec<
        PricingVariant<<T as Config>::AssetId, <T as Config>::AssetAmount>,
        ConstU32<MAX_PRICING_VARIANTS>,
    > = Default::default();
    let r = pricing.try_push(PricingVariant {
        reward_asset: extract_reward_id::<T>(),
        price_per_cpu_millisecond: price_per_cpu_millisecond.into(),
        bonus: 0.into(),
        maximum_slash: 0.into(),
    });
    assert!(r.is_ok(), "Expected Ok(_). Got {:#?}", r);
    Advertisement {
        pricing,
        allowed_consumers: None,
        capacity,
    }
}

/// return a usable job registration for use inside an extrinsic call
pub fn job_registration_with_reward<T: Config>(
    script: Script,
    cpu_milliseconds: u128,
    // reward: MinimumAssetImplementation,
) -> JobRegistrationFor<T> {
    // let r = JobRequirementsFor::<T> {
    //     slots: 1,
    //     cpu_milliseconds,
    //     reward: reward.into(),
    // };
    // let r: <T as Config>::RegistrationExtra = r.into();
    // let r: <T as pallet_acurast::Config>::RegistrationExtra = r.into();
    let r = <T as Config>::RegistrationExtra::benchmark_default();
    JobRegistrationFor::<T> {
        script,
        allowed_sources: None,
        allow_only_verified_sources: false,
        extra: r.into(),
    }
}

pub fn script() -> Script {
    SCRIPT_BYTES.to_vec().try_into().unwrap()
}

fn advertise_helper<T: Config>(submit: bool) -> (T::AccountId, AdvertisementFor<T>) {
    let caller: T::AccountId = processor_account::<T>();
    whitelist_account!(caller);

    let ad = advertisement::<T>(10000, 5);

    if submit {
        let register_call = AcurastMarketplace::<T>::advertise(
            RawOrigin::Signed(caller.clone()).into(),
            ad.clone(),
        );
        assert_ok!(register_call);
    }

    (caller, ad)
}

fn register_helper<T: Config>(submit: bool) -> (T::AccountId, JobRegistrationFor<T>) {
    let caller: T::AccountId = consumer_account::<T>();
    whitelist_account!(caller);

    // let asset_representation = MinimumAssetImplementation {
    //     id: 22,
    //     amount: 100,
    // };
    let job = job_registration_with_reward::<T>(script(), 2);

    if submit {
        let register_call =
            Acurast::<T>::register(RawOrigin::Signed(caller.clone()).into(), job.clone());
        assert_ok!(register_call);
    }

    (caller, job)
}

benchmarks! {

    advertise {
        // just create the data, do not submit the actual call (we want to benchmark `advertise`)
        let (caller, ad) = advertise_helper::<T>(false);
    }: _(RawOrigin::Signed(caller.clone()), ad.clone())
    verify {
        assert_last_event::<T>(Event::AdvertisementStored(
            ad, caller
        ).into());
    }

    delete_advertisement {
        // create the data and submit so we have an add in storage to delete when benchmarking `delete_advertisement`
        let (caller, _) = advertise_helper::<T>(true);
    }: _(RawOrigin::Signed(caller.clone()))
    verify {
        assert_last_event::<T>(Event::AdvertisementRemoved(
            caller
        ).into());
    }

    register {
        let _ = advertise_helper::<T>(true);
        let (caller, job) = register_helper::<T>(false);
    }: {
         pallet_acurast::Pallet::<T>::register(RawOrigin::Signed(caller.clone()).into(), job.clone())?
    }
    verify {
        assert_last_acurast_event::<T>(AcurastEvent::<T>::JobRegistrationStored(
            job, caller
        ).into());
    }

    deregister {
        let (caller, job) = register_helper::<T>(true);
    }: {
         pallet_acurast::Pallet::<T>::deregister(RawOrigin::Signed(caller.clone()).into(), job.script.clone())?
    }
    verify {
        assert_last_acurast_event::<T>(AcurastEvent::<T>::JobRegistrationRemoved(
            job.script, caller
        ).into());
    }

    fulfill {
        let (source, _) = advertise_helper::<T>(true);
        let (requester, job) = register_helper::<T>(true);
        let fulfillment = Fulfillment {
            script: job.script.clone(),
            payload: hex!("00").to_vec(),
        };
    }: {
         pallet_acurast::Pallet::<T>::fulfill(RawOrigin::Signed(source.clone()).into(), fulfillment.clone(), T::Lookup::unlookup(requester.clone()))?
    }
    verify {
        assert_last_acurast_event::<T>(AcurastEvent::<T>::ReceivedFulfillment(
            source.clone(),
            fulfillment,
            job,
            requester
        ).into());
    }

    impl_benchmark_test_suite!(AcurastMarketplace, mock::ExtBuilder::default().build(), mock::Test);
}
