use frame_support::{
    dispatch::Weight,
    pallet_prelude::GenesisBuild,
    parameter_types,
    traits::{AsEnsureOriginWithArg, Everything},
    PalletId,
};
use sp_core::*;
use sp_io;
use sp_runtime::traits::{
    AccountIdConversion, AccountIdLookup, BlakeTwo256, ConstU128, ConstU32, StaticLookup,
};
use sp_runtime::{bounded_vec, BoundedVec, DispatchError};
use sp_runtime::{generic, Percent};
use sp_std::prelude::*;

use pallet_acurast::{CertificateRevocationListUpdate, RevocationListUpdateBarrier};

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

impl AssetBarrier<MockAsset> for Barrier {
    fn can_use_asset(_asset: &MockAsset) -> bool {
        true
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
        AcurastMarketplace: crate::{Pallet, Call, Storage, Event<T>},
        MockPallet: mock_pallet::{Pallet, Event<T>}
    }
);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
}
parameter_types! {
    pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
    pub const MinimumPeriod: u64 = 6000;
    pub AllowedRevocationListUpdate: Vec<AccountId> = vec![alice_account_id(), <Test as crate::Config>::PalletId::get().into_account_truncating()];
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
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
}

impl pallet_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = AssetAmount;
    type AssetId = AssetId;
    type AssetIdParameter = codec::Compact<AssetId>;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
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
    type RemoveItemsLimit = ();
}

impl parachain_info::Config for Test {}

impl pallet_acurast::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RegistrationExtra = JobRequirementsFor<Self>;
    type MaxAllowedSources = frame_support::traits::ConstU16<4>;
    type PalletId = AcurastPalletId;
    type RevocationListUpdateBarrier = Barrier;
    type KeyAttestationBarrier = ();
    type UnixTime = pallet_timestamp::Pallet<Test>;
    type JobHooks = Pallet<Test>;
    type WeightInfo = pallet_acurast::weights::WeightInfo<Test>;
}

impl mock_pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
}

#[frame_support::pallet]
pub mod mock_pallet {
    use frame_support::pallet_prelude::*;

    use crate::stub::MockAsset;

    #[pallet::config]
    pub trait Config: frame_system::Config + crate::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        Locked(MockAsset),
        PayReward(MockAsset),
        PayMatcherReward(MockAsset),
    }
}

pub struct MockRewardManager {}

impl<T: Config + mock_pallet::Config> RewardManager<T> for MockRewardManager {
    type Reward = MockAsset;

    fn lock_reward(
        reward: Self::Reward,
        _owner: <<T>::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::Locked(reward));
        Ok(())
    }

    fn pay_reward(
        reward: Self::Reward,
        _target: <<T>::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::PayReward(reward));
        Ok(())
    }

    fn pay_matcher_reward(
        reward: Self::Reward,
        _matcher: <<T>::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::PayMatcherReward(reward));
        Ok(())
    }
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RegistrationExtra = JobRequirementsFor<Self>;
    type PalletId = AcurastPalletId;
    type AssetId = AssetId;
    type AssetAmount = AssetAmount;
    type RewardManager = MockRewardManager;
    type WeightInfo = weights::Weights<Test>;
}

pub fn events() -> Vec<RuntimeEvent> {
    let evt = System::events()
        .into_iter()
        .map(|evt| evt.event)
        .collect::<Vec<_>>();

    System::reset_events();

    evt
}

pub fn pallet_assets_account() -> <Test as frame_system::Config>::AccountId {
    <Test as Config>::PalletId::get().into_account_truncating()
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
    let pricing: BoundedVec<PricingVariant<AssetId, AssetAmount>, ConstU32<MAX_PRICING_VARIANTS>> =
        bounded_vec![PricingVariant {
            reward_asset: 0,
            fee_per_millisecond,
            fee_per_storage_byte,
            base_fee_per_execution: 0,
            scheduling_window: SchedulingWindow::Delta(2_628_000_000), // 1 month
        }];
    Advertisement {
        pricing,
        allowed_consumers: None,
        storage_capacity,
        max_memory,
        network_request_quota,
    }
}
