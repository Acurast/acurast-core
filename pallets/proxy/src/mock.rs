use std::marker::PhantomData;

use frame_support::traits::OriginTrait;
use scale_info::TypeInfo;
use sp_core::*;
use xcm::latest::{Junction, MultiLocation, OriginKind};
use xcm::prelude::*;
use xcm_executor::traits::ConvertOrigin;

use pallet_acurast_marketplace::Reward;

pub type AcurastAssetId = u32;
pub type AcurastAssetAmount = u128;

#[derive(Clone, Eq, PartialEq, Debug, Encode, Decode, TypeInfo)]
pub struct AcurastAsset(pub MultiAsset);

impl Reward for AcurastAsset {
    type AssetId = AcurastAssetId;
    type Balance = AcurastAssetAmount;
    type Error = ();

    fn with_amount(&mut self, amount: Self::Balance) -> Result<&Self, Self::Error> {
        self.0 = MultiAsset {
            id: self.0.id.clone(),
            fun: Fungible(amount),
        };
        Ok(self)
    }

    fn try_get_asset_id(&self) -> Result<Self::AssetId, Self::Error> {
        match &self.0.id {
            Concrete(location) => match location.last() {
                Some(GeneralIndex(id)) => (*id).try_into().map_err(|_| ()),
                _ => Err(()),
            },
            Abstract(_) => Err(()),
        }
    }

    fn try_get_amount(&self) -> Result<Self::Balance, Self::Error> {
        match &self.0.fun {
            Fungible(amount) => Ok(*amount),
            _ => Err(()),
        }
    }
}

pub mod acurast_runtime {
    use frame_support::{
        construct_runtime, parameter_types,
        sp_runtime::{testing::Header, traits::AccountIdLookup, AccountId32},
        traits::{Everything, Nothing},
        PalletId,
    };
    use pallet_xcm::XcmPassthrough;
    use polkadot_parachain::primitives::Sibling;
    use sp_core::*;
    use sp_std::prelude::*;
    use xcm::latest::prelude::*;
    use xcm_builder::{
        AccountId32Aliases, AllowUnpaidExecutionFrom, CurrencyAdapter as XcmCurrencyAdapter,
        EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds, IsConcrete, LocationInverter,
        NativeAsset, ParentIsPreset, SiblingParachainConvertsVia, SignedAccountId32AsNative,
        SignedToAccountId32, SovereignSignedViaLocation,
    };
    use xcm_executor::XcmExecutor;

    pub use pallet_acurast;
    use pallet_acurast::JobAssignmentUpdateBarrier;
    pub use pallet_acurast_marketplace;
    use pallet_acurast_marketplace::{AssetBarrier, AssetRewardManager, JobRequirements};

    use crate::mock::{AcurastAsset, AcurastAssetAmount, AcurastAssetId};

    pub type AccountId = AccountId32;
    pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, RelayNetwork>;
    pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
    pub type Block = frame_system::mocking::MockBlock<Runtime>;
    pub type LocationToAccountId = (
        ParentIsPreset<AccountId>,
        SiblingParachainConvertsVia<Sibling, AccountId>,
        AccountId32Aliases<RelayNetwork, AccountId>,
    );
    pub type LocalAssetTransactor =
        XcmCurrencyAdapter<Balances, IsConcrete<KsmLocation>, LocationToAccountId, AccountId, ()>;
    pub type XcmRouter = crate::tests::ParachainXcmRouter<MsgQueue>;
    pub type Barrier = AllowUnpaidExecutionFrom<Everything>;
    pub type XcmOriginToCallOrigin = (
        SovereignSignedViaLocation<LocationToAccountId, Origin>,
        SignedAccountId32AsNative<RelayNetwork, Origin>,
        // TODO: safety check of signature
        super::SignedAccountId32FromXcm<Origin>,
        XcmPassthrough<Origin>,
    );

    pub struct FulfillmentRouter;

    impl pallet_acurast::FulfillmentRouter<Runtime> for FulfillmentRouter {
        fn received_fulfillment(
            _origin: frame_system::pallet_prelude::OriginFor<Runtime>,
            _from: <Runtime as frame_system::Config>::AccountId,
            _fulfillment: pallet_acurast::Fulfillment,
            _registration: pallet_acurast::JobRegistrationFor<Runtime>,
            _requester: <<Runtime as frame_system::Config>::Lookup as frame_support::sp_runtime::traits::StaticLookup>::Target,
        ) -> frame_support::pallet_prelude::DispatchResultWithPostInfo {
            Ok(().into())
        }
    }

