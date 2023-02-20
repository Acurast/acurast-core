use frame_support::{
    pallet_prelude::GenesisBuild,
    sp_runtime::{
        generic,
        traits::{AccountIdLookup, BlakeTwo256, ConstU128, ConstU32},
        MultiSignature,
    },
    traits::{
        fungible::{Inspect, Mutate},
        fungibles::{InspectEnumerable, Transfer},
        nonfungibles::{Create, InspectEnumerable as NFTInspectEnumerable},
        AsEnsureOriginWithArg, Everything,
    },
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
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
        Block = Block<Test>,
        NodeBlock = Block<Test>,
        UncheckedExtrinsic = UncheckedExtrinsic<Test>,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        Assets: pallet_assets::{Pallet, Config<T>, Event<T>, Storage},
        Uniques: pallet_uniques::{Pallet, Storage, Event<T>, Call},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        AcurastProcessorManager: crate::{Pallet, Call, Storage, Event<T>},
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
}

impl pallet_uniques::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = u128;
    type ItemId = u128;
    type Currency = Balances;
    type ForceOrigin = EnsureRoot<Self::AccountId>;
    type CreateOrigin =
        AsEnsureOriginWithArg<EnsureRootWithSuccess<Self::AccountId, RootAccountId>>;
    type Locker = ();
    type CollectionDeposit = ConstU128<0>;
    type ItemDeposit = ConstU128<0>;
    type MetadataDepositBase = ConstU128<0>;
    type AttributeDepositBase = ConstU128<0>;
    type DepositPerByte = ConstU128<0>;
    type StringLimit = ConstU32<256>;
    type KeyLimit = ConstU32<256>;
    type ValueLimit = ConstU32<256>;
    type WeightInfo = pallet_uniques::weights::SubstrateWeight<Self>;
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Proof = MultiSignature;
    type ManagerId = AssetId;
    type ManagerIdProvider = AcurastManagerIdProvider;
    type ProcessorAssetRecovery = AcurastProcessorAssetRecovery;
    type MaxPairingUpdates = ConstU32<5>;
    type Counter = u64;
    type PairingProofExpirationTime = ConstU128<600000>;
    type UnixTime = pallet_timestamp::Pallet<Test>;
    type WeightInfo = ();
}

pub struct AcurastManagerIdProvider;
impl ManagerIdProvider<Test> for AcurastManagerIdProvider {
    fn create_manager_id(
        id: <Test as Config>::ManagerId,
        owner: &<Test as frame_system::Config>::AccountId,
    ) -> frame_support::pallet_prelude::DispatchResult {
        if Uniques::collection_owner(0).is_none() {
            Uniques::create_collection(&0, &alice_account_id(), &alice_account_id())?;
        }
        Uniques::do_mint(0, id, owner.clone(), |_| Ok(()))
    }

    fn manager_id_for(
        owner: &<Test as frame_system::Config>::AccountId,
    ) -> Result<<Test as Config>::ManagerId, frame_support::sp_runtime::DispatchError> {
        Uniques::owned_in_collection(&0, owner).nth(0).ok_or(
            frame_support::pallet_prelude::DispatchError::Other("Manager ID not found"),
        )
    }

    fn owner_for(
        manager_id: <Test as Config>::ManagerId,
    ) -> Result<<Test as frame_system::Config>::AccountId, frame_support::sp_runtime::DispatchError>
    {
        Uniques::owner(0, manager_id).ok_or(frame_support::pallet_prelude::DispatchError::Other(
            "Onwer for provided Manager ID not found",
        ))
    }
}

pub struct AcurastProcessorAssetRecovery;
impl ProcessorAssetRecovery<Test> for AcurastProcessorAssetRecovery {
    fn recover_assets(
        processor: &<Test as frame_system::Config>::AccountId,
        destination_account: &<Test as frame_system::Config>::AccountId,
    ) -> frame_support::pallet_prelude::DispatchResult {
        let usable_balance = Balances::reducible_balance(processor, true);
        if usable_balance > 0 {
            let burned = Balances::burn_from(processor, usable_balance)?;
            Balances::mint_into(destination_account, burned)?;
        }

        let ids = Assets::asset_ids();
        for id in ids {
            let balance = Assets::balance(id, processor);
            if balance > 0 {
                <Assets as Transfer<<Test as frame_system::Config>::AccountId>>::transfer(
                    id,
                    &processor,
                    &destination_account,
                    balance,
                    false,
                )?;
            }
        }
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
