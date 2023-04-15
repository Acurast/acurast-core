use frame_support::{
    dispatch::Weight, parameter_types, traits::ConstU32,
    weights::constants::RocksDbWeight as DbWeight,
};
use sp_core::H256;
use sp_runtime::traits::AccountIdLookup;
use sp_runtime::{
    generic,
    traits::{BlakeTwo256, Keccak256},
};

use stub::*;

use crate::tezos::TezosEncoder;
use crate::*;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block<Test>,
        NodeBlock = Block<Test>,
        UncheckedExtrinsic = UncheckedExtrinsic<Test>,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
        HyperdriveOutgoing: crate::{Pallet, Storage, Event<T>},
    }
);

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = BlockNumber;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = AccountIdLookup<AccountId, ()>;
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    const INDEXING_PREFIX: &'static [u8] = b"mmr-tez-";
    type Hasher = Keccak256;
    type Hash = H256;
    type OnNewRoot = ();
    type WeightInfo = ();
    type MaximumBlocksBeforeSnapshot = MaximumBlocksBeforeSnapshot;
}

impl WeightInfo for () {
    fn send_message() -> Weight {
        DbWeight::get().reads_writes(3, 3)
    }
}

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;

    pub const MaximumBlocksBeforeSnapshot: u64 = 2;
}