    pub struct AcurastBarrier;

    impl JobAssignmentUpdateBarrier<Runtime> for AcurastBarrier {
        fn can_update_assigned_jobs(
            origin: &<Runtime as frame_system::Config>::AccountId,
            updates: &Vec<
                pallet_acurast::JobAssignmentUpdate<<Runtime as frame_system::Config>::AccountId>,
            >,
        ) -> bool {
            updates.iter().all(|update| &update.job_id.0 == origin)
        }
    }

    impl AssetBarrier<AcurastAsset> for AcurastBarrier {
        fn can_use_asset(_asset: &AcurastAsset) -> bool {
            true
        }
    }

    pub const MILLISECS_PER_BLOCK: u64 = 12000;
    pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;
    pub const UNIT: AcurastAssetAmount = 1_000_000;
    pub const MICROUNIT: AcurastAssetAmount = 1;

    construct_runtime!(
        pub enum Runtime where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system::{Pallet, Call, Storage, Config, Event<T>} = 0,
            Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
            Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
            Assets: pallet_assets::{Pallet, Config<T>, Event<T>, Storage},
            ParachainInfo: parachain_info::{Pallet, Storage, Config},
            MsgQueue: super::mock_msg_queue::{Pallet, Storage, Event<T>},
            PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin},
            Acurast: pallet_acurast::{Pallet, Call, Storage, Event<T>} = 40,
            AcurastMarketplace: pallet_acurast_marketplace::{Pallet, Call, Storage, Event<T>} = 41,
        }
    );

    parameter_types! {
        pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
        pub const IsRelay: bool = false;
        pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
    }
    parameter_types! {
        pub const BlockHashCount: u64 = 250;
    }
    parameter_types! {
        pub ExistentialDeposit: AcurastAssetAmount = 1;
        pub const MaxLocks: u32 = 50;
        pub const MaxReserves: u32 = 50;
    }
    parameter_types! {
        pub const KsmLocation: MultiLocation = MultiLocation::parent();
        pub const RelayNetwork: NetworkId = NetworkId::Kusama;
        pub Ancestry: MultiLocation = Parachain(MsgQueue::parachain_id().into()).into();
    }
    parameter_types! {
        pub const UnitWeightCost: u64 = 1;
        pub KsmPerSecond: (AssetId, u128) = (Concrete(Parent.into()), 1);
        pub const MaxInstructions: u32 = 100;
    }

    pub struct XcmConfig;

    impl xcm_executor::Config for XcmConfig {
        type Call = Call;
        type XcmSender = XcmRouter;
        type AssetTransactor = LocalAssetTransactor;
        type OriginConverter = XcmOriginToCallOrigin;
        type IsReserve = NativeAsset;
        type IsTeleporter = ();
        type LocationInverter = LocationInverter<Ancestry>;
        type Barrier = Barrier;
        type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
        type Trader = FixedRateOfFungible<KsmPerSecond, ()>;
        type ResponseHandler = ();
        type AssetTrap = ();
        type AssetClaims = ();
        type SubscriptionService = ();
    }

    impl pallet_balances::Config for Runtime {
        type Balance = AcurastAssetAmount;
        type DustRemoval = ();
        type Event = Event;
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxLocks = MaxLocks;
        type MaxReserves = MaxReserves;
        type ReserveIdentifier = [u8; 8];
    }

    impl frame_system::Config for Runtime {
        type BaseCallFilter = Everything;
        type BlockWeights = ();
        type BlockLength = ();
        type Origin = Origin;
        type Call = Call;
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = frame_support::sp_runtime::traits::BlakeTwo256;
        type AccountId = AccountId;
        type Lookup = AccountIdLookup<AccountId, ()>;
        type Header = Header;
        type Event = Event;
        type BlockHashCount = BlockHashCount;
        type DbWeight = ();
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = pallet_balances::AccountData<AcurastAssetAmount>;
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = frame_support::traits::ConstU32<16>;
    }

    impl parachain_info::Config for Runtime {}

    impl pallet_timestamp::Config for Runtime {
        type Moment = u64;
        type OnTimestampSet = ();
        type MinimumPeriod = MinimumPeriod;
        type WeightInfo = ();
    }

    impl pallet_assets::Config for Runtime {
        type Event = Event;
        type Balance = AcurastAssetAmount;
        type AssetId = AcurastAssetId;
        type Currency = Balances;
        type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
        type AssetDeposit = frame_support::traits::ConstU128<0>;
        type AssetAccountDeposit = frame_support::traits::ConstU128<0>;
        type MetadataDepositBase = frame_support::traits::ConstU128<{ UNIT }>;
        type MetadataDepositPerByte = frame_support::traits::ConstU128<{ 10 * MICROUNIT }>;
        type ApprovalDeposit = frame_support::traits::ConstU128<{ 10 * MICROUNIT }>;
        type StringLimit = frame_support::traits::ConstU32<50>;
        type Freezer = ();
        type Extra = ();
        type WeightInfo = ();
    }

    pub struct FeeManagerImpl;

    impl pallet_acurast_marketplace::FeeManager for FeeManagerImpl {
        fn get_fee_percentage() -> sp_runtime::Percent {
            sp_runtime::Percent::from_percent(30)
        }

        fn pallet_id() -> PalletId {
            PalletId(*b"acurfees")
        }
    }

    impl pallet_acurast::Config for Runtime {
        type Event = Event;
        type RegistrationExtra = JobRequirements<AcurastAsset>;
        type FulfillmentRouter = FulfillmentRouter;
        type MaxAllowedSources = frame_support::traits::ConstU16<1000>;
        type PalletId = AcurastPalletId;
        type RevocationListUpdateBarrier = ();
        type JobAssignmentUpdateBarrier = AcurastBarrier;
        type UnixTime = pallet_timestamp::Pallet<Runtime>;
        type JobHooks = pallet_acurast_marketplace::Pallet<Runtime>;
        type WeightInfo = pallet_acurast::weights::WeightInfo<Runtime>;
    }

    impl pallet_acurast_marketplace::Config for Runtime {
        type Event = Event;
        type RegistrationExtra = JobRequirements<AcurastAsset>;
        type PalletId = AcurastPalletId;
        type AssetId = AcurastAssetId;
        type AssetAmount = AcurastAssetAmount;
        type RewardManager = AssetRewardManager<AcurastAsset, AcurastBarrier, FeeManagerImpl>;
        type WeightInfo = pallet_acurast_marketplace::weights::WeightInfo<Runtime>;
    }

    impl pallet_xcm::Config for Runtime {
        type Event = Event;
        type SendXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
        type XcmRouter = XcmRouter;
        type ExecuteXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
        type XcmExecuteFilter = Everything;
        type XcmExecutor = XcmExecutor<XcmConfig>;
        type XcmTeleportFilter = Nothing;
        type XcmReserveTransferFilter = Everything;
        type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
        type LocationInverter = LocationInverter<Ancestry>;
        type Origin = Origin;
        type Call = Call;
        const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
        type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    }

    impl super::mock_msg_queue::Config for Runtime {
        type Event = Event;
        type XcmExecutor = XcmExecutor<XcmConfig>;
    }
}

