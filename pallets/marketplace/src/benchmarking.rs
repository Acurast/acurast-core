use frame_benchmarking::{account, benchmarks, whitelist_account};
use frame_support::{
    assert_ok,
    sp_runtime::traits::{AccountIdConversion, Get, StaticLookup},
    traits::Currency,
};
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_core::*;
use sp_runtime::traits::ConstU32;
use sp_runtime::BoundedVec;
use sp_std::prelude::*;

pub use pallet::Config;
use pallet_acurast::{Event as AcurastEvent, Fulfillment, JobRegistrationFor, Script};
use pallet_acurast::{Pallet as Acurast, Schedule};

pub use crate::stub::*;
use crate::Pallet as AcurastMarketplace;

use super::*;

pub fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

pub fn assert_last_acurast_event<T: Config>(
    generic_event: <T as pallet_acurast::Config>::RuntimeEvent,
) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

pub fn advertisement<T: Config>(
    fee_per_millisecond: u128,
    storage_capacity: u32,
) -> AdvertisementFor<T>
where
    <T as Config>::RegistrationExtra: From<JobRequirementsFor<T>>,
    RewardFor<T>: From<MockAsset>,
    <T as Config>::AssetId: From<u32>,
    <T as Config>::AssetAmount: From<u128>,
{
    let mut pricing: BoundedVec<
        PricingVariant<<T as Config>::AssetId, <T as Config>::AssetAmount>,
        ConstU32<MAX_PRICING_VARIANTS>,
    > = Default::default();
    let r = pricing.try_push(PricingVariant {
        reward_asset: 0.into(),
        fee_per_millisecond: fee_per_millisecond.into(),
        fee_per_storage_byte: 5.into(),
        base_fee_per_execution: 0.into(),
        scheduling_window: SchedulingWindow::Delta(2_628_000_000), // 1 month
    });
    assert!(r.is_ok(), "Expected Ok(_). Got {:#?}", r);
    Advertisement {
        pricing,
        allowed_consumers: None,
        storage_capacity,
        max_memory: 80_000,
        network_request_quota: 5,
    }
}

pub fn job_registration_with_reward<T: Config>(
    script: Script,
    duration: u64,
    reward_value: u128,
) -> JobRegistrationFor<T>
where
    <T as Config>::RegistrationExtra: From<JobRequirementsFor<T>>,
    RewardFor<T>: From<MockAsset>,
{
    let r = JobRequirements {
        slots: 1,
        reward: asset(reward_value).into(),
        instant_match: None,
    };
    let r: <T as Config>::RegistrationExtra = r.into();
    let r: <T as pallet_acurast::Config>::RegistrationExtra = r.into();
    JobRegistrationFor::<T> {
        script,
        allowed_sources: None,
        allow_only_verified_sources: false,
        schedule: Schedule {
            duration,
            start_time: 1671800400000, // 23.12.2022 13:00
            end_time: 1671886800000,   // 24.12.2022 13:00 (one day later)
            interval: 180000,          // 30min
            max_start_delay: 5000,
        },
        memory: 5_000u32,
        network_requests: 5,
        storage: 20_000u32,
        extra: r,
    }
}

pub fn script() -> Script {
    SCRIPT_BYTES.to_vec().try_into().unwrap()
}

fn token_22_funded_account<T: Config>() -> T::AccountId
where
    T: pallet_assets::Config,
    <T as pallet_assets::Config>::AssetId: From<u32>,
    <T as pallet_assets::Config>::Balance: From<u128>,
{
    use pallet_assets::Pallet as Assets;
    let caller: T::AccountId = account("token_account", 0, SEED);
    whitelist_account!(caller);
    let pallet_account: T::AccountId = <T as Config>::PalletId::get().into_account_truncating();
    let pallet_origin: T::RuntimeOrigin = RawOrigin::Signed(pallet_account.clone()).into();

    T::Currency::make_free_balance_be(&caller, u32::MAX.into());

    // might fail if asset is already created in genesis config. Fail doesn't affect later mint
    let _create_token_call = Assets::<T>::create(
        pallet_origin.clone(),
        <T as pallet_assets::Config>::AssetId::from(22).into(),
        T::Lookup::unlookup(pallet_account.clone()),
        10u32.into(),
    );

    let mint_token_call = Assets::<T>::mint(
        pallet_origin,
        <T as pallet_assets::Config>::AssetId::from(22).into(),
        T::Lookup::unlookup(caller.clone()),
        INITIAL_BALANCE.into(),
    );
    assert_ok!(mint_token_call);

    caller
}

fn advertise_helper<T: Config>(submit: bool) -> (T::AccountId, AdvertisementFor<T>)
where
    T: pallet_assets::Config,
    <T as Config>::AssetId: From<u32>,
    <T as Config>::AssetAmount: From<u128>,
    <T as pallet_assets::Config>::AssetId: From<u32>,
    <T as pallet_assets::Config>::Balance: From<u128>,
    <T as Config>::RegistrationExtra: From<JobRequirementsFor<T>>,
    RewardFor<T>: From<MockAsset>,
{
    let caller: T::AccountId = token_22_funded_account::<T>();
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

fn register_helper<T: Config>(submit: bool) -> (T::AccountId, JobRegistrationFor<T>)
where
    T: pallet_assets::Config,
    <T as Config>::AssetId: From<u32>,
    <T as Config>::AssetAmount: From<u128>,
    <T as pallet_assets::Config>::AssetId: From<u32>,
    <T as pallet_assets::Config>::Balance: From<u128>,
    <T as Config>::RegistrationExtra: From<JobRequirementsFor<T>>,
    RewardFor<T>: From<MockAsset>,
{
    let caller: T::AccountId = token_22_funded_account::<T>();
    whitelist_account!(caller);

    let job = job_registration_with_reward::<T>(script(), 2, 20100);

    if submit {
        let register_call =
            Acurast::<T>::register(RawOrigin::Signed(caller.clone()).into(), job.clone());
        assert_ok!(register_call);
    }

    (caller, job)
}

benchmarks! {
    where_clause {  where
        T: pallet_assets::Config + pallet_acurast::Config,
        <T as Config>::RegistrationExtra: From<JobRequirementsFor<T>>,
        RewardFor<T>: From<MockAsset>,
        <T as Config>::AssetId: From<u32>,
        <T as Config>::AssetAmount: From<u128>,
        <T as pallet_assets::Config>::AssetId: From<u32>,
        <T as pallet_assets::Config>::Balance: From<u128>,
    }

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
