use acurast_common::{Fulfillment, Script};
use frame_support::sp_runtime::DispatchError;
use frame_support::{parameter_types, sp_runtime, traits::Everything, weights::Weight, PalletId};
use hex_literal::hex;
use sp_runtime::traits::{AccountIdLookup, BlakeTwo256};
use sp_runtime::{generic, AccountId32};

use crate::traits::OnFulfillment;

type AccountId = AccountId32;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
pub type BlockNumber = u32;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
        AcurastFulfillmentReceiver: crate::{Pallet, Call, Event<T>}
    }
);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
}
parameter_types! {
    pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
    pub const MinimumPeriod: u64 = 6000;
    pub AllowedFulfillAccounts: Vec<AccountId> = vec![bob_account_id()];
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
    type AccountData = ();
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

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type OnFulfillment = FulfillmentHandler;
    type WeightInfo = ();
}

pub struct FulfillmentHandler;
impl OnFulfillment<Test> for FulfillmentHandler {
    fn on_fulfillment(
        from: <Test as frame_system::Config>::AccountId,
        _fulfillment: crate::Fulfillment,
    ) -> sp_runtime::DispatchResultWithInfo<frame_support::dispatch::PostDispatchInfo> {
        if !AllowedFulfillAccounts::get().contains(&from) {
            return Err(DispatchError::BadOrigin.into());
        }
        Ok(().into())
    }
}

pub fn alice_account_id() -> AccountId {
    [0; 32].into()
}

pub fn bob_account_id() -> AccountId {
    [1; 32].into()
}

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

pub const SCRIPT_BYTES: [u8; 53] = hex!("697066733A2F2F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

pub fn script() -> Script {
    SCRIPT_BYTES.to_vec().try_into().unwrap()
}

pub fn fulfillment_for(script: Script) -> Fulfillment {
    Fulfillment {
        script,
        payload: hex!("00").to_vec(),
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