pub mod proxy_runtime {
    use frame_support::{
        construct_runtime, parameter_types,
        traits::{Everything, Nothing},
    };
    use pallet_xcm::XcmPassthrough;
    use polkadot_parachain::primitives::Sibling;
    use sp_core::H256;
    use sp_runtime::{testing::Header, traits::AccountIdLookup, AccountId32};
    use sp_std::prelude::*;
    use xcm::latest::prelude::*;
    use xcm_builder::{
        AccountId32Aliases, AllowUnpaidExecutionFrom, CurrencyAdapter as XcmCurrencyAdapter,
        EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds, IsConcrete, LocationInverter,
        NativeAsset, ParentIsPreset, SiblingParachainConvertsVia, SignedAccountId32AsNative,
        SignedToAccountId32, SovereignSignedViaLocation,
    };
    use xcm_executor::{Config, XcmExecutor};

    use pallet_acurast_marketplace::JobRequirements;

    use crate::mock::{AcurastAsset, AcurastAssetAmount, AcurastAssetId};

    pub type AccountId = AccountId32;
    pub type LocationToAccountId = (
        ParentIsPreset<AccountId>,
        SiblingParachainConvertsVia<Sibling, AccountId>,
        AccountId32Aliases<RelayNetwork, AccountId>,
    );
    pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, RelayNetwork>;
    pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
    pub type Block = frame_system::mocking::MockBlock<Runtime>;
    pub type XcmOriginToCallOrigin = (
        SovereignSignedViaLocation<LocationToAccountId, Origin>,
        SignedAccountId32AsNative<RelayNetwork, Origin>,
        // TODO: safety check of signature
        super::SignedAccountId32FromXcm<Origin>,
        XcmPassthrough<Origin>,
    );
    pub type LocalAssetTransactor =
        XcmCurrencyAdapter<Balances, IsConcrete<KsmLocation>, LocationToAccountId, AccountId, ()>;
    pub type XcmRouter = crate::tests::ParachainXcmRouter<MsgQueue>;
    pub type Barrier = AllowUnpaidExecutionFrom<Everything>;

