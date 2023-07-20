use frame_support::{pallet_prelude::GenesisBuild, parameter_types, traits::Everything, PalletId};
use sp_core::*;
use sp_io;
use sp_runtime::traits::{AccountIdConversion, AccountIdLookup, BlakeTwo256};
use sp_runtime::DispatchError;
use sp_runtime::{generic, Percent};
use sp_std::prelude::*;
use std::marker::PhantomData;

use pallet_acurast::{
    CertificateRevocationListUpdate, JobId, JobModules, RevocationListUpdateBarrier, CU32,
};

use crate::stub::*;
use crate::*;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub struct Barrier;

impl RevocationListUpdateBarrier<Test> for Barrier {
    fn can_update_revocation_list(
        origin: &<Test as frame_system::Config>::AccountId,
        _updates: &Vec<CertificateRevocationListUpdate>,
    ) -> bool {
        AllowedRevocationListUpdate::get().contains(origin)
    }
}

pub struct FeeManagerImpl;

impl FeeManager for FeeManagerImpl {
    fn get_fee_percentage() -> Percent {
        Percent::from_percent(30)
    }

    fn get_matcher_percentage() -> Percent {
        Percent::from_percent(10)
    }

    fn pallet_id() -> PalletId {
        PalletId(*b"acurfees")
    }
}

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();

        let parachain_info_config = parachain_info::GenesisConfig {
            parachain_id: 2000.into(),
        };

        <parachain_info::GenesisConfig as GenesisBuild<Test, _>>::assimilate_storage(
            &parachain_info_config,
            &mut t,
        )
        .unwrap();

        pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (alice_account_id(), INITIAL_BALANCE),
                (pallet_fees_account(), INITIAL_BALANCE),
                (bob_account_id(), INITIAL_BALANCE),
                (processor_account_id(), INITIAL_BALANCE),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| System::set_block_number(1));
        ext
    }
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {}
    }
}

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        ParachainInfo: parachain_info::{Pallet, Storage, Config},
        Acurast: pallet_acurast::{Pallet, Call, Storage, Event<T>},
        AcurastMarketplace: crate::{Pallet, Call, Storage, Event<T>},
        MockPallet: mock_pallet::{Pallet, Event<T>}
    }
);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
}
parameter_types! {
    pub const MinimumPeriod: u64 = 2000;
    pub AllowedRevocationListUpdate: Vec<AccountId> = vec![alice_account_id(), <Test as crate::Config>::PalletId::get().into_account_truncating()];
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}
parameter_types! {
    pub const MaxReserves: u32 = 50;
    pub const MaxLocks: u32 = 50;
    pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
    pub const ReportTolerance: u64 = 12000;
}

impl frame_system::Config for Test {
    type RuntimeCall = RuntimeCall;
    type Index = u32;
    type BlockNumber = BlockNumber;
    type Hash = sp_core::H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type DbWeight = ();
    type BaseCallFilter = Everything;
    type SystemWeightInfo = ();
    type BlockWeights = ();
    type BlockLength = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl pallet_balances::Config for Test {
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
}

impl parachain_info::Config for Test {}

impl pallet_acurast::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RegistrationExtra = JobRequirementsFor<Self>;
    type MaxAllowedSources = CU32<4>;
    type MaxCertificateRevocationListUpdates = frame_support::traits::ConstU32<10>;
    type PalletId = AcurastPalletId;
    type RevocationListUpdateBarrier = Barrier;
    type KeyAttestationBarrier = ();
    type UnixTime = pallet_timestamp::Pallet<Test>;
    type JobHooks = Pallet<Test>;
    type WeightInfo = pallet_acurast::weights::WeightInfo<Test>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = TestBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct TestBenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl pallet_acurast::benchmarking::BenchmarkHelper<Test> for TestBenchmarkHelper {
    fn registration_extra() -> <Test as pallet_acurast::Config>::RegistrationExtra {
        JobRequirements {
            slots: 1,
            reward: 1,
            min_reputation: None,
            instant_match: None,
        }
    }

    fn funded_account(index: u32) -> AccountId {
        let caller: AccountId = frame_benchmarking::account("token_account", index, SEED);
        <Balances as frame_support::traits::Currency<_>>::make_free_balance_be(
            &caller,
            u32::MAX.into(),
        );

        caller
    }
}

impl mock_pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
}

#[frame_support::pallet]
pub mod mock_pallet {
    use frame_support::pallet_prelude::*;
    use pallet_acurast::JobId;

    use crate::stub::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config + crate::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        LockReward((JobId<T::AccountId>, Balance)),
        PayReward((JobId<T::AccountId>, Balance, T::AccountId)),
        PayMatcherReward((Vec<(JobId<T::AccountId>, T::Balance)>, T::AccountId)),
        RefundReward((JobId<T::AccountId>, T::Balance)),
    }
}

