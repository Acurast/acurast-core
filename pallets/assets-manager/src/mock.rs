use frame_support::{
    pallet_prelude::GenesisBuild,
    sp_runtime::{
        generic,
        traits::{AccountIdLookup, BlakeTwo256, ConstU128, ConstU32},
    },
    traits::{AsEnsureOriginWithArg, Everything},
};
use sp_std::prelude::*;

use crate::stub::*;
use crate::*;

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();

        pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (alice_account_id(), INITIAL_BALANCE),
                (pallet_assets_account(), INITIAL_BALANCE),
                (bob_account_id(), INITIAL_BALANCE),
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
        Block = Block<Test>,
        NodeBlock = Block<Test>,
        UncheckedExtrinsic = UncheckedExtrinsic<Test>,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        Assets: pallet_assets::{Pallet, Config<T>, Event<T>, Storage},
        AcurastAssetManager: crate::{Pallet, Call, Storage, Event<T>},
    }
);

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

impl frame_system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u32;
    type BlockNumber = BlockNumber;
    type Hash = sp_core::H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type AssetId = AssetId;
    type AssetIdParameter = codec::Compact<AssetId>;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
    type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type AssetDeposit = ConstU128<0>;
    type AssetAccountDeposit = ConstU128<0>;
    type MetadataDepositBase = ConstU128<0>;
    type MetadataDepositPerByte = ConstU128<0>;
    type ApprovalDeposit = ConstU128<0>;
    type StringLimit = ConstU32<50>;
    type Freezer = ();
    type Extra = ();
    type WeightInfo = ();
    type RemoveItemsLimit = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = TestBenchmarkHelper;
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ManagerOrigin = frame_system::EnsureSigned<Self::AccountId>;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = TestBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct TestBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_assets::BenchmarkHelper<<Test as pallet_assets::Config>::AssetIdParameter>
    for TestBenchmarkHelper
{
    fn create_asset_id_parameter(id: u32) -> <Test as pallet_assets::Config>::AssetIdParameter {
        codec::Compact(id.into())
    }
}

#[cfg(feature = "runtime-benchmarks")]
impl crate::benchmarking::BenchmarkHelper<Test> for TestBenchmarkHelper {
    fn manager_account() -> <Test as frame_system::Config>::AccountId {
        alice_account_id()
    }
}