    pub struct XcmConfig;

    impl Config for XcmConfig {
        type Call = Call;
        type XcmSender = XcmRouter;
        type AssetTransactor = LocalAssetTransactor;
        type OriginConverter = XcmOriginToCallOrigin;
        type IsReserve = NativeAsset;
        type IsTeleporter = ();
        type LocationInverter = LocationInverter<Ancestry>;
        type Barrier = Barrier;
        type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
        type Trader = FixedRateOfFungible<KsmPerSecond, ()>;
        type ResponseHandler = ();
        type AssetTrap = ();
        type AssetClaims = ();
        type SubscriptionService = ();
    }

    pub const MILLISECS_PER_BLOCK: u64 = 12000;
    pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

    construct_runtime!(
        pub enum Runtime where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
            Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
            MsgQueue: super::mock_msg_queue::{Pallet, Storage, Event<T>},
            PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin},
            AcurastProxy: crate::{Pallet, Call, Event<T>} = 34,
        }
    );

    parameter_types! {
    pub const BlockHashCount: u64 = 250;
    }
    parameter_types! {
        pub ExistentialDeposit: AcurastAssetAmount = 1;
        pub const MaxLocks: u32 = 50;
        pub const MaxReserves: u32 = 50;
    }
    parameter_types! {
        pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
        pub const IsRelay: bool = false;
        pub Admins: Vec<AccountId> = vec![];
    }
    parameter_types! {
        pub const UnitWeightCost: u64 = 1;
        pub KsmPerSecond: (AssetId, u128) = (Concrete(Parent.into()), 1);
        pub const MaxInstructions: u32 = 100;
    }
    parameter_types! {
        pub const AcurastParachainId: u32 = 2000;
        pub const AcurastPalletId: u8 = 40;
        pub const AcurastMarketplacePalletId: u8 = 41;
    }
    parameter_types! {
        pub const KsmLocation: MultiLocation = MultiLocation::parent();
        pub const RelayNetwork: NetworkId = NetworkId::Kusama;
        pub Ancestry: MultiLocation = Parachain(MsgQueue::parachain_id().into()).into();
    }

    impl frame_system::Config for Runtime {
        type BaseCallFilter = Everything;
        type BlockWeights = ();
        type BlockLength = ();
        type Origin = Origin;
        type Call = Call;
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = frame_support::sp_runtime::traits::BlakeTwo256;
        type AccountId = AccountId;
        type Lookup = AccountIdLookup<AccountId, ()>;
        type Header = Header;
        type Event = Event;
        type BlockHashCount = BlockHashCount;
        type DbWeight = ();
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = pallet_balances::AccountData<AcurastAssetAmount>;
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = frame_support::traits::ConstU32<16>;
    }

    impl pallet_balances::Config for Runtime {
        type Balance = AcurastAssetAmount;
        type DustRemoval = ();
        type Event = Event;
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxLocks = MaxLocks;
        type MaxReserves = MaxReserves;
        type ReserveIdentifier = [u8; 8];
    }

    impl super::mock_msg_queue::Config for Runtime {
        type Event = Event;
        type XcmExecutor = XcmExecutor<XcmConfig>;
    }

    impl pallet_xcm::Config for Runtime {
        type Event = Event;
        type SendXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
        type XcmRouter = XcmRouter;
        type ExecuteXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
        type XcmExecuteFilter = Everything;
        type XcmExecutor = XcmExecutor<XcmConfig>;
        type XcmTeleportFilter = Nothing;
        type XcmReserveTransferFilter = Everything;
        type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
        type LocationInverter = LocationInverter<Ancestry>;
        type Origin = Origin;
        type Call = Call;
        const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
        type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    }

    impl crate::Config for Runtime {
        type Event = Event;
        type RegistrationExtra = JobRequirements<AcurastAsset>;
        type AssetId = AcurastAssetId;
        type AssetAmount = AcurastAssetAmount;
        type XcmSender = XcmRouter;
        type AcurastPalletId = AcurastPalletId;
        type AcurastMarketplacePalletId = AcurastMarketplacePalletId;
        type AcurastParachainId = AcurastParachainId;
    }

    impl pallet_timestamp::Config for Runtime {
        type Moment = u64;
        type OnTimestampSet = ();
        type MinimumPeriod = MinimumPeriod;
        type WeightInfo = ();
    }
}