pub struct MockRewardManager<Budget>(PhantomData<Budget>);

impl<T: Config + mock_pallet::Config, Budget: JobBudget<T>> RewardManager<T>
    for MockRewardManager<Budget>
{
    fn lock_reward(
        job_id: &JobId<T::AccountId>,
        reward: <T as Config>::Balance,
    ) -> Result<(), DispatchError> {
        mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::LockReward((
            job_id.clone(),
            reward.into(),
        )));
        Budget::reserve(job_id, reward).unwrap();
        Ok(())
    }

    fn pay_reward(
        job_id: &JobId<T::AccountId>,
        reward: <T as Config>::Balance,
        target: &T::AccountId,
    ) -> Result<(), DispatchError> {
        mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::PayReward((
            job_id.clone(),
            reward.into(),
            target.clone(),
        )));
        Budget::unreserve(job_id, reward).unwrap();
        Ok(())
    }

    fn pay_matcher_reward(
        remaining_rewards: Vec<(JobId<T::AccountId>, <T as Config>::Balance)>,
        matcher: &T::AccountId,
    ) -> Result<(), DispatchError> {
        mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::PayMatcherReward((
            remaining_rewards.clone(),
            matcher.clone(),
        )));

        let mut matcher_reward: T::Balance = 0u8.into();
        for (job_id, remaining_reward) in remaining_rewards.into_iter() {
            let matcher_fee = FeeManagerImpl::get_matcher_percentage().mul_floor(remaining_reward);
            Budget::unreserve(&job_id, matcher_fee)
                .map_err(|_| DispatchError::Other("Severe Error: JobBudget::unreserve failed"))?;
            matcher_reward += matcher_fee;
        }

        Ok(())
    }

    fn refund(job_id: &JobId<T::AccountId>) -> T::Balance {
        let remaining = Budget::unreserve_remaining(job_id);
        mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::RefundReward((
            job_id.clone(),
            remaining,
        )));
        remaining
    }
}

pub struct ManagerOf;

impl ManagerProvider<Test> for ManagerOf {
    fn manager_of(
        owner: &<Test as frame_system::Config>::AccountId,
    ) -> Result<<Test as frame_system::Config>::AccountId, DispatchError> {
        Ok(owner.clone())
    }
}

pub struct ProcessorLastSeenProvider;

impl crate::traits::ProcessorLastSeenProvider<Test> for ProcessorLastSeenProvider {
    fn last_seen(_processor: &<Test as frame_system::Config>::AccountId) -> Option<u128> {
        Some(AcurastMarketplace::now().unwrap().into())
    }
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxAllowedConsumers = pallet_acurast::CU32<4>;
    type MaxProposedMatches = frame_support::traits::ConstU32<10>;
    type RegistrationExtra = JobRequirementsFor<Self>;
    type PalletId = AcurastPalletId;
    type ReportTolerance = ReportTolerance;
    type Balance = Balance;
    type ManagerProvider = ManagerOf;
    type RewardManager = MockRewardManager<Pallet<Self>>;
    type ProcessorLastSeenProvider = ProcessorLastSeenProvider;
    type MarketplaceHooks = ();
    type WeightInfo = weights::Weights<Test>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = TestBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
impl crate::benchmarking::BenchmarkHelper<Test> for TestBenchmarkHelper {
    fn registration_extra(r: JobRequirementsFor<Test>) -> <Test as Config>::RegistrationExtra {
        r
    }

    fn funded_account(index: u32, amount: Balance) -> AccountId {
        let caller: AccountId = frame_benchmarking::account("token_account", index, SEED);
        <Balances as frame_support::traits::Currency<_>>::make_free_balance_be(&caller, amount);

        caller
    }
}

pub fn events() -> Vec<RuntimeEvent> {
    let evt = System::events()
        .into_iter()
        .map(|evt| evt.event)
        .collect::<Vec<_>>();

    System::reset_events();

    evt
}

pub fn pallet_fees_account() -> <Test as frame_system::Config>::AccountId {
    FeeManagerImpl::pallet_id().into_account_truncating()
}

pub fn advertisement(
    fee_per_millisecond: u128,
    fee_per_storage_byte: u128,
    storage_capacity: u32,
    max_memory: u32,
    network_request_quota: u8,
) -> AdvertisementFor<Test> {
    Advertisement {
        pricing: Pricing {
            fee_per_millisecond,
            fee_per_storage_byte,
            base_fee_per_execution: 0,
            scheduling_window: SchedulingWindow::Delta(2_628_000_000), // 1 month
        },
        allowed_consumers: None,
        storage_capacity,
        max_memory,
        network_request_quota,
        available_modules: JobModules::default(),
    }
}
