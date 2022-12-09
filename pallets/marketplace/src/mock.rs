use frame_support::pallet_prelude::*;
use frame_support::traits::Everything;
use frame_support::weights::Weight;
use frame_support::{pallet_prelude::GenesisBuild, PalletId};
use hex_literal::hex;
use sp_core::*;
use sp_io;
use sp_runtime::traits::{
    AccountIdConversion, AccountIdLookup, BlakeTwo256, ConstU128, ConstU32, StaticLookup,
};
use sp_runtime::{bounded_vec, BoundedVec};
use sp_runtime::{generic, parameter_types, Percent};
use sp_std::prelude::*;

use pallet_acurast::Script;
use pallet_acurast::{
    CertificateRevocationListUpdate, Fulfillment, FulfillmentRouter, JobAssignmentUpdate,
    JobAssignmentUpdateBarrier, JobRegistrationFor, RevocationListUpdateBarrier,
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

impl JobAssignmentUpdateBarrier<Test> for Barrier {
    fn can_update_assigned_jobs(
        origin: &<Test as frame_system::Config>::AccountId,
        updates: &Vec<JobAssignmentUpdate<<Test as frame_system::Config>::AccountId>>,
    ) -> bool {
        updates.iter().all(|update| &update.job_id.0 == origin)
    }
}

impl AssetBarrier<MockAsset> for Barrier {
    fn can_use_asset(_asset: &MockAsset) -> bool {
        true
    }
}

pub struct Router;

impl FulfillmentRouter<Test> for Router {
    fn received_fulfillment(
        _origin: frame_system::pallet_prelude::OriginFor<Test>,
        _from: <Test as frame_system::Config>::AccountId,
        _fulfillment: Fulfillment,
        _registration: JobRegistrationFor<Test>,
        _requester: <<Test as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Target,
    ) -> frame_support::pallet_prelude::DispatchResultWithPostInfo {
        Ok(().into())
    }
}

pub struct FeeManagerImpl;

impl FeeManager for FeeManagerImpl {
    fn get_fee_percentage() -> Percent {
        Percent::from_percent(30)
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
                (pallet_assets_account(), INITIAL_BALANCE),
                (pallet_fees_account(), INITIAL_BALANCE),
                (bob_account_id(), INITIAL_BALANCE),
                (processor_account_id(), INITIAL_BALANCE),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        // give alice an initial balance of token 22 (backed by statemint) to pay for a job
        // get the MockAsset representing token 22 with owned_asset()
        pallet_assets::GenesisConfig::<Test> {
            assets: vec![(22, pallet_assets_account(), false, 1_000)],
            metadata: vec![(22, "test_payment".into(), "tpt".into(), 12.into())],
            accounts: vec![
                (22, alice_account_id(), INITIAL_BALANCE),
                (22, bob_account_id(), INITIAL_BALANCE),
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
        Assets: pallet_assets::{Pallet, Config<T>, Event<T>, Storage},
        ParachainInfo: parachain_info::{Pallet, Storage, Config},
        Acurast: pallet_acurast::{Pallet, Call, Storage, Event<T>},
        AcurastMarketplace: crate::{Pallet, Call, Storage, Event<T>}
    }
);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
}
parameter_types! {
    pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
    pub const MinimumPeriod: u64 = 6000;
    pub AllowedRevocationListUpdate: Vec<AccountId> = vec![alice_account_id(), <Test as crate::Config>::PalletId::get().into_account_truncating()];
    pub AllowedJobAssignmentUpdate: Vec<AccountId> = vec![bob_account_id()];
    pub const ExistentialDeposit: AssetAmount = EXISTENTIAL_DEPOSIT;
}
parameter_types! {
    pub const MaxReserves: u32 = 50;
    pub const MaxLocks: u32 = 50;
}
parameter_types! {
    pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
}

impl frame_system::Config for Test {
    type Call = Call;
    type Index = u32;
    type BlockNumber = BlockNumber;
    type Hash = sp_core::H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    type Event = Event;
    type Origin = Origin;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<AssetAmount>;
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
    type Balance = AssetAmount;
    type DustRemoval = ();
    /// The ubiquitous event type.
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
}

impl pallet_assets::Config for Test {
    type Event = Event;
    type Balance = AssetAmount;
    type AssetId = AssetId;
    type Currency = Balances;
    type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type AssetDeposit = ConstU128<0>;
    type AssetAccountDeposit = ConstU128<0>;
    type MetadataDepositBase = ConstU128<{ UNIT }>;
    type MetadataDepositPerByte = ConstU128<{ 10 * MICROUNIT }>;
    type ApprovalDeposit = ConstU128<{ 10 * MICROUNIT }>;
    type StringLimit = ConstU32<50>;
    type Freezer = ();
    type Extra = ();
    type WeightInfo = ();
}

impl parachain_info::Config for Test {}

impl pallet_acurast::Config for Test {
    type Event = Event;
    type RegistrationExtra = JobRequirementsFor<Self>;
    type FulfillmentRouter = Router;
    type MaxAllowedSources = frame_support::traits::ConstU16<4>;
    type PalletId = AcurastPalletId;
    type RevocationListUpdateBarrier = Barrier;
    type JobAssignmentUpdateBarrier = Barrier;
    type UnixTime = pallet_timestamp::Pallet<Test>;
    type JobHooks = Pallet<Test>;
    type WeightInfo = pallet_acurast::weights::WeightInfo<Test>;
}

pub struct MockRewardManager {}

impl<T: Config> RewardManager<T> for MockRewardManager {
    type Reward = MockAsset;

    fn lock_reward(
        _reward: Self::Reward,
        _owner: <<T>::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn pay_reward(
        _reward: Self::Reward,
        _target: <<T>::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}

impl Config for Test {
    type Event = Event;
    type RegistrationExtra = JobRequirementsFor<Self>;
    type PalletId = AcurastPalletId;
    type AssetId = AssetId;
    type AssetAmount = AssetAmount;
    type RewardManager = MockRewardManager;
    type WeightInfo = weights::Weights<Test>;
}

pub fn events() -> Vec<Event> {
    let evt = System::events()
        .into_iter()
        .map(|evt| evt.event)
        .collect::<Vec<_>>();

    System::reset_events();

    evt
}

pub fn fulfillment_for(registration: &JobRegistrationFor<Test>) -> Fulfillment {
    Fulfillment {
        script: registration.script.clone(),
        payload: hex!("00").to_vec(),
    }
}

pub fn pallet_assets_account() -> <Test as frame_system::Config>::AccountId {
    <Test as Config>::PalletId::get().into_account_truncating()
}

pub fn pallet_fees_account() -> <Test as frame_system::Config>::AccountId {
    FeeManagerImpl::pallet_id().into_account_truncating()
}

pub fn advertisement(
    price_per_cpu_millisecond: u128,
    capacity: u32,
    allowed_consumers: Option<Vec<<Test as frame_system::Config>::AccountId>>,
) -> AdvertisementFor<Test> {
    let pricing: BoundedVec<PricingVariant<AssetId, AssetAmount>, ConstU32<MAX_PRICING_VARIANTS>> =
        bounded_vec![PricingVariant {
            reward_asset: 0,
            price_per_cpu_millisecond,
            bonus: 0,
            maximum_slash: 0,
        }];
    Advertisement {
        pricing,
        allowed_consumers,
        capacity,
    }
}

pub fn job_registration_with_reward(
    script: Script,
    cpu_milliseconds: u128,
    reward_value: u128,
    min_reputation: Option<u128>,
) -> JobRegistrationFor<Test> {
    JobRegistrationFor::<Test> {
        script,
        allowed_sources: None,
        allow_only_verified_sources: false,
        extra: JobRequirements {
            slots: 1,
            cpu_milliseconds,
            reward: asset(reward_value),
            min_reputation,
        },
    }
}