pub mod relay_chain {
    use frame_support::{
        construct_runtime, parameter_types,
        sp_runtime::{testing::Header, traits::IdentityLookup, AccountId32},
        traits::{Everything, Nothing},
    };
    use polkadot_parachain::primitives::Id as ParaId;
    use polkadot_runtime_parachains::{configuration, origin, shared, ump};
    use sp_core::H256;
    use xcm::latest::prelude::*;
    use xcm_builder::{
        AccountId32Aliases, AllowUnpaidExecutionFrom, ChildParachainAsNative,
        ChildParachainConvertsVia, ChildSystemParachainAsSuperuser,
        CurrencyAdapter as XcmCurrencyAdapter, FixedRateOfFungible, FixedWeightBounds, IsConcrete,
        LocationInverter, SignedAccountId32AsNative, SignedToAccountId32,
        SovereignSignedViaLocation,
    };
    use xcm_executor::{Config, XcmExecutor};

    use crate::mock::AcurastAssetAmount;

    pub type AccountId = AccountId32;
    pub type SovereignAccountOf = (
        ChildParachainConvertsVia<ParaId, AccountId>,
        AccountId32Aliases<KusamaNetwork, AccountId>,
    );
    pub type LocalAssetTransactor =
        XcmCurrencyAdapter<Balances, IsConcrete<KsmLocation>, SovereignAccountOf, AccountId, ()>;
    pub type LocalOriginConverter = (
        SovereignSignedViaLocation<SovereignAccountOf, Origin>,
        ChildParachainAsNative<origin::Origin, Origin>,
        SignedAccountId32AsNative<KusamaNetwork, Origin>,
        ChildSystemParachainAsSuperuser<ParaId, Origin>,
    );
    pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, KusamaNetwork>;
    pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
    pub type Block = frame_system::mocking::MockBlock<Runtime>;
    pub type XcmRouter = crate::tests::RelayChainXcmRouter;
    pub type Barrier = AllowUnpaidExecutionFrom<Everything>;

    pub struct XcmConfig;

    impl Config for XcmConfig {
        type Call = Call;
        type XcmSender = XcmRouter;
        type AssetTransactor = LocalAssetTransactor;
        type OriginConverter = LocalOriginConverter;
        type IsReserve = ();
        type IsTeleporter = ();
        type LocationInverter = LocationInverter<Ancestry>;
        type Barrier = Barrier;
        type Weigher = FixedWeightBounds<BaseXcmWeight, Call, MaxInstructions>;
        type Trader = FixedRateOfFungible<KsmPerSecond, ()>;
        type ResponseHandler = ();
        type AssetTrap = ();
        type AssetClaims = ();
        type SubscriptionService = ();
    }

