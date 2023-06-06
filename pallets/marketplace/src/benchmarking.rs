use frame_benchmarking::{account, benchmarks, whitelist_account};
use frame_support::{
    assert_ok,
    sp_runtime::traits::{AccountIdConversion, Get, StaticLookup},
    traits::Currency,
};
use frame_system::RawOrigin;
use sp_core::*;
use sp_runtime::BoundedVec;
use sp_runtime::{traits::ConstU32, DispatchError};
use sp_std::prelude::*;

pub use pallet::Config;
use pallet_acurast::{
    Event as AcurastEvent, JobId, JobModules, JobRegistrationFor, MultiOrigin, Pallet as Acurast,
    Schedule, Script,
};

pub use crate::stub::*;
use crate::Pallet as AcurastMarketplace;

use super::*;

pub trait BenchmarkHelper<T: Config> {
    /// Extends the job requirements, defined by benchmarking code in this pallet, with the containing struct RegistrationExtra.
    fn registration_extra(r: JobRequirementsFor<T>) -> <T as Config>::RegistrationExtra;
}

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
    RewardFor<T>: From<MockAsset>,
    <T as Config>::AssetId: From<u32>,
    <T as Config>::Balance: From<u128>,
{
    let mut pricing: BoundedVec<Pricing<<T as Config>::Balance>, ConstU32<MAX_PRICING_VARIANTS>> =
        Default::default();
    let r = pricing.try_push(Pricing {
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
        available_modules: JobModules::default(),
    }
}

pub fn job_registration_with_reward<T: Config>(
    script: Script,
    duration: u64,
    reward_value: u128,
) -> JobRegistrationFor<T>
where
    RewardFor<T>: From<MockAsset>,
{
    let r = JobRequirements {
        slots: 1,
        reward: asset(reward_value).into(),
        min_reputation: Some(0),
        instant_match: None,
    };
    let r: <T as Config>::RegistrationExtra = <T as Config>::BenchmarkHelper::registration_extra(r);
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
        required_modules: JobModules::default(),
        extra: r,
    }
}

pub fn script() -> Script {
    SCRIPT_BYTES.to_vec().try_into().unwrap()
}

fn token_22_funded_account<T: Config>(index: u32) -> T::AccountId {
    use pallet_assets::Pallet as Assets;
    let caller: T::AccountId = account("token_account", index, SEED);
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
    <T as Config>::Balance: From<u128>,
    <T as pallet_assets::Config>::AssetId: From<u32>,
    <T as pallet_assets::Config>::Balance: From<u128>,
    RewardFor<T>: From<MockAsset>,
{
    let caller: T::AccountId = token_22_funded_account::<T>(0);
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
    <T as Config>::Balance: From<u128>,
    <T as pallet_assets::Config>::AssetId: From<u32>,
    <T as pallet_assets::Config>::Balance: From<u128>,
    RewardFor<T>: From<MockAsset>,
{
    let caller: T::AccountId = token_22_funded_account::<T>(0);
    whitelist_account!(caller);

    let job = job_registration_with_reward::<T>(script(), 2, 20100);

    if submit {
        let register_call =
            Acurast::<T>::register(RawOrigin::Signed(caller.clone()).into(), job.clone());
        assert_ok!(register_call);
    }

    (caller, job)
}

fn acknowledge_match_helper<T: Config>(
) -> Result<(T::AccountId, JobId<T::AccountId>), DispatchError>
where
    T: pallet_assets::Config,
    <T as Config>::AssetId: From<u32>,
    <T as pallet_assets::Config>::AssetId: From<u32>,
    <T as pallet_assets::Config>::Balance: From<u128>,
    RewardFor<T>: From<MockAsset>,
{
    let consumer: T::AccountId = token_22_funded_account::<T>(0);
    let processor: T::AccountId = token_22_funded_account::<T>(1);
    let ad = advertisement::<T>(1, 1000);
    assert_ok!(AcurastMarketplace::<T>::advertise(
        RawOrigin::Signed(processor.clone()).into(),
        ad.clone(),
    ));
    let job = job_registration_with_reward::<T>(script(), 100, 1000);
    assert_ok!(Acurast::<T>::register(
        RawOrigin::Signed(consumer.clone()).into(),
        job.clone()
    ));

    Ok((
        processor,
        (
            MultiOrigin::Acurast(consumer),
            Acurast::<T>::job_id_sequence(),
        ),
    ))
}

benchmarks! {
    where_clause {  where
        T: pallet_assets::Config + pallet_acurast::Config,
        RewardFor<T>: From<MockAsset>,
        <T as Config>::AssetId: From<u32>,
        <T as Config>::Balance: From<u128>,
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
        let local_job_id = 1;
    }: {
         pallet_acurast::Pallet::<T>::register(RawOrigin::Signed(caller.clone()).into(), job.clone())?
    }
    verify {
        assert_last_acurast_event::<T>(AcurastEvent::<T>::JobRegistrationStored(
            job, (MultiOrigin::Acurast(caller), local_job_id)
        ).into());
    }

    deregister {
        let (caller, job) = register_helper::<T>(true);
        let local_job_id = 1;
    }: {
         pallet_acurast::Pallet::<T>::deregister(RawOrigin::Signed(caller.clone()).into(), local_job_id)?
    }
    verify {
        assert_last_acurast_event::<T>(AcurastEvent::<T>::JobRegistrationRemoved(
            (MultiOrigin::Acurast(caller), local_job_id)
        ).into());
    }

    acknowledge_match {
        let (processor, job_id) = acknowledge_match_helper::<T>()?;
        let pub_keys: PubKeys = vec![PubKey::SECP256r1([0u8; 33].to_vec().try_into().unwrap()), PubKey::SECP256k1([0u8; 33].to_vec().try_into().unwrap())].try_into().unwrap();
    }: _(RawOrigin::Signed(processor.clone()), job_id.clone(), pub_keys)

    impl_benchmark_test_suite!(AcurastMarketplace, mock::ExtBuilder::default().build(), mock::Test);
}
