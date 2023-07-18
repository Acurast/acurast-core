use std::marker::PhantomData;

use frame_support::parameter_types;
use frame_support::{
    sp_runtime::{
        generic,
        traits::{AccountIdLookup, BlakeTwo256},
    },
    traits::Everything,
};
use sp_runtime::DispatchError;
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
        AcurastVesting: crate::{Pallet, Call, Storage, Event<T>},
        MockPallet: mock_pallet::{Pallet, Event<T>}
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
    pub const DivestTolerance: BlockNumber = 2;
    pub const MaximumLockingPeriod: BlockNumber = 100;
    pub const BalanceUnit: u128 = UNIT;
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type DivestTolerance = DivestTolerance;
    type MaximumLockingPeriod = MaximumLockingPeriod;
    type Balance = Balance;
    type BalanceUnit = BalanceUnit;
    type BlockNumber = BlockNumber;
    type VestingBalance = MockVestingBalance<Self>;
    type WeightInfo = ();
}

impl mock_pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
}

#[frame_support::pallet]
pub mod mock_pallet {
    use frame_support::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + crate::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        LockStake(T::AccountId, T::Balance),
        PayAccrued(T::AccountId, T::Balance),
        PayKicker(T::AccountId, T::Balance),
        UnlockStake(T::AccountId, T::Balance),
    }
}

pub struct MockVestingBalance<T>(PhantomData<T>);

impl<T: Config + mock_pallet::Config> VestingBalance<T::AccountId, T::Balance>
    for MockVestingBalance<T>
{
    fn lock_stake(
        target: &T::AccountId,
        stake: <T as Config>::Balance,
    ) -> Result<(), DispatchError> {
        mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::LockStake(
            target.clone(),
            stake,
        ));
        Ok(())
    }

    fn pay_accrued(
        target: &T::AccountId,
        accrued: <T as Config>::Balance,
    ) -> Result<(), DispatchError> {
        mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::PayAccrued(
            target.clone(),
            accrued,
        ));
        Ok(())
    }

    fn pay_kicker(
        target: &T::AccountId,
        accrued: <T as Config>::Balance,
    ) -> Result<(), DispatchError> {
        mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::PayKicker(
            target.clone(),
            accrued,
        ));
        Ok(())
    }

    fn unlock_stake(
        target: &T::AccountId,
        stake: <T as Config>::Balance,
    ) -> Result<(), DispatchError> {
        mock_pallet::Pallet::deposit_event(mock_pallet::Event::<T>::UnlockStake(
            target.clone(),
            stake,
        ));
        Ok(())
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