    construct_runtime!(
        pub enum Runtime where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
            Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
            ParasOrigin: origin::{Pallet, Origin},
            ParasUmp: ump::{Pallet, Call, Storage, Event},
            XcmPallet: pallet_xcm::{Pallet, Call, Storage, Event<T>, Origin},
        }
    );

    parameter_types! {
        pub const KsmLocation: MultiLocation = Here.into();
        pub const KusamaNetwork: NetworkId = NetworkId::Kusama;
        pub const AnyNetwork: NetworkId = NetworkId::Any;
        pub Ancestry: MultiLocation = Here.into();
        pub UnitWeightCost: u64 = 1_000;
    }
    parameter_types! {
        pub const BaseXcmWeight: u64 = 1_000;
        pub KsmPerSecond: (AssetId, u128) = (Concrete(KsmLocation::get()), 1);
        pub const MaxInstructions: u32 = 100;
    }
    parameter_types! {
        pub const FirstMessageFactorPercent: u64 = 100;
    }
    parameter_types! {
        pub ExistentialDeposit: AcurastAssetAmount = 1;
        pub const MaxLocks: u32 = 50;
        pub const MaxReserves: u32 = 50;
    }
    parameter_types! {
        pub const BlockHashCount: u64 = 250;
    }

    impl frame_system::Config for Runtime {
        type BaseCallFilter = Everything;
        type BlockWeights = ();
        type BlockLength = ();
        type Origin = Origin;
        type Call = Call;
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = frame_support::sp_runtime::traits::BlakeTwo256;
        type AccountId = AccountId;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type Event = Event;
        type BlockHashCount = BlockHashCount;
        type DbWeight = ();
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = pallet_balances::AccountData<AcurastAssetAmount>;
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = frame_support::traits::ConstU32<16>;
    }

    impl pallet_balances::Config for Runtime {
        type Balance = AcurastAssetAmount;
        type DustRemoval = ();
        type Event = Event;
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type WeightInfo = ();
        type MaxLocks = MaxLocks;
        type MaxReserves = MaxReserves;
        type ReserveIdentifier = [u8; 8];
    }

    impl shared::Config for Runtime {}

    impl configuration::Config for Runtime {
        type WeightInfo = configuration::TestWeightInfo;
    }

    impl pallet_xcm::Config for Runtime {
        type Event = Event;
        type SendXcmOrigin = xcm_builder::EnsureXcmOrigin<Origin, LocalOriginToLocation>;
        type XcmRouter = XcmRouter;
        // Anyone can execute XCM messages locally...
        type ExecuteXcmOrigin = xcm_builder::EnsureXcmOrigin<Origin, LocalOriginToLocation>;
        type XcmExecuteFilter = Nothing;
        type XcmExecutor = XcmExecutor<XcmConfig>;
        type XcmTeleportFilter = Everything;
        type XcmReserveTransferFilter = Everything;
        type Weigher = FixedWeightBounds<BaseXcmWeight, Call, MaxInstructions>;
        type LocationInverter = LocationInverter<Ancestry>;
        type Origin = Origin;
        type Call = Call;
        const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
        type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    }

    impl ump::Config for Runtime {
        type Event = Event;
        type UmpSink = ump::XcmSink<XcmExecutor<XcmConfig>, Runtime>;
        type FirstMessageFactorPercent = FirstMessageFactorPercent;
        type ExecuteOverweightOrigin = frame_system::EnsureRoot<AccountId>;
        type WeightInfo = ump::TestWeightInfo;
    }

    impl origin::Config for Runtime {}
}

