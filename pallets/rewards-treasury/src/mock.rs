use frame_support::parameter_types;
use frame_support::{
    sp_runtime::{
        generic,
        traits::{AccountIdLookup, BlakeTwo256},
    },
    traits::Everything,
};
use sp_std::prelude::*;

use crate::stub::*;
use crate::*;

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let t = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
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
        RewardsTreasury: crate::{Pallet, Storage, Event<T>}
    }
);

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

parameter_types! {
    pub const ExistentialDeposit: Balance = MICROUNIT;
    pub const MaxReserves: u32 = 50;
    pub const MaxLocks: u32 = 50;
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

parameter_types! {
    pub const Epoch: BlockNumber = 5;
    pub const Treasury: AccountId = AccountId::new([7u8; 32]);
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Epoch = Epoch;
    type Treasury = Treasury;
}

pub fn events() -> Vec<RuntimeEvent> {
    let evt = System::events()
        .into_iter()
        .map(|evt| evt.event)
        .collect::<Vec<_>>();

    System::reset_events();

    evt
}