#[frame_support::pallet]
pub mod mock_msg_queue {
    use frame_support::pallet_prelude::*;
    use polkadot_parachain::primitives::{
        DmpMessageHandler, XcmpMessageFormat, XcmpMessageHandler,
    };
    use sp_runtime::traits::Hash;
    use xcm::latest::{ExecuteXcm, Outcome, Parent, Xcm};
    use xcm::prelude::{Parachain, XcmError};
    use xcm::VersionedXcm;
    use xcm_simulator::{ParaId, RelayBlockNumber};

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type XcmExecutor: ExecuteXcm<Self::Call>;
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn parachain_id)]
    pub(super) type ParachainId<T: Config> = StorageValue<_, ParaId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn received_dmp)]
    /// A queue of received DMP messages
    pub(super) type ReceivedDmp<T: Config> = StorageValue<_, Vec<Xcm<T::Call>>, ValueQuery>;

    impl<T: Config> Get<ParaId> for Pallet<T> {
        fn get() -> ParaId {
            Self::parachain_id()
        }
    }

    pub type MessageId = [u8; 32];

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        // XCMP
        /// Some XCM was executed OK.
        Success(Option<T::Hash>),
        /// Some XCM failed.
        Fail(Option<T::Hash>, XcmError),
        /// Bad XCM version used.
        BadVersion(Option<T::Hash>),
        /// Bad XCM format used.
        BadFormat(Option<T::Hash>),

        // DMP
        /// Downward message is invalid XCM.
        InvalidFormat(MessageId),
        /// Downward message is unsupported version of XCM.
        UnsupportedVersion(MessageId),
        /// Downward message executed with the given outcome.
        ExecutedDownward(MessageId, Outcome),
    }

    impl<T: Config> Pallet<T> {
        pub fn set_para_id(para_id: ParaId) {
            ParachainId::<T>::put(para_id);
        }

        fn handle_xcmp_message(
            sender: ParaId,
            _sent_at: RelayBlockNumber,
            xcm: VersionedXcm<T::Call>,
            max_weight: Weight,
        ) -> Result<Weight, XcmError> {
            let hash = Encode::using_encoded(&xcm, T::Hashing::hash);
            let (result, event) = match Xcm::<T::Call>::try_from(xcm) {
                Ok(xcm) => {
                    let location = (1, Parachain(sender.into()));
                    match T::XcmExecutor::execute_xcm(location, xcm, max_weight.ref_time()) {
                        Outcome::Error(e) => (Err(e.clone()), Event::Fail(Some(hash), e)),
                        Outcome::Complete(w) => {
                            (Ok(Weight::from_ref_time(w)), Event::Success(Some(hash)))
                        }
                        // As far as the caller is concerned, this was dispatched without error, so
                        // we just report the weight used.
                        Outcome::Incomplete(w, e) => {
                            (Ok(Weight::from_ref_time(w)), Event::Fail(Some(hash), e))
                        }
                    }
                }
                Err(()) => (
                    Err(XcmError::UnhandledXcmVersion),
                    Event::BadVersion(Some(hash)),
                ),
            };
            Self::deposit_event(event);
            result
        }
    }

    impl<T: Config> XcmpMessageHandler for Pallet<T> {
        fn handle_xcmp_messages<'a, I: Iterator<Item = (ParaId, RelayBlockNumber, &'a [u8])>>(
            iter: I,
            max_weight: Weight,
        ) -> Weight {
            for (sender, sent_at, data) in iter {
                let mut data_ref = data;
                let _ = XcmpMessageFormat::decode(&mut data_ref)
                    .expect("Simulator encodes with versioned xcm format; qed");

                let mut remaining_fragments = &data_ref[..];
                while !remaining_fragments.is_empty() {
                    if let Ok(xcm) = VersionedXcm::<T::Call>::decode(&mut remaining_fragments) {
                        let _ = Self::handle_xcmp_message(sender, sent_at, xcm, max_weight);
                    } else {
                        debug_assert!(false, "Invalid incoming XCMP message data");
                    }
                }
            }
            max_weight
        }
    }

    impl<T: Config> DmpMessageHandler for Pallet<T> {
        fn handle_dmp_messages(
            iter: impl Iterator<Item = (RelayBlockNumber, Vec<u8>)>,
            limit: Weight,
        ) -> Weight {
            for (_i, (_sent_at, data)) in iter.enumerate() {
                let id = sp_io::hashing::blake2_256(&data[..]);
                let maybe_msg =
                    VersionedXcm::<T::Call>::decode(&mut &data[..]).map(Xcm::<T::Call>::try_from);
                match maybe_msg {
                    Err(_) => {
                        Self::deposit_event(Event::InvalidFormat(id));
                    }
                    Ok(Err(())) => {
                        Self::deposit_event(Event::UnsupportedVersion(id));
                    }
                    Ok(Ok(x)) => {
                        let outcome =
                            T::XcmExecutor::execute_xcm(Parent, x.clone(), limit.ref_time());
                        <ReceivedDmp<T>>::append(x);
                        Self::deposit_event(Event::ExecutedDownward(id, outcome));
                    }
                }
            }
            limit
        }
    }
}

pub struct SignedAccountId32FromXcm<Origin>(PhantomData<Origin>);

impl<Origin: OriginTrait> ConvertOrigin<Origin> for SignedAccountId32FromXcm<Origin>
where
    Origin::AccountId: From<[u8; 32]>,
{
    fn convert_origin(
        origin: impl Into<MultiLocation>,
        kind: OriginKind,
    ) -> Result<Origin, MultiLocation> {
        let origin = origin.into();
        log::trace!(
            target: "xcm::origin_conversion",
            "SignedAccountId32AsNative origin: {:?}, kind: {:?}",
            origin, kind,
        );
        match (kind, origin) {
            (
                OriginKind::Xcm,
                MultiLocation {
                    parents: 1,
                    interior:
                        X2(Junction::Parachain(_para_id), Junction::AccountId32 { id, network: _ }),
                },
            ) => Ok(Origin::signed(id.into())),
            (_, origin) => Err(origin),
        }
    }
}
